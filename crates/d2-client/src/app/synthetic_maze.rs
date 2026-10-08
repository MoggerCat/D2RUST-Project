// Spec: specs/drlg/maze.md §4, §9; specs/drlg/preset.md §4, §6, §8, §9; specs/drlg/levels.md §5; specs/drlg/rooms.md §3.3
//! The synthetic game's maze level (task `q-act1-dungeons`,
//! `docs/handoff/q-act1-dungeons.md`): Cave Level 1 behind the Den of
//! Evil is built by the real maze generator and preset rooms
//! ([`WorldTypes`], one 24 × 24 floor DS1 for every cell), the flat
//! levels of the synthetic world by the app's own types. The warp pair
//! Den ↔ Cave Level 1 is d2rs-own, unverified: the maze room that carries
//! the way back is the first room of the level (the original takes its
//! exits from the cells' DS1 warp units).

use std::sync::Arc;

use d2_sim::drlg::maze::{Maze, MazeData, MazeRow, Specials};
use d2_sim::drlg::outdoor::{OutdoorData, SubFileMap};
use d2_sim::drlg::preset::{Ds1Input, Ds1Source, PresetData, PresetDef, PresetTables};
use d2_sim::drlg::tiles::cell;
use d2_sim::drlg::{
    room_flags, Drlg, DrlgData, DrlgError, DrlgRoomId, LevelIdx, LevelTypes, PresetUnit, RoomGrids,
};
use d2_sim::wiring::worldgen::levels::WorldTypes;

/// Cave Level 1 (act 0): a maze level, the Den's second exit.
pub const CAVE_LEVEL_1: u32 = 9;
/// The `lvlwarp` `Id` (and tile class) of the Den's stairs down and of
/// Cave Level 1's way back (synthetic rows 2 and 3).
pub const DEN_TO_CAVE: u32 = 13;
pub const CAVE_TO_DEN: u32 = 14;
/// Sub-tile of the Den's stairs tile (the way back is at
/// [`super::single_player::WARP_TILE_XY`]).
pub const DEN_STAIRS_XY: i32 = 10;

/// One 24 × 24 floor DS1 for every path.
pub struct FloorDs1(Ds1Input);

impl FloorDs1 {
    pub fn new() -> Self {
        let cells = 25 * 25;
        FloorDs1(Ds1Input {
            version: 18,
            width: 24,
            height: 24,
            floors: vec![vec![cell::FLOOR; cells]],
            shadow: vec![0; cells],
            ..Ds1Input::default()
        })
    }
}

impl Default for FloorDs1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Ds1Source for FloorDs1 {
    fn ds1(&self, _: &[u8]) -> Option<&Ds1Input> {
        Some(&self.0)
    }
}

/// The maze row of Cave Level 1: one room per cell kind, 24-tile cells.
pub fn maze_data() -> MazeData {
    MazeData {
        rows: super::synthetic_tower::maze_levels()
            .map(|level| MazeRow {
                level,
                rooms: [1; 3],
                size_x: 24,
                size_y: 24,
                merge: 0,
            })
            .collect(),
        prest_files: (0..1200).map(|d| (d, 1)).collect(),
        specials: Specials::shipped(),
    }
}

/// lvlprest: every row one DS1 file with DT1 mask 1.
pub fn preset_data() -> PresetData {
    let defs = (0..1200)
        .map(|i| {
            let mut d = PresetDef {
                def: i,
                files: 1,
                dt1_mask: 1,
                populate: 1,
                ..PresetDef::default()
            };
            d.file[0] = format!("def{i}.ds1").into_bytes();
            d
        })
        .collect();
    PresetData {
        defs,
        monpreset_acts: Default::default(),
        monpreset: Vec::new(),
        monstats_count: 0,
        superuniques_count: 0,
        hdm_item: -1,
        tables: PresetTables::spec().expect("preset-tables.tsv"),
    }
}

/// The maze level types over the synthetic DRLG view.
pub fn maze_types(data: Arc<DrlgData>) -> WorldTypes {
    WorldTypes::new(
        data,
        Maze::new(maze_data()),
        preset_data(),
        OutdoorData::default(),
        Box::new(FloorDs1::new()),
        Box::new(SubFileMap::default()),
    )
}

/// The flat levels by `flat`, [`CAVE_LEVEL_1`] by the maze generator.
pub struct SyntheticTypes<F> {
    flat: F,
    maze: WorldTypes,
}

impl<F> SyntheticTypes<F> {
    pub fn new(flat: F, data: Arc<DrlgData>) -> Self {
        Self {
            flat,
            maze: maze_types(data),
        }
    }
}

fn is_maze(id: u32) -> bool {
    super::synthetic_tower::maze_links(id).is_some()
}

fn room_is_maze(drlg: &Drlg, room: DrlgRoomId) -> bool {
    is_maze(drlg.level(drlg.room(room).level).id)
}

impl<F: LevelTypes> LevelTypes for SyntheticTypes<F> {
    fn create_act_levels(&mut self, drlg: &mut Drlg, data: &DrlgData) -> Result<(), DrlgError> {
        self.flat.create_act_levels(drlg, data)
    }

    fn init_level(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        l: LevelIdx,
    ) -> Result<(), DrlgError> {
        if is_maze(drlg.level(l).id) {
            self.maze.init_level(drlg, data, l)
        } else {
            self.flat.init_level(drlg, data, l)
        }
    }

    fn generate(&mut self, drlg: &mut Drlg, data: &DrlgData, l: LevelIdx) -> Result<(), DrlgError> {
        let id = drlg.level(l).id;
        let Some((_, on)) = super::synthetic_tower::maze_links(id) else {
            return self.flat.generate(drlg, data, l);
        };
        self.maze.generate(drlg, data, l)?;
        // The way back (warp slot 0, `rooms.md` §3.3) and, in the Tower
        // line, the way on (slot 1): the first room.
        if let Some(r) = drlg.level_rooms(l).first().copied() {
            drlg.room_mut(r).flags |= room_flags::WARP_0;
            if on.is_some() {
                drlg.room_mut(r).flags |= room_flags::WARP_0 << 1;
            }
        }
        Ok(())
    }

    fn reset_level(&mut self, drlg: &mut Drlg, l: LevelIdx) {
        if is_maze(drlg.level(l).id) {
            self.maze.reset_level(drlg, l)
        } else {
            self.flat.reset_level(drlg, l)
        }
    }

    fn add_preset_units(&mut self, drlg: &mut Drlg, room: DrlgRoomId) -> Result<(), DrlgError> {
        if room_is_maze(drlg, room) {
            self.maze.add_preset_units(drlg, room)
        } else {
            self.flat.add_preset_units(drlg, room)
        }
    }

    fn preset_units(&self, drlg: &Drlg, room: DrlgRoomId) -> Vec<PresetUnit> {
        if !room_is_maze(drlg, room) {
            return self.flat.preset_units(drlg, room);
        }
        let mut units = self.maze.preset_units(drlg, room);
        let level = drlg.room(room).level;
        let links = super::synthetic_tower::maze_links(drlg.level(level).id);
        if let (Some((back, on)), true) = (links, drlg.level_rooms(level).first() == Some(&room)) {
            units.push(PresetUnit {
                unit_type: 5,
                class: back,
                x: super::single_player::WARP_TILE_XY,
                y: super::single_player::WARP_TILE_XY,
            });
            if let Some(class) = on {
                units.push(PresetUnit {
                    unit_type: 5,
                    class,
                    x: DEN_STAIRS_XY,
                    y: DEN_STAIRS_XY,
                });
            }
        }
        units
    }

    fn room_grids(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        room: DrlgRoomId,
    ) -> Result<RoomGrids, DrlgError> {
        if room_is_maze(drlg, room) {
            self.maze.room_grids(drlg, data, room)
        } else {
            self.flat.room_grids(drlg, data, room)
        }
    }

    fn free_room_tiles(&mut self, drlg: &mut Drlg, room: DrlgRoomId) {
        if room_is_maze(drlg, room) {
            self.maze.free_room_tiles(drlg, room)
        } else {
            self.flat.free_room_tiles(drlg, room)
        }
    }

    fn door_unit(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        room: DrlgRoomId,
        wx: i32,
        wy: i32,
        cell: u32,
        orientation: u32,
    ) -> bool {
        if room_is_maze(drlg, room) {
            self.maze
                .door_unit(drlg, data, room, wx, wy, cell, orientation)
        } else {
            self.flat
                .door_unit(drlg, data, room, wx, wy, cell, orientation)
        }
    }

    fn warp_unit(&mut self, drlg: &mut Drlg, room: DrlgRoomId, wx: i32, wy: i32, cell: u32) {
        if room_is_maze(drlg, room) {
            self.maze.warp_unit(drlg, room, wx, wy, cell)
        } else {
            self.flat.warp_unit(drlg, room, wx, wy, cell)
        }
    }
}
