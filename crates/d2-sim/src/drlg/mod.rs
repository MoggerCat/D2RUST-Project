// Spec: specs/drlg/levels.md, specs/drlg/rooms.md
//! The dungeon random level generator (DRLG) of one act: levels, level
//! seeds, vis/warp records, DRLG rooms, rooms-near order, statuses and
//! activation, tile grids and collision.
//!
//! What a level *contains* (which rooms, their rects, flags and source
//! grids) is decided by the level type specs (`drlg/preset.md`,
//! `drlg/maze.md`, `drlg/outdoor.md`, not written yet). They plug in
//! through [`LevelTypes`]. Active rooms reach the unit lists of
//! [`crate::units`] through [`ActRooms`]. DT1 tiles arrive parsed through
//! [`TileSource`]. Table data comes from `d2_data::tables` records via
//! [`DrlgData`].
//!
//! Module map:
//! - [`data`]: the table view ([`DrlgData`]) built from typed records;
//! - [`seams`]: the traits other systems provide;
//! - [`level`]: act DRLG creation, levels, seeds, warps, coordinates,
//!   level lifecycle, spawn room (`levels.md`);
//! - [`room`]: DRLG rooms, rooms-near arrays, statuses, propagation,
//!   build (`rooms.md` §2–§4);
//! - [`active`]: active rooms, adjacency arrays, clients, deactivation
//!   (`rooms.md` §5–§8);
//! - [`tiles`]: tile library, tile choice, grid fill, linking, animation
//!   (`rooms.md` §9);
//! - [`collision`]: collision grids (`rooms.md` §10);
//! - [`logic`]: logical rooms (coordinate lists) and their lookups
//!   (`levels.md` §11.1–§11.4).

pub mod active;
pub mod collision;
pub mod data;
pub mod level;
pub mod logic;
pub mod maze;
pub mod outdoor;
pub mod preset;
pub mod room;
pub mod seams;
pub mod tiles;

#[cfg(test)]
mod tests;

use thiserror::Error;

pub use active::ActiveRoom;
pub use collision::CollisionGrid;
pub use data::{DoorTables, DrlgData, LevelDef, WallClass, WallRemap, WarpDef};
pub use level::{BuildCursor, Drlg, Dungeon, Level, SpawnTile, WarpRecord};
pub use logic::{CoordRec, LogicGrids, LogicInfo};
pub use room::{DrlgRoom, RoomKind, WarpLink};
pub use seams::{ActRooms, LevelTypes, NoLevelTypes, PresetUnit, Services, TileInfo, TileSource};
pub use tiles::{CellGrid, GridPass, RoomGrids, RoomTiles, TileRecord, TileRef};

/// A DRLG room (RoomEx) of one [`Drlg`], by slot. Slots are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DrlgRoomId(pub u32);

/// A level of one [`Drlg`], by slot (allocation order). Not a level id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LevelIdx(pub u32);

/// A rectangle in tile coordinates (signed, as the original's ints).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl TileRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    /// Half-open containment `0x0066B980` (`levels.md` §8.2).
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.x <= x && x < self.x + self.w && self.y <= y && y < self.y + self.h
    }

    /// Closed containment `0x0066B9D0` (border included).
    pub fn contains_closed(&self, x: i32, y: i32) -> bool {
        self.x <= x && x <= self.x + self.w && self.y <= y && y <= self.y + self.h
    }
}

/// Sub-tiles per tile (`0x00643560`).
pub const SUBTILES: i32 = 5;

/// Town level ids (`0x006E7D1C`, `0x006426A0`), act 1..5.
pub const TOWN_LEVELS: [u32; 5] = [1, 40, 75, 103, 109];

/// Act boundaries `0x006EB2F0` (`levels.md` §6.3).
pub const ACT_BOUNDARIES: [u32; 6] = [1, 40, 75, 103, 109, 1024];

/// Room flags (+0x28, `rooms.md` §1; D2MOO names).
pub mod room_flags {
    /// Warp toward vis slot `i`: `WARP_0 << i`.
    pub const WARP_0: u32 = 0x10;
    /// All eight warp bits.
    pub const WARP_MASK: u32 = 0xFF0;
    pub const WAYPOINT: u32 = 0x1_0000;
    pub const WAYPOINT_SMALL: u32 = 0x2_0000;
    /// Either waypoint flag (`levels.md` §5.4).
    pub const ANY_WAYPOINT: u32 = 0x3_0000;
    pub const AUTOMAP_REVEAL: u32 = 0x4_0000;
    pub const NO_LOS_DRAW: u32 = 0x8_0000;
    /// Tiles / active room built (`HAS_ROOM`).
    pub const HAS_ROOM: u32 = 0x10_0000;
    /// Set by `0x0066F1A0` with keep = 1 (`rooms.md` §9.2).
    pub const TILES_KEPT: u32 = 0x20_0000;
    /// A portal: blocks removal.
    pub const PORTAL: u32 = 0x40_0000;
    /// No population (next to a town).
    pub const NO_POPULATION: u32 = 0x80_0000;
    pub const TILE_LIB_LOADED: u32 = 0x100_0000;
    pub const PRESET_UNITS_ADDED: u32 = 0x200_0000;
    /// Room has animated tiles (`rooms.md` §9.7).
    pub const ANIMATED: u32 = 0x800_0000;
}

/// Act number (0..4) of a level id (`0x006427F0`, `levels.md` §6.3):
/// the first `a` with `id < T[a+1]`; ids ≥ 1024 give 0.
pub fn act_of_level(id: u32) -> u8 {
    for a in 0..5u8 {
        if id < ACT_BOUNDARIES[a as usize + 1] {
            return a;
        }
    }
    0
}

/// Town test `0x006426A0`.
pub fn is_town(id: u32) -> bool {
    TOWN_LEVELS.contains(&id)
}

/// The original's fatal errors and d2rs API misuse.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DrlgError {
    #[error("level id {0} has no leveldefs row")]
    UnknownLevel(u32),
    #[error("unknown DRLG room {0:?}")]
    UnknownRoom(DrlgRoomId),
    #[error("unknown level slot {0:?}")]
    UnknownLevelIdx(LevelIdx),
    #[error("vis/warp record with level id 0 (fatal in the original)")]
    WarpRecordLevelZero,
    #[error("no lvlwarp row with Id {0} (fatal in the original)")]
    NoLvlWarp(i32),
    #[error("room tile library is full (fatal 0x2A)")]
    LibraryFull,
    #[error("DT1 file not supplied: {0:?}")]
    MissingDt1(String),
    #[error("no tile for a key and no (10, 0, 0) fallback (fatal 0x73)")]
    NoTile,
    #[error("missing animation frame {0} (fatal 0xB6 / 0xCB)")]
    MissingFrame(u32),
    #[error("linked cell type {0} beyond the wall-remap index table (wall-remap.md §2)")]
    WallRemapType(u32),
    #[error("level has no waypoint room for spawn-tile index 13")]
    NoWaypointRoom,
    #[error("level has no rooms to spawn in")]
    NoSpawnRoom,
    #[error("removal test on a client-copy DRLG (fatal in the original)")]
    ClientCopyRemoval,
    #[error("not an active room of this DRLG")]
    NotActive,
    /// A level-type generator (`drlg/preset.md`, `maze.md`, `outdoor.md`)
    /// failed on this level id; the provider keeps its own error.
    #[error("level type generator failed on level {0} (error kept by the provider)")]
    LevelType(u32),
    /// `rooms.md` §9.6 C4: a corner record R first in its link chain
    /// (R +0x20 null) met with m ≠ 3; 1.14d writes through a null
    /// pointer at `0x0066E8A7`.
    #[error("linked corner record has no record after it in its chain (fault at 0x0066E8A7)")]
    LinkedCornerNoNext,
}
