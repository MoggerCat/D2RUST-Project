// Spec: specs/formats/animdata.md §3–§5; specs/sim/units.md §4.1, §4.2, §4.6; specs/combat/damage.md §7.1, §7.2; specs/combat/vitals.md §4.2, §4.3; specs/items/treasure.md §3.1–§3.5, §7
//! The animation schedule on AnimData records, the kill after a missile
//! hit (death mode with its target, experience) and the dead monster's
//! drop as real item units in its room.

use d2_data::fixup::maps::EquivMatrix;
use d2_data::tables::{Experience, Itemratio, Itemtypes, Monstats};
use d2_formats::animdata::{self, AnimData, AnimRecord};

use super::*;
use crate::combat::vitals::VitalsTables;
use crate::drlg::collision::bits;
use crate::items::tables::ItemRec;
use crate::items::{ty, ItemTables};
use crate::missiles::{create_missile, param_flags, unit_flag, MissileParams};
use crate::stats::stat as st;
use crate::tick::events::event;
use crate::treasure::{ItemData, TcEntry, TreasureClass, TreasureClasses};
use crate::units::hooks::Sim;
use crate::units::modes::{self, monster_mode};
use crate::wiring::economy::{
    monster_death_drop, DeathDrops, DropSpot, DropTables, FreeSpot, GameFields,
};

const TOHIT: u16 = 19;
const LEVEL_STAT: u16 = 12;
const EXPERIENCE: u16 = 13;
const MINDAMAGE: u16 = 21;
const MAXDAMAGE: u16 = 22;
const GOLD_STAT: u16 = 14;

const CAST: &[u8; 8] = b"SOSCHTH\0";
const DEATH: &[u8; 8] = b"M0DTHTH\0";

/// The fixture's AnimData: the cast, 8 frames at speed 256 with event 2
/// on frame 4; the death, 4 frames, no events.
fn anim_data() -> AnimData {
    let mut a = AnimData {
        buckets: vec![Vec::new(); animdata::BUCKETS],
    };
    for (name, frames, event) in [(CAST, 8, Some(4)), (DEATH, 4, None)] {
        let mut events = [0u8; animdata::EVENTS];
        if let Some(i) = event {
            events[i] = 2;
        }
        let len = name.iter().position(|&b| b == 0).unwrap();
        a.buckets[animdata::hash(&name[..len])].push(AnimRecord {
            name: *name,
            frames,
            speed: 256,
            events,
        });
    }
    a
}

fn with_anim(fx: &mut Fx) {
    let h = fx.sim.hooks();
    h.anim_data = Some(Arc::new(anim_data()));
    h.x.names.insert((UnitType::Player, 10), *CAST);
    h.x.names.insert((UnitType::Monster, 0), *DEATH);
}

/// `units.md` §4.1 then §4.2 main form on the player.
fn animate(fx: &mut Fx, u: UnitId, mode: u32) {
    let s = &mut fx.sim.sys;
    let mut sim = Sim {
        game: &mut fx.game,
        units: &mut s.units,
        stats: &mut s.stats,
        data: &s.data,
    };
    modes::set_mode(&mut sim, &mut s.hooks, u, mode).unwrap();
    modes::animate(&mut sim, &mut s.hooks, u).unwrap();
}

#[test]
fn mode_start_schedules_from_the_animdata_record_of_the_composed_name() {
    let mut fx = Fx::new();
    with_anim(&mut fx);
    let p = fx.spawn(UnitType::Player, 1, fx.a, 10, 10);
    fx.game.frame = 100;
    animate(&mut fx, p, 10);
    // §4.2: s = 256, F = 8 · 256: the event byte 2 of frame 4 → event 0
    // at f + 4, args (2, 0); the end at f + 8.
    assert_eq!(
        fx.timers(p),
        [(event::MODE_CHANGE, 104), (event::END_ANIM, 108)]
    );
    let r = fx.sim.sys.units.get(p).unwrap();
    assert_eq!((r.anim.frame_count, r.anim.speed), (8 * 256, 256));
    assert_eq!(r.anim.record.unwrap().frames, 8);
    fx.assert_clean();
}

#[test]
fn a_name_not_in_the_file_gets_the_default_record() {
    let mut fx = Fx::new();
    with_anim(&mut fx);
    fx.sim
        .hooks()
        .x
        .names
        .insert((UnitType::Player, 7), *b"SOA1HTH\0");
    let p = fx.spawn(UnitType::Player, 1, fx.a, 10, 10);
    animate(&mut fx, p, 7);
    // §3: frames 2048, speed 256, no events → only the end, at
    // f + 2048.
    let r = fx.sim.sys.units.get(p).unwrap();
    assert_eq!(r.anim.record.unwrap().frames, 2048);
    assert_eq!(fx.timers(p), [(event::END_ANIM, 2048)]);
    fx.assert_clean();
}

#[test]
fn without_a_composed_name_there_is_no_record() {
    let mut fx = Fx::new();
    with_anim(&mut fx);
    let p = fx.spawn(UnitType::Player, 1, fx.a, 10, 10);
    let s = &mut fx.sim.sys;
    let mut sim = Sim {
        game: &mut fx.game,
        units: &mut s.units,
        stats: &mut s.stats,
        data: &s.data,
    };
    modes::set_mode(&mut sim, &mut s.hooks, p, 8).unwrap();
    assert!(modes::animate(&mut sim, &mut s.hooks, p).is_err());
}

/// experience.txt: max level 3; thresholds 0, 500, 1500.
fn vitals() -> VitalsTables {
    let row = |v: u32| Experience {
        amazon: v,
        sorceress: v,
        necromancer: v,
        paladin: v,
        barbarian: v,
        druid: v,
        assassin: v,
        ..blank()
    };
    VitalsTables {
        charstats: Vec::new(),
        experience: vec![row(3), row(0), row(500), row(1500)],
    }
}

/// A player owner at (10, 10) (AR 300, level 1) and a monster at (13,
/// 10) (10 life, level 1, 100 experience), the monster's target flags
/// and collision bit set (kind init and movement specs).
fn kill_setup(fx: &mut Fx) -> (UnitId, UnitId) {
    with_anim(fx);
    fx.sim.hooks().vitals = Some(Arc::new(vitals()));
    let p = fx.spawn(UnitType::Player, 1, fx.a, 10, 10);
    let m = fx.spawn(UnitType::Monster, 0, fx.a, 13, 10);
    fx.stats(p, &[(TOHIT, 300), (LEVEL_STAT, 1)]);
    fx.stats(
        m,
        &[
            (LEVEL_STAT, 1),
            (EXPERIENCE, 100),
            (st::MAXHP, 2560),
            (st::HITPOINTS, 2560),
        ],
    );
    fx.sim.sys.units.get_mut(m).unwrap().flags |=
        unit_flag::IS_VALID_TARGET | unit_flag::CAN_BE_ATTACKED;
    fx.mark(m, bits::MONSTER);
    (p, m)
}

fn fire(fx: &mut Fx, owner: UnitId, tx: i32) -> UnitId {
    let p = MissileParams {
        owner: Some(owner),
        origin: Some(owner),
        class: 0,
        flags: param_flags::TARGET_ABSOLUTE,
        target_x: tx,
        target_y: 10,
        ..MissileParams::default()
    };
    let m = fx
        .sim
        .missiles(&mut fx.game, |g, cx| create_missile(g, cx, &p))
        .unwrap()
        .expect("created");
    // The damage setup `0x0059F900` is the skills spec's: set here.
    fx.stats(m, &[(MINDAMAGE, 2560), (MAXDAMAGE, 2560)]);
    m
}

/// `damage.md` §5.2 step 14 (life 0 → result 2), §7.1 (monster will die →
/// kill), §7.2 (the steps in order, the death mode change with the
/// attacker as target), `vitals.md` §4.2 (level factor 256 for equal
/// levels: 100) and §4.3 (no level-up below 500).
#[test]
fn a_killing_missile_runs_the_kill_and_gives_experience() {
    let mut fx = Fx::new();
    let (p, m) = kill_setup(&mut fx);
    fx.seed(p, seed_giving(10));
    fire(&mut fx, p, 13);
    for _ in 0..3 {
        fx.frame();
    }
    assert_eq!(fx.stat(m, st::HITPOINTS), 0);
    assert_eq!(fx.sim.sys.units.get(m).unwrap().mode, monster_mode::DT);
    let (pi, mi) = (p.0, m.0);
    assert_eq!(
        fx.sim.hooks().x.log,
        [
            format!("event 0 Some({mi})"),
            format!("event 11 Some({mi})"),
            format!("event 2 Some({mi})"),
            format!("event 10 Some({mi})"),
            format!("event 9 Some({pi})"),
            format!("reaction {pi} {mi} 0x3"),
            format!("kill PetCredit {mi} {pi}"),
            format!("kill AttackerBookkeeping {mi} {pi}"),
            format!("kill FaceAttacker {mi} {pi}"),
            format!("death start {mi} Some({pi})"),
            format!("kill QuestKill {mi} {pi}"),
            format!("kill BarricadeDoors {mi} {pi}"),
        ]
    );
    // The death animation: 4 frames → end at f + 4 (`units.md` §4.6).
    let f = fx.game.frame;
    assert_eq!(fx.timers(m), [(event::END_ANIM, f + 4)]);
    assert_eq!(fx.sim.hooks().mode_target, None);
    assert_eq!(fx.stat(p, EXPERIENCE), 100);
    fx.assert_clean();
}

/// `hirelings.md` §8 rule 1: the kill (flag 1) queues the killed monster
/// for the hireling host (`ActionHooks::pet_deaths`), before the pet
/// credit seam; a guarded kill queues nothing; without the queue nothing
/// is recorded.
// Covers: specs/world/hirelings.md §8 r1
#[test]
fn the_kill_queues_the_defender_for_the_hireling_host() {
    let mut fx = Fx::new();
    let (p, m) = kill_setup(&mut fx);
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::kill(w, m, p);
    });
    assert_eq!(fx.sim.hooks().pet_deaths, None);

    let mut fx = Fx::new();
    let (p, m) = kill_setup(&mut fx);
    fx.sim.hooks().pet_deaths = Some(Vec::new());
    fx.sim.sys.units.get_mut(m).unwrap().mode = monster_mode::DD;
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::kill(w, m, p);
    });
    assert_eq!(fx.sim.hooks().pet_deaths, Some(vec![]));
    fx.sim.sys.units.get_mut(m).unwrap().mode = 1;
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::kill(w, m, p);
    });
    assert_eq!(fx.sim.hooks().pet_deaths, Some(vec![m]));
}

/// A dead monster is not killed again (§7.2 guard), and an uninterruptible
/// defender only gets `death_delay` (§7.1).
// Covers: specs/combat/damage.md §7.1 r3
#[test]
fn kill_guards() {
    let mut fx = Fx::new();
    let (p, m) = kill_setup(&mut fx);
    fx.sim.sys.units.get_mut(m).unwrap().mode = monster_mode::DD;
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::kill(w, m, p);
    });
    assert!(fx.sim.hooks().x.log.is_empty());
    assert_eq!(fx.stat(p, EXPERIENCE), 0);

    let mut fx = Fx::new();
    let (p, m) = kill_setup(&mut fx);
    fx.sim.with(&mut fx.game, |_, v| {
        v.set_state(m, crate::stats::states::state::UNINTERRUPTABLE as u16, true)
    });
    let mut rec = crate::combat::DamageRecord {
        result: 3,
        ..Default::default()
    };
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::reaction(w, p, m, &mut rec);
    });
    assert_eq!(fx.sim.sys.units.get(m).unwrap().mode, 1);
    let has_92 = fx.sim.sys.stats.has_state(m, 92);
    assert!(has_92);
    assert_eq!(
        fx.sim.hooks().x.log,
        [format!("reaction {} {} 0x3", p.0, m.0)]
    );
}

/// `damage.md` §7.2 step 2: a victim with unit flag 0x04000000 gives no
/// experience, the rest of the kill runs; a player victim in mode 0 / 17
/// stops at the guard, a live one runs steps 1–2 only (no death mode
/// request, no quest parse).
// Covers: specs/combat/damage.md §7.2 r1, §7.2 r2, §7.2 r3; specs/combat/vitals.md §4.4 text
#[test]
fn kill_without_experience_and_player_victims() {
    let mut fx = Fx::new();
    let (p, m) = kill_setup(&mut fx);
    fx.sim.sys.units.get_mut(m).unwrap().flags |=
        crate::wiring::action::reaction::UNIT_FLAG_NO_EXPERIENCE;
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::kill(w, m, p);
    });
    assert_eq!(fx.stat(p, EXPERIENCE), 0);
    assert_eq!(fx.sim.sys.units.get(m).unwrap().mode, monster_mode::DT);
    // A player victim: dead (17) → nothing; alive → no death mode here.
    let mut fx = Fx::new();
    let (p, m) = kill_setup(&mut fx);
    let q = fx.spawn(UnitType::Player, 1, fx.a, 11, 10);
    fx.sim.sys.units.get_mut(q).unwrap().mode = 17;
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::kill(w, q, m);
    });
    assert!(fx.sim.hooks().x.log.is_empty());
    fx.sim.sys.units.get_mut(q).unwrap().mode = 1;
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::kill(w, q, m);
    });
    let (qi, mi) = (q.0, m.0);
    assert_eq!(
        fx.sim.hooks().x.log,
        [format!("kill AttackerBookkeeping {qi} {mi}")]
    );
    assert_eq!(fx.sim.sys.units.get(q).unwrap().mode, 1);
    let _ = p;
}

// ---- the drop ---------------------------------------------------------------------------

/// Gold only (type 4 under misc), TC 1 = one pick of gold.
pub(super) fn drop_tables() -> DropTables {
    let n: usize = 40;
    let words = n.div_ceil(32);
    let mut equiv = EquivMatrix {
        n,
        words,
        bits: vec![0; n * words],
    };
    for i in 1..n {
        equiv.bits[i * words] |= 1;
        equiv.bits[i * words + i / 32] |= 1 << (i % 32);
    }
    let (g, misc) = (usize::from(ty::GOLD), usize::from(ty::MISC));
    equiv.bits[g * words + misc / 32] |= 1 << (misc % 32);
    let mut itemtypes: Vec<Itemtypes> = (0..n)
        .map(|_| {
            let mut t: Itemtypes = blank();
            (t.class, t.staffmods, t.rare) = (0xFF, 0xFF, 1);
            t
        })
        .collect();
    // Gold is always normal quality (itemtypes `Normal`, `treasure.md`
    // §6 step 1).
    itemtypes[g].normal = 1;
    let mut ratio: Itemratio = blank();
    ratio.version = 1;
    let items = ItemTables {
        items: vec![ItemRec {
            code: *b"gld ",
            type_: ty::GOLD as i16,
            level: 1,
            ..ItemRec::default()
        }],
        itemtypes,
        equiv,
        itemratio: vec![ratio],
        valshift: vec![0; 359],
        stat_shift: 6,
        stat_mask: 0x3F,
        ..ItemTables::default()
    };
    let treasure_items = vec![ItemData {
        code: *b"gld ",
        ubercode: [0; 4],
        ultracode: [0; 4],
        version: 0,
        level: 1,
        type_: ty::GOLD,
        type2: 0,
        unique: 0,
        quest: 0,
        spawnable: 1,
    }];
    let tc = |name: &[u8], entries: Vec<TcEntry>, total| TreasureClass {
        name: name.to_vec(),
        group: 0,
        level: 0,
        total_classic: total,
        total_expansion: total,
        picks: 1,
        nodrop: 0,
        mods: [0; 6],
        entries,
    };
    let gold = TcEntry {
        start_classic: 0,
        start_expansion: 0,
        id: 0,
        row: 0,
        flags: 0,
        mods: [0; 6],
    };
    DropTables {
        items,
        tcs: TreasureClasses {
            tcs: vec![tc(b"none", Vec::new(), 0), tc(b"gold", vec![gold], 1)],
            group_offset: 0,
            chest: [None; 45],
            notes: Vec::new(),
        },
        treasure_items,
        superuniques: Vec::new(),
    }
}

/// The start spot as is.
struct Here;

impl FreeSpot for Here {
    fn free_spot(
        &mut self,
        room: Option<RoomId>,
        start: (i32, i32),
        _: (i32, i32),
    ) -> Option<DropSpot> {
        Some(DropSpot {
            room,
            x: start.0,
            y: start.1,
        })
    }
}

/// A monster of class 0 with treasure class 1 in a game with the drop
/// state; returns the drop state and the monster.
fn drop_setup(fx: &mut Fx) -> (DeathDrops, UnitId, UnitId) {
    let mut t = (*fx.sim.hooks().tables).clone();
    let mut m: Monstats = t.combat.monstats[0].clone();
    m.treasureclass1 = 1;
    t.combat.monstats[0] = m;
    fx.sim.hooks().tables = Arc::new(t);
    let p = fx.spawn(UnitType::Player, 1, fx.a, 10, 10);
    let mon = fx.spawn(UnitType::Monster, 0, fx.a, 13, 10);
    fx.stats(mon, &[(LEVEL_STAT, 3)]);
    let d = DeathDrops::new(
        Arc::new(drop_tables()),
        GameFields::new(Seed::init(), false),
    );
    (d, p, mon)
}

fn run_drop(fx: &mut Fx, d: &mut DeathDrops, mon: UnitId, p: UnitId) -> Vec<UnitId> {
    let s = &mut fx.sim.sys;
    let mut sim = Sim {
        game: &mut fx.game,
        units: &mut s.units,
        stats: &mut s.stats,
        data: &s.data,
    };
    monster_death_drop(&mut s.hooks, &mut sim, d, &mut Here, mon, Some(p))
}

/// `treasure.md` §3.1–§3.5, §7, §8: the walk on the monster's seed picks
/// the gold (one draw: `roll(1)`), the item is created on the game seed
/// (allocation and item seeds: two steps) at the start spot (x + 2,
/// y + 3, in the same room) and added to the room's units, in mode 3.
#[test]
fn a_dead_monster_drops_a_real_item_into_its_room() {
    let mut fx = Fx::new();
    let (mut d, p, mon) = drop_setup(&mut fx);
    let mut want_mon = fx.sim.sys.units.get(mon).unwrap().seed;
    want_mon.roll(1);
    let mut want_game = fx.sim.hooks().game_seed;
    want_game.step();
    want_game.step();
    let out = run_drop(&mut fx, &mut d, mon, p);
    assert!(
        d.failures.is_empty() && d.errors.is_empty(),
        "{:?} {:?}",
        d.failures,
        d.errors
    );
    assert_eq!(out.len(), 1);
    let item = out[0];
    let a = fx.a;
    assert_eq!(
        d.placed,
        [(
            item,
            DropSpot {
                room: Some(a),
                x: 15,
                y: 13
            }
        )]
    );
    assert_eq!(fx.game.lists.unit(item).unwrap().room(), Some(a));
    assert!(fx.game.lists.units_of_type(UnitType::Item).contains(&item));
    let r = fx.sim.sys.units.get(item).unwrap();
    assert_eq!((r.ty, r.class, r.mode), (UnitType::Item, 0, 3));
    assert_eq!(fx.sim.sys.hooks.items.get(item).unwrap().ilvl, 3);
    assert!(fx.stat(item, GOLD_STAT) > 0);
    assert_eq!(fx.sim.sys.units.get(mon).unwrap().seed, want_mon);
    assert_eq!(fx.sim.hooks().game_seed, want_game);
    assert_eq!(d.fields.seed, want_game);
    fx.assert_clean();
}

/// §3.1: unit flag 0x20000 or a wall / door bit at the position → no
/// drop, no draw.
#[test]
fn the_gate_stops_the_drop() {
    for case in 0..2 {
        let mut fx = Fx::new();
        let (mut d, p, mon) = drop_setup(&mut fx);
        if case == 0 {
            fx.sim.sys.units.get_mut(mon).unwrap().flags |= 0x20000;
        } else {
            fx.mark(mon, bits::DOOR);
        }
        let seed = fx.sim.sys.units.get(mon).unwrap().seed;
        assert!(run_drop(&mut fx, &mut d, mon, p).is_empty());
        assert!(d.placed.is_empty());
        assert_eq!(fx.sim.sys.units.get(mon).unwrap().seed, seed);
        assert!(fx.game.lists.units_of_type(UnitType::Item).is_empty());
    }
}

/// The drop fixture with the path provider on and the synthetic
/// walk-back field (`path::search` tests: vectors F1–F3) loaded.
fn drop_setup_paths(fx: &mut Fx) -> (DeathDrops, UnitId, UnitId) {
    let h = fx.sim.hooks();
    h.enable_paths().expect("embedded tables");
    h.paths.as_mut().unwrap().field = Some(Arc::new(crate::path::search::tests::sign_field()));
    drop_setup(fx)
}

/// Sets collision bits on the cell (x, y) of room A's grid.
fn set_cell(fx: &mut Fx, x: i32, y: i32, bit: u16) {
    let (a, game) = (fx.a, &fx.game);
    *fx.sim
        .sys
        .hooks
        .drlg
        .collision_mut(game, a, x, y)
        .expect("in a grid") |= bit;
}

fn spot_of(fx: &mut Fx, d: &mut DeathDrops, mon: UnitId, p: UnitId) -> DropSpot {
    let before = d.placed.len();
    let out = run_drop(fx, d, mon, p);
    assert_eq!(out.len(), 1, "{:?} {:?}", d.failures, d.errors);
    assert_eq!(d.placed.len(), before + 1);
    d.placed[before].1
}

/// `treasure.md` §7 step 2 on the floor drop (`path-placement.md` §9,
/// vector D2 translated by (+3, 0)): the monster at (13, 10) by its path
/// record (the `Pending` position is not set with the provider on), an
/// empty room → the start (15, 13). The item gets its static path at
/// the spot and its footprint (0x200).
#[test]
fn with_the_path_provider_the_drop_lands_on_the_floor_drop_spot() {
    let mut fx = Fx::new();
    let (mut d, p, mon) = drop_setup_paths(&mut fx);
    assert_eq!(fx.sim.hooks().path_position(mon), (13, 10));
    let a = fx.a;
    let spot = spot_of(&mut fx, &mut d, mon, p);
    assert_eq!(
        spot,
        DropSpot {
            room: Some(a),
            x: 15,
            y: 13
        }
    );
    let item = d.placed[0].0;
    assert!(matches!(
        fx.sim.hooks().paths.as_ref().unwrap().record(item),
        Some(crate::path::UnitPath::Static(_))
    ));
    assert_eq!(fx.sim.hooks().path_position(item), (15, 13));
    let cell = |fx: &mut Fx, x, y| {
        crate::path::collision::point_value(&fx.sim.hooks().drlg, Some(a), x, y, 0xFFFF)
    };
    assert_ne!(cell(&mut fx, 15, 13) & bits::ITEM, 0);
    fx.assert_clean();
}

/// Vector D1 translated: a wall column at x = 15 (the start's column)
/// → ring 1: (14, 12) d 2 kept, (16, *) fail the walk-back through the
/// wall, (14, 13) d 1 wins. Vector D3 translated: only an item bit
/// (0x200) at the start → (14, 13). M08: the same drop without the
/// blocked cells lands on the start (previous test).
#[test]
fn the_floor_drop_avoids_blocked_cells() {
    for case in 0..2 {
        let mut fx = Fx::new();
        let (mut d, p, mon) = drop_setup_paths(&mut fx);
        if case == 0 {
            for y in 0..40 {
                set_cell(&mut fx, 15, y, bits::WALL);
            }
        } else {
            set_cell(&mut fx, 15, 13, bits::ITEM);
        }
        let a = fx.a;
        assert_eq!(
            spot_of(&mut fx, &mut d, mon, p),
            DropSpot {
                room: Some(a),
                x: 14,
                y: 13
            },
            "case {case}"
        );
        fx.assert_clean();
    }
}

/// §7 step 2 "each seeing the previous ones": the first drop's item
/// footprint (0x200, in mask 0x3E01) blocks the start for the next, which
/// lands as in vector D3 (14, 13).
#[test]
fn a_dropped_item_blocks_the_next_drop() {
    let mut fx = Fx::new();
    let (mut d, p, mon) = drop_setup_paths(&mut fx);
    let a = fx.a;
    let first = spot_of(&mut fx, &mut d, mon, p);
    let second = spot_of(&mut fx, &mut d, mon, p);
    assert_eq!((first.x, first.y), (15, 13));
    assert_eq!(
        second,
        DropSpot {
            room: Some(a),
            x: 14,
            y: 13
        }
    );
    fx.assert_clean();
}

/// The provider on without the walk-back field: the drop keeps the
/// [`FreeSpot`] seam (here: the start as is), and the item is put there
/// (`treasure.md` §7 step 4: its static path at the spot).
#[test]
fn without_the_field_the_drop_keeps_the_free_spot_seam() {
    let mut fx = Fx::new();
    fx.sim.hooks().enable_paths().expect("embedded tables");
    let (mut d, p, mon) = drop_setup(&mut fx);
    // A wall at the start: the seam answers the start regardless.
    set_cell(&mut fx, 15, 13, bits::WALL);
    let spot = spot_of(&mut fx, &mut d, mon, p);
    assert_eq!((spot.x, spot.y), (15, 13));
    let item = d.placed[0].0;
    assert_eq!(fx.sim.hooks().path_position(item), (15, 13));
    fx.assert_clean();
}

fn run_quest_drop(
    fx: &mut Fx,
    d: &mut DeathDrops,
    unit: UnitId,
    code: [u8; 4],
    quality: u8,
) -> Option<UnitId> {
    let s = &mut fx.sim.sys;
    let mut sim = Sim {
        game: &mut fx.game,
        units: &mut s.units,
        stats: &mut s.stats,
        data: &s.data,
    };
    crate::wiring::economy::unit_quest_drop(
        &mut s.hooks,
        &mut sim,
        d,
        &mut Here,
        unit,
        Some(code),
        quality,
        -1,
        0,
    )
}

/// `treasure.md` §9 on the action wiring: the drop code names the class
/// (no draw on the unit seed); item level = the monster's level stat
/// (rule 2); the request (rule 5: spawn mode 3, init flags 1, quality)
/// creates a real item at the floor-drop start spot (rule 4, §7 rule 2),
/// on two game-seed steps.
// Covers: specs/items/treasure.md §9 r2, §9 r3, §9 r4, §9 r5
#[test]
fn the_quest_drop_creates_the_drop_code_item_at_the_unit() {
    let mut fx = Fx::new();
    let (mut d, _p, mon) = drop_setup(&mut fx);
    let want_mon = fx.sim.sys.units.get(mon).unwrap().seed;
    let mut want_game = fx.sim.hooks().game_seed;
    want_game.step();
    want_game.step();
    let item = run_quest_drop(&mut fx, &mut d, mon, *b"gld ", 2).expect("item");
    assert!(d.failures.is_empty() && d.errors.is_empty());
    let a = fx.a;
    assert_eq!(
        d.placed,
        [(
            item,
            DropSpot {
                room: Some(a),
                x: 15,
                y: 13
            }
        )]
    );
    let r = fx.sim.sys.units.get(item).unwrap();
    assert_eq!((r.ty, r.class, r.mode), (UnitType::Item, 0, 3));
    assert_eq!(fx.sim.sys.hooks.items.get(item).unwrap().ilvl, 3);
    assert_eq!(fx.sim.sys.units.get(mon).unwrap().seed, want_mon);
    assert_eq!(fx.sim.hooks().game_seed, want_game);
    fx.assert_clean();
}

/// §9 rule 3: a drop code missing from the items table is fatal 0x9EA
/// (logged; no item, no draw).
// Covers: specs/items/treasure.md §9 r3
#[test]
fn the_quest_drop_with_an_unknown_code_is_fatal() {
    let mut fx = Fx::new();
    let (mut d, _p, mon) = drop_setup(&mut fx);
    let game = fx.sim.hooks().game_seed;
    assert_eq!(run_quest_drop(&mut fx, &mut d, mon, *b"zzz ", 2), None);
    // The one `0x00559A30` (`drop_helpers::source_drop`) records its pick
    // fatals in `pick_errors`.
    assert_eq!(
        d.pick_errors,
        [crate::treasure::class_pick::PickError::Code(
            u32::from_le_bytes(*b"zzz ")
        )]
    );
    assert!(d.errors.is_empty());
    assert!(d.placed.is_empty());
    assert_eq!(fx.sim.hooks().game_seed, game);
}

/// `damage.md` §7.1 step 4.6: a get-hit result the get-hit test lets
/// through puts the monster into get-hit (mode 3, toward the attacker);
/// a soft result only sets unit flag 0x8000 and queues the unit (step
/// 4.7). A dead monster is left alone.
// Covers: specs/combat/damage.md §7.1 r4
#[test]
fn a_hit_puts_the_monster_into_get_hit_or_marks_it_soft() {
    use crate::combat::result;
    let mut fx = Fx::new();
    let (p, m) = kill_setup(&mut fx);
    let mut rec = crate::combat::DamageRecord {
        result: result::HIT | result::GET_HIT,
        total: 2000,
        ..Default::default()
    };
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::reaction(w, p, m, &mut rec);
    });
    assert_eq!(fx.sim.sys.units.get(m).unwrap().mode, monster_mode::GH);
    assert_eq!(fx.sim.hooks().mode_target, None);

    // Too small a hit: no get-hit, the soft path.
    let mut fx = Fx::new();
    let (p, m) = kill_setup(&mut fx);
    let mut rec = crate::combat::DamageRecord {
        result: result::HIT | result::GET_HIT,
        total: 100,
        ..Default::default()
    };
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::reaction(w, p, m, &mut rec);
    });
    let r = fx.sim.sys.units.get(m).unwrap();
    assert_eq!(r.mode, 1);
    assert_ne!(r.flags & 0x8000, 0, "soft hit");

    // A soft result alone; a dead monster is unchanged.
    let mut fx = Fx::new();
    let (p, m) = kill_setup(&mut fx);
    let mut rec = crate::combat::DamageRecord {
        result: result::HIT | result::SOFT_HIT,
        total: 100,
        ..Default::default()
    };
    fx.sim.sys.units.get_mut(m).unwrap().mode = monster_mode::DD;
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::reaction(w, p, m, &mut rec);
    });
    assert_eq!(fx.sim.sys.units.get(m).unwrap().flags & 0x8000, 0);
    fx.sim.sys.units.get_mut(m).unwrap().mode = 1;
    fx.sim.combat(&mut fx.game, |w, _| {
        crate::wiring::action::reaction::reaction(w, p, m, &mut rec);
    });
    assert_ne!(fx.sim.sys.units.get(m).unwrap().flags & 0x8000, 0);
}
