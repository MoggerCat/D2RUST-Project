// Spec: specs/sim/intents-events.md
//! Server → client: per-client buffers of up to 0x200 bytes (spec §3.2),
//! local delivery that splits them back into messages (§3.3), and the
//! client's receive lists (§3.4 rule 1).

use std::collections::{BTreeMap, VecDeque};

use crate::seams::{ClientId, MessageSink, MessageSizes};
use crate::transport::MAX_MESSAGE;

/// Data bytes per client buffer (spec §3.2 rule 1).
pub const BUFFER_SIZE: usize = 0x200;

/// First S→C id of the system receive list (spec §3.3 rule 1).
pub const FIRST_SYSTEM_ID: u8 = 0xAF;

/// Number of S→C ids (spec §3.1 rule 1: 0x00..=0xB4).
pub const SERVER_IDS: u8 = 0xB5;

/// A queue or delivery the original would fail with a fatal assert or a
/// buffer overrun.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum QueueError {
    #[error("message of {0} bytes does not fit a 0x200-byte client buffer")]
    TooLarge(usize),
    #[error("S->C id {0:#04x} >= 0xB5 in local delivery")]
    BadId(u8),
    #[error("S->C message of {0} bytes (local delivery asserts 1..=0x204)")]
    BadSize(usize),
    /// A direct send or a client flush on a sink without receive lists.
    #[error("S->C id {0:#04x}: this sink has no direct send or flush")]
    NoDirect(u8),
}

/// One server → client message or buffer the packet tap saw, in order
/// (`specs/tools/packets-trace.md` §2; read by [`crate::packets`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tapped {
    /// A message queued for a known client (`0x0053B280`).
    Queued(ClientId, Vec<u8>),
    /// A direct send (spec §3.3 rule 5).
    Direct(ClientId, Vec<u8>),
    /// A buffer popped by a flush and handed to local delivery.
    Flushed(ClientId, Vec<u8>),
}

/// The per-client buffer lists (spec §3.2 rules 1–2). Clients are known
/// once [`ClientBuffers::add_client`] ran; messages for any other id are
/// ignored.
#[derive(Debug, Default)]
pub struct ClientBuffers {
    clients: BTreeMap<ClientId, VecDeque<Vec<u8>>>,
    /// The packet tap: `Some` while a recorder listens; a copy of every
    /// queued message, direct send and popped buffer. Never read by the
    /// game.
    tap: Option<Vec<Tapped>>,
}

impl ClientBuffers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates the client's (empty) buffer list.
    pub fn add_client(&mut self, client: ClientId) {
        self.clients.entry(client).or_default();
    }

    /// Drops the client and anything still buffered for it.
    pub fn remove_client(&mut self, client: ClientId) {
        self.clients.remove(&client);
    }

    /// The client's buffers, head first (sizes and bytes).
    pub fn buffers(&self, client: ClientId) -> Option<&VecDeque<Vec<u8>>> {
        self.clients.get(&client)
    }

    /// Pops the client's head buffer (`0x005392A0`). Every pop is a
    /// flush's send (the tap records it as [`Tapped::Flushed`]).
    pub fn pop(&mut self, client: ClientId) -> Option<Vec<u8>> {
        let buf = self.clients.get_mut(&client)?.pop_front()?;
        self.note(|| Tapped::Flushed(client, buf.clone()));
        Some(buf)
    }

    /// Turns the packet tap on (an empty log) or off (dropped).
    pub fn set_tap(&mut self, on: bool) {
        self.tap = on.then(Vec::new);
    }

    /// What the tap saw since the last take, in order (empty when off).
    pub fn take_tap(&mut self) -> Vec<Tapped> {
        self.tap.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// Appends to the tap when it is on (the closure runs only then).
    pub(crate) fn note(&mut self, t: impl FnOnce() -> Tapped) {
        if let Some(tap) = self.tap.as_mut() {
            tap.push(t());
        }
    }
}

impl MessageSink for ClientBuffers {
    /// `0x0053B280` (spec §3.2 rule 1): append to the tail buffer, or to a
    /// new one when there is no tail or tail + size > 0x200. Messages are
    /// never split across buffers (rule 2).
    fn queue(&mut self, client: ClientId, msg: &[u8]) -> Result<(), QueueError> {
        if self.tap.is_some() && self.clients.contains_key(&client) {
            self.note(|| Tapped::Queued(client, msg.to_vec()));
        }
        let Some(list) = self.clients.get_mut(&client) else {
            return Ok(());
        };
        if msg.len() > BUFFER_SIZE {
            return Err(QueueError::TooLarge(msg.len()));
        }
        match list.back_mut() {
            Some(tail) if tail.len() + msg.len() <= BUFFER_SIZE => tail.extend_from_slice(msg),
            _ => {
                let mut buf = Vec::with_capacity(BUFFER_SIZE);
                buf.extend_from_slice(msg);
                list.push_back(buf);
            }
        }
        Ok(())
    }

    fn has_queued(&self, client: ClientId) -> bool {
        self.clients.get(&client).is_some_and(|l| !l.is_empty())
    }
}

/// The client's two receive lists (spec §3.3 rule 1): one message per
/// node; local mode 1 returns nodes at once (rule 4).
#[derive(Debug, Default)]
pub struct Inbox {
    /// Ids 0xAF..=0xB4.
    pub system: VecDeque<Vec<u8>>,
    /// Ids < 0xAF.
    pub game: VecDeque<Vec<u8>>,
}

impl Inbox {
    /// Appends one whole message to its list (also the direct-send path,
    /// §3.3 rule 5).
    pub fn push(&mut self, msg: &[u8]) -> Result<(), QueueError> {
        if msg.is_empty() || msg.len() > MAX_MESSAGE {
            return Err(QueueError::BadSize(msg.len()));
        }
        match msg[0] {
            id if id >= SERVER_IDS => return Err(QueueError::BadId(id)),
            id if id >= FIRST_SYSTEM_ID => self.system.push_back(msg.to_vec()),
            _ => self.game.push_back(msg.to_vec()),
        }
        Ok(())
    }

    /// Local delivery `0x0052AEB0` (spec §3.3 rules 1–3): split `buffer`
    /// by the S→C size rule; a size-rule failure ends the split and the
    /// rest of the buffer is lost. Returns the bytes discarded.
    pub fn deliver(
        &mut self,
        sizes: &impl MessageSizes,
        buffer: &[u8],
    ) -> Result<usize, QueueError> {
        let mut rest = buffer;
        while !rest.is_empty() {
            let n = match sizes.server_size(rest) {
                Ok(n) if n != 0 && n <= rest.len() => n,
                // Size 0, or a rule larger than the bytes left: the split
                // ends (a whole-buffer copy never runs past the buffer).
                _ => break,
            };
            self.push(&rest[..n])?;
            rest = &rest[n..];
        }
        Ok(rest.len())
    }

    /// Client receive `0x0044C6E0` (spec §3.4 rule 1): the system list
    /// until empty, then the game list.
    pub fn receive(&mut self) -> Vec<Vec<u8>> {
        self.system.drain(..).chain(self.game.drain(..)).collect()
    }
}
