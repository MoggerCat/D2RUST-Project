// Spec: specs/sim/intents-events.md
//! Gap tests: rules of the spec not yet claimed by other tests.

#[allow(unused_imports)]
use super::*;

use d2_proto::schema::{ProducedBy, Scope};
use d2_proto::{CLIENT_MESSAGES, SERVER_MESSAGES};

use super::fakes::*;
use crate::buffers::ClientBuffers;
use crate::dispatch::{self, Outcome};
use crate::host::{Handled, Host};
use crate::seams::*;
use crate::transport::{Classified, Queue, SendError, ServerQueues, MAX_MESSAGE};

type TestHost = Host<FakeGame, TsvSizes, FakeSession, ManualClock>;

fn host() -> TestHost {
    let mut h = Host::new(
        FakeGame::with_player(0, ALIVE),
        TsvSizes::new(),
        FakeSession::default(),
        ManualClock(1000),
    );
    h.connect(0);
    h
}

fn outcomes(r: &crate::host::FrameReport) -> Vec<Handled> {
    r.messages.iter().map(|m| m.handled).collect()
}

// Covers: specs/sim/intents-events.md §2.1 r3
#[test]
fn local_send_asserts_size_then_classifies() {
    // 0x0052AE50 asserts size <= 0x204, then classifies (rule 4).
    let s = TsvSizes::new();
    let mut q = ServerQueues::new();
    let mut walk = vec![0x01, 100, 0, 100, 0];
    walk.resize(MAX_MESSAGE, 0);
    assert_eq!(q.send(&s, 0, &walk), Ok(Classified::Queued(Queue::Game)));
    walk.push(0);
    assert_eq!(q.send(&s, 0, &walk), Err(SendError::TooLarge(0x205)));
    assert_eq!(q.len(Queue::Game), 1, "the oversized send queues nothing");
    // Past the assert, the classifier decides: a 4-byte 0x01 is dropped.
    assert_eq!(q.send(&s, 0, &walk[..4]), Ok(Classified::Incomplete));
    assert_eq!(q.len(Queue::Game), 1);
}

// Covers: specs/sim/intents-events.md §2.1 r6
#[test]
fn queues_are_fifo_across_clients() {
    let s = TsvSizes::new();
    let mut q = ServerQueues::new();
    let sends: [(ClientId, [u8; 5]); 4] = [
        (2, [0x01, 1, 0, 0, 0]),
        (0, [0x01, 2, 0, 0, 0]),
        (2, [0x03, 3, 0, 0, 0]),
        (1, [0x01, 4, 0, 0, 0]),
    ];
    for (c, m) in &sends {
        q.send(&s, *c, m).unwrap();
    }
    let got: Vec<(ClientId, u8)> = q.drain().iter().map(|d| (d.client, d.msg[1])).collect();
    assert_eq!(got, [(2, 1), (0, 2), (2, 3), (1, 4)]);
    assert!(q.is_empty());
}

// Covers: specs/sim/intents-events.md §2.2 r5, §edge-cases-original-bugs r1
#[test]
fn dispatch_result_has_no_consequence() {
    let mut h = host();
    // 0x01 in 6 bytes: queued (rule size 5 <= 6), refused by the handler
    // size check (3); a far point target (1); then a valid walk.
    h.queues
        .send(&h.sizes, 0, &[0x01, 100, 0, 100, 0, 0])
        .unwrap();
    h.queues.send(&h.sizes, 0, &[0x01, 200, 0, 100, 0]).unwrap();
    h.queues.send(&h.sizes, 0, &[0x01, 101, 0, 100, 0]).unwrap();
    let r = h.frame().unwrap();
    assert_eq!(
        outcomes(&r),
        [
            Handled::Game(Outcome::Dispatched(ResultCode::Malformed)),
            Handled::Game(Outcome::Dispatched(ResultCode::Refused)),
            Handled::Game(Outcome::Dispatched(ResultCode::Done)),
        ]
    );
    // The client is not dropped and nothing is sent for the rejections.
    assert!(h.record(0).is_some());
    assert_eq!(h.game.handled.len(), 1);
    assert!(h.buffers.buffers(0).unwrap().is_empty());
    // Its next message is processed as usual.
    h.clock.0 = 1040;
    h.queues.send(&h.sizes, 0, &[0x01, 102, 0, 100, 0]).unwrap();
    let r = h.frame().unwrap();
    assert_eq!(
        outcomes(&r),
        [Handled::Game(Outcome::Dispatched(ResultCode::Done))]
    );
    assert_eq!(h.game.handled.len(), 2);
}

/// A game whose handler returns a fixed code.
struct Coded {
    inner: FakeGame,
    code: ResultCode,
}

impl Intents for Coded {
    fn player(&self, client: ClientId) -> PlayerLookup {
        self.inner.player(client)
    }
    fn frame(&self) -> i32 {
        self.inner.frame()
    }
    fn point_state(&self, client: ClientId) -> Option<PointState> {
        self.inner.point_state(client)
    }
    fn set_point_accept(&mut self, client: ClientId, frame: i32) {
        self.inner.set_point_accept(client, frame)
    }
    fn queue_resync(&mut self, client: ClientId, out: &mut dyn MessageSink) {
        self.inner.queue_resync(client, out)
    }
    fn unit_target(&self, client: ClientId, unit_type: u32, unit_id: u32) -> UnitTarget {
        self.inner.unit_target(client, unit_type, unit_id)
    }
    fn handle(
        &mut self,
        client: ClientId,
        msg: &[u8],
        size: usize,
        out: &mut dyn MessageSink,
    ) -> ResultCode {
        self.inner.handle(client, msg, size, out);
        self.code
    }
    fn clients(&self) -> Vec<ClientId> {
        self.inner.clients()
    }
}

fn dispatch_with(g: &mut impl Intents, msg: &[u8]) -> ResultCode {
    let s = TsvSizes::new();
    let mut out = ClientBuffers::new();
    out.add_client(0);
    dispatch::dispatch(g, &s, &mut out, 0, ALIVE, msg, msg.len())
}

// Covers: specs/sim/intents-events.md §2.3 r5
#[test]
fn dispatcher_returns_the_handler_result() {
    for code in [
        ResultCode::Done,
        ResultCode::Refused,
        ResultCode::Invalid,
        ResultCode::Malformed,
    ] {
        let mut g = Coded {
            inner: FakeGame::with_player(0, ALIVE),
            code,
        };
        // 0x04 (run to a unit, unit message) and 0x01 (walk, point message).
        g.inner.units.insert((1, 7), UnitTarget::OwnedItem);
        assert_eq!(dispatch_with(&mut g, &[0x04, 1, 0, 0, 0, 7, 0, 0, 0]), code);
        assert_eq!(dispatch_with(&mut g, &[0x01, 100, 0, 100, 0]), code);
        assert_eq!(g.inner.handled.len(), 2);
    }
}

// Covers: specs/sim/intents-events.md §2.3 text
#[test]
fn result_codes_and_handler_arguments() {
    // Codes: 0 done, 1 refused, 2 invalid field, 3 malformed.
    assert_eq!(
        [
            ResultCode::Done as u8,
            ResultCode::Refused as u8,
            ResultCode::Invalid as u8,
            ResultCode::Malformed as u8,
        ],
        [0, 1, 2, 3]
    );
    let mut g = FakeGame::with_player(0, ALIVE);
    g.units.insert((1, 9), UnitTarget::OtherAct);
    // Out of range → 1; bad unit type → 2; wrong act → 2; wrong size → 3.
    assert_eq!(
        dispatch_with(&mut g, &[0x01, 200, 0, 100, 0]),
        ResultCode::Refused
    );
    assert_eq!(
        dispatch_with(&mut g, &[0x04, 6, 0, 0, 0, 9, 0, 0, 0]),
        ResultCode::Invalid
    );
    assert_eq!(
        dispatch_with(&mut g, &[0x04, 1, 0, 0, 0, 9, 0, 0, 0]),
        ResultCode::Invalid
    );
    assert_eq!(
        dispatch_with(&mut g, &[0x04, 1, 0, 0, 0, 9, 0, 0]),
        ResultCode::Malformed
    );
    assert!(g.handled.is_empty());
    // A handler is called with the client's player, the message and its
    // size.
    let walk = [0x01, 100, 0, 100, 0];
    assert_eq!(dispatch_with(&mut g, &walk), ResultCode::Done);
    assert_eq!(g.handled, [(0, walk.to_vec(), 5)]);
}

// Covers: specs/sim/intents-events.md §3.3 r4
#[test]
fn local_delivery_is_immediate() {
    // Single player (local mode 1): a flushed message is returned by the
    // client's pop in the same client frame, without waiting on the clock.
    let mut h = host();
    h.game.tick_out = vec![(0, vec![0x01; 8])];
    h.frame().unwrap();
    h.clock.0 = 1040;
    let r = h.frame().unwrap();
    assert!(r.ticked);
    assert_eq!(h.receive(0), [vec![0x01; 8]]);
    assert!(h.receive(0).is_empty());
}

// Covers: specs/sim/intents-events.md §4 r2
#[test]
fn tick_events_packed_and_flushed_after_the_tick() {
    // Events in production order, packed into 0x200-byte buffers per
    // client, flushed only after the tick.
    let mut h = host();
    let a = vec![0x5A; 40];
    let b = vec![0x9C, 0x00, 200];
    let mut b = b;
    b.resize(200, 0);
    let mut c = vec![0x9C, 0x00, 0xFF];
    c.resize(255, 0);
    let d = vec![0x01; 8];
    h.game.tick_out = vec![
        (0, a.clone()),
        (0, b.clone()),
        (0, c.clone()),
        (0, d.clone()),
    ];
    let r = h.frame().unwrap();
    assert!(!r.ticked);
    assert!(h.receive(0).is_empty());
    h.clock.0 = 1040;
    let r = h.frame().unwrap();
    assert!(r.ticked);
    // 40 + 200 + 255 = 495 fit one buffer; 495 + 8 = 503 too; so one.
    assert_eq!(r.flushed_buffers, 1);
    assert_eq!(h.receive(0), [a.clone(), b.clone(), c.clone(), d.clone()]);
    // One more large event makes a second buffer, order kept.
    let mut e = vec![0x9C, 0x00, 0x20];
    e.resize(32, 0);
    h.game.tick_out = vec![(0, c.clone()), (0, c.clone()), (0, e.clone())];
    h.clock.0 = 1080;
    let r = h.frame().unwrap();
    assert_eq!(r.flushed_buffers, 2);
    assert_eq!(h.receive(0), [c.clone(), c, e]);
}

fn client_ids(scope: Scope) -> Vec<u8> {
    CLIENT_MESSAGES
        .iter()
        .filter(|m| m.scope == scope)
        .map(|m| m.id)
        .collect()
}

fn server_ids(by: ProducedBy) -> Vec<u8> {
    SERVER_MESSAGES
        .iter()
        .filter(|m| m.produced_by == by)
        .map(|m| m.id)
        .collect()
}

// Covers: specs/sim/intents-events.md §4 r3
#[test]
fn session_rows_go_to_the_session_code() {
    assert_eq!(
        client_ids(Scope::Session),
        [0x67, 0x69, 0x6A, 0x6B, 0x6C, 0x6E, 0x70]
    );
    assert_eq!(
        server_ids(ProducedBy::Session),
        [0x00, 0x02, 0x04, 0x05, 0x06, 0x0B, 0x5B, 0x5C]
    );
    // C→S session messages reach the session handler, never the sim.
    let mut h = host();
    let mut sent = Vec::new();
    for id in client_ids(Scope::Session) {
        let mut m = vec![id; 1];
        let n = match id {
            0x6C => 7,
            _ => match CLIENT_MESSAGES[id as usize].transport_size.eval(&[id]) {
                d2_proto::schema::Size::Bytes(n) => n,
                other => panic!("{id:#x}: {other:?}"),
            },
        };
        m.resize(n, 0);
        assert_eq!(h.send_system(0, &m), Ok(Classified::Queued(Queue::System)));
        sent.push(m);
    }
    let r = h.frame().unwrap();
    assert!(outcomes(&r).iter().all(|o| *o == Handled::System));
    let seen: Vec<Vec<u8>> = h.session.seen.iter().map(|(_, m, _)| m.clone()).collect();
    assert_eq!(seen, sent);
    assert!(h.game.handled.is_empty());
}

// Covers: specs/sim/intents-events.md §4 r4
#[test]
fn out_of_scope_rows() {
    // Multiplayer and Battle.net / realm C→S ids.
    assert_eq!(client_ids(Scope::Out), [0x5D, 0x5E, 0x66, 0x68, 0x6D]);
    let mut out = vec![
        0x75, 0x77, 0x78, 0x79, 0x7F, 0x8B, 0x8C, 0x8D, 0x8F, 0x90, 0xAE,
    ];
    // 0xAF..=0xB4 except 0xB1, whose size entry is 0: a `none` row
    // (rule 5), never receivable.
    out.extend([0xAF, 0xB0, 0xB2, 0xB3, 0xB4]);
    let mut got = server_ids(ProducedBy::Out);
    got.extend(server_ids(ProducedBy::Transport));
    got.sort_unstable();
    assert_eq!(got, out);
    assert_eq!(SERVER_MESSAGES[0xB1].produced_by, ProducedBy::None);
    // Queue 2 (0xFF): no host callbacks in d2rs; dropped.
    let mut h = host();
    assert_eq!(
        h.send_system(0, &[0xFF; 16]),
        Ok(Classified::Queued(Queue::Admin))
    );
    let r = h.frame().unwrap();
    assert_eq!(outcomes(&r), [Handled::AdminIgnored]);
    assert!(h.session.seen.is_empty() && h.game.handled.is_empty());
}

// Covers: specs/sim/intents-events.md §4 r5
#[test]
fn none_rows_are_rejected_like_the_original() {
    let none = client_ids(Scope::None);
    assert!(!none.is_empty());
    let mut h = host();
    for id in none {
        let (code_ok, n) = match CLIENT_MESSAGES[id as usize].transport_size.eval(&[id]) {
            d2_proto::schema::Size::Bytes(n) => (true, n),
            _ => (false, 9),
        };
        let mut m = vec![id];
        m.resize(n, 0);
        let class = h.send_system(0, &m).unwrap();
        if !code_ok {
            // Size 0: never queued.
            assert_eq!(class, Classified::Incomplete, "{id:#x}");
            continue;
        }
        let r = h.frame().unwrap();
        let expected = match dispatch::kind(id) {
            dispatch::Kind::Stub0 => ResultCode::Done,
            dispatch::Kind::Stub3 | dispatch::Kind::None => ResultCode::Malformed,
            dispatch::Kind::Handler => panic!("{id:#x} has a handler"),
        };
        assert_eq!(
            outcomes(&r),
            [Handled::Game(Outcome::Dispatched(expected))],
            "{id:#x}"
        );
    }
    // No sim handler ran for any of them.
    assert!(h.game.handled.is_empty());
}
