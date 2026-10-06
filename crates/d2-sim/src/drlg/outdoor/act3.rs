// Spec: specs/drlg/outdoor-act3-act5.md, specs/drlg/outdoor.md
//! Act III level build `0x0067F450` (`outdoor.md` §9.3,
//! `outdoor-act3-act5.md` §4): link flags, jungle stamping
//! (`outdoor-act3-act5.md` §3), Kurast and Travincal (`kurast.rs`).

use super::grid::Gen;
use super::jungle::{fatal, BLOCK};
use super::OutdoorError;

/// lvlprest 573 "Jungle Head" (64×32, Files 0).
pub const JUNGLE_HEAD: u32 = 573;
/// lvlprest 574 "Jungle Tail" (64×32, Files 0).
pub const JUNGLE_TAIL: u32 = 574;
/// Clearing family add per level 76..78 (`0x006F2370`).
pub const CLEARING_FAMILY: [u32; 3] = [0, 10, 20];
/// Clearing file orders G (`0x006F2328`), 6 × 3.
pub const CLEARING_FILES: [i32; 18] = [0, 1, 2, 1, 0, 2, 0, 2, 1, 1, 2, 0, 2, 0, 1, 2, 1, 0];

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
        let diff = (self.drlg.difficulty as usize).min(2);
        let (sx, sy) = self.data.level(76)?.size[diff];
        let (sxb, syb) = (sx / BLOCK, sy / BLOCK);
        // Step 1: helper 0x0045C390, drawn before the check of step 2.
        let n = 2 + 4 * i32::from(self.info.jungle_clearings == 3);
        let r = self.seed().roll(n) as usize;
        // Step 2.
        let ids = self
            .info
            .jungle_ids
            .clone()
            .ok_or(OutdoorError::Fatal(fatal::NO_IDS))?;
        // TODO(spec §3): an id array shorter than SXb·SYb (leveldefs 76
        // changed between creation and build) is not described; read as 0.
        let id_at = |i: i32| ids.get(i as usize).copied().unwrap_or(0);
        // Step 3.
        let mut idx = 0i32;
        let mut c = 0usize;
        for i in 0..syb {
            if self.id == 76 && i == syb - 1 {
                let f = i32::from(id_at(sxb * syb - 1) == 0);
                self.stamp(0, 4 * i, JUNGLE_HEAD, f, false)?;
                idx += 2;
            } else if self.id == 78 && i == 0 {
                let f = i32::from(id_at(1) == 0);
                self.stamp(0, 0, JUNGLE_TAIL, f, false)?;
                idx += 2;
            } else {
                for j in 0..sxb {
                    let mut p = id_at(idx);
                    idx += 1;
                    let mut f = -1;
                    if p > JUNGLE_TAIL {
                        if c >= 3 {
                            return Err(OutdoorError::Fatal(fatal::CLEARINGS));
                        }
                        // `0x006F2370`: Webby, Boggy, Pygmy.
                        p += CLEARING_FAMILY[(self.id as usize).saturating_sub(76).min(2)];
                        f = CLEARING_FILES[3 * r + c];
                        c += 1;
                    }
                    if p != 0 {
                        self.stamp(4 * j, 4 * i, p, f, false)?;
                    }
                }
            }
        }
        Ok(())
    }
}
