// Spec: specs/monsters/init.md §5, §22; specs/sim/units.md §3.1, §3.2
//! [`WorldState`] as the action hooks' [`MonsterWorld`]: the monster type
//! init, the umod dispatcher and the world state's part of a unit free,
//! each on a [`WorldHost`] built over the call's unit side and the world
//! state. [`WorldSim`] lends its world state to the action hooks around
//! every timer event and tick hook it forwards ([`WorldSim::lend`]), so
//! the action adapters (missiles, AI, combat, skill use, the kill) reach
//! it wherever they allocate, remove, change a monster's mode or query
//! monster data.
//!
//! Population and init calls ([`WorldSim::host`]) hold the world state
//! directly instead: the action hooks then have no world, so a monster
//! population allocates gets its type init from population's own call
//! ([`super::population_init`]), not a second one from the allocator's
//! hook.

use std::any::Any;

use crate::game::Game;
use crate::monsters::init::{self, MonsterData};
use crate::units::hooks::Sim;
use crate::units::UnitId;

use super::super::action::{ActionHooks, ActionSim, MonsterWorld};
use super::{View, WorldHost, WorldPending, WorldSim, WorldState};

impl WorldState {
    /// A world state with the same tables, level types and game info and
    /// nothing else: what [`WorldSim::world`] holds while the real one is
    /// lent.
    pub(super) fn placeholder(&self) -> Self {
        Self::new(self.types.clone(), self.tables.clone(), self.init_info)
    }
}

impl<X: WorldPending> MonsterWorld<X> for WorldState {
    fn region_classes(&self, level: u32) -> Option<Vec<i32>> {
        let r = self.pop.regions.get(level as i32)?;
        let n = usize::from(r.mon_count).min(r.entries.len());
        Some(r.entries[..n].iter().map(|e| i32::from(e.class)).collect())
    }
    fn monstats_count(&self) -> u32 {
        self.tables.monstats.len() as u32
    }
    fn type_init(&mut self, sim: &mut Sim<'_>, h: &mut ActionHooks<X>, unit: UnitId) {
        let t = self.tables.clone();
        let mut wh = host(sim, h, self);
        init::type_init(&t.init(), &mut wh, unit);
    }

    fn reinit(
        &mut self,
        sim: &mut Sim<'_>,
        h: &mut ActionHooks<X>,
        unit: UnitId,
        class: i32,
        mode: u32,
    ) -> bool {
        let t = self.tables.clone();
        let mut wh = host(sim, h, self);
        init::reinit(&t.init(), &mut wh, unit, class, mode)
    }

    fn umods(
        &mut self,
        sim: &mut Sim<'_>,
        h: &mut ActionHooks<X>,
        unit: UnitId,
        arg: Option<UnitId>,
        mode: u8,
    ) {
        let t = self.tables.clone();
        let mut wh = host(sim, h, self);
        init::dispatch(&t.init(), &mut wh, unit, arg, mode);
    }

    fn assign_umod(&mut self, sim: &mut Sim<'_>, h: &mut ActionHooks<X>, unit: UnitId, umod: u8) {
        let t = self.tables.clone();
        let mut wh = host(sim, h, self);
        init::assign_umod(&t.init(), &mut wh, unit, umod, false);
    }

    fn forget(&mut self, unit: UnitId) {
        WorldState::forget(self, unit);
    }

    fn monster(&self, unit: UnitId) -> Option<&MonsterData> {
        self.monsters.get(unit)
    }

    fn monster_mut(&mut self, unit: UnitId) -> Option<&mut MonsterData> {
        self.monsters.get_mut(unit)
    }

    fn component_counts(&self, class: u32) -> Option<[u8; 16]> {
        let m = self.tables.monstats.get(class as usize)?;
        self.tables
            .components
            .get(usize::from(m.monstatsex))
            .copied()
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

/// The [`WorldHost`] of a lent-world call.
fn host<'a, X>(
    sim: &'a mut Sim<'_>,
    h: &'a mut ActionHooks<X>,
    w: &'a mut WorldState,
) -> WorldHost<'a, X> {
    WorldHost {
        game: &mut *sim.game,
        v: View::of(&mut *sim.units, &mut *sim.stats, sim.data, h),
        w,
    }
}

impl<X: WorldPending> WorldSim<X> {
    /// Runs `f` on the action systems with the world state lent to the
    /// action hooks ([`ActionHooks::monster_world`]): the entry for a host
    /// that drives the action systems directly (messages, skill use)
    /// and still wants the monster routes. When the hooks already hold a
    /// world (a host lent its own), `f` runs with that one.
    pub fn lend<R>(&mut self, f: impl FnOnce(&mut ActionSim<X>) -> R) -> R {
        let hooks = &mut self.action.sys.hooks;
        if hooks.monster_world.is_some() {
            return f(&mut self.action);
        }
        let placeholder = self.world.placeholder();
        let held = std::mem::replace(&mut self.world, placeholder);
        hooks.monster_world = Some(Box::new(held));
        let r = f(&mut self.action);
        match self
            .action
            .sys
            .hooks
            .monster_world
            .take()
            .map(|w| w.into_any().downcast::<WorldState>())
        {
            Some(Ok(w)) => self.world = *w,
            // `f` took the world out of the hooks or put another one in:
            // the game's monster state is gone (API misuse, fatal).
            _ => panic!("WorldSim::lend: the lent world state did not come back"),
        }
        r
    }

    /// [`ActionSim::with`] with the world state lent (allocation, removal,
    /// mode changes and queries through the monster routes).
    pub fn with<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut Game, &mut View<'_, X>) -> R,
    ) -> R {
        self.lend(|a| a.with(game, f))
    }
}
