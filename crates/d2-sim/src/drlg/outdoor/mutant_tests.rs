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
use crate::rng::Seed;

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

// ---- acts.rs ------------------------------------------------------------------

/// `outdoor.md` §8.3: the five (P, F, x, y) entries of each cliff row
/// (compared as a set: the spec does not order the middle entries).
#[test]
fn desert_cliff_rows() {
    use super::acts::desert_cliff_row;
    let sorted = |mut v: Vec<(u32, i32, i32, i32)>| {
        v.sort();
        v
    };
    for r in 0..8u32 {
        let want = match r {
            0..=2 => {
                let mut v = vec![(376, 1, 0, 4), (376, 2, 8, 4)];
                for x in [2, 4, 6] {
                    let p = if x == 2 + 2 * r as i32 { 378 } else { 377 };
                    v.push((p, -1, x, 4));
                }
                v
            }
            3 => vec![
                (376, 2, 8, 4),
                (377, -1, 6, 4),
                (382, -1, 4, 4),
                (381, -1, 4, 6),
                (379, 2, 4, 8),
            ],
            4 => vec![
                (376, 2, 8, 4),
                (378, -1, 6, 4),
                (382, -1, 4, 4),
                (380, -1, 4, 6),
                (379, 2, 4, 8),
            ],
            _ => {
                let mut v = vec![(379, 1, 4, 0), (379, 2, 4, 8)];
                for y in [2, 4, 6] {
                    let p = if y == 2 + 2 * (r as i32 - 5) {
                        381
                    } else {
                        380
                    };
                    v.push((p, -1, 4, y));
                }
                v
            }
        };
        assert_eq!(
            sorted(desert_cliff_row(r).to_vec()),
            sorted(want),
            "row {r}"
        );
    }
}

// ---- grid.rs: grids and preset primitives (§1.2, §5) ------------------------

use super::grid::{cell, shuffle_cells, Op};
use super::tests::Env;

/// `outdoor.md` §1.2: the grid ops of `0x0067C4F0` on an in-grid cell;
/// outside the grid nothing happens.
#[test]
fn grid_ops() {
    let mut g = Grid::new(2, 2);
    let apply = |g: &mut Grid, start: u32, op: Op, v: u32| {
        g.op(1, 1, Op::Set, start);
        g.op(1, 1, op, v);
        g.get(1, 1)
    };
    assert_eq!(apply(&mut g, 0b1100, Op::Or, 0b1010), 0b1110);
    assert_eq!(apply(&mut g, 0b1100, Op::And, 0b1010), 0b1000);
    assert_eq!(apply(&mut g, 0b1100, Op::Xor, 0b1010), 0b0110);
    assert_eq!(apply(&mut g, 0b1100, Op::Set, 0b1010), 0b1010);
    assert_eq!(apply(&mut g, 0b1100, Op::SetIfZero, 0b1010), 0b1100);
    assert_eq!(apply(&mut g, 0, Op::SetIfZero, 0b1010), 0b1010);
    assert_eq!(apply(&mut g, 0b1100, Op::AndNot, 0b1010), 0b0100);
    g.op(2, 0, Op::Set, 7);
    g.op(-1, 0, Op::Set, 7);
    assert_eq!(g.cells, [0, 0, 0, 0b0100]);
}

/// `outdoor.md` §5.1 r2, r3: every covered cell of a multi-cell preset
/// gets the preset bit, the file in bits 16..19 and (border presets with
/// the flag) bit 0x1; grid 0 is cleared and (x, y) := P.
#[test]
fn stamp_covers_every_cell() {
    let mut e = Env::new(2, 8, 8);
    e.od.presets[5] = PresetDef {
        size_x: 16,
        size_y: 24,
        files: 1,
    };
    e.info.grids[2].op(3, 4, Op::Set, 0x7_0000);
    e.info.grids[0].op(3, 3, Op::Set, 9);
    let mut g = e.gen();
    g.stamp(2, 3, 5, 6, true).unwrap();
    for y in 0..8 {
        for x in 0..8 {
            let inside = (2..4).contains(&x) && (3..6).contains(&y);
            let want = if inside {
                cell::PRESET | 6 << 16 | cell::BORDER
            } else {
                0
            };
            assert_eq!(g.g(2, x, y), want, "({x}, {y})");
            let p = if (x, y) == (2, 3) { 5 } else { 0 };
            assert_eq!(g.g(0, x, y), p, "({x}, {y})");
        }
    }
}

/// `outdoor.md` §5.2: margin m with flag bits 1 (y − m, h + m), 2 (w + m),
/// 4 (h + m), 8 (x − m, w + m); every cell must be spawn valid.
#[test]
fn fits_margins_by_flag() {
    for (flags, blocked, free) in [
        (1, (5, 3), (5, 7)),
        (2, (7, 5), (3, 5)),
        (4, (5, 7), (5, 3)),
        (8, (3, 5), (7, 5)),
    ] {
        for (at, want) in [(blocked, false), (free, true)] {
            let mut e = Env::new(2, 10, 10);
            e.info.grids[2].op(at.0, at.1, Op::Set, cell::NOT_SPAWN);
            let g = e.gen();
            assert_eq!(g.fits(5, 5, 0, 2, flags), Ok(want), "flags {flags} {at:?}");
            // Without a margin the flags do nothing.
            assert_eq!(g.fits(5, 5, 0, 0, flags), Ok(true));
        }
    }
}

/// `outdoor.md` §5.3: candidate cell of entry k is (x + 1, y + 1).
#[test]
fn shuffle_candidates_are_offset_by_one() {
    let mut e = Env::new(2, 6, 5);
    let mut s = e.seed();
    let want: Vec<_> = shuffle_cells(&mut s, 4, 3)
        .into_iter()
        .map(|(x, y)| (x + 1, y + 1))
        .collect();
    let mut g = e.gen();
    assert_eq!(g.shuffle(), want);
}

/// `outdoor.md` §5.4 SpawnRandomDS1: around the (only) path cell, the
/// neighbours are tried in the order dx = [−1, 0, 0, 1, −1, 1, 1, −1],
/// dy = [0, −1, 1, 0, −1, 1, −1, 1]; the first that fits is stamped.
#[test]
fn random_ds1_neighbour_order() {
    const DX: [i32; 8] = [-1, 0, 0, 1, -1, 1, 1, -1];
    const DY: [i32; 8] = [0, -1, 1, 0, -1, 1, -1, 1];
    for k in 0..8 {
        let mut e = Env::new(2, 9, 9);
        for y in 0..9 {
            for x in 0..9 {
                e.info.grids[2].op(x, y, Op::Set, cell::BLANK);
            }
        }
        e.info.grids[2].op(4, 4, Op::Set, cell::PATH);
        // Neighbours k.. are free; the earlier ones blocked.
        for (j, (dx, dy)) in DX.iter().zip(DY).enumerate().skip(k) {
            let _ = j;
            e.info.grids[2].op(4 + dx, 4 + dy, Op::Set, 0);
        }
        // One shuffle only: no fallback to SpawnOutdoorLevelPreset.
        let mut s = e.seed();
        shuffle_cells(&mut s, 7, 7);
        let mut g = e.gen();
        g.random_ds1(7, 0).unwrap();
        assert_eq!(g.g(0, 4 + DX[k], 4 + DY[k]), 7, "k {k}");
        assert_eq!(*g.seed(), s, "k {k}");
    }
}

/// `outdoor.md` §5.4 FarAway, computed from the rule text: rx := roll(gw
/// − 2), ry := roll(gh − 2); W, H := gw − 2, gh − 2; centre (rect.x +
/// rect.w/2, rect.y + rect.h/2); i in 0..=H, j in 0..=W: cell ((j + rx)
/// mod W + 1, (i + ry) mod H + 1); ax := |8x − cx + level.x + 4|, ay
/// likewise; d := (ax > ay ? ay + 2ax : ax + 2ay) / 2; the first strictly
/// larger d wins.
#[test]
fn far_away_by_the_rule() {
    // Rects around the level and a symmetric one (the level's centre:
    // corner ties decided by the scan start).
    let mut rects: Vec<TileRect> = (0..300)
        .map(|k| {
            TileRect::new(
                780 + 7 * k % 90,
                790 + 13 * k % 80,
                1 + 5 * k % 23,
                1 + 3 * k % 17,
            )
        })
        .collect();
    rects.extend([TileRect::new(800, 800, 72, 64); 40]);
    for (k, rect) in rects.into_iter().enumerate() {
        let blocked: Vec<(i32, i32)> = (0..k as i32 % 4)
            .map(|b| (1 + (3 * b + k as i32) % 7, 1 + (5 * b + k as i32) % 6))
            .collect();
        far_away_case(rect, 1000 + 7919 * k as u32, &blocked);
    }
    // Only (1, 2) and (2, 1) fit, with raw values 80 and 81 (d 40 both):
    // the first visited wins.
    let mut blocked = Vec::new();
    for y in 1..7 {
        for x in 1..8 {
            if (x, y) != (1, 2) && (x, y) != (2, 1) {
                blocked.push((x, y));
            }
        }
    }
    for k in 0..20 {
        far_away_case(TileRect::new(790, 791, 1, 1), 7919 * k, &blocked);
    }
}

/// FarAway on a 9 × 8 grid at (800, 800) against the rule text.
fn far_away_case(rect: TileRect, seed: u32, blocked: &[(i32, i32)]) {
    let (gw, gh) = (9, 8);
    let mut e = Env::new(2, gw, gh);
    e.drlg.level_mut(e.l).seed = Seed::init_low(seed);
    for &(x, y) in blocked {
        e.info.grids[2].op(x, y, Op::Set, cell::NOT_SPAWN);
    }
    let level = e.drlg.level(e.l).rect;
    let mut s = e.seed();
    let rx = s.roll(gw - 2) as i32;
    let ry = s.roll(gh - 2) as i32;
    let (w, h) = (gw - 2, gh - 2);
    let (cx, cy) = (rect.x + rect.w / 2, rect.y + rect.h / 2);
    let mut best: Option<(i32, i32, i32)> = None;
    for i in 0..=h {
        for j in 0..=w {
            let x = (j + rx) % w + 1;
            let y = (i + ry) % h + 1;
            if blocked.contains(&(x, y)) {
                continue;
            }
            let ax = (8 * x - cx + level.x + 4).abs();
            let ay = (8 * y - cy + level.y + 4).abs();
            let d = if ax > ay { ay + 2 * ax } else { ax + 2 * ay } / 2;
            if best.map_or(true, |b| d > b.2) {
                best = Some((x, y, d));
            }
        }
    }
    let (bx, by, _) = best.unwrap();
    let mut g = e.gen();
    assert_eq!(g.far_away(rect, 9, 0, 0, 15), Ok(true));
    assert_eq!(g.g(0, bx, by), 9, "{rect:?} seed {seed}");
    assert_eq!(*g.seed(), s, "two draws");
}

/// `outdoor.md` §5.4 Waypoint, level 3: i := first vis slot of level 3
/// holding 2, mask := 1 << (i + 4); the first cell (rows, then columns)
/// with grid-1 & mask **and** grid-2 & 0x400, clamped to 1..gw−2,
/// 1..gh−2, gets grid 1 |= 0x20000, grid 2 |= 0x800.
#[test]
fn waypoint_on_the_cold_plains_link() {
    for (at, clamped) in [((0, 3), (1, 3)), ((7, 2), (6, 2)), ((4, 7), (4, 6))] {
        let mut e = Env::new(3, 8, 8);
        e.data.levels[3].vis = [17, 2, 0, 0, 0, 0, 0, 0];
        let mask = 1 << (1 + 4);
        // Decoys earlier in scan order: mask only, link only.
        e.info.grids[1].op(1, 0, Op::Set, mask);
        e.info.grids[2].op(2, 0, Op::Set, cell::LINK);
        e.info.grids[1].op(at.0, at.1, Op::Set, mask);
        e.info.grids[2].op(at.0, at.1, Op::Set, cell::LINK);
        let mut g = e.gen();
        g.waypoint().unwrap();
        let (x, y) = clamped;
        assert_eq!(g.g(1, x, y) & 0x20000, 0x20000, "{at:?}");
        assert_eq!(g.g(2, x, y) & cell::WAYPOINT, cell::WAYPOINT, "{at:?}");
    }
}

/// `outdoor.md` §5.4 Shrines(n): k := lo' & 3, then each placed shrine
/// takes bit [0x1000, 0x2000, 0x4000, 0x8000][k] and k := (k + 1) mod 4.
#[test]
fn shrines_cycle_their_bits() {
    let mut e = Env::new(2, 8, 8);
    let mut g = e.gen();
    g.shrines(5);
    let mut bits: Vec<u32> = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            if g.g(2, x, y) & cell::SHRINE != 0 {
                bits.push(g.g(1, x, y));
            }
        }
    }
    bits.sort();
    // Five shrines: one bit twice, the other three once.
    assert_eq!(bits.len(), 5);
    for b in [0x1000, 0x2000, 0x4000, 0x8000] {
        assert!(bits.contains(&b), "{b:#x}");
    }
}
