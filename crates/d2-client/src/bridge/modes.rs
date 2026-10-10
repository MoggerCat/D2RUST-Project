// Spec: specs/client/model.md (§8 rules 1–6, §15 rule 3, §18 rule 1–2, §19 r3–r4, open questions 1–2), specs/audio/triggers-2.md (§20 r3)
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
//! - monster `0x004AFF60` (§19 rules 3–4): the mode column of the
//!   dispatch, see [`monster`];
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
) -> Result<bool, HandlerError> {
    // A missile's request returns 1 at once (rule 1): not stored.
    if key.unit_type == MISSILE {
        return Ok(true);
    }
    let Some(u) = w.units.get_mut(&key) else {
        return Ok(true);
    };
    // Rule 3.
    u.last_mode_request = Some(ModeRequest { code, record });
    u.mode_requests = u.mode_requests.wrapping_add(1);
    match key.unit_type {
        PLAYER => player(w, inputs, key, code, record),
        OBJECT => object(w, inputs, key, code, record, out).map(|()| true),
        ITEM => {
            item(w, key, code, record);
            Ok(true)
        }
        MONSTER => monster(w, inputs, key, code, record).map(|()| true),
        _ => Ok(true),
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
) -> Result<bool, HandlerError> {
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
    // The mode set `0x00624690` with the animation restart the client
    // player update ends the mode by ([`super::player_anim`]).
    let set = |w: &mut ClientWorld, m: u32| {
        super::player_anim::mode_set(w, inputs, key, m);
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
            // Step 5: the `cltstfunc` returns 0 → current := none, mode
            // set 1, and the request returns 0. 0x15's start clears the
            // target (§8 r7); 0x16's is the unit of (r2 type, r3 GUID).
            let target = (code == 0x16).then(|| UnitKey::new(r[2] as u8, r[3] as u32));
            if !super::use_state::client_start_passes(w, inputs, key, r[0] as u16, target) {
                if let Some(l) = w.units.get_mut(&key).and_then(|u| u.skills.as_mut()) {
                    l.current = None;
                }
                set(w, player_mode::NEUTRAL);
                return Ok(false);
            }
        }
        0x17 | 0x18 => set(w, player_mode::RUN),
        0x19 => {
            set(w, player_mode::BLOCK);
            check(w, inputs, key, x, y, 0, 0, 0)?;
        }
        _ => unreachable!("fatal codes returned above"),
    }
    Ok(true)
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
            // The animation set-up of the new mode (`world/objects-client.md`
            // §25 r8; measured REC-440: 1.14d's `0x004BCF60` runs
            // `0x00624390` itself after its `set_mode`, drawing again even
            // in the same mode, `facts/objects/objanim-a1-town.tsv`). Nothing without rows.
            if let Some(row) = inputs.objclient.rows.get(class as usize) {
                super::objects::anim_setup(u, row, mode)?;
            }
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

/// Monster modes of the machine (`client/model.md` §19).
pub mod monster_mode {
    pub const DEATH: u32 = 0;
    pub const NEUTRAL: u32 = 1;
    pub const WALK: u32 = 2;
    pub const GET_HIT: u32 = 3;
    pub const ATTACK1: u32 = 4;
    pub const ATTACK2: u32 = 5;
    pub const BLOCK: u32 = 6;
    pub const CAST: u32 = 7;
    pub const SKILL1: u32 = 8;
    pub const SKILL2: u32 = 9;
    pub const SKILL3: u32 = 10;
    pub const SKILL4: u32 = 11;
    pub const DEAD: u32 = 12;
    pub const KNOCKBACK: u32 = 13;
    pub const SEQUENCE: u32 = 14;
    pub const RUN: u32 = 15;
}

/// The mode table `0x006DA4D8` (`client/model.md` §19 r4 "T") of the
/// point and unit groups: the mode their code sets.
pub fn monster_table_mode(code: u8) -> Option<u32> {
    use monster_mode as m;
    Some(match code {
        0x04 | 0x05 => m::CAST,
        0x0A | 0x0B => m::ATTACK1,
        0x0C | 0x0D => m::SKILL1,
        0x0E | 0x0F => m::SKILL2,
        0x10 | 0x11 => m::ATTACK2,
        0x1A | 0x1B => m::SKILL3,
        0x1C | 0x1D => m::SKILL4,
        _ => return None,
    })
}

/// The neutral fallback "F" `0x004AE1D0` (§19 r4): a monster in mode
/// 1…15 other than 12 is set to mode 1; any other mode: nothing.
fn neutral_fallback(w: &mut ClientWorld, inputs: &ModelInputs, key: UnitKey) {
    let mode = w.units.get(&key).expect("checked by the caller").mode;
    if (1..=15).contains(&mode) && mode != monster_mode::DEAD {
        super::monster_anim::mode_set(w, inputs, key, monster_mode::NEUTRAL);
    }
}

/// The monster machine `0x004AFF60` (`client/model.md` §19 rules 3–4),
/// mode column: the class gate of the head (a class outside `monstats`
/// or without a `monstats2` row: no switch), then the dispatch by code.
/// The path, light, overlay and stat effects of the rows belong to their
/// own seams; NPC busy (monster data +0x28 bit 0) and the BaseId cases
/// (`iceglobe`, `vulture1`, the rule 5 death switch) read fields the
/// model does not hold and are not taken here.
///
/// PROVISIONAL (client/model.md OQ 1; REC-51): the client path helpers
/// (`0x00480780` to a unit, `0x004804A0` to a point) succeed when their
/// target is there (the model holds no client path record), as the
/// player machine's; and the "class has mode 2" test of code 7
/// (`0x0046C140`) reads as true (the client tables hold no monstats2
/// mode bits).
fn monster(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    code: u8,
    r: [i32; 7],
) -> Result<(), HandlerError> {
    use monster_mode as m;
    // §19 r3 class gate.
    let class = w.units[&key].class;
    if !inputs
        .tables
        .monsters
        .get(class as usize)
        .is_some_and(|c| c.is_some())
    {
        return Ok(());
    }
    // The mode set restarts the unit's animation (`monster_anim`).
    let set = |w: &mut ClientWorld, mode: u32| {
        super::monster_anim::mode_set(w, inputs, key, mode);
    };
    // +0xB0 (§19 r4: r6 for 0x06 / 0x14, r0 for 0x13).
    let hit = |w: &mut ClientWorld, h: i32| {
        w.units.get_mut(&key).expect("present").hit_class = h as u32;
    };
    match code {
        // Path to the unit (r0 type, r1 GUID): an absent unit → F.
        0x00 | 0x18 => {
            let target = UnitKey::new(r[0] as u8, r[1] as u32);
            if w.units.contains_key(&target) {
                set(w, if code == 0x00 { m::WALK } else { m::RUN });
            } else {
                neutral_fallback(w, inputs, key);
            }
        }
        0x01 | 0x17 => set(w, if code == 0x01 { m::WALK } else { m::RUN }),
        0x04 | 0x0B | 0x0C | 0x0E | 0x11 | 0x1A | 0x1C | 0x05 | 0x0A | 0x0D | 0x0F | 0x10
        | 0x1B | 0x1D => set(w, monster_table_mode(code).expect("table code")),
        0x06 => {
            hit(w, r[6]);
            set(w, m::GET_HIT);
        }
        0x07 => {
            // Position check (§6, kind 0), then within 1 sub-tile of
            // (r0, r1) → F; else walk (state 143 `attached` → F).
            let (x, y) = ((r[0] & 0xFFFF) as u16, (r[1] & 0xFFFF) as u16);
            check(w, inputs, key, x, y, 0, 0, 0)?;
            let u = &w.units[&key];
            let (cx, cy) = u.cell();
            let near = (i32::from(cx) - i32::from(x)).abs() <= 1
                && (i32::from(cy) - i32::from(y)).abs() <= 1;
            if near || u.states.contains(&143) {
                neutral_fallback(w, inputs, key);
            } else {
                set(w, m::WALK);
            }
        }
        // Rule 5 default D0.
        0x08 => set(w, m::DEATH),
        // `msg-units.md` §4 r6.2: mode := 0xC.
        0x09 => set(w, m::DEAD),
        0x12 => set(w, m::BLOCK),
        // No mode change (the KB mode comes with 0x14); the mode sound
        // `0x004CC5B0(U, 0xD, 1)` is the audio feed's.
        0x13 => hit(w, r[0]),
        0x14 => {
            hit(w, r[6]);
            set(w, m::KNOCKBACK);
        }
        // The skill entry's mode; 0xE (sequence) sets no mode.
        0x15 | 0x16 => {
            if let Some(mode) = skill_mode(inputs, r[0], MONSTER) {
                if mode != m::SEQUENCE {
                    set(w, mode);
                }
            }
        }
        // 0x02, 0x03, 0x19 and unknown codes.
        _ => neutral_fallback(w, inputs, key),
    }
    // The tail (`model.md` §19 r6) of a pathed request (a record and a
    // code other than 0x13, 0x15, 0x16, §19 r3): stat 67 from r4.
    if !matches!(code, 0x13 | 0x15 | 0x16) {
        super::monster_anim::velocity_tail(w, inputs, key, r[4]);
    }
    Ok(())
}
