// Spec: specs/client/bridge.md (§3), specs/world/waypoints.md (§5.1)
//! The app's single-player game and its server thread: the link is
//! `Send + Sync` (the game is not, and never leaves its thread), builder
//! failures are reported, the build is deterministic, and the live tables
//! (with `D2_GAME_DIR`, ignored by default) give a waypoint object.

use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData, Link, WaypointTables, DEFAULT_SEED};
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
        (g.player, g.waypoint, g.waypoint_guid)
    };
    assert_eq!(units(DEFAULT_SEED), units(DEFAULT_SEED));

    let (mut link, started) =
        single_player::start(GameData::Synthetic, DEFAULT_SEED, SystemClock::default()).unwrap();
    assert_eq!(link.protocol_version(), PROTOCOL_VERSION);
    let (player, guid) = (started.player, started.waypoint_guid);
    // The game behind the link is the one built: its waypoint unit has the
    // GUID the start reported, and the local client is joined.
    let seen = link
        .with(move |l| {
            let sim = &l.host().game;
            (
                sim.game.lists.unit(started.waypoint).map(|u| u.guid),
                sim.sim_client(d2_client::bridge::LOCAL_CLIENT).is_some(),
                sim.player_fields(player).is_some(),
            )
        })
        .unwrap();
    assert_eq!(seen, (Some(guid), true, true));
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
    let g = single_player::build(&GameData::Live(t), DEFAULT_SEED).unwrap();
    assert!(g.sim.world.waypoints.is_some());
}
