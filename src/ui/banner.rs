//! Hero banner carousel: full-width sliding pages with pagination dots.
//!
//! Collapses/expands via [`BannerCarousel::set_revealed`] (browse drives this
//! from the focused rail). Left/Right wrap; no auto-scroll.

use crate::anim::Tween;
use crate::layout::Layout;
use crate::model::BannerSlide;
use crate::renderer::Renderer;
use crate::theme;
use crate::ui::card;
use crate::ui::carousel::{HCarousel, NAV_TAU};

const DOT_RADIUS: i32 = 7;
const DOT_GAP: i32 = 22;

/// Standalone hero strip used above the browse rails.
#[derive(Debug, Clone)]
pub struct BannerCarousel {
    pages: HCarousel,
    /// `1` = fully shown under the header, `0` = collapsed.
    reveal: Tween,
    focused: bool,
}

impl BannerCarousel {
    pub fn new() -> Self {
        Self {
            pages: HCarousel::new(true),
            reveal: Tween::new(1.0, NAV_TAU),
            focused: false,
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

    /// Target reveal: `true` on the first rail, `false` when scrolled deeper.
    pub fn set_revealed(&mut self, shown: bool) {
        self.reveal.set_target(if shown { 1.0 } else { 0.0 });
        if !shown {
            self.focused = false;
        }
    }

    pub fn update(&mut self, dt: f32, page_count: usize) {
        self.pages.update(dt);
        self.pages.normalize(page_count);
        self.reveal.step(dt);
    }

    pub fn step(&mut self, delta: i32, page_count: usize) {
        self.pages.step(delta, page_count);
    }

    /// Draw the hero when reveal > 0.
    pub fn render(
        &self,
        r: &mut dyn Renderer,
        layout: &Layout,
        slides: &[BannerSlide],
    ) {
        let t = self.reveal.value();
        if t <= 0.001 || slides.is_empty() {
            return;
        }

        let margin = layout.safe_margin as i32;
        let dw = layout.design_w as i32;
        let banner_x = margin;
        let banner_w = dw - 2 * margin;
        let banner_h = layout.banner_h as i32;
        let header_h = layout.header_h as i32;
        let banner_top = (layout.header_h - layout.banner_h * (1.0 - t)) as i32;
        let slide_t = self.pages.anim_value();
        let viewport_l = banner_x as f32;
        let viewport_r = (banner_x + banner_w) as f32;

        r.fill_rect(banner_x, banner_top, banner_w, banner_h, theme::CARD_BG);

        let n = slides.len() as i32;
        let base = slide_t.floor() as i32;
        for offset in -1..=2 {
            let logical = base + offset;
            let idx = logical.rem_euclid(n) as usize;
            let x = viewport_l + (logical as f32 - slide_t) * banner_w as f32;
            if x + banner_w as f32 <= viewport_l || x >= viewport_r {
                continue;
            }
            let xi = x as i32;
            r.fill_rect(xi, banner_top, banner_w, banner_h, theme::CARD_BG);
            r.draw_image(xi, banner_top, banner_w, banner_h, &slides[idx].image_url);
        }

        // Mask safe-margin gutters so sliding art doesn't spill past the inset.
        r.fill_rect(0, banner_top, banner_x, banner_h, theme::BG);
        r.fill_rect(banner_x + banner_w, banner_top, margin, banner_h, theme::BG);

        let dots_y = banner_top + banner_h - margin / 2;
        if dots_y > header_h {
            let dots_x0 = banner_x + margin / 2;
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
            card::draw_focus_ring(r, banner_x, banner_top, banner_w, banner_h);
        }
    }
}

impl Default for BannerCarousel {
    fn default() -> Self {
        Self::new()
    }
}
