//! `VideoSink` backed by a DOM `<video>` element.

use crate::screen::VideoSink;
use web_sys::HtmlVideoElement;

pub struct HtmlVideoSink {
    el: HtmlVideoElement,
}

impl HtmlVideoSink {
    pub fn new(el: HtmlVideoElement) -> Self {
        Self { el }
    }
}

impl VideoSink for HtmlVideoSink {
    fn load_and_play(&mut self, url: &str) {
        // `current_src` is the resolved absolute URL; only reload if different.
        if self.el.current_src() != url {
            self.el.set_src(url);
            self.el.load();
        }
        let _ = self.el.play();
    }

    fn play(&mut self) {
        let _ = self.el.play();
    }

    fn pause(&mut self) {
        let _ = self.el.pause();
    }

    fn is_paused(&self) -> bool {
        self.el.paused()
    }

    fn current_time(&self) -> f64 {
        self.el.current_time()
    }

    fn duration(&self) -> f64 {
        let d = self.el.duration();
        if d.is_finite() {
            d
        } else {
            0.0
        }
    }

    fn seek(&mut self, t: f64) {
        self.el.set_current_time(t);
    }

    fn set_visible(&mut self, visible: bool) {
        let _ = self
            .el
            .style()
            .set_property("display", if visible { "block" } else { "none" });
    }
}
