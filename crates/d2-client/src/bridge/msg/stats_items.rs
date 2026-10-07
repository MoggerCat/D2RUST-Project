// Spec: specs/client/msg-stats-items.md
//! Stat messages (0x19–0x20) and item messages (0x9C, 0x9D, 0x3F, 0x42,
//! 0x47, 0x48). Item placement waits for the item bit-stream spec
//! (open question 3): until then an item unit records its last message.

use d2_proto::s2c::{parse, Message as S2c};

use super::super::dispatch::{HandlerError, Message};
use super::super::world::{
    ClientUnit, ClientWorld, ItemData, ItemRecord, KindData, UnitKey, UseCursor, ITEM, PLAYER,
};
use super::Bytes;

/// The post-write hook `0x0045D4B0` (§1 rule 5): every branch (leave the
/// dead mode, level and attribute refreshes) is Phase 6 UI or mode
/// machine behaviour (open questions 2; `model.md` open question 1): no
/// model field changes.
fn hook(_: &mut ClientUnit, _stat: u16, _value: i32) {}

/// 0x19–0x1F (§1 rules 1–3): the local player's stats.
pub fn local_stat(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let m = parse(msg.bytes)?;
    let key = w
        .local_player
        .filter(|k| w.units.contains_key(k))
        .ok_or(HandlerError::Fatal(0x9AA))?;
    // total(s) (`0x00625480`, `client/stat-lists.md` §1 rule 3).
    let total = |s| w.total(key, s, 0);
    let (stat, value) = match m {
        S2c::SmallGoldPickup(m) => (14, total(14).wrapping_add(i32::from(m.delta))),
        S2c::AddExpByte(m) => (13, total(13).wrapping_add(i32::from(m.value))),
        S2c::AddExpWord(m) => (13, total(13).wrapping_add(i32::from(m.value))),
        S2c::AddExpDword(m) => (13, m.value as i32),
        S2c::SetStatByte(m) => (u16::from(m.stat), i32::from(m.value)),
        S2c::SetStatWord(m) => (u16::from(m.stat), i32::from(m.value)),
        S2c::SetStatDword(m) => (u16::from(m.stat), m.value as i32),
        _ => return Err(HandlerError::Invalid("not 0x19..=0x1F")),
    };
    let u = w.units.get_mut(&key).expect("checked above");
    u.stats.insert(stat, value);
    hook(u, stat, value);
    Ok(())
}

/// 0x20 StatUpdate (§1 rule 4): a player's stat.
pub fn stat_update(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 10 {
        return Err(HandlerError::Invalid("0x20 is 10 bytes"));
    }
    let key = UnitKey::new(PLAYER, b.u32(1)?);
    let (stat, value) = (u16::from(b.u8(5)?), b.u32(6)? as i32);
    if let Some(u) = w.units.get_mut(&key) {
        u.stats.insert(stat, value);
        hook(u, stat, value);
    }
    Ok(())
}

/// The action handler of each action byte (§2 rule 2): `Some(true)` for
/// a 0x9C handler, `Some(false)` for a 0x9D handler, `None` past 0x17.
fn world_action(action: u8) -> Option<bool> {
    match action {
        0x00..=0x04 | 0x0A..=0x10 | 0x12 => Some(true),
        0x05..=0x09 | 0x11 | 0x13..=0x17 => Some(false),
        _ => None,
    }
}

/// 0x9C ItemActionWorld, 0x9D ItemActionOwned (§2).
pub fn item_action(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    let owned = msg.id == 0x9D;
    let size = usize::from(b.u8(2)?);
    let head = if owned { 13 } else { 8 };
    if size != msg.bytes.len() || size < head {
        return Err(HandlerError::Invalid("item message size byte"));
    }
    let action = b.u8(1)?;
    // Rule 2: past 0x17 nothing; an action of the other message is fatal.
    let Some(world) = world_action(action) else {
        return Ok(());
    };
    if world == owned {
        return Err(HandlerError::Fatal(if owned { 0xFB7 } else { 0xF6D }));
    }
    let key = UnitKey::new(ITEM, b.u32(4)?);
    let owner = if owned {
        let owner = UnitKey::new(b.u8(8)?, b.u32(9)?);
        // Rule 3: 0x9D needs the local player and the owner unit.
        if w.local().is_none() || !w.units.contains_key(&owner) {
            return Ok(());
        }
        Some(owner)
    } else {
        None
    };
    let record = ItemRecord {
        id: msg.id,
        action,
        category: b.u8(3)?,
        owner,
        stream: msg.bytes[head..].to_vec(),
    };
    // Rule 4. TODO(spec: msg-stats-items.md open question 3): the stream
    // header decides creation, re-creation, class and placement.
    let u = w.units.entry(key).or_insert_with(|| ClientUnit::new(key));
    match &mut u.kind {
        KindData::Item(d) => d.last = Some(record),
        k => {
            *k = KindData::Item(ItemData {
                last: Some(record),
                flags4: false,
            })
        }
    }
    Ok(())
}

/// 0x42 ClearCursor (§3 rule 1).
pub fn clear_cursor(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let S2c::ClearCursor(m) = parse(msg.bytes)? else {
        return Err(HandlerError::Invalid("not 0x42"));
    };
    let key = UnitKey::new(m.type_, m.unit);
    if w.local_player != Some(key) {
        return Ok(());
    }
    let cursor = match w.units.get_mut(&key).map(|u| &mut u.kind) {
        Some(KindData::Player(p)) => p.cursor_item.take(),
        _ => None,
    };
    if let Some(item) = cursor {
        w.remove(UnitKey::new(ITEM, item));
    }
    Ok(())
}

/// 0x3F UseStackableItem (§3 rule 2).
pub fn use_stackable_item(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let S2c::UseStackableItem(m) = parse(msg.bytes)? else {
        return Err(HandlerError::Invalid("not 0x3F"));
    };
    let key = UnitKey::new(ITEM, m.item);
    let set_flag = |w: &mut ClientWorld, on: bool| {
        if let Some(KindData::Item(d)) = w.units.get_mut(&key).map(|u| &mut u.kind) {
            d.flags4 = on;
        }
    };
    if m.arg == 0xFFFF && m.code == 0xFF {
        // Rule 2.1.
        set_flag(w, false);
        w.use_cursor = None;
        return Ok(());
    }
    // Rule 2.2 (the code-table scan is UI, `0x00455F20`).
    if !w.units.contains_key(&key) {
        return Ok(());
    }
    if m.arg == 0xFFFF {
        set_flag(w, true);
    }
    w.use_cursor = Some(UseCursor {
        item: key,
        code: m.code,
    });
    Ok(())
}

/// 0x47 Relator1, 0x48 Relator2 (§3 rule 3): the requirement refresh
/// `0x004C1350` (open question 4) sets no model field yet.
pub fn relator(_: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    match parse(msg.bytes)? {
        S2c::Relator1(_) | S2c::Relator2(_) => Ok(()),
        _ => Err(HandlerError::Invalid("not 0x47 / 0x48")),
    }
}
