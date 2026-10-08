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
use d2_proto::server::LoadAct;
use d2_proto::{FixedMessage, PROTOCOL_VERSION};
use d2_server::adapters::ProtoSizes;
use d2_server::host::{Host, SystemClock};
use d2_sim::rng::Seed;

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
// Covers: specs/sim/intents-events.md §8.1, §8.2 r2, §8.2 r3, §8.2 r7
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
    let chunks = link.receive();
    // S→C 0x03 carries game +0x80, the object control's `dwObjSeed`
    // (`intents-events.md` §8.2, `rng.md` §5.2).
    let load = chunks
        .iter()
        .find(|c| c[0] == LoadAct::ID)
        .map(|c| LoadAct::decode(c).unwrap())
        .expect("0x03 at the join");
    let obj_seed = link
        .with(|l| {
            l.host_mut()
                .game
                .events
                .action
                .hooks()
                .objects
                .as_ref()
                .map(|o| o.obj_seed)
        })
        .unwrap();
    assert_eq!(Some(load.f8), obj_seed);
    assert_ne!(load.f8, 0);
    let got = ids(chunks);
    assert_eq!(got.first(), Some(&0x59), "{got:02X?}");
    assert_eq!(got.last(), Some(&0x04), "{got:02X?}");
    // A new character has its player record (§8.2 rule 7): 0x5F after
    // 0x0B and the two 0x23 (no `StartSkill` without the vitals tables, so
    // no load 0x23).
    assert!(got.windows(2).any(|w| w == [0x0B, 0x5F]), "{got:02X?}");
    assert_eq!(got.iter().filter(|&&i| i == 0x23).count(), 2, "{got:02X?}");
    let (class, fields, knows, faults, log) = link
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).expect("joined");
            let class = sim.events.action.sys.units.get(p).map(|u| u.class);
            let fields = sim.player_fields(p).is_some();
            let faults = sim.session().map(|f| f.faults.len());
            let h = sim.events.action.hooks();
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
    // The new character is the stub load (`intents-events.md` §8.2 rule
    // 7, `formats/d2s-load.md` §1): its steps without a provider in the
    // synthetic game are named, nothing else is logged.
    let steps: Vec<&str> = log
        .iter()
        .map(|l| {
            l.strip_prefix("join: new character: Unapplied { step: \"")
                .and_then(|r| r.split('"').next())
                .unwrap_or(l)
        })
        .collect();
    assert_eq!(
        steps,
        [
            // No `charstats` rows: `client/msg-skills.md` §2 r8's Skill 1–10.
            "player skills",
            "new character set-up",
            "start stats",
            "start items",
            "start skill",
            "mouse skills",
            "quest entry"
        ]
    );
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
    assert!(g.sim.world.action.waypoints.is_some());
    let dungeon = &g.sim.events.action.hooks().drlg.dungeon;
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

/// Game creation at build (`rng.md` §5.2): on the `--seed` game seed
/// `{N, 666}` (unstepped, the fixed-seed branch), the regions, the object
/// control (`dwObjSeed` = the second step's lo', `objects.md` §2 rule 2),
/// the NPC control and the quest control each take one step, before any
/// unit; then the waypoint object's allocation takes one (§5.3). The
/// controls are the wired host's. The synthetic game has no drop state
/// and no hireling tables, and one unique-bit store (the hooks').
// Covers: specs/sim/rng.md §5.2, §5.3; specs/world/objects.md §2
#[test]
fn game_creation_derives_the_four_controls_in_order_before_the_first_unit() {
    let mut g = single_player::build(&GameData::Synthetic, DEFAULT_SEED).unwrap();
    let mut want = Seed::init_low(DEFAULT_SEED);
    want.step();
    let obj_seed = want.step();
    want.step();
    want.step();
    want.step(); // the waypoint object's allocation
    want.step(); // Akara (the synthetic town NPC, d2rs-own)
                 // The Moldy Tome in the Black Marsh (q-a1-tower, d2rs-own): one
                 // more object allocation, one more step (`rng.md` §5.3).
    want.step();
    let h = g.sim.events.action.hooks();
    assert_eq!(h.game_seed, want);
    assert_eq!(h.objects.as_ref().map(|o| o.obj_seed), Some(obj_seed));
    assert!(h.object_drops.is_none());
    assert_eq!(h.uniques, Default::default());
    let w = &g.sim.world;
    assert!(w.quests.record(1).is_some(), "the quest control's records");
    assert!(w.state.hireling_tables.is_none());
    // M08: another seed gives another object seed.
    let mut other = single_player::build(&GameData::Synthetic, DEFAULT_SEED + 1).unwrap();
    let o = other
        .sim
        .events
        .action
        .hooks()
        .objects
        .as_ref()
        .unwrap()
        .obj_seed;
    assert_ne!(o, obj_seed);
}

/// `--save` needs the user's tables: the synthetic data refuses it
/// (no fallback), and the character name of a save is its header's
/// (`formats/d2s.md` §2.1: +0x14, up to the NUL; +0x23 not read).
// Covers: specs/formats/d2s.md §2.1, §2.2 r4
#[test]
fn a_save_needs_the_users_tables_and_names_its_character() {
    let e = single_player::load_character(&GameData::Synthetic, "x.d2s".as_ref(), 0).unwrap_err();
    assert!(e.to_string().contains("D2_GAME_DIR"), "{e}");
    let mut b = vec![0u8; 0x14F];
    b[0x14..0x18].copy_from_slice(b"Kara");
    assert_eq!(single_player::save_name(&b), b"Kara");
    b[0x14..0x23].fill(b'a');
    b[0x23] = b'z';
    assert_eq!(single_player::save_name(&b), &[b'a'; 15][..]);
    assert_eq!(single_player::save_name(&b[..0x20]), b"");
}

/// The game on the user's files: game creation's NPC control holds the
/// `interact` NPCs' records, the chest drop's state and the hireling
/// tables are installed, and the join runs.
// Covers: specs/sim/rng.md §5.2; specs/world/npc.md §1.1; specs/items/treasure.md §4; specs/world/hirelings.md §10 r2
#[test]
#[ignore = "needs the game files in D2_GAME_DIR"]
fn live_game_creation_installs_the_drops_and_the_hireling_tables() {
    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR");
    let data = GameData::select(Some(dir.as_ref()), false).unwrap();
    let mut g = single_player::build(&data, DEFAULT_SEED).unwrap();
    let h = g.sim.events.action.hooks();
    assert!(h.object_drops.is_some());
    assert!(h.objects.is_some());
    let w = &g.sim.world;
    assert!(w.state.hireling_tables.is_some());
    assert!(!w.npc.records.is_empty(), "the interact NPCs' records");
    assert_eq!(w.state.vendors.len(), w.npc.records.len());
}

/// `--save`: a save read with the user's tables joins (the class and
/// name of its header in the 0x67, the load at the 0x6B). The save is
/// `$D2_SAVE` (an expansion, softcore character saved on Normal).
// Covers: specs/formats/d2s.md §1, §2.2, §9; specs/sim/intents-events.md §8.2 r2
#[test]
#[ignore = "needs the game files in D2_GAME_DIR and a save in D2_SAVE"]
fn a_save_from_the_command_line_joins() {
    use d2_client::bridge::link::SendQueue;
    use std::sync::atomic::{AtomicU32, Ordering};

    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR");
    let save = std::env::var("D2_SAVE").expect("D2_SAVE");
    let data = GameData::select(Some(dir.as_ref()), false).unwrap();
    let character = single_player::load_character(&data, save.as_ref(), 0).unwrap();
    let req = single_player::create_request_for(&character);
    let ms = std::sync::Arc::new(AtomicU32::new(1000));
    let (mut link, _) =
        single_player::start_with(data, DEFAULT_SEED, character, StepClock(ms.clone())).unwrap();
    link.send(SendQueue::System, &req.encode()).unwrap();
    link.pump().unwrap();
    ms.fetch_add(40, Ordering::SeqCst);
    link.pump().unwrap();
    link.receive();
    link.send(SendQueue::System, &[0x6B]).unwrap();
    ms.fetch_add(40, Ordering::SeqCst);
    link.pump().unwrap();
    let (player, faults, log) = link
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let p = single_player::local_player(sim);
            let faults = sim.session().map(|f| format!("{:?}", f.faults));
            (p, faults, sim.events.action.hooks().x.log.clone())
        })
        .unwrap();
    eprintln!("load log: {log:#?}");
    assert!(player.is_some(), "joined; faults {faults:?}");
}

/// The app's C→S 0x67 bytes (`client/model.md` §7 rule 9): the recorded
/// single-player layout for an expansion character; a classic save sends
/// bit 2 alone (PROVISIONAL there, REC-46).
// Covers: specs/client/model.md §7 r9
#[test]
fn the_create_request_has_the_builder_layout() {
    use d2_client::app::single_player::{Character, CREATE_FLAGS_CLASSIC};
    use d2_server::adapters::character::LoadContext;
    let b = single_player::create_request().encode();
    assert_eq!(b.len(), 46);
    assert_eq!(b[0], 0x67);
    assert_eq!(b[1..0x11], [0; 16], "empty game name");
    assert_eq!(b[0x11], 3, "game type 3");
    assert_eq!(b[0x12], single_player::PLAYER_CLASS as u8);
    assert_eq!((b[0x13], b[0x14]), (0, 0), "template, Normal");
    let mut name = [0u8; 16];
    name[..single_player::PLAYER_NAME.len()].copy_from_slice(single_player::PLAYER_NAME);
    assert_eq!(b[0x15..0x25], name);
    assert_eq!(b[0x25..0x27], [0, 0]);
    assert_eq!(b[0x27..0x2B], 0x0010_0004u32.to_le_bytes());
    assert_eq!(b[0x2B..0x2E], [0, 0, 0]);
    // A save's class and name; its status bit 5 picks the flags.
    for (status, flags) in [(0x20u16, 0x0010_0004u32), (0, CREATE_FLAGS_CLASSIC)] {
        let save = d2_formats::d2s::D2s::new_stub(b"Necro", 2, status, 1).unwrap();
        let r = single_player::create_request_for(&Character::Save(
            Box::new(save),
            LoadContext::default(),
        ));
        assert_eq!((r.class, &r.char_name[..6]), (2, &b"Necro\0"[..]));
        assert_eq!(r.flags, flags, "status {status:#x}");
    }
}
