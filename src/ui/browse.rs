//! Browse screen: hero [`BannerCarousel`] + vertically stacked card rails.
//!
//! Rails use [`HCarousel`] / [`draw_card_row`] for horizontal motion behind a
//! fixed focus anchor. The banner collapses when leaving the first rail and
//! expands again on return.

use crate::anim::Tween;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key, Screen, Transition};
use crate::theme;
use crate::ui::banner::BannerCarousel;
use crate::ui::card;
use crate::ui::carousel::{self, HCarousel, NAV_TAU};
use crate::ui::player::PlayerScreen;

pub struct BrowseScreen {
    focus_rail: usize,
    /// Per-rail remembered column (snap into the active carousel on rail change).
    focus_col: Vec<usize>,
    /// Horizontal carousel for the focused rail (clamped).
    rail_carousel: HCarousel,
    banner: BannerCarousel,
    anim_rail: Tween,
    initialized: bool,
}

impl BrowseScreen {
    pub fn new() -> Self {
        Self {
            focus_rail: 0,
            focus_col: Vec::new(),
            rail_carousel: HCarousel::new(false),
            banner: BannerCarousel::new(),
            anim_rail: Tween::new(0.0, NAV_TAU),
            initialized: false,
        }
    }

    /// Currently focused (rail, column) — exposed for tests.
    pub fn focus(&self) -> (usize, usize) {
        (self.focus_rail, self.rail_carousel.index())
    }

    pub fn banner_index(&self) -> usize {
        self.banner.index()
    }

    pub fn banner_focused(&self) -> bool {
        self.banner.focused()
    }

    pub fn banner_reveal_target(&self) -> f32 {
        self.banner.reveal_target()
    }

    fn ensure_init(&mut self, ctx: &Ctx) {
        if !self.initialized {
            self.focus_col = vec![0; ctx.catalog.rails.len()];
            self.initialized = true;
        }
    }

    fn sync_banner_reveal(&mut self) {
        self.banner.set_revealed(self.focus_rail == 0);
    }

    fn move_to_rail(&mut self, rail: usize) {
        // Remember the column we're leaving.
        if let Some(slot) = self.focus_col.get_mut(self.focus_rail) {
            *slot = self.rail_carousel.index();
        }
        self.focus_rail = rail;
        self.anim_rail.set_target(rail as f32);
        let col = self.focus_col.get(rail).copied().unwrap_or(0);
        self.rail_carousel.snap(col);
        self.sync_banner_reveal();
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
        self.rail_carousel.update(dt);
        self.banner
            .update(dt, ctx.catalog.banners.len());
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> Transition {
        self.ensure_init(ctx);
        let rails = &ctx.catalog.rails;
        let banners = &ctx.catalog.banners;
        if rails.is_empty() {
            return Transition::None;
        }

        if self.banner.focused() {
            match key {
                Key::Left => self.banner.step(-1, banners.len()),
                Key::Right => self.banner.step(1, banners.len()),
                Key::Down => self.banner.set_focused(false),
                Key::Up => {}
                Key::Enter => {
                    if let Some(slide) = banners.get(self.banner.index()) {
                        return Transition::Push(Box::new(PlayerScreen::new(slide.title.clone())));
                    }
                }
                Key::Back => {}
            }
            return Transition::None;
        }

        match key {
            Key::Left => {
                let count = rails[self.focus_rail].cards.len();
                self.rail_carousel.step(-1, count);
            }
            Key::Right => {
                let count = rails[self.focus_rail].cards.len();
                self.rail_carousel.step(1, count);
            }
            Key::Up => {
                if self.focus_rail == 0 {
                    if !banners.is_empty() {
                        self.banner.set_focused(true);
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
                let col = self.rail_carousel.index();
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
        let banner_t = self.banner.reveal_value();

        r.fill_rect(0, 0, dw, l.design_h as i32, theme::BG);

        let anim_rail = self.anim_rail.value();
        let card_w = l.card_w as i32;
        let card_h = l.card_h as i32;
        let focus_y = l.focus_y(banner_t);

        for (ri, rail) in ctx.catalog.rails.iter().enumerate() {
            let row_top = l.rail_y(ri, anim_rail, banner_t);
            let title_y = row_top - l.rail_title_h;
            if row_top + l.card_h < l.header_h || title_y > l.design_h {
                continue;
            }

            let col_off = if ri == self.focus_rail {
                self.rail_carousel.anim_value()
            } else {
                self.focus_col.get(ri).copied().unwrap_or(0) as f32
            };

            if title_y + 30.0 >= l.header_h {
                r.draw_text(
                    l.safe_margin as i32,
                    title_y as i32,
                    30,
                    theme::RAIL_TITLE,
                    &rail.title,
                );
            }

            carousel::draw_card_row(r, l, &rail.cards, row_top, col_off, l.header_h);
        }

        self.banner.render(r, l, &ctx.catalog.banners);

        if !self.banner.focused() {
            let fx = l.focus_x as i32;
            let fy = focus_y as i32;
            card::draw_focus_ring(r, fx, fy, card_w, card_h);

            if let Some(rail) = ctx.catalog.rails.get(self.focus_rail) {
                if let Some(item) = rail.cards.get(self.rail_carousel.index()) {
                    r.draw_text(fx, fy + card_h + 14, 28, theme::TEXT, &item.title);
                }
            }
        }

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
        assert!((s.banner_reveal_target() - 0.0).abs() < 1e-4);
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
        assert!((s.banner_reveal_target() - 1.0).abs() < 1e-4);
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
            s.handle_key(Key::Right, ctx);
            assert_eq!(s.banner_index(), 0);
        });
        assert_eq!(s.banner_index(), 0);
    }

    #[test]
    fn banner_fast_wrap_keeps_direction() {
        let s = with_ctx(|s, ctx| {
            let n = ctx.catalog.banners.len();
            s.handle_key(Key::Up, ctx);
            for _ in 0..n {
                s.handle_key(Key::Right, ctx);
            }
            assert_eq!(s.banner_index(), 0);
            s.handle_key(Key::Right, ctx);
            assert_eq!(s.banner_index(), 1);
            s.handle_key(Key::Right, ctx);
            assert_eq!(s.banner_index(), 2);
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
