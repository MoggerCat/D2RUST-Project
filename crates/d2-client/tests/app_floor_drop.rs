// Spec: specs/items/treasure.md (§7 step 2); specs/sim/path-placement.md (§7.3, §9)
//! The play game's drop spot (REC-108): the path provider carries the real
//! `ExpField.D2` walk-back field, so a monster's or chest's drop searches
//! its floor spot (`0x00555DA0`) instead of taking the start spot as is.
//! 1.14d, Cold Plains (`facts/items/a1-cold-plains-poke-kills.tsv`): a
//! fallen dead at (5170, 4660) drops at (5172, 4663) = (x + 2, y + 3), the
//! next one, dead at (5171, 4659) with its start cell taken, at
//! (5172, 4662).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::single_player::{self, DEFAULT_SEED};
use d2_client::bridge::Bridge;
use d2_sim::path::coords::Point;
use d2_sim::wiring::path::place::floor_drop;

mod app_support;

struct StepClock(Arc<AtomicU32>);

impl d2_server::seams::Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

// Covers: specs/items/treasure.md §7 r2; specs/sim/path-placement.md §7.3 r1, §9 r1
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_play_game_drops_on_the_floor_drop_spot_of_the_real_field() {
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, _) = single_player::start(
        app_support::game_data(),
        DEFAULT_SEED,
        StepClock(ms.clone()),
    )
    .unwrap();
    let server = Arc::new(std::sync::Mutex::new(link));
    let mut bridge = Bridge::new(app_support::SharedLink(server.clone())).unwrap();
    let frame = |b: &mut Bridge<_>| {
        ms.fetch_add(40, Ordering::SeqCst);
        b.frame().unwrap()
    };
    bridge
        .send_bytes(&single_player::create_request().encode())
        .unwrap();
    frame(&mut bridge);
    frame(&mut bridge);
    bridge.send_bytes(&[0x6B]).unwrap();
    for _ in 0..3 {
        frame(&mut bridge);
    }
    let (player, _) = app_support::local_player(&server).expect("joined");

    let r = app_support::with(&server, move |l| {
        let sim = &mut l.host_mut().game;
        let game = &sim.game;
        let room = game.lists.unit(player).and_then(|e| e.room());
        let h = sim.events.action.hooks();
        let field = h
            .paths
            .as_ref()
            .and_then(|p| p.field.clone())
            .expect("the field is on the path provider");
        let at = h.path_position(player);
        let start = Point::new(at.0 + 2, at.1 + 3);
        let free = room
            .and_then(|r| h.drlg.collision(game, r, start.x, start.y))
            .is_some_and(|c| c & 0x3E01 == 0);
        let out = floor_drop(&h.drlg, &field, room, Point::new(at.0, at.1), 1, true).unwrap();
        (field.width, field.height, at, free, out.1)
    });
    let (w, h, at, free, out) = r;
    assert_eq!((w, h), (256, 256));
    assert!(free, "the town cell {:?} + (2, 3) is open", at);
    assert_eq!(out, Point::new(at.0 + 2, at.1 + 3));
}
