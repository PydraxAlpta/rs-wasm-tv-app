//! Leanback rail stack: fixed focus anchor, vertically sliding rows of cards.
//!
//! Hold Up/Down (or Left/Right) to chain the next step before the current tween
//! settles; release to ease out to the current target.

use crate::anim::Tween;
use crate::geom::Rect;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key};
use crate::theme;
use super::card;
use super::carousel::{self, HCarousel, CHAIN_THRESHOLD, HOLD_SCROLL_DELAY, NAV_TAU};
use super::widget::{Flex, FocusResult, Widget};

pub struct RailList {
    focus_rail: usize,
    focus_col: Vec<usize>,
    rail_carousel: HCarousel,
    anim_rail: Tween,
    /// Directional key currently held (app-driven repeat, not OS key-repeat).
    held: Option<Key>,
    /// Seconds the current `held` key has been down.
    held_secs: f32,
    /// Extra top inset so rail 0 sits below the overlay banner while it is shown.
    banner_pad: f32,
    focused: bool,
    bounds: Rect,
    initialized: bool,
}

impl RailList {
    pub fn new() -> Self {
        Self {
            focus_rail: 0,
            focus_col: Vec::new(),
            rail_carousel: HCarousel::new(false),
            anim_rail: Tween::new(0.0, NAV_TAU),
            held: None,
            held_secs: 0.0,
            banner_pad: 0.0,
            focused: true,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
            initialized: false,
        }
    }

    pub fn focus(&self) -> (usize, usize) {
        (self.focus_rail, self.rail_carousel.index())
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

    pub fn focus_rail(&self) -> usize {
        self.focus_rail
    }

    pub fn anim_rail_settled(&self) -> bool {
        self.anim_rail.is_settled()
    }

    /// True when the vertical tween is close enough to chain or cross a focus edge.
    pub fn vertical_near_settle(&self) -> bool {
        (self.anim_rail.target() - self.anim_rail.value()).abs() < CHAIN_THRESHOLD
    }

    pub fn set_held(&mut self, key: Option<Key>) {
        if self.held != key {
            self.held_secs = 0.0;
        }
        self.held = key;
    }

    /// Continue an already-active vertical hold (skip the tap delay).
    pub fn arm_continuous_hold(&mut self, key: Key) {
        self.held = Some(key);
        self.held_secs = HOLD_SCROLL_DELAY;
    }

    /// Space reserved under the overlay banner (follows banner reveal).
    pub fn set_banner_pad(&mut self, pad: f32) {
        self.banner_pad = pad.max(0.0);
    }

    fn ensure_init(&mut self, ctx: &Ctx) {
        if !self.initialized {
            self.focus_col = vec![0; ctx.catalog.rails.len()];
            self.initialized = true;
        }
    }

    fn focus_card_y(&self, ctx: &Ctx) -> f32 {
        self.bounds.y + self.banner_pad + ctx.metrics.rail_title_h + 8.0
    }

    fn move_to_rail(&mut self, rail: usize) {
        if let Some(slot) = self.focus_col.get_mut(self.focus_rail) {
            *slot = self.rail_carousel.index();
        }
        self.focus_rail = rail;
        self.anim_rail.set_target(rail as f32);
        let col = self.focus_col.get(rail).copied().unwrap_or(0);
        self.rail_carousel.ease_to(col);
    }

    fn chain_held(&mut self, dt: f32, ctx: &Ctx) {
        let Some(held) = self.held else {
            return;
        };
        self.held_secs += dt;
        let rails = &ctx.catalog.rails;
        match held {
            Key::Up | Key::Down => {
                if self.held_secs < HOLD_SCROLL_DELAY {
                    return;
                }
                let remaining = (self.anim_rail.target() - self.anim_rail.value()).abs();
                if remaining >= CHAIN_THRESHOLD {
                    return;
                }
                match held {
                    Key::Down if self.focus_rail + 1 < rails.len() => {
                        self.move_to_rail(self.focus_rail + 1);
                    }
                    Key::Up if self.focus_rail > 0 => {
                        self.move_to_rail(self.focus_rail - 1);
                    }
                    _ => {}
                }
            }
            Key::Left | Key::Right => {
                if self.held_secs < HOLD_SCROLL_DELAY {
                    return;
                }
                let count = rails.get(self.focus_rail).map(|r| r.cards.len()).unwrap_or(0);
                let delta = if held == Key::Left { -1 } else { 1 };
                self.rail_carousel.hold_advance(delta, count);
            }
            _ => {}
        }
    }

    fn prefetch_nearby(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        let m = ctx.metrics;
        let anim = self.anim_rail.value();
        let focus_y = self.focus_card_y(ctx);
        let lo = (anim.floor() as i32 - 1).max(0) as usize;
        let hi = ((anim.ceil() as i32 + 2) as usize).min(ctx.catalog.rails.len());
        for ri in lo..hi {
            let Some(rail) = ctx.catalog.rails.get(ri) else {
                continue;
            };
            let row_top = focus_y + (ri as f32 - anim) * m.rail_step;
            if row_top + m.card_h < self.bounds.y || row_top > self.bounds.bottom() {
                // Still prefetch the focused/near rails even if slightly off-screen.
                if (ri as f32 - anim).abs() > 2.0 {
                    continue;
                }
            }
            let col_off = if ri == self.focus_rail {
                self.rail_carousel.anim_value()
            } else {
                self.focus_col.get(ri).copied().unwrap_or(0) as f32
            };
            let step = m.card_step();
            let focus_x = self.bounds.x;
            let view_l = self.bounds.x - m.safe_margin;
            let view_r = self.bounds.right() + m.safe_margin;
            for (ci, item) in rail.cards.iter().enumerate() {
                let x = focus_x + (ci as f32 - col_off) * step;
                if x + m.card_w < view_l || x > view_r {
                    continue;
                }
                r.prefetch_image(&item.image_url);
            }
        }
    }
}

impl Default for RailList {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for RailList {
    fn flex(&self) -> Flex {
        Flex::Grow(1.0)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        self.ensure_init(ctx);
        self.anim_rail.step(dt);
        self.rail_carousel.update(dt);
        self.chain_held(dt, ctx);
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        self.prefetch_nearby(r, ctx);

        let m = ctx.metrics;
        let anim_rail = self.anim_rail.value();
        let focus_y = self.focus_card_y(ctx);
        let card_w = m.card_w;
        let card_h = m.card_h;

        for (ri, rail) in ctx.catalog.rails.iter().enumerate() {
            let row_top = focus_y + (ri as f32 - anim_rail) * m.rail_step;
            let title_y = row_top - m.rail_title_h;
            if row_top + card_h < self.bounds.y || title_y > self.bounds.bottom() {
                continue;
            }

            let col_off = if ri == self.focus_rail {
                self.rail_carousel.anim_value()
            } else {
                self.focus_col.get(ri).copied().unwrap_or(0) as f32
            };

            if title_y + 30.0 >= self.bounds.y {
                r.draw_text(
                    self.bounds.x as i32,
                    title_y as i32,
                    30,
                    theme::RAIL_TITLE,
                    &rail.title,
                );
            }

            let row_bounds = Rect::new(
                self.bounds.x - m.safe_margin,
                row_top,
                self.bounds.w + 2.0 * m.safe_margin,
                card_h,
            );
            // Focus slot stays at the content left edge; cull window includes
            // the safe margins so scrolled-off neighbors stay visible there.
            carousel::draw_card_row(r, m, row_bounds, &rail.cards, col_off, self.bounds.x);
        }

        if self.focused {
            let focus_rect = Rect::new(self.bounds.x, focus_y, card_w, card_h);
            card::draw_focus_ring(r, focus_rect);
            if let Some(rail) = ctx.catalog.rails.get(self.focus_rail) {
                if let Some(item) = rail.cards.get(self.rail_carousel.index()) {
                    r.draw_text(
                        self.bounds.x as i32,
                        (focus_y + card_h + 14.0) as i32,
                        28,
                        theme::TEXT,
                        &item.title,
                    );
                }
            }
        }
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        if !self.focused {
            return FocusResult::Ignored;
        }
        self.ensure_init(ctx);
        let rails = &ctx.catalog.rails;
        if rails.is_empty() {
            return FocusResult::Handled;
        }
        match key {
            Key::Left => {
                let count = rails[self.focus_rail].cards.len();
                self.rail_carousel.step(-1, count);
                self.held = Some(Key::Left);
                self.held_secs = 0.0;
                FocusResult::Handled
            }
            Key::Right => {
                let count = rails[self.focus_rail].cards.len();
                self.rail_carousel.step(1, count);
                self.held = Some(Key::Right);
                self.held_secs = 0.0;
                FocusResult::Handled
            }
            Key::Up => {
                if self.focus_rail == 0 {
                    // Keep held so the page can continue Up → nav while the key is down.
                    self.held = Some(Key::Up);
                    self.held_secs = 0.0;
                    FocusResult::MoveOut(Key::Up)
                } else {
                    self.move_to_rail(self.focus_rail - 1);
                    self.held = Some(Key::Up);
                    self.held_secs = 0.0;
                    FocusResult::Handled
                }
            }
            Key::Down => {
                if self.focus_rail + 1 < rails.len() {
                    self.move_to_rail(self.focus_rail + 1);
                    self.held = Some(Key::Down);
                    self.held_secs = 0.0;
                    FocusResult::Handled
                } else {
                    self.held = Some(Key::Down);
                    self.held_secs = 0.0;
                    FocusResult::Handled
                }
            }
            Key::Enter => {
                self.held = None;
                FocusResult::Activate
            }
            Key::Back => FocusResult::Ignored,
        }
    }

    fn handle_key_up(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        if self.held != Some(key) {
            return FocusResult::Ignored;
        }
        // Continuous hold: stitch a smooth stop. Tap: leave the single step target.
        if matches!(key, Key::Left | Key::Right) && self.held_secs >= HOLD_SCROLL_DELAY {
            let count = ctx
                .catalog
                .rails
                .get(self.focus_rail)
                .map(|r| r.cards.len())
                .unwrap_or(0);
            let delta = if key == Key::Left { -1 } else { 1 };
            self.rail_carousel.release_ease(delta, count);
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

    fn with_rails(f: impl FnOnce(&mut RailList, &mut Ctx)) {
        let cat = Catalog::sample();
        let metrics = Metrics::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
            video: &mut video,
        };
        let mut rails = RailList::new();
        f(&mut rails, &mut ctx);
    }

    #[test]
    fn hold_down_chains_before_settle() {
        with_rails(|rails, ctx| {
            rails.handle_key(Key::Down, ctx);
            assert_eq!(rails.focus_rail(), 1);
            // Past HOLD_SCROLL_DELAY, then chain while still held.
            for _ in 0..30 {
                rails.update(1.0 / 60.0, ctx);
            }
            assert!(rails.focus_rail() >= 2, "expected chain while held, got {}", rails.focus_rail());
            rails.handle_key_up(Key::Down, ctx);
            let at_release = rails.focus_rail();
            for _ in 0..120 {
                rails.update(1.0 / 60.0, ctx);
            }
            assert_eq!(rails.focus_rail(), at_release);
            assert!(rails.anim_rail_settled());
        });
    }

    #[test]
    fn short_down_tap_moves_one_rail() {
        with_rails(|rails, ctx| {
            rails.handle_key(Key::Down, ctx);
            for _ in 0..8 {
                rails.update(1.0 / 60.0, ctx);
            }
            rails.handle_key_up(Key::Down, ctx);
            for _ in 0..120 {
                rails.update(1.0 / 60.0, ctx);
            }
            assert_eq!(rails.focus_rail(), 1);
            assert!(rails.anim_rail_settled());
        });
    }

    #[test]
    fn release_eases_out_without_extra_step() {
        with_rails(|rails, ctx| {
            rails.handle_key(Key::Down, ctx);
            rails.handle_key_up(Key::Down, ctx);
            for _ in 0..120 {
                rails.update(1.0 / 60.0, ctx);
            }
            assert_eq!(rails.focus_rail(), 1);
            assert!(rails.anim_rail_settled());
        });
    }

    #[test]
    fn short_right_tap_moves_one_card() {
        with_rails(|rails, ctx| {
            rails.handle_key(Key::Right, ctx);
            // Typical tap: a few frames then release (under HOLD_SCROLL_DELAY).
            for _ in 0..8 {
                rails.update(1.0 / 60.0, ctx);
            }
            rails.handle_key_up(Key::Right, ctx);
            for _ in 0..120 {
                rails.update(1.0 / 60.0, ctx);
            }
            assert_eq!(rails.focus().1, 1);
            assert!(rails.rail_carousel.is_settled());
        });
    }
}
