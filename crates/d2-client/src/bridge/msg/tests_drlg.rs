// Spec: specs/client/model.md (§2 r6, §9, §11, §12), specs/drlg/rooms.md (§4.2)
//! The client DRLG copy through the S→C handlers (synthetic level data:
//! level 2 is a row of three 8×8-tile preset rooms of floor tiles).

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_sim::drlg::room::LinkAt;
use d2_sim::drlg::tiles::{cell, FIXED_LIBRARY};
use d2_sim::drlg::{
    CellGrid, Drlg, DrlgData, DrlgError, DrlgRoomId, GridPass, LevelDef, LevelIdx, LevelTypes,
    RoomGrids, RoomKind, TileInfo, TileRect,
};
use d2_sim::rng::Seed;

use super::super::drlg::{ClientDrlg, DrlgSource};
use super::super::world::{ActiveRoom, LevelRow, MonsterClass, UnitKey, MONSTER, PLAYER};
use super::support::{hex, Model};

/// Level 2's rooms (tile rectangles), allocation order.
const ROOMS: [(i32, i32); 3] = [(0, 0), (8, 0), (16, 0)];

struct RowTypes;

impl LevelTypes for RowTypes {
    fn generate(
        &mut self,
        drlg: &mut Drlg,
        _: &DrlgData,
        level: LevelIdx,
    ) -> Result<(), DrlgError> {
        if drlg.level(level).id == 2 {
            for (x, y) in ROOMS {
                let r = drlg.alloc_room(level, RoomKind::Preset, TileRect::new(x, y, 8, 8));
                drlg.room_mut(r).dt1_mask = 1;
                drlg.link_room(r, LinkAt::Tail);
            }
        }
        Ok(())
    }

    fn room_grids(
        &mut self,
        drlg: &mut Drlg,
        _: &DrlgData,
        room: DrlgRoomId,
    ) -> Result<RoomGrids, DrlgError> {
        let r = drlg.room(room).rect;
        let (w, h) = (r.w as usize + 1, r.h as usize + 1);
        let mut g = CellGrid::new(w, h);
        for y in 0..h {
            for x in 0..w {
                g.set(x, y, cell::FLOOR);
            }
        }
        Ok(RoomGrids {
            passes: vec![GridPass {
                cells: g,
                orientation: None,
                fill_blanks: false,
            }],
            ..RoomGrids::default()
        })
    }
}

fn tile(o: u32, main: u32, sub: u32, rarity: u32) -> TileInfo {
    TileInfo {
        orientation: o,
        main,
        sub,
        rarity,
        material: 0,
        subtile_flags: [0; 25],
    }
}

fn source() -> DrlgSource {
    let mut data = DrlgData {
        levels: vec![LevelDef::default(); 4],
        ..DrlgData::default()
    };
    for l in &mut data.levels {
        l.warp = [-1; 8];
    }
    data.levels[2].drlg_type = 2;
    data.levels[2].level_type = 1;
    data.levels[2].size = [(24, 8); 3];
    let mut files = vec![Vec::new(); 32];
    files[0] = b"floor.dt1".to_vec();
    data.lvltypes = vec![vec![Vec::new(); 32], files];
    let mut t = BTreeMap::new();
    t.insert(b"floor.dt1".to_vec(), vec![tile(0, 0, 0, 1)]);
    t.insert(FIXED_LIBRARY[0].to_vec(), vec![]);
    t.insert(FIXED_LIBRARY[1].to_vec(), vec![]);
    t.insert(FIXED_LIBRARY[2].to_vec(), vec![tile(10, 0, 0, 0)]);
    DrlgSource {
        data: Arc::new(data),
        tiles: Arc::new(d2_server::world_data::Dt1Files(t)),
        types: Arc::new(|| Box::new(RowTypes)),
    }
}

/// A model with the client DRLG source, Levels rows (level 2 in act 0)
/// and `monstats` rows for 0xAC.
fn model() -> Model {
    let mut m = Model::default();
    m.inputs.drlg = Some(source());
    m.inputs.tables.levels = vec![LevelRow::default(); 4];
    m.inputs.tables.monsters = vec![Some(MonsterClass::default()); 155];
    m
}

/// 0x07 / 0x08 for level 2 at tile (x, y).
fn sight(show: bool, x: u16, y: u16) -> Vec<u8> {
    let mut b = vec![if show { 0x07 } else { 0x08 }];
    b.extend_from_slice(&x.to_le_bytes());
    b.extend_from_slice(&y.to_le_bytes());
    b.push(2);
    b
}

/// 0x59 for player 1 at (x, y).
fn assign_player(x: u16, y: u16) -> Vec<u8> {
    let mut b = hex("59 01 00 00 00 01 77 65 72 77 65 72");
    b.resize(0x16, 0);
    b.extend_from_slice(&x.to_le_bytes());
    b.extend_from_slice(&y.to_le_bytes());
    b
}

/// 0xAC for monster `guid` of class 0 at (x, y) with an empty stream.
fn assign_monster(guid: u32, x: u16, y: u16) -> Vec<u8> {
    let mut b = vec![0xAC];
    b.extend_from_slice(&guid.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&x.to_le_bytes());
    b.extend_from_slice(&y.to_le_bytes());
    // Life 0x80, size 14, stream: mode 1, no optional parts.
    b.extend_from_slice(&[0x80, 0x0E, 0x01]);
    b
}

/// The sub-tile rectangle of level-2 room `k`.
fn rect(k: usize) -> (i32, i32, i32, i32) {
    let (x, y) = ROOMS[k];
    (x * 5, y * 5, 40, 40)
}

fn rects(rooms: &[ActiveRoom]) -> Vec<(i32, i32, i32, i32)> {
    rooms.iter().map(|r| (r.x0, r.y0, r.w, r.h)).collect()
}

/// The act seed of 0x03 below.
const INIT: u32 = 0x1038_88C4;

// Covers: specs/client/model.md §12 r1, §7 r4; specs/drlg/levels.md §2 r3
#[test]
fn load_act_builds_the_client_drlg() {
    let mut m = model();
    m.hex("01 02 04 00 10 00 01 00");
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    let d = m.w.drlg.as_ref().expect("built");
    // The client copy: init seed u32@2, the game's difficulty, client
    // flag, town id 0 (no level generated at creation).
    assert_eq!(
        (d.drlg.act, d.drlg.init_seed, d.drlg.difficulty),
        (0, INIT, 2)
    );
    assert!(d.drlg.on_client);
    assert!(d.drlg.find_level(1).is_none());
    // The same draws as the server's DRLG creation of that seed.
    let mut s = Seed::init_low(INIT);
    assert_eq!(d.drlg.start_seed, s.step());
    assert_eq!(m.w.active_rooms, Some(Vec::new()));
    // Without a DRLG source no client DRLG is built.
    let mut bare = Model::default();
    bare.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    assert!(bare.w.drlg.is_none() && bare.w.active_rooms.is_none());
}

// Covers: specs/client/model.md §9 r1, §9 r2, §12 r1; specs/drlg/rooms.md §4.2, §5 r5
#[test]
fn rooms_come_in_sight_and_go() {
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    // 0x07: the room at tile (8, 0) of level 2 is built and listed.
    m.recv(&sight(true, 8, 0));
    let rooms = m.w.active_rooms.clone().unwrap();
    assert_eq!(rects(&rooms), [rect(1)]);
    assert_eq!(rooms[0].level, 2);
    // New active rooms are prepended (act list, newest first).
    m.recv(&sight(true, 0, 0));
    assert_eq!(
        rects(m.w.active_rooms.as_ref().unwrap()),
        [rect(0), rect(1)]
    );
    // The rooms-in-sight record still lists every message (§9 r4).
    assert_eq!(m.w.rooms_in_sight.len(), 2);
    // 0x08 for room 1: its status-1 count drops to 0, but room 0 (still
    // in sight) keeps it at status 2, so its tiles and active room stay.
    m.recv(&sight(false, 8, 0));
    assert_eq!(
        rects(m.w.active_rooms.as_ref().unwrap()),
        [rect(0), rect(1)]
    );
    let st = |m: &Model, k: usize| {
        let d = &m.w.drlg.as_ref().unwrap().drlg;
        let l = d.find_level(2).unwrap();
        d.room(d.level_rooms(l)[k]).status
    };
    assert_eq!((st(&m, 0), st(&m, 1), st(&m, 2)), (1, 2, 3));
    // 0x08 for room 0: every count drops to 0; the client copy frees the
    // tiles of rooms back at status 4 (`rooms.md` §4 unset handler 3).
    m.recv(&sight(false, 0, 0));
    assert_eq!(m.w.active_rooms, Some(Vec::new()));
    assert_eq!((st(&m, 0), st(&m, 1), st(&m, 2)), (4, 4, 4));
    // A point in no room of the level: an unspecified case, refused.
    m.recv(&sight(true, 200, 200));
    assert_eq!(m.rejected().len(), 1);
    assert_eq!(m.rejected()[0].0, 0x07);
    // A new 0x03 frees the act and builds a new one.
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    assert_eq!(m.w.active_rooms, Some(Vec::new()));
}

// Covers: specs/client/model.md §2 r6, §12 r5
#[test]
fn units_created_at_a_point_take_the_rooms_seed() {
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&sight(true, 8, 0));
    let room = m.w.active_rooms.as_ref().unwrap()[0].room;
    let seed = |m: &Model| {
        m.w.drlg
            .as_ref()
            .unwrap()
            .drlg
            .active_room(room)
            .unwrap()
            .seed
    };
    // A monster at sub-tile (45, 5) in room 1: the room's seed is stepped
    // once and the unit seed is init_low(lo').
    let mut room_seed = seed(&m);
    let lo = room_seed.step();
    m.recv(&assign_monster(6, 45, 5));
    assert_eq!(seed(&m), room_seed);
    let u = m.unit(UnitKey::new(MONSTER, 6));
    assert_eq!(u.seed, Some((lo, 666)));
    // A player: the room step, then one step of the new unit's seed.
    let lo = room_seed.step();
    m.recv(&assign_player(46, 6));
    let mut unit = Seed::init_low(lo);
    unit.step();
    assert_eq!(
        m.unit(UnitKey::new(PLAYER, 1)).seed,
        Some((unit.lo, unit.hi))
    );
    // At (0, 0): no room, no step.
    m.recv(&assign_monster(7, 0, 0));
    assert_eq!(seed(&m), room_seed);
    // At a point in no active room: fatal 0x13C, nothing created.
    let before = m.rejected().len();
    m.recv(&assign_monster(8, 100, 5));
    assert_eq!(m.rejected().len(), before + 1);
    assert_eq!(m.rejected()[before].1, "fatal assert 0x13C");
    assert!(!m.w.units.contains_key(&UnitKey::new(MONSTER, 8)));
}

// Covers: specs/client/model.md §11 r3, §11 r5, §12 r2
#[test]
fn the_players_level_is_its_rooms() {
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&assign_player(0, 0)).hex("0b 00 01 00 00 00");
    assert_eq!(m.w.player_level(), None);
    m.recv(&sight(true, 8, 0)).recv(&sight(true, 16, 0));
    // 0x15 into room 1 (sub-tile (45, 5)).
    m.hex("15 00 01 00 00 00 2d 00 05 00 01");
    assert_eq!(m.w.player_level(), Some(2));
    let own = m.w.local_room().copied().unwrap();
    assert_eq!((own.x0, own.y0), (40, 0));
    // The cell lookup from the player's room: room 2 is in its adjacency
    // array, so the point resolves there as the act lookup would.
    let other = m.w.room_at(85, 5).unwrap();
    assert_eq!((other.x0, other.y0), (80, 0));
    assert!(m
        .w
        .drlg
        .as_ref()
        .unwrap()
        .adjacency(own.room)
        .contains(&other.room));
    // A point in no active room: none (0x15 there is fatal 0x168).
    assert_eq!(m.w.room_at(5, 5), None);
    m.hex("15 00 01 00 00 00 05 00 05 00 01");
    assert_eq!(m.rejected().last().unwrap().1, "fatal assert 0x168");
    // The client DRLG is the bridge's own: building one directly from
    // the same source and seed gives the same rooms.
    let mut own_copy = ClientDrlg::build(&source(), 0, INIT, 0).unwrap();
    own_copy.set_in_sight(2, 8, 0, None).unwrap();
    own_copy.set_in_sight(2, 16, 0, None).unwrap();
    assert_eq!(own_copy.active_rooms(), m.w.active_rooms.clone().unwrap());
}

// Covers: specs/client/model.md §6 r8
#[test]
fn a_correction_needs_a_room_for_the_point() {
    use super::super::check::{check, Checked};
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&sight(true, 8, 0)).recv(&sight(true, 16, 0));
    m.recv(&assign_monster(6, 45, 5));
    let k = UnitKey::new(MONSTER, 6);
    // Far (> 15) and in no active room (room 0 is not in sight): nothing.
    let inputs = m.inputs.clone();
    assert_eq!(
        check(&mut m.w, &inputs, k, 5, 5, 0, 0, 0).unwrap(),
        Checked::NoRoom
    );
    assert_eq!(m.unit(k).position, Some((45, 5)));
    // Far and in room 2 (adjacent to the monster's room): teleported.
    assert_eq!(
        check(&mut m.w, &inputs, k, 85, 5, 0, 0, 0).unwrap(),
        Checked::Moved
    );
    assert_eq!(m.unit(k).position, Some((85, 5)));
}
