// Spec: specs/monsters/init.md §5, §6, §10, §16, §18 (the InitHost seam); specs/sim/stat-lists.md §8; specs/monsters/ai.md §3.3; specs/monsters/population.md §2.5, §6.3 step 5
//! Init → units, stats, AI and population: [`InitHost`] on
//! [`WorldHost`]. Unit records and base stats (layer 0) are the action
//! systems' ([`crate::units::record::Units`], [`crate::stats::StatLists`]);
//! AI control and the first AI setup are `monsters::ai`'s
//! ([`crate::monsters::ai::install`]); the level id is the unit's room's
//! DRLG level; region appearance entries and the boss count are the
//! game's population regions (`population.md` §2.5, §6.3 step 5); minion
//! lists are [`super::WorldState::minions`].
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
    /// The level id of the unit's room.
    fn unit_room_level(&self, unit: UnitId) -> i32 {
        self.room_of(unit).map_or(0, |r| self.room_level_id(r))
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

    fn set_alignment(&mut self, unit: UnitId, alignment: u8) {
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

    /// State toggle (`stat-lists.md` §9.2).
    fn set_state(&mut self, unit: UnitId, state: u16) {
        self.v.set_state(unit, state, true);
    }
}
