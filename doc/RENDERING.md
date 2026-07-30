# Rendering pipeline

The UI is drawn entirely with WebGL2 through raw `web-sys`. The core (`tv-ui`) never
touches GL — it calls the `Renderer` trait (`crates/tv-ui/src/renderer.rs`); the only
production backend is `WebGl2Renderer` (`crates/tv-ui-webgl/src/webgl2.rs`). This document
covers how that backend turns primitive calls into draw calls, and how it composits over
the `<video>` underlay.

## Design space & the letterboxed stage

All UI geometry is in a fixed **1920×1080 design space**, expressed as a `Rect` on `Ctx.design`
rather than a `tv-ui`-owned constant — each app defines its own `DESIGN_WIDTH`/`DESIGN_HEIGHT`
locals (`crates/tv-app/src/lib.rs`) and builds the `Rect` it passes in. The canvas backing
store is exactly that size; CSS scales the whole 16:9 stage to
fit the viewport with black letterboxing (`www/apps/tv-app/src/style.css`). Because both the design
space and the stage are 16:9, scaling never distorts and no per-frame DPR math is needed.
`WebGl2Renderer`'s vertex shaders map design pixels → clip space `[-1, 1]` via a
`u_resolution` uniform (Y flipped: design Y grows downward, GL Y grows upward).
VBOs store design-space coordinates; only `resize` needs to update the uniform.

## The DOM the entry point builds (`setup_app`)

`setup_app(root, player)` sets `root.innerHTML` to a `.stage` containing, in z-order:

```
<video id="player-video" class="video-underlay">   z 0  — JS-owned playback surface
<canvas id="ui" class="ui-canvas">                  z 1  — transparent WebGL2 overlay
<div id="perf-hud" class="perf-hud">                z 2  — FPS / frame / work HUD
```

The WebGL2 context is created with `alpha: true`, `antialias: false`, `depth: false`,
`stencil: false`, `premultipliedAlpha: false`. Alpha + no depth/stencil is what lets the
canvas be a see-through overlay: each frame is cleared to `rgba(0,0,0,0)` so the video
shows through wherever the UI paints nothing (the whole point in `PlayerScreen`).

## Two pipelines: batched vector, and textured quads

The backend has **two GL programs**:

1. **Vector program** — flat-coloured geometry. Vertices are design-pixel
   `(x, y, r, g, b, a)` (`FLOATS_PER_VERT = 6`); the VS maps `xy` to NDC.
   `stroke_line`/`stroke_circle`/`fill_circle`/`fill_triangle`
   push into one of two CPU-side `Vec<f32>` buffers — `tri_verts` (TRIANGLES) and
   `line_verts` (LINES). Circles are tessellated into `CIRCLE_SEGMENTS = 64` segments.
   `fill_rect`/`stroke_rect` are the trait's default compositions into triangles/lines.

2. **Texture program** — one textured quad at a time. Vertices are design-pixel
   `(x, y, u, v)` (`FLOATS_PER_TEX_VERT = 4`). Used for both images and rasterized text.
   Alpha blending (`SRC_ALPHA, ONE_MINUS_SRC_ALPHA`) is enabled only around textured draws.

### Batching and flush order

Colored primitives accumulate across many UI calls and are drawn in as few `drawArrays`
calls as possible. The batches are flushed (`flush_color_batches`) at exactly three points,
all chosen to **preserve draw order**:

- `end_frame` — final flush.
- before **every** textured quad (`draw_image`, `draw_image_cached`, `draw_text`) — so a
  poster painted after a card background actually lands on top of it.
- on `set_clip` — so geometry queued under one scissor rect isn't drawn under another.

This ordering is why `draw_card_row` (`carousel.rs`) paints a rail in **three passes** —
all card backgrounds, then all images, then all borders — instead of per-card: it lets the
fills and borders each batch into a single flush around the run of textured posters.

## Images: async load + double LRU (`image_cache.rs` + `webgl2.rs`)

`draw_image(url)` cannot block on a network fetch, so image loading is asynchronous and the
call **no-ops until the texture exists** (hence the placeholder `CARD_BG` fill drawn first).
Two caches sit behind it:

- **`ImageCache`** (`crates/tv-ui-webgl/src/image_cache.rs`) — an LRU of decoded `HtmlImageElement`s
  (cap 192), keyed by URL, with `Loading`/`Ready`/`Failed` status. `request` kicks off an
  `<img>` load with `crossorigin=anonymous`; `html_image` returns the element once `Ready`.
- **GPU texture LRU** (`WebGl2Renderer.textures`, cap 96) — uploaded `WebGlTexture`s keyed
  by URL. Eviction deletes the GL texture. A separate LRU (`text_textures`, cap 192) holds
  rasterized text.

The two caps are deliberately asymmetric: VRAM is the scarce resource on a TV, so the
texture cache is kept tight (visible rails + prefetch window, ~96), while decoded `<img>`
sources — cheap in system RAM — are retained more generously (~192) so re-upload after a
texture eviction (e.g. reverse-scroll, tab return) skips the network fetch + decode.

For that to pay off, the two LRUs must stay coherent. The hot draw path returns as soon as
the GPU texture is found and never re-reads `ImageCache`, so a poster that's on screen every
frame would otherwise drift toward eviction in the image cache while its texture stays hot.
To prevent that, each textured draw calls `ImageCache::touch(url)` — a promote-if-present
that reorders the image LRU to match on-screen usage but never starts a fetch (the upload
path already promotes via `html_image`).

`draw_textured_quad` prefers an already-uploaded texture even if the source `<img>` was
LRU-evicted; otherwise it pulls the ready `<img>` and uploads it (`texture_for`, with
`UNPACK_FLIP_Y` so UVs match the design-space orientation).

### Avoiding decode hitches during motion

Uploading a freshly-decoded poster mid-scroll causes a frame hitch. So during rail motion
`RailList`:

- calls `prefetch_image` on nearby posters (kicks off the async decode without drawing), and
- draws with `draw_image_cached` — which paints **only** textures already resident on the
  GPU and skips everything else.

Once `anim_rail.is_settled()`, it switches back to full `draw_image` (`ImageDraw::All` vs
`CachedOnly` in `carousel.rs`). The banner similarly guards its slides.

## Text: rasterize on a 2D canvas, cache, upload

`draw_text` rasterizes the string with an offscreen `CanvasRenderingContext2d`
(`{size}px sans-serif`, `textBaseline = "top"`, `measureText` for width, height padded to
`size * 1.25` for descenders), then uploads that canvas as a texture. Results are cached by
the key `"{size}px|r,g,b,a|text"`, so repeated labels are drawn from cache. This keeps glyph
rendering entirely inside the GL path with no font files in the wasm bundle. The synopsis in
`MetadataOverlay` gets a small word-wrap helper on top (`draw_wrapped_text`) with a fixed
line budget and ellipsis.

## Clipping (`set_clip`)

`set_clip(Some(rect))` flushes pending geometry then enables `SCISSOR_TEST` with the rect
converted to GL's bottom-left origin (`gl_y = height - y - h`); `set_clip(None)` disables it.
`MainShell` uses this to clip each sliding tab-page strip to its own rectangle so content
never bleeds into the neighbouring page or off-screen during the horizontal slide.

## The frame loop & perf HUD (`App::tick`)

```
begin_frame(CLEAR = rgba 0,0,0,0)   clear transparent, reset batch buffers, scissor off
top.update(dt, ctx)                 (before render) advance state
top.render(renderer, ctx)           widget tree issues Renderer calls
end_frame()                         flush remaining batches
```

`dt` comes from `performance.now()` deltas. The HUD tracks an exponential moving average
(`PERF_SMOOTH = 0.05`) of frame time and in-frame work time and rewrites its DOM text every
`HUD_REFRESH_MS = 250 ms` — kept as a plain DOM element on purpose, outside the GL path, so
it never pollutes the text-texture cache.

## Adding a new draw backend

Implement `Renderer` for a new type and hand it to `App`. Only the six primitives are
mandatory; `fill_rect`/`stroke_rect` come free, and `set_clip`/`prefetch_image`/
`draw_image_cached` have safe defaults (no clip / no-op / fall back to `draw_image`). Nothing
in `ui`, `model`, or `screen` needs to change — that is the whole point of the trait
boundary.
