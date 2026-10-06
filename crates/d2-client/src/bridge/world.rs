// Spec: specs/client/model.md (§1, §2, §5 rule 4), specs/client/bridge.md (§5)
//! Client world model: what the S→C messages have told the client. Plain
//! Rust, no Bevy. Not game state: `d2-sim` on the server is.
//!
//! [`ClientWorld`] holds the fields of `model.md` §1 rule 1 plus the
//! bridge's own counters (`bridge.md` §5 rule 3); [`ClientUnit`] the
//! fields of §1 rule 2, with the per-kind data of `msg-units.md` §1 and
//! `msg-stats-items.md` §2–§3. The tables and the visibility predicate the
//! message rules read are inputs ([`ModelInputs`]), not model fields.

use std::collections::BTreeMap;

use d2_proto::transport::server_message;

/// Unit types (`sim/unit-order.md` §1 rule 1).
pub const PLAYER: u8 = 0;
/// Unit type of monsters.
pub const MONSTER: u8 = 1;
pub const OBJECT: u8 = 2;
pub const MISSILE: u8 = 3;
pub const ITEM: u8 = 4;
pub const TILE: u8 = 5;

/// A unit's identity: (unit type, GUID) (`sim/unit-order.md` §1 rule 1).
/// Ordered by type, then GUID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitKey {
    pub unit_type: u8,
    pub guid: u32,
}

impl UnitKey {
    pub const fn new(unit_type: u8, guid: u32) -> Self {
        Self { unit_type, guid }
    }
}

/// A mode request handed to the unit-type mode machines (§8): `code` and
/// the 7-value record; entries 1.14d leaves unset are 0 (§8 rule 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeRequest {
    pub code: u8,
    pub record: [i32; 7],
}

/// Player data (`msg-units.md` §1.1, `msg-stats-items.md` §3 rule 1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerData {
    /// The 16 name bytes of 0x59 (zero-padded).
    pub name: [u8; 16],
    /// The inventory's cursor item (GUID of an item unit).
    /// TODO(spec: the item stream header, `msg-stats-items.md` open
    /// question 3): which item actions set it; only 0x42 clears it today.
    pub cursor_item: Option<u32>,
}

/// Monster data from 0xAC (`msg-units.md` §1.2 rules 1, 4, 5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonsterData {
    pub name_seed: u16,
    /// Type flags: champion 4, unique 8, superunique 2, minion 0x10,
    /// ghostly 0x40.
    pub flags: u8,
    pub hc_idx: u16,
    /// The umod bytes as read (the 9-byte buffer, zero after the list).
    pub umods: [u8; 9],
    /// The 16 component choices (0 when not sent).
    pub components: [u8; 16],
    /// −1 when not sent.
    pub value: i32,
    /// The optional 31-bit value (open question 3: what it sets).
    pub v31: Option<u32>,
    /// The stat list with flag 0x40: (stat, param) → value.
    pub stat_list: Option<BTreeMap<(u16, u16), i32>>,
}

/// The last item message an item unit received (`msg-stats-items.md` §2
/// rule 4), until the item stream is specified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemRecord {
    /// 0x9C or 0x9D.
    pub id: u8,
    pub action: u8,
    pub category: u8,
    /// The owner of 0x9D.
    pub owner: Option<UnitKey>,
    pub stream: Vec<u8>,
}

/// Item data (`msg-stats-items.md` §2 rule 4, §3 rule 2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemData {
    pub last: Option<ItemRecord>,
    /// Item flag 4 (0x3F).
    pub flags4: bool,
}

/// Object data (`msg-units.md` §1.3 rule 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectData {
    pub interact: u8,
}

/// The per-kind data at unit +0x14 (model §1 rule 2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum KindData {
    #[default]
    None,
    Player(PlayerData),
    Monster(Box<MonsterData>),
    Object(ObjectData),
    Item(ItemData),
}

/// A unit as the client knows it (model §1 rule 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientUnit {
    pub key: UnitKey,
    pub class: u32,
    /// Set only by creation and the mode machines (§8).
    pub mode: u32,
    /// Subtile cell; `None` = not placed (created at (0, 0), §2 rule 6).
    pub position: Option<(u16, u16)>,
    /// The last point given to the position check (§6 rule 3).
    pub server_point: (u16, u16),
    /// Stat list layer 0 base values.
    pub stats: BTreeMap<u16, i32>,
    /// Client copy of the unit seed {lo, hi} (§2 rule 6). `None` when it
    /// was derived from a client room's seed, which the model does not
    /// hold yet (open question 5).
    pub seed: Option<(u32, u32)>,
    /// Queued unit-handler messages (§4).
    pub queue: Vec<Vec<u8>>,
    pub last_mode_request: Option<ModeRequest>,
    pub kind: KindData,
}

impl ClientUnit {
    /// A unit with only its key: class 0, mode 0, not placed, no stats,
    /// seed {1, 666} (`init()`), empty queue, no kind data.
    pub fn new(key: UnitKey) -> Self {
        Self {
            key,
            class: 0,
            mode: 0,
            position: None,
            server_point: (0, 0),
            stats: BTreeMap::new(),
            seed: Some(INIT_SEED),
            queue: Vec::new(),
            last_mode_request: None,
            kind: KindData::None,
        }
    }

    /// Base value of `stat` (0 when absent).
    pub fn stat(&self, stat: u16) -> i32 {
        self.stats.get(&stat).copied().unwrap_or(0)
    }

    /// The position as a cell, (0, 0) when not placed.
    pub fn cell(&self) -> (u16, u16) {
        self.position.unwrap_or((0, 0))
    }

    /// Dead (§6 rule 2): player mode 0 or 0x11, monster mode 0 or 0xC.
    /// TODO(spec: model.md open question 4 area): unit flag 0x10000 is not
    /// in the model; no rule sets it on the client yet.
    pub fn is_dead(&self) -> bool {
        match self.key.unit_type {
            PLAYER => matches!(self.mode, 0 | 0x11),
            MONSTER => matches!(self.mode, 0 | 0xC),
            _ => false,
        }
    }
}

/// `init()` = {1, 666} (`sim/rng.md` §4).
pub const INIT_SEED: (u32, u32) = (1, 666);

/// The client DRLG act of 0x03 (§7 rule 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActLoad {
    pub act: u8,
    pub init_seed: u32,
    pub town_level: u16,
    /// u32@8.
    pub f8: u32,
}

/// One room-in-sight message (§9 rule 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomSight {
    /// 0x07 (true) or 0x08 (false).
    pub show: bool,
    pub level: u8,
    pub x: u16,
    pub y: u16,
}

/// One active room of the client act (§12 rule 2): its sub-tile
/// rectangle (active room +0x4C, +0x50, +0x54, +0x58) and the id of its
/// level (§11 rule 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveRoom {
    pub x0: i32,
    pub y0: i32,
    pub w: i32,
    pub h: i32,
    pub level: u16,
}

impl ActiveRoom {
    /// x0 ≤ x < x0 + w and y0 ≤ y < y0 + h (signed compares).
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.x0 <= x && x < self.x0 + self.w && self.y0 <= y && y < self.y0 + self.h
    }
}

/// The act lookup `0x00619DA0` (§12 rule 2 b): the first active room in
/// list order whose rectangle contains (x, y).
pub fn room_of_point(rooms: &[ActiveRoom], x: i32, y: i32) -> Option<&ActiveRoom> {
    rooms.iter().find(|r| r.contains(x, y))
}

/// One record of the pet list `[0x007BB5BC]` (§14 rule 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PetRecord {
    /// +0x00.
    pub class: u16,
    /// +0x04.
    pub pet_type: u8,
    /// +0x08.
    pub pet: u32,
    /// +0x0C.
    pub owner: u32,
    /// +0x1C: 100 at creation (§14 rule 1).
    pub f1c: u32,
    /// +0x20.
    pub gone: bool,
    /// +0x24…: the three values of 0x81 (§14 rule 3); `None` until a 0x81
    /// sets them. TODO(spec: model.md open question 10): the fields past
    /// +0x24 and who reads them.
    pub extra: Option<[u32; 3]>,
}

/// The pet type of a hireling (§14 rule 4).
pub const PET_HIRELING: u8 = 7;

/// The use-item cursor of 0x3F (`msg-stats-items.md` §3 rule 2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UseCursor {
    pub item: UnitKey,
    pub code: u8,
}

/// The client world model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientWorld {
    /// Bridge frames run (`bridge.md` §5 rule 3).
    pub frames: u64,
    /// Bridge frames whose pump ran a server tick (`bridge.md` §5 rule 3).
    pub server_ticks: u64,
    /// Set S: the units the server announced, in key order (§2 rule 8).
    pub units: BTreeMap<UnitKey, ClientUnit>,
    /// `[0x007A6A70]` (§3).
    pub local_player: Option<UnitKey>,
    pub difficulty: u8,
    pub expansion: u32,
    pub ladder: u8,
    pub game_flags: u32,
    pub act: Option<ActLoad>,
    pub in_game: bool,
    pub unloaded: bool,
    pub exit_requested: bool,
    pub rooms_in_sight: Vec<RoomSight>,
    /// C→S messages the client sends on its own (§6 rule 8, §7 rule 3),
    /// until the bridge hands them to its send path.
    pub outgoing: Vec<Vec<u8>>,
    /// 0x3F's use-item cursor.
    pub use_cursor: Option<UseCursor>,
    /// The pet list, newest first (§14 rule 5).
    pub pets: Vec<PetRecord>,
    /// The act whose palette is loaded (§11 rules 2, 4): the act of 0x03,
    /// replaced by the Levels `Act` of the new level on a room change.
    pub palette_act: Option<u8>,
    /// The active rooms of the client act in list order (§12 rules 1–2).
    /// `None`: the client DRLG is not built (TODO(spec: model.md §12 rule
    /// 1): the `d2-sim` DRLG act from the 0x03 seed is not wired into the
    /// bridge yet), so a placement is taken as in a room and the level is
    /// unknown.
    pub active_rooms: Option<Vec<ActiveRoom>>,
}

impl ClientWorld {
    /// The local player unit, if the model has one.
    pub fn local(&self) -> Option<&ClientUnit> {
        self.units.get(&self.local_player?)
    }

    /// Adds `unit` (§2 rule 4): an existing unit with the same key is
    /// removed first (rule 5: its queue is dropped, and it stops being the
    /// local player).
    pub fn add(&mut self, unit: ClientUnit) {
        self.remove(unit.key);
        self.units.insert(unit.key, unit);
    }

    /// Removes the unit `key` (§2 rule 5): its queue is dropped
    /// unapplied; the local player is cleared if it was that unit. A key
    /// not in the set: nothing.
    pub fn remove(&mut self, key: UnitKey) -> Option<ClientUnit> {
        let unit = self.units.remove(&key)?;
        if self.local_player == Some(key) {
            self.local_player = None;
        }
        Some(unit)
    }

    /// The hireling GUID of `player` (§14 rule 4, `0x00478F20(player, 7,
    /// 1)`): the first pet record in list order with type 7 and owner
    /// `player` (gone records included); none → −1.
    pub fn hireling_guid(&self, player: Option<UnitKey>) -> u32 {
        let Some(p) = player else {
            return u32::MAX;
        };
        self.pets
            .iter()
            .find(|r| r.pet_type == PET_HIRELING && r.owner == p.guid)
            .map_or(u32::MAX, |r| r.pet)
    }

    /// The local player's room (§12 rule 2 on its position); `None` with
    /// no client DRLG, no local player or an unplaced one.
    pub fn local_room(&self) -> Option<&ActiveRoom> {
        let (x, y) = self.local()?.position?;
        room_of_point(self.active_rooms.as_deref()?, i32::from(x), i32::from(y))
    }

    /// The local player's level (§11 rules 3, 5): the level id of its
    /// room; none while it has no room.
    pub fn player_level(&self) -> Option<u16> {
        self.local_room().map(|r| r.level)
    }
}

/// The 1.14d order of the queue drains (§5 rules 3–4): missiles,
/// players, monsters, objects, items; then `GUID & 0x7F` ascending, then
/// GUID descending.
pub fn update_order(units: &BTreeMap<UnitKey, ClientUnit>) -> Vec<UnitKey> {
    const TYPES: [u8; 5] = [MISSILE, PLAYER, MONSTER, OBJECT, ITEM];
    let mut keys: Vec<UnitKey> = units
        .keys()
        .copied()
        .filter(|k| TYPES.contains(&k.unit_type))
        .collect();
    keys.sort_by_key(|k| {
        let t = TYPES.iter().position(|&t| t == k.unit_type);
        (t, k.guid & 0x7F, std::cmp::Reverse(k.guid))
    });
    keys
}

/// The unit an S→C message addresses through the receive table's unit
/// handler (`intents-events.md` §3.4 rule 3; `bridge.md` §5 rule 4).
/// `None` for ids without a unit handler and for messages too short for
/// the lookup (`bridge.md` open question 3).
pub fn addressed_unit(msg: &[u8]) -> Option<UnitKey> {
    let &id = msg.first()?;
    server_message(id)?.client_unit_handler?;
    let u32_at = |off: usize| -> Option<u32> {
        let b = msg.get(off..off + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if (0x67..=0x6D).contains(&id) {
        Some(UnitKey {
            unit_type: MONSTER,
            guid: u32_at(1)?,
        })
    } else {
        Some(UnitKey {
            unit_type: *msg.get(1)?,
            guid: u32_at(2)?,
        })
    }
}

/// Visibility of a unit's sprite at a client pixel point
/// (`0x004DBF20`, model §6 rule 6, open question 7): a Phase 6 render
/// seam, taken as an input.
pub type VisibleFn = fn(&ClientUnit, i32, i32) -> bool;

/// One `monstats` row as 0xAC reads it (`msg-units.md` §1.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonsterClass {
    /// `monstats2` choice count of each of the 16 components.
    pub components: [u8; 16],
}

/// One `itemstatcost` row as the 0xAC stat list reads it (§1.2 rule 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatSend {
    /// `send bits` (+8).
    pub bits: u8,
    /// `send param bits` (+9).
    pub param_bits: u8,
    /// The signed flag (row +4 & `[0x006CE26C]`).
    pub signed: bool,
}

/// The tables the message rules read (`msg-units.md` Inputs).
/// TODO(spec: which `monstats2` columns hold the component choice counts
/// in the 1.14d `.bin`): until a spec names them, the app supplies no
/// rows and 0xAC creates nothing (§1.2 rule 2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientTables {
    /// One entry per `monstats` row: `None` = no `monstats2` row.
    pub monsters: Vec<Option<MonsterClass>>,
    /// One entry per `itemstatcost` row.
    pub stats: Vec<StatSend>,
    /// One entry per `Levels.txt` row, by level id (§11 rule 4).
    pub levels: Vec<LevelRow>,
}

/// The `Levels.txt` fields the model reads (§11 rules 3–4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelRow {
    /// `Act`.
    pub act: u8,
    /// `BlankScreen` (record +0x218, `render/composition.md` §3 step 2).
    pub blank_screen: bool,
}

/// Inputs of the message rules that are not model state.
#[derive(Clone, Debug, Default)]
pub struct ModelInputs {
    pub tables: ClientTables,
    /// `None`: the check (§6 rule 6) refuses when it needs visibility.
    pub visible: Option<VisibleFn>,
}
