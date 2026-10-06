// Spec: specs/client/render-pipeline.md (A6)
//! Draw order: the `DrawKey` layout and the stable sort.

use super::item::DrawItem;
use super::SceneError;

/// Sort key of a draw item: `pass:4 | major:28 | minor:24 | sub:8`, most
/// significant first (§A6). Items with equal keys keep build order.
///
/// TODO(spec: render/draw-order.md): which pass numbers exist and in which
/// order (floor, shadows, world, roof, UI, cursor), and the rule functions
/// filling `major`/`minor`/`sub` (§B6). Here the fields are plain numbers.
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

/// Sorts items by key, stably: ties keep build order (§A6). The compositor
/// draws in list order and never reorders.
pub fn order(items: &mut [DrawItem]) {
    items.sort_by_key(|item| item.key);
}
