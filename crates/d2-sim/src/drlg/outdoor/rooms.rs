// Spec: specs/drlg/outdoor.md
//! Level generation `0x00675360` (§3), cells to rooms `0x006750F0`
//! (§12.1) and the outdoor room with its grids `0x0067D540` /
//! `0x0067D2D0` (§12.2).

use super::super::data::DrlgData;
use super::super::level::Drlg;
use super::super::room::{LinkAt, RoomKind};
use super::super::tiles::{CellGrid, GridPass, RoomGrids};
use super::super::{room_flags, DrlgRoomId, LevelIdx, TileRect};
use super::grid::{cell, Gen, Grid};
use super::tilesub::{pick_sub_themes, room_substitution, RoomSub};
use super::{
    dispatch_act, Outdoor, OutdoorData, OutdoorError, OutdoorLevel, OutdoorPresets, OutdoorRoom,
    SubFiles,
};

/// Room tile size of an outdoor room.
pub const ROOM_TILES: i32 = 8;

/// DT1 mask by level type (§12.1).
pub fn dt1_mask(level_type: u32) -> u32 {
    match level_type {
        2 => 0x44103,
        16 | 22 | 27 | 28 => 0x1,
        21 => 0x4,
        30 | 31 => 0x11,
        _ => 0,
    }
}

/// Floor flags by level type (§12.2), OR'd into floor cells without
/// bits 0x3F0FF80.
pub fn floor_flags(level_type: u32, id: u32) -> u32 {
    match level_type {
        16 => 0x100,
        21 => 0x12_0000,
        22 => 0x10_0000,
        27 => 0xA0_0000,
        28 => 0x160_0000,
        31 if id == 117 => 0x60_0000,
        _ => 0,
    }
}

impl Outdoor {
    /// Level generation `0x00675360` (§3). The level seed was just
    /// re-set (`levels.md` §5).
    pub fn generate(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        od: &OutdoorData,
        subs: &dyn SubFiles,
        presets: &mut dyn OutdoorPresets,
        level: LevelIdx,
    ) -> Result<(), OutdoorError> {
        let mut info = self.levels.remove(&level).unwrap_or_default();
        let r = self.generate_with(drlg, data, od, subs, presets, level, &mut info);
        self.levels.insert(level, info);
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_with(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        od: &OutdoorData,
        subs: &dyn SubFiles,
        presets: &mut dyn OutdoorPresets,
        level: LevelIdx,
        info: &mut OutdoorLevel,
    ) -> Result<(), OutdoorError> {
        let rect = drlg.level(level).rect;
        let id = drlg.level(level).id;
        // Step 1.
        let (gw, gh) = (rect.w / 8, rect.h / 8);
        info.grids = [
            Grid::new(gw, gh),
            Grid::new(gw, gh),
            Grid::new(gw, gh),
            Grid::new(gw, gh),
        ];
        info.vertices.clear();
        info.path_ends.clear();
        info.paths.clear();
        info.build_list.clear();
        let mut g = Gen {
            drlg,
            data,
            od,
            subs,
            level,
            id,
            rect,
            info,
        };
        // Step 2.
        g.polygon()?;
        // Step 3.
        match dispatch_act(id) {
            0 => g.act1()?,
            1 => g.act2()?,
            2 => g.act3()?,
            3 => g.act4()?,
            _ => g.act5()?,
        }
        // Step 4.
        self.cells_to_rooms(&mut g, presets)
    }

    /// Cells to rooms `0x006750F0` (§12.1).
    pub fn cells_to_rooms(
        &mut self,
        g: &mut Gen<'_>,
        presets: &mut dyn OutdoorPresets,
    ) -> Result<(), OutdoorError> {
        let lt = g.drlg.level(g.level).level_type;
        let mask = dt1_mask(lt);
        let sd = g.od.sub_defs(g.id);
        for y in 0..g.gh() {
            for x in 0..g.gw() {
                let (ox, oy) = (g.rect.x + 8 * x, g.rect.y + 8 * y);
                let f1 = g.g(1, x, y);
                let f2 = g.g(2, x, y);
                if f2 & cell::PRESET != 0 {
                    let p = g.g(0, x, y);
                    if p != 0 {
                        presets.build_preset_cell(
                            g.drlg,
                            g.data,
                            g.level,
                            p,
                            ox,
                            oy,
                            (f2 >> 16) & 0xF,
                            f1,
                        )?;
                    }
                } else if f2 & cell::BLANK == 0 {
                    // Outdoor room `0x0067D540` (§12.2).
                    let id = g.drlg.alloc_room(
                        g.level,
                        RoomKind::Outdoor,
                        TileRect::new(ox, oy, ROOM_TILES, ROOM_TILES),
                    );
                    // TODO(outdoor.md §12.2): "add to the level" does not
                    // name head or tail; head (newest first) used.
                    g.drlg.link_room(id, LinkAt::Head);
                    let room = g.drlg.room_mut(id);
                    room.flags |= f1 | room_flags::NO_LOS_DRAW;
                    room.dt1_mask = mask;
                    let (picked, dt1) =
                        pick_sub_themes(g.od, &mut room.seed, sd.sub_type, sd.sub_theme)?;
                    room.dt1_mask |= dt1;
                    self.rooms.insert(
                        id,
                        OutdoorRoom {
                            flags: f2,
                            flags_ex: g.g(3, x, y),
                            sub_type: sd.sub_type,
                            sub_theme: sd.sub_theme,
                            picked,
                            ..OutdoorRoom::default()
                        },
                    );
                }
            }
        }
        Ok(())
    }

    /// Outdoor room grids `0x0067D2D0` (§12.2), called right after the
    /// room seed reset (`rooms.md` §9.2 step 3c).
    pub fn room_grids(
        &mut self,
        drlg: &mut Drlg,
        od: &OutdoorData,
        subs: &dyn SubFiles,
        room: DrlgRoomId,
    ) -> Result<RoomGrids, OutdoorError> {
        let r = drlg.room(room);
        let (rect, flags, level) = (r.rect, r.flags, r.level);
        let (id, lt) = (drlg.level(level).id, drlg.level(level).level_type);
        let sd = od.sub_defs(id);
        let n = (ROOM_TILES + 1) as usize;
        let data = self.rooms.entry(room).or_default();
        data.tile_type = CellGrid::new(n, n);
        data.wall = CellGrid::new(n, n);
        data.floor = CellGrid::new(n, n);
        data.roof_count = 0;
        data.shadows.clear();
        data.units.clear();
        for y in 0..ROOM_TILES as usize {
            for x in 0..ROOM_TILES as usize {
                data.floor.set(x, y, 0x40002);
            }
        }
        // TODO(outdoor.md §7.5.3, OQ 6): the Act I path floor needs the
        // 256-byte style table `0x006F2860` and its neighbour-bit order,
        // not transcribed; path floors are not drawn (no draws involved).
        let (sub_type, sub_theme, picked) = (data.sub_type, data.sub_theme, data.picked);
        let seed = &mut drlg.room_mut(room).seed;
        let mut rs = RoomSub {
            w: ROOM_TILES,
            h: ROOM_TILES,
            tile_x: rect.x,
            tile_y: rect.y,
            room: data,
        };
        let wp = (flags >> 16) & 3;
        if wp != 0 {
            room_substitution(od, subs, seed, &mut rs, sd.sub_waypoint, 0, wp)?;
        }
        let sh = (flags >> 12) & 0xF;
        if sh != 0 {
            room_substitution(od, subs, seed, &mut rs, sd.sub_shrine, 0, sh)?;
        }
        room_substitution(od, subs, seed, &mut rs, sub_type, sub_theme, picked)?;
        let ff = floor_flags(lt, id);
        for c in &mut data.floor.cells {
            if *c & 0x3F0_FF80 == 0 {
                *c |= ff;
            }
        }
        // TODO(outdoor.md §12.2): "grid edges" read as all four border
        // rows/columns of the (w+1)×(h+1) grids.
        for g in [&mut data.wall, &mut data.floor] {
            for y in 0..n {
                for x in 0..n {
                    if x == 0 || y == 0 || x == n - 1 || y == n - 1 {
                        let v = g.get(x as i32, y as i32);
                        g.set(x, y, v | 0x4);
                    }
                }
            }
        }
        // TODO(outdoor-tilesub.md §4.4): shadow tiles and roof growth are
        // kept in `OutdoorRoom`; the outdoor tile fill (`rooms.md` §9.5)
        // has no shadow pass to take them.
        Ok(RoomGrids {
            passes: vec![
                GridPass {
                    cells: data.wall.clone(),
                    orientation: Some(data.tile_type.clone()),
                    fill_blanks: false,
                },
                GridPass {
                    cells: data.floor.clone(),
                    orientation: None,
                    fill_blanks: false,
                },
            ],
            ..RoomGrids::default()
        })
    }
}
