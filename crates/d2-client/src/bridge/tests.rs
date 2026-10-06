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

use super::dispatch::{self, Dispatch, HandlerError, Message, Mismatch, Row};
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
    let r = b.receive_chunk(&[0x1A, 0x07, 0x5F, 1, 2, 3, 4]).unwrap();
    assert_eq!((r.messages, r.unowned, r.handled), (2, 2, 0));
    let log = b.log();
    assert_eq!(log.unowned.get(&0x1A), Some(&1));
    assert_eq!(log.unowned.get(&0x5F), Some(&1));
    assert!(log.discarded.is_empty());
    assert_eq!(b.world(), &ClientWorld::default());
}

// Covers: specs/client/bridge.md §2 r3
#[test]
fn unknown_id_ends_split() {
    let (mut b, _) = bridge();
    let r = b.receive_chunk(&[0x1A, 0x07, 0x80, 0x1A, 0x07]).unwrap();
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
    match b.receive_chunk(&[0x1A, 0x07, 0x5F, 1]) {
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
    assert_eq!(addressed_unit(&[0x1A, 0x07]), None);
}

// Synthetic handlers: add / remove the addressed unit.
fn add_unit(world: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let key = msg.unit.ok_or(HandlerError::Invalid("no unit"))?;
    world.units.insert(key, ClientUnit { key });
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
    d.set(0x1A, "test", refuse);
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
        vec![0x1A, 0x07],
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
    assert_eq!(b.log().rejected[0].id, 0x1A);
}

// Covers: specs/client/bridge.md §4 r4, §5 r3, §8 r1, §8 r3
#[test]
fn frame_order_and_counters() {
    let (mut b, link) = bridge();
    link.deliver(true, &[&[0x1A, 0x07]]);
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
    owned[0x1A].owner = Some("specs/client/x.md".into());
    assert_eq!(
        dispatch::check(&owned, dispatch::HANDLERS),
        vec![Mismatch::NoHandler { id: 0x1A }]
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
    let handler = [dispatch::Handler {
        id: 0x0E,
        owner: "specs/client/y.md",
        handle: add_unit,
    }];
    assert_eq!(
        dispatch::check(&rows, &handler),
        vec![Mismatch::Unowned { id: 0x0E }]
    );
    let mut both: Vec<Row> = rows.clone();
    both[0x0E].owner = Some("specs/client/z.md".into());
    assert_eq!(
        dispatch::check(&both, &handler),
        vec![Mismatch::Owner {
            id: 0x0E,
            tsv: "specs/client/z.md".into(),
            code: "specs/client/y.md".into(),
        }]
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
