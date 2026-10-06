// Spec: specs/drlg/outdoor.md
//! Kurast and Travincal stamps (§9.4): border rows of levels 79–81,
//! fixed and random presets (`0x0067F190`, `0x0067EED0`), Kurast
//! Causeway and Travincal (`0x0067F3B0`).

use super::grid::Gen;
use super::OutdoorError;

impl Gen<'_> {
    /// Kurast `0x0067F190` with its border rows (§9.4); levels 79..82.
    pub fn kurast(&mut self) -> Result<(), OutdoorError> {
        todo!()
    }

    /// Travincal `0x0067F3B0` (§9.4); level 83.
    pub fn travincal(&mut self) -> Result<(), OutdoorError> {
        todo!()
    }
}
