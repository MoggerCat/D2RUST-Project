// Spec: specs/world/waypoints.md §7 rule 5 and open question 1 (message order of a cross-act travel); specs/sim/path-placement.md §11
//! The act change of a level warp (`0x0053AEC0` → `0x00537340` +
//! `0x0053ACC0`, D2MOO `LEVEL_ChangeAct`), for a player whose destination
//! level is in another act.
//!
//! The messages follow the recording of `waypoints.md` open question 1:
//! the old act's removals (S→C 0x0A / 0x08, the client's room switch to
//! no room), 0x05 UnloadComplete, 0x03 LoadAct (the new act's init seed,
//! its town, the object seed), then the placement in the new act (the
//! room switch's 0x07 joins and the unit adds, 0x15 / 0x0D).
//!
//! PROVISIONAL (d2rs-own, unverified; REC in `docs/HANDOFF.md` §7): the
//! S→C 0x53 (darkness) and 0x5D between 0x03 and the room adds are not
//! sent, the quest-change argument (`arg` 0 / 5 of `NpcWorld::act_change`)
//! is read as the spawn tile index 0, and `0x0053ACC0`'s other steps
//! (owner spec missing, `impl-world-rest.md` G8) are not run.

use crate::drlg::act_of_level;
use crate::units::UnitId;
use crate::wiring::action::Pending;

use super::place::{log, with_shared};
use super::walk::PathCtx;

/// S→C 0x05 UnloadComplete.
const UNLOAD_COMPLETE: u8 = 0x05;

/// S→C 0x04 LoadComplete.
const LOAD_COMPLETE: u8 = 0x04;

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

/// Moves `player` to `level` (another act than its room's) with
/// `tile_index`: `true` when the player was placed. A destination act
/// without a DRLG, or no spawn room, leaves the player where it is
/// (nothing is sent in the first case; the second is logged by the
/// placement, after the act messages).
pub fn run<X: Pending>(mut c: PathCtx<'_, X>, player: UnitId, level: u32, tile_index: u32) -> bool {
    let act = act_of_level(level);
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
    // The old act's removals: the client leaves every room it had.
    let client = c
        .game
        .lists
        .clients()
        .into_iter()
        .find(|&k| c.game.lists.client(k).and_then(|e| e.player) == Some(player));
    if let Some(client) = client {
        c.v.room_switch(c.game, client, None);
    }
    c.v.h.x.send(player, &[UNLOAD_COMPLETE]);
    c.v.h
        .x
        .send(player, &load_act(act, init_seed, town as u16, obj_seed));
    let placed = with_shared(PathCtx::of(&mut c.v, c.game), |cv, host, lv| {
        let r = crate::path::place::level_warp_place(cv, host, lv, player, act, level, tile_index);
        log(cv, r).unwrap_or(false)
    });
    // The old act's removals included the player's own unit (it was
    // still in the old room): the new act re-adds it, at the placed point,
    // as the game entry does (`intents-events.md` §8 rule 3.1).
    // d2rs-own, unverified.
    if !placed {
        return false;
    }
    let (px, py) = c.v.h.path_position(player);
    if let Some(r) = c.v.units.get(player) {
        let (guid, class) = (r.guid, r.class);
        let name =
            c.v.h
                .session
                .names
                .get(&player)
                .copied()
                .unwrap_or_default();
        let m = crate::wiring::action::switch::assign_player(
            guid,
            class as u8,
            &name,
            px as u16,
            py as u16,
        );
        c.v.h.x.send(player, &m);
        c.v.player_part_b(c.game, player, player);
        // The handshake 0x0B makes it the local player again.
        let g = guid.to_le_bytes();
        c.v.h.x.send(player, &[0x0B, 0, g[0], g[1], g[2], g[3]]);
    }
    // LoadComplete: the client is in the game again (needs the placed
    // local player). d2rs-own, unverified: the recording shows none, the
    // client's `in_game` needs it.
    c.v.h.x.send(player, &[LOAD_COMPLETE]);
    true
}
