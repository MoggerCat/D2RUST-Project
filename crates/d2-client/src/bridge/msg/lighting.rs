// Spec: specs/render/lighting.md (§9.1 creation, §9.2 r2–r4)
//! The environment record of the client act: created with the act
//! (0x03, [`create_environment`]), set by S→C 0x53 ([`darkness`]) and by
//! the eclipse of S→C 0x5D ([`eclipse`]). The record and its rules are
//! `crate::rules::lighting::environment`; the handlers here pick its
//! inputs from the model.

use super::super::dispatch::{HandlerError, Message};
use super::super::world::ClientWorld;
use super::Bytes;
use crate::rules::lighting::environment::{
    act_index, EnvError, Environment, PeriodTables, ECLIPSE_SET,
};

fn periods() -> Result<PeriodTables, HandlerError> {
    PeriodTables::builtin().map_err(|_| HandlerError::Invalid("render/env-periods.tsv"))
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
    // r4.4. TODO(spec: render/lighting.md §9.2 r4.4): the day period `p`
    // of `0x0061C100(act, 0)` (0–3) is not given as a function of the
    // record, and the object refresh `0x004BC5E0` is open question 11:
    // the cache `[0x007A6A74]` and the refresh are not run.
    // r4.5: the requirement refresh of 0x47 on P sets no model field
    // (`client/msg-stats-items.md` §3 rule 3).
    Ok(())
}
