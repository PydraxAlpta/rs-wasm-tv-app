# Design docs

In-depth documentation for `rs-wasm-leanback`. Start with the top-level
[`README.md`](../README.md) for setup/commands and [`CLAUDE.md`](../CLAUDE.md) for the
condensed orientation; these files go deeper.

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — the layer map. The core/glue split, the three
  decoupling traits (`Renderer` / `Screen` / `VideoSink`), the widget tree, the pages, the
  app object and per-frame data flow, and how it builds/hosts.
- **[NAVIGATION.md](NAVIGATION.md)** — the leanback focus model: fixed focus anchor + moving
  content, `Tween` smoothing, the two-index rail model, clamped vs wrapping carousels,
  app-driven hold-to-scroll, cross-zone chaining, the shell's tab sliding, and key mapping.
- **[RENDERING.md](RENDERING.md)** — the WebGL2 backend: design space, the transparent
  overlay-over-video compositing, the batched vector + textured-quad pipelines, flush
  ordering, the image/text/GPU caches, motion-time decode avoidance, clipping, and the
  frame loop + perf HUD.

> The plural [`docs/`](../docs) directory (not this one) is the generated GitHub Pages build
> output and is overwritten by `mise run docs`. Hand-written docs live here in `doc/`.
