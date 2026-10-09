// Spec: specs/client/model.md (§9, §11, §12), specs/render/composition.md (§4), specs/sim/path-placement.md (§13)
//! The client DRLG in the play mode's wiring, headless: `add_game` +
//! `add_client_data` over the app's own single-player game, whose
//! session flow (`d2_server::adapters::session_flow`) answers the
//! client's C→S 0x67 with 0x01, 0x00, 0x02 and its 0x6B (the bridge's
//! answer to 0x02) with the join: 0x59, 0x0B, 0x03, 0x07 and 0x15,
//! followed by the room switch's 0x07s. The client builds its own act DRLG from 0x03 and the
//! rooms 0x07 brings in sight; the local player's level is the level of
//! its room. The recorded join (ignored test) is delivered by a scripted
//! link in the recorded order (0x01, 0x03, 0x59 at (0, 0), 0x0B, 0x07,
//! 0x15, 0x04; `model.md` §11 rule 3).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, send_create_game};
use d2_client::app::single_player::{self, GameData};
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::world::{ClientTables, ClientWorld};
use d2_client::bridge::{Bridge, BridgeResource};
use d2_client::rules::OpenMode;
use d2_client::world_view::WorldViewState;
use d2_proto::PROTOCOL_VERSION;
use d2_server::seams::Clock;

mod app_support;
use app_support::SharedLink;

/// Delivers one chunk list per pump (each pump a tick).
struct Script(VecDeque<Vec<Vec<u8>>>, Vec<Vec<u8>>);

impl ServerLink for Script {
    fn protocol_version(&self) -> u32 {
        PROTOCOL_VERSION
    }
    fn send(&mut self, _: SendQueue, _: &[u8]) -> Result<Sent, LinkError> {
        Ok(Sent::Queued)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.1 = self.0.pop_front().unwrap_or_default();
        Ok(Pumped { ticked: true })
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.1)
    }
}

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// 0x59 for player 1 ("werwer", class 1) at (0, 0).
fn assign_player() -> Vec<u8> {
    let mut b = hex("59 01 00 00 00 01 77 65 72 77 65 72");
    b.resize(0x1A, 0);
    b
}

/// The join of `model.md` §11 rule 3 with 0x03 `load_act`, the 0x07
/// `reveal` and the 0x15 `place`; frame 1 receives everything up to
/// 0x15, frame 2 the 0x04.
fn join(load_act: &str, reveal: &str, place: &str) -> Script {
    let frame1 = vec![
        hex("01 00 04 00 10 00 01 00"),
        hex(load_act),
        assign_player(),
        hex("0b 00 01 00 00 00"),
        hex(reveal),
        hex(place),
    ];
    Script(VecDeque::from([frame1, vec![hex("04")]]), Vec::new())
}

fn check_join(w: &ClientWorld, rejected: usize) -> u16 {
    assert_eq!(rejected, 0);
    assert!(w.in_game);
    let act = w.act.expect("0x03 received");
    let d = w.drlg.as_ref().expect("client DRLG built");
    assert_eq!((d.drlg.act, d.drlg.init_seed), (act.act, act.init_seed));
    assert!(d.drlg.on_client);
    let own = w
        .local_room()
        .expect("the local player is in an active room");
    assert_eq!(w.player_level(), Some(own.level));
    own.level
}

/// The host clock, advanced by the test.
struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

// Covers: specs/client/model.md §12 r1, §11 r3, §11 r5, §9 r1; specs/sim/path-placement.md §13 r3; specs/sim/tick.md §6 r6; specs/sim/intents-events.md §8.3
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_join_builds_the_client_drlg_in_the_app() {
    let data = app_support::game_data();
    // The app's own game on its server thread, entered through the
    // session flow (`intents-events.md` §8): the client's 0x67 → 0x01,
    // 0x00, 0x02 with tick 1's flush; the client's 0x6B → 0x59, 0xAA,
    // 0x76, 0x0B, 0x03, 0x53, game entry's 0x07, its room switch's 0x07s
    // (and the add messages of the rooms' units), 0x15 and 0x7E with tick
    // 2's flush, which adds 0x04.
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, _) = single_player::start(
        data.clone(),
        single_player::DEFAULT_SEED,
        StepClock(ms.clone()),
    )
    .unwrap();
    let server = Arc::new(Mutex::new(link));
    let mut app = App::new();
    app.insert_resource(d2_client::bridge::mirror::ScriptedClock(ms.clone()));
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    add_game(&mut app, Box::new(SharedLink(server.clone())), false).unwrap();
    send_create_game(&mut app).unwrap();
    add_client_data(
        &mut app,
        single_player::client_drlg_source(&data),
        single_player::client_level_rows(&data),
    );
    app_support::live_tables(&mut app);
    // No original UI here: the open mode it would hand over with every
    // panel closed (`ui/panels.md` §4.2), so the world view can place.
    app.world_mut()
        .resource_mut::<WorldViewState>()
        .feed
        .set_ui_open_mode(OpenMode::new(0).unwrap());
    // Frame 1 drains the 0x67 (game creation) and starts the host's tick
    // clock (no tick, nothing received).
    app.update();
    assert!(app
        .world()
        .resource::<BridgeResource>()
        .0
        .world()
        .act
        .is_none());
    // Frame 2: tick 1's flush carries 0x01, 0x00, 0x02; the bridge
    // answers 0x02 with 0x6B. No player yet.
    ms.fetch_add(40, Ordering::SeqCst);
    app.update();
    {
        let w = app.world().resource::<BridgeResource>().0.world();
        assert!(w.act.is_none() && w.local_player.is_none() && !w.in_game);
    }
    assert_eq!(app_support::local_player(&server), None);
    // Frame 3: the drain runs the join (0x6B), tick 2's flush carries it.
    ms.fetch_add(40, Ordering::SeqCst);
    app.update();
    let (player, guid) = app_support::local_player(&server).expect("joined");
    let server_pos = app_support::with(&server, move |l| {
        l.host_mut()
            .game
            .events
            .action
            .hooks()
            .path_position(player)
    });
    let b = &app.world().resource::<BridgeResource>().0;
    let w = b.world();
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
    // 0x03: act 0, the synthetic act's init seed (1), its town level 1,
    // game +0x80 = 0 (the app game has no object control).
    let act = w.act.expect("0x03 received");
    assert_eq!((act.act, act.init_seed), (0, 1));
    let d = w.drlg.as_ref().expect("client DRLG built");
    assert_eq!((d.drlg.act, d.drlg.init_seed), (act.act, act.init_seed));
    assert!(d.drlg.on_client);
    // 0x59 + 0x0B: the local player; 0x15: placed where the server put it.
    let me = w.local_player.expect("0x0B named the local player");
    assert_eq!(me.guid, guid);
    let own = w
        .local_room()
        .expect("the local player is in an active room");
    assert_eq!(u32::from(own.level), single_player::ACT1_TOWN);
    assert_eq!(w.player_level(), Some(own.level));
    let pos = w.units[&me].position.expect("placed by 0x15");
    assert_eq!((i32::from(pos.0), i32::from(pos.1)), server_pos);
    // Game entry's 0x07 for the spawn room, then the room switch's for
    // each room of its adjacency array (the synthetic town is one room).
    let shown: Vec<_> = w
        .rooms_in_sight
        .iter()
        .map(|r| (r.show, r.level, r.x, r.y))
        .collect();
    // The synthetic town room is at tile (16, 0); the Blood Moor room
    // east of it (tile (24, 0), level 2) is in the town room's adjacency
    // across the level border (`drlg/rooms.md` §3.3).
    assert_eq!(
        shown,
        vec![(true, 1, 16, 0), (true, 1, 16, 0), (true, 2, 24, 0)]
    );
    assert_eq!(
        w.active_rooms.as_ref().map(|r| r.len()),
        Some(2),
        "the one room of the town and the bordering Blood Moor room"
    );
    // Tick 2 populated the town room, so the client's room was ready and
    // the client pass sent 0x04 (`tick.md` §6 rule 6): the
    // client is in game.
    assert!(w.in_game);
    // The feed answers BlankScreen from the player's level's row (the
    // synthetic rows have BlankScreen 0).
    let state = app.world().resource::<WorldViewState>();
    assert!(!state.feed.blank_screen(w).unwrap());
}

/// The recorded join of `client/model.md` §Test vectors (recording
/// `20261006-022633`): 0x03 seq 142 (act 0, init seed 0x103888C4), 0x07
/// seq 144 (level 1, tile (0x3A0, 0x388)), 0x15 seq 154 (player 1 to
/// (4673, 4548)). Expect the client DRLG of the user's tables to have a
/// level-1 room of origin tile (928, 904) holding the player: level 1.
///
/// `D2_GAME_DIR=<install> cargo test -p d2-client --test app_client_drlg -- --ignored`
// Covers: specs/client/model.md §12 r1, §9 r1, §11 r3; specs/render/composition.md §4
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn the_recorded_join_on_the_install() {
    use d2_client::app::palette::{act_palette_path, ActPalettes};

    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
    let data = GameData::select(Some(std::path::Path::new(&dir))).unwrap();
    let GameData::Live(live) = &data;
    let link = join(
        "03 00 c4 88 38 10 01 00 61 d1 e0 9f",
        "07 a0 03 88 03 01",
        "15 00 01 00 00 00 41 12 c4 11 01",
    );
    let mut bridge = Bridge::new(link).unwrap();
    bridge.set_drlg_source(Some(single_player::client_drlg_source(&data)));
    bridge.set_tables(ClientTables {
        levels: single_player::client_level_rows(&data),
        ..ClientTables::default()
    });
    // The install's client tables: the join selects the real skills.
    app_support::live_bridge_tables(&mut bridge);
    bridge.frame().unwrap();
    bridge.frame().unwrap();
    let w = bridge.world();
    let level = check_join(w, bridge.log().rejected.len());
    assert_eq!(level, 1);
    let own = w.local_room().unwrap();
    assert_eq!((own.x0, own.y0), (928 * 5, 904 * 5));
    // Every act palette is in the archives and is a valid `pal.pl2`.
    let palettes = ActPalettes::live(live.archives.as_ref()).unwrap();
    for a in 0..5 {
        d2_client::scene::present_palette(palettes.of(a))
            .unwrap_or_else(|e| panic!("{}: {e}", act_palette_path(a)));
    }
}

/// The session join of the app's game on the user's files: game entry in
/// the Rogue Encampment (`sim/path-placement.md` §13: the town's spawn
/// search), the client DRLG built from the 0x03 and the 0x07s, the local
/// player in a level-1 room at the server's point. Prints the received
/// 0x07 count (1 from game entry + the spawn room's adjacency array).
///
/// `D2_GAME_DIR=<install> cargo test -p d2-client --test app_client_drlg -- --ignored`
// Covers: specs/sim/path-placement.md §13 r3; specs/client/model.md §12 r1, §11 r3
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn the_session_join_on_the_install() {
    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
    let data = GameData::select(Some(std::path::Path::new(&dir))).unwrap();
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
    // The session sequence: 0x67, then 0x6B after the flush with 0x02.
    bridge.send(&single_player::create_request()).unwrap();
    bridge.frame().unwrap();
    for _ in 0..2 {
        ms.fetch_add(40, Ordering::SeqCst);
        bridge.frame().unwrap();
    }
    let server_pos = bridge
        .link_mut()
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let (player, _) = single_player::local_player(sim).expect("joined");
            sim.events.action.hooks().path_position(player)
        })
        .unwrap();
    assert!(
        bridge.log().rejected.is_empty(),
        "{:?}",
        bridge.log().rejected
    );
    let w = bridge.world();
    assert_eq!(w.act.map(|a| (a.act, a.town_level)), Some((0, 1)));
    let me = w.local_player.expect("0x0B named the local player");
    let pos = w.units[&me].position.expect("placed by 0x15");
    assert_eq!((i32::from(pos.0), i32::from(pos.1)), server_pos);
    assert_eq!(w.player_level(), Some(1));
    println!(
        "player at {pos:?}, {} rooms in sight: {:?}",
        w.rooms_in_sight.len(),
        w.rooms_in_sight
    );
    assert!(w.rooms_in_sight.len() >= 2);
}

// Covers: specs/render/lighting.md §8
/// The monster light columns the model creates monster lights from
/// (`MonsterClass::light`, `light_rgb`; §8 monster row), on the user's
/// install: each `monstats` row carries its `MonStatsEx` row's `Light`
/// and `light-r/g/b`, and the §8 measured facts hold (100 `monstats2`
/// rows with `Light` > 0; the fallen shamans' 5 with 230, 168, 255).
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn monster_light_rows_come_from_the_users_monstats2() {
    use d2_data::tables::{decode_all, Monstats, Monstats2};
    let d = app_support::live();
    let a = d.archives.as_ref();
    let rows = single_player::client_unit_rows(a).unwrap();
    let set = d2_data::bin::load_from(a, "eng").unwrap();
    let m1: Vec<Monstats> = decode_all(set.table("monstats").unwrap()).unwrap();
    let m2: Vec<Monstats2> = decode_all(set.table("monstats2").unwrap()).unwrap();
    assert_eq!(m2.iter().filter(|r| r.light > 0).count(), 100);
    let mut checked = 0;
    for (m, class) in m1.iter().zip(&rows.monsters) {
        let Some(class) = class else { continue };
        let x = &m2[usize::from(m.monstatsex)];
        assert_eq!(
            (class.light, class.light_rgb),
            (x.light, (x.light_r, x.light_g, x.light_b))
        );
        checked += 1;
    }
    assert!(
        checked > 600,
        "{checked} monstats rows with a monstats2 row"
    );
    assert!(rows
        .monsters
        .iter()
        .flatten()
        .any(|c| (c.light, c.light_rgb) == (5, (230, 168, 255))));
}

// Covers: specs/render/draw-order-2.md §12 l2 r2
// Covers: specs/render/draw-order-2.md §12 l2 r3
/// The Arreat Summit background on the user's archives: `summit01` and
/// `cloud01` load and give the 12 mountain cels (resolution mode 2) and
/// the 10 clouds' 20 cels, in pass 1, the clouds in draw mode 3 through
/// the act V tables.
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_summit_background_draws_from_the_users_archives() {
    use d2_client::bridge::drlg::DrlgRoomId;
    use d2_client::bridge::world::{ActiveRoom, ClientUnit, UnitKey};
    use d2_client::rules::camera::{Camera, ClientPos, FrameSize};
    use d2_client::rules::shading::ShadeTables;
    use d2_client::scene::{order::pass, BlendOp};
    use d2_client::world_view::background_view::BackgroundView;
    use d2_client::world_view::{ViewAssets, WorldFrame};
    use d2_sim::rng::Seed;
    let d = app_support::live();
    let pl2 = d
        .archives
        .source()
        .read_file(r"data\global\palette\act5\pal.pl2")
        .expect("act V pal.pl2")
        .unwrap();
    let mut assets = ViewAssets::from_pl2(&pl2).unwrap();
    let pl2 = d2_formats::palette::Pl2::parse(&pl2).unwrap();
    assets.shades = Some(ShadeTables::push(&mut assets.maps, &pl2));
    let mut w = ClientWorld::default();
    let p = UnitKey::new(0, 1);
    let mut u = ClientUnit::new(p);
    u.position = Some((5000, 5000));
    w.units.insert(p, u);
    w.local_player = Some(p);
    w.active_rooms = Some(vec![ActiveRoom {
        x0: 4900,
        y0: 4900,
        w: 200,
        h: 200,
        level: 120,
        room: DrlgRoomId(1),
    }]);
    w.room_units.place(p, Some(DrlgRoomId(1)));
    let mut frame = WorldFrame {
        camera: Some(Camera::new(
            FrameSize::D2RS,
            OpenMode::NONE,
            ClientPos { x: 0, y: 0 },
            (0, 0),
        )),
        ..WorldFrame::default()
    };
    let mut v = BackgroundView::new(d.archives.source(), Some(Seed::new(7, 666)));
    let log = v.add_to_frame(&w, 0, 10_063 + 2_056, &mut assets, &mut frame);
    assert!(log.is_empty(), "{log:?}");
    assert_eq!(frame.items.len(), 12 + 20);
    assert!(frame
        .items
        .iter()
        .all(|i| i.key.pass() == pass::LEVEL_BACKGROUND));
    let opaque = frame
        .items
        .iter()
        .filter(|i| i.blend == BlendOp::Opaque)
        .count();
    assert_eq!(opaque, 12, "the mountains opaque, the clouds blended");
}

// Covers: specs/render/shading.md §6 r1
/// The `states` rows the model's state messages and colour call read, on
/// the user's install (`shading.md` §6 r1.1 live rows): seven states have
/// a `colorpri`, all with a `colorshift`; 90 `blue` 100 / 108 / 150, 215,
/// 255 and 2 `poison` 95 / 104 / 128, 255, 128.
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn state_colour_rows_come_from_the_users_states() {
    let d = app_support::live();
    let rows = single_player::client_unit_rows(d.archives.as_ref()).unwrap();
    assert_eq!(rows.states.len(), 185);
    let coloured: Vec<usize> = (0..rows.states.len())
        .filter(|&s| rows.states[s].colorpri > 0)
        .collect();
    assert_eq!(coloured.len(), 7, "{coloured:?}");
    assert!(coloured.iter().all(|&s| rows.states[s].colorshift != 0));
    let row = |s: usize| {
        let r = rows.states[s];
        (r.colorpri, r.colorshift, r.light_rgb)
    };
    assert_eq!(row(90), (100, 108, (150, 215, 255)));
    assert_eq!(row(2), (95, 104, (128, 255, 128)));
}
