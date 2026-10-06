// Spec: specs/drlg/outdoor.md, specs/drlg/outdoor-act3-act5.md
//! Act V level build (`outdoor.md` §11, `outdoor-act3-act5.md` §5).

use super::grid::Gen;
use super::OutdoorError;

impl Gen<'_> {
    /// Act V `0x0067E600` (§11).
    pub fn act5(&mut self) -> Result<(), OutdoorError> {
        if self.id == 110 {
            // Siege strip `0x0067E560`.
            let s = self.od.preset(865)?.size_x / 8;
            for i in 0..15 {
                let x = self.gw() - s * (i + 1);
                if x < 0 {
                    return Err(OutdoorError::SiegeStrip(865 + i as u32));
                }
                self.stamp(x, 0, 865 + i as u32, 0, false)?;
            }
            return Ok(());
        }
        self.link_flags()?;
        // TODO(outdoor.md §11, OQ 9): the barricade border walk
        // (`0x0067DCF0`), ravine (`0x0067DEF0`), entrances (`0x0067DB50`),
        // caves (`0x0067DA70`), siege link (`0x0067E4B0`), the barricade
        // border substitution (type 12, `outdoor-tilesub.md` §2.3 — see
        // `BorderCtx::barricade`), prisons (`0x0067E240`) and special
        // presets (`0x0067E160`) are not specified beyond their order;
        // not built.
        Ok(())
    }
}
