// Spec: specs/drlg/outdoor-act3-act5.md, specs/drlg/outdoor.md
//! Act III level build `0x0067F450` (`outdoor.md` §9.3,
//! `outdoor-act3-act5.md` §4): link flags, jungle stamping
//! (`outdoor-act3-act5.md` §3), Kurast and Travincal (`kurast.rs`).

use super::grid::Gen;
use super::OutdoorError;

impl Gen<'_> {
    /// Act III `0x0067F450` (§9.3).
    pub fn act3(&mut self) -> Result<(), OutdoorError> {
        self.link_flags()?;
        if (76..=78).contains(&self.id) {
            self.jungle_stamping()?;
        }
        self.kurast()?;
        self.travincal()
    }

    /// Jungle stamping `0x0067E910` (`outdoor-act3-act5.md` §3).
    pub fn jungle_stamping(&mut self) -> Result<(), OutdoorError> {
        todo!()
    }
}
