//! Browse screen: vertically stacked rails of portrait cards with a fixed
//! focus anchor and a static page header.
//!
//! Card/rail positions derive from *animated fractional* focused indices
//! (`anim_col`, `anim_rail`), so the focus frame stays pinned at
//! `layout.focus_x/focus_y` while the content slides behind it — only in the
//! area below `layout.header_h`. Vertical moves and horizontal moves within a
//! rail animate; switching rails snaps the horizontal offset to that rail's
//! remembered column so it doesn't slide sideways while moving up/down.

use crate::anim::Tween;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key, Screen, Transition};
use crate::theme;
use crate::ui::player::PlayerScreen;

/// Time-constant (seconds) for navigation easing — small = snappy.
const NAV_TAU: f32 = 0.11;

pub struct BrowseScreen {
    focus_rail: usize,
    /// Remembered column per rail (so returning to a rail keeps its position).
    focus_col: Vec<usize>,
    anim_rail: Tween,
    anim_col: Tween,
    initialized: bool,
}

impl BrowseScreen {
    pub fn new() -> Self {
        Self {
            focus_rail: 0,
            focus_col: Vec::new(),
            anim_rail: Tween::new(0.0, NAV_TAU),
            anim_col: Tween::new(0.0, NAV_TAU),
            initialized: false,
        }
    }

    /// Currently focused (rail, column) — exposed for tests.
    pub fn focus(&self) -> (usize, usize) {
        (
            self.focus_rail,
            self.focus_col.get(self.focus_rail).copied().unwrap_or(0),
        )
    }

    fn ensure_init(&mut self, ctx: &Ctx) {
        if !self.initialized {
            self.focus_col = vec![0; ctx.catalog.rails.len()];
            self.initialized = true;
        }
    }

    fn current_col(&self) -> usize {
        self.focus_col.get(self.focus_rail).copied().unwrap_or(0)
    }
}

impl Default for BrowseScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for BrowseScreen {
    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        self.ensure_init(ctx);
        self.anim_rail.step(dt);
        self.anim_col.step(dt);
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> Transition {
        self.ensure_init(ctx);
        let rails = &ctx.catalog.rails;
        if rails.is_empty() {
            return Transition::None;
        }

        match key {
            Key::Left => {
                let col = self.current_col();
                if col > 0 {
                    self.focus_col[self.focus_rail] = col - 1;
                    self.anim_col.set_target((col - 1) as f32);
                }
            }
            Key::Right => {
                let col = self.current_col();
                let last = rails[self.focus_rail].cards.len().saturating_sub(1);
                if col < last {
                    self.focus_col[self.focus_rail] = col + 1;
                    self.anim_col.set_target((col + 1) as f32);
                }
            }
            Key::Up => {
                if self.focus_rail > 0 {
                    self.focus_rail -= 1;
                    self.anim_rail.set_target(self.focus_rail as f32);
                    // Show the new rail at its remembered column without sliding.
                    self.anim_col.snap(self.current_col() as f32);
                }
            }
            Key::Down => {
                if self.focus_rail + 1 < rails.len() {
                    self.focus_rail += 1;
                    self.anim_rail.set_target(self.focus_rail as f32);
                    self.anim_col.snap(self.current_col() as f32);
                }
            }
            Key::Enter => {
                let col = self.current_col();
                if let Some(card) = rails[self.focus_rail].cards.get(col) {
                    return Transition::Push(Box::new(PlayerScreen::new(card.title.clone())));
                }
            }
            Key::Back => {}
        }
        Transition::None
    }

    fn render(&mut self, r: &mut dyn Renderer, ctx: &mut Ctx) {
        self.ensure_init(ctx);
        let l = ctx.layout;
        let dw = l.design_w as i32;
        let header_h = l.header_h as i32;

        // Opaque background (the GL canvas itself is transparent).
        r.fill_rect(0, 0, dw, l.design_h as i32, theme::BG);

        let anim_rail = self.anim_rail.value();
        let card_w = l.card_w as i32;
        let card_h = l.card_h as i32;

        for (ri, rail) in ctx.catalog.rails.iter().enumerate() {
            let row_top = l.rail_y(ri, anim_rail);
            let title_y = row_top - l.rail_title_h;
            // Cull rails fully above the header band or fully below the screen.
            if row_top + l.card_h < l.header_h || title_y > l.design_h {
                continue;
            }
            let row_top_i = row_top as i32;

            let col_off = if ri == self.focus_rail {
                self.anim_col.value()
            } else {
                self.focus_col.get(ri).copied().unwrap_or(0) as f32
            };

            // Rail title (skip if it sits under the header band).
            if title_y + 30.0 >= l.header_h {
                r.draw_text(
                    l.safe_margin as i32,
                    title_y as i32,
                    30,
                    theme::RAIL_TITLE,
                    &rail.title,
                );
            }

            for (ci, card) in rail.cards.iter().enumerate() {
                let x = l.card_x(ci, col_off);
                // Cull cards outside the visible content area.
                if x + l.card_w < 0.0 || x > l.design_w {
                    continue;
                }
                if row_top + l.card_h < l.header_h || row_top > l.design_h {
                    continue;
                }
                let xi = x as i32;
                // Placeholder fill (shown until the image finishes loading), then art.
                r.fill_rect(xi, row_top_i, card_w, card_h, theme::CARD_BG);
                r.draw_image(xi, row_top_i, card_w, card_h, &card.image_url);
                r.stroke_rect(xi, row_top_i, card_w, card_h, theme::CARD_BORDER);
            }
        }

        // Focus frame — fixed anchor above the cards.
        let fx = l.focus_x as i32;
        let fy = l.focus_y as i32;
        for i in 0..4 {
            let a = 200u8.saturating_sub(i as u8 * 45);
            r.stroke_rect(
                fx - i,
                fy - i,
                card_w + 2 * i,
                card_h + 2 * i,
                theme::FOCUS.with_alpha(a),
            );
        }

        // Focused card title, below the focus frame.
        if let Some(rail) = ctx.catalog.rails.get(self.focus_rail) {
            if let Some(card) = rail.cards.get(self.current_col()) {
                r.draw_text(fx, fy + card_h + 14, 28, theme::TEXT, &card.title);
            }
        }

        // Static header band — painted after rails so scrolling content never
        // covers the app name.
        r.fill_rect(0, 0, dw, header_h, theme::BG);
        r.draw_text(
            l.safe_margin as i32,
            (l.safe_margin * 0.55) as i32,
            52,
            theme::HEADER,
            "WASM TV",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Layout;
    use crate::model::Catalog;
    use crate::screen::{Ctx, VideoSink};

    struct NullSink;
    impl VideoSink for NullSink {
        fn load_and_play(&mut self, _url: &str) {}
        fn play(&mut self) {}
        fn pause(&mut self) {}
        fn is_paused(&self) -> bool {
            true
        }
        fn current_time(&self) -> f64 {
            0.0
        }
        fn duration(&self) -> f64 {
            0.0
        }
        fn seek(&mut self, _t: f64) {}
        fn set_visible(&mut self, _v: bool) {}
    }

    fn with_ctx(f: impl FnOnce(&mut BrowseScreen, &mut Ctx)) -> BrowseScreen {
        let cat = Catalog::sample();
        let lay = Layout::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            layout: &lay,
            video: &mut video,
        };
        let mut screen = BrowseScreen::new();
        f(&mut screen, &mut ctx);
        screen
    }

    #[test]
    fn right_advances_and_clamps_at_end() {
        let s = with_ctx(|s, ctx| {
            for _ in 0..50 {
                s.handle_key(Key::Right, ctx);
            }
        });
        // 10 cards → last index 9, never overflows.
        assert_eq!(s.focus(), (0, 9));
    }

    #[test]
    fn left_clamps_at_zero() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Left, ctx);
            s.handle_key(Key::Left, ctx);
        });
        assert_eq!(s.focus(), (0, 0));
    }

    #[test]
    fn down_advances_and_clamps_at_last_rail() {
        let s = with_ctx(|s, ctx| {
            for _ in 0..50 {
                s.handle_key(Key::Down, ctx);
            }
        });
        assert_eq!(s.focus().0, 19); // 20 rails → last index 19
    }

    #[test]
    fn column_is_remembered_per_rail() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Right, ctx); // rail 0 → col 1
            s.handle_key(Key::Right, ctx); // rail 0 → col 2
            s.handle_key(Key::Down, ctx); // rail 1, col 0
            s.handle_key(Key::Right, ctx); // rail 1 → col 1
            s.handle_key(Key::Up, ctx); // back to rail 0
        });
        // Rail 0 remembered col 2.
        assert_eq!(s.focus(), (0, 2));
    }

    #[test]
    fn enter_pushes_player() {
        let cat = Catalog::sample();
        let lay = Layout::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            layout: &lay,
            video: &mut video,
        };
        let mut screen = BrowseScreen::new();
        let t = screen.handle_key(Key::Enter, &mut ctx);
        assert!(matches!(t, Transition::Push(_)));
    }
}
