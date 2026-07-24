//! Player screen: video underlay + bottom control chrome as a widget tree.
//!
//! Controls sit above the TV safe margin and autohide after idle.

use crate::anim::Tween;
use crate::buffer::Color;
use crate::geom::{Insets, Rect};
use crate::model::SAMPLE_VIDEO_URL;
use crate::renderer::Renderer;
use crate::screen::{Ctx, Key, Screen, Transition};
use crate::theme;
use crate::ui::components::containers::layout_column;
use crate::ui::components::widget::{Flex, Widget};

const SEEK_STEP: f64 = 5.0;
/// Height of the control block (title + scrub + hints).
const BAR_H: f32 = 180.0;
/// Hide chrome after this many seconds without input.
const HIDE_AFTER_SECS: f32 = 5.0;
const FADE_TAU: f32 = 0.2;

pub struct PlayerScreen {
    title: String,
    started: bool,
    /// Seconds since last control-revealing input.
    idle_secs: f32,
    /// 1 = fully shown, 0 = hidden.
    chrome_vis: Tween,
}

impl PlayerScreen {
    pub fn new(title: String) -> Self {
        Self {
            title,
            started: false,
            idle_secs: 0.0,
            chrome_vis: Tween::new(1.0, FADE_TAU),
        }
    }

    fn bump_controls(&mut self) {
        self.idle_secs = 0.0;
        self.chrome_vis.set_target(1.0);
    }
}

impl Screen for PlayerScreen {
    fn update(&mut self, dt: f32, ctx: &mut Ctx) {
        if !self.started {
            ctx.video.set_visible(true);
            ctx.video.load_and_play(SAMPLE_VIDEO_URL);
            self.started = true;
            self.bump_controls();
        }

        self.idle_secs += dt;
        if self.idle_secs >= HIDE_AFTER_SECS {
            self.chrome_vis.set_target(0.0);
        }
        self.chrome_vis.step(dt);
    }

    fn handle_key(&mut self, key: Key, ctx: &mut Ctx) -> Transition {
        match key {
            Key::Back => {
                ctx.video.pause();
                ctx.video.set_visible(false);
                return Transition::Pop;
            }
            Key::Enter => {
                self.bump_controls();
                if ctx.video.is_paused() {
                    ctx.video.play();
                } else {
                    ctx.video.pause();
                }
            }
            Key::Left => {
                self.bump_controls();
                let t = (ctx.video.current_time() - SEEK_STEP).max(0.0);
                ctx.video.seek(t);
            }
            Key::Right => {
                self.bump_controls();
                let dur = ctx.video.duration();
                let mut t = ctx.video.current_time() + SEEK_STEP;
                if dur > 0.0 {
                    t = t.min(dur);
                }
                ctx.video.seek(t);
            }
            Key::Up | Key::Down => {
                self.bump_controls();
            }
        }
        Transition::None
    }

    fn render(&mut self, r: &mut dyn Renderer, ctx: &mut Ctx) {
        let vis = self.chrome_vis.value().clamp(0.0, 1.0);
        if vis <= 0.01 {
            return;
        }

        let full = Rect::design();
        let margin = ctx.metrics.safe_margin;
        // Lift the whole chrome column above the bottom safe area.
        let area = full.inset(Insets {
            top: 0.0,
            right: 0.0,
            bottom: margin,
            left: 0.0,
        });

        let mut spacer = PlayerSpacer {
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        };
        let mut chrome = PlayerChrome {
            title: self.title.clone(),
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
            alpha: vis,
        };

        {
            let sp = &mut spacer as &mut dyn Widget;
            let ch = &mut chrome as &mut dyn Widget;
            layout_column(area, 0.0, &mut [sp, ch]);
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
    alpha: f32,
}

impl Widget for PlayerChrome {
    fn flex(&self) -> Flex {
        Flex::Fixed(BAR_H)
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn render(&self, r: &mut dyn Renderer, ctx: &Ctx) {
        let a = (self.alpha * 255.0) as u8;
        let scrim_a = ((theme::SCRIM.a as f32) * self.alpha) as u8;
        let full_w = crate::DESIGN_WIDTH as i32;
        let (bx, by, bw, _bh) = self.bounds.as_i32();

        // Scrim stays inside the chrome band (no bleed past the bottom).
        r.fill_rect(
            0,
            by - 24,
            full_w,
            BAR_H as i32 + 24,
            Color::rgba(0, 0, 0, scrim_a),
        );

        let text = theme::TEXT.with_alpha(a);
        let dim = theme::TEXT_DIM.with_alpha(a);
        let track = theme::TRACK.with_alpha(a);
        let accent = theme::FOCUS.with_alpha(a);
        let hint = Color::rgb(140, 140, 150).with_alpha(a);

        r.draw_text(bx, by + 8, 40, text, &self.title);

        let cur = ctx.video.current_time();
        let dur = ctx.video.duration();
        let frac = if dur > 0.0 {
            (cur / dur).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };

        let track_y = by + 68;
        let track_h = 8;
        r.fill_rect(bx, track_y, bw, track_h, track);
        r.fill_rect(
            bx,
            track_y,
            (bw as f32 * frac) as i32,
            track_h,
            accent,
        );
        let knob_x = bx + (bw as f32 * frac) as i32;
        r.fill_circle(knob_x, track_y + track_h / 2, 10, accent);

        let state = if ctx.video.is_paused() {
            "❚❚ Paused"
        } else {
            "▶ Playing"
        };
        let line = format!("{state}    {} / {}", fmt_time(cur), fmt_time(dur));
        r.draw_text(bx, track_y + 24, 26, dim, &line);
        r.draw_text(
            bx,
            track_y + 58,
            22,
            hint,
            "Enter: play/pause   ◀ ▶: seek 5s   Back: details",
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

    #[test]
    fn formats_times() {
        assert_eq!(fmt_time(0.0), "0:00");
        assert_eq!(fmt_time(9.0), "0:09");
        assert_eq!(fmt_time(75.0), "1:15");
        assert_eq!(fmt_time(3661.0), "1:01:01");
        assert_eq!(fmt_time(f64::NAN), "0:00");
    }

    #[test]
    fn controls_autohide_after_idle() {
        let cat = Catalog::sample();
        let metrics = Metrics::tv();
        let mut video = NullSink;
        let mut ctx = Ctx {
            catalog: &cat,
            metrics: &metrics,
            video: &mut video,
        };
        let mut screen = PlayerScreen::new("T".into());
        screen.update(0.0, &mut ctx);
        assert!((screen.chrome_vis.target() - 1.0).abs() < 1e-4);

        screen.update(HIDE_AFTER_SECS + 0.1, &mut ctx);
        assert!((screen.chrome_vis.target() - 0.0).abs() < 1e-4);

        screen.handle_key(Key::Enter, &mut ctx);
        assert!((screen.chrome_vis.target() - 1.0).abs() < 1e-4);
        assert!(screen.idle_secs < 0.01);
    }
}
