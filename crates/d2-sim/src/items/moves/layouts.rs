// Spec: specs/items/inventory-moves.md
//! S→C byte layouts of §11 and the gold messages of §10.3. Little-endian;
//! the machine copy is `sim/server-messages.tsv` (checked in the tests).

use super::{Guid, MoveFatal, MAX_ITEM_MSG};

/// 0x9C: [1] action, [2] total size, [3] category, [4..7] item GUID,
/// [8..] item bit stream.
pub fn item_world(action: u8, category: u8, item: Guid, bits: &[u8]) -> Result<Vec<u8>, MoveFatal> {
    let size = 8 + bits.len();
    if size >= MAX_ITEM_MSG {
        return Err(MoveFatal::MessageSize(size));
    }
    let mut b = vec![0x9C, action, size as u8, category];
    b.extend_from_slice(&item.to_le_bytes());
    b.extend_from_slice(bits);
    Ok(b)
}

/// 0x9D: [1] action, [2] total size, [3] category, [4..7] item GUID,
/// [8] owner type, [9..12] owner GUID, [13..] item bit stream.
pub fn item_owned(
    action: u8,
    category: u8,
    item: Guid,
    owner_type: u8,
    owner: Guid,
    bits: &[u8],
) -> Result<Vec<u8>, MoveFatal> {
    let size = 13 + bits.len();
    if size >= MAX_ITEM_MSG {
        return Err(MoveFatal::MessageSize(size));
    }
    let mut b = vec![0x9D, action, size as u8, category];
    b.extend_from_slice(&item.to_le_bytes());
    b.push(owner_type);
    b.extend_from_slice(&owner.to_le_bytes());
    b.extend_from_slice(bits);
    Ok(b)
}

/// 0x7D (18 bytes): [1] owner type, [2..5] owner GUID, [6..9] item GUID,
/// [10..13] flag, [14..17] state.
pub fn item_state(owner_type: u8, owner: Guid, item: Guid, flag: u32, state: u32) -> Vec<u8> {
    let mut b = vec![0x7D, owner_type];
    b.extend_from_slice(&owner.to_le_bytes());
    b.extend_from_slice(&item.to_le_bytes());
    b.extend_from_slice(&flag.to_le_bytes());
    b.extend_from_slice(&state.to_le_bytes());
    b
}

fn relator(id: u8, unit_type: u8, arg: u8, unit: Guid) -> Vec<u8> {
    let mut b = vec![id, unit_type, arg];
    b.extend_from_slice(&unit.to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b
}

/// 0x47 (11 bytes): [1] unit type, [2] 0, [3..6] GUID, [7..10] 0.
pub fn relator1(unit_type: u8, unit: Guid) -> Vec<u8> {
    relator(0x47, unit_type, 0, unit)
}

/// 0x48 (11 bytes): [1] unit type, [2] argument, [3..6] GUID, [7..10] 0.
pub fn relator2(unit_type: u8, arg: u8, unit: Guid) -> Vec<u8> {
    relator(0x48, unit_type, arg, unit)
}

/// 0x42 (6 bytes): [1] unit type, [2..5] GUID.
pub fn clear_cursor(unit_type: u8, unit: Guid) -> Vec<u8> {
    let mut b = vec![0x42, unit_type];
    b.extend_from_slice(&unit.to_le_bytes());
    b
}

/// 0x3F (8 bytes): [1] code, [2..5] item GUID, [6..7] u16 argument.
pub fn use_stackable(code: u8, item: Guid, arg: u16) -> Vec<u8> {
    let mut b = vec![0x3F, code];
    b.extend_from_slice(&item.to_le_bytes());
    b.extend_from_slice(&arg.to_le_bytes());
    b
}

/// Gold sync `0x0053E9B0(new, old)` (§10.3): `None` when unchanged.
pub fn gold(new: u32, old: u32) -> Option<Vec<u8>> {
    if new == old {
        return None;
    }
    let delta = i64::from(new) - i64::from(old);
    Some(if (1..=254).contains(&delta) {
        vec![0x19, delta as u8]
    } else if new < 0xFF {
        vec![0x1D, 0x0E, new as u8]
    } else if new < 0xFFFF {
        let mut b = vec![0x1E, 0x0E];
        b.extend_from_slice(&(new as u16).to_le_bytes());
        b
    } else {
        let mut b = vec![0x1F, 0x0E];
        b.extend_from_slice(&new.to_le_bytes());
        b
    })
}

/// "Can't do that" `0x00549A60` (§7.4 step 2): S→C 0x5A, 40 bytes
/// `5A 0E 01` then 37 zero bytes.
pub fn cant_do_that() -> Vec<u8> {
    let mut b = vec![0u8; 40];
    b[0] = 0x5A;
    b[1] = 0x0E;
    b[2] = 0x01;
    b
}

/// S→C 0x7C (6 bytes, `0x0053B3D0`): [1] unit type, [2..5] GUID (§7.18).
pub fn item_used(unit_type: u8, unit: Guid) -> Vec<u8> {
    let mut b = vec![0x7C, unit_type];
    b.extend_from_slice(&unit.to_le_bytes());
    b
}

/// Bytes queued for every S→C 0x3E (`0x0053D130`, `sim/intents-events.md`
/// edge case 13: `push 0x22`).
pub const UPDATE_ITEM_STATS_LEN: usize = 0x22;

/// S→C 0x3E UpdateItemStats `0x0053D130(client, item, 1, stat, value,
/// param)`: size u8@1 = stream length + 2, then the bit fields of
/// `client/msg-stats-items.md` §5 r1 (GUID, set flag, 9-bit stat, value,
/// param), zero-padded to [`UPDATE_ITEM_STATS_LEN`] bytes.
///
/// PROVISIONAL (`client/msg-stats-items.md` §5 r1, REC-289): the reader
/// is specified, the writer's choice of width is not; read as the
/// smallest that holds the field (GUID / value: 8 bits below 0x100, 16
/// below 0x10000, else 32; param: 8 below 0x100, else 16). Settled by a
/// recording of a stack merge's 0x3E bytes.
pub fn update_item_stat(item: Guid, stat: u16, value: i32, param: u32) -> Vec<u8> {
    use crate::items::bitstream::BitWriter;
    let mut w = BitWriter::new(UPDATE_ITEM_STATS_LEN - 2);
    let sized = |w: &mut BitWriter, v: u32| {
        if v < 0x100 {
            w.raw(1, 0);
            w.raw(8, v);
        } else if v < 0x1_0000 {
            w.raw(1, 1);
            w.raw(1, 0);
            w.raw(16, v);
        } else {
            w.raw(1, 1);
            w.raw(1, 1);
            w.raw(32, v);
        }
    };
    sized(&mut w, item);
    w.raw(1, 1);
    w.raw(9, u32::from(stat));
    sized(&mut w, value as u32);
    if param < 0x100 {
        w.raw(1, 0);
        w.raw(8, param);
    } else {
        w.raw(1, 1);
        w.raw(16, param);
    }
    w.pad_to_byte();
    let stream = w.finish().unwrap_or_default();
    let mut out = vec![0u8; UPDATE_ITEM_STATS_LEN];
    out[0] = 0x3E;
    out[1] = (stream.len() + 2) as u8;
    out[2..2 + stream.len()].copy_from_slice(&stream);
    out
}
