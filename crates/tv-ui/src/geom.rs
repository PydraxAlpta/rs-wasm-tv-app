//! Basic geometry for layout and painting in design-space coordinates.

use crate::{DESIGN_HEIGHT, DESIGN_WIDTH};

/// Axis-aligned rectangle in design pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn from_size(size: Size) -> Self {
        Self::new(0.0, 0.0, size.w, size.h)
    }

    pub fn design() -> Self {
        Self::new(0.0, 0.0, DESIGN_WIDTH as f32, DESIGN_HEIGHT as f32)
    }

    pub fn size(self) -> Size {
        Size::new(self.w, self.h)
    }

    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    pub fn intersect(self, other: Self) -> Self {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        Self {
            x: x0,
            y: y0,
            w: (x1 - x0).max(0.0),
            h: (y1 - y0).max(0.0),
        }
    }

    pub fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    pub fn inset(self, insets: Insets) -> Self {
        Self {
            x: self.x + insets.left,
            y: self.y + insets.top,
            w: (self.w - insets.horizontal()).max(0.0),
            h: (self.h - insets.vertical()).max(0.0),
        }
    }

    pub fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px < self.right() && py < self.bottom()
    }

    pub fn as_i32(self) -> (i32, i32, i32, i32) {
        (self.x as i32, self.y as i32, self.w as i32, self.h as i32)
    }
}

/// Width × height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

impl Size {
    pub const fn new(w: f32, h: f32) -> Self {
        Self { w, h }
    }

    pub fn design() -> Self {
        Self::new(DESIGN_WIDTH as f32, DESIGN_HEIGHT as f32)
    }
}

/// Edge insets (padding / safe area).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Insets {
    pub const fn uniform(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    pub const fn vh(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inset_shrinks_rect() {
        let r = Rect::new(10.0, 20.0, 100.0, 80.0).inset(Insets::uniform(10.0));
        assert_eq!(r, Rect::new(20.0, 30.0, 80.0, 60.0));
    }

    #[test]
    fn intersect_clips() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, -10.0, 100.0, 60.0);
        let i = a.intersect(b);
        assert_eq!(i, Rect::new(50.0, 0.0, 50.0, 50.0));
        assert!(a.intersect(Rect::new(200.0, 0.0, 10.0, 10.0)).is_empty());
    }
}
