// Spec: specs/world/hirelings.md
//! Hirelings: the `hireling.txt` rows, offer and costs ([`rows`]), the
//! hireling pet list and its messages ([`pets`]), creating, following,
//! death and revive ([`life`]), level stats, experience and the stat
//! messages ([`level`]) and the item swap ([`items`]).
//!
//! The hire list and the hire / resurrect handlers are `world/npc.md` §7
//! ([`crate::world::npc::hire`]); they reach this module through the
//! `NpcWorld` mercenary calls, which the interaction wiring maps here.
//!
//! The pet list kept here is the hireling's (pet type 7) only: the other
//! pet types are `sim/pets.md`'s (`d2-sim::player::pets`, another
//! session). Everything outside the spec (unit records, stats, skills,
//! messages, rooms, the AI, the owner link) is reached through one seam,
//! [`HirelingWorld`].

pub mod items;
pub mod level;
pub mod life;
pub mod pets;
pub mod rows;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use d2_data::bin::BinTable;
use d2_data::tables::{Pettype, Record};

use crate::units::UnitId;

pub use rows::{resurrect_cost, threshold, HirelingRow, HirelingRows, Offer, RowSkill};

/// Pet type of a hireling (`pettype` row 7, `hireable`).
pub const PET_HIRELING: u8 = 7;
/// Unit type of a player.
pub const UNIT_PLAYER: u8 = 0;
/// `saved_id` of a new hire (§3.2).
pub const NEW_HIRE: u16 = 0xFFFF;
/// Highest level gained by experience: MaxLvl − 1 (§7.3 rule 3).
pub const XP_LEVEL_CAP: i32 = 98;
/// Hireling classes (§1.1 rule 4); the Act 3 shield rule's class (§11
/// rule 2).
pub mod class {
    pub const ACT1: u32 = 271;
    pub const ACT2: u32 = 338;
    pub const ACT3: u32 = 359;
    pub const ACT5: u32 = 561;
}

/// Unit flags (+0xC4) and flags 2 (+0xC8) this spec writes (§Constants).
pub mod flags {
    /// §3.2 rule 1: NOXP | NOTC | ISMERC.
    pub const INIT: u32 = 0x0402_0200;
    /// §3.2 rule 7 (D2MOO ISREVIVE): owned.
    pub const OWNED: u32 = 0x8000_0000;
    /// §8 rule 4: dead.
    pub const DEAD: u32 = 0x1_0000;
    /// §5 rule 5: NOXP set on an expired pet.
    pub const NOXP: u32 = 0x400_0000;
    /// §9 rule 6: NOXP, NOTC, ISVALIDTARGET, CANBEATTACKED, TARGETABLE.
    pub const REVIVE: u32 = 0x0402_000E;
    /// §9 rule 6: cleared from flags 2.
    pub const REVIVE_CLEAR2: u32 = 0x4_0000;
    /// §6 rule 5: set in flags 2 by a warp.
    pub const WARP2: u32 = 0x1_0000;
}

/// Stat ids (§Constants).
pub mod stat {
    pub const STRENGTH: u16 = 0;
    pub const DEXTERITY: u16 = 2;
    pub const HITPOINTS: u16 = 6;
    pub const MAXHP: u16 = 7;
    pub const LEVEL: u16 = 12;
    pub const EXPERIENCE: u16 = 13;
    pub const GOLD: u16 = 14;
    pub const TOHIT: u16 = 19;
    pub const MINDAMAGE: u16 = 21;
    pub const MAXDAMAGE: u16 = 22;
    pub const SECONDARY_MINDAMAGE: u16 = 23;
    pub const SECONDARY_MAXDAMAGE: u16 = 24;
    pub const NEXTEXP: u16 = 30;
    pub const ARMORCLASS: u16 = 31;
    pub const FIRERESIST: u16 = 39;
    pub const LIGHTRESIST: u16 = 41;
    pub const COLDRESIST: u16 = 43;
    pub const POISONRESIST: u16 = 45;
    pub const HPREGEN: u16 = 74;
    pub const ADDEXPERIENCE: u16 = 85;
    pub const ALIGNMENT: u16 = 172;
}

/// State ids (§Constants).
pub mod state {
    pub const FREEZE: u16 = 1;
    pub const ALIGNMENT: u16 = 105;
    pub const SHATTER: u16 = 107;
    pub const RECYCLED: u16 = 154;
}

/// What can go wrong outside the 1.14d rules (table decode, a fatal
/// assertion of 1.14d).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HirelingError {
    #[error("hireling table: {0}")]
    Table(String),
    /// §5 rule 5: the pet count went negative (fatal assertion).
    #[error("hireling pet count below zero")]
    NegativeCount,
    /// §13 rule 4: a stat id above 0xFE (fatal assertion).
    #[error("stat {0} does not fit a hireling stat message")]
    StatId(u16),
}

/// The hire slot handed to the init (§3.2: {name id u16, seed u32}).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Slot {
    pub name: u16,
    pub seed: u32,
}

/// One pet node of the hireling list (§5 rule 2, 24 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PetNode {
    /// +0x00 bit 0.
    pub dead: bool,
    /// +0x04.
    pub guid: u32,
    /// +0x08: the slot seed.
    pub seed: u32,
    /// +0x0C.
    pub name: u16,
    /// +0x10: the `hireling` `Id`.
    pub id: u32,
}

/// A player's hireling list (pet list entry 7, §5 rule 2: head, count,
/// max). Nodes are held oldest first (head at index 0).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PetList {
    pub nodes: Vec<PetNode>,
    /// +0x08. 0: not yet set (§5 rule 3 recomputes it).
    pub max: i32,
}

/// The data this module reads besides the rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirelingTables {
    pub rows: HirelingRows,
    /// `experience` `MaxLvl` of class 0 (`0x00611830(0)`; 99 in 1.14d).
    pub max_level: i32,
    /// `pettype` row 7 flags byte +4 (warp 0x1, range 0x2) and `basemax`
    /// (§6 rule 2: 1.14d warp 1, range 0, basemax 1).
    pub pet_flags: u8,
    pub pet_basemax: i32,
}

impl HirelingTables {
    /// `pettype` row 7 `warp` (mask `0x006CE268`).
    pub const WARP: u8 = 0x1;
    /// `pettype` row 7 `range` (mask `0x006CE26C`).
    pub const RANGE: u8 = 0x2;

    /// The tables from the fixed-up `hireling` and `pettype` tables and
    /// the `experience` `MaxLvl` of class 0 (`vitals.md` §4.1).
    pub fn from_tables(
        hireling: &BinTable,
        pettype: &BinTable,
        max_level: i32,
    ) -> Result<Self, HirelingError> {
        if pettype.name != Pettype::TABLE || pettype.record_size != Pettype::SIZE {
            return Err(HirelingError::Table(format!(
                "{} ({}-byte records) is not pettype",
                pettype.name, pettype.record_size
            )));
        }
        let row = pettype
            .iter()
            .nth(usize::from(PET_HIRELING))
            .map(Pettype::decode)
            .ok_or_else(|| HirelingError::Table("pettype has no row 7".into()))?;
        Ok(Self {
            rows: HirelingRows::from_table(hireling)?,
            max_level,
            pet_flags: (u8::from(row.warp) * Self::WARP) | (u8::from(row.range) * Self::RANGE),
            pet_basemax: i32::from(row.basemax),
        })
    }
}

/// The game's hireling state: one [`PetList`] per player with lists.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HirelingState {
    pub lists: BTreeMap<UnitId, PetList>,
}

impl HirelingState {
    pub fn list(&self, player: UnitId) -> Option<&PetList> {
        self.lists.get(&player)
    }
    pub fn list_mut(&mut self, player: UnitId) -> &mut PetList {
        self.lists.entry(player).or_default()
    }
    /// §5 rule 4 node part: the first node with `any` or bit 0 clear.
    pub fn first_node(&self, player: UnitId, any: bool) -> Option<&PetNode> {
        self.list(player)?.nodes.iter().find(|n| any || !n.dead)
    }
    /// §5 rule 4 `0x00574BD0` (type 7 part): the node of a GUID.
    pub fn node_by_guid(&self, player: UnitId, guid: u32) -> Option<&PetNode> {
        self.list(player)?.nodes.iter().find(|n| n.guid == guid)
    }
}

/// Everything the hireling rules reach outside this spec. Unit handles
/// are [`UnitId`]; GUIDs are the unit's +0x0C.
pub trait HirelingWorld {
    // ---- game
    /// Game +0x6D: difficulty (0-based).
    fn difficulty(&self) -> u8;
    fn expansion(&self) -> bool;
    /// The players in broadcast order (`0x005538D0`, `unit-order.md`:
    /// players without state 7).
    fn players(&self) -> Vec<UnitId>;
    /// A message to the player's client.
    fn send(&mut self, player: UnitId, bytes: &[u8]);

    // ---- units
    fn guid(&self, unit: UnitId) -> u32;
    /// `0x00552F60(game, 1, GUID)`: the monster of a GUID.
    fn monster_by_guid(&self, guid: u32) -> Option<UnitId>;
    /// Unit type (0 player, 1 monster, …).
    fn unit_type(&self, unit: UnitId) -> u8;
    /// Monstats class.
    fn class(&self, unit: UnitId) -> u32;
    fn mode(&self, unit: UnitId) -> u32;
    /// `0x00624690` / `0x00553570`: a mode change.
    fn set_mode(&mut self, unit: UnitId, mode: u8);
    /// +0xC4.
    fn flags(&self, unit: UnitId) -> u32;
    fn set_flags(&mut self, unit: UnitId, flags: u32);
    /// +0xC8.
    fn flags2(&self, unit: UnitId) -> u32;
    fn set_flags2(&mut self, unit: UnitId, flags: u32);
    /// Whether the unit is in a room (§6 rule 4).
    fn in_room(&self, unit: UnitId) -> bool;

    // ---- stats, states, skills
    /// `0x00625480(unit, stat, 0)`: total value.
    fn stat(&self, unit: UnitId, stat: u16) -> i32;
    /// `0x006253B0(unit, stat, 0)`: base value.
    fn base_stat(&self, unit: UnitId, stat: u16) -> i32;
    /// `0x00627260(unit, stat, v, 0)`: set as a base value.
    fn set_base_stat(&mut self, unit: UnitId, stat: u16, value: i32);
    /// `0x00625D10`: maximum life.
    fn max_life(&self, unit: UnitId) -> i32;
    /// `0x005543B0(unit, state, 0)` stat part (§3.2 rule 2): `stat` :=
    /// `value` in the stat list of `state`, creating the list if missing.
    fn set_state_stat(&mut self, unit: UnitId, state: u16, stat: u16, value: i32);
    /// Remove a state (and its stat list) from the unit (§9 rule 5).
    fn remove_state(&mut self, unit: UnitId, state: u16);
    /// `skills` count.
    fn skill_count(&self) -> u32;
    /// `skills` `reqlevel` (+0x174, u16 read signed) of a skill record;
    /// `None`: no record.
    fn skill_reqlevel(&self, skill: u32) -> Option<i16>;
    /// `0x0056DEB0`: the unit's skill level of `skill` := `level`.
    fn set_skill_level(&mut self, unit: UnitId, skill: u32, level: i32);

    // ---- other owners (AI, rooms, monsters, items, events)
    /// `0x0058F030(game, merc, GUID, type, 0, 0)` (§5 rule 1); GUID −1
    /// clears the owner.
    fn set_owner(&mut self, merc: UnitId, guid: u32, unit_type: u8);
    /// `0x0058F0D0`: the owner (GUID, type).
    fn owner(&self, merc: UnitId) -> Option<(u32, u8)>;
    /// `0x005B1900(game, merc, 0, player +0xD0)`: join the player's team
    /// (§3.2 rule 3).
    fn join_team(&mut self, merc: UnitId, player: UnitId);
    /// §3.2 rule 10 (`0x005A4850(game, merc, 0x13, 0)`) and rule 11
    /// (monster data bytes +4, +5, +9, +10, +11 := 0).
    fn hireling_ai(&mut self, merc: UnitId);
    /// §3.2 rule 4: queue the unit's removal from its room
    /// (`0x0061A270(room, 1, GUID)`) and free it (`0x00555600`), items
    /// included.
    fn free_unit(&mut self, unit: UnitId);
    /// §6 rule 4: queue the unit's removal from its room only.
    fn queue_room_removal(&mut self, unit: UnitId);
    /// The death event on a unit (§6 rule 4).
    fn death_event(&mut self, unit: UnitId);
    /// §5 rule 5 / `pets.md` §7 (`0x00574450`): flags |= NOXP, then kill
    /// or a death mode request.
    fn dismiss(&mut self, unit: UnitId);
    /// §6 rule 5 (`0x00574D90` → `0x00574CC0`): move the pet to the
    /// player's room and position (room lists, act, update queue, path
    /// reset). The flags 2 bit is set by the caller.
    fn warp_to(&mut self, pet: UnitId, player: UnitId);
    /// §7.3 rule 6: event 0x5B on the merc for the player (`0x00553380`)
    /// and unit event 12 (`0x005C0C30(game, 12, merc, 0, 0)`).
    fn level_events(&mut self, player: UnitId, merc: UnitId);
    /// §9 rule 7 (`0x00577470`): re-apply the stats of every active
    /// equipped item.
    fn reapply_item_stats(&mut self, merc: UnitId);
}
