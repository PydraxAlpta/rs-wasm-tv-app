//! Theme-level geometry tokens for the TV UI (not page structure).

use crate::geom::Insets;

/// Spacing, card size, and type metrics shared across screens/widgets.
#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    pub safe_margin: f32,
    pub header_h: f32,
    pub banner_h: f32,
    pub card_w: f32,
    pub card_h: f32,
    pub card_gap: f32,
    pub rail_title_h: f32,
    pub rail_step: f32,
    /// Left edge of the fixed leanback focus card slot (within content).
    pub focus_x: f32,
}

impl Metrics {
    pub fn tv() -> Self {
        let safe_margin = 64.0;
        let rail_title_h = 44.0;
        let card_w = 200.0;
        let card_h = 300.0;
        Self {
            safe_margin,
            header_h: 140.0,
            banner_h: 420.0,
            card_w,
            card_h,
            card_gap: 24.0,
            rail_title_h,
            rail_step: rail_title_h + card_h + 56.0 + 36.0,
            focus_x: safe_margin,
        }
    }

    pub fn safe_insets(self) -> Insets {
        Insets::uniform(self.safe_margin)
    }

    pub fn card_step(self) -> f32 {
        self.card_w + self.card_gap
    }
}

/// Back-compat alias while call sites migrate.
pub type Layout = Metrics;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tv_metrics_are_portrait_cards() {
        let m = Metrics::tv();
        assert!(m.card_h > m.card_w);
        assert!(m.banner_h > 0.0);
    }
}
