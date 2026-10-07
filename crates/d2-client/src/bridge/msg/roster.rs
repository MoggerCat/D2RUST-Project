// Spec: specs/client/msg-units.md (§8)
//! The player roster (`msg-units.md` §8): S→C 0x5B PlayerJoined, 0x5C
//! PlayerLeft, 0x65 PlayerKillCount, 0x82 PortalOwnership and 0x8E
//! CorpseAssign over `ClientWorld::roster` / `roster_inactive`. 0x5B,
//! 0x5C and 0x65 emit `RosterChanged` with the active records (rule 9).

use super::super::dispatch::{HandlerError, Message};
use super::super::output::Output;
use super::super::world::{ClientWorld, KindData, RosterRecord, UnitKey, OBJECT};
use super::Bytes;

fn changed(w: &ClientWorld, msg: &Message<'_>) {
    msg.out.push(Output::RosterChanged {
        roster: w.roster.clone(),
    });
}

/// The NUL-terminated bytes of `b` from `off` (the NUL excluded); a
/// string without a NUL ends with the message.
fn cstr(b: &[u8], off: usize) -> &[u8] {
    let s = b.get(off..).unwrap_or(&[]);
    let n = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    &s[..n]
}

/// `strcpy(record + 0x46 + at, s)`: the string and its NUL.
fn copy_string(strings: &mut Vec<u8>, at: usize, s: &[u8]) {
    let end = at + s.len() + 1;
    if strings.len() < end {
        strings.resize(end, 0);
    }
    strings[at..at + s.len()].copy_from_slice(s);
    strings[at + s.len()] = 0;
}

/// The name of a record up to its NUL.
fn name_of(n: &[u8; 16]) -> &[u8] {
    let k = n.iter().position(|&c| c == 0).unwrap_or(16);
    &n[..k]
}

/// 0x5B PlayerJoined (§8 r3): size u16@1, GUID u32@3, class u8@7, name
/// @8 (16 bytes), u16@0x18 … u16@0x20, string 1 @0x22, string 2 after
/// string 1's NUL.
pub fn player_joined(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    let size = usize::from(b.u16(1)?);
    if size != msg.bytes.len() || size < 34 {
        return Err(HandlerError::Invalid("0x5B size word"));
    }
    let guid = b.u32(3)?;
    if guid == u32::MAX {
        return Err(HandlerError::Fatal(0xB3));
    }
    let mut name = [0u8; 16];
    name.copy_from_slice(b.slice(8, 16)?);
    let s1 = cstr(msg.bytes, 0x22);
    let s2 = cstr(msg.bytes, 0x22 + s1.len() + 1);
    // Found by GUID or by name (`0x00479360`, active list), updated in
    // place. TODO(spec: msg-units.md §8 r3): the name comparison of
    // `0x00479360` (case, length) is not stated; bytes up to the NUL.
    let found = w
        .roster
        .iter()
        .position(|r| r.guid == guid || name_of(&r.name) == name_of(&name));
    let (mut rec, at) = match found {
        Some(i) => (w.roster.remove(i), Some(i)),
        None => match w.roster_inactive.iter().position(|r| r.guid == guid) {
            Some(i) => (w.roster_inactive.remove(i), None),
            None => (RosterRecord::default(), None),
        },
    };
    // `0x004793C0`: u16@0x1C is not stored.
    rec.name = name;
    rec.guid = guid;
    rec.class = u32::from(b.u8(7)?);
    rec.f20 = b.u16(0x18)?;
    rec.f22 = b.u16(0x1A)?;
    rec.f30 = b.u16(0x1E)?;
    rec.f44 = b.u16(0x20)?;
    copy_string(&mut rec.strings, 0, s1);
    copy_string(&mut rec.strings, 4, s2);
    match at {
        Some(i) => w.roster.insert(i, rec),
        // A new or moved record is prepended.
        None => w.roster.insert(0, rec),
    }
    changed(w, msg);
    Ok(())
}

/// 0x5C PlayerLeft (§8 r4): GUID u32@1; the active record with that
/// GUID (not the corpse list) is freed; `RosterChanged` either way.
pub fn player_left(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 5 {
        return Err(HandlerError::Invalid("0x5C is 5 bytes"));
    }
    let guid = b.u32(1)?;
    if guid == u32::MAX {
        return Err(HandlerError::Fatal(0x121));
    }
    if let Some(i) = w.roster.iter().position(|r| r.guid == guid) {
        w.roster.remove(i);
    }
    changed(w, msg);
    Ok(())
}

/// 0x65 PlayerKillCount (§8 r5): GUID u32@1, count u16@5 (sign-extended).
pub fn player_kill_count(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 7 {
        return Err(HandlerError::Invalid("0x65 is 7 bytes"));
    }
    let Some(i) = w.roster_find(b.u32(1)?) else {
        return Ok(());
    };
    w.roster[i].kills = i32::from(b.u16(5)? as i16);
    changed(w, msg);
    Ok(())
}

/// 0x82 PortalOwnership (§8 r7): owner GUID u32@1, owner name @5–@20,
/// portal GUIDs u32@21, u32@25.
pub fn portal_ownership(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 29 {
        return Err(HandlerError::Invalid("0x82 is 29 bytes"));
    }
    let mut name = [0u8; 16];
    name.copy_from_slice(b.slice(5, 16)?);
    let (a, c) = (b.u32(21)?, b.u32(25)?);
    let set_name = |w: &mut ClientWorld, guid: u32| {
        if let Some(KindData::Object(d)) = w
            .units
            .get_mut(&UnitKey::new(OBJECT, guid))
            .map(|u| &mut u.kind)
        {
            d.owner_name = Some(name);
        }
    };
    set_name(w, a);
    if c == u32::MAX {
        return Ok(());
    }
    set_name(w, c);
    if let Some(i) = w.roster_find(b.u32(1)?) {
        w.roster[i].portals = (a, c);
    }
    Ok(())
}

/// 0x8E CorpseAssign (§8 r8): u8@1, player GUID u32@2, corpse GUID
/// u32@6. No UI refresh.
pub fn corpse_assign(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 10 {
        return Err(HandlerError::Invalid("0x8E is 10 bytes"));
    }
    let (player, corpse) = (b.u32(2)?, b.u32(6)?);
    if b.u8(1)? != 0 {
        // `0x0047A3D0`.
        let rec = match w.roster_find(player) {
            Some(i) => &mut w.roster[i],
            None => {
                w.roster_inactive.insert(
                    0,
                    RosterRecord {
                        guid: player,
                        ..RosterRecord::default()
                    },
                );
                &mut w.roster_inactive[0]
            }
        };
        if !rec.corpses.contains(&corpse) {
            rec.corpses.insert(0, corpse);
        }
    } else if let Some(i) = w.roster_find(player) {
        // `0x0047A490`.
        let list = &mut w.roster[i].corpses;
        if let Some(k) = list.iter().position(|&c| c == corpse) {
            list.remove(k);
        }
    }
    Ok(())
}
