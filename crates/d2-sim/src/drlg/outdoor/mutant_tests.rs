// Spec: specs/drlg/outdoor.md, specs/drlg/outdoor-tilesub.md (mutation-testing tests)
//! Tests written to kill mutants `cargo mutants` left alive in
//! `drlg::outdoor`. Each asserts what the specs say; the survivors no
//! test can observe are listed in `docs/handoff/mutants-drlg.md`.

use d2_data::tables::{Leveldefs, Lvlprest, Lvlsub, Record};

use super::tests::{act1_data, data, od, Presets, Rec};
use super::*;
use crate::drlg::room::LinkAt;
use crate::drlg::tiles::RoomGrids;
use crate::drlg::{DrlgError, NoLevelTypes, PresetUnit, RoomKind, TileRect};

fn u32s(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// `outdoor.md` Constants (leveldefs `SubType`, `SubTheme`,
/// `SubWaypoint`, `SubShrine`; lvlprest `SizeX/Y`, `Files`) and
/// `outdoor-tilesub.md` §1 (every lvlsub column, file order).
#[test]
fn table_view_from_records() {
    let mut l = vec![0u8; Leveldefs::SIZE];
    u32s(&mut l, 56, 6);
    u32s(&mut l, 60, u32::MAX);
    u32s(&mut l, 64, 2);
    u32s(&mut l, 68, 3);
    let mut p = vec![0u8; Lvlprest::SIZE];
    u32s(&mut p, 40, 16);
    u32s(&mut p, 44, 24);
    u32s(&mut p, 64, 5);
    let mut s = vec![0u8; Lvlsub::SIZE];
    u32s(&mut s, 0, 4);
    s[4..9].copy_from_slice(b"x.ds1");
    u32s(&mut s, 64, 1);
    u32s(&mut s, 68, 2);
    u32s(&mut s, 72, 0x30);
    u32s(&mut s, 76, 7);
    for k in 0..5u32 {
        u32s(&mut s, 284 + 4 * k as usize, 10 + k);
        u32s(&mut s, 304 + 4 * k as usize, 20 + k);
        u32s(&mut s, 324 + 4 * k as usize, 30 + k);
    }
    let d = OutdoorData::from_tables(
        &[Leveldefs::decode(&l)],
        &[Lvlprest::decode(&p)],
        &[Lvlsub::decode(&s)],
    );
    assert_eq!(
        d.sub_defs(0),
        SubDefs {
            sub_type: 6,
            sub_theme: -1,
            sub_waypoint: 2,
            sub_shrine: 3,
        }
    );
    assert_eq!(
        d.preset(0),
        Ok(&PresetDef {
            size_x: 16,
            size_y: 24,
            files: 5,
        })
    );
    assert_eq!(
        d.subs,
        [tilesub::SubRow {
            type_: 4,
            file: b"x.ds1".to_vec(),
            check_all: 1,
            bord_type: 2,
            dt1_mask: 0x30,
            grid_size: 7,
            prob: [10, 11, 12, 13, 14],
            trials: [20, 21, 22, 23, 24],
            max: [30, 31, 32, 33, 34],
        }]
    );
}

/// `outdoor-tilesub.md` §1 r3: wall grid k ≥ 1 reads ORed with k << 18
/// (layer 0 as stored); tile-type grid k is the layer's own grid; a
/// missing layer reads 0.
#[test]
fn sub_file_layer_reads() {
    let grid = |v: u32| {
        let mut g = crate::drlg::tiles::CellGrid::new(2, 2);
        g.set(1, 1, v);
        g
    };
    let f = SubFile {
        walls: vec![grid(5), grid(5), grid(5), grid(5)],
        tile_types: vec![grid(9), grid(11)],
        ..SubFile::default()
    };
    assert_eq!(f.wall_at(0, 1, 1), 5);
    assert_eq!(f.wall_at(1, 1, 1), 5 | 1 << 18);
    assert_eq!(f.wall_at(3, 1, 1), 5 | 3 << 18);
    assert_eq!(f.wall_at(3, 0, 0), 3 << 18);
    assert_eq!(f.wall_at(4, 1, 1), 0);
    assert_eq!(f.tile_type_at(0, 1, 1), 9);
    assert_eq!(f.tile_type_at(1, 1, 1), 11);
    assert_eq!(f.tile_type_at(2, 1, 1), 0);
}

// ---- the LevelTypes adapter (§3 dispatch) -----------------------------------

/// Records every hook the adapter forwards to the other level types.
#[derive(Default)]
struct Hooks {
    inits: Vec<u32>,
    resets: Vec<u32>,
    added: Vec<DrlgRoomId>,
    grids: Vec<DrlgRoomId>,
    freed: Vec<DrlgRoomId>,
    doors: Vec<(i32, i32, u32, u32)>,
    warps: Vec<(i32, i32, u32)>,
}

fn marker_unit() -> PresetUnit {
    PresetUnit {
        unit_type: 2,
        class: 7,
        x: 1,
        y: 2,
    }
}

fn marker_grids() -> RoomGrids {
    RoomGrids {
        kill_edge_x: true,
        anim_speed: 9,
        ..RoomGrids::default()
    }
}

impl LevelTypes for Hooks {
    fn init_level(&mut self, drlg: &mut Drlg, _: &DrlgData, l: LevelIdx) -> Result<(), DrlgError> {
        self.inits.push(drlg.level(l).id);
        Ok(())
    }

    fn generate(&mut self, drlg: &mut Drlg, _: &DrlgData, l: LevelIdx) -> Result<(), DrlgError> {
        let r = drlg.alloc_room(l, RoomKind::Preset, TileRect::new(0, 0, 8, 8));
        drlg.link_room(r, LinkAt::Tail);
        Ok(())
    }

    fn reset_level(&mut self, drlg: &mut Drlg, l: LevelIdx) {
        self.resets.push(drlg.level(l).id);
    }

    fn add_preset_units(&mut self, _: &mut Drlg, room: DrlgRoomId) -> Result<(), DrlgError> {
        self.added.push(room);
        Ok(())
    }

    fn preset_units(&self, _: &Drlg, _: DrlgRoomId) -> Vec<PresetUnit> {
        vec![marker_unit()]
    }

    fn room_grids(
        &mut self,
        _: &mut Drlg,
        _: &DrlgData,
        room: DrlgRoomId,
    ) -> Result<RoomGrids, DrlgError> {
        self.grids.push(room);
        Ok(marker_grids())
    }

    fn free_room_tiles(&mut self, _: &mut Drlg, room: DrlgRoomId) {
        self.freed.push(room);
    }

    #[allow(clippy::too_many_arguments)]
    fn door_unit(
        &mut self,
        _: &mut Drlg,
        _: &DrlgData,
        _: DrlgRoomId,
        wx: i32,
        wy: i32,
        cell: u32,
        orientation: u32,
    ) {
        self.doors.push((wx, wy, cell, orientation));
    }

    fn warp_unit(&mut self, _: &mut Drlg, _: DrlgRoomId, wx: i32, wy: i32, cell: u32) {
        self.warps.push((wx, wy, cell));
    }
}

/// `levels.md` §4.3, §5.2, §9.4 and `rooms.md` §4, §9: the type hooks
/// dispatch by DrlgType. A non-outdoor level and its rooms reach the
/// other level types for every hook; an outdoor level's init only
/// zeroes its outdoor info (`0x00675320`).
#[test]
fn adapter_dispatches_by_drlg_type() {
    let mut data = data();
    data.levels[5].drlg_type = 2;
    data.levels[5].size = [(8, 8); 3];
    data.levels[2].drlg_type = 3;
    let od = od();
    let subs = SubFileMap::default();
    let mut outdoor = Outdoor::default();
    let mut presets = Presets::default();
    let mut hooks = Hooks::default();
    let mut drlg = Drlg::create(0, 1, 0, 0, false, &data, &mut NoLevelTypes).unwrap();
    let mut types = OutdoorTypes {
        outdoor: &mut outdoor,
        od: &od,
        subs: &subs,
        presets: &mut presets,
        others: &mut hooks,
        last_error: None,
    };
    let l5 = drlg.get_or_alloc_level(&data, &mut types, 5).unwrap();
    let l2 = drlg.get_or_alloc_level(&data, &mut types, 2).unwrap();
    assert_eq!(types.outdoor.level(l2), Some(&OutdoorLevel::default()));
    assert_eq!(types.outdoor.level(l5), None);
    drlg.generate_level(&data, &mut types, l5).unwrap();
    let r = drlg.level_rooms(l5)[0];
    types.add_preset_units(&mut drlg, r).unwrap();
    assert_eq!(types.preset_units(&drlg, r), [marker_unit()]);
    assert_eq!(types.room_grids(&mut drlg, &data, r), Ok(marker_grids()));
    types.free_room_tiles(&mut drlg, r);
    types.door_unit(&mut drlg, &data, r, 3, 4, 5, 9);
    types.warp_unit(&mut drlg, r, 6, 7, 8);
    types.reset_level(&mut drlg, l5);
    assert_eq!(types.last_error, None);
    assert_eq!(hooks.inits, [5]);
    assert_eq!(hooks.added, [r]);
    assert_eq!(hooks.grids, [r]);
    assert_eq!(hooks.freed, [r]);
    assert_eq!(hooks.doors, [(3, 4, 5, 9)]);
    assert_eq!(hooks.warps, [(6, 7, 8)]);
    assert_eq!(hooks.resets, [5]);
}

/// `levels.md` §4.3 through the act placer (§2): the levels it allocates
/// get their type init, except outdoor ones (zeroed info only).
#[test]
fn act_placer_inits_only_non_outdoor_levels() {
    let data = act1_data();
    let od = od();
    let subs = SubFileMap::default();
    let mut outdoor = Outdoor::default();
    let mut presets = Presets::default();
    let mut rec = Rec::default();
    let mut types = OutdoorTypes {
        outdoor: &mut outdoor,
        od: &od,
        subs: &subs,
        presets: &mut presets,
        others: &mut rec,
        last_error: None,
    };
    let drlg = Drlg::create(0, 644_409_375, 0, 0, false, &data, &mut types).unwrap();
    let mut levels = drlg.level_list();
    levels.sort();
    let want: Vec<u32> = levels
        .iter()
        .map(|&l| drlg.level(l))
        .filter(|l| l.drlg_type != 3)
        .map(|l| l.id)
        .collect();
    assert!(!want.is_empty());
    assert_eq!(rec.inits, want);
}

/// Freeing a room's tiles frees its outdoor grids (`rooms.md` §9.2 type
/// 1 `0x0067D680`); the room's other outdoor data stays.
#[test]
fn outdoor_room_tiles_freed() {
    let grid = || {
        let mut g = crate::drlg::tiles::CellGrid::new(9, 9);
        g.set(1, 1, 3);
        g
    };
    let room = OutdoorRoom {
        flags: 4,
        tile_type: grid(),
        wall: grid(),
        floor: grid(),
        ..OutdoorRoom::default()
    };
    let mut o = Outdoor::default();
    o.rooms.insert(DrlgRoomId(3), room.clone());
    assert_eq!(o.room(DrlgRoomId(3)), Some(&room));
    assert_eq!(o.room(DrlgRoomId(4)), None);
    o.free_room_tiles(DrlgRoomId(3));
    assert_eq!(
        o.room(DrlgRoomId(3)),
        Some(&OutdoorRoom {
            flags: 4,
            ..OutdoorRoom::default()
        })
    );
}
