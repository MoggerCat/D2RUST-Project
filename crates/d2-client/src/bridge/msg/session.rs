// Spec: specs/client/model.md (§3 rule 1, §7, §9 rules 1–5, §11 rule 2, §12 rule 1), specs/sim/unit-order.md (§5 rule 6), specs/render/lighting.md (§9.1, §9.2 r3)
//! Session messages (0x00–0x06), the local player message (0x0B) and the
//! room-in-sight messages (0x07, 0x08). 0x03 builds the client DRLG act
//! and 0x07 / 0x08 set its rooms in sight ([`super::super::drlg`]); the
//! UI, automap and sound set-ups these handlers also run in 1.14d are
//! Phase 6 client behaviour and change no model field.

use d2_proto::s2c::{parse, Message as S2c};

use super::super::dispatch::{HandlerError, Message};
use super::super::drlg::{ClientDrlg, ClientDrlgError};
use super::super::output::Output;
use super::super::world::{ActLoad, ClientWorld, RoomSight, SessionMark, UnitKey};
use super::lighting::{act_load_eclipse, create_environment};
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

/// 0x03 LoadAct (§7 rule 4, §12 rule 1): an existing client act is
/// freed, then the client DRLG of the act is built from the init seed
/// u32@2 with the client flag (no client DRLG without a DRLG source).
pub fn load_act(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let S2c::LoadAct(m) = parsed(msg)? else {
        return Err(HandlerError::Invalid("not 0x03"));
    };
    w.mark_session(SessionMark::LoadAct(m.act));
    w.act = Some(ActLoad {
        act: m.act,
        init_seed: m.f2,
        town_level: m.f6,
        f8: m.f8,
    });
    // §11 rule 2: the act of 0x03 is the palette act. u16@6 is the act's
    // town, never the player's level (§11 rules 1, 5).
    w.palette_act = Some(m.act);
    // The old act's rooms are freed with it: their units leave the room
    // lists (`sim/unit-order.md` §5 rule 6).
    // The client act free `0x0061AFD0` frees every active room the same
    // way as a room leaving sight (`model.md` §16 r1, r2): the units
    // linked in them get flag 0x800000 and flags-2 0x20.
    for r in w.active_rooms.take().unwrap_or_default() {
        w.free_active_room(r.room);
    }
    w.drlg = None;
    w.room_units = Default::default();
    if let Some(src) = &msg.inputs.drlg {
        w.drlg =
            Some(ClientDrlg::build(src, m.act, m.f2, w.difficulty).map_err(ClientDrlgError::from)?);
        w.refresh_active_rooms()?;
    }
    // `0x0044E100` replaced the client act (`render/composition.md` §3
    // step 4: the next drawn frame is cleared after drawing).
    w.act_loads = w.act_loads.wrapping_add(1);
    // The act's environment record (`render/lighting.md` §9.1), then the
    // pending eclipse (§9.2 r3).
    w.environment = Some(create_environment()?);
    act_load_eclipse(w, m.act)
}

/// 0x04 LoadComplete (§7 rule 5): the local player must have a room
/// (fatal 0x527). A placed unit has one; an unplaced one (created at
/// (0, 0)) has none (§2 rule 6).
pub fn load_complete(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    parsed(msg)?;
    if w.local().and_then(|u| u.position).is_none() {
        return Err(HandlerError::Fatal(0x527));
    }
    w.mark_session(SessionMark::LoadComplete);
    w.in_game = true;
    w.unloaded = false;
    Ok(())
}

/// 0x05 UnloadComplete (§7 rule 6).
pub fn unload_complete(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    parsed(msg)?;
    w.mark_session(SessionMark::Unload);
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

/// The client loop's result after the game (`client/msg-ui.md` OQ 8,
/// `0x0044B8A0`): `[0x0070EE8C]` ≠ 0 → 0; else by the session record's
/// game type: 2, 3, 6, 7 → 3, any other → 4 (also `[0x007A0440]`).
/// Front-end state (where the program goes after the game), not model.
pub fn end_of_game_result(flag_70ee8c: bool, game_type: u32) -> u32 {
    if flag_70ee8c {
        0
    } else if matches!(game_type, 2 | 3 | 6 | 7) {
        3
    } else {
        4
    }
}

// Covers: specs/client/msg-ui.md §8
#[cfg(test)]
#[test]
fn end_of_game_result_by_game_type() {
    assert_eq!(end_of_game_result(true, 3), 0);
    let r: Vec<u32> = (0..12).map(|t| end_of_game_result(false, t)).collect();
    assert_eq!(r, [4, 4, 3, 3, 4, 4, 3, 3, 4, 4, 4, 4]);
}

/// The error number of `0x0044E380(n)` for a 0xB4 code c (§7 r8.1,
/// jump table `0x0045C7E8`): c = 0 or c > 26 → 9.
pub fn join_refused_error(c: u32) -> u8 {
    match c {
        1..=6 => (c - 1) as u8,
        7..=21 => (c + 3) as u8,
        22 => 9,
        23 => 0x19,
        24 => 0x1A,
        25 => 0x1C,
        26 => 0x1B,
        _ => 9,
    }
}

/// 0xB4 load refusal (§7 rule 8; system handler `0x0045C6D0`): code
/// u32@1 → one `JoinRefused` output {n}; no model write here (the UI
/// layer returns `exit_requested` / `in_game` as requests, r8.3).
pub fn join_refused(_: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    if msg.bytes.len() != 5 {
        return Err(HandlerError::Invalid("0xB4 is 5 bytes"));
    }
    let c = Bytes(msg.bytes).u32(1)?;
    msg.out.push(Output::JoinRefused {
        error: join_refused_error(c),
    });
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
    // §9 rules 1–2 on the client DRLG; the local player's DRLG room is
    // the hint (used only when it is in that level).
    let hint = w.local_room().map(|r| r.room);
    let Some(drlg) = w.drlg.as_mut() else {
        return Ok(());
    };
    let found = if show {
        drlg.set_in_sight(sight.level, sight.x, sight.y, hint)?
    } else {
        drlg.unset_in_sight(sight.level, sight.x, sight.y, hint)?
    };
    w.refresh_active_rooms()?;
    // §9 rule 5: no room at the point. 0x07 reads the status-1 count of
    // the null room (an access violation that ends 1.14d); 0x08 tests the
    // room and does nothing.
    if found.is_none() && show {
        return Err(HandlerError::Crash {
            at: 0x0061_B672,
            what: "0x07 at a point in no room of the level reads the null room's count",
        });
    }
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

/// 0xAF ConnectionInfo (`model.md` §7 r10): `connected` := 1; u8@1 is
/// not read.
pub fn connection_info(w: &mut ClientWorld, _: &Message<'_>) -> Result<(), HandlerError> {
    w.connected = true;
    Ok(())
}

/// 0xB0 ConnectionTerminated (`model.md` §7 r10): `connected` := 0.
pub fn connection_terminated(w: &mut ClientWorld, _: &Message<'_>) -> Result<(), HandlerError> {
    w.connected = false;
    Ok(())
}

/// 0x8F Pong (`model.md` §7 r11 item 2, 33 bytes): `pong[i]` :=
/// u32@(1 + 4i); `pong[4]` := now; `rtt` := now − `sent_ms`; the mean
/// over the first 10 samples. d2rs has no clock in the model and sends
/// no 0x6D, so now and `sent_ms` are both 0 and `rtt` is 0.
pub fn pong(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = super::Bytes(msg.bytes);
    if msg.bytes.len() != 33 {
        return Err(HandlerError::Invalid("0x8F is 33 bytes"));
    }
    let p = &mut w.ping;
    for i in 0..8 {
        p.pong[i] = b.u32(1 + 4 * i)?;
    }
    p.pong[4] = 0;
    p.rtt = 0;
    if p.samples < 10 {
        p.mean = p.mean.wrapping_mul(p.samples) / (p.samples + 1);
        p.samples += 1;
    }
    Ok(())
}
