// Spec: specs/client/bridge.md (§3), specs/world/waypoints.md (§5.1)
//! The app's single-player game and its server thread: the link is
//! `Send + Sync` (the game is not, and never leaves its thread), builder
//! failures are reported, the build is deterministic, and the live tables
//! (with `D2_GAME_DIR`, ignored by default) give a waypoint object.

use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{
    self, GameData, Link, WaypointTables, ACT2_TOWN, COLD_PLAINS, DEFAULT_SEED,
};
use d2_client::bridge::link::ServerLink;
use d2_client::bridge::local::{LocalLink, PendingSession};
use d2_proto::PROTOCOL_VERSION;
use d2_server::adapters::ProtoSizes;
use d2_server::host::{Host, SystemClock};

fn send_sync<T: Send + Sync>() {}

#[test]
fn the_link_is_send_and_sync() {
    send_sync::<ThreadLink<Link<SystemClock>>>();
    send_sync::<Box<dyn ServerLink + Send + Sync>>();
}

#[test]
fn a_failing_builder_is_an_error() {
    let r = ThreadLink::<Link<SystemClock>>::spawn(|| Err::<Link<SystemClock>, _>("no tables"));
    let e = r.err().expect("refused");
    assert!(e.to_string().contains("no tables"), "{e}");
}

#[test]
fn the_build_is_deterministic_and_runs_on_its_thread() {
    let units = |seed| {
        let g = single_player::build(&GameData::Synthetic, seed).unwrap();
        (g.waypoint, g.waypoint_guid)
    };
    assert_eq!(units(DEFAULT_SEED), units(DEFAULT_SEED));

    let (mut link, started) =
        single_player::start(GameData::Synthetic, DEFAULT_SEED, SystemClock::default()).unwrap();
    assert_eq!(link.protocol_version(), PROTOCOL_VERSION);
    let guid = started.waypoint_guid;
    // The game behind the link is the one built: its waypoint unit has the
    // GUID the start reported, the session flow is set, and no client
    // record or player exists before the client's C→S 0x67 / 0x6B
    // (`intents-events.md` §8).
    let seen = link
        .with(move |l| {
            let sim = &l.host().game;
            (
                sim.game.lists.unit(started.waypoint).map(|u| u.guid),
                sim.session().is_some(),
                sim.sim_client(d2_client::bridge::LOCAL_CLIENT).is_some(),
                single_player::local_player(sim),
            )
        })
        .unwrap();
    assert_eq!(seen, (Some(guid), true, false, None));
    // A pump on the server thread: the first frame starts the host clock.
    assert!(!link.pump().unwrap().ticked);
}

#[test]
fn a_link_built_elsewhere_runs_unchanged() {
    // Any `ServerLink` can live on the thread: here the bare local link.
    let mut link = ThreadLink::spawn(|| {
        let g = single_player::build(&GameData::Synthetic, DEFAULT_SEED)?;
        Ok::<_, single_player::BuildError>(LocalLink::new(Host::new(
            g.sim,
            ProtoSizes,
            PendingSession::default(),
            SystemClock::default(),
        )))
    })
    .unwrap();
    assert!(link.receive().is_empty());
}

/// The host clock, advanced by the test.
struct StepClock(std::sync::Arc<std::sync::atomic::AtomicU32>);

impl d2_server::seams::Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// The app's session sequence on the link (`intents-events.md` §8): the
/// client's 0x67 (`single_player::create_request`) is drained in the
/// first frame and answered with 0x01, 0x00, 0x02 at tick 1's flush; the
/// loader runs only at the 0x6B: it creates a player of the request's
/// class (sorceress), knowing Cold Plains' waypoint, with its player
/// fields; the join's 0x59 … 0x7E and 0x04 follow with tick 2's flush.
// Covers: specs/sim/intents-events.md §8.1, §8.2 r2, §8.2 r3
#[test]
fn the_session_flow_creates_the_game_then_loads_the_character_at_the_join() {
    use d2_client::bridge::link::SendQueue;
    use std::sync::atomic::{AtomicU32, Ordering};

    let ms = std::sync::Arc::new(AtomicU32::new(1000));
    let (mut link, _) =
        single_player::start(GameData::Synthetic, DEFAULT_SEED, StepClock(ms.clone())).unwrap();
    let ids = |chunks: Vec<Vec<u8>>| -> Vec<u8> { chunks.iter().map(|c| c[0]).collect() };
    let req = single_player::create_request();
    assert_eq!(req.encode()[0], 0x67);
    link.send(SendQueue::System, &req.encode()).unwrap();
    assert!(!link.pump().unwrap().ticked);
    assert!(link.receive().is_empty());
    ms.fetch_add(40, Ordering::SeqCst);
    assert!(link.pump().unwrap().ticked);
    assert_eq!(ids(link.receive()), [0x01, 0x00, 0x02]);
    let before = link
        .with(|l| {
            let sim = &l.host().game;
            (
                sim.sim_client(d2_client::bridge::LOCAL_CLIENT).is_some(),
                single_player::local_player(sim),
            )
        })
        .unwrap();
    assert_eq!(before, (true, None), "the character loads at the join");

    link.send(SendQueue::System, &[0x6B]).unwrap();
    ms.fetch_add(40, Ordering::SeqCst);
    assert!(link.pump().unwrap().ticked);
    let got = ids(link.receive());
    assert_eq!(got.first(), Some(&0x59), "{got:02X?}");
    assert_eq!(got.last(), Some(&0x04), "{got:02X?}");
    let (class, fields, knows, faults, log) = link
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).expect("joined");
            let class = sim.events.sys.units.get(p).map(|u| u.class);
            let fields = sim.player_fields(p).is_some();
            let faults = sim.session().map(|f| f.faults.len());
            let h = sim.events.hooks();
            let knows = h
                .waypoints
                .get_mut(&p)
                .map(|r| r.get_mut(0).test(1).unwrap_or(false));
            (class, fields, knows, faults, h.x.log.clone())
        })
        .unwrap();
    assert_eq!(class, Some(single_player::PLAYER_CLASS));
    assert!(fields);
    assert_eq!(
        knows,
        Some(true),
        "the synthetic Cold Plains waypoint (index 1)"
    );
    assert_eq!(faults, Some(0));
    assert!(log.is_empty(), "{log:?}");
}

/// M08 for the test above: the same 0x67 without flag bits 1 and 2 is
/// refused by the server's checks (§2.5): no client record, nothing
/// sent, a 0x6B after it finds no record and no player is loaded.
// Covers: specs/sim/intents-events.md §2.5, §8.2 r1
#[test]
fn a_refused_create_request_starts_nothing() {
    use d2_client::bridge::link::SendQueue;
    use std::sync::atomic::{AtomicU32, Ordering};

    let ms = std::sync::Arc::new(AtomicU32::new(1000));
    let (mut link, _) =
        single_player::start(GameData::Synthetic, DEFAULT_SEED, StepClock(ms.clone())).unwrap();
    let mut req = single_player::create_request();
    req.flags &= !0x6;
    link.send(SendQueue::System, &req.encode()).unwrap();
    link.pump().unwrap();
    ms.fetch_add(40, Ordering::SeqCst);
    link.pump().unwrap();
    link.send(SendQueue::System, &[0x6B]).unwrap();
    ms.fetch_add(40, Ordering::SeqCst);
    link.pump().unwrap();
    assert!(link.receive().is_empty());
    let (client, player, faults) = link
        .with(|l| {
            let sim = &l.host().game;
            (
                sim.sim_client(d2_client::bridge::LOCAL_CLIENT).is_some(),
                single_player::local_player(sim),
                sim.session().map(|f| f.faults.len()),
            )
        })
        .unwrap();
    assert_eq!((client, player, faults), (false, None, Some(2)));
}

/// `waypoints.md` §5.1 rule 1 on the user's own tables: the chosen
/// waypoint object has operate function 23 and init function 17, and no
/// earlier row has both.
#[test]
#[ignore = "needs the game files in D2_GAME_DIR"]
fn live_tables_give_a_waypoint_object() {
    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR");
    let archives = d2_formats::mpq::ArchiveSet::open_dir(&dir).unwrap();
    let t = WaypointTables::live(&archives).unwrap();
    let class = t.object_class as usize;
    assert_eq!(
        (t.objects[class].operatefn, t.objects[class].initfn),
        (23, 17)
    );
    assert!(t.objects[..class]
        .iter()
        .all(|o| !(o.operatefn == 23 && o.initfn == 17)));
}

/// The client skill rows (`client/msg-skills.md` Inputs) from the user's
/// `skills` table: one per row, in row order, with `maxlvl`, `anim`,
/// `monanim`, `passivestate` read as the typed record has them.
/// `D2_GAME_DIR=<install> cargo test -p d2-client --test app_single_player -- --ignored client_skill_rows`
// Covers: specs/client/msg-skills.md §2 r1, §2 r4
#[test]
#[ignore = "needs the game files in D2_GAME_DIR"]
fn client_skill_rows_from_the_install() {
    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR");
    let archives = d2_formats::mpq::ArchiveSet::open_dir(&dir).unwrap();
    let rows = single_player::client_skill_rows(&archives).unwrap();
    let set = d2_data::bin::load(&archives, "eng").unwrap();
    let skills: Vec<d2_data::tables::Skills> =
        d2_data::tables::decode_all(set.table("skills").unwrap()).unwrap();
    assert_eq!(rows.len(), skills.len());
    for (r, s) in rows.iter().zip(&skills) {
        assert_eq!(
            (r.anim, r.monanim, r.passivestate, r.maxlvl),
            (s.anim, s.monanim, s.passivestate, s.maxlvl)
        );
    }
    let passive = rows.iter().filter(|r| r.passivestate as i16 > 0).count();
    eprintln!("{} skills rows, {passive} with a passive state", rows.len());
}

/// The game on the user's files (`GameData::select`): levels generated
/// through drlg-data's providers and the level-type dispatcher. Act 0
/// holds the town (level 1, generated at act creation) and Cold Plains,
/// act 1 Lut Gholein (its town), each with rooms; the waypoint world is
/// set.
#[test]
#[ignore = "needs the game files in D2_GAME_DIR"]
fn live_data_generates_the_levels_from_the_users_files() {
    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR");
    let data = GameData::select(Some(dir.as_ref()), false).unwrap();
    assert!(matches!(data, GameData::Live(_)));
    let mut g = single_player::build(&data, DEFAULT_SEED).unwrap();
    assert!(g.sim.world.waypoints.is_some());
    let dungeon = &g.sim.events.hooks().drlg.dungeon;
    for (act, level) in [(0, 1), (0, COLD_PLAINS), (1, ACT2_TOWN)] {
        let d = dungeon.acts[act].as_ref().expect("act created");
        let l = d.find_level(level).expect("level allocated");
        let rooms = d.level_rooms(l).len();
        assert!(rooms > 0, "level {level}: no rooms");
        eprintln!(
            "act {act} level {level}: {rooms} rooms, rect {:?}",
            d.level(l).rect
        );
    }
}

/// Without a game directory, or with `--synthetic`, the data is synthetic;
/// a directory that does not load is an error, never a fallback.
#[test]
fn data_selection_falls_back_only_without_game_files() {
    assert!(matches!(
        GameData::select(None, false).unwrap(),
        GameData::Synthetic
    ));
    let missing = std::path::Path::new("/nonexistent/d2rs-no-game-dir");
    assert!(matches!(
        GameData::select(Some(missing), true).unwrap(),
        GameData::Synthetic
    ));
    let e = GameData::select(Some(missing), false).unwrap_err();
    assert!(e.to_string().contains("d2rs-no-game-dir"), "{e}");
}
