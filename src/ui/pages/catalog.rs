//! Catalog page: banner + rails + metadata overlay (no app chrome).
//!
//! Hosted under [`super::shell::MainShell`]; Up from the banner leaves to the nav.

use crate::geom::Rect;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key, Screen, Transition};
use crate::ui::components::banner::BannerCarousel;
use crate::ui::components::containers::layout_column;
use crate::ui::components::focus::{index_from_zone, zone_from_index, FocusScope, FocusZone};
use crate::ui::components::metadata_overlay::{MetadataItem, MetadataOverlay};
use crate::ui::components::carousel::{HOLD_SCROLL_DELAY, NAV_TAU};
use crate::ui::components::rail_list::RailList;
use crate::ui::components::widget::{FocusResult, Widget};
use super::player::PlayerScreen;

/// Pause on a focus zone before hold-traverse continues to the next zone.
const BOUNDARY_DWELL: f32 = NAV_TAU;

/// Result of routing a key into a catalog page.
pub enum PageKeyResult {
    None,
    Transition(Transition),
    /// Focus should leave the page (shell nav).
    MoveOut(Key),
}

/// One tab's browse content (Home / Movies / Shows share this type).
pub struct CatalogPage {
    banner: BannerCarousel,
    rails: RailList,
    scope: FocusScope,
    overlay: MetadataOverlay,
    /// Content area assigned by the shell (may be horizontally offset while sliding).
    content_bounds: Rect,
    /// Full page rect (for overlay), same horizontal origin as content.
    page_bounds: Rect,
    /// Held Up/Down for crossing banner ↔ rails ↔ (nav via pending_move_out).
    held_vertical: Option<Key>,
    /// Seconds the current vertical key has been held.
    held_vertical_secs: f32,
    /// Countdown before hold-traverse leaves the current zone.
    boundary_cooldown: f32,
    /// Consumed by the shell (Up → nav).
    pending_move_out: Option<Key>,
}

impl CatalogPage {
    pub fn new() -> Self {
        let mut page = Self {
            banner: BannerCarousel::new(420.0),
            rails: RailList::new(),
            scope: FocusScope::new(index_from_zone(FocusZone::Banner)),
            overlay: MetadataOverlay::new(),
            content_bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
            page_bounds: Rect::design(),
            held_vertical: None,
            held_vertical_secs: 0.0,
            boundary_cooldown: 0.0,
            pending_move_out: None,
        };
        // Banner holds focus until the user moves into the rails.
        page.banner.set_focused(true);
        page.rails.set_focused(false);
        page
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

    pub fn overlay_open(&self) -> bool {
        self.overlay.is_open()
    }

    pub fn overlay_active(&self) -> bool {
        self.overlay.is_active()
    }

    pub fn page_bounds(&self) -> Rect {
        self.page_bounds
    }

    pub fn set_content_focused(&mut self, focused: bool) {
        if !focused {
            self.banner.set_focused(false);
            self.rails.set_focused(false);
            self.held_vertical = None;
            self.held_vertical_secs = 0.0;
            self.boundary_cooldown = 0.0;
            self.rails.set_held(None);
        } else {
            self.sync_focus_flags();
        }
    }

    /// Focus the top content zone (banner if any, else rails).
    pub fn focus_top(&mut self, ctx: &Ctx) {
        if ctx.catalog.banners.is_empty() {
            self.scope.set_index(index_from_zone(FocusZone::Rails));
        } else {
            self.scope.set_index(index_from_zone(FocusZone::Banner));
        }
        self.sync_focus_flags();
        self.sync_banner_reveal();
    }

    /// Begin hold-traverse after the shell moves focus into this page (e.g. nav Down).
    pub fn begin_hold_traverse(&mut self, key: Key) {
        if matches!(key, Key::Up | Key::Down) {
            self.held_vertical = Some(key);
            self.held_vertical_secs = 0.0;
            self.boundary_cooldown = HOLD_SCROLL_DELAY;
            self.pending_move_out = None;
        }
    }

    /// Shell polls this each frame (Up leaves content for the nav).
    pub fn take_pending_move_out(&mut self) -> Option<Key> {
        self.pending_move_out.take()
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

    /// Place this page in the sliding strip. `page_x` is the left edge in design space.
    pub fn layout_in_strip(&mut self, ctx: &Ctx, page_x: f32, nav_h: f32) {
        let m = ctx.metrics;
        self.banner.set_full_height(m.banner_h);
        let full_w = crate::DESIGN_WIDTH as f32;
        let full_h = crate::DESIGN_HEIGHT as f32;
        self.page_bounds = Rect::new(page_x, 0.0, full_w, full_h);
        let content = Rect::new(
            page_x + m.safe_margin,
            nav_h,
            full_w - 2.0 * m.safe_margin,
            (full_h - nav_h).max(0.0),
        );
        self.content_bounds = content;
        let banner = &mut self.banner as &mut dyn Widget;
        let rails = &mut self.rails as &mut dyn Widget;
        layout_column(content, 0.0, &mut [banner, rails]);
        // Banner is flex-zero; draw it as an overlay so rail bounds stay stable.
        self.banner.layout_overlay(content);
        self.rails
            .set_banner_pad(m.banner_h * self.banner.reveal_value());
        // Overlay sits under the header so the nav stays visible.
        let overlay_bounds = Rect::new(page_x, nav_h, full_w, (full_h - nav_h).max(0.0));
        self.overlay.layout(overlay_bounds);
    }

    pub fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        self.sync_banner_reveal();
        self.banner.update(dt, ctx);
        self.rails
            .set_banner_pad(ctx.metrics.banner_h * self.banner.reveal_value());
        self.rails.update(dt, ctx);
        self.overlay.update(dt, ctx);
        if !self.overlay.is_active() {
            self.hold_traverse(dt, ctx);
        }
        self.sync_focus_flags();
    }

    /// While Up/Down is held, walk focus across rails ↔ banner (and signal nav).
    fn hold_traverse(&mut self, dt: f32, ctx: &Ctx) {
        let Some(dir) = self.held_vertical else {
            return;
        };
        self.held_vertical_secs += dt;
        if self.boundary_cooldown > 0.0 {
            self.boundary_cooldown = (self.boundary_cooldown - dt).max(0.0);
        }
        // Taps only move one zone/rail; continuous traverse needs a real hold.
        if self.held_vertical_secs < HOLD_SCROLL_DELAY {
            return;
        }

        match (zone_from_index(self.scope.index()), dir) {
            (FocusZone::Rails, Key::Up) => {
                if self.rails.focus_rail() == 0 && self.rails.vertical_near_settle() {
                    if ctx.catalog.banners.is_empty() {
                        self.pending_move_out = Some(Key::Up);
                        self.held_vertical = None;
                        self.held_vertical_secs = 0.0;
                        self.rails.set_held(None);
                    } else {
                        self.scope.set_index(index_from_zone(FocusZone::Banner));
                        self.rails.set_held(None);
                        self.boundary_cooldown = BOUNDARY_DWELL;
                        self.sync_focus_flags();
                        self.sync_banner_reveal();
                    }
                }
            }
            (FocusZone::Banner, Key::Up) => {
                if self.boundary_cooldown <= 0.0 {
                    self.pending_move_out = Some(Key::Up);
                    self.held_vertical = None;
                    self.held_vertical_secs = 0.0;
                }
            }
            (FocusZone::Banner, Key::Down) => {
                if self.boundary_cooldown <= 0.0 {
                    self.scope.set_index(index_from_zone(FocusZone::Rails));
                    // Already in continuous hold — skip the tap delay on rails.
                    self.rails.arm_continuous_hold(Key::Down);
                    self.boundary_cooldown = BOUNDARY_DWELL;
                    self.sync_focus_flags();
                    self.sync_banner_reveal();
                }
            }
            (FocusZone::Rails, Key::Down) => {
                if self.boundary_cooldown <= 0.0 {
                    self.rails.arm_continuous_hold(Key::Down);
                } else {
                    self.rails.set_held(None);
                }
            }
            _ => {}
        }
    }

    pub fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> PageKeyResult {
        if self.overlay.is_active() {
            match self.overlay.handle_key(key, ctx) {
                FocusResult::Activate => {
                    if let Some(item) = self.overlay.item() {
                        let title = item.title.clone();
                        return PageKeyResult::Transition(Transition::Push(Box::new(
                            PlayerScreen::new(title),
                        )));
                    }
                }
                FocusResult::Handled | FocusResult::Ignored | FocusResult::MoveOut(_) => {}
            }
            return PageKeyResult::None;
        }

        if matches!(key, Key::Up | Key::Down) {
            self.held_vertical = Some(key);
            self.held_vertical_secs = 0.0;
            self.pending_move_out = None;
        }

        let zone_before = zone_from_index(self.scope.index());
        self.sync_focus_flags();
        let result = {
            let banner = &mut self.banner as &mut dyn Widget;
            let rails = &mut self.rails as &mut dyn Widget;
            self.scope
                .handle_key(key, ctx, &mut [banner, rails])
        };

        if zone_from_index(self.scope.index()) == FocusZone::Banner
            && ctx.catalog.banners.is_empty()
        {
            self.scope.set_index(index_from_zone(FocusZone::Rails));
        }

        let zone_after = zone_from_index(self.scope.index());
        // Crossing into a zone: dwell before hold-chaining continues.
        if zone_before == FocusZone::Banner && zone_after == FocusZone::Rails {
            self.boundary_cooldown = HOLD_SCROLL_DELAY;
            self.rails.set_held(None);
        } else if zone_before == FocusZone::Rails && zone_after == FocusZone::Banner {
            self.boundary_cooldown = HOLD_SCROLL_DELAY;
            self.rails.set_held(None);
        } else if zone_after == FocusZone::Banner && matches!(self.held_vertical, Some(Key::Up)) {
            self.boundary_cooldown = HOLD_SCROLL_DELAY;
        }

        let out = match result {
            FocusResult::Activate => {
                if let Some(item) = self.selected_metadata(ctx) {
                    self.overlay.open(item);
                }
                PageKeyResult::None
            }
            FocusResult::MoveOut(Key::Up) => PageKeyResult::MoveOut(Key::Up),
            _ => PageKeyResult::None,
        };
        self.sync_focus_flags();
        self.sync_banner_reveal();
        out
    }

    pub fn handle_key_up(&mut self, key: Key, ctx: &mut Ctx) -> PageKeyResult {
        if self.held_vertical == Some(key) {
            self.held_vertical = None;
            self.held_vertical_secs = 0.0;
            self.boundary_cooldown = 0.0;
            self.pending_move_out = None;
        }
        if self.overlay.is_active() {
            let _ = self.overlay.handle_key_up(key, ctx);
            return PageKeyResult::None;
        }
        let banner = &mut self.banner as &mut dyn Widget;
        let rails = &mut self.rails as &mut dyn Widget;
        let _ = self.scope.handle_key_up(key, ctx, &mut [banner, rails]);
        PageKeyResult::None
    }

    pub fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        // Skip pages fully off-screen.
        let full_w = crate::DESIGN_WIDTH as f32;
        if self.page_bounds.right() < 0.0 || self.page_bounds.x > full_w {
            return;
        }
        self.rails.render(r, ctx);
        self.banner.render(r, ctx);
        self.overlay.render(r, ctx);
    }

    fn selected_metadata(&self, ctx: &Ctx) -> Option<MetadataItem> {
        match zone_from_index(self.scope.index()) {
            FocusZone::Banner => {
                let i = self.banner.index();
                ctx.catalog.banners.get(i).map(|b| MetadataItem {
                    title: b.title.clone(),
                    image_url: b.image_url.clone(),
                    rail_index: 0,
                    card_index: i,
                })
            }
            FocusZone::Rails => {
                let (ri, ci) = self.rails.focus();
                ctx.catalog
                    .rails
                    .get(ri)
                    .and_then(|r| r.cards.get(ci))
                    .map(|c| MetadataItem {
                        title: c.title.clone(),
                        image_url: c.image_url.clone(),
                        rail_index: ri,
                        card_index: ci,
                    })
            }
        }
    }
}

impl Default for CatalogPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Thin Screen adapter for unit tests that drive a lone catalog page.
impl Screen for CatalogPage {
    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        CatalogPage::update(self, dt, ctx);
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> Transition {
        match CatalogPage::handle_key(self, key, ctx) {
            PageKeyResult::Transition(t) => t,
            PageKeyResult::None | PageKeyResult::MoveOut(_) => Transition::None,
        }
    }

    fn handle_key_up(&mut self, key: Key, ctx: &mut Ctx) -> Transition {
        let _ = CatalogPage::handle_key_up(self, key, ctx);
        Transition::None
    }

    fn render(&mut self, r: &mut dyn Renderer, ctx: &mut Ctx) {
        let m = ctx.metrics;
        let full = Rect::design();
        r.fill_rect(
            full.x as i32,
            full.y as i32,
            full.w as i32,
            full.h as i32,
            crate::theme::BG,
        );
        self.layout_in_strip(ctx, 0.0, m.header_h);
        CatalogPage::render(self, r, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Metrics;
    use crate::model::Catalog;
    use crate::screen::VideoSink;

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

    fn with_ctx(f: impl FnOnce(&mut CatalogPage, &mut Ctx)) -> CatalogPage {
        let cat = Catalog::sample();
        let metrics = Metrics::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
            video: &mut video,
        };
        let mut screen = CatalogPage::new();
        screen.set_content_focused(true);
        f(&mut screen, &mut ctx);
        screen
    }

    #[test]
    fn starts_on_banner() {
        let s = with_ctx(|_, _| {});
        assert!(s.banner_focused());
    }

    #[test]
    fn right_advances_and_clamps_at_end() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Down, ctx); // banner → rails
            for _ in 0..50 {
                s.handle_key(Key::Right, ctx);
            }
        });
        let last = Catalog::sample().rails[0].cards.len() - 1;
        assert_eq!(s.focus(), (0, last));
    }

    #[test]
    fn left_clamps_at_zero() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Down, ctx);
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
        assert_eq!(s.focus().0, Catalog::sample().rails.len() - 1);
        assert!((s.banner_reveal_target() - 0.0).abs() < 1e-4);
    }

    #[test]
    fn column_is_remembered_per_rail() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Down, ctx);
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
            s.handle_key(Key::Down, ctx);
            assert!(!s.banner_focused());
            s.handle_key(Key::Up, ctx);
        });
        assert!(s.banner_focused());
        assert_eq!(s.focus().0, 0);
    }

    #[test]
    fn banner_wraps_around() {
        let s = with_ctx(|s, ctx| {
            assert!(s.banner_focused());
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
            assert!(s.banner_focused());
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
            assert!(s.banner_focused());
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
    fn enter_opens_metadata_overlay() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Down, ctx);
            s.handle_key(Key::Right, ctx);
            s.handle_key(Key::Enter, ctx);
        });
        assert!(s.overlay_open());
        let item = s.overlay.item().unwrap();
        assert_eq!(item.rail_index, 0);
        assert_eq!(item.card_index, 1);
        assert_eq!(item.title, "Glass Orchard");
    }

    #[test]
    fn overlay_play_pushes_player() {
        let cat = Catalog::sample();
        let metrics = Metrics::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
            video: &mut video,
        };
        let mut screen = CatalogPage::new();
        screen.set_content_focused(true);
        // Banner Enter opens banner metadata; move to rails first.
        assert!(matches!(
            Screen::handle_key(&mut screen, Key::Down, &mut ctx),
            Transition::None
        ));
        assert!(matches!(
            Screen::handle_key(&mut screen, Key::Enter, &mut ctx),
            Transition::None
        ));
        assert!(screen.overlay_open());
        let t = Screen::handle_key(&mut screen, Key::Enter, &mut ctx);
        assert!(matches!(t, Transition::Push(_)));
        assert!(screen.overlay_open());
    }

    #[test]
    fn overlay_back_closes() {
        let s = with_ctx(|s, ctx| {
            s.handle_key(Key::Down, ctx);
            s.handle_key(Key::Enter, ctx);
            assert!(s.overlay_open());
            s.handle_key(Key::Back, ctx);
        });
        assert!(!s.overlay_open());
        assert!(s.overlay_active());
    }

    #[test]
    fn up_from_banner_moves_out() {
        let _ = with_ctx(|s, ctx| {
            assert!(s.banner_focused());
            let r = CatalogPage::handle_key(s, Key::Up, ctx);
            assert!(matches!(r, PageKeyResult::MoveOut(Key::Up)));
        });
    }

    #[test]
    fn hold_up_from_rails_crosses_banner_then_pending_nav() {
        let _ = with_ctx(|s, ctx| {
            s.handle_key(Key::Down, ctx); // banner → rails
            s.handle_key(Key::Down, ctx); // rail 1
            s.handle_key(Key::Up, ctx); // toward rail 0
            for _ in 0..45 {
                s.update(1.0 / 60.0, ctx);
            }
            assert!(
                s.banner_focused(),
                "hold Up should land on banner before nav"
            );
            for _ in 0..30 {
                s.update(1.0 / 60.0, ctx);
            }
            assert!(matches!(s.take_pending_move_out(), Some(Key::Up)));
        });
    }

    #[test]
    fn down_from_banner_lands_on_first_rail() {
        let s = with_ctx(|s, ctx| {
            assert!(s.banner_focused());
            s.handle_key(Key::Down, ctx);
            // Simulate a few frames before keyup — must not chain past rail 0.
            for _ in 0..5 {
                s.update(1.0 / 60.0, ctx);
            }
            s.handle_key_up(Key::Down, ctx);
            for _ in 0..30 {
                s.update(1.0 / 60.0, ctx);
            }
        });
        assert!(!s.banner_focused());
        assert_eq!(s.focus().0, 0);
    }

    #[test]
    fn hold_down_from_banner_continues_into_rails() {
        let s = with_ctx(|s, ctx| {
            assert!(s.banner_focused());
            s.handle_key(Key::Down, ctx); // banner → rails, held Down
            for _ in 0..40 {
                s.update(1.0 / 60.0, ctx);
            }
        });
        assert!(!s.banner_focused());
        assert!(s.focus().0 >= 1, "held Down should chain into deeper rails");
    }
}
