// Spec: specs/world/objects.md
//! Objects: the object control (§2), creation and init dispatch (§3),
//! the object animation at a mode change (§4), the generic init functions
//! (§5), preset classes (§6), operate dispatch (§7), the object timer
//! events and the client update messages (§14).
//!
//! Per-function behavior lives in submodules: chests, breakables and traps
//! ([`chests`], §8), shrines ([`shrines`], §9), doors, wells, portals and
//! torches ([`misc`], §10–§13). Units, rooms, items, monsters, stats and
//! messages belong to other specs; they are reached through
//! [`ObjectWorld`] and the per-module extension traits, bundled as
//! [`ObjectHost`]. Quest objects (owner `world/quests.md`), waypoints
//! (`world/waypoints.md`) and the `todo` rows of `object-functions.tsv`
//! are reported to the caller ([`Route`]) instead of run here.

use std::collections::BTreeMap;

use d2_data::tables::{Leveldefs, Levels, Objects, Objgroup, Shrines};

use crate::rng::Seed;
use crate::units::{RoomId, UnitId};

pub mod chests;
#[cfg(test)]
pub(crate) mod fake;
pub mod mech;
pub mod misc;
pub mod populate;
pub mod shrines;
#[cfg(test)]
mod tests;

pub use chests::ChestWorld;
pub use mech::MechWorld;
pub use misc::MiscWorld;
pub use shrines::{ShrineWorld, StateList, StateRequest};

/// `objects.txt` rows the dispatchers accept (§3 rule 5).
pub const CLASS_BOUND: u16 = 573;
/// Init function bound (§3 rule 5, table `0x00731BC0`).
pub const INIT_FN_BOUND: u8 = 80;
/// Operate function bound (§7.2 rule 3, table `0x00732D18`).
pub const OPERATE_FN_BOUND: u8 = 101;
/// Object mode bound (§3 rule 2, §4).
pub const MODE_BOUND: u8 = 8;
/// Classes the operate dispatch refuses (§7.2 rule 3).
pub const REFUSED_CLASSES: [u16; 3] = [22, 121, 122];
/// The stash class: operable with an item on the cursor (§7.2 rule 2).
pub const STASH_CLASS: u16 = 267;
/// Player class id of the assassin (§8.1 rule 2).
pub const ASSASSIN: u8 = 6;
/// Town levels by act (§5.5, `world/waypoints.md` §1 rule 4).
pub const TOWNS: [u32; 5] = [1, 40, 75, 103, 109];

/// Unit flags (+0xC4) used for objects (§1).
pub mod oflags {
    /// Changed: sends S→C 0x0E (§14).
    pub const CHANGED: u32 = 0x1;
    pub const SELECTABLE: u32 = 0x2;
    pub const ATTACKABLE: u32 = 0x4;
    /// Cleared at init (§3 rule 1).
    pub const INIT_CLEARED: u32 = 0x8;
    /// Keep mode: blocks the PreOperate roll (§3 rule 8, set by §6).
    pub const KEEP_MODE: u32 = 0x80;
    pub const HOVER_FREED: u32 = 0x100;
    pub const SOUND_QUEUED: u32 = 0x400;
}

/// Object timer event types (`sim/units.md` §6.4).
pub mod oevent {
    pub const END_ANIM: u8 = 1;
    pub const WELL_REFILL: u8 = 2;
    pub const TRAP: u8 = 4;
    pub const SHRINE_RESET: u8 = 5;
    pub const HOVER: u8 = 6;
    pub const QUEST: u8 = 7;
    pub const DELAYED_PORTAL: u8 = 11;
}

/// Sound ids (§14).
pub mod sound {
    pub const PORTAL: u8 = 8;
    pub const UNLOCK: u8 = 11;
    pub const TRAP_ARMED: u8 = 13;
    pub const PORTAL_REFUSED: u8 = 19;
    pub const LOCKED: u8 = 22;
}

/// Fatal asserts of the original, returned instead of aborting.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ObjectError {
    #[error("object mode {0} ≥ 8")]
    Mode(u8),
    #[error("object class {0} ≥ 573")]
    Class(u16),
    #[error("init function {0} ≥ 80")]
    InitFn(u8),
    #[error("operate function {0} ≥ 101")]
    OperateFn(u8),
    #[error("shrine class {0} has an empty list")]
    EmptyShrineList(u8),
    #[error("town level {0} > 255")]
    TownLevel(u32),
    #[error("well charges {0} out of range at refill")]
    WellCharges(u8),
    #[error("unit {0:?} has no object data")]
    NoData(UnitId),
    #[error("{table} has no row {row}")]
    NoRow { table: &'static str, row: u32 },
    #[error("object {0} allocation failed (fatal)")]
    AllocFailed(u16),
    #[error("shrine effect with no operator (null read)")]
    ShrineNoOperator,
    #[error("portal operated with no player (0x0058494F)")]
    PortalOperator,
    #[error("portal {0:?}: no destination room")]
    NoPortalDestination(UnitId),
    #[error("portal travel: placing {0:?} failed (fatal)")]
    PortalPlacement(UnitId),
    #[error("object {0:?}: no warp tile in its room (fatal)")]
    NoWarpTile(UnitId),
    #[error("key test with no unit (0x0055F173)")]
    KeyTestNoUnit,
    #[error("room {0:?} has no active room seed")]
    NoActiveRoom(RoomId),
    #[error("populate function {0} ≥ 10")]
    PopulateFn(u8),
    #[error("populate density {0} out of range")]
    Density(u8),
    #[error("room theme {0}: body not specified (object-population.md open question 5)")]
    Theme(u32),
}

// ------------------------------------------------------------------ tables

/// The tables the object code reads (§1, Constants).
#[derive(Debug, Clone, Default)]
pub struct ObjectTables {
    pub objects: Vec<Objects>,
    pub shrines: Vec<Shrines>,
    pub levels: Vec<Levels>,
    /// `objgroup.txt` (`d2exp`; object population, `object-population.md`
    /// §5).
    pub objgroup: Vec<Objgroup>,
    /// `leveldefs` (0x9C-byte records, `0x0061E470`): the portal quest
    /// gate (§12 rule 7).
    pub leveldefs: Vec<Leveldefs>,
}

impl ObjectTables {
    pub fn object(&self, class: u16) -> Result<&Objects, ObjectError> {
        self.objects.get(class as usize).ok_or(ObjectError::NoRow {
            table: "objects",
            row: class.into(),
        })
    }
    pub fn shrine(&self, id: u16) -> Result<&Shrines, ObjectError> {
        self.shrines.get(id as usize).ok_or(ObjectError::NoRow {
            table: "shrines",
            row: id.into(),
        })
    }
    pub fn level(&self, id: u32) -> Option<&Levels> {
        self.levels.get(id as usize)
    }
    /// `MonLvl1` (+0x10) read as i16 (§5.2).
    pub fn mon_lvl1(&self, level: u32) -> i16 {
        self.level(level).map_or(0, |l| l.monlvl1 as i16)
    }
}

/// Per-mode `objects.txt` columns (indexed 0–7; `mode < 8` checked by the
/// callers).
pub fn frame_cnt(o: &Objects, mode: u8) -> u32 {
    [
        o.framecnt0,
        o.framecnt1,
        o.framecnt2,
        o.framecnt3,
        o.framecnt4,
        o.framecnt5,
        o.framecnt6,
        o.framecnt7,
    ][mode as usize & 7]
}
pub fn frame_delta(o: &Objects, mode: u8) -> u16 {
    [
        o.framedelta0,
        o.framedelta1,
        o.framedelta2,
        o.framedelta3,
        o.framedelta4,
        o.framedelta5,
        o.framedelta6,
        o.framedelta7,
    ][mode as usize & 7]
}
pub fn start(o: &Objects, mode: u8) -> u8 {
    [
        o.start0, o.start1, o.start2, o.start3, o.start4, o.start5, o.start6, o.start7,
    ][mode as usize & 7]
}
pub fn selectable(o: &Objects, mode: u8) -> u8 {
    [
        o.selectable0,
        o.selectable1,
        o.selectable2,
        o.selectable3,
        o.selectable4,
        o.selectable5,
        o.selectable6,
        o.selectable7,
    ][mode as usize & 7]
}
/// The `FrameCnt1` column value (the table stores it × 256,
/// `data/fixups.md` §13): "fc1" of the ENDANIM delays.
pub fn fc1(o: &Objects) -> i32 {
    (o.framecnt1 >> 8) as i32
}

// ------------------------------------------------------------------ §1, §2

/// The 0x38-byte object data (unit +0x14, §1) plus the object unit fields
/// only object code uses (spark byte, timer argument, drop code, last
/// host tick). Kept per object unit in [`ObjectControl::data`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObjectData {
    pub guid: u32,
    pub class: u16,
    /// +0x04 `InteractType`.
    pub interact: u8,
    /// +0x05 portal flags (§1; portal creation ORs 0x3, travel 0x5).
    pub portal_flags: u8,
    /// +0x08 the shrines.txt row.
    pub shrine: Option<u16>,
    /// +0x0C operator GUID + 1 (0 = none).
    pub operator: u32,
    /// Unit +0x78.
    pub spark: u8,
    /// Unit +0x7C: the timer argument's owner GUID (−1 at allocation).
    pub owner: Option<i32>,
    /// Unit +0xB8: drop item code (0 = none).
    pub drop_code: u32,
    /// Unit +0xD4: last host tick of a door or gate operation.
    pub last_tick: u32,
}

/// A level's object region (§2 rule 4; the population fields:
/// `object-population.md` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// +0x00 `levels.Act`.
    pub act: u8,
    /// +0x04: rooms counted so far.
    pub counted: i32,
    /// +0x08: populated-room total, 0x7FFFFFFF until set.
    pub w08: i32,
    /// +0x10: health shrines.
    pub health: i32,
    /// +0x14: shrines (≤ 10).
    pub shrines: i32,
    /// +0x18: wells (≤ 4).
    pub wells: i32,
    /// +0x1C.
    pub w1c: i32,
    /// +0x20: well points.
    pub well_points: [(i32, i32); 4],
    /// +0x40: shrine points.
    pub shrine_points: [(i32, i32); 10],
}

impl Region {
    /// A region as the control build leaves it (§2 rule 4).
    pub fn new(act: u8) -> Self {
        Self {
            act,
            counted: 0,
            w08: 0x7FFF_FFFF,
            health: 0,
            shrines: 0,
            wells: 0,
            w1c: -1,
            well_points: [(0, 0); 4],
            shrine_points: [(0, 0); 10],
        }
    }
}

/// The object control (game +0x10F0, §2) and the per-object data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectControl {
    /// +0x00: the control seed.
    pub seed: Seed,
    /// +0x48 + 4·id: one region per level id 1 … count − 1 (0 = none).
    pub regions: Vec<Option<Region>>,
    /// +0x28 + 4·class: shrine row ids per `effectclass` 0–7.
    pub shrine_lists: [Vec<u16>; 8],
    pub data: BTreeMap<UnitId, ObjectData>,
}

impl ObjectControl {
    /// `0x00546C60` (§2): built once per game. Steps `game_seed` once;
    /// returns the control and `lo'`.
    pub fn new(game_seed: &mut Seed, t: &ObjectTables) -> (Self, u32) {
        let lo = game_seed.step();
        let mut regions = vec![None];
        for l in t.levels.iter().skip(1) {
            regions.push(Some(Region::new(l.act)));
        }
        let mut shrine_lists: [Vec<u16>; 8] = Default::default();
        for (i, s) in t.shrines.iter().enumerate() {
            if let Some(list) = shrine_lists.get_mut(s.effectclass as usize) {
                list.push(i as u16);
            }
        }
        (
            Self {
                seed: Seed::init_low(lo),
                regions,
                shrine_lists,
                data: BTreeMap::new(),
            },
            lo,
        )
    }

    pub fn get(&self, obj: UnitId) -> Result<&ObjectData, ObjectError> {
        self.data.get(&obj).ok_or(ObjectError::NoData(obj))
    }
    pub fn get_mut(&mut self, obj: UnitId) -> Result<&mut ObjectData, ObjectError> {
        self.data.get_mut(&obj).ok_or(ObjectError::NoData(obj))
    }
}

// ------------------------------------------------------------------ seams

/// What an operator is (§7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// A player and its class id.
    Player(u8),
    Monster,
    Other,
}

/// The base seam to the rest of the game. Expected providers in brackets.
pub trait ObjectWorld {
    /// Game +0xA8 (tick).
    fn frame(&self) -> i32;
    /// The host's `GetTickCount` (edge case 9: an input, never read by
    /// `d2-sim`).
    fn host_tick(&self) -> u32;
    /// Unit +0x0C (units).
    fn guid(&self, unit: UnitId) -> u32;
    /// `0x00552F60`: the object unit with `guid` (unit lists).
    fn find_object(&self, guid: u32) -> Option<UnitId>;
    /// The kind of a unit that operates (units).
    fn operator(&self, unit: UnitId) -> Operator;
    /// Unit +0x10 (units).
    fn mode(&self, unit: UnitId) -> u8;
    /// Store the mode, set flag 0x1 and, when `queue`, queue the unit for
    /// update (`sim/units.md` §4, `unit-order.md` §6). The animation is
    /// set by [`set_mode`] through [`ObjectWorld::set_anim`].
    fn write_mode(&mut self, unit: UnitId, mode: u8, queue: bool);
    /// Unit +0x44 frame, +0x48 frame count, +0x4C speed (§4).
    fn set_anim(&mut self, unit: UnitId, frame_count: i32, frame: i32, speed: i16);
    /// Unit +0x20 (units).
    fn unit_seed(&mut self, unit: UnitId) -> Option<&mut Seed>;
    /// Unit +0xC4 (units).
    fn flags(&self, unit: UnitId) -> u32;
    fn set_flags(&mut self, unit: UnitId, flags: u32);
    /// Queue for the update pass (`unit-order.md` §6).
    fn queue_update(&mut self, unit: UnitId);
    /// The unit's room (path / DRLG).
    fn room(&self, unit: UnitId) -> Option<RoomId>;
    /// The level id of the unit's room (DRLG).
    fn level(&self, unit: UnitId) -> Option<u32>;
    /// The level id of a room (DRLG). Default: none.
    fn room_level(&self, room: RoomId) -> Option<u32> {
        let _ = room;
        None
    }
    /// Sub-tile position (path).
    fn position(&self, unit: UnitId) -> (i32, i32);
    /// Schedule object event `ev` at `frame` (`tick.md` §5).
    fn schedule(&mut self, unit: UnitId, ev: u8, frame: i32);
    /// `0x00540F30`: cancel every timer of the unit.
    fn cancel_timers(&mut self, unit: UnitId);
    /// `0x00620A70(O, room, x, y)`: stamp the object's footprint at the
    /// room and point given (§5.5: an init passes its record's; the
    /// door and the gate O's own; `sim/path-placement.md` §3, §5.1).
    fn stamp_footprint(&mut self, unit: UnitId, room: Option<RoomId>, x: i32, y: i32);
    /// `0x00623830`: free the object's footprint (path placement).
    fn free_footprint(&mut self, unit: UnitId);
    /// Attach sound `id` to `unit` (no target when `to` is `None`); `now`:
    /// sent at once to the unit's client (`0x00571740`).
    fn sound(&mut self, unit: UnitId, id: u8, to: Option<UnitId>, now: bool);
    /// `0x0055F140`: the key test and use (§8.1 rule 2; inventory).
    fn key_test(&mut self, player: UnitId) -> bool;
    /// `0x00623660`: the operator is in interact range (path).
    fn in_interact_range(&self, operator: UnitId, object: UnitId) -> bool;
    /// `0x00554100`: the player's interact info is active.
    fn interact_active(&self, player: UnitId) -> bool;
    /// Player data +0x4C ≠ 0.
    fn player_busy(&self, player: UnitId) -> bool;
    /// An item is on the player's cursor (inventory).
    fn cursor_item(&self, player: UnitId) -> bool;
    /// Allocate an object unit (`sim/units.md` §1) of `class` in `room` at
    /// (x, y) and `mode`. A provider that cannot run [`create`] here (the
    /// control is lent to the caller) leaves it to [`allocate`]. `None`:
    /// not allocated.
    fn allocate_object(
        &mut self,
        room: RoomId,
        class: u16,
        x: i32,
        y: i32,
        mode: u8,
    ) -> Option<UnitId>;
    /// `SUNIT_Add` (`sim/units.md` §3.1 step 8) of an
    /// [`Self::allocate_object`] unit whose [`create`] [`allocate`] ran:
    /// the init comes first (r7.1). A provider that links in
    /// [`Self::allocate_object`] keeps the default (nothing).
    fn add_object(&mut self, _obj: UnitId, _room: RoomId, _x: i32, _y: i32) {}
    /// `0x0061AEB0`: the act II staff-tomb level (quest spec).
    fn staff_tomb_level(&self) -> u32;
    /// Unit +0x10 := `mode` written directly: no mode set, no animation
    /// setup, no queue, no changed flag (`objects-2.md` §18.1, §18.6).
    fn store_mode(&mut self, unit: UnitId, mode: u8);
    /// Init 13 (`objects-2.md` §17): quest chain `chain`'s record exists
    /// (`0x00543640`) → link the object to it (`0x005436B0`,
    /// `world/quests.md` §4.6) and return `true`. Default: no record.
    fn quest_link(&mut self, object: UnitId, chain: u8) -> bool {
        let _ = (object, chain);
        false
    }
    /// `0x00463740(room, x, y)`: the room holding the point among `room`
    /// and its adjacency array.
    fn room_at(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId> {
        let _ = (room, x, y);
        None
    }
    /// `0x00559300` (`objects-2.md` §20.3): a gold drop at (x, y) in
    /// `room`. Default: nothing.
    fn gold_drop(&mut self, room: RoomId, x: i32, y: i32) {
        let _ = (room, x, y);
    }
    /// `0x0064D800(room, x, y, 1, 1, mask)` = 0: the point is free of
    /// `mask`. Default: not free.
    fn point_free(&self, room: RoomId, x: i32, y: i32, mask: u32) -> bool {
        let _ = (room, x, y, mask);
        false
    }
}

/// Every seam the dispatchers need.
pub trait ObjectHost: ObjectWorld + ChestWorld + ShrineWorld + MiscWorld + MechWorld {}
impl<T: ObjectWorld + ChestWorld + ShrineWorld + MiscWorld + MechWorld> ObjectHost for T {}

// ------------------------------------------------------------------ §4

/// `0x00624690` for an object: store `mode`, then the animation setup
/// `0x00624390` (§4). `queue`: the mode set queues the unit (every caller
/// except event 1, `sim/units.md` §6.4).
pub fn set_mode<W: ObjectWorld>(
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    class: u16,
    mode: u8,
    queue: bool,
) -> Result<(), ObjectError> {
    if mode >= MODE_BOUND {
        return Err(ObjectError::Mode(mode));
    }
    let o = t.object(class)?;
    // Rule 5 / `sim/units.md` §4.1: the same mode only queues the unit and
    // sets flag 0x1 (no animation setup, no draw).
    let same = w.mode(obj) == mode;
    w.write_mode(obj, mode, queue);
    if same {
        return Ok(());
    }
    let frame_count = frame_cnt(o, mode) as i32;
    let frame = i32::from(start(o, mode)) * 256;
    let d = frame_delta(o, mode) as i16;
    let speed = if o.sync != 0 {
        d
    } else {
        let r = w.unit_seed(obj).map_or(0, |s| s.roll(i32::from(d >> 3))) as i32;
        // Rule 4: a 32-bit sum of the sign-extended values, clamped, low
        // 16 bits stored.
        let s = r + i32::from(d) - i32::from(d >> 4);
        s.clamp(0, 0x7FFF) as i16
    };
    w.set_anim(obj, frame_count, frame, speed);
    Ok(())
}

/// The mode change of an object whose class is in its data.
pub fn set_object_mode<W: ObjectWorld>(
    ctl: &ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    mode: u8,
) -> Result<(), ObjectError> {
    let class = ctl.get(obj)?.class;
    set_mode(t, w, obj, class, mode, true)
}

/// ENDANIM (§8.2): event 1 at frame + fc1 + 1.
pub fn schedule_endanim<W: ObjectWorld>(w: &mut W, o: &Objects, obj: UnitId) {
    let at = w.frame() + fc1(o) + 1;
    w.schedule(obj, oevent::END_ANIM, at);
}

fn set_flag<W: ObjectWorld>(w: &mut W, u: UnitId, f: u32, on: bool) {
    let v = w.flags(u);
    w.set_flags(u, if on { v | f } else { v & !f });
}

/// Clear flag 0x2 (selectable).
pub fn clear_selectable<W: ObjectWorld>(w: &mut W, u: UnitId) {
    set_flag(w, u, oflags::SELECTABLE, false);
}

// ------------------------------------------------------------------ routes

/// Who runs an init or operate function number (`object-functions.tsv`
/// `owner`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Run by this module.
    Here,
    /// `world/quests.md`.
    Quest,
    /// `world/waypoints.md`.
    Waypoint,
    /// `todo` in the table: not specified yet, nothing runs.
    NotCovered,
    /// Null table entry.
    Null,
}

/// The route of init function `n` (§5, `object-functions.tsv`).
pub fn init_route(n: u8) -> Route {
    match n {
        0 | 35 | 36 | 40 => Route::Null,
        1 | 2 | 3 | 5 | 11 | 12 | 16 | 57 => Route::Here,
        17 => Route::Waypoint,
        8 | 10 | 13 | 14 | 22 | 24 | 26 | 27 | 28 | 34 | 51 | 58 => Route::Here,
        n if n < INIT_FN_BOUND => Route::Quest,
        _ => Route::Null,
    }
}

/// The route of operate function `n` (§7.2, `object-functions.tsv`).
pub fn operate_route(n: u8) -> Route {
    match n {
        0 | 35..=38 | 60 | 74..=100 => Route::Null,
        1 | 2 | 3 | 4 | 5 | 7 | 8 | 11 | 14 | 15 | 22 | 68 => Route::Here,
        // `objects-2.md` §16, §18.4.
        13 | 16 | 17 | 18 | 19 | 20 | 26 | 27 | 29 | 30 | 32 | 47 | 48 | 50 | 51 | 61 => {
            Route::Here
        }
        23 => Route::Waypoint,
        n if n < OPERATE_FN_BOUND => Route::Quest,
        _ => Route::Null,
    }
}

// ------------------------------------------------------------------ §3

/// What creation ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Created {
    /// The init function's route (a quest, waypoint or not-covered init
    /// ran nothing here; the caller runs or records it).
    pub init: Route,
    pub init_fn: u8,
}

/// `0x0054F5D0` (§3): the init dispatch of a freshly allocated object
/// whose unit already holds `mode` (the allocation mode). Runs before the
/// unit is added to the world. [`create_init`] then [`create_rest`]; a
/// caller that runs a quest init itself calls them apart, the init
/// between them (rule 6, `quests-act1-rest.md` §9 item 7).
#[allow(clippy::too_many_arguments)]
pub fn create<W: ObjectWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    class: u16,
    guid: u32,
    room: Option<RoomId>,
    mode: u8,
    x: i32,
    y: i32,
) -> Result<Created, ObjectError> {
    let created = create_init(ctl, t, w, obj, class, guid, room, mode, x, y)?;
    create_rest(ctl, t, w, obj, mode)?;
    Ok(created)
}

/// §3 rules 1–6: the object data, the flags, the timers and the init
/// function this spec owns; any other init's route is returned for the
/// caller, who runs it before [`create_rest`].
#[allow(clippy::too_many_arguments)]
pub fn create_init<W: ObjectWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    class: u16,
    guid: u32,
    room: Option<RoomId>,
    mode: u8,
    x: i32,
    y: i32,
) -> Result<Created, ObjectError> {
    // Rule 1.
    let data = ctl.data.entry(obj).or_default();
    *data = ObjectData {
        guid,
        class,
        ..ObjectData::default()
    };
    set_flag(w, obj, oflags::INIT_CLEARED, false);
    // Rule 2.
    if mode >= MODE_BOUND {
        return Err(ObjectError::Mode(mode));
    }
    // Rule 5 (class) before the record is read.
    if class >= CLASS_BOUND {
        return Err(ObjectError::Class(class));
    }
    let o = t.object(class)?;
    // Rule 3.
    set_flag(w, obj, oflags::ATTACKABLE, o.isattackable0 != 0);
    // Rule 4.
    w.cancel_timers(obj);
    // Rule 5.
    let init_fn = o.initfn;
    if init_fn >= INIT_FN_BOUND {
        return Err(ObjectError::InitFn(init_fn));
    }
    // Rule 6.
    let route = init_route(init_fn);
    if route == Route::Here {
        run_init(ctl, t, w, obj, init_fn, room, x, y)?;
    }
    Ok(Created {
        init: route,
        init_fn,
    })
}

/// §3 rules 7–9 after the init: selectable (by the allocation mode
/// `m0`), the `PreOperate` draw, the owner.
pub fn create_rest<W: ObjectWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    m0: u8,
) -> Result<(), ObjectError> {
    let class = ctl.get(obj)?.class;
    let o = t.object(class)?;
    // Rule 7.
    set_flag(w, obj, oflags::SELECTABLE, selectable(o, m0) != 0);
    // Rule 8.
    if o.preoperate != 0 && w.flags(obj) & oflags::KEEP_MODE == 0 && ctl.seed.roll(14) == 0 {
        set_mode(t, w, obj, class, 2, true)?;
    }
    // Rule 9.
    ctl.get_mut(obj)?.owner = Some(-1);
    Ok(())
}

/// The init functions this spec owns (§5).
#[allow(clippy::too_many_arguments)]
fn run_init<W: ObjectWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    n: u8,
    room: Option<RoomId>,
    x: i32,
    y: i32,
) -> Result<(), ObjectError> {
    // The level of the init record's room: §3 runs before the unit is
    // added to the world, so the unit has no room of its own yet.
    let level = room
        .and_then(|r| w.room_level(r))
        .or_else(|| w.level(obj))
        .unwrap_or(0);
    match n {
        1 => init_shrine(ctl, t, obj, level),
        2 => {
            init_urn(ctl, t, obj, level);
            Ok(())
        }
        3 => init_chest(ctl, t, w, obj, level),
        57 => {
            init_chest(ctl, t, w, obj, level)?;
            ctl.get_mut(obj)?.spark = 1;
            Ok(())
        }
        16 => {
            let class = ctl.get(obj)?.class;
            let p2 = t.object(class)?.parm2 as u8;
            ctl.get_mut(obj)?.interact = p2.wrapping_mul(2);
            Ok(())
        }
        5 => Ok(()),
        11 => init_town_portal(ctl, t, w, obj, level, room, x, y),
        12 => init_permanent_portal(ctl, t, w, obj, level, room, x, y),
        8 | 10 | 13 | 14 | 22 | 24 | 26 | 27 | 28 | 34 | 51 | 58 => {
            mech::init(ctl, t, w, obj, n, room, x, y)
        }
        _ => Ok(()),
    }
}

/// `0x0054F770` (§5.1 rule 3): the shrine pick of `class` for `level`.
pub fn shrine_pick(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    class: u8,
    level: u32,
) -> Result<u16, ObjectError> {
    let class = if class == 0 || class > 4 { 2 } else { class };
    let list = &ctl.shrine_lists[class as usize];
    if list.is_empty() {
        return Err(ObjectError::EmptyShrineList(class));
    }
    let mut id = 1;
    for _ in 0..8 {
        let i = ctl.seed.roll(list.len() as i32) as usize;
        id = list[i];
        if id == 0 {
            id = 1;
        }
        if level >= t.shrine(id)?.levelmin {
            break;
        }
    }
    Ok(id)
}

/// Init 1 (`0x0054F9D0`, §5.1).
fn init_shrine(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    obj: UnitId,
    level: u32,
) -> Result<(), ObjectError> {
    let class = ctl.get(obj)?.class;
    let parm0 = t.object(class)?.parm0;
    let mut id = if parm0 != 0 {
        let c = match parm0 {
            1 => 2,
            2 => 3,
            _ => {
                if ctl.seed.roll(10) == 0 {
                    1
                } else {
                    4
                }
            }
        };
        shrine_pick(ctl, t, c, level)?
    } else {
        let mut id = 1;
        for _ in 0..8 {
            let n = t.shrines.len() as i32 - 1;
            id = if n < 1 {
                1
            } else {
                ctl.seed.roll(n) as u16 + 1
            };
            if level >= t.shrine(id)?.levelmin {
                break;
            }
        }
        id
    };
    // Rule 4.
    id = match id {
        4 => 2,
        5 => 3,
        16 => 18,
        x => x,
    };
    // Rule 5.
    let d = ctl.get_mut(obj)?;
    d.interact = id as u8;
    d.shrine = Some(id);
    Ok(())
}

/// Init 2 (`0x0054FBB0`, §5.2).
fn init_urn(ctl: &mut ObjectControl, t: &ObjectTables, obj: UnitId, level: u32) {
    let lvl = i32::from(t.mon_lvl1(level));
    let r = ctl.seed.roll(100);
    let threshold = ((lvl / 8) + 5) as u16;
    let interact = if r < u32::from(threshold) {
        1 + ctl.seed.roll(8) as u8
    } else {
        0
    };
    if let Some(d) = ctl.data.get_mut(&obj) {
        d.interact = interact;
    }
}

/// Init 3 (`0x0054FCB0`, §5.2).
fn init_chest<W: ObjectWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    level: u32,
) -> Result<(), ObjectError> {
    init_urn(ctl, t, obj, level);
    let class = ctl.get(obj)?.class;
    let lockable = t.object(class)?.lockable != 0;
    let locked = if lockable {
        let lvl = i32::from(t.mon_lvl1(level));
        (ctl.seed.roll(100) as i32) < lvl / 2 + 8
    } else {
        false
    };
    let d = ctl.get_mut(obj)?;
    d.interact = if locked {
        d.interact | 0x80
    } else {
        d.interact & 0x7F
    };
    let lo = ctl.seed.step();
    if let Some(s) = w.unit_seed(obj) {
        *s = Seed::init_low(lo % 65534 + 1);
    }
    Ok(())
}

/// Init 11 (`0x00550140`, §5.5): town portal.
#[allow(clippy::too_many_arguments)]
fn init_town_portal<W: ObjectWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    level: u32,
    room: Option<RoomId>,
    x: i32,
    y: i32,
) -> Result<(), ObjectError> {
    let act = t.level(level).map_or(0, |l| l.act);
    let town = TOWNS.get(act as usize).copied().unwrap_or(0);
    if town > 255 {
        return Err(ObjectError::TownLevel(town));
    }
    let class = ctl.get(obj)?.class;
    ctl.get_mut(obj)?.interact = town as u8;
    if w.mode(obj) == 1 {
        w.stamp_footprint(obj, room, x, y);
        schedule_endanim(w, t.object(class)?, obj);
    }
    Ok(())
}

/// Init 12 (`0x0054FE70`, §5.5): permanent portal.
#[allow(clippy::too_many_arguments)]
fn init_permanent_portal<W: ObjectWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    level: u32,
    room: Option<RoomId>,
    x: i32,
    y: i32,
) -> Result<(), ObjectError> {
    let class = ctl.get(obj)?.class;
    let o = t.object(class)?;
    if w.mode(obj) == 0 {
        set_mode(t, w, obj, class, 1, true)?;
        w.stamp_footprint(obj, room, x, y);
        schedule_endanim(w, o, obj);
    }
    let tomb = w.staff_tomb_level();
    let d = ctl.get_mut(obj)?;
    let dest = match level {
        1 => Some(39),
        39 => Some(1),
        38 => Some(4),
        4 => Some(38),
        74 => Some(46),
        46 => Some(74),
        73 => Some(tomb),
        l if l == tomb => Some(73),
        121 => Some(109),
        109 => Some(121),
        125 => Some(111),
        126 => Some(112),
        127 => Some(117),
        _ => None,
    };
    if let Some(dest) = dest {
        d.interact = dest as u8;
    } else if let Some(want) = match level {
        111 => Some(125u8),
        112 => Some(126),
        117 => Some(127),
        _ => None,
    } {
        if d.interact != want {
            d.interact = want;
            let at = w.frame() + 1;
            w.schedule(obj, oevent::DELAYED_PORTAL, at);
        }
    }
    Ok(())
}

/// An object allocated from inside an object call (§6, §8.3): the unit
/// through [`ObjectWorld::allocate_object`], then, when the provider could
/// not run the init dispatch (the control is held by the caller), §3 on it
/// here.
#[allow(clippy::too_many_arguments)]
pub fn allocate<W: ObjectWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    room: RoomId,
    class: u16,
    x: i32,
    y: i32,
    mode: u8,
) -> Result<Option<UnitId>, ObjectError> {
    let Some(obj) = w.allocate_object(room, class, x, y, mode) else {
        return Ok(None);
    };
    if !ctl.data.contains_key(&obj) {
        let guid = w.guid(obj);
        create(ctl, t, w, obj, class, guid, Some(room), mode, x, y)?;
        w.add_object(obj, room, x, y);
    }
    Ok(Some(obj))
}

// ------------------------------------------------------------------ §6

/// `0x006E1080` rows {class, min, max} for presets 574–579 (§6).
pub const PRESET_SHRINES: [(u16, u16, u16); 6] = [
    (136, 2, 6),
    (136, 7, 7),
    (136, 8, 11),
    (136, 12, 12),
    (136, 1, 5),
    (136, 14, 14),
];

/// What a preset class 574–582 did (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// Class ≤ 573 or index ≥ 9: no handler.
    None,
    /// The object allocated (or `None` when the allocation failed).
    Object(Option<UnitId>),
    /// A handler whose behavior is not specified yet (581, 582, and 580
    /// outside level 25: open question 6).
    NotCovered(u32),
}

/// `0x0054F490` (§6): the spawner of DS1 preset classes beyond
/// `objects.txt`.
#[allow(clippy::too_many_arguments)]
pub fn create_preset<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    room: RoomId,
    level: u32,
    class: u32,
    x: i32,
    y: i32,
    mode: u8,
) -> Result<Preset, ObjectError> {
    if class <= 573 {
        return Ok(Preset::None);
    }
    let index = class - 574;
    if index >= 9 {
        return Ok(Preset::None);
    }
    match class {
        574..=579 => {
            let (c, min, max) = PRESET_SHRINES[index as usize];
            let Some(obj) = allocate(ctl, t, w, room, c, x, y, mode)? else {
                return Ok(Preset::Object(None));
            };
            let r = w
                .unit_seed(obj)
                .map_or(0, |s| s.roll(i32::from(max) - i32::from(min)));
            let mut id = min + r as u16;
            if id == 4 || id == 5 {
                id = 2;
            }
            if let Some(d) = ctl.data.get_mut(&obj) {
                d.interact = id as u8;
                d.shrine = Some(id);
            }
            Ok(Preset::Object(Some(obj)))
        }
        580 if level == 25 => {
            // Edge case 22 / §22: 580 runs 581's handler, which allocates in
            // mode 0 whatever the preset mode is.
            let Some(obj) = allocate(ctl, t, w, room, 371, x, y, 0)? else {
                return Ok(Preset::Object(None));
            };
            if let Some(d) = ctl.data.get_mut(&obj) {
                d.spark = 1;
                d.interact = 3;
            }
            set_flag(w, obj, oflags::KEEP_MODE, true);
            // `objects-2.md` §24 rule 1: the ordinary mode set (already in
            // mode 0: no setup, no draw; queued, flag 0x1).
            set_mode(t, w, obj, 371, 0, true)?;
            Ok(Preset::Object(Some(obj)))
        }
        _ => Ok(Preset::NotCovered(class)),
    }
}

// ------------------------------------------------------------------ §7

/// One operate call's record (§7.2 rule 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operate {
    pub object: UnitId,
    pub operator: Option<UnitId>,
    pub class: u16,
    pub operate_fn: u8,
}

/// What the dispatch did (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    /// A function ran here (or the dispatch returned without one): its
    /// return value.
    Done(i32),
    /// Operate 23: the caller runs `world/waypoints.md` §5.2.
    Waypoint(Operate),
    /// A quest function: the caller runs the quest spec's.
    Quest(Operate),
    /// A `todo` function: nothing ran. Also a portal (operate 15) whose
    /// rule 3 the host did not run (§12, open question 7).
    NotCovered(Operate),
}

/// `0x00584540` (§7.1): the entry with the range check. Returns the
/// entry's result (0: no such object, else 1) and what the dispatch did.
pub fn operate_in_range<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    operator: Option<UnitId>,
    guid: u32,
) -> Result<(i32, Option<Dispatch>), ObjectError> {
    let Some(obj) = w.find_object(guid) else {
        return Ok((0, None));
    };
    if let Some(op) = operator {
        if w.operator(op) == Operator::Monster {
            let class = ctl.get(obj)?.class;
            if t.object(class)?.monsterok == 0 {
                return Ok((1, None));
            }
        }
        if !w.in_interact_range(op, obj) {
            return Ok((1, None));
        }
    }
    let d = dispatch(ctl, t, w, obj, operator)?;
    Ok((1, Some(d)))
}

/// `0x00584420` (§7.2): the operate dispatch (skills call it directly).
pub fn dispatch<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    operator: Option<UnitId>,
) -> Result<Dispatch, ObjectError> {
    let class = ctl.get(obj)?.class;
    // Rule 2.
    if let Some(p) = operator {
        if let Operator::Player(_) = w.operator(p) {
            if w.interact_active(p)
                || w.player_busy(p)
                || (w.cursor_item(p) && class != STASH_CLASS)
            {
                return Ok(Dispatch::Done(0));
            }
        }
    }
    // Rule 3.
    let n = t.object(class)?.operatefn;
    if n >= OPERATE_FN_BOUND {
        return Err(ObjectError::OperateFn(n));
    }
    if REFUSED_CLASSES.contains(&class) {
        return Ok(Dispatch::Done(0));
    }
    crate::cov!(Object, class, n);
    let op = Operate {
        object: obj,
        operator,
        class,
        operate_fn: n,
    };
    // Rule 4.
    Ok(match operate_route(n) {
        Route::Null => Dispatch::Done(0),
        Route::Waypoint => Dispatch::Waypoint(op),
        Route::Quest => Dispatch::Quest(op),
        Route::NotCovered => Dispatch::NotCovered(op),
        Route::Here => match run_operate(ctl, t, w, &op)? {
            Some(r) => Dispatch::Done(r),
            None => Dispatch::NotCovered(op),
        },
    })
}

/// The operate functions this spec owns; `None`: the function stopped at a
/// step not specified yet (portal rule 3).
fn run_operate<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    op: &Operate,
) -> Result<Option<i32>, ObjectError> {
    match op.operate_fn {
        1 | 3 | 4 | 5 | 7 | 14 | 68 => chests::operate(ctl, t, w, op).map(Some),
        2 => shrines::operate(ctl, t, w, op).map(Some),
        8 => misc::door(ctl, t, w, op).map(Some),
        11 => misc::torch(ctl, t, w, op).map(Some),
        15 => misc::portal(ctl, t, w, op),
        22 => misc::well(ctl, t, w, op).map(Some),
        13 | 16..=20 | 26 | 27 | 29 | 30 | 32 | 47 | 48 | 50 | 51 | 61 => {
            mech::operate(ctl, t, w, op).map(Some)
        }
        _ => Ok(Some(0)),
    }
}

// ------------------------------------------------------------------ events

/// What an object timer event did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventRun {
    Done,
    /// Event 7: the quest object event (`world/quests.md`).
    Quest,
    /// An event type no spec gives a handler.
    NotCovered(u8),
}

/// The object timer event handlers (`sim/units.md` §6.4).
pub fn object_event<W: ObjectHost>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
    ev: u8,
) -> Result<EventRun, ObjectError> {
    match ev {
        oevent::END_ANIM => end_anim(ctl, t, w, obj)?,
        oevent::WELL_REFILL => misc::well_refill(ctl, t, w, obj)?,
        oevent::TRAP => chests::trap_event(ctl, t, w, obj)?,
        oevent::SHRINE_RESET => shrines::reset_event(ctl, t, w, obj)?,
        oevent::HOVER => shrines::hover_event(ctl, t, w, obj)?,
        oevent::QUEST => return Ok(EventRun::Quest),
        oevent::DELAYED_PORTAL => delayed_portal(ctl, t, w, obj)?,
        // `objects-2.md` §18.
        0 | 3 | 8 | 9 | 10 => mech::event(ctl, t, w, obj, ev)?,
        e => return Ok(EventRun::NotCovered(e)),
    }
    Ok(EventRun::Done)
}

/// Event 1 `0x00581490` (`objects-2.md` §18.6): mode 1 and `Mode2` ≠ 0 →
/// the mode field := 2 written directly (no setup, draw, queue or
/// changed flag); inside that branch, `HasCollision2` = 0 → free the
/// footprint.
fn end_anim<W: ObjectWorld>(
    ctl: &mut ObjectControl,
    t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<(), ObjectError> {
    let class = ctl.get(obj)?.class;
    let o = t.object(class)?;
    if w.mode(obj) == 1 && o.mode2 != 0 {
        w.store_mode(obj, 2);
        if o.hascollision2 == 0 {
            w.free_footprint(obj);
        }
    }
    Ok(())
}

/// Event 11 `0x00581410` (`sim/units.md` §6.4): a portal object by the
/// object's level: 111 → 125, 112 → 126, else 127.
///
/// TODO(units.md §6.4 type 11): the creator `0x0056CF40`'s placement and
/// mode are not stated; the matching portal is requested from the seam.
fn delayed_portal<W: ObjectHost>(
    _ctl: &mut ObjectControl,
    _t: &ObjectTables,
    w: &mut W,
    obj: UnitId,
) -> Result<(), ObjectError> {
    let level = w.level(obj).unwrap_or(0);
    let row = match level {
        111 => 125,
        112 => 126,
        _ => 127,
    };
    w.create_level_portal(obj, row);
    Ok(())
}

// ------------------------------------------------------------------ §14

/// S→C 0x0E (`0x0053B470`, 12 bytes, §14 rule 1).
pub fn state_message(guid: u32, selectable: bool, mode: u32) -> [u8; 12] {
    let mut m = [0u8; 12];
    m[0] = 0x0E;
    m[1] = 2;
    m[2..6].copy_from_slice(&guid.to_le_bytes());
    m[6] = 3;
    m[7] = u8::from(selectable);
    m[8..12].copy_from_slice(&mode.to_le_bytes());
    m
}

/// S→C 0x4D (`0x0053D4D0`, 17 bytes, §14 rule 1).
pub fn shrine_message(object: u32, operator: u32, code: u8) -> [u8; 17] {
    let mut m = [0u8; 17];
    m[0] = 0x4D;
    m[1] = 2;
    m[2..6].copy_from_slice(&object.to_le_bytes());
    m[6..10].copy_from_slice(&operator.to_le_bytes());
    m[10] = code;
    m
}

/// S→C 0x60 (`0x0053D900`, 7 bytes, §14 builder details): portal flags
/// (+0x05), `InteractType` (destination level), object GUID.
pub fn portal_message(d: &ObjectData) -> [u8; 7] {
    let g = d.guid.to_le_bytes();
    [0x60, d.portal_flags, d.interact, g[0], g[1], g[2], g[3]]
}

/// What the update pass sends for one queued object (§14 rule 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateMessage {
    State([u8; 12]),
    Shrine([u8; 17]),
    /// S→C 0x60 ([`portal_message`]).
    Portal([u8; 7]),
}

/// `0x00581AD0` rule 1 for one queued object: the messages it sends to
/// each client, in order. Rule 2 (sound, hover, flags 2, `0x00571CD0`)
/// goes through [`MiscWorld::update_extras`].
pub fn update_messages<W: ObjectWorld>(
    ctl: &ObjectControl,
    t: &ObjectTables,
    w: &W,
    obj: UnitId,
) -> Result<Vec<UpdateMessage>, ObjectError> {
    let mut out = Vec::new();
    let flags = w.flags(obj);
    if flags & oflags::CHANGED == 0 {
        return Ok(out);
    }
    let d = ctl.get(obj)?;
    let mode = w.mode(obj);
    out.push(UpdateMessage::State(state_message(
        d.guid,
        flags & oflags::SELECTABLE != 0,
        u32::from(mode),
    )));
    let o = t.object(d.class)?;
    if o.subclass & 4 != 0 {
        out.push(UpdateMessage::Portal(portal_message(d)));
    } else if mode == 1 && o.subclass & 1 != 0 && d.operator != 0 {
        let code = match d.shrine {
            Some(id) => t.shrine(id)?.code,
            None => 0,
        };
        // §14 builder details: the operator field − 1 (the operator's
        // GUID).
        out.push(UpdateMessage::Shrine(shrine_message(
            d.guid,
            d.operator.wrapping_sub(1),
            code,
        )));
    }
    Ok(out)
}

#[cfg(test)]
mod mutant_tests;
