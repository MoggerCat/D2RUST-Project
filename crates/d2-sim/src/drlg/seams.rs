// Spec: specs/drlg/levels.md, specs/drlg/rooms.md
//! The traits the DRLG needs from systems other specs own, and the
//! bundle of services a room build uses.
//!
//! | Seam | Expected provider |
//! |---|---|
//! | [`LevelTypes`] | level type specs `drlg/preset.md`, `drlg/maze.md`, `drlg/outdoor.md` (future DRLG sessions) |
//! | [`ActRooms`] | `crate::units::UnitLists` (implemented in [`super::active`]) |
//! | [`TileSource`] | the caller holding parsed DT1s (`d2_formats::dt1::Dt1`; d2-server / world) |

use crate::units::RoomId;

use super::data::DrlgData;
use super::level::Drlg;
use super::tiles::RoomGrids;
use super::{DrlgError, DrlgRoomId, LevelIdx};

/// The level-type generators (`levels.md` §3.7, §4.3, §5.2, §9.4;
/// `rooms.md` §2.5, §4 handler 3, §9.2 step 3c). Every method receives
/// the DRLG; a method that allocates levels calls
/// [`Drlg::get_or_alloc_level`] with `self` as the `types` argument.
///
/// Draws: each method draws only what its own spec says, from the seeds
/// it is given access to through `drlg` (level seeds, room seeds, the
/// DRLG seed for the act placer).
#[allow(unused_variables)]
pub trait LevelTypes {
    /// `0x00678AD0` (`levels.md` §3.7, `drlg/outdoor.md`): create and
    /// place the act's levels at DRLG creation. Draws from the DRLG seed.
    fn create_act_levels(&mut self, drlg: &mut Drlg, data: &DrlgData) -> Result<(), DrlgError> {
        Ok(())
    }

    /// Level-data init at allocation (`levels.md` §4.3): maze
    /// `0x00673B10`, preset `0x00667430` (preset file draw), outdoor
    /// `0x00675320`. The level is not yet in the list. For maze and preset
    /// levels the DRLG applies §6.1 (size and position) after this call.
    fn init_level(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        level: LevelIdx,
    ) -> Result<(), DrlgError> {
        Ok(())
    }

    /// Room generation (`levels.md` §5.2): maze `0x00673B30`, preset
    /// `0x00668100`, outdoor `0x00675360`. Creates rooms with
    /// [`Drlg::alloc_room`] and links them with [`Drlg::link_room`]; the
    /// level seed was just re-initialized.
    fn generate(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        level: LevelIdx,
    ) -> Result<(), DrlgError> {
        Ok(())
    }

    /// Type data reset when a level's rooms are freed (`levels.md` §9.4):
    /// maze `0x00673FE0`, preset `0x006674D0`, outdoor `0x006754C0`; also
    /// the preset map (+0x1B0) and build list (+0x1CC).
    fn reset_level(&mut self, drlg: &mut Drlg, level: LevelIdx) {}

    /// Status-3 handler (`rooms.md` §4): add a preset room's units
    /// (`0x00667890`, `drlg/preset.md`). The DRLG sets the flag after.
    fn add_preset_units(&mut self, drlg: &mut Drlg, room: DrlgRoomId) -> Result<(), DrlgError> {
        Ok(())
    }

    /// The preset units of a room (`levels.md` §10.4 reads type-2 units).
    fn preset_units(&self, drlg: &Drlg, room: DrlgRoomId) -> Vec<PresetUnit> {
        Vec::new()
    }

    /// Grid init (`rooms.md` §9.2 step 3c; type 1 `0x0067D2D0`, type 2
    /// `0x006667D0`): the room's packed source grids. Runs right after the
    /// room seed reset; outdoor grids draw from the room seed.
    fn room_grids(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        room: DrlgRoomId,
    ) -> Result<RoomGrids, DrlgError> {
        Ok(RoomGrids::default())
    }

    /// Free type data of a room's tiles (`rooms.md` §9.2 `0x0066F1A0`:
    /// type 1 `0x0067D680`, type 2 `0x00666610`).
    fn free_room_tiles(&mut self, drlg: &mut Drlg, room: DrlgRoomId) {}

    /// A hidden door cell's preset unit (`0x0066D9E0`, `rooms.md` §9.5.1
    /// step 3 and door records). The door tables are not transcribed
    /// (open question 10); the `roll(3)` for objects 91–92 is drawn from
    /// the room seed (`drlg.room_mut(room).seed`) by the provider.
    /// `orientation` is the cell's tile type (`preset.md` §11: 9 = right
    /// door).
    #[allow(clippy::too_many_arguments)]
    fn door_unit(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        room: DrlgRoomId,
        wx: i32,
        wy: i32,
        cell: u32,
        orientation: u32,
    ) {
    }

    /// A hidden exit cell's warp unit (`0x0066E1C0`, `rooms.md` §9.5.1
    /// step 3; wall warp tiles for sub 0 or 4).
    fn warp_unit(&mut self, drlg: &mut Drlg, room: DrlgRoomId, wx: i32, wy: i32, cell: u32) {}
}

/// A [`LevelTypes`] that generates nothing (levels stay empty).
#[derive(Clone, Copy, Debug, Default)]
pub struct NoLevelTypes;

impl LevelTypes for NoLevelTypes {}

/// A preset unit of a room (`levels.md` §10.4): unit type (2 = object),
/// class, position in sub-tiles relative to the room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresetUnit {
    pub unit_type: u32,
    pub class: u32,
    pub x: i32,
    pub y: i32,
}

/// The act room list the active rooms live in (`rooms.md` §5.5, §8.2;
/// `unit-order.md` §4). Provider: `UnitLists` ([`super::active`]).
pub trait ActRooms {
    /// Allocate an active room record and prepend it to `act`'s room
    /// list, setting the act's pending-room flag (`0x00619890`). `flags`
    /// are active-room flags (+0x34: bit 0 populated, bit 2 no update).
    fn create_active_room(&mut self, act: u8, flags: u32) -> RoomId;

    /// Write a room's adjacency array (+0x00 / +0x24), in order.
    fn set_adjacent(&mut self, room: RoomId, adjacent: &[RoomId]);

    /// Unlink from the act list (if still linked) and free the record
    /// (`0x0061A910`, `0x0061A840`). Returns the active-room flags
    /// (+0x34) as they were, for `other flags := flags & 1`.
    fn remove_active_room(&mut self, room: RoomId) -> u32;
}

/// One DT1 tile header as the DRLG reads it (`formats/dt1.md`;
/// `rooms.md` §9.3 accessors).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileInfo {
    /// Orientation (+0x14).
    pub orientation: u32,
    /// Main index (+0x18).
    pub main: u32,
    /// Sub index (+0x1C).
    pub sub: u32,
    /// Rarity / frame number (+0x20).
    pub rarity: u32,
    /// Material flags (+0x06).
    pub material: u16,
    /// Sub-tile flags (+0x28), bottom row first (`rooms.md` §10.4).
    pub subtile_flags: [u8; 25],
}

/// Parsed DT1 files by path (no I/O in the sim). Paths are the
/// `lvltypes` `File` strings verbatim and the three fixed library paths
/// of `rooms.md` §9.3 ([`super::tiles::FIXED_LIBRARY`]); the provider
/// resolves them (case-insensitive, relative to `DATA\GLOBAL\Tiles\`).
pub trait TileSource {
    /// The tiles of a DT1 file in file order, or `None` if unknown.
    fn dt1(&self, path: &[u8]) -> Option<&[TileInfo]>;
}

/// The services a room build needs.
pub struct Services<'a> {
    pub data: &'a DrlgData,
    pub tiles: &'a dyn TileSource,
    pub types: &'a mut dyn LevelTypes,
    pub rooms: &'a mut dyn ActRooms,
}
