// Spec: specs/sim/intents-events.md
//! Size lookup per direction and the local transport's message
//! classification (C→S, §2.1 rule 4) and buffer split (S→C, §3.3).

use crate::generated::{CLIENT_MESSAGES, SERVER_MESSAGES};
use crate::schema::{ClientMessage, ServerMessage, Size};

/// Largest message either direction carries (0x204).
pub const MAX_MESSAGE: usize = 0x204;
/// Size of a per-client S→C buffer (§3.2).
pub const BUFFER_SIZE: usize = 0x200;
/// Size of a queue-2 (id 0xFF) admin message (§2.1 rule 4).
pub const ADMIN_SIZE: usize = 16;
/// First C→S system id (§2.1 rule 4).
pub const FIRST_SYSTEM_ID: u8 = 0x67;
/// First S→C id delivered to the client's system list (§3.3 rule 1).
pub const FIRST_SERVER_SYSTEM_ID: u8 = 0xAF;

/// The C→S descriptor of `id`, if the size table has an entry for it.
pub fn client_message(id: u8) -> Option<&'static ClientMessage> {
    CLIENT_MESSAGES.get(id as usize)
}

/// The S→C descriptor of `id`, if the size table has an entry for it.
pub fn server_message(id: u8) -> Option<&'static ServerMessage> {
    SERVER_MESSAGES.get(id as usize)
}

/// C→S size of the message at the start of `b` (rule `0x0052BC20`, §2.1
/// rule 5). Ids past the table (0x71..) are [`Size::Invalid`].
pub fn client_size(b: &[u8]) -> Size {
    match b.first() {
        None => Size::Incomplete,
        Some(&id) => match client_message(id) {
            Some(m) => m.transport_size.eval(b),
            None => Size::Invalid,
        },
    }
}

/// S→C size of the message at the start of `b` (rule `0x0052B920`, §3.1
/// rule 1). Ids past the table (0xB5..) are [`Size::Invalid`].
pub fn server_size(b: &[u8]) -> Size {
    match b.first() {
        None => Size::Incomplete,
        Some(&id) => match server_message(id) {
            Some(m) => m.size.eval(b),
            None => Size::Invalid,
        },
    }
}

/// Server queue a C→S message goes to (§2.1 rule 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientQueue {
    /// Queue 0: system ids 0x67..0x70.
    System,
    /// Queue 1: game ids below 0x67.
    Game,
    /// Queue 2: id 0xFF, subject to the net object's gate (`0x006BF6C0`),
    /// which the caller applies; failing it is [`Classified::Invalid`].
    Admin,
}

/// Classifier result (`0x0052B100`, §2.1 rule 4). In local mode only
/// `Queue` enqueues; the others drop the message silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classified {
    Queue(ClientQueue),
    /// Result 3: too short, size rule 0, over 0x204, or over the given size.
    Incomplete,
    /// Result 4: id 0x71..0xFE.
    Invalid,
    /// The chat size rule came out negative; the spec does not say what
    /// the classifier does then (`docs/HANDOFF.md` open question).
    NegativeSize(i32),
}

/// Classifies one C→S message of exactly `b.len()` bytes.
pub fn classify_client(b: &[u8]) -> Classified {
    let Some(&id) = b.first() else {
        return Classified::Incomplete;
    };
    if (0x71..=0xFE).contains(&id) {
        return Classified::Invalid;
    }
    let size = if id == 0xFF {
        Size::Bytes(ADMIN_SIZE)
    } else {
        client_size(b)
    };
    match size {
        Size::Bytes(n) if n <= MAX_MESSAGE && n <= b.len() => {}
        Size::Negative(n) => return Classified::NegativeSize(n),
        _ => return Classified::Incomplete,
    }
    Classified::Queue(match id {
        0..FIRST_SYSTEM_ID => ClientQueue::Game,
        0xFF => ClientQueue::Admin,
        _ => ClientQueue::System,
    })
}

/// Client list an S→C message is delivered to (§3.3 rule 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerList {
    /// Ids below 0xAF.
    Game,
    /// Ids 0xAF..0xB4.
    System,
}

pub fn server_list(id: u8) -> ServerList {
    if id < FIRST_SERVER_SYSTEM_ID {
        ServerList::Game
    } else {
        ServerList::System
    }
}

/// A flushed S→C buffer split into messages (§3.3 rules 1–3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Split<'a> {
    pub messages: Vec<&'a [u8]>,
    /// Bytes after a message whose size rule gave 0 or could not be
    /// evaluated: 1.14d discards them (§3.3 rule 3).
    pub discarded: &'a [u8],
}

/// A buffer the split cannot take: a fatal assert in 1.14d (§3.3 rule 2),
/// or a message running past the buffer's end (buffers hold whole
/// messages, §3.2 rule 2).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SplitError {
    #[error("message at byte {at} is {size} bytes, over 0x204")]
    TooLarge { at: usize, size: usize },
    #[error("message at byte {at} is {size} bytes, past the buffer's end")]
    Truncated { at: usize, size: usize },
}

/// Splits a flushed S→C buffer into messages with the size rule.
pub fn split_server_buffer(buf: &[u8]) -> Result<Split<'_>, SplitError> {
    let mut messages = Vec::new();
    let mut at = 0;
    while at < buf.len() {
        let rest = &buf[at..];
        let size = match server_size(rest) {
            Size::Bytes(n) => n,
            // S→C rules never come out negative.
            Size::Invalid | Size::Incomplete | Size::Negative(_) => break,
        };
        if size > MAX_MESSAGE {
            return Err(SplitError::TooLarge { at, size });
        }
        if size > rest.len() {
            return Err(SplitError::Truncated { at, size });
        }
        messages.push(&rest[..size]);
        at += size;
    }
    Ok(Split {
        messages,
        discarded: &buf[at..],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    /// Spec test vectors, C→S sizes.
    #[test]
    fn client_size_vectors() {
        let chat = |last: u8| bytes(&[&[0x15, 0x01, 0x00], b"hi\0bob\0", &[last]]);
        assert_eq!(client_size(&chat(0x00)), Size::Bytes(11));
        assert_eq!(client_size(&chat(0x05)), Size::Bytes(16));
        assert_eq!(client_size(&chat(0xFF)), Size::Bytes(10));
        assert_eq!(client_size(&[0x66, 0x05, 0x00]), Size::Bytes(8));
        assert_eq!(client_size(&[0x66, 0xFD, 0x01]), Size::Bytes(512));
        assert_eq!(client_size(&[0x66, 0xFE, 0x01]), Size::Bytes(3));
        assert_eq!(client_size(&[0x6C, 0x10, 0, 0, 0, 0]), Size::Bytes(23));
        assert_eq!(client_size(&[0x6C, 0x10]), Size::Incomplete);
        for id in [0x2C, 0x4A, 0x64] {
            assert_eq!(client_size(&[id; 16]), Size::Invalid);
        }
        // Chat without its strings' NULs, or without the trailing byte.
        assert_eq!(client_size(&[0x15, 0x01, 0x00, b'h']), Size::Incomplete);
        assert_eq!(client_size(b"\x15\x01\x00hi\0bob\0"), Size::Incomplete);
        // A large negative trailing byte.
        assert_eq!(client_size(&chat(0x80)), Size::Negative(11 - 128));
    }

    /// Spec test vectors, classifier.
    #[test]
    fn classifier_vectors() {
        assert_eq!(classify_client(&[0x80; 5]), Classified::Invalid);
        assert_eq!(
            classify_client(&[0xFF; 16]),
            Classified::Queue(ClientQueue::Admin)
        );
        assert_eq!(
            classify_client(&[0x6B]),
            Classified::Queue(ClientQueue::System)
        );
        assert_eq!(
            classify_client(&[0x01, 0x10, 0x00, 0x20, 0x00]),
            Classified::Queue(ClientQueue::Game)
        );
        assert_eq!(
            classify_client(&[0x01, 0x10, 0x00, 0x20]),
            Classified::Incomplete
        );
        assert_eq!(classify_client(&[]), Classified::Incomplete);
        // Size 0 in the table: never queued.
        assert_eq!(classify_client(&[0x2C; 8]), Classified::Incomplete);
        // Over 0x204 is never accepted, longer buffers are (whole copy).
        assert_eq!(
            classify_client(&[0x01; 6]),
            Classified::Queue(ClientQueue::Game)
        );
        let mut chat = bytes(&[&[0x15, 0x01, 0x00], b"hi\0bob\0", &[0x05]]);
        assert_eq!(classify_client(&chat), Classified::Incomplete);
        chat.resize(16, 0);
        assert_eq!(classify_client(&chat), Classified::Queue(ClientQueue::Game));
    }

    /// Spec test vectors, S→C sizes.
    #[test]
    fn server_size_vectors() {
        let mut m94 = vec![0x94, 0x03];
        m94.extend([0; 7]);
        assert_eq!(server_size(&m94), Size::Bytes(15));
        assert_eq!(server_size(&[0xAF, 0x00]), Size::Bytes(2));
        assert_eq!(server_size(&[0xAF, 0x05]), Size::Bytes(6));
        assert_eq!(server_size(&[0xAE, 0x10, 0x00]), Size::Bytes(19));
        let m26 = bytes(&[&[0x26], &[0; 9], b"a\0bc\0"]);
        assert_eq!(server_size(&m26), Size::Bytes(15));
        assert_eq!(server_size(&[0x16, 0x20, 0x00]), Size::Incomplete);
        let mut m16 = vec![0x16, 0x20, 0x00];
        m16.resize(13, 0);
        assert_eq!(server_size(&m16), Size::Bytes(32));
        assert_eq!(server_size(&[0x80]), Size::Invalid);
        assert_eq!(server_size(&[0xB5]), Size::Invalid);
        assert_eq!(server_size(&[0x1A, 0x07]), Size::Bytes(2));
    }

    #[test]
    fn split() {
        // 0x1A (2 bytes), 0x5F (5 bytes), then an id with size 0 ends it.
        let buf = [0x1A, 0x07, 0x5F, 1, 2, 3, 4, 0x80, 0x1A, 0x07];
        let s = split_server_buffer(&buf).unwrap();
        assert_eq!(s.messages, vec![&buf[0..2], &buf[2..7]]);
        assert_eq!(s.discarded, &buf[7..]);
        assert_eq!(
            split_server_buffer(&[0x1A, 0x07, 0x5F, 1]),
            Err(SplitError::Truncated { at: 2, size: 5 })
        );
        let mut big = vec![0x16, 0x05, 0x02];
        big.resize(13, 0);
        assert_eq!(
            split_server_buffer(&big),
            Err(SplitError::TooLarge { at: 0, size: 0x205 })
        );
        assert_eq!(server_list(0xAE), ServerList::Game);
        assert_eq!(server_list(0xAF), ServerList::System);
    }
}
