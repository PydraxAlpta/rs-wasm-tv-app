//! `VideoSink` that forwards playback to a JS `PlayerAdapter`.
//!
//! Rust creates `#player-video`; JS owns all subsequent media control. The
//! adapter discovers the element by id (TV: at most one underlay).

use tv_ui::screen::VideoSink;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_PLAYER_ADAPTER: &'static str = r#"
export interface PlayerAdapter {
  loadAndPlay(url: string): void;
  play(): void;
  pause(): void;
  isPaused(): boolean;
  currentTime(): number;
  duration(): number;
  seek(t: number): void;
  setVisible(visible: boolean): void;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "PlayerAdapter")]
    pub type JsPlayer;

    #[wasm_bindgen(method, js_name = loadAndPlay)]
    fn load_and_play(this: &JsPlayer, url: &str);

    #[wasm_bindgen(method)]
    fn play(this: &JsPlayer);

    #[wasm_bindgen(method)]
    fn pause(this: &JsPlayer);

    #[wasm_bindgen(method, js_name = isPaused)]
    fn is_paused(this: &JsPlayer) -> bool;

    #[wasm_bindgen(method, js_name = currentTime)]
    fn current_time(this: &JsPlayer) -> f64;

    #[wasm_bindgen(method)]
    fn duration(this: &JsPlayer) -> f64;

    #[wasm_bindgen(method)]
    fn seek(this: &JsPlayer, t: f64);

    #[wasm_bindgen(method, js_name = setVisible)]
    fn set_visible(this: &JsPlayer, visible: bool);
}

/// Thin bridge: screens keep talking to [`VideoSink`]; calls land in JS.
pub struct JsPlayerSink {
    player: JsPlayer,
}

impl JsPlayerSink {
    pub fn new(player: JsPlayer) -> Self {
        Self { player }
    }
}

impl VideoSink for JsPlayerSink {
    fn load_and_play(&mut self, url: &str) {
        self.player.load_and_play(url);
    }

    fn play(&mut self) {
        self.player.play();
    }

    fn pause(&mut self) {
        self.player.pause();
    }

    fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    fn current_time(&self) -> f64 {
        self.player.current_time()
    }

    fn duration(&self) -> f64 {
        self.player.duration()
    }

    fn seek(&mut self, t: f64) {
        self.player.seek(t);
    }

    fn set_visible(&mut self, visible: bool) {
        self.player.set_visible(visible);
    }
}
