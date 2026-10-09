// Spec: specs/drlg/levels.md §3.7, §4.3, §5.2, §9.4; specs/drlg/rooms.md §4, §9.2, §9.5.1; specs/drlg/preset.md §3, §8–§11; specs/drlg/maze.md §1, §4; specs/drlg/outdoor.md §2.3, §3, §12
//! DRLG ↔ level types: [`WorldTypes`] is the act DRLG's
//! [`LevelTypes`], dispatching by leveldefs `DrlgType` (1 maze, 2 preset,
//! 3 outdoor) to [`Maze`], [`Presets`] and [`Outdoor`]. Rooms are told
//! apart by the type data that owns them (outdoor room data or preset
//! room data). [`SharedTypes`] is the handle the act DRLG holds; the
//! population adapter reads the preset rooms through the same handle.
//!
//! Errors: [`LevelTypes`] returns [`DrlgError`]; a level type's own error
//! is kept in [`WorldTypes::errors`] and reported as
//! [`DrlgError::LevelType`] (the level id), a wrapped [`DrlgError`] as
//! itself.

use std::cell::{Ref, RefCell, RefMut};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::drlg::maze::{Maze, MazeError};
use crate::drlg::outdoor::{Outdoor, OutdoorData, OutdoorError, SubFiles, DRLG_OUTDOOR};
use crate::drlg::preset::{Ds1Cache, Ds1Source, PresetCtx, PresetData, PresetError, Presets};
use crate::drlg::{
    ClientPreset, Drlg, DrlgData, DrlgError, DrlgRoomId, LevelIdx, LevelTypes, PresetUnit,
    RoomGrids,
};

use super::maze_presets::MazeToPreset;
use super::outdoor_presets::OutdoorToPreset;
use super::WorldgenError;

/// leveldefs `DrlgType` of maze levels (`drlg/maze.md`).
pub const DRLG_MAZE: u32 = 1;
/// leveldefs `DrlgType` of preset levels (`drlg/preset.md`).
pub const DRLG_PRESET: u32 = 2;

/// The level types of every act DRLG of a game: their tables, their
/// per-act state, the DS1 sources and the process-wide DS1 cache.
pub struct WorldTypes {
    /// The DRLG table view (the preset context of the hooks that are not
    /// given it: status 3, reset).
    pub drlg_data: Arc<DrlgData>,
    /// One maze state for all acts (keyed by act and level slot).
    pub maze: Maze,
    /// Preset state per act DRLG.
    pub presets: BTreeMap<u8, Presets>,
    /// Outdoor state per act DRLG.
    pub outdoor: BTreeMap<u8, Outdoor>,
    pub preset_data: PresetData,
    pub outdoor_data: OutdoorData,
    /// Parsed lvlprest DS1 files (no I/O in the sim). `Send + Sync` so a
    /// client DRLG copy (`client/model.md` §12 r1) can live in the Bevy
    /// app's bridge resource.
    pub ds1: Box<dyn Ds1Source + Send + Sync>,
    pub ds1_cache: Ds1Cache,
    /// Parsed lvlsub DS1 files.
    pub subs: Box<dyn SubFiles + Send + Sync>,
    /// Level-type errors, in order.
    pub errors: Vec<WorldgenError>,
}

/// The parts of [`WorldTypes`] one act's call works on (split borrows).
pub(super) struct Parts<'a> {
    pub drlg_data: &'a DrlgData,
    pub maze: &'a mut Maze,
    pub presets: &'a mut Presets,
    pub outdoor: &'a mut Outdoor,
    pub pd: &'a PresetData,
    pub od: &'a OutdoorData,
    pub src: &'a dyn Ds1Source,
    pub cache: &'a mut Ds1Cache,
    pub subs: &'a dyn SubFiles,
    pub errors: &'a mut Vec<WorldgenError>,
}

impl WorldTypes {
    pub fn new(
        drlg_data: Arc<DrlgData>,
        maze: Maze,
        preset_data: PresetData,
        outdoor_data: OutdoorData,
        ds1: Box<dyn Ds1Source + Send + Sync>,
        subs: Box<dyn SubFiles + Send + Sync>,
    ) -> Self {
        Self {
            drlg_data,
            maze,
            presets: BTreeMap::new(),
            outdoor: BTreeMap::new(),
            preset_data,
            outdoor_data,
            ds1,
            ds1_cache: Ds1Cache::default(),
            subs,
            errors: Vec::new(),
        }
    }

    pub(super) fn parts(&mut self, act: u8) -> Parts<'_> {
        Parts {
            drlg_data: &self.drlg_data,
            maze: &mut self.maze,
            presets: self.presets.entry(act).or_default(),
            outdoor: self.outdoor.entry(act).or_default(),
            pd: &self.preset_data,
            od: &self.outdoor_data,
            src: &*self.ds1,
            cache: &mut self.ds1_cache,
            subs: &*self.subs,
            errors: &mut self.errors,
        }
    }

    /// The preset state of an act, if any.
    pub fn act_presets(&self, act: u8) -> Option<&Presets> {
        self.presets.get(&act)
    }

    /// [`Self::act_presets`], mutable.
    pub fn act_presets_mut(&mut self, act: u8) -> Option<&mut Presets> {
        self.presets.get_mut(&act)
    }

    /// The outdoor state of an act, if any.
    pub fn act_outdoor(&self, act: u8) -> Option<&Outdoor> {
        self.outdoor.get(&act)
    }
}

/// A preset error as the DRLG seam reports it.
pub(super) fn preset_err(errors: &mut Vec<WorldgenError>, id: u32, e: PresetError) -> DrlgError {
    match e {
        PresetError::Drlg(d) => d,
        e => {
            errors.push(WorldgenError::Preset(e));
            DrlgError::LevelType(id)
        }
    }
}

/// A maze error as the DRLG seam reports it.
pub(super) fn maze_err(errors: &mut Vec<WorldgenError>, id: u32, e: MazeError) -> DrlgError {
    match e {
        MazeError::Drlg(d) => d,
        e => {
            errors.push(WorldgenError::Maze(e));
            DrlgError::LevelType(id)
        }
    }
}

/// An outdoor error as the DRLG seam reports it.
pub(super) fn outdoor_err(errors: &mut Vec<WorldgenError>, id: u32, e: OutdoorError) -> DrlgError {
    match e {
        OutdoorError::Drlg(d) => d,
        e => {
            errors.push(WorldgenError::Outdoor(e));
            DrlgError::LevelType(id)
        }
    }
}

/// The level types an allocation (`levels.md` §4.3) reaches while one of
/// the generators is borrowed: the type inits only. Outdoor inits are
/// lazy during the act placer (`outdoor` = `None`, [`Outdoor::info_mut`]);
/// a maze level cannot be initialized while the maze generator runs
/// (`maze` = `None`; no 1.14d path does it: `maze.md` §7 allocates the
/// preset levels 27 and 108).
pub(super) struct AllocView<'a> {
    pub maze: Option<&'a mut Maze>,
    pub presets: &'a mut Presets,
    pub outdoor: Option<&'a mut Outdoor>,
    pub pd: &'a PresetData,
    pub src: &'a dyn Ds1Source,
    pub cache: &'a mut Ds1Cache,
    pub errors: &'a mut Vec<WorldgenError>,
}

impl LevelTypes for AllocView<'_> {
    fn init_level(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        l: LevelIdx,
    ) -> Result<(), DrlgError> {
        let id = drlg.level(l).id;
        match drlg.level(l).drlg_type {
            DRLG_MAZE => match self.maze.as_deref_mut() {
                Some(m) => m
                    .init_level(drlg, l)
                    .map_err(|e| maze_err(self.errors, id, e)),
                None => {
                    self.errors.push(WorldgenError::MazeBusy(id));
                    Err(DrlgError::LevelType(id))
                }
            },
            DRLG_PRESET => {
                let ctx = PresetCtx {
                    drlg: data,
                    data: self.pd,
                    source: self.src,
                    cache: &mut *self.cache,
                };
                self.presets
                    .init_level(drlg, &ctx, l)
                    .map_err(|e| preset_err(self.errors, id, e))
            }
            DRLG_OUTDOOR => {
                if let Some(o) = self.outdoor.as_deref_mut() {
                    o.init_level(l);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl LevelTypes for WorldTypes {
    /// `preset.md` §3.2 step 4, generic branch (the lvlprest `AutoMap` of
    /// the level's preset row; towns 40, 103, 109 take the town-automap
    /// branch, which streams nothing).
    fn automap_streams(&self, drlg: &Drlg, level: LevelIdx) -> bool {
        let lv = drlg.level(level);
        if lv.drlg_type != DRLG_PRESET || matches!(lv.id, 40 | 103 | 109) {
            return false;
        }
        self.preset_data
            .def_for_level(lv.id)
            .and_then(|row| self.preset_data.def(row).ok())
            .is_some_and(|d| d.automap != 0)
    }

    /// The act placer `0x00678AD0` (`outdoor.md` §2), then the preset
    /// directions it decided (`outdoor.md` §2.3 step 4, the overwrite of
    /// `preset.md` §3.1) written into the preset infos. Allocation inits
    /// the levels' types on the way (`levels.md` §4.3).
    fn create_act_levels(&mut self, drlg: &mut Drlg, data: &DrlgData) -> Result<(), DrlgError> {
        let p = self.parts(drlg.act);
        let mut alloc = AllocView {
            maze: Some(p.maze),
            presets: p.presets,
            outdoor: None,
            pd: p.pd,
            src: p.src,
            cache: p.cache,
            errors: p.errors,
        };
        let r = p.outdoor.create_act_levels(drlg, data, p.od, &mut alloc);
        r.map_err(|e| outdoor_err(alloc.errors, 0, e))?;
        for (&id, &dir) in &p.outdoor.preset_direction {
            let Some(l) = drlg.find_level(id) else {
                continue;
            };
            if drlg.level(l).drlg_type != DRLG_PRESET {
                continue;
            }
            alloc
                .presets
                .set_direction(l, dir)
                .map_err(|e| preset_err(alloc.errors, id, e))?;
        }
        Ok(())
    }

    /// Type init at allocation (`levels.md` §4.3): maze `0x00673B10`,
    /// preset `0x00667430`, outdoor `0x00675320`.
    fn init_level(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        l: LevelIdx,
    ) -> Result<(), DrlgError> {
        let p = self.parts(drlg.act);
        AllocView {
            maze: Some(p.maze),
            presets: p.presets,
            outdoor: Some(p.outdoor),
            pd: p.pd,
            src: p.src,
            cache: p.cache,
            errors: p.errors,
        }
        .init_level(drlg, data, l)
    }

    /// Room generation (`levels.md` §5.2): maze `0x00673B30` building its
    /// cells through the preset maps (`maze.md` §9), preset `0x00668100`,
    /// outdoor `0x00675360` building its preset cells through the preset
    /// maps (`outdoor.md` §12.1).
    fn generate(&mut self, drlg: &mut Drlg, data: &DrlgData, l: LevelIdx) -> Result<(), DrlgError> {
        let id = drlg.level(l).id;
        let p = self.parts(drlg.act);
        match drlg.level(l).drlg_type {
            DRLG_MAZE => {
                let mut mp = MazeToPreset {
                    presets: p.presets,
                    outdoor: p.outdoor,
                    pd: p.pd,
                    src: p.src,
                    cache: p.cache,
                    errors: p.errors,
                };
                let r = p.maze.generate(drlg, data, &mut mp, l);
                r.map_err(|e| maze_err(mp.errors, id, e))
            }
            DRLG_PRESET => {
                let mut ctx = PresetCtx {
                    drlg: data,
                    data: p.pd,
                    source: p.src,
                    cache: p.cache,
                };
                let r = p.presets.generate(drlg, &mut ctx, l);
                r.map(|_| ()).map_err(|e| preset_err(p.errors, id, e))
            }
            DRLG_OUTDOOR => {
                let mut op = OutdoorToPreset {
                    presets: p.presets,
                    pd: p.pd,
                    src: p.src,
                    cache: p.cache,
                    errors: p.errors,
                };
                let r = p.outdoor.generate(drlg, data, p.od, p.subs, &mut op, l);
                r.map_err(|e| outdoor_err(op.errors, id, e))
            }
            _ => Ok(()),
        }
    }

    /// Type data reset when the level's rooms are freed and the level is
    /// kept (`levels.md` §9.4): maze `0x00673FE0` / outdoor `0x006754C0`,
    /// then the level's preset maps (+0x1B0) for every type; a preset
    /// level keeps its info (`preset.md` §3.3, keep = 1).
    fn reset_level(&mut self, drlg: &mut Drlg, l: LevelIdx) {
        let p = self.parts(drlg.act);
        match drlg.level(l).drlg_type {
            DRLG_MAZE => p.maze.reset_level(drlg, l),
            DRLG_OUTDOOR => p.outdoor.reset_level(drlg, l),
            _ => {}
        }
        let mut ctx = PresetCtx {
            drlg: p.drlg_data,
            data: p.pd,
            source: p.src,
            cache: p.cache,
        };
        p.presets.reset_level(&mut ctx, l, true);
    }

    /// Status-3 handler `0x00667890` (`preset.md` §8) on a preset room;
    /// other rooms have none.
    fn add_preset_units(&mut self, drlg: &mut Drlg, room: DrlgRoomId) -> Result<(), DrlgError> {
        let id = drlg.level(drlg.room(room).level).id;
        let p = self.parts(drlg.act);
        if p.presets.room(room).is_err() {
            return Ok(());
        }
        let mut ctx = PresetCtx {
            drlg: p.drlg_data,
            data: p.pd,
            source: p.src,
            cache: p.cache,
        };
        p.presets
            .add_preset_units(drlg, &mut ctx, room)
            .map_err(|e| preset_err(p.errors, id, e))
    }

    /// The room's preset units: a preset room's list (`preset.md` §9), or
    /// the units an outdoor room's substitution added
    /// (`outdoor-tilesub.md`, [`crate::drlg::outdoor::OutdoorRoom::units`]).
    fn preset_units(&self, drlg: &Drlg, room: DrlgRoomId) -> Vec<PresetUnit> {
        if let Some(p) = self.presets.get(&drlg.act) {
            if p.room(room).is_ok() {
                return p.preset_units(room);
            }
        }
        self.outdoor
            .get(&drlg.act)
            .and_then(|o| o.room(room))
            .map(|r| r.units.clone())
            .unwrap_or_default()
    }

    /// The client presets (`client/model.md` §5 r6.2): a preset room's
    /// units with flag bit 0. An outdoor room's substitution units carry
    /// no flag word in the model, so none of them is a client preset.
    fn client_presets(&self, drlg: &Drlg, room: DrlgRoomId) -> Vec<ClientPreset> {
        self.presets
            .get(&drlg.act)
            .map(|p| p.client_presets(room))
            .unwrap_or_default()
    }

    /// Grid init (`rooms.md` §9.2 step 3c): outdoor `0x0067D2D0`
    /// (`outdoor.md` §12.2), preset `0x006667D0` (`preset.md` §9–§10).
    fn room_grids(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        room: DrlgRoomId,
    ) -> Result<RoomGrids, DrlgError> {
        let id = drlg.level(drlg.room(room).level).id;
        let p = self.parts(drlg.act);
        if p.outdoor.rooms.contains_key(&room) {
            let r = p.outdoor.room_grids(drlg, p.od, p.subs, room);
            return r.map_err(|e| outdoor_err(p.errors, id, e));
        }
        if p.presets.room(room).is_ok() {
            let mut ctx = PresetCtx {
                drlg: data,
                data: p.pd,
                source: p.src,
                cache: p.cache,
            };
            let r = p.presets.room_grids(drlg, &mut ctx, room);
            return r.map_err(|e| preset_err(p.errors, id, e));
        }
        Ok(RoomGrids::default())
    }

    /// `0x0066F1A0`'s type free: outdoor `0x0067D680`. The preset free
    /// `0x00666610` is not specified.
    // TODO(preset.md §3.3, rooms.md §9.2): type-2 tile free `0x00666610`
    // (open question 10 of the preset handoff).
    fn free_room_tiles(&mut self, drlg: &mut Drlg, room: DrlgRoomId) {
        let p = self.parts(drlg.act);
        if p.outdoor.rooms.contains_key(&room) {
            p.outdoor.free_room_tiles(room);
        }
    }

    /// A hidden exit cell's warp tile unit `0x0066E1C0`
    /// (`sim/path-placement.md` §12.1) in a preset room: a tile preset
    /// unit (type 5, class = the lvlwarp `Id`) prepended to the room's
    /// list; the tile units come from it when the room is populated
    /// (`rooms.md` §6 "Warp tile units across deactivation"). Outdoor
    /// rooms carry no exit cells of their own (their links are walks).
    fn warp_unit(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        room: DrlgRoomId,
        t: u32,
        wx: i32,
        wy: i32,
        cell: u32,
    ) {
        let p = self.parts(drlg.act);
        if p.presets.room(room).is_err() {
            return;
        }
        let mut view = PresetWarps {
            drlg,
            data,
            presets: &mut *p.presets,
        };
        if let Err(e) = crate::path::warp::warp_tile_preset(&mut view, room, t, wx, wy, cell) {
            p.errors.push(WorldgenError::Warp(e));
        }
    }

    /// A door cell's preset unit `0x0066D9E0` (`preset.md` §11) in a
    /// preset room; true when the door record gets flag 0x20
    /// ([`crate::drlg::preset::DoorOutcome::sets_record_flag`], set by the
    /// tile code, `rooms.md` §9.5.1).
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
        let p = self.parts(drlg.act);
        if p.presets.room(room).is_err() {
            return false;
        }
        let ctx = PresetCtx {
            drlg: data,
            data: p.pd,
            source: p.src,
            cache: p.cache,
        };
        match p
            .presets
            .door_unit(drlg, &ctx, room, wx, wy, cell, orientation)
        {
            Ok(outcome) => outcome.sets_record_flag(),
            Err(e) => {
                p.errors.push(WorldgenError::Preset(e));
                false
            }
        }
    }
}

/// [`crate::path::place_seams::WarpTileView`] over a preset room: the room's tile rect
/// and level, the `lvlwarp` row of the exit's slot, and the room's preset
/// unit list.
struct PresetWarps<'a> {
    drlg: &'a Drlg,
    data: &'a DrlgData,
    presets: &'a mut Presets,
}

impl crate::path::place_seams::WarpTileView for PresetWarps<'_> {
    type DrlgRoom = DrlgRoomId;

    fn tile_rect(&self, room: DrlgRoomId) -> crate::drlg::TileRect {
        self.drlg.room(room).rect
    }

    fn lvlwarp(
        &self,
        room: DrlgRoomId,
        slot: u32,
        letter: u8,
    ) -> Option<crate::path::place_seams::LvlWarp> {
        let level_id = self.drlg.level(self.drlg.room(room).level).id;
        let row = self
            .drlg
            .lvlwarp_row(self.data, level_id, slot as usize, letter)
            .ok()?;
        let (offset_x, offset_y) = self.data.warp_offsets.get(row).copied().unwrap_or((0, 0));
        Some(crate::path::place_seams::LvlWarp {
            id: self.data.warps.get(row)?.id as u32,
            offset_x,
            offset_y,
        })
    }

    fn add_preset_unit(&mut self, room: DrlgRoomId, ty: u8, class: u32, mode: u32, x: i32, y: i32) {
        // The room was checked to be a preset room by the caller.
        let _ = self.presets.add_unit_front(
            room,
            crate::drlg::preset::PresetUnit {
                unit_type: u32::from(ty),
                class: class as i32,
                mode,
                x,
                y,
                flags: 0,
                path: None,
            },
        );
    }
}

/// The handle to a game's [`WorldTypes`] the act DRLGs hold
/// ([`crate::wiring::action::DrlgWorld::types`]); population reads the
/// preset rooms through a clone. Calls never nest (a generator reaches
/// other levels through its own allocation view, not the handle).
#[derive(Clone)]
pub struct SharedTypes(pub Rc<RefCell<WorldTypes>>);

impl SharedTypes {
    pub fn new(types: WorldTypes) -> Self {
        Self(Rc::new(RefCell::new(types)))
    }

    pub fn borrow(&self) -> Ref<'_, WorldTypes> {
        self.0.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<'_, WorldTypes> {
        self.0.borrow_mut()
    }
}

impl LevelTypes for SharedTypes {
    fn automap_streams(&self, drlg: &Drlg, level: LevelIdx) -> bool {
        self.0.borrow().automap_streams(drlg, level)
    }

    fn create_act_levels(&mut self, drlg: &mut Drlg, data: &DrlgData) -> Result<(), DrlgError> {
        self.0.borrow_mut().create_act_levels(drlg, data)
    }

    fn init_level(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        l: LevelIdx,
    ) -> Result<(), DrlgError> {
        self.0.borrow_mut().init_level(drlg, data, l)
    }

    fn generate(&mut self, drlg: &mut Drlg, data: &DrlgData, l: LevelIdx) -> Result<(), DrlgError> {
        self.0.borrow_mut().generate(drlg, data, l)
    }

    fn reset_level(&mut self, drlg: &mut Drlg, l: LevelIdx) {
        self.0.borrow_mut().reset_level(drlg, l)
    }

    fn add_preset_units(&mut self, drlg: &mut Drlg, room: DrlgRoomId) -> Result<(), DrlgError> {
        self.0.borrow_mut().add_preset_units(drlg, room)
    }

    fn preset_units(&self, drlg: &Drlg, room: DrlgRoomId) -> Vec<PresetUnit> {
        self.0.borrow().preset_units(drlg, room)
    }

    fn client_presets(&self, drlg: &Drlg, room: DrlgRoomId) -> Vec<ClientPreset> {
        self.0.borrow().client_presets(drlg, room)
    }

    fn room_grids(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        room: DrlgRoomId,
    ) -> Result<RoomGrids, DrlgError> {
        self.0.borrow_mut().room_grids(drlg, data, room)
    }

    fn free_room_tiles(&mut self, drlg: &mut Drlg, room: DrlgRoomId) {
        self.0.borrow_mut().free_room_tiles(drlg, room)
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
        self.0
            .borrow_mut()
            .door_unit(drlg, data, room, wx, wy, cell, orientation)
    }

    fn warp_unit(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        room: DrlgRoomId,
        t: u32,
        wx: i32,
        wy: i32,
        cell: u32,
    ) {
        self.0
            .borrow_mut()
            .warp_unit(drlg, data, room, t, wx, wy, cell)
    }
}
