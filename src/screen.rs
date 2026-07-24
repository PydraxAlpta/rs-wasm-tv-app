//! The screen-stack router: an extensible, platform-agnostic UI surface.
//!
//! Screens draw through the [`Renderer`] trait and drive video playback through
//! the [`VideoSink`] trait, so they contain no `web-sys` code and are unit
//! testable off-wasm. New screens (details, settings, …) implement [`Screen`]
//! and are pushed/popped via [`Transition`].

use crate::layout::Layout;
use crate::model::Catalog;
use crate::renderer::Renderer;

/// Logical navigation input (already mapped from remote / keyboard keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Back,
}

/// Playback surface a screen can drive. Implemented by the wasm layer over an
/// `HtmlVideoElement`; stubbed in tests.
pub trait VideoSink {
    fn load_and_play(&mut self, url: &str);
    fn play(&mut self);
    fn pause(&mut self);
    fn is_paused(&self) -> bool;
    fn current_time(&self) -> f64;
    fn duration(&self) -> f64;
    fn seek(&mut self, t: f64);
    fn set_visible(&mut self, visible: bool);
}

/// Shared services handed to a screen each update/render/key call.
pub struct Ctx<'a> {
    pub catalog: &'a Catalog,
    pub layout: &'a Layout,
    pub video: &'a mut dyn VideoSink,
}

/// What a screen wants the stack to do after handling a key.
pub enum Transition {
    None,
    Push(Box<dyn Screen>),
    Pop,
}

/// A full-screen view. The stack renders only the top screen.
pub trait Screen {
    fn update(&mut self, dt: f32, ctx: &mut Ctx);
    fn render(&mut self, r: &mut dyn Renderer, ctx: &mut Ctx);
    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> Transition;
}
