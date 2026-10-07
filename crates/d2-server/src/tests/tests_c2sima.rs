// Spec: specs/sim/intents-events.md
//! Coverage claims for `intents-events.md` §3 (S→C ids that cannot be
//! received, buffer order).

use super::fakes::*;
use crate::buffers::ClientBuffers;
use crate::seams::*;

// Covers: specs/sim/intents-events.md §3.1 r3
#[test]
fn size_zero_ids_are_not_receivable() {
    let s = TsvSizes::new();
    for id in [0x83u8, 0x84, 0x88] {
        assert_eq!(s.server_size(&[id; 16]), Err(SizeError::Invalid), "{id:#x}");
    }
}

// Covers: specs/sim/intents-events.md §3.2 r4
#[test]
fn buffers_pop_in_queue_order() {
    let mut b = ClientBuffers::new();
    b.add_client(0);
    MessageSink::queue(&mut b, 0, &[0x9C; 0x200]).unwrap();
    MessageSink::queue(&mut b, 0, &[0x9D; 4]).unwrap();
    assert_eq!(b.pop(0).unwrap()[0], 0x9C);
    assert_eq!(b.pop(0).unwrap()[0], 0x9D);
    assert!(b.pop(0).is_none());
}
