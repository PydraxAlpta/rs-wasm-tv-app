//! tv-ui-web: JS bindings for mounting `tv-ui` widgets onto a host-provided
//! canvas, without going through a full app shell (`tv-app`'s `setupApp`).
//!
//! This crate is the seam `tv-ui-webgl` was built for: a caller supplies a
//! `<canvas>` it already owns (and its own content, as JSON) and gets back a
//! small, self-contained render+input loop it can tear down with `unmount()`.
//! Named generically (not "carousel-only") — more `tv-ui` components can grow
//! their own `mount*` export here later. Only `mountCarousels` exists today.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, KeyboardEvent};

use tv_ui::geom::Rect;
use tv_ui::model::{Card, Catalog, Rail};
use tv_ui::screen::{Ctx, Key, NullVideoSink};
use tv_ui::ui::components::widget::tick_widget;
use tv_ui::ui::{RailList, Widget};
use tv_ui::{Color, Metrics, Renderer};
use tv_ui_webgl::{context_from_canvas, ImageCache, WebGl2Renderer};

/// Design-space resolution for an embedded carousel strip — much smaller than
/// `tv-app`'s full 1920×1080 stage, since this mounts into existing page layout.
const EMBED_WIDTH: u32 = 960;
const EMBED_HEIGHT: u32 = 600;

/// Dark, opaque clear — unlike `tv-app` there is no `<video>` underlay to show
/// through, so the canvas paints its own background.
const CLEAR: Color = Color::rgba(18, 18, 22, 255);

// --- JSON content the host page supplies -----------------------------------

#[derive(Deserialize)]
struct CardDto {
    id: u32,
    title: String,
    #[serde(rename = "imageUrl")]
    image_url: String,
}

#[derive(Deserialize)]
struct RailDto {
    title: String,
    cards: Vec<CardDto>,
}

#[derive(Deserialize)]
struct CatalogDto {
    rails: Vec<RailDto>,
}

impl From<CatalogDto> for Catalog {
    fn from(dto: CatalogDto) -> Self {
        Catalog {
            banners: Vec::new(),
            rails: dto
                .rails
                .into_iter()
                .map(|r| Rail {
                    title: r.title,
                    cards: r
                        .cards
                        .into_iter()
                        .map(|c| Card {
                            id: c.id,
                            title: c.title,
                            image_url: c.image_url,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

/// A `Metrics` sized so three rails fit inside [`EMBED_WIDTH`]×[`EMBED_HEIGHT`]
/// — much smaller than `Metrics::tv()`'s card/spacing tokens.
/// `tv_ui::Metrics` has no hardcoded design resolution, so any consumer can
/// build its own like this.
///
/// `rail_step` needs more headroom than a naive scale-down of `Metrics::tv()`
/// suggests: `RailList::render` draws the focused card's title below each row
/// at a *fixed* pixel font size (28px name + 30px next rail title, regardless
/// of `Metrics`), so shrinking `card_h` alone without keeping that fixed text
/// height in mind causes the next rail's title to overlap it.
fn embed_metrics() -> Metrics {
    let rail_title_h = 18.0;
    let card_h = 110.0;
    Metrics {
        safe_margin: 24.0,
        header_h: 0.0,
        banner_h: 0.0,
        card_w: 75.0,
        card_h,
        card_gap: 10.0,
        rail_title_h,
        rail_step: card_h + rail_title_h + 60.0,
        focus_x: 24.0,
    }
}

// --- Render/input loop -------------------------------------------------------

struct App {
    renderer: WebGl2Renderer,
    catalog: Catalog,
    metrics: Metrics,
    rail: RailList,
    design: Rect,
    last_ts: Option<f64>,
}

impl App {
    fn tick(&mut self, ts: f64) {
        let frame_ms = match self.last_ts {
            Some(prev) => (ts - prev).max(0.0),
            None => 0.0,
        };
        self.last_ts = Some(ts);
        let dt = (frame_ms / 1000.0) as f32;

        // Split-borrow so `Ctx` can hold catalog/metrics while the renderer is
        // used independently — same pattern as `tv-app`'s `App::tick`.
        let App {
            renderer,
            catalog,
            metrics,
            rail,
            design,
            ..
        } = self;
        let mut video = NullVideoSink;
        let mut ctx = Ctx {
            catalog,
            metrics,
            video: &mut video,
        };

        renderer.begin_frame(CLEAR);
        tick_widget(rail, *design, dt, renderer, &mut ctx);
        renderer.end_frame();
    }

    /// Route a key into the rail stack. There is no screen stack here, so any
    /// `FocusResult` (e.g. `Activate` on Enter, `MoveOut` at an edge) is
    /// dropped — this widget has nowhere to hand it off to.
    fn handle_key(&mut self, key: Key) {
        let App {
            catalog,
            metrics,
            rail,
            ..
        } = self;
        let mut video = NullVideoSink;
        let mut ctx = Ctx {
            catalog,
            metrics,
            video: &mut video,
        };
        let _ = rail.handle_key(key, &mut ctx);
    }

    fn handle_key_up(&mut self, key: Key) {
        let App {
            catalog,
            metrics,
            rail,
            ..
        } = self;
        let mut video = NullVideoSink;
        let mut ctx = Ctx {
            catalog,
            metrics,
            video: &mut video,
        };
        let _ = rail.handle_key_up(key, &mut ctx);
    }
}

fn map_key(event: &KeyboardEvent) -> Option<Key> {
    match event.key().as_str() {
        "ArrowUp" => Some(Key::Up),
        "ArrowDown" => Some(Key::Down),
        "ArrowLeft" => Some(Key::Left),
        "ArrowRight" => Some(Key::Right),
        "Enter" | " " | "Spacebar" => Some(Key::Enter),
        "Backspace" | "Escape" => Some(Key::Back),
        _ => None,
    }
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

/// The rAF closure is self-referencing (it reschedules itself), so it's held
/// behind `Rc<RefCell<Option<..>>>` — see `start_animation_loop`.
type FrameClosure = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect_throw("requestAnimationFrame failed");
}

/// Self-rescheduling rAF loop, gated on `running` each frame so `unmount()`
/// can stop it (rather than `tv-app`'s `forget()`, which never stops).
fn start_animation_loop(app: Rc<RefCell<App>>, running: Rc<Cell<bool>>) -> FrameClosure {
    let frame: FrameClosure = Rc::new(RefCell::new(None));
    let frame_for_callback = frame.clone();

    *frame.borrow_mut() = Some(Closure::wrap(Box::new(move |_timestamp_ms: f64| {
        if !running.get() {
            return;
        }
        app.borrow_mut().tick(performance_now());
        request_animation_frame(frame_for_callback.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut(f64)>));

    request_animation_frame(frame.borrow().as_ref().unwrap());
    frame
}

fn install_keydown(
    canvas: &HtmlCanvasElement,
    app: Rc<RefCell<App>>,
) -> Closure<dyn FnMut(KeyboardEvent)> {
    let handler = Closure::wrap(Box::new(move |event: KeyboardEvent| {
        let Some(key) = map_key(&event) else {
            return;
        };
        // App-driven hold chaining for arrows; ignore OS auto-repeat.
        if event.repeat() && matches!(key, Key::Up | Key::Down | Key::Left | Key::Right) {
            event.prevent_default();
            return;
        }
        event.prevent_default();
        app.borrow_mut().handle_key(key);
    }) as Box<dyn FnMut(KeyboardEvent)>);

    canvas
        .add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref())
        .expect_throw("failed to add keydown listener");
    handler
}

fn install_keyup(
    canvas: &HtmlCanvasElement,
    app: Rc<RefCell<App>>,
) -> Closure<dyn FnMut(KeyboardEvent)> {
    let handler = Closure::wrap(Box::new(move |event: KeyboardEvent| {
        let Some(key) = map_key(&event) else {
            return;
        };
        event.prevent_default();
        app.borrow_mut().handle_key_up(key);
    }) as Box<dyn FnMut(KeyboardEvent)>);

    canvas
        .add_event_listener_with_callback("keyup", handler.as_ref().unchecked_ref())
        .expect_throw("failed to add keyup listener");
    handler
}

/// Handle to a mounted carousel strip. Keeps the render loop and input
/// listeners alive; call [`CarouselHandle::unmount`] to tear both down.
#[wasm_bindgen]
pub struct CarouselHandle {
    canvas: HtmlCanvasElement,
    running: Rc<Cell<bool>>,
    // Kept alive only so the rAF closure isn't dropped mid-flight; `unmount`
    // stops rescheduling via `running` rather than cancelling the frame id.
    _frame: FrameClosure,
    keydown: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    keyup: Option<Closure<dyn FnMut(KeyboardEvent)>>,
}

#[wasm_bindgen]
impl CarouselHandle {
    /// Stop the render loop and remove the keyboard listeners this mount
    /// installed on its canvas. Idempotent — safe to call more than once.
    pub fn unmount(&mut self) {
        self.running.set(false);
        if let Some(handler) = self.keydown.take() {
            let _ = self
                .canvas
                .remove_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
        }
        if let Some(handler) = self.keyup.take() {
            let _ = self
                .canvas
                .remove_event_listener_with_callback("keyup", handler.as_ref().unchecked_ref());
        }
    }
}

/// Mount a small set of horizontal card rows onto `canvas`, reading content
/// from `data_json` (`{ "rails": [{ "title", "cards": [{ "id", "title",
/// "imageUrl" }] }] }`). Arrow keys move focus once the canvas has it
/// (give it `tabindex` in host CSS/HTML, or click it, to receive keys).
#[wasm_bindgen(js_name = mountCarousels)]
pub fn mount_carousels(
    canvas: HtmlCanvasElement,
    data_json: &str,
) -> Result<CarouselHandle, JsValue> {
    let dto: CatalogDto = serde_json::from_str(data_json)
        .map_err(|e| JsValue::from_str(&format!("invalid carousel data: {e}")))?;
    let catalog: Catalog = dto.into();

    canvas.set_width(EMBED_WIDTH);
    canvas.set_height(EMBED_HEIGHT);

    let gl = context_from_canvas(&canvas)
        .ok_or_else(|| JsValue::from_str("WebGL2 is not available in this browser"))?;

    let images = ImageCache::new();
    let renderer = WebGl2Renderer::new(gl, EMBED_WIDTH, EMBED_HEIGHT, images);

    let mut rail = RailList::new();
    rail.set_focused(true);

    let app = Rc::new(RefCell::new(App {
        renderer,
        catalog,
        metrics: embed_metrics(),
        rail,
        design: Rect::new(0.0, 0.0, EMBED_WIDTH as f32, EMBED_HEIGHT as f32),
        last_ts: None,
    }));

    let running = Rc::new(Cell::new(true));
    let frame = start_animation_loop(app.clone(), running.clone());
    let keydown = install_keydown(&canvas, app.clone());
    let keyup = install_keyup(&canvas, app);

    Ok(CarouselHandle {
        canvas,
        running,
        _frame: frame,
        keydown: Some(keydown),
        keyup: Some(keyup),
    })
}
