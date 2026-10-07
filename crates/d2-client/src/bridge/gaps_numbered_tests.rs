// Spec: specs/client/bridge.md
//! One synthetic test per rule unit not covered by `tests.rs` /
//! `local_tests.rs`: only the bridge talks to the link (§1 rule 2), any
//! link serves the bridge unchanged (§3 rule 4), the world model's fields
//! (§5 rule 1), no interpolation or prediction (§7 rule 4), the dispatch
//! TSV is source (§9 rule 2), and nothing bridge-side is written to disk
//! (§9 rule 3).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use d2_proto::client::Walk;
use d2_proto::PROTOCOL_VERSION;

use super::dispatch::{self, Dispatch, HandlerError, Message};
use super::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use super::world::{ClientUnit, ClientWorld, UnitKey};
use super::Bridge;

/// A link that is neither the local host nor a test of `tests.rs`: it
/// stands for "another `ServerLink`" (an online link, §3 rule 4).
#[derive(Default)]
struct OtherLink {
    sent: Vec<(SendQueue, Vec<u8>)>,
    pumps: usize,
    deliveries: VecDeque<(bool, Vec<Vec<u8>>)>,
    current: Vec<Vec<u8>>,
}

impl ServerLink for OtherLink {
    fn protocol_version(&self) -> u32 {
        PROTOCOL_VERSION
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.sent.push((queue, msg.to_vec()));
        Ok(Sent::Queued)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.pumps += 1;
        let (ticked, chunks) = self.deliveries.pop_front().unwrap_or_default();
        self.current = chunks;
        Ok(Pumped { ticked })
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.current)
    }
}

/// A synthetic handler: inserts the addressed unit (mechanism only).
fn insert_unit(world: &mut ClientWorld, m: &Message<'_>) -> Result<(), HandlerError> {
    if let Some(key) = m.unit {
        world.units.insert(key, ClientUnit::new(key));
    }
    Ok(())
}

fn unit_dispatch() -> Dispatch {
    let mut d = Dispatch::empty();
    d.set(0x6D, "test", insert_unit);
    d
}

/// S→C 0x6D (10 bytes): a monster unit message, GUID at +1.
fn monster_msg(guid: u32) -> Vec<u8> {
    let mut m = vec![0u8; 10];
    m[0] = 0x6D;
    m[1..5].copy_from_slice(&guid.to_le_bytes());
    m
}

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every non-test `.rs` file under `dir`, with its code lines (outside
/// `//` comments), in path order.
fn sources(dir: &Path) -> Vec<(PathBuf, Vec<String>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            if !name.ends_with(".rs") || name.contains("tests") {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            let lines = text
                .lines()
                .map(|l| l.split("//").next().unwrap_or("").to_string())
                .filter(|l| !l.trim().is_empty())
                .collect();
            out.push((p, lines));
        }
    }
    out.sort();
    out
}

// Covers: specs/client/bridge.md §1 r2
#[test]
fn only_the_bridge_talks_to_the_server_link() {
    let src = src_dir();
    let bridge = src.join("bridge");
    let mut callers = Vec::new();
    for (path, lines) in sources(&src) {
        if path.starts_with(&bridge) {
            continue;
        }
        // A link adapter (a `ServerLink` that wraps another link, e.g. the
        // server thread) forwards the link calls; it is part of the link.
        let is_link = lines
            .iter()
            .any(|l| l.contains("impl") && l.contains("ServerLink for"));
        for l in &lines {
            if l.contains(".pump()") || l.contains(".receive()") || l.contains("link_mut(") {
                callers.push((path.clone(), is_link, l.trim().to_string()));
            }
        }
    }
    let outside: Vec<_> = callers.iter().filter(|c| !c.1).collect();
    assert!(
        outside.is_empty(),
        "link calls outside the bridge: {outside:?}"
    );
    // The scan sees the adapter's forwarding calls (it can fail).
    assert!(callers.iter().any(|c| c.1), "{callers:?}");
    // Bevy reaches the game through the resource: the mirror's systems
    // hold `BridgeResource` and call the bridge, never a link.
    let mirror = std::fs::read_to_string(bridge.join("mirror.rs")).unwrap();
    assert!(mirror.contains("bridge.0.frame()"));
    assert!(!mirror.contains(".pump()") && !mirror.contains(".receive()"));
}

// Covers: specs/client/bridge.md §3 r4
#[test]
fn any_server_link_serves_the_bridge_unchanged() {
    // Only the local adapter names the in-process server; the bridge
    // itself is written against the trait.
    let bridge = src_dir().join("bridge");
    for (path, lines) in sources(&bridge) {
        let local = path.ends_with("local.rs");
        let names_server = lines.iter().any(|l| l.contains("d2_server"));
        assert_eq!(names_server, local, "{}", path.display());
    }
    // Another link, boxed as the app holds it, runs the same send and
    // receive paths.
    let mut link = OtherLink::default();
    link.deliver(true, vec![monster_msg(7)]);
    let mut b =
        Bridge::with_dispatch(Box::new(link) as Box<dyn ServerLink>, unit_dispatch()).unwrap();
    b.send(&Walk { x: 3, y: 4 }).unwrap();
    let report = b.frame().unwrap();
    assert_eq!((report.ticked, report.handled), (true, 1));
    assert!(b.world().units.contains_key(&UnitKey {
        unit_type: 1,
        guid: 7
    }));
}

impl OtherLink {
    fn deliver(&mut self, ticked: bool, chunks: Vec<Vec<u8>>) {
        self.deliveries.push_back((ticked, chunks));
    }
}

// Covers: specs/client/bridge.md §5 r1
#[test]
fn client_world_holds_only_stated_fields() {
    // Exhaustive destructuring: a new field fails to compile here until
    // an owner spec states it and this test names it.
    // The fields of `client/model.md` §1 rule 1 and `bridge.md` §5 rule 3.
    let ClientWorld {
        frames,
        server_ticks,
        units,
        local_player,
        difficulty,
        expansion,
        ladder,
        game_flags,
        act,
        in_game,
        unloaded,
        exit_requested,
        rooms_in_sight,
        outgoing,
        use_cursor,
        // `client/model.md` §14 rule 5, §11 rules 2 and 4, §12 rules 1–2.
        pets,
        palette_act,
        drlg,
        active_rooms,
        // `sim/unit-order.md` §5 rules 6–8, `render/lighting.md` §6.3,
        // `drlg/rooms.md` §4.6 (last paragraph).
        room_units,
        lights,
        drlg_updates,
        // `render/lighting.md` §9.2 r3–r4; `client/msg-skills.md` §4 r2.
        environment,
        eclipse_pending,
        skill_tree_flag,
        // `render/lighting.md` §9.2 r4.4.
        env_period_cache,
        // `render/lighting.md` §10 r4; `client/msg-units.md` §8 r9;
        // `client/msg-stats-items.md` §5 r6–r7.
        overrides,
        roster,
        roster_inactive,
        weapon_set,
        item_table_ext,
    } = ClientWorld::default();
    assert!(overrides == Default::default() && roster.is_empty() && roster_inactive.is_empty());
    assert!(weapon_set == 0 && item_table_ext.is_empty());
    assert!(pets.is_empty() && palette_act.is_none() && active_rooms.is_none());
    assert!(room_units == Default::default() && lights.is_empty() && drlg_updates == 0);
    assert!(drlg.is_none());
    assert!(environment.is_none() && !eclipse_pending && skill_tree_flag.is_none());
    assert_eq!(env_period_cache, 0);
    assert_eq!((frames, server_ticks, units.len()), (0, 0, 0));
    assert_eq!((local_player, act, use_cursor), (None, None, None));
    assert_eq!((difficulty, expansion, ladder, game_flags), (0, 0, 0, 0));
    assert!(!in_game && !unloaded && !exit_requested);
    assert!(rooms_in_sight.is_empty() && outgoing.is_empty());
    let key = UnitKey {
        unit_type: 1,
        guid: 2,
    };
    // `client/model.md` §1 rule 2.
    let ClientUnit {
        key: k,
        class,
        mode,
        position,
        server_point,
        stats,
        seed,
        queue,
        last_mode_request,
        kind,
        // `client/msg-skills.md` §1 r1; `client/msg-ui.md` §1 r4.
        skills,
        quest_untargetable,
        // `client/msg-ui.md` §16 r4, §17 r2; `client/stat-lists.md` §3.
        flag_2,
        states,
        state_lists,
        // `client/msg-ui.md` §16 r4.3 (open question 10 decided as A).
        turned_toward,
        path_stopped,
        // `client/msg-units.md` §7 r7.2, §1.2 r3–r4.
        direction_of,
        flags_ex,
        room_freed,
    } = ClientUnit::new(key);
    assert!(skills.is_none() && !quest_untargetable);
    assert!(turned_toward.is_none() && !path_stopped);
    assert!(direction_of.is_none() && flags_ex == 0 && !room_freed);
    assert!(flag_2.is_none() && states.is_empty() && state_lists.is_empty());
    assert_eq!(k, key);
    assert_eq!((class, mode, position, server_point), (0, 0, None, (0, 0)));
    assert_eq!(seed, Some((1, 666)));
    assert!(stats.is_empty() && queue.is_empty() && last_mode_request.is_none());
    assert_eq!(kind, super::world::KindData::None);
    // Not game state: the model's module does not use the simulation.
    let world = std::fs::read_to_string(src_dir().join("bridge/world.rs")).unwrap();
    assert!(!world.contains("d2_sim"));
}

// Covers: specs/client/bridge.md §7 r4
#[test]
fn no_interpolation_or_prediction_between_messages() {
    let mut link = OtherLink::default();
    link.deliver(true, vec![monster_msg(7)]);
    for ticked in [false, true, false, true] {
        link.deliver(ticked, vec![]);
    }
    let mut b = Bridge::with_dispatch(link, unit_dispatch()).unwrap();
    b.frame().unwrap();
    let stated = b.world().units.clone();
    // Frames without S→C messages (ticked or not) and sent intents leave
    // every unit as the last message stated it: nothing advances it.
    for _ in 0..4 {
        b.send(&Walk { x: 9, y: 9 }).unwrap();
        b.frame().unwrap();
        assert_eq!(b.world().units, stated);
    }
    assert_eq!((b.world().frames, b.world().server_ticks), (5, 3));
}

// Covers: specs/client/bridge.md §9 r2
#[test]
fn dispatch_tsv_is_source_compiled_in() {
    // The table is the spec file, built into the binary: equal to it byte
    // for byte, read with no file I/O and no version field.
    let on_disk = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs/client/bridge-dispatch.tsv"),
    )
    .unwrap();
    assert_eq!(dispatch::TSV, on_disk);
    assert_eq!(dispatch::TSV.lines().next(), Some("id\tname\towner"));
    let rows = dispatch::parse(dispatch::TSV).unwrap();
    assert_eq!(rows.len(), dispatch::IDS);
    // The dispatch module reads no file at run time.
    let src = std::fs::read_to_string(src_dir().join("bridge/dispatch.rs")).unwrap();
    assert!(!src.contains("std::fs") && !src.contains("File::"));
}

// Covers: specs/client/bridge.md §9 r3
#[test]
fn bridge_writes_nothing_to_disk() {
    // No bridge data is written yet, so no format (and no version field)
    // exists. A writer added here must bring its versioned format (§9
    // rule 3); this scan fails until it is reviewed.
    for (path, lines) in sources(&src_dir().join("bridge")) {
        for l in &lines {
            for needle in ["std::fs", "File::", "OpenOptions", "fs::write", "BufWriter"] {
                assert!(!l.contains(needle), "{}: {l}", path.display());
            }
        }
    }
}
