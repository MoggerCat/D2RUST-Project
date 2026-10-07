// Spec: specs/monsters/init.md §5, §22; specs/monsters/umod-callbacks.md §2; specs/sim/units.md §3.1, §3.2, §4.6; specs/combat/damage.md §5.2 step 9; specs/missiles/missiles.md rule 28; specs/combat/hit.md; specs/monsters/ai.md §2.4
//! The monster side of a game as the action hooks reach it: a
//! [`MonsterWorld`] lent to [`ActionHooks::monster_world`] (the
//! world-generation state, `wiring::worldgen::WorldState`, implements it;
//! `WorldSim` lends it around its timer events and tick hooks). With it:
//!
//! - a monster the allocator makes (`0x00555230`, `units.md` §3.1) gets
//!   its type init `0x00574250` (`init.md` §5) from the per-kind init
//!   hook, wherever the allocation comes from;
//! - a removed unit (`0x00555600`, §3.2) leaves the monster state too;
//! - the umod dispatcher `0x005A4270` (`init.md` §22) runs for monster
//!   event 7 (mode 2), the monster mode change `0x005A7C20` (modes 0 and
//!   1), the combat hook `0x005A4390` (mode 3, `damage.md` §5.2 step 9)
//!   and the missile hook `0x005A43B0` (mode 5, `missiles.md` rule 28);
//! - the monster-data queries (`0x005A0180` type flags, the monster
//!   level) read the monster data.
//!
//! Without a world (an [`super::ActionSim`] alone, or while the world is
//! lent to a call that holds it directly) every one of these keeps its
//! [`Pending`] answer, as before.

use std::any::Any;

use crate::monsters::init::MonsterData;
use crate::units::hooks::Sim;
use crate::units::UnitId;

use super::{ActionHooks, Pending, WiringError};

/// Umod dispatcher modes (`init.md` §22).
pub mod umod_mode {
    /// `0x005A4350`, monster mode change `0x005A7C20`.
    pub const MODE_CHANGE: u8 = 0;
    /// `0x005A4360`, monster mode change `0x005A7C20` (second site).
    pub const MODE_SET: u8 = 1;
    /// `0x005A4370`, monster timer event 7.
    pub const EVENT7: u8 = 2;
    /// `0x005A4390`, combat `0x0057C6C0`.
    pub const HIT: u8 = 3;
    /// `0x005A43A0`, the reaction `0x0057CEE0` (`umod-callbacks.md` §2
    /// rule 5). Its sites are inside the monster-defender branches of
    /// the reaction, which stay [`super::Pending::reaction`]'s
    /// (`damage.md` §7.1 OQ3): the provider of those branches runs it.
    pub const GET_HIT: u8 = 4;
    /// `0x005A43B0`, missile creation `0x0059FA30`.
    pub const MISSILE: u8 = 5;
}

/// The monster state of a game (monster data, umods, minion and owner
/// links, monster init), lent to the action hooks. The calls get the
/// action hooks with the world taken out of them, so the two never
/// alias.
pub trait MonsterWorld<X> {
    /// Monster type init `0x00574250` (`init.md` §5) of a monster the
    /// allocator just made (the allocator's last step, `units.md` §3.1).
    fn type_init(&mut self, sim: &mut Sim<'_>, h: &mut ActionHooks<X>, unit: UnitId);
    /// The umod dispatcher `0x005A4270(game, unit, arg, mode)`.
    fn umods(
        &mut self,
        sim: &mut Sim<'_>,
        h: &mut ActionHooks<X>,
        unit: UnitId,
        arg: Option<UnitId>,
        mode: u8,
    );
    /// The monster state's part of a unit free.
    fn forget(&mut self, unit: UnitId);
    /// The monster data (unit +0x14) of `unit`, if it has one.
    fn monster(&self, unit: UnitId) -> Option<&MonsterData>;
    /// The classes of the first `+0x10` entries of the level's monster
    /// region (`monsters/population.md` §2.2; the object trap monster id,
    /// `world/objects.md` §8.3). Default: no region.
    fn region_classes(&self, level: u32) -> Option<Vec<i32>> {
        let _ = level;
        None
    }
    /// The `monstats` row count. Default 0.
    fn monstats_count(&self) -> u32 {
        0
    }
    /// The concrete state back (the lender downcasts it).
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<X> ActionHooks<X> {
    /// Runs `f` on the lent monster world with the hooks; `None` when no
    /// world is lent. A call while the world is already out (a monster
    /// route inside a monster route) is logged as
    /// [`WiringError::Reentrant`] and not run.
    pub fn with_monster_world<R>(
        &mut self,
        f: impl FnOnce(&mut dyn MonsterWorld<X>, &mut Self) -> R,
    ) -> Option<R> {
        let Some(mut w) = self.monster_world.take() else {
            if self.monster_world_out {
                self.errors.push(WiringError::Reentrant("monster world"));
            }
            return None;
        };
        self.monster_world_out = true;
        let r = f(&mut *w, self);
        self.monster_world_out = false;
        self.monster_world = Some(w);
        Some(r)
    }

    /// Lends `w` to the hooks from inside a monster route (a umod
    /// callback's mode set, `umod-callbacks.md` §22.1): the world the
    /// route holds goes back in for the nested call. Returns the "out"
    /// state to hand to [`Self::take_relent_monster_world`].
    pub fn relend_monster_world(&mut self, w: Box<dyn MonsterWorld<X>>) -> bool {
        let out = self.monster_world_out;
        self.monster_world = Some(w);
        self.monster_world_out = false;
        out
    }

    /// Takes back the world lent by [`Self::relend_monster_world`] and
    /// restores the "out" state; `None` when the nested call did not
    /// return it.
    pub fn take_relent_monster_world<W: 'static>(&mut self, out: bool) -> Option<W> {
        self.monster_world_out = out;
        self.monster_world
            .take()
            .and_then(|w| w.into_any().downcast::<W>().ok())
            .map(|w| *w)
    }

    /// The monster data of `unit` in the lent world.
    pub fn monster_data(&self, unit: UnitId) -> Option<&MonsterData> {
        self.monster_world.as_ref()?.monster(unit)
    }

    /// Runs the umod dispatcher in `mode` on `unit` when a world is lent;
    /// false when none is (the caller then takes its pending default).
    pub fn run_umods(
        &mut self,
        sim: &mut Sim<'_>,
        unit: UnitId,
        arg: Option<UnitId>,
        mode: u8,
    ) -> bool {
        self.with_monster_world(|w, h| w.umods(sim, h, unit, arg, mode))
            .is_some()
    }
}

impl<X: Pending> ActionHooks<X> {
    /// `0x005A0180(unit, mask)`: monster data type flags (+0x16) & mask
    /// (`init.md` Outputs); a unit without monster data asks
    /// [`Pending::monster_flag`].
    pub fn monster_flag(&self, unit: UnitId, mask: u32) -> bool {
        match self.monster_data(unit) {
            Some(m) => u32::from(m.type_flags) & mask != 0,
            None => self.x.monster_flag(unit, mask),
        }
    }
}
