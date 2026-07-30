//! Test-only fixtures shared across this crate's `#[cfg(test)]` modules.
//! Not part of the public API — generic navigation-exercise data, not a copy
//! of any app's demo content (that lives in the consuming app instead).

use crate::geom::Rect;
use crate::model::{BannerSlide, Card, Catalog, Rail};

/// A reasonable design-space size for tests — the exact value doesn't matter
/// for behavior, just needs to be nonzero/sane (`tv-ui` has no opinion on
/// resolution; real consumers supply their own via `Ctx::design`).
pub(crate) fn test_design() -> Rect {
    Rect::new(0.0, 0.0, 1920.0, 1080.0)
}

const BANNER_COUNT: usize = 5;
const RAIL_COUNT: usize = 20;
const PER_RAIL: usize = 20;

/// Same shape as `tv-app`'s old demo catalog (5 banners, 20 rails × 20
/// cards) so hold-to-scroll/lazy-reveal tests have enough content to
/// exercise, but with plain generic labels instead of demo flavor.
pub(crate) fn sample_catalog() -> Catalog {
    let mut banners = Vec::with_capacity(BANNER_COUNT);
    for b in 0..BANNER_COUNT {
        banners.push(BannerSlide {
            title: format!("Banner {b}"),
            image_url: String::new(),
        });
    }

    let mut rails = Vec::with_capacity(RAIL_COUNT);
    let mut id = 0u32;
    for r in 0..RAIL_COUNT {
        let mut cards = Vec::with_capacity(PER_RAIL);
        for c in 0..PER_RAIL {
            cards.push(Card {
                id,
                title: format!("Card {r}-{c}"),
                image_url: String::new(),
            });
            id += 1;
        }
        rails.push(Rail {
            title: format!("Rail {r}"),
            cards,
        });
    }
    Catalog { banners, rails }
}
