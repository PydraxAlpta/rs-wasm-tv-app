//! Reusable poster card drawing primitives.

use crate::renderer::Renderer;
use crate::theme;

/// Draw a poster tile: placeholder, image, border.
pub fn draw_card(r: &mut dyn Renderer, x: i32, y: i32, w: i32, h: i32, image_url: &str) {
    r.fill_rect(x, y, w, h, theme::CARD_BG);
    r.draw_image(x, y, w, h, image_url);
    r.stroke_rect(x, y, w, h, theme::CARD_BORDER);
}

/// Multi-layer focus stroke used by cards and the hero banner.
pub fn draw_focus_ring(r: &mut dyn Renderer, x: i32, y: i32, w: i32, h: i32) {
    for i in 0..4 {
        let a = 200u8.saturating_sub(i as u8 * 45);
        r.stroke_rect(x - i, y - i, w + 2 * i, h + 2 * i, theme::FOCUS.with_alpha(a));
    }
}
