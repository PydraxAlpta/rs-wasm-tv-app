# rs-wasm-leanback

A retained-mode **TV/leanback UI** — top nav + a hero banner + Netflix-style carousels ("rails" of cards) + a metadata overlay + a video player — written in Rust, compiled to WebAssembly, and rendered entirely with **WebGL2** via raw `web-sys`. Remote/d-pad keys drive a fixed-focus spatial navigation model. A Vite + TypeScript app in `www/` hosts the wasm module and owns `<video>` playback, and `tizen/config.xml` targets Samsung Tizen TVs at 1920×1080.

The core (`model`, `geom`, `metrics`, `anim`, `screen`, `ui`, `renderer`, `theme`, `buffer`) is platform-agnostic Rust and unit-testable off-wasm; only `wasm/` and `utils.rs` are `#[cfg(target_arch = "wasm32")]` browser glue.

In-depth design docs live in [`doc/`](doc/): [architecture](doc/ARCHITECTURE.md), [navigation](doc/NAVIGATION.md), [rendering](doc/RENDERING.md). (The plural `docs/` directory is the generated GitHub Pages build output.)

## Prerequisites

- Rust toolchain + [wasm-pack](https://rustwasm.github.io/wasm-pack/), with the `wasm32-unknown-unknown` target
- [cargo-watch](https://crates.io/crates/cargo-watch) (for `mise run dev`)
- [mise](https://mise.jdx.dev/) (Node/pnpm + task runner), or install Node/pnpm yourself

## Develop

```bash
mise install                       # Node + pnpm from mise.toml
cd www && pnpm install && cd ..
mise run dev                       # wasm-pack --dev (watch) + Vite dev server, in parallel
```

- The Rust crate builds into `pkg/` via `wasm-pack` (gitignored).
- Vite resolves the crate by name with an alias to `../pkg` and reloads on rebuild, so no `pnpm i` is needed after every wasm change.

## Build & preview

```bash
mise run build                     # wasm-pack --release → pkg/, then www: tsc && vite build
mise run preview                   # build, then vite preview
mise run build:wasm                # release wasm only → pkg/
```

## Test

The real suite is the inline `#[cfg(test)]` modules in the core crate, which run off-wasm:

```bash
cargo test                         # platform-agnostic unit tests (layout math, navigation, tweens, …)
cargo test <name>                  # a single test
```

`tests/web.rs` is a `wasm-bindgen-test` browser harness (placeholder) and runs only under
`wasm-pack test --headless --firefox`, not plain `cargo test`.

## Architecture

Three traits decouple the layers:

- **`Renderer`** (`src/renderer.rs`) — pluggable draw backend of primitives; UI never touches GL directly. Implemented by `WebGl2Renderer` (`src/wasm/webgl2.rs`).
- **`Screen` + `Transition`** (`src/screen.rs`) — a screen-stack router; the app renders only the top screen and screens navigate by returning `Push`/`Pop`.
- **`VideoSink`** (`src/screen.rs`) — abstracts `<video>` playback so screens hold no `web-sys`; on wasm it forwards to a JS `PlayerAdapter`.

The UI is a retained-mode **widget tree** (`src/ui/components/` + `src/ui/pages/`) with a
`Flex` layout system. The leanback navigation is a fixed focus-anchor / moving-content
model: the focus ring stays at a fixed screen position while the content grid slides behind
it, with animated fractional indices smoothed by `Tween` (`src/anim.rs`). Card/spacing
geometry lives in `Metrics::tv()` (`src/metrics.rs`). See [`doc/`](doc/) for the full write-up.

## Layout

| Path | Role |
|------|------|
| `src/model.rs` | Content model (`Card` / `Rail` / `BannerSlide` / `Catalog`); add content here |
| `src/geom.rs` | Pure geometry: `Rect` / `Size` / `Insets` |
| `src/metrics.rs` | Card sizes, spacing, band heights (`Metrics::tv()`; `Layout` is an alias) |
| `src/anim.rs` | `Tween` — frame-rate-independent exponential smoothing |
| `src/screen.rs` | Navigation core: `Screen`, `Transition`, `VideoSink`, `Ctx`, `Key` |
| `src/renderer.rs` | The `Renderer` draw-primitive trait |
| `src/ui/components/` | Reusable widgets: nav bar, banner, carousel, rail list, overlay, containers, focus |
| `src/ui/pages/` | Full-screen views: `MainShell`, `CatalogPage`, `PlayerScreen` |
| `src/wasm/` | Browser glue: `setupApp` entry, rAF loop, WebGL2 renderer, image cache, video sink |
| `pkg/` | wasm-pack output (gitignored; created on build) |
| `www/` | Vite + TypeScript host app (imports the crate by name; owns the `<video>` player) |
| `tizen/config.xml` | Samsung Tizen TV app manifest |
| `mise.toml` | `dev` / `build` / `preview` / `docs` tasks |

Extending: add content in `model.rs`, add a reusable widget in `src/ui/components/`, add a
view by implementing `Screen` in `src/ui/pages/`, add a draw backend by implementing
`Renderer` — these are independent and shouldn't touch each other.
