//! Player screen: drives the DOM `<video>` (which shows through the transparent
//! GL canvas) and draws a control bar over it — progress, play/pause and title.

use crate::buffer::Color;
use crate::model::SAMPLE_VIDEO_URL;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key, Screen, Transition};
use crate::theme;

const SEEK_STEP: f64 = 5.0;

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
        // No full-screen fill: the video plays behind the transparent canvas.
        let l = ctx.layout;
        let dw = l.design_w as i32;
        let margin = l.safe_margin as i32;

        let bar_h = 150;
        let bar_top = l.design_h as i32 - bar_h - margin / 2;

        // Scrim so controls stay legible over bright video.
        r.fill_rect(0, bar_top - 40, dw, bar_h + 80, theme::SCRIM);

        // Title.
        r.draw_text(margin, bar_top, 44, theme::TEXT, &self.title);

        // Scrub track + fill.
        let cur = ctx.video.current_time();
        let dur = ctx.video.duration();
        let frac = if dur > 0.0 {
            (cur / dur).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let track_x = margin;
        let track_w = dw - 2 * margin;
        let track_y = bar_top + 74;
        let track_h = 8;
        r.fill_rect(track_x, track_y, track_w, track_h, theme::TRACK);
        r.fill_rect(track_x, track_y, (track_w as f32 * frac) as i32, track_h, theme::FOCUS);
        // Playhead knob.
        let knob_x = track_x + (track_w as f32 * frac) as i32;
        r.fill_circle(knob_x, track_y + track_h / 2, 10, theme::FOCUS);

        // State + timecodes.
        let state = if ctx.video.is_paused() { "❚❚ Paused" } else { "▶ Playing" };
        let line = format!("{state}    {} / {}", fmt_time(cur), fmt_time(dur));
        r.draw_text(track_x, track_y + 22, 26, theme::TEXT_DIM, &line);

        // Hint.
        r.draw_text(
            track_x,
            track_y + 60,
            22,
            Color::rgb(140, 140, 150),
            "Enter: play/pause   ◀ ▶: seek 5s   Back: browse",
        );
    }
}

/// Seconds → `M:SS` (or `H:MM:SS`).
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
