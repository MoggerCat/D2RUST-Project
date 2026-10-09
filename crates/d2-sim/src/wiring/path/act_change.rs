// Spec: specs/world/waypoints.md §11; specs/flows/act-change.md §1; specs/sim/path-placement.md §7, §11
//! The act change `0x0053ACC0` (D2MOO `LEVEL_ChangeAct`) of a level warp
//! (`0x0053AEC0` → `0x00537340` + `0x0053ACC0`) for a player whose
//! destination level is in another act, in the order of
//! `waypoints.md` §11 steps 1–19: client state 5, spawn and free point,
//! leave the old room, enter the new one, the client's room switch to
//! none (the old act's removals), S→C 0x05, the unit's act, S→C 0x03
//! then 0x53, the room switch to the new room (0x07 and its units), the
//! player's update (0x15) and room-change messages, the pets follow.
//! S→C 0x04 is the next tick's client pass's, when the new room is ready
//! (`sim/tick.md` §6 rule 4). The player stays the client's local player:
//! it is not re-added.
//!
//! Not run here (TODO(waypoints.md §11)): step 1's arena flag test (the
//! single-player arena flags never have bit value 2), step 6's classic
//! pet drop and step 7's disguise check (owners: `hirelings.md` §6 r4,
//! `vitals.md` §4.8), step 10's removal to the old room's other clients
//! (none in single player).

use crate::drlg::act_of_level;
use crate::path::place::level_spawn_point;
use crate::path::place_seams::CollisionView;
use crate::path::search::free_point_step;
use crate::units::lists::client_state;
use crate::units::UnitId;
use crate::wiring::action::Pending;

use super::place::{log, with_shared};
use super::walk::PathCtx;

/// Step 9's free-point mask (`0x0064E7E0(R, &(x, y), size, 0x1C89, 5)`).
const ACT_CHANGE_MASK: u32 = 0x1C89;
/// Step 9's search step.
const ACT_CHANGE_STEP: i32 = 5;
/// Flag-ex 0x10000: S→C 0x15 at the unit's update (step 18).
const REASSIGN_EX: u32 = 0x1_0000;

/// S→C 0x05 UnloadComplete.
const UNLOAD_COMPLETE: u8 = 0x05;

/// S→C 0x03 LoadAct (`sim/server-messages.tsv`: act u8 @1, init seed
/// u32 @2, town level u16 @6, object seed u32 @8).
pub fn load_act(act: u8, init_seed: u32, town: u16, obj_seed: u32) -> [u8; 12] {
    let mut m = [0u8; 12];
    m[0] = 0x03;
    m[1] = act;
    m[2..6].copy_from_slice(&init_seed.to_le_bytes());
    m[6..8].copy_from_slice(&town.to_le_bytes());
    m[8..12].copy_from_slice(&obj_seed.to_le_bytes());
    m
}

/// Takes `player` out of its room: footprint cleared, off the room list,
/// the path's room none.
fn leave_room<X: Pending>(c: &mut PathCtx<'_, X>, player: UnitId) {
    c.v.path_remove_footprint(player, true);
    if c.game.lists.room_remove(player).is_err() {
        return;
    }
    if let Some(d) = c.v.h.paths.as_mut().and_then(|p| p.dynamic_mut(player)) {
        d.room = None;
    }
}

/// The act change `0x0053ACC0` (`waypoints.md` §11 steps 1–19) of
/// `player` to `level` with `tile_index`: `true` when the player was moved.
/// The client goes to state 5 (step 4); 0x04 is not sent here: the next
/// tick's client pass sends it when the new room is ready
/// (`flows/act-change.md` §1 r4, `sim/tick.md` §6 rule 4).
pub fn run<X: Pending>(mut c: PathCtx<'_, X>, player: UnitId, level: u32, tile_index: u32) -> bool {
    let act = act_of_level(level);
    // Step 2: the player's own act is a fatal assert (0x19F) in 1.14d;
    // its only caller never asks for it.
    let old_act = c
        .game
        .lists
        .unit(player)
        .and_then(|u| u.room())
        .and_then(|r| c.game.lists.room(r))
        .map(|r| r.act);
    if old_act == Some(act) {
        return false;
    }
    // Step 3: d2rs builds every act's DRLG with the dungeon; an act
    // without one cannot be entered (nothing changes, nothing is sent).
    let Some(init_seed) =
        c.v.h
            .drlg
            .dungeon
            .acts
            .get(usize::from(act))
            .and_then(Option::as_ref)
            .map(|d| d.init_seed)
    else {
        return false;
    };
    let obj_seed = c.v.h.objects.as_ref().map_or(0, |o| o.obj_seed);
    let town = crate::drlg::TOWN_LEVELS
        .get(usize::from(act))
        .copied()
        .unwrap_or_default();
    let client = c
        .game
        .lists
        .clients()
        .into_iter()
        .find(|&k| c.game.lists.client(k).and_then(|e| e.player) == Some(player));
    // Step 4.
    if let Some(e) = client.and_then(|k| c.game.lists.client_mut(k)) {
        e.state = client_state::CHANGING_ACT;
    }
    // Steps 8–9: the spawn search (`0x0061B060`), then the free point
    // from it (`0x0064E7E0`, mask 0x1C89, step 5). Failing, steps 3–7
    // stay done and nothing is sent (the client stays in state 5).
    let spawn = with_shared(PathCtx::of(&mut c.v, c.game), |cv, _, lv| {
        let size = cv.unit_size(player);
        let r = level_spawn_point(cv, lv, Some(act), level, tile_index, size);
        let (room, mut p) = log(cv, r)??;
        let r = free_point_step(
            cv,
            Some(room),
            &mut p,
            size,
            ACT_CHANGE_MASK,
            ACT_CHANGE_STEP,
        );
        log(cv, r)?.map(|room| (room, p))
    });
    let Some((room, p)) = spawn else {
        return false;
    };
    // Step 10: the old room left (footprint cleared, off its unit list,
    // path room none). Its room-change messages go to O's other clients
    // only: none in single player (TODO(waypoints.md §11 step 10):
    // multiplayer).
    leave_room(&mut c, player);
    // Step 11: into R at the step-9 point, queued for update.
    c.teleport(player, Some(room), p.x, p.y);
    let _ = c.game.lists.queue_update(player);
    // Step 12: the old act's removals.
    if let Some(k) = client {
        c.v.room_switch(c.game, k, None);
    }
    // Step 13.
    c.v.h.x.send(player, &[UNLOAD_COMPLETE]);
    // Step 14: d2rs keeps no client act of its own: the client's act is
    // its room's, which step 17 sets.
    // Step 15.
    if let Some(r) = c.v.units.get_mut(player) {
        r.act = act;
    }
    // Step 16: 0x03 then 0x53 (the act's environment record, created at
    // the act's first use, `render/lighting.md` §9.1).
    c.v.h
        .x
        .send(player, &load_act(act, init_seed, town as u16, obj_seed));
    let env = c.game.lists.act_mut(act).map(|r| {
        if !r.built {
            r.built = true;
            r.environment = crate::world::environment::Environment::CREATED;
        }
        r.environment
    });
    if let Some(env) = env {
        c.v.h.x.send(player, &env.message());
    }
    // Step 17: the new act's rooms and their units.
    if let Some(k) = client {
        c.v.room_switch(c.game, k, Some(room));
    }
    // Step 18: S→C 0x15 at the update (flag-ex 0x10000).
    let _ = c.game.lists.queue_update(player);
    if let Some(r) = c.v.units.get_mut(player) {
        r.flags2 |= REASSIGN_EX;
    }
    c.room_change_messages(player);
    let _ = c.game.lists.queue_update(player);
    // Step 19: pets follow (`hirelings.md` §6 rule 1), the host's.
    if let Some(q) = c.v.h.pet_follows.as_mut() {
        q.push(player);
    }
    true
}
