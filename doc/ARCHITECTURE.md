# Architecture

`rs-wasm-leanback` is a retained-mode TV/leanback UI written in Rust, compiled to
WebAssembly, and rendered with WebGL2 through raw `web-sys`. This document is the
map: what the layers are, how they talk, and where the boundaries are drawn.

For the two hardest sub-systems see the companion docs:

- [`NAVIGATION.md`](NAVIGATION.md) — the fixed-focus spatial model, hold-to-scroll, zone chaining.
- [`RENDERING.md`](RENDERING.md) — the WebGL2 backend, batching, texture/text/image caches, compositing.

## The core / glue split

The single defining decision (`src/lib.rs`): everything that can be pure Rust *is*
pure Rust, and only the irreducible browser code is compiled for wasm.

```
Platform-agnostic core (unit-testable off-wasm)      Browser glue (cfg(wasm32))
──────────────────────────────────────────────      ──────────────────────────
model     content: Card / Rail / BannerSlide         wasm/mod.rs      entry, rAF loop, input
geom      Rect / Size / Insets                        wasm/webgl2.rs   Renderer impl
metrics   Metrics (card sizes, band heights)          wasm/video.rs    VideoSink → JS PlayerAdapter
anim      Tween (exponential smoothing)               wasm/image_cache async <img> LRU
screen    Screen / Transition / VideoSink / Ctx / Key utils.rs         panic hook
renderer  Renderer trait (draw primitives)
theme     colour palette
buffer    Color
ui/       widget tree: components/ + pages/
```

The `ui`, `model`, `metrics`, `anim`, `screen`, `renderer` modules never mention
`web-sys`. That is what makes the whole navigation/layout/animation surface testable
with a plain `cargo test` (see the `#[cfg(test)]` modules throughout, which drive the
UI with a `NullSink` video stub).

## Three decoupling traits

Everything crosses the core/glue boundary through three traits declared in the core
and implemented in the glue:

### `Renderer` (`src/renderer.rs`)

The stable set of drawing primitives the UI is allowed to call: `stroke_line`,
`stroke_circle`, `fill_circle`, `fill_triangle`, `draw_image`, `draw_text`. Composite
helpers (`fill_rect`, `stroke_rect`) are default methods built from those primitives,
so the UI composes rather than growing the trait. Motion/streaming extras — `set_clip`,
`prefetch_image`, `draw_image_cached` — also have safe default behaviours so a minimal
backend still works. The only production implementation is `WebGl2Renderer`.

### `Screen` + `Transition` (`src/screen.rs`)

A screen is a full-screen view with `update` / `render` / `handle_key` /
`handle_key_up`. The app keeps a `stack: Vec<Box<dyn Screen>>`, ticks and renders only
the top screen, and a screen requests navigation by returning a `Transition`:

- `Transition::Push(Box<dyn Screen>)` — pushes a new screen (e.g. catalog → player).
- `Transition::Pop` — pops; popping the last screen empties the stack and exits the app.
- `Transition::None` — stay.

### `VideoSink` (`src/screen.rs`)

Abstracts `<video>` so screens hold no `web-sys`. `load_and_play` / `play` / `pause` /
`is_paused` / `current_time` / `duration` / `seek` / `set_visible`. On wasm it is
`JsPlayerSink`, which forwards each call to a JS `PlayerAdapter` (`www/src/player.ts`)
that actually owns the `<video>` element. Tests use a trivial `NullSink`.

### `Ctx<'a>`

The bundle handed to every `update`/`render`/`handle_key`: `catalog: &Catalog`,
`metrics: &Metrics`, `video: &mut dyn VideoSink`. This is what lets widgets read content
and geometry and drive playback without any globals.

## The widget layer (`src/ui/`)

The UI is a retained-mode widget tree. `ui/components/` holds reusable pieces;
`ui/pages/` holds the full-screen views that sit on the screen stack.

### `Widget` trait (`components/widget.rs`)

```
flex()      -> Flex            how to size on the parent's main axis
measure()   -> Size            intrinsic size for Flex::Hug
layout(Rect)                   parent assigns this node its bounds
update(dt, ctx)                advance animation/state
render(r, ctx)                 paint (immutable — geometry already resolved)
handle_key(key, ctx)    -> FocusResult
handle_key_up(key, ctx) -> FocusResult
bounds()    -> Rect
```

- **`Flex`** — `Fixed(px)`, `Grow(weight)`, or `Hug` (size to `measure`).
- **`FocusResult`** — how a focused child reports back: `Handled`, `MoveOut(Key)` (focus
  should leave in a direction — the parent decides the next target), `Activate` (Enter),
  or `Ignored`.

`tick_widget` runs a layout → update → render pass for a screen-rooted tree.

### Containers (`components/containers.rs`)

`Column`, `Row` (flex main-axis layout with `Fixed`/`Grow`/`Hug` resolution), `Padding`,
`SafeArea` (applies TV safe-area insets from `Metrics`), `Stack` (overlay in paint order),
`Spacer`. Because some pages own **typed, non-boxed** children (so they can call
type-specific methods), the free function `layout_column(bounds, gap, &mut [&mut dyn Widget])`
provides the same column algorithm without requiring `Box<dyn Widget>` ownership.

### Focus routing (`components/focus.rs`)

`FocusScope` is an ordered focus group. It routes a key to the active child; on a
vertical `MoveOut` it walks to the previous/next sibling, and only bubbles `MoveOut` to
its own parent when it is already at the edge. `FocusZone { Banner, Rails }` is the
catalog page's two-zone abstraction, with `index_from_zone` / `zone_from_index` mapping
to the `FocusScope` child index.

### Content widgets

| Widget | File | Role |
| --- | --- | --- |
| `NavBar` | `nav_bar.rs` | Top tabs (Home/Movies/Shows), brand, animated underline `Tween`. |
| `BannerCarousel` | `banner.rs` | Wrapping hero strip. Drawn as a **zero-flex overlay** with a reveal `Tween`, so collapsing it doesn't reflow the rails. |
| `HCarousel` + `draw_card_row` | `carousel.rs` | The horizontal index/tween engine (clamped or wrapping) and the batched three-pass card-row painter. Also holds the shared timing/threshold constants. |
| `RailList` | `rail_list.rs` | The vertical leanback rail stack: fixed focus anchor, per-rail remembered column, lazy rail batches, hold-chaining. |
| `MetadataOverlay` | `metadata_overlay.rs` | Slide-up details page (poster + filler metadata + focused Play button). |
| `card` | `card.rs` | Poster tile + multi-layer focus-ring primitives. |
| `Header` | `header.rs` | Static title band (available building block). |

### Pages (`src/ui/pages/`)

- **`MainShell`** — the app root `Screen`. Owns the `NavBar` and a lazily-populated
  `[Option<CatalogPage>; 3]` (Home loads eagerly, the others on first visit). A `slide`
  `Tween` horizontally translates the active tab's page into view; each page strip is
  clipped with `set_clip` so neighbours don't bleed while sliding. `ShellFocus` toggles
  between the nav and the content; Up from the top content zone moves to the nav, Down
  from the nav enters content (and can hold-chain straight into the rails).
- **`CatalogPage`** — one tab's browse content: a `BannerCarousel`, a `RailList`, a
  `MetadataOverlay`, and the `FocusScope` that switches between banner and rails. Handles
  vertical hold-traverse across the banner ↔ rails ↔ (nav) boundaries. Also implements a
  thin `Screen` adapter used only by its own unit tests.
- **`PlayerScreen`** — the video screen. On first `update` it makes the video visible and
  starts playback (`SAMPLE_VIDEO_URL`); it renders a bottom control block (title, scrub
  bar with a knob, state line, key hints) that autohides after `HIDE_AFTER_SECS` of idle
  via a fade `Tween`. Left/Right seek ±5 s, Enter toggles play/pause, Back returns to
  browse.

## The app object (`src/wasm/mod.rs`)

`App` is the wasm-side owner: `renderer`, `video`, `catalog`, `metrics`, the screen
`stack`, timing state, and the perf-HUD element. Two behaviours are worth knowing:

- **Split-borrow in `tick`/`handle_key`.** `Ctx` needs `catalog`/`metrics`/`video` while
  the renderer is borrowed separately. The code destructures `self` into its fields so the
  borrow checker sees disjoint borrows. Preserve this pattern when editing `App`.
- **Exit on empty stack.** A `Back` that reaches the root screen returns `Pop`; the app
  pops, sees the stack is empty, and calls `exit_app()`. A `Back` consumed by an open
  overlay is turned into `Transition::None` and must *not* exit.

## Content model (`src/model.rs`)

`Catalog { banners: Vec<BannerSlide>, rails: Vec<Rail> }`, `Rail { title, cards }`,
`Card { id, title, image_url }`. `Catalog::sample()` builds demo content (5 banners,
20 rails × 20 cards) with stable picsum seeds and one shared sample video URL. Adding
content here requires no changes anywhere else.

## Data flow per frame

```
requestAnimationFrame(ts)
  └─ App::tick(now)
       dt = now - last_ts
       Ctx { catalog, metrics, video }         (split-borrow; renderer held separately)
       top.update(dt, ctx)                      advance tweens, hold-chaining, lazy loads
       renderer.begin_frame(transparent clear)
       top.render(renderer, ctx)                widget tree paints via Renderer primitives
       renderer.end_frame()                     flush batched vector geometry
       update_perf_hud(...)
```

Input is event-driven and separate: `keydown`/`keyup` → `map_key` → `App::handle_key` /
`handle_key_up` → the top screen's key handlers → possibly a `Transition`.

## Build & hosting

- **Rust → wasm:** `wasm-pack` emits `pkg/` (gitignored). `mise run dev` runs
  `cargo watch → wasm-pack build --dev` alongside the Vite dev server.
- **JS host:** `www/` (Vite + TS). `vite.config.ts` aliases the package name
  `rs-wasm-leanback` to `../pkg`, uses `vite-plugin-wasm`, and sets `base: "./"` for
  Tizen relative loading. A `watch-pkg` plugin reloads on wasm rebuild.
- **GitHub Pages:** `mise run docs` builds and copies `www/dist` → `docs/`. That `docs/`
  is generated output — do not hand-edit it (this doc set lives in `doc/`).
- **Tizen:** `tizen/config.xml` targets `tv-samsung`, landscape, 1920×1080. A `.wgt` is
  packaged manually with Tizen Studio's CLI against `www/dist` + this config.
