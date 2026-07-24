//! Hero banner carousel widget: full-width sliding pages with pagination dots.

use crate::anim::Tween;
use crate::geom::Rect;
use crate::model::BannerSlide;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key};
use crate::theme;
use super::card;
use super::carousel::{HCarousel, NAV_TAU};
use super::widget::{Flex, FocusResult, Widget};

const DOT_RADIUS: i32 = 7;
const DOT_GAP: i32 = 22;

/// Standalone hero strip. Height is controlled by the parent via layout bounds
/// (collapse = parent assigns a shrinking height).
#[derive(Debug, Clone)]
pub struct BannerCarousel {
    pages: HCarousel,
    /// `1` = fully shown, `0` = collapsed (drives preferred flex height).
    reveal: Tween,
    focused: bool,
    bounds: Rect,
    full_height: f32,
}

impl BannerCarousel {
    pub fn new(full_height: f32) -> Self {
        Self {
            pages: HCarousel::new(true),
            reveal: Tween::new(1.0, NAV_TAU),
            focused: false,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
            full_height,
        }
    }

    pub fn index(&self) -> usize {
        self.pages.index()
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn reveal_value(&self) -> f32 {
        self.reveal.value()
    }

    pub fn reveal_target(&self) -> f32 {
        self.reveal.target()
    }

    pub fn set_revealed(&mut self, shown: bool) {
        self.reveal.set_target(if shown { 1.0 } else { 0.0 });
        if !shown {
            self.focused = false;
        }
    }

    pub fn set_full_height(&mut self, h: f32) {
        self.full_height = h;
    }

    pub fn step(&mut self, delta: i32, page_count: usize) {
        self.pages.step(delta, page_count);
    }

    fn paint(&self, r: &mut dyn Renderer, slides: &[BannerSlide]) {
        if self.bounds.h <= 0.5 || slides.is_empty() {
            return;
        }
        let (bx, by, bw, bh) = self.bounds.as_i32();
        if bw <= 0 || bh <= 0 {
            return;
        }

        let slide_t = self.pages.anim_value();
        let viewport_l = self.bounds.x;
        let viewport_r = self.bounds.right();

        r.fill_rect(bx, by, bw, bh, theme::CARD_BG);

        let n = slides.len() as i32;
        let base = slide_t.floor() as i32;
        for offset in -1..=2 {
            let logical = base + offset;
            let idx = logical.rem_euclid(n) as usize;
            let x = viewport_l + (logical as f32 - slide_t) * self.bounds.w;
            if x + self.bounds.w <= viewport_l || x >= viewport_r {
                continue;
            }
            let xi = x as i32;
            r.fill_rect(xi, by, bw, bh, theme::CARD_BG);
            r.draw_image(xi, by, bw, bh, &slides[idx].image_url);
        }

        // Dots — bottom-left inside bounds.
        let dots_y = by + bh - 24;
        if dots_y > by {
            let dots_x0 = bx + 24;
            for i in 0..slides.len() {
                let cx = dots_x0 + i as i32 * DOT_GAP;
                let color = if i == self.pages.index() {
                    theme::FOCUS
                } else {
                    theme::TEXT_DIM.with_alpha(180)
                };
                r.fill_circle(cx, dots_y, DOT_RADIUS, color);
            }
        }

        if self.focused {
            card::draw_focus_ring(r, self.bounds);
        }
    }
}

impl Widget for BannerCarousel {
    fn flex(&self) -> Flex {
        Flex::Fixed(self.full_height * self.reveal.value())
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        self.pages.update(dt);
        self.pages.normalize(ctx.catalog.banners.len());
        self.reveal.step(dt);
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        self.paint(r, &ctx.catalog.banners);
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        if !self.focused {
            return FocusResult::Ignored;
        }
        let n = ctx.catalog.banners.len();
        match key {
            Key::Left => {
                self.step(-1, n);
                FocusResult::Handled
            }
            Key::Right => {
                self.step(1, n);
                FocusResult::Handled
            }
            Key::Down => FocusResult::MoveOut(Key::Down),
            Key::Up => FocusResult::Handled,
            Key::Enter => FocusResult::Activate,
            Key::Back => FocusResult::Ignored,
        }
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}
