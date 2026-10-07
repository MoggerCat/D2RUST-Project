// Spec: specs/client/model.md (§4 rules 5–6, §5), specs/drlg/rooms.md (§4.6 rule 1, last paragraph)
//! The client update pass: in a frame whose pump ran a server tick, while
//! `in_game`, each unit's queue is drained in the 1.14d unit order and
//! every queued message goes to its unit handler. After the units, the
//! client DRLG's part of the update runs: the build timer and, every 13th
//! update, the level free (`drlg/rooms.md` §4.6).
//!
//! PROVISIONAL (client/model.md OQ 1, OQ 2): the per-type unit updates
//! that run before each drain (player, monster, object, missile, item
//! mode machines `0x004AFF60` etc.; local walk prediction `0x00463390`)
//! change no model field: the client keeps no modes beyond what messages
//! state and the local player follows the server position. Settled by
//! the Phase 6 unit-modes spec plus a frames-raw `client_update` counter
//! vs server tick recording (`render/camera.md` OQ 5 / 8; HIGH-PRIORITY
//! CAPTURE: client seed draws in animation).

use super::dispatch::{Dispatch, Handle, HandlerError, UnitMessage};
use super::output::{Output, Outputs};
use super::receive::{ReceiveLog, Rejected};
use super::world::{update_order, ClientWorld, ModelInputs};

/// Drains every unit's queue (§5 rules 2–4). Returns the number of
/// messages applied. A handler error is recorded as a rejection; the
/// drain goes on with the next message. Handler outputs are appended to
/// `outputs` in update order (`bridge.md` §10 rule 2).
pub fn update_pass(
    world: &mut ClientWorld,
    inputs: &ModelInputs,
    dispatch: &Dispatch,
    log: &mut ReceiveLog,
    outputs: &mut Vec<Output>,
) -> usize {
    let mut applied = 0;
    for key in update_order(&world.units) {
        // Looked up again: an earlier unit's messages may have removed it
        // (§5 rule 2).
        let local = world.local_player == Some(key);
        let Some(unit) = world.units.get_mut(&key) else {
            continue;
        };
        // §5 rule 5: unit flag 0x800000 (the client room free) on a unit
        // other than the local player runs nothing; with flags-2 0x20 the
        // client sends C→S 0x4B [type u32][GUID u32] and clears both bits
        // (a server unit stays until the server answers).
        if unit.room_freed && !local {
            if unit.flags_ex & 0x20 != 0 {
                unit.room_freed = false;
                unit.flags_ex &= !0x20;
                let mut m = vec![0x4B];
                m.extend_from_slice(&u32::from(key.unit_type).to_le_bytes());
                m.extend_from_slice(&key.guid.to_le_bytes());
                world.outgoing.push(m);
            }
            continue;
        }
        let queue = std::mem::take(&mut unit.queue);
        for bytes in &queue {
            let id = bytes[0];
            let sink = Outputs::default();
            let result = match dispatch.get(id).map(|e| e.handle) {
                Some(Handle::Unit(handle)) => handle(
                    world,
                    &UnitMessage {
                        id,
                        bytes,
                        unit: key,
                        inputs,
                        out: &sink,
                    },
                ),
                // Only unit-handler ids are queued (§4 rule 1); 1.14d
                // asserts on anything else (§4 rule 5).
                _ => Err(HandlerError::Invalid(
                    "queued message without a unit handler",
                )),
            };
            match result {
                Ok(()) => {
                    applied += 1;
                    log.drained += 1;
                    outputs.extend(sink.take());
                }
                Err(error) => log.rejected.push(Rejected { id, error }),
            }
        }
    }
    if let Err(error) = drlg_update(world) {
        // Not a message: recorded under the client update's own id 0.
        // A DRLG error is a fatal error of the original's code.
        log.rejected.push(Rejected { id: 0, error });
    }
    applied
}

/// The client DRLG part of the update (`drlg/rooms.md` §4.6 rule 1, last
/// paragraph): the counter `[0x007A0498]` += 1, the build timer on the
/// client act's DRLG, then the level free when the counter is a multiple
/// of 13; new active rooms then run the act room callback. Nothing
/// without a client DRLG.
pub fn drlg_update(world: &mut ClientWorld) -> Result<(), HandlerError> {
    world.drlg_updates = world.drlg_updates.wrapping_add(1);
    let free = world.drlg_updates.is_multiple_of(13);
    let Some(drlg) = world.drlg.as_mut() else {
        return Ok(());
    };
    drlg.client_update(free)?;
    world.refresh_active_rooms()?;
    Ok(())
}
