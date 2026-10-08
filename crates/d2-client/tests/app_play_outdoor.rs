// Spec: specs/sim/intents-events.md (§2.4 rules 3–4), specs/sim/pathing.md (§1, §10), specs/sim/tick.md (§6), specs/client/model.md (§9, §12), specs/render/draw-order.md (§9); preview: docs/PLAN.md decisions D1–D2
//! Walking out of the Rogue Encampment on the install, as `play --new`
//! runs it (the app's single-player game, the client DRLG from S→C 0x03
//! and the rooms 0x07 brings in sight):
//!
//! - C→S 0x03 (run to a point) moves the server's player: the point
//!   parser's range test reads the live path position (the app's game
//!   never staged one, so every walk was refused);
//! - the server player reaches the Blood Moor (level 2) and the room
//!   switch brings its rooms into the client DRLG;
//! - the preview's near rooms follow the predicted position (the model's
//!   player is never moved by the server while it walks, `pathing.md`
//!   §10 r2, and its town room is freed by 0x08): Blood Moor rooms with
//!   floor records, the local player filed in one of them, and every DT1
//!   tile they name readable from the archives.
//!
//! `D2_GAME_DIR=<install> cargo test -p d2-client --test app_play_outdoor -- --ignored`

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::play::unspecified_palette;
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData, Link};
use d2_client::bridge::world::ClientTables;
use d2_client::bridge::Bridge;
use d2_client::rules::draw_order::TileArray;
use d2_client::world_view::near_rooms::{Dt1Entry, MapState};
use d2_client::world_view::tile_assets::TileAssets;
use d2_client::world_view::{preview, ViewAssets};
use d2_server::seams::Clock;

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

type B = Bridge<ThreadLink<Link<StepClock>>>;

const BLOOD_MOOR: u32 = 2;

/// Server facts: the player's path position and its room's level id.
fn server(bridge: &mut B) -> ((i32, i32), Option<u32>) {
    bridge
        .link_mut()
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let (player, _) = single_player::local_player(sim).expect("joined");
            let room = sim.game.lists.unit(player).and_then(|u| u.room());
            let h = sim.events.action.hooks();
            let pos = h.path_position(player);
            let lvl = room.and_then(|r| h.drlg.level_id(&sim.game, r));
            (pos, lvl)
        })
        .unwrap()
}

/// The next click (≤ 15 sub-tiles along a walkable 8-way path over the
/// server's built act-1 collision grid, player mask 0x1C09 of
/// `sim/pathing.md`) toward a sub-tile of the Blood Moor at least `depth`
/// tiles past its west edge; with none reachable yet, toward the reached
/// cell nearest the level's centre (a user clicking toward the exit).
fn next_click(bridge: &mut B, from: (i32, i32), depth: i32) -> Option<(i32, i32)> {
    bridge
        .link_mut()
        .with(move |l| {
            let sim = &mut l.host_mut().game;
            let d = sim.events.action.hooks().drlg.dungeon.acts[0].as_ref()?;
            let rect = d.level(d.find_level(BLOOD_MOOR)?).rect;
            let goal = |(x, y): (i32, i32)| {
                let (tx, ty) = (x.div_euclid(5), y.div_euclid(5));
                tx >= rect.x + depth && tx < rect.x + rect.w && ty >= rect.y && ty < rect.y + rect.h
            };
            let free = |p: (i32, i32)| d.collision_at(p.0, p.1).is_some_and(|c| c & 0x1C09 == 0);
            let centre = ((rect.x + rect.w / 2) * 5, (rect.y + rect.h / 2) * 5);
            let dist = |p: (i32, i32)| (p.0 - centre.0).abs().max((p.1 - centre.1).abs());
            let mut prev = BTreeMap::from([(from, from)]);
            let mut q = VecDeque::from([from]);
            let mut best = from;
            let click = |to: (i32, i32), prev: &BTreeMap<_, _>| {
                let mut path = vec![to];
                let mut c = to;
                while c != from {
                    c = prev[&c];
                    path.push(c);
                }
                path.reverse();
                path[path.len().min(15) - 1]
            };
            while let Some(p) = q.pop_front() {
                if goal(p) {
                    return Some(click(p, &prev));
                }
                if dist(p) < dist(best) {
                    best = p;
                }
                if (p.0 - from.0).abs() > 300 || (p.1 - from.1).abs() > 300 {
                    continue;
                }
                for (dx, dy) in [
                    (1, 0),
                    (-1, 0),
                    (0, 1),
                    (0, -1),
                    (1, 1),
                    (1, -1),
                    (-1, 1),
                    (-1, -1),
                ] {
                    let n = (p.0 + dx, p.1 + dy);
                    if !prev.contains_key(&n) && free(n) {
                        prev.insert(n, p);
                        q.push_back(n);
                    }
                }
            }
            (best != from).then(|| click(best, &prev))
        })
        .unwrap()
}

fn frames(bridge: &mut B, ms: &AtomicU32, n: usize) {
    for _ in 0..n {
        ms.fetch_add(40, Ordering::SeqCst);
        bridge.frame().unwrap();
    }
}

/// Clicks toward the Blood Moor (C→S 0x03 run to a point) until the
/// server's player stands `depth` tiles inside it; false when it gets
/// stuck.
fn run_into_moor(bridge: &mut B, ms: &AtomicU32, depth: i32) -> bool {
    for _ in 0..80 {
        let (pos, lvl) = server(bridge);
        let Some((x, y)) = next_click(bridge, pos, depth) else {
            return lvl == Some(BLOOD_MOOR);
        };
        if (x, y) == pos {
            return true;
        }
        let mut m = vec![0x03];
        m.extend_from_slice(&(x as u16).to_le_bytes());
        m.extend_from_slice(&(y as u16).to_le_bytes());
        bridge.send_bytes(&m).unwrap();
        for _ in 0..40 {
            frames(bridge, ms, 1);
            if server(bridge).0 == (x, y) {
                break;
            }
        }
        if server(bridge).0 == pos {
            println!("stuck at {pos:?} clicking {:?}", (x, y));
            return false;
        }
    }
    false
}

// Covers: specs/sim/intents-events.md §2.4 r3; specs/sim/tick.md §6 r5; specs/client/model.md §9 r1, §12 r2; specs/render/draw-order.md §9
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn walking_out_of_town_brings_the_blood_moor_in() {
    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
    let data = GameData::select(Some(std::path::Path::new(&dir)), false).unwrap();
    let GameData::Live(live) = &data else {
        panic!("live data expected");
    };
    let archives = live.archives.clone();
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, _) = single_player::start(
        data.clone(),
        single_player::DEFAULT_SEED,
        StepClock(ms.clone()),
    )
    .unwrap();
    let mut bridge = Bridge::new(link).unwrap();
    let levels = single_player::client_level_rows(&data);
    bridge.set_drlg_source(Some(single_player::client_drlg_source(&data)));
    bridge.set_tables(ClientTables {
        levels: levels.clone(),
        ..ClientTables::default()
    });
    bridge.send(&single_player::create_request()).unwrap();
    frames(&mut bridge, &ms, 4);
    let (start, lvl) = server(&mut bridge);
    assert_eq!(lvl, Some(1), "the player starts in the Rogue Encampment");

    // 1. The walk reaches the server and leaves the town.
    assert!(run_into_moor(&mut bridge, &ms, 6), "reach the Blood Moor");
    frames(&mut bridge, &ms, 10);
    let (pos, lvl) = server(&mut bridge);
    assert_ne!(pos, start, "C→S 0x03 moved the server's player");
    assert_eq!(lvl, Some(BLOOD_MOOR));

    // The server's units in the act's active rooms, by type (informative:
    // the Blood Moor's population).
    let by_type = bridge
        .link_mut()
        .with(|l| {
            let g = &l.host().game.game;
            let mut n = BTreeMap::new();
            let mut room = g.lists.room_first(0);
            while let Some(r) = room {
                for u in g.lists.room_units(r) {
                    if let Some(e) = g.lists.unit(u) {
                        *n.entry(format!("{:?}", e.ty)).or_insert(0) += 1;
                    }
                }
                room = g.lists.room_next(r);
            }
            n
        })
        .unwrap();
    println!("server units in act 1 rooms: {by_type:?}");

    // 2. The room switch's 0x07s built Blood Moor rooms in the client DRLG.
    let w = bridge.world();
    println!(
        "server {pos:?}; client model pos {:?}, model room {:?}, {} active rooms, {} units, {} rooms in sight; rejected {:?}",
        w.local().and_then(|u| u.position),
        w.local_room().map(|r| r.level),
        w.active_rooms.as_ref().map_or(0, |r| r.len()),
        w.units.len(),
        w.rooms_in_sight.len(),
        bridge.log().rejected
    );
    let moor_rooms = w
        .active_rooms
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|r| u32::from(r.level) == BLOOD_MOOR)
        .count();
    assert!(moor_rooms > 0, "Blood Moor rooms in the client DRLG");

    // 3. The preview's near rooms at the predicted position: the position
    // the server walked to.
    let me = w.local_player.expect("local player");
    let at = (
        ((pos.0 as u32) << 16) | 0x8000,
        ((pos.1 as u32) << 16) | 0x8000,
    );
    let mut map = MapState::default();
    map.local_at = Some((me, at));
    let near = map
        .near_rooms(w, Some(&levels), |u| Ok(preview::unit_facts(w, u)))
        .unwrap()
        .expect("near rooms at the predicted position");
    assert_eq!(near.level.id, BLOOD_MOOR);
    assert!(near
        .rooms
        .iter()
        .any(|r| r.level == BLOOD_MOOR && !r.floors.is_empty()));
    assert!(
        near.rooms
            .iter()
            .any(|r| r.units.iter().any(|u| u.key == me)),
        "the local player is filed in a near room"
    );
    let counts: Vec<[usize; 3]> = near
        .rooms
        .iter()
        .map(|r| [r.walls.len(), r.floors.len(), r.shadows.len()])
        .collect();
    let mut entries: Vec<Dt1Entry> = Vec::new();
    for (i, c) in counts.iter().enumerate() {
        for (a, array) in [TileArray::Wall, TileArray::Floor, TileArray::Shadow]
            .into_iter()
            .enumerate()
        {
            for rec in 0..c[a] {
                entries.extend(map.entry(i, array, rec).cloned());
            }
        }
    }
    assert!(!entries.is_empty());

    // 4. Every DT1 tile the near rooms name is readable from the archives.
    let mut tiles = TileAssets::new(Some(archives), None);
    let mut assets = ViewAssets::new(unspecified_palette());
    let failed = tiles.ensure(entries.iter(), &mut assets);
    println!(
        "{} entries, {} failed: {failed:?}",
        entries.len(),
        failed.len()
    );
    assert!(failed.is_empty(), "{failed:?}");
}
