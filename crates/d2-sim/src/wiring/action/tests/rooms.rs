// Spec: specs/drlg/rooms.md §5–§8 (Test vectors), specs/drlg/levels.md §9.1, specs/sim/tick.md §3 steps 9–10, §6.5; specs/sim/unit-order.md §4
//! DRLG room activation ↔ the act room lists: streamed rooms live in
//! [`crate::units::UnitLists`]; tick step 9 counts and removes inactive
//! rooms through the DRLG; a client's room change keeps rooms active.

use super::*;
use crate::units::lists::client_state;

fn act_rooms(fx: &Fx) -> Vec<RoomId> {
    let mut v = Vec::new();
    let mut cur = fx.game.lists.room_first(0);
    while let Some(r) = cur {
        v.push(r);
        cur = fx.game.lists.room_next(r);
    }
    v
}

#[test]
fn streamed_rooms_are_active_in_the_act_room_list() {
    let mut fx = Fx::new();
    let (a, b) = (fx.a, fx.b);
    // `unit-order.md` §4: prepended at activation (B streamed last).
    assert_eq!(act_rooms(&fx), [b, a]);
    for r in [a, b] {
        let e = fx.game.lists.room(r).unwrap();
        // `rooms.md` §6: each adjacency array holds the room itself and
        // its active neighbour.
        let mut adj = e.adjacent.clone();
        adj.sort();
        assert_eq!(adj, [a, b]);
    }
    let game = &fx.game;
    let d = &fx.sim.sys.hooks.drlg;
    assert_eq!(d.level_id(game, a), Some(LEVEL));
    assert_eq!(d.subtiles(game, b), Some(TileRect::new(40, 0, 40, 40)));
    assert_eq!(d.find_room(game, a, 45, 3), Some(b));
    assert_eq!(d.find_room(game, a, 85, 3), None);
    assert!(!d.in_town(game, a));
    let _ = fx.sim.hooks();
    fx.assert_clean();
}

#[test]
fn inactive_rooms_are_removed_by_tick_step_9() {
    // `rooms.md` §7.2: no clients → +1 per step-9 pass (every 12 frames);
    // removed when > 10, i.e. on the 11th pass (frame 132), through
    // `0x0061A910` (act list unlink, then the DRLG's rest, §8.2).
    let mut fx = Fx::new();
    let (a, b) = (fx.a, fx.b);
    while fx.game.frame < 131 {
        fx.tick();
    }
    assert_eq!(act_rooms(&fx).len(), 2);
    fx.tick();
    assert_eq!(fx.game.frame, 132);
    assert!(act_rooms(&fx).is_empty());
    for r in [a, b] {
        assert!(fx.game.lists.room(r).is_none(), "record freed");
        let game = &fx.game;
        assert!(fx.sim.sys.hooks.drlg.drlg_room(game, r).is_none());
    }
    fx.assert_clean();
}

#[test]
fn client_room_change_keeps_its_rooms_active() {
    // `tick.md` §6.5: the player's room differs from the client's → the
    // room switch `0x00537B50` (`rooms.md` §4.1): the client joins every
    // room of A's adjacency array; their counters stay 0.
    let mut fx = Fx::new();
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 10, 10);
    let c = fx
        .game
        .lists
        .add_client(Some(p), None, client_state::IN_GAME);
    fx.tick();
    assert_eq!(fx.game.lists.client(c).unwrap().room, Some(a));
    while fx.game.frame < 200 {
        fx.tick();
    }
    assert_eq!(act_rooms(&fx).len(), 2);
    let game = &fx.game;
    let (d, r) = fx.sim.sys.hooks.drlg.drlg_room(game, a).unwrap();
    let ar = d.active_room(r).unwrap();
    assert_eq!(ar.clients, [c]);
    assert_eq!(ar.inactivity, 0);
    fx.assert_clean();
}

/// The S→C 0x07 the room switch sends for DRLG room `r`.
fn reveal_of(d: &crate::drlg::Drlg, r: crate::drlg::DrlgRoomId) -> Vec<u8> {
    let room = d.room(r);
    crate::wiring::path::place::map_reveal(
        room.rect.x as u16,
        room.rect.y as u16,
        d.level(room.level).id as u8,
    )
    .to_vec()
}

// Covers: specs/sim/path-placement.md §11 text; specs/drlg/rooms.md §4.1
#[test]
fn room_switch_reveals_each_joined_room_in_adjacency_order() {
    // `0x00537B50` → `0x0053A8E0` for each room of the new adjacency
    // array missing from the old: S→C 0x07 (tile x, tile y, level id) to
    // the client's player. A, B, C in a row: A's array is {A, B}, C's is
    // {B, C}; the first switch (no old room) reveals A's whole array, the
    // switch A → C only C.
    let mut fx = Fx::with_rooms(&[
        (LEVEL, TileRect::new(0, 0, 8, 8)),
        (LEVEL, TileRect::new(8, 0, 8, 8)),
        (LEVEL, TileRect::new(16, 0, 8, 8)),
    ]);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 10, 10);
    fx.game
        .lists
        .add_client(Some(p), None, client_state::IN_GAME);
    fx.sim.sys.hooks.x.sent.clear();
    fx.tick();
    let reveals = |fx: &Fx| -> Vec<Vec<u8>> {
        fx.sim
            .sys
            .hooks
            .x
            .sent
            .iter()
            .filter(|(u, m)| *u == p && m[0] == 0x07)
            .map(|(_, m)| m.clone())
            .collect()
    };
    let game = &fx.game;
    let (d, ra) = fx.sim.sys.hooks.drlg.drlg_room(game, a).unwrap();
    let want: Vec<Vec<u8>> = d
        .active_room(ra)
        .unwrap()
        .adjacency
        .iter()
        .map(|&r| reveal_of(d, r))
        .collect();
    assert_eq!(want.len(), 2);
    assert_eq!(reveals(&fx), want);
    // The third room is active now (B's array streamed it, `rooms.md`
    // §4.1 status 2); move the player there.
    let c = act_rooms(&fx)
        .into_iter()
        .find(|&r| {
            fx.sim.sys.hooks.drlg.subtiles(&fx.game, r) == Some(TileRect::new(80, 0, 40, 40))
        })
        .expect("room C active");
    fx.game.lists.change_room(p, c).unwrap();
    fx.sim.sys.hooks.x.sent.clear();
    fx.tick();
    let game = &fx.game;
    let (d, rc) = fx.sim.sys.hooks.drlg.drlg_room(game, c).unwrap();
    assert_eq!(reveals(&fx), vec![reveal_of(d, rc)]);
    assert_eq!(reveal_of(d, rc), [0x07, 16, 0, 0, 0, LEVEL as u8]);
    fx.assert_clean();
}

// Covers: specs/sim/intents-events.md §7.8 r2, §7.8 r3, §7.8 r5, §7.2; specs/sim/tick.md §6 r4, §6 r6
#[test]
fn room_switch_sends_add_and_leave_messages_and_the_join_completes() {
    // A, B, C in a row (A's array {A, B}, C's {B, C}). An object and a
    // warp tile stand in A. A joining client (state 3) whose player is in
    // A: tick 1's per-client update switches to A (0x07 A, the object's
    // 0x51, the tile's 0x09, 0x07 B), the rooms are populated by step 3,
    // so the room is ready and 0x04 follows, the client in game, then the
    // join sequence (§8.3: 0x5B, 0x65, the join 0x5A). Then the
    // player moves to C: 0x07 C, then A's leave: 0x0A for each unit of A,
    // 0x08 A.
    let mut fx = Fx::with_rooms(&[
        (LEVEL, TileRect::new(0, 0, 8, 8)),
        (LEVEL, TileRect::new(8, 0, 8, 8)),
        (LEVEL, TileRect::new(16, 0, 8, 8)),
    ]);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 10, 10);
    let o = fx.spawn(UnitType::Object, 7, a, 12, 14);
    let t = fx.spawn(UnitType::Tile, 3, a, 20, 21);
    let c = fx
        .game
        .lists
        .add_client(Some(p), None, client_state::JOINING);
    let mut name = [0u8; 16];
    name[..4].copy_from_slice(b"Jade");
    fx.sim.sys.hooks.session.names.insert(p, name);
    fx.sim.sys.hooks.x.sent.clear();
    fx.tick();
    let sent = |fx: &mut Fx| -> Vec<Vec<u8>> {
        let v = fx
            .sim
            .sys
            .hooks
            .x
            .sent
            .iter()
            .filter(|(u, _)| *u == p)
            .map(|(_, m)| m.clone())
            .collect();
        fx.sim.sys.hooks.x.sent.clear();
        v
    };
    let guid = |fx: &Fx, u: UnitId| fx.game.lists.unit(u).unwrap().guid;
    let (go, gt) = (guid(&fx, o), guid(&fx, t));
    let game = &fx.game;
    let (d, ra) = fx.sim.sys.hooks.drlg.drlg_room(game, a).unwrap();
    let adj = d.active_room(ra).unwrap().adjacency.clone();
    assert_eq!(adj.len(), 2);
    let (rev_a, rev_b) = (reveal_of(d, adj[0]), reveal_of(d, adj[1]));
    assert_eq!(adj[0], ra, "the room itself first in its array");
    let hide_a = {
        let mut m = rev_a.clone();
        m[0] = 0x08;
        m
    };
    let unit_list: Vec<UnitId> = fx.game.lists.room_units(a);
    let mut adds: Vec<Vec<u8>> = Vec::new();
    for &u in &unit_list {
        if u == o {
            adds.push(crate::units::messages::assign_object(go, 7, 12, 14, 1, 0).to_vec());
        } else if u == t {
            adds.push(crate::units::messages::assign_warp(5, gt, 3, 20, 21).to_vec());
        }
    }
    let mut want = vec![rev_a.clone()];
    want.extend(adds);
    want.push(rev_b);
    want.push(vec![0x04]);
    let gp = guid(&fx, p);
    let level = fx.sim.sys.stats.unit_total(p, 12, 0) as u16;
    use crate::units::messages::{player_event, player_joined, player_kill_count};
    want.push(player_joined(gp, 0, &name, level, 0xFFFF));
    want.push(player_kill_count(gp, 0).to_vec());
    want.push(player_event(2, &name).to_vec());
    assert_eq!(sent(&mut fx), want);
    assert_eq!(
        fx.game.lists.client(c).unwrap().state,
        client_state::IN_GAME
    );
    // A → C.
    let rc_id = act_rooms(&fx)
        .into_iter()
        .find(|&r| {
            fx.sim.sys.hooks.drlg.subtiles(&fx.game, r) == Some(TileRect::new(80, 0, 40, 40))
        })
        .expect("room C active");
    fx.game.lists.change_room(p, rc_id).unwrap();
    fx.tick();
    let game = &fx.game;
    let (d, rc) = fx.sim.sys.hooks.drlg.drlg_room(game, rc_id).unwrap();
    let mut want = vec![reveal_of(d, rc)];
    for u in fx.game.lists.room_units(a) {
        let e = fx.game.lists.unit(u).unwrap();
        want.push(crate::units::messages::remove_unit(e.ty as u8, e.guid).to_vec());
    }
    want.push(hide_a);
    assert_eq!(sent(&mut fx), want);
    assert_eq!(fx.game.lists.room_units(a).len(), 2);
    fx.assert_clean();
}
