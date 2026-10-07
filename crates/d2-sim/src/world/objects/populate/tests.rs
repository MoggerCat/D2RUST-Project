// Spec: specs/world/object-population.md (Test vectors, Edge cases)
use super::super::fake::{blank_level, blank_object, blank_shrine, Call, Fake};
use super::*;
use d2_data::tables::Record;

const ROOM: RoomId = RoomId(3);
/// The populated level of the fixture room.
const LEVEL: u32 = 2;

impl PopulateWorld for Fake {
    fn room_seed(&mut self, _room: RoomId) -> Option<&mut Seed> {
        self.seeds.get_mut(&UnitId(u32::MAX))
    }
    fn populated_room_count(&mut self, act: u8, level: u32) -> i32 {
        self.calls.push(Call::Other(format!("count {act} {level}")));
        self.stats
            .get(&(UnitId(u32::MAX), 0))
            .copied()
            .unwrap_or(20)
    }
    fn box_query(&self, _: RoomId, _: i32, _: i32, _: u32, _: u32, _: u32) -> u32 {
        // Blocked everywhere when stat (MAX, 1) is set.
        self.stats.get(&(UnitId(u32::MAX), 1)).copied().unwrap_or(0) as u32
    }
    fn set_unit_class(&mut self, unit: UnitId, class: u16) {
        self.calls
            .push(Call::Other(format!("class {} {class}", unit.0)));
    }
}

fn room_seed(f: &mut Fake) -> &mut Seed {
    f.seeds.get_mut(&UnitId(u32::MAX)).unwrap()
}

struct Fx {
    ctl: ObjectControl,
    t: ObjectTables,
    f: Fake,
    info: RoomInfo,
}

fn blank_group() -> Objgroup {
    Objgroup::decode(&[0; Objgroup::SIZE])
}

/// Level 2 with slot 0 = group 1 (`ObjPrb` `prb`); group 1 members
/// (class 10, PROB 30) and (class 11, PROB 70), both `PopulateFn` 3 with
/// density 32; a 40 × 40 room at (100, 100); room seed {1, 666}.
fn fx(prb: u8) -> Fx {
    let mut objects = vec![blank_object(); 120];
    for c in [10, 11] {
        objects[c].populatefn = 3;
        objects[c].sizex = 1;
        objects[c].sizey = 1;
    }
    let mut levels = vec![blank_level(); 4];
    levels[LEVEL as usize].objgrp0 = 1;
    levels[LEVEL as usize].objprb0 = prb;
    let mut g = blank_group();
    (g.id0, g.density0, g.prob0) = (10, 32, 30);
    (g.id1, g.density1, g.prob1) = (11, 32, 70);
    let mut shrines = vec![blank_shrine(); 3];
    shrines[2].effectclass = 2;
    shrines[2].levelmin = 1;
    let t = ObjectTables {
        objects,
        shrines,
        levels,
        objgroup: vec![blank_group(), g],
    };
    let (ctl, _) = ObjectControl::new(&mut Seed::new(7, 666), &t);
    let mut f = Fake {
        next_alloc: Some(100),
        ..Fake::default()
    };
    f.seeds.insert(UnitId(u32::MAX), Seed::new(1, 666));
    Fx {
        ctl,
        t,
        f,
        info: RoomInfo {
            room: ROOM,
            level: LEVEL,
            populated: LEVEL,
            waypoint: false,
            dirt_path: false,
            rect: (100, 100, 40, 40),
        },
    }
}

fn run_room(x: &mut Fx) -> Populated {
    populate_room(&mut x.ctl, &x.t, &mut x.f, &x.info).unwrap()
}

fn allocs(f: &Fake) -> Vec<u16> {
    f.calls
        .iter()
        .filter_map(|c| match c {
            Call::Allocate(_, class, ..) => Some(*class),
            _ => None,
        })
        .collect()
}

/// Covers: object-population.md Test vectors rows 1 and 2 (§5).
#[test]
fn slot_roll_and_member_pick() {
    let mut x = fx(60);
    let out = run_room(&mut x);
    // r = 51 ≤ 60; r2 = 31: PROB {30, 70} → member 1 (class 11).
    assert!(!out.objects.is_empty());
    assert!(allocs(&x.f).iter().all(|&c| c == 11));

    let mut x = fx(50);
    let out = run_room(&mut x);
    assert!(out.objects.is_empty());
    // One R step per slot (8), no member draw for slot 0.
    let mut expect = Seed::new(1, 666);
    assert_eq!(expect.step() % 100, 51);
    let after_one = expect;
    assert_eq!(after_one, Seed::new(1_791_398_751, 0));
    for _ in 1..8 {
        expect.step();
    }
    assert_eq!(*room_seed(&mut x.f), expect);
}

/// Covers: object-population.md §2 (want health, caps, spacing vectors).
#[test]
fn region_tests() {
    let mut r = Region::new(0);
    (r.w08, r.counted) = (20, 16);
    assert!(want_health(&r));
    r.counted = 15;
    assert!(!want_health(&r));
    r.health = 1;
    r.counted = 16;
    assert!(!want_health(&r));
    r.w08 = 0;
    assert!(!want_health(&r));

    let mut r = Region::new(0);
    r.w08 = 30;
    r.shrines = 3;
    assert!(!shrine_cap(&r));
    r.shrines = 4;
    assert!(shrine_cap(&r));
    r.shrines = 10;
    r.w08 = 1000;
    assert!(shrine_cap(&r));
    r.wells = 4;
    assert!(well_cap(&r));

    let mut r = Region::new(0);
    record_shrine(&mut r, (100, 100));
    assert!(!shrine_spaced(&r, 140, 300), "dx 40 < 50");
    assert!(shrine_spaced(&r, 160, 160));
    // Edge case 4: a cross, not a box.
    assert!(!shrine_spaced(&r, 600, 100));
    record_well(&mut r, (100, 100));
    assert!(!well_spaced(&r, 199, 500));
    assert!(well_spaced(&r, 200, 200));
    // Record stops at the cap.
    for i in 0..20 {
        record_shrine(&mut r, (i, i));
    }
    assert_eq!(r.shrines, 10);
}

/// Covers: object-population.md Test vectors (theme gate rows), §4 and
/// edge case 2.
#[test]
fn theme_gate_draws_and_never_runs() {
    let mut x = fx(0);
    x.ctl.seed = Seed::new(1, 666);
    {
        let r = region(&mut x.ctl, LEVEL).unwrap();
        (r.w08, r.counted) = (20, 15);
    }
    assert!(!theme_gate(&mut x.ctl, LEVEL, 60).unwrap());
    assert_eq!(x.ctl.seed, Seed::new(1_791_398_751, 0), "one C draw");
    // r < p + 12: every pick of {3, 4, 5, 6} fails (t = 3 inactive, others ≥ 4).
    for lo in 0..200u32 {
        let mut s = Seed::new(lo, 0);
        let mut probe = s;
        if probe.step() % 100 >= 22 {
            continue;
        }
        x.ctl.seed = s;
        {
            let r = region(&mut x.ctl, LEVEL).unwrap();
            (r.w08, r.counted) = (20, 19);
        }
        assert!(!theme_gate(&mut x.ctl, LEVEL, 60).unwrap());
        s.step();
        s.roll(4);
        assert_eq!(x.ctl.seed, s, "two C draws");
    }
}

/// Covers: object-population.md Test vectors (count row), §7.
#[test]
fn count_formula() {
    let x = fx(0);
    let mut out = Populated::default();
    let mut f = Fake::default();
    let mut ctl = x.ctl.clone();
    let cx = Cx {
        ctl: &mut ctl,
        t: &x.t,
        w: &mut f,
        info: &x.info,
        out: &mut out,
    };
    assert_eq!(cx.count(32), 1);
    assert_eq!(cx.count(125), 5);
}

fn cx_run(x: &mut Fx, f: impl FnOnce(&mut Cx<'_, Fake>) -> Result<(), ObjectError>) -> Populated {
    let mut out = Populated::default();
    let mut cx = Cx {
        ctl: &mut x.ctl,
        t: &x.t,
        w: &mut x.f,
        info: &x.info,
        out: &mut out,
    };
    f(&mut cx).unwrap();
    out
}

/// Covers: object-population.md Test vectors (fn 4 row), §7.4.
#[test]
fn fn4_first_class() {
    let mut x = fx(0);
    x.t.objects[7].sizex = 1;
    x.t.objects[7].sizey = 1;
    x.info.rect = (100, 100, 1000, 1000);
    x.ctl.seed = Seed::new(1, 666);
    let mut probe = Seed::new(1, 666);
    assert_eq!(probe.step() % 100, 51);
    assert_eq!(probe.step(), 791_599_131);
    cx_run(&mut x, |c| c.fn4(32));
    assert_eq!(allocs(&x.f).first(), Some(&11));
    assert!(allocs(&x.f).len() <= 8, "at most 8 per call");
}

/// Covers: object-population.md Test vectors (fn 7 row), edge case 3.
#[test]
fn fn7_places_nothing() {
    let mut x = fx(0);
    x.info.rect = (100, 100, 400, 400);
    let out = cx_run(&mut x, |c| c.fn7(10));
    assert!(out.objects.is_empty());
    assert!(allocs(&x.f).is_empty());
    assert_ne!(x.ctl.seed, fx(0).ctl.seed, "draws still happen");
}

/// Covers: object-population.md §3 (pre-check order, counters).
#[test]
fn precheck_skips_and_counts() {
    for (wp, dirt, pop) in [
        (true, false, LEVEL),
        (false, true, LEVEL),
        (false, false, 0),
    ] {
        let mut x = fx(90);
        x.info.waypoint = wp;
        x.info.dirt_path = dirt;
        x.info.populated = pop;
        let before = *room_seed(&mut x.f);
        assert!(run_room(&mut x).objects.is_empty());
        assert_eq!(*room_seed(&mut x.f), before, "no slot draws");
    }
    // A town level skips too.
    let mut x = fx(90);
    x.t.levels = vec![blank_level(); 50];
    x.t.levels[40].objgrp0 = 1;
    x.info.level = 40;
    x.info.populated = 40;
    x.ctl = ObjectControl::new(&mut Seed::new(7, 666), &x.t).0;
    let before = *room_seed(&mut x.f);
    run_room(&mut x);
    assert_eq!(*room_seed(&mut x.f), before);

    // Total set once from the room count; counted increments per room.
    let mut x = fx(0);
    run_room(&mut x);
    run_room(&mut x);
    let r = *region(&mut x.ctl, LEVEL).unwrap();
    assert_eq!((r.w08, r.counted), (20, 2));
    let counts =
        x.f.calls_of(|c| matches!(c, Call::Other(s) if s.starts_with("count")));
    assert_eq!(counts.len(), 1);
}

/// Covers: object-population.md edge case 7 (missing group ends the room).
#[test]
fn missing_group_ends_room() {
    let mut x = fx(100);
    x.t.levels[LEVEL as usize].objgrp0 = 9;
    x.t.levels[LEVEL as usize].objgrp1 = 1;
    x.t.levels[LEVEL as usize].objprb1 = 100;
    run_room(&mut x);
    let mut expect = Seed::new(1, 666);
    expect.step();
    assert_eq!(*room_seed(&mut x.f), expect, "only slot 0's draw");
}

/// Covers: object-population.md §5 rule 3, edge case 1.
#[test]
fn want_health_suppresses_subclass_rows() {
    let mut x = fx(99);
    x.t.objects[1].subclass = 8;
    {
        let r = region(&mut x.ctl, LEVEL).unwrap();
        (r.w08, r.counted) = (20, 16);
    }
    run_room(&mut x);
    assert!(allocs(&x.f).is_empty(), "r = 100 > ObjPrb");
}

/// Covers: object-population.md §7.2 rule 4.3, edge case 5.
#[test]
fn fn2_forced_health_shrine() {
    let mut x = fx(0);
    x.t.objects[30].parm1 = 77;
    x.t.objects[30].sizex = 1;
    x.t.objects[30].sizey = 1;
    {
        let r = region(&mut x.ctl, LEVEL).unwrap();
        (r.w08, r.counted) = (20, 16);
    }
    let mut got = None;
    cx_run(&mut x, |c| {
        got = c.fn2(30)?;
        Ok(())
    });
    let u = got.expect("placed");
    let d = x.ctl.get(u).unwrap();
    assert_eq!((d.interact, d.shrine, d.class), (2, Some(2), 30));
    let r = *region(&mut x.ctl, LEVEL).unwrap();
    assert_eq!((r.health, r.shrines), (1, 1));
    assert_eq!(r.shrine_points[0], x.f.positions[&u]);
    assert!(x
        .f
        .calls
        .contains(&Call::Other(format!("class {} 77", u.0))));
}

/// Covers: object-population.md §7.8 rule 1 (cap: no draw).
#[test]
fn fn8_cap_no_draw() {
    let mut x = fx(0);
    region(&mut x.ctl, LEVEL).unwrap().wells = 4;
    let before = *room_seed(&mut x.f);
    let c0 = x.ctl.seed;
    cx_run(&mut x, |c| c.fn8(10, 30).map(drop));
    assert_eq!(*room_seed(&mut x.f), before);
    assert_eq!(x.ctl.seed, c0);
}

/// Covers: object-population.md §6 (blocked collision: no placement).
#[test]
fn blocked_room_places_nothing() {
    let mut x = fx(100);
    x.f.stats.insert((UnitId(u32::MAX), 1), 1);
    assert!(run_room(&mut x).objects.is_empty());
}

/// Covers: object-population.md §5 rule 6 (fatal paths).
#[test]
fn fatal_populate_fn_and_density() {
    let mut x = fx(100);
    x.t.objects[11].populatefn = 10;
    assert_eq!(
        populate_room(&mut x.ctl, &x.t, &mut x.f, &x.info),
        Err(ObjectError::PopulateFn(10))
    );
    let mut x = fx(100);
    x.t.objgroup[1].density1 = 128;
    assert_eq!(
        populate_room(&mut x.ctl, &x.t, &mut x.f, &x.info),
        Err(ObjectError::Density(128))
    );
}
