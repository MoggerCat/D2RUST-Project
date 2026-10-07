// Spec: specs/world/quests.md
// Spec: specs/world/quests-act1.md (§10, split out of `quests.md`)
//! The quest system: flag records (§1), quest control and records (§2),
//! game entry (§3), event dispatch (§4), the updater and its timers (§5),
//! status messages (§6), NPC dialog hooks (§7), act transitions and
//! portals (§8), helpers (§9). Act I state machines are in [`act1`], Act II's in [`act2`].
//!
//! Units, items, NPC chat, monsters and levels belong to other specs and
//! are reached through [`QuestWorld`]. Message bytes leave through
//! `QuestWorld::send`. Callbacks the spec only catalogues (Acts II–V, and
//! Act I functions it does not describe) are reported through
//! `QuestWorld::unhandled` instead of being guessed.

pub mod act1;
pub mod act2;
pub mod act3;
pub mod act4;
pub mod act5;
pub mod late;
pub mod tables;

#[cfg(test)]
mod act1_rest_misc_tests;
#[cfg(test)]
mod act1_rest_q4_tests;
#[cfg(test)]
mod act1_tests;
#[cfg(test)]
mod act3_tests;
#[cfg(test)]
mod gaps_tests;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use crate::rng::Seed;
use crate::units::{RoomId, UnitId};

pub use tables::{MessageEntry, QuestRow, QuestTables};

/// Bytes of a flag record (§1.1).
pub const FLAG_BYTES: usize = 0x60;
/// Quest slots (§1.1).
pub const SLOTS: u8 = 42;
/// Init-table rows and intro records (§2.3).
pub const TABLE_ROWS: usize = 37;
pub const INTRO_ROWS: usize = 4;

/// Slot bits (§1.2, D2MOO names).
pub mod bit {
    pub const REWARD_GRANTED: u8 = 0;
    pub const REWARD_PENDING: u8 = 1;
    pub const STARTED: u8 = 2;
    pub const LEAVE_TOWN: u8 = 3;
    pub const ENTER_AREA: u8 = 4;
    pub const CUSTOM1: u8 = 5;
    pub const UPDATE_QUEST_LOG: u8 = 12;
    pub const PRIMARY_GOAL_DONE: u8 = 13;
    pub const COMPLETED_NOW: u8 = 14;
    pub const COMPLETED_BEFORE: u8 = 15;
}

/// Event ids (§4.1).
pub mod event {
    pub const NPC_ACTIVATE: u8 = 0;
    pub const NPC_DEACTIVATE: u8 = 2;
    pub const CHANGED_LEVEL: u8 = 3;
    pub const ITEM_PICKED_UP: u8 = 4;
    pub const ITEM_DROPPED: u8 = 5;
    pub const EVENT6: u8 = 6;
    pub const MONSTER_KILLED: u8 = 8;
    pub const PLAYER_DROPPED_WITH_QUEST_ITEM: u8 = 9;
    pub const PLAYER_LEAVES_GAME: u8 = 10;
    pub const SCROLL_MESSAGE: u8 = 11;
    pub const PLAYER_STARTED_GAME: u8 = 13;
    pub const PLAYER_JOINED_GAME: u8 = 14;
}

/// NPC class ids used by the quest code.
pub mod npc {
    pub const AKARA: u16 = 148;
    pub const KASHYA: u16 = 150;
    pub const CHARSI: u16 = 154;
    pub const WARRIV1: u16 = 155;
    pub const MESHIF1: u16 = 210;
    pub const CAIN5: u16 = 265;
    pub const NAVI: u16 = 266;
    pub const TYRAEL2: u16 = 367;
}

/// Fatal asserts of the original, returned instead of aborting.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QuestError {
    #[error("flag record copy of {0} bytes (want 0x60)")]
    Size(usize),
    #[error("quest set not picked")]
    NotPicked,
    #[error("no quest record for chain {0}")]
    NoRecord(u8),
    #[error("chain {0} has no sequence function")]
    NoSequenceFn(u8),
    #[error("timer created while the updater runs")]
    TimerWhileExecuting,
    #[error("default status rule on filter {0} > 40")]
    Filter(u8),
    #[error("quest table: {0}")]
    Table(#[from] crate::world::TsvError),
    #[error("quests.tsv: {0}")]
    TableShape(&'static str),
    /// A fatal assert inside a quest callback (`quests-act1.md` §10): the
    /// function's 1.14d address.
    #[error("quest callback {0:#x}: fatal assert")]
    Fatal(u32),
}

// ------------------------------------------------------------------ §1

/// A quest flag record (§1.1): 42 slots × 16 bits, LSB first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestFlags(pub [u8; FLAG_BYTES]);

impl Default for QuestFlags {
    /// `0x0065C430`: zeroed.
    fn default() -> Self {
        Self([0; FLAG_BYTES])
    }
}

impl QuestFlags {
    fn pos(q: u8, b: u8) -> (usize, u8) {
        let n = 16 * usize::from(q) + usize::from(b);
        (n >> 3, 1 << (n & 7))
    }

    /// `0x0065C310`.
    pub fn get(&self, q: u8, b: u8) -> bool {
        let (i, m) = Self::pos(q, b);
        self.0[i] & m != 0
    }

    /// `0x0065C360`.
    pub fn set(&mut self, q: u8, b: u8) {
        let (i, m) = Self::pos(q, b);
        self.0[i] |= m;
    }

    /// `0x0065C3A0`.
    pub fn clear(&mut self, q: u8, b: u8) {
        let (i, m) = Self::pos(q, b);
        self.0[i] &= !m;
    }

    /// `0x0065C3E0`: clear bits 2..11 of slot q.
    pub fn reset_progress(&mut self, q: u8) {
        for b in 2..=11 {
            self.clear(q, b);
        }
    }

    /// Slot q's word (little-endian u16).
    pub fn word(&self, q: u8) -> u16 {
        let i = 2 * usize::from(q);
        u16::from_le_bytes([self.0[i], self.0[i + 1]])
    }

    /// `0x0065C4D0`: copy in (§1.6). With `normalize`, for every slot 0..41
    /// clear bits 13 and 14, then set bit 15 if bit 1 is set.
    pub fn copy_in(buf: &[u8], normalize: bool) -> Result<Self, QuestError> {
        let bytes: [u8; FLAG_BYTES] = buf.try_into().map_err(|_| QuestError::Size(buf.len()))?;
        let mut r = Self(bytes);
        if normalize {
            for q in 0..SLOTS {
                r.clear(q, bit::PRIMARY_GOAL_DONE);
                r.clear(q, bit::COMPLETED_NOW);
                if r.get(q, bit::REWARD_PENDING) {
                    r.set(q, bit::COMPLETED_BEFORE);
                }
            }
        }
        Ok(r)
    }

    /// `0x0065C560`.
    pub fn copy_out(&self) -> [u8; FLAG_BYTES] {
        self.0
    }
}

/// A player's quest state (player data +0x10 and +0x60, §1.4, §6.7).
/// Owned by the player unit (provider: the units group).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerQuests {
    /// Flag record per difficulty.
    pub flags: [QuestFlags; 3],
    /// NPC intro record per difficulty: NPC class ids heard.
    pub intro: [BTreeSet<u16>; 3],
}

// ------------------------------------------------------------------ §2

/// A unit's quest chain (unit +0x74, §4.6): chain ids, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestChain(pub Vec<u8>);

/// A list of up to 32 player GUIDs (§9.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuidList(pub Vec<u32>);

impl GuidList {
    /// `0x00545200`: ignores duplicates and a full list.
    pub fn add(&mut self, guid: u32) {
        if self.0.len() < 32 && !self.0.contains(&guid) {
            self.0.push(guid);
        }
    }

    /// `0x00545240`: remove by swapping with the last.
    pub fn remove(&mut self, guid: u32) {
        if let Some(i) = self.0.iter().position(|&g| g == guid) {
            self.0.swap_remove(i);
        }
    }

    /// `0x00545290`.
    pub fn contains(&self, guid: u32) -> bool {
        self.0.contains(&guid)
    }
}

/// A quest record (§2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestRecord {
    pub chain: u8,
    pub act: u8,
    /// +0x09 (`bNotIntro`).
    pub not_intro: bool,
    /// +0x0A (`bActive`).
    pub active: bool,
    /// +0x0B: status shown to clients.
    pub status: u8,
    /// +0x0C.
    pub state: u8,
    /// +0x0D.
    pub init_no: u8,
    /// +0x10.
    pub seq_id: Option<u8>,
    /// +0x14 (low byte sent in 0x5D).
    pub flags: u8,
    /// +0xE0.
    pub filter: u8,
    /// +0xE4.
    pub flag2: Option<u8>,
    /// +0x1C: player GUIDs.
    pub guids: GuidList,
    /// Bit per event id with a non-null callback (+0xA0).
    pub callbacks: u16,
    pub status_fn: Option<u32>,
    pub active_fn: Option<u32>,
    pub seq_fn: Option<u32>,
    /// NPC message table address (+0xDC).
    pub msgs: Option<u32>,
    /// Per-quest extra data (+0x18).
    pub extra: act1::Extra,
}

impl QuestRecord {
    pub fn has_callback(&self, ev: u8) -> bool {
        self.callbacks & (1 << ev) != 0
    }

    pub fn clear_callback(&mut self, ev: u8) {
        self.callbacks &= !(1 << ev);
    }
}

/// What a quest timer runs (§5). Only the callbacks the spec describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerFn {
    /// `0x00590230`: Den of Evil status 5 while state 4; returns 1.
    DenOfEvilStatus,
    /// `0x00590BF0`: Burial Grounds status 3; returns 1 (§10.5).
    BurialStatus,
    /// `0x00593260`: the Tristram Cain removal walk; returns 1 (§10.6).
    CainRemoval,
    /// `0x00592D50`: the Tristram portal at the class-17 stone; kept
    /// until created or the stone is gone (`quests-act1-rest.md` §2.3).
    TristramPortal,
    /// `0x005954C0`: Forgotten Tower status 13; returns 1 (§10.7).
    TowerStatus,
    /// `0x00596500`: Andariel's portals and status 3 (§10.8).
    AndarielPortals,
    /// `0x00596580`: chain 6's sequence timer, state 0 → 1 (§10.8).
    SlaughterOpen,
    /// An Act II timer (`world/quests-act2.md`).
    Act2(act2::Timer),
    /// An Act III timer (`quests-act3.md`).
    Act3(act3::Timer),
    /// Act IV timers (`quests-act4.md`).
    Act4(act4::Timer),
    /// Act V timers (`quests-act5.md`, `quests-act5-2.md`).
    Act5(act5::Timer),
    /// Test probe: logs through `unhandled(chain, tick)`, never removed.
    #[cfg(test)]
    Probe,
}

/// A quest timer (§5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestTimer {
    pub func: TimerFn,
    pub chain: u8,
    pub due: u32,
    pub period: u32,
}

/// The quest control (game +0x10F4, §2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestControl {
    /// Records in list order: newest first (§2.3).
    pub records: Vec<QuestRecord>,
    pub executing: bool,
    pub picked: bool,
    /// Game quest flag record (never saved).
    pub game: QuestFlags,
    /// Timer list, head first.
    pub timers: Vec<QuestTimer>,
    pub tick: u32,
    pub seed: Seed,
    /// FX byte for 0x89.
    pub fx: u8,
    /// Init-table rows (for §3 and 0x5E).
    pub rows: Vec<QuestRow>,
    /// NPC message tables.
    pub messages: Vec<MessageEntry>,
    /// Fatal asserts the callbacks reached ([`QuestError::Fatal`]), in
    /// order; the original aborts at the first.
    pub faults: Vec<QuestError>,
}

/// The seam to the rest of the game. Expected providers in brackets.
pub trait QuestWorld {
    // Game.
    fn frame(&self) -> i32;
    /// Game +0x6D.
    fn difficulty(&self) -> u8;
    /// Game +0x70.
    fn expansion(&self) -> bool;
    /// Game +0x6A.
    fn game_type(&self) -> u8;
    /// Game +0xC0: the game has an Act II (DRLG).
    fn has_act2(&self) -> bool;

    // Players and units (units group).
    /// Every player in the walk order of `0x005537D0`
    /// (`quests-act1-rest.md` §8 item 9: hash buckets 0–127, each from its
    /// head; players with state 7 skipped).
    fn players(&self) -> Vec<UnitId>;
    /// `0x00539070` / `0x00537860`: the first client's player.
    fn first_client_player(&self) -> Option<UnitId>;
    fn guid(&self, unit: UnitId) -> u32;
    fn player_by_guid(&self, guid: u32) -> Option<UnitId>;
    /// The player's quest records (player data; `None`: missing).
    fn quests(&mut self, player: UnitId) -> Option<&mut PlayerQuests>;
    /// Act of the unit's room's level (`None`: no room).
    fn unit_act(&self, unit: UnitId) -> Option<u8>;
    /// Level id of the unit's room.
    fn unit_level(&self, unit: UnitId) -> Option<u32>;
    fn player_class(&self, player: UnitId) -> u8;
    /// The unit seed (+0x20).
    fn unit_seed(&mut self, unit: UnitId) -> &mut Seed;
    fn stat(&self, unit: UnitId, stat: u16) -> i32;
    /// `0x006253B0`: the unit's base stat, layer 0 (A1Q3's level test).
    fn base_stat(&self, unit: UnitId, stat: u16) -> i32;
    /// `0x006272B0`: add to a stat.
    fn add_stat(&mut self, unit: UnitId, stat: u16, delta: i32);
    /// `0x00553380`.
    fn attach_sound(&mut self, player: UnitId, sound: u16);
    /// Player data +0x4C.
    fn player_byte_4c(&self, player: UnitId) -> u8;
    fn set_player_byte_4c(&mut self, player: UnitId, v: u8);
    /// The unit's quest chain (unit +0x74).
    fn quest_chain(&mut self, unit: UnitId) -> Option<&mut QuestChain>;
    /// What a killer or victim is (§4.4).
    fn unit_kind(&self, unit: UnitId) -> UnitKind;
    /// The monster with this GUID and its class (NPC lookup).
    fn monster_by_guid(&self, guid: u32) -> Option<(UnitId, u16)>;
    /// A monster unit's class id (NPC class), if it is a monster.
    fn monster_class(&self, unit: UnitId) -> Option<u16>;
    /// The players P with a room for which A1Q2's J3 test holds
    /// (§10.5): P's room is the unit's room, or the unit's room is in
    /// P's room's room list (`0x00619790`, `drlg/rooms.md` §10.4). Empty
    /// when the unit has no room.
    fn players_near(&self, unit: UnitId) -> Vec<UnitId>;
    /// `0x00554630` and the party list at game +0x1D2C (§10.4 I3): the
    /// members of the player's party in list order, GUID lookups that
    /// fail skipped; `None` when the party id is 0xFFFF (no party; a
    /// single player is in none, open question 7).
    fn party_members(&self, player: UnitId) -> Option<Vec<UnitId>>;

    // Messages (server transport).
    fn send(&mut self, player: UnitId, msg: &[u8]);
    /// The 0x27 text list (`world/npc.md`, `0x00661480`).
    fn send_text_list(&mut self, player: UnitId, npc: UnitId, list: &[(u16, u32)]);

    // Items (items / inventory).
    fn has_item(&self, player: UnitId, code: [u8; 4]) -> bool;
    /// `0x00628590`: an item unit's code (`None`: not an item).
    fn item_code(&self, item: UnitId) -> Option<[u8; 4]>;
    /// `0x00544160`: delete the player's item with `code`.
    fn delete_item(&mut self, player: UnitId, code: [u8; 4]);
    /// `0x005466B0` (§9.1): create, place or drop a reward item.
    fn reward_item(
        &mut self,
        player: UnitId,
        code: [u8; 4],
        level: i32,
        quality: u8,
        droppable: bool,
    ) -> Option<UnitId>;
    /// `0x00559A30`: drop an item of `code` at a unit (normal quality 2).
    fn drop_item_at(&mut self, unit: UnitId, code: [u8; 4], quality: u8) -> bool;
    /// Player inventory items with their items record `quest` byte ≠ 0,
    /// in inventory order (§4.5).
    fn quest_items(&self, player: UnitId) -> Vec<(UnitId, u8)>;

    // Levels and objects (DRLG / objects).
    /// Level 8's monster region: (spawn count, kill count, room count
    /// +0x04, populated rooms).
    fn den_region(&self) -> (u32, u32, u32, u32);
    /// `0x0061AEB0`: level id of the true tomb.
    fn true_tomb_level(&self) -> u32;
    /// `0x00545340`: free spot near the player (size, mask, radius, limit).
    fn free_spot(
        &mut self,
        player: UnitId,
        size: u32,
        mask: u32,
        radius: u32,
        limit: u32,
    ) -> Option<(i32, i32)>;
    /// `0x0056D130`: a portal object of `class` to `level` at (x, y).
    fn create_portal(&mut self, player: UnitId, x: i32, y: i32, class: u16, level: u32) -> bool;
    /// Schedule object timer event 7 (QUESTFN) at `frame` (tick).
    fn schedule_quest_event(&mut self, object: UnitId, frame: i32);
    /// An object's mode (+0x10); 0 when there is no object (§10.5).
    fn object_mode(&self, object: UnitId) -> i32;
    /// `0x00624690`: set an object's mode (objects).
    fn set_object_mode(&mut self, object: UnitId, mode: i32);
    /// `0x00552F60` with type 2: the object with this GUID and its class.
    fn object_by_guid(&self, guid: u32) -> Option<(UnitId, u16)>;
    /// `0x00579180(npc)`: the mercenary reward (NPC spec).
    fn mercenary_reward(&mut self, player: UnitId, npc: u16);

    // Act I quest seams (§10.6–§10.8; paths, rooms, monsters, objects).
    /// `0x00620870`: the unit's position and room (`None`: no room).
    fn unit_position(&self, unit: UnitId) -> Option<(i32, i32, RoomId)>;
    /// `0x00619730`: (x, y) inside the room's tile rectangle, the last
    /// row and column excluded (§10.6 step 15).
    fn room_contains(&self, room: RoomId, x: i32, y: i32) -> bool;
    /// `0x00463740`: the room holding (x, y), searched from `room`.
    fn room_at(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId>;
    /// `0x00545340` from a point: a free spot (size, mask, radius, limit).
    #[allow(clippy::too_many_arguments)]
    fn free_spot_at(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        size: u32,
        mask: u32,
        radius: u32,
        limit: u32,
    ) -> Option<(i32, i32, RoomId)>;
    /// `0x005B2F20(game, room, x, y, class, mode, r, 0)`: spawn a monster.
    #[allow(clippy::too_many_arguments)]
    fn spawn_monster(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: u8,
        r: u32,
    ) -> Option<UnitId>;
    /// Unit +0xC4 |= `flags`.
    fn or_unit_flags(&mut self, unit: UnitId, flags: u32);
    /// Every monster, in the walk order of `0x005537D0` (type 1).
    fn monsters(&self) -> Vec<UnitId>;
    /// `0x00572DC0` / `0x00573180`: the players chatting with `npc`;
    /// `None` when no chat is open with it.
    fn npc_chat_clients(&self, npc: UnitId) -> Option<Vec<UnitId>>;
    /// `0x005A7E60(monster, 0, buf)` + `0x005A7C20(game, buf, 1)`:
    /// request the monster's removal mode (`monsters/init.md`).
    fn remove_monster(&mut self, monster: UnitId);
    /// `0x00543140(game, 1, class, 0)`: drop act `act`'s stored preset
    /// of monster `class`.
    fn drop_preset_monster(&mut self, act: u8, class: u16);
    /// The first object of `class` in the rooms of `object`'s room list
    /// (§10.6 stone operate).
    fn find_object_near(&self, object: UnitId, class: u16) -> Option<UnitId>;
    /// `0x0056EDE0` then `0x0061AED0`: an object of `class` at (x, y),
    /// its room refreshed.
    fn create_object(&mut self, room: RoomId, x: i32, y: i32, class: u16) -> Option<UnitId>;
    /// `0x00640E90`: the object's animation length (>> 8 gives frames).
    fn object_anim_length(&self, object: UnitId) -> i32;
    /// `0x005417D0`: schedule object event `ev` at `frame`.
    fn schedule_object_event(&mut self, object: UnitId, ev: u8, frame: i32);
    /// `0x005456A0(player, object, msg)`: open quest message `msg`.
    fn open_quest_message(&mut self, player: UnitId, object: UnitId, msg: u16);

    // Act I remainder seams (`quests-act1-rest.md`).
    /// `0x005B2F20(game, room, x, y, class, mode, spread, flags)`: spawn a
    /// monster with spawn flags (`monsters/init.md`).
    #[allow(clippy::too_many_arguments)]
    fn spawn_monster_flags(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: u8,
        spread: i32,
        flags: u32,
    ) -> Option<UnitId>;
    /// `0x0056D130(game, owner, room, x, y, level, 0, class, exact)`: a
    /// portal object of `class` to `level`; `exact` = at (x, y) only,
    /// else a free spot is searched (`quests-act1-rest.md` §1.2, §2.3).
    #[allow(clippy::too_many_arguments)]
    fn open_portal(
        &mut self,
        owner: Option<UnitId>,
        room: RoomId,
        x: i32,
        y: i32,
        level: u32,
        class: u16,
        exact: bool,
    ) -> Option<UnitId>;
    /// `0x0056EDE0(game, owner, skill, level, class, x, y)`
    /// (`quests-act1-rest.md` §4.1): create a missile.
    #[allow(clippy::too_many_arguments)]
    fn create_missile(
        &mut self,
        owner: UnitId,
        skill: u16,
        level: u8,
        class: u16,
        x: i32,
        y: i32,
    ) -> Option<UnitId>;
    /// `0x0064A710` / `0x0064A760`: missile data +0x28 and +0x2C.
    fn set_missile_target(&mut self, missile: UnitId, a: u32, b: u32);
    /// `0x0061AED0(room, 0)` on the unit's room.
    fn refresh_room(&mut self, unit: UnitId);
    /// `0x00555230(game, 2, class, …, mode)`: allocate an object of
    /// `class` at (x, y) in `room` with `mode`.
    fn spawn_object(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: i32,
    ) -> Option<UnitId>;
    /// The save flags (client +0x0A) of the player's client
    /// (`0x005531C0`); `None`: no client.
    fn client_save_flags(&self, player: UnitId) -> Option<u16>;
    fn set_client_save_flags(&mut self, player: UnitId, flags: u16);

    // Act III quest seams (`quests-act3.md`; the Act II seams below are
    // shared with it). The default bodies report the function through
    // `unhandled` (chain 0xFF) and do nothing, until a host provides them.
    /// Game +0xC4: the game has an Act III (DRLG).
    fn has_act3(&mut self) -> bool {
        self.unhandled(0xFF, 0x005B_9A30);
        false
    }
    /// `0x005A43E0(game, room, 0, class, 1, 0, 0, 1)` (monster spec).
    fn spawn_monster_in_room(&mut self, room: RoomId, class: u16) -> Option<UnitId> {
        let _ = (room, class);
        self.unhandled(0xFF, 0x005A_43E0);
        None
    }
    /// `0x005B3090`: spawn a monster at a unit in `mode`.
    fn spawn_monster_at_unit(&mut self, unit: UnitId, class: u16, mode: u8) -> Option<UnitId> {
        let _ = (unit, class, mode);
        self.unhandled(0xFF, 0x005B_3090);
        None
    }
    /// `0x005DDFC0` then `0x005DFEE0`: kill a monster (monster spec).
    fn kill_monster(&mut self, monster: UnitId) {
        let _ = monster;
        self.unhandled(0xFF, 0x005D_DFC0);
    }
    /// `0x00619DA0`: the room covering (x, y).
    fn room_covering(&mut self, x: i32, y: i32) -> Option<RoomId> {
        let _ = (x, y);
        self.unhandled(0xFF, 0x0061_9DA0);
        None
    }
    /// A unit of type 0 (player) in `room` or a room of its room list
    /// (`0x00619790`), scanned in list order.
    fn player_in_rooms(&mut self, room: RoomId) -> bool {
        let _ = room;
        self.unhandled(0xFF, 0x0061_9790);
        false
    }
    /// `0x0059D9D0`: the sewer stairs' warp (object spec).
    fn stairs_warp(&mut self, object: UnitId, player: UnitId) {
        let _ = (object, player);
        self.unhandled(0xFF, 0x0059_D9D0);
    }
    /// The first player in the object's room unit list closer than
    /// `dist` (`0x00641530`).
    fn player_near_object(&mut self, object: UnitId, dist: i32) -> Option<UnitId> {
        let _ = (object, dist);
        self.unhandled(0xFF, 0x0064_1530);
        None
    }
    /// `0x005A0180(victim)` or `0x0063E9F0(0, victim)` (unique /
    /// champion / boss tests, monster spec).
    fn special_monster(&mut self, victim: UnitId) -> bool {
        let _ = victim;
        self.unhandled(0xFF, 0x005A_0180);
        false
    }
    /// `0x006229F0(room, x, y, mask)`: nonzero = blocked.
    fn blocked(&mut self, room: RoomId, x: i32, y: i32, mask: u32) -> bool {
        let _ = (room, x, y, mask);
        self.unhandled(0xFF, 0x0062_29F0);
        false
    }
    /// `0x0063BEF0` on the inventory: the code of the player's weapon.
    fn weapon_code(&mut self, player: UnitId) -> Option<[u8; 4]> {
        let _ = player;
        self.unhandled(0xFF, 0x0063_BEF0);
        None
    }

    /// A function the spec names but does not specify was reached; the
    /// host logs it (open questions 6–8).
    fn unhandled(&mut self, chain: u8, function: u32);

    // Act II quest seams (`world/quests-act2.md`). Each default reports
    // the original function through `unhandled` (chain 0xFF) and returns
    // the neutral value, so hosts that do not provide one yet still build.

    /// `0x005382B0`: the player's client is in act `act` (0-based).
    fn client_in_act(&mut self, player: UnitId, act: u8) -> bool {
        let _ = (player, act);
        self.unhandled(0xFF, 0x0053_82B0);
        false
    }
    /// `0x0061C450`: start the Tainted Sun on act `act` (environment).
    fn start_tainted_sun(&mut self, act: u8) {
        let _ = act;
        self.unhandled(0xFF, 0x0061_C450);
    }
    /// `0x0061C4D0`: end the Tainted Sun on Act II (environment).
    fn end_tainted_sun(&mut self) {
        self.unhandled(0xFF, 0x0061_C4D0);
    }
    /// `0x00545850(op)`: the shared quest-chest gate (object spec).
    fn quest_chest_gate(&mut self, object: UnitId, player: UnitId) -> bool {
        let _ = (object, player);
        self.unhandled(0xFF, 0x0054_5850);
        false
    }
    /// Set the object's (or monster's) drop code to `code`, then
    /// `0x00559A30(game, unit, quality, &level, 0, −1, droppable)`: one
    /// item. `level`: the value in the level variable (`None`: the spec
    /// does not name it, quests-act2 open question). Returns the item.
    fn quest_drop(
        &mut self,
        unit: UnitId,
        code: [u8; 4],
        quality: u8,
        level: Option<i32>,
        droppable: bool,
    ) -> Option<UnitId> {
        let _ = (unit, code, quality, level, droppable);
        self.unhandled(0xFF, 0x0055_9A30);
        None
    }
    /// `0x006280D0(item, 0x10)`: mark an item identified.
    fn identify_item(&mut self, item: UnitId) {
        let _ = item;
        self.unhandled(0xFF, 0x0062_80D0);
    }
    /// `0x00585B90(op, kind)`: the chest's own treasure (object spec).
    fn object_treasure(&mut self, object: UnitId, kind: u8) {
        let _ = (object, kind);
        self.unhandled(0xFF, 0x0058_5B90);
    }
    /// `0x00585970(game, object, 'gld ', 2)`: one normal gold pile.
    fn drop_gold(&mut self, object: UnitId) {
        let _ = object;
        self.unhandled(0xFF, 0x0058_5970);
    }
    /// `0x0061AED0`: set or clear a room's has-portal flag.
    fn set_room_portal(&mut self, room: RoomId, on: bool) {
        let _ = (room, on);
        self.unhandled(0xFF, 0x0061_AED0);
    }
    /// `0x00555230(game, room, x, y, class, flags 1, 0, 0)`: an object
    /// (the Act II call form; `spawn_object` is the Act I form with a mode).
    fn spawn_quest_object(&mut self, room: RoomId, x: i32, y: i32, class: u16) -> Option<UnitId> {
        let _ = (room, x, y, class);
        self.unhandled(0xFF, 0x0055_5230);
        None
    }
    /// `0x00623830`: free an object's collision.
    fn free_object_collision(&mut self, object: UnitId) {
        let _ = object;
        self.unhandled(0xFF, 0x0062_3830);
    }
    /// `0x00535060(player) == 1`: the player is busy.
    fn player_busy(&mut self, player: UnitId) -> bool {
        let _ = player;
        self.unhandled(0xFF, 0x0053_5060);
        false
    }
    /// The player's interact unit (type, GUID), if any.
    fn interact_unit(&mut self, player: UnitId) -> Option<(u8, u32)> {
        let _ = player;
        self.unhandled(0xFF, 0x0055_4120);
        None
    }
    /// `0x00554120(player, type, guid)` (`Some`) / `0x00554190` (`None`).
    fn set_interact_unit(&mut self, player: UnitId, unit: Option<(u8, u32)>) {
        let _ = (player, unit);
        self.unhandled(0xFF, 0x0055_4120);
    }
    /// `0x0053D8D0`: S→C 0x58, the orifice insert dialog.
    fn open_insert_dialog(&mut self, player: UnitId, object: UnitId) {
        let _ = (player, object);
        self.unhandled(0xFF, 0x0053_D8D0);
    }
    /// `Range` of `missiles.txt` row `row`; `None` when the table has
    /// no such row.
    fn missile_range(&mut self, row: u32) -> Option<i32> {
        let _ = row;
        self.unhandled(0xFF, 0x0059_DD80);
        None
    }
    /// `0x005678A0`: the player is trading.
    fn is_trading(&mut self, player: UnitId) -> bool {
        let _ = player;
        self.unhandled(0xFF, 0x0056_78A0);
        false
    }
    /// `0x0052E050`: remove a unit.
    fn remove_unit(&mut self, unit: UnitId) {
        let _ = unit;
        self.unhandled(0xFF, 0x0052_E050);
    }
    /// `0x00572DC0`, then `0x00573180(0, 1)` when it found an interact
    /// unit (AI spec): true when it did.
    fn npc_hold_chat(&mut self, npc: UnitId) -> bool {
        let _ = npc;
        self.unhandled(0xFF, 0x0057_2DC0);
        false
    }
    /// `0x0061B060(act, level, kind, …, 3)` then `0x0052D0F0`: a spawn
    /// location of `kind` in `level`.
    fn spawn_location(&mut self, act: u8, level: u32, kind: u8) -> Option<(i32, i32, RoomId)> {
        let _ = (act, level, kind);
        self.unhandled(0xFF, 0x0061_B060);
        None
    }
    /// `0x0064E7E0`: a free spot from a point (size, mask, radius).
    fn free_spot_near(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        size: u32,
        mask: u32,
        radius: u32,
    ) -> Option<(i32, i32, RoomId)> {
        let _ = (room, x, y, size, mask, radius);
        self.unhandled(0xFF, 0x0064_E7E0);
        None
    }
    /// `0x005DC5C0`: distance between two units.
    fn unit_distance(&mut self, a: UnitId, b: UnitId) -> i32 {
        let _ = (a, b);
        self.unhandled(0xFF, 0x005D_C5C0);
        i32::MAX
    }
    /// `0x006416D0`: a living player within `radius` of the unit.
    fn living_player_within(&mut self, unit: UnitId, radius: i32) -> bool {
        let _ = (unit, radius);
        self.unhandled(0xFF, 0x0064_16D0);
        false
    }
    /// `0x005723C0`: the player heard NPC `class`'s intro (chain 38;
    /// quests-act2 open question 5).
    fn npc_intro_heard(&mut self, player: UnitId, class: u16) -> bool {
        let _ = (player, class);
        self.unhandled(0xFF, 0x0057_23C0);
        false
    }
    /// `0x00572360`: set NPC `class`'s intro bit (chain 38).
    fn set_npc_intro(&mut self, player: UnitId, class: u16) {
        let _ = (player, class);
        self.unhandled(0xFF, 0x0057_2360);
    }

    // Act IV / V seams. Each has a default that reports the 1.14d
    // function through `unhandled` (chain 0xFE) so hosts that do not
    // provide it yet keep building.

    // -- Act IV, Fallen Angel / Hell's Forge / gossip (quests-act4.md §3, §4, §6).

    /// `0x00619DA0` on the DRLG of act `act` (0-based; Act IV's is game
    /// +0xC8): the room covering (x, y), if any (act4 §3.6, the ghost's
    /// spawn room).
    fn room_in_act_at(&mut self, act: u8, x: i32, y: i32) -> Option<RoomId> {
        let _ = (act, x, y);
        self.unhandled(0xFE, 0x0061_9DA0);
        None
    }

    /// `0x006416D0`: the distance between two units (`None`: not known;
    /// act4 §3.6 Izual's ghost, act5 §4.6 / §4.10 barbarians; paths
    /// spec).
    fn distance_between(&mut self, a: UnitId, b: UnitId) -> Option<i32> {
        let _ = (a, b);
        self.unhandled(0xFE, 0x0064_16D0);
        None
    }

    /// `0x0063BEF0` and the items record code (+0x80): the code of the
    /// weapon the player wields; `None` without an inventory or a weapon
    /// (act4 §4.6, the Hellforge's hammer test).
    fn wielded_weapon_code(&mut self, player: UnitId) -> Option<[u8; 4]> {
        let _ = player;
        self.unhandled(0xFE, 0x0063_BEF0);
        None
    }
    // -- end Act IV q1/q3 seams.

    // -- Act IV, Terror's End (quests-act4.md §5).
    /// The object's `objects.txt` `FrameCnt1` column value (record
    /// +0xDC ÷ 256), read by the seal activation `0x005B5630` for its
    /// end-animation event at f + 2·fc1 (`sim/units.md` §6.4).
    fn object_frame_count1(&mut self, object: UnitId) -> i32 {
        let _ = object;
        self.unhandled(0xFE, 0x005B_5630);
        0
    }
    /// The u16 entry `n` of the data-tables array at +0xAE0 (read
    /// elsewhere by `0x00586B30`; entries 36–38 = the seal bosses, open
    /// question 8).
    fn superunique_id(&mut self, n: u8) -> u16 {
        let _ = n;
        self.unhandled(0xFE, 0x0058_6B30);
        0
    }
    /// `0x00545C30(game, dummy, &(x, y), arg, id)`: spawn superunique
    /// `id` at (x, y) beside the dummy object (`arg` 2 here).
    fn spawn_superunique(
        &mut self,
        dummy: UnitId,
        x: i32,
        y: i32,
        arg: u32,
        id: u16,
    ) -> Option<UnitId> {
        let _ = (dummy, x, y, arg, id);
        self.unhandled(0xFE, 0x0054_5C30);
        None
    }
    /// The units of type 1 (monsters) of each active room of the Act IV
    /// DRLG (`0x0061A180(game +0xC8)`, next room +0x7C) whose level is
    /// `level`, in room-list then unit-list order (+0x74, next +0xE8,
    /// read before the unit is handled).
    fn level_monsters(&mut self, level: u32) -> Vec<UnitId> {
        let _ = level;
        self.unhandled(0xFE, 0x0061_A180);
        Vec::new()
    }
    /// `0x005541B0`: the unit is dead (≠ 0).
    fn unit_dead(&mut self, unit: UnitId) -> bool {
        let _ = unit;
        self.unhandled(0xFE, 0x0055_41B0);
        false
    }
    /// `0x006259B0`: the unit's alignment (0 = evil).
    fn alignment(&mut self, unit: UnitId) -> u32 {
        let _ = unit;
        self.unhandled(0xFE, 0x0062_59B0);
        0
    }
    /// `0x005351C0`: end the player's interaction (classic end of game).
    fn end_interaction(&mut self, player: UnitId) {
        let _ = player;
        self.unhandled(0xFE, 0x0053_51C0);
    }
    /// `0x0053AEC0(game, player, level, arg)`: level warp
    /// (`drlg/levels.md`).
    fn warp_to_level(&mut self, player: UnitId, level: u32, arg: u32) {
        let _ = (player, level, arg);
        self.unhandled(0xFE, 0x0053_AEC0);
    }
    /// `0x00530590(game, 0)`: end the game (host, open question 10).
    fn end_game(&mut self) {
        self.unhandled(0xFE, 0x0053_0590);
    }
    /// `0x0052E2A0(game)`: the host save pass (acts in game types 1 and
    /// 2 only; host, open question 10).
    fn save_pass(&mut self) {
        self.unhandled(0xFE, 0x0052_E2A0);
    }
    /// The player's client exists and `0x00535060` (busy) returns 0
    /// (`items/inventory.md`).
    fn client_idle(&mut self, player: UnitId) -> bool {
        let _ = player;
        self.unhandled(0xFE, 0x0053_5060);
        false
    }
    /// `0x0054B830(game, player, level, arg)`: act change to `level`.
    fn act_change(&mut self, player: UnitId, level: u32, arg: u32) {
        let _ = (player, level, arg);
        self.unhandled(0xFE, 0x0054_B830);
    }
    /// `0x005B4FF0`: the level's waypoint index (`0x00660E00`) set in
    /// player data +0x1C + 4·`difficulty` (`0x00660EC0`,
    /// `world/waypoints.md`).
    fn activate_waypoint(&mut self, player: UnitId, level: u32, difficulty: u8) {
        let _ = (player, level, difficulty);
        self.unhandled(0xFE, 0x005B_4FF0);
    }
    // -- end Act IV q2 seams.

    // -- Act V part 1 (quests-act5.md).
    /// `0x00555230(game, 2, class, x, y, room, f0, f1, f2)`: an object
    /// of `class` at (x, y) in `room` with the three flag arguments the
    /// spec lists ("flags 1, 1, 0"; object spec). `None`: not created.
    fn place_object(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        flags: [u8; 3],
    ) -> Option<UnitId> {
        let _ = (room, x, y, class, flags);
        self.unhandled(0xFE, 0x0055_5230);
        None
    }
    /// The units of every room in `room`'s adjacent-room list
    /// (`0x00619790`, the room itself included as the list holds it),
    /// room by room in list order, each room's unit list in order.
    fn adjacent_units(&mut self, room: RoomId) -> Vec<UnitId> {
        let _ = room;
        self.unhandled(0xFE, 0x0061_9790);
        Vec::new()
    }
    /// A unit's mode (+0x10), read inline by `0x00588040`, `0x005888D0`
    /// and `0x00588E10` (monsters), and as `0x0058D510` reads it for
    /// players; 0 when there is no unit.
    fn unit_mode(&mut self, unit: UnitId) -> i32 {
        let _ = unit;
        self.unhandled(0xFE, 0x0058_8040);
        0
    }
    /// "Critical spawn" `0x005459A0(game, x, y, room, 1, class)`
    /// (monster spec).
    fn critical_spawn(&mut self, room: RoomId, x: i32, y: i32, class: u16) -> Option<UnitId> {
        let _ = (room, x, y, class);
        self.unhandled(0xFE, 0x0054_59A0);
        None
    }
    /// "Kill in place" (`quests-act5.md` §1.1): the unit's interaction
    /// is ended, it is put in mode 12 and removed (`0x005A7E60`,
    /// `0x005A7C20`, `0x0061A270`, `0x00623830`, `0x0064C370`).
    fn kill_in_place(&mut self, unit: UnitId) {
        let _ = unit;
        self.unhandled(0xFE, 0x005A_7E60);
    }
    /// `0x0058F000` then `0x00666120`: apply a stored NPC map AI (the
    /// map-AI record `map_ai`, a handle the map-AI store passed) to the
    /// unit. False (nothing applied) when the record's +4 is 0.
    fn apply_map_ai(&mut self, unit: UnitId, map_ai: u32) -> bool {
        let _ = (unit, map_ai);
        self.unhandled(0xFE, 0x0058_F000);
        false
    }
    /// Town cleanup (`0x005893E0`, inline): Anya's interaction ends and
    /// she leaves her room without dying (monster spec).
    fn npc_leave_town(&mut self, unit: UnitId) {
        let _ = unit;
        self.unhandled(0xFE, 0x0058_93E0);
    }
    /// `0x00589340`: Nihlathak killed in town (interaction ended, path
    /// freed, AI event 2 deleted, stat 6 := 0, mode 12, refresh, unit
    /// flags |= 1; monster spec).
    fn kill_in_town(&mut self, unit: UnitId) {
        let _ = unit;
        self.unhandled(0xFE, 0x0058_9340);
    }
    /// Thaw step 1 (`0x0058AAB0`, inline): the frozen object leaves its
    /// room (object spec).
    fn object_leave_room(&mut self, object: UnitId) {
        let _ = object;
        self.unhandled(0xFE, 0x0058_AAB0);
    }
    /// `0x00558200(player, 0)`: the item level of Anya's rare item
    /// (item spec; `quests-act5.md` open question 2).
    fn quest_item_level(&mut self, player: UnitId) -> i32 {
        let _ = player;
        self.unhandled(0xFE, 0x0055_8200);
        0
    }
    /// The item's `items.txt` drop sound (record +0x124), read inline by
    /// `0x00589580`.
    fn item_drop_sound(&mut self, item: UnitId) -> i32 {
        let _ = item;
        self.unhandled(0xFE, 0x0058_9580);
        0
    }
    /// `0x006251F0` + `0x00626E10` + `0x00548520`: a new stat list on
    /// the player with stats 39, 41, 43, 45 := `v`, the four sent
    /// (`quests-act5.md` open question 3: stacking).
    fn add_resist_list(&mut self, player: UnitId, v: i32) {
        let _ = (player, v);
        self.unhandled(0xFE, 0x0062_51F0);
    }
    /// `0x0054E600`: preset spawn of superunique `superunique`'s monster
    /// at (x, y) in `room` (monster spec).
    fn preset_superunique_spawn(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        superunique: u16,
    ) -> Option<UnitId> {
        let _ = (room, x, y, superunique);
        self.unhandled(0xFE, 0x0054_E600);
        None
    }
    // -- end Act V part 1 seams.

    // -- Act V part 2 (quests-act5-2.md).
    /// `0x00660E00` / `0x00660E50` on player data +0x1C + 4·d: the
    /// player's waypoint of `level` is active (`world/waypoints.md`).
    fn waypoint_active(&mut self, player: UnitId, level: u32) -> bool {
        let _ = (player, level);
        self.unhandled(0xFE, 0x0066_0E50);
        false
    }
    /// `0x00545C30(game, unit, position, 2, superunique)` with the unit's
    /// own position: spawn the superunique there (monster spec). The same
    /// function as `spawn_superunique` with an explicit spot; one provider
    /// serves both.
    fn spawn_superunique_at_unit(&mut self, at: UnitId, superunique: u16) -> Option<UnitId> {
        let _ = (at, superunique);
        self.unhandled(0xFE, 0x0054_5C30);
        None
    }
    /// `0x0058C8D0`: missile `missile` from `from` towards `to` (flags,
    /// level; missile spec).
    fn quest_missile(&mut self, from: UnitId, to: UnitId, missile: u16, flags: u32, level: u8) {
        let _ = (from, to, missile, flags, level);
        self.unhandled(0xFE, 0x0058_C8D0);
    }
    /// `0x0058BEC0`: remove a spawned Ancient (unit state 54 →
    /// `0x005544B0`; in a room → mode 12, out of the room, collision
    /// freed; monster spec).
    fn remove_ancient(&mut self, monster: UnitId) {
        let _ = monster;
        self.unhandled(0xFE, 0x0058_BEC0);
    }
    /// `0x00611830`: the player class's maximum level.
    fn max_level(&mut self, player: UnitId) -> i32 {
        let _ = player;
        self.unhandled(0xFE, 0x0061_1830);
        0
    }
    /// `0x00611800`: the experience threshold T(level) of the player's
    /// class.
    fn experience_threshold(&mut self, player: UnitId, level: i32) -> u32 {
        let _ = (player, level);
        self.unhandled(0xFE, 0x0061_1800);
        0
    }
    /// `0x00570880`: level up (character progression spec).
    fn level_up(&mut self, player: UnitId) {
        let _ = player;
        self.unhandled(0xFE, 0x0057_0880);
    }
    /// `0x005353F0` then `0x00535430`: close the player's town portal
    /// if it is in `level`.
    fn close_town_portal(&mut self, player: UnitId, level: u32) {
        let _ = (player, level);
        self.unhandled(0xFE, 0x0053_5430);
    }
    /// `0x0059D9D0`: the stairs' warp of `object` for the player (object
    /// spec).
    fn object_stairs_warp(&mut self, player: UnitId, object: UnitId) {
        let _ = (player, object);
        self.unhandled(0xFE, 0x0059_D9D0);
    }
    /// `0x0056EDE0` with type 2: object `class` at the unit's position
    /// (`flags` the first of the three trailing arguments, then 0, 0).
    fn create_object_at(&mut self, at: UnitId, class: u16, flags: u32) -> Option<UnitId> {
        let _ = (at, class, flags);
        self.unhandled(0xFE, 0x0056_EDE0);
        None
    }
    /// `0x0056EDE0` with the missile type: missile `class` at the unit
    /// (missile spec).
    fn create_missile_at(&mut self, at: UnitId, class: u16) -> Option<UnitId> {
        let _ = (at, class);
        self.unhandled(0xFE, 0x0056_EDE0);
        None
    }
    /// `0x00538680(client, act, difficulty)`: character progression
    /// (save spec; `quests-act5-2.md` open question 2).
    fn character_progression(&mut self, player: UnitId, act: u8, difficulty: u8) {
        let _ = (player, act, difficulty);
        self.unhandled(0xFE, 0x0053_8680);
    }
    /// `0x0055B030`: a gold pile of `amount` at the unit.
    fn drop_gold_amount(&mut self, at: UnitId, amount: u32) {
        let _ = (at, amount);
        self.unhandled(0xFE, 0x0055_B030);
    }
    /// `monstats.txt` row count (datatables +0xA80), read by `0x0058E830`.
    fn monstats_rows(&mut self) -> u32 {
        self.unhandled(0xFE, 0x0058_E830);
        0
    }
    /// `monstats.txt` flags byte +0x0E bit 6 of `class` (the zoo test of
    /// `0x0058E830`; open question 4).
    fn zoo_eligible(&mut self, class: u32) -> bool {
        let _ = class;
        self.unhandled(0xFE, 0x0058_E830);
        false
    }
    // -- end Act V part 2 seams.
}

/// A unit as the kill parse sees it (§4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    Player,
    Monster {
        class: u32,
        /// `superuniques.txt` hcIdx, if a superunique.
        superunique: Option<u32>,
        /// `0x0058F090`: the owning player, if any.
        owner: Option<UnitId>,
    },
    Other,
}

/// Event arguments (§4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EventArgs {
    pub event: u8,
    pub target: Option<UnitId>,
    pub player: Option<UnitId>,
    /// Event 3: old level; event 11: NPC class.
    pub a: u32,
    /// Event 3: new level; event 11: message index.
    pub b: u32,
}

/// A text list being built for 0x27: (message index, menu).
pub type TextList = Vec<(u16, u32)>;

/// Players' current flag record.
fn flags_of<W: QuestWorld>(w: &mut W, player: UnitId) -> Option<&mut QuestFlags> {
    let d = usize::from(w.difficulty());
    w.quests(player).map(|q| &mut q.flags[d])
}

impl QuestControl {
    /// `0x00545D80` (§2.3): every record, then the quest seed from one
    /// game-seed step, then an empty game record.
    pub fn new(tables: &QuestTables, game_seed: &mut Seed) -> Result<Self, QuestError> {
        if tables.rows.len() != TABLE_ROWS + INTRO_ROWS {
            return Err(QuestError::TableShape("want 37 + 4 rows"));
        }
        let mut made = Vec::new();
        for (i, row) in tables.rows.iter().enumerate() {
            if usize::from(row.index) != i || (i < TABLE_ROWS) == row.flag.is_none() {
                return Err(QuestError::TableShape("index / flag columns"));
            }
            let intro = i >= TABLE_ROWS;
            let mut r = QuestRecord {
                chain: row.chain,
                act: row.act,
                not_intro: !intro,
                active: intro,
                status: 0,
                state: 0,
                init_no: row.init_no.unwrap_or(0),
                seq_id: row.seq_id,
                flags: 0,
                // TODO(quests OQ6): row 40's init (Act V intro) is not
                // disassembled; its filter is taken as the intros' 42.
                filter: row.filter.unwrap_or(42),
                flag2: row.flag2,
                guids: GuidList::default(),
                callbacks: row.callbacks.iter().fold(0, |m, &(ev, _)| m | 1 << ev),
                status_fn: row.status_fn,
                active_fn: row.active_fn,
                seq_fn: row.seq_fn,
                msgs: row.msgs,
                extra: act1::Extra::default(),
            };
            act1::init(&mut r);
            act2::init(&mut r);
            act3::init(&mut r);
            made.push(r);
        }
        made.reverse();
        Ok(Self {
            records: made,
            executing: false,
            picked: false,
            game: QuestFlags::default(),
            timers: Vec::new(),
            tick: 0,
            seed: game_seed.derive(),
            fx: 0,
            rows: tables.rows.clone(),
            messages: tables.messages.clone(),
            faults: Vec::new(),
        })
    }

    /// `0x00543640`: the record index of `chain`.
    pub fn find(&self, chain: u8) -> Option<usize> {
        self.records.iter().position(|r| r.chain == chain)
    }

    pub fn record(&self, chain: u8) -> Option<&QuestRecord> {
        self.find(chain).map(|i| &self.records[i])
    }

    pub fn record_mut(&mut self, chain: u8) -> Option<&mut QuestRecord> {
        self.find(chain).map(|i| &mut self.records[i])
    }

    /// `0x00543F10` (§5): prepend a timer.
    pub fn add_timer(&mut self, chain: u8, func: TimerFn, period: u32) -> Result<(), QuestError> {
        if self.executing {
            return Err(QuestError::TimerWhileExecuting);
        }
        self.timers.insert(
            0,
            QuestTimer {
                func,
                chain,
                due: self.tick.wrapping_add(period),
                period,
            },
        );
        Ok(())
    }

    /// `0x00543E10`: the updater (§5), run at tick step 8 when frame % 20
    /// = 0.
    pub fn update<W: QuestWorld>(&mut self, w: &mut W) {
        self.tick = self.tick.wrapping_add(1);
        if self.tick == u32::MAX {
            for t in &mut self.timers {
                t.due = u32::MAX - t.due;
            }
        }
        self.executing = true;
        let mut i = 0;
        while i < self.timers.len() {
            let t = self.timers[i];
            if t.due < self.tick {
                if act1::run_timer(self, w, t.func, t.chain) {
                    self.timers.remove(i);
                    continue;
                }
                self.timers[i].due = self.tick.wrapping_add(t.period);
            }
            i += 1;
        }
        self.executing = false;
    }

    // ------------------------------------------------------------ §3

    /// `0x00546270`: a player enters the game (§3). `mode` 1 for the
    /// callers `0x00532590` / `0x00569F80`, 0 for `0x005344B0` /
    /// `0x00534520` (open question 4: which one single player takes).
    pub fn player_enters<W: QuestWorld>(
        &mut self,
        w: &mut W,
        player: UnitId,
        mode: u8,
    ) -> Result<(), QuestError> {
        let args = EventArgs {
            player: Some(player),
            target: Some(player),
            ..EventArgs::default()
        };
        if self.picked {
            self.dispatch_list(w, event::PLAYER_JOINED_GAME, args, true, None);
        } else {
            self.picked = true;
            if mode == 0 {
                // The global byte `0x0073150C` is 1 in 1.14d.
                let pf = flags_of(w, player).copied().unwrap_or_default();
                for k in 0..TABLE_ROWS {
                    let row = &self.rows[k];
                    let (chain, no_set_state) = (row.chain, row.no_set_state);
                    let Some(slot) = row.flag else { continue };
                    if no_set_state == Some(false)
                        && (pf.get(slot, bit::REWARD_GRANTED)
                            || pf.get(slot, bit::COMPLETED_BEFORE))
                    {
                        let r = self.record_mut(chain).ok_or(QuestError::NoRecord(chain))?;
                        r.not_intro = false;
                        r.active = false;
                        r.status = 0;
                        self.game.set(slot, bit::COMPLETED_BEFORE);
                    }
                }
                self.dispatch_list(w, event::PLAYER_STARTED_GAME, args, true, None);
            }
            for chain in [1, 8, 18, 22, 31] {
                let r = self.record(chain).ok_or(QuestError::NoRecord(chain))?;
                if r.seq_fn.is_none() {
                    return Err(QuestError::NoSequenceFn(chain));
                }
                act1::sequence(self, w, chain);
            }
        }
        // Steps 5–8.
        let mut m = vec![0x5E];
        for row in &self.rows[..TABLE_ROWS] {
            let r = self
                .record(row.chain)
                .ok_or(QuestError::NoRecord(row.chain))?;
            m.push(u8::from(r.not_intro));
        }
        w.send(player, &m);
        send_player_flags(w, player, 6, 0);
        self.send_game_flags(w, player);
        if self.record(1).is_some_and(|r| r.not_intro && r.state >= 4) {
            w.send(player, &[0x89, 0x00]);
        }
        Ok(())
    }

    /// `0x00544520`: S→C 0x29 (97 bytes).
    pub fn send_game_flags<W: QuestWorld>(&self, w: &mut W, player: UnitId) {
        let mut m = Vec::with_capacity(97);
        m.push(0x29);
        m.extend_from_slice(&self.game.0);
        w.send(player, &m);
    }

    // ------------------------------------------------------------ §4

    /// `0x005438E0`: dispatch to all records (§4.2). `by_act`: filter by
    /// the act of `args.player`'s room. `list`: event 0's text list.
    pub fn dispatch_list<W: QuestWorld>(
        &mut self,
        w: &mut W,
        ev: u8,
        mut args: EventArgs,
        by_act: bool,
        mut list: Option<&mut TextList>,
    ) {
        args.event = ev;
        let act = if by_act {
            args.player.and_then(|p| w.unit_act(p))
        } else {
            None
        };
        // All callers pass ignore_active = 1.
        for i in 0..self.records.len() {
            let r = &self.records[i];
            if !r.has_callback(ev) || act.is_some_and(|a| a != r.act) {
                continue;
            }
            act1::callback(self, w, i, args, list.as_deref_mut(), false);
        }
    }

    /// `0x005439A0`: dispatch along the target unit's chain (§4.3).
    pub fn dispatch_chain<W: QuestWorld>(
        &mut self,
        w: &mut W,
        ev: u8,
        mut args: EventArgs,
        force: bool,
    ) {
        args.event = ev;
        let Some(target) = args.target else { return };
        let Some(chain) = w.quest_chain(target).map(|c| c.0.clone()) else {
            return;
        };
        for c in chain {
            let Some(i) = self.find(c) else { continue };
            let r = &self.records[i];
            if r.has_callback(ev) && (r.active || force) {
                act1::callback(self, w, i, args, None, force);
            }
        }
    }

    /// `0x005436B0`: add a link for `chain` to the unit (§4.6). `special`
    /// names the special case the caller found (chain 4 with an object of
    /// class 61: `0x00592F80`; chains 8 and 12: `ret 4` stubs); it runs
    /// before the link is added.
    pub fn add_link<W: QuestWorld>(
        &mut self,
        w: &mut W,
        unit: UnitId,
        chain: u8,
        special: Option<u32>,
    ) -> bool {
        if self.find(chain).is_none() {
            return false;
        }
        if let Some(f) = special {
            match chain {
                4 => act1::q4::link_object(self, w, Some(unit)),
                8 | 12 => {}
                _ => w.unhandled(chain, f),
            }
        }
        let Some(c) = w.quest_chain(unit) else {
            return false;
        };
        if c.0.contains(&chain) {
            return false;
        }
        c.0.insert(0, chain);
        true
    }

    /// `0x00545CD0`: link a new monster to its level's `Quest` chain.
    pub fn link_monster<W: QuestWorld>(&mut self, w: &mut W, unit: UnitId, level_quest: u8) {
        if level_quest != 0 {
            self.add_link(w, unit, level_quest, None);
        }
    }

    /// `0x00543A30`: kill parse (§4.4).
    pub fn monster_killed<W: QuestWorld>(
        &mut self,
        w: &mut W,
        victim: UnitId,
        killer: Option<UnitId>,
    ) {
        if w.quest_chain(victim).is_none_or(|c| c.0.is_empty()) {
            return;
        }
        let killer = match killer {
            None if w.game_type() == 3 => w.first_client_player(),
            k => k,
        };
        let player = killer.and_then(|k| match w.unit_kind(k) {
            UnitKind::Player => Some(k),
            UnitKind::Monster { owner, .. } => owner,
            UnitKind::Other => None,
        });
        let force = match w.unit_kind(victim) {
            UnitKind::Monster {
                class, superunique, ..
            } => {
                superunique.is_some_and(|s| matches!(s, 26 | 27 | 29 | 36..=39 | 42..=45 | 60))
                    || matches!(class, 242 | 243 | 391 | 544)
            }
            _ => false,
        };
        let args = EventArgs {
            target: Some(victim),
            player,
            ..EventArgs::default()
        };
        self.dispatch_chain(w, event::MONSTER_KILLED, args, force);
    }

    /// `0x00543BD0`: a player leaves (§4.5).
    pub fn player_leaves<W: QuestWorld>(&mut self, w: &mut W, player: UnitId) {
        for (item, quest) in w.quest_items(player) {
            let Some(i) = self.find(quest.wrapping_sub(1)) else {
                continue;
            };
            if self.records[i].has_callback(event::PLAYER_DROPPED_WITH_QUEST_ITEM) {
                let args = EventArgs {
                    event: event::PLAYER_DROPPED_WITH_QUEST_ITEM,
                    target: Some(item),
                    player: Some(player),
                    ..EventArgs::default()
                };
                act1::callback(self, w, i, args, None, false);
            }
        }
        let args = EventArgs {
            player: Some(player),
            target: Some(player),
            ..EventArgs::default()
        };
        self.dispatch_list(w, event::PLAYER_LEAVES_GAME, args, false, None);
    }

    /// `0x00543B90`: level change (event 3, (1, 0)).
    pub fn changed_level<W: QuestWorld>(&mut self, w: &mut W, player: UnitId, old: u32, new: u32) {
        let args = EventArgs {
            target: Some(player),
            player: Some(player),
            a: old,
            b: new,
            ..EventArgs::default()
        };
        self.dispatch_list(w, event::CHANGED_LEVEL, args, false, None);
    }

    /// `0x00543D50`: NPC chat end (event 2, (1, 1)).
    pub fn npc_deactivate<W: QuestWorld>(&mut self, w: &mut W, player: UnitId, npc: UnitId) {
        let args = EventArgs {
            target: Some(npc),
            player: Some(player),
            ..EventArgs::default()
        };
        self.dispatch_list(w, event::NPC_DEACTIVATE, args, true, None);
    }

    /// `0x00543D10`: NPC chat start (event 0, (1, 1)); quest records add
    /// their lines to `list`.
    pub fn npc_activate<W: QuestWorld>(
        &mut self,
        w: &mut W,
        player: UnitId,
        npc: UnitId,
        list: &mut TextList,
    ) {
        let args = EventArgs {
            target: Some(npc),
            player: Some(player),
            ..EventArgs::default()
        };
        self.dispatch_list(w, event::NPC_ACTIVATE, args, true, Some(list));
    }

    /// `0x00543D80` / `0x00543DB0`: item picked up (4) or dropped (5),
    /// along the item's chain, force 0 (edge case 3).
    pub fn item_event<W: QuestWorld>(&mut self, w: &mut W, ev: u8, player: UnitId, item: UnitId) {
        let args = EventArgs {
            target: Some(item),
            player: Some(player),
            ..EventArgs::default()
        };
        self.dispatch_chain(w, ev, args, false);
    }

    /// `0x00545780` (§7.2): refresh an NPC's text after a state change.
    pub fn refresh_text<W: QuestWorld>(&mut self, w: &mut W, player: UnitId, npc: UnitId) {
        let mut list = TextList::new();
        self.npc_activate(w, player, npc, &mut list);
        w.send_text_list(player, npc, &list);
        self.send_game_flags(w, player);
    }

    /// `0x00543790` (§7.1): add the table's lines for (state, npc).
    pub fn add_messages(&self, table: u32, list: &mut TextList, npc: u16, state: u8) {
        let mut entries: Vec<&MessageEntry> = self
            .messages
            .iter()
            .filter(|m| m.table == table && m.state == state && m.npc == npc)
            .collect();
        entries.sort_by_key(|m| m.slot);
        for m in entries {
            list.push((m.string, if m.menu == 1 { 0 } else { m.menu }));
        }
    }

    // ------------------------------------------------------------ §6

    /// `0x00543F90`: the default status rule (§6.1).
    pub fn default_status(r: &QuestRecord, pf: &QuestFlags) -> Result<u8, QuestError> {
        let q = r.filter;
        let (s, n, l) = (r.state, r.init_no, r.status);
        let done = pf.get(q, bit::PRIMARY_GOAL_DONE);
        let now = pf.get(q, bit::COMPLETED_NOW);
        Ok(if s < n {
            if now || (r.chain == 4 && l == 6 && !done) {
                12
            } else {
                l
            }
        } else if done {
            if q > 40 {
                return Err(QuestError::Filter(q));
            }
            l
        } else if r.chain == 4 && now {
            if s == 6 {
                12
            } else {
                l
            }
        } else if r.chain == 10 {
            if l == 4 {
                4
            } else {
                12
            }
        } else {
            12
        })
    }

    /// The status a record reports to a player: its status function, or
    /// the default rule. `None`: nothing reported.
    fn status_for<W: QuestWorld>(
        &self,
        w: &mut W,
        i: usize,
        player: UnitId,
    ) -> Result<Option<u8>, QuestError> {
        let r = &self.records[i];
        let pf = flags_of(w, player).copied().unwrap_or_default();
        match r.status_fn {
            Some(f) => Ok(act1::status_fn(self, w, i, player, &pf, f)),
            None => Self::default_status(r, &pf).map(Some),
        }
    }

    /// `0x00546040` for C→S 0x40 (§6.2): 0x28, maybe 0x50, then 0x52.
    pub fn request_quest_data<W: QuestWorld>(
        &self,
        w: &mut W,
        player: UnitId,
    ) -> Result<(), QuestError> {
        send_player_flags(w, player, 6, 0);
        let mut list = [0u8; 41];
        for i in 0..self.records.len() {
            let r = &self.records[i];
            if r.status == 0 {
                continue;
            }
            if r.chain > 40 {
                return Err(QuestError::NoRecord(r.chain));
            }
            if let Some(s) = self.status_for(w, i, player)? {
                if let Some(slot) = list.get_mut(usize::from(r.filter)) {
                    *slot = s;
                }
            }
        }
        let pf = flags_of(w, player).copied().unwrap_or_default();
        let staff = (pf.get(12, 0) || pf.get(12, 13)) && w.has_act2();
        if list[1] != 0 || list[36] != 0 || staff {
            let mut m = [0u8; 15];
            m[0] = 0x50;
            m[1..3].copy_from_slice(&1u16.to_le_bytes());
            if list[1] != 0 {
                let left = self.record(1).map_or(0, act1::den_monsters_left);
                m[3..5].copy_from_slice(&left.to_le_bytes());
            }
            if staff {
                let tomb = (w.true_tomb_level() as i32 - 66) as i16;
                m[5..7].copy_from_slice(&tomb.to_le_bytes());
            }
            if list[36] != 0 {
                let left = act5::barbarians_left(self, w);
                m[7..9].copy_from_slice(&left.to_le_bytes());
            }
            w.send(player, &m);
        }
        let mut m = vec![0x52];
        m.extend_from_slice(&list);
        w.send(player, &m);
        Ok(())
    }

    /// `0x00544190` (§6.3): S→C 0x5D for one quest to one player.
    pub fn send_status<W: QuestWorld>(
        &self,
        w: &mut W,
        player: UnitId,
        chain: u8,
    ) -> Result<(), QuestError> {
        let Some(i) = self.find(chain) else {
            return Ok(());
        };
        let r = &self.records[i];
        if w.unit_act(player) != Some(r.act) {
            return Ok(());
        }
        // TODO(quests §6.3): a status function returning false leaves the
        // status byte undefined; d2rs sends the record's status byte.
        let status = self.status_for(w, i, player)?.unwrap_or(r.status);
        let extra = match r.filter {
            1 => act1::den_monsters_left(r),
            36 => act5::barbarians_left(self, w),
            _ => 0,
        };
        let mut m = [0u8; 6];
        m[0] = 0x5D;
        m[1] = chain;
        m[2] = r.flags;
        m[3] = status;
        m[4..6].copy_from_slice(&extra.to_le_bytes());
        w.send(player, &m);
        Ok(())
    }

    /// `0x00544300` with iterate: set the status byte, then 0x5D to every
    /// player. TODO(quests OQ7): the per-quest iterate functions' flag
    /// tests are not specified; d2rs sends to every player (§10.1).
    pub fn set_status_all<W: QuestWorld>(
        &mut self,
        w: &mut W,
        chain: u8,
        status: u8,
    ) -> Result<(), QuestError> {
        let i = self.find(chain).ok_or(QuestError::NoRecord(chain))?;
        self.records[i].status = status;
        for p in w.players() {
            self.send_status(w, p, chain)?;
        }
        Ok(())
    }

    /// `0x00544590` (§6.4): S→C 0x8A when an active function wants the
    /// player to talk to `npc`.
    pub fn npc_wants_interact<W: QuestWorld>(
        &self,
        w: &mut W,
        player: UnitId,
        npc: UnitId,
        npc_class: u16,
    ) -> Result<(), QuestError> {
        if !self.picked {
            return Err(QuestError::NotPicked);
        }
        let act = w.unit_act(player);
        for i in 0..self.records.len() {
            let r = &self.records[i];
            let Some(f) = r.active_fn else { continue };
            if Some(r.act) != act {
                continue;
            }
            if act1::active_fn(self, w, i, player, npc_class, f) {
                let mut m = [0u8; 6];
                m[0] = 0x8A;
                m[1] = 1;
                m[2..6].copy_from_slice(&w.guid(npc).to_le_bytes());
                w.send(player, &m);
                return Ok(());
            }
        }
        Ok(())
    }

    /// `0x00545760` (§6.5): store b, then 0x28 and `89 b` to every player.
    pub fn unique_event<W: QuestWorld>(&mut self, w: &mut W, b: u8) {
        self.fx = b;
        for p in w.players() {
            send_player_flags(w, p, 6, 0);
            w.send(p, &[0x89, b]);
        }
    }

    // ------------------------------------------------------------ §7

    /// C→S 0x31 (`0x0054BA90` → `0x005443B0`, §7.3). Returns the result
    /// code.
    pub fn quest_message<W: QuestWorld>(&mut self, w: &mut W, player: UnitId, msg: &[u8]) -> u32 {
        if msg.len() != 9 {
            return 3;
        }
        let guid = u32::from_le_bytes(msg[1..5].try_into().expect("4 bytes"));
        let index = u16::from_le_bytes([msg[5], msg[6]]);
        let npc = if guid != u32::MAX {
            w.monster_by_guid(guid)
        } else {
            None
        };
        let args = EventArgs {
            target: npc.map(|n| n.0),
            player: Some(player),
            a: u32::from(npc.map_or(0, |n| n.1)),
            b: u32::from(index),
            ..EventArgs::default()
        };
        self.dispatch_list(w, event::SCROLL_MESSAGE, args, true, None);
        0
    }

    /// C→S 0x58 (`0x0054C9C0`, §1.7). Returns the result code.
    pub fn quest_completed<W: QuestWorld>(&self, w: &mut W, player: UnitId, msg: &[u8]) -> u32 {
        if msg.len() != 3 {
            return 3;
        }
        let q = u16::from_le_bytes([msg[1], msg[2]]);
        if q >= u16::from(SLOTS) {
            return 2;
        }
        if let Some(f) = flags_of(w, player) {
            f.set(q as u8, bit::UPDATE_QUEST_LOG);
        }
        0
    }

    // ------------------------------------------------------------ §8

    /// `0x005467E0` (§8.1): NPC act travel, before the act change.
    pub fn act_completion<W: QuestWorld>(
        &mut self,
        w: &mut W,
        player: UnitId,
        npc_class: u16,
    ) -> Result<(), QuestError> {
        match npc_class {
            npc::WARRIV1 => {
                let Some(f) = flags_of(w, player) else {
                    return Ok(());
                };
                if !f.get(7, 0) {
                    f.set(7, 0);
                    f.set(7, 13);
                    send_player_flags(w, player, 6, 0);
                    can_go_to_act(w, player, 2);
                    set_intro_flags(w, player, 0);
                }
                let f = flags_of(w, player).copied().unwrap_or_default();
                if f.get(6, 13) && f.get(6, 0) {
                    if let Some(i) = self.find(6).filter(|&i| self.records[i].not_intro) {
                        // Chain 6's callback 3 with a = 1, b = 40 (§10.8).
                        let args = EventArgs {
                            event: event::CHANGED_LEVEL,
                            player: Some(player),
                            a: 1,
                            b: 40,
                            ..EventArgs::default()
                        };
                        act1::callback(self, w, i, args, None, false);
                    }
                }
                act1::q4::act_change(self, w, player);
            }
            npc::MESHIF1 => {
                let Some(f) = flags_of(w, player) else {
                    return Ok(());
                };
                if !f.get(15, 0) {
                    if !f.get(10, 0) {
                        f.set(10, 0);
                        f.set(10, 13);
                        w.delete_item(player, *b"msf ");
                        w.delete_item(player, *b"vip ");
                    }
                    let f = flags_of(w, player).expect("checked");
                    f.set(15, 0);
                    f.set(15, 13);
                    set_intro_flags(w, player, 1);
                    send_player_flags(w, player, 6, 0);
                    can_go_to_act(w, player, 3);
                }
            }
            npc::TYRAEL2 => {
                let expansion = w.expansion();
                let Some(f) = flags_of(w, player) else {
                    return Ok(());
                };
                if !f.get(28, 0) && expansion {
                    f.set(28, 0);
                    f.set(28, 13);
                    // The Act II list (sic, §8.1).
                    set_intro_flags(w, player, 1);
                    send_player_flags(w, player, 6, 0);
                    if w.player_byte_4c(player) != 1 {
                        w.send(player, &[0x5D, 0x17, 2, 0, 0, 0]);
                        can_go_to_act(w, player, 5);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// `0x00546AC0` (§8.1): object warp to `level`. Level 102 is the
    /// Durance; others go to `0x005BCFD0` (A3Q6, unspecified).
    pub fn object_warp<W: QuestWorld>(&mut self, w: &mut W, player: UnitId, level: u32) {
        if level != 102 {
            act3::durance_warp(self, w);
            return;
        }
        let Some(f) = flags_of(w, player) else { return };
        if f.get(23, 0) {
            return;
        }
        if !f.get(18, 0) {
            f.set(18, 0);
            f.set(18, 13);
            for code in [*b"qey ", *b"qhr ", *b"qbr ", *b"qf1 ", *b"qf2 "] {
                w.delete_item(player, code);
            }
        }
        let f = flags_of(w, player).expect("checked");
        f.set(23, 0);
        f.set(23, 13);
        set_intro_flags(w, player, 2);
        send_player_flags(w, player, 6, 0);
        can_go_to_act(w, player, 4);
    }
}

/// `0x0053D670`: S→C 0x28 (103 bytes) with the player's current record.
pub fn send_player_flags<W: QuestWorld>(w: &mut W, player: UnitId, unit_type: u8, guid: u32) {
    let rec = flags_of(w, player).copied().unwrap_or_default();
    let mut m = Vec::with_capacity(103);
    m.push(0x28);
    m.push(unit_type);
    m.extend_from_slice(&guid.to_le_bytes());
    m.push(0);
    m.extend_from_slice(&rec.0);
    w.send(player, &m);
}

/// `0x00538680(client, step, difficulty)` on save flags (client +0x0A;
/// `quests-act1-rest.md` §5): bits 8–12 hold the progression p; n = m ·
/// difficulty + step with m = 5 for an expansion character (bit 5), else
/// 4; p is raised to n, never lowered. n is or-ed in unmasked.
pub fn progression(flags: u16, step: u8, difficulty: u8) -> u16 {
    let m = ((u32::from(flags) & 0x20) | 0x80) >> 5;
    let n = m * u32::from(difficulty) + u32::from(step);
    let p = (u32::from(flags) >> 8) & 0x1F;
    if n < p {
        return flags;
    }
    ((u32::from(flags) & 0xE0FF) | (n << 8)) as u16
}

/// [`progression`] on the player's client (`0x005531C0`); nothing when
/// the host has no client for it.
pub fn raise_progression<W: QuestWorld>(w: &mut W, player: UnitId, step: u8, difficulty: u8) {
    if let Some(f) = w.client_save_flags(player) {
        w.set_client_save_flags(player, progression(f, step, difficulty));
    }
}

/// Player data +0x4C ≠ 1 → set it and send `61 act` (§8.1).
fn can_go_to_act<W: QuestWorld>(w: &mut W, player: UnitId, act: u8) {
    if w.player_byte_4c(player) != 1 {
        w.set_player_byte_4c(player, 1);
        w.send(player, &[0x61, act]);
    }
}

/// NPC intro lists by act (§Constants; Act IV has none).
pub const INTRO_NPCS: [&[u16]; 5] = [
    &[148, 154, 150, 147, 155, 265],
    &[175, 176, 177, 178, 202, 200, 210, 201, 198, 199, 244],
    &[245, 252, 253, 254, 255, 264, 297],
    &[],
    &[520, 521, 511, 512, 513, 514, 515],
];

/// `0x00544FA0(act)`: set the intro bit of every NPC of the act's list.
pub fn set_intro_flags<W: QuestWorld>(w: &mut W, player: UnitId, act: u8) {
    let d = usize::from(w.difficulty());
    if let Some(q) = w.quests(player) {
        for &n in INTRO_NPCS[usize::from(act)] {
            q.intro[d].insert(n);
        }
    }
}

/// `0x00545100` (§6.7): S→C 0x91 for acts 0, 1, 2, 4; introduced NPCs
/// packed at the front; sent only if one is set.
pub fn npc_gossip<W: QuestWorld>(w: &mut W, player: UnitId, act: u8) {
    if act == 3 || act > 4 {
        return;
    }
    let d = usize::from(w.difficulty());
    let heard = w
        .quests(player)
        .map(|q| q.intro[d].clone())
        .unwrap_or_default();
    let mut m = [0xFFu8; 26];
    m[0] = 0x91;
    m[1] = act;
    let mut n = 0;
    for &c in INTRO_NPCS[usize::from(act)] {
        if heard.contains(&c) {
            m[2 + 2 * n..4 + 2 * n].copy_from_slice(&c.to_le_bytes());
            n += 1;
        }
    }
    if n > 0 {
        w.send(player, &m);
    }
}

/// `0x005455F0` (§9.3): for each GUID whose player exists and lacks bits
/// 0 and 1 of `slot`: set bits 13 and 1; attach `sound` if ≠ 0.
pub fn grant_pending<W: QuestWorld>(w: &mut W, list: &GuidList, slot: u8, sound: u16) {
    for &g in &list.0 {
        let Some(p) = w.player_by_guid(g) else {
            continue;
        };
        let Some(f) = flags_of(w, p) else { continue };
        if f.get(slot, bit::REWARD_GRANTED) || f.get(slot, bit::REWARD_PENDING) {
            continue;
        }
        f.set(slot, bit::PRIMARY_GOAL_DONE);
        f.set(slot, bit::REWARD_PENDING);
        if sound != 0 {
            w.attach_sound(p, sound);
        }
    }
}

/// Result of a warp or portal check (§8.2, §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpCheck {
    Open,
    /// Decided by an Act II–V function the spec does not specify.
    Delegate(u32),
}

/// `0x00545B80` (§8.2).
pub fn warp_check(from: u32, to: u32) -> WarpCheck {
    match to {
        73 => WarpCheck::Delegate(0x0059_DB20),
        100 => WarpCheck::Delegate(0x005B_BFA0),
        118 | 128 if from == 120 => WarpCheck::Delegate(0x0058_D090),
        132 => WarpCheck::Delegate(0x0058_E640),
        _ => WarpCheck::Open,
    }
}

/// `0x00545830` (§8.3).
pub fn portal_check(level: u32) -> WarpCheck {
    if level == 73 {
        WarpCheck::Delegate(0x0059_DFD0)
    } else {
        WarpCheck::Open
    }
}

/// `0x00594140` (§8.4): the Cow portal (cube output kind 1).
pub fn cow_portal<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, player: UnitId) -> bool {
    let expansion = w.expansion();
    let in_town = w.unit_level(player) == Some(1);
    let pf = flags_of(w, player).copied().unwrap_or_default();
    let refused = ctl.game.get(4, 11)
        || pf.get(4, 10)
        || (!expansion && !pf.get(26, 0))
        || (expansion && !pf.get(40, 0))
        || !in_town;
    if !refused {
        if let Some((x, y)) = w.free_spot(player, 3, 0x400, 4, 100) {
            if w.create_portal(player, x, y, 60, 39) {
                ctl.game.set(4, 11);
                return true;
            }
        }
    }
    // TODO(quests §8.4): the spec names the sound for the refusal tests;
    // that a failed spot search or portal creation also plays it is
    // `world/cube.md` §9's reading ("on failure").
    w.attach_sound(player, u16::from(crate::world::cube::SOUND_COW_REFUSED));
    false
}

/// `0x00544840` (§9.4): C→S 0x3E read a clue item (after the handler's
/// item checks). `bkd ` → the Cairn stone order (§10.6); `trs ` →
/// `0x0059D6A0`.
pub fn read_clue<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, player: UnitId, code: [u8; 4]) {
    match &code {
        b"bkd " => act1::send_stone_order(ctl, w, player),
        b"trs " => true_tomb_clue(ctl, w, player),
        _ => {}
    }
}

/// `0x0059D6A0` (§9.4): 0x50 with u16 13 and the true tomb's level − 66
/// (0 when the level is 0); a nonzero level is kept at chain 13's extra
/// +0x34. Bytes 5–14 are never written in the original (stack); 0 here.
fn true_tomb_clue<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, player: UnitId) {
    let level = w.true_tomb_level();
    let tomb = if level == 0 { 0 } else { level as i32 - 66 };
    if level != 0 {
        if let Some(r) = ctl.record_mut(13) {
            r.extra.tomb_level = level;
        }
    }
    let mut m = [0u8; 15];
    m[0] = 0x50;
    m[1..3].copy_from_slice(&13u16.to_le_bytes());
    m[3..5].copy_from_slice(&(tomb as i16).to_le_bytes());
    w.send(player, &m);
}

/// `0x005449E0` (§9.5): object timer event 7 by object class. The Act
/// II–V functions are catalogued only (§11) and reported.
pub fn object_event<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId, class: u16) {
    // "Record c": no record → nothing.
    let record = |ctl: &QuestControl, w: &mut W, chain: u8, f: u32| {
        if ctl.find(chain).is_some() {
            w.unhandled(chain, f);
        }
    };
    match class {
        // Cain's gibbet (`quests-act1-rest.md` §1.2).
        0x1A => act1::q4::gibbet_event(ctl, w, object),
        0x7A => act2::q4::harem_blocker(ctl, w, object),
        0x83 => {
            let Some(level) = w.unit_level(object) else {
                return;
            };
            if w.object_mode(object) == 1 {
                w.set_object_mode(object, 2);
            }
            match level {
                76 => w.unhandled(0xFF, 0x005B_23C0),
                108 => act4::q2::dummy_event(ctl, w, object),
                _ => {}
            }
        }
        0xBD => match (w.unit_act(object), w.unit_level(object)) {
            (Some(0), _) => record(ctl, w, 4, 0x0059_42C0),
            (_, Some(l)) if l == 109 || l >= 113 => {
                if ctl.find(33).is_some() {
                    act5::q3::portal_event(ctl, w, object);
                }
            }
            _ => {
                if ctl.find(32).is_some() {
                    act5::q2::portal_event(ctl, w, object);
                }
            }
        },
        act1::WIRT_BODY => act1::wirt_body(ctl, w, object),
        0x155 => act3::bridge_event(ctl, w, object),
        0x16F => act3::lever_event(ctl, w, object),
        0x173 => act1::q5::chest_event(ctl, w, object),
        0x178 if ctl.find(24).is_some() => act4::q3::forge_event(ctl, w, object),
        0x1CB => act5::q4::temple_portal_event(ctl, w, object),
        0x1CC => act5::q3::anya_dummy_event(ctl, w, object),
        0x1CD => act5::q3::nihlathak_dummy_event(ctl, w, object),
        0x1DA..=0x1DC if ctl.find(35).is_some() => act5::q5::statue_event(ctl, w, object),
        _ => {}
    }
}

#[cfg(test)]
mod mutant_tests;
