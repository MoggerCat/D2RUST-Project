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
