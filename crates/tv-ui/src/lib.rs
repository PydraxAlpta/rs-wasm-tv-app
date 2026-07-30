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
#[cfg(test)]
mod test_support;
pub mod theme;
pub mod ui;

pub use buffer::Color;
pub use metrics::{Layout, Metrics};
pub use model::Catalog;
pub use renderer::{ImageBlit, Renderer};
pub use screen::NullVideoSink;
