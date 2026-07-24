//! Focus routing for directional TV navigation between sibling widgets.

use crate::screen::{Ctx, Key};
use super::widget::{FocusResult, Widget};

/// Which focusable region is active on the browse screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusZone {
    Banner,
    Rails,
}

/// Ordered focus group: keys go to the active child; `MoveOut` walks the list.
pub struct FocusScope {
    focused: usize,
}

impl FocusScope {
    pub fn new(initial: usize) -> Self {
        Self { focused: initial }
    }

    pub fn index(&self) -> usize {
        self.focused
    }

    pub fn set_index(&mut self, index: usize) {
        self.focused = index;
    }

    /// Route `key` into `children[focused]`. Vertical `MoveOut` changes focus
    /// within the group; if the edge is hit, returns `MoveOut` to the parent.
    pub fn handle_key(
        &mut self,
        key: Key,
        ctx: &mut Ctx,
        children: &mut [&mut dyn Widget],
    ) -> FocusResult {
        if children.is_empty() {
            return FocusResult::Ignored;
        }
        self.focused = self.focused.min(children.len() - 1);
        let result = children[self.focused].handle_key(key, ctx);
        match result {
            FocusResult::MoveOut(Key::Down) => {
                if self.focused + 1 < children.len() {
                    self.focused += 1;
                    FocusResult::Handled
                } else {
                    FocusResult::MoveOut(Key::Down)
                }
            }
            FocusResult::MoveOut(Key::Up) => {
                if self.focused > 0 {
                    self.focused -= 1;
                    FocusResult::Handled
                } else {
                    FocusResult::MoveOut(Key::Up)
                }
            }
            other => other,
        }
    }
}

/// Map a focus index used by browse (0 = banner, 1 = rails) to [`FocusZone`].
pub fn zone_from_index(index: usize) -> FocusZone {
    if index == 0 {
        FocusZone::Banner
    } else {
        FocusZone::Rails
    }
}

pub fn index_from_zone(zone: FocusZone) -> usize {
    match zone {
        FocusZone::Banner => 0,
        FocusZone::Rails => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_index_round_trip() {
        assert_eq!(zone_from_index(0), FocusZone::Banner);
        assert_eq!(zone_from_index(1), FocusZone::Rails);
        assert_eq!(index_from_zone(FocusZone::Banner), 0);
        assert_eq!(index_from_zone(FocusZone::Rails), 1);
    }
}
