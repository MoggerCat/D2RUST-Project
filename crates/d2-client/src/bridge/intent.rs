// Spec: specs/client/bridge.md
//! Send path (§4): C→S bytes built with `d2-proto`, checked with the
//! 1.14d classifier before they reach the link.

use d2_proto::transport::{classify_client, Classified, ClientQueue, MAX_MESSAGE};
use d2_proto::FixedMessage;

use super::link::SendQueue;

/// The 1.14d game-message sender asserts size < 0x200 (`intents-events.md`
/// §2.1 rule 1).
pub const MAX_GAME_SEND: usize = 0x200;

/// A message the bridge refuses to send (§4 rule 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IntentError {
    #[error("message refused by the classifier: {0:?}")]
    NotSendable(Classified),
    #[error("id 0xFF (admin queue) is out of scope")]
    AdminQueue,
    #[error("game message of {0} bytes (the sender asserts < 0x200)")]
    GameTooLarge(usize),
    /// A system message the classifier queues (it accepts a buffer longer
    /// than the size rule) but the transport asserts on (size ≤ 0x204,
    /// `intents-events.md` §2.1 rule 3).
    #[error("message of {0} bytes (the transport asserts <= 0x204)")]
    TooLarge(usize),
}

/// Encodes a typed C→S message (the TSV layout; unlisted bytes 0).
pub fn encode<M: FixedMessage>(msg: &M) -> Vec<u8> {
    let mut b = vec![0; M::SIZE];
    msg.write(&mut b);
    b
}

/// The send queue of `msg`, or why it may not be sent (§4 rules 2–3).
pub fn route(msg: &[u8]) -> Result<SendQueue, IntentError> {
    match classify_client(msg) {
        Classified::Queue(q) => {
            let queue = SendQueue::of(q).ok_or(IntentError::AdminQueue)?;
            if q == ClientQueue::Game && msg.len() >= MAX_GAME_SEND {
                return Err(IntentError::GameTooLarge(msg.len()));
            }
            if msg.len() > MAX_MESSAGE {
                return Err(IntentError::TooLarge(msg.len()));
            }
            Ok(queue)
        }
        other => Err(IntentError::NotSendable(other)),
    }
}
