# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`rs-wasm-tv-app` is a retained-mode **TV/leanback UI** (Netflix-style nav + hero banner + carousels + metadata overlay + video player) written in Rust, compiled to WebAssembly, and rendered entirely with **WebGL2 via raw `web-sys`**. Remote/d-pad keys drive a fixed-focus spatial navigation model. `tizen/config.xml` targets Samsung Tizen TVs (1920×1080).

This project was generated from a `rust-wasm-vite` cargo-generate template (a counter starter); the scaffolding (`www/` wiring, `mise.toml`, wasm glue pattern) comes from there, but the app itself is now the leanback UI described here.

The Rust side is a **Cargo workspace** of four crates under `crates/`, so the carousel/navigation logic and the WebGL2 renderer are reusable libraries rather than being locked inside one app binary:

- **`tv-ui`** (`crates/tv-ui/`) — pure Rust: `model`, `geom`, `metrics`, `anim`, `screen`, `ui`, `renderer`, `theme`, `buffer`. No wasm/web-sys/js-sys deps; unit-testable with plain `cargo test`.
- **`tv-ui-webgl`** (`crates/tv-ui-webgl/`) — the `WebGl2Renderer` (implements `tv_ui::Renderer`) and `ImageCache`, given an already-created `WebGl2RenderingContext` (it never creates or owns a canvas). Depends only on `tv-ui`.
- **`tv-app`** (`crates/tv-app/`, package name `rs-wasm-tv-app`) — the thin composition layer for the full leanback experience: DOM setup, the rAF loop, input mapping, the JS `PlayerAdapter`/`VideoSink` bridge, and the sample catalog. Has a `#[wasm_bindgen]` entry point, `setupApp`.
- **`tv-ui-web`** (`crates/tv-ui-web/`) — a second, smaller wasm-bindgen binding: mounts a handful of `tv-ui` widgets (currently just carousel rails, via `mountCarousels`) onto a host-provided `<canvas>`, for embedding into an existing DOM page rather than taking over the whole screen. Named generically so more `mount*` exports can be added later.

`tv-ui` + `tv-ui-webgl` have no video/JS coupling and no app-specific assumptions, so they're shared by both wasm-bindgen crates above; each of `tv-app`/`tv-ui-web` is one product built on top of them, not the only one that could be.

Two Vite + TypeScript apps live under `www/apps/` (a pnpm workspace): **`tv-app`** hosts `crates/tv-app`'s `setupApp` and owns `<video>` playback; **`embed`** hosts `crates/tv-ui-web`'s `mountCarousels` inside an otherwise ordinary page, to demonstrate the library use case.

Deeper design docs live in [`doc/`](doc/): [`ARCHITECTURE.md`](doc/ARCHITECTURE.md), [`NAVIGATION.md`](doc/NAVIGATION.md), [`RENDERING.md`](doc/RENDERING.md). (Note: the `docs/` directory — plural — is the generated GitHub Pages build output and is overwritten by `mise run docs`; hand-written docs go in `doc/`.)

## Commands

Tasks are defined in `mise.toml` (mise is the task runner; pnpm is the JS package manager, in a workspace rooted at `www/` with member apps `www/apps/tv-app` and `www/apps/embed`).

`dev`, `build`, and `preview` all take an optional `project` argument — `tv-app`, `embed`, or `both` (the default) — and dispatch to per-project tasks (`dev:tv-app`, `build:embed`, …), each with its own `depends` chain and `sources`/`outputs`, so re-running with nothing changed is a no-op (`sources up-to-date, skipping`). `docs` is the exception: it can only ever publish one project into the single `docs/` directory, so it takes just `tv-app` (default) or `embed`, no `both`.

```bash
# First-time setup
mise install                       # Node + pnpm from mise.toml
cd www && pnpm install && cd ..    # installs both apps' deps (pnpm workspace)

# Develop: wasm-pack --dev (watch) + Vite dev server, in parallel, for both apps by default
mise run dev                       # = dev:tv-app ::: dev:embed
mise run dev tv-app                # just the full leanback app
mise run dev embed                 # just the carousel-in-existing-DOM demo

# Build / preview production (same project argument, same default)
mise run build                     # release wasm → crates/{tv-app,tv-ui-web}/pkg/, then tsc && vite build for both apps
mise run preview                   # serve both apps' production builds
mise run docs                      # build tv-app (default) and copy its dist → docs/ for GitHub Pages
mise run docs embed                # build embed instead and publish that
```

Prerequisites: Rust toolchain, `wasm-pack`, `cargo-watch` (for `mise run dev`), and the `wasm32-unknown-unknown` target. rust-analyzer is pinned to that target with `allFeatures` in `.zed/settings.json`.

### Tests

There is no mise task for tests. The real test suite is the inline `#[cfg(test)]` modules throughout the `tv-ui` crate, which run off-wasm:

```bash
cargo test                         # runs the whole workspace's platform-agnostic unit tests
cargo test -p tv-ui <name>         # single test, e.g. cargo test -p tv-ui hold_down_chains_before_settle
```

`crates/tv-app/tests/web.rs` is a `wasm-bindgen-test` browser harness (currently just a placeholder) and is gated `#![cfg(target_arch = "wasm32")]`, so it runs only under `wasm-pack test crates/tv-app --headless --firefox` (or `--chrome`), not plain `cargo test`.

### Lint / format

No custom config; use stock `cargo fmt` / `cargo clippy --workspace`. TypeScript is type-checked by `tsc` as part of `pnpm build`.

## Architecture

The defining decision (`crates/tv-ui/src/lib.rs`): a strict split between **platform-agnostic core logic** (`tv-ui`) and **wasm browser glue** (`tv-ui-webgl` + `tv-app`), now expressed as crate boundaries rather than a `cfg(wasm32)` gate. Everything in `model`, `geom`, `metrics`, `anim`, `screen`, `ui`, `renderer`, `theme`, `buffer` lives in `tv-ui` and is pure Rust, unit-testable off-wasm; it has zero wasm-bindgen/web-sys/js-sys deps.

Three traits decouple the layers:

- **`Renderer`** (`crates/tv-ui/src/renderer.rs`) — the pluggable draw backend (primitives only: lines, circles, triangles, `draw_image`, `draw_text`; `fill_rect`/`stroke_rect` are default compositions, plus `set_clip`, `prefetch_image`, and `draw_image_cached` for motion). UI never touches GL directly. Implemented by `WebGl2Renderer` (`crates/tv-ui-webgl/src/webgl2.rs`).
- **`Screen` + `Transition`** (`crates/tv-ui/src/screen.rs`) — the screen-stack router. `App` (in `tv-app`) holds `stack: Vec<Box<dyn Screen>>`, renders only the top screen, and screens request navigation by returning `Transition::{None, Push, Pop}`. `Screen` also has `handle_key_up` for hold-to-scroll release.
- **`VideoSink`** (`crates/tv-ui/src/screen.rs`) — abstracts `<video>` playback so screens hold no `web-sys`. In `tv-app` it is backed by `JsPlayerSink` (`crates/tv-app/src/video.rs`), which forwards to a JS `PlayerAdapter` that owns the `<video>` element (`www/apps/tv-app/src/player.ts`); tests use a `NullSink` stub (a `NullVideoSink` also lives in `tv-ui::screen` for external consumers like `tv-ui-web`). This wiring is deliberately kept in `tv-app` only — `tv-ui`/`tv-ui-webgl` carry no video/JS coupling.

`Ctx<'a>` (`crates/tv-ui/src/screen.rs`) bundles `catalog`, `metrics`, and `&mut dyn VideoSink`, and is handed to every `update`/`render`/`handle_key` call.

### The widget layer (`crates/tv-ui/src/ui/`)

The UI is a retained-mode **widget tree**, split into reusable `components/` and full-screen `pages/`.

- **`Widget` trait** (`ui/components/widget.rs`) — `flex()` / `measure()` / `layout(bounds)` / `update` / `render` / `handle_key` / `handle_key_up` / `bounds()`. Parents assign each child a `Rect` via `layout`. `Flex::{Fixed, Grow, Hug}` drives sizing. `FocusResult::{Handled, MoveOut(Key), Activate, Ignored}` is how a focused widget reports key routing back to its parent.
- **Containers** (`components/containers.rs`) — `Column`, `Row`, `Padding`, `SafeArea`, `Stack`, `Spacer`, plus the free function `layout_column` used by pages that own typed (non-boxed) children and still want column geometry.
- **Focus routing** (`components/focus.rs`) — `FocusScope` routes keys to its active child and walks siblings on vertical `MoveOut`, bubbling `MoveOut` to the parent at the edge. `FocusZone` (Banner / Rails) is the catalog page's two-zone model.
- **Content widgets** — `NavBar` (top tabs Home/Movies/Shows + animated underline), `BannerCarousel` (wrapping hero strip drawn as a zero-flex overlay with a reveal tween), `HCarousel` + `draw_card_row` (`components/carousel.rs`, the horizontal index/tween engine and batched row painter), `RailList` (the vertical leanback rail stack), `MetadataOverlay` (slide-up details page with a Play action), `card` (poster + focus-ring primitives), `Header`.
- **Pages** (`ui/pages/`) — `MainShell` (app root Screen), `CatalogPage` (one tab's banner+rails+overlay), `PlayerScreen` (video underlay + autohiding control chrome). `MainShell`/`PlayerScreen` are the `Screen`s on the stack; `CatalogPage` also has a thin `Screen` adapter used only by its own unit tests.

`MainShell` owns the `NavBar` plus a lazily-populated `[Option<CatalogPage>; 3]` and a `Tween` `slide` that horizontally translates the active page into view when the tab changes. It clips each page strip with `set_clip` so neighbours don't bleed during the slide.

### The leanback focus model (non-obvious — read `metrics.rs` + `ui/components/rail_list.rs` + `ui/components/carousel.rs` + `anim.rs` together)

Fixed focus-anchor / moving-content: the focus ring is drawn at a fixed screen position (content left edge, `metrics.focus_x`) and the content grid slides behind it. In `RailList::render`, a card's X = `focus_x + (col - anim_col) * card_step` and a row's Y = `focus_y + (rail - anim_rail) * rail_step` — when the animated index equals the focused index, the focused card lands on the anchor. `RailList` tracks integer `focus_rail` plus a **per-rail remembered column** (`focus_col: Vec<usize>`), drives fractional animated indices with `Tween`s + an `HCarousel`, and **snaps** the horizontal position when switching rails so it doesn't slide sideways.

Hold-to-scroll is app-driven (OS key-repeat is ignored via `event.repeat()`): a held direction keeps `HOLD_AHEAD` units of runway ahead of the tween so motion never settles between cards, and release (`handle_key_up`) eases out forward, coasting one extra step when the natural stop is too close (`release_ease`). Taps under `HOLD_SCROLL_DELAY` move exactly one step. Vertical hold also *chains across zones* (rails ↔ banner ↔ nav) with a `BOUNDARY_DWELL` pause at each edge — see [`doc/NAVIGATION.md`](doc/NAVIGATION.md).

Rails lazy-reveal in batches of `RAIL_BATCH` (5) as focus nears the end of what's loaded. `Enter` on a card/banner opens the `MetadataOverlay`; `Enter` on the overlay pushes `PlayerScreen`.

`Metrics::tv()` in `crates/tv-ui/src/metrics.rs` is the single knob for card size / spacing / band heights (`Layout` is a back-compat type alias for `Metrics`). Pure geometry (`Rect`/`Size`/`Insets`) lives in `crates/tv-ui/src/geom.rs`. Design space is a fixed 1920×1080 (`DESIGN_WIDTH`/`DESIGN_HEIGHT` in `crates/tv-ui/src/lib.rs`).

### Rendering loop & compositing

Entry point is `#[wasm_bindgen(js_name = setupApp)] setup_app(root, player)` in `crates/tv-app/src/lib.rs`. It injects a `<video id="player-video">` underlay, a `<canvas id="ui">` overlay, and a `#perf-hud` div into `root`; gets a transparent (alpha, no depth/stencil/AA, non-premultiplied) WebGL2 context via `tv_ui_webgl::context_from_canvas`; builds `App` in `Rc<RefCell<App>>` with the JS `player` wrapped in `JsPlayerSink`; installs `keydown`/`keyup` listeners; and starts a self-rescheduling `requestAnimationFrame` loop. The HUD shows an EMA of frame time / work time / FPS.

`App::tick` computes `dt` from `performance.now()` deltas and uses a struct-destructure **split-borrow** to build `Ctx` (catalog/metrics/video) while borrowing the renderer separately — a deliberate borrow-checker workaround; preserve this pattern when editing. The canvas is cleared fully transparent (`CLEAR = rgba 0,0,0,0`) so the `<video>` underlay shows through in the player.

`WebGl2Renderer` (`crates/tv-ui-webgl/src/webgl2.rs`) batches vector primitives into two vertex `Vec<f32>` buffers (triangles + lines) and flushes them on `end_frame`, before any textured quad, and on `set_clip` (to preserve draw call order). Text is rasterized on an offscreen 2D canvas and cached by `size|color|text`; images are decoded/retained by an async LRU `ImageCache` (`crates/tv-ui-webgl/src/image_cache.rs`) and uploaded to GPU textures in a separate LRU — so `draw_image` no-ops until loaded (hence the placeholder card fill drawn first). During rail motion, `RailList` prefetches nearby posters and draws with `draw_image_cached` (GPU-resident only) to avoid decode/upload hitching the frame; full `draw_image` resumes once the tween settles. `set_clip` maps a design-space `Rect` to a GL scissor box. See [`doc/RENDERING.md`](doc/RENDERING.md).

### Input (leanback / TV)

Logical `Key` enum (`Up/Down/Left/Right/Enter/Back`) is decoupled from physical keys. `map_key` in `crates/tv-app/src/lib.rs` translates browser `KeyboardEvent`s, including Tizen remote Back (keyCode `10009` and `461`). Both `keydown` and `keyup` are routed (keyup drives hold-to-scroll release); OS auto-repeat on arrows is dropped so hold-chaining is fully app-driven. `Back` that bubbles to the root screen pops the empty stack and exits via `tizen.application.getCurrentApplication().exit()` (walked with `js_sys::Reflect`), falling back to `window.close()`. A `Back` consumed by an open overlay returns `Transition::None` and must not exit.

### Extending

Per the module doc comments, these are independent: add content in `crates/tv-ui/src/model.rs` (`Card`/`Rail`/`BannerSlide`/`Catalog`), add a reusable widget in `crates/tv-ui/src/ui/components/`, add a full-screen view by implementing `Screen` in `crates/tv-ui/src/ui/pages/`, add a draw backend by implementing `Renderer` (e.g. in a new crate alongside `tv-ui-webgl`). None should touch the others.

## wasm ↔ JS wiring

`www/apps/tv-app/src/main.ts` imports `setupApp` from the package name `rs-wasm-tv-app` and calls it with the `#app` root and a `PlayerAdapter` from `createHtml5Player()` (`www/apps/tv-app/src/player.ts`), which drives the Rust-created `#player-video` element. Its `vite.config.ts` aliases that package name to `../../../crates/tv-app/pkg` (the wasm-pack output, gitignored), uses `vite-plugin-wasm`, sets `base: "./"` (required for Tizen relative loading), and adds a `watch-pkg` plugin so Vite reloads on wasm rebuild — so no `pnpm i` is needed after each rebuild. Its `tsconfig.json` (extending the workspace's shared `www/tsconfig.base.json`) has a matching `paths` entry for the same alias (needed for `tsc`, which doesn't see Vite's `resolve.alias`). `www/apps/tv-app/src/style.css` centres a letterboxed 16:9 stage; the canvas backing store is 1920×1080 and CSS-scaled to fit.

`www/apps/embed/` follows the identical alias/paths pattern for the package name `tv-ui-web` → `../../../crates/tv-ui-web/pkg`, except its `index.html` is an ordinary page with a small `<canvas>` in the middle, and `src/main.ts` calls `mountCarousels(canvas, json)` instead of `setupApp`.

## Tizen

Only `tizen/config.xml` exists (profile `tv-samsung`, `<content src="index.html"/>`, landscape, hwkey-events enabled). There is no packaging script in the repo; a `.wgt` is built manually with Samsung's Tizen Studio CLI against the `www/apps/tv-app/dist` output plus this config.
