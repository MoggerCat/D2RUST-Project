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
        roof_height: 0,
        height: 0,
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

// Covers: specs/client/model.md §9 r1, §9 r2, §9 r5, §12 r1; specs/drlg/rooms.md §4.2, §5 r5
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
    // A point in no room of the level (§9 r5): 0x07 reads the null
    // room's count, an access violation that ends 1.14d (refused and
    // recorded); 0x08 tests the room and does nothing. Both are recorded.
    m.recv(&sight(true, 200, 200));
    assert_eq!(m.rejected().len(), 1);
    assert_eq!(m.rejected()[0].0, 0x07);
    assert_eq!(
        m.rejected()[0].1,
        "access violation at 0x0061B672: 0x07 at a point in no room of the level reads the \
         null room's count"
    );
    m.recv(&sight(false, 200, 200));
    assert_eq!(m.rejected().len(), 1);
    assert_eq!(m.w.rooms_in_sight.len(), 6);
    // A new 0x03 frees the act and builds a new one.
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    assert_eq!(m.w.active_rooms, Some(Vec::new()));
}

// Covers: specs/client/model.md §2 r6, §2 r7, §12 r5
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

// Covers: specs/sim/unit-order.md §5 r6, §5 r8
#[test]
fn units_live_in_their_rooms_lists() {
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&sight(true, 8, 0)).recv(&sight(true, 16, 0));
    let rooms = m.w.active_rooms.clone().unwrap();
    let (r2, r1) = (rooms[0].room, rooms[1].room);
    let mon = UnitKey::new(MONSTER, 6);
    let pl = UnitKey::new(PLAYER, 1);
    // Creation at a point: the head of the creation room's list.
    m.recv(&assign_monster(6, 45, 5));
    m.recv(&assign_player(46, 6));
    assert_eq!(m.w.room_units.list(r1), [pl, mon]);
    assert_eq!(m.w.unit_room(pl).map(|r| r.room), Some(r1));
    // At (0, 0): no room, in no list.
    m.recv(&assign_monster(7, 0, 0));
    assert_eq!(m.w.room_units.room_of(UnitKey::new(MONSTER, 7)), None);
    // 0x15 to room 2: the room recache (leave, head of the new list).
    m.hex("0b 00 01 00 00 00");
    m.hex("15 00 01 00 00 00 55 00 05 00 01");
    assert_eq!(m.w.room_units.list(r1), [mon]);
    assert_eq!(m.w.room_units.list(r2), [pl]);
    assert_eq!(m.w.local_room().map(|r| r.room), Some(r2));
    // 0x0A: the unit free leaves the list.
    m.hex("0a 01 06 00 00 00");
    assert!(m.w.room_units.list(r1).is_empty());
    // Both rooms out of sight: the client room free empties the lists.
    m.recv(&sight(false, 8, 0)).recv(&sight(false, 16, 0));
    assert_eq!(m.w.active_rooms, Some(Vec::new()));
    assert_eq!(m.w.room_units.room_of(pl), None);
    assert_eq!(m.w.local_room(), None);
}

// Covers: specs/sim/unit-order.md §5 r7
#[test]
fn the_draw_order_of_a_list_is_written_back() {
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&sight(true, 8, 0));
    let r1 = m.w.active_rooms.as_ref().unwrap()[0].room;
    m.recv(&assign_monster(6, 45, 5))
        .recv(&assign_monster(7, 46, 5));
    let (a, b) = (UnitKey::new(MONSTER, 6), UnitKey::new(MONSTER, 7));
    assert_eq!(m.w.room_units.list(r1), [b, a]);
    // A permutation is kept; anything else is refused.
    assert!(m.w.room_units.set_order(r1, &[a, b]));
    assert_eq!(m.w.room_units.list(r1), [a, b]);
    assert!(!m.w.room_units.set_order(r1, &[a]));
    assert!(!m.w.room_units.set_order(r1, &[a, UnitKey::new(MONSTER, 9)]));
    assert_eq!(m.w.room_units.list(r1), [a, b]);
    // Later inserts prepend into the kept order.
    m.recv(&assign_monster(8, 47, 5));
    assert_eq!(m.w.room_units.list(r1), [UnitKey::new(MONSTER, 8), a, b]);
}

// Covers: specs/drlg/rooms.md §4.6 r1, §4.6 r6, §4.6 r7
#[test]
fn the_update_pass_runs_the_build_timer() {
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&sight(true, 8, 0));
    let two = |m: &Model| m.w.drlg.as_ref().unwrap().drlg.status_list(2).to_vec();
    let waiting = two(&m);
    assert_eq!(waiting.len(), 2);
    let active = |m: &Model| {
        m.w.active_rooms
            .as_ref()
            .unwrap()
            .iter()
            .map(|r| r.room)
            .collect::<Vec<_>>()
    };
    assert_eq!(active(&m).len(), 1);
    // Set handler 1 built room 1 (T := 5, B = 1): the fifth update builds
    // the first status-2 room, the tenth the next.
    for _ in 0..4 {
        m.drain();
    }
    assert_eq!(active(&m).len(), 1);
    m.drain();
    assert_eq!(active(&m)[0], waiting[0]);
    for _ in 0..4 {
        m.drain();
    }
    assert_eq!(active(&m).len(), 2);
    m.drain();
    assert_eq!(active(&m)[0], waiting[1]);
    assert_eq!(m.w.drlg_updates, 10);
    // Every room is now active: the units' room lookups see them.
    assert!(m.w.room_at(5, 5).is_some() && m.w.room_at(85, 5).is_some());
    assert!(m.rejected().is_empty());
}

// Covers: specs/drlg/rooms.md §5 r9; specs/render/lighting.md §6.4
#[test]
fn a_new_active_room_drops_light_caches_reaching_it() {
    use crate::rules::lighting::records::{LightKind, Owner};
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&sight(true, 8, 0));
    // Two monsters in room 1 (sub-tiles 40…79): one 5 sub-tiles from
    // room 2, one 35 away.
    m.recv(&assign_monster(6, 75, 5))
        .recv(&assign_monster(7, 45, 5));
    let owner = |guid| Owner {
        unit_type: 1,
        guid,
        client_only: false,
    };
    let mut cached = |guid| {
        let id =
            m.w.lights
                .create(
                    Some(owner(guid)),
                    (0, 0),
                    LightKind::Cached,
                    10,
                    255,
                    0,
                    0,
                    0,
                )
                .unwrap();
        m.w.lights.get_mut(id).unwrap().cache_valid = true;
        id
    };
    let (near, far) = (cached(6), cached(7));
    // 0x07 builds room 2: the callback probes (75 + 10, 5) from room 1,
    // which its adjacency array resolves to room 2.
    m.recv(&sight(true, 16, 0));
    assert!(!m.w.lights.get(near).unwrap().cache_valid);
    assert!(m.w.lights.get(far).unwrap().cache_valid);
    // An owner the model does not hold: fatal 0x591.
    m.w.lights
        .create(Some(owner(99)), (0, 0), LightKind::Cached, 3, 255, 0, 0, 0)
        .unwrap();
    m.recv(&sight(true, 0, 0));
    assert_eq!(m.rejected().last().unwrap().1, "fatal assert 0x591");
}

/// An inner feed whose unit facts are all zero / false (the facts the
/// model does not hold), for the near-room build.
struct ZeroFacts;

impl crate::rules::ViewSource for ZeroFacts {
    fn unit_position(
        &self,
        u: &crate::bridge::ClientUnit,
    ) -> Result<crate::rules::UnitPosition, String> {
        crate::world_view::model_feed::unit_position(u)
    }
    fn unit_offset(
        &self,
        _: &crate::bridge::ClientUnit,
        _: &crate::world_view::UnitPose,
    ) -> Result<(i32, i32), String> {
        Ok((0, 0))
    }
    fn map_tiles(
        &self,
        _: &super::super::world::ClientWorld,
        _: &crate::world_view::ViewAssets,
    ) -> Result<Vec<crate::rules::MapTile>, crate::world_view::ViewError> {
        Ok(Vec::new())
    }
}

impl crate::world_view::ViewFeed for ZeroFacts {
    fn player(
        &self,
        _: &super::super::world::ClientWorld,
    ) -> Result<Option<crate::rules::UnitPosition>, crate::world_view::ViewError> {
        Ok(None)
    }
    fn open_mode(
        &self,
        _: &super::super::world::ClientWorld,
    ) -> Result<crate::rules::OpenMode, crate::world_view::ViewError> {
        Ok(crate::rules::OpenMode::new(0).unwrap())
    }
    fn shake(
        &self,
        _: &super::super::world::ClientWorld,
    ) -> Result<Option<crate::world_view::feed::RunningShake>, crate::world_view::ViewError> {
        Ok(None)
    }
    fn player_seed(
        &mut self,
        _: &super::super::world::ClientWorld,
    ) -> Result<&mut Seed, crate::world_view::ViewError> {
        unreachable!("not read by the near-room build")
    }
    fn blank_screen(
        &self,
        _: &super::super::world::ClientWorld,
    ) -> Result<bool, crate::world_view::ViewError> {
        Ok(true)
    }
    fn unit_facts(
        &self,
        _: &super::super::world::ClientWorld,
        _: &crate::bridge::ClientUnit,
    ) -> Result<crate::rules::draw_order::UnitFacts, crate::world_view::ViewError> {
        Ok(crate::rules::draw_order::UnitFacts::default())
    }
}

// Covers: specs/render/draw-order.md §9, §3 r2; specs/drlg/rooms.md §9.3 text; specs/sim/unit-order.md §5 r7
#[test]
fn near_rooms_come_from_the_client_drlg() {
    use crate::rules::draw_order::{TileArray, REC_DRAWN};
    use crate::world_view::model_feed::ModelFeed;
    use crate::world_view::ViewFeed;
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&sight(true, 8, 0)).recv(&sight(true, 16, 0));
    m.recv(&assign_player(46, 6)).hex("0b 00 01 00 00 00");
    m.recv(&assign_monster(6, 47, 9));
    let mut feed = ModelFeed::new(ZeroFacts).with_map();
    // Without Levels rows the level facts (DrawEdges) are unknown.
    assert!(feed.near_rooms(&m.w).is_err());
    let mut rows = vec![LevelRow::default(); 4];
    rows[2].draw_edges = true;
    feed.levels = Some(rows);
    // The map off: the inner feed's answer (none).
    assert!(ModelFeed::new(ZeroFacts)
        .near_rooms(&m.w)
        .unwrap()
        .is_none());
    let cd = m.w.drlg.clone().unwrap();
    let own = m.w.local_room().copied().unwrap();
    let adjacency = cd.adjacency(own.room);
    let near = feed.near_rooms(&m.w).unwrap().unwrap();
    // The local player's room's adjacency array, in order.
    assert_eq!(near.rooms.len(), adjacency.len());
    assert_eq!((near.level.id, near.level.draw_edges), (2, true));
    assert_eq!(near.player_tile, (9, 1));
    let rec = cd.drlg.coord_at(own.room, 46, 6).unwrap();
    assert_eq!(near.player_logical, rec.index as i32);
    let k = adjacency.iter().position(|&r| r == own.room).unwrap();
    let room = &near.rooms[k];
    let drlg_room = cd.drlg.room(own.room);
    assert_eq!(
        (room.tiles.x, room.tiles.y, room.tiles.w, room.tiles.h),
        (8, 0, 8, 8)
    );
    assert_eq!(room.subtile_origin, (40, 0));
    let tiles = drlg_room.tiles().unwrap();
    assert_eq!(room.floors.len(), tiles.floors.len());
    assert_eq!(room.walls.len(), tiles.walls.len());
    assert!(!room.floors.is_empty());
    assert_eq!(room.floors[0].flags, tiles.floors[0].flags);
    assert_eq!(
        (room.floors[0].dt1.roof_height, room.floors[0].dt1.height),
        (0, 0)
    );
    // The unit list in the client's order (newest first), the model's
    // facts over the inner feed's.
    let keys: Vec<UnitKey> = room.units.iter().map(|u| u.key).collect();
    assert_eq!(keys, [UnitKey::new(MONSTER, 6), UnitKey::new(PLAYER, 1)]);
    assert!(room.units[1].facts.local && !room.units[0].facts.local);
    assert_eq!(room.units[1].facts.mode, 5);
    // The draw writes flags into a record and sorts the list (here by
    // hand): the flags persist into the next frame's build, the order
    // goes back to the client's list.
    room_mut(near, k).floors[0].flags |= REC_DRAWN;
    room_mut(near, k).units.reverse();
    room_mut(near, k).units_sorted = true;
    let orders = feed.take_unit_orders();
    assert_eq!(orders.len(), 1);
    assert!(feed.take_unit_orders().is_empty());
    assert!(m.w.room_units.set_order(orders[0].0, &orders[0].1));
    m.w.frames += 1;
    let near = feed.near_rooms(&m.w).unwrap().unwrap();
    assert_ne!(near.rooms[k].floors[0].flags & REC_DRAWN, 0);
    let keys: Vec<UnitKey> = near.rooms[k].units.iter().map(|u| u.key).collect();
    assert_eq!(keys, [UnitKey::new(PLAYER, 1), UnitKey::new(MONSTER, 6)]);
    // Each record's DT1 entry is (path, index in file order).
    let map = feed.map.as_ref().unwrap();
    assert_eq!(
        map.entry(k, TileArray::Floor, 0),
        Some(&(b"floor.dt1".to_vec(), 0))
    );
    // The default inner feed refuses the facts the model lacks.
    let mut bare = ModelFeed::<crate::world_view::feed::NoFeed>::default().with_map();
    bare.levels = Some(vec![LevelRow::default(); 4]);
    assert!(bare.near_rooms(&m.w).is_err());
}

// Covers: specs/render/draw-order.md §9
// Covers: specs/seams/world-screen.md §2.4
#[test]
fn the_fade_player_tile_is_the_predicted_sub_tile() {
    use crate::world_view::model_feed::ModelFeed;
    use crate::world_view::preview::Preview;
    use crate::world_view::ViewFeed;
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&sight(true, 8, 0)).recv(&sight(true, 16, 0));
    m.recv(&assign_player(46, 6)).hex("0b 00 01 00 00 00");
    let mut feed = ModelFeed::new(ZeroFacts).with_preview(Preview::default());
    feed.levels = Some(vec![LevelRow::default(); 4]);
    // The model cell (46, 6) without a prediction.
    let near = feed.near_rooms(&m.w).unwrap().unwrap();
    assert_eq!(near.player_tile, (9, 1));
    // The walk prediction at sub-tile (52, 11): the camera and the
    // player draw use it, so the fade centre does too (§9: the path
    // sub-tile / 5).
    let me = m.w.local_player.unwrap();
    feed.set_local_prediction(Some((me, ((52 << 16) | 0x8000, (11 << 16) | 0x8000))));
    m.w.frames += 1;
    let near = feed.near_rooms(&m.w).unwrap().unwrap();
    assert_eq!(near.player_tile, (10, 2));
}

fn room_mut(
    near: &mut crate::rules::draw_order::NearRooms,
    k: usize,
) -> &mut crate::rules::draw_order::Room {
    &mut near.rooms[k]
}

// Covers: specs/client/model.md §16 r1, §16 r2, §16 r3, §16 r4
#[test]
fn a_freed_room_leaves_a_server_unit_roomless_and_it_sends_0x4b_once() {
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&sight(true, 0, 0)).recv(&sight(true, 16, 0));
    let (a, b, d) = (
        UnitKey::new(MONSTER, 6),
        UnitKey::new(MONSTER, 7),
        UnitKey::new(MONSTER, 8),
    );
    // A in room 2, B in room 0, D created at (0, 0): in no room.
    m.recv(&assign_monster(6, 85, 5));
    m.recv(&assign_monster(7, 5, 5));
    m.recv(&assign_monster(8, 0, 0));
    let flags = |m: &Model, k| (m.unit(k).room_freed, m.unit(k).flag_ex & 0x20 != 0);
    // r4: a room that stays in sight frees nothing (room 2 is still an
    // active room after its own 0x08).
    m.recv(&sight(false, 16, 0));
    assert_eq!(m.w.active_rooms.as_ref().unwrap().len(), 2);
    assert_eq!(flags(&m, a), (false, false));
    // r1, first path: the last 0x08 takes every room to status 4 and the
    // active rooms are freed.
    m.recv(&sight(false, 0, 0));
    assert_eq!(m.w.active_rooms, Some(Vec::new()));
    // r2: each unit still linked in a freed room gets 0x800000 and, as a
    // server unit (no flag 0x400000), flags-2 0x20; D, in no room, nothing.
    assert_eq!(flags(&m, a), (true, true));
    assert_eq!(flags(&m, b), (true, true));
    assert_eq!(flags(&m, d), (false, false));
    // r3: the next update pass skips A's update and queue, sends C→S 0x4B
    // (type 1, its GUID) and clears both bits; D sends nothing (r4).
    m.w.units
        .get_mut(&a)
        .unwrap()
        .queue
        .push(hex("6d 06 00 00 00 1a 12 b9 11 80"));
    m.drain();
    assert_eq!(
        m.w.outgoing,
        [
            hex("4b 01 00 00 00 06 00 00 00"),
            hex("4b 01 00 00 00 07 00 00 00")
        ]
    );
    assert_eq!(flags(&m, a), (false, false));
    assert_eq!(m.unit(a).queue.len(), 1, "the queue was not drained");
    m.drain();
    assert_eq!(m.w.outgoing.len(), 2, "once");
    // r1, second path: the whole client act freed (a new 0x03) frees the
    // rooms still linked the same way.
    m.recv(&sight(true, 0, 0));
    m.recv(&assign_monster(9, 5, 5));
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    assert_eq!(flags(&m, UnitKey::new(MONSTER, 9)), (true, true));
}

// Covers: specs/client/model.md §3 r3, §12 r2; specs/sim/unit-order.md §5 r6
#[test]
fn the_predicted_walk_recaches_the_local_players_room() {
    use crate::world_view::model_feed::ModelFeed;
    use crate::world_view::ViewFeed;
    let mut m = model();
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    m.recv(&sight(true, 8, 0)).recv(&sight(true, 0, 0));
    m.recv(&assign_player(46, 6)).hex("0b 00 01 00 00 00");
    let room_of = |m: &Model| m.w.local_room().map(|r| (r.x0, r.y0, r.w, r.h));
    assert_eq!(room_of(&m), Some(rect(1)));
    // The prediction walked the player west into room 0: its room
    // follows (the model position is the prediction's, not written).
    assert!(m.w.recache_local_room(10, 6));
    assert_eq!(room_of(&m), Some(rect(0)));
    assert_eq!(m.w.local().unwrap().cell(), (46, 6));
    let pl = UnitKey::new(PLAYER, 1);
    assert_eq!(m.w.room_units.room_of(pl), m.w.local_room().map(|r| r.room));
    // The near rooms are room 0's adjacency, with the player listed there.
    let mut rows = vec![LevelRow::default(); 4];
    rows[2].draw_edges = false;
    let mut feed = ModelFeed::new(ZeroFacts).with_map();
    feed.levels = Some(rows);
    let own = m.w.local_room().copied().unwrap();
    let cd = m.w.drlg.clone().unwrap();
    let near = feed.near_rooms(&m.w).unwrap().unwrap();
    assert_eq!(near.rooms.len(), cd.adjacency(own.room).len());
    assert!(near
        .rooms
        .iter()
        .any(|r| r.tiles.x == 0 && r.units.iter().any(|u| u.key == pl)));
    // Same room again, or a point in no active room: unchanged.
    assert!(!m.w.recache_local_room(11, 6));
    assert!(!m.w.recache_local_room(1000, 1000));
    assert_eq!(room_of(&m), Some(rect(0)));
    // Unlinked from every room (a room free): the act lookup finds it.
    m.w.room_units.leave(pl);
    assert_eq!(m.w.local_room(), None);
    assert!(m.w.recache_local_room(46, 6));
    assert_eq!(room_of(&m), Some(rect(1)));
}
