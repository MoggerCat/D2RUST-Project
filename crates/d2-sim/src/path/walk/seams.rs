// Spec: specs/sim/pathing.md (Inputs; the seams to path-placement.md, units.md, stats.md, skills)
//! What walk code needs from systems it does not own.
//!
//! [`WalkPath`] lists the dynamic-path fields (`sim/path-placement.md`
//! §2.3, D2MOO `D2DynamicPathStrc`) this module reads and writes, under
//! the spec's names. The path record type itself belongs to the path core
//! (`path-placement.md` §1–§6); a provider copies its record into a
//! `WalkPath` and back ([`PathWorld::load_path`], [`PathWorld::store_path`]).
//! [`PathWorld`] is the collision, room and footprint side
//! (`path-placement.md` §4–§6). [`WalkUnits`] is everything unit-, mode-,
//! stat-, skill- and message-shaped; its defaults are the narrowest
//! reading (nothing happens, or the value that makes the caller do
//! nothing), like `wiring::action::Pending`, and each default names the
//! spec that will own it.

use crate::game::Game;
use crate::rng::Seed;
use crate::units::{ClientId, RoomId, UnitId, UnitType};

/// A sub-tile point (path points, targets; u16 in the record).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }
}

/// Point slots in a path (+0x9C: 78 × {u16 x, u16 y}).
pub const MAX_POINTS: usize = 78;
/// Saved-step slots (+0x1D8: 10 × {u16 x, u16 y}).
pub const MAX_SAVED_STEPS: usize = 10;

/// Path flags (+0x34), `path-placement.md` §2.3.
pub mod flag {
    /// A path point (or a move's destination) lies outside the room.
    pub const OUTSIDE_ROOM: u32 = 0x1;
    /// Room changed since the room-change messages.
    pub const ROOM_CHANGED: u32 = 0x2;
    /// Move footprints without testing.
    pub const FORCED: u32 = 0x4;
    /// The last step crossed at least one cell.
    pub const CROSSED: u32 = 0x8;
    /// Keeps the target (allocation argument).
    pub const KEEP_TARGET: u32 = 0x10;
    /// A path is active.
    pub const ACTIVE: u32 = 0x20;
    /// Face away (−32 on the computed direction).
    pub const FACE_AWAY: u32 = 0x200;
    /// Remove the target unit's footprint while computing (§3 step 6).
    pub const REMOVE_TARGET_FOOTPRINT: u32 = 0x800;
    /// Prepare a blocked target (§4).
    pub const PREPARE_TARGET: u32 = 0x1000;
    pub const SAVE_PREV_TYPE: u32 = 0x2000;
    pub const PREV_TYPE_KEPT: u32 = 0x4000;
    pub const SAVE_VELOCITY: u32 = 0x8000;
    pub const VELOCITY_KEPT: u32 = 0x10000;
    /// Store saved steps (§9.4).
    pub const SAVE_STEPS: u32 = 0x20000;
    /// Missile path.
    pub const MISSILE: u32 = 0x40000;
    /// The path-type bits replaced by a type change (`0x00648CF0`).
    pub const TYPE_BITS: u32 = 0x7FF00;
}

/// The target unit (+0x58 pointer, +0x5C type, +0x60 GUID).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetUnit {
    pub unit: UnitId,
    pub ty: UnitType,
    pub guid: u32,
}

/// The dynamic-path fields walk code uses (`path-placement.md` §2.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalkPath {
    /// +0x00, +0x04: precise x, y (16.16).
    pub precise_x: u32,
    pub precise_y: u32,
    /// +0x08, +0x0C: client x, y (`path-placement.md` §1 rule 3).
    pub client_x: i32,
    pub client_y: i32,
    /// +0x10: target.
    pub target: Point,
    /// +0x14: previous target.
    pub prev_target: Point,
    /// +0x18: final target.
    pub final_target: Point,
    /// +0x1C: room.
    pub room: Option<RoomId>,
    /// +0x20: previous room.
    pub prev_room: Option<RoomId>,
    /// +0x24, +0x28: current point index, point count.
    pub index: i32,
    pub count: i32,
    /// +0x30: owner unit.
    pub owner: UnitId,
    /// +0x34: flags ([`flag`]).
    pub flags: u32,
    /// +0x38: 15 after a velocity change, 0 after a new path (not read).
    pub field_38: u32,
    /// +0x3C, +0x40: path type, previous path type.
    pub path_type: u8,
    pub prev_type: u8,
    /// +0x44: unit size.
    pub size: i32,
    /// +0x48: collision pattern.
    pub pattern: u8,
    /// +0x4C: footprint mask.
    pub footprint_mask: u16,
    /// +0x50: move-test mask.
    pub move_mask: u16,
    /// +0x54: collided-with mask.
    pub collided: u16,
    /// +0x58..+0x60: target unit.
    pub target_unit: Option<TargetUnit>,
    /// +0x64, +0x65, +0x66: direction, new direction, turn step.
    pub direction: u8,
    pub new_direction: u8,
    pub turn_step: i8,
    /// +0x68: target lead.
    pub lead: u8,
    /// +0x6A, +0x6E: direction vector (length 4096).
    pub dir_vec: (i32, i32),
    /// +0x72, +0x76: velocity vector (16.16 per tick).
    pub vel_vec: (i32, i32),
    /// +0x7C velocity, +0x80 saved, +0x84 max, +0x88 acceleration,
    /// +0x8C acceleration counter.
    pub velocity: i32,
    pub saved_velocity: i32,
    pub max_velocity: i32,
    pub acceleration: i32,
    pub accel_counter: i32,
    /// +0x90 distance budget, +0x91 max path distance, +0x92 IDA* start
    /// score, +0x93 stop distance.
    pub distance_budget: u8,
    pub max_distance: u8,
    pub idastar_score: u8,
    pub stop_distance: u8,
    /// +0x98: direction offset of the path type.
    pub dir_offset: i32,
    /// +0x9C: points.
    pub points: [Point; MAX_POINTS],
    /// +0x1D4, +0x1D8: saved-step count, saved steps.
    pub saved_count: i32,
    pub saved_steps: [Point; MAX_SAVED_STEPS],
}

impl WalkPath {
    /// A zeroed record (allocation zeroes 0x200 bytes) owned by `owner`.
    pub fn zeroed(owner: UnitId) -> WalkPath {
        WalkPath {
            precise_x: 0,
            precise_y: 0,
            client_x: 0,
            client_y: 0,
            target: Point::default(),
            prev_target: Point::default(),
            final_target: Point::default(),
            room: None,
            prev_room: None,
            index: 0,
            count: 0,
            owner,
            flags: 0,
            field_38: 0,
            path_type: 0,
            prev_type: 0,
            size: 0,
            pattern: 0,
            footprint_mask: 0,
            move_mask: 0,
            collided: 0,
            target_unit: None,
            direction: 0,
            new_direction: 0,
            turn_step: 0,
            lead: 0,
            dir_vec: (0, 0),
            vel_vec: (0, 0),
            velocity: 0,
            saved_velocity: 0,
            max_velocity: 0,
            acceleration: 0,
            accel_counter: 0,
            distance_budget: 0,
            max_distance: 0,
            idastar_score: 0,
            stop_distance: 0,
            dir_offset: 0,
            points: [Point::default(); MAX_POINTS],
            saved_count: 0,
            saved_steps: [Point::default(); MAX_SAVED_STEPS],
        }
    }

    /// The current sub-tile (high 16 bits of the precise position).
    pub fn cell(&self) -> Point {
        Point::new((self.precise_x >> 16) as i32, (self.precise_y >> 16) as i32)
    }

    /// The live points `points[0..count]`.
    pub fn live_points(&self) -> &[Point] {
        let n = self.count.clamp(0, MAX_POINTS as i32) as usize;
        &self.points[..n]
    }
}

/// A fatal error of the original (an assert or a null dereference the
/// game does not survive).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WalkError {
    #[error("fatal: {0}")]
    Fatal(&'static str),
}

/// The skills.txt fields of the unit's used skill that walk code reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UsedSkill {
    /// Skill id.
    pub id: u16,
    /// `SeqInput` (+0x16).
    pub seq_input: i32,
    /// `srvdofunc` (+0x2E).
    pub srvdofunc: i32,
    /// `interrupt` flag (byte +7 & mask `0x006CE284`).
    pub interrupt: bool,
    /// The skill flags read by `0x006446A0` (skill +0x0C).
    pub skill_flags: u32,
}

/// What the path core provides: path records, collision queries,
/// footprints, room lookups and room lists. Provider: `impl-path-core`
/// (`sim/path-placement.md` §1–§6) on the DRLG rooms.
pub trait PathWorld {
    /// Copies the unit's dynamic path into a [`WalkPath`]; `None` = no path
    /// (unit +0x2C null).
    fn load_path(&self, unit: UnitId) -> Option<WalkPath>;
    /// Writes the walk fields back into the unit's path record.
    fn store_path(&mut self, unit: UnitId, path: &WalkPath);
    /// Cell lookup `0x00463740(room, x, y)` (`path-placement.md` §4 rule 1).
    fn cell_room(&self, room: Option<RoomId>, x: i32, y: i32) -> Option<RoomId>;
    /// The room's sub-tile rect (room +0x4C x, +0x50 y, +0x54 w, +0x58 h).
    fn room_rect(&self, room: RoomId) -> (i32, i32, i32, i32);
    /// The room is in a town (`0x0061AB00`).
    fn room_in_town(&self, room: RoomId) -> bool;
    /// Pattern query `0x0064D910`: 1 if any cell of `pattern` at (x, y)
    /// collides with `mask`, looked up from `room` (§4 rule 5).
    fn pattern_collides(
        &self,
        room: Option<RoomId>,
        x: i32,
        y: i32,
        pattern: u8,
        mask: u16,
    ) -> bool;
    /// Footprint remove `0x00649560(unit, force)` at the unit's stored
    /// position (§5.2); returns whether it cleared.
    fn remove_footprint(&mut self, unit: UnitId, force: bool) -> bool;
    /// Footprint add `0x00649400(unit)` at the unit's stored position.
    fn add_footprint(&mut self, unit: UnitId);
    /// Try move `0x0064EDA0(room, old, new, pattern, foot, test)` (§6 rule
    /// 1): the pattern query result at `new` (0 = moved).
    fn try_move(
        &mut self,
        room: Option<RoomId>,
        old: Point,
        new: Point,
        pattern: u8,
        foot: u16,
        test: u16,
    ) -> u16;
    /// Forced move `0x0064EFA0` (§6 rule 2).
    fn forced_move(&mut self, room: Option<RoomId>, old: Point, new: Point, pattern: u8, foot: u16);
    /// Missile move `0x0064ED20` (§6 rule 3, size shapes); returns the
    /// query result.
    fn missile_move(
        &mut self,
        room: Option<RoomId>,
        old: Point,
        new: Point,
        size: i32,
        foot: u16,
        test: u16,
    ) -> u16;
    /// The unit leaves its room's unit list (`0x0064C370`).
    fn room_list_remove(&mut self, unit: UnitId, room: RoomId);
    /// Room list insert (`0x0064C350`).
    fn room_list_insert(&mut self, unit: UnitId, room: RoomId);
    /// Queue for update (`0x0064C040`, `unit-order.md` §6).
    fn queue_for_update(&mut self, unit: UnitId);
    /// The room's client array, sorted by client address (`drlg/rooms.md`
    /// §7).
    fn room_clients(&self, room: RoomId) -> Vec<ClientId>;
}

/// Units, modes, timers, stats, skills and messages. Every default is
/// the narrowest reading; the owner spec is named on each.
#[allow(unused_variables)]
pub trait WalkUnits {
    /// Unit type (unit +0x00).
    fn unit_type(&self, unit: UnitId) -> UnitType;
    /// Class (unit +0x04).
    fn class(&self, unit: UnitId) -> u32 {
        0
    }
    /// GUID (unit +0x0C).
    fn guid(&self, unit: UnitId) -> u32 {
        unit.0
    }
    /// Current mode (unit +0x10).
    fn mode(&self, unit: UnitId) -> u32;
    /// Unit lookup by type and GUID (`0x00552F60`).
    fn find_unit(&self, ty: UnitType, guid: u32) -> Option<UnitId> {
        None
    }
    /// Position of any unit in sub-tiles (`path-placement.md` §2.1).
    fn position(&self, unit: UnitId) -> Point {
        Point::default()
    }
    /// Unit size (`0x00620510`, `path-placement.md` §3).
    fn unit_size(&self, unit: UnitId) -> i32 {
        0
    }
    /// Door test `0x00621A70`; `Some(orientation)` (`0x00621AC0`) for a
    /// door. Owner: the objects spec.
    fn door_orientation(&self, unit: UnitId) -> Option<bool> {
        None
    }
    /// The smallest positive expire among the unit's type-1 timers
    /// (`0x005415A0`); 0 if none. Owner: `sim/tick.md`.
    fn first_type1_expire(&self, game: &Game, unit: UnitId) -> i32 {
        0
    }
    /// Inventory cursor item (inventory +0x20 ≠ 0). Owner: inventory spec.
    fn has_cursor_item(&self, unit: UnitId) -> bool {
        false
    }
    fn has_state(&self, unit: UnitId, state: u16) -> bool {
        false
    }
    /// `stat` of the stat list of `state` (`0x00625D00`).
    fn state_stat(&self, unit: UnitId, state: u16, stat: u16) -> i32 {
        0
    }
    /// Stat value (`sim/stats.md`).
    fn stat(&self, unit: UnitId, stat: u16) -> i32 {
        0
    }
    /// Item/skill stat getter (`0x00625500`).
    fn item_stat(&self, unit: UnitId, stat: u16) -> i32 {
        0
    }
    /// Base stat add (`0x006272B0`).
    fn add_base_stat(&mut self, game: &mut Game, unit: UnitId, stat: u16, delta: i32) {}
    /// Base stat set.
    fn set_base_stat(&mut self, game: &mut Game, unit: UnitId, stat: u16, value: i32) {}
    /// The unit seed (unit +0x20).
    fn seed(&mut self, unit: UnitId) -> &mut Seed;
    /// The used skill's skills.txt fields (`0x00620250`); `None` = no used
    /// skill or no row.
    fn used_skill(&self, unit: UnitId) -> Option<UsedSkill> {
        None
    }
    /// Sets the used skill (`0x00620210`; `None` = no skill).
    fn set_used_skill(&mut self, unit: UnitId, skill: Option<u16>) {}
    /// Player data +0x154 := 0 and +0x150 := 0 (§1.2 step 4).
    fn clear_queued_action(&mut self, unit: UnitId) {}
    /// Mode set `0x00553570` (`sim/units.md` §4.1). The velocity half of
    /// the animation-rate routine `0x00623F50` is [`super::velocity`]; the
    /// walk entry points run it right after this call.
    fn set_mode(&mut self, game: &mut Game, unit: UnitId, mode: u32) {}
    /// Cancel the unit's events of a type (`0x00553990`).
    fn cancel_events(&mut self, game: &mut Game, unit: UnitId, ty: u8) {}
    /// Every-tick event 0 (`0x00553F00`, `sim/units.md` §4.4).
    fn schedule_event0(&mut self, game: &mut Game, unit: UnitId) {}
    /// Start function of a mode other than 2, 3, 6, 19 (table
    /// `0x006E1740`), point or unit form. Owner: the mode's spec.
    fn start_other_mode(&mut self, game: &mut Game, unit: UnitId, mode: u32, target: StartTarget) {}
    /// Starting mode 3 or 19 attaches the run stat list (`0x00620E80`,
    /// flag 4, stat 67 = `value`, §8.2). Owner: `sim/stat-lists.md`.
    fn attach_run_stats(&mut self, game: &mut Game, unit: UnitId, value: i32) {}
    /// charstats `WalkVelocity`, `RunVelocity`, `RunDrain` of a player.
    fn charstats_velocity(&self, unit: UnitId) -> (i32, i32, i32) {
        (0, 0, 0)
    }
    /// monstats `Velocity` and the `npc` bit of a monster.
    fn monstats_velocity(&self, unit: UnitId) -> (i32, bool) {
        (0, false)
    }
    /// armor `speed` of the torso item (body location 3); `None` = none.
    fn torso_speed(&self, unit: UnitId) -> Option<i32> {
        None
    }
    /// The monster may be in town (`0x0063E860`, `path-placement.md` §3).
    fn monster_can_be_in_town(&self, unit: UnitId) -> bool {
        true
    }
    /// Unit flag set (unit +0xC4).
    fn set_unit_flag(&mut self, unit: UnitId, bit: u32) {}
    /// State 13 event-0 branch (`0x005C9D90`). Owner: the skills spec.
    fn state13_step(&mut self, game: &mut Game, unit: UnitId) {}
    /// A monster's AI room memo (monster data +0x50) := 0.
    fn clear_ai_room_memo(&mut self, unit: UnitId) {}
    /// The client's player (client record); `None` = none.
    fn client_player(&self, client: ClientId) -> Option<UnitId> {
        None
    }
    /// Unit removal message (`0x00571F90`). Owner: the unit-update spec.
    fn send_unit_removal(&mut self, game: &mut Game, client: ClientId, unit: UnitId) {}
    /// Unit add messages (`0x00571600`). Owner: the unit-update spec.
    fn send_unit_add(&mut self, game: &mut Game, client: ClientId, unit: UnitId) {}
    /// Target lead `0x00679190` / `0x00679250` (path +0x68 ≠ 0): x87
    /// floating point, pathing.md open question 4. `None` = unspecified;
    /// the caller then keeps the target unchanged.
    fn target_lead(&self, unit: UnitId, target: Point, lead: u8) -> Option<Point> {
        None
    }
    /// Monster circling `0x00679B30` (direction offset ≠ 0), and the
    /// path functions of types 0, 3, 8, 9, 11, 12, 15, 16 (pathing.md open
    /// question 3). Returns the point count; the default finds no path.
    fn other_path_function(&mut self, path: &mut WalkPath, info: &PathInfo) -> i32 {
        0
    }
    /// `0x00649120`: a player's or monster's re-path budget (pathing.md
    /// open question 8). Default 0 = no re-path.
    fn repath_budget(&self, unit: UnitId) -> i32 {
        0
    }
}

/// Target of a mode start (point or unit form).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartTarget {
    Point(Point),
    Unit(UnitId),
}

/// The record path functions receive (D2MOO `D2PathInfoStrc`, §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathInfo {
    pub start: Point,
    pub target: Point,
    pub start_room: Option<RoomId>,
    pub target_room: Option<RoomId>,
    /// Target slack r (§3 step 4).
    pub slack: i32,
    pub max_distance: i32,
    pub idastar_score: i32,
    pub path_type: u8,
    pub size: i32,
    pub pattern: u8,
    pub move_mask: u16,
}
