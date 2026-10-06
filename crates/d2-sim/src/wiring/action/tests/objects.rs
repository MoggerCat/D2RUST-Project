// Spec: specs/world/objects.md §2, §3, §4, §7, §14; specs/world/waypoints.md §5.2; specs/sim/units.md §6.4
//! Objects on the action wiring: the object state built at game
//! creation, objects created through the real unit allocation (init
//! dispatch on the control seed), the monster door operate, the object
//! timer events through the game's timer queue, the waypoint mode change
//! and the 0x13 object case on the object module. Only `Pending` (interact
//! range, footprints, routes) is faked.

use std::sync::Arc;

use d2_data::tables::{Levels, Objects};

use super::*;
use crate::monsters::ai::{AiModes, AiTargets};
use crate::units::record::flags as uflags;
use crate::wiring::action::{ObjectCase, ObjectRoute};
use crate::world::objects::{
    self as obj, oevent, oflags, Dispatch, ObjectTables, ObjectWorld, Operate, Route,
};

const CHEST: u32 = 0;
const TORCH: u32 = 1;
const DOOR: u32 = 2;
const QUEST_DOOR: u32 = 3;
const WAYPOINT: u32 = 4;
const LOCKED_OUT_DOOR: u32 = 5;

/// objects.txt rows 0–5; `levels` 150 blank rows with `MonLvl1` 1 for the
/// fixture's level.
fn tables() -> Arc<ObjectTables> {
    let row = |initfn: u8, operatefn: u8, monsterok: u8| {
        let mut o: Objects = blank();
        o.initfn = initfn;
        o.operatefn = operatefn;
        o.monsterok = monsterok;
        o
    };
    let mut chest = row(3, 4, 0);
    chest.lockable = 1;
    let mut wp = row(17, 23, 0);
    wp.framecnt1 = 10 << 8;
    wp.mode2 = 1;
    let mut levels = vec![blank::<Levels>(); 150];
    levels[LEVEL as usize].monlvl1 = 1;
    Arc::new(ObjectTables {
        objects: vec![
            chest,
            row(0, 11, 0),
            row(5, 8, 1),
            row(0, 9, 1),
            wp,
            row(5, 8, 0),
        ],
        shrines: Vec::new(),
        levels,
    })
}

/// The fixture with the object state created.
fn fx() -> Fx {
    let mut fx = Fx::new();
    fx.sim.create_objects(tables());
    fx
}

fn create(fx: &mut Fx, class: u32, x: i32) -> UnitId {
    let a = fx.a;
    fx.sim
        .with(&mut fx.game, |g, v| v.create_object(g, a, class, x, 20, 0))
        .expect("allocated")
}

fn guid(fx: &Fx, u: UnitId) -> u32 {
    fx.game.lists.unit(u).unwrap().guid
}

// Covers: specs/world/objects.md §2 r2, §3 r1, §3 r4, §3 r6, §3 r9, §5.2
#[test]
fn chest_created_through_allocation_gets_its_init() {
    // Game creation steps the game seed once for the control (§2 r2).
    let mut fx = Fx::new();
    let mut seed = fx.sim.hooks().game_seed;
    let lo = seed.step();
    fx.sim.create_objects(tables());
    assert_eq!(fx.sim.hooks().game_seed, seed);
    let st = fx.sim.hooks().objects.as_ref().unwrap();
    assert_eq!(st.control.seed, Seed::init_low(lo));
    // §5.2 test vector: C = {1, 666}, lockable, MonLvl1 1 → draws 51,
    // 31: InteractType 0, U := init_low(55249).
    fx.sim.hooks().objects.as_mut().unwrap().control.seed = Seed::init_low(1);
    let o = create(&mut fx, CHEST, 20);
    let g = guid(&fx, o);
    let st = fx.sim.hooks().objects.as_ref().unwrap();
    let d = st.control.data[&o];
    assert_eq!(d.class, CHEST as u16);
    assert_eq!(d.guid, g);
    assert_eq!(d.interact, 0);
    assert_eq!(d.owner, Some(-1));
    assert_eq!(st.control.seed, Seed::new(671_516_612, 330_169_957));
    assert_eq!(fx.sim.sys.units.get(o).unwrap().seed, Seed::init_low(55249));
    // The object's position is its allocation's (path, `Pending::place`).
    assert_eq!(fx.sim.hooks().x.position(o), (20, 20));
    fx.assert_clean();
}

// Covers: specs/world/objects.md §3 r6, §7.1 r3, §7.1 r4, §7.2 r4, §13
#[test]
fn torch_and_door_inits_and_the_0x13_case_reach_the_operate() {
    let mut fx = fx();
    let torch = create(&mut fx, TORCH, 20);
    let door = create(&mut fx, DOOR, 24);
    // Init 5 (door) runs here; init 0 (torch) is a null entry: no route.
    assert!(
        fx.sim.hooks().x.log.is_empty(),
        "{:?}",
        fx.sim.hooks().x.log
    );
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    let g = guid(&fx, torch);
    let r = fx.sim.operate_object_message(&mut fx.game, p, g);
    assert_eq!(r, Some(ObjectCase::Code(0)));
    assert_eq!(*fx.sim.hooks().x.ranged.borrow(), [(p, torch)]);
    // The dispatch runs operate 11 (torch, returns 1, §13).
    let r = fx.sim.objects(&mut fx.game, |ctl, t, w| {
        obj::dispatch(ctl, t, w, torch, Some(p))
    });
    assert_eq!(r, Some(Ok(Dispatch::Done(1))));
    // Missing object → 1; a door is reached the same way.
    let r = fx.sim.operate_object_message(&mut fx.game, p, 0xDEAD);
    assert_eq!(r, Some(ObjectCase::Code(1)));
    let g = guid(&fx, door);
    assert_eq!(
        fx.sim.operate_object_message(&mut fx.game, p, g),
        Some(ObjectCase::Code(0))
    );
    fx.assert_clean();
}

// Covers: specs/world/objects.md §7.1 r2, §7.1 r3, §7.1 r4, §7.2 r4
#[test]
fn monster_door_operate_routes_through_the_entry() {
    let mut fx = fx();
    let door = create(&mut fx, DOOR, 20);
    let shut = create(&mut fx, LOCKED_OUT_DOOR, 22);
    let quest = create(&mut fx, QUEST_DOOR, 24);
    let a = fx.a;
    let m = fx.spawn(UnitType::Monster, 0, a, 26, 20);
    fx.sim.with(&mut fx.game, |g, v| {
        // `MonsterOK` comes from objects.txt.
        assert!(AiTargets::door_monster_ok(v, door));
        assert!(!AiTargets::door_monster_ok(v, shut));
        AiModes::operate_door(v, g, m, door);
        // MonsterOK 0: the entry returns before the range test (§7.1 r2).
        AiModes::operate_door(v, g, m, shut);
        AiModes::operate_door(v, g, m, quest);
    });
    assert_eq!(*fx.sim.hooks().x.ranged.borrow(), [(m, door), (m, quest)]);
    // Operate 9 is a quest function: handed back with the monster.
    let route = ObjectRoute::Operate(Dispatch::Quest(Operate {
        object: quest,
        operator: Some(m),
        class: QUEST_DOOR as u16,
        operate_fn: 9,
    }));
    assert_eq!(fx.sim.hooks().x.log, [format!("object route {route:?}")]);
    fx.assert_clean();
}

// Covers: specs/world/objects.md §4 r1, §4 r2, §14 r1; specs/sim/units.md §6.4
#[test]
fn object_timer_event_reaches_object_event() {
    // Event 1 (ENDANIM) through the game's timer queue: mode 1 → 2 when
    // `Mode2` ≠ 0 (no update queued), then the footprint is freed
    // (`HasCollision2` = 0).
    let mut fx = fx();
    let o = create(&mut fx, WAYPOINT, 20);
    fx.game.frame = 5;
    let r = fx.sim.objects(&mut fx.game, |ctl, t, w| {
        obj::set_object_mode(ctl, t, w, o, 1)?;
        w.schedule(o, oevent::END_ANIM, 7);
        Ok::<_, obj::ObjectError>(w.flags(o))
    });
    let f = r.unwrap().unwrap();
    assert_ne!(f & oflags::CHANGED, 0);
    let rec = fx.sim.sys.units.get(o).unwrap();
    assert_eq!((rec.mode, rec.anim.frame_count), (1, 10 << 8));
    assert!(fx.timers(o).contains(&(oevent::END_ANIM, 7)));
    fx.frame();
    assert_eq!(fx.sim.sys.units.get(o).unwrap().mode, 1);
    fx.frame();
    assert_eq!(fx.sim.sys.units.get(o).unwrap().mode, 2);
    let log = &fx.sim.hooks().x.log;
    assert_eq!(log.last().unwrap(), &format!("free footprint {}", o.0));
    fx.assert_clean();
}

// Covers: specs/world/objects.md §3 r6, §4 r1, §7.2 r4; specs/world/waypoints.md §5.2
#[test]
fn waypoint_object_routes_and_mode_change() {
    let mut fx = fx();
    let o = create(&mut fx, WAYPOINT, 20);
    // Init 17 is the waypoint spec's: handed back.
    let created = obj::Created {
        init: Route::Waypoint,
        init_fn: 17,
    };
    let init = ObjectRoute::Init { object: o, created };
    assert_eq!(fx.sim.hooks().x.log, [format!("object route {init:?}")]);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    let g = guid(&fx, o);
    // 0x13: operate 23 is handed back to the caller.
    let r = fx.sim.operate_object_message(&mut fx.game, p, g);
    assert_eq!(
        r,
        Some(ObjectCase::Waypoint(Operate {
            object: o,
            operator: Some(p),
            class: WAYPOINT as u16,
            operate_fn: 23,
        }))
    );
    // The waypoint mode change runs on the object module (not Pending):
    // mode, flag 0x1, update queue, animation.
    fx.sim.waypoints(&mut fx.game, |w| {
        crate::world::waypoints::WaypointWorld::set_object_mode(w, o, 1)
    });
    assert_eq!(fx.sim.hooks().x.log.len(), 1);
    let rec = fx.sim.sys.units.get(o).unwrap();
    assert_eq!(rec.mode, 1);
    assert_ne!(rec.flags & uflags::CHANGED, 0);
    assert_eq!(rec.anim.frame_count, 10 << 8);
    assert!(fx.game.lists.update_queue(a).contains(&o));
    fx.assert_clean();
}

// Covers: specs/world/objects.md §14 r1
#[test]
fn update_pass_sends_the_state_message() {
    // `send_unit_update` (tick.md §6.5) for a queued object: S→C 0x0E to
    // the client's player.
    let mut fx = fx();
    let o = create(&mut fx, WAYPOINT, 20);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    let c = fx
        .game
        .lists
        .add_client(Some(p), Some(a), crate::units::lists::client_state::IN_GAME);
    fx.sim.objects(&mut fx.game, |ctl, t, w| {
        obj::set_object_mode(ctl, t, w, o, 1).unwrap();
    });
    crate::tick::TickHooks::send_unit_update(&mut fx.sim, &mut fx.game, c, o);
    let want = obj::state_message(guid(&fx, o), false, 1);
    assert_eq!(fx.sim.hooks().x.sent, [(p, want.to_vec())]);
    fx.assert_clean();
}
