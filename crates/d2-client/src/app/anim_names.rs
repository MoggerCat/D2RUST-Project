// Spec: specs/formats/animdata.md (§2, §5, OQ2); specs/sim/units.md §4.3; specs/render/unit-composite.md §2
//! The AnimData key of a server unit in a mode, for the play host.
//!
//! The server's mode start looks the unit's animation up by the COF name
//! the composer `0x0064F5B0` builds (`<token><mode><weapon class>`,
//! `animdata.md` §5). That composer for players and units with an
//! inventory is `animdata.md` OQ2, so the host answers with the client
//! art's own name rules ([`unit_cof`]): the same unit token and mode
//! token tables, the D1 weapon-class fill (bare hands, no items;
//! `// d2rs-own, unverified`). Without it every mode start that needs an
//! animation (attack, cast, get-hit, death) ends in `Anim(NoRecord)` and
//! its action frame never fires. PROVISIONAL until OQ2 is specified.

use d2_sim::units::UnitType;

use crate::bridge::world::{ClientUnit, UnitKey};
use crate::world_view::unit_assets::{unit_cof, UnitLooks};

/// The 8-byte NUL-padded AnimData key of a unit of `ty` and `class` in
/// `mode`; `None` for a unit without a composite or a name over 7 bytes.
// d2rs-own, unverified.
pub fn anim_key(looks: &UnitLooks, ty: UnitType, class: u32, mode: u32) -> Option<[u8; 8]> {
    let mut unit = ClientUnit::new(UnitKey::new(ty as u8, 0));
    unit.class = class;
    unit.mode = mode;
    let name = unit_cof(looks, &unit)?.short().to_ascii_uppercase();
    let b = name.as_bytes();
    (b.len() <= 8).then(|| {
        let mut k = [0u8; 8];
        k[..b.len()].copy_from_slice(b);
        k
    })
}

/// The animation rate `0x00623F50`: the AnimData speed as is (no rate
/// stats or states in the preview; the spec is not written).
// d2rs-own, unverified.
pub fn anim_rate(speed: Option<u32>) -> i16 {
    speed.map_or(0, |s| s as i16)
}
