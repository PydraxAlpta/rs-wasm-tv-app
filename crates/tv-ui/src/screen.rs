//! The screen-stack router: an extensible, platform-agnostic UI surface.
//!
//! Screens draw through the [`Renderer`] trait and drive video playback through
//! the [`VideoSink`] trait, so they contain no `web-sys` code and are unit
//! testable off-wasm. On wasm, `VideoSink` is backed by a JS `PlayerAdapter`.
//! New screens implement [`Screen`] and are pushed/popped via [`Transition`].

use crate::geom::Rect;
use crate::metrics::Metrics;
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

/// Playback surface a screen can drive.
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

/// A [`VideoSink`] that drops every call. Useful for hosts that embed
/// video-agnostic widgets (e.g. a bare `RailList`) and have no player to wire up.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullVideoSink;

impl VideoSink for NullVideoSink {
    fn load_and_play(&mut self, _url: &str) {}
    fn play(&mut self) {}
    fn pause(&mut self) {}
    fn is_paused(&self) -> bool {
        true
    }
    fn current_time(&self) -> f64 {
        0.0
    }
    fn duration(&self) -> f64 {
        0.0
    }
    fn seek(&mut self, _t: f64) {}
    fn set_visible(&mut self, _visible: bool) {}
}

/// Shared services handed to a screen each update/render/key call.
pub struct Ctx<'a> {
    pub catalog: &'a Catalog,
    pub metrics: &'a Metrics,
    pub video: &'a mut dyn VideoSink,
    /// The host-supplied design-space bounds (e.g. the canvas backing
    /// store's full size) — no fixed resolution is baked into this crate.
    pub design: Rect,
}

/// Enough identity for a driver to resolve what to do next (e.g. map a card
/// id to a playable URL) — a screen reports this instead of building the next
/// screen itself, when it needs data outside its own knowledge. `id` is
/// `Option` because not every activatable item has one (e.g. banners).
#[derive(Debug, Clone, PartialEq)]
pub struct ActivatedItem {
    pub id: Option<u32>,
    pub title: String,
    pub image_url: String,
}

/// What a screen wants the stack to do after handling a key.
pub enum Transition {
    None,
    Push(Box<dyn Screen>),
    Pop,
    /// A screen reported an activation it can't resolve into a concrete next
    /// screen itself (e.g. "Play" was pressed) — the driver decides what, if
    /// anything, to push.
    Activate(ActivatedItem),
}

/// A full-screen view. The stack renders only the top screen.
pub trait Screen {
    fn update(&mut self, dt: f32, ctx: &mut Ctx);
    fn render(&mut self, r: &mut dyn Renderer, ctx: &mut Ctx);
    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> Transition;

    /// Key release. Default ignores; directional hold-to-scroll clears here.
    fn handle_key_up(&mut self, _key: Key, _ctx: &mut Ctx) -> Transition {
        Transition::None
    }
}
