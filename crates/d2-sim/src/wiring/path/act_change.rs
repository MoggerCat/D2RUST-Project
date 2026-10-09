// Spec: specs/world/waypoints.md §11 (act change `0x0053ACC0`) and open question 1 (message order of a cross-act travel); specs/flows/act-change.md §1; specs/sim/path-placement.md §6 r4, §7, §7.1, §11
//! The act change `0x0053ACC0(game, client, level, tile index)` (D2MOO
//! `LEVEL_ChangeAct`) of a level warp (`0x0053AEC0` → `0x00537340` +
//! `0x0053ACC0`) for a player whose destination level is in another act,
//! in the order of `waypoints.md` §11 steps 1–19: client state 5, spawn
//! and free point, leave the old room, enter the new one, the client's
//! room switch to none (the old act's removals), S→C 0x05, the unit's
//! act, S→C 0x03 then 0x53, the room switch to the new room (0x07 and its
//! units), the player's update (0x15) and room-change messages, the pets
//! follow. The player stays the client's local player: it is not
//! re-added.
//!
//! The messages follow the recording of `waypoints.md` open question 1:
//! the player's removal for the other clients of its old room, the old
//! act's removals (S→C 0x0A / 0x08, the client's room switch to no room;
//! never the player's own unit, which left its room first), 0x05
//! UnloadComplete, 0x03 LoadAct (the new act's init seed, its town, the
//! object seed) and 0x53, then the room switch's 0x07 joins and the unit
//! adds. The player's own placement reaches its client as S→C 0x15 from
//! the next per-client update (unit flag-ex 0x10000, `sim/intents-events.md`
//! §7 rule 1), and S→C 0x04 follows when the client's room is ready
//! (client state 5, `sim/tick.md` §6 rule 4).
//!
//! The hireling part (`hirelings.md` §6 rules 3–4: the classic pet drop
//! of step 6) is the [`HirelingCall::ActChange`] the warp queued
//! (`super::place::level_warp`); it runs after this function
//! (TODO(hirelings-2.md §19), noted at the host).
//!
//! Not run here (TODO(waypoints.md §11)): step 1's arena flag test (the
//! single-player arena flags never have bit value 2) and step 7's
//! disguise check (owner: `vitals.md` §4.8).
//!
//! [`HirelingCall::ActChange`]: crate::wiring::action::HirelingCall::ActChange

use crate::drlg::act_of_level;
use crate::path::place::level_spawn_point;
use crate::path::place_seams::{mask, CollisionView};
use crate::path::search::free_point_step;
use crate::units::lists::client_state;
use crate::units::UnitId;
use crate::wiring::action::{Pending, WiringError};

use super::place::{log, with_shared};
use super::walk::PathCtx;

/// Step 9's search step (`0x0064E7E0(R, &(x, y), size, 0x1C89, 5)`).
const ACT_CHANGE_STEP: i32 = 5;
/// Flag-ex 0x10000: S→C 0x15 at the unit's update (step 18).
const REASSIGN_EX: u32 = 0x1_0000;

/// S→C 0x05 UnloadComplete (`0x0053B320(client, 5)`).
const UNLOAD_COMPLETE: u8 = 0x05;

/// Fatal asserts of `0x0053ACC0` (`waypoints.md` §11 steps 2, 10, 11).
const FATAL_SAME_ACT: u32 = 0x19F;
const FATAL_LEAVE: u32 = 0x1D1;
const FATAL_ENTER: u32 = 0x1D6;

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

/// Step 3: act `act`'s DRLG, created by `0x0053AC70` when missing
/// (`drlg/levels.md` §2) on the game's init seed (game +0x7C) and
/// difficulty (+0x6D), read from an act already built (its copies,
/// `levels.md` §3 step 1). `false`: no act is built, or the creation
/// failed (logged).
// TODO(levels.md §2 rule 2): an arena game passes its arena level as the
// town; d2rs keeps no arena flag, so the act's own town is used.
fn build_act<X: Pending>(c: &mut PathCtx<'_, X>, act: u8) -> bool {
    let drlg = &mut c.v.h.drlg;
    if drlg
        .dungeon
        .acts
        .get(usize::from(act))
        .is_some_and(Option::is_some)
    {
        return true;
    }
    let Some((init_seed, difficulty)) = drlg
        .dungeon
        .acts
        .iter()
        .flatten()
        .next()
        .map(|d| (d.init_seed, d.difficulty))
    else {
        return false;
    };
    let data = drlg.data.clone();
    let r =
        drlg.dungeon
            .get_or_create(act, init_seed, difficulty, None, &data, drlg.types.as_mut());
    match r {
        Ok(_) => true,
        Err(e) => {
            c.v.h
                .errors
                .push(crate::wiring::action::WiringError::Drlg(e));
            false
        }
    }
}

/// `0x0053ACC0` for `player` to `level` (in another act than the
/// player's client) with `tile_index` (`waypoints.md` §11): `true` when
/// the player was moved. The client goes to state 5 (step 4); 0x04 is not
/// sent here: the next tick's client pass sends it when the new room is
/// ready (`flows/act-change.md` §1 r4, `sim/tick.md` §6 rule 4). No spawn
/// room or no free point leaves the player where it is (steps 8–9:
/// nothing is sent; the client state stays 5 as in 1.14d).
pub fn run<X: Pending>(mut c: PathCtx<'_, X>, player: UnitId, level: u32, tile_index: u32) -> bool {
    // Step 1: d2rs has no arena record (`0x0053FCE0`); its flag bit 1 is
    // clear in every game d2rs runs.
    let act = act_of_level(level);
    let client = c
        .game
        .lists
        .clients()
        .into_iter()
        .find(|&k| c.game.lists.client(k).and_then(|e| e.player) == Some(player));
    // Step 2: the client's act (client +0x1AC) is the act of its room.
    let client_act = client
        .and_then(|k| c.game.lists.client(k))
        .and_then(|e| e.room)
        .and_then(|r| c.game.lists.room(r))
        .map(|r| r.act);
    if client_act == Some(act) {
        c.v.h.errors.push(WiringError::ActChange(FATAL_SAME_ACT));
        return false;
    }
    // Step 3: act A not built → build it (`0x0053AC70`, `levels.md` §2:
    // the game's init seed and difficulty, which every built act holds a
    // copy of). A game with no act built, or a failed build: nothing
    // changes, nothing is sent.
    if !build_act(&mut c, act) {
        return false;
    }
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
    // Step 4.
    if let Some(e) = client.and_then(|k| c.game.lists.client_mut(k)) {
        e.state = client_state::CHANGING_ACT;
    }
    // Steps 5–7: P is `player`; the classic pet drop is the queued
    // hireling call (module docs).
    // TODO(spec: combat/vitals.md §4.8 item 1.1): the disguise check
    // `0x00646020` (a shapeshifted player loses its disguise states) has
    // no d2rs implementation; the states stay.
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
            mask::ACT_CHANGE,
            ACT_CHANGE_STEP,
        );
        log(cv, r)?.map(|room| (room, p))
    });
    let Some((room, p)) = spawn else {
        return false;
    };
    // Step 10: P leaves its room O (teleport to (none, 0, 0): footprint
    // cleared, off O's list, previous room := O), then the room-change
    // messages with O as the old room (the other clients of O get P's
    // removal).
    let old = c.game.lists.unit(player).and_then(|e| e.room());
    let errors = c.v.h.errors.len();
    c.teleport_clear(player, None, 0, 0);
    if c.v.h.errors.len() > errors {
        c.v.h.errors.push(WiringError::ActChange(FATAL_LEAVE));
        return false;
    }
    // `0x00554670(game, P, O)`: O is the old room of the messages (the
    // teleport's reset recaches again and leaves the previous room none).
    if let Some(d) = c.v.h.paths.as_mut().and_then(|p| p.dynamic_mut(player)) {
        d.prev_room = old;
    }
    c.room_change_messages(player);
    // Step 11: P enters R at the step-9 point (R's list, queued).
    c.teleport_clear(player, Some(room), p.x, p.y);
    if c.v.h.errors.len() > errors {
        c.v.h.errors.push(WiringError::ActChange(FATAL_ENTER));
        return false;
    }
    queue(&mut c, player);
    // Step 12: the client's room switch to none: the old act's removals
    // (P is in no room of the old act any more).
    if let Some(k) = client {
        c.v.room_switch(c.game, k, None);
    }
    // Step 13.
    c.v.h.x.send(player, &[UNLOAD_COMPLETE]);
    // Step 14: d2rs keeps no client act of its own: the client's act is
    // its room's, which step 17 sets. Step 15: the unit record's act
    // (+0x18; the act record +0x1C is the act's DRLG).
    if let Some(r) = c.v.units.get_mut(player) {
        r.act = act;
    }
    // Step 16: 0x03 (`0x0053ABE0`: game +0x7C, the act's town, game
    // +0x80), then 0x53 (the act's environment record, created at the
    // act's first use, `render/lighting.md` §9.1).
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
    // Step 17: the room switch to R: the new act's 0x07 and unit adds.
    if let Some(k) = client {
        c.v.room_switch(c.game, k, Some(room));
    }
    // Step 18: queued, flag-ex 0x10000 (S→C 0x15 at the update), the
    // room-change messages (the other clients of R get P's adds), queued
    // again.
    queue(&mut c, player);
    if let Some(r) = c.v.units.get_mut(player) {
        r.flags2 |= REASSIGN_EX;
    }
    c.room_change_messages(player);
    queue(&mut c, player);
    // Step 19: pets follow (`hirelings.md` §6 rule 1), the host's.
    if let Some(q) = c.v.h.pet_follows.as_mut() {
        q.push(player);
    }
    true
}

/// `0x0064C040`: queue the unit for update.
fn queue<X: Pending>(c: &mut PathCtx<'_, X>, unit: UnitId) {
    if let Err(e) = c.game.lists.queue_update(unit) {
        c.v.h
            .errors
            .push(WiringError::Unit(crate::units::modes::UnitError::Game(
                e.into(),
            )));
    }
}
