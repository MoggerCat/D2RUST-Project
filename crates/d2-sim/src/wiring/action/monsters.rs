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
use crate::units::{RoomId, UnitId};

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
    /// `0x005A4850(game, unit, umod, 0)`: append `umod` to the unit's
    /// list and run its init (`init.md`, `assign_umod`).
    fn assign_umod(&mut self, sim: &mut Sim<'_>, h: &mut ActionHooks<X>, unit: UnitId, umod: u8);
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
    /// `0x00547E50` from the death start `0x005A6FF0` (`population.md`
    /// §13 item 3): the dying monster's region kill count; `alignment`
    /// is its alignment. Default: nothing.
    fn count_death(&mut self, unit: UnitId, alignment: u8) {
        let _ = (unit, alignment);
    }
    /// Level 8's region for the Den of Evil quest (`quests-act1.md`
    /// §10.4 event 8): (evil spawned, evil killed, rooms visited).
    /// Default: no region.
    fn den_counts(&self) -> Option<(u32, u32, u32)> {
        None
    }
    /// Class reinit `0x00574370(game, unit, class, mode)` (`init.md`
    /// §27). Default: nothing (false).
    fn reinit(
        &mut self,
        sim: &mut Sim<'_>,
        h: &mut ActionHooks<X>,
        unit: UnitId,
        class: i32,
        mode: u32,
    ) -> bool {
        let _ = (sim, h, unit, class, mode);
        false
    }
    /// The `monstats` row count. Default 0.
    fn monstats_count(&self) -> u32 {
        0
    }
    /// The monster data of `unit`, mutable. Default: none.
    fn monster_mut(&mut self, _unit: UnitId) -> Option<&mut MonsterData> {
        None
    }
    /// The choice counts of the class's 16 components (`0x006647C0`,
    /// monstats2 of `MonStatsEx`). Default: none.
    fn component_counts(&self, _class: u32) -> Option<[u8; 16]> {
        None
    }
    /// The creation `0x005B2F20(room, x, y, class, mode, spread, flags)`
    /// (`monsters/population.md` §9: placement on the room seed, the
    /// allocation with its type init, alignment, normal and boss mods,
    /// party) on the lent world. `None`: the world cannot run it (the
    /// caller allocates plainly); `Some(None)`: nothing placed.
    #[allow(clippy::too_many_arguments)]
    fn spawn_at(
        &mut self,
        sim: &mut Sim<'_>,
        h: &mut ActionHooks<X>,
        room: RoomId,
        x: i32,
        y: i32,
        class: i32,
        mode: u8,
        spread: i32,
        flags: u16,
    ) -> Option<Option<UnitId>> {
        let _ = (sim, h, room, x, y, class, mode, spread, flags);
        None
    }
    /// The creation `0x005B23C0(unit, class, mode, spread, flags)` near a
    /// unit (`monsters/population.md` §9 around the unit's position in its
    /// room) on the lent world. `None`: the world cannot run it;
    /// `Some(None)`: nothing placed.
    #[allow(clippy::too_many_arguments)]
    fn spawn_near(
        &mut self,
        sim: &mut Sim<'_>,
        h: &mut ActionHooks<X>,
        unit: UnitId,
        class: i32,
        mode: u8,
        spread: i32,
        flags: u16,
    ) -> Option<Option<UnitId>> {
        let _ = (sim, h, unit, class, mode, spread, flags);
        None
    }
    /// The preset spawn `0x0054E600(room, class, x, y, mode)` on the lent
    /// world (`monsters/population.md` §11.2: a class past the monstats
    /// rows is superunique `class - rows`, §11.4, with its init, minions
    /// and quest links). `None`: the world cannot run it;
    /// `Some(None)`: nothing made.
    #[allow(clippy::too_many_arguments)]
    fn spawn_preset(
        &mut self,
        sim: &mut Sim<'_>,
        h: &mut ActionHooks<X>,
        room: RoomId,
        x: i32,
        y: i32,
        class: i32,
        mode: u8,
    ) -> Option<Option<UnitId>> {
        let _ = (sim, h, room, x, y, class, mode);
        None
    }
    /// The random boss `0x005A43E0(room, 0, class, champion allowed, 0,
    /// 0, warp check)` (`monsters/population.md` §6.2) on the lent
    /// world: the boss with its modifiers and minions. `None`: the world
    /// cannot run it; `Some(None)`: nothing made.
    fn spawn_random_boss(
        &mut self,
        sim: &mut Sim<'_>,
        h: &mut ActionHooks<X>,
        room: RoomId,
        class: i32,
    ) -> Option<Option<UnitId>> {
        let _ = (sim, h, room, class);
        None
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

    /// Runs `f` while a monster route holds the world and runs a call
    /// that is itself a world call with the world in hand (population's
    /// creation inside [`MonsterWorld::spawn_at`]): its allocations take
    /// their type init from that call, as population's own do, so the
    /// hooks read as "no world lent" rather than "out".
    pub fn as_world_holder<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let out = std::mem::replace(&mut self.monster_world_out, false);
        let r = f(self);
        self.monster_world_out = out;
        r
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

    /// The monster data of `unit` in the lent world, mutable.
    pub fn monster_data_mut(&mut self, unit: UnitId) -> Option<&mut MonsterData> {
        self.monster_world.as_mut()?.monster_mut(unit)
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
    /// `0x005734C0(unit, v)`: the monster data's `dwAiState` (+0x54,
    /// `monsters/ai.md` §3 "AI state"); a unit without monster data in
    /// the lent world asks [`Pending::set_monster_ai_state`].
    pub fn set_monster_ai_state(&mut self, unit: UnitId, v: u32) {
        match self.monster_data_mut(unit) {
            Some(m) => m.ai_state = v,
            None => self.x.set_monster_ai_state(unit, v),
        }
    }

    /// `0x005A68E0(unit, m)`, the AI-state half of the monster mode set
    /// `0x005A7C20` (`ai.md` §3 "AI state" rule 2): `m` is the mode being
    /// left; nothing for mode 1; old state ≥ 16 → state − 16; state 13
    /// leaving mode 3 stays; else the state is `m`.
    pub fn leave_monster_mode(&mut self, unit: UnitId, m: u32) {
        if m == 1 {
            return;
        }
        let s = self.ai_state_of(unit);
        let new = if s >= 16 {
            s - 16
        } else if s == 13 && m == 3 {
            13
        } else {
            m
        };
        self.set_monster_ai_state(unit, new);
    }

    fn ai_state_of(&self, unit: UnitId) -> u32 {
        match self.monster_data(unit) {
            Some(d) => d.ai_state,
            None => self.x.ai_state(unit),
        }
    }

    /// `0x005A0180(unit, mask)`: monster data type flags (+0x16) & mask
    /// (`init.md` Outputs); a unit without monster data asks
    /// [`Pending::monster_flag`].
    pub fn monster_flag(&self, unit: UnitId, mask: u32) -> bool {
        match self.monster_data(unit) {
            Some(m) => u32::from(m.type_flags) & mask != 0,
            None => self.x.monster_flag(unit, mask),
        }
    }

    /// `0x005A03A0(unit)`: a monster whose data has type flag 0x02
    /// (superunique) gives its hcIdx (+0x26), any other unit none
    /// (`treasure.md` §3.2); a unit without monster data asks
    /// [`Pending::superunique`].
    pub fn superunique(&self, unit: UnitId) -> Option<u16> {
        use crate::monsters::init::type_flag;
        match self.monster_data(unit) {
            Some(m) if m.type_flags & type_flag::SUPERUNIQUE != 0 => Some(m.boss_hc_idx),
            Some(_) => None,
            None => self.x.superunique(unit),
        }
    }

    /// The monstats row of a monster with monster data in the lent world.
    fn monstats_of(&self, unit: UnitId) -> Option<&d2_data::tables::Monstats> {
        let class = self.monster_data(unit)?.class;
        self.tables
            .combat
            .monstats
            .get(usize::try_from(class).ok()?)
    }

    /// `0x0063E9F0`: the unit is a monster whose monstats row has the
    /// `boss` flag (`ai.md` §2.4 step 1). A unit without monster data asks
    /// [`Pending::is_boss`].
    pub fn is_boss(&self, unit: UnitId) -> bool {
        match self.monstats_of(unit) {
            Some(m) => m.boss,
            None => self.x.is_boss(unit),
        }
    }

    /// `0x0063E940`: monstats `demon`.
    pub fn is_demon(&self, unit: UnitId) -> bool {
        match self.monstats_of(unit) {
            Some(m) => m.demon,
            None => self.x.is_demon(unit),
        }
    }

    /// `0x0063E990`: monstats `lUndead` or `hUndead`.
    pub fn is_undead(&self, unit: UnitId) -> bool {
        match self.monstats_of(unit) {
            Some(m) => m.lundead || m.hundead,
            None => self.x.is_undead(unit),
        }
    }

    /// `0x0063EDC0`: monstats `primeevil`.
    pub fn is_prime_evil(&self, unit: UnitId) -> bool {
        match self.monstats_of(unit) {
            Some(m) => m.primeevil,
            None => self.x.is_prime_evil(unit),
        }
    }
}
