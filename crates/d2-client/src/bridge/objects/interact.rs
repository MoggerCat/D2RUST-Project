// Spec: specs/client/model.md (§8 rules 4 and 7), specs/world/objects-client.md (§26.13 r6)
//! The interact sender `0x00480930(type, GUID)` and its two callers: the
//! player mode request code 0x02 (`model.md` §8 rule 4) and the
//! local-player command 0x13 of `ClientFn` 13 (`0x00481030`, with the
//! action gate `0x00480BA0`).

use super::super::dispatch::HandlerError;
use super::super::output::Output;
use super::super::world::{
    ClientWorld, ModeRequest, ModelInputs, UnitKey, ITEM, MONSTER, OBJECT, PLAYER,
};
use super::{ObjFx, ObjSound};

/// C→S 0x13 interact (9 bytes, `0x004786A0`).
pub const INTERACT: u8 = 0x13;
/// C→S 0x16 item pick-up (13 bytes, `0x004786D0`).
pub const PICKUP: u8 = 0x16;
/// The monster interact repeat window, ms (`0x00480AA7`).
pub const MONSTER_REPEAT_MS: u32 = 200;
/// The mode request code that sends the interact (§8 rule 4).
pub const CODE_INTERACT: u8 = 0x02;
/// The player mode that ignores a flag-0 request (§8 rule 4).
pub const MODE_SEQUENCE: u32 = 0x13;

/// C→S 0x13 {0x13, type u32, GUID u32}.
pub fn interact_bytes(unit_type: u32, guid: u32) -> Vec<u8> {
    let mut m = vec![INTERACT];
    m.extend_from_slice(&unit_type.to_le_bytes());
    m.extend_from_slice(&guid.to_le_bytes());
    m
}

/// The interact sender `0x00480930(type, GUID)` (§8 rule 7). U is looked
/// up in set S; none → nothing. Sends append to `outgoing`; the sound
/// and effect calls are returned in call order.
/// TODO(spec: client/model.md §8 rule 7): the path reset `0x00648B90` of
/// the object case has no client path record in the model.
pub fn send(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    unit_type: u16,
    guid: u32,
) -> Result<Vec<Output>, HandlerError> {
    let mut out = Vec::new();
    let Ok(t) = u8::try_from(unit_type) else {
        return Ok(out);
    };
    let key = UnitKey::new(t, guid);
    if !w.units.contains_key(&key) {
        return Ok(out);
    }
    let p = w.local_player.filter(|k| w.units.contains_key(k));
    let ty = u32::from(unit_type);
    match t {
        PLAYER => {
            face(w, p, key)?;
            w.outgoing.push(interact_bytes(ty, guid));
        }
        MONSTER => {
            let now = inputs.now;
            let u = w.units.get_mut(&key).expect("looked up above");
            if now.wrapping_sub(u.interact_ms) >= MONSTER_REPEAT_MS {
                u.interact_ms = now;
                w.outgoing.push(interact_bytes(ty, guid));
            }
        }
        OBJECT => object(w, inputs, p, key, &mut out)?,
        ITEM => {
            let mut m = vec![PICKUP];
            m.extend_from_slice(&4u32.to_le_bytes());
            m.extend_from_slice(&guid.to_le_bytes());
            m.extend_from_slice(&u32::from(inputs.objclient.pickup_flag).to_le_bytes());
            w.outgoing.push(m);
        }
        _ => {}
    }
    Ok(out)
}

/// P faces U (`0x00621C00(P, x, y)` with U's client point): kept as the
/// turn's input (`ClientUnit::turned_toward`).
fn face(w: &mut ClientWorld, p: Option<UnitKey>, key: UnitKey) -> Result<(), HandlerError> {
    let p = p.ok_or(HandlerError::Crash {
        at: 0x0048_0930,
        what: "the local player (none) in the interact sender",
    })?;
    if let Some(pu) = w.units.get_mut(&p) {
        pu.turned_toward = Some(key);
    }
    Ok(())
}

/// The object case of §8 rule 7.
fn object(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    p: Option<UnitKey>,
    key: UnitKey,
    out: &mut Vec<Output>,
) -> Result<(), HandlerError> {
    let (flag_4, class, mode) = {
        let u = &w.units[&key];
        (u.flag_4, u.class, u.mode)
    };
    if !flag_4 {
        w.outgoing.push(interact_bytes(u32::from(OBJECT), key.guid));
        return Ok(());
    }
    face(w, p, key)?;
    let p = p.expect("face checked it");
    let mut r = [0i32; 7];
    r[0] = skill_id_of(w, p, 1)?;
    let needs = match (class, mode) {
        (404, 0) => Some(*b"qf2 "),
        (376, 2) => Some(*b"hfh "),
        _ => None,
    };
    if let Some(code) = needs {
        if inputs.objclient.hand_code != Some(code) {
            out.push(Output::ObjectSound(ObjSound::PlayerEvent {
                player: p,
                event: INTERACT,
            }));
            return Ok(());
        }
        r[0] = skill_id_of(w, p, 0)?;
    }
    if !(class == 376 && mode == 0) {
        r[1] = -1;
        r[2] = 2;
        r[3] = key.guid as i32;
        out.push(Output::ObjectFx(ObjFx::SkillStart {
            player: p,
            record: r,
        }));
    }
    w.outgoing.push(interact_bytes(u32::from(OBJECT), key.guid));
    Ok(())
}

/// The id (`0x00643CE0`) of P's entry of `skill`.
/// PROVISIONAL (client/model.md §8 rule 7; REC-objclient-2): "P's skill
/// on side 1 (`0x006439F0`)" is read as `0x006439F0(P, 1)`, the highest
/// entry of skill 1 (`skills/levels.md` §1), and "skill 0" as the native
/// entry of skill 0; either entry's id is its skill. No entry: the id of
/// a null entry is not specified (an error).
fn skill_id_of(w: &ClientWorld, p: UnitKey, skill: u16) -> Result<i32, HandlerError> {
    w.units[&p]
        .skills
        .as_ref()
        .and_then(|l| {
            l.entries
                .iter()
                .find(|e| e.skill == skill && !e.has_charges)
        })
        .map(|e| i32::from(e.skill))
        .ok_or(HandlerError::Unspecified(
            "client/model.md §8 rule 7: the skill id of a missing entry",
        ))
}

/// The player mode request code 0x02 of a queued message (§8 rule 4,
/// flag 1): `0x00480930(r0 & 0xFFFF, r1)`.
pub fn mode_request_code_2(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    record: [i32; 7],
) -> Result<Vec<Output>, HandlerError> {
    send(w, inputs, (record[0] & 0xFFFF) as u16, record[1] as u32)
}

/// The action gate `0x00480BA0` (P): 1 when P's current skill
/// (`0x00620250`) is none, its id is out of range, its `skills` row has
/// the gate bit, or P's mode is 1 or 5; else 0.
pub fn action_gate(w: &ClientWorld, inputs: &ModelInputs, p: UnitKey) -> bool {
    let Some(pu) = w.units.get(&p) else {
        return true;
    };
    if matches!(pu.mode, 1 | 5) {
        return true;
    }
    let Some(e) = pu
        .skills
        .as_ref()
        .and_then(|l| l.current.and_then(|i| l.entries.get(i)))
    else {
        return true;
    };
    let id = usize::from(e.skill);
    id >= inputs.tables.skills.len()
        || inputs
            .objclient
            .skill_gate
            .get(id)
            .copied()
            .unwrap_or(false)
}

/// The local-player command 0x13 on (2, GUID) (`0x00481030`, §26.13 r6):
/// the gate returns 0 → nothing; else P's mode request code 2 with
/// record {2, GUID, 0, …} and flag 0 (`0x00480C10`): stored like every
/// request (§8 rule 3); P in mode 0x13 ignores it; else the interact
/// sender. `0x00480B40` sends nothing more for command 0x13.
pub fn command_13(
    w: &mut ClientWorld,
    inputs: &ModelInputs,
    p: UnitKey,
    guid: u32,
) -> Result<Vec<Output>, HandlerError> {
    if !action_gate(w, inputs, p) {
        return Ok(Vec::new());
    }
    let record = [i32::from(OBJECT), guid as i32, 0, 0, 0, 0, 0];
    let Some(pu) = w.units.get_mut(&p) else {
        return Ok(Vec::new());
    };
    pu.last_mode_request = Some(ModeRequest {
        code: CODE_INTERACT,
        record,
    });
    if pu.mode == MODE_SEQUENCE {
        return Ok(Vec::new());
    }
    mode_request_code_2(w, inputs, record)
}
