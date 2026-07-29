//! Hero banner carousel widget: full-width sliding pages with pagination dots.

use crate::anim::Tween;
use crate::geom::Rect;
use crate::model::BannerSlide;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key};
use crate::theme;
use super::card;
use super::carousel::{HCarousel, HOLD_SCROLL_DELAY, NAV_TAU};
use super::widget::{Flex, FocusResult, Widget};

const DOT_RADIUS: i32 = 7;
const DOT_GAP: i32 = 22;

/// Standalone hero strip drawn as an overlay (zero flex height).
///
/// Collapse is visual only via the reveal tween; parents keep rail layout stable
/// and pass `full_height * reveal` as rail top padding.
#[derive(Debug, Clone)]
pub struct BannerCarousel {
    pages: HCarousel,
    /// `1` = fully shown, `0` = collapsed.
    reveal: Tween,
    /// Held Left/Right for app-driven chaining (OS key-repeat is ignored).
    held: Option<Key>,
    held_secs: f32,
    focused: bool,
    bounds: Rect,
    full_height: f32,
}

impl BannerCarousel {
    pub fn new(full_height: f32) -> Self {
        Self {
            pages: HCarousel::new(true),
            reveal: Tween::new(1.0, NAV_TAU),
            held: None,
            held_secs: 0.0,
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
        if !focused {
            self.held = None;
            self.held_secs = 0.0;
        }
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
            self.held = None;
            self.held_secs = 0.0;
        }
    }

    pub fn set_full_height(&mut self, h: f32) {
        self.full_height = h;
    }

    pub fn step(&mut self, delta: i32, page_count: usize) {
        self.pages.step(delta, page_count);
    }

    /// Place the overlay at `origin` with height `full_height * reveal`.
    pub fn layout_overlay(&mut self, origin: Rect) {
        let h = self.full_height * self.reveal.value();
        self.bounds = Rect::new(origin.x, origin.y, origin.w, h);
    }

    fn chain_held(&mut self, dt: f32, page_count: usize) {
        let Some(held) = self.held else {
            return;
        };
        self.held_secs += dt;
        if !matches!(held, Key::Left | Key::Right) || page_count == 0 {
            return;
        }
        if self.held_secs < HOLD_SCROLL_DELAY {
            return;
        }
        let delta = if held == Key::Left { -1 } else { 1 };
        self.pages.hold_advance(delta, page_count);
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
            // Strict bounds check — avoid zero-size / fully clipped draws that
            // flash when the parent page is mid-slide.
            if x + self.bounds.w <= viewport_l + 0.5 || x >= viewport_r - 0.5 {
                continue;
            }
            let xi = x.round() as i32;
            r.fill_rect(xi, by, bw, bh, theme::CARD_BG);
            r.draw_image(xi, by, bw, bh, &slides[idx].image_url);
        }

        // Dots — bottom-left inside bounds.
        let dots_y = by + bh - 24;
        if dots_y > by {
            let dots_x0 = bx + 24;
            for i in 0..slides.len() {
                let cx = dots_x0 + i as i32 * DOT_GAP;
                let active = i == self.pages.index();
                let color = if active && self.focused {
                    theme::TEXT
                } else if active {
                    theme::FOCUS
                } else {
                    theme::TEXT_DIM.with_alpha(180)
                };
                r.fill_circle(cx, dots_y, DOT_RADIUS, color);
            }
        }

        if self.focused {
            // Ring grows outward from the banner edge.
            card::draw_focus_ring_strong(r, self.bounds);
        }
    }
}

impl Widget for BannerCarousel {
    fn flex(&self) -> Flex {
        // Overlay: does not push rails down via flex reflow.
        Flex::Fixed(0.0)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        self.pages.update(dt);
        let n = ctx.catalog.banners.len();
        self.chain_held(dt, n);
        self.pages.normalize(n);
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
                self.held = Some(Key::Left);
                self.held_secs = 0.0;
                FocusResult::Handled
            }
            Key::Right => {
                self.step(1, n);
                self.held = Some(Key::Right);
                self.held_secs = 0.0;
                FocusResult::Handled
            }
            Key::Down => {
                self.held = None;
                self.held_secs = 0.0;
                FocusResult::MoveOut(Key::Down)
            }
            Key::Up => {
                self.held = None;
                self.held_secs = 0.0;
                FocusResult::MoveOut(Key::Up)
            }
            Key::Enter => {
                self.held = None;
                self.held_secs = 0.0;
                FocusResult::Activate
            }
            Key::Back => FocusResult::Ignored,
        }
    }

    fn handle_key_up(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        if self.held != Some(key) {
            return FocusResult::Ignored;
        }
        if matches!(key, Key::Left | Key::Right) && self.held_secs >= HOLD_SCROLL_DELAY {
            let n = ctx.catalog.banners.len();
            let delta = if key == Key::Left { -1 } else { 1 };
            self.pages.release_ease(delta, n);
        }
        self.held = None;
        self.held_secs = 0.0;
        FocusResult::Handled
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Metrics;
    use crate::model::Catalog;
    use crate::screen::VideoSink;

    struct NullSink;
    impl VideoSink for NullSink {
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
        fn set_visible(&mut self, _v: bool) {}
    }

    #[test]
    fn hold_right_chains_before_settle() {
        let cat = Catalog::sample();
        let metrics = Metrics::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
            video: &mut video,
        };
        let mut banner = BannerCarousel::new(420.0);
        banner.set_focused(true);
        let start = banner.index();
        banner.handle_key(Key::Right, &mut ctx);
        // Past HOLD_SCROLL_DELAY so continuous scroll engages.
        for _ in 0..30 {
            banner.update(1.0 / 60.0, &mut ctx);
        }
        assert_ne!(
            banner.index(),
            start,
            "should have advanced while held"
        );
        banner.handle_key_up(Key::Right, &mut ctx);
        for _ in 0..120 {
            banner.update(1.0 / 60.0, &mut ctx);
        }
        assert!(banner.pages.is_settled());
        assert_ne!(banner.index(), start);
    }
}
