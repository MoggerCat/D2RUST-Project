// Spec: specs/drlg/outdoor.md §12.1; specs/drlg/preset.md §4, §6
//! Outdoor → preset: [`OutdoorToPreset`] is the [`OutdoorPresets`] of the
//! outdoor generator, on the act's real [`Presets`]. A preset cell
//! (`outdoor.md` §12.1) allocates a DS1 map at the cell's tile origin
//! with rect size 0 (the lvlprest size applies, `preset.md` §4) on the
//! level seed, overwrites its file, and builds it with the cell's room
//! flags. The act placer's preset directions reach the preset infos in
//! [`super::levels::WorldTypes`]'s `create_act_levels`.

use crate::drlg::outdoor::OutdoorPresets;
use crate::drlg::preset::{Ds1Cache, Ds1Source, PresetCtx, PresetData, PresetError, Presets};
use crate::drlg::{Drlg, DrlgData, DrlgError, LevelIdx, TileRect};

use super::levels::preset_err;
use super::WorldgenError;

/// The preset side of one outdoor generation (see the module doc).
pub struct OutdoorToPreset<'a> {
    pub presets: &'a mut Presets,
    pub pd: &'a PresetData,
    pub src: &'a dyn Ds1Source,
    pub cache: &'a mut Ds1Cache,
    pub errors: &'a mut Vec<WorldgenError>,
}

impl OutdoorToPreset<'_> {
    #[allow(clippy::too_many_arguments)]
    fn build(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        level: LevelIdx,
        def: u32,
        x: i32,
        y: i32,
        file: u32,
        room_flags: u32,
    ) -> Result<(), PresetError> {
        let mut ctx = PresetCtx {
            drlg: data,
            data: self.pd,
            source: self.src,
            cache: &mut *self.cache,
        };
        // `0x00666ED0`: the default file draw on the level seed (site
        // `0x00666F33`), overridden right after.
        let map = self
            .presets
            .alloc_map(drlg, &ctx, level, def, TileRect::new(x, y, 0, 0))?;
        // `0x00666EC0`.
        self.presets.map_mut(map)?.picked_file = file as i32;
        // `0x00667ED0`.
        // TODO(outdoor.md §12.1, preset.md §6): the single-room argument
        // of the outdoor call is not stated; multi-room is used.
        self.presets
            .build_area(drlg, &mut ctx, level, map, room_flags, false)?;
        Ok(())
    }
}

impl OutdoorPresets for OutdoorToPreset<'_> {
    fn build_preset_cell(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        level: LevelIdx,
        def: u32,
        x: i32,
        y: i32,
        file: u32,
        room_flags: u32,
    ) -> Result<(), DrlgError> {
        let id = drlg.level(level).id;
        self.build(drlg, data, level, def, x, y, file, room_flags)
            .map_err(|e| preset_err(self.errors, id, e))
    }
}
