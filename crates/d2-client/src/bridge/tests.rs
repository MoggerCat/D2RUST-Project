// Spec: specs/client/bridge.md
//! Test vectors of the spec, without a window or GPU. Handlers registered
//! here are synthetic: they test the mechanism, not any message's meaning.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_proto::client::{Walk, WalkToUnit};
use d2_proto::schema::FieldType;
use d2_proto::transport::{Classified, SplitError};
use d2_proto::{CLIENT_MESSAGES, PROTOCOL_VERSION};

use super::dispatch::{self, Dispatch, Handle, HandlerError, Message, Mismatch, Row};
use super::intent::IntentError;
use super::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use super::mirror::{BridgePlugin, BridgeResource, MirrorIndex, UnitView};
use super::world::{addressed_unit, ClientUnit, ClientWorld, UnitKey};
use super::{Bridge, BridgeError};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Send(SendQueue, Vec<u8>),
    Pump,
}

/// A scripted server: records sends and pumps, replays ticks and chunks.
#[derive(Default)]
struct Script {
    version: u32,
    events: Vec<Event>,
    ticks: VecDeque<bool>,
    deliveries: VecDeque<Vec<Vec<u8>>>,
}

#[derive(Clone)]
struct ScriptedLink(Arc<Mutex<Script>>);

impl ScriptedLink {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Script {
            version: PROTOCOL_VERSION,
            ..Script::default()
        })))
    }

    fn script(&self) -> std::sync::MutexGuard<'_, Script> {
        self.0.lock().unwrap()
    }

    /// Queues one pump's result and the chunks delivered after it.
    fn deliver(&self, ticked: bool, chunks: &[&[u8]]) {
        let mut s = self.script();
        s.ticks.push_back(ticked);
        s.deliveries
            .push_back(chunks.iter().map(|c| c.to_vec()).collect());
    }
}

impl ServerLink for ScriptedLink {
    fn protocol_version(&self) -> u32 {
        self.script().version
    }

    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.script().events.push(Event::Send(queue, msg.to_vec()));
        Ok(Sent::Queued)
    }

    fn pump(&mut self) -> Result<Pumped, LinkError> {
        let mut s = self.script();
        s.events.push(Event::Pump);
        Ok(Pumped {
            ticked: s.ticks.pop_front().unwrap_or(false),
        })
    }

    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.script().deliveries.pop_front().unwrap_or_default()
    }
}

fn bridge() -> (Bridge<ScriptedLink>, ScriptedLink) {
    let link = ScriptedLink::new();
    (Bridge::new(link.clone()).unwrap(), link)
}

fn sent(link: &ScriptedLink) -> Vec<(SendQueue, Vec<u8>)> {
    link.script()
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Send(q, b) => Some((*q, b.clone())),
            Event::Pump => None,
        })
        .collect()
}

/// The bytes of a C→S message built from its `d2-proto` layout: each
/// named field's value little-endian at its offset.
fn from_layout(id: u8, size: usize, values: &[(&str, u32)]) -> Vec<u8> {
    let desc = &CLIENT_MESSAGES[id as usize];
    let mut b = vec![0; size];
    b[0] = id;
    for &(name, v) in values {
        let f = desc.layout.iter().find(|f| f.name == name).unwrap();
        let off = f.offset.unwrap() as usize;
        let n = match f.ty {
            FieldType::U8 => 1,
            FieldType::U16 => 2,
            FieldType::U32 => 4,
            other => panic!("unexpected field type {other:?}"),
        };
        b[off..off + n].copy_from_slice(&v.to_le_bytes()[..n]);
    }
    b
}

// Covers: specs/client/bridge.md §4 r1, §4 r2
#[test]
fn intent_bytes_match_layouts() {
    let (mut b, link) = bridge();
    b.send(&Walk {
        x: 0x1234,
        y: 0x5678,
    })
    .unwrap();
    b.send(&WalkToUnit {
        type_: 1,
        id: 0xAABB_CCDD,
    })
    .unwrap();
    let walk = vec![0x01, 0x34, 0x12, 0x78, 0x56];
    let to_unit = vec![0x02, 1, 0, 0, 0, 0xDD, 0xCC, 0xBB, 0xAA];
    assert_eq!(walk, from_layout(0x01, 5, &[("x", 0x1234), ("y", 0x5678)]));
    assert_eq!(
        to_unit,
        from_layout(0x02, 9, &[("type", 1), ("id", 0xAABB_CCDD)])
    );
    assert_eq!(
        sent(&link),
        vec![(SendQueue::Game, walk), (SendQueue::Game, to_unit)]
    );
}

// Covers: specs/client/bridge.md §4 r2, §4 r3
#[test]
fn intent_routing_and_refusals() {
    let (mut b, link) = bridge();
    assert_eq!(b.send_bytes(&[0x6B]).unwrap(), Sent::Queued);
    let refused = |r: Result<Sent, BridgeError>| match r {
        Err(BridgeError::Intent(e)) => e,
        other => panic!("expected an intent error, got {other:?}"),
    };
    assert_eq!(
        refused(b.send_bytes(&[0x80; 5])),
        IntentError::NotSendable(Classified::Invalid)
    );
    assert_eq!(
        refused(b.send_bytes(&[0x01, 0, 0])),
        IntentError::NotSendable(Classified::Incomplete)
    );
    assert_eq!(refused(b.send_bytes(&[0xFF; 16])), IntentError::AdminQueue);
    // Chat with a large trailing byte: the size rule is negative.
    let chat = [&[0x15, 0x01, 0x00][..], b"hi\0bob\0", &[0x80]].concat();
    assert_eq!(
        refused(b.send_bytes(&chat)),
        IntentError::NotSendable(Classified::NegativeSize(11 - 128))
    );
    // Only the system message reached the link.
    assert_eq!(sent(&link), vec![(SendQueue::System, vec![0x6B])]);
}

// Covers: specs/client/bridge.md §4 r3
#[test]
fn game_send_limit() {
    // 0x66 (warden response): u16 at +1 plus 3, capped at 0x1FD.
    let mut big = vec![0x66, 0xFD, 0x01];
    big.resize(0x200, 0);
    assert_eq!(
        super::intent::route(&big),
        Err(IntentError::GameTooLarge(0x200))
    );
    let mut ok = vec![0x66, 0xFC, 0x01];
    ok.resize(0x1FF, 0);
    assert_eq!(super::intent::route(&ok), Ok(SendQueue::Game));
}

// Covers: specs/client/bridge.md §2 r2, §6 r3
#[test]
fn split_and_unowned() {
    let (mut b, _) = bridge();
    let r = b.receive_chunk(&[0x61, 0x07, 0x5F, 1, 2, 3, 4]).unwrap();
    assert_eq!((r.messages, r.unowned, r.handled), (2, 2, 0));
    let log = b.log();
    assert_eq!(log.unowned.get(&0x61), Some(&1));
    assert_eq!(log.unowned.get(&0x5F), Some(&1));
    assert!(log.discarded.is_empty());
    assert_eq!(b.world(), &ClientWorld::default());
}

// Covers: specs/client/bridge.md §2 r3
#[test]
fn unknown_id_ends_split() {
    let (mut b, _) = bridge();
    let r = b.receive_chunk(&[0x61, 0x07, 0x80, 0x61, 0x07]).unwrap();
    assert_eq!((r.messages, r.discarded_bytes), (1, 3));
    assert_eq!(
        b.log().discarded,
        vec![super::receive::Discarded {
            first: 0x80,
            bytes: 3
        }]
    );
}

// Covers: specs/client/bridge.md §2 r4
#[test]
fn fatal_chunks_are_refused_whole() {
    let (mut b, _) = bridge();
    match b.receive_chunk(&[0x61, 0x07, 0x5F, 1]) {
        Err(BridgeError::Split(SplitError::Truncated { at: 2, size: 5 })) => {}
        other => panic!("{other:?}"),
    }
    let mut big = vec![0x16, 0x05, 0x02];
    big.resize(13, 0);
    match b.receive_chunk(&big) {
        Err(BridgeError::Split(SplitError::TooLarge { at: 0, size: 0x205 })) => {}
        other => panic!("{other:?}"),
    }
    // Nothing of either chunk was dispatched.
    assert!(b.log().unowned.is_empty());
}

// Covers: specs/client/bridge.md §5 r4
#[test]
fn addressed_units() {
    let mut m6d = vec![0x6D, 7, 0, 0, 0];
    m6d.resize(10, 0);
    assert_eq!(
        addressed_unit(&m6d),
        Some(UnitKey {
            unit_type: 1,
            guid: 7
        })
    );
    let mut m0e = vec![0x0E, 2, 9, 0, 0, 0];
    m0e.resize(12, 0);
    assert_eq!(
        addressed_unit(&m0e),
        Some(UnitKey {
            unit_type: 2,
            guid: 9
        })
    );
    assert_eq!(addressed_unit(&[0x6E]), None);
    assert_eq!(addressed_unit(&[0x61, 0x07]), None);
}

// Synthetic handlers: add / remove the addressed unit.
fn add_unit(world: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let key = msg.unit.ok_or(HandlerError::Invalid("no unit"))?;
    world.units.insert(key, ClientUnit::new(key));
    Ok(())
}

fn remove_unit(world: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let key = msg.unit.ok_or(HandlerError::Invalid("no unit"))?;
    world.units.remove(&key);
    Ok(())
}

fn refuse(_: &mut ClientWorld, _: &Message<'_>) -> Result<(), HandlerError> {
    Err(HandlerError::Invalid("synthetic refusal"))
}

fn test_dispatch() -> Dispatch {
    let mut d = Dispatch::empty();
    d.set(0x6D, "test", add_unit);
    d.set(0x0E, "test", add_unit);
    d.set(0x0F, "test", remove_unit);
    d.set(0x61, "test", refuse);
    d
}

fn msg(id: u8, size: usize, unit_type: u8, guid: u32) -> Vec<u8> {
    let mut b = vec![id];
    if (0x67..=0x6D).contains(&id) {
        b.extend(guid.to_le_bytes());
    } else {
        b.push(unit_type);
        b.extend(guid.to_le_bytes());
    }
    b.resize(size, 0);
    b
}

// Covers: specs/client/bridge.md §2 r2, §5 r4, §6 r4
#[test]
fn world_updates_from_messages() {
    let link = ScriptedLink::new();
    let mut b = Bridge::with_dispatch(link.clone(), test_dispatch()).unwrap();
    let chunk = [
        msg(0x6D, 10, 1, 7),
        msg(0x0E, 12, 2, 9),
        vec![0x61, 0x07],
        msg(0x0F, 16, 2, 9),
        msg(0x0E, 12, 4, 3),
    ]
    .concat();
    let r = b.receive_chunk(&chunk).unwrap();
    assert_eq!((r.messages, r.handled, r.rejected), (5, 4, 1));
    let keys: Vec<_> = b.world().units.keys().copied().collect();
    let key = |unit_type, guid| UnitKey { unit_type, guid };
    assert_eq!(keys, vec![key(1, 7), key(4, 3)]);
    assert_eq!(b.log().rejected.len(), 1);
    assert_eq!(b.log().rejected[0].id, 0x61);
}

// Covers: specs/client/bridge.md §4 r4, §5 r3, §8 r1, §8 r3
#[test]
fn frame_order_and_counters() {
    let (mut b, link) = bridge();
    link.deliver(true, &[&[0x61, 0x07]]);
    link.deliver(false, &[]);
    let r1 = b.frame().unwrap();
    assert!(r1.ticked);
    assert_eq!((r1.chunks, r1.messages), (1, 1));
    b.send(&Walk { x: 1, y: 2 }).unwrap();
    let r2 = b.frame().unwrap();
    assert!(!r2.ticked);
    assert_eq!((b.world().frames, b.world().server_ticks), (2, 1));
    // The intent of frame 1 reaches the link before pump 2.
    assert_eq!(
        link.script().events,
        vec![
            Event::Pump,
            Event::Send(SendQueue::Game, vec![0x01, 1, 0, 2, 0]),
            Event::Pump,
        ]
    );
}

// Covers: specs/client/bridge.md §9 r1
#[test]
fn version_mismatch_is_refused() {
    let link = ScriptedLink::new();
    link.script().version = PROTOCOL_VERSION + 1;
    match Bridge::new(link) {
        Err(BridgeError::Version { client, server }) => {
            assert_eq!((client, server), (PROTOCOL_VERSION, PROTOCOL_VERSION + 1));
        }
        Err(e) => panic!("{e:?}"),
        Ok(_) => panic!("accepted a link of another version"),
    }
}

// Covers: specs/client/bridge.md §6 r1, §6 r5
#[test]
fn dispatch_table_matches_spec() {
    let rows = dispatch::parse(dispatch::TSV).unwrap();
    assert_eq!(rows.len(), dispatch::IDS);
    assert_eq!(dispatch::check(&rows, dispatch::HANDLERS), vec![]);
    assert!(Dispatch::from_spec().is_ok());
}

/// METHODS M08: a changed row is reported, and only that row.
// Covers: specs/client/bridge.md §6 r5
#[test]
fn dispatch_check_catches_perturbations() {
    let rows = dispatch::parse(dispatch::TSV).unwrap();
    let mut owned = rows.clone();
    owned[0x61].owner = Some("specs/client/x.md".into());
    assert_eq!(
        dispatch::check(&owned, dispatch::HANDLERS),
        vec![Mismatch::NoHandler { id: 0x61 }]
    );
    let mut moved = rows.clone();
    moved[0x1A].owner = Some("specs/client/x.md".into());
    assert_eq!(
        dispatch::check(&moved, dispatch::HANDLERS),
        vec![Mismatch::Owner {
            id: 0x1A,
            tsv: "specs/client/x.md".into(),
            code: "specs/client/msg-stats-items.md".into(),
        }]
    );
    let mut renamed = rows.clone();
    renamed[0x5F].name = "Renamed".into();
    assert_eq!(
        dispatch::check(&renamed, dispatch::HANDLERS),
        vec![Mismatch::Name {
            id: 0x5F,
            tsv: "Renamed".into(),
            proto: rows[0x5F].name.clone(),
        }]
    );
    let tbd: Vec<Row> = rows
        .iter()
        .map(|r| Row {
            owner: None,
            ..r.clone()
        })
        .collect();
    let handler = [dispatch::Handler {
        id: 0x5F,
        owner: "specs/client/y.md",
        handle: Handle::General(add_unit),
    }];
    assert_eq!(
        dispatch::check(&tbd, &handler),
        vec![Mismatch::Unowned { id: 0x5F }]
    );
    let mut both: Vec<Row> = tbd.clone();
    both[0x5F].owner = Some("specs/client/z.md".into());
    assert_eq!(
        dispatch::check(&both, &handler),
        vec![Mismatch::Owner {
            id: 0x5F,
            tsv: "specs/client/z.md".into(),
            code: "specs/client/y.md".into(),
        }]
    );
    // The handler kind follows the receive table (`model.md` §4 rule 1):
    // a general handler for a unit-handler id, and the reverse.
    let mut kinds: Vec<dispatch::Handler> = dispatch::HANDLERS.to_vec();
    let i = kinds.iter().position(|h| h.id == 0x6D).unwrap();
    kinds[i].handle = Handle::General(add_unit);
    let j = kinds.iter().position(|h| h.id == 0x15).unwrap();
    kinds[j].handle = Handle::Unit(super::msg::units::queued);
    assert_eq!(
        dispatch::check(&rows, &kinds),
        vec![Mismatch::Kind { id: 0x15 }, Mismatch::Kind { id: 0x6D }]
    );
    // Strict parse: a missing row, a wrong id.
    let short: String = dispatch::TSV
        .lines()
        .take(10)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(dispatch::parse(&short), Err(dispatch::TsvError::Rows(9)));
    let swapped = dispatch::TSV.replacen("0x01\t", "0x02\t", 1);
    assert_eq!(
        dispatch::parse(&swapped),
        Err(dispatch::TsvError::Id {
            line: 3,
            expected: 1
        })
    );
}

// Covers: specs/client/bridge.md §7 r1, §7 r3
#[test]
fn bevy_mirror_follows_the_model() {
    let link = ScriptedLink::new();
    link.deliver(
        false,
        &[&[msg(0x6D, 10, 1, 7), msg(0x0E, 12, 2, 9)].concat()],
    );
    link.deliver(true, &[&msg(0x0F, 16, 2, 9)]);
    link.deliver(false, &[]);
    let bridge = Bridge::with_dispatch(Box::new(link.clone()) as _, test_dispatch()).unwrap();
    let mut app = App::new();
    app.add_plugins(BridgePlugin)
        .insert_resource(BridgeResource(bridge));

    let views = |app: &mut App| -> Vec<UnitKey> {
        let mut q = app.world_mut().query::<&UnitView>();
        let mut keys: Vec<_> = q.iter(app.world()).map(|v| v.key).collect();
        keys.sort();
        keys
    };
    let key = |unit_type, guid| UnitKey { unit_type, guid };

    app.update();
    assert_eq!(views(&mut app), vec![key(1, 7), key(2, 9)]);
    let index: Vec<_> = app
        .world()
        .resource::<MirrorIndex>()
        .0
        .keys()
        .copied()
        .collect();
    assert_eq!(index, vec![key(1, 7), key(2, 9)]);

    app.update();
    assert_eq!(views(&mut app), vec![key(1, 7)]);
    app.update();
    assert_eq!(views(&mut app), vec![key(1, 7)]);
    let world = app.world().resource::<BridgeResource>().0.world();
    assert_eq!((world.frames, world.server_ticks), (3, 1));
}

// Gap tests (docs/handoff/gaps-client-formats.md).

thread_local! {
    /// Messages seen by [`record`], in dispatch order (per test thread).
    static SEEN: std::cell::RefCell<Vec<Vec<u8>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Synthetic handler: records the message bytes as the handler got them.
fn record(_: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    SEEN.with(|s| s.borrow_mut().push(msg.bytes.to_vec()));
    Ok(())
}

fn seen() -> Vec<Vec<u8>> {
    SEEN.with(|s| s.borrow().clone())
}

fn recording_dispatch() -> Dispatch {
    let mut d = Dispatch::empty();
    for id in [0x0E, 0x6D, 0x61] {
        d.set(id, "test", record);
    }
    d
}

/// Every line of `src` outside `//` comments.
fn code_lines(src: &str) -> impl Iterator<Item = &str> {
    src.lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .filter(|l| !l.trim().is_empty())
}

// Covers: specs/client/bridge.md §1 r3
#[test]
fn bridge_modules_except_mirror_have_no_bevy_type() {
    let sources = [
        ("mod.rs", include_str!("mod.rs")),
        ("dispatch.rs", include_str!("dispatch.rs")),
        ("intent.rs", include_str!("intent.rs")),
        ("link.rs", include_str!("link.rs")),
        ("local.rs", include_str!("local.rs")),
        ("receive.rs", include_str!("receive.rs")),
        ("world.rs", include_str!("world.rs")),
        ("bits.rs", include_str!("bits.rs")),
        ("check.rs", include_str!("check.rs")),
        ("update.rs", include_str!("update.rs")),
        ("msg/mod.rs", include_str!("msg/mod.rs")),
        ("msg/session.rs", include_str!("msg/session.rs")),
        ("msg/units.rs", include_str!("msg/units.rs")),
        ("msg/stats_items.rs", include_str!("msg/stats_items.rs")),
    ];
    for (name, src) in sources {
        for line in code_lines(src) {
            assert!(
                !line.to_ascii_lowercase().contains("bevy"),
                "{name}: {line}"
            );
        }
    }
    // The mirror is the one that does use Bevy (the scan can fail).
    assert!(code_lines(include_str!("mirror.rs")).any(|l| l.contains("bevy::")));
    // And the plain part runs without a window, GPU or `App`.
    let (mut b, _) = bridge();
    b.send(&Walk { x: 1, y: 1 }).unwrap();
    b.frame().unwrap();
}

// Covers: specs/client/bridge.md §1 r1
#[test]
fn model_changes_only_through_dispatched_messages() {
    let (mut b, link) = bridge();
    // Intents are sent, never predicted: no model change.
    b.send(&Walk { x: 5, y: 6 }).unwrap();
    b.send(&WalkToUnit { type_: 1, id: 7 }).unwrap();
    assert_eq!(b.world(), &ClientWorld::default());
    // Unowned messages (even unit messages) change nothing but the
    // bridge's own counters.
    link.deliver(
        true,
        &[&[msg(0x6D, 10, 1, 7), msg(0x0E, 12, 2, 9)].concat()],
    );
    b.frame().unwrap();
    assert_eq!(
        b.world(),
        &ClientWorld {
            frames: 1,
            server_ticks: 1,
            ..ClientWorld::default()
        }
    );
}

// Covers: specs/client/bridge.md §1 r4
#[test]
fn bytes_cross_the_boundary_unchanged() {
    // Out: typed and raw messages reach the link byte for byte.
    let (mut b, link) = bridge();
    let warden = [0x66, 0x02, 0x00, 0xAB, 0xCD];
    let walk = [0x01, 0x34, 0x12, 0x78, 0x56];
    b.send_bytes(&warden).unwrap();
    b.send_bytes(&walk).unwrap();
    b.send_bytes(&[0x6B]).unwrap();
    assert_eq!(
        sent(&link),
        vec![
            (SendQueue::Game, warden.to_vec()),
            (SendQueue::Game, walk.to_vec()),
            (SendQueue::System, vec![0x6B]),
        ]
    );
    // In: each handler sees exactly its message's bytes of the chunk.
    let mut b = Bridge::with_dispatch(ScriptedLink::new(), recording_dispatch()).unwrap();
    let (m1, m2) = (msg(0x0E, 12, 2, 0xA1B2_C3D4), msg(0x6D, 10, 1, 0x0102_0304));
    let chunk = [m1.clone(), vec![0x61, 0x07], m2.clone()].concat();
    b.receive_chunk(&chunk).unwrap();
    assert_eq!(seen(), vec![m1, vec![0x61, 0x07], m2]);
}

// Covers: specs/client/bridge.md §2 r1
#[test]
fn buffer_and_node_chunks_split_alike() {
    let messages = [msg(0x6D, 10, 1, 7), vec![0x61, 0x07], msg(0x0E, 12, 2, 9)];
    let run = |chunks: &[&[u8]]| {
        let link = ScriptedLink::new();
        link.deliver(false, chunks);
        let mut b = Bridge::with_dispatch(link, test_dispatch()).unwrap();
        let report = b.frame().unwrap();
        (report.messages, b.world().clone(), b.log().rejected.len())
    };
    let buffer = messages.concat();
    let whole = run(&[&buffer]);
    let nodes = run(&[&messages[0], &messages[1], &messages[2]]);
    assert_eq!(whole, nodes);
    assert_eq!((whole.0, whole.1.units.len(), whole.2), (3, 2, 1));
}

// Covers: specs/client/bridge.md §2 r5
#[test]
fn chunks_are_handled_in_delivery_order() {
    let link = ScriptedLink::new();
    let (a, c) = (msg(0x6D, 10, 1, 1), msg(0x0E, 12, 2, 2));
    // Delivered "system list" first, then game list; neither is reordered
    // even though the ids would sort the other way.
    link.deliver(false, &[&[0x61, 0x07], &a, &c]);
    let mut b = Bridge::with_dispatch(link, recording_dispatch()).unwrap();
    assert_eq!(b.frame().unwrap().chunks, 3);
    assert_eq!(seen(), vec![vec![0x61, 0x07], a, c]);
}

// Covers: specs/client/bridge.md §5 r2
#[test]
fn units_iterate_in_type_then_guid_order() {
    let mut b = Bridge::with_dispatch(ScriptedLink::new(), test_dispatch()).unwrap();
    let chunk = [
        msg(0x0E, 12, 4, 1),
        msg(0x6D, 10, 1, 9),
        msg(0x0E, 12, 2, 0),
        msg(0x0E, 12, 1, 0xFFFF_FFFF),
        msg(0x6D, 10, 1, 3),
    ]
    .concat();
    b.receive_chunk(&chunk).unwrap();
    let key = |unit_type, guid| UnitKey { unit_type, guid };
    let keys: Vec<_> = b.world().units.keys().copied().collect();
    assert_eq!(
        keys,
        vec![
            key(1, 3),
            key(1, 9),
            key(1, 0xFFFF_FFFF),
            key(2, 0),
            key(4, 1)
        ]
    );
    assert!(b.world().units.iter().all(|(k, u)| u.key == *k));
}

// Covers: specs/client/bridge.md §6 r2
#[test]
fn owned_rows_are_exactly_the_registered_handlers() {
    let rows = dispatch::parse(dispatch::TSV).unwrap();
    let owned: Vec<(u8, &str)> = rows
        .iter()
        .filter_map(|r| r.owner.as_deref().map(|o| (r.id, o)))
        .collect();
    let registered: Vec<(u8, &str)> = dispatch::HANDLERS.iter().map(|h| (h.id, h.owner)).collect();
    assert_eq!(owned, registered);
    // The three client model specs own 53 ids (0x7A, 0x81: model §14).
    assert_eq!(owned.len(), 53);
    for (_, o) in &owned {
        assert!(
            [
                super::msg::MODEL,
                super::msg::UNITS,
                super::msg::STATS_ITEMS
            ]
            .contains(o),
            "{o}"
        );
    }
    let d = Dispatch::from_spec().unwrap();
    for r in &rows {
        assert_eq!(d.get(r.id).is_some(), r.owner.is_some(), "{:#04X}", r.id);
    }
}

// Covers: specs/client/bridge.md §7 r2
#[test]
fn views_are_overwritten_from_the_model() {
    let link = ScriptedLink::new();
    link.deliver(false, &[&msg(0x6D, 10, 1, 7)]);
    let bridge = Bridge::with_dispatch(Box::new(link) as _, test_dispatch()).unwrap();
    let mut app = App::new();
    app.add_plugins(BridgePlugin)
        .insert_resource(BridgeResource(bridge));
    app.update();
    let key = UnitKey {
        unit_type: 1,
        guid: 7,
    };
    let bogus = UnitKey {
        unit_type: 3,
        guid: 99,
    };
    let mut q = app.world_mut().query::<&mut UnitView>();
    for mut v in q.iter_mut(app.world_mut()) {
        v.key = bogus;
    }
    app.update();
    let mut q = app.world_mut().query::<&UnitView>();
    let views: Vec<_> = q.iter(app.world()).map(|v| v.key).collect();
    assert_eq!(views, vec![key]);
    // Never written back: the model is unchanged.
    let world = app.world().resource::<BridgeResource>().0.world();
    assert_eq!(world.units.keys().copied().collect::<Vec<_>>(), vec![key]);
}

// Covers: specs/client/bridge.md §8 r2
#[test]
fn one_pump_per_frame_whether_or_not_it_ticked() {
    let (mut b, link) = bridge();
    for ticked in [true, true, false, true, false, false] {
        link.deliver(ticked, &[]);
    }
    for _ in 0..6 {
        b.frame().unwrap();
    }
    let pumps = link
        .script()
        .events
        .iter()
        .filter(|e| **e == Event::Pump)
        .count();
    assert_eq!(pumps, 6);
    assert_eq!((b.world().frames, b.world().server_ticks), (6, 3));
}

/// A link whose pump fails.
struct BrokenLink;

impl ServerLink for BrokenLink {
    fn protocol_version(&self) -> u32 {
        PROTOCOL_VERSION
    }
    fn send(&mut self, _: SendQueue, _: &[u8]) -> Result<Sent, LinkError> {
        Ok(Sent::Queued)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        Err(LinkError::Server("server down".into()))
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        Vec::new()
    }
}

// Covers: specs/client/bridge.md §8 r4
#[test]
fn errors_stop_the_frame_records_do_not() {
    let mut broken = Bridge::new(BrokenLink).unwrap();
    assert!(matches!(broken.frame(), Err(BridgeError::Link(_))));

    // A refused chunk is an error; the chunk after it is not processed.
    let link = ScriptedLink::new();
    link.deliver(false, &[&[0x61, 0x07, 0x5F, 1], &[0x61, 0x07]]);
    let mut b = Bridge::with_dispatch(link.clone(), recording_dispatch()).unwrap();
    assert!(matches!(
        b.frame(),
        Err(BridgeError::Split(SplitError::Truncated { .. }))
    ));
    assert!(seen().is_empty());

    // Unowned ids, discarded bytes and handler rejections are recorded,
    // and the frame succeeds.
    link.deliver(false, &[&[0x5F, 1, 2, 3, 4, 0x61, 0x07, 0x80, 0x00]]);
    let mut b = Bridge::with_dispatch(link, test_dispatch()).unwrap();
    let r = b.frame().unwrap();
    assert_eq!((r.unowned, r.rejected, r.discarded_bytes), (1, 1, 2));
}

// Covers: specs/client/bridge.md §edge-cases-original-bugs
#[test]
fn size_zero_ids_end_the_split_and_one_byte_unit_ids_address_nothing() {
    for id in [0x83u8, 0x84, 0x88, 0x80] {
        let (mut b, _) = bridge();
        let r = b.receive_chunk(&[0x61, 0x07, id, 0x61, 0x07]).unwrap();
        assert_eq!((r.messages, r.discarded_bytes), (1, 3), "id {id:#04X}");
        assert_eq!(
            b.log().discarded,
            vec![super::receive::Discarded {
                first: id,
                bytes: 3
            }]
        );
    }
    for id in 0x6Eu8..=0x72 {
        assert!(
            d2_proto::transport::server_message(id)
                .unwrap()
                .client_unit_handler
                .is_some(),
            "{id:#04X} has a unit handler"
        );
        assert_eq!(addressed_unit(&[id]), None);
    }
}

/// The bytes of a hex string.
fn hx(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

// Covers: specs/client/model.md §5 r1, §5 r2, §7 r3
// Covers: specs/client/bridge.md §8 r1
#[test]
fn update_pass_runs_on_ticked_in_game_frames_and_answers_are_sent() {
    let (mut b, link) = bridge();
    let mut join = hx("59 01 00 00 00 01 77 65 72 77 65 72");
    join.resize(0x16, 0);
    join.extend(hx("41 12 c4 11"));
    let obj = hx("51 02 0d 00 00 00 25 00 14 12 c0 11 02 00");
    let state = hx("0e 02 0d 00 00 00 03 00 02 00 00 00");
    let chunk = [
        hx("02"),
        hx("03 00 c4 88 38 10 01 00 61 d1 e0 9f"),
        join,
        hx("0b 00 01 00 00 00"),
        obj,
        state.clone(),
        hx("04"),
    ]
    .concat();
    // Frame 1 does not tick: the 0x0E waits in the object's queue.
    link.deliver(false, &[&chunk]);
    let r = b.frame().unwrap();
    assert_eq!((r.handled, r.queued, r.drained, r.answered), (6, 1, 0, 1));
    let key = UnitKey::new(2, 13);
    assert!(b.world().in_game);
    assert_eq!(b.world().units[&key].queue.len(), 1);
    // The 0x6B answer to 0x02 went through the send path (system queue).
    assert_eq!(sent(&link), vec![(SendQueue::System, vec![0x6B])]);
    assert!(b.world().outgoing.is_empty());
    // Frame 2 ticks: the update pass applies it.
    link.deliver(true, &[]);
    let r = b.frame().unwrap();
    assert_eq!(r.drained, 1);
    let u = &b.world().units[&key];
    assert!(u.queue.is_empty());
    assert_eq!(
        u.last_mode_request.map(|m| (m.code, m.record)),
        Some((3, [0, 2, 0, 0, 0, 0, 0]))
    );
    // Out of game (0x05): a ticked frame drains nothing.
    link.deliver(true, &[&[hx("05"), state].concat()]);
    let r = b.frame().unwrap();
    assert_eq!((r.queued, r.drained), (1, 0));
    assert_eq!(b.world().units[&key].queue.len(), 1);
    assert!(b.log().rejected.is_empty());
}
