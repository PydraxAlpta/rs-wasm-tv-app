//! Shared async image loader for the WebGL2 backend.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::HtmlImageElement;

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

/// Loads images by URL once and hands the decoded `<img>` element to the backend.
pub struct ImageCache {
    entries: HashMap<String, Entry>,
    this: Weak<RefCell<ImageCache>>,
}

pub type ImageCacheHandle = Rc<RefCell<ImageCache>>;

impl ImageCache {
    pub fn new() -> ImageCacheHandle {
        let cache = Rc::new(RefCell::new(Self {
            entries: HashMap::new(),
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
        if self.entries.contains_key(url) {
            return;
        }

        let element = match HtmlImageElement::new() {
            Ok(el) => el,
            Err(_) => {
                self.entries.insert(
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
        self.entries.insert(
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

    /// The decoded element, once ready; kicks off a load on first request.
    pub fn html_image(this: &ImageCacheHandle, url: &str) -> Option<HtmlImageElement> {
        Self::request(this, url);
        let cache = this.borrow();
        let entry = cache.entries.get(url)?;
        if entry.status != Status::Ready {
            return None;
        }
        entry.element.clone()
    }
}
