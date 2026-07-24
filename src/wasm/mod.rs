//! Browser entry point: builds the DOM, drives the rAF loop, and routes remote
//! / keyboard input into the screen stack.
//!
//! TV / d-pad key map:
//!   Arrows        → focus navigation
//!   Enter / Space → activate (open card / toggle playback)
//!   Escape / Backspace / Tizen Back (keyCode 10009) → back / exit

mod image_cache;
mod video;
mod webgl2;

use std::cell::RefCell;
use std::rc::Rc;

use image_cache::ImageCache;
use video::{JsPlayer, JsPlayerSink};
use webgl2::WebGl2Renderer;

use crate::renderer::Renderer;
use crate::screen::{Ctx, Key, Screen, Transition};
use crate::ui::BrowseScreen;
use crate::utils::set_panic_hook;
use crate::{Catalog, Color, Layout, DESIGN_HEIGHT, DESIGN_WIDTH};

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, HtmlElement, KeyboardEvent, WebGl2RenderingContext, WebGlContextAttributes,
};

/// Transparent clear so the `<video>` underlay shows through in the player.
const CLEAR: Color = Color::rgba(0, 0, 0, 0);

struct App {
    renderer: WebGl2Renderer,
    video: JsPlayerSink,
    catalog: Catalog,
    layout: Layout,
    stack: Vec<Box<dyn Screen>>,
    last_ts: Option<f64>,
}

impl App {
    fn tick(&mut self, ts: f64) {
        let dt = match self.last_ts {
            Some(prev) => (((ts - prev) / 1000.0) as f32).max(0.0),
            None => 0.0,
        };
        self.last_ts = Some(ts);

        // Split-borrow the fields so `Ctx` can hold the catalog/layout/video
        // while the renderer is used independently.
        let App {
            renderer,
            video,
            catalog,
            layout,
            stack,
            ..
        } = self;
        let mut ctx = Ctx {
            catalog,
            layout,
            video,
        };
        if let Some(top) = stack.last_mut() {
            top.update(dt, &mut ctx);
            renderer.begin_frame(CLEAR);
            top.render(renderer, &mut ctx);
            renderer.end_frame();
        }
    }

    /// Handle a logical key. Returns `true` if the app should exit.
    fn handle_key(&mut self, key: Key) -> bool {
        let App {
            video,
            catalog,
            layout,
            stack,
            ..
        } = self;
        let mut ctx = Ctx {
            catalog,
            layout,
            video,
        };
        let transition = match stack.last_mut() {
            Some(top) => top.handle_key(key, &mut ctx),
            None => Transition::None,
        };
        match transition {
            Transition::Push(screen) => {
                stack.push(screen);
                false
            }
            Transition::Pop => {
                stack.pop();
                stack.is_empty()
            }
            // Back on the root (browse) screen exits the app.
            Transition::None => key == Key::Back && stack.len() <= 1,
        }
    }
}

/// Boot the leanback UI. `player` is a JS PlayerAdapter that drives
/// `#player-video` after this function creates the element.
#[wasm_bindgen(js_name = setupApp)]
pub fn setup_app(root: HtmlElement, player: JsPlayer) {
    set_panic_hook();

    root.set_inner_html(
        r#"<div class="stage">
  <video id="player-video" class="video-underlay" playsinline loop crossorigin="anonymous" style="display:none"></video>
  <canvas id="ui" class="ui-canvas"></canvas>
</div>"#,
    );

    let canvas = query_el::<HtmlCanvasElement>(&root, "#ui");
    canvas.set_width(DESIGN_WIDTH);
    canvas.set_height(DESIGN_HEIGHT);
    let gl = webgl2_context(&canvas);

    let images = ImageCache::new();
    let renderer = WebGl2Renderer::new(gl, DESIGN_WIDTH, DESIGN_HEIGHT, images);

    let app = Rc::new(RefCell::new(App {
        renderer,
        video: JsPlayerSink::new(player),
        catalog: Catalog::sample(),
        layout: Layout::tv(),
        stack: vec![Box::new(BrowseScreen::new())],
        last_ts: None,
    }));

    install_keydown(app.clone());
    start_animation_loop(app);
}

fn query_el<T: JsCast>(root: &HtmlElement, selector: &str) -> T {
    root.query_selector(selector)
        .expect_throw("query_selector failed")
        .unwrap_or_else(|| wasm_bindgen::throw_str(&format!("element {selector} missing")))
        .dyn_into::<T>()
        .unwrap_or_else(|_| wasm_bindgen::throw_str(&format!("element {selector} has wrong type")))
}

/// WebGL2 context configured for a transparent overlay over the video.
fn webgl2_context(canvas: &HtmlCanvasElement) -> WebGl2RenderingContext {
    let attrs = WebGlContextAttributes::new();
    attrs.set_antialias(false);
    attrs.set_alpha(true);
    attrs.set_depth(false);
    attrs.set_stencil(false);
    attrs.set_premultiplied_alpha(false);

    canvas
        .get_context_with_context_options("webgl2", attrs.as_ref())
        .expect_throw("Failed to get WebGL2 context")
        .expect_throw("WebGL2 is not available in this browser")
        .dyn_into()
        .unwrap_throw()
}

fn map_key(event: &KeyboardEvent) -> Option<Key> {
    // Tizen remote Back arrives as a key code, not a named key.
    if event.key_code() == 10009 || event.key_code() == 461 {
        return Some(Key::Back);
    }
    match event.key().as_str() {
        "ArrowUp" => Some(Key::Up),
        "ArrowDown" => Some(Key::Down),
        "ArrowLeft" => Some(Key::Left),
        "ArrowRight" => Some(Key::Right),
        "Enter" | " " | "Spacebar" => Some(Key::Enter),
        "Backspace" | "Escape" | "GoBack" | "BrowserBack" => Some(Key::Back),
        _ => None,
    }
}

fn install_keydown(app: Rc<RefCell<App>>) {
    let handler = Closure::wrap(Box::new(move |event: KeyboardEvent| {
        let Some(key) = map_key(&event) else {
            return;
        };
        event.prevent_default();
        let should_exit = app.borrow_mut().handle_key(key);
        if should_exit {
            exit_app();
        }
    }) as Box<dyn FnMut(KeyboardEvent)>);

    window()
        .document()
        .expect_throw("no document")
        .add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref())
        .expect_throw("failed to add keydown listener");
    handler.forget();
}

fn window() -> web_sys::Window {
    web_sys::window().expect_throw("no global `window`")
}

fn performance_now() -> f64 {
    window()
        .performance()
        .expect_throw("no `performance`")
        .now()
}

fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect_throw("requestAnimationFrame failed");
}

fn start_animation_loop(app: Rc<RefCell<App>>) {
    let frame: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let frame_for_callback = frame.clone();

    *frame.borrow_mut() = Some(Closure::wrap(Box::new(move |_timestamp_ms: f64| {
        app.borrow_mut().tick(performance_now());
        request_animation_frame(frame_for_callback.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut(f64)>));

    request_animation_frame(frame.borrow().as_ref().unwrap());
}

/// Top-level exit: Tizen app exit if available, else `window.close()`.
fn exit_app() {
    let win = window();
    if let Ok(tizen) = Reflect::get(win.as_ref(), &JsValue::from_str("tizen")) {
        if !tizen.is_undefined() && !tizen.is_null() {
            if let Ok(application) = Reflect::get(&tizen, &JsValue::from_str("application")) {
                if let Ok(get_current) =
                    Reflect::get(&application, &JsValue::from_str("getCurrentApplication"))
                {
                    if let Ok(get_fn) = get_current.dyn_into::<js_sys::Function>() {
                        if let Ok(current_app) = get_fn.call0(&application) {
                            if let Ok(exit) = Reflect::get(&current_app, &JsValue::from_str("exit"))
                            {
                                if let Ok(exit_fn) = exit.dyn_into::<js_sys::Function>() {
                                    let _ = exit_fn.call0(&current_app);
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let _ = win.close();
}
