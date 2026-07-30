//! tv-ui-webgl: a WebGL2 [`tv_ui::Renderer`] backend, given an already-created
//! `WebGl2RenderingContext`.
//!
//! Draws via WebGL2 into a canvas the host provides — this crate never creates
//! or owns a canvas element itself, so it can be mounted into any DOM. Batches
//! vector primitives (lines/triangles), caches banner/text as per-URL GPU
//! textures, and draws card rails via an instanced `TEXTURE_2D_ARRAY`.
//!
//! This is also the intended seam for a future JS-mountable component API (a
//! `#[wasm_bindgen]` handle that mounts a `tv_ui` widget tree onto a caller's
//! canvas without going through a full app); no such export exists yet.

mod image_cache;
mod webgl2;

pub use image_cache::{ImageCache, ImageCacheHandle};
pub use webgl2::WebGl2Renderer;

use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext, WebGlContextAttributes};

/// WebGL2 context configured for a transparent overlay (no depth/stencil/AA,
/// non-premultiplied alpha) — suitable for compositing over other content
/// (e.g. a `<video>` underlay).
pub fn context_from_canvas(canvas: &HtmlCanvasElement) -> Option<WebGl2RenderingContext> {
    let attrs = WebGlContextAttributes::new();
    attrs.set_antialias(false);
    attrs.set_alpha(true);
    attrs.set_depth(false);
    attrs.set_stencil(false);
    attrs.set_premultiplied_alpha(false);

    canvas
        .get_context_with_context_options("webgl2", attrs.as_ref())
        .ok()??
        .dyn_into()
        .ok()
}
