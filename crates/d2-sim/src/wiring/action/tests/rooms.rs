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
