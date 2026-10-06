// Spec: specs/client/bridge.md
//! The server link (§3): the narrow interface the bridge needs from the
//! in-process `d2-server` host (or, later, a remote one over `d2-net`).

use d2_proto::transport::ClientQueue;

/// Client id of the local player (`intents-events.md` Inputs).
pub const LOCAL_CLIENT: u32 = 0;

/// Server queue a C→S message is handed to (§4 rule 2). Queue 2 (admin)
/// is never sent by the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendQueue {
    /// Ids below 0x67: the client's game-message sender.
    Game,
    /// Ids 0x67..=0x70: the system-message senders.
    System,
}

impl SendQueue {
    /// The send queue of a classifier queue; `None` for the admin queue.
    pub fn of(queue: ClientQueue) -> Option<Self> {
        match queue {
            ClientQueue::Game => Some(Self::Game),
            ClientQueue::System => Some(Self::System),
            ClientQueue::Admin => None,
        }
    }
}

/// What the link did with a sent message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent {
    /// Appended to the server queue.
    Queued,
    /// Dropped by the client sender's duplicate filter
    /// (`intents-events.md` §2.1 rule 1).
    Filtered,
}

/// One server frame (§3 rule 1, `pump`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pumped {
    /// The tick driver ran a tick (and the flush ran).
    pub ticked: bool,
}

/// A failure inside the link or the server behind it.
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("server: {0}")]
    Server(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// The bridge's view of the server (§3 rule 1). Implemented by the
/// in-process host adapter once `d2-server` is wired (spec open question
/// 1), and by test fakes.
pub trait ServerLink {
    /// The server's `d2_proto::PROTOCOL_VERSION` (§9 rule 1).
    fn protocol_version(&self) -> u32;

    /// Hands one C→S message to the server's sender for `queue`.
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError>;

    /// One server frame: drain → tick driver → flush if a tick ran.
    fn pump(&mut self) -> Result<Pumped, LinkError>;

    /// The S→C chunks delivered to the local client since the last call,
    /// system list first, each as delivered (§2 rule 1).
    fn receive(&mut self) -> Vec<Vec<u8>>;
}
