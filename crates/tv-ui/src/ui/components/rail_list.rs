//! Leanback rail stack: fixed focus anchor, vertically sliding rows of cards.
//!
//! Hold Up/Down (or Left/Right) keeps a runway ahead of the tween (same as
//! horizontal); release eases out forward, coasting past a too-close stop.

use super::card;
use super::carousel::{
    self, HCarousel, CHAIN_THRESHOLD, HOLD_AHEAD, HOLD_SCROLL_DELAY, RAIL_BATCH, RAIL_TAU,
    RELEASE_MIN_RUN,
};
use super::widget::{Flex, FocusResult, Widget};
use crate::anim::Tween;
use crate::geom::Rect;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key};
use crate::theme;

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
    /// How many catalog rails are currently available (lazy batches of [`RAIL_BATCH`]).
    loaded_rails: usize,
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
            anim_rail: Tween::new(0.0, RAIL_TAU),
            held: None,
            held_secs: 0.0,
            banner_pad: 0.0,
            loaded_rails: RAIL_BATCH,
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

    /// How many catalog rails are currently revealed to navigation/render.
    pub fn loaded_rails(&self) -> usize {
        self.loaded_rails
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
        let total = ctx.catalog.rails.len();
        if !self.initialized {
            self.loaded_rails = RAIL_BATCH.min(total);
            self.initialized = true;
        }
        // Catalog may grow after init (e.g. a host appending more rails at
        // runtime); extend the remembered-column table rather than resizing
        // once, so newly appended rails get a tracked column too.
        if self.focus_col.len() < total {
            self.focus_col.resize(total, 0);
        }
    }

    fn visible_rail_count(&self, ctx: &Ctx) -> usize {
        self.loaded_rails.min(ctx.catalog.rails.len())
    }

    /// Reveal the next batch when focus nears the end of what's loaded.
    fn maybe_load_more(&mut self, ctx: &Ctx) {
        let total = ctx.catalog.rails.len();
        if self.loaded_rails >= total {
            return;
        }
        if self.focus_rail + 2 >= self.loaded_rails {
            self.loaded_rails = (self.loaded_rails + RAIL_BATCH).min(total);
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
        // Snap — do not ease from the previous rail's column into this one.
        let col = self.focus_col.get(rail).copied().unwrap_or(0);
        self.rail_carousel.snap(col);
    }

    /// While Up/Down is held, keep ~[`HOLD_AHEAD`] rails of runway ahead.
    fn hold_advance_rail(&mut self, delta: i32, count: usize) {
        if count == 0 || delta == 0 {
            return;
        }
        let ahead = if delta > 0 {
            self.anim_rail.target() - self.anim_rail.value()
        } else {
            self.anim_rail.value() - self.anim_rail.target()
        };
        if ahead < HOLD_AHEAD {
            if delta > 0 && self.focus_rail + 1 < count {
                self.move_to_rail(self.focus_rail + 1);
            } else if delta < 0 && self.focus_rail > 0 {
                self.move_to_rail(self.focus_rail - 1);
            }
        }
    }

    /// On key release after a continuous vertical hold: ease out forward,
    /// coasting one more rail when the chosen stop would be too close.
    fn release_ease_rail(&mut self, delta: i32, count: usize) {
        if count == 0 {
            return;
        }
        let v = self.anim_rail.value();
        let mut target = if delta > 0 {
            v.ceil()
        } else if delta < 0 {
            v.floor()
        } else {
            v.round()
        };

        if delta > 0 && target > v && (target - v) < RELEASE_MIN_RUN {
            target += 1.0;
        } else if delta < 0 && target < v && (v - target) < RELEASE_MIN_RUN {
            target -= 1.0;
        }

        let last = (count.saturating_sub(1)) as f32;
        target = target.clamp(0.0, last);
        let rail = target as usize;

        if let Some(slot) = self.focus_col.get_mut(self.focus_rail) {
            *slot = self.rail_carousel.index();
        }
        self.focus_rail = rail;
        self.anim_rail.set_target(target);
        let col = self.focus_col.get(rail).copied().unwrap_or(0);
        self.rail_carousel.snap(col);
    }

    fn chain_held(&mut self, dt: f32, ctx: &Ctx) {
        let Some(held) = self.held else {
            return;
        };
        self.held_secs += dt;
        self.maybe_load_more(ctx);
        let n = self.visible_rail_count(ctx);
        match held {
            Key::Up | Key::Down => {
                if self.held_secs < HOLD_SCROLL_DELAY {
                    return;
                }
                let delta = if held == Key::Up { -1 } else { 1 };
                self.hold_advance_rail(delta, n);
            }
            Key::Left | Key::Right => {
                if self.held_secs < HOLD_SCROLL_DELAY {
                    return;
                }
                let count = ctx
                    .catalog
                    .rails
                    .get(self.focus_rail)
                    .map(|r| r.cards.len())
                    .unwrap_or(0);
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
        let n = self.visible_rail_count(ctx);
        let lo = (anim.floor() as i32 - 1).max(0) as usize;
        let hi = ((anim.ceil() as i32 + 2) as usize).min(n);
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
        self.maybe_load_more(ctx);
        self.anim_rail.step(dt);
        self.rail_carousel.update(dt);
        self.chain_held(dt, ctx);
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        // Prefetch decode while scrolling; the renderer's per-frame upload budget
        // amortizes GPU uploads so new rails can appear during vertical motion.
        self.prefetch_nearby(r, ctx);

        let m = ctx.metrics;
        let anim_rail = self.anim_rail.value();
        let focus_y = self.focus_card_y(ctx);
        let card_w = m.card_w;
        let card_h = m.card_h;
        let n = self.visible_rail_count(ctx);

        for ri in 0..n {
            let Some(rail) = ctx.catalog.rails.get(ri) else {
                break;
            };
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

            if title_y + m.rail_title_font >= self.bounds.y {
                r.draw_text(
                    self.bounds.x as i32,
                    title_y as i32,
                    m.rail_title_font.round() as i32,
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
            carousel::draw_card_row(
                r,
                m,
                row_bounds,
                &rail.cards,
                col_off,
                self.bounds.x,
                carousel::ImageDraw::All,
            );

            // Current card title rides with each rail (not pinned to the focus slot).
            let col = if ri == self.focus_rail {
                self.rail_carousel.index()
            } else {
                self.focus_col.get(ri).copied().unwrap_or(0)
            };
            if let Some(item) = rail.cards.get(col) {
                let name_y = row_top + card_h + 14.0;
                if name_y + m.card_name_font >= self.bounds.y && name_y <= self.bounds.bottom() {
                    r.draw_text(
                        self.bounds.x as i32,
                        name_y as i32,
                        m.card_name_font.round() as i32,
                        theme::TEXT,
                        &item.title,
                    );
                }
            }
        }

        if self.focused {
            let focus_rect = Rect::new(self.bounds.x, focus_y, card_w, card_h);
            card::draw_focus_ring(r, focus_rect);
        }
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        if !self.focused {
            return FocusResult::Ignored;
        }
        self.ensure_init(ctx);
        self.maybe_load_more(ctx);
        let n = self.visible_rail_count(ctx);
        if n == 0 {
            return FocusResult::Handled;
        }
        match key {
            Key::Left => {
                let count = ctx.catalog.rails[self.focus_rail].cards.len();
                self.rail_carousel.step(-1, count);
                self.held = Some(Key::Left);
                self.held_secs = 0.0;
                FocusResult::Handled
            }
            Key::Right => {
                let count = ctx.catalog.rails[self.focus_rail].cards.len();
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
                if self.focus_rail + 1 < n {
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
        if self.held_secs >= HOLD_SCROLL_DELAY {
            match key {
                Key::Left | Key::Right => {
                    let count = ctx
                        .catalog
                        .rails
                        .get(self.focus_rail)
                        .map(|r| r.cards.len())
                        .unwrap_or(0);
                    let delta = if key == Key::Left { -1 } else { 1 };
                    self.rail_carousel.release_ease(delta, count);
                }
                Key::Up | Key::Down => {
                    let n = self.visible_rail_count(ctx);
                    let delta = if key == Key::Up { -1 } else { 1 };
                    self.release_ease_rail(delta, n);
                }
                _ => {}
            }
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
    use crate::model::{Catalog, Rail};
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
        let cat = crate::test_support::sample_catalog();
        let metrics = Metrics::default();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
            video: &mut video,
            design: crate::test_support::test_design(),
        };
        let mut rails = RailList::new();
        f(&mut rails, &mut ctx);
    }

    #[test]
    fn focus_col_grows_with_appended_rails() {
        let mut cat = Catalog {
            banners: vec![],
            rails: vec![
                Rail {
                    title: "A".into(),
                    cards: vec![],
                },
                Rail {
                    title: "B".into(),
                    cards: vec![],
                },
            ],
        };
        let metrics = Metrics::default();
        let mut video = NullSink;
        let mut rails = RailList::new();

        {
            let ctx = Ctx {
                catalog: &cat,
                metrics: &metrics,
                video: &mut video,
                design: crate::test_support::test_design(),
            };
            rails.ensure_init(&ctx);
        }
        assert_eq!(rails.focus_col.len(), 2);
        rails.focus_col[0] = 3; // simulate a remembered column before growth

        // Host appends a rail at runtime (e.g. `appendRails`) without remounting.
        cat.rails.push(Rail {
            title: "C".into(),
            cards: vec![],
        });
        {
            let ctx = Ctx {
                catalog: &cat,
                metrics: &metrics,
                video: &mut video,
                design: crate::test_support::test_design(),
            };
            rails.ensure_init(&ctx);
        }
        assert_eq!(
            rails.focus_col.len(),
            3,
            "focus_col should grow to track the appended rail"
        );
        assert_eq!(
            rails.focus_col[0], 3,
            "existing remembered column preserved"
        );
        assert_eq!(
            rails.focus_col[2], 0,
            "newly appended rail gets a default column"
        );
    }

    #[test]
    fn hold_down_chains_before_settle() {
        with_rails(|rails, ctx| {
            rails.handle_key(Key::Down, ctx);
            assert_eq!(rails.focus_rail(), 1);
            // Past HOLD_SCROLL_DELAY, then keep runway while still held.
            for _ in 0..30 {
                rails.update(1.0 / 60.0, ctx);
            }
            assert!(
                rails.focus_rail() >= 2,
                "expected chain while held, got {}",
                rails.focus_rail()
            );
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
    fn hold_down_keeps_runway_ahead() {
        with_rails(|rails, ctx| {
            rails.handle_key(Key::Down, ctx);
            for _ in 0..40 {
                rails.update(1.0 / 60.0, ctx);
                let ahead = rails.anim_rail.target() - rails.anim_rail.value();
                let n = rails.visible_rail_count(ctx);
                assert!(
                    ahead > 0.15 || rails.focus_rail() + 1 >= n,
                    "expected continuous runway, ahead={}",
                    ahead
                );
            }
            assert!(
                rails.focus_rail() > 1,
                "should have queued multiple rails while held"
            );
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
    fn vertical_release_ease_drops_far_target() {
        with_rails(|rails, ctx| {
            rails.ensure_init(ctx);
            rails.move_to_rail(1);
            rails.move_to_rail(2);
            rails.move_to_rail(3);
            rails.anim_rail.step(1.0 / 60.0);
            let v = rails.anim_rail.value();
            assert!(rails.anim_rail.target() > v + 1.0);
            let n = rails.visible_rail_count(ctx);
            rails.release_ease_rail(1, n);
            assert!(rails.anim_rail.target() < v + 2.5);
            assert!(rails.anim_rail.target() >= v.ceil() - 1e-3);
            assert_eq!(rails.focus_rail(), rails.anim_rail.target() as usize);
        });
    }

    #[test]
    fn vertical_release_ease_coasts_when_too_close() {
        with_rails(|rails, ctx| {
            rails.ensure_init(ctx);
            rails.anim_rail.snap(0.0);
            rails.anim_rail.set_target(1.0);
            rails.focus_rail = 1;
            for _ in 0..200 {
                rails.anim_rail.step(1.0 / 60.0);
                let v = rails.anim_rail.value();
                if (1.0 - v) < RELEASE_MIN_RUN && (1.0 - v) > 0.05 {
                    break;
                }
            }
            let v = rails.anim_rail.value();
            assert!(
                1.0 - v < RELEASE_MIN_RUN,
                "precondition: tight remaining, v={}",
                v
            );
            let n = rails.visible_rail_count(ctx);
            rails.release_ease_rail(1, n);
            assert!(
                rails.focus_rail() >= 2,
                "should coast to rail 2, got {}",
                rails.focus_rail()
            );
        });
    }

    #[test]
    fn vertical_release_ease_never_goes_backward() {
        with_rails(|rails, ctx| {
            rails.ensure_init(ctx);
            rails.move_to_rail(1);
            rails.anim_rail.step(1.0 / 60.0);
            let v = rails.anim_rail.value();
            let n = rails.visible_rail_count(ctx);
            rails.release_ease_rail(1, n);
            assert!(
                rails.anim_rail.target() >= v.ceil() - 1e-3,
                "must commit forward, v={} target={}",
                v,
                rails.anim_rail.target()
            );
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

    #[test]
    fn rails_load_in_batches_of_five() {
        with_rails(|rails, ctx| {
            rails.ensure_init(ctx);
            assert_eq!(
                rails.loaded_rails(),
                RAIL_BATCH.min(ctx.catalog.rails.len())
            );
            // Walk toward the end of the first batch — should reveal the next 5.
            for _ in 0..4 {
                rails.handle_key(Key::Down, ctx);
            }
            assert!(rails.loaded_rails() >= 10);
            assert!(rails.loaded_rails() <= ctx.catalog.rails.len());
        });
    }
}
