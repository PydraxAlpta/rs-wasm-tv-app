//! tv-ui-web: JS bindings for mounting `tv-ui` widgets onto a host-provided
//! canvas, without going through a full app shell (`tv-app`'s `setupApp`).
//!
//! This crate is the seam `tv-ui-webgl` was built for: a caller supplies a
//! `<canvas>` it already owns (and its own content, as typed objects) and
//! gets back a live handle: mutate content on the fly (`setRails` /
//! `appendRails` / `updateRail`, no full re-render), listen for `select` /
//! `focuschange` via a plain `EventTarget`, and `unmount()` to tear down.
//! Named generically (not "carousel-only") — more `tv-ui` components can grow
//! their own `mount*` export here later. Only `mountCarousels` exists today.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use tsify_next::Tsify;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CustomEvent, CustomEventInit, EventTarget, HtmlCanvasElement, KeyboardEvent};

use tv_ui::geom::Rect;
use tv_ui::model::{Card, Catalog, Rail};
use tv_ui::screen::{Ctx, Key, NullVideoSink};
use tv_ui::ui::components::widget::tick_widget;
use tv_ui::ui::{FocusResult, RailList, Widget};
use tv_ui::{Color, Metrics, Renderer};
use tv_ui_webgl::{context_from_canvas, ImageCache, WebGl2Renderer};

/// Design-space resolution for an embedded carousel strip — much smaller than
/// `tv-app`'s full 1920×1080 stage, since this mounts into existing page layout.
const EMBED_WIDTH: u32 = 960;
const EMBED_HEIGHT: u32 = 660;

/// Dark, opaque clear — unlike `tv-app` there is no `<video>` underlay to show
/// through, so the canvas paints its own background.
const CLEAR: Color = Color::rgba(18, 18, 22, 255);

// --- Typed content the host page supplies -----------------------------------
//
// Plain JS objects in, via tsify-next: the generated `tv_ui_web.d.ts` carries
// real `CardInput`/`RailInput` interfaces, no `JSON.stringify` round trip.

#[derive(Tsify, Deserialize)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct CardInput {
    pub id: u32,
    pub title: String,
    pub image_url: String,
}

#[derive(Tsify, Deserialize)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct RailInput {
    pub title: String,
    pub cards: Vec<CardInput>,
}

impl From<CardInput> for Card {
    fn from(c: CardInput) -> Self {
        Card {
            id: c.id,
            title: c.title,
            image_url: c.image_url,
        }
    }
}

impl From<RailInput> for Rail {
    fn from(r: RailInput) -> Self {
        Rail {
            title: r.title,
            cards: r.cards.into_iter().map(Card::from).collect(),
        }
    }
}

// --- Outbound event payloads -------------------------------------------------
//
// Dispatched as `CustomEvent` `detail`s on the handle's `eventTarget` (see
// `emit` below) — real `SelectEvent`/`FocusEvent` interfaces in the .d.ts.

#[derive(Tsify, Serialize, Clone)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct FocusEvent {
    pub rail_index: usize,
    pub card_index: usize,
}

#[derive(Tsify, Serialize, Clone)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct SelectEvent {
    pub rail_index: usize,
    pub card_index: usize,
    pub card_id: u32,
    pub card_title: String,
}

/// Build and dispatch a `CustomEvent` on `target`. Must be called with no
/// `App` borrow held — a listener that calls back into `appendRails` etc.
/// would otherwise re-enter the `RefCell` and panic.
fn emit<T: Serialize>(target: &EventTarget, kind: &str, detail: &T) {
    let Ok(detail) = serde_wasm_bindgen::to_value(detail) else {
        return;
    };
    let init = CustomEventInit::new();
    init.set_detail(&detail);
    if let Ok(ev) = CustomEvent::new_with_event_init_dict(kind, &init) {
        let _ = target.dispatch_event(&ev);
    }
}

/// A `Metrics` sized so three rails fit inside [`EMBED_WIDTH`]×[`EMBED_HEIGHT`]
/// — much smaller than `Metrics::tv()`'s card/spacing tokens.
/// `tv_ui::Metrics` has no hardcoded design resolution, so any consumer can
/// build its own like this.
///
/// Two `RailList::render` details don't scale with `Metrics` and drive the
/// numbers below: it draws each rail's title at a *fixed* 30px font at
/// `row_top - rail_title_h`, and the focused card's name at a *fixed* 28px
/// font at `row_top + card_h + 14`. So `rail_title_h` must stay >= the fixed
/// title font size (else the title text spills downward past `row_top`, into
/// its own row's cards), and `rail_step` must leave room for the *previous*
/// row's fixed-size name text before the next row's title starts.
fn embed_metrics() -> Metrics {
    let rail_title_h = 40.0;
    let card_h = 110.0;
    Metrics {
        safe_margin: 24.0,
        header_h: 0.0,
        banner_h: 0.0,
        card_w: 75.0,
        card_h,
        card_gap: 10.0,
        rail_title_h,
        rail_step: card_h + rail_title_h + 70.0,
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
    events: EventTarget,
    /// Last focus reported via a `focuschange` event, to detect movement.
    last_focus: Option<(usize, usize)>,
}

impl App {
    /// Advance one frame. Returns a `FocusEvent` if focus moved since the
    /// last tick; the caller must emit it after releasing the `App` borrow.
    fn tick(&mut self, ts: f64) -> Option<FocusEvent> {
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

        let focus = self.rail.focus();
        if self.last_focus != Some(focus) {
            self.last_focus = Some(focus);
            Some(FocusEvent {
                rail_index: focus.0,
                card_index: focus.1,
            })
        } else {
            None
        }
    }

    /// Route a key into the rail stack. There is no screen stack here, so a
    /// `MoveOut` (focus trying to leave the rail area) has nowhere to go and
    /// is dropped; `Activate` (Enter) is turned into a `SelectEvent` by the
    /// caller, which has access to the activated card.
    fn handle_key(&mut self, key: Key) -> FocusResult {
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
        rail.handle_key(key, &mut ctx)
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

    fn selected_card(&self) -> Option<SelectEvent> {
        let (rail_index, card_index) = self.rail.focus();
        let card = self.catalog.rails.get(rail_index)?.cards.get(card_index)?;
        Some(SelectEvent {
            rail_index,
            card_index,
            card_id: card.id,
            card_title: card.title.clone(),
        })
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
        // Release the borrow before emitting — a `focuschange` listener may
        // call back into the handle (e.g. `appendRails`). Cloning `events`
        // into a local ends the borrow immediately; passing `&app.borrow()...`
        // directly would keep it alive for the whole `emit` call, including
        // the listener it synchronously invokes, and panic on re-entry.
        let focus_event = app.borrow_mut().tick(performance_now());
        if let Some(event) = focus_event {
            let events = app.borrow().events.clone();
            emit(&events, "focuschange", &event);
        }
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

        let result = app.borrow_mut().handle_key(key);
        // Emit outside the borrow: a `select` listener may call back into
        // the handle (e.g. to `appendRails` more content). As above, clone
        // `events` into a local first — passing `&app.borrow()...` directly
        // would keep the borrow alive through the listener and panic.
        if result == FocusResult::Activate {
            let selected = app.borrow().selected_card();
            if let Some(event) = selected {
                let events = app.borrow().events.clone();
                emit(&events, "select", &event);
            }
        }
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
    app: Rc<RefCell<App>>,
    events: EventTarget,
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
    /// Fires `select` (a card was activated — `SelectEvent` detail) and
    /// `focuschange` (focus moved to another rail/card — `FocusEvent`
    /// detail). Standard `EventTarget`: `addEventListener`/
    /// `removeEventListener`, any number of listeners.
    #[wasm_bindgen(getter = eventTarget)]
    pub fn event_target(&self) -> EventTarget {
        self.events.clone()
    }

    /// Replace all content and reset focus to the first rail/card — for
    /// loading a genuinely new dataset (contrast [`Self::append_rails`]).
    #[wasm_bindgen(js_name = setRails)]
    pub fn set_rails(&self, rails: Vec<RailInput>) {
        let mut app = self.app.borrow_mut();
        app.catalog.rails = rails.into_iter().map(Rail::from).collect();
        app.rail = RailList::new();
        app.rail.set_focused(true);
        app.last_focus = None;
    }

    /// Add more rails to the end without resetting focus, scroll position,
    /// running animations, or the GPU texture cache — "load more".
    #[wasm_bindgen(js_name = appendRails)]
    pub fn append_rails(&self, rails: Vec<RailInput>) {
        let mut app = self.app.borrow_mut();
        app.catalog.rails.extend(rails.into_iter().map(Rail::from));
    }

    /// Replace one rail's title/cards in place, by index. Out-of-range
    /// indices are ignored.
    #[wasm_bindgen(js_name = updateRail)]
    pub fn update_rail(&self, index: usize, rail: RailInput) {
        let mut app = self.app.borrow_mut();
        if let Some(slot) = app.catalog.rails.get_mut(index) {
            *slot = Rail::from(rail);
        }
    }

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

/// Mount a small set of horizontal card rows onto `canvas`, reading `rails`
/// as plain typed objects (`{ title, cards: [{ id, title, imageUrl }] }`).
/// Arrow keys move focus once the canvas has it (give it `tabindex` in host
/// CSS/HTML, or click it, to receive keys). Listen on the returned handle's
/// `eventTarget` for `select`/`focuschange`, and use `setRails`/
/// `appendRails`/`updateRail` to change content on a live mount.
#[wasm_bindgen(js_name = mountCarousels)]
pub fn mount_carousels(
    canvas: HtmlCanvasElement,
    rails: Vec<RailInput>,
) -> Result<CarouselHandle, JsValue> {
    let catalog = Catalog {
        banners: Vec::new(),
        rails: rails.into_iter().map(Rail::from).collect(),
    };

    canvas.set_width(EMBED_WIDTH);
    canvas.set_height(EMBED_HEIGHT);

    let gl = context_from_canvas(&canvas)
        .ok_or_else(|| JsValue::from_str("WebGL2 is not available in this browser"))?;

    let images = ImageCache::new();
    let renderer = WebGl2Renderer::new(gl, EMBED_WIDTH, EMBED_HEIGHT, images);

    let mut rail = RailList::new();
    rail.set_focused(true);

    let events = EventTarget::new()?;

    let app = Rc::new(RefCell::new(App {
        renderer,
        catalog,
        metrics: embed_metrics(),
        rail,
        design: Rect::new(0.0, 0.0, EMBED_WIDTH as f32, EMBED_HEIGHT as f32),
        last_ts: None,
        events: events.clone(),
        last_focus: None,
    }));

    let running = Rc::new(Cell::new(true));
    let frame = start_animation_loop(app.clone(), running.clone());
    let keydown = install_keydown(&canvas, app.clone());
    let keyup = install_keyup(&canvas, app.clone());

    Ok(CarouselHandle {
        app,
        events,
        canvas,
        running,
        _frame: frame,
        keydown: Some(keydown),
        keyup: Some(keyup),
    })
}
