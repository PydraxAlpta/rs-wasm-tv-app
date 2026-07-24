# rs-wasm-leanback

A retained-mode **TV/leanback UI** — Netflix-style carousels ("rails" of cards) plus a video player — written in Rust, compiled to WebAssembly, and rendered entirely with **WebGL2** via raw `web-sys`. Remote/d-pad keys drive a fixed-focus spatial navigation model. A Vite + TypeScript app in `www/` hosts the wasm module, and `tizen/config.xml` targets Samsung Tizen TVs at 1920×1080.

The core (`model`, `layout`, `anim`, `screen`, `ui`, `renderer`, `theme`, `buffer`) is platform-agnostic Rust and unit-testable off-wasm; only `wasm/` and `utils.rs` are `#[cfg(target_arch = "wasm32")]` browser glue.

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
- **`VideoSink`** (`src/screen.rs`) — abstracts `<video>` playback so screens hold no `web-sys`.

The leanback navigation is a fixed focus-anchor / moving-content model: the focus ring stays at a fixed screen position while the content grid slides behind it, with animated fractional indices smoothed by `Tween` (`src/anim.rs`). All browse-page geometry lives in `Layout::tv()` (`src/layout.rs`).

## Layout

| Path | Role |
|------|------|
| `src/model.rs` | Content model (`Card` / `Rail` / `Catalog`); add content here |
| `src/layout.rs` | All browse-page geometry in the 1920×1080 design space |
| `src/screen.rs` | Navigation core: `Screen`, `Transition`, `VideoSink`, `Ctx`, `Key` |
| `src/renderer.rs` | The `Renderer` draw-primitive trait |
| `src/ui/` | Concrete screens (`browse`, `player`) |
| `src/wasm/` | Browser glue: `setupApp` entry, rAF loop, WebGL2 renderer, image cache, video sink |
| `pkg/` | wasm-pack output (gitignored; created on build) |
| `www/` | Vite + TypeScript host app (imports the crate by name) |
| `tizen/config.xml` | Samsung Tizen TV app manifest |
| `mise.toml` | `dev` / `build` / `preview` tasks |

Extending: add content in `model.rs`, add a view by implementing `Screen` in `src/ui/`, add a draw backend by implementing `Renderer` — these are independent and shouldn't touch each other.
