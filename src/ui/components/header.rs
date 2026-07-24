//! Static page header band.

use crate::geom::{Rect, Size};
use crate::renderer::Renderer;
use crate::screen::Ctx;
use crate::theme;
use super::widget::{Flex, Widget};

pub struct Header {
    title: String,
    height: f32,
    bounds: Rect,
}

impl Header {
    pub fn new(title: impl Into<String>, height: f32) -> Self {
        Self {
            title: title.into(),
            height,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    pub fn set_height(&mut self, height: f32) {
        self.height = height;
    }
}

impl Widget for Header {
    fn flex(&self) -> Flex {
        Flex::Fixed(self.height)
    }

    fn measure(&self, available: Size) -> Size {
        Size::new(available.w, self.height)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        let (x, y, w, h) = self.bounds.as_i32();
        r.fill_rect(x, y, w, h, theme::BG);
        let margin = ctx.metrics.safe_margin;
        r.draw_text(
            (self.bounds.x + margin) as i32,
            (self.bounds.y + margin * 0.55) as i32,
            52,
            theme::HEADER,
            &self.title,
        );
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}
