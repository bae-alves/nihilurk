//! An inclusive tile rectangle — one room, as `crate::map` lays it out.

/// A room's footprint: `x1..=x2` by `y1..=y2`, inclusive on every edge.
///
/// Rooms never need an overlap test: `build_tiles` places at most one per
/// section of a 3x3 grid with gutters between, so two of them cannot touch.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Rect {
    /// The left edge, inclusive.
    pub x1: i32,
    /// The right edge, inclusive.
    pub x2: i32,
    /// The top edge, inclusive.
    pub y1: i32,
    /// The bottom edge, inclusive.
    pub y2: i32,
}

impl Rect {
    /// The room with its top-left tile at `(x, y)`. The edges are inclusive, so
    /// `x2` is `x + w` and the room is one tile wider than `w` (and taller than
    /// `h`).
    ///
    /// ```
    /// use models::Rect;
    ///
    /// let room = Rect::new(2, 3, 4, 2);
    /// assert_eq!((room.x1, room.x2, room.y1, room.y2), (2, 6, 3, 5));
    /// assert_eq!(room.center(), (4, 4));
    /// ```
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
