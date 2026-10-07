// Spec: specs/drlg/rooms.md §2–§8 (test vectors, rules)

use super::fakes::*;
use crate::drlg::room::{is_near, near_gaps, sort_near};
use crate::drlg::*;
use crate::rng::Seed;
use crate::units::{ClientId, UnitType};

const INIT: u32 = 644_409_375;

// Covers: specs/drlg/rooms.md §3 r1
#[test]
fn near_gap_vectors() {
    let a = TileRect::new(0, 0, 8, 8);
    let cases = [
        (TileRect::new(8, 0, 8, 8), (0, -8), true),
        (TileRect::new(14, 0, 8, 8), (6, -8), false),
        (TileRect::new(13, 13, 4, 4), (5, 5), true),
    ];
    for (r, gaps, near) in cases {
        assert_eq!(near_gaps(&a, &r), gaps);
        assert_eq!(is_near(&a, &r), near);
    }
    // The room itself has negative gaps.
    assert_eq!(near_gaps(&a, &a), (-8, -8));
}

// Covers: specs/drlg/rooms.md §3 r2
#[test]
fn sort_vectors() {
    let f = |v: &[TileRect]| {
        let mut idx: Vec<usize> = (0..v.len()).collect();
        sort_near(&mut idx, |i| v[i]);
        idx
    };
    // [A=(10,0), B=(0,0)] → [B, A].
    assert_eq!(
        f(&[TileRect::new(10, 0, 8, 8), TileRect::new(0, 0, 8, 8)]),
        [1, 0]
    );
    // [A=(0,10), B=(10,0), C=(0,0)] → [C, A, B].
    assert_eq!(
        f(&[
            TileRect::new(0, 10, 8, 8),
            TileRect::new(10, 0, 8, 8),
            TileRect::new(0, 0, 8, 8)
        ]),
        [2, 0, 1]
    );
    // [A=(0,0), B=(8,8), C=(8,0)] → [A, C, B].
    assert_eq!(
        f(&[
            TileRect::new(0, 0, 8, 8),
            TileRect::new(8, 8, 8, 8),
            TileRect::new(8, 0, 8, 8)
        ]),
        [0, 2, 1]
    );
}

/// A row of six 8×8 preset rooms in level 2: near(i) = {i−1, i, i+1}.
fn row_world() -> (World, Drlg, Vec<DrlgRoomId>) {
    let mut dat = data();
    gen_level(&mut dat, 2, 2);
    dat.levels[2].size = [(48, 8); 3];
    let mut types = FakeTypes::default();
    types
        .rooms
        .insert(2, (0..6).map(|i| preset(8 * i, 0, 8, 8)).collect());
    types.default_grid = Some(floor_grid);
    let mut w = World::new(dat, types);
    let mut d = w.drlg(INIT);
    let l = d.get_or_alloc_level(&w.data, &mut w.types, 2).unwrap();
    d.generate_level(&w.data, &mut w.types, l).unwrap();
    let rooms = d.level_rooms(l);
    (w, d, rooms)
}

fn statuses(d: &Drlg, rooms: &[DrlgRoomId]) -> Vec<u8> {
    rooms.iter().map(|&r| d.room(r).status).collect()
}

// Covers: specs/drlg/rooms.md §4 text, §4 r1, §4 r2, §4 r3, §4.1, §5 r5, §6 r1, §6 r2, §7 r1
#[test]
fn client_enter_and_move_statuses() {
    let (mut w, mut d, r) = row_world();
    let c = ClientId(0);
    let mut svc = w.svc();
    d.client_changes_room(&mut svc, c, None, Some(r[0]))
        .unwrap();
    assert_eq!(statuses(&d, &r), [0, 1, 2, 3, 4, 4]);
    assert_eq!(d.room(r[0]).near(), Some(&[r[0], r[1]][..]));
    // Hand count of §4 Propagate: count[1] once (from r0's own entry),
    // count[2] from r0 and r1's arrays, count[3] four times.
    assert_eq!(d.room(r[0]).counts, [1, 1, 2, 4]);
    // Built: rooms 0 and 1 (status ≤ 1), in depth-first order: the act
    // list is newest first.
    let built: Vec<_> = r
        .iter()
        .filter(|&&x| d.room(x).active().is_some())
        .collect();
    assert_eq!(built.len(), 2);
    let a0 = d.active_room(r[0]).unwrap().id;
    let a1 = d.active_room(r[1]).unwrap().id;
    assert_eq!(w.lists.active_rooms(0), [a1, a0]);
    // Adjacency arrays: near order restricted to active rooms, mirrored
    // into the unit lists.
    assert_eq!(w.lists.room(a0).unwrap().adjacent, [a0, a1]);
    assert_eq!(w.lists.room(a1).unwrap().adjacent, [a0, a1]);
    assert_eq!(d.active_room(r[0]).unwrap().clients, [c]);
    assert_eq!(d.active_room(r[1]).unwrap().clients, [c]);
    // Status lists: tail insertion.
    assert_eq!(d.status_list(1), &[r[1]]);

    let mut svc = w.svc();
    d.client_changes_room(&mut svc, c, Some(r[0]), Some(r[1]))
        .unwrap();
    assert_eq!(statuses(&d, &r), [1, 0, 1, 2, 3, 4]);
    let a2 = d.active_room(r[2]).unwrap().id;
    assert_eq!(w.lists.active_rooms(0), [a2, a1, a0]);
    assert_eq!(w.lists.room(a1).unwrap().adjacent, [a0, a1, a2]);
    // Room 0's array was refilled when room 2 was built? No: room 2 is not
    // in room 0's near list.
    assert_eq!(w.lists.room(a0).unwrap().adjacent, [a0, a1]);
    assert_eq!(d.active_room(r[2]).unwrap().clients, [c]);

    // Leave: everything drops back to 4, active rooms stay.
    let mut svc = w.svc();
    d.client_changes_room(&mut svc, c, Some(r[1]), None)
        .unwrap();
    assert_eq!(statuses(&d, &r), [4; 6]);
    assert!(d.active_room(r[0]).unwrap().clients.is_empty());
    assert_eq!(d.room(r[0]).counts, [0; 4]);
}

// Covers: specs/drlg/rooms.md §4.3, §4.4 r2, §4.4 r5, §5 r4, §8 r2, §8 r3
#[test]
fn build_seeds_and_rebuild_repeat() {
    let (mut w, mut d, r) = row_world();
    let mut svc = w.svc();
    let a = d.stream_room(&mut svc, r[3]).unwrap().unwrap();
    // Streaming changes no status.
    assert_eq!(d.room(r[3]).status, 4);
    // Room seed reset to dwInitSeed, 81 floor choices over 4 tiles of
    // rarity 1 (power of two), then the active-room step.
    let mut s = Seed::init_low(d.room(r[3]).init_seed);
    for _ in 0..81 {
        s.roll(4);
    }
    let act = s.derive();
    assert_eq!(d.room(r[3]).seed, s);
    assert_eq!(d.active_room(r[3]).unwrap().seed, act);
    assert_eq!(d.room(r[3]).tiles().unwrap().floors.len(), 81);
    // Remove and rebuild: same seeds.
    d.remove_active_room(&mut svc, a).unwrap();
    assert!(d.room(r[3]).tiles().is_none());
    assert_eq!(d.room(r[3]).flags & room_flags::HAS_ROOM, 0);
    assert_eq!(d.freed_rooms, 1);
    d.stream_room(&mut svc, r[3]).unwrap();
    assert_eq!(d.room(r[3]).seed, s);
    assert_eq!(d.active_room(r[3]).unwrap().seed, act);
    assert_eq!((d.rooms_built, d.builds_since_update), (2, 2));
}

// Covers: specs/drlg/rooms.md §5 r3, §5 r6, §6 r2, §6 r3, §8 r2
#[test]
fn removal_fixes_neighbours_and_copies_populated() {
    let (mut w, mut d, r) = row_world();
    let mut svc = w.svc();
    for &x in &r[..4] {
        d.stream_room(&mut svc, x).unwrap();
    }
    let ids: Vec<_> = r[..4]
        .iter()
        .map(|&x| d.active_room(x).unwrap().id)
        .collect();
    assert_eq!(d.active_room(r[1]).unwrap().adjacency, [r[0], r[1], r[2]]);
    // §6.3 vector shape: removing an entry puts the last in its place.
    d.remove_active_room(&mut svc, ids[0]).unwrap();
    assert_eq!(d.active_room(r[1]).unwrap().adjacency, [r[2], r[1]]);
    assert_eq!(w.lists.room(ids[1]).unwrap().adjacent, [ids[2], ids[1]]);
    assert!(w.lists.room(ids[0]).is_none(), "record freed");
    assert_eq!(w.lists.active_rooms(0), [ids[3], ids[2], ids[1]]);
    // Populated flag → other flags bit 0.
    w.lists.room_mut(ids[2]).unwrap().populated = true;
    let mut svc = w.svc();
    d.remove_active_room(&mut svc, ids[2]).unwrap();
    assert_eq!(d.room(r[2]).other_flags, 1);
    // Rebuilt populated room starts with active flag bit 0.
    let a = d.stream_room(&mut svc, r[2]).unwrap().unwrap();
    assert_eq!(d.active_room(r[2]).unwrap().flags, 1);
    // A refill (the new activation) restores near order for neighbours.
    assert_eq!(d.active_room(r[1]).unwrap().adjacency, [r[1], r[2]]);
    assert!(w.lists.room(a).unwrap().populated);
}

// Covers: specs/drlg/rooms.md §6 r3
#[test]
fn adjacency_removal_vector() {
    // [R, X, Y, Z]; remove X → [R, Z, Y]. Four rooms near R in that near
    // order: R=(0,0), X=(8,0), Y=(0,8), Z=(8,8)? Built from the sort.
    let mut dat = data();
    gen_level(&mut dat, 2, 2);
    let mut types = FakeTypes::default();
    types.rooms.insert(
        2,
        vec![
            preset(0, 0, 8, 8),
            preset(0, 8, 8, 8),
            preset(8, 0, 8, 8),
            preset(8, 8, 8, 8),
        ],
    );
    types.default_grid = Some(floor_grid);
    let mut w = World::new(dat, types);
    let mut d = w.drlg(INIT);
    let l = d.get_or_alloc_level(&w.data, &mut w.types, 2).unwrap();
    d.generate_level(&w.data, &mut w.types, l).unwrap();
    let rooms = d.level_rooms(l);
    let mut svc = w.svc();
    for &x in &rooms {
        d.stream_room(&mut svc, x).unwrap();
    }
    let arr = d.active_room(rooms[0]).unwrap().adjacency.clone();
    assert_eq!(arr.len(), 4);
    assert_eq!(arr[0], rooms[0]);
    let x = arr[1];
    let xa = d.active_room(x).unwrap().id;
    d.remove_active_room(&mut svc, xa).unwrap();
    assert_eq!(
        d.active_room(rooms[0]).unwrap().adjacency,
        [arr[0], arr[3], arr[2]]
    );
}

// Covers: specs/drlg/rooms.md §7 r2, §7 r3, §8 r1
#[test]
fn inactivity_counter_and_removal_test() {
    let (mut w, mut d, r) = row_world();
    let c = ClientId(3);
    let mut svc = w.svc();
    let a5 = d.stream_room(&mut svc, r[5]).unwrap().unwrap();
    // Counter 0, no client: removed on pass 11 (counter > 10).
    let mut removed_at = None;
    for pass in 1..=12 {
        let n = d.room_inactivity(a5).unwrap();
        if n > active::REMOVAL_THRESHOLD && d.allows_removal(a5).unwrap() {
            removed_at = Some(pass);
            break;
        }
    }
    assert_eq!(removed_at, Some(11));
    // With a client the counter resets.
    d.add_room_client(r[5], c);
    assert_eq!(d.room_inactivity(a5), Ok(0));
    d.remove_room_client(r[5], c);
    assert_eq!(d.room_inactivity(a5), Ok(1));
    // Status ≤ 1 blocks removal; portal blocks removal.
    d.client_changes_room(&mut svc, c, None, Some(r[4]))
        .unwrap();
    assert_eq!(d.allows_removal(a5), Ok(false));
    d.client_changes_room(&mut svc, c, Some(r[4]), None)
        .unwrap();
    assert_eq!(d.allows_removal(a5), Ok(true));
    d.room_mut(r[5]).flags |= room_flags::PORTAL;
    assert_eq!(d.allows_removal(a5), Ok(false));
}

// Covers: specs/drlg/rooms.md §8 r1
#[test]
fn town_rooms_stay_while_any_room_is_seen() {
    let mut dat = data();
    gen_level(&mut dat, 1, 2);
    let mut types = FakeTypes::default();
    types
        .rooms
        .insert(1, (0..6).map(|i| preset(8 * i, 0, 8, 8)).collect());
    types.default_grid = Some(floor_grid);
    let mut w = World::new(dat, types);
    let mut d = Drlg::create(0, INIT, 0, 1, false, &w.data, &mut w.types).unwrap();
    let l = d.find_level(1).unwrap();
    let r = d.level_rooms(l);
    let mut svc = w.svc();
    let a5 = d.stream_room(&mut svc, r[5]).unwrap().unwrap();
    d.client_changes_room(&mut svc, ClientId(0), None, Some(r[0]))
        .unwrap();
    assert_eq!(d.room(r[5]).status, 4);
    assert_eq!(d.allows_removal(a5), Ok(false));
}

// Covers: specs/drlg/rooms.md §8 r1
#[test]
fn client_copy_removal_is_fatal() {
    let mut w = World::new(data(), FakeTypes::default());
    let d = Drlg::create(0, INIT, 0, 0, true, &w.data, &mut w.types).unwrap();
    assert_eq!(
        d.allows_removal(crate::units::RoomId(0)),
        Err(DrlgError::ClientCopyRemoval)
    );
}

// Covers: specs/drlg/rooms.md §7 r1
#[test]
fn client_arrays_sorted() {
    let (mut w, mut d, r) = row_world();
    let mut svc = w.svc();
    d.stream_room(&mut svc, r[0]).unwrap();
    for c in [5, 2, 9] {
        d.add_room_client(r[0], ClientId(c));
    }
    assert_eq!(
        d.active_room(r[0]).unwrap().clients,
        [ClientId(2), ClientId(5), ClientId(9)]
    );
    d.remove_room_client(r[0], ClientId(2));
    assert_eq!(
        d.active_room(r[0]).unwrap().clients,
        [ClientId(5), ClientId(9)]
    );
}

fn warp_world(warp: i32) -> (World, Drlg, DrlgRoomId, LevelIdx) {
    let mut dat = data();
    gen_level(&mut dat, 2, 2);
    gen_level(&mut dat, 3, 2);
    dat.levels[2].vis = [0, 3, 0, 0, 0, 0, 0, 0];
    dat.levels[2].warp = [-1, warp, -1, -1, -1, -1, -1, -1];
    dat.levels[3].vis = [2, 0, 0, 0, 0, 0, 0, 0];
    dat.warps = vec![
        WarpDef {
            id: 4,
            direction: b'l',
            ..WarpDef::default()
        },
        WarpDef {
            id: 5,
            direction: b'r',
            ..WarpDef::default()
        },
    ];
    let mut types = FakeTypes::default();
    let mut a = preset(0, 0, 8, 8);
    a.flags = room_flags::WARP_0 << 1;
    types.rooms.insert(2, vec![a, preset(8, 0, 8, 8)]);
    let mut t1 = preset(4, 30, 8, 8);
    t1.flags = room_flags::WARP_0;
    let mut t2 = preset(4, 12, 8, 8);
    t2.flags = room_flags::WARP_0;
    types.rooms.insert(3, vec![preset(50, 50, 8, 8), t1, t2]);
    let mut w = World::new(dat, types);
    let mut d = w.drlg(INIT);
    let l = d.get_or_alloc_level(&w.data, &mut w.types, 2).unwrap();
    d.generate_level(&w.data, &mut w.types, l).unwrap();
    let room = d.level_rooms(l)[0];
    d.build_near(&w.data, &mut w.types, room).unwrap();
    let l3 = d.find_level(3).unwrap();
    (w, d, room, l3)
}

// Covers: specs/drlg/rooms.md §3 r3; specs/drlg/levels.md §5 r5
#[test]
fn warp_link_with_warp_id() {
    let (w, d, room, l3) = warp_world(5);
    // Level 3 generated inside the near build.
    assert_eq!(w.types.generated, [2, 3]);
    let t1 = d.level_rooms(l3)[1];
    let near = d.room(room).near().unwrap();
    assert_eq!(near.len(), 3);
    assert!(near.contains(&t1));
    assert_eq!(
        d.room(room).warp_links,
        [WarpLink {
            target: t1,
            enabled: true,
            lvlwarp_row: 1
        }]
    );
}

// Covers: specs/drlg/rooms.md §3 r3
#[test]
fn warp_link_without_warp_id_uses_gap_rule() {
    let (_w, d, room, l3) = warp_world(-1);
    let t2 = d.level_rooms(l3)[2];
    let near = d.room(room).near().unwrap();
    // t1 at y 30 is too far; t2 at y 12 has gaps (−4, 4).
    assert_eq!(near.len(), 3);
    assert!(near.contains(&t2));
    assert!(d.room(room).warp_links.is_empty());
}

// Covers: specs/drlg/rooms.md §3 r4
#[test]
fn town_border_flag() {
    let mut dat = data();
    gen_level(&mut dat, 1, 2);
    gen_level(&mut dat, 2, 2);
    dat.levels[2].vis = [1, 0, 0, 0, 0, 0, 0, 0];
    dat.levels[2].warp = [7, -1, -1, -1, -1, -1, -1, -1];
    dat.levels[1].vis = [2, 0, 0, 0, 0, 0, 0, 0];
    dat.warps = vec![WarpDef {
        id: 7,
        direction: b'b',
        ..WarpDef::default()
    }];
    let mut types = FakeTypes::default();
    let mut town = preset(0, 0, 8, 8);
    town.flags = room_flags::WARP_0;
    types.rooms.insert(1, vec![town]);
    let mut out = preset(0, 8, 8, 8);
    out.flags = room_flags::WARP_0;
    types.rooms.insert(2, vec![out, preset(8, 8, 8, 8)]);
    let mut w = World::new(dat, types);
    let mut d = w.drlg(INIT);
    let l = d.get_or_alloc_level(&w.data, &mut w.types, 2).unwrap();
    d.generate_level(&w.data, &mut w.types, l).unwrap();
    let r = d.level_rooms(l);
    d.build_near(&w.data, &mut w.types, r[0]).unwrap();
    d.build_near(&w.data, &mut w.types, r[1]).unwrap();
    assert_ne!(d.room(r[0]).flags & room_flags::NO_POPULATION, 0);
    assert_eq!(d.room(r[1]).flags & room_flags::NO_POPULATION, 0);
    // The town room itself never gets it.
    let t = d.level_rooms(d.find_level(1).unwrap())[0];
    d.build_near(&w.data, &mut w.types, t).unwrap();
    assert_eq!(d.room(t).flags & room_flags::NO_POPULATION, 0);
}

// Covers: specs/drlg/rooms.md §4 text
#[test]
fn preset_units_added_once_by_handler_3() {
    let (mut w, mut d, r) = row_world();
    let mut svc = w.svc();
    d.client_changes_room(&mut svc, ClientId(0), None, Some(r[0]))
        .unwrap();
    d.client_changes_room(&mut svc, ClientId(0), Some(r[0]), Some(r[1]))
        .unwrap();
    // Rooms 0..4 reached status ≤ 3 (rooms 0..3 on entry, 4 on the move).
    let mut sorted = w.types.preset_units_added.clone();
    sorted.sort();
    assert_eq!(sorted, [r[0], r[1], r[2], r[3], r[4]]);
}

/// PW1: a room removed while units are still in it (tick step 9 normally
/// compresses them first, §8.2). The units leave the freed record (§5.3,
/// list order), so a later room in the reused slot does not hold them.
/// TODO(rooms.md §8.2): the flag 0x800000 / flag-ex 0x20 and path update
/// of `0x0061A840` (unit specs; handoff `prop-fixes` Q3).
// Covers: specs/drlg/rooms.md §8 r2
#[test]
fn removal_of_a_room_with_units_unlinks_them() {
    let (mut w, mut d, r) = row_world();
    let mut svc = w.svc();
    let a = d.stream_room(&mut svc, r[0]).unwrap().unwrap();
    let g = w.lists.guids.alloc(UnitType::Monster);
    let u = w
        .lists
        .add_unit(UnitType::Monster, g, Some(a), true)
        .unwrap();
    let g = w.lists.guids.alloc(UnitType::Player);
    let v = w
        .lists
        .add_unit(UnitType::Player, g, Some(a), false)
        .unwrap();
    let mut svc = w.svc();
    d.remove_active_room(&mut svc, a).unwrap();
    assert!(w.lists.room(a).is_none(), "record freed");
    for x in [u, v] {
        assert_eq!(w.lists.unit(x).unwrap().room(), None);
    }
    let mut svc = w.svc();
    let b = d.stream_room(&mut svc, r[0]).unwrap().unwrap();
    assert_eq!(w.lists.room_unit_first(b), None);
}

// Covers: specs/drlg/rooms.md §4.2; specs/client/model.md §9 r1, §9 r2
#[test]
fn client_in_sight_by_coordinates() {
    let mut dat = data();
    gen_level(&mut dat, 2, 2);
    dat.levels[2].size = [(48, 8); 3];
    let mut types = FakeTypes::default();
    types
        .rooms
        .insert(2, (0..6).map(|i| preset(8 * i, 0, 8, 8)).collect());
    types.default_grid = Some(floor_grid);
    let mut w = World::new(dat, types);
    // A client copy (`levels.md` §2 r3): town id 0, client flag.
    let mut d = Drlg::create(0, INIT, 0, 0, true, &w.data, &mut w.types).unwrap();
    let mut svc = w.svc();
    // The level is allocated and generated by the lookup.
    let hit = d.set_in_sight_at(&mut svc, 2, 8, 0, None).unwrap();
    let l = d.find_level(2).unwrap();
    let r = d.level_rooms(l);
    assert_eq!(hit, Some(r[1]));
    assert_eq!(statuses(&d, &r), [2, 1, 2, 3, 4, 4]);
    assert_eq!(d.room(r[1]).counts[1], 1);
    // Only the room in sight is built; a client copy room gets the
    // automap-reveal flag, so its active room has flag 4 (§5 r3).
    let a1 = d.active_room(r[1]).unwrap();
    assert_eq!(a1.flags, 4);
    assert!(r
        .iter()
        .filter(|&&x| x != r[1])
        .all(|&x| d.active_room(x).is_none()));
    // A second 0x07 for the same room: its count is not 0, nothing.
    let mut svc = w.svc();
    d.set_in_sight_at(&mut svc, 2, 12, 4, None).unwrap();
    assert_eq!(d.room(r[1]).counts[1], 1);
    // No room at the point (outside every room of the level).
    let mut svc = w.svc();
    assert_eq!(
        d.set_in_sight_at(&mut svc, 2, 100, 100, None).unwrap(),
        None
    );
    // 0x08: back to status 4; on a client copy the status-3 unset frees
    // the tiles and removes the active room (§4 unset handler 3).
    let mut svc = w.svc();
    assert_eq!(
        d.unset_in_sight_at(&mut svc, 2, 8, 0, None).unwrap(),
        Some(r[1])
    );
    assert_eq!(statuses(&d, &r), [4; 6]);
    assert_eq!(d.room(r[1]).counts, [0; 4]);
    assert!(d.active_room(r[1]).is_none());
    assert!(w.lists.active_rooms(0).is_empty());
    // Unset again: the count is 0, nothing changes.
    let mut svc = w.svc();
    d.unset_in_sight_at(&mut svc, 2, 8, 0, None).unwrap();
    assert_eq!(d.room(r[1]).counts, [0; 4]);
}
