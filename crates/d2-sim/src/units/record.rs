// Spec: specs/sim/units.md §2
//! The unit record fields the simulation owns (§2), beside the list
//! fields of [`super::lists`]: one [`UnitRecord`] per live unit in
//! [`Units`], keyed by [`UnitId`]. Fields whose owner spec is not
//! written yet (path, inventory, per-kind data, combat list) are not
//! here; they stay with their systems.

use std::collections::BTreeMap;

use crate::rng::Seed;
use crate::stats::ListId;

use super::{UnitId, UnitType};

/// Unit flags (+0xC4, §2).
pub mod flags {
    /// Changed: set by every mode set.
    pub const CHANGED: u32 = 0x1;
    /// Tile.
    pub const TILE: u32 = 0x2;
    /// Seed set (every allocation).
    pub const SEED_SET: u32 = 0x10;
    /// Cleared by attack-mode starts.
    pub const ATTACK_PENDING: u32 = 0x40;
    /// Hover freed.
    pub const HOVER_FREED: u32 = 0x100;
    /// Dead.
    pub const DEAD: u32 = 0x10000;
    /// Monster mode changing.
    pub const MODE_CHANGING: u32 = 0x80000;
}

/// Unit flags 2 (+0xC8, §2).
pub mod flags2 {
    /// Disguised (`stat-lists.md` §9.2).
    pub const DISGUISE: u32 = 0x8;
    /// Expansion game.
    pub const EXPANSION: u32 = 0x0200_0000;
    /// Server unit (every allocation).
    pub const SERVER: u32 = 0x0400_0000;
}

/// Node index at allocation (+0xD0).
pub const INITIAL_NODE_INDEX: u32 = 11;

/// Event bytes per AnimData record (§4.2 main-form bound).
pub const ANIM_EVENTS: usize = 144;

/// The AnimData record fields §4.2 reads (`formats/animdata.md` §5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimRecord {
    /// Frames per direction.
    pub frames: u32,
    /// Byte +0x0F, the speed's high byte: event index −1 of the §4.2
    /// variants (edge case 1).
    pub byte_0f: u8,
    /// Event bytes, +0x10 + i.
    pub events: [u8; ANIM_EVENTS],
}

/// A sequence's frame count, speed and event bytes (`0x006634C0`,
/// sequence spec).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sequence {
    /// +0x34, 8.8.
    pub frame_count: i32,
    /// +0x3C.
    pub speed: i32,
    /// Event byte per frame index.
    pub events: Vec<u8>,
}

/// Animation fields (+0x30 … +0x50).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Anim {
    /// +0x30 (with +0x34, +0x3C).
    pub sequence: Option<Sequence>,
    /// +0x44, current frame, 8.8.
    pub frame: i32,
    /// +0x48, frame count, 8.8.
    pub frame_count: i32,
    /// +0x4C, i16, 1/256 frame per tick.
    pub speed: i16,
    /// +0x4E.
    pub action_frame: u8,
    /// +0x50.
    pub record: Option<AnimRecord>,
}

/// The simulation's unit record (§2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitRecord {
    /// +0x00.
    pub ty: UnitType,
    /// +0x04.
    pub class: u32,
    /// +0x0C (also in the lists).
    pub guid: u32,
    /// +0x10.
    pub mode: u32,
    /// +0x18; +0x1C is the act record of the game.
    pub act: u8,
    /// +0x20, +0x28 (`rng.md` §5.3).
    pub seed: Seed,
    pub init_seed: u32,
    /// Item seed and start seed (item data +0x04, +0x10).
    pub item_seed: Option<(Seed, u32)>,
    pub anim: Anim,
    /// +0x5C.
    pub stats: Option<ListId>,
    /// +0xA4: the hover's timeout frame (`0x006611D0`), when a hover is
    /// set.
    pub hover: Option<i32>,
    /// +0xC4.
    pub flags: u32,
    /// +0xC8.
    pub flags2: u32,
    /// +0xD0.
    pub node_index: u32,
}

impl UnitRecord {
    /// A record with every field at its allocation default.
    pub fn new(ty: UnitType, class: u32, guid: u32) -> Self {
        Self {
            ty,
            class,
            guid,
            mode: 0,
            act: 0,
            seed: Seed::init(),
            init_seed: 0,
            item_seed: None,
            anim: Anim::default(),
            stats: None,
            hover: None,
            flags: 0,
            flags2: 0,
            node_index: INITIAL_NODE_INDEX,
        }
    }

    /// "Dead" `0x005541B0` (§2): flag 0x10000, a player in mode 0 or 17,
    /// a monster in mode 0 or 12, and every other unit type.
    pub fn is_dead(&self) -> bool {
        if self.flags & flags::DEAD != 0 {
            return true;
        }
        match self.ty {
            UnitType::Player => matches!(self.mode, 0 | 17),
            UnitType::Monster => matches!(self.mode, 0 | 12),
            _ => true,
        }
    }
}

/// The unit records of one game.
#[derive(Clone, Debug, Default)]
pub struct Units {
    records: BTreeMap<UnitId, UnitRecord>,
}

impl Units {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: UnitId) -> Option<&UnitRecord> {
        self.records.get(&id)
    }

    pub fn get_mut(&mut self, id: UnitId) -> Option<&mut UnitRecord> {
        self.records.get_mut(&id)
    }

    pub fn insert(&mut self, id: UnitId, record: UnitRecord) {
        self.records.insert(id, record);
    }

    pub fn remove(&mut self, id: UnitId) -> Option<UnitRecord> {
        self.records.remove(&id)
    }

    /// `0x005541B0`: a missing unit counts as dead.
    pub fn is_dead(&self, id: UnitId) -> bool {
        self.get(id).is_none_or(UnitRecord::is_dead)
    }
}
