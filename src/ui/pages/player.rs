//! Player screen: video underlay + bottom control chrome as a widget tree.
//!
//! ```text
//! Column
//!   Spacer (Grow)     — transparent; <video> shows through
//!   PlayerChrome      — title, scrubber, hints (horizontally inset)
//! ```

use crate::buffer::Color;
use crate::geom::{Insets, Rect};
use crate::model::SAMPLE_VIDEO_URL;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key, Screen, Transition};
use crate::theme;
use crate::ui::components::containers::layout_column;
use crate::ui::components::widget::{Flex, Widget};

const SEEK_STEP: f64 = 5.0;
const BAR_H: f32 = 150.0;

pub struct PlayerScreen {
    title: String,
    started: bool,
}

impl PlayerScreen {
    pub fn new(title: String) -> Self {
        Self {
            title,
            started: false,
        }
    }
}

impl Screen for PlayerScreen {
    fn update(&mut self, _dt: f32, ctx: &mut Ctx) {
        if !self.started {
            ctx.video.set_visible(true);
            ctx.video.load_and_play(SAMPLE_VIDEO_URL);
            self.started = true;
        }
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> Transition {
        match key {
            Key::Enter => {
                if ctx.video.is_paused() {
                    ctx.video.play();
                } else {
                    ctx.video.pause();
                }
            }
            Key::Left => {
                let t = (ctx.video.current_time() - SEEK_STEP).max(0.0);
                ctx.video.seek(t);
            }
            Key::Right => {
                let dur = ctx.video.duration();
                let mut t = ctx.video.current_time() + SEEK_STEP;
                if dur > 0.0 {
                    t = t.min(dur);
                }
                ctx.video.seek(t);
            }
            Key::Back => {
                ctx.video.pause();
                ctx.video.set_visible(false);
                return Transition::Pop;
            }
            Key::Up | Key::Down => {}
        }
        Transition::None
    }

    fn render(&mut self, r: &mut dyn Renderer, ctx: &mut Ctx) {
        let full = Rect::design();
        let margin = ctx.metrics.safe_margin;

        let mut spacer = PlayerSpacer {
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        };
        let mut chrome = PlayerChrome {
            title: self.title.clone(),
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        };

        {
            let sp = &mut spacer as &mut dyn Widget;
            let ch = &mut chrome as &mut dyn Widget;
            layout_column(full, 0.0, &mut [sp, ch]);
        }

        let bar = chrome.bounds().inset(Insets::vh(0.0, margin));
        chrome.layout(bar);
        chrome.render(r, ctx);
    }
}

/// Transparent grow region so the video underlay remains visible.
struct PlayerSpacer {
    bounds: Rect,
}

impl Widget for PlayerSpacer {
    fn flex(&self) -> Flex {
        Flex::Grow(1.0)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn render(&self, _r: &mut dyn Renderer, _ctx: &Ctx) {}

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

struct PlayerChrome {
    title: String,
    bounds: Rect,
}

impl Widget for PlayerChrome {
    fn flex(&self) -> Flex {
        Flex::Fixed(BAR_H)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        let full_w = crate::DESIGN_WIDTH as i32;
        let (bx, by, bw, bh) = self.bounds.as_i32();

        r.fill_rect(0, by - 40, full_w, bh + 80, theme::SCRIM);
        r.draw_text(bx, by, 44, theme::TEXT, &self.title);

        let cur = ctx.video.current_time();
        let dur = ctx.video.duration();
        let frac = if dur > 0.0 {
            (cur / dur).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };

        let track_y = by + 74;
        let track_h = 8;
        r.fill_rect(bx, track_y, bw, track_h, theme::TRACK);
        r.fill_rect(
            bx,
            track_y,
            (bw as f32 * frac) as i32,
            track_h,
            theme::FOCUS,
        );
        let knob_x = bx + (bw as f32 * frac) as i32;
        r.fill_circle(knob_x, track_y + track_h / 2, 10, theme::FOCUS);

        let state = if ctx.video.is_paused() {
            "❚❚ Paused"
        } else {
            "▶ Playing"
        };
        let line = format!("{state}    {} / {}", fmt_time(cur), fmt_time(dur));
        r.draw_text(bx, track_y + 22, 26, theme::TEXT_DIM, &line);
        r.draw_text(
            bx,
            track_y + 60,
            22,
            Color::rgb(140, 140, 150),
            "Enter: play/pause   ◀ ▶: seek 5s   Back: browse",
        );
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

fn fmt_time(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "0:00".to_string();
    }
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_times() {
        assert_eq!(fmt_time(0.0), "0:00");
        assert_eq!(fmt_time(9.0), "0:09");
        assert_eq!(fmt_time(75.0), "1:15");
        assert_eq!(fmt_time(3661.0), "1:01:01");
        assert_eq!(fmt_time(f64::NAN), "0:00");
    }
}
