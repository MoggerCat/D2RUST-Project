// Spec: specs/client/msg-stats-items.md
//! Stat messages (0x19–0x20) and item messages (0x9C, 0x9D, 0x3F, 0x42,
//! 0x47, 0x48). An item unit records its last message; the stream
//! header the actions read (open question 3) is the provisional
//! [`ItemHeader`].

use d2_proto::s2c::{parse, Message as S2c};

use super::super::bits::BitReader;
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

/// The stream header the item actions read (`0x0062E410`,
/// `msg-stats-items.md` §2 r5.3: mode byte +8, flags +0x0C, page +0x10,
/// body location +0x11).
///
/// PROVISIONAL (client/msg-stats-items.md OQ 3, with items/bitstream.md):
/// the header is the head of the item bit stream (`items/bitstream.md`
/// §2 r3, §3 r1–r2, §4.1 r1–r3): 32 bits flags F, 10 bits version, 3
/// bits mode, then the location: mode 3 or 5 → 16 bits x, 16 bits y;
/// else 4 bits body location, 4 bits x, 4 bits y, 3 bits page + 1
/// (0 → page 0xFF). Settled by a Ghidra read of 0x0062E410 plus a join /
/// trade packet recording with items (HIGH-PRIORITY CAPTURE: wire byte
/// layout).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemHeader {
    pub flags: u32,
    pub mode: u8,
    pub body: u8,
    pub page: u8,
    pub x: u16,
    pub y: u16,
}

impl ItemHeader {
    /// The header of `stream`; `None` when the stream is shorter than it.
    pub fn peek(stream: &[u8]) -> Option<Self> {
        let mut r = BitReader::new(stream);
        let flags = r.read(32);
        let _version = r.read(10);
        let mode = r.read(3) as u8;
        let mut h = ItemHeader {
            flags,
            mode,
            page: 0xFF,
            ..ItemHeader::default()
        };
        if matches!(mode, 3 | 5) {
            h.x = r.read(16) as u16;
            h.y = r.read(16) as u16;
        } else {
            h.body = r.read(4) as u8;
            h.x = r.read(4) as u16;
            h.y = r.read(4) as u16;
            let p = r.read(3) as u8;
            h.page = if p == 0 { 0xFF } else { p - 1 };
        }
        (!r.overflow).then_some(h)
    }
}

/// A cursor write of an item action (§2 r5): set the inventory unit's
/// cursor to the item, or clear it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CursorWrite {
    Set(UnitKey),
    Clear(UnitKey),
}

/// The item of `owner` at body location `loc` (header mode 1), from the
/// item units' last records (the model holds no inventory nodes; see
/// [`ItemHeader`]).
fn body_item(w: &ClientWorld, owner: UnitKey, loc: u8, except: UnitKey) -> Option<UnitKey> {
    w.units.values().find_map(|u| match &u.kind {
        KindData::Item(d) if u.key != except => d.last.as_ref().and_then(|r| {
            let h = ItemHeader::peek(&r.stream)?;
            (r.owner == Some(owner) && h.mode == 1 && h.body == loc).then_some(u.key)
        }),
        _ => None,
    })
}

/// The cursor writes of the item actions (§2 r5.1–r5.3). `p` is the local
/// player; `owner` the 0x9D owner (0x9C: the local player); `was` the
/// item's header before this message (its last record), `h` the new one.
/// The grid and belt placements the table conditions on are taken as
/// succeeding (no inventory model).
#[allow(clippy::too_many_arguments)]
fn cursor_write(
    w: &ClientWorld,
    action: u8,
    p: UnitKey,
    owner: UnitKey,
    item: UnitKey,
    h: &ItemHeader,
    was: Option<&ItemHeader>,
    existed: bool,
) -> Option<CursorWrite> {
    let f = h.flags;
    let is_p = owner == p;
    match action {
        0x12 => Some(CursorWrite::Set(p)),
        0x01 if h.mode == 4 => Some(CursorWrite::Set(p)),
        0x02 => {
            let on_cursor = matches!(
                w.units.get(&p).map(|u| &u.kind),
                Some(KindData::Player(d)) if d.cursor_item == Some(item.guid)
            );
            (existed && on_cursor).then_some(CursorWrite::Clear(p))
        }
        0x04 | 0x0B | 0x0C => (is_p && h.page != 1).then_some(CursorWrite::Clear(p)),
        0x05 => {
            let in_grid = was.is_some_and(|w| w.mode == 0);
            (in_grid && f & 0x20 == 0).then_some(CursorWrite::Set(owner))
        }
        0x06 => (is_p && owner.unit_type < 2 && f & 0x8 == 0).then_some(CursorWrite::Clear(p)),
        0x07 if is_p => {
            let other = match h.body {
                4 | 11 => 5,
                5 | 12 => 4,
                b => b,
            };
            body_item(w, p, other, item).map(|_| CursorWrite::Set(p))
        }
        0x08 => (is_p && f & 0x20 == 0).then_some(CursorWrite::Set(p)),
        0x09 if f & 0x80 != 0 && is_p => body_item(w, p, h.body, item).map(|_| CursorWrite::Set(p)),
        0x0A => {
            let own = match w.units.get(&item).map(|u| &u.kind) {
                Some(KindData::Item(d)) => d.flags,
                _ => 0,
            };
            (own & 0x8 == 0).then_some(CursorWrite::Clear(p))
        }
        0x0D if h.mode == 0 => Some(CursorWrite::Set(p)),
        0x0E => Some(CursorWrite::Clear(p)),
        0x0F => {
            let in_belt = was.is_some_and(|w| w.mode == 2);
            (in_belt && f & 0x20 == 0).then_some(CursorWrite::Set(p))
        }
        0x10 if h.mode == 2 => Some(CursorWrite::Set(p)),
        0x11 => is_p.then_some(CursorWrite::Clear(p)),
        0x13 => (f & 0x8 == 0).then_some(CursorWrite::Clear(p)),
        0x15 => match h.mode {
            0 if is_p => (h.page != 1).then_some(CursorWrite::Clear(p)),
            1 if is_p => (f & 0x8 == 0).then_some(CursorWrite::Clear(p)),
            4 if is_p => Some(CursorWrite::Set(p)),
            _ => None,
        },
        0x16 => Some(CursorWrite::Set(owner)),
        _ => None,
    }
}

/// The item set or taken by a [`CursorWrite::Set`] (§2 r5.3): the item
/// taken off a body location for 0x07 / 0x09, else the message's item.
fn set_item(w: &ClientWorld, action: u8, p: UnitKey, item: UnitKey, h: &ItemHeader) -> UnitKey {
    match action {
        0x07 => {
            let other = match h.body {
                4 | 11 => 5,
                5 | 12 => 4,
                b => b,
            };
            body_item(w, p, other, item).unwrap_or(item)
        }
        0x09 => body_item(w, p, h.body, item).unwrap_or(item),
        _ => item,
    }
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
    // d2rs-own, unverified: the shop panel's trade epoch (`ItemRecord::seq`).
    if action == 0x0B {
        w.store_serial += 1;
    }
    let record = ItemRecord {
        id: msg.id,
        action,
        category: b.u8(3)?,
        owner,
        stream: msg.bytes[head..].to_vec(),
        seq: if action == 0x0B { w.store_serial } else { 0 },
    };
    // Rule 4: the stream header (provisional [`ItemHeader`]) decides
    // placement and the cursor writes of rule 5.
    let header = ItemHeader::peek(&record.stream);
    let existed = w.units.contains_key(&key);
    let was = w.units.get(&key).and_then(|u| match &u.kind {
        KindData::Item(d) => d.last.as_ref().and_then(|r| ItemHeader::peek(&r.stream)),
        _ => None,
    });
    let local = w.local_player.filter(|k| w.units.contains_key(k));
    match (action, header) {
        (0x12, _) if local.is_none() => return Err(HandlerError::Fatal(0xB0C)),
        // Rule 5.2: GroundToCursor acts only on a mode-4 header.
        (0x01, Some(h)) if h.mode != 4 => return Ok(()),
        // §2 r5.2 makes an item already in S fatal 0x3A0. Not enforced:
        // the d2rs server's pick-up sends no room delete notice (0x0A)
        // before 0x9C action 1, so the ground item is still in S
        // (`docs/handoff/impl-triage-client.md`); the record replaces it.
        _ => {}
    }
    let write = match (header, local) {
        (Some(h), Some(p)) => cursor_write(
            w,
            action,
            p,
            owner.unwrap_or(p),
            key,
            &h,
            was.as_ref(),
            existed,
        )
        .map(|c| (c, set_item(w, action, p, key, &h))),
        _ => None,
    };
    // Rule 6: the belt column-ready bytes (slots 0–3 only).
    if let Some(h) = header {
        let mut ready = |x: u16, v: bool| {
            if let Some(b) = w.belt_ready.get_mut(usize::from(x)) {
                *b = v;
            }
        };
        match action {
            0x0E => ready(h.x, true),
            0x0F => ready(h.x, false),
            0x15 if h.mode == 2 => {
                if let Some(old) = was.filter(|o| o.mode == 2) {
                    ready(old.x, false);
                }
                ready(h.x, true);
            }
            _ => {}
        }
    }
    let u = w.units.entry(key).or_insert_with(|| ClientUnit::new(key));
    // A ground item (header mode 3 or 5) stands at its sub-tile; any
    // other mode leaves the world (no cell).
    if let Some(h) = header {
        u.position = matches!(h.mode, 3 | 5).then_some((h.x, h.y));
    }
    match &mut u.kind {
        KindData::Item(d) => d.last = Some(record),
        k => {
            *k = KindData::Item(ItemData {
                last: Some(record),
                ..ItemData::default()
            })
        }
    }
    super::super::item_lists::refresh(w, key);
    // Rule 5: the cursor of the inventory's unit (a player's
    // `cursor_item`), then the UI cursor refresh (UI state).
    if let Some((c, item)) = write {
        let (unit, value) = match c {
            CursorWrite::Set(u) => (u, Some(item.guid)),
            CursorWrite::Clear(u) => (u, None),
        };
        if let Some(KindData::Player(d)) = w.units.get_mut(&unit).map(|u| &mut u.kind) {
            d.cursor_item = value;
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
        // Rule 2.1 (with `0x004C2180`, §5 r3).
        set_flag(w, false);
        super::items::clear_scroll_state(w, key);
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
