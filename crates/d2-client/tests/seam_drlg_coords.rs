// Spec: specs/seams/drlg-coords.md
//! Contract checks of the DRLG coordinate seam between the server's act
//! DRLG (`d2-sim`, owned by the game) and the client's copy built from
//! S→C 0x03 and the 0x07 / 0x08 room messages
//! (`d2_client::bridge::drlg::ClientDrlg`). Both sides are the app's
//! own single-player game on the synthetic data, driven through the real
//! bridge and server thread (no Bevy): after every frame each room the
//! client holds active is compared with the server's room at the same
//! point, field by field, and the local player's room is compared with
//! the server's.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::single_player::{self};
use d2_client::bridge::world::ClientTables;
use d2_client::bridge::Bridge;
use d2_server::seams::Clock;
use d2_sim::drlg::TileRect;

mod app_support;

/// The host clock, advanced by the test.
struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// One DRLG room as either side holds it: level id, tile rectangle,
/// creation seed, active sub-tile rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RoomFacts {
    level: u32,
    tiles: TileRect,
    init_seed: u32,
    subtiles: Option<TileRect>,
}

type Link = single_player::Link<StepClock>;
type Thread = d2_client::app::server_thread::ThreadLink<Link>;

/// The server's room of level `level` at tile (x, y), in the act of the
/// player's room.
fn server_rooms(
    bridge: &mut Bridge<Thread>,
    points: Vec<(u32, i32, i32)>,
) -> Vec<Option<RoomFacts>> {
    bridge
        .link_mut()
        .with(move |l: &mut Link| {
            let sim = &mut l.host_mut().game;
            let hooks = sim.events.action.hooks();
            let Some(d) = hooks.drlg.dungeon.acts.first().and_then(Option::as_ref) else {
                return vec![None; points.len()];
            };
            points
                .iter()
                .map(|&(level, x, y)| {
                    let idx = d.find_level(level)?;
                    d.level_rooms(idx)
                        .into_iter()
                        .find(|&r| d.room(r).rect.contains(x, y))
                        .map(|r| {
                            let room = d.room(r);
                            RoomFacts {
                                level,
                                tiles: room.rect,
                                init_seed: room.init_seed,
                                subtiles: d.active_room(r).map(|a| a.subtiles),
                            }
                        })
                })
                .collect()
        })
        .unwrap()
}

/// The server player's sub-tile and its room's facts.
fn server_player(bridge: &mut Bridge<Thread>) -> Option<((i32, i32), RoomFacts)> {
    bridge
        .link_mut()
        .with(|l: &mut Link| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim)?;
            let room = sim.game.lists.unit(p)?.room()?;
            let pos = sim.events.action.hooks().path_position(p);
            let hooks = sim.events.action.hooks();
            let (d, r) = hooks.drlg.drlg_room(&sim.game, room)?;
            let dr = d.room(r);
            Some((
                pos,
                RoomFacts {
                    level: d.level(dr.level).id,
                    tiles: dr.rect,
                    init_seed: dr.init_seed,
                    subtiles: d.active_room(r).map(|a| a.subtiles),
                },
            ))
        })
        .unwrap()
}

/// Every active room of the client DRLG, in the client's list order.
fn client_rooms(bridge: &Bridge<Thread>) -> Vec<RoomFacts> {
    let w = bridge.world();
    let Some(cd) = w.drlg.as_ref() else {
        return Vec::new();
    };
    w.active_rooms
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|a| {
            let room = cd.drlg.room(a.room);
            RoomFacts {
                level: u32::from(a.level),
                tiles: room.rect,
                init_seed: room.init_seed,
                subtiles: Some(TileRect::new(a.x0, a.y0, a.w, a.h)),
            }
        })
        .collect()
}

/// §2.2–§2.4: each client room equals the server's room at its origin
/// (same level, tile rectangle, creation seed and sub-tile rectangle =
/// tiles × 5); §2.5: the local player's client room is the server's.
fn check_rooms(bridge: &mut Bridge<Thread>, when: &str) {
    let client = client_rooms(bridge);
    let points = client
        .iter()
        .map(|c| (c.level, c.tiles.x, c.tiles.y))
        .collect();
    let server = server_rooms(bridge, points);
    for (c, s) in client.iter().zip(&server) {
        let s = s.unwrap_or_else(|| panic!("{when}: client room {c:?} has no server room"));
        assert_eq!(
            (c.level, c.tiles, c.init_seed),
            (s.level, s.tiles, s.init_seed),
            "{when}: client and server DRLG rooms differ"
        );
        let t = c.tiles;
        assert_eq!(
            c.subtiles,
            Some(TileRect::new(t.x * 5, t.y * 5, t.w * 5, t.h * 5)),
            "{when}: client active room sub-tiles are not the tiles × 5"
        );
        if let Some(sub) = s.subtiles {
            assert_eq!(
                Some(sub),
                c.subtiles,
                "{when}: active sub-tile rectangles differ"
            );
        }
    }
    let Some((pos, s)) = server_player(bridge) else {
        return;
    };
    let w = bridge.world();
    let own = w.local_room().copied().unwrap_or_else(|| {
        panic!("{when}: server player at {pos:?} in {s:?}, client player in no room")
    });
    // The local player's own walk is the client's to step (`model.md` §3
    // rule 3; the play mode's prediction): between server positions the
    // client's point lags the server's, so the rooms are compared where
    // both sides hold the same point.
    let me = w.local_player.expect("local player");
    let cpos = w.units[&me].position.expect("placed");
    if (i32::from(cpos.0), i32::from(cpos.1)) != pos {
        assert!(
            own.contains(cpos.0.into(), cpos.1.into()),
            "{when}: client point {cpos:?} outside its room {own:?}"
        );
        return;
    }
    let client_room = w.drlg.as_ref().unwrap().drlg.room(own.room);
    assert_eq!(
        (u32::from(own.level), client_room.rect),
        (s.level, s.tiles),
        "{when}: the client's player room is not the server's (player at {pos:?})"
    );
    assert!(
        own.contains(pos.0, pos.1),
        "{when}: {pos:?} outside {own:?}"
    );
}

fn start() -> (Bridge<Thread>, Arc<AtomicU32>) {
    let data = app_support::game_data();
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, _) = single_player::start(
        data.clone(),
        single_player::DEFAULT_SEED,
        StepClock(ms.clone()),
    )
    .unwrap();
    let mut bridge = Bridge::new(link).unwrap();
    bridge.set_drlg_source(Some(single_player::client_drlg_source(&data)));
    bridge.set_tables(ClientTables {
        levels: single_player::client_level_rows(&data),
        ..ClientTables::default()
    });
    // The install's client tables: the join selects the real skills.
    app_support::live_bridge_tables(&mut bridge);
    bridge.send(&single_player::create_request()).unwrap();
    bridge.frame().unwrap();
    for _ in 0..3 {
        ms.fetch_add(40, Ordering::SeqCst);
        bridge.frame().unwrap();
    }
    (bridge, ms)
}

// Covers: specs/seams/drlg-coords.md §2.1, §2.2, §2.3, §2.4, §2.5
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn client_rooms_equal_the_servers_after_the_join() {
    let (mut bridge, _) = start();
    assert!(
        bridge.log().rejected.is_empty(),
        "{:?}",
        bridge.log().rejected
    );
    let w = bridge.world();
    let act = w.act.expect("0x03");
    let cd = w.drlg.as_ref().expect("client DRLG");
    // §2.1: the client act is built from 0x03's act and seed.
    assert_eq!((cd.drlg.act, cd.drlg.init_seed), (act.act, act.init_seed));
    let server_seed = bridge
        .link_mut()
        .with(|l: &mut Link| {
            let sim = &mut l.host_mut().game;
            let hooks = sim.events.action.hooks();
            hooks.drlg.dungeon.acts[0].as_ref().map(|d| d.init_seed)
        })
        .unwrap();
    assert_eq!(Some(act.init_seed), server_seed);
    assert!(!client_rooms(&bridge).is_empty());
    check_rooms(&mut bridge, "after the join");
}

// Covers: specs/seams/drlg-coords.md §2.2, §2.3, §2.5, §2.6
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn client_rooms_follow_the_server_across_the_level_border() {
    let (mut bridge, ms) = start();
    let start_pos = server_player(&mut bridge).expect("placed").0;
    // The synthetic town room spans sub-tiles x 80..120, the Blood Moor
    // room 120..160 (`app_level_border.rs`). Stage the charstats
    // velocities and velocity percent of a 1.14d install, as there.
    bridge
        .link_mut()
        .with(|l: &mut Link| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            let hooks = g.events.action.hooks();
            let mut t = (*hooks.tables).clone();
            use d2_data::tables::{Charstats, Record};
            let mut row = Charstats::decode(&[0u8; Charstats::SIZE]);
            row.walkvelocity = 6;
            row.runvelocity = 9;
            t.combat.charstats = vec![row; 7];
            hooks.tables = Arc::new(t);
            let game = &mut g.game;
            g.events.action.with(game, |_, v| v.set_base(p, 67, 100));
        })
        .unwrap();
    let target = (140u16, start_pos.1 as u16);
    bridge
        .send(&d2_proto::client::Walk {
            x: target.0,
            y: target.1,
        })
        .unwrap();
    let mut levels = Vec::new();
    for i in 0..400 {
        ms.fetch_add(40, Ordering::SeqCst);
        bridge.frame().unwrap();
        check_rooms(&mut bridge, &format!("frame {i}"));
        let (pos, room) = server_player(&mut bridge).expect("placed");
        if levels.last() != Some(&room.level) {
            levels.push(room.level);
        }
        if pos.0 == i32::from(target.0) {
            break;
        }
    }
    assert_eq!(
        levels,
        vec![single_player::ACT1_TOWN, single_player::BLOOD_MOOR]
    );
    // After the walk: every client room still has its server room
    // (§2.2–§2.4). The client's own point is the client's to step (the
    // play mode's prediction, not run here), so it is not compared.
    for i in 0..10 {
        ms.fetch_add(40, Ordering::SeqCst);
        bridge.frame().unwrap();
        check_rooms(&mut bridge, &format!("after the walk {i}"));
    }
    assert!(
        bridge.log().rejected.is_empty(),
        "{:?}",
        bridge.log().rejected
    );
}
