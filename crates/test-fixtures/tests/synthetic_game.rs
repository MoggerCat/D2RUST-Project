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
use d2_server::adapters::session::{
    create_game, enter_game, Entry, GameSetup, HotKey, JoinError, PlayerRecord, SkillHand,
};
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

/// The join test's character: the recorded record (`-022633` seq 113–141:
/// hand 1 skill 0, hand 0 skill 36, no item), portal flags 5, hot keys in
/// slot 3 (skill 0, flag set) and slot 7 (a skill id past the `skills`
/// rows: not sent, `intents-events.md` §8.2 rule 3.6).
fn entry(skills: usize) -> Entry {
    let mut e = Entry::new(0, name());
    e.hotkeys[3] = HotKey {
        skill: 0,
        flag: true,
        item: u32::MAX,
    };
    e.hotkeys[7] = HotKey {
        skill: skills as i16,
        flag: false,
        item: 9,
    };
    e.record = Some(PlayerRecord {
        portal_flags: 5,
        hands: [
            SkillHand {
                skill: 36,
                item: u32::MAX,
            },
            SkillHand {
                skill: 0,
                item: u32::MAX,
            },
        ],
    });
    e
}

/// What the client receives from the session join (`enter_game`) and the
/// next ticks, with the facts the expected bytes are built from.
#[derive(Debug, PartialEq, Eq)]
struct Joined {
    guid: u32,
    obj_seed: u32,
    room_rect: (i32, i32, i32, i32),
    pos: (i32, i32),
    /// The waypoint's GUID, class, position, mode and interact byte.
    waypoint: (u32, u16, (i32, i32), u8, u8),
    /// The client's state after the frames.
    state: u32,
    /// Messages of the first flush (the join and tick 1).
    first: usize,
    /// `skills` rows of the game.
    skills: usize,
    received: Vec<Vec<u8>>,
}

/// The game of the join test (`intents-events.md` §8.1: Normal,
/// expansion, not ladder, the recorded arena flags).
const SETUP: GameSetup = GameSetup {
    difficulty: 0,
    arena_flags: 0x0010_0004,
    expansion: true,
    ladder: false,
};

/// The town of [`run`] with its waypoint, an unplaced player, its client
/// record, then `create_game`, `enter_game` and `FRAMES` frames.
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
    let wp = alloc(UnitType::Object, wp_class, Some(room), wp_at);
    // The player as the save loader leaves it: no room, at (0, 0).
    let player = alloc(UnitType::Player, CLASS, None, (0, 0));
    sim.action.sys.units.get_mut(player).unwrap().mode = 1;
    let guid = game.lists.unit(player).unwrap().guid;
    let wp_guid = game.lists.unit(wp).unwrap().guid;
    let obj_seed = sim.action.hooks().objects.as_ref().unwrap().obj_seed;
    let world = ActionWorld {
        waypoints: Some(d.waypoints().unwrap()),
        ..ActionWorld::default()
    };
    let mut s: Sim = SimGame::with_world(game, sim, world);
    // The client record as its allocation leaves it (state 0); game
    // creation sets state 1, the join 2 then 3 (`intents-events.md` §8).
    s.join(CLIENT, Some(player), None, 0).expect("join");
    create_game(&mut s, CLIENT, &SETUP).expect("game creation");
    let skills = s.events.action.hooks().tables.skills.skills.len();
    let entry = entry(skills);
    assert_eq!(enter_game(&mut s, CLIENT, &entry), Ok(player));
    let id = s.sim_client(CLIENT).unwrap();
    assert_eq!(
        s.game.lists.client(id).unwrap().state,
        client_state::JOINING
    );
    // The waypoint's 0x51 fields at the join.
    let a = &mut s.events.action;
    let wp_mode = a.sys.units.get(wp).unwrap().mode as u8;
    let wp_interact = a
        .hooks()
        .objects
        .as_ref()
        .and_then(|o| o.control.data.get(&wp))
        .map_or(0, |d| d.interact);
    let waypoint = (wp_guid, wp_class as u16, wp_at, wp_mode, wp_interact);
    // A second entry of the placed player is refused.
    assert_eq!(
        enter_game(&mut s, CLIENT, &entry),
        Err(JoinError::Placed(player))
    );
    let mut host: TestHost = Host::new(s, ProtoSizes, NoSession, Ms(1000));
    host.connect(CLIENT);
    host.frame().expect("first frame");
    let mut received = host.receive(CLIENT);
    // The first flush (the host's first frame starts the tick driver's
    // clock and may not tick).
    let mut first = None;
    for f in 0..FRAMES {
        host.clock.0 += 40;
        host.frame().unwrap_or_else(|e| panic!("frame {f}: {e:?}"));
        received.extend(host.receive(CLIENT));
        if first.is_none() && !received.is_empty() {
            first = Some(received.len());
        }
        let s = &host.game;
        assert!(s.tick_faults.is_empty(), "frame {f}: {:?}", s.tick_faults);
        assert_eq!(s.events.errors(), Vec::<String>::new(), "frame {f}");
    }
    let s = &mut host.game;
    let pos = s.events.action.hooks().path_position(player);
    let state = s.game.lists.client(id).unwrap().state;
    Joined {
        guid,
        obj_seed,
        room_rect: (rect.x, rect.y, rect.w, rect.h),
        pos,
        waypoint,
        state,
        first: first.unwrap_or(0),
        skills,
        received,
    }
}

// Covers: specs/sim/path-placement.md §11, §13 r1, §13 r3; specs/client/model.md §11 r1, §11 r3; specs/sim/intents-events.md §7.2, §7.8 r2, §7.8 r5, §8.1, §8.2 r3, §8.2 r4, §8.2 r5, §8.2 r6, §8.3; specs/sim/tick.md §6 r6
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
    assert!(j.skills > 0, "the synthetic skills table has rows");
    let g = j.guid.to_le_bytes();
    let mut assign = vec![0x59, g[0], g[1], g[2], g[3], CLASS as u8];
    assign.extend(name());
    assign.extend([0, 0, 0, 0]);
    // The player has no state: the stream is the end marker alone.
    let states = vec![0xAA, 0, g[0], g[1], g[2], g[3], 8, 0xFF];
    let proximity = vec![0x76, 0, g[0], g[1], g[2], g[3]];
    let handshake = vec![0x0B, 0, g[0], g[1], g[2], g[3]];
    let portal = vec![0x5F, 5, 0, 0, 0];
    let hotkey = vec![0x7B, 3, 0, 0x80, 0xFF, 0xFF, 0xFF, 0xFF];
    let hand = |h: u8, skill: u8| {
        vec![
            0x23, 0, g[0], g[1], g[2], g[3], h, skill, 0, 0xFF, 0xFF, 0xFF, 0xFF,
        ]
    };
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
    let (wg, wc, (wx, wy), wm, wi) = j.waypoint;
    let object = {
        let mut b = vec![0x51, 2];
        b.extend(wg.to_le_bytes());
        b.extend(wc.to_le_bytes());
        b.extend((wx as u16).to_le_bytes());
        b.extend((wy as u16).to_le_bytes());
        b.extend([wm, wi]);
        b
    };
    let mut place = vec![0x15, 0, g[0], g[1], g[2], g[3]];
    place.extend((x as u16).to_le_bytes());
    place.extend((y as u16).to_le_bytes());
    place.push(1);
    // Game creation (§8.1), the join (§8.2: the player's own add
    // messages, 0x0B, 0x5F, slot 3's hot key, the two hands; no 0x95: the
    // player has no life), then game entry: 0x07 for the spawn room, the
    // room switch (the town has one room: its 0x07 again, then the add
    // messages of its units, the waypoint), 0x15, 0x7E; all before the
    // first tick (`path-placement.md` §11 "Recipients").
    let want = vec![
        vec![0x01, 0x00, 0x04, 0x00, 0x10, 0x00, 0x01, 0x00],
        vec![0x00],
        vec![0x02],
        assign,
        states,
        proximity,
        handshake,
        portal,
        hotkey,
        hand(1, 0),
        hand(0, 36),
        load,
        reveal.clone(),
        reveal,
        object,
        place,
        vec![0x7E, 0, 0, 0, 0],
    ];
    assert_eq!(
        j.received[..want.len()].to_vec(),
        want,
        "join prefix of {:02x?}",
        j.received
    );
    // The first tick populates the town room, so the client's room is
    // ready: 0x04 once, the client in game (`tick.md` §6 rule 6).
    let rest = &j.received[want.len()..];
    assert_eq!(rest.iter().filter(|m| m[..] == [0x04]).count(), 1);
    let at = j.received.iter().position(|m| m[..] == [0x04]).unwrap();
    assert!(
        at < j.first,
        "0x04 at {at}, after the first flush's {}",
        j.first
    );
    assert_eq!(j.state, client_state::IN_GAME);
    // No further room comes into sight while the player stands still.
    assert!(!rest.iter().any(|m| m[0] == 0x07 || m[0] == 0x08));
    // Determinism on the synthetic data.
    assert_eq!(join_run(), j);
}
