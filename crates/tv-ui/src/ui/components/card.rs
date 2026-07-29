//! Reusable poster card drawing primitives.

use crate::geom::Rect;
use crate::renderer::Renderer;
use crate::theme;

/// Draw a poster tile: placeholder, image, border.
pub fn draw_card(r: &mut dyn Renderer, rect: Rect, image_url: &str) {
    draw_card_bg(r, rect);
    let (x, y, w, h) = rect.as_i32();
    r.draw_image(x, y, w, h, image_url);
    draw_card_border(r, rect);
}

/// Placeholder fill only (for batched row draws).
pub fn draw_card_bg(r: &mut dyn Renderer, rect: Rect) {
    let (x, y, w, h) = rect.as_i32();
    r.fill_rect(x, y, w, h, theme::CARD_BG);
}

/// Border only (for batched row draws).
pub fn draw_card_border(r: &mut dyn Renderer, rect: Rect) {
    let (x, y, w, h) = rect.as_i32();
    r.stroke_rect(x, y, w, h, theme::CARD_BORDER);
}

/// Multi-layer focus stroke used by cards and the hero banner.
pub fn draw_focus_ring(r: &mut dyn Renderer, rect: Rect) {
    draw_focus_ring_layers(r, rect, 4);
}

/// Stronger ring for large surfaces (e.g. the hero banner).
pub fn draw_focus_ring_strong(r: &mut dyn Renderer, rect: Rect) {
    draw_focus_ring_layers(r, rect, 7);
}

fn draw_focus_ring_layers(r: &mut dyn Renderer, rect: Rect, layers: i32) {
    let (x, y, w, h) = rect.as_i32();
    for i in 0..layers {
        let a = 220u8.saturating_sub(i as u8 * 28);
        r.stroke_rect(x - i, y - i, w + 2 * i, h + 2 * i, theme::FOCUS.with_alpha(a));
    }
}
