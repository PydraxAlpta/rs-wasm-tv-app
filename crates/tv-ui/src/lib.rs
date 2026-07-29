//! tv-ui: platform-agnostic carousels + navigation for a retained-mode TV/leanback UI.
//!
//! Everything here is pure Rust, unit-testable off any target: `model`, `geom`,
//! `metrics`, `anim`, `screen`, `ui`, `renderer`, `theme`, `buffer`. Drawing goes
//! through the [`Renderer`] trait — this crate has no web-sys/wasm-bindgen deps
//! and knows nothing about WebGL, the DOM, or JS. A backend (e.g. `tv-ui-webgl`)
//! implements `Renderer`; a host application composes screens (`ui::pages`) and
//! drives the `Screen`/`Transition` stack (`screen`).

pub mod anim;
pub mod buffer;
pub mod geom;
pub mod metrics;
pub mod model;
pub mod renderer;
pub mod screen;
pub mod theme;
pub mod ui;

pub use buffer::Color;
pub use metrics::{Layout, Metrics};
pub use model::Catalog;
pub use renderer::Renderer;
pub use screen::NullVideoSink;

/// Fixed design-space resolution. The canvas backing store is this size; CSS
/// scales it to the viewport with aspect-preserving letterboxing.
pub const DESIGN_WIDTH: u32 = 1920;
pub const DESIGN_HEIGHT: u32 = 1080;
