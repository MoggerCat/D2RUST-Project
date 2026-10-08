// Spec: specs/render/lighting.md (§9.1 creation, §9.2 r2–r4, §10 r4)
//! The environment record of the client act: created with the act
//! (0x03, [`create_environment`]), set by S→C 0x53 ([`darkness`]) and by
//! the eclipse of S→C 0x5D ([`eclipse`]) and 0x5A code 0x12
//! ([`event_eclipse`]); S→C 0x89 ([`unique_event`]) sets the scripted
//! override globals (§10 r4). The record and its rules are
//! `crate::rules::lighting::environment`; the handlers here pick its
//! inputs from the model.

use super::super::dispatch::{HandlerError, Message};
use super::super::world::{ClientWorld, UnitKey, OBJECT};
use super::Bytes;
use crate::rules::lighting::environment::{
    act_index, EnvError, Environment, PeriodTables, ECLIPSE_SET,
};
use crate::rules::lighting::records::{unit_light_pos, LightKind, Owner};
use crate::rules::lighting::sources::{self, ObjectLight};

/// The period tables of `render/env-periods.tsv` (§9.1), parsed once.
pub(crate) fn periods() -> Result<PeriodTables, HandlerError> {
    static TABLES: std::sync::OnceLock<Option<PeriodTables>> = std::sync::OnceLock::new();
    TABLES
        .get_or_init(|| PeriodTables::builtin().ok())
        .ok_or(HandlerError::Invalid("render/env-periods.tsv"))
}

/// The lighting part of a client update (`0x0044C790`, once per client
/// update, never per drawn frame), with `L` = the local player's room's
/// level id (0 when none):
/// 1. the scripted overrides (`0x0046BEB0`, §10 r5): the darkness step
///    (§10 r3, base = the room ambient without override, §3.1 r2–r3),
///    then the Den and levels 107/108 counters;
/// 2. the environment record (`0x0061BFC0`, §9.2 r1).
///
/// The spec places `0x0046BEB0` at `0x0044C7B0` and does not order
/// `0x0061BFC0` against it or the unit walk; neither reads the other's
/// state. The Den lights the Den counter starts at 30 (§10 r1:
/// client missile 287, two player-seed draws per try) are not placed:
/// the bridge has no client missile creation yet.
pub fn lighting_update(
    w: &mut ClientWorld,
    levels: &[super::super::world::LevelRow],
) -> Result<(), HandlerError> {
    let level = w.player_level().map_or(0, u32::from);
    if w.overrides.darkness.is_some() {
        let defs = levels
            .get(level as usize)
            .map(|r| r.ambient)
            .unwrap_or_default();
        let base = match &w.environment {
            Some(env) => {
                crate::rules::lighting::environment::room_ambient_without_override(defs, env)
            }
            None => defs,
        };
        w.overrides.update_darkness(base, level);
    }
    // TODO(spec: render/lighting.md §10 r1): the Den lights of a `true`.
    let _den_lights = w.overrides.update_counters();
    if let Some(env) = w.environment.as_mut() {
        env.update(&periods()?, level);
    }
    Ok(())
}

impl From<EnvError> for HandlerError {
    fn from(e: EnvError) -> Self {
        match e {
            EnvError::BadIndex(_) => {
                HandlerError::Invalid("0x53: period index outside 0..=5 (fatal in 1.14d)")
            }
            EnvError::NegativeTicks(_) => {
                HandlerError::Invalid("0x53: negative ticks (fatal in 1.14d)")
            }
        }
    }
}

/// The act's environment record at creation (`0x0061BE40`, §9.1).
/// `GetTickCount()` at creation (+0x10) has no client reader (§9.1) and
/// the bridge reads no clock: 0.
pub fn create_environment() -> Result<Environment, HandlerError> {
    Ok(Environment::new(&periods()?, 0))
}

/// The act load's eclipse (§9.2 r3, `0x0044E142`): with the pending flag
/// set and act 2 (byte 1) loaded, the setter runs with the eclipse.
pub fn act_load_eclipse(w: &mut ClientWorld, act: u8) -> Result<(), HandlerError> {
    if !(w.eclipse_pending && act == 1) {
        return Ok(());
    }
    set_eclipse(w)
}

/// S→C 0x5A code 0x12 (`client/msg-ui.md` §19 r2): with a client act,
/// the setter `0x0061C240(act, room of the local player, 5, 0, 1)`.
pub fn event_eclipse(w: &mut ClientWorld) -> Result<(), HandlerError> {
    if w.act.is_none() {
        return Ok(());
    }
    set_eclipse(w)
}

/// S→C 0x89 UniqueEvent (§10 r4, `0x0045EA30` → `0x0046B630`): id u8@1;
/// id ≥ 32 is fatal 0x1D9, ≥ 20 fatal 0x1DA; else the id's bit and its
/// handler's override state (`crate::rules::lighting::overrides`).
/// The client effects of ids 1, 3, 6, 12 (level 108), 14, 16, 17, 19
/// (client missiles, `0x0046F870`) are Phase 6 effects with no §10
/// output row.
pub fn unique_event(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 2 {
        return Err(HandlerError::Invalid("0x89 is 2 bytes"));
    }
    let id = b.u8(1)?;
    match id {
        32.. => return Err(HandlerError::Fatal(0x1D9)),
        20.. => return Err(HandlerError::Fatal(0x1DA)),
        _ => {}
    }
    let level = w.player_level().map_or(0, u32::from);
    // Id 13: `[0x007A7464]` := `0x00410A80()` + 90 (§10 r4): wall-clock
    // seconds, no RNG draw (the host's input).
    let now = msg.inputs.wall_seconds.map_or(0, |f| f());
    w.overrides
        .unique_event(id, level, || now)
        .map_err(|_| HandlerError::Invalid("0x89 id"))?;
    Ok(())
}

fn set_eclipse(w: &mut ClientWorld) -> Result<(), HandlerError> {
    let level = w.player_level().map_or(0, u32::from);
    let env = w.environment.as_mut().ok_or(HandlerError::Invalid(
        "client act without its environment record",
    ))?;
    let (index, ticks, eclipse) = ECLIPSE_SET;
    env.set_from_server(&periods()?, index, ticks, eclipse, act_index(level), level)?;
    Ok(())
}

/// The eclipse of S→C 0x5D (§9.2 r3, `0x0044C820`): with a client act the
/// setter runs now with index 5, ticks 0, eclipse 1; without one the
/// pending flag `[0x007A060E]` is set.
pub fn eclipse(w: &mut ClientWorld) -> Result<(), HandlerError> {
    if w.act.is_none() {
        w.eclipse_pending = true;
        return Ok(());
    }
    set_eclipse(w)
}

/// The object day/night refresh of §9.2 r4.4: every object of set S in
/// bucket order (`GUID & 0x7F` ascending, each chain in descending GUID,
/// `client/model.md` §2) gets `0x004BC5E0(obj, 0)` (open question 11).
fn day_refresh(w: &mut ClientWorld, msg: &Message<'_>, p: i32) -> Result<(), HandlerError> {
    let mut objects: Vec<UnitKey> = w
        .units
        .keys()
        .filter(|k| k.unit_type == OBJECT)
        .copied()
        .collect();
    objects.sort_by_key(|k| (k.guid & 0x7F, std::cmp::Reverse(k.guid)));
    for key in objects {
        object_env_refresh(w, msg, key, p)?;
    }
    Ok(())
}

/// `0x004BC5E0(object, 0)` (`render/lighting.md` OQ 11): only objects
/// whose `EnvEffect` ≠ 0 change. p 1–3: mode 0 → mode 1 and flag 0x2 :=
/// `Selectable1`; then the light of the (new) mode. p 0: mode 1 or 2 →
/// mode 0, flag 0x2 := `Selectable0`, the light of mode 0; mode 0 →
/// nothing. p > 3: fatal 0x66. The graphics refresh and animation
/// re-init are render state. Without the class's `objects.txt` row the
/// test cannot run: nothing changes.
fn object_env_refresh(
    w: &mut ClientWorld,
    msg: &Message<'_>,
    key: UnitKey,
    p: i32,
) -> Result<(), HandlerError> {
    let Some(u) = w.units.get(&key) else {
        return Ok(());
    };
    let Some(row) = msg.inputs.tables.objects.get(u.class as usize).copied() else {
        return Ok(());
    };
    if !row.env_effect {
        return Ok(());
    }
    let mode = u.mode;
    let new_mode = match p {
        1..=3 => {
            if mode == 0 {
                Some(1)
            } else {
                None
            }
        }
        0 => match mode {
            1 | 2 => Some(0),
            _ => return Ok(()),
        },
        _ => return Err(HandlerError::Fatal(0x66)),
    };
    let mode = new_mode.unwrap_or(mode);
    if let Some(m) = new_mode {
        let u = w.units.get_mut(&key).expect("present");
        u.mode = m;
        u.flag_2 = Some(row.selectable[m as usize & 7]);
    }
    let lit = row.lit[mode as usize & 7];
    object_light(w, key, lit, row.rgb);
    Ok(())
}

/// The object light `0x004BC580` (`render/lighting.md` §8 object row):
/// kind 2, radius `Lit` / 2, the objects color; `Lit` = 0 removes it;
/// an object with a light gets a new target.
pub fn object_light(w: &mut ClientWorld, key: UnitKey, lit: u8, rgb: (u8, u8, u8)) {
    object_light_of(w, key, false, lit, rgb);
}

/// [`object_light`] for an object of set S (`client_only` false) or set
/// C (the `ObjFx::Light` of the client object code,
/// `world/objects-client.md` §28 r3).
pub fn object_light_of(
    w: &mut ClientWorld,
    key: UnitKey,
    client_only: bool,
    lit: u8,
    rgb: (u8, u8, u8),
) {
    let owner = Owner {
        unit_type: u32::from(key.unit_type),
        guid: key.guid,
        client_only,
    };
    let current = w
        .lights
        .iter()
        .find(|(_, r)| r.owner() == Some(owner))
        .map(|(id, _)| id);
    match sources::object_light(lit, current.is_some(), rgb) {
        ObjectLight::Remove => {
            if let Some(id) = current {
                let _ = w.lights.remove(id);
            }
        }
        ObjectLight::SetTarget(r) => {
            if let Some(id) = current {
                w.lights.set_target(id, r);
            }
        }
        ObjectLight::Create(req) => {
            let set = if client_only {
                &w.objclient.set_c
            } else {
                &w.units
            };
            let (x, y) = set.get(&key).map_or((0, 0), |u| u.cell());
            let pos = (
                unit_light_pos(i32::from(x) << 16),
                unit_light_pos(i32::from(y) << 16),
            );
            w.lights.create(
                Some(owner),
                pos,
                LightKind::Cached,
                req.radius,
                req.i,
                req.r,
                req.g,
                req.b,
            );
        }
    }
}

/// S→C 0x53 Darkness (§9.2 r4): u32@1 period index, u32@5 ticks, u8@9
/// eclipse, applied to the client act's record when the client act is
/// the local player's.
pub fn darkness(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 10 {
        return Err(HandlerError::Invalid("0x53 is 10 bytes"));
    }
    let (index, ticks, flag) = (b.u32(1)? as i32, b.u32(5)? as i32, b.u8(9)?);
    // r4.1.
    let p = w
        .local()
        .ok_or(HandlerError::Invalid(
            "0x53 without a local player (1.14d reads through a null pointer)",
        ))?
        .key;
    // r4.2: P has an act once it is placed (creation at a point,
    // `client/model.md` §2 rule 6, or 0x15); the model has one client act.
    let placed = w.units[&p].position.is_some();
    if w.act.is_none() {
        if placed {
            return Ok(());
        }
        // Both pointers null: the original goes on with no act.
        return Err(HandlerError::Unspecified(
            "render/lighting.md §9.2 r4.2: 0x53 with no client act and an unplaced player",
        ));
    }
    if !placed {
        return Ok(());
    }
    // r4.3: the setter with P's room (none → `L` = 0).
    let level = w.unit_room(p).map_or(0, |r| u32::from(r.level));
    let env = w.environment.as_mut().ok_or(HandlerError::Invalid(
        "client act without its environment record",
    ))?;
    env.set_from_server(&periods()?, index, ticks, flag, act_index(level), level)?;
    // r4.4: the day period p (env +0x04) against the cache.
    let p = env.kind;
    if p != w.env_period_cache {
        w.env_period_cache = p;
        day_refresh(w, msg, p)?;
    }
    // r4.5: the requirement refresh of 0x47 on P sets no model field
    // (`client/msg-stats-items.md` §3 rule 3).
    Ok(())
}
