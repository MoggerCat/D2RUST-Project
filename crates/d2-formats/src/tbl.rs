// Spec: specs/formats/tbl.md
//! String tables (`.tbl`).

use crate::cursor::{invalid, Cursor, FormatError};

const FORMAT: &str = "tbl";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TblHeader {
    pub crc: u16,
    pub num_elements: u16,
    pub hash_table_size: u32,
    pub version: u8,
    pub data_start: u32,
    pub max_tries: u32,
    pub file_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TblEntry {
    pub used: bool,
    pub index: u16,
    pub hash: u32,
    /// Key bytes (without the NUL). Empty for unused slots.
    pub key: Vec<u8>,
    /// Value bytes (without the NUL). Raw 8-bit text (Windows-1252 for
    /// English); `ÿc` (0xFF 'c') starts a color code.
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringTable {
    pub header: TblHeader,
    /// Element number → hash slot.
    pub indices: Vec<u16>,
    /// Hash slots.
    pub entries: Vec<TblEntry>,
}

/// The key hash (spec §Key lookup), before reduction modulo the table size.
pub fn key_hash(key: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &c in key {
        h = (h << 4).wrapping_add(u32::from(c));
        let t = h & 0xF000_0000;
        if t != 0 {
            h = (h & 0x0FFF_FFFF) ^ (t >> 24);
        }
    }
    h
}

fn cstring(data: &[u8], at: usize) -> Result<&[u8], FormatError> {
    let tail = data
        .get(at..)
        .ok_or_else(|| invalid(FORMAT, format!("key offset {at:#x} past end of file")))?;
    let len = tail
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| invalid(FORMAT, format!("unterminated key at {at:#x}")))?;
    Ok(&tail[..len])
}

impl StringTable {
    pub fn parse(data: &[u8]) -> Result<StringTable, FormatError> {
        let mut c = Cursor::new(data, FORMAT);
        let header = TblHeader {
            crc: c.u16()?,
            num_elements: c.u16()?,
            hash_table_size: c.u32()?,
            version: c.u8()?,
            data_start: c.u32()?,
            max_tries: c.u32()?,
            file_size: c.u32()?,
        };
        let indices = (0..header.num_elements)
            .map(|_| c.u16())
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(&bad) = indices
            .iter()
            .find(|&&i| u32::from(i) >= header.hash_table_size)
        {
            return Err(invalid(
                FORMAT,
                format!("index {bad} outside the hash table"),
            ));
        }
        // 17 bytes per slot; checked against the file before allocating.
        let table_bytes = u64::from(header.hash_table_size) * 17;
        if c.pos() as u64 + table_bytes > data.len() as u64 {
            return Err(invalid(FORMAT, "hash table extends past end of file"));
        }

        let mut entries = Vec::with_capacity(header.hash_table_size as usize);
        // Key and value bytes copied so far. Slots whose strings overlap
        // could otherwise copy the same bytes once per slot (quadratic in
        // the file size); strings that don't overlap never exceed it.
        let mut copied = 0usize;
        for slot in 0..header.hash_table_size {
            let used = c.u8()? != 0;
            let index = c.u16()?;
            let hash = c.u32()?;
            let key_offset = c.u32()? as usize;
            let value_offset = c.u32()? as usize;
            let value_length = usize::from(c.u16()?);
            let (key, value) = if used {
                let key = cstring(data, key_offset)
                    .map_err(|e| invalid(FORMAT, format!("slot {slot}: {e}")))?;
                let value: &[u8] = if value_length == 0 {
                    &[]
                } else {
                    value_offset
                        .checked_add(value_length - 1)
                        .and_then(|end| data.get(value_offset..end))
                        .ok_or_else(|| {
                            invalid(FORMAT, format!("slot {slot}: value past end of file"))
                        })?
                };
                copied += key.len() + value.len();
                if copied > data.len() {
                    return Err(invalid(
                        FORMAT,
                        format!("slot {slot}: strings add up to more than the file size"),
                    ));
                }
                (key.to_vec(), value.to_vec())
            } else {
                (Vec::new(), Vec::new())
            };
            entries.push(TblEntry {
                used,
                index,
                hash,
                key,
                value,
            });
        }
        Ok(StringTable {
            header,
            indices,
            entries,
        })
    }

    /// The entry for element number `i`, or `None` if out of range or empty.
    pub fn element(&self, i: usize) -> Option<&TblEntry> {
        let slot = usize::from(*self.indices.get(i)?);
        self.entries.get(slot).filter(|e| e.used)
    }

    /// Hash slot of `key`, following the original probing rule.
    pub fn find_slot(&self, key: &[u8]) -> Option<usize> {
        let size = self.header.hash_table_size as usize;
        if size == 0 {
            return None;
        }
        let mut slot = key_hash(key) as usize % size;
        // Probing past `size` slots revisits slots already seen, so the
        // answer is the same with the count capped at the table size.
        let tries = (self.header.max_tries as usize).min(size);
        for _ in 0..tries {
            let e = &self.entries[slot];
            if !e.used {
                return None;
            }
            if e.key == key {
                return Some(slot);
            }
            slot = (slot + 1) % size;
        }
        None
    }

    /// The value for `key`.
    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.find_slot(key)
            .map(|s| self.entries[s].value.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/formats/tbl.md §key-lookup
    #[test]
    fn hash_vectors() {
        assert_eq!(key_hash(b""), 0);
        assert_eq!(key_hash(b"A") % 1000, 65);
        assert_eq!(key_hash(b"AB") % 1000, 106);
    }

    /// Builds a table the way the original layout describes: header,
    /// indices, slots, then key/value strings.
    fn build(pairs: &[(&str, &str)], size: u32) -> Vec<u8> {
        let header_len = 21 + 2 * pairs.len() + 17 * size as usize;
        let mut slots: Vec<Option<(usize, u32)>> = vec![None; size as usize];
        let mut indices = Vec::new();
        for (i, (k, _)) in pairs.iter().enumerate() {
            let h = key_hash(k.as_bytes());
            let mut s = (h % size) as usize;
            while slots[s].is_some() {
                s = (s + 1) % size as usize;
            }
            slots[s] = Some((i, h));
            indices.push(s as u16);
        }
        let mut strings = Vec::new();
        let mut offsets = Vec::new();
        for (k, v) in pairs {
            let ko = header_len + strings.len();
            strings.extend_from_slice(k.as_bytes());
            strings.push(0);
            let vo = header_len + strings.len();
            strings.extend_from_slice(v.as_bytes());
            strings.push(0);
            offsets.push((ko, vo, v.len() + 1));
        }
        let total = header_len + strings.len();
        let mut d = Vec::new();
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&(pairs.len() as u16).to_le_bytes());
        d.extend_from_slice(&size.to_le_bytes());
        d.push(1);
        d.extend_from_slice(&(header_len as u32).to_le_bytes());
        d.extend_from_slice(&size.to_le_bytes());
        d.extend_from_slice(&(total as u32).to_le_bytes());
        for i in &indices {
            d.extend_from_slice(&i.to_le_bytes());
        }
        for slot in &slots {
            match slot {
                Some((i, h)) => {
                    let (ko, vo, vl) = offsets[*i];
                    d.push(1);
                    d.extend_from_slice(&(*i as u16).to_le_bytes());
                    d.extend_from_slice(&h.to_le_bytes());
                    d.extend_from_slice(&(ko as u32).to_le_bytes());
                    d.extend_from_slice(&(vo as u32).to_le_bytes());
                    d.extend_from_slice(&(vl as u16).to_le_bytes());
                }
                None => d.extend_from_slice(&[0u8; 17]),
            }
        }
        d.extend(strings);
        d
    }

    // Covers: specs/formats/tbl.md §strings, §element-access, §key-lookup
    #[test]
    fn round_trip() {
        // "A" and "Q" collide in a table of 16 (65 % 16 == 81 % 16 == 1).
        let pairs = [("A", "first"), ("Q", "second"), ("Key3", "ÿc1red")];
        let t = StringTable::parse(&build(&pairs, 16)).unwrap();
        for (i, (k, v)) in pairs.iter().enumerate() {
            let e = t.element(i).unwrap();
            assert_eq!(e.key, k.as_bytes());
            assert_eq!(e.value, v.as_bytes());
            assert_eq!(t.get(k.as_bytes()), Some(v.as_bytes()));
        }
        assert_eq!(t.get(b"missing"), None);
        assert!(t.element(3).is_none());
    }

    // Covers: specs/formats/tbl.md §header-21-bytes
    #[test]
    fn truncated_is_an_error() {
        let data = build(&[("A", "x")], 4);
        assert!(StringTable::parse(&data[..data.len() - 4]).is_err());
        assert!(StringTable::parse(&data[..30]).is_err());
    }

    #[test]
    fn regress_lookup_with_huge_max_tries() {
        // A full table and max_tries u32::MAX: a missing key probed 4
        // billion slots. Probing past the table size revisits slots, so
        // the capped probe gives the same answer.
        let mut data = build(&[("A", "x"), ("B", "y"), ("C", "z")], 3);
        data[0x0D..0x11].copy_from_slice(&u32::MAX.to_le_bytes());
        let t = StringTable::parse(&data).unwrap();
        // Under the robustness deadline (the old probe took ~45 s here).
        let t = crate::robust::bounded(move || {
            assert_eq!(t.get(b"missing"), None);
            t
        });
        assert_eq!(t.get(b"C"), Some(&b"z"[..]));
    }

    #[test]
    fn regress_slots_sharing_one_string() {
        // 64 used slots whose keys all start at one 200-byte string copied
        // it 64 times (quadratic in the file size for large tables).
        let size = 64usize;
        let strings_at = 21 + 17 * size;
        let mut d = Vec::new();
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&(size as u32).to_le_bytes());
        d.push(1);
        d.extend_from_slice(&(strings_at as u32).to_le_bytes());
        d.extend_from_slice(&(size as u32).to_le_bytes());
        d.extend_from_slice(&((strings_at + 201) as u32).to_le_bytes());
        for _ in 0..size {
            d.push(1);
            d.extend_from_slice(&[0; 6]);
            d.extend_from_slice(&(strings_at as u32).to_le_bytes());
            d.extend_from_slice(&0u32.to_le_bytes());
            d.extend_from_slice(&0u16.to_le_bytes());
        }
        d.extend_from_slice(&[b'k'; 200]);
        d.push(0);
        let err = StringTable::parse(&d).unwrap_err();
        assert!(err.to_string().contains("more than the file size"), "{err}");
    }

    mod robust {
        use super::*;
        use crate::robust::{bounded, bytes, mutated};
        use crate::robust_tests::config;
        use proptest::prelude::*;

        fn valid() -> Vec<u8> {
            build(&[("A", "first"), ("Q", "second"), ("Key3", "x")], 8)
        }

        #[test]
        fn builder_is_valid() {
            assert!(StringTable::parse(&valid()).is_ok());
        }

        proptest! {
            #![proptest_config(config(64))]

            #[test]
            fn mutated_file(data in mutated(valid()), key in bytes(8), i in any::<usize>()) {
                bounded(move || {
                    if let Ok(t) = StringTable::parse(&data) {
                        let _ = t.get(&key);
                        let _ = t.get(b"Q");
                        let _ = t.element(i);
                        let _ = t.element(i % 4);
                    }
                });
            }
        }
    }
}
