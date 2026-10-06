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
}

/// The per-client buffer lists (spec §3.2 rules 1–2). Clients are known
/// once [`ClientBuffers::add_client`] ran; messages for any other id are
/// ignored.
#[derive(Debug, Default)]
pub struct ClientBuffers {
    clients: BTreeMap<ClientId, VecDeque<Vec<u8>>>,
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

    /// Pops the client's head buffer (`0x005392A0`).
    pub fn pop(&mut self, client: ClientId) -> Option<Vec<u8>> {
        self.clients.get_mut(&client)?.pop_front()
    }
}

impl MessageSink for ClientBuffers {
    /// `0x0053B280` (spec §3.2 rule 1): append to the tail buffer, or to a
    /// new one when there is no tail or tail + size > 0x200. Messages are
    /// never split across buffers (rule 2).
    fn queue(&mut self, client: ClientId, msg: &[u8]) -> Result<(), QueueError> {
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
