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

## Three pipelines: batched vector, single textured quads, and instanced array

The backend has **three GL programs**:

1. **Vector program** — flat-coloured geometry. Vertices are design-pixel
   `(x, y, r, g, b, a)` (`FLOATS_PER_VERT = 6`); the VS maps `xy` to NDC.
   `stroke_line`/`stroke_circle`/`fill_circle`/`fill_triangle`
   push into one of two CPU-side `Vec<f32>` buffers — `tri_verts` (TRIANGLES) and
   `line_verts` (LINES). Circles are tessellated into `CIRCLE_SEGMENTS = 64` segments.
   `fill_rect`/`stroke_rect` are the trait's default compositions into triangles/lines.

2. **Texture program** — one textured quad at a time. Vertices are design-pixel
   `(x, y, u, v)` (`FLOATS_PER_TEX_VERT = 4`). Used for banner `draw_image` and
   rasterized text. Alpha blending (`SRC_ALPHA, ONE_MINUS_SRC_ALPHA`) is enabled
   only around textured draws.

3. **Array program** — instanced card posters via `draw_images` /
   `draw_images_cached`. A unit quad plus per-instance `(x, y, w, h, layer)` feeds
   a fixed-size `TEXTURE_2D_ARRAY` (layer WxH passed into `WebGl2Renderer::new`,
   depth `IMAGE_TEX_CAP = 96`). One bind + one `drawArraysInstanced` paints a whole
   rail's visible posters.

### Batching and flush order

Colored primitives accumulate across many UI calls and are drawn in as few `drawArrays`
calls as possible. The batches are flushed (`flush_color_batches`) at exactly three points,
all chosen to **preserve draw order**:

- `end_frame` — final flush.
- before textured work (`draw_image`, `draw_images`, `draw_text`, and their `_cached`
  variants) — so a poster painted after a card background actually lands on top of it.
- on `set_clip` — so geometry queued under one scissor rect isn't drawn under another.

This ordering is why `draw_card_row` (`carousel.rs`) paints a rail in **three passes** —
all card backgrounds, then one `draw_images` batch, then all borders — instead of
per-card: fills and borders each batch into a single flush around the instanced posters.

## Images: async load + caches (`image_cache.rs` + `webgl2.rs`)

`draw_image` / `draw_images` cannot block on a network fetch, so image loading is
asynchronous and missing textures **no-op** (hence the placeholder `CARD_BG` fill
drawn first). Caches:

- **`ImageCache`** (`crates/tv-ui-webgl/src/image_cache.rs`) — an LRU of decoded `HtmlImageElement`s
  (cap 192), keyed by URL, with `Loading`/`Ready`/`Failed` status. `request` kicks off an
  `<img>` load with `crossorigin=anonymous`; `html_image` returns the element once `Ready`.
- **Card array** (`TEXTURE_2D_ARRAY` + `array_slots` URL→layer LRU, cap 96) — used by
  `draw_images`. Sources are resampled into the layer size via a scratch canvas, then
  `texSubImage3D`. Eviction returns the layer index to a free list.
- **Per-URL `TEXTURE_2D` LRU** (`textures`, cap 96) — banner / odd-size `draw_image`.
  Eviction deletes the GL texture. A separate LRU (`text_textures`, cap 192) holds
  rasterized text.

The VRAM caps stay tight (~96) while decoded `<img>` sources are retained more
generously (~192) so re-upload after eviction skips the network fetch + decode.

For that to pay off, the GPU and decode LRUs must stay coherent. Hot draw paths call
`ImageCache::touch(url)` — promote-if-present, never starts a fetch.

`draw_textured_quad` (2D path) prefers an already-uploaded texture even if the source
`<img>` was LRU-evicted; otherwise it pulls the ready `<img>` and uploads it
(`texture_for`, with `UNPACK_FLIP_Y` so UVs match the design-space orientation).
The array path resamples through a scratch canvas into the fixed layer size the same way.

### Avoiding decode / upload hitches

Uploading many freshly-decoded posters in one frame causes a hitch on TV GPUs.
Mitigations:

- **Vertical rail motion:** `RailList` uses `draw_images_cached` — paint only array
  layers already resident; prefetch still decodes in the background.
- **Banner motion:** `BannerCarousel` keeps `draw_image` during the page tween so
  neighbors can upload under the per-frame budget (avoids blank slides / sudden
  pops). Ease uses a slightly slower `BANNER_TAU` so hold-chaining does not race
  through full-bleed pages.
- **Per-frame upload budget:** `WebGl2Renderer` allows at most
  `image_uploads_per_frame` new GPU uploads per `begin_frame` (default
  [`DEFAULT_IMAGE_UPLOADS_PER_FRAME`](../crates/tv-ui-webgl/src/webgl2.rs) = 2),
  shared by the array and 2D image paths. Configure via
  [`WebGl2RendererConfig`](../crates/tv-ui-webgl/src/webgl2.rs) at construction
  (or `set_image_uploads_per_frame` later). Horizontal card browse keeps
  `draw_images` (`All`) so posters appear as they decode, but sprawl across
  frames instead of one spike.
- Exact-size sources skip the scratch-canvas resample on array upload.

Once `anim_rail.is_settled()`, full `draw_images` is used (`ImageDraw::All` vs
`CachedOnly` in `carousel.rs`). `ImageDraw::CachedOnly` remains available for
other callers that want resident-only draws.

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
