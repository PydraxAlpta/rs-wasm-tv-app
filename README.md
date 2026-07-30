# rs-wasm-tv-app

A retained-mode **TV/leanback UI** — top nav + a hero banner + Netflix-style carousels ("rails" of cards) + a metadata overlay + a video player — written in Rust, compiled to WebAssembly, and rendered entirely with **WebGL2** via raw `web-sys`. Remote/d-pad keys drive a fixed-focus spatial navigation model. `tizen/config.xml` targets Samsung Tizen TVs at 1920×1080.

The Rust side is a **Cargo workspace** of four crates under `crates/`, so the carousel/navigation logic and the WebGL2 renderer are reusable libraries rather than being locked inside one app binary:

- **`tv-ui`** — pure Rust (`model`, `geom`, `metrics`, `anim`, `screen`, `ui`, `renderer`, `theme`, `buffer`), no wasm/web-sys deps, unit-testable off-wasm.
- **`tv-ui-webgl`** — the `WebGl2Renderer` + `ImageCache`, given an already-created GL context; never creates or owns a canvas itself.
- **`tv-app`** (package name `rs-wasm-tv-app`) — the full leanback experience: DOM setup, input, video wiring, `setupApp` entry point.
- **`tv-ui-web`** — a smaller wasm-bindgen binding, `mountCarousels`, for embedding a few carousel rows into an existing DOM page instead of taking over the screen.

Two Vite + TypeScript apps live under `www/apps/` (a pnpm workspace): **`tv-app`** hosts `setupApp` and owns `<video>` playback; **`embed`** hosts `mountCarousels` inside an ordinary page.

In-depth design docs live in [`doc/`](doc/): [architecture](doc/ARCHITECTURE.md), [navigation](doc/NAVIGATION.md), [rendering](doc/RENDERING.md). (The plural `docs/` directory is the generated GitHub Pages build output.)

## Prerequisites

- Rust toolchain + [wasm-pack](https://rustwasm.github.io/wasm-pack/), with the `wasm32-unknown-unknown` target
- [cargo-watch](https://crates.io/crates/cargo-watch) (for `mise run dev`)
- [mise](https://mise.jdx.dev/) (Node/pnpm + task runner), or install Node/pnpm yourself

## Develop

`dev`, `build`, and `preview` take an optional `project` argument — `tv-app`, `embed`, or `both` (default) — and dispatch to per-project tasks with their own `depends`/`sources`/`outputs`, so re-running with nothing changed just prints `sources up-to-date, skipping`.

```bash
mise install                       # Node + pnpm from mise.toml
cd www && pnpm install && cd ..     # installs both apps' deps (pnpm workspace)
mise run dev                       # wasm-pack --dev (watch) + Vite dev server, both apps in parallel
mise run dev tv-app                # just the full leanback app
mise run dev embed                 # just the carousel embed demo
```

- Each Rust binding crate builds into its own `pkg/` via `wasm-pack` (gitignored): `crates/tv-app/pkg/`, `crates/tv-ui-web/pkg/`.
- Each app's Vite config resolves its crate by name with an alias to that `pkg/` dir and reloads on rebuild, so no `pnpm i` is needed after every wasm change.

## Build & preview

```bash
mise run build                     # wasm-pack --release + tsc && vite build, both apps by default
mise run preview                   # serve both apps' production builds
mise run docs                      # build tv-app (default) and copy its dist → docs/ for GitHub Pages
mise run docs embed                # build embed instead and publish that
```

`docs` is the one command that doesn't default to "both" — the `docs/` directory can only hold one project's output at a time, so it takes `tv-app` (default) or `embed`.

## Test

The real suite is the inline `#[cfg(test)]` modules in the `tv-ui` crate, which run off-wasm:

```bash
cargo test                         # whole workspace's platform-agnostic unit tests
cargo test -p tv-ui <name>         # a single test
```

`crates/tv-app/tests/web.rs` is a `wasm-bindgen-test` browser harness (placeholder) and runs only under
`wasm-pack test crates/tv-app --headless --firefox`, not plain `cargo test`.

## Architecture

Three traits decouple the layers:

- **`Renderer`** (`crates/tv-ui/src/renderer.rs`) — pluggable draw backend of primitives; UI never touches GL directly. Implemented by `WebGl2Renderer` (`crates/tv-ui-webgl/src/webgl2.rs`).
- **`Screen` + `Transition`** (`crates/tv-ui/src/screen.rs`) — a screen-stack router; `tv-app` renders only the top screen and screens navigate by returning `Push`/`Pop`.
- **`VideoSink`** (`crates/tv-ui/src/screen.rs`) — abstracts `<video>` playback so screens hold no `web-sys`; `tv-app` forwards to a JS `PlayerAdapter`. A `NullVideoSink` stub is available for consumers (like `tv-ui-web`) that don't need video.

The UI is a retained-mode **widget tree** (`crates/tv-ui/src/ui/components/` + `crates/tv-ui/src/ui/pages/`) with a
`Flex` layout system. The leanback navigation is a fixed focus-anchor / moving-content
model: the focus ring stays at a fixed screen position while the content grid slides behind
it, with animated fractional indices smoothed by `Tween` (`crates/tv-ui/src/anim.rs`). Card/spacing
geometry lives in `Metrics::default()` (`crates/tv-ui/src/metrics.rs`) — `Metrics` has no
hardcoded design resolution, so `tv-ui-web` builds its own smaller instance (`embed_metrics()`)
for its embedded canvas by overriding every field.
See [`doc/`](doc/) for the full write-up.

## Layout

| Path | Role |
|------|------|
| `crates/tv-ui/src/model.rs` | Content model (`Card` / `Rail` / `BannerSlide` / `Catalog`); add content here |
| `crates/tv-ui/src/geom.rs` | Pure geometry: `Rect` / `Size` / `Insets` |
| `crates/tv-ui/src/metrics.rs` | Card sizes, spacing, band heights, fonts (`Metrics::default()`; `Layout` is an alias) |
| `crates/tv-ui/src/anim.rs` | `Tween` — frame-rate-independent exponential smoothing |
| `crates/tv-ui/src/screen.rs` | Navigation core: `Screen`, `Transition`, `VideoSink`, `NullVideoSink`, `Ctx`, `Key` |
| `crates/tv-ui/src/renderer.rs` | The `Renderer` draw-primitive trait |
| `crates/tv-ui/src/ui/components/` | Reusable widgets: nav bar, banner, carousel, rail list, overlay, containers, focus |
| `crates/tv-ui/src/ui/pages/` | Full-screen views: `MainShell`, `CatalogPage`, `PlayerScreen` |
| `crates/tv-ui-webgl/src/` | `WebGl2Renderer`, `ImageCache`, `context_from_canvas` |
| `crates/tv-app/src/` | `setupApp` entry, rAF loop, input mapping, JS video sink |
| `crates/tv-ui-web/src/` | `mountCarousels` entry — the embed-into-existing-DOM binding |
| `www/apps/tv-app/` | Vite + TypeScript host for `setupApp`; owns the `<video>` player |
| `www/apps/embed/` | Vite + TypeScript host for `mountCarousels`, an ordinary page with a small canvas |
| `tizen/config.xml` | Samsung Tizen TV app manifest |
| `mise.toml` | `dev` / `build` / `preview` / `docs` tasks, each taking a `tv-app`/`embed`/`both` project argument |

Extending: add content in `model.rs`, add a reusable widget in `ui/components/`, add a
view by implementing `Screen` in `ui/pages/`, add a draw backend by implementing
`Renderer` — these are independent and shouldn't touch each other. A new binding crate
(alongside `tv-app`/`tv-ui-web`) can compose any of the above without touching the others.
