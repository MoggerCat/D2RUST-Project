// Spec: specs/client/model.md (§1, §2, §5 rule 4, §12 rule 2, §15 rule 1), specs/client/bridge.md (§5), specs/client/stat-lists.md (§1 rule 3, §3), specs/client/msg-units.md (§7, §8), specs/client/msg-stats-items.md (§5), specs/sim/unit-order.md (§5 rules 6–8), specs/render/lighting.md (§6.4, §10), specs/drlg/rooms.md (§4.6, §5 rule 9)
//! Client world model: what the S→C messages have told the client. Plain
//! Rust, no Bevy. Not game state: `d2-sim` on the server is.
//!
//! [`ClientWorld`] holds the fields of `model.md` §1 rule 1 plus the
//! bridge's own counters (`bridge.md` §5 rule 3); [`ClientUnit`] the
//! fields of §1 rule 2, with the per-kind data of `msg-units.md` §1 and
//! `msg-stats-items.md` §2–§3. The tables and the visibility predicate the
//! message rules read are inputs ([`ModelInputs`]), not model fields.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub use super::objects::{ClientObjects, ObjClientInputs};

use super::drlg::{ClientDrlg, DrlgRoomId, DrlgSource};
use super::skills::SkillList;
use crate::rules::lighting::environment::Environment;
use crate::rules::lighting::overrides::Overrides;
use crate::rules::lighting::records::{LightError, LightList, LightRooms, Owner, RoomId};
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
    /// question 3): which item actions set it; 0x42 and 0x58 code 5
    /// clear it.
    pub cursor_item: Option<u32>,
    /// Player data +0x2C, written by S→C 0x5F (`msg-units.md` §7 r4);
    /// its reader is `msg-units.md` open question 8.
    pub f2c: u32,
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
    /// Monster data +0x40, written by S→C 0x98 (`msg-units.md` §7 r9;
    /// −1 for 0xFFFF); `None` until written. Meaning: open question 9.
    pub f40: Option<i32>,
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
    /// d2rs-own, unverified: [`ClientWorld::store_serial`] when this
    /// store-shown record (action 0x0B) arrived; 0 for every other
    /// action. Lets the shop panel tell the items of the current trade
    /// from those of an earlier one.
    pub seq: u32,
}

/// Item data (`msg-stats-items.md` §2 rule 4, §3 rule 2, §5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemData {
    pub last: Option<ItemRecord>,
    /// Item flag 4 (0x3F, 0x3E, 0x40).
    pub flags4: bool,
    /// The other bits of the item flag word (item data +0x18) as the
    /// model rules write them (0x3E, 0x40, 0x7D); bit 4 is `flags4`.
    pub flags: u32,
    /// The properties of the last record's lists (`bridge::item_lists`).
    pub props: Vec<super::item_lists::ItemProp>,
    /// The item is a charm (type `char`).
    pub charm: bool,
}

impl ItemData {
    /// `0x006280D0(item, mask, on)` on the item flag word.
    pub fn set_flags(&mut self, mask: u32, on: bool) {
        if mask & 4 != 0 {
            self.flags4 = on;
        }
        let rest = mask & !4;
        if on {
            self.flags |= rest;
        } else {
            self.flags &= !rest;
        }
    }
}

/// Object data (`msg-units.md` §1.3 rule 3, §7 r5, §8 r7;
/// `model.md` §15 rule 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectData {
    /// +0x04: interact (0x51), the destination level (0x60).
    pub interact: u8,
    /// +0x05: the portal flags (0x60 ORs bits 0–1).
    pub portal_flags: u8,
    /// +0x28: the owner name of a portal (0x82, 16 bytes).
    pub owner_name: Option<[u8; 16]>,
    /// The shrine record's `Code` (shrine data +0x08, `model.md` §15
    /// rule 1); `None`: no shrine record. Set by 0x51 for a shrine
    /// (`msg-units.md` §1.3 r3, `0x004BD6B0`: the shrines record of
    /// index interact).
    pub shrine: Option<u8>,
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
    /// How many times the position check took the server's point for the
    /// local player while the play preview predicts its walk (§6 rule 8,
    /// `check::Checked::Followed`): the prediction snaps on a change.
    /// d2rs-own, PROVISIONAL REC-277.
    pub follows: u32,
    /// Stat list layer 0 base values.
    pub stats: BTreeMap<u16, i32>,
    /// Client copy of the unit seed {lo, hi} (§2 rule 6). `None` when it
    /// was derived from a client room's seed with no client DRLG in the
    /// model (no DRLG source).
    pub seed: Option<(u32, u32)>,
    /// Queued unit-handler messages (§4).
    pub queue: Vec<Vec<u8>>,
    pub last_mode_request: Option<ModeRequest>,
    /// How many mode requests (§8 rule 1) the unit has had: d2rs
    /// bookkeeping (no 1.14d field), so a reader can tell a new request
    /// from a repeat of the same one.
    pub mode_requests: u32,
    pub kind: KindData,
    /// The skill list (+0xA8, `msg-skills.md` §1 rule 1); `None` = no
    /// list.
    pub skills: Option<SkillList>,
    /// Unit flag +0xC4 bit 0x2 cleared by S→C 0x5D (`msg-ui.md` §1
    /// rule 4).
    pub quest_untargetable: bool,
    /// Unit flag +0xC4 bit 0x2 as last written by a model rule: 0x5D
    /// clears it (with `quest_untargetable`), 0x28 and 0x62 set it
    /// (`msg-ui.md` §16 r4, §17 r2). `None`: never written; the initial
    /// value per kind is `msg-ui.md` open question 2.
    pub flag_2: Option<bool>,
    /// The state bits that are on (`client/stat-lists.md` §3 r3;
    /// `sim/stat-lists.md` §9.2).
    pub states: BTreeSet<u8>,
    /// The stat list of each state (`client/stat-lists.md` §3 r2):
    /// (stat, param) → value, attached to the unit.
    pub state_lists: BTreeMap<u8, BTreeMap<(u16, u16), i32>>,
    /// The unit this unit was last turned toward by a model rule
    /// (`msg-ui.md` §16 r4.3: `0x00649EF0` on its path toward that
    /// unit's position, `0x00464420` / `0x004644E0`). The model holds no
    /// client path record, so the turn's input is kept, not the
    /// direction.
    pub turned_toward: Option<UnitKey>,
    /// `0x00648730` (path flag 0x20 cleared, point count := 0) was called
    /// on the unit's path by a model rule (`msg-ui.md` §16 r4.3).
    pub path_stopped: bool,
    /// The unit whose path direction this unit's path took
    /// (`0x006487F0` → `0x006488A0`; `msg-units.md` §7 r7.2: a corpse
    /// takes its player's). The model holds no client path record, so
    /// the source is kept, not the direction.
    pub direction_of: Option<UnitKey>,
    /// Unit flag `+0xC4` 0x800000: set only by the client room free
    /// `0x0061A840` (`model.md` §5 r5, `drlg/rooms.md` §8 r4).
    pub room_freed: bool,
    /// Unit flag `+0xC4` 0x200: set by the client hireling setup of 0x81
    /// (`model.md` §14 r3); 0xAB skips such a unit (`msg-units.md` §7
    /// r11).
    pub flag_200: bool,
    /// +0xD4 (u32, 0 at creation): the interact stamp of a monster
    /// (§8 rule 7) and the timer T of the client object functions
    /// (`world/objects-client.md` §25 r5).
    pub interact_ms: u32,
    /// +0x44: the animation frame (signed, 8.8 fixed point; §18 rule 1).
    pub frame: i32,
    /// +0xC8: flag-ex. Bit 0x2000000 := `expansion` ≠ 0 at creation
    /// (§2 rule 6); the other bits are written by the rules that own
    /// them (`world/objects-client.md` §26.2; `msg-units.md` §1.2 r3, r4:
    /// 0x40000 cleared for a hireling in mode 1, 0x400 the source-unit
    /// link; 0x20 set by the client room free, `drlg/rooms.md` §8 r4).
    pub flag_ex: u32,
    /// Unit flag +0xC4 bit 0x4, read by the interact sender (§8 rule 7).
    /// TODO(spec: client/model.md §8 rule 7): no model rule writes it.
    pub flag_4: bool,
    /// +0xB0: the last hit class (§18 rule 1), written by the player
    /// mode machine (§8 rule 4: `+0xB0` := r2); 0 at creation.
    pub hit_class: u32,
}

/// The reserved `outgoing` slot of 0x28's dialog branch (`msg-ui.md`
/// §16 r4, open question 10 decided as A): C→S 0x31 of case B2 goes here,
/// right after the handler's 0x2F, so the bytes keep 1.14d's order.
pub const DIALOG_REPLY_SLOT: Vec<u8> = Vec::new();

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
            follows: 0,
            stats: BTreeMap::new(),
            seed: Some(INIT_SEED),
            queue: Vec::new(),
            last_mode_request: None,
            mode_requests: 0,
            kind: KindData::None,
            skills: None,
            quest_untargetable: false,
            flag_2: None,
            states: BTreeSet::new(),
            state_lists: BTreeMap::new(),
            turned_toward: None,
            path_stopped: false,
            direction_of: None,
            room_freed: false,
            flag_200: false,
            interact_ms: 0,
            frame: 0,
            flag_ex: 0,
            flag_4: false,
            hit_class: 0,
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
    /// PROVISIONAL (client/msg-ui.md OQ 2 → client/msg-units.md, unit
    /// flag word +0xC4): unit flag 0x10000 is set only by the dead-state
    /// flow the server sync carries (0x15's flag), which the model keeps
    /// as the mode, so the test reads the mode alone; settled by a Ghidra
    /// xref of all +0xC4 writers.
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

/// A session message the loading flow cares about, in arrival order
/// (`ui/frontend-loading.md` L9, L10). d2rs-own: the model keeps edges
/// only, so a repeated same-act 0x03 or a 0x05 / 0x04 pair inside one
/// frame would otherwise be lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionMark {
    /// S→C 0x03 of this act.
    LoadAct(u8),
    /// S→C 0x04.
    LoadComplete,
    /// S→C 0x05.
    Unload,
    /// S→C 0x61 with this video id.
    Video(u8),
}

/// Marks kept in [`ClientWorld::session_log`].
pub const SESSION_LOG_CAP: usize = 64;

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
/// rectangle (active room +0x4C, +0x50, +0x54, +0x58), the id of its
/// level (§11 rule 3) and its DRLG room (active room +0x10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveRoom {
    pub x0: i32,
    pub y0: i32,
    pub w: i32,
    pub h: i32,
    pub level: u16,
    pub room: DrlgRoomId,
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
    /// sets them. PROVISIONAL (client/model.md §14, OQ 10): only the three
    /// 0x81 values are stored (no other field past +0x24, no reader of
    /// +0x1C modelled); settled by a recording with a hireling (0x7A /
    /// 0x81; HANDOFF §7 PC 2 recording list).
    pub extra: Option<[u32; 3]>,
}

/// `PingState` (`model.md` §7 r11): the fields 0x8F writes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PingState {
    /// `[0x007A04A4]`: last round trip in ms (0 until a 0x8F; d2rs sends
    /// no ping, so it stays 0).
    pub rtt: u32,
    /// `[0x007A04CC]`.
    pub samples: u32,
    /// `[0x007A04D0]`.
    pub mean: u32,
    /// `[0x007A04D4]`…`[0x007A04F0]`.
    pub pong: [u32; 8],
}

/// The pet type of a hireling (§14 rule 4).
pub const PET_HIRELING: u8 = 7;

/// The pet type the 0x75 pet pass visits (`msg-units.md` §8 r10).
pub const PET_TYPE_PASS: u8 = 4;

/// One player roster record (`msg-units.md` §8 r1; 0xD8 bytes in
/// 1.14d, the UI handle +0x34 and the formatted string +0x66 are UI
/// fields and not held).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RosterRecord {
    /// +0x00 (16 bytes, NUL-terminated).
    pub name: [u8; 16],
    /// +0x10.
    pub guid: u32,
    /// +0x14: life percent.
    pub life: u32,
    /// +0x18: kill count.
    pub kills: i32,
    /// +0x1C.
    pub class: u32,
    /// +0x20, +0x22.
    pub f20: u16,
    pub f22: u16,
    /// +0x30.
    pub f30: u16,
    /// +0x38: the corpse list, head first.
    pub corpses: Vec<u32>,
    /// +0x3C, +0x40: portal GUIDs.
    pub portals: (u32, u32),
    /// +0x44.
    pub f44: u16,
    /// The bytes written from +0x46 on by the two string copies of 0x5B
    /// (string 1 at +0x46, string 2 at +0x4A, each with its NUL);
    /// unwritten bytes are 0.
    pub strings: Vec<u8>,
}

/// The use-item cursor of 0x3F (`msg-stats-items.md` §3 rule 2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UseCursor {
    pub item: UnitKey,
    pub code: u8,
}

/// The units of each client active room (`sim/unit-order.md` §5 rules
/// 6–8): the room list at active room +0x74 (head first, insert =
/// prepend `0x0064C350`, remove `0x0064C370`), the client's only link
/// from a unit to a room. A unit at (0, 0) or with no room is in no list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomUnits {
    lists: BTreeMap<DrlgRoomId, Vec<UnitKey>>,
    rooms: BTreeMap<UnitKey, DrlgRoomId>,
}

impl RoomUnits {
    /// The room's list, head first (empty for a room with no units).
    pub fn list(&self, room: DrlgRoomId) -> &[UnitKey] {
        self.lists.get(&room).map_or(&[], Vec::as_slice)
    }

    /// The room whose list holds `key`.
    pub fn room_of(&self, key: UnitKey) -> Option<DrlgRoomId> {
        self.rooms.get(&key).copied()
    }

    /// Unit leaves its room (`0x0064C370` / `0x0064C450`); not in a list:
    /// nothing.
    pub fn leave(&mut self, key: UnitKey) {
        let Some(room) = self.rooms.remove(&key) else {
            return;
        };
        if let Some(list) = self.lists.get_mut(&room) {
            list.retain(|&k| k != key);
            if list.is_empty() {
                self.lists.remove(&room);
            }
        }
    }

    /// The room recache (`0x0064FAD0`, rule 6): leave the old room, then
    /// insert at the head of `room`'s list when it is not none (rule 2).
    pub fn place(&mut self, key: UnitKey, room: Option<DrlgRoomId>) {
        self.leave(key);
        if let Some(room) = room {
            self.lists.entry(room).or_default().insert(0, key);
            self.rooms.insert(key, room);
        }
    }

    /// The client room free (`0x0061A840`, rule 6 table): each unit of
    /// the room leaves it. Returns them in list order.
    pub fn free_room(&mut self, room: DrlgRoomId) -> Vec<UnitKey> {
        let units = self.lists.remove(&room).unwrap_or_default();
        for k in &units {
            self.rooms.remove(k);
        }
        units
    }

    /// The draw's Y sort written back (rule 7): `order` is the room's list
    /// after the stable sort the draw ran on it. Refused (`false`, nothing
    /// changed) unless it holds exactly the list's units.
    pub fn set_order(&mut self, room: DrlgRoomId, order: &[UnitKey]) -> bool {
        let Some(list) = self.lists.get_mut(&room) else {
            return order.is_empty();
        };
        let mut a = list.clone();
        let mut b = order.to_vec();
        a.sort();
        b.sort();
        if a != b {
            return false;
        }
        list.copy_from_slice(order);
        true
    }
}

/// The play preview's walk prediction of the local player as the
/// position check sees it ([`ClientWorld::local_walk`]). d2rs-own,
/// unverified. PROVISIONAL (`client/model.md` OQ2; REC-51, REC-277).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalWalk {
    /// The model position when the cell was recorded: a placement since
    /// then wins over the cell.
    pub at: (u16, u16),
    /// The predicted precise (16.16) position.
    pub pos: (u32, u32),
    /// The mode the prediction moves the player in (2 walk, 3 run);
    /// `None`: standing, the model's mode stands.
    pub mode: Option<u32>,
}

impl LocalWalk {
    /// The predicted sub-tile: the high 16 bits of [`Self::pos`]
    /// (`sim/path-placement.md` §1 r2).
    pub fn cell(&self) -> (u16, u16) {
        ((self.pos.0 >> 16) as u16, (self.pos.1 >> 16) as u16)
    }
}

/// The client world model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientWorld {
    /// Session messages seen so far (count, never reset).
    pub session_total: u64,
    /// The last [`SESSION_LOG_CAP`] of them, oldest first.
    pub session_log: Vec<SessionMark>,
    /// Bridge frames run (`bridge.md` §5 rule 3).
    pub frames: u64,
    /// The item tables the streams are decoded with
    /// (`bridge::item_lists`).
    pub item_tables: super::item_lists::ItemTablesRef,
    /// d2rs-own, unverified: S→C 0x9C action 0x0B records received (the
    /// store items a trade open shows, `world/vendors.md` §4 step 3).
    pub store_serial: u32,
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
    /// `connected` `[0x007A0618]` (`model.md` §7 r10): 0xAF sets, 0xB0 clears.
    pub connected: bool,
    /// The ping state (`model.md` §7 r11), written by 0x8F.
    pub ping: PingState,
    /// Palette level of pet monsters set by the 0x75 pet pass (render
    /// state, `msg-units.md` §8 r10; PROVISIONAL).
    pub pet_palette: BTreeMap<UnitKey, u8>,
    /// The belt column-ready bytes `[0x007BEFB0 + c]`
    /// (`msg-stats-items.md` §2 r6), written only by 0x9C 0x0E, 0x0F and
    /// 0x15 mode 2.
    pub belt_ready: [bool; 4],
    pub rooms_in_sight: Vec<RoomSight>,
    /// C→S messages the client sends on its own (§6 rule 8, §7 rule 3),
    /// until the bridge hands them to its send path. An empty entry is a
    /// reserved slot ([`DIALOG_REPLY_SLOT`]): nothing after it is sent
    /// until the UI's answer fills or clears it.
    pub outgoing: Vec<Vec<u8>>,
    /// The play preview's own-walk cell of the local player, with the
    /// model position it was recorded against ([`Self::set_local_walk`],
    /// read by [`Self::predicted`]). `None`: no prediction (the strict
    /// path, synthetic data without speeds).
    pub local_walk: Option<LocalWalk>,
    /// 0x3F's use-item cursor.
    pub use_cursor: Option<UseCursor>,
    /// The pet list, newest first (§14 rule 5).
    pub pets: Vec<PetRecord>,
    /// S→C 0x03 act loads handled (count, never reset): each one replaces
    /// the client act through `0x0044E100`, which sets the post-draw clear
    /// counter (`render/composition.md` §3 step 4).
    pub act_loads: u64,
    /// The act whose palette is loaded (§11 rules 2, 4): the act of 0x03,
    /// replaced by the Levels `Pal` of the new level on a room change.
    pub palette_act: Option<u8>,
    /// The client DRLG act (`[0x007A0634]`, §12 rule 1): built by 0x03
    /// from [`ModelInputs::drlg`], rooms set in sight by 0x07 / 0x08.
    /// `None` before 0x03 or without a DRLG source.
    pub drlg: Option<ClientDrlg>,
    /// The active rooms of the client act in list order (§12 rules 1–2),
    /// derived from `drlg` and refreshed by the handlers that change it
    /// (0x03, 0x07, 0x08). `None`: no client DRLG (no DRLG source: the
    /// headless configuration of `ModelInputs::default`), so a placement
    /// is taken as in a room and the level is unknown.
    pub active_rooms: Option<Vec<ActiveRoom>>,
    /// The client room unit lists (`sim/unit-order.md` §5 rules 6–8).
    pub room_units: RoomUnits,
    /// The client's light list (`render/lighting.md` §6.3). Its records
    /// are created by unit code no model rule runs yet; the act room
    /// callback (`rooms.md` §5 rule 9) runs over it for every new active
    /// room.
    pub lights: LightList,
    /// `[0x007A0498]`: client updates that ran the DRLG part
    /// (`rooms.md` §4.6, last paragraph: += 1 first, level free on every
    /// multiple of 13).
    pub drlg_updates: u32,
    /// The allied count (+0x28) of each client room as stat 172 changes
    /// it (`client/stat-lists.md` §3 r2; `sim/unit-order.md` §5 r2).
    pub room_allied: BTreeMap<DrlgRoomId, i32>,
    /// The environment record of the client act (act +0x04,
    /// `render/lighting.md` §9.1, §9.2 r4): created with the act by 0x03,
    /// set by 0x53.
    pub environment: Option<Environment>,
    /// The day-period cache `[0x007A6A74]` of the 0x53 object refresh
    /// (`render/lighting.md` §9.2 r4.4); a zero-initialised global.
    pub env_period_cache: i32,
    /// The eclipse pending flag `[0x007A060E]` (`render/lighting.md`
    /// §9.2 r3): set by 0x5D without a client act.
    pub eclipse_pending: bool,
    /// The skill-tree flag `[0x007C0C3C]`: `Some(0)` once 0x21 cleared it
    /// (`msg-skills.md` §4 rule 2); no other model rule writes it
    /// (`msg-skills.md` open question 3).
    pub skill_tree_flag: Option<u32>,
    /// The scripted ambient override globals (`render/lighting.md` §10),
    /// written by S→C 0x89 (§10 r4).
    pub overrides: Overrides,
    /// The active player roster `[0x007BB5C0]`, head first
    /// (`msg-units.md` §8 r1, r9).
    pub roster: Vec<RosterRecord>,
    /// The inactive roster `[0x007BB5C4]`, head first.
    pub roster_inactive: Vec<RosterRecord>,
    /// The active weapon set `[0x007BCC4C]` (0 / 1) of the local player
    /// (`msg-stats-items.md` §5 r6).
    pub weapon_set: u8,
    /// The runtime item table `[0x0096CA9C]` as S→C 0xA6 writes it
    /// (`msg-stats-items.md` §5 r7): entries of 0x120 bytes. Entries
    /// built at load (`0x006394A0`) are not held (open question 7).
    pub item_table_ext: Vec<Vec<u8>>,
    /// Set C and the client latches of the object functions
    /// (`world/objects-client.md` §27, `model.md` §2 rule 1).
    pub objclient: ClientObjects,
    /// Units freed since the receive path last moved them out, in free
    /// order: each becomes one `UnitFreed` output (`client/bridge.md` §10
    /// r3.1 (a)).
    pub freed: Vec<UnitKey>,
}

impl ClientWorld {
    /// Records a session message for the loading flow.
    pub fn mark_session(&mut self, m: SessionMark) {
        self.session_total += 1;
        self.session_log.push(m);
        if self.session_log.len() > SESSION_LOG_CAP {
            self.session_log.remove(0);
        }
    }

    /// The local player unit, if the model has one.
    pub fn local(&self) -> Option<&ClientUnit> {
        self.units.get(&self.local_player?)
    }

    /// Records the play preview's predicted precise position of the local
    /// player and the mode it moves in ([`Self::local_walk`]); `None`
    /// clears it.
    pub fn set_local_walk(&mut self, pos: Option<(u32, u32)>, mode: Option<u32>) {
        let at = self.local().and_then(|u| u.position);
        self.local_walk = at.zip(pos).map(|(at, pos)| LocalWalk { at, pos, mode });
    }

    /// The local player `unit`'s walk prediction, while the model
    /// position is still the one it was recorded against (a placement
    /// since then wins). The position check's rule 8 reads it
    /// (`bridge::check`, REC-277).
    pub fn predicted(&self, unit: &ClientUnit) -> Option<LocalWalk> {
        self.local_walk
            .filter(|w| self.local_player == Some(unit.key) && unit.position == Some(w.at))
    }

    /// The local player's own precise (16.16) position this frame
    /// (`seams/movement-prediction.md` §2.9 r1–r2): the walk prediction
    /// while it stands ([`Self::predicted`]), else the model cell's
    /// centre (`client/model.md` §3 r3). Every reader of "the local
    /// player's position" between placements reads this one value: the
    /// draw, the click and hover cameras, labels, the automap, the near
    /// rooms and the interaction distances (`seams/world-screen.md`
    /// §2.2). `None`: no local player or no position.
    pub fn local_position(&self) -> Option<(u32, u32)> {
        let p = self.local()?;
        if let Some(w) = self.predicted(p) {
            return Some(w.pos);
        }
        let (x, y) = p.position?;
        Some(((u32::from(x) << 16) | 0x8000, (u32::from(y) << 16) | 0x8000))
    }

    /// The sub-tile of [`Self::local_position`].
    pub fn local_cell(&self) -> Option<(u16, u16)> {
        self.local_position()
            .map(|(x, y)| ((x >> 16) as u16, (y >> 16) as u16))
    }

    /// Adds `unit` (§2 rule 4): an existing unit with the same key is
    /// removed first (rule 5: its queue is dropped, and it stops being the
    /// local player).
    pub fn add(&mut self, unit: ClientUnit) {
        self.remove(unit.key);
        self.units.insert(unit.key, unit);
    }

    /// Removes the unit `key` (§2 rule 5): its queue is dropped
    /// unapplied; the local player is cleared if it was that unit; the
    /// free is recorded for its `UnitFreed` output (`client/bridge.md`
    /// §10 r3.1). A key not in the set: nothing.
    pub fn remove(&mut self, key: UnitKey) -> Option<ClientUnit> {
        let unit = self.units.remove(&key)?;
        self.freed.push(key);
        // The unit free leaves the room list (`unit-order.md` §5 rule 6).
        self.room_units.leave(key);
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
        self.unit_room(self.local_player?)
    }

    /// Room of a point (`0x00465420`, §2 rule 7, §12 rule 2) for a point
    /// other than (0, 0): the lookup of [`ClientWorld::room_from`] from
    /// the local player's room. `None` when no room is found (fatal 0x13C
    /// for the caller) or with no client DRLG.
    pub fn room_at(&self, x: u16, y: u16) -> Option<ActiveRoom> {
        self.room_from(self.local_room(), x, y)
    }

    /// (a) The cell lookup from `start` (`sim/path-placement.md` §4 rule
    /// 1: that room if it contains the point, else the first room of its
    /// adjacency array that does), then (b) the act lookup over the
    /// active rooms in list order (§12 rule 2). `None` when neither finds
    /// one or with no client DRLG.
    pub fn room_from(&self, start: Option<&ActiveRoom>, x: u16, y: u16) -> Option<ActiveRoom> {
        let rooms = self.active_rooms.as_deref()?;
        let (x, y) = (i32::from(x), i32::from(y));
        if let Some(found) = start.and_then(|own| self.cell_lookup(own, x, y)) {
            return Some(found);
        }
        room_of_point(rooms, x, y).copied()
    }

    /// The play preview's own-walk room recache (decision D2): the local
    /// player moved by the client's walk prediction to sub-tile `(x, y)`
    /// is linked to the room of that point (§12 rule 2 from its current
    /// room, or the act lookup when it has none) through the room recache
    /// of `sim/unit-order.md` §5 rule 6 (`0x0064FAD0`), as the 1.14d
    /// client's own path step does (§3 rule 3). The model's `position`
    /// is not written (the prediction keeps it). No room at the point
    /// (not in sight yet) or the same room: unchanged, `false`.
    ///
    /// d2rs-own, unverified. PROVISIONAL (open question 2; REC-51).
    pub fn recache_local_room(&mut self, x: u16, y: u16) -> bool {
        let Some(key) = self.local_player else {
            return false;
        };
        if !self.units.contains_key(&key) {
            return false;
        }
        let current = self.local_room().copied();
        let Some(found) = self.room_from(current.as_ref(), x, y) else {
            return false;
        };
        if current.is_some_and(|c| c.room == found.room) {
            return false;
        }
        self.room_units.place(key, Some(found.room));
        true
    }

    /// The room of a unit: the active room whose unit list holds it
    /// (`sim/unit-order.md` §5 rule 6, the client's only unit → room
    /// link); `None` when it is in no list or with no client DRLG.
    pub fn unit_room(&self, key: UnitKey) -> Option<&ActiveRoom> {
        let room = self.room_units.room_of(key)?;
        self.active_rooms
            .as_deref()?
            .iter()
            .find(|r| r.room == room)
    }

    /// The client active-room free `0x0061A840` (`drlg/rooms.md` §8 r4,
    /// `client/model.md` §16 r2): each unit linked in the room leaves it;
    /// flags-2 0x20 (server units: no flag 0x400000) and flag 0x800000
    /// are set on it.
    pub fn free_active_room(&mut self, room: DrlgRoomId) {
        for k in self.room_units.free_room(room) {
            if let Some(u) = self.units.get_mut(&k) {
                u.flag_ex |= 0x20;
                u.room_freed = true;
            }
        }
    }

    /// Refreshes [`ClientWorld::active_rooms`] from the client DRLG after
    /// a DRLG change: the units of a room that is no longer active leave
    /// its list (the client room free, `sim/unit-order.md` §5 rule 6),
    /// then the act room callback runs for each new active room in
    /// creation order (`drlg/rooms.md` §5 rule 9, `render/lighting.md`
    /// §6.4: the light cache). The callbacks run after the whole change:
    /// the callback reads only the owners' rooms (unchanged by a DRLG
    /// change) and the cell lookup from them, whose answer for the new
    /// room is the same once the change ends (rooms do not overlap, and
    /// a set-in-sight or a timed build removes no active room).
    pub fn refresh_active_rooms(&mut self) -> Result<(), LightError> {
        let old = self.active_rooms.take().unwrap_or_default();
        self.active_rooms = self.drlg.as_ref().map(ClientDrlg::active_rooms);
        let gone: Vec<DrlgRoomId> = {
            let now = self.active_rooms.as_deref().unwrap_or(&[]);
            old.iter()
                .map(|r| r.room)
                .filter(|&r| !now.iter().any(|n| n.room == r))
                .collect()
        };
        // `drlg/rooms.md` §8 r4: each unit of a freed room (server units:
        // no flag 0x400000) gets flags-2 0x20, then flag 0x800000, then
        // leaves the room.
        for room in gone {
            self.free_active_room(room);
        }
        let created = self
            .drlg
            .as_mut()
            .map_or_else(Vec::new, ClientDrlg::take_created);
        let mut lights = std::mem::take(&mut self.lights);
        let mut result = Ok(());
        for room in created {
            if let Err(e) = lights.room_created(light_room(room), &*self) {
                result = Err(e);
                break;
            }
        }
        self.lights = lights;
        result
    }

    /// The cell lookup `0x00463740(room, x, y)` (`sim/path-placement.md`
    /// §4 rule 1): `start` itself if it contains the point, else the first
    /// room of its adjacency array that does; none otherwise.
    pub fn cell_lookup(&self, start: &ActiveRoom, x: i32, y: i32) -> Option<ActiveRoom> {
        let rooms = self.active_rooms.as_deref()?;
        if start.contains(x, y) {
            return Some(*start);
        }
        let adjacency = self
            .drlg
            .as_ref()
            .map_or_else(Vec::new, |d| d.adjacency(start.room));
        adjacency
            .iter()
            .filter_map(|&n| rooms.iter().find(|r| r.room == n))
            .find(|r| r.contains(x, y))
            .copied()
    }

    /// `base(unit, stat, layer)` (`0x006253B0`, `client/stat-lists.md`
    /// §1 rule 3): the unit's base value. The model holds the layer-0
    /// base array (`ClientUnit::stats`); every base write is layer 0, so
    /// another layer has no entry (0). A unit not in S reads 0 (a null
    /// unit, `sim/stats.md` §4.2).
    pub fn base(&self, key: UnitKey, stat: u16, layer: u16) -> i32 {
        match self.units.get(&key) {
            Some(u) if layer == 0 => u.stat(stat),
            _ => 0,
        }
    }

    /// `total(unit, stat, layer)` (`0x00625480`, `client/stat-lists.md`
    /// §1 rule 3): the full array of the unit's list: the base plus the
    /// lists attached to the unit (§1 rule 2: the state lists of 0xA7–0xAA
    /// and the passive-state lists, `ClientUnit::state_lists`, each
    /// propagating its stats into the owner's full array,
    /// `sim/stat-lists.md` §8.1) and the lists of the attached items
    /// (§2, `bridge::item_lists`).
    pub fn total(&self, key: UnitKey, stat: u16, layer: u16) -> i32 {
        let Some(u) = self.units.get(&key) else {
            return 0;
        };
        u.state_lists
            .values()
            .filter_map(|l| l.get(&(stat, layer)))
            .fold(self.base(key, stat, layer), |a, &v| a.wrapping_add(v))
            .wrapping_add(self.item_lists_total(key, stat, layer))
    }

    /// The roster lookup `0x004792E0(GUID)` (`msg-units.md` §8 r2):
    /// GUID −1 → none; else the first active record with that GUID or
    /// holding it in its corpse list.
    pub fn roster_find(&self, guid: u32) -> Option<usize> {
        if guid == u32::MAX {
            return None;
        }
        self.roster
            .iter()
            .position(|r| r.guid == guid || r.corpses.contains(&guid))
    }

    /// The local player's level (§11 rules 3, 5): the level id of its
    /// room; none while it has no room.
    pub fn player_level(&self) -> Option<u16> {
        self.local_room().map(|r| r.level)
    }
}

/// The light code's room handle of a DRLG room (one active room per
/// DRLG room at a time).
pub fn light_room(room: DrlgRoomId) -> RoomId {
    room.0
}

/// The client world as the light code's new-room rule reads it
/// (`render/lighting.md` §6.4). The model holds set S only, so an owner
/// marked client-only (set C) is not found.
impl LightRooms for ClientWorld {
    fn owner_position(&self, owner: &Owner) -> Option<(i32, i32)> {
        let (x, y) = self.light_owner(owner)?.cell();
        let centre = |c: u16| (i32::from(c) << 16) | 0x8000;
        Some((centre(x), centre(y)))
    }

    fn owner_subtile(&self, owner: &Owner) -> Option<(i32, i32)> {
        let (x, y) = self.light_owner(owner)?.cell();
        Some((i32::from(x), i32::from(y)))
    }

    fn owner_room(&self, owner: &Owner) -> Option<RoomId> {
        let u = self.light_owner(owner)?;
        self.room_units.room_of(u.key).map(light_room)
    }

    fn cell_room(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId> {
        let start = self
            .active_rooms
            .as_deref()?
            .iter()
            .find(|r| light_room(r.room) == room)?;
        self.cell_lookup(start, x, y).map(|r| light_room(r.room))
    }
}

impl ClientWorld {
    /// The owner lookup of `render/lighting.md` §6.4 rule 1 over set S.
    fn light_owner(&self, owner: &Owner) -> Option<&ClientUnit> {
        if owner.client_only {
            return None;
        }
        let t = u8::try_from(owner.unit_type).ok()?;
        self.units.get(&UnitKey::new(t, owner.guid))
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
/// (`0x004DBF20`, model §6 rule 6, §13): a render seam, taken as an
/// input (§13 r6). A closure so the render side can answer from its own
/// state (camera, COF and cel stores: `world_view::visibility`); the
/// bridge only calls it.
#[derive(Clone)]
pub struct VisibleFn(Arc<Visible>);

/// The closure behind a [`VisibleFn`].
type Visible = dyn Fn(&ClientUnit, i32, i32) -> bool + Send + Sync;

impl VisibleFn {
    pub fn new(f: impl Fn(&ClientUnit, i32, i32) -> bool + Send + Sync + 'static) -> Self {
        VisibleFn(Arc::new(f))
    }

    /// `visible(U, a, b)` (§13), (a, b) in client pixel space.
    pub fn visible(&self, unit: &ClientUnit, a: i32, b: i32) -> bool {
        (self.0)(unit, a, b)
    }
}

impl std::fmt::Debug for VisibleFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VisibleFn(..)")
    }
}

/// One `monstats` row as 0xAC reads it (`msg-units.md` §1.2), with the
/// flags 0x28 reads (`msg-ui.md` §16 r4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonsterClass {
    /// `monstats2` choice count of each of the 16 components.
    pub components: [u8; 16],
    /// `monstats` flag bit 8 `npc`.
    pub npc: bool,
    /// `monstats` flag bit 9 `interact` (`0x00457490(class, 9)`).
    pub interact: bool,
    /// The columns of the monster set-up `0x004AE8D0` (`msg-units.md`
    /// §1.2 r6); `None`: the tables do not give them (the set-up's table
    /// parts are not run).
    pub setup: Option<MonsterSetup>,
}

/// The `monstats` / `monstats2` columns of the monster set-up
/// (`msg-units.md` §1.2 r6), by difficulty where the table has three.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonsterSetup {
    /// `Level`, `Level(N)`, `Level(H)` (+0xAA + 2d).
    pub level: [u16; 3],
    /// `ResDm`, `ResMa`, `ResFi`, `ResLi`, `ResCo`, `ResPo` by difficulty
    /// (stats 36, 37, 39, 41, 43, 45).
    pub res: [[u16; 3]; 6],
    /// `Velocity` (+0x32).
    pub velocity: u16,
    /// `Align` (classic scaling, `monsters/init.md` §13).
    pub align: u8,
    /// `Skill1`…`Skill8` (+0x170 + 2i, signed), `Sk1lvl`… (+0x198 + i),
    /// `Sk1mode`… (+0x180 + i).
    pub skills: [(i16, u8, u8); 8],
    /// `monstats2` `isSel` (byte +4 bit 3), `shadow` (byte +5 bit 6),
    /// `isAtt` (byte +5 bit 1).
    pub is_sel: bool,
    pub shadow: bool,
    pub is_att: bool,
}

/// One `objects.txt` row as the shrine requests read it (`model.md`
/// §15 rule 1, step 4.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectRow {
    /// `SubClass` (+0x167): bit 0 = shrine.
    pub subclass: u8,
    /// `ShrineFunction` (+0x16F).
    pub shrine_function: u8,
    /// `EnvEffect` (+0x139): the day/night refresh changes the object
    /// (`render/lighting.md` OQ 11).
    pub env_effect: bool,
    /// `Lit0`…`Lit7` (+0x110 + mode): light radius × 2.
    pub lit: [u8; 8],
    /// `Selectable0`…`Selectable7`.
    pub selectable: [bool; 8],
    /// `Red`, `Green`, `Blue`: the light color.
    pub rgb: (u8, u8, u8),
}

/// One `states` row as the state messages read it
/// (`client/stat-lists.md` §3 r3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateRow {
    /// The flag `[0x006CE284]` (+0x14): on a dead unit, state on only
    /// sets the bit.
    pub dead_bit_only: bool,
    /// The flag `[0x006CE278]`: state on keeps an existing list.
    pub keep_list: bool,
}

/// One `skilldesc` row as 0x93 reads it (`msg-skills.md` §9 r3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillDescRow {
    /// `skillpage` (+2, read signed).
    pub page: i8,
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

/// The unit-message rows the app loads from the user's tables
/// ([`crate::bridge::Bridge::set_unit_rows`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitRows {
    pub monsters: Vec<Option<MonsterClass>>,
    /// `difficultylevels` `MonsterSkillBonus` by difficulty.
    pub monster_skill_bonus: [i32; 3],
    pub stats: Vec<StatSend>,
    pub objects: Vec<ObjectRow>,
    pub shrines: Vec<u8>,
}

impl MonsterClass {
    /// The row of a `monstats` class from its `monstats2` record bytes
    /// (`msg-units.md` §1.2 r7): the choice count of component i is the
    /// u8 at record +0x15 + i (the `HDv` … `S8v` columns); `None` when
    /// the record is shorter.
    pub fn from_record(monstats2: &[u8], npc: bool, interact: bool) -> Option<Self> {
        let mut components = [0u8; 16];
        components.copy_from_slice(monstats2.get(0x15..0x25)?);
        Some(MonsterClass {
            components,
            npc,
            interact,
            setup: None,
        })
    }
}

/// The tables the message rules read (`msg-units.md` Inputs). The
/// `monstats2` choice counts are record +0x15 + i (§1.2 r7,
/// [`MonsterClass::from_record`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientTables {
    /// One entry per `monstats` row: `None` = no `monstats2` row.
    pub monsters: Vec<Option<MonsterClass>>,
    /// The act level bonus of the monster skills by difficulty
    /// (`0x00611D30(d)` +0x10, `difficultylevels`; `msg-units.md` §1.2
    /// r6.8).
    pub monster_skill_bonus: [i32; 3],
    /// One entry per `itemstatcost` row.
    pub stats: Vec<StatSend>,
    /// One entry per `Levels.txt` row, by level id (§11 rule 4).
    pub levels: Vec<LevelRow>,
    /// One entry per `skills` row, by skill id (`msg-skills.md` Inputs);
    /// the skill count is the row count.
    pub skills: Vec<SkillRow>,
    /// One entry per `skilldesc` row (`msg-skills.md` §9).
    pub skilldesc: Vec<SkillDescRow>,
    /// One entry per `objects.txt` row, by class (`model.md` §15).
    pub objects: Vec<ObjectRow>,
    /// The `Code` byte (+0) of each `shrines.txt` row, by index
    /// (`msg-units.md` §1.3 r3: table `[0x0096D468]`, count
    /// `[0x0096D46C]`).
    pub shrines: Vec<u8>,
    /// One entry per `states` row, by state id
    /// (`client/stat-lists.md` §3).
    pub states: Vec<StateRow>,
    /// The overlay count (data tables +0xBC0, `msg-units.md` §7 r2).
    pub overlay_count: u32,
    /// One entry per `charstats` row, by class: its `Skill 1`…`Skill 10`
    /// (record +0xAE, i16 as u16; `msg-skills.md` §2 rule 8).
    pub class_skills: Vec<[u16; 10]>,
}

/// The `skills` fields the client skill list reads (`msg-skills.md`
/// Inputs, §2; `skills/levels.md` §6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillRow {
    /// +0x10 `anim`.
    pub anim: u8,
    /// +0x11 `monanim`.
    pub monanim: u8,
    /// +0x94 `passivestate` (read signed; > 0 = a passive state).
    pub passivestate: u16,
    /// `maxlvl` (u16 at 300, read signed).
    pub maxlvl: u16,
    /// `charclass` (i8, +0x0C; `skills/levels.md` §1).
    pub charclass: i8,
    /// `srvdofunc` (+0x2E, read signed; `msg-skills.md` §10).
    pub srvdofunc: i16,
    /// `enhanceable` (bit 17; `msg-skills.md` §9).
    pub enhanceable: bool,
    /// `skilldesc` (+0x194): the `skilldesc` row.
    pub skilldesc: u16,
    /// `EType` (+0x1DC).
    pub etype: u8,
    /// `range` (+0x14, the `@range` index: 0 none, 1 h2h, 2 rng, 3 both,
    /// 4 loc; `ui/controls.md` §6 r4, `skills/use.md` §3 r6).
    pub range: u8,
    /// The flag columns `ui/controls.md` §6 r8 reads, by `skills.txt`
    /// bit (`controls::click::skill_flag`).
    pub flags: u32,
}

/// The `Levels.txt` fields the client reads of the player's level
/// (§11 rules 3–4; `audio/environment.md` §1 r2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelRow {
    /// `Pal` (+0x02): the palette act of the room change (§11 rule 4).
    pub pal: u8,
    /// `Act` (+0x03).
    pub act: u8,
    /// `BlankScreen` (record +0x218, `render/composition.md` §3 step 2).
    pub blank_screen: bool,
    /// `SoundEnv`: the `soundenviron` row of the level
    /// (`audio/environment.md` §1 r2).
    pub sound_env: u8,
    /// `DrawEdges` (+9, `render/draw-order.md` §6 r2).
    pub draw_edges: bool,
    /// `Rain` (+0x05, `render/draw-order-2.md` §11.8).
    pub rain: bool,
    /// `Mud` (+0x06, `render/draw-order-2.md` §11.5).
    pub mud: bool,
}

/// Inputs of the message rules that are not model state.
#[derive(Clone, Debug, Default)]
pub struct ModelInputs {
    pub tables: ClientTables,
    /// `None`: the check (§6 rule 6) refuses when it needs visibility.
    pub visible: Option<VisibleFn>,
    /// What the client DRLG of 0x03 is built from (§12 rule 1). `None`:
    /// no client DRLG is built (`ClientWorld::drlg` stays `None`).
    pub drlg: Option<DrlgSource>,
    /// The `d2exp.mpq` check `0x00408F20` (`msg-stats-items.md` §5 r6).
    pub expansion_installed: bool,
    /// `0x00410A80()`: wall-clock seconds (`render/lighting.md` §10 r4:
    /// a `time()` base plus elapsed `GetTickCount` / 1000; client-only, a
    /// host input). 0 when the host gives none.
    pub wall_seconds: Option<fn() -> i32>,
    /// `GetTickCount()` of this update, wrapping milliseconds (§5 rule 2;
    /// `world/objects-client.md` §25 r6): the live client passes the host
    /// clock, tests and replays a scripted value.
    pub now: u32,
    /// What the client object functions read beside the model
    /// (`world/objects-client.md` Inputs).
    pub objclient: ObjClientInputs,
    /// The skills tables and formula buffers the passive refresh
    /// evaluates (`client/msg-skills.md` §2 r4); `None`: a passive skill
    /// is a handler error.
    pub skill_tables: Option<std::sync::Arc<super::passive::Tables>>,
}
