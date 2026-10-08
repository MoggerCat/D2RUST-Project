// Spec: specs/audio/triggers.md (§2 r1, r4)
//! S→C 0x2C PlaySound: no model state; the event unit is looked up in
//! set S and one `ServerSound` output (`client/bridge.md` §10) carries
//! its key, class and position as read now (§10 r3.1 (b)), with the
//! event. The audio layer applies the event table (`audio/triggers.md`
//! §2 r2–r3) at delivery.

use super::super::dispatch::{HandlerError, Message};
use super::super::output::Output;
use super::super::world::{ClientWorld, UnitKey};
use super::Bytes;

/// 0x2C PlaySound (§2 r1, r4): unit type u8@1, GUID u32@2, event u16@6;
/// a unit not in S → nothing.
pub fn play_sound(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 8 {
        return Err(HandlerError::Invalid("0x2C is 8 bytes"));
    }
    let key = UnitKey::new(b.u8(1)?, b.u32(2)?);
    let event = b.u16(6)?;
    if let Some(u) = w.units.get(&key) {
        msg.out.push(Output::ServerSound {
            unit: key,
            class: u.class,
            at: u.position,
            event,
        });
    }
    Ok(())
}
