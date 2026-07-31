//! Reusable poster card drawing primitives.

use crate::geom::Rect;
use crate::renderer::Renderer;
use crate::theme;

/// Draw a poster tile: placeholder, image, border.
pub fn draw_card(r: &mut dyn Renderer, rect: Rect, image_url: &str, radius: i32) {
    draw_card_bg(r, rect, radius);
    let (x, y, w, h) = rect.as_i32();
    r.draw_image(x, y, w, h, image_url, radius);
    draw_card_border(r, rect, radius);
}

/// Placeholder fill only (for batched row draws).
pub fn draw_card_bg(r: &mut dyn Renderer, rect: Rect, radius: i32) {
    let (x, y, w, h) = rect.as_i32();
    r.fill_round_rect(x, y, w, h, radius, theme::CARD_BG);
}

/// Border only (for batched row draws).
pub fn draw_card_border(r: &mut dyn Renderer, rect: Rect, radius: i32) {
    let (x, y, w, h) = rect.as_i32();
    r.stroke_round_rect(x, y, w, h, radius, theme::CARD_BORDER);
}

/// Multi-layer focus stroke used by cards and the hero banner.
pub fn draw_focus_ring(r: &mut dyn Renderer, rect: Rect, radius: i32) {
    draw_focus_ring_layers(r, rect, radius, 4);
}

/// Stronger ring for large surfaces (e.g. the hero banner).
pub fn draw_focus_ring_strong(r: &mut dyn Renderer, rect: Rect, radius: i32) {
    draw_focus_ring_layers(r, rect, radius, 7);
}

fn draw_focus_ring_layers(r: &mut dyn Renderer, rect: Rect, radius: i32, layers: i32) {
    let (x, y, w, h) = rect.as_i32();
    for i in 0..layers {
        // Full opacity: the WebGL SDF path blends, and translucent FOCUS over
        // dark cards reads much darker than the old unblended `stroke_rect` lines
        // (which wrote full RGB regardless of the alpha channel).
        r.stroke_round_rect(
            x - i,
            y - i,
            w + 2 * i,
            h + 2 * i,
            radius + i,
            theme::FOCUS,
        );
    }
}
