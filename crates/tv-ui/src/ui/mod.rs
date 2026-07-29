//! UI layer: reusable components and full-screen pages.

pub mod components;
pub mod pages;

pub use components::carousel::draw_card_row;
pub use components::rail_list::RailList;
pub use components::{BannerCarousel, Flex, FocusResult, HCarousel, Widget};
pub use pages::{CatalogPage, MainShell, PlayerScreen};
