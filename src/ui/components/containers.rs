//! Box-layout containers: Column, Row, Padding, SafeArea, Stack, Spacer.

use crate::geom::{Insets, Rect, Size};
use crate::metrics::Metrics;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key};
use super::widget::{Flex, FocusResult, Widget};

struct ChildSlot {
    widget: Box<dyn Widget>,
    flex: Flex,
    bounds: Rect,
}

/// Vertical stack. Main axis = Y.
pub struct Column {
    children: Vec<ChildSlot>,
    gap: f32,
    bounds: Rect,
}

impl Column {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            gap: 0.0,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    pub fn child(mut self, widget: impl Widget + 'static) -> Self {
        let flex = widget.flex();
        self.children.push(ChildSlot {
            widget: Box::new(widget),
            flex,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        });
        self
    }

    pub fn child_box(mut self, widget: Box<dyn Widget>) -> Self {
        let flex = widget.flex();
        self.children.push(ChildSlot {
            widget,
            flex,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        });
        self
    }
}

impl Default for Column {
    fn default() -> Self {
        Self::new()
    }
}

/// Lay out borrowed widgets top-to-bottom (same algorithm as [`Column`]).
/// Used by screens that own typed children and still want column geometry.
pub fn layout_column(bounds: Rect, gap: f32, widgets: &mut [&mut dyn Widget]) {
    let n = widgets.len();
    if n == 0 {
        return;
    }
    let gaps = gap * (n.saturating_sub(1) as f32);
    let mut flexes = Vec::with_capacity(n);
    let mut fixed = 0.0;
    let mut grow_total = 0.0;
    for w in widgets.iter_mut() {
        let flex = match w.flex() {
            Flex::Hug => {
                let m = w.measure(Size::new(bounds.w, bounds.h));
                Flex::Fixed(m.h)
            }
            other => other,
        };
        match flex {
            Flex::Fixed(h) => fixed += h,
            Flex::Grow(weight) => grow_total += weight.max(0.0),
            Flex::Hug => {}
        }
        flexes.push(flex);
    }
    let leftover = (bounds.h - gaps - fixed).max(0.0);
    let mut y = bounds.y;
    for (w, flex) in widgets.iter_mut().zip(flexes.into_iter()) {
        let h = match flex {
            Flex::Fixed(h) => h,
            Flex::Grow(weight) if grow_total > 0.0 => leftover * (weight / grow_total),
            Flex::Grow(_) | Flex::Hug => 0.0,
        };
        w.layout(Rect::new(bounds.x, y, bounds.w, h));
        y += h + gap;
    }
}

impl Widget for Column {
    fn flex(&self) -> Flex {
        Flex::Grow(1.0)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
        let n = self.children.len();
        if n == 0 {
            return;
        }
        let gaps = self.gap * (n.saturating_sub(1) as f32);
        let mut fixed = 0.0;
        let mut grow_total = 0.0;
        for (i, child) in self.children.iter_mut().enumerate() {
            child.flex = child.widget.flex();
            match child.flex {
                Flex::Fixed(h) => fixed += h,
                Flex::Grow(w) => grow_total += w.max(0.0),
                Flex::Hug => {
                    let m = child.widget.measure(Size::new(bounds.w, bounds.h));
                    child.flex = Flex::Fixed(m.h);
                    fixed += m.h;
                    let _ = i;
                }
            }
        }
        let leftover = (bounds.h - gaps - fixed).max(0.0);
        let mut y = bounds.y;
        for child in &mut self.children {
            let h = match child.flex {
                Flex::Fixed(h) => h,
                Flex::Grow(w) if grow_total > 0.0 => leftover * (w / grow_total),
                Flex::Grow(_) => 0.0,
                Flex::Hug => 0.0,
            };
            child.bounds = Rect::new(bounds.x, y, bounds.w, h);
            child.widget.layout(child.bounds);
            y += h + self.gap;
        }
    }

    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        for child in &mut self.children {
            child.widget.update(dt, ctx);
        }
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        for child in &self.children {
            child.widget.render(r, ctx);
        }
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        for child in &mut self.children {
            match child.widget.handle_key(key, ctx) {
                FocusResult::Ignored => {}
                other => return other,
            }
        }
        FocusResult::Ignored
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Horizontal stack. Main axis = X.
pub struct Row {
    children: Vec<ChildSlot>,
    gap: f32,
    bounds: Rect,
}

impl Row {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            gap: 0.0,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    pub fn child(mut self, widget: impl Widget + 'static) -> Self {
        let flex = widget.flex();
        self.children.push(ChildSlot {
            widget: Box::new(widget),
            flex,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        });
        self
    }
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Row {
    fn flex(&self) -> Flex {
        Flex::Grow(1.0)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
        let n = self.children.len();
        if n == 0 {
            return;
        }
        let gaps = self.gap * (n.saturating_sub(1) as f32);
        let mut fixed = 0.0;
        let mut grow_total = 0.0;
        for child in &mut self.children {
            child.flex = child.widget.flex();
            match child.flex {
                Flex::Fixed(w) => fixed += w,
                Flex::Grow(w) => grow_total += w.max(0.0),
                Flex::Hug => {
                    let m = child.widget.measure(Size::new(bounds.w, bounds.h));
                    child.flex = Flex::Fixed(m.w);
                    fixed += m.w;
                }
            }
        }
        let leftover = (bounds.w - gaps - fixed).max(0.0);
        let mut x = bounds.x;
        for child in &mut self.children {
            let w = match child.flex {
                Flex::Fixed(w) => w,
                Flex::Grow(g) if grow_total > 0.0 => leftover * (g / grow_total),
                Flex::Grow(_) => 0.0,
                Flex::Hug => 0.0,
            };
            child.bounds = Rect::new(x, bounds.y, w, bounds.h);
            child.widget.layout(child.bounds);
            x += w + self.gap;
        }
    }

    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        for child in &mut self.children {
            child.widget.update(dt, ctx);
        }
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        for child in &self.children {
            child.widget.render(r, ctx);
        }
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        for child in &mut self.children {
            match child.widget.handle_key(key, ctx) {
                FocusResult::Ignored => {}
                other => return other,
            }
        }
        FocusResult::Ignored
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Pads a single child.
pub struct Padding {
    insets: Insets,
    child: Box<dyn Widget>,
    bounds: Rect,
}

impl Padding {
    pub fn new(insets: Insets, child: impl Widget + 'static) -> Self {
        Self {
            insets,
            child: Box::new(child),
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Horizontal margins only (full-bleed vertically).
    pub fn horizontal(margin: f32, child: impl Widget + 'static) -> Self {
        Self::new(Insets::vh(0.0, margin), child)
    }
}

impl Widget for Padding {
    fn flex(&self) -> Flex {
        self.child.flex()
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.child.layout(bounds.inset(self.insets));
    }

    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        self.child.update(dt, ctx);
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        self.child.render(r, ctx);
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        self.child.handle_key(key, ctx)
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Applies TV safe-area insets from metrics at layout time.
pub struct SafeArea {
    child: Box<dyn Widget>,
    bounds: Rect,
    /// When true, only left/right insets are applied.
    horizontal_only: bool,
}

impl SafeArea {
    pub fn new(child: impl Widget + 'static) -> Self {
        Self {
            child: Box::new(child),
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
            horizontal_only: false,
        }
    }

    pub fn horizontal(child: impl Widget + 'static) -> Self {
        Self {
            child: Box::new(child),
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
            horizontal_only: true,
        }
    }
}

impl Widget for SafeArea {
    fn flex(&self) -> Flex {
        Flex::Grow(1.0)
    }

    fn layout(&mut self, bounds: Rect) {
        // Without metrics here, treat as identity; host should call
        // [`SafeArea::layout_with`] or wrap with [`Padding`].
        self.bounds = bounds;
        self.child.layout(bounds);
    }

    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        self.child.update(dt, ctx);
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        self.child.render(r, ctx);
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        self.child.handle_key(key, ctx)
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

impl SafeArea {
    pub fn layout_with(&mut self, bounds: Rect, metrics: &Metrics) {
        self.bounds = bounds;
        let insets = if self.horizontal_only {
            Insets::vh(0.0, metrics.safe_margin)
        } else {
            metrics.safe_insets()
        };
        self.child.layout(bounds.inset(insets));
    }
}

/// Overlay children in the same bounds (paint order = push order).
pub struct Stack {
    children: Vec<Box<dyn Widget>>,
    bounds: Rect,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    pub fn child(mut self, widget: impl Widget + 'static) -> Self {
        self.children.push(Box::new(widget));
        self
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Stack {
    fn flex(&self) -> Flex {
        Flex::Grow(1.0)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
        for child in &mut self.children {
            child.layout(bounds);
        }
    }

    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        for child in &mut self.children {
            child.update(dt, ctx);
        }
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        for child in &self.children {
            child.render(r, ctx);
        }
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        for child in &mut self.children {
            match child.handle_key(key, ctx) {
                FocusResult::Ignored => {}
                other => return other,
            }
        }
        FocusResult::Ignored
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Empty growable space.
pub struct Spacer {
    weight: f32,
    bounds: Rect,
}

impl Spacer {
    pub fn new(weight: f32) -> Self {
        Self {
            weight: weight.max(0.0),
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }
}

impl Widget for Spacer {
    fn flex(&self) -> Flex {
        Flex::Grow(self.weight)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn render(&self, _r: &mut dyn Renderer, _ctx: &Ctx) {}

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Helper: inset a full-bleed rect by TV safe margins.
pub fn safe_content_rect(full: Rect, metrics: &Metrics) -> Rect {
    full.inset(metrics.safe_insets())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    struct FixedBox {
        h: f32,
        bounds: Rect,
    }

    impl Widget for FixedBox {
        fn flex(&self) -> Flex {
            Flex::Fixed(self.h)
        }
        fn layout(&mut self, bounds: Rect) {
            self.bounds = bounds;
        }
        fn render(&self, r: &mut dyn Renderer, _ctx: &Ctx) {
            let (x, y, w, h) = self.bounds.as_i32();
            r.fill_rect(x, y, w, h, theme::BG);
        }
        fn bounds(&self) -> Rect {
            self.bounds
        }
    }

    #[test]
    fn column_assigns_fixed_and_grow() {
        let mut col = Column::new()
            .child(FixedBox {
                h: 100.0,
                bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
            })
            .child(Spacer::new(1.0));
        col.layout(Rect::new(0.0, 0.0, 200.0, 400.0));
        assert!((col.children[0].bounds.h - 100.0).abs() < 1e-3);
        assert!((col.children[1].bounds.h - 300.0).abs() < 1e-3);
        assert!((col.children[1].bounds.y - 100.0).abs() < 1e-3);
    }
}
