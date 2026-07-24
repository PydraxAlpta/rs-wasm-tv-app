//! Pluggable draw backend: the stable set of drawing primitives the UI calls.

use crate::buffer::Color;

/// Backend surface. Grows only when a **new drawing primitive** is genuinely
/// needed; higher-level UI should compose these methods instead of extending
/// the trait. `fill_rect`/`stroke_rect` are provided compositions.
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
    /// Backends load/cache by URL asynchronously; until ready this may no-op.
    fn draw_image(&mut self, x: i32, y: i32, width: i32, height: i32, url: &str);

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
}
