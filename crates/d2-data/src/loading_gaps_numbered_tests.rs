// Spec: specs/data/loading.md "Edge cases & original bugs" 3, "Test vectors"
//! Gap tests for `loading.md` rule units no claim named before. Synthetic
//! bytes only.

use crate::txt::{ErrorCode, TxtTable};

fn fails(data: &[u8]) -> (ErrorCode, Option<usize>) {
    let e = TxtTable::parse("data\\global\\excel\\t.txt", data).unwrap_err();
    (e.code, e.line)
}

/// The original-only reader behaviors (an unterminated last line and the
/// `Expansion` test 1.14d runs on it) are not reproduced: such a file is
/// rejected, so no record is dropped. The `loading.md` vector: the
/// `Expansion` row is not the last line, the unterminated `3\t4` is, and
/// the file fails with E6 at line 4 instead of yielding one record.
// Covers: specs/data/loading.md §edge-cases-original-bugs r3
#[test]
fn original_only_reader_behaviors_are_rejected() {
    assert_eq!(
        fails(b"a\tb\r\n1\t2\r\nExpansion\r\n3\t4"),
        (ErrorCode::E6, Some(4))
    );
    // The quirk's own case: a final unterminated `Expansion` line would
    // make 1.14d drop the last real record.
    assert_eq!(
        fails(b"a\tb\r\n1\t2\r\n3\t4\r\nExpansion"),
        (ErrorCode::E6, Some(4))
    );
    // Terminated, the same rows load: `Expansion` removed, both records kept.
    let t = TxtTable::parse("t.txt", b"a\tb\r\n1\t2\r\n3\t4\r\nExpansion\r\n").unwrap();
    assert_eq!(t.records.len(), 2);
    assert_eq!(t.records[1].cells, [b"3".to_vec(), b"4".to_vec()]);
}
