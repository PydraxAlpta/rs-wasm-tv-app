# Architecture

`rs-wasm-tv-app` is a retained-mode TV/leanback UI written in Rust, compiled to
WebAssembly, and rendered with WebGL2 through raw `web-sys`. This document is the
map: what the layers are, how they talk, and where the boundaries are drawn.

For the two hardest sub-systems see the companion docs:

- [`NAVIGATION.md`](NAVIGATION.md) — the fixed-focus spatial model, hold-to-scroll, zone chaining.
- [`RENDERING.md`](RENDERING.md) — the WebGL2 backend, batching, texture/text/image caches, compositing.

## The core / glue split

The single defining decision, now expressed as **crate boundaries** rather than a
`cfg(wasm32)` gate: everything that can be pure Rust *is* pure Rust, and only the
irreducible browser code is compiled for wasm. The workspace has four crates — the
three below, plus `crates/tv-ui-web` (a second, smaller wasm-bindgen binding,
`mountCarousels`, that mounts a `tv-ui` widget onto a host-provided canvas instead of
taking over the whole screen; see "Build & hosting" below):

```
crates/tv-ui/            crates/tv-ui-webgl/          crates/tv-app/
(pure rlib)              (web-sys rlib)               (cdylib, pkg "rs-wasm-tv-app")
─────────────────────    ─────────────────────────    ───────────────────────────────
model     content        webgl2.rs   Renderer impl    lib.rs    entry, rAF loop, input
geom      Rect/Size       image_cache async <img> LRU  video.rs  VideoSink → JS PlayerAdapter
metrics   Metrics                                      utils.rs  panic hook
anim      Tween
screen    Screen/Transition/VideoSink/Ctx/Key
renderer  Renderer trait
theme     colour palette
buffer    Color
ui/       widget tree: components/ + pages/
```

`tv-ui` never mentions `web-sys`/`wasm-bindgen`/`js-sys`. That is what makes the whole
navigation/layout/animation surface testable with a plain `cargo test` (see the
`#[cfg(test)]` modules throughout, which drive the UI with a `NullSink` video stub).
`tv-ui-webgl` depends only on `tv-ui` and provides `WebGl2Renderer` given an
already-created `WebGl2RenderingContext` — it never creates or owns a canvas, so it
can in principle be mounted into any DOM. `tv-app` is the thin composition layer for
the full leanback experience: it owns the DOM, wires the sample catalog, and exposes
the wasm-bindgen entry point `setupApp`. In short: `tv-ui` + `tv-ui-webgl` are reusable
libraries; `tv-app` and `tv-ui-web` are two different products built on them.

## Three decoupling traits

Everything crosses the core/glue boundary through three traits declared in the core
and implemented in the glue:

### `Renderer` (`crates/tv-ui/src/renderer.rs`)

The stable set of drawing primitives the UI is allowed to call: `stroke_line`,
`stroke_circle`, `fill_circle`, `fill_triangle`, `draw_image`, `draw_text`. Composite
helpers (`fill_rect`, `stroke_rect`) are default methods built from those primitives,
so the UI composes rather than growing the trait. Motion/streaming extras — `set_clip`,
`prefetch_image`, `draw_image_cached` — also have safe default behaviours so a minimal
backend still works. The only production implementation is `WebGl2Renderer`.

### `Screen` + `Transition` (`crates/tv-ui/src/screen.rs`)

A screen is a full-screen view with `update` / `render` / `handle_key` /
`handle_key_up`. The app keeps a `stack: Vec<Box<dyn Screen>>`, ticks and renders only
the top screen, and a screen requests navigation by returning a `Transition`:

- `Transition::Push(Box<dyn Screen>)` — pushes a new screen (e.g. catalog → player).
- `Transition::Pop` — pops; popping the last screen empties the stack and exits the app.
- `Transition::None` — stay.

### `VideoSink` (`crates/tv-ui/src/screen.rs`)

Abstracts `<video>` so screens hold no `web-sys`. `load_and_play` / `play` / `pause` /
`is_paused` / `current_time` / `duration` / `seek` / `set_visible`. On wasm it is
`JsPlayerSink`, which forwards each call to a JS `PlayerAdapter` (`www/apps/tv-app/src/player.ts`)
that actually owns the `<video>` element. Tests use a trivial `NullSink`.

### `Ctx<'a>`

The bundle handed to every `update`/`render`/`handle_key`: `catalog: &Catalog`,
`metrics: &Metrics`, `video: &mut dyn VideoSink`. This is what lets widgets read content
and geometry and drive playback without any globals.

## The widget layer (`crates/tv-ui/src/ui/`)

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
| `NavBar` | `nav_bar.rs` | Tabs + brand, both caller-supplied (`NavBar::new(brand, labels)`), animated underline `Tween`. |
| `BannerCarousel` | `banner.rs` | Wrapping hero strip. Drawn as a **zero-flex overlay** with a reveal `Tween`, so collapsing it doesn't reflow the rails. |
| `HCarousel` + `draw_card_row` | `carousel.rs` | The horizontal index/tween engine (clamped or wrapping) and the batched three-pass card-row painter. Also holds the shared timing/threshold constants. |
| `RailList` | `rail_list.rs` | The vertical leanback rail stack: fixed focus anchor, per-rail remembered column, lazy rail batches, hold-chaining. |
| `MetadataOverlay` | `metadata_overlay.rs` | Slide-up details page (poster + filler metadata + focused Play button). |
| `card` | `card.rs` | Poster tile + multi-layer focus-ring primitives. |
| `Header` | `header.rs` | Static title band (available building block). |

### Pages (`crates/tv-ui/src/ui/pages/`)

- **`MainShell`** — the app root `Screen`. `MainShell::new(brand, tab_labels)` owns the
  `NavBar` and a lazily-populated `Vec<Option<CatalogPage>>` (index 0 loads eagerly, the
  rest on first visit); `tv-app` supplies its own brand/tabs at the one production call
  site — nothing about tab identity or count is baked into `tv-ui`. A `slide`
  `Tween` horizontally translates the active tab's page into view; each page strip is
  clipped with `set_clip` so neighbours don't bleed while sliding. `ShellFocus` toggles
  between the nav and the content; Up from the top content zone moves to the nav, Down
  from the nav enters content (and can hold-chain straight into the rails).
- **`CatalogPage`** — one tab's browse content: a `BannerCarousel`, a `RailList`, a
  `MetadataOverlay`, and the `FocusScope` that switches between banner and rails. Handles
  vertical hold-traverse across the banner ↔ rails ↔ (nav) boundaries. Also implements a
  thin `Screen` adapter used only by its own unit tests.
- **`PlayerScreen`** — the video screen. `PlayerScreen::new(title, url)` takes both from the
  caller; on first `update` it makes the video visible and starts playback at `url`. It
  renders a bottom control block (title, scrub
  bar with a knob, state line, key hints) that autohides after `HIDE_AFTER_SECS` of idle
  via a fade `Tween`. Left/Right seek ±5 s, Enter toggles play/pause, Back returns to
  browse.

## The app object (`crates/tv-app/src/lib.rs`)

`App` is the wasm-side owner: `renderer`, `video`, `catalog`, `metrics`, the screen
`stack`, timing state, and the perf-HUD element. Two behaviours are worth knowing:

- **Split-borrow in `tick`/`handle_key`.** `Ctx` needs `catalog`/`metrics`/`video` while
  the renderer is borrowed separately. The code destructures `self` into its fields so the
  borrow checker sees disjoint borrows. Preserve this pattern when editing `App`.
- **Exit on empty stack.** A `Back` that reaches the root screen returns `Pop`; the app
  pops, sees the stack is empty, and calls `exit_app()`. A `Back` consumed by an open
  overlay is turned into `Transition::None` and must *not* exit.
- **Activate bubbles up, doesn't resolve itself.** `Enter` on the overlay's Play button
  doesn't push `PlayerScreen` directly — the overlay only knows the card's own render data
  (`id`/`title`/`image_url`), not what "play" should mean. It returns
  `Transition::Activate(ActivatedItem { id, title, image_url })`, and `App::handle_key`
  (the driving app, not the library) resolves `id` to a video URL via
  `content::video_url_for` and pushes `PlayerScreen::new(title, url)`. Same shape as
  `tv-ui-web`'s `select`/`focuschange` `CustomEvent`s: components report, the app decides.

## Content model (`crates/tv-ui/src/model.rs`)

`Catalog { banners: Vec<BannerSlide>, rails: Vec<Rail> }`, `Rail { title, cards }`,
`Card { id, title, image_url }` — plain struct defs only; `tv-ui` has no demo data of its
own. Adding content fields here requires no changes anywhere else. `tv-app`'s
`crates/tv-app/src/content.rs` builds the actual demo content (`sample_catalog()`: 5
banners, 20 rails × 20 cards, stable picsum seeds, one shared `SAMPLE_VIDEO_URL` resolved
via `video_url_for`); `tv-ui`'s own tests use a separate generic `#[cfg(test)]` fixture in
`crates/tv-ui/src/test_support.rs` instead.

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

- **Rust → wasm:** `wasm-pack build crates/tv-app` emits `crates/tv-app/pkg/`; `wasm-pack
  build crates/tv-ui-web` emits `crates/tv-ui-web/pkg/` (both gitignored) — `tv-ui`/
  `tv-ui-webgl` are plain library crates pulled in as path dependencies, never built with
  `wasm-pack` directly. `mise run dev` (default: both projects, `mise run dev <project>` for
  one) runs `cargo watch → wasm-pack build ... --dev` alongside each app's Vite dev server.
- **JS host:** `www/` is a pnpm workspace with two member apps, `www/apps/tv-app` and
  `www/apps/embed`. Each app's `vite.config.ts` aliases its crate's package name
  (`rs-wasm-tv-app` → `../../../crates/tv-app/pkg`, `tv-ui-web` → `../../../crates/tv-ui-web/pkg`)
  and sets `base: "./"` for Tizen relative loading; a `watch-pkg` plugin reloads on wasm
  rebuild. A shared `www/tsconfig.base.json` holds common compiler options.
- **GitHub Pages:** `mise run docs` (default `tv-app`, or `mise run docs embed`) builds the
  chosen project and copies its `dist/` → `docs/` — `docs/` can only ever hold one project's
  output at a time, unlike `dev`/`build`/`preview` which default to running both. That
  `docs/` is generated output — do not hand-edit it (this doc set lives in `doc/`).
- **Tizen:** `tizen/config.xml` targets `tv-samsung`, landscape, 1920×1080. A `.wgt` is
  packaged manually with Tizen Studio's CLI against `www/apps/tv-app/dist` + this config.
