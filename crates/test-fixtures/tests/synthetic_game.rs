// Spec: specs/drlg/levels.md §3 (act creation), specs/drlg/preset.md §5–§6, specs/drlg/rooms.md §4.1, §9.3–§9.5, specs/sim/units.md §3, specs/combat/vitals.md §1, specs/sim/tick.md §3, specs/sim/intents-events.md §1 (a game on synthetic data), specs/sim/path-placement.md §11, §13 (the session join)
//! A game from the synthetic install, end to end and in CI: the
//! archives (tables, strings, AnimData and the DRLG's DS1 / DT1 files,
//! [`test_fixtures::drlg`]) → the loaded and fixed-up set →
//! [`GameData`] (table views, `world_data` providers) → a `WorldSim`
//! with act 0 created → the one-room town generated and streamed, its
//! waypoint and a player allocated → `SimGame::join` → `Host::frame`s.
//!
//! Expected values are the fixture's own content (the town's size, the
//! waypoint record) and spec facts (creation life and mana,
//! `vitals.md` §1); everything else is an invariant (no wiring error, no
//! frame fault, the player stays in its room, two runs are identical).

use std::path::PathBuf;
use std::sync::OnceLock;

use d2_data::tables::Objects;
use d2_server::adapters::handlers::world::ActionWorld;
use d2_server::adapters::session::{enter_game, Entry, JoinError};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use d2_server::host::Host;
use d2_server::seams::{ClientId, Clock, MessageSink, PlayerGate, Pos, SessionHandler};
use d2_sim::combat::vitals::init_player_stats;
use d2_sim::game::Game;
use d2_sim::stats::lists::NoHost;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::interaction::{VitalsRest, VitalsView};
use test_fixtures::content::{FLOOR_DT1, PRESET_DS1, PRESET_SIZE, WALL_DT1};
use test_fixtures::drlg::{archive_name, FIXED_LIBRARY, TOWN_WAYPOINT};
use test_fixtures::game::{ActCreation, GameData, Seams, Sim};
use test_fixtures::{install, synth};

/// The synthetic town (levels row 1, lvlprest Def 0).
const TOWN: u32 = 1;
/// Fixture choices (`run` stages the player at a fixed point; the
/// session join places it by the spawn search, `join_run`).
const INIT: u32 = 0x1234_5678;
const GAME_SEED: u32 = 1234;
const CLASS: u32 = 3;
const CLIENT: ClientId = 0;
const FRAMES: usize = 50;
/// Sub-tiles per tile (`rooms.md` §9.2).
const SUB: i32 = 5;

fn data() -> &'static GameData {
    static D: OnceLock<GameData> = OnceLock::new();
    D.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("synthetic-game-{}", std::process::id()));
        let i = install::build(&dir, &synth::synthetic()).unwrap_or_else(|e| panic!("{e}"));
        GameData::from_install(&i).unwrap_or_else(|e| panic!("{e}"))
    })
}

struct NoRest;
impl VitalsRest for NoRest {
    fn refresh(&mut self, _: UnitId) {}
    fn level_up_notify(&mut self, _: UnitId) {}
    fn level_up_event(&mut self, _: UnitId) {}
}

struct NoSession;
impl SessionHandler for NoSession {
    fn system_message(&mut self, _: ClientId, _: &[u8], _: usize, _: &mut dyn MessageSink) {}
}

struct Ms(u32);
impl Clock for Ms {
    fn now_ms(&mut self) -> u32 {
        self.0
    }
}

type TestHost = Host<Sim, ProtoSizes, NoSession, Ms>;

/// What one run leaves behind, compared across runs.
#[derive(Debug, PartialEq, Eq)]
struct Run {
    player: UnitId,
    pos: (i32, i32),
    room_rect: (i32, i32, i32, i32),
    waypoint: (i32, i32),
    life: i32,
    received: Vec<Vec<u8>>,
}

#[test]
fn providers_hold_the_named_files() {
    let d = data();
    // Every lvlprest DS1 (4), lvlsub DS1 (1), lvltypes DT1 (2 floors + the
    // wall) and the fixed library (3).
    assert_eq!(d.files.ds1.0.len(), 4);
    assert_eq!(d.files.subs.0.len(), 1);
    assert_eq!(d.files.dt1.0.len(), 6);
    let keys: Vec<String> = d
        .files
        .ds1
        .0
        .keys()
        .map(|k| String::from_utf8_lossy(k).into_owned())
        .collect();
    let town = d
        .files
        .ds1
        .0
        .get(archive_name(PRESET_DS1[0]).as_bytes())
        .unwrap_or_else(|| panic!("town DS1 not among {keys:?}"));
    // Stored size = lvlprest SizeX / SizeY (`preset.md` §6).
    assert_eq!((town.width, town.height), (PRESET_SIZE, PRESET_SIZE));
    let (kind, id, x, y) = TOWN_WAYPOINT;
    assert_eq!(town.objects.len(), 1);
    let o = &town.objects[0];
    assert_eq!((o.kind, o.id, o.x, o.y), (kind, id, x, y));
    for lib in FIXED_LIBRARY {
        assert!(d.files.dt1.0.contains_key(lib.as_bytes()), "{lib}");
    }
    // The lvltypes and lvlprest strings are fixed up to archive names
    // (`fixups.md` §12), which the provider keys on.
    for f in FLOOR_DT1.iter().chain([&WALL_DT1]) {
        assert!(
            d.files.dt1.0.contains_key(archive_name(f).as_bytes()),
            "{f}"
        );
    }
    // The archive name the provider asks with is the fixture's.
    assert_eq!(
        d2_server::world_data::archive_name(PRESET_DS1[0].as_bytes()),
        archive_name(PRESET_DS1[0]).into_bytes()
    );
}

fn run() -> Run {
    let d = data();
    let (mut sim, types) = d
        .world_sim(
            ActCreation::TownOnly,
            INIT,
            TOWN,
            GAME_SEED,
            Seams::default(),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let vitals = d.vitals().unwrap();
    let objects: Vec<Objects> = d.rows().unwrap();

    // The town generated (`levels.md` §3 step 8) and its one room streamed.
    let mut game = Game::new();
    game.lists.ensure_act(0).unwrap();
    let (room, rect, wp) = sim
        .action
        .hooks()
        .drlg
        .with_act(0, &mut game.lists, |dr, svc| {
            let lv = dr.get_or_alloc_level(svc.data, svc.types, TOWN)?;
            if dr.level_rooms(lv).is_empty() {
                dr.generate_level(svc.data, svc.types, lv)?;
            }
            let rooms = dr.level_rooms(lv);
            assert_eq!(rooms.len(), 1, "a one-room town");
            let r = rooms[0];
            // The room build moves the map's preset units onto the
            // room's list, room-relative (`preset.md` §9).
            let active = dr.stream_room(svc, r)?;
            let t = types.borrow();
            let presets = t.act_presets(0).expect("act 0 presets");
            let units = presets.room_units(r);
            assert_eq!(units.len(), 1, "the waypoint preset");
            let u = &units[0];
            assert_eq!(u.unit_type, 2);
            let class = u.class;
            assert_eq!(
                objects[class as usize].operatefn, 23,
                "the waypoint row (objects row 0)"
            );
            let wp = (u.x, u.y, class as u32);
            drop(t);
            let rect = dr.room(r).rect;
            // Every cell of the (w + 1) × (h + 1) grid has the floor bit:
            // one floor record each, chosen from the written floor tile
            // (key (0, 0, 0), not the (10, 0, 0) fallback, `rooms.md` §9.4).
            let tiles = dr.room(r).tiles().expect("the room's tiles");
            assert_eq!(
                tiles.floors.len(),
                ((rect.w + 1) * (rect.h + 1)) as usize,
                "floor records"
            );
            for f in &tiles.floors {
                let t = dr.tile_info(f.tile);
                assert_eq!((t.orientation, t.main, t.sub), (0, 0, 0));
            }
            assert!(tiles.walls.is_empty() && tiles.shadows.is_empty());
            Ok::<_, d2_sim::drlg::DrlgError>((active, rect, wp))
        })
        .expect("act 0 has a DRLG")
        .unwrap_or_else(|e| panic!("town: {e:?}"));
    let room = room.expect("the town room is active");
    assert_eq!(sim.errors(), Vec::<String>::new(), "game creation");
    // The preset is the whole room: PRESET_SIZE tiles square.
    assert_eq!((rect.w, rect.h), (PRESET_SIZE as i32, PRESET_SIZE as i32));

    // Room-relative preset sub-tiles + the room's origin (`population.md` §9).
    let wp_at = (rect.x * SUB + wp.0, rect.y * SUB + wp.1);
    let start = (wp_at.0 + 5, wp_at.1 + 5);
    let mut alloc = |ty, class, (x, y): (i32, i32)| {
        let req = AllocRequest {
            ty,
            class,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: ty == UnitType::Player,
        };
        sim.action
            .with(&mut game, |g, v| v.allocate(g, &req, x, y))
            .expect("allocated")
    };
    let object = alloc(UnitType::Object, wp.2, wp_at);
    let player = alloc(UnitType::Player, CLASS, start);
    sim.action.sys.units.get_mut(player).unwrap().mode = 1;
    {
        let s = &mut sim.action.sys;
        let mut v = VitalsView {
            units: &s.units,
            stats: &mut s.stats,
            hooks: &mut NoHost,
            rest: &mut NoRest,
        };
        init_player_stats(&mut v, &vitals, player, 0);
    }
    // Creation life (`vitals.md` §1: (vit + hpadd) << 8) from the
    // synthetic charstats row.
    let cs = vitals.charstats(CLASS as i32).expect("charstats row");
    let life = (i32::from(cs.vit) + i32::from(cs.hpadd)) << 8;
    assert_eq!(sim.action.sys.stats.unit_total(player, 6, 0), life);

    let world = ActionWorld {
        waypoints: Some(d.waypoints().unwrap()),
        ..ActionWorld::default()
    };
    let mut s: Sim = SimGame::with_world(game, sim, world);
    s.join(CLIENT, Some(player), None, client_state::IN_GAME)
        .expect("join");
    s.set_player(
        player,
        PlayerFields {
            gate: PlayerGate {
                mode: 1,
                uninterruptable: false,
            },
            data: Some(PlayerData { last_accept: 0 }),
        },
    );
    s.set_unit(
        object,
        UnitFacts {
            act: 0,
            pos: Pos {
                x: wp_at.0,
                y: wp_at.1,
            },
            owner: None,
        },
    );
    let mut host: TestHost = Host::new(s, ProtoSizes, NoSession, Ms(1000));
    host.connect(CLIENT);
    // The first frame starts the tick timer (no tick).
    host.frame().expect("first frame");
    let mut received = host.receive(CLIENT);
    for f in 0..FRAMES {
        host.clock.0 += 40;
        let r = host.frame().unwrap_or_else(|e| panic!("frame {f}: {e:?}"));
        assert!(r.ticked, "one tick per frame");
        received.extend(host.receive(CLIENT));
        let s = &host.game;
        assert!(s.tick_faults.is_empty(), "frame {f}: {:?}", s.tick_faults);
        assert!(s.world.faults.is_empty(), "frame {f}: {:?}", s.world.faults);
        assert_eq!(s.events.errors(), Vec::<String>::new(), "frame {f}");
    }
    let s = &mut host.game;
    assert!(
        s.game.lists.unit(player).is_some(),
        "the player is still there"
    );
    // The player's position (its path, `path-placement.md` §2.1).
    let pos = s.events.action.hooks().path_position(player);
    assert!(
        pos.0 >= rect.x * SUB
            && pos.0 < (rect.x + rect.w) * SUB
            && pos.1 >= rect.y * SUB
            && pos.1 < (rect.y + rect.h) * SUB,
        "player at {pos:?} outside the town room {rect:?}"
    );
    Run {
        player,
        pos,
        room_rect: (rect.x, rect.y, rect.w, rect.h),
        waypoint: wp_at,
        life,
        received,
    }
}

#[test]
fn town_join_and_frames() {
    let a = run();
    // This run stages the player in the town room without the session
    // join (`town_entry_sends_the_join_sequence` runs that): the count of
    // what the idle client receives is printed, and compared across runs.
    println!(
        "player {:?} at {:?} in {:?}, waypoint {:?}, {} messages",
        a.player,
        a.pos,
        a.room_rect,
        a.waypoint,
        a.received.len()
    );
    // Determinism on the synthetic data: a second game is identical.
    assert_eq!(run(), a);
}

/// The character name of the join test (zero-padded, 0x59 bytes 6..22).
fn name() -> [u8; 16] {
    let mut n = [0u8; 16];
    n[..6].copy_from_slice(b"werwer");
    n
}

/// What the client receives from the session join (`enter_game`) and the
/// next ticks, with the facts the expected bytes are built from.
#[derive(Debug, PartialEq, Eq)]
struct Joined {
    guid: u32,
    obj_seed: u32,
    room_rect: (i32, i32, i32, i32),
    pos: (i32, i32),
    received: Vec<Vec<u8>>,
}

/// The town of [`run`] with its waypoint, an unplaced player joined in
/// state 4, then `enter_game` and `FRAMES` frames.
fn join_run() -> Joined {
    let d = data();
    let (mut sim, _) = d
        .world_sim(
            ActCreation::TownOnly,
            INIT,
            TOWN,
            GAME_SEED,
            Seams::default(),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let mut game = Game::new();
    game.lists.ensure_act(0).unwrap();
    let (room, rect) = sim
        .action
        .hooks()
        .drlg
        .with_act(0, &mut game.lists, |dr, svc| {
            let lv = dr.get_or_alloc_level(svc.data, svc.types, TOWN)?;
            let r = dr.level_rooms(lv)[0];
            Ok::<_, d2_sim::drlg::DrlgError>((dr.stream_room(svc, r)?, dr.room(r).rect))
        })
        .expect("act 0 has a DRLG")
        .unwrap_or_else(|e| panic!("town: {e:?}"));
    let room = room.expect("the town room is active");
    let (_, _, wx, wy) = TOWN_WAYPOINT;
    let wp_at = (rect.x * SUB + wx as i32, rect.y * SUB + wy as i32);
    let mut alloc = |ty, class, room, (x, y): (i32, i32)| {
        let req = AllocRequest {
            ty,
            class,
            room,
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: ty == UnitType::Player,
        };
        sim.action
            .with(&mut game, |g, v| v.allocate(g, &req, x, y))
            .expect("allocated")
    };
    let objects: Vec<Objects> = d.rows().unwrap();
    let wp_class = objects
        .iter()
        .position(|o| o.operatefn == 23)
        .expect("the waypoint row") as u32;
    alloc(UnitType::Object, wp_class, Some(room), wp_at);
    // The player as the save loader leaves it: no room, at (0, 0).
    let player = alloc(UnitType::Player, CLASS, None, (0, 0));
    sim.action.sys.units.get_mut(player).unwrap().mode = 1;
    let guid = game.lists.unit(player).unwrap().guid;
    let obj_seed = sim.action.hooks().objects.as_ref().unwrap().obj_seed;
    let world = ActionWorld {
        waypoints: Some(d.waypoints().unwrap()),
        ..ActionWorld::default()
    };
    let mut s: Sim = SimGame::with_world(game, sim, world);
    s.join(CLIENT, Some(player), None, client_state::IN_GAME)
        .expect("join");
    let entry = Entry {
        act: 0,
        name: name(),
    };
    assert_eq!(enter_game(&mut s, CLIENT, &entry), Ok(player));
    // A second entry of the placed player is refused.
    assert_eq!(
        enter_game(&mut s, CLIENT, &entry),
        Err(JoinError::Placed(player))
    );
    let mut host: TestHost = Host::new(s, ProtoSizes, NoSession, Ms(1000));
    host.connect(CLIENT);
    host.frame().expect("first frame");
    let mut received = host.receive(CLIENT);
    for f in 0..FRAMES {
        host.clock.0 += 40;
        host.frame().unwrap_or_else(|e| panic!("frame {f}: {e:?}"));
        received.extend(host.receive(CLIENT));
        let s = &host.game;
        assert!(s.tick_faults.is_empty(), "frame {f}: {:?}", s.tick_faults);
        assert_eq!(s.events.errors(), Vec::<String>::new(), "frame {f}");
    }
    let s = &mut host.game;
    let pos = s.events.action.hooks().path_position(player);
    Joined {
        guid,
        obj_seed,
        room_rect: (rect.x, rect.y, rect.w, rect.h),
        pos,
        received,
    }
}

// Covers: specs/sim/path-placement.md §11, §13 r1, §13 r3; specs/client/model.md §11 r1, §11 r3; specs/sim/intents-events.md §7.2
#[test]
fn town_entry_sends_the_join_sequence() {
    let j = join_run();
    let (rx, ry, rw, rh) = j.room_rect;
    // The spawn search put the player in the one town room.
    let (x, y) = j.pos;
    assert!(
        x >= rx * SUB && x < (rx + rw) * SUB && y >= ry * SUB && y < (ry + rh) * SUB,
        "player at {:?} outside the town room {:?}",
        j.pos,
        j.room_rect
    );
    let g = j.guid.to_le_bytes();
    let mut assign = vec![0x59, g[0], g[1], g[2], g[3], CLASS as u8];
    assign.extend(name());
    assign.extend([0, 0, 0, 0]);
    let mut load = vec![0x03, 0];
    load.extend(INIT.to_le_bytes());
    load.extend((TOWN as u16).to_le_bytes());
    load.extend(j.obj_seed.to_le_bytes());
    let reveal = {
        let mut b = vec![0x07];
        b.extend((rx as u16).to_le_bytes());
        b.extend((ry as u16).to_le_bytes());
        b.push(TOWN as u8);
        b
    };
    let mut place = vec![0x15, 0, g[0], g[1], g[2], g[3]];
    place.extend((x as u16).to_le_bytes());
    place.extend((y as u16).to_le_bytes());
    place.push(1);
    let handshake = vec![0x0B, 0, g[0], g[1], g[2], g[3]];
    // The session part (enter_game), then the first tick's room switch:
    // one 0x07 per room of the spawn room's adjacency array (the town has
    // one room, so its own) (`path-placement.md` §11 "Recipients").
    assert_eq!(
        j.received[..6].to_vec(),
        vec![assign, handshake, load, reveal.clone(), place, reveal],
        "join prefix of {:02x?}",
        j.received
    );
    // No further room comes into sight while the player stands still.
    assert!(!j.received[6..].iter().any(|m| m[0] == 0x07));
    // Determinism on the synthetic data.
    assert_eq!(join_run(), j);
}
