// Spec: specs/formats/mpq.md
//! Read-only access to MPQ archives (format version 0, as used by D2).
//!
//! ```no_run
//! # fn main() -> Result<(), d2_formats::mpq::MpqError> {
//! let archive = d2_formats::mpq::Archive::open("game/d2data.mpq")?;
//! let bytes = archive.read(r"data\global\excel\armor.txt")?;
//! # Ok(()) }
//! ```

mod adpcm;
mod bits;
pub mod crypto;
mod explode;
mod huffman;
#[cfg(test)]
mod robust_tests;
mod set;
mod tables;

pub use set::{
    archive_file_name, priority, search_order, ArchiveSet, ArchiveSpec, OPEN_ORDER, PRIORITY,
};

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crypto::{decrypt, hash, HashType, BLOCK_TABLE_KEY, CRYPT_TABLE, HASH_TABLE_KEY};

/// Block table flags (§6).
pub mod flags {
    pub const IMPLODE: u32 = 0x0000_0100;
    pub const COMPRESS: u32 = 0x0000_0200;
    pub const ENCRYPTED: u32 = 0x0001_0000;
    pub const FIX_KEY: u32 = 0x0002_0000;
    pub const PATCH_FILE: u32 = 0x0010_0000;
    pub const SINGLE_UNIT: u32 = 0x0100_0000;
    pub const DELETE_MARKER: u32 = 0x0200_0000;
    pub const SECTOR_CRC: u32 = 0x0400_0000;
    pub const EXISTS: u32 = 0x8000_0000;
}

/// Sector compression mask bits (§9).
pub mod compression {
    pub const HUFFMAN: u8 = 0x01;
    pub const ZLIB: u8 = 0x02;
    pub const PKWARE: u8 = 0x08;
    pub const BZIP2: u8 = 0x10;
    pub const SPARSE: u8 = 0x20;
    pub const ADPCM_MONO: u8 = 0x40;
    pub const ADPCM_STEREO: u8 = 0x80;
    pub const LZMA: u8 = 0x12;
}

const HEADER_MAGIC: [u8; 4] = *b"MPQ\x1A";
const USER_DATA_MAGIC: [u8; 4] = *b"MPQ\x1B";
const HASH_EMPTY: u32 = 0xFFFF_FFFF;
const HASH_DELETED: u32 = 0xFFFF_FFFE;

#[derive(Debug, thiserror::Error)]
pub enum MpqError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("not an MPQ archive")]
    NotAnArchive,
    #[error("unsupported MPQ format version {0}")]
    UnsupportedVersion(u16),
    #[error("corrupt archive: {0}")]
    Corrupt(String),
    #[error("file not found: {0}")]
    NotFound(String),
    #[error("block index {0} out of range")]
    BadBlockIndex(usize),
    #[error("encrypted block {0} needs a key")]
    KeyRequired(usize),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("block {block}: {source}")]
    Codec { block: usize, source: CodecError },
}

/// A decompressor rejected its input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{codec}: {reason}")]
pub struct CodecError {
    pub codec: &'static str,
    pub reason: &'static str,
}

/// Archive header (§1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Offset of the archive inside the file (`A` in the spec).
    pub offset: u64,
    pub header_size: u32,
    pub archive_size: u32,
    pub format_version: u16,
    pub sector_size_shift: u16,
    pub hash_table_pos: u32,
    pub block_table_pos: u32,
    pub hash_table_count: u32,
    pub block_table_count: u32,
}

/// Hash table entry (§5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashEntry {
    pub name_a: u32,
    pub name_b: u32,
    pub locale: u16,
    pub platform: u16,
    pub block_index: u32,
}

impl HashEntry {
    pub fn is_empty(&self) -> bool {
        self.block_index == HASH_EMPTY
    }
    pub fn is_deleted(&self) -> bool {
        self.block_index == HASH_DELETED
    }
}

/// Block table entry (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockEntry {
    pub file_pos: u32,
    pub compressed_size: u32,
    pub file_size: u32,
    pub flags: u32,
}

impl BlockEntry {
    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
    fn is_compressed(&self) -> bool {
        self.has(flags::COMPRESS | flags::IMPLODE)
    }
}

/// Counts of the compression masks seen while reading, indexed by mask byte.
/// Index 0 counts stored (uncompressed) sectors. Used by survey tooling.
#[derive(Debug, Clone)]
pub struct SectorStats {
    pub masks: [u64; 256],
    /// Sectors decompressed with the IMPLODE block flag (no mask byte).
    pub imploded: u64,
}

impl Default for SectorStats {
    fn default() -> Self {
        Self {
            masks: [0; 256],
            imploded: 0,
        }
    }
}

/// An open MPQ archive. Reads use positional I/O, so `&Archive` can be
/// shared between threads.
#[derive(Debug)]
pub struct Archive {
    path: PathBuf,
    file: File,
    file_len: u64,
    header: Header,
    hash_table: Vec<HashEntry>,
    block_table: Vec<BlockEntry>,
}

fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[cfg(unix)]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.read_exact_at(buf, offset)
}

#[cfg(windows)]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    let mut done = 0;
    while done < buf.len() {
        let n = file.seek_read(&mut buf[done..], offset + done as u64)?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        done += n;
    }
    Ok(())
}

fn corrupt(msg: impl Into<String>) -> MpqError {
    MpqError::Corrupt(msg.into())
}

impl Archive {
    /// Opens an archive and reads its header and tables.
    pub fn open(path: impl AsRef<Path>) -> Result<Archive, MpqError> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        let file_len = file.metadata()?.len();
        let header = Self::find_header(&file, file_len)?;

        let hash_bytes = Self::read_table(
            &file,
            file_len,
            header.offset + u64::from(header.hash_table_pos),
            header.hash_table_count,
            HASH_TABLE_KEY,
            "hash table",
        )?;
        let hash_table = hash_bytes
            .as_chunks::<16>()
            .0
            .iter()
            .map(|e| HashEntry {
                name_a: le_u32(e, 0),
                name_b: le_u32(e, 4),
                locale: le_u16(e, 8),
                platform: le_u16(e, 10),
                block_index: le_u32(e, 12),
            })
            .collect();

        let block_bytes = Self::read_table(
            &file,
            file_len,
            header.offset + u64::from(header.block_table_pos),
            header.block_table_count,
            BLOCK_TABLE_KEY,
            "block table",
        )?;
        let block_table = block_bytes
            .as_chunks::<16>()
            .0
            .iter()
            .map(|e| BlockEntry {
                file_pos: le_u32(e, 0),
                compressed_size: le_u32(e, 4),
                file_size: le_u32(e, 8),
                flags: le_u32(e, 12),
            })
            .collect();

        Ok(Archive {
            path,
            file,
            file_len,
            header,
            hash_table,
            block_table,
        })
    }

    /// Finds and validates the header at the first 0x200-aligned "MPQ\x1A"
    /// (following a user-data header if one is found first).
    fn find_header(file: &File, file_len: u64) -> Result<Header, MpqError> {
        const CHUNK: u64 = 1 << 20;
        let mut chunk = vec![0u8; CHUNK.min(file_len) as usize];
        let mut base = 0u64;
        while base + 32 <= file_len {
            let len = CHUNK.min(file_len - base) as usize;
            read_exact_at(file, &mut chunk[..len], base)?;
            let mut at = 0usize;
            while at + 4 <= len {
                let magic = &chunk[at..at + 4];
                let candidate = base + at as u64;
                if magic == HEADER_MAGIC {
                    return Self::parse_header(file, file_len, candidate);
                }
                if magic == USER_DATA_MAGIC && at + 12 <= len {
                    let header_offset = u64::from(le_u32(&chunk, at + 8));
                    return Self::parse_header(file, file_len, candidate + header_offset);
                }
                at += 0x200;
            }
            base += CHUNK;
        }
        Err(MpqError::NotAnArchive)
    }

    fn parse_header(file: &File, file_len: u64, offset: u64) -> Result<Header, MpqError> {
        if offset + 32 > file_len {
            return Err(MpqError::NotAnArchive);
        }
        let mut h = [0u8; 32];
        read_exact_at(file, &mut h, offset)?;
        if h[0..4] != HEADER_MAGIC {
            return Err(MpqError::NotAnArchive);
        }
        let header = Header {
            offset,
            header_size: le_u32(&h, 0x04),
            archive_size: le_u32(&h, 0x08),
            format_version: le_u16(&h, 0x0C),
            sector_size_shift: le_u16(&h, 0x0E),
            hash_table_pos: le_u32(&h, 0x10),
            block_table_pos: le_u32(&h, 0x14),
            hash_table_count: le_u32(&h, 0x18),
            block_table_count: le_u32(&h, 0x1C),
        };
        if header.header_size < 0x20 {
            return Err(corrupt(format!("header size {:#x}", header.header_size)));
        }
        if header.format_version != 0 {
            return Err(MpqError::UnsupportedVersion(header.format_version));
        }
        if header.sector_size_shift > 15 {
            return Err(corrupt(format!(
                "sector size shift {}",
                header.sector_size_shift
            )));
        }
        if !header.hash_table_count.is_power_of_two() {
            return Err(corrupt(format!(
                "hash table count {} is not a power of two",
                header.hash_table_count
            )));
        }
        Ok(header)
    }

    fn read_table(
        file: &File,
        file_len: u64,
        pos: u64,
        count: u32,
        key: u32,
        what: &str,
    ) -> Result<Vec<u8>, MpqError> {
        let len = u64::from(count) * 16;
        if pos.checked_add(len).is_none_or(|end| end > file_len) {
            return Err(corrupt(format!("{what} extends past end of file")));
        }
        let mut bytes = vec![0u8; len as usize];
        read_exact_at(file, &mut bytes, pos)?;
        decrypt(&mut bytes, key);
        Ok(bytes)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn hash_table(&self) -> &[HashEntry] {
        &self.hash_table
    }

    pub fn block_table(&self) -> &[BlockEntry] {
        &self.block_table
    }

    pub fn sector_size(&self) -> u32 {
        0x200 << self.header.sector_size_shift
    }

    /// Looks up `name` (§5) and returns its block index.
    pub fn find(&self, name: &str) -> Option<usize> {
        let bytes = name.as_bytes();
        let count = self.hash_table.len();
        let start = hash(bytes, HashType::TableOffset) as usize & (count - 1);
        let a = hash(bytes, HashType::NameA);
        let b = hash(bytes, HashType::NameB);

        let mut first = None;
        for i in 0..count {
            let e = &self.hash_table[(start + i) & (count - 1)];
            if e.is_empty() {
                break;
            }
            if e.is_deleted() {
                continue;
            }
            if e.name_a == a && e.name_b == b && (e.block_index as usize) < self.block_table.len() {
                if e.locale == 0 {
                    return Some(e.block_index as usize);
                }
                first.get_or_insert(e.block_index as usize);
            }
        }
        first
    }

    pub fn contains(&self, name: &str) -> bool {
        self.find(name)
            .is_some_and(|i| !self.block_table[i].has(flags::DELETE_MARKER))
    }

    /// Reads a file by name.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, MpqError> {
        let index = self
            .find(name)
            .ok_or_else(|| MpqError::NotFound(name.to_owned()))?;
        if self.block_table[index].has(flags::DELETE_MARKER) {
            return Err(MpqError::NotFound(name.to_owned()));
        }
        let key = self.file_key(name, index);
        self.read_block(index, key)
    }

    /// The encryption key for `name` stored in block `index` (§7), or
    /// `None` if the block isn't encrypted.
    pub fn file_key(&self, name: &str, index: usize) -> Option<u32> {
        let block = self.block_table.get(index)?;
        if !block.has(flags::ENCRYPTED) {
            return None;
        }
        let plain = name.rsplit(['\\', '/']).next().unwrap_or(name);
        let base = hash(plain.as_bytes(), HashType::FileKey);
        Some(if block.has(flags::FIX_KEY) {
            base.wrapping_add(block.file_pos) ^ block.file_size
        } else {
            base
        })
    }

    /// Reads block `index`. `key` is required if the block is encrypted.
    pub fn read_block(&self, index: usize, key: Option<u32>) -> Result<Vec<u8>, MpqError> {
        self.read_block_impl(index, key, None)
    }

    /// Like [`Archive::read_block`], also counting compression masks.
    pub fn read_block_stats(
        &self,
        index: usize,
        key: Option<u32>,
        stats: &mut SectorStats,
    ) -> Result<Vec<u8>, MpqError> {
        self.read_block_impl(index, key, Some(stats))
    }

    fn read_raw(&self, block: &BlockEntry, index: usize) -> Result<Vec<u8>, MpqError> {
        let start = self.header.offset + u64::from(block.file_pos);
        let len = u64::from(block.compressed_size);
        if start + len > self.file_len {
            return Err(corrupt(format!("block {index} extends past end of file")));
        }
        let mut data = vec![0u8; len as usize];
        read_exact_at(&self.file, &mut data, start)?;
        Ok(data)
    }

    fn read_block_impl(
        &self,
        index: usize,
        key: Option<u32>,
        mut stats: Option<&mut SectorStats>,
    ) -> Result<Vec<u8>, MpqError> {
        let block = *self
            .block_table
            .get(index)
            .ok_or(MpqError::BadBlockIndex(index))?;
        if !block.has(flags::EXISTS) {
            return Err(corrupt(format!("block {index} is not in use")));
        }
        if block.has(flags::PATCH_FILE) {
            return Err(MpqError::Unsupported(format!(
                "block {index} is a patch file"
            )));
        }
        let file_size = block.file_size as usize;
        if file_size == 0 {
            return Ok(Vec::new());
        }
        let encrypted = block.has(flags::ENCRYPTED);
        let key = match (encrypted, key) {
            (true, None) => return Err(MpqError::KeyRequired(index)),
            (true, Some(k)) => k,
            (false, _) => 0,
        };
        let mut data = self.read_raw(&block, index)?;
        let sector_size = self.sector_size() as usize;
        let codec = |source| MpqError::Codec {
            block: index,
            source,
        };

        if block.has(flags::SINGLE_UNIT) {
            if encrypted {
                decrypt(&mut data, key);
            }
            if block.is_compressed() && data.len() < file_size {
                return decompress_sector(&block, &data, file_size, stats.as_deref_mut())
                    .map_err(codec);
            }
            if data.len() < file_size {
                return Err(corrupt(format!(
                    "block {index} is shorter than its file size"
                )));
            }
            data.truncate(file_size);
            return Ok(data);
        }

        let sectors = file_size.div_ceil(sector_size);
        // Grows with decoded sectors: `file_size` is untrusted.
        let mut out = Vec::new();

        if block.is_compressed() {
            let entries = sectors + 1 + usize::from(block.has(flags::SECTOR_CRC));
            let table_len = entries * 4;
            if data.len() < table_len {
                return Err(corrupt(format!("block {index}: offset table truncated")));
            }
            let mut table = data[..table_len].to_vec();
            if encrypted {
                decrypt(&mut table, key.wrapping_sub(1));
            }
            let offsets: Vec<usize> = table
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c) as usize)
                .collect();
            if offsets[0] != table_len {
                return Err(corrupt(format!(
                    "block {index}: offset table starts at {:#x}, expected {table_len:#x}",
                    offsets[0]
                )));
            }
            if offsets.windows(2).any(|w| w[1] < w[0]) || offsets[sectors] > data.len() {
                return Err(corrupt(format!("block {index}: invalid sector offsets")));
            }
            for i in 0..sectors {
                let expected = sector_size.min(file_size - i * sector_size);
                let sector = &mut data[offsets[i]..offsets[i + 1]];
                if encrypted {
                    decrypt(sector, key.wrapping_add(i as u32));
                }
                if sector.len() < expected {
                    let bytes = decompress_sector(&block, sector, expected, stats.as_deref_mut())
                        .map_err(codec)?;
                    out.extend_from_slice(&bytes);
                } else if sector.len() == expected {
                    if let Some(s) = stats.as_deref_mut() {
                        s.masks[0] += 1;
                    }
                    out.extend_from_slice(sector);
                } else {
                    return Err(corrupt(format!(
                        "block {index}: sector {i} is longer than its decompressed size"
                    )));
                }
            }
        } else {
            if data.len() < file_size {
                return Err(corrupt(format!(
                    "block {index} is shorter than its file size"
                )));
            }
            data.truncate(file_size);
            if encrypted {
                for (i, chunk) in data.chunks_mut(sector_size).enumerate() {
                    decrypt(chunk, key.wrapping_add(i as u32));
                }
            }
            if let Some(s) = stats {
                s.masks[0] += sectors as u64;
            }
            out = data;
        }

        if out.len() != file_size {
            return Err(corrupt(format!(
                "block {index}: decoded {} bytes, expected {file_size}",
                out.len()
            )));
        }
        Ok(out)
    }

    /// Recovers the key of an encrypted, compressed block without its name
    /// (§13). Returns `None` if no candidate decodes.
    pub fn recover_key(&self, index: usize) -> Result<Option<u32>, MpqError> {
        let block = *self
            .block_table
            .get(index)
            .ok_or(MpqError::BadBlockIndex(index))?;
        if !block.has(flags::ENCRYPTED)
            || !block.is_compressed()
            || block.has(flags::SINGLE_UNIT)
            || block.file_size == 0
            || block.compressed_size < 8
        {
            return Ok(None);
        }
        let sector_size = self.sector_size();
        let sectors = block.file_size.div_ceil(sector_size);
        let p = (sectors + 1 + u32::from(block.has(flags::SECTOR_CRC))) * 4;

        let mut head = [0u8; 8];
        read_exact_at(
            &self.file,
            &mut head,
            self.header.offset + u64::from(block.file_pos),
        )?;
        let e0 = le_u32(&head, 0);
        let k12 = (e0 ^ p).wrapping_sub(0xEEEE_EEEE);

        for i in 0..0x100 {
            let k1 = k12.wrapping_sub(CRYPT_TABLE[0x400 + i]);
            let mut probe = head;
            decrypt(&mut probe, k1);
            if le_u32(&probe, 0) == p && le_u32(&probe, 4) <= p + sector_size {
                let candidate = k1.wrapping_add(1);
                if self.read_block(index, Some(candidate)).is_ok() {
                    return Ok(Some(candidate));
                }
            }
        }
        Ok(None)
    }

    /// File names from `(listfile)` (§14), if the archive has one.
    pub fn listfile(&self) -> Result<Option<Vec<String>>, MpqError> {
        if !self.contains("(listfile)") {
            return Ok(None);
        }
        let bytes = self.read("(listfile)")?;
        let text = String::from_utf8_lossy(&bytes);
        Ok(Some(
            text.split(['\r', '\n', ';'])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        ))
    }
}

/// Decompresses one sector (§9) to exactly `expected` bytes.
fn decompress_sector(
    block: &BlockEntry,
    sector: &[u8],
    expected: usize,
    stats: Option<&mut SectorStats>,
) -> Result<Vec<u8>, CodecError> {
    let out = if block.has(flags::IMPLODE) {
        if let Some(s) = stats {
            s.imploded += 1;
        }
        explode::explode(sector, expected)?
    } else {
        let (&mask, payload) = sector.split_first().ok_or(CodecError {
            codec: "sector",
            reason: "empty compressed sector",
        })?;
        if let Some(s) = stats {
            s.masks[usize::from(mask)] += 1;
        }
        decompress_masked(mask, payload, expected)?
    };
    if out.len() != expected {
        return Err(CodecError {
            codec: "sector",
            reason: "decompressed size does not match",
        });
    }
    Ok(out)
}

fn decompress_masked(mask: u8, payload: &[u8], expected: usize) -> Result<Vec<u8>, CodecError> {
    use compression::*;
    let unsupported = |reason| CodecError {
        codec: "sector",
        reason,
    };
    if mask == LZMA {
        return Err(unsupported("LZMA compression is not supported"));
    }
    if mask & SPARSE != 0 {
        return Err(unsupported("sparse compression is not supported"));
    }
    let known = HUFFMAN | ZLIB | PKWARE | BZIP2 | ADPCM_MONO | ADPCM_STEREO;
    if mask & !known != 0 || mask & (ADPCM_MONO | ADPCM_STEREO) == ADPCM_MONO | ADPCM_STEREO {
        return Err(unsupported("invalid compression mask"));
    }
    if mask & BZIP2 != 0 {
        return Err(unsupported("bzip2 compression is not supported yet"));
    }
    if mask & ZLIB != 0 {
        return Err(unsupported("zlib compression is not supported yet"));
    }

    // Decoders run in the fixed order of §9; each consumes the last output.
    let mut buf: Option<Vec<u8>> = None;
    if mask & PKWARE != 0 {
        buf = Some(explode::explode(
            buf.as_deref().unwrap_or(payload),
            expected,
        )?);
    }
    if mask & HUFFMAN != 0 {
        buf = Some(huffman::decompress(
            buf.as_deref().unwrap_or(payload),
            expected,
        )?);
    }
    if mask & ADPCM_STEREO != 0 {
        buf = Some(adpcm::decompress(
            buf.as_deref().unwrap_or(payload),
            2,
            expected,
        ));
    }
    if mask & ADPCM_MONO != 0 {
        buf = Some(adpcm::decompress(
            buf.as_deref().unwrap_or(payload),
            1,
            expected,
        ));
    }
    buf.ok_or(unsupported("empty compression mask"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_pkware_sector() {
        let mut sector = vec![compression::PKWARE];
        sector.extend_from_slice(&[0x00, 0x04, 0x82, 0x24, 0x25, 0x8F, 0x80, 0x7F]);
        let block = BlockEntry {
            file_pos: 0,
            compressed_size: 0,
            file_size: 13,
            flags: flags::EXISTS | flags::COMPRESS,
        };
        let out = decompress_sector(&block, &sector, 13, None).unwrap();
        assert_eq!(out, b"AIAIAIAIAIAIA");
    }

    #[test]
    fn size_mismatch_is_an_error() {
        let mut sector = vec![compression::PKWARE];
        sector.extend_from_slice(&[0x00, 0x04, 0x82, 0x24, 0x25, 0x8F, 0x80, 0x7F]);
        let block = BlockEntry {
            file_pos: 0,
            compressed_size: 0,
            file_size: 20,
            flags: flags::EXISTS | flags::COMPRESS,
        };
        assert!(decompress_sector(&block, &sector, 20, None).is_err());
    }

    #[test]
    fn rejects_unknown_masks() {
        for mask in [0x04, 0x12, 0x20, 0xC0] {
            assert!(
                decompress_masked(mask, &[0; 8], 8).is_err(),
                "mask {mask:#x}"
            );
        }
    }
}
