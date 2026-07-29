//! Top navigation bar: brand + Home / Movies / Shows with animated underline.

use crate::anim::Tween;
use crate::geom::Rect;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key};
use crate::theme;
use crate::ui::components::carousel::NAV_TAU;
use crate::ui::components::widget::{Flex, FocusResult, Widget};

pub const TAB_COUNT: usize = 3;
const LABEL_SIZE: f32 = 28.0;
/// Fixed hit/layout width for every tab label (underline matches this).
const TAB_W: f32 = 120.0;
const TAB_GAP: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home = 0,
    Movies = 1,
    Shows = 2,
}

impl Tab {
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => Tab::Movies,
            2 => Tab::Shows,
            _ => Tab::Home,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Movies => "Movies",
            Tab::Shows => "Shows",
        }
    }

    pub fn all() -> [Tab; TAB_COUNT] {
        [Tab::Home, Tab::Movies, Tab::Shows]
    }
}

pub struct NavBar {
    height: f32,
    selected: Tab,
    /// Underline center X (design space), animated toward the selected tab.
    selector_x: Tween,
    selector_w: Tween,
    focused: bool,
    bounds: Rect,
    tab_rects: [Rect; TAB_COUNT],
}

impl NavBar {
    pub fn new(height: f32) -> Self {
        let mut bar = Self {
            height,
            selected: Tab::Home,
            selector_x: Tween::new(0.0, NAV_TAU),
            selector_w: Tween::new(TAB_W, NAV_TAU),
            focused: false,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
            tab_rects: [Rect::new(0.0, 0.0, 0.0, 0.0); TAB_COUNT],
        };
        bar.layout_tabs(crate::DESIGN_WIDTH as f32);
        bar.snap_selector_to_selected();
        bar
    }

    pub fn selected(&self) -> Tab {
        self.selected
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

    pub fn select(&mut self, tab: Tab) {
        if self.selected != tab {
            self.selected = tab;
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
    fn selector_metrics(&self, tab: Tab) -> (f32, f32) {
        let rect = self.tab_rects[tab.index()];
        (rect.x + rect.w * 0.5, TAB_W)
    }

    fn layout_tabs(&mut self, _total_w: f32) {
        let margin = 64.0;
        // Brand "WASM TV" at 48px ≈ 250px; park tabs just to its right.
        let brand_w = 260.0;
        let mut x = margin + brand_w;
        let y = self.bounds.y;
        let h = self.height;
        for i in 0..TAB_COUNT {
            self.tab_rects[i] = Rect::new(x, y, TAB_W, h);
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
        if width_changed || self.selector_x.target() == 0.0 && self.selected == Tab::Home {
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
            "WASM TV",
        );

        let label_y = title_y + 10.0;
        for tab in Tab::all() {
            let rect = self.tab_rects[tab.index()];
            let selected = tab == self.selected;
            let color = if self.focused && selected {
                theme::FOCUS
            } else if selected {
                theme::HEADER
            } else {
                theme::TEXT_DIM
            };
            let label = tab.label();
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
                let i = self.selected.index();
                if i > 0 {
                    self.select(Tab::from_index(i - 1));
                }
                FocusResult::Handled
            }
            Key::Right => {
                let i = self.selected.index();
                if i + 1 < TAB_COUNT {
                    self.select(Tab::from_index(i + 1));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_indices_round_trip() {
        for t in Tab::all() {
            assert_eq!(Tab::from_index(t.index()), t);
        }
    }
}
