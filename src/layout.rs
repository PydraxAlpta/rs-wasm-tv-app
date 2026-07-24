//! All tunable geometry lives here — the single knob for changing how the
//! browse page is laid out. Coordinates are in the fixed 1920×1080 design space.

/// Layout constants for the browse page.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub design_w: f32,
    pub design_h: f32,
    /// TV overscan-safe inset from the edges.
    pub safe_margin: f32,

    /// Fixed page-header band height; rails scroll only below this.
    pub header_h: f32,

    pub card_w: f32,
    pub card_h: f32,
    pub card_gap: f32,

    /// Vertical space reserved above a card row for its rail title.
    pub rail_title_h: f32,
    /// Vertical distance between consecutive rail rows.
    pub rail_step: f32,

    /// Fixed on-screen focus anchor (top-left of the focused card slot).
    pub focus_x: f32,
    pub focus_y: f32,
    /// Scale applied to the focused card (reserved for polish; unused by v1).
    pub focus_scale: f32,
}

impl Layout {
    pub fn tv() -> Self {
        let safe_margin = 64.0;
        let header_h = 140.0;
        let rail_title_h = 44.0;
        // Portrait poster tiles (~2:3).
        let card_w = 200.0;
        let card_h = 300.0;
        Self {
            design_w: 1920.0,
            design_h: 1080.0,
            safe_margin,
            header_h,
            card_w,
            card_h,
            card_gap: 24.0,
            rail_title_h,
            // Title + card + focused-title line + gap before next rail title.
            rail_step: rail_title_h + card_h + 56.0 + 36.0,
            focus_x: safe_margin,
            // First rail's cards sit just under the header + its rail title.
            focus_y: header_h + rail_title_h + 8.0,
            focus_scale: 1.08,
        }
    }

    /// Horizontal distance between the left edges of adjacent cards.
    pub fn card_step(&self) -> f32 {
        self.card_w + self.card_gap
    }

    /// Left edge of card `col` given the animated fractional focused column.
    /// When `anim_col == col` the card sits exactly at `focus_x`.
    pub fn card_x(&self, col: usize, anim_col: f32) -> f32 {
        self.focus_x + (col as f32 - anim_col) * self.card_step()
    }

    /// Top of rail `rail`'s card row given the animated fractional focused rail.
    /// When `anim_rail == rail` the row sits exactly at `focus_y`.
    pub fn rail_y(&self, rail: usize, anim_rail: f32) -> f32 {
        self.focus_y + (rail as f32 - anim_rail) * self.rail_step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_card_sits_at_focus_anchor() {
        let l = Layout::tv();
        // Settled on column 3 → card 3 lands exactly at focus_x.
        assert!((l.card_x(3, 3.0) - l.focus_x).abs() < 1e-4);
        // Settled on rail 2 → rail 2 row lands exactly at focus_y.
        assert!((l.rail_y(2, 2.0) - l.focus_y).abs() < 1e-4);
    }

    #[test]
    fn later_columns_are_to_the_right() {
        let l = Layout::tv();
        assert!(l.card_x(5, 0.0) > l.card_x(4, 0.0));
        assert!((l.card_x(1, 0.0) - l.card_x(0, 0.0) - l.card_step()).abs() < 1e-4);
    }

    #[test]
    fn focus_anchor_sits_below_header() {
        let l = Layout::tv();
        assert!(l.focus_y - l.rail_title_h >= l.header_h);
        assert!(l.card_h > l.card_w); // portrait tiles
    }
}
