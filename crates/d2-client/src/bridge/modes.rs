// Spec: specs/client/model.md (§8 rules 1–6, §15 rule 3, §18 rule 1–2, open questions 1–2), specs/audio/triggers-2.md (§20 r3)
//! The client mode machines: the mode request `0x00480C10(code, U,
//! record, 1)` that ends every unit-handler message of `msg-units.md` §4
//! (§8 rule 1), dispatched by U's type:
//!
//! - player `0x00461250` (§8 rule 4): the code table's mode, `+0xB0`
//!   (last hit class, §18 rule 1) and position-check writes; the
//!   interact sender of code 0x02 stays in [`super::objects::interact`];
//! - object `0x004BD6D0` (§8 rule 5, §15 rule 3): code 3 is the object
//!   mode change `0x004BCF60` (mode := r1, the object light of the new
//!   mode, the mode sound call inside the change, `audio/triggers-2.md`
//!   §20 r3); code 0x15 the shrine use (in `msg::units`); other codes
//!   fatal 0x39C;
//! - item `0x004C1B80` (§8 rule 6): code 2 → mode := r1, flag 0x2 :=
//!   (r0 ≠ 0);
//! - monster `0x004AFF60`: open question 1, see [`monster_mode`];
//! - missile: nothing.
//!
//! Every request is also kept as `last_mode_request` (§8 rule 3).
//!
//! Local walk prediction (`0x00463390`, open question 2) is not a mode
//! machine rule: PROVISIONAL (client/model.md OQ 2; REC-51): the local
//! player is drawn at the last server-sent position, no prediction.

use d2_sim::monsters::mode_message::MODE_ROWS;

use super::check::check;
use super::dispatch::HandlerError;
use super::msg::lighting::object_light;
use super::objects::{ObjSound, ObjUnit};
use super::output::{Output, Outputs};
use super::world::{
    ClientWorld, ModeRequest, ModelInputs, UnitKey, ITEM, MISSILE, MONSTER, OBJECT, PLAYER,
};
use crate::rules::lighting::records::Owner;

/// Player modes the table names (`sim/units.md` player modes).
pub mod player_mode {
    pub const DEATH: u32 = 0;
    pub const NEUTRAL: u32 = 1;
    pub const WALK: u32 = 2;
    pub const RUN: u32 = 3;
    pub const GET_HIT: u32 = 4;
    pub const TOWN_NEUTRAL: u32 = 5;
    pub const TOWN_WALK: u32 = 6;
    pub const THROW: u32 = 9;
    pub const BLOCK: u32 = 0xD;
    pub const DEAD: u32 = 0x11;
    pub const SEQUENCE: u32 = 0x13;
}

/// The player request code 0x13 (§8 rule 4: the only code that keeps the
/// light and the two set-up calls).
pub const CODE_SEQUENCE: u8 = 0x13;

/// Fatal of an invalid player code (§8 rule 4).
pub const FATAL_PLAYER_CODE: u32 = 0x432;
/// Fatal of an object code other than 3 and 0x15 (§8 rule 5).
pub const FATAL_OBJECT_CODE: u32 = 0x39C;

/// The mode request `0x00480C10(code, U, record, 1)` (§8 rule 1). A unit
/// not in S: nothing (the handlers run on a drained unit, `model.md` §4).
pub fn mode_request(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    code: u8,
    record: [i32; 7],
    out: &Outputs,
) -> Result<(), HandlerError> {
    // A missile's request returns 1 at once (rule 1): not stored.
    if key.unit_type == MISSILE {
        return Ok(());
    }
    let Some(u) = w.units.get_mut(&key) else {
        return Ok(());
    };
    // Rule 3.
    u.last_mode_request = Some(ModeRequest { code, record });
    match key.unit_type {
        PLAYER => player(w, inputs, key, code, record),
        OBJECT => object(w, inputs, key, code, record, out),
        ITEM => {
            item(w, key, code, record);
            Ok(())
        }
        MONSTER => {
            let m = match code {
                0x15 | 0x16 => skill_mode(inputs, record[0], MONSTER),
                _ => monster_mode(code),
            };
            if let Some(m) = m {
                w.units.get_mut(&key).expect("present").mode = m;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The room-in-town test `0x0061AB00` on U's room (the room list of
/// `sim/unit-order.md` §5 r6 and the active room's level): a unit in no
/// room is not in town.
pub fn in_town(w: &ClientWorld, key: UnitKey) -> bool {
    let Some(room) = w.room_units.room_of(key) else {
        return false;
    };
    w.active_rooms
        .as_deref()
        .and_then(|rooms| rooms.iter().find(|r| r.room == room))
        .is_some_and(|r| d2_sim::drlg::is_town(u32::from(r.level)))
}

/// The neutral and walk modes of U (§8 rule 4): 5 / 6 in town, else 1 / 2.
pub fn neutral_walk(w: &ClientWorld, key: UnitKey) -> (u32, u32) {
    if in_town(w, key) {
        (player_mode::TOWN_NEUTRAL, player_mode::TOWN_WALK)
    } else {
        (player_mode::NEUTRAL, player_mode::WALK)
    }
}

/// `0x00643A00(U, 0)` then `0x004743D0`: the unit's light (a cast light
/// of the client skill start, `render/lighting.md` §8 r3) is detached
/// and removed; a unit without one: nothing.
fn remove_unit_light(w: &mut ClientWorld, key: UnitKey) {
    let owner = Owner {
        unit_type: u32::from(key.unit_type),
        guid: key.guid,
        client_only: false,
    };
    let id = w
        .lights
        .iter()
        .find(|(_, r)| r.owner() == Some(owner))
        .map(|(id, _)| id);
    if let Some(id) = id {
        let _ = w.lights.remove(id);
    }
}

/// Places U at (x, y): the room of the point (`model.md` §2 rule 7;
/// none is fatal 0x168), the position, and the room list recache
/// (`sim/unit-order.md` §5 rule 6). Without the client DRLG the point is
/// taken as in a room.
fn place(w: &mut ClientWorld, key: UnitKey, x: u16, y: u16) -> Result<(), HandlerError> {
    let room = match &w.active_rooms {
        Some(_) => Some(w.room_at(x, y).ok_or(HandlerError::Fatal(0x168))?),
        None => None,
    };
    if let Some(u) = w.units.get_mut(&key) {
        u.position = Some((x, y));
    }
    if w.active_rooms.is_some() {
        w.room_units.place(key, room.map(|r| r.room));
    }
    Ok(())
}

/// The player machine `0x00461250` (§8 rule 4), with flag 1 (every
/// queued message passes 1).
fn player(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    code: u8,
    r: [i32; 7],
) -> Result<(), HandlerError> {
    if matches!(code, 3..=5 | 0x0A..=0x11) || code > 0x19 {
        return Err(HandlerError::Fatal(FATAL_PLAYER_CODE));
    }
    remove_unit_light(w, key);
    // `0x004611F0` / `0x00620210` (unless code 0x13) are presentation
    // and used-skill resets of the client animation; no model field.
    let (neutral, walk) = neutral_walk(w, key);
    let u = w.units.get_mut(&key).expect("checked by the caller");
    // A unit in mode 0x13 with flag 1: its path is stopped
    // (`0x00650590`); then `0x00648DC0(path)` (no client path record).
    if u.mode == player_mode::SEQUENCE {
        u.path_stopped = true;
    }
    let (x, y) = (r[0] as u16, r[1] as u16);
    let set = |w: &mut ClientWorld, m: u32| {
        w.units.get_mut(&key).expect("present").mode = m;
    };
    let hit = |w: &mut ClientWorld| {
        w.units.get_mut(&key).expect("present").hit_class = r[2] as u32;
    };
    match code {
        // `0x00480780` / `0x004804A0` (path to a unit / a point), then
        // mode := walk. PROVISIONAL (client/model.md OQ 1; REC-51): the
        // path helpers succeed (the client holds no path record, so the
        // "helper returned 0 → neutral" branch never runs).
        0x00 | 0x01 => set(w, walk),
        // The interact sender (rule 7) runs from the message handler
        // (`objects::interact::mode_request_code_2`): no mode change.
        0x02 => {}
        0x06 => {
            hit(w);
            set(w, player_mode::GET_HIT);
            check(w, inputs, key, x, y, 1, 0, 0)?;
        }
        0x07 => {
            // was-dead `0x00464820` and `0x004647D0` feed the re-init
            // `0x00480EF0(U, r0, r1, was-dead)`.
            let u = w.units.get_mut(&key).expect("present");
            let was_dead = u.is_dead();
            u.flag_2 = Some(true);
            u.mode = neutral;
            // PROVISIONAL (REC-279; d2rs-own, unverified): `0x00480EF0`
            // places a unit that was dead at (r0, r1), as 0x15's place
            // (`msg-units.md` §3 rule 4, without the free-point fallback).
            // The respawn's 0x0D code 7 (C→S 0x41) is the only message
            // that gives the client the town point: the 0x15 before it
            // finds the player dead and leaves it (§3 rule 4.3).
            if was_dead && (x, y) != (0, 0) {
                place(w, key, x, y)?;
            }
        }
        0x08 => {
            // The local player's UI resets (`0x00456300`, hover target)
            // belong to the UI layer.
            hit(w);
            check(w, inputs, key, x, y, 0, 0, 0)?;
            set(w, player_mode::DEATH);
        }
        0x09 => set(w, player_mode::DEAD),
        0x12 => {
            hit(w);
            // PROVISIONAL (client/model.md §8 r4 code 0x12; REC-51): the
            // test `0x0063C8F0(inventory, 0)` / COF weapon class 0xD
            // needs the client inventory and the COF weapon class, which
            // the model does not hold: read as false (mode unchanged).
            check(w, inputs, key, x, y, 1, 0, 0)?;
        }
        0x13 => {
            hit(w);
            // `0x00480D20` adds overlays, `0x004CC5B0` makes a sound
            // (§8 rule 7: neither sends nor writes the model).
            check(w, inputs, key, x, y, 1, 0, 0)?;
        }
        0x14 => {
            hit(w);
            let u = w.units.get_mut(&key).expect("present");
            u.path_stopped = true;
            u.mode = player_mode::SEQUENCE;
        }
        // The client skill start (`0x004C6F40` / `0x004C6EB0`) after the
        // target fix-up: render/lighting.md §8 r3 (cast light); the
        // skill's mode is the client animation's (open question 1).
        0x15 | 0x16 => {
            if let Some(m) = skill_mode(inputs, r[0], PLAYER) {
                set(w, m);
            }
        }
        0x17 | 0x18 => set(w, player_mode::RUN),
        0x19 => {
            set(w, player_mode::BLOCK);
            check(w, inputs, key, x, y, 0, 0, 0)?;
        }
        _ => unreachable!("fatal codes returned above"),
    }
    Ok(())
}

/// The object machine `0x004BD6D0` (§8 rule 5): code 3 is the mode
/// change `0x004BCF60` (§15 rule 3: r1 is the mode), code 0x15 the
/// shrine use (run by the caller after this), anything else fatal.
fn object(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    code: u8,
    r: [i32; 7],
    out: &Outputs,
) -> Result<(), HandlerError> {
    match code {
        3 => {
            let mode = r[1] as u32;
            let u = w.units.get_mut(&key).expect("checked by the caller");
            u.mode = mode;
            let class = u.class;
            // The object light of the new mode (`render/lighting.md` §8
            // object row: `Lit<mode>` / 2). Without the class's row the
            // light cannot be read: nothing.
            if let Some(row) = inputs.tables.objects.get(class as usize).copied() {
                let lit = *row.lit.get(mode as usize).ok_or(HandlerError::Invalid(
                    "object mode past the eight objects.txt modes",
                ))?;
                object_light(w, key, lit, row.rgb);
            }
            // The mode sound call inside the change (`0x004BD062`,
            // `audio/triggers-2.md` §20 r3).
            let unit = ObjUnit {
                key,
                client_only: false,
            };
            out.push(Output::ObjectSound(ObjSound::Mode {
                unit,
                class,
                mode,
                local_dist: super::objects::local_distance(w, inputs, unit),
            }));
            Ok(())
        }
        0x15 => Ok(()),
        _ => Err(HandlerError::Fatal(FATAL_OBJECT_CODE)),
    }
}

/// The item machine `0x004C1B80(r0, r1)` (§8 rule 6).
fn item(w: &mut ClientWorld, key: UnitKey, code: u8, r: [i32; 7]) {
    if code != 2 {
        return;
    }
    let u = w.units.get_mut(&key).expect("checked by the caller");
    u.mode = r[1] as u32;
    u.flag_2 = Some(r[0] != 0);
}

/// The mode of the client skill start of codes 0x15 / 0x16 (S→C 0x4D /
/// 0x4C, `msg-units.md` §4 rule 1: record[0] = the skill id): the
/// skill's `skills.txt` `anim` (player modes) or `monanim` (monster
/// modes); a skill without a row or a mode past the type's table:
/// `None` (mode unchanged).
///
/// PROVISIONAL (client/model.md OQ 1; REC-51): the client skill start
/// `0x004C6F40` / `0x004C6EB0` is not specified; read as "mode := the
/// skill's animation mode". d2rs-own, unverified.
pub fn skill_mode(inputs: &ModelInputs, skill: i32, unit_type: u8) -> Option<u32> {
    let row = inputs.tables.skills.get(usize::try_from(skill).ok()?)?;
    let (m, count) = if unit_type == PLAYER {
        (row.anim, 20)
    } else {
        (row.monanim, MODE_ROWS.len() as u8)
    };
    (m < count).then_some(u32::from(m))
}

/// The monster machine `0x004AFF60` (open question 1).
///
/// PROVISIONAL (client/model.md OQ 1; REC-51): a client monster changes
/// mode only as the S→C messages state: the request code is the one the
/// server's mode table `0x006E1D90` (`sim/intents-events.md` §7.4 rule 1)
/// sends for a mode, so the mode is the first mode of that table (in
/// mode order) whose to-point or to-unit code is `code` (codes 12 / 13,
/// shared by S1 and SQ, read as S1); a code no mode sends changes
/// nothing. No client-side mode steps and no client seed draws.
pub fn monster_mode(code: u8) -> Option<u32> {
    MODE_ROWS
        .iter()
        .position(|r| r.code_to_point == code || r.code_to_unit == code)
        .map(|m| m as u32)
}
