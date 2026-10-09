// Spec: specs/sim/tick.md
//! The host schedule (`tick.md` §1, §8; `intents-events.md` §1): one
//! client frame drains the server queues, runs at most one tick when
//! 40 ms have passed, and flushes every client after a tick. Wall-clock
//! time comes from an injected [`Clock`] and never reaches the sim.

use std::collections::BTreeMap;
use std::time::Instant;

use crate::buffers::{ClientBuffers, Inbox, QueueError, Tapped};
use crate::dispatch::{process_game_message, ClientRecord, DispatchError, Outcome};
use crate::packets::{PacketEvent, PacketObserver};
use crate::seams::{
    ClientId, Clock, Intents, MessageSink, MessageSizes, PlayerLookup, SessionHandler, Tick,
};
use crate::transport::{Classified, DuplicateFilter, Queue, SendError, ServerQueues};

/// Ticks per second (`tick.md` §1 rule 1; global `0x00731014`).
pub const TICK_RATE: u32 = 25;

/// Tick length in ms: `1000 / rate`, integer division (40).
pub const TICK_MS: u32 = 1000 / TICK_RATE;

/// Flush throttle unless forced (`intents-events.md` §3.2 rule 3).
pub const FLUSH_MS: u32 = 40;

/// The real clock: milliseconds since the clock was made.
#[derive(Debug)]
pub struct SystemClock(Instant);

impl Default for SystemClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}

impl Clock for SystemClock {
    fn now_ms(&mut self) -> u32 {
        // Truncation is the wrap of `timeGetTime`.
        self.0.elapsed().as_millis() as u32
    }
}

/// Tick driver `0x0052FC20` (`tick.md` §1 rule 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickDriver {
    /// `0x00883D58`; 0 means "first use" (§1 r2: the test is `last == 0`,
    /// no separate flag), then set to the masked `now`.
    pub last: u32,
}

impl Default for TickDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl TickDriver {
    pub fn new() -> Self {
        Self { last: 0 }
    }

    /// True when a tick is due at `now` (`timeGetTime()`), updating
    /// `last`. With `catch_up` the lag carried forward is at most one
    /// tick (rule 3).
    pub fn poll(&mut self, now: u32, catch_up: bool) -> bool {
        let now = now & 0x7FFF_FFFF;
        // Edge case 8: a masked clock of exactly 0 on first use leaves
        // `last` at 0, so the next call initialises it again.
        if self.last == 0 {
            self.last = now;
        }
        // Wrapping 32-bit subtraction compared signed (`jge`); after the
        // 31-bit wrap it is negative and no tick runs (edge case 7).
        let elapsed = now.wrapping_sub(self.last) as i32;
        if elapsed < TICK_MS as i32 {
            return false;
        }
        let mut excess = elapsed - TICK_MS as i32;
        if catch_up && excess >= TICK_MS as i32 {
            excess = TICK_MS as i32;
        }
        self.last = now.wrapping_sub(excess as u32);
        true
    }
}

/// What the host did with one drained message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handled {
    /// Queue 0, passed to the session handler (§2.5).
    System,
    /// Queue 1 (§2.2).
    Game(Outcome),
    /// Queue 2: runs only with host callbacks (realm/admin), which d2rs
    /// does not have; dropped.
    AdminIgnored,
}

/// One drained message and its fate, in drain order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandledMessage {
    pub client: ClientId,
    pub id: u8,
    pub size: usize,
    pub handled: Handled,
}

/// One client frame's server part.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameReport {
    pub messages: Vec<HandledMessage>,
    pub ticked: bool,
    /// Buffers handed to local delivery by the flush.
    pub flushed_buffers: usize,
    /// Bytes lost to a split that ended early (`intents-events.md` §3.3
    /// rule 3).
    pub discarded_bytes: usize,
}

/// A state the original fails with a fatal assert.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum HostError {
    #[error(transparent)]
    Dispatch(#[from] DispatchError),
    #[error(transparent)]
    Queue(#[from] QueueError),
}

/// The in-process server host of single player.
pub struct Host<G, S, H, C> {
    pub game: G,
    pub sizes: S,
    pub session: H,
    pub clock: C,
    pub queues: ServerQueues,
    pub buffers: ClientBuffers,
    pub driver: TickDriver,
    /// `catch_up` argument of the driver (1 in single player, §1 rule 4).
    pub catch_up: bool,
    records: BTreeMap<ClientId, ClientRecord>,
    inboxes: BTreeMap<ClientId, Inbox>,
    filters: BTreeMap<ClientId, DuplicateFilter>,
    last_flush: Option<u32>,
    /// The packet recorder (`specs/tools/packets-trace.md` §2); `None` by
    /// default. It only reads: the game runs the same with or without it.
    packets: Option<Box<dyn PacketObserver + Send>>,
}

impl<G, S, H, C> Host<G, S, H, C>
where
    G: Intents + Tick,
    S: MessageSizes,
    H: SessionHandler,
    C: Clock,
{
    pub fn new(game: G, sizes: S, session: H, clock: C) -> Self {
        Self {
            game,
            sizes,
            session,
            clock,
            queues: ServerQueues::new(),
            buffers: ClientBuffers::new(),
            driver: TickDriver::new(),
            catch_up: true,
            records: BTreeMap::new(),
            inboxes: BTreeMap::new(),
            filters: BTreeMap::new(),
            last_flush: None,
            packets: None,
        }
    }

    /// Installs (or with `None` removes) the packet recorder
    /// (`packets-trace.md` §2): from now on every message the host moves
    /// is also reported to it, in order. The game is not changed.
    pub fn set_packet_observer(&mut self, observer: Option<Box<dyn PacketObserver + Send>>) {
        self.buffers.set_tap(observer.is_some());
        self.packets = observer;
    }

    /// Reports one event to the recorder, if any.
    fn note(&mut self, ev: PacketEvent<'_>) {
        if let Some(p) = self.packets.as_mut() {
            p.packet(ev);
        }
    }

    /// Reports what the buffers' tap saw since the last call (queued
    /// messages, direct sends, flushed buffers), in order.
    fn note_tap(&mut self) {
        let Some(p) = self.packets.as_mut() else {
            return;
        };
        for t in self.buffers.take_tap() {
            p.packet(match &t {
                Tapped::Queued(client, msg) => PacketEvent::S2c {
                    client: *client,
                    msg,
                },
                Tapped::Direct(client, msg) => PacketEvent::Net {
                    client: *client,
                    msg,
                    direct: true,
                },
                Tapped::Flushed(client, msg) => PacketEvent::Net {
                    client: *client,
                    msg,
                    direct: false,
                },
            });
        }
    }

    /// Registers a client: its record, buffers, receive lists and sender.
    pub fn connect(&mut self, client: ClientId) {
        self.records.entry(client).or_default();
        self.buffers.add_client(client);
        self.inboxes.entry(client).or_default();
        self.filters.entry(client).or_default();
    }

    /// Forgets a client and everything queued for it.
    pub fn disconnect(&mut self, client: ClientId) {
        self.records.remove(&client);
        self.buffers.remove_client(client);
        self.inboxes.remove(&client);
        self.filters.remove(&client);
    }

    /// The server's record of `client`.
    pub fn record(&self, client: ClientId) -> Option<&ClientRecord> {
        self.records.get(&client)
    }

    /// The client's game-message sender (`intents-events.md` §2.1 rule 1):
    /// `None` when the duplicate filter drops it.
    pub fn send_game(
        &mut self,
        client: ClientId,
        msg: &[u8],
    ) -> Result<Option<Classified>, SendError> {
        let now = self.clock.now_ms();
        self.note(PacketEvent::ClientSend { client, msg });
        let filter = self.filters.entry(client).or_default();
        if !filter.pass(msg, now)? {
            return Ok(None);
        }
        self.note(PacketEvent::ClientOut { client, msg });
        self.queues.send(&self.sizes, client, msg).map(Some)
    }

    /// System-message senders (§2.1 rule 2): no filter.
    pub fn send_system(&mut self, client: ClientId, msg: &[u8]) -> Result<Classified, SendError> {
        self.note(PacketEvent::ClientOut { client, msg });
        self.queues.send(&self.sizes, client, msg)
    }

    /// Direct sends (§3.3 rule 5): straight to the client's receive
    /// lists, ahead of anything still buffered.
    pub fn send_direct(&mut self, client: ClientId, msg: &[u8]) -> Result<(), QueueError> {
        self.buffers.note(|| Tapped::Direct(client, msg.to_vec()));
        self.note_tap();
        self.inboxes.entry(client).or_default().push(msg)
    }

    /// One client frame's server part (`tick.md` §1 rule 4,
    /// `intents-events.md` §1 rule 1): drain → tick driver → flush if a
    /// tick ran. The clock is read once.
    pub fn frame(&mut self) -> Result<FrameReport, HostError> {
        let now = self.clock.now_ms();
        self.game.set_host_tick(now);
        // tools/perf: wall-clock timing of the parts, off unless enabled.
        let timed = crate::perf::enabled();
        let t0 = Instant::now();
        let mut report = FrameReport {
            messages: self.drain(now)?,
            ..FrameReport::default()
        };
        if self.driver.poll(now, self.catch_up) {
            report.ticked = true;
            let drain_us = crate::perf::us_since(t0);
            let t1 = Instant::now();
            if self.packets.is_some() {
                let frame = self.game.frame().wrapping_add(1);
                self.note(PacketEvent::Tick { frame });
            }
            self.game.tick(&mut self.buffers);
            if self.packets.is_some() {
                self.note_tap();
                let frame = self.game.frame();
                self.note(PacketEvent::TickEnd { frame });
            }
            let tick_us = crate::perf::us_since(t1);
            let t2 = Instant::now();
            let (buffers, discarded) = self.flush(true, now)?;
            report.flushed_buffers = buffers;
            report.discarded_bytes = discarded;
            if timed {
                crate::perf::record(crate::perf::TickTime {
                    frame: self.game.frame() as u32,
                    drain_us,
                    tick_us,
                    flush_us: crate::perf::us_since(t2),
                });
            }
        }
        Ok(report)
    }

    /// Drain `0x0052CFE0` and the per-queue handlers (§2.1 rule 7).
    fn drain(&mut self, now: u32) -> Result<Vec<HandledMessage>, HostError> {
        let mut out = Vec::new();
        self.note(PacketEvent::Drain);
        for d in self.queues.drain() {
            if d.queue != Queue::Admin {
                self.note(PacketEvent::C2s {
                    system: d.queue == Queue::System,
                    client: d.client,
                    size: d.size,
                    msg: &d.msg,
                });
            }
            // The dispatcher runs exactly when the player lookup finds a
            // player (`process_game_message`); read before, for the record.
            if self.packets.is_some()
                && d.queue == Queue::Game
                && matches!(self.game.player(d.client), PlayerLookup::Player(_))
                && self.records.contains_key(&d.client)
            {
                let game_frame = self.game.frame();
                self.note(PacketEvent::Dispatch {
                    client: d.client,
                    id: d.msg[0],
                    size: d.size,
                    game_frame,
                });
            }
            let handled = match d.queue {
                Queue::System => {
                    // The game's session part first (`Intents::session_message`),
                    // with the direct send and the client flush of the leave
                    // (§2.5 rule 2).
                    let mut sink = SystemSink {
                        buffers: &mut self.buffers,
                        inboxes: &mut self.inboxes,
                        sizes: &self.sizes,
                    };
                    if !self
                        .game
                        .session_message(d.client, &d.msg, d.size, &mut sink)
                    {
                        self.session
                            .system_message(d.client, &d.msg, d.size, &mut self.buffers);
                    }
                    Handled::System
                }
                Queue::Game => Handled::Game(process_game_message(
                    &mut self.game,
                    &self.sizes,
                    &mut self.records,
                    &mut self.buffers,
                    d.client,
                    &d.msg,
                    d.size,
                    now,
                )?),
                Queue::Admin => Handled::AdminIgnored,
            };
            self.note_tap();
            if let Handled::Game(Outcome::Dispatched(code)) = handled {
                self.note(PacketEvent::Result {
                    client: d.client,
                    code,
                });
            }
            out.push(HandledMessage {
                client: d.client,
                id: d.msg[0],
                size: d.size,
                handled,
            });
        }
        Ok(out)
    }

    /// Flush `0x0052FD90(force, 0)` (§3.2 rules 3–4): unless forced, only
    /// when ≥ 40 ms passed since the last flush; then every client in the
    /// game's client list, buffers head first, each split into the
    /// client's receive lists. Returns (buffers sent, bytes discarded).
    /// Game types 1 and 2 (three buffers per flush, message 0xB3, §3.2
    /// rule 5) are not single player and not implemented; the empty-game
    /// timeout belongs to the session code.
    pub fn flush(&mut self, force: bool, now: u32) -> Result<(usize, usize), HostError> {
        self.note(PacketEvent::Flush);
        let r = self.flush_inner(force, now);
        self.note_tap();
        r
    }

    fn flush_inner(&mut self, force: bool, now: u32) -> Result<(usize, usize), HostError> {
        if !force {
            if let Some(last) = self.last_flush {
                if now.wrapping_sub(last) < FLUSH_MS {
                    return Ok((0, 0));
                }
            }
        }
        self.last_flush = Some(now);
        let (mut sent, mut discarded) = (0, 0);
        for client in self.game.clients() {
            while let Some(buf) = self.buffers.pop(client) {
                let inbox = self.inboxes.entry(client).or_default();
                discarded += inbox.deliver(&self.sizes, &buf)?;
                sent += 1;
            }
        }
        Ok((sent, discarded))
    }

    /// Client receive (§3.4 rule 1): the client's system list, then its
    /// game list, in delivery order.
    pub fn receive(&mut self, client: ClientId) -> Vec<Vec<u8>> {
        self.inboxes
            .get_mut(&client)
            .map(Inbox::receive)
            .unwrap_or_default()
    }
}

/// The sink of the system-queue drain: queued sends go to the client's
/// buffers; direct sends (§3.3 rule 5) and a client flush (`0x0052E320`)
/// go to its receive lists.
struct SystemSink<'a, S> {
    buffers: &'a mut ClientBuffers,
    inboxes: &'a mut BTreeMap<ClientId, Inbox>,
    sizes: &'a S,
}

impl<S: MessageSizes> MessageSink for SystemSink<'_, S> {
    fn queue(&mut self, client: ClientId, msg: &[u8]) -> Result<(), QueueError> {
        self.buffers.queue(client, msg)
    }
    fn has_queued(&self, client: ClientId) -> bool {
        self.buffers.has_queued(client)
    }
    fn send_direct(&mut self, client: ClientId, msg: &[u8]) -> Result<(), QueueError> {
        self.buffers.note(|| Tapped::Direct(client, msg.to_vec()));
        self.inboxes.entry(client).or_default().push(msg)
    }
    fn flush_client(&mut self, client: ClientId) -> Result<(), QueueError> {
        while let Some(buf) = self.buffers.pop(client) {
            self.inboxes
                .entry(client)
                .or_default()
                .deliver(self.sizes, &buf)?;
        }
        Ok(())
    }
}
