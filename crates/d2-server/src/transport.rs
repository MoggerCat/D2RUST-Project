// Spec: specs/sim/intents-events.md
//! Client → server local transport (spec §2.1): the client's duplicate
//! filter, the classifier, the three server queues and the drain.

use std::collections::VecDeque;

use crate::seams::{ClientId, MessageSizes, SizeError};

/// Largest message the transport accepts (spec Constants: 0x204).
pub const MAX_MESSAGE: usize = 0x204;

/// The drain copies at most 0x200 bytes including the 4-byte client id
/// (spec §2.1 rule 7).
pub const DRAIN_COPY: usize = 0x200 - 4;

/// Size of the queue-2 message 0xFF (spec §2.1 rule 4).
pub const ADMIN_SIZE: usize = 16;

/// The client game-message sender asserts size < 0x200 (spec §2.1 rule 1).
pub const MAX_GAME_SEND: usize = 0x200;

/// Server queue (spec §2.1 rule 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Queue {
    /// 0: system messages 0x67..=0x70.
    System = 0,
    /// 1: game messages < 0x67.
    Game = 1,
    /// 2: realm/admin 0xFF.
    Admin = 2,
}

/// Classifier result (spec §2.1 rule 4). Results 3 and 4 drop the message
/// silently in local mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classified {
    /// Results 1 and 2: appended to this queue.
    Queued(Queue),
    /// Result 3: incomplete (empty, size rule 0, > 0x204 or > the given
    /// size).
    Incomplete,
    /// Result 4: invalid id (0x71..=0xFE, or 0xFF with the gate closed).
    Invalid,
    /// The chat size rule came out negative ([`crate::seams::SizeError::Negative`]):
    /// the original's result is not in the spec; not queued.
    NegativeSize(i32),
}

/// A send the original would fail with a fatal assert.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    #[error("game message of {0} bytes (the sender asserts < 0x200)")]
    GameTooLarge(usize),
    #[error("message of {0} bytes (the transport asserts <= 0x204)")]
    TooLarge(usize),
}

/// Classifier `0x0052B100` (spec §2.1 rule 4). `admin_gate` is the net
/// object's gate `0x006BF6C0` for id 0xFF.
pub fn classify(sizes: &impl MessageSizes, msg: &[u8], admin_gate: bool) -> Classified {
    let Some(&id) = msg.first() else {
        return Classified::Incomplete;
    };
    if (0x71..=0xFE).contains(&id) {
        return Classified::Invalid;
    }
    let rule = if id == 0xFF {
        Ok(ADMIN_SIZE)
    } else {
        sizes.client_size(msg)
    };
    match rule {
        Ok(n) if n != 0 && n <= MAX_MESSAGE && n <= msg.len() => {}
        Err(SizeError::Negative(n)) => return Classified::NegativeSize(n),
        _ => return Classified::Incomplete,
    }
    match id {
        0x00..=0x66 => Classified::Queued(Queue::Game),
        0x67..=0x70 => Classified::Queued(Queue::System),
        _ if admin_gate => Classified::Queued(Queue::Admin),
        _ => Classified::Invalid,
    }
}

/// One drained message (spec §2.1 rules 6–7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drained {
    pub queue: Queue,
    pub client: ClientId,
    /// The drain copy: the first `min(size, 0x1FC)` bytes.
    pub msg: Vec<u8>,
    /// The full queued size, which handlers see.
    pub size: usize,
}

/// The three server queues (spec §2.1 rule 6): FIFO, the whole given
/// buffer is queued (not the rule size).
#[derive(Debug)]
pub struct ServerQueues {
    queues: [VecDeque<(ClientId, Vec<u8>)>; 3],
    /// Gate `0x006BF6C0` for queue 2.
    pub admin_gate: bool,
}

impl Default for ServerQueues {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerQueues {
    /// Empty queues; the admin gate passes (spec test vector "0xFF (16
    /// bytes) → queue 2").
    pub fn new() -> Self {
        Self {
            queues: Default::default(),
            admin_gate: true,
        }
    }

    /// Net send `0x0052AE50` in local mode: asserts size ≤ 0x204, then
    /// classifies and enqueues (spec §2.1 rules 3–4).
    pub fn send(
        &mut self,
        sizes: &impl MessageSizes,
        client: ClientId,
        msg: &[u8],
    ) -> Result<Classified, SendError> {
        if msg.len() > MAX_MESSAGE {
            return Err(SendError::TooLarge(msg.len()));
        }
        let class = classify(sizes, msg, self.admin_gate);
        if let Classified::Queued(q) = class {
            self.queues[q as usize].push_back((client, msg.to_vec()));
        }
        Ok(class)
    }

    /// Messages waiting in `queue`.
    pub fn len(&self, queue: Queue) -> usize {
        self.queues[queue as usize].len()
    }

    /// True when all three queues are empty.
    pub fn is_empty(&self) -> bool {
        self.queues.iter().all(VecDeque::is_empty)
    }

    /// Drain `0x0052CFE0` (spec §2.1 rule 7): queue 0 until empty, then
    /// queue 1, then queue 2; each message truncated to the drain copy.
    pub fn drain(&mut self) -> Vec<Drained> {
        let mut out = Vec::new();
        for queue in [Queue::System, Queue::Game, Queue::Admin] {
            for (client, mut msg) in self.queues[queue as usize].drain(..) {
                let size = msg.len();
                msg.truncate(DRAIN_COPY);
                out.push(Drained {
                    queue,
                    client,
                    msg,
                    size,
                });
            }
        }
        out
    }
}

/// Duplicate-filter window for a game message id (spec §2.1 rule 1);
/// `None` = never filtered.
pub fn duplicate_window(id: u8) -> Option<u32> {
    match id {
        0x05..=0x0A | 0x0C..=0x11 => Some(50),
        0x3A => None,
        _ => Some(200),
    }
}

/// The client game-message sender `0x00478350` (spec §2.1 rule 1): drops
/// a repeat of the last sent message within its window. The comparison
/// runs over the new message's size against a 0x200-byte store that each
/// send overwrites only for its own length (`0x007BB3B8`; the bytes past a
/// shorter message stay from earlier ones), store and time start zeroed.
#[derive(Debug)]
pub struct DuplicateFilter {
    stored: [u8; MAX_GAME_SEND],
    at: u32,
}

impl Default for DuplicateFilter {
    fn default() -> Self {
        Self {
            stored: [0; MAX_GAME_SEND],
            at: 0,
        }
    }
}

impl DuplicateFilter {
    /// True when `msg` sent at `now_ms` passes; a sent message replaces
    /// the stored copy and time, a filtered one leaves both as they were.
    pub fn pass(&mut self, msg: &[u8], now_ms: u32) -> Result<bool, SendError> {
        if msg.len() >= MAX_GAME_SEND {
            return Err(SendError::GameTooLarge(msg.len()));
        }
        if let Some(window) = msg.first().and_then(|&id| duplicate_window(id)) {
            if self.stored[..msg.len()] == *msg && now_ms.wrapping_sub(self.at) < window {
                return Ok(false);
            }
        }
        self.stored[..msg.len()].copy_from_slice(msg);
        self.at = now_ms;
        Ok(true)
    }
}
