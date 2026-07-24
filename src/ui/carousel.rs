//! Horizontal carousel navigation + card-row rendering.
//!
//! [`HCarousel`] owns the focused index and slide tween. Card rails clamp at
//! the ends; wrapping carousels (e.g. the hero) use unbounded targets so rapid
//! wraps never reverse mid-flight.

use crate::anim::Tween;
use crate::layout::Layout;
use crate::model::Card;
use crate::renderer::Renderer;
use crate::ui::card;

/// Time-constant (seconds) for carousel easing — small = snappy.
pub const NAV_TAU: f32 = 0.11;

/// Horizontal index + animated fractional offset.
#[derive(Debug, Clone)]
pub struct HCarousel {
    index: usize,
    anim: Tween,
    wrap: bool,
}

impl HCarousel {
    pub fn new(wrap: bool) -> Self {
        Self {
            index: 0,
            anim: Tween::new(0.0, NAV_TAU),
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

    /// Jump index + animation with no easing (e.g. when switching rails).
    pub fn snap(&mut self, index: usize) {
        self.index = index;
        self.anim.snap(index as f32);
    }

    /// Move by `delta` (−1 / +1). Clamps when `!wrap`; wraps with an unbounded
    /// target when `wrap` so fast loops keep direction.
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

    /// After a wrapping scroll settles, fold the unbounded value into `0..n`.
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

/// Draw one horizontal rail of cards at `row_y`, sliding behind a fixed focus
/// anchor (`layout.focus_x`). Culls off-screen tiles and anything above `cull_top`.
pub fn draw_card_row(
    r: &mut dyn Renderer,
    layout: &Layout,
    cards: &[Card],
    row_y: f32,
    anim_col: f32,
    cull_top: f32,
) {
    let card_w = layout.card_w as i32;
    let card_h = layout.card_h as i32;
    let row_top_i = row_y as i32;
    let dw = layout.design_w;

    if row_y + layout.card_h < cull_top || row_y > layout.design_h {
        return;
    }

    for (ci, item) in cards.iter().enumerate() {
        let x = layout.card_x(ci, anim_col);
        if x + layout.card_w < 0.0 || x > dw {
            continue;
        }
        if row_y + layout.card_h < cull_top {
            continue;
        }
        card::draw_card(r, x as i32, row_top_i, card_w, card_h, &item.image_url);
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
        c.step(1, n);
        assert_eq!(c.index(), 2);
        assert!((c.target() - (n as f32 + 2.0)).abs() < 1e-4);
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
        assert_eq!(c.index(), 0);
    }
}
