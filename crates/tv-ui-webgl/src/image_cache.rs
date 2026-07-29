//! Shared async image loader for the WebGL2 backend (LRU-bounded).

use std::cell::RefCell;
use std::num::NonZeroUsize;
use std::rc::{Rc, Weak};

use lru::LruCache;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::HtmlImageElement;

/// Decoded-image (system memory) budget. Kept larger than the GPU texture cap
/// (`webgl2::IMAGE_TEX_CAP`) on purpose: RAM is cheap relative to VRAM, so we
/// retain sources for several rails above/below the viewport. When a GPU texture
/// is evicted, its `<img>` is likely still here, so re-upload skips a refetch.
const IMAGE_CACHE_CAP: usize = 192;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Loading,
    Ready,
    Failed,
}

struct Entry {
    status: Status,
    element: Option<HtmlImageElement>,
}

/// Loads images by URL and retains a bounded LRU of decoded `<img>` elements.
pub struct ImageCache {
    entries: LruCache<String, Entry>,
    this: Weak<RefCell<ImageCache>>,
}

pub type ImageCacheHandle = Rc<RefCell<ImageCache>>;

impl ImageCache {
    pub fn new() -> ImageCacheHandle {
        let cap = NonZeroUsize::new(IMAGE_CACHE_CAP).expect("IMAGE_CACHE_CAP > 0");
        let cache = Rc::new(RefCell::new(Self {
            entries: LruCache::new(cap),
            this: Weak::new(),
        }));
        cache.borrow_mut().this = Rc::downgrade(&cache);
        cache
    }

    fn handle(&self) -> ImageCacheHandle {
        self.this
            .upgrade()
            .expect("ImageCache handle dropped while cache is borrowed")
    }

    /// Kick off a load if this URL has not been seen yet.
    pub fn request(this: &ImageCacheHandle, url: &str) {
        this.borrow_mut().request_inner(url);
    }

    fn request_inner(&mut self, url: &str) {
        // `contains` does not promote — keep Loading entries from jumping to MRU
        // until a draw actually uses them.
        if self.entries.contains(url) {
            return;
        }

        let element = match HtmlImageElement::new() {
            Ok(el) => el,
            Err(_) => {
                self.entries.put(
                    url.to_string(),
                    Entry {
                        status: Status::Failed,
                        element: None,
                    },
                );
                return;
            }
        };

        element.set_cross_origin(Some("anonymous"));
        self.entries.put(
            url.to_string(),
            Entry {
                status: Status::Loading,
                element: Some(element.clone()),
            },
        );

        let handle = self.handle();
        let key_ok = url.to_string();
        let on_load = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            if let Some(entry) = handle.borrow_mut().entries.get_mut(&key_ok) {
                entry.status = Status::Ready;
            }
        }) as Box<dyn FnMut(web_sys::Event)>);

        let handle_err = self.handle();
        let key_err = url.to_string();
        let on_error = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            if let Some(entry) = handle_err.borrow_mut().entries.get_mut(&key_err) {
                entry.status = Status::Failed;
                entry.element = None;
            }
        }) as Box<dyn FnMut(web_sys::Event)>);

        element.set_onload(Some(on_load.as_ref().unchecked_ref()));
        element.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        element.set_src(url);
        on_load.forget();
        on_error.forget();
    }

    /// Promote `url` to most-recently-used **if it is already cached**, without
    /// starting a load. Called when a URL is drawn from its GPU texture so the
    /// decoded-image LRU tracks actual on-screen usage instead of drifting toward
    /// eviction while its texture stays hot (see the double-cache note in
    /// `doc/RENDERING.md`). `get` promotes when present and is a no-op when absent,
    /// so this never triggers a fetch.
    pub fn touch(this: &ImageCacheHandle, url: &str) {
        let _ = this.borrow_mut().entries.get(url);
    }

    /// The decoded element, once ready; kicks off a load on first request.
    /// A successful hit promotes the URL to most-recently-used.
    pub fn html_image(this: &ImageCacheHandle, url: &str) -> Option<HtmlImageElement> {
        Self::request(this, url);
        let mut cache = this.borrow_mut();
        let entry = cache.entries.get(url)?;
        if entry.status != Status::Ready {
            return None;
        }
        entry.element.clone()
    }
}
