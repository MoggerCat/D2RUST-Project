// Spec: specs/render/sprite-placement.md
//! What a draw at (X, Y) covers: cel placement for both orientations
//! (§2, §4), the rasterizer's row clipping (§5), DT1 blocks (§7), and the
//! d2rs mapping onto `IndexFrame` offsets and the `DrawItem` top-left
//! (§8). Integer math only.
//!
//! Transparency (§6) is the frame decoders' (`IndexFrame` index 0); it is
//! exact only while no live DC6 run or DT1 block pixel holds 0 (open
//! question 1, queued).

use crate::frames::{FrameAnchor, IndexFrame};
use crate::scene::Rect;

/// The cel fields the row drawer reads (§1): `w`, `h`, `xoff`, `yoff` and
/// bit 0 of the orientation word (`top_down`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cel {
    pub width: u32,
    pub height: u32,
    pub xoff: i32,
    pub yoff: i32,
    /// Orientation bit 0 set (§4).
    pub top_down: bool,
}

impl Cel {
    /// Columns covered by a draw at `x`, inclusive (§2): `X + xoff …
    /// X + xoff + w − 1`.
    pub fn columns(&self, x: i32) -> (i64, i64) {
        let first = i64::from(x) + i64::from(self.xoff);
        (first, first + i64::from(self.width) - 1)
    }

    /// Rows covered by a draw at `y`, inclusive, before clipping: `Y + yoff
    /// − h + 1 … Y + yoff` (§2), or `Y + yoff … Y + yoff + h − 1` with the
    /// orientation bit set (§4).
    pub fn rows(&self, y: i32) -> (i64, i64) {
        let anchor = i64::from(y) + i64::from(self.yoff);
        let h = i64::from(self.height);
        if self.top_down {
            (anchor, anchor + h - 1)
        } else {
            (anchor - h + 1, anchor)
        }
    }

    /// The rasterizer's row plan for a draw at `y` into a frame `height`
    /// rows high (§5).
    pub fn row_plan(&self, y: i32, height: i32) -> RowPlan {
        RowPlan::new(self, y, height)
    }
}

/// Which encoded rows the rasterizer draws and where (§5,
/// `0x00601530`–`0x00601559`). Computed as if the cel ran bottom-up from
/// row `Y + yoff`, whatever the orientation bit (Edge cases).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowPlan {
    /// Encoded rows skipped (the bottom clamp to `H − 1`).
    pub skip: u32,
    /// Screen row of the first drawn encoded row.
    pub start: i64,
    /// Encoded rows drawn (0: none).
    pub count: u32,
    /// The drawer's direction: down (top-down cel) or up (bottom-up).
    pub top_down: bool,
}

impl RowPlan {
    fn new(cel: &Cel, y: i32, height: i32) -> Self {
        let bottom = i64::from(y) + i64::from(cel.yoff);
        let last = i64::from(height) - 1;
        let skip = (bottom - last).max(0);
        let start = bottom.min(last);
        let left = (i64::from(cel.height) - skip).max(0);
        let count = left.min(start + 1).max(0);
        RowPlan {
            skip: skip.min(i64::from(cel.height)) as u32,
            start,
            count: count as u32,
            top_down: cel.top_down,
        }
    }

    /// Screen row of encoded row `k` (`skip ≤ k < skip + count`).
    pub fn screen_row(&self, k: u32) -> i64 {
        let i = i64::from(k) - i64::from(self.skip);
        if self.top_down {
            self.start + i
        } else {
            self.start - i
        }
    }
}

impl Cel {
    /// The cel the DCC decoder builds for a frame (§3): `xoff`, `yoff` are
    /// the frame header's `x offset`, `y offset` unchanged, and an even
    /// `variable0` leaves it bottom-up (odd ones are refused when frames
    /// are built, `frames::FrameError::DccVariable0`).
    pub fn dcc(x_offset: i32, y_offset: i32, width: u32, height: u32) -> Cel {
        Cel {
            width,
            height,
            xoff: x_offset,
            yoff: y_offset,
            top_down: false,
        }
    }

    /// The cel of a DC6 frame (§3): the frame header is the cel, `flip & 1`
    /// the orientation bit.
    pub fn dc6(flip: u32, width: u32, height: u32, offset_x: i32, offset_y: i32) -> Cel {
        Cel {
            width,
            height,
            xoff: offset_x,
            yoff: offset_y,
            top_down: flip & 1 == 1,
        }
    }
}

/// §8: the `DrawItem` top-left of `frame` drawn at (X, Y):
/// `(X + x_off, Y + y_off − height + 1)` for a DC6 frame with
/// `flip & 1 = 0`, `(X + x_off, Y + y_off)` for every other frame.
pub fn draw_position(frame: &IndexFrame, x: i32, y: i32) -> (i32, i32) {
    let top = match frame.anchor {
        FrameAnchor::Top | FrameAnchor::TopDown => y + frame.y_off,
        FrameAnchor::Bottom => y + frame.y_off - frame.height as i32 + 1,
    };
    (x + frame.x_off, top)
}

/// A frame placed for the compositor: the `DrawItem` top-left and the
/// clip that reproduces the rasterizer's rows (§5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Placed {
    pub x: i32,
    pub y: i32,
    /// `None`: no pixel is drawn.
    pub clip: Option<Rect>,
}

impl Placed {
    /// Whether `clip` draws the same pixels of a `width × height` image as
    /// the plain `frame` clip (so a draw shared clip can stand for it).
    pub fn same_as_frame(&self, width: u32, height: u32, frame: Rect) -> bool {
        let image = Rect::new(self.x, self.y, width, height);
        let ours = self
            .clip
            .and_then(|c| c.intersect(&frame))
            .and_then(|c| c.intersect(&image));
        ours == frame.intersect(&image)
    }
}

/// Places cel `cel` drawn at (X, Y) into `frame` (`[0, W) × [0, H)` at
/// the origin; column clip `[L, R) = [0, W)`, §5).
///
/// A bottom-up cel draws exactly its image rows inside `[0, H)`: its clip
/// is the frame. A top-down cel (§4, Edge cases) starts at row
/// `min(Y + yoff, H − 1)` after skipping `max(0, Y + yoff − (H − 1))` of
/// its top rows and draws at most `Y + yoff + 1` rows: the top-left moves
/// up by the skip and the clip keeps only the drawn rows. Rows the
/// original would write below the frame (outside the surface) are not
/// drawn.
pub fn place_cel(cel: &Cel, x: i32, y: i32, frame: Rect) -> Placed {
    let left = x + cel.xoff;
    if !cel.top_down {
        return Placed {
            x: left,
            y: y + cel.yoff - cel.height as i32 + 1,
            clip: Some(frame),
        };
    }
    let plan = cel.row_plan(y, frame.height as i32);
    let rows = Rect::new(frame.x, plan.start as i32, frame.width, plan.count);
    Placed {
        x: left,
        y: (plan.start - i64::from(plan.skip)) as i32,
        clip: if plan.count == 0 {
            None
        } else {
            rows.intersect(&frame)
        },
    }
}

/// [`place_cel`] for a d2rs frame (§8): a DC6 frame as its cel by
/// [`FrameAnchor`]; a DCC box or DT1 image at `(X + x_off, Y + y_off)`
/// clipped to the frame (a DCC frame is a bottom-up cel, §3; DT1 blocks
/// clip per pixel, §7).
pub fn place(frame: &IndexFrame, x: i32, y: i32, clip: Rect) -> Placed {
    let (w, h) = (frame.width, frame.height);
    match frame.anchor {
        FrameAnchor::Top => Placed {
            x: x + frame.x_off,
            y: y + frame.y_off,
            clip: Some(clip),
        },
        FrameAnchor::Bottom => place_cel(&Cel::dc6(0, w, h, frame.x_off, frame.y_off), x, y, clip),
        FrameAnchor::TopDown => place_cel(&Cel::dc6(1, w, h, frame.x_off, frame.y_off), x, y, clip),
    }
}

/// §7: screen position of DT1 block `(bx, by)`'s pixel `(px, py)` for a
/// tile whose blocks start at `origin` (the drawer's (X, Y), after the
/// floor drawer's −80 and panel shift, `camera::Camera::block_origin`).
pub fn block_pixel(origin: (i32, i32), block: (i32, i32), pixel: (i32, i32)) -> (i32, i32) {
    (origin.0 + block.0 + pixel.0, origin.1 + block.1 + pixel.1)
}
