// Spec: specs/monsters/population.md §3, §9, §11.1, §11.2, §13; specs/drlg/rooms.md §5, §10; specs/monsters/init.md §4, §5 (population ↔ DRLG rooms and monster init)
//! Population on streamed preset rooms: its room queries answered by the
//! real DRLG, its creations run through monster init and the unit
//! allocator.

use super::*;
use crate::drlg::TileRect;
use crate::monsters::population::placement::MASK_DEFAULT;
use crate::monsters::population::{preset, room, PopWorld, PresetUnit as PopPreset, RoomBox};
use crate::units::UnitType;

/// [`ISLE`] generated, its rooms (8000, 8000) and (8008, 8000)
/// streamed; the active rooms.
pub fn isle(fx: &mut Fx) -> (RoomId, RoomId) {
    let (_, rooms) = fx.generate(ISLE).unwrap();
    let at = |fx: &Fx, x| {
        *rooms
            .iter()
            .find(|&&r| fx.drlg().room(r).rect == TileRect::new(x, 8000, 8, 8))
            .unwrap()
    };
    let (a, b) = (at(fx, 8000), at(fx, 8008));
    let active = fx.stream(&[a, b]).unwrap();
    (active[0], active[1])
}

/// The monsters of the game, by allocation.
pub fn monsters(fx: &Fx) -> Vec<UnitId> {
    let mut v = fx.game.lists.units_of_type(UnitType::Monster);
    v.sort();
    v
}

// Covers: specs/monsters/population.md §9.3 text; specs/drlg/rooms.md §10.1
#[test]
fn population_queries_read_the_real_rooms() {
    let mut fx = Fx::new(isle_ds1s());
    let (a, b) = isle(&mut fx);
    let dr = fx.drlg().drlg_room_of(a).unwrap();
    let seed = fx.drlg().active_room(dr).unwrap().seed;
    let (x, y) = ISLE_MONSTER;
    fx.sim.host(&mut fx.game, |h| {
        assert_eq!(h.room_level(a), ISLE as i32);
        assert_eq!(
            h.room_box(a),
            RoomBox {
                x: 40000,
                y: 40000,
                width: 40,
                height: 40
            }
        );
        // `0x00463740`: a point of B found from A through its adjacency.
        assert_eq!(h.room_at(a, 40045, 40005), Some(b));
        assert_eq!(h.room_at(a, 40005, 40005), Some(a));
        assert_eq!(
            h.preset_units(a),
            [PopPreset {
                unit_type: 1,
                mode: 1,
                class: 0,
                x: x as i32,
                y: y as i32,
                has_data: false,
                done: false,
            }]
        );
        assert!(h.preset_units(b).is_empty());
        let recs = h.tile_records(a);
        assert!(!recs.is_empty());
        assert!(recs
            .iter()
            .all(|r| (8000..8009).contains(&r.x) && (8000..8009).contains(&r.y)));
        assert!(!h.collides(a, 40012, 40010, 1, MASK_DEFAULT));
        assert_eq!(h.client_count(a), 0);
        // The room seed is the active room's (+0x6C): a draw advances it.
        assert_eq!(*h.room_seed(a), seed);
        h.room_seed(a).step();
    });
    let mut s = seed;
    s.step();
    assert_eq!(fx.drlg().active_room(dr).unwrap().seed, s);
    fx.assert_clean();
}

// Covers: specs/monsters/population.md §11.1, §11.2, §9.6 r1, §9.6 r2, §9.6 r4, §9.6 r5; specs/monsters/init.md §4 r1, §5 r1, §5 r2, §5 r3, §5 r5, §5 r6
#[test]
fn preset_monster_is_created_through_monster_init() {
    let mut fx = Fx::new(isle_ds1s());
    let (a, _) = isle(&mut fx);
    fx.sim.create_regions();
    let game_seed = fx.sim.action.sys.hooks.game_seed;
    fx.sim
        .population(&mut fx.game, |cx| preset::place_presets(cx, a));
    fx.assert_clean();
    let m = monsters(&fx);
    assert_eq!(m.len(), 1);
    let u = m[0];
    assert_eq!(fx.game.lists.unit(u).unwrap().room(), Some(a));
    // Allocation: one game-seed step for the unit seed (`rng.md` §5.3).
    let mut g = game_seed;
    let init_seed = g.step();
    assert_eq!(fx.sim.action.sys.hooks.game_seed, g);
    let rec = fx.sim.action.sys.units.get(u).unwrap();
    assert_eq!(rec.init_seed, init_seed);
    assert_eq!(rec.class, 0);
    // Type init: unit flags 0x0A, then the preset flags 0x3000000.
    assert_eq!(rec.flags & 0x0300_000A, 0x0300_000A);
    // At the preset point: room box + DS1 offset.
    let (x, y) = ISLE_MONSTER;
    assert_eq!(
        fx.sim.action.sys.hooks.x.pos[&u],
        (40000 + x as i32, 40000 + y as i32)
    );
    let md = fx.sim.world.monsters.get(u).unwrap();
    assert_eq!((md.class, md.level_id), (0, ISLE as i32));
    // The AI control exists and the first AI setup installed a function.
    let ai = fx
        .sim
        .action
        .sys
        .hooks
        .ai_store()
        .control(u)
        .cloned()
        .unwrap();
    assert_ne!(ai.function, 0);
    // Counted in the level's region (`population.md` §13.1).
    assert_eq!(
        fx.sim
            .world
            .pop
            .regions
            .get(ISLE as i32)
            .unwrap()
            .evil_spawned,
        1
    );
    assert_eq!(
        fx.sim.action.sys.hooks.x.log,
        [
            format!("align {} 0", u.0),
            format!("preset {} class 0 at {x},{y}", u.0),
        ]
    );
}

/// Monsters with their positions and seeds.
type Spawned = Vec<(UnitId, (i32, i32), Seed)>;

/// A room population run (§3) on room A with one coordinate rectangle
/// covering it; the monsters, the region's spawned and rooms-with-spawns
/// counts.
fn populate() -> (Spawned, i32, i32) {
    let mut fx = Fx::new(isle_ds1s());
    let (a, _) = isle(&mut fx);
    let x = &mut fx.sim.action.sys.hooks.x;
    x.populate = true;
    x.room_count = 1;
    x.coords.insert(
        a,
        vec![CoordRect {
            rect: [8000, 8000, 8008, 8008],
            node_flag: 0,
            index: 1,
        }],
    );
    fx.sim.create_regions();
    fx.sim
        .population(&mut fx.game, |cx| room::populate_room(cx, a));
    fx.assert_clean();
    let units = monsters(&fx);
    let out = units
        .iter()
        .map(|&u| {
            let md = fx.sim.world.monsters.get(u).unwrap();
            assert_eq!((md.class, md.level_id), (0, ISLE as i32));
            assert!(fx.sim.action.sys.hooks.ai_store().control(u).is_some());
            (
                u,
                fx.sim.action.sys.hooks.x.pos[&u],
                fx.sim.action.sys.units.get(u).unwrap().seed,
            )
        })
        .collect();
    let r = fx.sim.world.pop.regions.get(ISLE as i32).unwrap();
    (out, r.evil_spawned, r.rooms_with_spawns)
}

// Covers: specs/monsters/population.md §3.1 r1, §3.1 r2, §3.1 r3, §3.1 r4, §3.1 r5, §3.2 r1, §3.2 r2, §3.2 r3, §7 r6, §9.6 r1, §13 r1
#[test]
fn room_population_creates_packs_through_monster_init() {
    let (units, spawned, rooms) = populate();
    assert!(!units.is_empty());
    assert_eq!(spawned, units.len() as i32);
    assert_eq!(rooms, 1);
    // Every monster stands inside the coordinate rectangle (sub-tiles).
    for (_, (x, y), _) in &units {
        assert!((40000..40040).contains(x) && (40000..40040).contains(y));
    }
    assert_eq!(units, populate().0);
}
