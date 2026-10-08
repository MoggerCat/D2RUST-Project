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
use crate::combat::CombatWorld;
use crate::monsters::ai::{AiModes, AiTargets};
use crate::skills::SkillUnits;
use crate::units::record::flags as uflags;
use crate::wiring::action::{ObjectCase, ObjectRoute};
use crate::world::objects::MiscWorld;
use crate::world::objects::{
    self as obj, oevent, oflags, Dispatch, ObjectTables, ObjectWorld, Operate, Route,
};

const CHEST: u32 = 0;
const TORCH: u32 = 1;
const DOOR: u32 = 2;
const QUEST_DOOR: u32 = 3;
const WAYPOINT: u32 = 4;
const LOCKED_OUT_DOOR: u32 = 5;
/// A 2 × 2 door that blocks vision (footprint mask 0x806).
const BIG_DOOR: u32 = 6;

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
    // A 2 × 2 footprint with `BlockMissile`: mask 0x404
    // (`path-placement.md` §3).
    (wp.sizex, wp.sizey, wp.blockmissile) = (2, 2, 1);
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
        objgroup: Vec::new(),
        leveldefs: Vec::new(),
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

// Covers: specs/world/objects.md §7.3 r1, §7.3 r2, §7.3 r3, §7.3 r4, §7.3 r5
#[test]
fn the_0x13_object_case_results() {
    use crate::wiring::action::ObjectReach;
    let mut fx = fx();
    let torch = create(&mut fx, TORCH, 20);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    let g = guid(&fx, torch);
    // r1: no object with the GUID → 1.
    assert_eq!(
        fx.sim.operate_object_message(&mut fx.game, p, 0xBEEF),
        Some(ObjectCase::Code(1))
    );
    // r2: object mode ≥ 8 → 3 (the approach is not consulted).
    fx.sim.sys.units.get_mut(torch).unwrap().mode = 8;
    fx.sim.hooks().x.reach = Some(ObjectReach::TooFar);
    assert_eq!(
        fx.sim.operate_object_message(&mut fx.game, p, g),
        Some(ObjectCase::Code(3))
    );
    fx.sim.sys.units.get_mut(torch).unwrap().mode = 0;
    // r3: too far → 1; r4: not in range / obstructed → walk → 0, and the
    // operate does not run.
    assert_eq!(
        fx.sim.operate_object_message(&mut fx.game, p, g),
        Some(ObjectCase::Code(1))
    );
    fx.sim.hooks().x.reach = Some(ObjectReach::Walk);
    assert_eq!(
        fx.sim.operate_object_message(&mut fx.game, p, g),
        Some(ObjectCase::Code(0))
    );
    assert!(fx.sim.hooks().x.ranged.borrow().is_empty());
    // r5: in range → the operate entry runs and the case gives 0 (the
    // entry's own result 0, object gone, maps to 3; with the same GUID
    // lookup on both sides the wiring cannot reach it).
    fx.sim.hooks().x.reach = Some(ObjectReach::Operate);
    assert_eq!(
        fx.sim.operate_object_message(&mut fx.game, p, g),
        Some(ObjectCase::Code(0))
    );
    assert_eq!(fx.sim.hooks().x.ranged.borrow().len(), 1);
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

// Covers: specs/world/objects.md §4 r1, §4 r2, §14 r1, §5.5; specs/sim/units.md §6.4; specs/world/objects-2.md §18.6; specs/sim/path-placement.md §3, §5.1
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
    // The footprint stamped at (20, 20): box (19..=20, 19..=20).
    let a = fx.a;
    let cell = |fx: &mut Fx, x, y| fx.sim.hooks().drlg.collision(&fx.game, a, x, y).unwrap();
    let boxed = [(19, 19), (20, 19), (19, 20), (20, 20)];
    let before: Vec<u16> = boxed.iter().map(|&(x, y)| cell(&mut fx, x, y)).collect();
    let outside = cell(&mut fx, 21, 21);
    fx.sim.objects(&mut fx.game, |_, _, w| {
        w.stamp_footprint(o, Some(a), 20, 20);
    });
    for (x, y) in boxed {
        assert_eq!(cell(&mut fx, x, y) & 0x404, 0x404, "({x}, {y})");
    }
    assert_eq!(cell(&mut fx, 21, 21), outside);
    let rec = fx.sim.sys.units.get(o).unwrap();
    assert_eq!((rec.mode, rec.anim.frame_count), (1, 10 << 8));
    assert!(fx.timers(o).contains(&(oevent::END_ANIM, 7)));
    fx.frame();
    assert_eq!(fx.sim.sys.units.get(o).unwrap().mode, 1);
    fx.frame();
    assert_eq!(fx.sim.sys.units.get(o).unwrap().mode, 2);
    // `0x00623830`: the box cleared at the object's room and position.
    for (&(x, y), b) in boxed.iter().zip(&before) {
        assert_eq!(cell(&mut fx, x, y), b & !0x404, "({x}, {y})");
    }
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
    // Already announced (the room clean-up cleared unit flag 0x10).
    fx.sim.sys.units.get_mut(o).unwrap().flags &= !crate::units::record::flags::SEED_SET;
    crate::tick::TickHooks::send_unit_update(&mut fx.sim, &mut fx.game, c, o);
    let want = obj::state_message(guid(&fx, o), false, 1);
    assert_eq!(fx.sim.hooks().x.sent, [(p, want.to_vec())]);
    fx.assert_clean();
}

// Covers: specs/sim/intents-events.md §7.1 r2, §7.2
#[test]
fn update_pass_announces_a_new_object_first() {
    // An object created in a room the client already holds (unit flag
    // 0x10 still set): its add message 0x51 goes out in the client pass,
    // before the object update's state message.
    let mut fx = fx();
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    let c = fx
        .game
        .lists
        .add_client(Some(p), Some(a), crate::units::lists::client_state::IN_GAME);
    let o = create(&mut fx, WAYPOINT, 20);
    fx.sim.objects(&mut fx.game, |ctl, t, w| {
        obj::set_object_mode(ctl, t, w, o, 1).unwrap();
    });
    crate::tick::TickHooks::send_unit_update(&mut fx.sim, &mut fx.game, c, o);
    let og = guid(&fx, o);
    let sent = fx.sim.hooks().x.sent.clone();
    assert!(sent.iter().all(|(to, _)| *to == p));
    assert_eq!(sent[0].1[0], 0x51);
    assert_eq!(sent[0].1[1], 2);
    assert_eq!(sent[0].1[2..6], og.to_le_bytes());
    let want = obj::state_message(og, false, 1);
    assert_eq!(sent.last().unwrap().1, want.to_vec());
    // The client's own player is never announced to itself.
    fx.sim.hooks().x.sent.clear();
    crate::tick::TickHooks::send_unit_update(&mut fx.sim, &mut fx.game, c, p);
    assert!(fx.sim.hooks().x.sent.iter().all(|(_, m)| m[0] != 0x59));
    fx.assert_clean();
}

// Covers: specs/world/objects.md §3 r1, §3 r9, §6
#[test]
fn preset_shrine_allocates_inside_the_object_call() {
    // Preset 575 (`0x006E1080` {136, 7, 7}): the shrine object is
    // allocated while the preset spawner holds the control; the init
    // dispatch runs on it there, then the id is set (`roll(0)` draws
    // nothing, id 7).
    let mut fx = Fx::new();
    let mut t = (*tables()).clone();
    t.objects.resize(137, blank());
    fx.sim.create_objects(Arc::new(t));
    let a = fx.a;
    let o = fx
        .sim
        .with(&mut fx.game, |g, v| v.create_object(g, a, 575, 20, 20, 0))
        .expect("allocated");
    let g = guid(&fx, o);
    let d = fx.sim.hooks().objects.as_ref().unwrap().control.data[&o];
    assert_eq!((d.class, d.guid), (136, g));
    assert_eq!((d.interact, d.shrine, d.owner), (7, Some(7), Some(-1)));
    assert_eq!(fx.sim.sys.units.get(o).unwrap().class, 136);
    fx.assert_clean();
}

/// The chest-drop fixture: the path provider with the synthetic
/// walk-back field, the object state, the drop state with the gold TC as
/// every chest TC (`treasure.md` §1.6).
fn drop_fx() -> Fx {
    let mut fx = Fx::new();
    let h = fx.sim.hooks();
    h.enable_paths().expect("embedded tables");
    h.paths.as_mut().unwrap().field = Some(Arc::new(crate::path::search::tests::sign_field()));
    let mut t = super::death::drop_tables();
    t.tcs.chest = [Some(1); 45];
    h.object_drops = Some(Box::new(crate::wiring::economy::DeathDrops::new(
        Arc::new(t),
        crate::wiring::economy::GameFields::new(Seed::init(), false),
    )));
    fx.sim.create_objects(tables());
    fx
}

// Covers: specs/items/treasure.md §4 r1, §4 r5, §4 r6, §7 r2, §7 r3
#[test]
fn the_chest_drop_walks_on_the_object_seed_and_drops_into_its_room() {
    // `D(0)` by a player on a chest at (20, 20): the walk on the chest's
    // unit seed picks the gold (`roll(1)`), the item is created on the
    // game seed (two steps) at the start (22, 23) in the chest's room,
    // item level = the area level of the chest's level (`MonLvl1` 1).
    let mut fx = drop_fx();
    let o = create(&mut fx, CHEST, 20);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 10, 10);
    let mut want_obj = fx.sim.sys.units.get(o).unwrap().seed;
    want_obj.roll(1);
    let mut want_game = fx.sim.hooks().game_seed;
    want_game.step();
    want_game.step();
    // The game's one unique-bit store (`ActionHooks::uniques`) is the
    // drop's: lent to it and back, not a copy kept in the drop state.
    fx.sim.hooks().uniques.0[0] = 1 << 5;
    let op = Operate {
        object: o,
        operator: Some(p),
        class: CHEST as u16,
        operate_fn: 4,
    };
    let item = fx
        .sim
        .objects(&mut fx.game, |_, _, w| {
            crate::world::objects::ChestWorld::chest_drop(w, &op, 0)
        })
        .flatten()
        .expect("an item");
    let d = fx.sim.hooks().object_drops.take().unwrap();
    assert!(d.failures.is_empty() && d.errors.is_empty());
    assert!(fx.sim.hooks().uniques.get(5));
    assert_eq!(d.fields.uniques, crate::items::UniqueBits::default());
    assert_eq!(d.placed.len(), 1);
    assert_eq!(d.placed[0].0, item);
    let spot = d.placed[0].1;
    assert_eq!((spot.room, spot.x, spot.y), (Some(a), 22, 23));
    assert_eq!(fx.game.lists.unit(item).unwrap().room(), Some(a));
    let r = fx.sim.sys.units.get(item).unwrap();
    assert_eq!((r.ty, r.mode), (UnitType::Item, 3));
    assert_eq!(fx.sim.hooks().items.get(item).unwrap().ilvl, 1);
    assert_eq!(fx.sim.sys.units.get(o).unwrap().seed, want_obj);
    assert_eq!(fx.sim.hooks().game_seed, want_game);
    // The chest seams on the same view: unit type, item quality, the
    // room's units in list order.
    let (ty, q, units) = fx
        .sim
        .objects(&mut fx.game, |_, _, w| {
            use crate::world::objects::ChestWorld;
            (w.unit_type(item), w.item_quality(item), w.room_units(a))
        })
        .unwrap();
    assert_eq!(ty, Some(4));
    assert_eq!(q, Some(fx.sim.hooks().items.get(item).unwrap().quality));
    assert_eq!(units, fx.game.lists.room_units(a));
    assert!(units.contains(&item) && units.contains(&o));
    fx.assert_clean();
}

// Covers: specs/items/treasure.md §4 r1
#[test]
fn without_drop_state_or_room_the_chest_drop_is_none() {
    // No drop state: none, no draw (as before the provider).
    let mut fx = fx();
    let o = create(&mut fx, CHEST, 20);
    let seed = fx.sim.sys.units.get(o).unwrap().seed;
    let op = Operate {
        object: o,
        operator: None,
        class: CHEST as u16,
        operate_fn: 4,
    };
    let r = fx.sim.objects(&mut fx.game, |_, _, w| {
        crate::world::objects::ChestWorld::chest_drop(w, &op, 0)
    });
    assert_eq!(r, Some(None));
    assert_eq!(fx.sim.sys.units.get(o).unwrap().seed, seed);
    // With drop state, an object without a room: none, no draw (§4 r1).
    let mut fx = drop_fx();
    let o = create(&mut fx, CHEST, 20);
    fx.game.lists.room_remove(o).unwrap();
    let seed = fx.sim.sys.units.get(o).unwrap().seed;
    let op = Operate { object: o, ..op };
    let r = fx.sim.objects(&mut fx.game, |_, _, w| {
        crate::world::objects::ChestWorld::chest_drop(w, &op, 0)
    });
    assert_eq!(r, Some(None));
    assert_eq!(fx.sim.sys.units.get(o).unwrap().seed, seed);
    assert!(fx
        .sim
        .hooks()
        .object_drops
        .as_ref()
        .unwrap()
        .placed
        .is_empty());
}

// Covers: specs/world/objects.md §11 r1, §11 r2, §11 r3
#[test]
fn a_well_heals_life_on_the_players_stat_list() {
    // A live well (`Parm0` 750, `Parm1` 128, `Parm2` 1, `Parm3` 3) with
    // 2 charges: life 10 of 100 (8.8 fixed) → min(10 + 50, 100) = 60;
    // mana and stamina at their maxima (0) do not change; no state lists.
    // Used: 1 charge left, mode 2 − 1 / 1 = 1, refill at frame + 751.
    const WELL: u32 = 6;
    let mut fx = Fx::new();
    let mut t = (*tables()).clone();
    let mut well: Objects = blank();
    well.operatefn = 22;
    (well.parm0, well.parm1, well.parm2, well.parm3) = (750, 128, 1, 3);
    t.objects.push(well);
    fx.sim.create_objects(Arc::new(t));
    let o = create(&mut fx, WELL, 20);
    fx.sim
        .hooks()
        .objects
        .as_mut()
        .unwrap()
        .control
        .data
        .get_mut(&o)
        .unwrap()
        .interact = 2;
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    fx.stats(p, &[(6, 10 << 8), (7, 100 << 8)]);
    let op = Operate {
        object: o,
        operator: Some(p),
        class: WELL as u16,
        operate_fn: 22,
    };
    let frame = fx.game.frame;
    let r = fx.sim.objects(&mut fx.game, |ctl, t, w| {
        crate::world::objects::misc::well(ctl, t, w, &op)
    });
    assert_eq!(r, Some(Ok(0)));
    assert_eq!(fx.stat(p, 6), 60 << 8);
    assert_eq!(fx.stat(p, 8), 0);
    let st = fx.sim.hooks().objects.as_ref().unwrap();
    assert_eq!(st.control.data[&o].interact, 1);
    assert_eq!(fx.sim.sys.units.get(o).unwrap().mode, 1);
    assert!(fx
        .timers(o)
        .contains(&(crate::world::objects::oevent::WELL_REFILL, frame + 751)));
    // A full player: nothing used, no charge spent.
    fx.stats(p, &[(6, 100 << 8)]);
    let r = fx.sim.objects(&mut fx.game, |ctl, t, w| {
        crate::world::objects::misc::well(ctl, t, w, &op)
    });
    assert_eq!(r, Some(Ok(0)));
    let st = fx.sim.hooks().objects.as_ref().unwrap();
    assert_eq!(st.control.data[&o].interact, 1);
    fx.assert_clean();
}

// Covers: specs/world/objects.md §9.2
#[test]
fn refill_and_life_shrines_act_on_the_players_stat_list() {
    // Code 1 (refill `0x005828E0`): life and mana += max − current;
    // code 2 (`0x00582860`): life := max life.
    let mut fx = fx();
    let o = create(&mut fx, TORCH, 20);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    fx.stats(p, &[(6, 5 << 8), (7, 40 << 8), (8, 1 << 8), (9, 30 << 8)]);
    let mut s: d2_data::tables::Shrines = blank();
    s.code = 1;
    let r = fx.sim.objects(&mut fx.game, |_, t, w| {
        crate::world::objects::shrines::effect(t, w, o, p, &s)
    });
    assert_eq!(r, Some(Ok(())));
    assert_eq!((fx.stat(p, 6), fx.stat(p, 8)), (40 << 8, 30 << 8));
    fx.stats(p, &[(6, 3 << 8)]);
    s.code = 2;
    let r = fx.sim.objects(&mut fx.game, |_, t, w| {
        crate::world::objects::shrines::effect(t, w, o, p, &s)
    });
    assert_eq!(r, Some(Ok(())));
    assert_eq!(fx.stat(p, 6), 40 << 8);
    fx.assert_clean();
}

// Covers: specs/world/quests-act2.md §1.3; specs/items/treasure.md §4 r6
#[test]
fn a_quest_chests_treasure_drops_through_the_lent_economy() {
    // `0x00585B90(op, 4)` from a quest chest (`HostQuests`): the item
    // store and game seed are the economy's while a quest call runs; the
    // item lands in that store and the seed step is the economy's.
    use crate::wiring::economy::{Economy, EconomyQuests, GameFields, HostQuests, ItemStore};
    use crate::world::quests::QuestWorld;
    let mut fx = drop_fx();
    let o = create(&mut fx, CHEST, 20);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 10, 10);
    let mut want_game = fx.sim.hooks().game_seed;
    want_game.step();
    want_game.step();
    let tables = super::death::drop_tables().items;
    let mut rest = crate::wiring::interaction::tests::Rest::new();
    let s = &mut fx.sim.sys;
    let mut fields = GameFields::new(s.hooks.game_seed, false);
    let mut items = std::mem::replace(&mut s.hooks.items, ItemStore::new());
    {
        let mut econ = Economy {
            game: &mut fx.game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
            hooks: &mut s.hooks,
            fields: &mut fields,
            tables: &tables,
            items: &mut items,
        };
        let mut w = HostQuests::new(EconomyQuests::new(&mut econ, &mut rest));
        w.object_treasure(o, p, 4);
    }
    let d = s.hooks.object_drops.as_ref().unwrap();
    assert!(d.failures.is_empty() && d.errors.is_empty());
    assert_eq!(d.placed.len(), 1);
    let item = d.placed[0].0;
    assert!(items.get(item).is_some());
    assert!(s.hooks.items.get(item).is_none());
    assert_eq!(fields.seed, want_game);
    s.hooks.items = items;
    fx.assert_clean();
}

// Covers: specs/world/quests-act2.md §8.7
#[test]
fn host_quests_read_the_missile_range_from_the_action_tables() {
    use crate::wiring::economy::{Economy, EconomyQuests, GameFields, HostQuests, ItemStore};
    use crate::world::quests::QuestWorld;
    let mut fx = fx();
    let mut t = (*fx.sim.hooks().tables).clone();
    let n = t.missiles.len();
    t.missiles[n - 1].range = 20;
    fx.sim.hooks().tables = Arc::new(t);
    let tables = super::death::drop_tables().items;
    let mut rest = crate::wiring::interaction::tests::Rest::new();
    let s = &mut fx.sim.sys;
    let mut fields = GameFields::new(s.hooks.game_seed, false);
    let mut items = ItemStore::new();
    let mut econ = Economy {
        game: &mut fx.game,
        units: &mut s.units,
        stats: &mut s.stats,
        data: &s.data,
        hooks: &mut s.hooks,
        fields: &mut fields,
        tables: &tables,
        items: &mut items,
    };
    let mut w = HostQuests::new(EconomyQuests::new(&mut econ, &mut rest));
    assert_eq!(w.missile_range(n as u32 - 1), Some(20));
    assert_eq!(w.missile_range(n as u32), None);
}

// ---- the drop helpers (`objects-2.md` §20) -------------------------------------------

/// [`drop_fx`] with the pick rows of its one item (gold, index 0) in
/// `part`'s place: `armor` rows before it (none) and the rest misc.
fn picks_fx(armor: usize) -> Fx {
    use crate::treasure::class_pick::{ClassPicks, PickRow};
    let mut fx = drop_fx();
    let picks = ClassPicks {
        rows: vec![PickRow {
            code: *b"gld ",
            spawnable: 1,
            level: 1,
            rarity: 1,
            type_: 4,
            ..PickRow::default()
        }],
        weapons: 0,
        armor,
        parts: None,
    };
    let h = fx.sim.hooks();
    let d = h.object_drops.take().unwrap();
    // A rolled quality (request quality 0, `objects-2.md` §20) reads the
    // itemratio divisors: 1 each.
    let mut t = (*d.tables).clone();
    for r in &mut t.items.itemratio {
        (r.uniquedivisor, r.raredivisor, r.setdivisor) = (1, 1, 1);
        (r.magicdivisor, r.hiqualitydivisor, r.normaldivisor) = (1, 1, 1);
    }
    let d = crate::wiring::economy::DeathDrops::new(Arc::new(t), d.fields);
    h.object_drops = Some(Box::new(d.with_picks(Arc::new(picks))));
    fx
}

fn room_seed(fx: &Fx) -> Seed {
    let d = fx.sim.sys.hooks.drlg.dungeon.acts[0].as_ref().unwrap();
    let r = d.drlg_room_of(fx.a).unwrap();
    d.active_room(r).unwrap().seed
}

// Covers: specs/world/objects-2.md §20.3; specs/items/treasure.md §8 r1
#[test]
fn the_gold_helper_drops_gold_into_the_room_with_flag_0x2000_clear() {
    let mut fx = picks_fx(0);
    let a = fx.a;
    let mut want_game = fx.sim.hooks().game_seed;
    want_game.step();
    want_game.step();
    let before = room_seed(&fx);
    fx.sim
        .objects(&mut fx.game, |_, _, w| w.gold_drop(a, 20, 20));
    let d = fx.sim.hooks().object_drops.take().unwrap();
    assert!(
        d.failures.is_empty() && d.pick_errors.is_empty(),
        "{:?} {:?}",
        d.failures,
        d.pick_errors
    );
    assert_eq!(d.placed.len(), 1);
    let (item, spot) = d.placed[0];
    assert_eq!((spot.room, spot.x, spot.y), (Some(a), 22, 23));
    let i = fx.sim.hooks().items.get(item).unwrap();
    // L = area level 1 (`MonLvl1` 1; > 1 → − 1 does not apply).
    assert_eq!((i.record, i.ilvl), (0, 1));
    assert_eq!(i.flags & 0x2000, 0);
    assert_eq!(fx.sim.hooks().game_seed, want_game);
    // No pick: the gold helper does not draw on the room seed.
    assert_eq!(room_seed(&fx), before);
}

// Covers: specs/world/objects-2.md §20.1, §20.5
#[test]
fn the_armor_helper_picks_on_the_room_seed_with_the_superior_flag() {
    // The one row read as the armor part: rarity 1 − A(1) = 1 → one
    // `roll(1)`, then the pick `roll(1)`: two room-seed steps.
    let mut fx = picks_fx(1);
    let o = create(&mut fx, CHEST, 20);
    let mut want = room_seed(&fx);
    want.step();
    want.step();
    fx.sim.objects(&mut fx.game, |_, _, w| {
        crate::world::objects::MechWorld::stand_drop(w, o, false)
    });
    assert_eq!(room_seed(&fx), want);
    let d = fx.sim.hooks().object_drops.take().unwrap();
    assert_eq!(d.placed.len(), 1);
    let item = d.placed[0].0;
    assert_eq!(fx.sim.hooks().items.get(item).unwrap().ilvl, 1);
    // The weapon helper with no weapon part: −1 at once, nothing drawn.
    let mut fx = picks_fx(1);
    let o = create(&mut fx, CHEST, 20);
    let before = room_seed(&fx);
    fx.sim.objects(&mut fx.game, |_, _, w| {
        crate::world::objects::MechWorld::stand_drop(w, o, true)
    });
    assert_eq!(room_seed(&fx), before);
    assert!(fx
        .sim
        .hooks()
        .object_drops
        .as_ref()
        .unwrap()
        .placed
        .is_empty());
}

// Covers: specs/world/objects-2.md §20.4, §20.6; specs/world/objects.md §8
#[test]
fn source_and_code_drops_use_the_unit_seed_and_level() {
    use crate::treasure::class_pick::PickError;
    // The code drop `C('gld ')`: the code's index, no unit-seed draw,
    // ilvl = the chest's area level.
    let mut fx = picks_fx(0);
    let o = create(&mut fx, CHEST, 20);
    let seed = fx.sim.sys.units.get(o).unwrap().seed;
    let gld = u32::from_le_bytes(*b"gld ");
    let item = fx
        .sim
        .objects(&mut fx.game, |_, _, w| {
            crate::world::objects::ChestWorld::code_drop(w, o, gld)
        })
        .flatten()
        .expect("a gold item");
    assert_eq!(fx.sim.sys.units.get(o).unwrap().seed, seed);
    assert_eq!(fx.sim.hooks().items.get(item).unwrap().ilvl, 1);
    // An unknown code: fatal 0x9EA, nothing created.
    fx.sim.objects(&mut fx.game, |_, _, w| {
        crate::world::objects::ChestWorld::drop_item_code(w, o, 0x2020_2020)
    });
    let d = fx.sim.hooks().object_drops.as_ref().unwrap();
    assert_eq!(d.pick_errors.len(), 1);
    assert_eq!(d.placed.len(), 1);
    // Drop code 0: the random class; the fixture's gold row is index 0,
    // which `0x00556240` refuses (fatal 0x17F) before its `roll(100)`:
    // no draw, nothing created.
    fx.sim.objects(&mut fx.game, |_, _, w| {
        crate::world::objects::MechWorld::drop_code_quality(w, o, 0, 2)
    });
    assert_eq!(fx.sim.sys.units.get(o).unwrap().seed, seed);
    let d = fx.sim.hooks().object_drops.as_ref().unwrap();
    assert_eq!(
        d.pick_errors,
        [PickError::Code(0x2020_2020), PickError::NoGold]
    );
    assert_eq!(d.placed.len(), 1);
}

// Covers: specs/world/quests-act2.md §1.3; specs/world/objects-2.md §20.4
#[test]
fn host_quests_drop_gold_and_quest_drops_through_the_drop_helpers() {
    // `0x00585970(game, object, 'gld ', 2)` and `0x00559A30` with a drop
    // code from a quest call: the items land in the economy's store.
    use crate::wiring::economy::{Economy, EconomyQuests, GameFields, HostQuests, ItemStore};
    use crate::world::quests::QuestWorld;
    let mut fx = picks_fx(0);
    let o = create(&mut fx, CHEST, 20);
    let tables = fx
        .sim
        .hooks()
        .object_drops
        .as_ref()
        .unwrap()
        .tables
        .items
        .clone();
    let mut rest = crate::wiring::interaction::tests::Rest::new();
    let s = &mut fx.sim.sys;
    let mut fields = GameFields::new(s.hooks.game_seed, false);
    let mut items = std::mem::replace(&mut s.hooks.items, ItemStore::new());
    let (a, b) = {
        let mut econ = Economy {
            game: &mut fx.game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
            hooks: &mut s.hooks,
            fields: &mut fields,
            tables: &tables,
            items: &mut items,
        };
        let mut w = HostQuests::new(EconomyQuests::new(&mut econ, &mut rest));
        w.drop_gold(o);
        let b = w.quest_drop(o, *b"gld ", 2, None, false);
        (w.drop_item_at(o, *b"gld ", 2), b)
    };
    assert!(a);
    let d = s.hooks.object_drops.as_ref().unwrap();
    assert!(d.failures.is_empty() && d.pick_errors.is_empty());
    assert_eq!(d.placed.len(), 3);
    assert_eq!(Some(d.placed[1].0), b);
    for (item, _) in &d.placed {
        assert!(items.get(*item).is_some());
        assert!(s.hooks.items.get(*item).is_none());
    }
    s.hooks.items = items;
    fx.assert_clean();
}

// Covers: specs/world/objects-2.md §21
#[test]
fn curable_state_removal() {
    let mut fx = fx();
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    // States 45 and 46 are curable in the fixture's table, 30 is not.
    fx.sim.combat(&mut fx.game, |w, _| {
        for s in [30u16, 45] {
            w.create_state_list(p, s, p, 1000);
            w.set_state(p, s, true);
        }
        // 46: the state is set but has no stat list: left alone.
        w.set_state(p, 46, true);
    });
    let cure = |fx: &mut Fx| {
        fx.sim
            .objects(&mut fx.game, |_, _, w| w.cure_states(p))
            .unwrap()
    };
    assert!(cure(&mut fx));
    fx.sim.combat(&mut fx.game, |w, _| {
        assert_eq!(w.state_list_expiry(p, 45), None, "list freed");
        assert_eq!(w.state_list_expiry(p, 30), Some(1000), "not curable");
        assert!(w.has_state(p, 46));
    });
    // Nothing left to remove: 0, and no draws on any seed.
    let before = fx.sim.hooks().objects.as_ref().unwrap().control.seed;
    assert!(!cure(&mut fx));
    assert_eq!(
        fx.sim.hooks().objects.as_ref().unwrap().control.seed,
        before
    );
}

// Covers: specs/world/objects.md §10 r2; specs/sim/path-placement.md §3
#[test]
fn door_operate_frees_then_restamps_the_footprint() {
    let mut fx = Fx::new();
    let mut t = (*tables()).clone();
    let mut big: Objects = blank();
    (big.initfn, big.operatefn, big.monsterok) = (5, 8, 1);
    (big.sizex, big.sizey, big.isdoor, big.blocksvis) = (2, 2, 1, 1);
    big.hascollision0 = 1;
    t.objects.push(big);
    fx.sim.create_objects(Arc::new(t));
    let door = create(&mut fx, BIG_DOOR, 20);
    let a = fx.a;
    let cell = |fx: &mut Fx| fx.sim.hooks().drlg.collision(&fx.game, a, 20, 20).unwrap() & 0x806;
    // The closed door's footprint stands (stamped by the preset code).
    fx.sim.objects(&mut fx.game, |_, _, w| {
        w.stamp_footprint(door, Some(a), 20, 20)
    });
    assert_eq!(cell(&mut fx), 0x806);
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    let g = guid(&fx, door);
    fx.sim.hooks().x.reach = Some(crate::wiring::action::ObjectReach::Operate);
    let op = |fx: &mut Fx| fx.sim.operate_object_message(&mut fx.game, p, g);
    // Open: the footprint is freed, mode 2.
    fx.sim.hooks().objects.as_mut().unwrap().host_tick = 1000;
    op(&mut fx);
    assert_eq!(fx.sim.sys.units.get(door).unwrap().mode, 2);
    assert_eq!(cell(&mut fx), 0);
    // Close (after the 500 ms debounce): stamped again, mode 0.
    fx.sim.hooks().objects.as_mut().unwrap().host_tick += 1000;
    op(&mut fx);
    assert_eq!(fx.sim.sys.units.get(door).unwrap().mode, 0);
    assert_eq!(cell(&mut fx), 0x806);
}

// Covers: specs/world/objects.md §9.1 r3, §14 r2
#[test]
fn shrine_hover_is_kept_sent_and_expires() {
    let mut fx = fx();
    let o = create(&mut fx, CHEST, 20);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    fx.game.frame = 10;
    fx.sim.objects(&mut fx.game, |_, _, w| {
        assert!(crate::world::objects::ShrineWorld::create_hover(w, o, 3690));
        w.queue_update(o);
        w.set_flags(o, w.flags(o) | oflags::HOVER_FREED);
    });
    // "3690" is 4 characters: 8 · 4 + 125 frames.
    let exp = fx.sim.sys.units.get(o).unwrap().hover;
    assert_eq!(exp, Some(10 + 157));
    assert!(fx.sim.with(&mut fx.game, |g, v| v.object_update(g, p, o)));
    let sent = &fx.sim.hooks().x.sent;
    let m = &sent.last().expect("hover message").1;
    assert_eq!(m[0], 0x26);
    assert_eq!(m[1], 5);
    assert!(m.windows(4).any(|w| w == b"3690"));
    fx.sim.objects(&mut fx.game, |_, _, w| {
        crate::world::objects::ShrineWorld::free_hover(w, o);
    });
    assert_eq!(fx.sim.sys.units.get(o).unwrap().hover, None);
}

// Covers: specs/world/objects.md §9.2
#[test]
fn a_shrine_state_carries_its_stats_and_ends_on_its_tick() {
    // Code 7 (armor shrine `0x005839B0`): state 129 on the player for the
    // row's duration, stat 25 = arg1 and to-hit on the same list; the
    // type-12 timer frees the list on the expiry frame.
    let mut fx = fx();
    let o = create(&mut fx, TORCH, 20);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    let mut s: d2_data::tables::Shrines = blank();
    s.code = 7;
    s.arg0 = 50;
    s.arg1 = 25;
    s.duration_in_frames = 5;
    let start = fx.game.frame;
    let r = fx.sim.objects(&mut fx.game, |_, t, w| {
        crate::world::objects::shrines::effect(t, w, o, p, &s)
    });
    assert_eq!(r, Some(Ok(())));
    assert!(fx
        .sim
        .with(&mut fx.game, |_, v| v.state_list(p, 129).is_some()));
    assert_eq!(fx.stat(p, 25), 25);
    assert!(fx.timers(p).contains(&(12, start + 5)));
    while fx.game.frame < start + 4 {
        fx.frame();
    }
    assert_eq!(fx.stat(p, 25), 25);
    fx.frame();
    assert_eq!(fx.game.frame, start + 5);
    assert!(fx
        .sim
        .with(&mut fx.game, |_, v| v.state_list(p, 129).is_none()));
    assert_eq!(fx.stat(p, 25), 0);
    fx.sim.combat(&mut fx.game, |w, _| {
        assert!(!w.has_state(p, 129), "state off")
    });
    fx.assert_clean();
}

// Covers: specs/world/objects.md §12 r13
#[test]
fn portal_use_sets_state_102() {
    use crate::world::objects::misc::MiscWorld;
    let mut fx = fx();
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    fx.game.frame = 10;
    fx.sim
        .objects(&mut fx.game, |_, _, w| w.just_portaled(p, 85));
    fx.sim.combat(&mut fx.game, |w, _| {
        assert!(w.has_state(p, 102), "state 102 on");
        assert_eq!(w.state_list_expiry(p, 102), Some(85), "f + 75");
    });
}

fn use_shrine(fx: &mut Fx, o: UnitId, p: UnitId, code: u8, duration: i32) {
    let mut s: d2_data::tables::Shrines = blank();
    s.code = code;
    s.arg0 = 50;
    s.arg1 = 25;
    s.duration_in_frames = duration as u32;
    let r = fx.sim.objects(&mut fx.game, |_, t, w| {
        crate::world::objects::shrines::effect(t, w, o, p, &s)
    });
    assert_eq!(r, Some(Ok(())));
}

// Covers: specs/skills/bodies.md §2.7 r4; specs/world/objects.md §9.2
#[test]
fn a_second_shrine_use_refreshes_the_state_instead_of_replacing_it() {
    let mut fx = fx();
    let o = create(&mut fx, TORCH, 20);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    let start = fx.game.frame;
    use_shrine(&mut fx, o, p, 7, 10);
    for _ in 0..6 {
        fx.frame();
    }
    use_shrine(&mut fx, o, p, 7, 10);
    assert_eq!(fx.stat(p, 25), 25, "one list, not two");
    assert!(fx.timers(p).contains(&(12, start + 16)));
    while fx.game.frame < start + 10 {
        fx.frame();
    }
    fx.sim.combat(&mut fx.game, |w, _| {
        assert!(w.has_state(p, 129), "still on")
    });
    while fx.game.frame < start + 16 {
        fx.frame();
    }
    fx.sim
        .combat(&mut fx.game, |w, _| assert!(!w.has_state(p, 129), "off"));
}

// Covers: specs/world/objects.md §9.2
#[test]
fn the_stamina_shrine_callback_clamps_stamina_when_it_ends() {
    let mut fx = fx();
    let o = create(&mut fx, TORCH, 20);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 22, 20);
    fx.sim.with(&mut fx.game, |_, v| {
        v.set_base(p, 11, 25600);
        v.set_base(p, 162, 100);
    });
    let max = fx.sim.with(&mut fx.game, |_, v| v.stats.max_stamina(p));
    assert!(max > 0);
    use_shrine(&mut fx, o, p, 14, 5);
    let during = fx.stat(p, 10);
    assert!(during > max, "stamina is 2v while the state lasts");
    for _ in 0..5 {
        fx.frame();
    }
    assert_eq!(fx.stat(p, 10), max, "clamped to the maximum");
    fx.sim
        .combat(&mut fx.game, |w, _| assert!(!w.has_state(p, 136), "off"));
}
