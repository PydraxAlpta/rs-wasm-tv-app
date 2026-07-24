//! UI layer: reusable components and full-screen pages.

pub mod components;
pub mod pages;

pub use components::{
    BannerCarousel, Flex, FocusResult, HCarousel, Widget,
};
pub use pages::{BrowseScreen, PlayerScreen};
