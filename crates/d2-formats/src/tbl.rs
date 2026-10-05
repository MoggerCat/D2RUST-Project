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
        for slot in 0..header.hash_table_size {
            let used = c.u8()? != 0;
            let index = c.u16()?;
            let hash = c.u32()?;
            let key_offset = c.u32()? as usize;
            let value_offset = c.u32()? as usize;
            let value_length = usize::from(c.u16()?);
            let (key, value) = if used {
                let key = cstring(data, key_offset)
                    .map_err(|e| invalid(FORMAT, format!("slot {slot}: {e}")))?
                    .to_vec();
                let value = if value_length == 0 {
                    Vec::new()
                } else {
                    data.get(value_offset..value_offset + value_length - 1)
                        .ok_or_else(|| {
                            invalid(FORMAT, format!("slot {slot}: value past end of file"))
                        })?
                        .to_vec()
                };
                (key, value)
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
        for _ in 0..self.header.max_tries {
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

    #[test]
    fn truncated_is_an_error() {
        let data = build(&[("A", "x")], 4);
        assert!(StringTable::parse(&data[..data.len() - 4]).is_err());
        assert!(StringTable::parse(&data[..30]).is_err());
    }
}
