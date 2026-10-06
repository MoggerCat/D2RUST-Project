// Spec: specs/sim/tick.md §5.5, §5.6; specs/sim/units.md §3.2, §5, §6.2; specs/monsters/init.md §22
//! The world state in [`super::WorldSim`]'s timer events and unit
//! removals. Both run the action systems with the world state lent to
//! the action hooks ([`super::monster_world`]), so the action hooks'
//! monster routes reach it:
//!
//! - Monster event 7 (`0x005A4370`) → [`init::dispatch`] in mode 2 on the
//!   monster's data (`init.md` §22). The unit dispatch keeps its checks
//!   (handler table, the frozen-monster drop of `tick.md` §5.6, event 7
//!   included) before the hook runs.
//! - A unit removed while an event runs, by an action adapter (missile
//!   collide-kill, …) or through [`WorldSim::remove_unit`] (`units.md`
//!   §3.2, the kind free) also leaves the world state: monster data
//!   (unit +0x14), its minion list, its owner link, a superunique's
//!   pending init tail ([`WorldState::forget`]).
//!
//! [`init::dispatch`]: crate::monsters::init::dispatch

use crate::game::Game;
use crate::units::UnitId;

use super::{WorldPending, WorldSim, WorldState};

impl WorldState {
    /// A removed unit leaves the world state: its monster data, its
    /// minion list, its owner link and a pending superunique tail.
    pub fn forget(&mut self, unit: UnitId) {
        self.monsters.remove(unit);
        self.minions.remove(&unit);
        self.owners.remove(&unit);
        self.superunique_tail.remove(&unit);
    }
}

impl<X: WorldPending> WorldSim<X> {
    /// Unit removal `0x00555600` (`units.md` §3.2) with the world state
    /// lent: the entry for a host (or the corpse and death code once
    /// specified) that removes a unit outside the timer events.
    pub fn remove_unit(&mut self, game: &mut Game, unit: UnitId) {
        self.with(game, |g, v| v.remove(g, unit));
    }
}
