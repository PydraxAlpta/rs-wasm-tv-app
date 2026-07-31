//! Horizontal carousel navigation + card-row rendering.
//!
//! [`HCarousel`] owns the focused index and slide tween. Card rails clamp at
//! the ends; wrapping carousels (e.g. the hero) use unbounded targets so rapid
//! wraps never reverse mid-flight.

use crate::anim::Tween;
use crate::geom::Rect;
use crate::metrics::Metrics;
use crate::model::Card;
use crate::renderer::{ImageBlit, Renderer};
use super::card;

/// Time-constant (seconds) for carousel easing — small = snappy.
pub const NAV_TAU: f32 = 0.11;

/// Vertical rail scroll — slower than horizontal for a longer ease.
pub const RAIL_TAU: f32 = 0.2;

/// Hero banner scroll — a bit slower than card rails so hold-chaining does not
/// race through full-bleed slides; still snappy enough that the ease-out does
/// not linger.
pub const BANNER_TAU: f32 = 0.16;

/// How many rails become available per lazy-load batch.
pub const RAIL_BATCH: usize = 5;

/// Vertical / zone hold-chain: start the next step this close to the target.
pub const CHAIN_THRESHOLD: f32 = 0.4;

/// Horizontal hold: keep at least this much runway (index units) ahead of
/// `current` so the exponential tween never settles between cards.
pub const HOLD_AHEAD: f32 = 1.0;

/// On release after a continuous hold: if the chosen stop is closer than this,
/// coast one more card so the ease-out has room to feel smooth.
pub const RELEASE_MIN_RUN: f32 = 0.4;

/// Seconds a Left/Right key must be held before continuous scroll starts.
/// Shorter presses are a single-card tap.
pub const HOLD_SCROLL_DELAY: f32 = 0.2;

/// Horizontal index + animated fractional offset.
#[derive(Debug, Clone)]
pub struct HCarousel {
    index: usize,
    anim: Tween,
    wrap: bool,
}

impl HCarousel {
    pub fn new(wrap: bool) -> Self {
        Self::with_tau(wrap, NAV_TAU)
    }

    pub fn with_tau(wrap: bool, tau: f32) -> Self {
        Self {
            index: 0,
            anim: Tween::new(0.0, tau),
            wrap,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn anim_value(&self) -> f32 {
        self.anim.value()
    }

    pub fn target(&self) -> f32 {
        self.anim.target()
    }

    pub fn is_settled(&self) -> bool {
        self.anim.is_settled()
    }

    pub fn update(&mut self, dt: f32) {
        self.anim.step(dt);
    }

    pub fn snap(&mut self, index: usize) {
        self.index = index;
        self.anim.snap(index as f32);
    }

    /// Ease toward `index` without snapping (used when switching rails).
    pub fn ease_to(&mut self, index: usize) {
        self.index = index;
        self.anim.set_target(index as f32);
    }

    pub fn step(&mut self, delta: i32, count: usize) {
        if count == 0 || delta == 0 {
            return;
        }
        if self.wrap {
            let n = count as f32;
            let new_target = self.anim.target() + delta as f32;
            self.anim.set_target(new_target);
            self.index = new_target.rem_euclid(n) as usize;
        } else {
            let last = count.saturating_sub(1);
            let next = (self.index as i32 + delta).clamp(0, last as i32) as usize;
            if next != self.index {
                self.index = next;
                self.anim.set_target(next as f32);
            }
        }
    }

    /// While a direction is held, keep the tween target far enough ahead that
    /// motion stays continuous instead of settling per card.
    pub fn hold_advance(&mut self, delta: i32, count: usize) {
        if count == 0 || delta == 0 {
            return;
        }
        let ahead = if delta > 0 {
            self.anim.target() - self.anim.value()
        } else {
            self.anim.value() - self.anim.target()
        };
        if ahead < HOLD_AHEAD {
            self.step(delta, count);
        }
    }

    /// On key release: ease out forward in the travel direction.
    ///
    /// Always commits to the next card ahead (never eases backward). If that
    /// stop would be too close for a smooth ease, coast one more card.
    pub fn release_ease(&mut self, delta: i32, count: usize) {
        if count == 0 {
            return;
        }
        let v = self.anim.value();
        let mut target = if delta > 0 {
            v.ceil()
        } else if delta < 0 {
            v.floor()
        } else {
            v.round()
        };

        // Already on an integer: stay there (ceil/floor of 3.0 is 3.0).
        // Too close for a smooth stop — keep going one more card.
        if delta > 0 && target > v && (target - v) < RELEASE_MIN_RUN {
            target += 1.0;
        } else if delta < 0 && target < v && (v - target) < RELEASE_MIN_RUN {
            target -= 1.0;
        }

        if self.wrap {
            let n = count as f32;
            self.anim.set_target(target);
            self.index = target.rem_euclid(n) as usize;
        } else {
            let last = (count.saturating_sub(1)) as f32;
            target = target.clamp(0.0, last);
            self.anim.set_target(target);
            self.index = target as usize;
        }
    }

    pub fn normalize(&mut self, count: usize) {
        if !self.wrap || count == 0 || !self.anim.is_settled() {
            return;
        }
        let n = count as f32;
        let v = self.anim.value();
        let normalized = v.rem_euclid(n);
        if (normalized - v).abs() > 1e-3 {
            self.anim.snap(normalized);
        }
    }
}

/// Whether card posters may upload new GPU textures this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageDraw {
    /// Normal path — decode/upload on demand.
    All,
    /// Only textures already on the GPU (placeholders otherwise).
    CachedOnly,
}

/// Draw a horizontal rail of cards inside `bounds`, sliding so column
/// `anim_col` sits at `focus_x` (absolute design X).
///
/// Three passes (fills → images → borders) so color batches flush once
/// before textured quads instead of once per card.
pub fn draw_card_row(
    r: &mut dyn Renderer,
    metrics: &Metrics,
    bounds: Rect,
    cards: &[Card],
    anim_col: f32,
    focus_x: f32,
    images: ImageDraw,
) {
    let card_w = metrics.card_w;
    let card_h = metrics.card_h;
    let step = metrics.card_step();
    let row_y = bounds.y;

    let card_visible = |ci: usize| -> Option<Rect> {
        let x = focus_x + (ci as f32 - anim_col) * step;
        let card_rect = Rect::new(x, row_y, card_w, card_h);
        if card_rect.right() < bounds.x || card_rect.x > bounds.right() {
            return None;
        }
        if card_rect.bottom() < bounds.y || card_rect.y > bounds.bottom() {
            return None;
        }
        Some(card_rect)
    };

    for (ci, _) in cards.iter().enumerate() {
        if let Some(rect) = card_visible(ci) {
            card::draw_card_bg(r, rect);
        }
    }

    let mut blits = Vec::new();
    for (ci, item) in cards.iter().enumerate() {
        if let Some(rect) = card_visible(ci) {
            let (x, y, w, h) = rect.as_i32();
            blits.push(ImageBlit {
                x,
                y,
                w,
                h,
                url: &item.image_url,
            });
        }
    }
    match images {
        ImageDraw::All => r.draw_images(&blits),
        ImageDraw::CachedOnly => r.draw_images_cached(&blits),
    }

    for (ci, _) in cards.iter().enumerate() {
        if let Some(rect) = card_visible(ci) {
            card::draw_card_border(r, rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_stops_at_ends() {
        let mut c = HCarousel::new(false);
        c.step(-1, 10);
        assert_eq!(c.index(), 0);
        for _ in 0..50 {
            c.step(1, 10);
        }
        assert_eq!(c.index(), 9);
    }

    #[test]
    fn wrap_uses_unbounded_target() {
        let mut c = HCarousel::new(true);
        c.step(-1, 5);
        assert_eq!(c.index(), 4);
        assert!((c.target() - (-1.0)).abs() < 1e-4);
        c.step(1, 5);
        assert_eq!(c.index(), 0);
        assert!((c.target() - 0.0).abs() < 1e-4);
    }

    #[test]
    fn fast_wrap_keeps_direction() {
        let mut c = HCarousel::new(true);
        let n = 5usize;
        for _ in 0..n {
            c.step(1, n);
        }
        assert_eq!(c.index(), 0);
        assert!((c.target() - n as f32).abs() < 1e-4);
        c.step(1, n);
        assert_eq!(c.index(), 1);
        assert!((c.target() - (n as f32 + 1.0)).abs() < 1e-4);
    }

    #[test]
    fn normalize_folds_after_settle() {
        let mut c = HCarousel::new(true);
        for _ in 0..5 {
            c.step(1, 5);
        }
        for _ in 0..120 {
            c.update(1.0 / 60.0);
            c.normalize(5);
        }
        assert!(c.is_settled());
        assert!((c.anim_value() - 0.0).abs() < 1e-3);
    }

    #[test]
    fn hold_advance_keeps_runway_ahead() {
        let mut c = HCarousel::new(false);
        c.step(1, 20);
        // Mimic a held Right: never let the tween settle between cards.
        for _ in 0..30 {
            c.update(1.0 / 60.0);
            c.hold_advance(1, 20);
            let ahead = c.target() - c.anim_value();
            assert!(
                ahead > 0.15 || c.index() >= 19,
                "expected continuous runway, ahead={}",
                ahead
            );
        }
        assert!(c.target() > 2.0, "should have queued multiple cards while held");
    }

    #[test]
    fn release_ease_drops_far_target() {
        let mut c = HCarousel::new(false);
        c.step(1, 20);
        c.step(1, 20);
        c.step(1, 20);
        c.update(1.0 / 60.0);
        let v = c.anim_value();
        assert!(c.target() > v + 1.0);
        c.release_ease(1, 20);
        // Should not keep the far queued target.
        assert!(c.target() < v + 2.5);
        assert!(c.target() >= v.ceil() - 1e-3);
    }

    #[test]
    fn release_ease_coasts_when_too_close_to_stop() {
        let mut c = HCarousel::new(false);
        c.snap(0);
        c.ease_to(1);
        // Advance until within MIN_RUN of target 1.
        for _ in 0..200 {
            c.update(1.0 / 60.0);
            let v = c.anim_value();
            if (1.0 - v) < RELEASE_MIN_RUN && (1.0 - v) > 0.05 {
                break;
            }
        }
        let v = c.anim_value();
        assert!(1.0 - v < RELEASE_MIN_RUN, "precondition: tight remaining, v={}", v);
        c.release_ease(1, 20);
        assert!(
            c.target() >= 2.0 - 1e-3,
            "should coast to card 2, got target {}",
            c.target()
        );
    }

    #[test]
    fn release_ease_never_goes_backward() {
        let mut c = HCarousel::new(false);
        c.step(1, 20);
        c.update(1.0 / 60.0);
        let v = c.anim_value();
        c.release_ease(1, 20);
        assert!(
            c.target() >= v.ceil() - 1e-3,
            "must commit forward, v={} target={}",
            v,
            c.target()
        );
    }
}
