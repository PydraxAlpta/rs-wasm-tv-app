//! Content model. New pages/rails/cards are added here without touching the
//! rendering or navigation layers. Pure data shapes only — sample/demo
//! content lives in whichever app is consuming this crate (e.g. `tv-app`'s
//! `content::sample_catalog()`), not here.

/// One selectable tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: u32,
    pub title: String,
    /// Poster art URL (loaded asynchronously by the renderer).
    pub image_url: String,
}

/// A horizontal carousel of cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rail {
    pub title: String,
    pub cards: Vec<Card>,
}

/// Full-width hero slide above the rails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BannerSlide {
    pub title: String,
    pub image_url: String,
}

/// The whole browse page: hero banners + an ordered list of rails.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Catalog {
    pub banners: Vec<BannerSlide>,
    pub rails: Vec<Rail>,
}
