// Spec: specs/client/model.md (§3 rule 1, §7, §9)
//! Session messages (0x00–0x06), the local player message (0x0B) and the
//! room-in-sight messages (0x07, 0x08). The UI, automap, sound and
//! client-DRLG set-ups these handlers also run in 1.14d are Phase 6
//! client behaviour and change no model field.

use d2_proto::s2c::{parse, Message as S2c};

use super::super::dispatch::{HandlerError, Message};
use super::super::world::{ActLoad, ClientWorld, RoomSight, UnitKey};
use super::Bytes;

fn parsed(msg: &Message<'_>) -> Result<S2c, HandlerError> {
    Ok(parse(msg.bytes)?)
}

/// 0x00 GameLoading: the empty handler `0x0045C900` (§7 rule 1).
pub fn game_loading(_: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    parsed(msg)?;
    Ok(())
}

/// 0x01 GameFlags (§7 rule 2).
pub fn game_flags(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let S2c::GameFlags(m) = parsed(msg)? else {
        return Err(HandlerError::Invalid("not 0x01"));
    };
    w.difficulty = m.difficulty;
    w.expansion = u32::from(m.expansion);
    w.ladder = m.ladder;
    w.game_flags = m.unk;
    Ok(())
}

/// 0x02 LoadSuccessful (§7 rule 3): the client answers with the system
/// message 0x6B.
pub fn load_successful(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    parsed(msg)?;
    w.outgoing.push(vec![0x6B]);
    Ok(())
}

/// 0x03 LoadAct (§7 rule 4). TODO(spec: model.md open question 5): the
/// client DRLG act built from these values is not in the model.
pub fn load_act(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let S2c::LoadAct(m) = parsed(msg)? else {
        return Err(HandlerError::Invalid("not 0x03"));
    };
    w.act = Some(ActLoad {
        act: m.act,
        init_seed: m.f2,
        town_level: m.f6,
        f8: m.f8,
    });
    Ok(())
}

/// 0x04 LoadComplete (§7 rule 5): the local player must have a room
/// (fatal 0x527). A placed unit has one; an unplaced one (created at
/// (0, 0)) has none (§2 rule 6).
pub fn load_complete(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    parsed(msg)?;
    if w.local().and_then(|u| u.position).is_none() {
        return Err(HandlerError::Fatal(0x527));
    }
    w.in_game = true;
    w.unloaded = false;
    Ok(())
}

/// 0x05 UnloadComplete (§7 rule 6).
pub fn unload_complete(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    parsed(msg)?;
    w.in_game = false;
    w.unloaded = true;
    Ok(())
}

/// 0x06 GameExit (§7 rule 7; single player).
pub fn game_exit(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    parsed(msg)?;
    w.exit_requested = true;
    Ok(())
}

/// The fields of 0x07 / 0x08: x u16@1, y u16@3, level u8@5 (§9).
fn room_sight(w: &mut ClientWorld, msg: &Message<'_>, show: bool) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    let sight = RoomSight {
        show,
        x: b.u16(1)?,
        y: b.u16(3)?,
        level: b.u8(5)?,
    };
    // The client act must exist (fatal 0x58A / 0x59E).
    if w.act.is_none() {
        return Err(HandlerError::Fatal(if show { 0x58A } else { 0x59E }));
    }
    w.rooms_in_sight.push(sight);
    Ok(())
}

/// 0x07 MapReveal (§9 rules 1, 4).
pub fn map_reveal(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    parsed(msg)?;
    room_sight(w, msg, true)
}

/// 0x08 MapHide (§9 rules 2, 4).
pub fn map_hide(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    if msg.bytes.len() != 6 {
        return Err(HandlerError::Invalid("0x08 is 6 bytes"));
    }
    room_sight(w, msg, false)
}

/// 0x0B GameHandshake (§3 rule 1): type u8@1, GUID u32@2; a unit in the
/// set becomes the local player, an unknown key changes nothing.
pub fn game_handshake(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 6 {
        return Err(HandlerError::Invalid("0x0B is 6 bytes"));
    }
    let key = UnitKey::new(b.u8(1)?, b.u32(2)?);
    if w.units.contains_key(&key) {
        w.local_player = Some(key);
    }
    Ok(())
}
