// Spec: specs/client/ui.md
//! Integer geometry of the 800×600 logical frame (spec §A1, §A5).

/// Logical frame width (spec §A5).
pub const FRAME_W: u16 = 800;
/// Logical frame height (spec §A5).
pub const FRAME_H: u16 = 600;
/// The whole logical frame.
pub const FRAME: Rect = Rect::new(0, 0, FRAME_W, FRAME_H);

/// An integer point in the 800×600 frame (ours; the spec's `IVec2`, kept
/// free of Bevy types).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// An integer rectangle: `x <= px < x + w`, `y <= py < y + h`. Sizes are
/// unsigned, so a negative size cannot exist.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }

    pub const fn origin(&self) -> Point {
        Point::new(self.x, self.y)
    }

    /// Exclusive right edge, in `i64`: a rect may end past `i32::MAX`.
    pub const fn right(&self) -> i64 {
        self.x as i64 + self.w as i64
    }

    /// Exclusive bottom edge, in `i64` (as [`Self::right`]).
    pub const fn bottom(&self) -> i64 {
        self.y as i64 + self.h as i64
    }

    /// Half-open containment; an empty rect contains nothing.
    pub const fn contains(&self, p: Point) -> bool {
        p.x >= self.x
            && (p.x as i64) < self.right()
            && p.y >= self.y
            && (p.y as i64) < self.bottom()
    }
}
