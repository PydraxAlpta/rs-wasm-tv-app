//! Top navigation bar: brand + tabs with an animated underline. Both the
//! brand string and the tab labels are supplied by the caller — this widget
//! has no opinion on what a TV app's tabs should be called.

use crate::anim::Tween;
use crate::geom::Rect;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key};
use crate::theme;
use crate::ui::components::carousel::NAV_TAU;
use crate::ui::components::widget::{Flex, FocusResult, Widget};

const LABEL_SIZE: f32 = 28.0;
/// Fixed hit/layout width for every tab label (underline matches this).
const TAB_W: f32 = 120.0;
const TAB_GAP: f32 = 16.0;

pub struct NavBar {
    height: f32,
    brand: String,
    labels: Vec<String>,
    selected: usize,
    /// Underline center X (design space), animated toward the selected tab.
    selector_x: Tween,
    selector_w: Tween,
    focused: bool,
    bounds: Rect,
    tab_rects: Vec<Rect>,
}

impl NavBar {
    pub fn new(brand: String, labels: Vec<String>) -> Self {
        let tab_rects = vec![Rect::new(0.0, 0.0, 0.0, 0.0); labels.len()];
        let mut bar = Self {
            // Immediately overwritten every frame by the shell's `set_height`
            // — no real value is known at construction.
            height: 0.0,
            brand,
            labels,
            selected: 0,
            selector_x: Tween::new(0.0, NAV_TAU),
            selector_w: Tween::new(TAB_W, NAV_TAU),
            focused: false,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
            tab_rects,
        };
        // `_total_w` is unused by `layout_tabs` today (tabs are laid out from
        // a fixed left margin, not the total width); a real bounds value
        // arrives via the first `Widget::layout` call regardless.
        bar.layout_tabs(0.0);
        bar.snap_selector_to_selected();
        bar
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn tab_count(&self) -> usize {
        self.labels.len()
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn set_height(&mut self, height: f32) {
        self.height = height;
    }

    pub fn select(&mut self, index: usize) {
        if index < self.labels.len() && self.selected != index {
            self.selected = index;
            self.animate_selector_to_selected();
        }
    }

    fn snap_selector_to_selected(&mut self) {
        let (cx, w) = self.selector_metrics(self.selected);
        self.selector_x.snap(cx);
        self.selector_w.snap(w);
    }

    fn animate_selector_to_selected(&mut self) {
        let (cx, w) = self.selector_metrics(self.selected);
        self.selector_x.set_target(cx);
        self.selector_w.set_target(w);
    }

    /// Underline center X (tab slot center) and fixed width.
    fn selector_metrics(&self, index: usize) -> (f32, f32) {
        let rect = self
            .tab_rects
            .get(index)
            .copied()
            .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        (rect.x + rect.w * 0.5, TAB_W)
    }

    fn layout_tabs(&mut self, _total_w: f32) {
        let margin = 64.0;
        // Brand text at 48px is roughly this wide; park tabs just to its right.
        let brand_w = 260.0;
        let mut x = margin + brand_w;
        let y = self.bounds.y;
        let h = self.height;
        for rect in self.tab_rects.iter_mut() {
            *rect = Rect::new(x, y, TAB_W, h);
            x += TAB_W + TAB_GAP;
        }
    }
}

impl Widget for NavBar {
    fn flex(&self) -> Flex {
        Flex::Fixed(self.height)
    }

    fn layout(&mut self, bounds: Rect) {
        let width_changed = (self.bounds.w - bounds.w).abs() > 0.5;
        self.bounds = bounds;
        self.layout_tabs(bounds.w);
        if width_changed || self.selector_x.target() == 0.0 && self.selected == 0 {
            self.animate_selector_to_selected();
        }
    }

    fn update(&mut self, dt: f32, _ctx: &mut Ctx) {
        self.selector_x.step(dt);
        self.selector_w.step(dt);
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        let (x, y, w, h) = self.bounds.as_i32();
        r.fill_rect(x, y, w, h, theme::BG);

        let margin = ctx.metrics.safe_margin;
        let title_y = self.bounds.y + margin * 0.45;
        r.draw_text(
            (self.bounds.x + margin) as i32,
            title_y as i32,
            48,
            theme::HEADER,
            &self.brand,
        );

        let label_y = title_y + 10.0;
        for (i, label) in self.labels.iter().enumerate() {
            let rect = self.tab_rects[i];
            let selected = i == self.selected;
            let color = if self.focused && selected {
                theme::FOCUS
            } else if selected {
                theme::HEADER
            } else {
                theme::TEXT_DIM
            };
            r.draw_text(
                rect.x as i32,
                label_y as i32,
                LABEL_SIZE as i32,
                color,
                label,
            );
        }

        let sel_w = self.selector_w.value();
        let sel_cx = self.selector_x.value();
        // Sit just under the tab label (not near the bottom of the nav band).
        let ul_y = label_y + LABEL_SIZE + 6.0;
        r.fill_rect(
            (sel_cx - sel_w * 0.5) as i32,
            ul_y as i32,
            sel_w.max(1.0) as i32,
            3,
            theme::FOCUS,
        );
    }

    fn handle_key(&mut self, key: Key, _ctx: &mut Ctx) -> FocusResult {
        if !self.focused {
            return FocusResult::Ignored;
        }
        match key {
            Key::Left => {
                if self.selected > 0 {
                    self.select(self.selected - 1);
                }
                FocusResult::Handled
            }
            Key::Right => {
                if self.selected + 1 < self.labels.len() {
                    self.select(self.selected + 1);
                }
                FocusResult::Handled
            }
            Key::Down => FocusResult::MoveOut(Key::Down),
            Key::Up | Key::Enter => FocusResult::Handled,
            Key::Back => FocusResult::Ignored,
        }
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}
