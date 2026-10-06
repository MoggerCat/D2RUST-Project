// Spec: specs/formats/mpq.md §1, §5–§9
//! A synthetic MPQ archive for `benches/formats.rs` (criterion). Only with
//! the `bench-fixtures` feature; built from the spec's layout, never from
//! game files.

use super::crypto::{encrypt, hash, HashType, BLOCK_TABLE_KEY, HASH_TABLE_KEY};
use super::{compression, flags, huffman};

/// Sector size of the archive (header shift 0: 512 bytes).
pub const SECTOR: usize = 512;
const HASH_COUNT: usize = 16;

/// Names of the synthetic files, with their plain contents.
pub fn files() -> Vec<(&'static str, Vec<u8>)> {
    let words = [
        "strength",
        "dexterity",
        "vitality",
        "energy",
        "life",
        "mana",
    ];
    let mut text = Vec::new();
    let mut i = 0usize;
    while text.len() < 256 * 1024 {
        text.extend_from_slice(words[i % words.len()].as_bytes());
        text.push(if i.is_multiple_of(7) { b'\n' } else { b'\t' });
        i = i.wrapping_mul(31).wrapping_add(17);
    }
    let plain: Vec<u8> = (0..64 * 1024u32).map(|i| (i * 13) as u8).collect();
    vec![
        ("data\\global\\excel\\big.txt", text),
        ("data\\global\\plain.bin", plain),
    ]
}

fn key_for(name: &str) -> u32 {
    let plain = name.rsplit(['\\', '/']).next().unwrap_or(name);
    hash(plain.as_bytes(), HashType::FileKey)
}

/// A sectored, Huffman-compressed, encrypted block (§8, §9).
fn compressed_block(name: &str, c: &[u8]) -> Vec<u8> {
    let sectors: Vec<Vec<u8>> = c
        .chunks(SECTOR)
        .map(|chunk| {
            let mut s = vec![compression::HUFFMAN];
            s.extend_from_slice(&huffman::compress(0, chunk));
            if s.len() < chunk.len() {
                s
            } else {
                chunk.to_vec()
            }
        })
        .collect();
    let key = key_for(name);
    let mut offsets = Vec::new();
    let mut pos = (sectors.len() + 1) * 4;
    for s in &sectors {
        offsets.push(pos as u32);
        pos += s.len();
    }
    offsets.push(pos as u32);
    let mut table: Vec<u8> = offsets.iter().flat_map(|o| o.to_le_bytes()).collect();
    encrypt(&mut table, key.wrapping_sub(1));
    let mut out = table;
    for (i, s) in sectors.iter().enumerate() {
        let mut s = s.clone();
        encrypt(&mut s, key.wrapping_add(i as u32));
        out.extend_from_slice(&s);
    }
    out
}

/// The archive bytes of [`files`]: the first file compressed and
/// encrypted, the second stored plain.
pub fn archive() -> Vec<u8> {
    let files = files();
    let mut data = Vec::new();
    let mut blocks: Vec<[u32; 4]> = Vec::new();
    for (i, (name, c)) in files.iter().enumerate() {
        let file_pos = (32 + data.len()) as u32;
        let (f, s) = if i == 0 {
            (
                flags::EXISTS | flags::COMPRESS | flags::ENCRYPTED,
                compressed_block(name, c),
            )
        } else {
            (flags::EXISTS, c.clone())
        };
        blocks.push([file_pos, s.len() as u32, c.len() as u32, f]);
        data.extend_from_slice(&s);
    }
    let mut hash_t = vec![0xFFu8; HASH_COUNT * 16];
    for (index, (name, _)) in files.iter().enumerate() {
        let start = hash(name.as_bytes(), HashType::TableOffset) as usize;
        let slot = (0..HASH_COUNT)
            .map(|i| (start + i) % HASH_COUNT)
            .find(|&s| hash_t[s * 16 + 12..s * 16 + 16] == [0xFF; 4])
            .expect("free hash slot");
        let e = &mut hash_t[slot * 16..slot * 16 + 16];
        e[0..4].copy_from_slice(&hash(name.as_bytes(), HashType::NameA).to_le_bytes());
        e[4..8].copy_from_slice(&hash(name.as_bytes(), HashType::NameB).to_le_bytes());
        e[8..12].fill(0);
        e[12..16].copy_from_slice(&(index as u32).to_le_bytes());
    }
    let mut block_t: Vec<u8> = blocks
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    encrypt(&mut hash_t, HASH_TABLE_KEY);
    encrypt(&mut block_t, BLOCK_TABLE_KEY);
    let hash_pos = (32 + data.len()) as u32;
    let block_pos = hash_pos + hash_t.len() as u32;
    let total = block_pos + block_t.len() as u32;
    let mut out = b"MPQ\x1A".to_vec();
    for v in [0x20, total] {
        out.extend_from_slice(&u32::to_le_bytes(v));
    }
    out.extend_from_slice(&[0; 4]);
    for v in [hash_pos, block_pos, HASH_COUNT as u32, blocks.len() as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&data);
    out.extend_from_slice(&hash_t);
    out.extend_from_slice(&block_t);
    out
}
