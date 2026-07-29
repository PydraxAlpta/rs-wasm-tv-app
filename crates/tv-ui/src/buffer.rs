//! Packed RGBA color used across the renderer and UI.

/// Packed RGBA color (0–255 per channel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    /// Same color with a different alpha (handy for overlays/scrims).
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const RED: Self = Self::rgb(220, 60, 60);
    pub const GREEN: Self = Self::rgb(60, 180, 80);
    pub const BLUE: Self = Self::rgb(70, 110, 220);
    pub const YELLOW: Self = Self::rgb(230, 200, 50);

    /// CSS `rgba(...)` string for Canvas 2D style APIs.
    pub fn to_css_rgba(self) -> String {
        format!(
            "rgba({},{},{},{:.3})",
            self.r,
            self.g,
            self.b,
            f64::from(self.a) / 255.0
        )
    }
}
