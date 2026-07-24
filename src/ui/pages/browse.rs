//! Browse screen: Header + BannerCarousel + RailList as a column tree.
//!
//! Banner collapse shrinks its flex height so rails reflow upward.

use crate::geom::{Insets, Rect};
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key, Screen, Transition};
use crate::theme;
use crate::ui::components::banner::BannerCarousel;
use crate::ui::components::containers::layout_column;
use crate::ui::components::focus::{index_from_zone, zone_from_index, FocusScope, FocusZone};
use crate::ui::components::header::Header;
use super::player::PlayerScreen;
use crate::ui::components::rail_list::RailList;
use crate::ui::components::widget::{FocusResult, Widget};

pub struct BrowseScreen {
    header: Header,
    banner: BannerCarousel,
    rails: RailList,
    scope: FocusScope,
}

impl BrowseScreen {
    pub fn new() -> Self {
        Self {
            header: Header::new("WASM TV", 140.0),
            banner: BannerCarousel::new(420.0),
            rails: RailList::new(),
            scope: FocusScope::new(index_from_zone(FocusZone::Rails)),
        }
    }

    pub fn focus(&self) -> (usize, usize) {
        self.rails.focus()
    }

    pub fn banner_index(&self) -> usize {
        self.banner.index()
    }

    pub fn banner_focused(&self) -> bool {
        zone_from_index(self.scope.index()) == FocusZone::Banner
    }

    pub fn banner_reveal_target(&self) -> f32 {
        self.banner.reveal_target()
    }

    fn sync_focus_flags(&mut self) {
        match zone_from_index(self.scope.index()) {
            FocusZone::Banner => {
                self.banner.set_focused(true);
                self.rails.set_focused(false);
            }
            FocusZone::Rails => {
                self.banner.set_focused(false);
                self.rails.set_focused(true);
            }
        }
    }

    fn sync_banner_reveal(&mut self) {
        self.banner.set_revealed(self.rails.focus_rail() == 0);
    }

    fn layout_tree(&mut self, ctx: &Ctx) {
        let m = ctx.metrics;
        self.header.set_height(m.header_h);
        self.banner.set_full_height(m.banner_h);

        let full = Rect::design();
        // Full-bleed header band, then horizontally inset content column.
        self.header
            .layout(Rect::new(0.0, 0.0, full.w, m.header_h));

        let content = full.inset(Insets {
            top: m.header_h,
            right: m.safe_margin,
            bottom: 0.0,
            left: m.safe_margin,
        });
        let banner = &mut self.banner as &mut dyn Widget;
        let rails = &mut self.rails as &mut dyn Widget;
        layout_column(content, 0.0, &mut [banner, rails]);
    }
}

impl Default for BrowseScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for BrowseScreen {
    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        self.sync_banner_reveal();
        self.banner.update(dt, ctx);
        self.rails.update(dt, ctx);
        self.sync_focus_flags();
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> Transition {
        self.sync_focus_flags();
        let result = {
            let banner = &mut self.banner as &mut dyn Widget;
            let rails = &mut self.rails as &mut dyn Widget;
            self.scope
                .handle_key(key, ctx, &mut [banner, rails])
        };

        // Don't park focus on an empty banner strip.
        if zone_from_index(self.scope.index()) == FocusZone::Banner
            && ctx.catalog.banners.is_empty()
        {
            self.scope.set_index(index_from_zone(FocusZone::Rails));
        }

        match result {
            FocusResult::Activate => {
                let title = match zone_from_index(self.scope.index()) {
                    FocusZone::Banner => ctx
                        .catalog
                        .banners
                        .get(self.banner.index())
                        .map(|b| b.title.clone()),
                    FocusZone::Rails => {
                        let (ri, ci) = self.rails.focus();
                        ctx.catalog
                            .rails
                            .get(ri)
                            .and_then(|r| r.cards.get(ci))
                            .map(|c| c.title.clone())
                    }
                };
                if let Some(title) = title {
                    return Transition::Push(Box::new(PlayerScreen::new(title)));
                }
            }
            FocusResult::MoveOut(Key::Up) => {
                // Already at top of scope (banner); stay.
            }
            _ => {}
        }
        self.sync_focus_flags();
        self.sync_banner_reveal();
        Transition::None
    }

    fn render(&mut self, r: &mut dyn Renderer, ctx: &mut Ctx) {
        let full = Rect::design();
        r.fill_rect(
            full.x as i32,
            full.y as i32,
            full.w as i32,
            full.h as i32,
            theme::BG,
        );

        self.layout_tree(ctx);
        self.rails.render(r, ctx);
        self.banner.render(r, ctx);
        // Header last so scrolling content never covers the title band.
        self.header.render(r, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Metrics;
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
        let metrics = Metrics::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
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
        let metrics = Metrics::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
            video: &mut video,
        };
        let mut screen = BrowseScreen::new();
        let t = screen.handle_key(Key::Enter, &mut ctx);
        assert!(matches!(t, Transition::Push(_)));
    }
}
