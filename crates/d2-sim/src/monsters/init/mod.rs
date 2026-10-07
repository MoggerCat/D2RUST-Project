// Spec: specs/monsters/init.md
//! Monster creation and initialization: the creation sequence after
//! placement (`0x005B2A00`, §4), the monster type init (`0x00574250`,
//! §5) with stats, skills, components, monprop and monequip (§6–§13),
//! normal and boss mods (§14), boss spawns, umod choice and umod init
//! functions (§16–§20), restore paths (§21), the umod callback
//! dispatcher and the type-7 event (§22) with the callback bodies
//! (`monsters/umod-callbacks.md`, [`callbacks`], [`find`]), and the
//! init-owned pieces of
//! the client name draw and the 0xAC monster assign message (§23–§24).
//!
//! Every call into a system another spec owns goes through
//! [`InitHost`] (`seams.rs`); the monster data block (unit +0x14) is
//! [`MonsterData`] in a [`MonsterStore`] the host owns. All randomness is
//! the unit seed in [`crate::units::UnitRecord::seed`] (reached through
//! [`InitHost::units`]), in the order of the spec's Randomness section.

mod calc;
pub mod callbacks;
mod create;
pub mod find;
mod message;
mod seams;
mod umods;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use d2_data::bin::BinTable;
use d2_data::tables::{
    Difficultylevels, Levels, Monequip, Monlvl, Monprop, Monstats, Monstats2, Monumod, Superuniques,
};

use crate::units::UnitId;

pub use calc::{
    a1_damage, area_level, classic_scaling, hp_regen, monlvl_dm, monster_level, pct, player_bonus,
    stats_by_level, LevelStats, PlayerBonus,
};
pub use create::{
    assign_umod, boss_mods, components, create, monequip, monprop, normal_mods, normal_mods_for,
    stats_and_skills, type_init,
};
pub use message::{
    assign_mode, component_bits, components_field, unique_name, write_boss_section, BitWriter,
    UniqueName, LIFE_AT_SPAWN,
};
pub use seams::InitHost;
pub use umods::{
    aura_choice, boss_minions_and_init, callback, champion_pack_member, choose_umods, dispatch,
    eligible, handle_event7, mark_boss, mark_unique, pick_champion, pick_unique, random_boss,
    restore_boss, restore_minion, run_umod_init, superunique_finish, superunique_init,
    superunique_mods, xfer_umods, Gate, Saved, UmodRow, AURAS, UMODS, UMODS_TSV,
};

/// Stat ids (`itemstatcost.txt` rows) init reads or writes.
pub mod stat {
    pub use crate::stats::stat::{HITPOINTS, HPREGEN, LAST_SENT_HP_PCT, LEVEL, MAXHP};
    pub const EXPERIENCE: u16 = 13;
    pub const ITEM_ARMOR_PERCENT: u16 = 16;
    pub const TOBLOCK: u16 = 20;
    pub const DAMAGEPERCENT: u16 = 25;
    pub const ARMORCLASS: u16 = 31;
    pub const DAMAGERESIST: u16 = 36;
    pub const MAGICRESIST: u16 = 37;
    pub const FIRERESIST: u16 = 39;
    pub const LIGHTRESIST: u16 = 41;
    pub const COLDRESIST: u16 = 43;
    pub const POISONRESIST: u16 = 45;
    pub const FIREMINDAM: u16 = 48;
    pub const FIREMAXDAM: u16 = 49;
    pub const LIGHTMINDAM: u16 = 50;
    pub const LIGHTMAXDAM: u16 = 51;
    pub const COLDMINDAM: u16 = 54;
    pub const COLDMAXDAM: u16 = 55;
    pub const COLDLENGTH: u16 = 56;
    pub const POISONMINDAM: u16 = 57;
    pub const POISONMAXDAM: u16 = 58;
    pub const POISONLENGTH: u16 = 59;
    pub const MANADRAINMINDAM: u16 = 62;
    pub const MANADRAINMAXDAM: u16 = 63;
    pub const VELOCITYPERCENT: u16 = 67;
    pub const ATTACKRATE: u16 = 68;
    pub const OTHER_ANIMRATE: u16 = 69;
    pub const MONSTER_PLAYERCOUNT: u16 = 100;
    pub const ITEM_TOHIT_PERCENT: u16 = 119;
}

/// Type flags (monster data +0x16, D2MOO `nTypeFlag`).
pub mod type_flag {
    pub const BOSS: u16 = 0x01;
    pub const SUPERUNIQUE: u16 = 0x02;
    pub const CHAMPION: u16 = 0x04;
    pub const UNIQUE: u16 = 0x08;
    pub const MINION: u16 = 0x10;
    pub const POSSESSED: u16 = 0x20;
    pub const GHOSTLY: u16 = 0x40;
}

/// Creation flags (request +0x24, `population.md` §9.5).
pub mod create_flag {
    pub const PROBE: u16 = 0x01;
    pub const NO_NORMAL_MODS: u16 = 0x02;
    pub const NON_WATER: u16 = 0x04;
    pub const NOT_COUNTED: u16 = 0x08;
    pub const KEEP_GUID: u16 = 0x20;
    pub const NO_PARTY: u16 = 0x40;
    pub const IGNORE_COLLISION: u16 = 0x80;
}

/// Unit flags (unit +0xC4) set by init.
pub mod unit_flag {
    /// `|= 0x0A` at type init (§5 step 1).
    pub const AT_INIT: u32 = 0x0A;
    /// monstats2 `isAtt` (§6 step 15).
    pub const IS_ATT: u32 = 0x04;
    /// `Align` 1 (`0x005543B0`, `population.md` §9.6 step 4).
    pub const ALIGN1: u32 = 0x0002_0000;
    /// monstats `petIgnore` (§6 step 15).
    pub const PET_IGNORE: u32 = 0x4000_0000;
}

/// Monster modes init tests (`monsters/ai.md` mode numbers).
pub mod mode {
    pub const DEATH: u32 = 0;
    pub const NEUTRAL: u32 = 1;
    pub const GETHIT: u32 = 3;
    pub const SKILL1: u32 = 8;
    pub const SKILL2: u32 = 9;
    pub const DEAD: u32 = 12;
}

/// Monster timer event type 7 (§22; `sim/tick.md` §5.6).
pub const EVENT_UMOD: u32 = 7;

/// Most umods a monster holds (monster data +0x1C, 9 bytes).
pub const MAX_UMODS: usize = 9;

/// HP cap before the ×256 (§6 step 8).
pub const HP_CAP: i32 = 0x7F_FFFF;

/// The 0x28-byte create request (§2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CreateRequest {
    /// +0x04 active room (`None` = the caller passes none).
    pub room: Option<crate::units::RoomId>,
    /// +0x08: a coord list given (the list itself is the population
    /// session's; this is its handle).
    pub coord_list: Option<u32>,
    /// +0x0C monstats row.
    pub class: u32,
    /// +0x10 initial mode.
    pub mode: u32,
    /// +0x14 GUID, used with [`create_flag::KEEP_GUID`].
    pub guid: u32,
    /// +0x18 / +0x1C requested subtile position.
    pub x: i32,
    pub y: i32,
    /// +0x20 ring search limit.
    pub spread: i32,
    /// +0x24 creation flags ([`create_flag`]).
    pub flags: u16,
}

/// The monster data block (unit +0x14) as far as init writes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonsterData {
    /// +0x00: the monstats row.
    pub class: u32,
    /// +0x04 nComponent[16] (§10).
    pub components: [u8; 16],
    /// +0x14 wNameSeed.
    pub name_seed: u16,
    /// +0x16 nTypeFlag ([`type_flag`]).
    pub type_flags: u16,
    /// +0x18: frame of the last lightning burst (`umod-callbacks.md`
    /// §10.2).
    pub last_burst: i32,
    /// +0x1C nMonUmod[], 0-terminated unless full.
    pub umods: [u8; MAX_UMODS],
    /// +0x26 wBossHcIdx (superunique row, §20).
    pub boss_hc_idx: u16,
    /// +0x58 dwTxtLevelNo.
    pub level_id: i32,
    /// +0x5C bit 2: summoner "not counted" (§4 step 2).
    pub not_counted: bool,
}

impl MonsterData {
    /// Number of umods (up to the first 0, at most 9).
    pub fn umod_count(&self) -> usize {
        self.umods.iter().position(|&u| u == 0).unwrap_or(MAX_UMODS)
    }

    /// The umods in order.
    pub fn umod_list(&self) -> &[u8] {
        &self.umods[..self.umod_count()]
    }

    /// Appends `umod` when fewer than 9 are held; false when full.
    pub fn push_umod(&mut self, umod: u8) -> bool {
        let n = self.umod_count();
        if n >= MAX_UMODS {
            return false;
        }
        self.umods[n] = umod;
        true
    }

    pub fn has_umod(&self, umod: u8) -> bool {
        self.umod_list().contains(&umod)
    }

    pub fn has_flag(&self, f: u16) -> bool {
        self.type_flags & f != 0
    }
}

/// Work the original does that d2rs has no body for yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unhandled {
    /// A callback address without a body (none of `umods.tsv`'s).
    Callback {
        addr: u32,
        unit: UnitId,
        umod: u8,
        mode: u8,
    },
    /// A fatal assertion of a callback body (`umod-callbacks.md` §8
    /// step 3: umod 14's mode 0 on a unit that is not a monster).
    Assert { addr: u32, unit: UnitId },
}

/// The monster data of every monster, by unit.
#[derive(Clone, Debug, Default)]
pub struct MonsterStore {
    units: BTreeMap<UnitId, MonsterData>,
    pub unhandled: Vec<Unhandled>,
}

impl MonsterStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, u: UnitId) -> Option<&MonsterData> {
        self.units.get(&u)
    }
    pub fn get_mut(&mut self, u: UnitId) -> Option<&mut MonsterData> {
        self.units.get_mut(&u)
    }
    /// The unit's data, created empty when absent.
    pub fn entry(&mut self, u: UnitId) -> &mut MonsterData {
        self.units.entry(u).or_default()
    }
    /// Unit removal frees the block (the units session calls this).
    pub fn remove(&mut self, u: UnitId) {
        self.units.remove(&u);
    }
}

/// Game settings init reads (§Inputs). `players` is `0x00535790`,
/// `players_x` the global `0x00883D70`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameInfo {
    /// Game +0x6D.
    pub difficulty: u8,
    /// Game +0x70 ≠ 0.
    pub expansion: bool,
    /// Game +0x6A.
    pub game_type: u8,
    /// Game +0x74 ≠ 0.
    pub ladder: bool,
    pub players: i32,
    pub players_x: i32,
}

impl GameInfo {
    /// The L-flag of §8.1: game +0x6A ≠ 0 or game +0x74 ≠ 0.
    pub fn l_flag(&self) -> bool {
        self.game_type != 0 || self.ladder
    }

    /// The difficulty clamped to 0..2 (§6 step 4 without the hireling
    /// rule).
    pub fn d(&self) -> usize {
        usize::from(self.difficulty.min(2))
    }
}

/// Per-row facts compiled at load that the typed records lack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonstatsExtra {
    /// `Sk1mode`..`Sk8mode` (monstats record +0x180..+0x187, signed).
    pub skill_modes: [i8; 8],
}

/// The `Sk<i>mode` bytes of every `monstats.bin` record (+0x180).
pub fn monstats_extra(t: &BinTable) -> Vec<MonstatsExtra> {
    t.iter()
        .map(|r| {
            let mut skill_modes = [-1i8; 8];
            if let Some(b) = r.get(0x180..0x188) {
                for (m, &v) in skill_modes.iter_mut().zip(b) {
                    *m = v as i8;
                }
            }
            MonstatsExtra { skill_modes }
        })
        .collect()
}

/// The component choice counts of every `monstats2.bin` record
/// (+0x15..+0x24, built from `HDv`…`S8v`, §10).
pub fn component_counts(t: &BinTable) -> Vec<[u8; 16]> {
    t.iter()
        .map(|r| {
            r.get(0x15..0x25)
                .and_then(|b| b.try_into().ok())
                .unwrap_or([0; 16])
        })
        .collect()
}

/// Ids init needs that the tables give by name, resolved by the
/// integrator (the spec names them, not their rows).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NamedIds {
    /// skills `monteleport` (umod 26, §19.6).
    pub monteleport: Option<u16>,
    /// monstats `BaseId` of bloodraven (§14.2).
    pub bloodraven: Option<u16>,
}

/// The tables init reads, typed `d2-data` records (`data/loading.md`).
#[derive(Clone, Copy)]
pub struct InitTables<'a> {
    pub monstats: &'a [Monstats],
    pub monstats2: &'a [Monstats2],
    pub monlvl: &'a [Monlvl],
    pub levels: &'a [Levels],
    pub monprop: &'a [Monprop],
    pub monequip: &'a [Monequip],
    pub monumod: &'a [Monumod],
    pub superuniques: &'a [Superuniques],
    pub difficultylevels: &'a [Difficultylevels],
    /// Per monstats row ([`monstats_extra`]).
    pub monstats_extra: &'a [MonstatsExtra],
    /// Per monstats2 row ([`component_counts`]).
    pub components: &'a [[u8; 16]],
    pub ids: NamedIds,
}

/// What init works with besides the host: the tables.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub tables: InitTables<'a>,
}

impl<'a> Ctx<'a> {
    pub fn monstats(&self, class: u32) -> Option<&'a Monstats> {
        self.tables.monstats.get(class as usize)
    }

    /// The class's monstats2 record (`MonStatsEx`).
    pub fn monstats2(&self, class: u32) -> Option<&'a Monstats2> {
        let m = self.monstats(class)?;
        self.tables.monstats2.get(usize::from(m.monstatsex))
    }

    /// monumod row `i` `constants` (K[i] of §19).
    pub fn k(&self, i: usize) -> i32 {
        self.tables.monumod.get(i).map_or(0, |r| r.constants as i32)
    }

    /// difficultylevels `ChampionDamageBonus` (B of §19).
    pub fn champion_bonus(&self, d: usize) -> i32 {
        self.tables
            .difficultylevels
            .get(d)
            .map_or(0, |r| r.championdamagebonus as i32)
    }
}

/// monstats values are signed 16-bit (§8.1).
pub(crate) fn s16(v: u16) -> i32 {
    i32::from(v as i16)
}
