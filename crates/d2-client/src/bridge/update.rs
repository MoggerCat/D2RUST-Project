// Spec: specs/client/model.md (§4 rules 5–6, §5)
//! The client update pass: in a frame whose pump ran a server tick, while
//! `in_game`, each unit's queue is drained in the 1.14d unit order and
//! every queued message goes to its unit handler. The per-type unit
//! updates that run before each drain (player, monster, object, missile,
//! item) are Phase 6 client behaviour: TODO(spec: model.md open
//! questions 1, 2).

use super::dispatch::{Dispatch, Handle, HandlerError, UnitMessage};
use super::receive::{ReceiveLog, Rejected};
use super::world::{update_order, ClientWorld, ModelInputs};

/// Drains every unit's queue (§5 rules 2–4). Returns the number of
/// messages applied. A handler error is recorded as a rejection; the
/// drain goes on with the next message.
pub fn update_pass(
    world: &mut ClientWorld,
    inputs: &ModelInputs,
    dispatch: &Dispatch,
    log: &mut ReceiveLog,
) -> usize {
    let mut applied = 0;
    // TODO(spec: model.md open question 4): unit flag 0x800000 skips a
    // unit that is not the local player; no client rule sets it yet.
    for key in update_order(&world.units) {
        // Looked up again: an earlier unit's messages may have removed it
        // (§5 rule 2).
        let Some(unit) = world.units.get_mut(&key) else {
            continue;
        };
        let queue = std::mem::take(&mut unit.queue);
        for bytes in &queue {
            let id = bytes[0];
            let result = match dispatch.get(id).map(|e| e.handle) {
                Some(Handle::Unit(handle)) => handle(
                    world,
                    &UnitMessage {
                        id,
                        bytes,
                        unit: key,
                        inputs,
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
                }
                Err(error) => log.rejected.push(Rejected { id, error }),
            }
        }
    }
    applied
}
