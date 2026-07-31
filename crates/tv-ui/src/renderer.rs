//! Pluggable draw backend: the stable set of drawing primitives the UI calls.

use crate::buffer::Color;

/// One image blit for [`Renderer::draw_images`]: stretch `url` into the dest rect.
#[derive(Debug, Clone, Copy)]
pub struct ImageBlit<'a> {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub url: &'a str,
    /// Corner radius in design pixels; `0` is a sharp rect.
    pub radius: i32,
}

/// Clamp a corner radius so it never exceeds half the shorter side.
pub fn clamp_corner_radius(width: i32, height: i32, radius: i32) -> i32 {
    if width <= 0 || height <= 0 || radius <= 0 {
        return 0;
    }
    radius.min(width.min(height) / 2)
}

/// Backend surface. Grows only when a **new drawing primitive** is genuinely
/// needed; higher-level UI should compose these methods instead of extending
/// the trait. `fill_rect`/`stroke_rect`/`fill_round_rect`/`stroke_round_rect`
/// are provided compositions.
pub trait Renderer {
    fn begin_frame(&mut self, clear: Color);
    fn end_frame(&mut self);

    fn stroke_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color);
    fn stroke_circle(&mut self, cx: i32, cy: i32, radius: i32, color: Color);
    fn fill_circle(&mut self, cx: i32, cy: i32, radius: i32, color: Color);
    fn fill_triangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: Color,
    );

    /// Draw an image stretched into the destination rect.
    ///
    /// `radius` rounds the corners (design pixels); `0` is sharp. Backends
    /// load/cache by URL asynchronously; until ready this may no-op.
    fn draw_image(&mut self, x: i32, y: i32, width: i32, height: i32, url: &str, radius: i32);

    /// Draw an image only if it is already GPU-resident.
    ///
    /// Used during motion to avoid decode/upload hitching the frame. Default
    /// falls back to [`Self::draw_image`].
    fn draw_image_cached(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        url: &str,
        radius: i32,
    ) {
        self.draw_image(x, y, width, height, url, radius);
    }

    /// Draw many images in one batch (backends may use a texture array).
    ///
    /// Default loops [`Self::draw_image`]. Empty slices are a no-op.
    fn draw_images(&mut self, images: &[ImageBlit<'_>]) {
        for img in images {
            if img.w <= 0 || img.h <= 0 {
                continue;
            }
            self.draw_image(img.x, img.y, img.w, img.h, img.url, img.radius);
        }
    }

    /// Like [`Self::draw_images`], but only GPU-resident textures (motion path).
    ///
    /// Default loops [`Self::draw_image_cached`].
    fn draw_images_cached(&mut self, images: &[ImageBlit<'_>]) {
        for img in images {
            if img.w <= 0 || img.h <= 0 {
                continue;
            }
            self.draw_image_cached(img.x, img.y, img.w, img.h, img.url, img.radius);
        }
    }

    /// Kick off an async image load without drawing. Default no-op.
    fn prefetch_image(&mut self, _url: &str) {}

    /// Draw text with top-left at `(x, y)`. `size` is font size in CSS pixels
    /// (glyph height), rendered with a system sans-serif font.
    fn draw_text(&mut self, x: i32, y: i32, size: i32, color: Color, text: &str);

    /// Restrict subsequent drawing to `clip` in design-space pixels, or clear
    /// the clip when `None`. Backends that lack scissor may no-op.
    fn set_clip(&mut self, _clip: Option<crate::geom::Rect>) {}

    /// Axis-aligned filled rectangle, composed from two triangles.
    fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        if width <= 0 || height <= 0 {
            return;
        }
        let (x0, y0) = (x, y);
        let (x1, y1) = (x + width, y + height);
        self.fill_triangle(x0, y0, x1, y0, x0, y1, color);
        self.fill_triangle(x1, y0, x1, y1, x0, y1, color);
    }

    /// Axis-aligned rectangle outline, composed from four lines.
    fn stroke_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        if width <= 0 || height <= 0 {
            return;
        }
        let (x0, y0) = (x, y);
        let (x1, y1) = (x + width, y + height);
        self.stroke_line(x0, y0, x1, y0, color);
        self.stroke_line(x1, y0, x1, y1, color);
        self.stroke_line(x1, y1, x0, y1, color);
        self.stroke_line(x0, y1, x0, y0, color);
    }

    /// Filled axis-aligned rounded rectangle.
    ///
    /// Default tessellates via [`Self::fill_rect`] + corner [`Self::fill_circle`]s.
    /// Opaque fills only — translucent colors will double-blend at overlaps.
    fn fill_round_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        radius: i32,
        color: Color,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }
        let r = clamp_corner_radius(width, height, radius);
        if r == 0 {
            self.fill_rect(x, y, width, height, color);
            return;
        }
        // Center strip (full height) + left/right mid strips + four corner disks.
        self.fill_rect(x + r, y, width - 2 * r, height, color);
        self.fill_rect(x, y + r, r, height - 2 * r, color);
        self.fill_rect(x + width - r, y + r, r, height - 2 * r, color);
        self.fill_circle(x + r, y + r, r, color);
        self.fill_circle(x + width - r, y + r, r, color);
        self.fill_circle(x + r, y + height - r, r, color);
        self.fill_circle(x + width - r, y + height - r, r, color);
    }

    /// Hollow axis-aligned rounded rectangle outline.
    ///
    /// Default draws straight edges + quarter-circle arcs via [`Self::stroke_line`].
    fn stroke_round_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        radius: i32,
        color: Color,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }
        let r = clamp_corner_radius(width, height, radius);
        if r == 0 {
            self.stroke_rect(x, y, width, height, color);
            return;
        }
        let x1 = x + width;
        let y1 = y + height;
        self.stroke_line(x + r, y, x1 - r, y, color);
        self.stroke_line(x1, y + r, x1, y1 - r, color);
        self.stroke_line(x1 - r, y1, x + r, y1, color);
        self.stroke_line(x, y1 - r, x, y + r, color);
        // Y-down design space: angle 0 = right, π/2 = down.
        stroke_arc(self, x + r, y + r, r, std::f64::consts::PI, std::f64::consts::PI * 1.5, color);
        stroke_arc(
            self,
            x1 - r,
            y + r,
            r,
            std::f64::consts::PI * 1.5,
            std::f64::consts::TAU,
            color,
        );
        stroke_arc(self, x1 - r, y1 - r, r, 0.0, std::f64::consts::FRAC_PI_2, color);
        stroke_arc(
            self,
            x + r,
            y1 - r,
            r,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
            color,
        );
    }
}

const ARC_SEGMENTS_PER_TURN: i32 = 64;

fn stroke_arc<R: Renderer + ?Sized>(
    r: &mut R,
    cx: i32,
    cy: i32,
    radius: i32,
    a0: f64,
    a1: f64,
    color: Color,
) {
    if radius <= 0 {
        return;
    }
    let span = (a1 - a0).abs();
    let n = ((span / std::f64::consts::TAU) * ARC_SEGMENTS_PER_TURN as f64)
        .ceil()
        .max(1.0) as i32;
    let rr = radius as f64;
    for i in 0..n {
        let t0 = a0 + (a1 - a0) * (i as f64) / (n as f64);
        let t1 = a0 + (a1 - a0) * ((i + 1) as f64) / (n as f64);
        let x0 = cx + (rr * t0.cos()).round() as i32;
        let y0 = cy + (rr * t0.sin()).round() as i32;
        let x1 = cx + (rr * t1.cos()).round() as i32;
        let y1 = cy + (rr * t1.sin()).round() as i32;
        r.stroke_line(x0, y0, x1, y1, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_radius_respects_short_side() {
        assert_eq!(clamp_corner_radius(200, 300, 32), 32);
        assert_eq!(clamp_corner_radius(40, 300, 32), 20);
        assert_eq!(clamp_corner_radius(200, 300, 0), 0);
        assert_eq!(clamp_corner_radius(0, 300, 32), 0);
    }
}
