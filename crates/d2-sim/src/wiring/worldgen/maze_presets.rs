// Spec: specs/drlg/maze.md §7, §9; specs/drlg/preset.md §3.1, §4, §6; specs/drlg/levels.md §4.2
//! Maze → preset: [`MazeToPreset`] is the [`MazePresets`] of the maze
//! generator, on the act's real [`Presets`]. A cell's DS1 map is
//! allocated on the level seed (`preset.md` §4, the default
//! `roll(Files)`), its file overwritten when the maze chose one
//! (`maze.md` §9 step 2), then built into preset rooms (`preset.md` §6;
//! single-room mode for cells of at most 12 × 12).

use crate::drlg::maze::{MapId, MazeLink, MazePresets};
use crate::drlg::outdoor::Outdoor;
use crate::drlg::preset::{self, Ds1Cache, Ds1Source, PresetCtx, PresetData, Presets};
use crate::drlg::{Drlg, DrlgData, DrlgError, LevelIdx, TileRect};

use super::levels::{preset_err, AllocView};
use super::WorldgenError;

/// The preset side of one maze generation (see the module doc).
pub struct MazeToPreset<'a> {
    pub presets: &'a mut Presets,
    /// Outdoor inits of levels the maze allocates (`maze.md` §7).
    pub outdoor: &'a mut Outdoor,
    pub pd: &'a PresetData,
    pub src: &'a dyn Ds1Source,
    pub cache: &'a mut Ds1Cache,
    pub errors: &'a mut Vec<WorldgenError>,
}

impl MazePresets for MazeToPreset<'_> {
    /// Get-or-allocate `0x00642BB0` (`levels.md` §4.2) with the type inits
    /// of the other generators (the maze one is busy).
    fn level(&mut self, drlg: &mut Drlg, data: &DrlgData, id: u32) -> Result<LevelIdx, DrlgError> {
        let mut alloc = AllocView {
            maze: None,
            presets: &mut *self.presets,
            outdoor: Some(&mut *self.outdoor),
            pd: self.pd,
            src: self.src,
            cache: &mut *self.cache,
            errors: &mut *self.errors,
        };
        drlg.get_or_alloc_level(data, &mut alloc, id)
    }

    /// The preset info's direction (+0x04, `preset.md` §3.1; −1 reads as
    /// `u32::MAX`, which the maze's finder table rejects).
    fn preset_direction(&self, drlg: &Drlg, level: LevelIdx) -> Result<u32, DrlgError> {
        self.presets
            .info(level)
            .map(|i| i.direction as u32)
            .ok_or(DrlgError::LevelType(drlg.level(level).id))
    }

    /// `0x00666ED0` (`preset.md` §4): draws `roll(Files)` on the level
    /// seed.
    fn alloc_map(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        level: LevelIdx,
        def: u32,
        rect: TileRect,
    ) -> Result<MapId, DrlgError> {
        let id = drlg.level(level).id;
        let ctx = PresetCtx {
            drlg: data,
            data: self.pd,
            source: self.src,
            cache: &mut *self.cache,
        };
        self.presets
            .alloc_map(drlg, &ctx, level, def, rect)
            .map(|m| MapId(m.0))
            .map_err(|e| preset_err(self.errors, id, e))
    }

    /// `0x00666EC0`: the map's picked file.
    fn set_map_file(&mut self, map: MapId, file: i32) {
        match self.presets.map_mut(preset::MapId(map.0)) {
            Ok(m) => m.picked_file = file,
            Err(e) => self.errors.push(WorldgenError::Preset(e)),
        }
    }

    /// `0x00667ED0` (`preset.md` §6) with no extra room flags, single-room
    /// mode when `small`.
    // TODO(maze.md §9 step 3, preset.md §6): the room flags F the maze
    // passes are not stated; 0 (no flags) is used.
    // TODO(maze.md §9 step 4, maze handoff open question 4): DRLG rooms
    // have no orth links (`drlg::room`); the cell's init-flag links are
    // not carried to the built rooms. No draw depends on them.
    fn build_map(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        level: LevelIdx,
        map: MapId,
        small: bool,
        _links: &[MazeLink],
    ) -> Result<(), DrlgError> {
        let id = drlg.level(level).id;
        let mut ctx = PresetCtx {
            drlg: data,
            data: self.pd,
            source: self.src,
            cache: &mut *self.cache,
        };
        self.presets
            .build_area(drlg, &mut ctx, level, preset::MapId(map.0), 0, small)
            .map(|_| ())
            .map_err(|e| preset_err(self.errors, id, e))
    }
}
