//! Browse screen: hero banner carousel + vertically stacked portrait rails.
//!
//! Card/rail positions derive from *animated fractional* focused indices
//! (`anim_col`, `anim_rail`), so the focus frame stays pinned while content
//! slides behind it. The full-width banner sits under the static header; it
//! collapses when leaving the first rail (rails slide up) and expands again
//! when returning. Banner slides change only via Left/Right while the banner
//! itself is focused (horizontal slide tween) — no auto-scroll.

use crate::anim::Tween;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key, Screen, Transition};
use crate::theme;
use crate::ui::player::PlayerScreen;

/// Time-constant (seconds) for navigation easing — small = snappy.
const NAV_TAU: f32 = 0.11;

const DOT_RADIUS: i32 = 7;
const DOT_GAP: i32 = 22;

pub struct BrowseScreen {
    focus_rail: usize,
    /// Remembered column per rail (so returning to a rail keeps its position).
    focus_col: Vec<usize>,
    /// Index into `catalog.banners`.
    banner_idx: usize,
    /// When true, Left/Right drive the banner instead of the focused rail.
    banner_focused: bool,
    anim_rail: Tween,
    anim_col: Tween,
    /// `1` = banner fully shown, `0` = collapsed (rail ≥ 1).
    anim_banner: Tween,
    /// Fractional banner index — drives the horizontal slide.
    anim_banner_slide: Tween,
    initialized: bool,
}

impl BrowseScreen {
    pub fn new() -> Self {
        Self {
            focus_rail: 0,
            focus_col: Vec::new(),
            banner_idx: 0,
            banner_focused: false,
            anim_rail: Tween::new(0.0, NAV_TAU),
            anim_col: Tween::new(0.0, NAV_TAU),
            anim_banner: Tween::new(1.0, NAV_TAU),
            anim_banner_slide: Tween::new(0.0, NAV_TAU),
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

    pub fn banner_index(&self) -> usize {
        self.banner_idx
    }

    pub fn banner_focused(&self) -> bool {
        self.banner_focused
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

    fn sync_banner_target(&mut self) {
        let t = if self.focus_rail == 0 { 1.0 } else { 0.0 };
        self.anim_banner.set_target(t);
    }

    /// Advance/retreat the banner with wrap-around. The slide tween target moves
    /// by ±1 from its *current target* (not the logical index) so rapid wraps
    /// never reverse direction mid-flight; [`normalize_banner_slide`] folds the
    /// unbounded value back into `0..n` once settled.
    fn step_banner(&mut self, delta: i32, count: usize) {
        if count == 0 || delta == 0 {
            return;
        }
        let n = count as f32;
        let new_target = self.anim_banner_slide.target() + delta as f32;
        self.anim_banner_slide.set_target(new_target);
        self.banner_idx = new_target.rem_euclid(n) as usize;
    }

    fn normalize_banner_slide(&mut self, count: usize) {
        if count == 0 || !self.anim_banner_slide.is_settled() {
            return;
        }
        let n = count as f32;
        let normalized = self.anim_banner_slide.value().rem_euclid(n);
        if (normalized - self.anim_banner_slide.value()).abs() > 1e-3 {
            self.anim_banner_slide.snap(normalized);
        }
    }

    fn move_to_rail(&mut self, rail: usize) {
        self.focus_rail = rail;
        self.anim_rail.set_target(rail as f32);
        self.anim_col.snap(self.current_col() as f32);
        if rail > 0 {
            self.banner_focused = false;
        }
        self.sync_banner_target();
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
        self.anim_banner.step(dt);
        self.anim_banner_slide.step(dt);
        self.normalize_banner_slide(ctx.catalog.banners.len());
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> Transition {
        self.ensure_init(ctx);
        let rails = &ctx.catalog.rails;
        let banners = &ctx.catalog.banners;
        if rails.is_empty() {
            return Transition::None;
        }

        if self.banner_focused {
            match key {
                Key::Left => self.step_banner(-1, banners.len()),
                Key::Right => self.step_banner(1, banners.len()),
                Key::Down => {
                    self.banner_focused = false;
                }
                Key::Up => {}
                Key::Enter => {
                    if let Some(slide) = banners.get(self.banner_idx) {
                        return Transition::Push(Box::new(PlayerScreen::new(slide.title.clone())));
                    }
                }
                Key::Back => {}
            }
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
                if self.focus_rail == 0 {
                    if !banners.is_empty() {
                        self.banner_focused = true;
                    }
                } else {
                    self.move_to_rail(self.focus_rail - 1);
                }
            }
            Key::Down => {
                if self.focus_rail + 1 < rails.len() {
                    self.move_to_rail(self.focus_rail + 1);
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
        let banner_t = self.anim_banner.value();
        let banner_h = l.banner_h as i32;

        // Opaque background (the GL canvas itself is transparent).
        r.fill_rect(0, 0, dw, l.design_h as i32, theme::BG);

        let anim_rail = self.anim_rail.value();
        let card_w = l.card_w as i32;
        let card_h = l.card_h as i32;
        let focus_y = l.focus_y(banner_t);

        for (ri, rail) in ctx.catalog.rails.iter().enumerate() {
            let row_top = l.rail_y(ri, anim_rail, banner_t);
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
                r.fill_rect(xi, row_top_i, card_w, card_h, theme::CARD_BG);
                r.draw_image(xi, row_top_i, card_w, card_h, &card.image_url);
                r.stroke_rect(xi, row_top_i, card_w, card_h, theme::CARD_BORDER);
            }
        }

        // Hero banner — full content width with TV safe insets; slides under the
        // header as `banner_t` collapses. Images slide horizontally on Left/Right.
        if banner_t > 0.001 {
            let margin = l.safe_margin as i32;
            let banner_x = margin;
            let banner_w = dw - 2 * margin;
            let banner_top = (l.header_h - l.banner_h * (1.0 - banner_t)) as i32;
            let slide_t = self.anim_banner_slide.value();
            let viewport_l = banner_x as f32;
            let viewport_r = (banner_x + banner_w) as f32;

            r.fill_rect(banner_x, banner_top, banner_w, banner_h, theme::CARD_BG);

            let banners = &ctx.catalog.banners;
            let n = banners.len() as i32;
            if n > 0 {
                let draw_at = |r: &mut dyn Renderer, logical_i: f32, url: &str| {
                    let x = viewport_l + (logical_i - slide_t) * banner_w as f32;
                    if x + banner_w as f32 <= viewport_l || x >= viewport_r {
                        return;
                    }
                    let xi = x as i32;
                    r.fill_rect(xi, banner_top, banner_w, banner_h, theme::CARD_BG);
                    r.draw_image(xi, banner_top, banner_w, banner_h, url);
                };

                // Draw modular copies around the unbounded slide position so
                // wrap-around (and fast multi-wrap) still slides one way.
                let base = slide_t.floor() as i32;
                for offset in -1..=2 {
                    let logical = base + offset;
                    let idx = logical.rem_euclid(n) as usize;
                    draw_at(r, logical as f32, &banners[idx].image_url);
                }
            }

            // Mask safe-margin gutters so sliding art doesn't spill past the inset.
            r.fill_rect(0, banner_top, banner_x, banner_h, theme::BG);
            r.fill_rect(banner_x + banner_w, banner_top, margin, banner_h, theme::BG);

            // Pagination dots — bottom-left of the banner.
            let dots_y = banner_top + banner_h - margin / 2;
            if dots_y > header_h {
                let dots_x0 = banner_x + margin / 2;
                for (i, _) in ctx.catalog.banners.iter().enumerate() {
                    let cx = dots_x0 + i as i32 * DOT_GAP;
                    let color = if i == self.banner_idx {
                        theme::FOCUS
                    } else {
                        theme::TEXT_DIM.with_alpha(180)
                    };
                    r.fill_circle(cx, dots_y, DOT_RADIUS, color);
                }
            }

            if self.banner_focused {
                for i in 0..4 {
                    let a = 200u8.saturating_sub(i as u8 * 45);
                    r.stroke_rect(
                        banner_x - i,
                        banner_top + i,
                        banner_w + 2 * i,
                        banner_h - 2 * i,
                        theme::FOCUS.with_alpha(a),
                    );
                }
            }
        }

        // Card focus frame — only when the rails own focus.
        if !self.banner_focused {
            let fx = l.focus_x as i32;
            let fy = focus_y as i32;
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

            if let Some(rail) = ctx.catalog.rails.get(self.focus_rail) {
                if let Some(card) = rail.cards.get(self.current_col()) {
                    r.draw_text(fx, fy + card_h + 14, 28, theme::TEXT, &card.title);
                }
            }
        }

        // Static header band — painted last so scrolling content never covers it.
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
        assert_eq!(s.focus().0, 19);
        assert!((s.anim_banner.target() - 0.0).abs() < 1e-4);
    }

    #[test]
    fn column_is_remembered_per_rail() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Right, ctx);
            s.handle_key(Key::Right, ctx);
            s.handle_key(Key::Down, ctx);
            s.handle_key(Key::Right, ctx);
            s.handle_key(Key::Up, ctx);
        });
        assert_eq!(s.focus(), (0, 2));
        assert!((s.anim_banner.target() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn up_on_first_rail_focuses_banner() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Up, ctx);
        });
        assert!(s.banner_focused());
        assert_eq!(s.focus().0, 0);
    }

    #[test]
    fn banner_wraps_around() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Up, ctx);
            s.handle_key(Key::Left, ctx);
            assert_eq!(s.banner_index(), ctx.catalog.banners.len() - 1);
            assert!((s.anim_banner_slide.target() - (-1.0)).abs() < 1e-4);
            s.handle_key(Key::Right, ctx);
            assert_eq!(s.banner_index(), 0);
            assert!((s.anim_banner_slide.target() - 0.0).abs() < 1e-4);
        });
        assert_eq!(s.banner_index(), 0);
    }

    #[test]
    fn banner_fast_wrap_keeps_direction() {
        let s = with_ctx(|s, ctx| {
            let n = ctx.catalog.banners.len();
            s.handle_key(Key::Up, ctx);
            // Race through a full loop without waiting for settles.
            for _ in 0..n {
                s.handle_key(Key::Right, ctx);
            }
            assert_eq!(s.banner_index(), 0);
            assert!((s.anim_banner_slide.target() - n as f32).abs() < 1e-4);
            // Next step must keep going forward (n+1), not reverse toward 1.
            s.handle_key(Key::Right, ctx);
            assert_eq!(s.banner_index(), 1);
            assert!((s.anim_banner_slide.target() - (n as f32 + 1.0)).abs() < 1e-4);
            s.handle_key(Key::Right, ctx);
            assert_eq!(s.banner_index(), 2);
            assert!((s.anim_banner_slide.target() - (n as f32 + 2.0)).abs() < 1e-4);
        });
        assert_eq!(s.banner_index(), 2);
    }

    #[test]
    fn banner_left_right_and_down_to_rails() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Up, ctx);
            s.handle_key(Key::Right, ctx);
            s.handle_key(Key::Right, ctx);
            assert_eq!(s.banner_index(), 2);
            assert!((s.anim_banner_slide.target() - 2.0).abs() < 1e-4);
            s.handle_key(Key::Down, ctx);
        });
        assert!(!s.banner_focused());
        assert_eq!(s.banner_index(), 2);
        assert_eq!(s.focus(), (0, 0));
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
