// Spec: specs/monsters/population.md (Related specs; the seams to other systems)
//! What population needs from systems owned by other specs. Each trait
//! names its expected provider; tests use a fake. The decisions (draws,
//! tests, counts) stay in the population modules.
//!
//! [`MonsterInit`] is the narrow seam to monster creation itself
//! (`monsters/init.md`, implemented in a parallel session): allocation,
//! alignment, init, boss modifiers and monster data writes.

use super::{CoordRect, PopState, PresetUnit, RoomBox, TileRec};
use crate::rng::Seed;
use crate::units::{RoomId, UnitId};

/// Rooms, coordinates, collision, seeds and unit positions. Provider: the
/// DRLG (`drlg/levels.md`, `drlg/rooms.md`, `drlg/preset.md`), units
/// (`sim/units.md`) and quests (`world/quests.md`).
pub trait PopWorld {
    /// The game seed (game +0xD0).
    fn game_seed(&mut self) -> &mut Seed;
    /// The active room seed (room +0x6C).
    fn room_seed(&mut self, room: RoomId) -> &mut Seed;
    /// A unit's seed (unit +0x20).
    fn unit_seed(&mut self, unit: UnitId) -> &mut Seed;
    /// `0x0061A1B0`: level id of the room.
    fn room_level(&self, room: RoomId) -> i32;
    /// `0x0061A1F0`: level id of the populated room (0 = none).
    fn populated_level(&self, room: RoomId) -> i32;
    /// `0x0061ABF0(act, level)`: the level's populated-room count.
    fn populated_room_count(&self, act: u8, level: i32) -> i32;
    /// `0x0061AD50`: the room's coordinate records, in `next` order.
    fn coord_list(&self, room: RoomId) -> Vec<CoordRect>;
    /// `0x0061AD30(room, x, y)`: the coordinate record at a point.
    fn coord_at(&self, room: RoomId, x: i32, y: i32) -> Option<CoordRect>;
    /// `0x0061B130`: coordinate index at a subtile point.
    fn coord_index_at(&self, room: RoomId, x: i32, y: i32) -> i32;
    /// `0x00619730`: the room's subtile box.
    fn room_box(&self, room: RoomId) -> RoomBox;
    /// `0x0061AC10`: the room's warp points (subtiles).
    fn warp_points(&self, room: RoomId) -> Vec<(i32, i32)>;
    /// `0x006427F0`: the level's spawn location of `kind` (tiles), for the
    /// room's level.
    fn spawn_location(&self, room: RoomId, kind: u8) -> Option<(i32, i32)>;
    /// `0x00463740(room, x, y)`: the room holding a point.
    fn room_at(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId>;
    /// `0x0064D9B0(room, x, y, size, mask) ≠ 0` (`sim/units.md`).
    fn collides(&self, room: RoomId, x: i32, y: i32, size: i32, mask: u16) -> bool;
    /// `0x0064CB30(room, x, y, mask) ≠ 0`.
    fn mask_at(&self, room: RoomId, x: i32, y: i32, mask: u16) -> bool;
    /// `0x00619660`: the room's tile records, in order.
    fn tile_records(&self, room: RoomId) -> Vec<TileRec>;
    /// `0x00619FD0`: the room's preset units, in list order.
    fn preset_units(&self, room: RoomId) -> Vec<PresetUnit>;
    /// Client count (room +0x78).
    fn client_count(&self, room: RoomId) -> u32;
    /// `0x00620BB0`: the room a unit stands in.
    fn unit_room(&self, unit: UnitId) -> Option<RoomId>;
    /// The unit's subtile position (path x/y, or static path).
    fn unit_position(&self, unit: UnitId) -> (i32, i32);
    /// `0x005444B0`: a quest flag (quests spec).
    fn quest_flag(&self, flag: u8) -> bool;
    /// `0x005B5210(game) ≠ 0` (Act 4 Diablo quest state).
    fn chaos_blocks_population(&self) -> bool;
    /// `0x0064E840`: the nearest free point (mask 0x3C01, size 1).
    fn nearest_free_point(&self, room: RoomId, x: i32, y: i32) -> Option<(RoomId, i32, i32)>;
}

/// Who `0x0058F030` names as the owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerKey {
    /// The unit's GUID.
    Guid(UnitId),
    /// The owner data of a unit (`0x00451F50`).
    DataOf(UnitId),
}

/// `0x00555230` arguments for a monster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alloc {
    pub class: i32,
    pub x: i32,
    pub y: i32,
    pub room: RoomId,
    pub mode: u8,
    /// `Some` with creation flag 0x20 (allocation flag 3); else flag 1.
    pub guid: Option<u32>,
}

/// Monster creation and monster data. Provider: `monsters/init.md`
/// (monster data, modifiers, init), `sim/units.md` (allocation, flags),
/// `monsters/ai.md` (owner data), objects and quests specs.
pub trait MonsterInit {
    /// `0x00555230(type 1, …)`: one game-seed step, a GUID, the unit.
    /// The allocator's monster type init (`monsters/init.md` §5 step 4)
    /// reads and extends the game's regions (§2.5 `0x00547BC0`), so they
    /// come with the call.
    fn allocate_monster(&mut self, a: Alloc, state: &mut PopState) -> Option<UnitId>;
    /// `0x00573570(unit, flag, 1)`.
    fn set_monster_flag(&mut self, unit: UnitId, flag: u32);
    /// `0x00552D60`: the coordinate record: `rect`, or with `None` the one
    /// of `0x0061AD30(room, x, y)`.
    fn set_coord_record(
        &mut self,
        unit: UnitId,
        rect: Option<CoordRect>,
        room: RoomId,
        x: i32,
        y: i32,
    );
    /// `0x005543B0`: the creation alignment (0 evil, 1, 2).
    fn set_alignment(&mut self, unit: UnitId, align: u8);
    /// Unit flags (+0xC4) `|=`.
    fn set_unit_flags(&mut self, unit: UnitId, flags: u32);
    /// `0x005B21B0`: per-class extras.
    fn class_extras(&mut self, unit: UnitId);
    /// `0x005B1CF0`: monster init.
    fn init_monster(&mut self, unit: UnitId);
    /// Monstats row of a unit.
    fn unit_class(&self, unit: UnitId) -> i32;
    /// `0x00573520`: the monster's level id.
    fn unit_level(&self, unit: UnitId) -> i32;
    /// Monster type flags (monster data +0x16).
    fn type_flags(&self, unit: UnitId) -> u16;
    /// Monster type flags `|=`.
    fn set_type_flags(&mut self, unit: UnitId, flags: u16);
    /// `0x005A0760(boss, game, champion allowed)`.
    fn boss_modifiers(&mut self, boss: UnitId, champion_allowed: bool);
    /// The tail of `0x005A2120`: init functions of modifiers 1–4 and the
    /// boss's own modifiers.
    fn boss_modifier_init(&mut self, boss: UnitId);
    /// `0x005A48C0` (modifier 16) / `0x005A4850(…, m, 1)` (modifier 22).
    /// `0x005A48C0` counts the unit in its region (`monsters/init.md`
    /// §16.2, `0x005A0320`), so the regions come with the call.
    fn add_modifier(&mut self, unit: UnitId, m: u8, state: &mut PopState);
    /// `0x005A0930`: transfer the boss modifiers with `xfer`.
    fn transfer_modifiers(&mut self, boss: UnitId, minion: UnitId);
    /// `0x0058F030(game, unit, owner, a, b, c)`.
    fn set_owner_data(&mut self, unit: UnitId, owner: OwnerKey, a: i32, b: i32, c: i32);
    /// `0x0058F030` for a unique's minion (§6.5 step 4). TODO(spec:
    /// population.md §6.5 r4): the arguments are not stated.
    fn unique_minion_owner_data(&mut self, boss: UnitId, minion: UnitId);
    /// `0x0058F100`: add to the leader's minion list.
    fn add_minion(&mut self, leader: UnitId, minion: UnitId);
    /// `0x005DD330`: owner GUID and type.
    fn set_owner(&mut self, minion: UnitId, owner: UnitId);
    /// `0x00544E80`: the quest hook of a new boss.
    fn boss_quest_hook(&mut self, boss: UnitId);
    /// `0x005A0200` and the rest of superunique init (§11.4 step 4).
    fn superunique_init(&mut self, boss: UnitId, su: i32);
    /// Owner data of the hcIdx 60 boss. TODO(spec: population.md §11.4
    /// r6): the arguments are not stated.
    fn superunique_owner_data(&mut self, boss: UnitId);
    /// `0x005B24E0(boss, class, a, b, c, flags)`. TODO(spec: population.md
    /// open question 4): argument meaning not read.
    fn group_spawn(&mut self, boss: UnitId, class: i32, a: i32, b: i32, c: i32, flags: u16);
    /// `0x00555230(type 2, class, …)`: an object.
    fn create_object(&mut self, room: RoomId, class: i32, x: i32, y: i32);
    /// The object of a barricade door (§11.3 step 3). TODO(spec:
    /// population.md open question 3): object ids not read.
    fn barricade_object(&mut self, unit: UnitId, class: i32);
    /// `0x0058F000`, then `0x00666120` when the preset has data.
    fn preset_created(&mut self, unit: UnitId, preset: &PresetUnit);
    /// `0x005417D0`: event 7 at frame + 250 + `roll(50)`. TODO(spec:
    /// population.md §11.5 r4): the seed of the `roll(50)` is not stated.
    fn schedule_monumod(&mut self, unit: UnitId);
    /// `0x005B1990(game, unit, a, b)` (open question 7).
    fn change_alignment(&mut self, unit: UnitId, a: i32, b: i32);
    /// `0x00542B40`: inactive unit restore (`sim/units.md`).
    fn restore_inactive_units(&mut self, room: RoomId);
    /// `0x00552610`: object population (objects spec).
    fn populate_objects(&mut self, room: RoomId);
}

/// Everything population calls.
pub trait PopHost: PopWorld + MonsterInit {}
impl<T: PopWorld + MonsterInit + ?Sized> PopHost for T {}
