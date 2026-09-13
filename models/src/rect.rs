//! An inclusive tile rectangle — one room, as `crate::map` lays it out.

/// A room's footprint: `x1..=x2` by `y1..=y2`, inclusive on every edge.
///
/// Rooms never need an overlap test: `build_tiles` places at most one per
/// section of a 3x3 grid with gutters between, so two of them cannot touch.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Rect {
    pub x1: i32,
    pub x2: i32,
    pub y1: i32,
    pub y2: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y + h,
        }
    }

    /// The room's middle tile — where the up-stair goes, and where the player
    /// lands on arriving.
    pub fn center(&self) -> (i32, i32) {
        ((self.x1 + self.x2) / 2, (self.y1 + self.y2) / 2)
    }
}
