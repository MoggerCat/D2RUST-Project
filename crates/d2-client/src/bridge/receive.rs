// Spec: specs/client/bridge.md
//! Receive path (§2): split S→C chunks with `d2-proto`'s size rule and
//! dispatch each message by id. Unowned ids and discarded bytes are
//! recorded, never interpreted.

use std::collections::BTreeMap;

use d2_proto::transport::{split_server_buffer, SplitError};

use super::dispatch::{Dispatch, HandlerError, Message};
use super::world::{addressed_unit, ClientWorld};

/// Bytes a split discarded (§2 rule 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Discarded {
    /// The first discarded byte: the id the size rule refused.
    pub first: u8,
    pub bytes: usize,
}

/// A handler's refusal (§6 rule 4).
#[derive(Debug, PartialEq, Eq)]
pub struct Rejected {
    pub id: u8,
    pub error: HandlerError,
}

/// Everything the receive path did not apply to the model.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReceiveLog {
    /// Messages applied by a handler.
    pub handled: u64,
    /// Messages without a handler, counted per id (§6 rule 3).
    pub unowned: BTreeMap<u8, u64>,
    pub discarded: Vec<Discarded>,
    pub rejected: Vec<Rejected>,
}

/// What one chunk did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkReport {
    pub messages: usize,
    pub handled: usize,
    pub unowned: usize,
    pub rejected: usize,
    pub discarded_bytes: usize,
}

/// Splits `chunk` and dispatches its messages in order. A chunk 1.14d
/// asserts on (§2 rule 4) is refused whole: nothing in it is dispatched.
pub fn receive_chunk(
    world: &mut ClientWorld,
    dispatch: &Dispatch,
    log: &mut ReceiveLog,
    chunk: &[u8],
) -> Result<ChunkReport, SplitError> {
    let split = split_server_buffer(chunk)?;
    let mut report = ChunkReport {
        messages: split.messages.len(),
        ..ChunkReport::default()
    };
    for bytes in split.messages {
        let msg = Message {
            id: bytes[0],
            bytes,
            unit: addressed_unit(bytes),
        };
        match dispatch.get(msg.id) {
            None => {
                *log.unowned.entry(msg.id).or_default() += 1;
                report.unowned += 1;
            }
            Some(entry) => match (entry.handle)(world, &msg) {
                Ok(()) => {
                    log.handled += 1;
                    report.handled += 1;
                }
                Err(error) => {
                    log.rejected.push(Rejected { id: msg.id, error });
                    report.rejected += 1;
                }
            },
        }
    }
    if let Some(&first) = split.discarded.first() {
        log.discarded.push(Discarded {
            first,
            bytes: split.discarded.len(),
        });
        report.discarded_bytes = split.discarded.len();
    }
    Ok(report)
}
