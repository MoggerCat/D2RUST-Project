// Spec: specs/sim/tick.md
//! `tick.md` §1 vectors (tick driver) and the loop order of
//! `intents-events.md` §1 on the host.

use super::fakes::*;
use crate::dispatch::Outcome;
use crate::host::*;
use crate::seams::*;

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

// Covers: specs/sim/tick.md §1 r1
#[test]
fn tick_length() {
    assert_eq!(TICK_RATE, 25);
    assert_eq!(TICK_MS, 40);
}

// Covers: specs/sim/tick.md §1 r2, §1 r3, §edge-cases-original-bugs r6
#[test]
fn driver_vectors() {
    let at = |last, now, catch_up| {
        let mut d = TickDriver { last: Some(last) };
        let ran = d.poll(now, catch_up);
        (ran, d.last.unwrap())
    };
    assert_eq!(at(1000, 1039, true), (false, 1000));
    assert_eq!(at(1000, 1040, true), (true, 1040));
    assert_eq!(at(1000, 1150, true), (true, 1110));
    assert_eq!(at(1000, 1150, false), (true, 1040));
    // After the catch-up the next call at the same time ticks again, then
    // the lag is dropped (rule 3).
    let mut d = TickDriver { last: Some(1000) };
    assert!(d.poll(1150, true));
    assert!(d.poll(1150, true));
    assert_eq!(d.last, Some(1150));
    assert!(!d.poll(1150, true));
}

// Covers: specs/sim/tick.md §1 r2
#[test]
fn driver_first_use_and_mask() {
    let mut d = TickDriver::new();
    assert!(!d.poll(5000, true));
    assert_eq!(d.last, Some(5000));
    // `now & 0x7FFFFFFF` with a signed difference, as §1.2 states: across
    // the 31-bit wrap the difference is negative and no tick runs
    // (not an edge case the spec lists; queued in HANDOFF §5).
    let mut d = TickDriver {
        last: Some(0x7FFF_FFF0),
    };
    assert!(!d.poll(0x8000_0018, true));
    assert_eq!(d.last, Some(0x7FFF_FFF0));
    // Without the mask bit the difference is 40: a tick.
    let mut d = TickDriver { last: Some(0x10) };
    assert!(d.poll(0x8000_0038, true));
    assert_eq!(d.last, Some(0x38));
}

// Covers: specs/sim/tick.md §1 r4; specs/sim/intents-events.md §1 r1, §1 r2, §1 r3, §1 r4
#[test]
fn loop_order() {
    let mut h = host();
    // First frame: the driver starts its clock, no tick, no flush.
    h.send_game(0, &[0x01, 100, 0, 100, 0]).unwrap();
    let r = h.frame().unwrap();
    assert!(!r.ticked);
    assert_eq!(
        r.messages[0].handled,
        Outcome::Dispatched(ResultCode::Done).into()
    );
    // The handler ran before any tick, with frame 0.
    assert_eq!(h.game.handled.len(), 1);
    assert_eq!(h.game.point_state(0).unwrap().last_accept, 0);

    // A handler message queued without a tick stays buffered.
    h.game.handler_out = vec![(0, vec![0x0C; 9])];
    h.game.tick_out = vec![(0, vec![0x01; 8])];
    h.clock.0 = 1020;
    h.send_game(0, &[0x03, 100, 0, 101, 0]).unwrap();
    let r = h.frame().unwrap();
    assert!(!r.ticked);
    assert_eq!(h.buffers.buffers(0).unwrap().len(), 1);
    assert!(h.receive(0).is_empty());

    // Next frame: drain (handler message) → tick (tick message) → flush.
    h.clock.0 = 1040;
    h.send_game(0, &[0x01, 100, 0, 102, 0]).unwrap();
    let r = h.frame().unwrap();
    assert!(r.ticked);
    assert_eq!(h.game.frame, 1);
    assert_eq!(r.flushed_buffers, 1);
    assert_eq!(h.game.handled.len(), 3);
    let got: Vec<u8> = h.receive(0).iter().map(|m| m[0]).collect();
    assert_eq!(got, [0x0C, 0x0C, 0x01]);
    assert_eq!(h.record(0).unwrap().last_message_ms, 1040);
}

impl From<Outcome> for Handled {
    fn from(o: Outcome) -> Self {
        Handled::Game(o)
    }
}

// Covers: specs/sim/tick.md §1 r5; specs/sim/intents-events.md §1 r2
#[test]
fn messages_between_ticks_run_with_the_previous_frame() {
    let mut h = host();
    h.frame().unwrap();
    h.clock.0 = 1040;
    assert!(h.frame().unwrap().ticked);
    // Three messages arrive before the next tick; all run at frame 1, in
    // arrival order, before frame 2.
    for y in [100u8, 101, 102] {
        h.send_game(0, &[0x01, 100, 0, y, 0]).unwrap();
    }
    h.clock.0 = 1080;
    let r = h.frame().unwrap();
    assert!(r.ticked);
    let ys: Vec<u8> = h.game.handled.iter().map(|(_, m, _)| m[3]).collect();
    assert_eq!(ys, [100, 101, 102]);
    assert_eq!(h.game.point_state(0).unwrap().last_accept, 1);
    assert_eq!(h.game.frame, 2);
}

// Covers: specs/sim/intents-events.md §2.1 r7
#[test]
fn drain_routes_every_queue() {
    let mut h = host();
    let mut ff = vec![0xFF];
    ff.resize(16, 0);
    h.send_game(0, &[0x01, 100, 0, 100, 0]).unwrap();
    h.send_system(0, &[0x6B]).unwrap();
    h.send_system(0, &ff).unwrap();
    // Not in a game.
    h.connect(4);
    h.send_game(4, &[0x01, 100, 0, 100, 0]).unwrap();
    let r = h.frame().unwrap();
    let handled: Vec<(u32, u8, Handled)> = r
        .messages
        .iter()
        .map(|m| (m.client, m.id, m.handled))
        .collect();
    assert_eq!(
        handled,
        [
            (0, 0x6B, Handled::System),
            (0, 0x01, Outcome::Dispatched(ResultCode::Done).into()),
            (4, 0x01, Outcome::NotInGame.into()),
            (0, 0xFF, Handled::AdminIgnored),
        ]
    );
    assert_eq!(h.session.seen, [(0, vec![0x6B], 1)]);
}

// Covers: specs/sim/intents-events.md §2.1 r1, §2.1 r2
#[test]
fn duplicate_filter_on_the_host() {
    let mut h = host();
    let walk = [0x03, 0x10, 0x00, 0x20, 0x00];
    assert!(h.send_game(0, &walk).unwrap().is_some());
    h.clock.0 += 120;
    assert_eq!(h.send_game(0, &walk), Ok(None));
    // System messages are not filtered.
    assert!(h.send_system(0, &[0x6B]).is_ok());
    assert!(h.send_system(0, &[0x6B]).is_ok());
    assert_eq!(h.queues.len(crate::transport::Queue::System), 2);
}

#[test]
fn flush_follows_the_client_list() {
    let mut h = host();
    h.connect(1);
    h.game.players.insert(1, h.game.players[&0].clone());
    // Client list [1, 0] (joined 0 then 1, §7.2 prepends).
    h.game.client_list = vec![1, 0];
    h.game.tick_out = vec![(0, vec![0x0C; 9]), (1, vec![0x01; 8]), (0, vec![0x01; 8])];
    h.frame().unwrap();
    h.clock.0 = 1040;
    let r = h.frame().unwrap();
    assert_eq!(r.flushed_buffers, 2);
    assert_eq!(h.receive(1), [vec![0x01; 8]]);
    assert_eq!(h.receive(0), [vec![0x0C; 9], vec![0x01; 8]]);
    // A client outside the client list is not flushed.
    h.connect(2);
    h.game.tick_out = vec![(2, vec![0x01; 8])];
    h.clock.0 = 1080;
    h.frame().unwrap();
    assert!(h.receive(2).is_empty());
    assert_eq!(h.buffers.buffers(2).unwrap().len(), 1);
}

// Covers: specs/sim/intents-events.md §3.3 r5, §3.4 r1
#[test]
fn direct_sends_overtake_buffered_messages() {
    let mut h = host();
    h.game.handler_out = vec![(0, vec![0x0C; 9])];
    h.send_game(0, &[0x01, 100, 0, 100, 0]).unwrap();
    h.frame().unwrap();
    h.send_direct(0, &[0x06]).unwrap();
    h.send_direct(0, &[0xB0]).unwrap();
    h.clock.0 = 1040;
    h.frame().unwrap();
    let got: Vec<u8> = h.receive(0).iter().map(|m| m[0]).collect();
    // System list first, then the game list: direct 0x06 before 0x0C.
    assert_eq!(got, [0xB0, 0x06, 0x0C]);
}

#[test]
fn flush_throttle_unless_forced() {
    let mut h = host();
    assert_eq!(h.flush(false, 1000), Ok((0, 0)));
    h.buffers.queue_for_test(0, &[0x01; 8]);
    assert_eq!(h.flush(false, 1039), Ok((0, 0)));
    assert_eq!(h.flush(false, 1040), Ok((1, 0)));
    h.buffers.queue_for_test(0, &[0x01; 8]);
    assert_eq!(h.flush(true, 1041), Ok((1, 0)));
}

impl crate::buffers::ClientBuffers {
    fn queue_for_test(&mut self, client: ClientId, msg: &[u8]) {
        MessageSink::queue(self, client, msg).unwrap();
    }
}

// Covers: specs/sim/tick.md §1 text
#[test]
fn same_messages_same_result_regardless_of_timing() {
    // tick.md §1: the same messages before the same frame give the same
    // outcome whatever the wall-clock gaps.
    let play = |times: &[u32]| {
        let mut h = host();
        h.game.tick_out = vec![(0, vec![0x01; 8])];
        let mut log = Vec::new();
        for (i, &t) in times.iter().enumerate() {
            h.clock.0 = t;
            if i % 2 == 0 {
                h.send_game(0, &[0x01, 100, 0, 100 + i as u8, 0]).unwrap();
            }
            let r = h.frame().unwrap();
            log.push((r.ticked, h.game.frame, h.receive(0).len()));
        }
        (log, h.game.handled.clone())
    };
    let a = play(&[1000, 1040, 1080, 1120, 1160]);
    let b = play(&[1000, 1041, 1095, 1121, 1170]);
    assert_eq!(a, b);
}
