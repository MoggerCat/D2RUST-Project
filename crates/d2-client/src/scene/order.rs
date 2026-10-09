// Spec: specs/client/render-pipeline.md (A6), specs/render/draw-order.md (§10)
//! Draw order: the `DrawKey` layout, the pass numbers and the stable sort.

use super::item::DrawItem;
use super::SceneError;

/// Sort key of a draw item: `pass:4 | major:28 | minor:24 | sub:8`, most
/// significant first (§A6). Items with equal keys keep build order.
///
/// The pass numbers are [`pass`] (`draw-order.md` §10); the world's
/// `major`/`minor` come from `rules::draw_order`, `sub` from the unit
/// composite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DrawKey(pub u64);

impl DrawKey {
    pub const PASS_MAX: u32 = (1 << 4) - 1;
    pub const MAJOR_MAX: u32 = (1 << 28) - 1;
    pub const MINOR_MAX: u32 = (1 << 24) - 1;

    /// Packs the four fields; a field out of range is an error, never
    /// truncated.
    pub fn new(pass: u32, major: u32, minor: u32, sub: u8) -> Result<Self, SceneError> {
        for (field, value, max) in [
            ("pass", pass, Self::PASS_MAX),
            ("major", major, Self::MAJOR_MAX),
            ("minor", minor, Self::MINOR_MAX),
        ] {
            if value > max {
                return Err(SceneError::KeyField { field, value, max });
            }
        }
        Ok(DrawKey(
            u64::from(pass) << 60 | u64::from(major) << 32 | u64::from(minor) << 8 | u64::from(sub),
        ))
    }

    pub fn pass(&self) -> u32 {
        (self.0 >> 60) as u32
    }

    pub fn major(&self) -> u32 {
        (self.0 >> 32) as u32 & Self::MAJOR_MAX
    }

    pub fn minor(&self) -> u32 {
        (self.0 >> 8) as u32 & Self::MINOR_MAX
    }

    pub fn sub(&self) -> u8 {
        self.0 as u8
    }
}

/// The `pass` field of each frame pass (`draw-order.md` §1, §10).
pub mod pass {
    /// Levels 74 / 120 (`0x00476290` / `0x00476460`).
    pub const LEVEL_BACKGROUND: u32 = 1;
    pub const LOWER_WALLS: u32 = 2;
    pub const FLOORS: u32 = 3;
    /// `0x00473C00` (draw-order open question 2).
    pub const UNIDENTIFIED_4: u32 = 4;
    pub const SHADOWS: u32 = 5;
    pub const WALLS_UNITS: u32 = 6;
    pub const ROOFS: u32 = 7;
    /// `0x00475B20`, `0x00473910` (open question 2).
    pub const UNIDENTIFIED_8: u32 = 8;
    pub const UNIDENTIFIED_9: u32 = 9;
    /// `0x004DC000`.
    pub const SCREEN_FADE: u32 = 10;
    /// Everything after the world draw `0x00476BC0`.
    pub const UI: u32 = 11;
    /// The UI pass's majors (`ui/panels.md` §5): the automap of step 3
    /// draws before every panel draw of steps 4–10.
    pub const UI_AUTOMAP_MAJOR: u32 = 0;
    /// The panels' draws (the UI root's list, in emission order).
    pub const UI_PANELS_MAJOR: u32 = 1;
}

/// Sorts items by key, stably: ties keep build order (§A6). The compositor
/// draws in list order and never reorders.
pub fn order(items: &mut [DrawItem]) {
    items.sort_by_key(|item| item.key);
}
