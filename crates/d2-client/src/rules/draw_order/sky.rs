// Spec: specs/render/draw-order-2.md (§11.6, §11.7)
//! The frame's pass 4 and pass 9 outputs as the view consumes them: the
//! pool cels ([`PoolDraw`]) and the sky draws ([`SkyDraw`]: lightning
//! flash, particle lines) of one frame, computed from the weather state
//! the feed lends ([`super::source::WeatherFrame`]).

use crate::rules::camera::{FrameSize, OpenMode};

use super::weather::{LevelWeather, PoolDraw, SkyDraw, Thunder};

/// What the passes need besides the weather state: the frame, the open
/// mode, `shiftX` (`camera.md` §1), the local player's level and the
/// pass-9 inputs the model does not hold (`d2rs-own, unverified` in the
/// preview).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkyFrame {
    pub frame: FrameSize,
    pub mode: OpenMode,
    pub shift_x: i32,
    pub level: LevelWeather,
    /// Frames counted in the last 1,000 ms (`[0x007BB390]`).
    pub frame_rate: u32,
    /// Low-quality setting `[0x0072DA50]` ≠ 0.
    pub low_quality: bool,
}

/// The draws of passes 4 and 9 for one frame, in pass order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkyPasses {
    /// Pass 4 (§11.6): splash cels, then bubble cels.
    pub pools: Vec<PoolDraw>,
    /// Pass 9 (§11.7): the flash, or the particle lines.
    pub sky: Vec<SkyDraw>,
    /// The thunder strike the pass started (sound 202): emitted, not
    /// played here (sound is deferred).
    pub thunder: Option<Thunder>,
}

impl SkyPasses {
    pub fn is_empty(&self) -> bool {
        self.pools.is_empty() && self.sky.is_empty()
    }
}
