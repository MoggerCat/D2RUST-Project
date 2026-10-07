// Spec: specs/client/model.md (§4 rules 5–6, §5), specs/drlg/rooms.md (§4.6 rule 1, last paragraph), specs/world/objects-client.md (§25 r2–r3, §28 r2)
//! The client update pass: in a frame whose pump ran a server tick, while
//! `in_game`, each unit's queue is drained in the 1.14d unit order and
//! every queued message goes to its unit handler. Before an object's
//! drain its per-type update runs (`0x004BDFF0`, [`super::objects`]);
//! the C objects (set C) run theirs, then the second `ClientFn` call, at
//! their place in the walk (§5 rule 3: after the S and C missiles, before
//! the S players). The other per-type unit updates (player, monster,
//! missile, item) are Phase 6 client behaviour: TODO(spec: model.md open
//! questions 1, 2). After the units, the client DRLG's part of the update
//! runs: the build timer and, every 13th update, the level free
//! (`drlg/rooms.md` §4.6).

use super::dispatch::{Dispatch, Handle, HandlerError, UnitMessage};
use super::objects::{self, ObjUnit};
use super::output::{Output, Outputs};
use super::receive::{ReceiveLog, Rejected};
use super::world::{update_order, ClientWorld, ModelInputs, OBJECT};

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
    let order = update_order(&world.units);
    // §5 rule 3: S missiles first, then the C missiles (their update is
    // Phase 6) and the C objects; then the other S types.
    let missiles = order
        .iter()
        .take_while(|k| k.unit_type == super::world::MISSILE)
        .count();
    let mut c_objects_done = false;
    // TODO(spec: model.md open question 4): unit flag 0x800000 skips a
    // unit that is not the local player; no client rule sets it yet.
    for (i, &key) in order.iter().enumerate() {
        if i >= missiles && !c_objects_done {
            c_objects_done = true;
            c_objects(world, inputs, log, outputs);
        }
        if key.unit_type == OBJECT && world.units.contains_key(&key) {
            // The object update `0x004BDFF0` (§5 rule 2).
            let unit = ObjUnit {
                key,
                client_only: false,
            };
            if let Err(error) = objects::object_update(world, inputs, unit, outputs) {
                log.rejected.push(Rejected { id: 0, error });
            }
        }
        // Looked up again: an earlier unit's messages or its own update
        // may have removed it (§5 rule 2).
        let Some(unit) = world.units.get_mut(&key) else {
            continue;
        };
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
    if !c_objects_done {
        c_objects(world, inputs, log, outputs);
    }
    // TODO(spec: model.md §5 rule 3): the C monsters walk last; their
    // update and `0x0046D780` are Phase 6, so it runs nothing yet.
    if let Err(error) = drlg_update(world) {
        // Not a message: recorded under the client update's own id 0.
        // A DRLG error is a fatal error of the original's code.
        log.rejected.push(Rejected { id: 0, error });
    }
    applied
}

/// The C objects' walk (`0x00463CC0`, §5 rule 3): each runs the object
/// update (call site A), then, still in set C, the dispatch once more
/// (call site B). Errors are recorded under the update's id 0.
fn c_objects(
    world: &mut ClientWorld,
    inputs: &ModelInputs,
    log: &mut ReceiveLog,
    outputs: &mut Vec<Output>,
) {
    for key in objects::c_order(world, OBJECT) {
        if !world.objclient.set_c.contains_key(&key) {
            continue;
        }
        let unit = ObjUnit {
            key,
            client_only: true,
        };
        let r = objects::object_update(world, inputs, unit, outputs)
            .and_then(|()| objects::site_b(world, inputs, key, outputs));
        if let Err(error) = r {
            log.rejected.push(Rejected { id: 0, error });
        }
    }
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
