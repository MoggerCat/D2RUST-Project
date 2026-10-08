// Spec: specs/items/treasure.md (§3.1–§3.5, §7); specs/sim/units.md (§4.6)
//! The play host's monster death start `0x005A6FF0` (`Pending::monster_death_start`).
//!
//! The start's body is not written beyond two callees (`units.md` §4.6):
//! it sets mode DT and calls the drop gate and drop (`treasure.md` §3.1,
//! [`monster_death_drop`]). The play game holds the drop state on
//! `ActionHooks::object_drops` (the chest drop's, `single_player.rs`);
//! a game without it drops nothing. The items go to the game's one item
//! store and the server's item pass announces them (0x9C ground items).
//!
//! PROVISIONAL (M22; REC-104): the free-spot seam of a game without a
//! path walk-back field takes the start spot as is. `// d2rs-own, unverified`.

use d2_sim::units::hooks::Sim;
use d2_sim::units::{modes, RoomId, UnitId};
use d2_sim::wiring::action::{ActionHooks, Pending};
use d2_sim::wiring::economy::{monster_death_drop, DropSpot, FreeSpot};

/// The monster death mode (`MONSTER_MODES[0]`, DT).
const MODE_DT: u32 = 0;

/// The start spot as is (`treasure.md` §7 step 2 without the collision
/// search). d2rs-own, unverified.
struct StartSpot;

impl FreeSpot for StartSpot {
    fn free_spot(
        &mut self,
        room: Option<RoomId>,
        start: (i32, i32),
        _: (i32, i32),
    ) -> Option<DropSpot> {
        Some(DropSpot {
            room,
            x: start.0,
            y: start.1,
        })
    }
}

/// Sets mode DT and drops the monster's treasure when the game holds
/// the drop state. Returns whether the mode started.
pub fn death_start<X: Pending>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
    target: Option<UnitId>,
) -> bool {
    if modes::set_mode(sim, h, unit, MODE_DT).is_err() {
        return false;
    }
    if let Some(mut d) = h.object_drops.take() {
        monster_death_drop(h, sim, &mut d, &mut StartSpot, unit, target);
        h.object_drops = Some(d);
    }
    true
}
