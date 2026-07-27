//! Root shell: top nav + lazy catalog pages with horizontal slide transitions.

use crate::anim::Tween;
use crate::geom::Rect;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key, Screen, Transition};
use crate::theme;
use crate::ui::components::nav_bar::{NavBar, Tab, TAB_COUNT};
use crate::ui::components::widget::{FocusResult, Widget};
use crate::ui::pages::catalog::{CatalogPage, PageKeyResult};
use crate::{DESIGN_HEIGHT, DESIGN_WIDTH};

const SLIDE_TAU: f32 = 0.18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellFocus {
    Nav,
    Content,
}

/// App root: Home / Movies / Shows behind a shared nav bar.
pub struct MainShell {
    nav: NavBar,
    pages: [Option<CatalogPage>; TAB_COUNT],
    /// Fractional tab index for the sliding content strip.
    slide: Tween,
    focus: ShellFocus,
}

impl MainShell {
    pub fn new() -> Self {
        let mut pages: [Option<CatalogPage>; TAB_COUNT] = [None, None, None];
        pages[Tab::Home.index()] = Some(CatalogPage::new());
        let mut shell = Self {
            nav: NavBar::new(140.0),
            pages,
            slide: Tween::new(Tab::Home.index() as f32, SLIDE_TAU),
            focus: ShellFocus::Content,
        };
        shell.sync_focus_flags();
        shell
    }

    pub fn selected_tab(&self) -> Tab {
        self.nav.selected()
    }

    pub fn page_loaded(&self, tab: Tab) -> bool {
        self.pages[tab.index()].is_some()
    }

    pub fn slide_target(&self) -> f32 {
        self.slide.target()
    }

    pub fn nav_focused(&self) -> bool {
        self.focus == ShellFocus::Nav
    }

    fn ensure_page(&mut self, tab: Tab) {
        let i = tab.index();
        if self.pages[i].is_none() {
            self.pages[i] = Some(CatalogPage::new());
        }
    }

    fn active_page_mut(&mut self) -> Option<&mut CatalogPage> {
        let i = self.nav.selected().index();
        self.pages[i].as_mut()
    }

    fn sync_focus_flags(&mut self) {
        let nav_on = self.focus == ShellFocus::Nav;
        self.nav.set_focused(nav_on);
        let selected = self.nav.selected().index();
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some(p) = page {
                p.set_content_focused(!nav_on && i == selected);
            }
        }
    }

    fn layout_all(&mut self, ctx: &Ctx) {
        let m = ctx.metrics;
        self.nav.set_height(m.header_h);
        let full = Rect::design();
        self.nav
            .layout(Rect::new(0.0, 0.0, full.w, m.header_h));

        let slide = self.slide.value();
        let w = DESIGN_WIDTH as f32;
        for i in 0..TAB_COUNT {
            if let Some(page) = self.pages[i].as_mut() {
                let page_x = (i as f32 - slide) * w;
                page.layout_in_strip(ctx, page_x, m.header_h);
            }
        }
    }
}

impl Default for MainShell {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for MainShell {
    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        self.nav.update(dt, ctx);
        self.slide.step(dt);
        let slide = self.slide.value();
        for (i, page) in self.pages.iter_mut().enumerate() {
            // Skip work on pages that are fully off the sliding strip.
            if (i as f32 - slide).abs() < 1.2 {
                if let Some(p) = page {
                    p.update(dt, ctx);
                }
            }
        }

        // Hold-Up from content (banner dwell) → nav.
        if self.focus == ShellFocus::Content {
            let selected = self.nav.selected().index();
            if let Some(page) = self.pages[selected].as_mut() {
                if matches!(page.take_pending_move_out(), Some(Key::Up)) {
                    self.focus = ShellFocus::Nav;
                }
            }
        }

        self.sync_focus_flags();
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> Transition {
        // Overlay / player keys always go to the active page first.
        if let Some(page) = self.active_page_mut() {
            if page.overlay_active() {
                return match page.handle_key(key, ctx) {
                    PageKeyResult::Transition(t) => t,
                    PageKeyResult::None | PageKeyResult::MoveOut(_) => Transition::None,
                };
            }
        }

        match self.focus {
            ShellFocus::Nav => {
                let before = self.nav.selected();
                let result = self.nav.handle_key(key, ctx);
                let after = self.nav.selected();
                if after != before {
                    self.ensure_page(after);
                    self.slide.set_target(after.index() as f32);
                }
                if matches!(result, FocusResult::MoveOut(Key::Down)) {
                    self.focus = ShellFocus::Content;
                    self.ensure_page(after);
                    if let Some(page) = self.active_page_mut() {
                        page.focus_top(ctx);
                        // Hold-Down continues banner → rails after a short dwell.
                        page.begin_hold_traverse(Key::Down);
                    }
                }
                self.sync_focus_flags();
                if key == Key::Back {
                    Transition::Pop
                } else {
                    Transition::None
                }
            }
            ShellFocus::Content => {
                self.ensure_page(self.nav.selected());
                let result = match self.active_page_mut() {
                    Some(page) => page.handle_key(key, ctx),
                    None => PageKeyResult::None,
                };
                match result {
                    PageKeyResult::Transition(t) => t,
                    PageKeyResult::MoveOut(Key::Up) => {
                        self.focus = ShellFocus::Nav;
                        self.sync_focus_flags();
                        Transition::None
                    }
                    PageKeyResult::MoveOut(_) | PageKeyResult::None => {
                        self.sync_focus_flags();
                        // Back on browse (no overlay) pops the root → app exit.
                        // Overlay Back is handled above and returns None without Pop.
                        if key == Key::Back {
                            Transition::Pop
                        } else {
                            Transition::None
                        }
                    }
                }
            }
        }
    }

    fn handle_key_up(&mut self, key: Key, ctx: &mut Ctx) -> Transition {
        if let Some(page) = self.active_page_mut() {
            if page.overlay_active() {
                let _ = page.handle_key_up(key, ctx);
                return Transition::None;
            }
        }
        match self.focus {
            ShellFocus::Nav => {
                let _ = self.nav.handle_key_up(key, ctx);
            }
            ShellFocus::Content => {
                self.ensure_page(self.nav.selected());
                if let Some(page) = self.active_page_mut() {
                    let _ = page.handle_key_up(key, ctx);
                }
            }
        }
        Transition::None
    }

    fn render(&mut self, r: &mut dyn Renderer, ctx: &mut Ctx) {
        r.fill_rect(
            0,
            0,
            DESIGN_WIDTH as i32,
            DESIGN_HEIGHT as i32,
            theme::BG,
        );
        self.layout_all(ctx);

        let view = Rect::design();
        for page in self.pages.iter().flatten() {
            // Clip each strip so banner/rails never bleed into the neighbour page
            // (or off the screen) while sliding.
            let clip = page.page_bounds().intersect(view);
            if clip.is_empty() {
                continue;
            }
            r.set_clip(Some(clip));
            page.render(r, ctx);
        }
        r.set_clip(None);
        self.nav.render(r, ctx);
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

    fn with_shell(f: impl FnOnce(&mut MainShell, &mut Ctx)) -> MainShell {
        let cat = Catalog::sample();
        let metrics = Metrics::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
            video: &mut video,
        };
        let mut shell = MainShell::new();
        f(&mut shell, &mut ctx);
        shell
    }

    #[test]
    fn home_loaded_at_start_others_lazy() {
        let s = MainShell::new();
        assert!(s.page_loaded(Tab::Home));
        assert!(!s.page_loaded(Tab::Movies));
        assert!(!s.page_loaded(Tab::Shows));
    }

    #[test]
    fn right_on_nav_loads_movies_and_slides() {
        let s = with_shell(|s, ctx| {
            // Start on banner → Up to nav
            s.handle_key(Key::Up, ctx);
            assert!(s.nav_focused());
            s.handle_key(Key::Right, ctx);
        });
        assert_eq!(s.selected_tab(), Tab::Movies);
        assert!(s.page_loaded(Tab::Movies));
        assert!((s.slide_target() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn slide_direction_matches_tab_delta() {
        let s = with_shell(|s, ctx| {
            s.handle_key(Key::Up, ctx);
            s.handle_key(Key::Right, ctx);
            s.handle_key(Key::Right, ctx);
        });
        assert_eq!(s.selected_tab(), Tab::Shows);
        assert!((s.slide_target() - 2.0).abs() < 1e-4);
        assert!(s.page_loaded(Tab::Shows));
    }

    #[test]
    fn down_from_nav_enters_content() {
        let s = with_shell(|s, ctx| {
            s.handle_key(Key::Up, ctx);
            assert!(s.nav_focused());
            s.handle_key(Key::Down, ctx);
        });
        assert!(!s.nav_focused());
    }

    #[test]
    fn hold_up_from_content_reaches_nav() {
        let s = with_shell(|s, ctx| {
            s.handle_key(Key::Down, ctx); // banner → rails
            s.handle_key(Key::Down, ctx); // rail 1
            s.handle_key(Key::Up, ctx);
            for _ in 0..150 {
                s.update(1.0 / 60.0, ctx);
            }
        });
        assert!(s.nav_focused(), "hold Up should walk rails → banner → nav");
    }

    #[test]
    fn hold_down_from_nav_reaches_rails() {
        let s = with_shell(|s, ctx| {
            s.handle_key(Key::Up, ctx);
            assert!(s.nav_focused());
            s.handle_key(Key::Down, ctx); // nav → banner + hold traverse
            for _ in 0..90 {
                s.update(1.0 / 60.0, ctx);
            }
        });
        assert!(!s.nav_focused());
        let page = s.pages[0].as_ref().unwrap();
        assert!(
            !page.banner_focused(),
            "hold Down from nav should continue into rails"
        );
    }

    #[test]
    fn back_on_browse_pops_root() {
        let cat = Catalog::sample();
        let metrics = Metrics::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
            video: &mut video,
        };
        let mut shell = MainShell::new();
        assert!(matches!(
            Screen::handle_key(&mut shell, Key::Back, &mut ctx),
            Transition::Pop
        ));
    }

    #[test]
    fn back_on_metadata_does_not_pop() {
        let cat = Catalog::sample();
        let metrics = Metrics::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
            video: &mut video,
        };
        let mut shell = MainShell::new();
        Screen::handle_key(&mut shell, Key::Down, &mut ctx);
        Screen::handle_key(&mut shell, Key::Enter, &mut ctx);
        assert!(shell.active_page_mut().unwrap().overlay_open());
        assert!(matches!(
            Screen::handle_key(&mut shell, Key::Back, &mut ctx),
            Transition::None
        ));
        assert!(!shell.active_page_mut().unwrap().overlay_open());
    }
}
