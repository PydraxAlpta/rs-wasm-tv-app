//! Colour palette for the UI, kept separate so the look is easy to retune.

use crate::buffer::Color;

pub const BG: Color = Color::rgb(16, 16, 20);
pub const HEADER: Color = Color::rgb(235, 235, 240);
pub const RAIL_TITLE: Color = Color::rgb(210, 210, 218);
pub const CARD_BG: Color = Color::rgb(32, 32, 40);
pub const CARD_BORDER: Color = Color::rgb(58, 58, 70);
/// Focus ring / accent.
pub const FOCUS: Color = Color::rgb(120, 180, 255);
pub const TEXT: Color = Color::WHITE;
pub const TEXT_DIM: Color = Color::rgb(180, 180, 190);
/// Translucent scrim behind the player controls.
pub const SCRIM: Color = Color::rgba(0, 0, 0, 150);
/// Unfilled portion of the scrub track.
pub const TRACK: Color = Color::rgb(90, 90, 102);
