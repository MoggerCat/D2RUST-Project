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
//! The play game's path provider carries the real `ExpField.D2` walk-back
//! field (`single_player::LiveData::expfield`), so the drop lands on the
//! floor-drop spot (`treasure.md` §7 step 2, `path-placement.md` §9) and
//! [`StartSpot`] below is only the seam's answer for a game without the
//! field (REC-108, settled by `tests/app_floor_drop.rs` and
//! `facts/items/a1-cold-plains-poke-kills.tsv`).

use d2_sim::units::hooks::Sim;
use d2_sim::units::{modes, UnitId};
use d2_sim::wiring::action::{ActionHooks, Pending};
use d2_sim::wiring::economy::{monster_death_drop, StartSpot};

/// The monster death mode (`MONSTER_MODES[0]`, DT).
const MODE_DT: u32 = 0;

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
    // Step 1.2's footprint (`units.md` §4.6): the corpse stops blocking
    // (the Act V cage doors, `quests-act5.md` §4.7).
    d2_sim::wiring::action::View::of(sim.units, sim.stats, sim.data, h).death_footprint(unit);
    if let Some(mut d) = h.object_drops.take() {
        monster_death_drop(h, sim, &mut d, &mut StartSpot, unit, target);
        h.object_drops = Some(d);
    }
    true
}
