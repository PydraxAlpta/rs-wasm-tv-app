# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`rs-wasm-leanback` is a retained-mode **TV/leanback UI** (Netflix-style carousels + a video player) written in Rust, compiled to WebAssembly, and rendered entirely with **WebGL2 via raw `web-sys`**. Remote/d-pad keys drive a fixed-focus spatial navigation model. A Vite + TypeScript app in `www/` hosts the wasm module, and `tizen/config.xml` targets Samsung Tizen TVs (1920×1080).

This project was generated from a `rust-wasm-vite` cargo-generate template (a counter starter); the scaffolding (`www/` wiring, `mise.toml`, wasm glue pattern) comes from there, but the app itself is now the leanback UI described here.

## Commands

Tasks are defined in `mise.toml` (mise is the task runner; pnpm is the JS package manager).

```bash
# First-time setup
mise install                       # Node + pnpm from mise.toml
cd www && pnpm install && cd ..

# Develop: wasm-pack --dev (watch) + Vite dev server, in parallel
mise run dev                       # = dev:wasm (cargo watch → wasm-pack build --dev) + dev:web (vite)

# Build / preview production
mise run build                     # wasm-pack --release → pkg/, then www: tsc && vite build
mise run preview                   # build, then vite preview
mise run build:wasm                # release wasm only → pkg/
```

Prerequisites: Rust toolchain, `wasm-pack`, `cargo-watch` (for `mise run dev`), and the `wasm32-unknown-unknown` target. rust-analyzer is pinned to that target with `allFeatures` in `.zed/settings.json`.

### Tests

There is no mise task for tests. The real test suite is the inline `#[cfg(test)]` modules in the core crate, which run off-wasm:

```bash
cargo test                         # runs the platform-agnostic unit tests
cargo test <name>                  # single test, e.g. cargo test focused_card_sits_at_focus_anchor
```

`tests/web.rs` is a `wasm-bindgen-test` browser harness (currently just a placeholder) and is gated `#![cfg(target_arch = "wasm32")]`, so it runs only under `wasm-pack test --headless --firefox` (or `--chrome`), not plain `cargo test`.

### Lint / format

No custom config; use stock `cargo fmt` / `cargo clippy`. TypeScript is type-checked by `tsc` as part of `pnpm build`.

## Architecture

The defining decision (`src/lib.rs:1-6`): a strict split between **platform-agnostic core logic** and **wasm browser glue**. Everything in `model`, `layout`, `anim`, `screen`, `ui`, `renderer`, `theme`, `buffer` is pure Rust and unit-testable off-wasm; only `wasm/` and `utils.rs` are `#[cfg(target_arch = "wasm32")]`.

Three traits decouple the layers:

- **`Renderer`** (`src/renderer.rs`) — the pluggable draw backend (primitives only: lines, circles, triangles, `draw_image`, `draw_text`; `fill_rect`/`stroke_rect` are default compositions). UI never touches GL directly. Implemented by `WebGl2Renderer` (`src/wasm/webgl2.rs`).
- **`Screen` + `Transition`** (`src/screen.rs`) — the screen-stack router. `App` holds `stack: Vec<Box<dyn Screen>>`, renders only the top screen, and screens request navigation by returning `Transition::{None, Push, Pop}`.
- **`VideoSink`** (`src/screen.rs`) — abstracts `<video>` playback so screens hold no `web-sys`. Real impl `HtmlVideoSink` (`src/wasm/video.rs`); tests use a `NullSink` stub.

`Ctx<'a>` (`src/screen.rs`) bundles `catalog`, `layout`, and `&mut dyn VideoSink`, and is handed to every `update`/`render`/`handle_key` call.

### The leanback focus model (non-obvious — read `layout.rs` + `ui/browse.rs` + `anim.rs` together)

Fixed focus-anchor / moving-content: the focus ring is drawn at a fixed screen position (`layout.focus_x/focus_y`) and the content grid slides behind it. `Layout::card_x(col, anim_col)` = `focus_x + (col - anim_col) * card_step` — when `anim_col == col` the focused card lands on the anchor; same for `rail_y` vertically. `BrowseScreen` tracks integer `focus_rail` plus a **per-rail remembered column** (`focus_col: Vec<usize>`), and drives fractional animated indices with two `Tween`s (`anim.rs`, exponential smoother `1 - e^(-dt/tau)`). Switching rails snaps `anim_col` to the new rail's remembered column so it doesn't slide sideways. `Enter` pushes `PlayerScreen`.

`Layout::tv()` in `src/layout.rs` is the single knob for all browse-page geometry (design space is a fixed 1920×1080; `DESIGN_WIDTH`/`DESIGN_HEIGHT` in `lib.rs`).

### Rendering loop & compositing

Entry point is `#[wasm_bindgen(js_name = setupApp)] setup_app(root)` in `src/wasm/mod.rs`. It injects a `<video class="video-underlay">` under a `<canvas class="ui-canvas">`, creates a transparent (alpha, no depth/stencil/AA) WebGL2 context, builds `App` in `Rc<RefCell<App>>`, installs the keydown listener, and starts a self-rescheduling `requestAnimationFrame` loop.

`App::tick` uses a struct-destructure **split-borrow** to build `Ctx` (catalog/layout/video) while borrowing the renderer separately — a deliberate borrow-checker workaround; preserve this pattern when editing. The canvas is cleared fully transparent (`CLEAR = rgba 0,0,0,0`) so the `<video>` underlay shows through in the player. `WebGl2Renderer` batches vector primitives into vertex `Vec<f32>` buffers and flushes on `end_frame`/before any textured quad (to preserve call order); text is rasterized on an offscreen 2D canvas and cached by `size|color|text`, images cached by URL (async `ImageCache` in `src/wasm/image_cache.rs`, so `draw_image` no-ops until loaded — hence the placeholder fill first).

### Input (leanback / TV)

Logical `Key` enum (`Up/Down/Left/Right/Enter/Back`) is decoupled from physical keys. `map_key` in `src/wasm/mod.rs` translates browser `KeyboardEvent`s, including Tizen remote Back (keyCode `10009` and `461`). Back on the root screen exits via `tizen.application.getCurrentApplication().exit()` (walked with `js_sys::Reflect`), falling back to `window.close()`.

### Extending

Per the module doc comments, these are independent: add content in `src/model.rs` (`Card`/`Rail`/`Catalog`), add a view by implementing `Screen` in `src/ui/`, add a draw backend by implementing `Renderer`. None should touch the others.

## wasm ↔ JS wiring

`www/src/main.ts` imports `setupApp` from the package name `rs-wasm-leanback`; `www/vite.config.ts` aliases that name to `../pkg` (the wasm-pack output, gitignored), uses `vite-plugin-wasm`, sets `base: "./"` (required for Tizen relative loading), and adds a `watch-pkg` plugin so Vite reloads on wasm rebuild — so no `pnpm i` is needed after each rebuild.

## Tizen

Only `tizen/config.xml` exists (profile `tv-samsung`, `<content src="index.html"/>`, landscape, hwkey-events enabled). There is no packaging script in the repo; a `.wgt` is built manually with Samsung's Tizen Studio CLI against the `www/dist` output plus this config.
