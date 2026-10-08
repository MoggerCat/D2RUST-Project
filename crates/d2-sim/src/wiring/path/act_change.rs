// Spec: specs/world/waypoints.md §11 (act change `0x0053ACC0`) and open question 1 (message order of a cross-act travel); specs/sim/path-placement.md §6 r4, §7.1, §11
//! The act change `0x0053ACC0(game, client, level, tile index)` of a
//! level warp (`0x0053AEC0` → `0x00537340` + `0x0053ACC0`), for a player
//! whose destination level is in another act (`waypoints.md` §11 steps
//! 1–19).
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
//! of step 6 and the follow of step 19) is the [`HirelingCall::ActChange`]
//! the warp queued (`super::place::level_warp`); it runs after this
//! function (TODO(hirelings-2.md §19), noted at the host).
//!
//! [`HirelingCall::ActChange`]: crate::wiring::action::HirelingCall::ActChange

use crate::drlg::act_of_level;
use crate::path::coords::Point;
use crate::path::place_seams::mask;
use crate::units::lists::client_state;
use crate::units::UnitId;
use crate::wiring::action::{Pending, WiringError};

use super::place::{log, with_shared, Rooms};
use super::walk::PathCtx;

/// S→C 0x05 UnloadComplete (`0x0053B320(client, 5)`).
const UNLOAD_COMPLETE: u8 = 0x05;

/// Step 9's step of the free point `0x0064E7E0` (`waypoints.md` §11).
const FREE_STEP: i32 = 5;

/// Unit flag-ex (+0xC8) bit of step 18: S→C 0x15 at the update.
const RESYNC: u32 = 0x10000;

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

/// `0x0053ACC0` for `player` to `level` (in another act than the
/// player's client) with `tile_index` (`waypoints.md` §11): `true` when
/// the player was moved. A destination act without a DRLG, no spawn room
/// or no free point leaves the player where it is (steps 8–9: nothing is
/// sent; the client state stays 5 as in 1.14d).
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
    // Step 3: the act's DRLG is built by the host at game creation; a
    // missing one cannot be built here (no DRLG data path in the view).
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
    if let Some(a) = c.game.lists.act_mut(act) {
        if !a.built {
            a.built = true;
            a.environment = crate::world::environment::Environment::CREATED;
        }
    }
    // Step 4.
    if let Some(e) = client.and_then(|k| c.game.lists.client_mut(k)) {
        e.state = client_state::CHANGING_ACT;
    }
    // Steps 5–7: P is `player`; the classic pet drop is the queued
    // hireling call (module docs).
    // TODO(spec: combat/vitals.md §4.8 item 1.1): the disguise check
    // `0x00646020` (a shapeshifted player loses its disguise states) has
    // no d2rs implementation; the states stay.
    // Step 8: the spawn point `0x0061B060` with P's size.
    let size = c.v.path_size(player);
    let spawn = with_shared(PathCtx::of(&mut c.v, c.game), |cv, _, lv| {
        let r = crate::path::place::level_spawn_point(&*cv, lv, Some(act), level, tile_index, size);
        log(cv, r).flatten()
    });
    let Some((room, p)) = spawn else {
        return false;
    };
    // Step 9: the free point from R (mask 0x1C89, step 5).
    let mut q = Point::new(p.x, p.y);
    let found = crate::path::search::free_point_step(
        &Rooms(&c.v.h.drlg),
        Some(room),
        &mut q,
        size,
        mask::ACT_CHANGE,
        FREE_STEP,
    );
    let room = match found {
        Ok(Some(r)) => r,
        Ok(None) => return false,
        Err(e) => {
            c.v.h.errors.push(WiringError::Place(e));
            return false;
        }
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
    c.teleport_clear(player, Some(room), q.x, q.y);
    if c.v.h.errors.len() > errors {
        c.v.h.errors.push(WiringError::ActChange(FATAL_ENTER));
        return false;
    }
    // Step 12: the client's room switch to none: the old act's removals
    // (P is in no room of the old act any more).
    if let Some(k) = client {
        c.v.room_switch(c.game, k, None);
    }
    // Step 13.
    c.v.h.x.send(player, &[UNLOAD_COMPLETE]);
    // Step 14: the client's act follows its room (step 17). Step 15: the
    // unit record's act (+0x18; the act record +0x1C is the act's DRLG).
    if let Some(r) = c.v.units.get_mut(player) {
        r.act = act;
    }
    // Step 16: 0x03 (`0x0053ABE0`: game +0x7C, the act's town, game
    // +0x80), then 0x53 (the act's environment record).
    let obj_seed = c.v.h.objects.as_ref().map_or(0, |o| o.obj_seed);
    let town = crate::drlg::TOWN_LEVELS
        .get(usize::from(act))
        .copied()
        .unwrap_or_default();
    c.v.h
        .x
        .send(player, &load_act(act, init_seed, town as u16, obj_seed));
    if let Some(env) = c.game.lists.act(act).map(|a| a.environment.message()) {
        c.v.h.x.send(player, &env);
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
        r.flags2 |= RESYNC;
    }
    c.room_change_messages(player);
    queue(&mut c, player);
    // Step 19: the pet follow is the queued hireling call (module docs).
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
