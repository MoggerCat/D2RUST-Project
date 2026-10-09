// Spec: specs/client/msg-stats-items.md (§4, §5)
//! Hireling stats (0x9E–0xA2, §4) and the item state messages (0x3E,
//! 0x40, 0x7C, 0x7D, 0x92, 0x97, 0xA6, §5).
//!
//! The requirement refresh `0x004C1350` (§3 rule 3, open question 4),
//! the gfx refreshes and the stat-list links of items to their owners
//! (`client/stat-lists.md` §2; the item stream, open question 3) set no
//! model field yet.

use super::super::bits::BitReader;
use super::super::dispatch::{HandlerError, Message};
use super::super::world::{ClientWorld, KindData, UnitKey, ITEM, MONSTER, PLAYER};
use super::states::state_off;
use super::Bytes;

/// 0x9E–0xA2 (§4): stat u8@1, GUID u32@2, value @6 (u8, u16, u32 set;
/// u8, u16 add) on the monster (1, GUID).
pub fn merc_stat(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    let (expected, value, add) = match msg.id {
        0x9E => (7, u32::from(b.u8(6)?), false),
        0x9F => (8, u32::from(b.u16(6)?), false),
        0xA0 => (10, b.u32(6)?, false),
        0xA1 => (7, u32::from(b.u8(6)?), true),
        0xA2 => (8, u32::from(b.u16(6)?), true),
        _ => return Err(HandlerError::Invalid("not 0x9E..=0xA2")),
    };
    if msg.bytes.len() != expected {
        return Err(HandlerError::Invalid("hireling stat message size"));
    }
    let stat = u16::from(b.u8(1)?);
    let key = UnitKey::new(MONSTER, b.u32(2)?);
    let Some(u) = w.units.get_mut(&key) else {
        return Ok(());
    };
    // Rule 3: stat 12 first runs the requirement refresh of 0x47 (no
    // model field, module doc).
    let v = if add {
        u.stat(stat).wrapping_add(value as i32)
    } else {
        value as i32
    };
    u.stats.insert(stat, v);
    Ok(())
}

/// The bit-width choice of 0x3E (§5 r1): 1 bit a; 0 → 8 bits, else 1
/// bit b: 16 (b = 0) or 32.
fn sized(r: &mut BitReader<'_>) -> u32 {
    if r.read(1) == 0 {
        r.read(8)
    } else if r.read(1) == 0 {
        r.read(16)
    } else {
        r.read(32)
    }
}

/// 0x3E UpdateItemStats (§5 r1): size u8@1, the bit fields from @2.
pub fn update_item_stats(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    let size = usize::from(b.u8(1)?);
    if size != msg.bytes.len() || size < 2 {
        return Err(HandlerError::Invalid("0x3E size byte"));
    }
    let mut r = BitReader::new(&msg.bytes[2..]);
    let guid = sized(&mut r);
    let set = r.read(1) == 1;
    let stat = r.read(9) as u16;
    let value = sized(&mut r) as i32;
    let _param = if r.read(1) == 0 {
        r.read(8)
    } else {
        r.read(16)
    };
    let key = UnitKey::new(ITEM, guid);
    let local = w.local().is_some();
    let Some(u) = w.units.get_mut(&key) else {
        return Ok(());
    };
    if stat == 204 {
        // Rule 1.1 (`client/stat-lists.md` §2 r1): stat 204 goes to the
        // item's flag-0x40 list entry keyed by the layer. PROVISIONAL
        // (client/msg-stats-items.md OQ 3): the model holds no item stat
        // lists (they come from the item stream), so nothing is written;
        // settled by a Ghidra read of 0x0062E410 plus a join / trade
        // packet recording with items (HIGH-PRIORITY CAPTURE).
        return Ok(());
    }
    // Rule 1.2.
    if set {
        u.stats.insert(stat, value);
    }
    if local && stat == 70 && value > 0 {
        if let KindData::Item(d) = &mut u.kind {
            d.set_flags(4 | 0x4000, false);
        }
    }
    Ok(())
}

/// 0x40 ItemFlags (§5 r2): GUID u32@1, mask u32@5, value u32@9.
pub fn item_flags(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 13 {
        return Err(HandlerError::Invalid("0x40 is 13 bytes"));
    }
    let key = UnitKey::new(ITEM, b.u32(1)?);
    let (mask, value) = (b.u32(5)?, b.u32(9)?);
    if let Some(KindData::Item(d)) = w.units.get_mut(&key).map(|u| &mut u.kind) {
        d.set_flags(mask, value != 0);
    }
    Ok(())
}

/// `0x004C2180(unit)` (§5 r3; also 0x3F rule 2.1): a state-54 list →
/// state 54 off, the list freed.
pub fn clear_scroll_state(w: &mut ClientWorld, key: UnitKey) {
    if w.units
        .get(&key)
        .is_some_and(|u| u.state_lists.contains_key(&54))
    {
        state_off(w, key, 54);
    }
}

/// 0x7C UseScroll (§5 r3): type u8@1, GUID u32@2.
pub fn use_scroll(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 6 {
        return Err(HandlerError::Invalid("0x7C is 6 bytes"));
    }
    clear_scroll_state(w, UnitKey::new(b.u8(1)?, b.u32(2)?));
    Ok(())
}

/// 0x7D SetItemState (§5 r4): owner type u8@1, owner GUID u32@2, item
/// GUID u32@6, code u32@10, value u32@14.
pub fn set_item_state(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 18 {
        return Err(HandlerError::Invalid("0x7D is 18 bytes"));
    }
    let owner = UnitKey::new(b.u8(1)?, b.u32(2)?);
    let item = UnitKey::new(ITEM, b.u32(6)?);
    let (code, value) = (b.u32(10)?, b.u32(14)?);
    if !w.units.contains_key(&owner) {
        return Ok(());
    }
    let Some(KindData::Item(d)) = w.units.get_mut(&item).map(|u| &mut u.kind) else {
        return Ok(());
    };
    match code {
        0x100 => d.set_flags(0x100, value != 0),
        0x200 => d.set_flags(0x100, false),
        _ => {}
    }
    Ok(())
}

/// 0x92 RemoveItemsDisplay (§5 r5): type u8@1, GUID u32@2. Only a unit
/// with an inventory changes: for each node, in order, of kind 3 (body)
/// or kind 1 when the item is an active inventory item (`0x0062FF70`:
/// not broken, flag 0x4000 clear, a charm, page 0; `items/inventory.md`
/// §5.6), the item is unlinked and re-added (its place in the node list
/// moves to the end; the item unit stays in S), a body node's slot is
/// cleared, the set-item update with remove detaches the owner's set
/// list and the item's stat list is detached unless item flag 0x100 is
/// set (already detached). In the model that is
/// [`ItemData::unlinked`](super::super::world::ItemData::unlinked): the
/// item's properties stop counting until its next record re-adds it.
/// The gfx refreshes, `0x0063BEF0`, the requirement refresh `0x004C1350`
/// and the final `0x0063E0B0(inventory)` write no model field.
// PROVISIONAL (REC-416; `client/msg-stats-items.md` OQ 3): the model
// holds no inventory nodes, so the nodes are the item units whose last
// record names U as owner (a 0x9C item belongs to the local player), in
// GUID order, and a "unit with an inventory" is a player or monster in S;
// the node order and the fatal 0xD4F / 0xD5A / 0xD5B asserts have no
// model to run on; settled by a join / trade recording with items (the
// 0x92 bytes and the 0x9D that follows) and a Ghidra read of 0x0062E410.
pub fn remove_items_display(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    if msg.bytes.len() != 6 {
        return Err(HandlerError::Invalid("0x92 is 6 bytes"));
    }
    let b = Bytes(msg.bytes);
    let unit = UnitKey::new(b.u8(1)?, b.u32(2)?);
    if !matches!(unit.unit_type, PLAYER | MONSTER) || !w.units.contains_key(&unit) {
        return Ok(());
    }
    let nodes: Vec<UnitKey> = w
        .units
        .keys()
        .copied()
        .filter(|&k| k.unit_type == ITEM)
        .filter(|&k| {
            super::super::items::item(w, k).is_some_and(|v| {
                if v.store || v.owner != Some(unit) {
                    return false;
                }
                match v.mode {
                    super::super::items::mode::BODY => true,
                    // `0x0062FF70`: not broken (flag 0x100), flag 0x4000
                    // clear, a charm, page 0.
                    super::super::items::mode::STORED => {
                        v.page == 0
                            && matches!(&w.units[&k].kind,
                                KindData::Item(d) if d.charm && d.flags & 0x4100 == 0)
                    }
                    _ => false,
                }
            })
        })
        .collect();
    for k in nodes {
        if let Some(KindData::Item(d)) = w.units.get_mut(&k).map(|u| &mut u.kind) {
            d.unlinked = true;
        }
    }
    Ok(())
}

/// 0x97 WeaponSwitch (§5 r6): with `d2exp.mpq` and an expansion game,
/// `weapon_set` := 1 − itself.
pub fn weapon_switch(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    if msg.bytes.len() != 1 {
        return Err(HandlerError::Invalid("0x97 is 1 byte"));
    }
    if msg.inputs.expansion_installed && w.expansion != 0 {
        w.weapon_set = 1 - w.weapon_set.min(1);
    }
    Ok(())
}

/// The runtime item table entry size (§5 r7).
pub const ITEM_ENTRY: usize = 0x120;

/// 0xA6 (§5 r7): code u8@1, size u16@2, index u16@4, record @6 (0x120
/// bytes always copied; a shorter message is a handler error).
pub fn item_table_entry(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    let size = usize::from(b.u16(2)?);
    if size != msg.bytes.len() || size < 4 {
        return Err(HandlerError::Invalid("0xA6 size word"));
    }
    if b.u8(1)? != 0 {
        return Ok(());
    }
    let index = usize::from(b.u16(4)?);
    let record = b
        .slice(6, ITEM_ENTRY)
        .map_err(|_| HandlerError::Invalid("0xA6 shorter than 0x126 bytes (1.14d reads past it)"))?
        .to_vec();
    let t = &mut w.item_table_ext;
    if index >= t.len() {
        t.resize(index + 1, vec![0; ITEM_ENTRY]);
    }
    t[index] = record;
    Ok(())
}
