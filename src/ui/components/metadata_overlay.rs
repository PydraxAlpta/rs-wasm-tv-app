//! Metadata overlay for a selected card / banner.
//!
//! Slides up from below to cover the browse content under the top nav.

use crate::anim::Tween;
use crate::buffer::Color;
use crate::geom::Rect;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key};
use crate::theme;
use crate::ui::components::card;
use crate::ui::components::widget::{Flex, FocusResult, Widget};

const SLIDE_TAU: f32 = 0.16;
const POSTER_W: f32 = 420.0;
const POSTER_H: f32 = 630.0;
const PLAY_BTN_W: f32 = 280.0;
const PLAY_BTN_H: f32 = 72.0;

/// Payload shown in the overlay (title + art + catalog indices for filler copy).
#[derive(Debug, Clone)]
pub struct MetadataItem {
    pub title: String,
    pub image_url: String,
    pub rail_index: usize,
    pub card_index: usize,
}

/// Full-screen page with poster, filler metadata, and a focused Play action.
pub struct MetadataOverlay {
    item: Option<MetadataItem>,
    /// 0 = off-screen below, 1 = fully covering the content area under the nav.
    slide: Tween,
    /// True while open or still animating closed (so we keep painting / eating keys).
    active: bool,
    bounds: Rect,
}

impl MetadataOverlay {
    pub fn new() -> Self {
        Self {
            item: None,
            slide: Tween::new(0.0, SLIDE_TAU),
            active: false,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_open(&self) -> bool {
        self.active && self.slide.target() > 0.5
    }

    pub fn open(&mut self, item: MetadataItem) {
        self.item = Some(item);
        self.active = true;
        self.slide.set_target(1.0);
    }

    pub fn close(&mut self) {
        self.slide.set_target(0.0);
    }

    pub fn item(&self) -> Option<&MetadataItem> {
        self.item.as_ref()
    }

    /// Page rect translated up from below as `slide` goes 0 → 1.
    /// Uses [`layout`](Widget::layout) bounds so the overlay tracks a sliding tab page.
    fn page_rect(&self) -> Rect {
        let t = self.slide.value().clamp(0.0, 1.0);
        let full = if self.bounds.w > 1.0 {
            self.bounds
        } else {
            Rect::design()
        };
        Rect::new(full.x, full.y + full.h * (1.0 - t), full.w, full.h)
    }
}

impl Default for MetadataOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for MetadataOverlay {
    fn flex(&self) -> Flex {
        Flex::Fixed(0.0)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn update(&mut self, dt: f32, _ctx: &mut Ctx) {
        if !self.active {
            return;
        }
        self.slide.step(dt);
        if self.slide.target() < 0.5 && self.slide.is_settled() {
            self.active = false;
            self.item = None;
        }
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        if !self.active {
            return;
        }
        let Some(item) = self.item.as_ref() else {
            return;
        };
        let t = self.slide.value().clamp(0.0, 1.0);
        if t <= 0.001 {
            return;
        }

        let page = self.page_rect();
        let (px, py, pw, ph) = page.as_i32();
        r.fill_rect(px, py, pw, ph, theme::BG);

        let margin = ctx.metrics.safe_margin;
        let content_top = page.y + margin * 1.2;
        let poster = Rect::new(page.x + margin, content_top, POSTER_W, POSTER_H);
        card::draw_card(r, poster, &item.image_url);

        let text_x = (poster.right() + 56.0) as i32;
        let text_right = (page.right() - margin) as i32;
        let text_w = (text_right - text_x).max(100);

        r.draw_text(
            text_x,
            content_top as i32,
            56,
            theme::HEADER,
            &item.title,
        );

        let rail_n = item.rail_index + 1;
        let card_n = item.card_index + 1;
        let genre = filler_genre(item.rail_index, item.card_index);
        let runtime = 40 + (item.rail_index * 7 + item.card_index * 3) % 90;
        let rating = 6.0 + ((item.rail_index + item.card_index * 2) % 40) as f32 / 10.0;
        let year = 2012 + (item.rail_index * 3 + item.card_index) % 14;

        let meta_line = format!(
            "{genre}  ·  {year}  ·  {runtime} min  ·  ★ {rating:.1}"
        );
        r.draw_text(
            text_x,
            (content_top + 80.0) as i32,
            28,
            theme::TEXT_DIM,
            &meta_line,
        );
        r.draw_text(
            text_x,
            (content_top + 120.0) as i32,
            26,
            theme::RAIL_TITLE,
            &format!("Rail {rail_n}  ·  Card #{card_n}"),
        );

        let synopsis = filler_synopsis(item.rail_index, item.card_index, &item.title);
        draw_wrapped_text(
            r,
            text_x,
            (content_top + 180.0) as i32,
            text_w,
            28,
            38,
            8,
            theme::TEXT_DIM,
            &synopsis,
        );

        let btn = Rect::new(
            text_x as f32,
            poster.bottom() - PLAY_BTN_H,
            PLAY_BTN_W,
            PLAY_BTN_H,
        );
        let (bx, by, bw, bh) = btn.as_i32();
        r.fill_rect(bx, by, bw, bh, theme::FOCUS);
        r.draw_text(bx + 56, by + 18, 34, theme::BG, "▶  Play");
        card::draw_focus_ring(r, btn);

        r.draw_text(
            bx + bw + 32,
            by + 24,
            24,
            theme::TEXT_DIM,
            "Enter: play   Back: close",
        );
    }

    fn handle_key(&mut self, key: Key, _ctx: &mut Ctx) -> FocusResult {
        if !self.is_open() {
            return FocusResult::Ignored;
        }
        match key {
            Key::Enter => FocusResult::Activate,
            Key::Back | Key::Up => {
                self.close();
                FocusResult::Handled
            }
            Key::Down | Key::Left | Key::Right => FocusResult::Handled,
        }
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

fn filler_genre(rail: usize, card: usize) -> &'static str {
    const GENRES: &[&str] = &[
        "Drama",
        "Thriller",
        "Sci-Fi",
        "Comedy",
        "Documentary",
        "Action",
        "Mystery",
        "Adventure",
    ];
    GENRES[(rail * 3 + card) % GENRES.len()]
}

fn filler_synopsis(rail: usize, card: usize, title: &str) -> String {
    format!(
        "{title} — Rail {rn} item {cn}. When a quiet town discovers a signal buried under \
         decades of silence, unlikely allies must decide what to protect and what to reveal. \
         Secrets surface, loyalties shift, and the next choice rewrites everything they thought \
         they knew about home. Cast credits and episode notes are placeholders keyed to this \
         rail and card so every title feels distinct while browsing.",
        rn = rail + 1,
        cn = card + 1,
    )
}

/// Very small word-wrap for the overlay synopsis (fixed line budget).
fn draw_wrapped_text(
    r: &mut dyn Renderer,
    x: i32,
    y: i32,
    max_w: i32,
    size: i32,
    line_h: i32,
    max_lines: usize,
    color: Color,
    text: &str,
) {
    let approx_char_w = (size as f32 * 0.52) as i32;
    let chars_per_line = (max_w / approx_char_w.max(1)).max(8) as usize;

    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in words {
        let next = if cur.is_empty() {
            w.to_string()
        } else {
            format!("{cur} {w}")
        };
        if next.len() <= chars_per_line {
            cur = next;
        } else {
            if !cur.is_empty() {
                lines.push(cur);
            }
            cur = w.to_string();
            if lines.len() >= max_lines {
                break;
            }
        }
    }
    if !cur.is_empty() && lines.len() < max_lines {
        lines.push(cur);
    }
    if lines.len() == max_lines {
        if let Some(last) = lines.last_mut() {
            if last.len() > 3 {
                last.truncate(last.len().saturating_sub(3));
                last.push_str("...");
            }
        }
    }

    for (i, line) in lines.iter().enumerate() {
        r.draw_text(x, y + i as i32 * line_h, size, color, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_sets_slide_target() {
        let mut o = MetadataOverlay::new();
        o.open(MetadataItem {
            title: "T".into(),
            image_url: "u".into(),
            rail_index: 2,
            card_index: 4,
        });
        assert!(o.is_active());
        assert!(o.is_open());
        assert!((o.slide.target() - 1.0).abs() < 1e-4);
        assert_eq!(o.item().unwrap().rail_index, 2);
    }

    #[test]
    fn close_animates_toward_zero() {
        let mut o = MetadataOverlay::new();
        o.open(MetadataItem {
            title: "T".into(),
            image_url: "u".into(),
            rail_index: 0,
            card_index: 0,
        });
        o.close();
        assert!((o.slide.target() - 0.0).abs() < 1e-4);
        assert!(o.is_active());
    }

    #[test]
    fn page_covers_content_below_header_when_open() {
        let header_h = 140.0;
        let mut o = MetadataOverlay::new();
        o.layout(Rect::new(
            0.0,
            header_h,
            crate::DESIGN_WIDTH as f32,
            crate::DESIGN_HEIGHT as f32 - header_h,
        ));
        o.open(MetadataItem {
            title: "T".into(),
            image_url: "u".into(),
            rail_index: 0,
            card_index: 0,
        });
        o.slide.snap(1.0);
        let page = o.page_rect();
        assert!((page.x - 0.0).abs() < 1e-3);
        assert!((page.y - header_h).abs() < 1e-3);
        assert!((page.w - crate::DESIGN_WIDTH as f32).abs() < 1e-3);
        assert!((page.h - (crate::DESIGN_HEIGHT as f32 - header_h)).abs() < 1e-3);
    }
}
