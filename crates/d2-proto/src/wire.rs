// Spec: specs/sim/intents-events.md
//! Explicit little-endian reads and writes for the typed messages of
//! [`crate::generated`] (§2.4 rule 10).

/// A typed decode that failed.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("message is empty")]
    Empty,
    #[error("id 0x{found:02X}, expected 0x{expected:02X}")]
    WrongId { expected: u8, found: u8 },
    #[error("{found} bytes, expected {expected}")]
    WrongSize { expected: usize, found: usize },
}

/// A message with a fixed size and a layout of fixed fields. Bytes the
/// layout does not list (those no 1.14d handler reads) decode to nothing
/// and encode as 0.
pub trait FixedMessage: Sized {
    const ID: u8;
    const SIZE: usize;
    /// Decodes exactly `SIZE` bytes starting with `ID`.
    fn decode(b: &[u8]) -> Result<Self, DecodeError>;
    /// Writes the message into `out` (`SIZE` bytes, zeroed first).
    /// Panics if `out` has another length or a bit field holds a value
    /// wider than its bits.
    fn write(&self, out: &mut [u8]);
}

pub(crate) fn check(b: &[u8], id: u8, size: usize) -> Result<(), DecodeError> {
    match b.first() {
        None => Err(DecodeError::Empty),
        Some(&f) if f != id => Err(DecodeError::WrongId {
            expected: id,
            found: f,
        }),
        _ if b.len() != size => Err(DecodeError::WrongSize {
            expected: size,
            found: b.len(),
        }),
        _ => Ok(()),
    }
}

pub(crate) fn start(out: &mut [u8], id: u8, size: usize) {
    assert_eq!(out.len(), size, "output buffer size");
    out.fill(0);
    out[0] = id;
}

pub(crate) fn u8_at(b: &[u8], off: usize) -> u8 {
    b[off]
}

pub(crate) fn u16_at(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

pub(crate) fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

pub(crate) fn bytes16_at(b: &[u8], off: usize) -> [u8; 16] {
    let mut a = [0; 16];
    a.copy_from_slice(&b[off..off + 16]);
    a
}

/// ORs `v`'s little-endian bytes in at `off` (fields share no bit, so
/// ORing builds the message in any order).
fn or_bytes(out: &mut [u8], off: usize, v: &[u8]) {
    for (o, b) in out[off..off + v.len()].iter_mut().zip(v) {
        *o |= b;
    }
}

pub(crate) fn put_u8(out: &mut [u8], off: usize, v: u8) {
    or_bytes(out, off, &[v]);
}

pub(crate) fn put_u16(out: &mut [u8], off: usize, v: u16) {
    or_bytes(out, off, &v.to_le_bytes());
}

pub(crate) fn put_u32(out: &mut [u8], off: usize, v: u32) {
    or_bytes(out, off, &v.to_le_bytes());
}

pub(crate) fn put_bytes16(out: &mut [u8], off: usize, v: &[u8; 16]) {
    or_bytes(out, off, v);
}

/// Bits `0..n` of the u32 at `off`.
pub(crate) fn bits_at(b: &[u8], off: usize, n: u32) -> u32 {
    u32_at(b, off) & ((1u32 << n) - 1)
}

/// Bit `n` of the u32 at `off`.
pub(crate) fn bit_at(b: &[u8], off: usize, n: u32) -> bool {
    u32_at(b, off) >> n & 1 == 1
}

pub(crate) fn put_bits(out: &mut [u8], off: usize, n: u32, v: u32) {
    assert!(v < 1u32 << n, "value {v} does not fit in {n} bits");
    put_u32(out, off, v);
}

pub(crate) fn put_bit(out: &mut [u8], off: usize, n: u32, v: bool) {
    put_u32(out, off, (v as u32) << n);
}
