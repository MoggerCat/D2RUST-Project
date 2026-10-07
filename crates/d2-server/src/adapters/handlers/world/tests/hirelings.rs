// Spec: specs/world/hirelings-2.md §15, §19 (tests)
//! The owner's death on the wired host: the player mode-17 start queues
//! the player (`ActionHooks::owner_deaths`) and the host runs
//! `0x00575BC0` on its hireling lists after the tick
//! (`d2_sim::world::hirelings::life::player_death`).

use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::UnitType;
use d2_sim::world::hirelings::{HirelingTables, PetNode};

use super::trade_quests::Fx;

// Covers: specs/world/hirelings-2.md §15 r1, §15 r3, §19
#[test]
fn a_queued_owner_death_kills_the_hireling_after_the_tick() {
    let mut fx = Fx::new(|_| {});
    let req = AllocRequest {
        ty: UnitType::Monster,
        class: 0,
        room: None,
        add: true,
        fixed_guid: None,
        mode: 1,
        allied: false,
    };
    let s = &mut fx.h.game;
    let merc = s
        .events
        .with(&mut s.game, |g, v| v.allocate(g, &req, 0, 0))
        .unwrap();
    let gm = fx.guid(merc);
    let p = fx.player;
    let w = fx.world();
    w.state.hireling_tables = Some(HirelingTables {
        rows: Default::default(),
        exp_ratios: Default::default(),
        max_level: 99,
        pet_flags: HirelingTables::WARP,
        pet_basemax: 1,
    });
    w.state.hirelings.list_mut(p).nodes = vec![PetNode {
        guid: gm,
        name: 0x0D68,
        ..PetNode::default()
    }];
    // The mode-17 start queues the owner (on from the first frame);
    // here staged.
    fx.h.connect(0);
    fx.h.clock.0 += 40;
    assert!(fx.h.frame().unwrap().ticked);
    let q = fx.h.game.events.hooks().owner_deaths.as_mut().unwrap();
    assert!(q.is_empty());
    q.push(p);
    fx.h.receive(0);
    fx.h.clock.0 += 40;
    assert!(fx.h.frame().unwrap().ticked);
    let node = fx.world().state.hirelings.list(p).unwrap().nodes[0];
    assert!(node.dead);
    // 0x7A remove (GUID only, @9), then 0x9B (name 0x0D68, cost at level
    // 0 = 0) to the owner; the unit stays.
    let mut remove = vec![0x7A, 0, 0, 0, 0, 0, 0, 0, 0];
    remove.extend_from_slice(&gm.to_le_bytes());
    let to_p: Vec<Vec<u8>> =
        fx.h.receive(0)
            .into_iter()
            .filter(|m| m[0] == 0x9B || m[0] == 0x7A)
            .collect();
    assert_eq!(to_p, vec![remove, vec![0x9B, 0x68, 0x0D, 0, 0, 0, 0]]);
    assert!(fx.h.game.events.sys.units.get(merc).is_some());
    assert_eq!(fx.h.game.events.hooks().owner_deaths, Some(vec![]));
}
