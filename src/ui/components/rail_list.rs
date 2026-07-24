//! Leanback rail stack: fixed focus anchor, vertically sliding rows of cards.

use crate::anim::Tween;
use crate::geom::Rect;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key};
use crate::theme;
use super::card;
use super::carousel::{self, HCarousel, NAV_TAU};
use super::widget::{Flex, FocusResult, Widget};

pub struct RailList {
    focus_rail: usize,
    focus_col: Vec<usize>,
    rail_carousel: HCarousel,
    anim_rail: Tween,
    focused: bool,
    bounds: Rect,
    initialized: bool,
}

impl RailList {
    pub fn new() -> Self {
        Self {
            focus_rail: 0,
            focus_col: Vec::new(),
            rail_carousel: HCarousel::new(false),
            anim_rail: Tween::new(0.0, NAV_TAU),
            focused: true,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
            initialized: false,
        }
    }

    pub fn focus(&self) -> (usize, usize) {
        (self.focus_rail, self.rail_carousel.index())
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn focus_rail(&self) -> usize {
        self.focus_rail
    }

    fn ensure_init(&mut self, ctx: &Ctx) {
        if !self.initialized {
            self.focus_col = vec![0; ctx.catalog.rails.len()];
            self.initialized = true;
        }
    }

    fn focus_card_y(&self, ctx: &Ctx) -> f32 {
        self.bounds.y + ctx.metrics.rail_title_h + 8.0
    }

    fn move_to_rail(&mut self, rail: usize) {
        if let Some(slot) = self.focus_col.get_mut(self.focus_rail) {
            *slot = self.rail_carousel.index();
        }
        self.focus_rail = rail;
        self.anim_rail.set_target(rail as f32);
        let col = self.focus_col.get(rail).copied().unwrap_or(0);
        self.rail_carousel.snap(col);
    }
}

impl Default for RailList {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for RailList {
    fn flex(&self) -> Flex {
        Flex::Grow(1.0)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        self.ensure_init(ctx);
        self.anim_rail.step(dt);
        self.rail_carousel.update(dt);
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        let m = ctx.metrics;
        let anim_rail = self.anim_rail.value();
        let focus_y = self.focus_card_y(ctx);
        let card_w = m.card_w;
        let card_h = m.card_h;

        for (ri, rail) in ctx.catalog.rails.iter().enumerate() {
            let row_top = focus_y + (ri as f32 - anim_rail) * m.rail_step;
            let title_y = row_top - m.rail_title_h;
            if row_top + card_h < self.bounds.y || title_y > self.bounds.bottom() {
                continue;
            }

            let col_off = if ri == self.focus_rail {
                self.rail_carousel.anim_value()
            } else {
                self.focus_col.get(ri).copied().unwrap_or(0) as f32
            };

            if title_y + 30.0 >= self.bounds.y {
                r.draw_text(
                    self.bounds.x as i32,
                    title_y as i32,
                    30,
                    theme::RAIL_TITLE,
                    &rail.title,
                );
            }

            let row_bounds = Rect::new(
                self.bounds.x - m.safe_margin,
                row_top,
                self.bounds.w + 2.0 * m.safe_margin,
                card_h,
            );
            // Focus slot stays at the content left edge; cull window includes
            // the safe margins so scrolled-off neighbors stay visible there.
            carousel::draw_card_row(r, m, row_bounds, &rail.cards, col_off, self.bounds.x);
        }

        if self.focused {
            let focus_rect = Rect::new(self.bounds.x, focus_y, card_w, card_h);
            card::draw_focus_ring(r, focus_rect);
            if let Some(rail) = ctx.catalog.rails.get(self.focus_rail) {
                if let Some(item) = rail.cards.get(self.rail_carousel.index()) {
                    r.draw_text(
                        self.bounds.x as i32,
                        (focus_y + card_h + 14.0) as i32,
                        28,
                        theme::TEXT,
                        &item.title,
                    );
                }
            }
        }
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> FocusResult {
        if !self.focused {
            return FocusResult::Ignored;
        }
        self.ensure_init(ctx);
        let rails = &ctx.catalog.rails;
        if rails.is_empty() {
            return FocusResult::Handled;
        }
        match key {
            Key::Left => {
                let count = rails[self.focus_rail].cards.len();
                self.rail_carousel.step(-1, count);
                FocusResult::Handled
            }
            Key::Right => {
                let count = rails[self.focus_rail].cards.len();
                self.rail_carousel.step(1, count);
                FocusResult::Handled
            }
            Key::Up => {
                if self.focus_rail == 0 {
                    FocusResult::MoveOut(Key::Up)
                } else {
                    self.move_to_rail(self.focus_rail - 1);
                    FocusResult::Handled
                }
            }
            Key::Down => {
                if self.focus_rail + 1 < rails.len() {
                    self.move_to_rail(self.focus_rail + 1);
                    FocusResult::Handled
                } else {
                    FocusResult::Handled
                }
            }
            Key::Enter => FocusResult::Activate,
            Key::Back => FocusResult::Ignored,
        }
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}
