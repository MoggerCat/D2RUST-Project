// Spec: specs/client/ui.md
//! Window → frame coordinate mapping (spec §A4).
//!
//! The 800×600 image is presented scaled by an integer factor and centered
//! with black bars (`render-pipeline.md` §A9). A cursor position in window
//! pixels maps back by the inverse: subtract the bar, divide by the scale
//! with integer division, clamp to the frame. Positions in the bars are
//! outside the frame. The present stage must place the image with the
//! same [`Presentation`] so both directions agree.

use super::geom::{Point, FRAME_H, FRAME_W};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    /// `render-pipeline.md` §A9 only defines integer scales ≥ 1; a window
    /// smaller than the frame has none (open question in the notes).
    #[error("window {w}×{h} is smaller than the 800×600 frame")]
    TooSmall { w: u32, h: u32 },
}

/// Where a window position lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePos {
    Inside(Point),
    /// In a black bar or outside the window.
    Outside,
}

/// Placement of the frame in a window, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presentation {
    pub window_w: u32,
    pub window_h: u32,
    /// Largest integer factor with `800·scale ≤ w` and `600·scale ≤ h`.
    pub scale: u32,
    /// Left bar width: `(w − 800·scale) / 2`, rounded down (ours).
    pub left: u32,
    /// Top bar height: `(h − 600·scale) / 2`, rounded down (ours).
    pub top: u32,
}

impl Presentation {
    pub fn new(window_w: u32, window_h: u32) -> Result<Self, FrameError> {
        let scale = (window_w / u32::from(FRAME_W)).min(window_h / u32::from(FRAME_H));
        if scale == 0 {
            return Err(FrameError::TooSmall {
                w: window_w,
                h: window_h,
            });
        }
        Ok(Self {
            window_w,
            window_h,
            scale,
            left: (window_w - u32::from(FRAME_W) * scale) / 2,
            top: (window_h - u32::from(FRAME_H) * scale) / 2,
        })
    }

    /// The window pixel (physical) at the top-left of a frame pixel: the
    /// inverse of [`Self::to_frame`] (`to_frame(from_frame(p)) == p`).
    pub fn from_frame(&self, fx: i32, fy: i32) -> (i64, i64) {
        let s = i64::from(self.scale);
        (
            i64::from(self.left) + i64::from(fx) * s,
            i64::from(self.top) + i64::from(fy) * s,
        )
    }

    /// The offset (physical pixels, x right, y up) from the window's
    /// centre to the centre of the presented image placed with its
    /// top-left at ([`Self::left`], [`Self::top`]): what a sprite centred
    /// on the window must move by so the image sits where
    /// [`Self::to_frame`] maps the cursor (`seams/bridge-app.md` §2.7).
    /// Non-zero (a half pixel) only with an odd leftover width or height.
    pub fn centre_offset(&self) -> (f32, f32) {
        let s = self.scale as f32;
        let cx = self.left as f32 + f32::from(FRAME_W) * s / 2.0;
        let cy = self.top as f32 + f32::from(FRAME_H) * s / 2.0;
        (
            cx - self.window_w as f32 / 2.0,
            self.window_h as f32 / 2.0 - cy,
        )
    }

    /// Maps a window pixel (physical, top-left origin) to the frame.
    pub fn to_frame(&self, x: i64, y: i64) -> FramePos {
        let s = i64::from(self.scale);
        // Saturating: a far-off position (the edge floors any float,
        // infinities included) stays outside instead of wrapping.
        let dx = x.saturating_sub(i64::from(self.left));
        let dy = y.saturating_sub(i64::from(self.top));
        let (fw, fh) = (i64::from(FRAME_W), i64::from(FRAME_H));
        if dx < 0 || dy < 0 || dx >= fw * s || dy >= fh * s {
            return FramePos::Outside;
        }
        // Inside the image the quotient is already in range; the clamp is
        // the spec's §A4 step and keeps the result in the frame by
        // construction.
        let fx = (dx / s).clamp(0, fw - 1);
        let fy = (dy / s).clamp(0, fh - 1);
        FramePos::Inside(Point::new(fx as i32, fy as i32))
    }
}
