// Spec: specs/render/composition.md (§2 framebuffer, §3 frame cycle, §4 palette)
//! The persistent index framebuffer and the frame cycle of 1.14d's GDI
//! reference renderer: clear at the start of a frame (BlankScreen), draw,
//! the post-draw clear, present through one palette.
//!
//! The clears are expressed as a [`FramePlan`] that both compositors apply
//! around the draws (CPU: [`super::cpu::compose_frame`]; GPU: the
//! `clear_rows` / `clear_after` params of `gpu_compositor`), so the frame
//! cycle is computed once and the two images stay byte-identical.
//!
//! §7 (DirectDraw: whole-surface clear each frame, gamma ramp, entry 0
//! black) belongs to display type 3, not to the GDI reference, and is not
//! implemented.

use d2_formats::palette::{Palette, Rgb};

use super::cpu;
use super::item::{DrawItem, FrameSource, MapTable};
use super::{Rect, SceneError};

/// Bottom rows the GDI `StartDraw` never clears (§3 step 2: the first
/// `(H − 47) × W` bytes are cleared).
pub const UNCLEARED_ROWS: u32 = 47;

/// Bytes of a PL2 file that hold the presented palette (§4): 256 entries
/// of R, G, B, x.
pub const PL2_PALETTE_BYTES: usize = 1024;

/// What one frame does around its draws, in view rows (§3 steps 2 and 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FramePlan {
    /// Rows `0..clear_rows` start the frame as index 0; the other rows
    /// keep the previous frame's index.
    pub clear_rows: u32,
    /// Every pixel becomes index 0 after all drawing (`ClearScreen(0)`
    /// when the counter `[0x0070F2C0]` is above 0).
    pub clear_after: bool,
}

impl FramePlan {
    /// No clear: draws go onto the base unchanged and stay.
    pub const NONE: FramePlan = FramePlan {
        clear_rows: 0,
        clear_after: false,
    };
}

/// The GDI driver's 8-bit framebuffer (§2) and the frame-cycle state it
/// carries between frames (§3): the pixels persist, and so does the
/// post-draw clear counter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameCycle {
    width: u32,
    height: u32,
    /// Row-major, top row first: pixel `(x, y)` is byte `y × W + x`.
    pixels: Vec<u8>,
    /// `[0x0070F2C0]`.
    post_clear: u32,
}

impl FrameCycle {
    /// A `width × height` framebuffer of index 0 with the counter at 0.
    /// The height must exceed [`UNCLEARED_ROWS`]: the §3 clear is defined
    /// only there (1.14d's modes are 480, 600 and 700 rows).
    pub fn new(width: u32, height: u32) -> Result<Self, SceneError> {
        Self::with_pixels(width, height, vec![0; width as usize * height as usize])
    }

    /// A framebuffer holding `pixels` (e.g. a recorded previous frame).
    pub fn with_pixels(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, SceneError> {
        if height <= UNCLEARED_ROWS {
            return Err(SceneError::FramebufferHeight { height });
        }
        if pixels.len() as u64 != u64::from(width) * u64::from(height) {
            return Err(SceneError::BaseSize {
                len: pixels.len(),
                pixels: u64::from(width) * u64::from(height),
            });
        }
        Ok(FrameCycle {
            width,
            height,
            pixels,
            post_clear: 0,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// The framebuffer as a view at the origin: what both compositors
    /// compose a frame of this cycle into.
    pub fn view(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    /// The framebuffer: after [`FrameCycle::compose`] or
    /// [`FrameCycle::commit`], the presented frame (§3: the framebuffer at
    /// `EndScene` entry).
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// The post-draw clear counter `[0x0070F2C0]`.
    pub fn post_clear(&self) -> u32 {
        self.post_clear
    }

    /// Sets the counter (`0x0044E100` writes 1; which event calls it is
    /// `composition.md` OQ4).
    pub fn set_post_clear(&mut self, counter: u32) {
        self.post_clear = counter;
    }

    /// The clears of the next frame. `blank_screen` is the BlankScreen
    /// flag of the player's current level (`Levels.txt`, §3 step 2).
    pub fn plan(&self, blank_screen: bool) -> FramePlan {
        FramePlan {
            clear_rows: if blank_screen {
                self.height - UNCLEARED_ROWS
            } else {
                0
            },
            clear_after: self.post_clear > 0,
        }
    }

    /// One whole frame on the CPU reference: [`FrameCycle::plan`], the
    /// draws onto the persistent framebuffer, then [`FrameCycle::commit`].
    /// Returns the presented index frame. On error nothing changes.
    pub fn compose<F: FrameSource + ?Sized>(
        &mut self,
        blank_screen: bool,
        items: &[DrawItem],
        frames: &F,
        maps: &MapTable,
    ) -> Result<&[u8], SceneError> {
        let plan = self.plan(blank_screen);
        let next = cpu::compose_frame(items, frames, maps, self.view(), &self.pixels, plan)?;
        self.commit(plan, next)?;
        Ok(&self.pixels)
    }

    /// Takes `presented` (a frame composed elsewhere, e.g. read back from
    /// the GPU compositor, with `plan` from [`FrameCycle::plan`]) as the
    /// new framebuffer and steps the counter as §3 step 4 does.
    pub fn commit(&mut self, plan: FramePlan, presented: Vec<u8>) -> Result<(), SceneError> {
        if presented.len() != self.pixels.len() {
            return Err(SceneError::BaseSize {
                len: presented.len(),
                pixels: self.pixels.len() as u64,
            });
        }
        if plan != self.plan(plan.clear_rows != 0) {
            return Err(SceneError::FramePlan(plan));
        }
        if plan.clear_after {
            self.post_clear -= 1;
        }
        self.pixels = presented;
        Ok(())
    }
}

/// The presented palette of an act (§4): the first 1,024 bytes of its
/// `pal.pl2`, index `i` = `(pl2[4i], pl2[4i + 1], pl2[4i + 2])`, index 0
/// included; byte `4i + 3` is not a color.
pub fn present_palette(pl2: &[u8]) -> Result<Palette, SceneError> {
    let head = pl2
        .get(..PL2_PALETTE_BYTES)
        .ok_or(SceneError::Pl2Size { len: pl2.len() })?;
    let mut palette = Palette {
        colors: [Rgb::default(); 256],
    };
    for (c, e) in palette.colors.iter_mut().zip(head.as_chunks::<4>().0) {
        *c = Rgb {
            r: e[0],
            g: e[1],
            b: e[2],
        };
    }
    Ok(palette)
}
