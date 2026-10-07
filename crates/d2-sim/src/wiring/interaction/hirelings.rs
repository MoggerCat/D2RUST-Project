// Spec: specs/world/hirelings.md §3, §5, §9; specs/world/npc.md §7.3, §7.4, §7.5 (wiring of the mercenary calls)
//! The hireling rules ([`crate::world::hirelings`]) on the desk's
//! providers: [`HirelingWorld`] on the unit records (GUID, type, class,
//! mode, flags, room), the unit lists (GUID lookup, the players), the
//! stat lists (stats, maxima, states) and the transport of the quests'
//! rest; the calls no written spec provides stay a seam
//! ([`HirelingRest`]). The game's hireling state and tables live in
//! [`super::InteractionState`] (`hirelings`, `hireling_tables`).
//!
//! The `NpcWorld` mercenary calls of `world::npc` (hire §7.3, resurrect
//! §7.4, quest mercenary §7.5) land here: `init_mercenary` →
//! [`life::init`], `revive_mercenary` → [`life::revive`], `pet(7, any)`
//! → the hireling pet list (§5 rule 4).

use super::{Desk, InteractionError, PlayerQuestsRef};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::{UnitId, UnitType};
use crate::wiring::economy::QuestRest;
use crate::world::hirelings::{life, HirelingState, HirelingTables, HirelingWorld, Slot, NEW_HIRE};
use crate::world::npc::MercInit;

use super::NpcRest;

/// State 7 (players skipped by the broadcast `0x005538D0`,
/// `unit-order.md` §2 rule 5).
const STATE_BROADCAST_SKIP: u32 = 7;

/// The hireling calls no written spec provides yet, each with its owner.
pub trait HirelingRest {
    /// `0x00624690` / `0x00553570`: a mode change (monster modes spec).
    fn set_mode(&mut self, unit: UnitId, mode: u8);
    /// `0x005543B0` stat part: `stat` := `value` in the stat list of
    /// `state`, creating the list (state spec).
    fn set_state_stat(&mut self, unit: UnitId, state: u16, stat: u16, value: i32);
    /// `skills` count and `reqlevel` (+0x174) (skill tables).
    fn skill_count(&self) -> u32;
    fn skill_reqlevel(&self, skill: u32) -> Option<i16>;
    /// `0x0056DEB0` (skills spec).
    fn set_skill_level(&mut self, unit: UnitId, skill: u32, level: i32);
    /// `0x0058F030` / `0x0058F0D0` (monsters/ai.md control block).
    fn set_owner(&mut self, merc: UnitId, guid: u32, unit_type: u8);
    fn owner(&self, merc: UnitId) -> Option<(u32, u8)>;
    /// `0x005B1900` (team lists).
    fn join_team(&mut self, merc: UnitId, player: UnitId);
    /// `0x005A4850(game, merc, 0x13, 0)` and the monster data bytes
    /// (AI spec).
    fn hireling_ai(&mut self, merc: UnitId);
    /// `0x0061A270` + `0x00555600` (rooms, unit removal).
    fn free_unit(&mut self, unit: UnitId);
    /// `0x0061A270` alone.
    fn queue_room_removal(&mut self, unit: UnitId);
    /// The death event of a unit (unit events).
    fn death_event(&mut self, unit: UnitId);
    /// `0x00574450` (`sim/pets.md` §7: kill or death mode request).
    fn dismiss(&mut self, unit: UnitId);
    /// `0x00574D90` → `0x00574CC0` (rooms, path).
    fn warp_to(&mut self, pet: UnitId, player: UnitId);
    /// `0x00553380` event 0x5B and unit event 12.
    fn level_events(&mut self, player: UnitId, merc: UnitId);
    /// `0x00577470` (item stats).
    fn reapply_item_stats(&mut self, merc: UnitId);
}

/// The hireling world of one call: the desk without its hireling state
/// (taken out for the call so the rules can borrow both).
pub struct HireView<'v, 'd, 'a, H, R> {
    pub desk: &'v mut Desk<'d, 'a, H, R>,
}

impl<H: LifecycleHooks, R: NpcRest + QuestRest + PlayerQuestsRef> HirelingWorld
    for HireView<'_, '_, '_, H, R>
{
    fn difficulty(&self) -> u8 {
        self.desk.econ.fields.difficulty
    }
    fn expansion(&self) -> bool {
        self.desk.econ.fields.expansion
    }
    /// The player list in order, without players in state 7.
    fn players(&self) -> Vec<UnitId> {
        let e = &self.desk.econ;
        e.game
            .lists
            .units_of_type(UnitType::Player)
            .into_iter()
            .filter(|&p| !e.stats.has_state(p, STATE_BROADCAST_SKIP))
            .collect()
    }
    fn send(&mut self, player: UnitId, bytes: &[u8]) {
        QuestRest::send(&mut *self.desk.rest, player, bytes);
    }
    fn guid(&self, unit: UnitId) -> u32 {
        self.desk.econ.units.get(unit).map_or(u32::MAX, |r| r.guid)
    }
    fn monster_by_guid(&self, guid: u32) -> Option<UnitId> {
        self.desk.econ.game.lists.find_unit(UnitType::Monster, guid)
    }
    fn unit_type(&self, unit: UnitId) -> u8 {
        self.desk
            .econ
            .game
            .lists
            .unit(unit)
            .map_or(0xFF, |e| e.ty as u8)
    }
    fn class(&self, unit: UnitId) -> u32 {
        self.desk.econ.units.get(unit).map_or(0, |r| r.class)
    }
    fn mode(&self, unit: UnitId) -> u32 {
        self.desk.econ.units.get(unit).map_or(0, |r| r.mode)
    }
    fn set_mode(&mut self, unit: UnitId, mode: u8) {
        HirelingRest::set_mode(&mut *self.desk.rest, unit, mode);
    }
    fn flags(&self, unit: UnitId) -> u32 {
        self.desk.econ.units.get(unit).map_or(0, |r| r.flags)
    }
    fn set_flags(&mut self, unit: UnitId, flags: u32) {
        if let Some(r) = self.desk.econ.units.get_mut(unit) {
            r.flags = flags;
        }
    }
    fn flags2(&self, unit: UnitId) -> u32 {
        self.desk.econ.units.get(unit).map_or(0, |r| r.flags2)
    }
    fn set_flags2(&mut self, unit: UnitId, flags: u32) {
        if let Some(r) = self.desk.econ.units.get_mut(unit) {
            r.flags2 = flags;
        }
    }
    fn in_room(&self, unit: UnitId) -> bool {
        self.desk
            .econ
            .game
            .lists
            .unit(unit)
            .is_some_and(|e| e.room().is_some())
    }
    fn stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.desk.econ.stats.unit_total(unit, stat, 0)
    }
    fn base_stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.desk.econ.stats.unit_base(unit, stat, 0)
    }
    fn set_base_stat(&mut self, unit: UnitId, stat: u16, value: i32) {
        let e = &mut *self.desk.econ;
        e.stats.unit_set(&mut *e.hooks, unit, stat, value, 0);
    }
    fn max_life(&self, unit: UnitId) -> i32 {
        self.desk.econ.stats.max_life(unit)
    }
    fn set_state_stat(&mut self, unit: UnitId, state: u16, stat: u16, value: i32) {
        self.desk.rest.set_state_stat(unit, state, stat, value);
    }
    /// The state's stat list freed (`stat-lists.md` §9).
    fn remove_state(&mut self, unit: UnitId, state: u16) {
        let e = &mut *self.desk.econ;
        e.stats
            .free_state_list(&mut *e.hooks, unit, u32::from(state));
    }
    fn skill_count(&self) -> u32 {
        self.desk.rest.skill_count()
    }
    fn skill_reqlevel(&self, skill: u32) -> Option<i16> {
        self.desk.rest.skill_reqlevel(skill)
    }
    fn set_skill_level(&mut self, unit: UnitId, skill: u32, level: i32) {
        self.desk.rest.set_skill_level(unit, skill, level);
    }
    fn set_owner(&mut self, merc: UnitId, guid: u32, unit_type: u8) {
        self.desk.rest.set_owner(merc, guid, unit_type);
    }
    fn owner(&self, merc: UnitId) -> Option<(u32, u8)> {
        self.desk.rest.owner(merc)
    }
    fn join_team(&mut self, merc: UnitId, player: UnitId) {
        self.desk.rest.join_team(merc, player);
    }
    fn hireling_ai(&mut self, merc: UnitId) {
        self.desk.rest.hireling_ai(merc);
    }
    fn free_unit(&mut self, unit: UnitId) {
        self.desk.rest.free_unit(unit);
    }
    fn queue_room_removal(&mut self, unit: UnitId) {
        self.desk.rest.queue_room_removal(unit);
    }
    fn death_event(&mut self, unit: UnitId) {
        self.desk.rest.death_event(unit);
    }
    fn dismiss(&mut self, unit: UnitId) {
        self.desk.rest.dismiss(unit);
    }
    fn warp_to(&mut self, pet: UnitId, player: UnitId) {
        self.desk.rest.warp_to(pet, player);
    }
    fn level_events(&mut self, player: UnitId, merc: UnitId) {
        self.desk.rest.level_events(player, merc);
    }
    fn reapply_item_stats(&mut self, merc: UnitId) {
        self.desk.rest.reapply_item_stats(merc);
    }
}

impl<'a, H: LifecycleHooks, R: NpcRest + QuestRest + PlayerQuestsRef> Desk<'_, 'a, H, R> {
    /// Runs `f` on the hireling world with the game's hireling state and
    /// tables; no tables → [`InteractionError::NoHirelingTables`], nothing
    /// runs.
    pub fn with_hirelings<T>(
        &mut self,
        f: impl FnOnce(&mut HireView<'_, '_, 'a, H, R>, &HirelingTables, &mut HirelingState) -> T,
    ) -> Option<T> {
        let Some(t) = self.state.hireling_tables.take() else {
            self.state.errors.push(InteractionError::NoHirelingTables);
            return None;
        };
        let mut st = std::mem::take(&mut self.state.hirelings);
        let out = f(&mut HireView { desk: self }, &t, &mut st);
        self.state.hirelings = st;
        self.state.hireling_tables = Some(t);
        Some(out)
    }

    /// `npc.md` §7.3 step 8 / §7.5 → `0x00573270` (`hirelings.md` §3.2)
    /// for a new hire (`saved_id` 0xFFFF). The offer of `init` is not
    /// passed: the init re-runs §2 itself (edge case 1).
    pub(super) fn hireling_init(&mut self, player: UnitId, merc: UnitId, init: &MercInit) {
        let slot = Slot {
            name: init.name,
            seed: init.seed,
        };
        let r = self
            .with_hirelings(|w, t, st| life::init(w, t, st, player, merc, NEW_HIRE, slot, false));
        if let Some(Err(e)) = r {
            self.state.errors.push(InteractionError::Hireling(e));
        }
    }

    /// `npc.md` §7.4 step 4 → `0x00579AA0` (`hirelings.md` §9 rules 3–9).
    pub(super) fn hireling_revive(&mut self, player: UnitId, merc: UnitId) {
        let r = self.with_hirelings(|w, t, st| life::revive(w, t, st, player, merc));
        if let Some(Err(e)) = r {
            self.state.errors.push(InteractionError::Hireling(e));
        }
    }

    /// `0x00574EC0(game, player, 7, any)` (`hirelings.md` §5 rule 4).
    pub(super) fn hireling_pet(&self, player: UnitId, any: bool) -> Option<UnitId> {
        let node = self.state.hirelings.first_node(player, any)?;
        self.econ.game.lists.find_unit(UnitType::Monster, node.guid)
    }
}
