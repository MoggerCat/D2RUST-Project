// Spec: specs/skills/sequences.md (client mode machine), specs/client/model.md (§4 rules 5–6, §5, §5 rules 6.1 and 6.4), specs/drlg/rooms.md (§4.6 rule 1, last paragraph), specs/world/objects-client.md (§25 r2–r3, §28 r2), specs/render/lighting.md (§9.2 r1, §10 r5)
//! The client update pass: in a frame whose pump ran a server tick, while
//! `in_game`, each unit's queue is drained in the 1.14d unit order and
//! every queued message goes to its unit handler. Before an object's
//! drain its per-type update runs (`0x004BDFF0`, [`super::objects`]);
//! the C objects (set C) run theirs, then the second `ClientFn` call, at
//! their place in the walk (§5 rule 3: after the S and C missiles, before
//! the S players). After the units, the client DRLG's part of the update
//! runs: the build timer and, every 13th update, the level free
//! (`drlg/rooms.md` §4.6).
//!
//! The mode machines themselves run inside the queued messages' mode
//! requests ([`super::modes`], `model.md` §8). The other per-type unit
//! updates that run before each drain (player `0x00463390`, monster
//! `0x004B13A0`, missile, item) change no model field, except the
//! monster anim step ([`super::monster_anim`]) and the player's mode end
//! ([`super::player_anim`]):
//! PROVISIONAL (client/model.md OQ 1, OQ 2; REC-51): no other client-side
//! mode steps, no client seed draws in the animation and no local walk
//! prediction (the local player follows the server position).

use super::dispatch::{Dispatch, Handle, HandlerError, UnitMessage};
use super::msg::lighting::{lighting_update, object_light_of};
use super::objects::{self, ObjFx, ObjUnit};
use super::output::{move_freed, Output, Outputs};
use super::receive::{ReceiveLog, Rejected};
use super::world::{update_order, ClientWorld, ModelInputs, MONSTER, OBJECT};

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
    // §5 r6.1: the room pass `0x0044C750` runs first (critters and
    // client presets of the rooms not yet populated).
    super::critters::room_pass(world, inputs);
    let order = update_order(&world.units);
    // §5 rule 3: S missiles first, then the C missiles
    // (`missiles/client.md` §C6) and the C objects; then the other S
    // types.
    let missiles = order
        .iter()
        .take_while(|k| k.unit_type == super::world::MISSILE)
        .count();
    let mut c_objects_done = false;
    for (i, &key) in order.iter().enumerate() {
        if i >= missiles && !c_objects_done {
            c_objects_done = true;
            c_missiles(world, inputs, log, outputs);
            c_objects(world, inputs, log, outputs);
        }
        let local = world.local_player == Some(key);
        // §5 rule 5: unit flag 0x800000 (the client room free) on a unit
        // other than the local player runs nothing (no per-type update,
        // no drain, §5 rule 2); with flag-ex 0x20 the client sends C→S
        // 0x4B [type u32][GUID u32] and clears both bits (a server unit
        // stays until the server answers).
        if let Some(unit) = world.units.get_mut(&key) {
            if unit.room_freed && !local {
                if unit.flag_ex & 0x20 != 0 {
                    unit.room_freed = false;
                    unit.flag_ex &= !0x20;
                    let mut m = vec![0x4B];
                    m.extend_from_slice(&u32::from(key.unit_type).to_le_bytes());
                    m.extend_from_slice(&key.guid.to_le_bytes());
                    world.outgoing.push(m);
                }
                continue;
            }
        }
        if key.unit_type == OBJECT && world.units.contains_key(&key) {
            // The object update `0x004BDFF0` (§5 rule 2).
            let unit = ObjUnit {
                key,
                client_only: false,
            };
            let start = outputs.len();
            if let Err(error) = objects::object_update(world, inputs, unit, outputs) {
                log.rejected.push(Rejected { id: 0, error });
            }
            apply_object_lights(world, &outputs[start..]);
            move_freed(world, outputs);
        }
        // The monster update `0x004B13A0`'s anim step (`model.md` §19
        // r8.5), before the drain.
        if key.unit_type == MONSTER {
            super::monster_anim::step(world, key);
        }
        // The player update `0x00463390`'s mode end (an attack, cast or
        // hit animation over → neutral), before the drain
        // (`skills/sequences.md` client mode machine; REC-1000).
        if key.unit_type == super::world::PLAYER {
            if let Err(error) = super::player_anim::step(world, inputs, key) {
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
            move_freed(world, outputs);
        }
    }
    if !c_objects_done {
        c_missiles(world, inputs, log, outputs);
        c_objects(world, inputs, log, outputs);
    }
    // §5 r3: the C monsters walk last; each runs the critter AI
    // `0x0046D780` (r6.4). Their per-unit monster update (the anim step)
    // is not run on set C.
    super::critters::c_monsters(world, inputs);
    if let Err(error) = drlg_update(world) {
        // Not a message: recorded under the client update's own id 0.
        // A DRLG error is a fatal error of the original's code.
        log.rejected.push(Rejected { id: 0, error });
    }
    // `render/lighting.md` §9.2 r1, §10 r5: once per client update.
    if let Err(error) = lighting_update(world, inputs) {
        log.rejected.push(Rejected { id: 0, error });
    }
    // The Den lights' creates (`render/lighting.md` §10 r5).
    missile_sounds(world, outputs);
    move_freed(world, outputs);
    applied
}

/// The C objects' walk (`0x00463CC0`, §5 rule 3): each runs the object
/// update (call site A), then, still in set C, the dispatch once more
/// (call site B). Errors are recorded under the update's id 0.
/// The C missiles' walk (§5 rule 3, before the C objects): each runs the
/// client missile dispatch `0x004D2C70` (`missiles/client.md` §C6).
fn c_missiles(
    world: &mut ClientWorld,
    inputs: &ModelInputs,
    log: &mut ReceiveLog,
    outputs: &mut Vec<Output>,
) {
    // `missiles/client.md` §C9 r4.1: the client's state-86 lists count
    // down first (PROVISIONAL REC-545).
    super::client_missiles::tick_just_hit(world);
    // PROVISIONAL REC-546: the unit footprints the missiles read.
    super::client_missiles::stamp_unit_footprints(world, &inputs.tables.monsters);
    for key in objects::c_order(world, super::world::MISSILE) {
        let env = super::client_missiles::Env {
            rows: &inputs.tables.missiles,
            lights: inputs.high_light_quality,
            skills: inputs.skill_tables.as_deref(),
            monsters: &inputs.tables.monsters,
        };
        let r = super::client_missiles::update_with(world, &env, key);
        if let Err(error) = r {
            log.rejected.push(Rejected { id: 0, error });
        }
    }
    missile_sounds(world, outputs);
    move_freed(world, outputs);
}

/// The client missiles' sound calls (`audio/triggers.md` §8 r3) in
/// update order, before the frees of the same walk.
fn missile_sounds(world: &mut ClientWorld, outputs: &mut Vec<Output>) {
    outputs.extend(
        world
            .objclient
            .missile_sounds
            .drain(..)
            .map(Output::MissileSound),
    );
}

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
        let start = outputs.len();
        let r = objects::object_update(world, inputs, unit, outputs)
            .and_then(|()| objects::site_b(world, inputs, key, outputs));
        if let Err(error) = r {
            log.rejected.push(Rejected { id: 0, error });
        }
        apply_object_lights(world, &outputs[start..]);
        move_freed(world, outputs);
    }
}

/// The object lights the client object code asked for
/// (`ObjFx::Light`, `0x004BC580`; `render/lighting.md` §8 object row):
/// the light list is model state, so the update pass applies them at
/// once, in call order; the output stays in the list for the effects
/// layer.
fn apply_object_lights(world: &mut ClientWorld, new: &[Output]) {
    for o in new {
        if let Output::ObjectFx(ObjFx::Light { unit, lit, rgb }) = *o {
            object_light_of(world, unit.key, unit.client_only, lit, rgb);
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
