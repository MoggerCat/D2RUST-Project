// Spec: specs/monsters/init.md §5, §6, §10, §16, §17.3, §18 (the InitHost seam); specs/monsters/umod-callbacks.md (the callback seams); specs/sim/stat-lists.md §8; specs/monsters/ai.md §3.3; specs/monsters/population.md §2.5, §6.3 step 5; specs/data/runtime-maps.md §2
//! Init → units, stats, AI and population: [`InitHost`] on
//! [`WorldHost`]. Unit records and base stats (layer 0) are the action
//! systems' ([`crate::units::record::Units`], [`crate::stats::StatLists`]);
//! AI control and the first AI setup are `monsters::ai`'s
//! ([`crate::monsters::ai::install`]); the level id is the unit's room's
//! DRLG level; region appearance entries and the boss count are the
//! game's population regions (`population.md` §2.5, §6.3 step 5); minion
//! lists are [`super::WorldState::minions`]; montype nesting is the
//! montype equivalence matrix ([`super::WorldTables::montype_equiv`],
//! `runtime-maps.md` §2).
//!
//! Creation seams (`place`, `allocate`, `boss_spawn`, `spawn_boss_minion`,
//! `spawn_with_guid`, `party_minions`, `register_spawn`) keep their
//! defaults: in this wiring every monster is created by population's
//! `0x005B2A00` (`population.md` §9) through [`super::population_init`],
//! and the init entry points that would create (`create`, `random_boss`,
//! the full `superunique_init`, the restore paths) are not called.
//! The items, skills, quests and states calls keep their defaults too
//! (their providers are other groups), as does `set_combat_mode`
//! (`0x00553570`: the mode it sets for a dead monster is not stated).
//! `has_inventory` and `has_item_at` (unit +0x60, `init.md` §6 step 13,
//! §12) keep theirs with `new_inventory`: no monster inventory exists in
//! this wiring, so "no inventory" is what the wired world holds.

use crate::game::Game;
use crate::monsters::ai::{self, AiControl};
use crate::monsters::init::{self, InitHost, MonsterStore};
use crate::monsters::population::OwnerKey;
use crate::units::hooks::Sim;
use crate::units::modes;
use crate::units::record::Units;
use crate::units::UnitId;

use super::{View, WiringError, WorldHost, WorldPending, WorldgenError};

impl<X: WorldPending> WorldHost<'_, X> {
    /// The level id of the unit's room: during the type init the
    /// allocation's room (`units.md` §3.1 r7.2), the unit being in no
    /// room until step 8.
    fn unit_room_level(&self, unit: UnitId) -> i32 {
        self.v
            .init_room(self.game, unit)
            .map_or(0, |r| self.room_level_id(r))
    }

    /// A host-placed monster's creation-time AI (`init.md` §5 step 5,
    /// `ai.md` §3.3): control block, install state 0, first think next
    /// frame. PROVISIONAL (REC-254); d2rs-own, unverified.
    pub(super) fn start_host_ai(&mut self, unit: UnitId) {
        InitHost::alloc_ai(self, unit);
        InitHost::ai_install(self, unit, 0);
        let at = self.game.frame + 1;
        let _ = self.game.schedule_event(
            unit,
            u32::from(crate::tick::events::event::AI_THINK),
            at,
            None,
            0,
            0,
        );
    }

    /// `0x00547BC0` (`population.md` §2.5) for the unit's class in its
    /// room's region: the entry, added on demand on the unit seed.
    fn region_entry(&mut self, unit: UnitId, class: u32) -> Option<(i32, usize)> {
        let level = self.unit_room_level(unit);
        let t = self.w.tables.clone();
        self.regions("region entry")?;
        let seed = &mut self.v.units.get_mut(unit)?.seed;
        let i = self
            .w
            .pop
            .regions
            .entry_for(&t.pop, level, class as i32, seed)?;
        Some((level, i))
    }
}

impl<X: WorldPending> InitHost for WorldHost<'_, X> {
    fn game(&mut self) -> &mut Game {
        self.game
    }

    fn units(&mut self) -> &mut Units {
        self.v.units
    }

    fn monsters(&mut self) -> &mut MonsterStore {
        &mut self.w.monsters
    }

    fn info(&self) -> init::GameInfo {
        self.w.init_info
    }

    /// `0x00573930`'s write of difficulty 2 (game +0x6D, `init.md` §9).
    fn set_difficulty(&mut self, d: u8) {
        self.w.init_info.difficulty = d;
        self.w.pop_info.difficulty = d;
        self.v.h.ai_info.difficulty = d;
    }

    /// `StatLists::unit_base`, layer 0.
    fn stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.v.stats.unit_base(unit, stat, 0)
    }

    /// `StatLists::unit_set`, layer 0 (the unit's value callback).
    fn set_stat(&mut self, unit: UnitId, stat: u16, value: i32) {
        self.v.set_base(unit, stat, value);
    }

    /// `init.md` §5 step 2: the AI control block (`AiStore`).
    fn alloc_ai(&mut self, unit: UnitId) {
        match self.v.h.ai.as_mut() {
            Some(ai) => ai.entry(unit).control = Some(AiControl::default()),
            None => self.w.errors.push(WorldgenError::AiLent),
        }
    }

    /// `0x005B0E00` (`ai.md` §3.3) through the AI module's context.
    fn ai_install(&mut self, unit: UnitId, state: u32) {
        let Some(mut store) = self.v.h.ai.take() else {
            self.w.errors.push(WorldgenError::AiLent);
            return;
        };
        let t = self.v.h.tables.clone();
        let info = self.v.h.ai_info;
        {
            let mut v = View::of(
                &mut *self.v.units,
                &mut *self.v.stats,
                self.v.data,
                &mut *self.v.h,
            );
            let mut cx = ai::Ctx {
                tables: ai::AiTables {
                    monstats: &t.combat.monstats,
                    monstats2: &t.combat.monstats2,
                    levels: &t.levels,
                    skill_modes: &t.skill_modes,
                    skills: &t.skills.skills,
                    missiles: &t.skills.missiles,
                },
                info,
                store: &mut store,
                world: &mut v,
            };
            ai::install(self.game, &mut cx, unit, state);
        }
        self.v.h.ai = Some(store);
    }

    /// The level id of the unit's room (`init.md` §5 step 6).
    fn level_id(&mut self, unit: UnitId) -> i32 {
        self.unit_room_level(unit)
    }

    /// The variant count (+0x03) of the class's region entry.
    fn region_variant_count(&mut self, unit: UnitId, class: u32) -> u8 {
        let Some((level, i)) = self.region_entry(unit, class) else {
            return 0;
        };
        self.w
            .pop
            .regions
            .get(level)
            .and_then(|r| r.entries.get(i))
            .map_or(0, |e| e.variant_count)
    }

    /// The 16 component bytes of variant `i` (region data +0x04 + 16·i).
    fn region_variant(&mut self, unit: UnitId, class: u32, i: u32) -> [u8; 16] {
        let Some((level, e)) = self.region_entry(unit, class) else {
            return [0; 16];
        };
        self.w
            .pop
            .regions
            .get(level)
            .and_then(|r| r.entries.get(e))
            .and_then(|e| e.variants.get(i as usize))
            .copied()
            .unwrap_or([0; 16])
    }

    /// `0x005543B0`: the unit's state-105 list
    /// ([`crate::wiring::action::View::set_alignment`]),
    /// then the host's copy.
    fn set_alignment(&mut self, unit: UnitId, alignment: u8) {
        self.v.set_alignment(self.game, unit, alignment);
        self.v.h.x.set_alignment(unit, alignment);
    }

    /// `0x005A0320`: bosses spawned (+0x2C8) of the unit's region += 1
    /// (`population.md` §6.3 step 5).
    fn count_region_boss(&mut self, unit: UnitId) {
        let level = self.w.monsters.get(unit).map_or(0, |m| m.level_id);
        if let Some(r) = self
            .regions("boss count")
            .and_then(|regions| regions.get_mut(level))
        {
            r.bosses = r.bosses.wrapping_add(1);
        }
    }

    fn boss_quest_hook(&mut self, unit: UnitId) {
        self.v.h.x.boss_quest_hook(unit);
    }

    fn quest_chain(&mut self, unit: UnitId, chain: u32) {
        self.v.h.x.monster_quest_chain(unit, chain);
    }

    /// `0x00545CD0` (`quests.md` §4.6): a link for the chain in the level
    /// row's `Quest` (1.14d: Den of Evil 1, Arcane Sanctuary 11).
    fn attach_quest_chain(&mut self, unit: UnitId) {
        let level = self.unit_room_level(unit);
        let quest = self.w.tables.pop.level(level).map_or(0, |l| l.quest);
        if quest != 0 {
            self.v.h.x.monster_quest_chain(unit, u32::from(quest));
        }
    }

    fn quest_preset_boss(&mut self, unit: UnitId) {
        self.v.h.x.quest_preset_boss(unit);
    }

    /// `0x0058F030(game, boss, boss GUID, 1, 1, 0)`.
    fn boss_owner_data(&mut self, unit: UnitId) {
        self.v
            .h
            .x
            .set_owner_data(unit, OwnerKey::Guid(unit), 1, 1, 0);
    }

    /// `0x0058F100` and `0x005DD330` (owner data `0x0058F030` is
    /// pending: `ai.md`).
    fn link_minion(&mut self, boss: UnitId, minion: UnitId) {
        self.v.h.x.unique_minion_owner_data(boss, minion);
        self.w.minions.entry(boss).or_default().push(minion);
        self.w.owners.insert(minion, boss);
    }

    /// `0x005A0070` (`init.md` §17.3 rule 2, `runtime-maps.md` §2): type
    /// `montype` is `ty` or nested in it, bit (row `montype`, column `ty`)
    /// of the montype matrix itself (out-of-range rows and columns are 0).
    /// The eligibility test asks it with row = the exclude type and
    /// column = the class's MonType.
    fn montype_is(&mut self, montype: u16, ty: u16) -> bool {
        self.w
            .tables
            .montype_equiv
            .get(usize::from(montype), usize::from(ty))
    }

    /// `0x0058F380`.
    fn minions(&mut self, boss: UnitId) -> Vec<UnitId> {
        self.w.minions.get(&boss).cloned().unwrap_or_default()
    }

    /// `0x005533D0` (`units.md` §4.1): prepare animation, after the
    /// type init (`init.md` §5 step 7).
    fn after_type_init(&mut self, unit: UnitId) {
        let mut sim = Sim {
            game: &mut *self.game,
            units: &mut *self.v.units,
            stats: &mut *self.v.stats,
            data: self.v.data,
        };
        if let Err(e) = modes::prepare_animation(&mut sim, &mut *self.v.h, unit) {
            self.w
                .errors
                .push(WorldgenError::Wiring(WiringError::Unit(e)));
        }
    }

    /// `init.md` §6 step 12: the monster base list, the only list with
    /// flag 1 (`stat-lists.md` §2, `0x0057407D`), allocated
    /// (`0x006251F0`) and attached to the unit (`0x00626E10`). The mode
    /// damage `0x005A4F50` (`skills/bodies-2.md` §2.1) and the umod
    /// callbacks write into it.
    ///
    /// PROVISIONAL (init.md §6 step 12, REC-891): the call's owner and
    /// attach `reset` are not stated; owner = the monster, reset = 1 (a
    /// DYNAMIC list would keep `mindamage` / `maxdamage` / `tohit` out of
    /// the unit's totals, and 1.14d monsters hit with them).
    fn post_extra_list(&mut self, unit: UnitId) {
        let Some((ty, guid)) = self.v.units.get(unit).map(|r| (r.ty, r.guid)) else {
            return;
        };
        let l = self.v.stats.alloc(1, 0, ty.index() as u32, guid);
        self.v.stats.attach(&mut *self.v.h, unit, l, true);
    }

    /// State toggle (`stat-lists.md` §9.2).
    fn set_state(&mut self, unit: UnitId, state: u16) {
        self.v.set_state(unit, state, true);
    }

    /// §14.3 bloodraven: state 118 corpse_noselect on
    /// (`0x00639DB0(unit, 118, 1)`; recorded in the 0xAA of
    /// `items-drops-hel-10`).
    fn set_corpse_noselect(&mut self, unit: UnitId) {
        self.v.set_state(unit, 118, true);
    }

    // ---- umod callbacks (`umod-callbacks.md`; bodies in `umod_host.rs`)

    fn stat_total(&self, unit: UnitId, stat: u16) -> i32 {
        self.v.stat(unit, stat)
    }
    fn set_base_list_stat(&mut self, unit: UnitId, stat: u16, value: i32) {
        self.umod_set_base_list(unit, stat, value);
    }
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.v.h.path_position(unit)
    }
    fn has_state(&self, unit: UnitId, state: u16) -> bool {
        self.v.stats.has_state(unit, u32::from(state))
    }
    fn has_state_in_group(&self, unit: UnitId, group: u8) -> bool {
        self.v.stats.has_group(unit, usize::from(group))
    }
    fn set_mode(&mut self, unit: UnitId, mode: u32) {
        self.umod_set_mode(unit, mode);
    }
    fn owner(&self, unit: UnitId) -> Option<UnitId> {
        self.umod_owner(unit)
    }
    /// The wired world's minion owner link ([`super::WorldState::owners`]).
    fn minion_owner(&mut self, unit: UnitId) -> Option<UnitId> {
        self.w.owners.get(&unit).copied()
    }
    fn alignment(&self, unit: UnitId) -> u8 {
        self.v.h.x.alignment(unit)
    }
    fn hostile(&self, a: UnitId, b: UnitId) -> bool {
        self.v.h.x.may_attack(a, b)
    }
    fn target(&self, unit: UnitId) -> Option<UnitId> {
        self.v.h.x.umod_target(unit)
    }
    fn target_position(&self, unit: UnitId) -> Option<(i32, i32)> {
        self.v.h.x.umod_target_position(unit)
    }
    fn path_target_point(&self, missile: UnitId) -> (i32, i32) {
        self.v.h.x.path_target_point(missile)
    }
    fn create_missile(
        &mut self,
        req: crate::skills::use_::bodies::MissileRequest<UnitId>,
    ) -> Option<UnitId> {
        self.umod_create_missile(req)
    }
    fn missile_row_flags(&self, class: i32) -> Option<u32> {
        self.umod_missile_flags(class)
    }
    fn missile_velocity(&self, class: i32, level: i32) -> i32 {
        self.umod_missile_velocity(class, level)
    }
    fn missile_skill_level(&self, missile: UnitId) -> (i32, i32) {
        self.umod_missile_skill_level(missile)
    }
    fn find_units(&mut self, near: UnitId, q: init::find::FindQuery) -> Vec<UnitId> {
        self.umod_find_units(near, q)
    }
    fn line_clear(&mut self, from: (i32, i32), unit: UnitId) -> bool {
        self.umod_line_clear(from, unit)
    }
    fn missile_hit(&mut self, src: UnitId, unit: UnitId, rec: &crate::combat::DamageRecord) {
        self.umod_missile_hit(src, unit, rec);
    }
    fn skill_calc(
        &mut self,
        unit: UnitId,
        skill: u16,
        calc: init::callbacks::SkillCalc,
        level: i32,
    ) -> i32 {
        self.umod_skill_calc(unit, skill, calc, level)
    }
    fn aura_fields(&self, skill: u16) -> Option<init::callbacks::AuraFields> {
        self.umod_aura_fields(skill)
    }
    fn stat_and_state_counts(&self) -> (i32, i32) {
        self.umod_counts()
    }
    fn apply_state(&mut self, req: init::callbacks::StateApply) -> Option<crate::stats::ListId> {
        self.v.h.x.umod_apply_state(req)
    }
    fn set_list_stat(&mut self, list: crate::stats::ListId, stat: u16, value: i32) {
        self.v.set_list_stat(list, stat, value);
    }
    /// `0x0058F160`: the unit's minion list in the wired world.
    fn free_minions(&mut self, unit: UnitId) {
        self.w.minions.remove(&unit);
    }
    fn clear_owner_data(&mut self, unit: UnitId) {
        self.umod_clear_owner_data(unit);
    }
    fn remove_pet(&mut self, owner: UnitId, pet: UnitId) {
        self.v.h.x.remove_pet(owner, pet);
    }
    fn is_undead(&self, unit: UnitId) -> bool {
        self.v.h.is_undead(unit)
    }
    fn missile_range(&self, class: i32) -> Option<i32> {
        self.v.h.x.umod_missile_range(class)
    }
    fn set_uber_death(&mut self, slot: usize) -> bool {
        self.v.h.x.umod_set_uber_death(slot)
    }
    fn game_8c(&self) -> i32 {
        self.v.h.x.umod_game_8c()
    }
    /// §15.2: the drop helper `0x00559A30` with the item code `code`
    /// and quality `arg` (`items/treasure.md` §9,
    /// [`crate::wiring::economy::unit_quest_drop`]) when the game holds
    /// the drop state; with `announce`, `0x0055FE80(game, 0, item)` on the
    /// item ([`WorldPending::umod_recharge`]). Without the drop state:
    /// [`WorldPending::umod_quest_drop`].
    fn quest_drop(&mut self, unit: UnitId, code: [u8; 4], arg: i32, announce: bool) {
        let Some(mut d) = self.v.h.object_drops.take() else {
            self.v.h.x.umod_quest_drop(unit, code, arg, announce);
            return;
        };
        let item = {
            let mut sim = Sim {
                game: &mut *self.game,
                units: &mut *self.v.units,
                stats: &mut *self.v.stats,
                data: self.v.data,
            };
            crate::wiring::economy::unit_quest_drop(
                &mut *self.v.h,
                &mut sim,
                &mut d,
                &mut crate::wiring::economy::NoSpot,
                unit,
                Some(code),
                arg as u8,
                -1,
                0,
            )
        };
        self.v.h.object_drops = Some(d);
        if let (true, Some(item)) = (announce, item) {
            self.v.h.x.umod_recharge(item);
        }
    }
    /// `init.md` §6 step 13: the monster's inventory (`0x0063ABD0`) is a
    /// holdings entry ([`ActionHooks::monster_equip`], PROVISIONAL,
    /// REC-1030); the NPC store inventory is the vendors'.
    ///
    /// [`ActionHooks::monster_equip`]: crate::wiring::action::ActionHooks
    fn new_inventory(&mut self, unit: UnitId, npc_store: bool) {
        if !npc_store {
            self.v.h.monster_equip.entry(unit).or_default();
        }
    }
    /// Unit +0x60 is set.
    fn has_inventory(&mut self, unit: UnitId) -> bool {
        self.v.h.monster_equip.contains_key(&unit)
    }
    /// `init.md` §12: the monster's inventory holds an item at `loc`
    /// (PROVISIONAL, REC-1030: [`ActionHooks::monster_equip`]).
    ///
    /// [`ActionHooks::monster_equip`]: crate::wiring::action::ActionHooks
    fn has_item_at(&mut self, unit: UnitId, loc: u8) -> bool {
        let sim = Sim {
            game: &mut *self.game,
            units: &mut *self.v.units,
            stats: &mut *self.v.stats,
            data: self.v.data,
        };
        crate::wiring::economy::monster_item_at(&*self.v.h, &sim, unit, loc).is_some()
    }
    /// `0x00573B20` (`init.md` §12) when the game holds the drop state
    /// (the item tables); without it no item is made.
    fn create_equip_item(
        &mut self,
        unit: UnitId,
        code: [u8; 4],
        loc: u8,
        modifier: u8,
        level: i32,
    ) {
        let Some(mut d) = self.v.h.object_drops.take() else {
            return;
        };
        {
            let mut sim = Sim {
                game: &mut *self.game,
                units: &mut *self.v.units,
                stats: &mut *self.v.stats,
                data: self.v.data,
            };
            crate::wiring::economy::create_monster_equip(
                &mut *self.v.h,
                &mut sim,
                &mut d,
                unit,
                code,
                loc,
                modifier,
                level,
            );
        }
        self.v.h.object_drops = Some(d);
    }
    /// `init.md` §11: the monprop property assignment `0x0065FD70` on the
    /// unit ([`crate::wiring::economy::monster_property`]); without the
    /// drop state (no item tables) nothing is written.
    fn apply_property(&mut self, unit: UnitId, prop: i32, par: i32, min: i32, max: i32) {
        let Some(d) = self.v.h.object_drops.take() else {
            return;
        };
        {
            let mut sim = Sim {
                game: &mut *self.game,
                units: &mut *self.v.units,
                stats: &mut *self.v.stats,
                data: self.v.data,
            };
            crate::wiring::economy::monster_property(
                &mut *self.v.h,
                &mut sim,
                &d,
                unit,
                prop,
                par,
                min,
                max,
            );
        }
        self.v.h.object_drops = Some(d);
    }
    /// `init.md` §14.3 ([`crate::wiring::economy::tier_code`]); without
    /// the drop state the code stays.
    fn item_tier_code(&mut self, code: [u8; 4], difficulty: u8) -> [u8; 4] {
        match self.v.h.object_drops.as_ref() {
            Some(d) => {
                crate::wiring::economy::tier_code(&d.tables.treasure_items, code, difficulty)
            }
            None => code,
        }
    }
    /// `init.md` §14.3: `0x00573B20(game, unit, &entry, level, 4)`, the
    /// monequip helper with quality 4 (magic).
    fn create_boss_item(&mut self, unit: UnitId, code: [u8; 4], loc: u8, level: i32) {
        self.create_equip_item(unit, code, loc, 4, level);
    }
    fn steal_belt_item(&mut self, unit: UnitId, target: UnitId) {
        self.v.h.x.steal_belt_item(unit, target);
    }
    fn spawn_near(&mut self, unit: UnitId, class: u32, mode: u32, spread: i32, flags: u32) {
        self.v.h.x.spawn_near(unit, class, mode, spread, flags);
    }
    fn footprint_occupied(&mut self, unit: UnitId) -> bool {
        self.umod_footprint_occupied(unit)
    }
    /// `0x005DD250(unit, 1)`: AI control flags |= `flag` (`umod-init-bodies.md`
    /// §4 r4); a unit without an AI control writes nothing.
    /// §19.5: give and assign the aura skill (`0x0056DEB0`, `0x005701B0`).
    fn give_aura(&mut self, unit: UnitId, skill: u16, level: i32) {
        let mut sim = Sim {
            game: &mut *self.game,
            units: &mut *self.v.units,
            stats: &mut *self.v.stats,
            data: self.v.data,
        };
        X::monster_right_aura(&mut *self.v.h, &mut sim, unit, i32::from(skill), level);
    }
    fn set_ai_flag(&mut self, unit: UnitId, flag: u16) {
        if let Some(c) = self.v.h.ai.as_mut().and_then(|ai| ai.control_mut(unit)) {
            c.flags |= flag;
        }
    }
    fn ai_param0(&mut self, unit: UnitId) -> i32 {
        self.v.h.x.ai_param0(unit)
    }
    fn set_ai_param0(&mut self, unit: UnitId, v: i32) {
        self.v.h.x.set_ai_param0(unit, v);
    }
    fn can_raise(&mut self, unit: UnitId) -> bool {
        self.v.h.x.can_raise(unit)
    }
    fn ai_use_skill(&mut self, unit: UnitId, mode: u32, skill: u16) {
        self.v.h.x.ai_use_skill(unit, mode, skill);
    }
    fn give_skill(&mut self, unit: UnitId, skill: u16, level: i32, _mode: Option<u8>) {
        self.v
            .h
            .natural_skills
            .entry(unit)
            .or_default()
            .insert(i32::from(skill), level);
    }
    fn skill_level(&mut self, unit: UnitId, skill: u16) -> Option<i32> {
        self.v.h.x.skill_level(unit, skill)
    }
    /// `0x00621F20` as the AI reads it ([`crate::monsters::ai::AiUnits`]).
    fn life_percent(&self, unit: UnitId) -> i32 {
        crate::monsters::ai::AiUnits::life_percent(&self.v, unit)
    }
    fn room_area_level(&mut self, unit: UnitId) -> Option<i32> {
        self.umod_room_area_level(unit)
    }
}
