//! Retained-mode widget protocol and flex sizing.

use crate::geom::{Rect, Size};
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key};

/// How a widget wants to size along the main axis of a flex container.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Flex {
    Fixed(f32),
    Grow(f32),
    Hug,
}

impl Default for Flex {
    fn default() -> Self {
        Flex::Hug
    }
}

/// Result of routing a key into a focusable widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusResult {
    Handled,
    /// Focus should leave this widget in `dir` (parent decides next target).
    MoveOut(Key),
    Activate,
    Ignored,
}

/// A composable UI node. Parents assign `bounds` via [`Widget::layout`].
pub trait Widget {
    fn flex(&self) -> Flex {
        Flex::Hug
    }

    /// Intrinsic size when using [`Flex::Hug`]. Default: zero.
    fn measure(&self, _available: Size) -> Size {
        Size::new(0.0, 0.0)
    }

    fn layout(&mut self, bounds: Rect);

    fn update(&mut self, _dt: f32, _ctx: &mut Ctx) {}

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx);

    fn handle_key(&mut self, _key: Key, _ctx: &mut Ctx) -> FocusResult {
        FocusResult::Ignored
    }

    /// Key release. Default ignores; hold-to-scroll widgets clear held direction here.
    fn handle_key_up(&mut self, _key: Key, _ctx: &mut Ctx) -> FocusResult {
        FocusResult::Ignored
    }

    fn bounds(&self) -> Rect;
}

/// Run a layout → update → render pass for a screen-rooted widget tree.
/// `ctx.design` supplies the root's bounds.
pub fn tick_widget(root: &mut dyn Widget, dt: f32, r: &mut dyn Renderer, ctx: &mut Ctx) {
    root.layout(ctx.design);
    root.update(dt, ctx);
    root.render(r, ctx);
}
