//! rs-wasm-leanback: a retained-mode TV/leanback UI (carousels + video player)
//! rendered with WebGL2 via raw `web-sys`.
//!
//! The core layers (`model`, `layout`, `anim`, `screen`, `ui`, `renderer`,
//! `theme`) are platform-agnostic and unit-testable off-wasm. Browser glue
//! lives in `wasm` and is compiled only for `wasm32`.

pub mod anim;
pub mod buffer;
pub mod layout;
pub mod model;
pub mod renderer;
pub mod screen;
pub mod theme;
pub mod ui;

pub use buffer::Color;
pub use layout::Layout;
pub use model::Catalog;
pub use renderer::Renderer;

/// Fixed design-space resolution. The canvas backing store is this size; CSS
/// scales it to the viewport with aspect-preserving letterboxing.
pub const DESIGN_WIDTH: u32 = 1920;
pub const DESIGN_HEIGHT: u32 = 1080;

#[cfg(target_arch = "wasm32")]
mod utils;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
