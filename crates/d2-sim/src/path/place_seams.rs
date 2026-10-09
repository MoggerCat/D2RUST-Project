// Spec: specs/sim/path-placement.md §1–§6 (the operations §7–§12 call), §10–§12 (their other seams)
//! Seams of the free-point searches, unit placement and warps
//! ([`super::search`], [`super::place`], [`super::warp`]).
//!
//! [`CollisionView`] lists exactly the operations of `path-placement.md`
//! §1–§6 those modules call, under the spec's names and addresses; its
//! provider is the path core (§1–§6, path records, collision grids,
//! footprints). [`PlaceHost`] holds what the units, messages and walk
//! code answer, [`LevelView`] what the DRLG answers, [`WarpTileView`] the
//! DRLG side of the warp tile preset (§12.1). None of them has a provider
//! yet: every method states the spec rule it stands for, and the hosts
//! that have no answer take the narrowest default (nothing happens).

use std::fmt::Debug;

use super::coords::Point;
use crate::drlg::TileRect;

/// Collision masks of §3 used by §7–§12.
pub mod mask {
    /// Player move / placement: WALL, NOPLAYER, OBJECT, DOOR, NO_PATH.
    pub const PLAYER_PLACE: u32 = 0x1C09;
    /// Item floor: WALL, ITEM, OBJECT, DOOR, NO_PATH, PET.
    pub const ITEM_FLOOR: u32 = 0x3E01;
    /// Walk-back field: WALL, DOOR.
    pub const FIELD: u32 = 0x801;
    /// The act change's free point `0x0064E7E0(R, &pt, size, 0x1C89, 5)`
    /// (`world/waypoints.md` §11 step 9).
    pub const ACT_CHANGE: u32 = 0x1C89;
    /// The portal pair's field-search field mask (`world/objects-2.md`
    /// §25 rule 5; the spot mask is [`ITEM_FLOOR`]).
    pub const PORTAL_FIELD: u32 = 0xC01;
    /// The portal pair's destination free point (`world/objects-2.md`
    /// §25 rules 11 and 13).
    pub const PORTAL_DEST: u32 = 0xBE11;
}

/// The value a cell without a room (or a room without a grid) reads,
/// unmasked (§4 rule 2, edge case 1).
pub const MISSING_ROOM_VALUE: u32 = 0x27;

/// The operations of §1–§6 that the searches, placement and warps call.
/// Provider: the path core (`d2_sim::path`, §1–§6).
pub trait CollisionView {
    /// An active room (the collision grid's owner, `drlg/rooms.md` §1).
    type Room: Copy + Eq + Debug;
    /// A unit with a path record.
    type Unit: Copy + Eq + Debug;

    // ---- §4 collision queries --------------------------------------

    /// §4 rule 1, cell lookup `0x00463740(room, x, y)`: the hint room if
    /// its rect contains the cell, else the first room of its adjacency
    /// array containing it, else none; a null hint gives none.
    fn cell_room(&self, hint: Option<Self::Room>, x: i32, y: i32) -> Option<Self::Room>;
    /// The sub-tile rect of a room (§4 rule 1, room +0x4C..+0x58).
    fn room_rect(&self, room: Self::Room) -> TileRect;
    /// §4 rule 2, cell value: the grid value of the cell looked up from
    /// `room`, unmasked; no room or no grid → [`MISSING_ROOM_VALUE`].
    fn cell_value(&self, room: Self::Room, x: i32, y: i32) -> u32;
    /// Point query `0x0064CB30` (§4 rule 5): the masked cell value
    /// (rule 2: a missing room reads 0x27 unmasked).
    fn point_query(&self, room: Self::Room, x: i32, y: i32, mask: u32) -> u32;
    /// Size query `0x0064D9B0` (§4 rules 3–5): size 0, 1 point; 2 plus;
    /// 3 3×3 box; other sizes 0xFFFF. OR of the masked values.
    fn size_query(&self, room: Self::Room, x: i32, y: i32, size: i32, mask: u32) -> u32;
    /// Box query (`0x0064CEB0`, §4 rule 4): the `sx` × `sy` box centred
    /// on (x, y) (left = x − sx/2, bottom = y − sy/2, unsigned halving),
    /// clipped into sub-boxes at room edges; OR of the masked values.
    fn box_query(&self, room: Self::Room, x: i32, y: i32, sx: u32, sy: u32, mask: u32) -> u32;

    // ---- §2, §3 unit path facts ------------------------------------

    /// The unit has a path record (unit +0x2C ≠ null, §2.1).
    fn has_path(&self, unit: Self::Unit) -> bool;
    /// Room of a unit (`0x00620BB0`, §2.1).
    fn unit_room(&self, unit: Self::Unit) -> Option<Self::Room>;
    /// Unit size (`0x00620510`, §3).
    fn unit_size(&self, unit: Self::Unit) -> i32;

    // ---- §2.5, §6 moves --------------------------------------------

    /// Teleport `0x00650910(path, room, x, y)` (§6 rule 4): always
    /// succeeds (footprint move, position, room, movement reset).
    fn teleport(&mut self, unit: Self::Unit, room: Self::Room, x: i32, y: i32);
    /// Adding a unit to the world `0x00554850(flag 0)` (§2.5) as game
    /// entry `0x005394A0` calls it for the player (§11): the room-changed
    /// flag is set.
    fn add_player_to_world(&mut self, unit: Self::Unit, room: Self::Room, x: i32, y: i32);
    /// Game entry's room switch of the player's client to `room`
    /// (`0x005381F0` → `0x00537B50`, §11; owner
    /// `sim/intents-events.md` §7.8): S→C 0x07 and the add messages for
    /// every room of the room's adjacency array, client +0x1B4 := `room`.
    /// Default: nothing.
    fn client_room_switch(&mut self, player: Self::Unit, room: Self::Room) {
        let _ = (player, room);
    }
}

/// The unit flags 2 (+0xC8) bits §10 rule 5 sets.
pub mod flags2 {
    /// `alt` = 0.
    pub const PLACED: u32 = 0x10000;
    /// `alt` ≠ 0.
    pub const PLACED_ALT: u32 = 0x800;
}

/// Timer event a player placement schedules (§10 rule 6, `sim/units.md`
/// §6), with callback `0x00554570`, this many frames ahead.
pub const PLACE_TIMER_EVENT: u8 = 14;
/// Frames ahead of the type-14 timer (Constants table).
pub const PLACE_TIMER_DELAY: u32 = 50;

/// S→C messages of §10–§12 (`sim/server-messages.tsv`). Unit type and
/// GUID are the host's to fill in from the unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceMessage<U> {
    /// 0x07 MapReveal (`0x0053BC50`): tile x, tile y of a room, its level.
    MapReveal { x: u16, y: u16, level: u8 },
    /// 0x15 ReassignPlayer (`0x0053BC10`): unit, x, y, flag.
    ReassignPlayer { unit: U, x: u16, y: u16, flag: u8 },
    /// 0x0D PlayerStop (`0x0053B4B0`): unit, a, x, y, b, life percent.
    PlayerStop {
        unit: U,
        a: u8,
        x: u16,
        y: u16,
        b: u8,
        life_pct: u8,
    },
    /// 0x7E (`0x0053DB70`, 5 bytes), the last message of game entry
    /// (§11): only the id is written in 1.14d
    /// (`sim/intents-events.md` edge case 10).
    GameEntryDone,
}

/// Units, messages, timers and walk calls of §10–§12 that are not §1–§6.
#[allow(unused_variables)]
pub trait PlaceHost<U> {
    /// Unit type 0 (`sim/units.md` §2).
    fn is_player(&self, unit: U) -> bool;
    /// Queue for update `0x0064C040` (`sim/unit-order.md` §6).
    fn queue_update(&mut self, unit: U) {}
    /// Unit flags 2 (+0xC8) |= `bits` ([`flags2`]).
    fn or_flags2(&mut self, unit: U, bits: u32) {}
    /// Room-change messages `0x00554670(game, unit, 0)` (`sim/pathing.md`
    /// §9.8).
    fn room_change_messages(&mut self, unit: U) {}
    /// Sends a message to the client of `player` (`0x005531C0`): every
    /// message of §10–§12 goes to the moving player's own client (§11
    /// "Recipients").
    fn send(&mut self, player: U, msg: PlaceMessage<U>) {}
    /// Player data +0x148, +0x14C := x, y (§10 rule 6).
    fn set_player_point(&mut self, player: U, x: i32, y: i32) {}
    /// Position history write `0x00554FD0` (§10 rule 7): entry := (x, y)
    /// unconditionally ([`super::history::PositionHistory::place_write`]).
    fn history_write(&mut self, player: U, x: i32, y: i32) {}
    /// Schedules timer `event` for `unit` at frame + `delay` (callback
    /// `0x00554570` for event 14, `sim/units.md` §6).
    fn schedule_event(&mut self, unit: U, event: u8, delay: u32) {}
    /// Pets follow the player `0x005754B0` (pet / mercenary spec, open
    /// question 7).
    fn pets_follow(&mut self, player: U) {}
    /// Player mode request, point form: walk (mode 2) to (x, y)
    /// (`0x005809D0(no skill, 2, target, 0)`, `sim/pathing.md` §1.2).
    fn request_walk(&mut self, player: U, x: i32, y: i32) {}
    /// Life percent `0x00621F20`.
    fn life_percent(&self, unit: U) -> u8 {
        0
    }
}

/// A room's 0x07 MapReveal fields: tile x, tile y, level id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomReveal {
    pub tile_x: i32,
    pub tile_y: i32,
    pub level: u32,
}

/// The destination of a warp tile (`0x006195A0`, §12.2 rule 1) and
/// the facts §12.2 reads from it and its lvlwarp record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WarpDestination<R> {
    /// The destination tile's room.
    pub room: R,
    /// The destination tile's position.
    pub point: Point,
    /// lvlwarp `ExitWalkX`, `ExitWalkY` (+0x14, +0x18).
    pub exit_walk_x: i32,
    pub exit_walk_y: i32,
    /// Level of the source tile and of the destination (§12.2 rule 3).
    pub source_level: u32,
    pub level: u32,
}

/// DRLG answers of §10–§12.
#[allow(unused_variables)]
pub trait LevelView<R> {
    /// An act (`drlg/levels.md` §2).
    type Act: Copy + Debug;
    /// Spawn room and position of a level `0x0066B2B0` (`drlg/levels.md`
    /// §10; may draw `roll` on the level seed): the room and the position
    /// in tiles. `None`: no spawn room.
    fn spawn_room(&mut self, act: Self::Act, level: u32, tile_index: u32) -> Option<(R, i32, i32)>;
    /// The act's start level (act +0x08).
    fn act_start_level(&self, act: Self::Act) -> u32;
    /// A room's 0x07 fields (tile x, tile y, level id).
    fn room_reveal(&self, room: R) -> RoomReveal;
    /// `0x006195A0(room of the tile, tile class)` (§12.2 rule 1): in the
    /// source DRLG room's warp-link list the first link whose lvlwarp
    /// `Id` = class gives the destination DRLG room D; D's link back to
    /// the source gives the record (`ExitWalkX/Y` are the destination
    /// side's); the destination tile is the first tile unit of class =
    /// that record's `Id` in D's active room. `None`: nothing.
    fn warp_destination(&self, tile_room: R, tile_class: u32) -> Option<WarpDestination<R>> {
        None
    }
    /// Quest warp gate `0x00545B80(source level, destination level)`
    /// (§12.2 rule 3; its checks: `world/quests.md` §8.2). Non-zero
    /// blocks.
    fn quest_gate(&self, source_level: u32, level: u32) -> u32 {
        0
    }
}

/// lvlwarp fields §12.1 reads (`drlg/levels.md` §7 rule 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LvlWarp {
    /// `Id`: the warp tile's class.
    pub id: u32,
    pub offset_x: i32,
    pub offset_y: i32,
}

/// DRLG side of the warp tile preset (§12.1, `0x0066E1C0`).
pub trait WarpTileView {
    /// A DRLG room.
    type DrlgRoom: Copy + Debug;
    /// The room's tile rect (tile x, y, width, height).
    fn tile_rect(&self, room: Self::DrlgRoom) -> TileRect;
    /// lvlwarp record of warp slot `slot` of the room's level for
    /// direction letter `letter` (`drlg/levels.md` §7 rule 4,
    /// `0x0066AF50`). `None` is fatal there.
    fn lvlwarp(&self, room: Self::DrlgRoom, slot: u32, letter: u8) -> Option<LvlWarp>;
    /// Prepends a preset unit to the room's list (`0x0066BF30`): unit
    /// type, class, mode, position in sub-tiles relative to the room.
    fn add_preset_unit(
        &mut self,
        room: Self::DrlgRoom,
        ty: u8,
        class: u32,
        mode: u32,
        x: i32,
        y: i32,
    );
}

/// Fatal asserts of §7–§12 (the original exits) and malformed walk-back
/// fields; d2rs returns them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlaceError {
    /// §10 rule 1: unit without a path.
    #[error("placement of a unit without a path")]
    NoPath,
    /// §11 rule 1: act null.
    #[error("level spawn point without an act")]
    NoAct,
    /// §11 game entry: no spawn room (fatal assert in `0x005394A0`).
    #[error("game entry without a spawn room")]
    NoSpawnRoom,
    /// §11 rule 3: no free point around the spawn (edge case 6).
    #[error("no free point around the level spawn")]
    SpawnNotFree,
    /// §12.1 rule 1: no lvlwarp record for the slot and letter.
    #[error("no lvlwarp record for warp slot {slot}")]
    NoLvlWarp { slot: u32 },
    /// §7.3: a walk-back read outside the 256 × 256 field (unreachable
    /// with the 1.14d file and the 50-ring searches; the original reads
    /// past the buffer).
    #[error("walk-back field read outside the field at ({x}, {y})")]
    FieldOutOfRange { x: i32, y: i32 },
    /// §7.3: a field byte above 8 (not in the 1.14d file).
    #[error("walk-back field direction {0} > 8")]
    FieldDirection(u8),
    /// §7.3: a walk that does not reach the centre (not in the 1.14d
    /// file, measured; the original would loop).
    #[error("walk-back field walk does not reach the centre")]
    FieldNoEnd,
}
