// Spec: specs/missiles/bodies.md, specs/missiles/bodies-2.md
//! The bodies of `bodies_ext` / `bodies_ext2` on the missile fake: the
//! specs' test vectors on synthetic rows, the seams scripted by `Bodies`.

use super::*;
use crate::missiles::bodies_ext::{chaos_turn, fire_disc, nova_offsets};
use crate::missiles::bodies_ext2::{scatter_at_target, ORB_C, ORB_S};

/// A world with one live missile of row 0 (`r`, at the owner's (100,
/// 100), skill 7, level 10) and optional rows from 1.
fn world(r: MissileRow, extra: &[MissileRow]) -> (World, UnitId) {
    let mut w = World::new(r);
    w.tables.extend_from_slice(extra);
    let mut p = w.params();
    p.level = 10;
    p.skill = 7;
    let m = w.create(&p).expect("missile");
    w.fake.log.clear();
    w.fake.calls.clear();
    (w, m)
}

/// A plain sub-missile row.
fn sub() -> MissileRow {
    let mut r = row();
    r.psrvdofunc = 0;
    r
}

macro_rules! cx {
    ($w:expr) => {
        Ctx {
            tables: &$w.tables,
            store: &mut $w.store,
            world: &mut $w.fake,
        }
    };
}

fn srv_do(w: &mut World, i: i16, m: UnitId) -> i32 {
    let mut cx = cx!(w);
    catalogue::run_srv_do(&mut w.game, &mut cx, i, m)
}

fn srv_hit(w: &mut World, i: i16, m: UnitId, u: Option<UnitId>) -> i32 {
    let mut cx = cx!(w);
    catalogue::run_srv_hit(&mut w.game, &mut cx, i, m, u, 0)
}

/// Missiles other than `m`, in allocation (slot) order.
fn others(w: &World, m: UnitId) -> Vec<UnitId> {
    w.store.missiles().filter(|&u| u != m).collect()
}

fn tpoint(w: &World, u: UnitId) -> (i32, i32) {
    w.fake.paths[&u].target_point.expect("target point")
}

fn set_frames(w: &mut World, m: UnitId, total: i16, current: i16) {
    let d = w.store.get_mut(m).unwrap();
    d.total = total;
    d.current = current;
}

fn set_data(w: &mut World, m: UnitId, a: i32, b: i32) {
    w.store.get_mut(m).unwrap().target = (a, b);
}

fn guid(w: &World, u: UnitId) -> i32 {
    w.game.lists.unit(u).unwrap().guid as i32
}

/// A skill 7 with (calc1, calc2, aurarange, auralen, calc4).
fn skill(w: &mut World, v: [i32; 5]) {
    w.fake.mb.skills.insert(7, v);
}

fn field(w: &mut World, code: u8, v: i32) {
    w.fake.mb.fields.insert((7, code), v);
}

// ---------------------------------------------------------------- §1–§5

// Covers: specs/missiles/bodies.md §1 r2, §1 r3, §1 r4, §edge-cases-original-bugs r2
#[test]
fn cairn_stones_opens_the_portal_once() {
    let mut r = row();
    (r.param1, r.param2, r.param3, r.param4, r.param5) = (40, 2, 17, 38, 100);
    r.range = 300;
    r.submissile1 = 0xFFFF;
    let (mut w, m) = world(r, &[]);
    set_frames(&mut w, m, 300, 150);
    srv_do(&mut w, 17, m);
    assert_eq!(w.fake.logged("portal"), 0, "150 > 140");
    set_frames(&mut w, m, 300, 140);
    srv_do(&mut w, 17, m);
    assert!(w.fake.log.contains(&"portal (100, 100) 38 60".to_string()));
    assert_eq!(w.store.get(m).unwrap().target.0, 1);
    assert_eq!(w.fake.logged("refresh"), 1);
    srv_do(&mut w, 17, m);
    assert_eq!(w.fake.logged("portal"), 1);
    // Server-hit 32 at expiry: the portal is already open.
    assert_eq!(srv_hit(&mut w, 32, m, None), 0);
    assert_eq!(w.fake.logged("portal"), 1);
}

// Covers: specs/missiles/bodies-2.md §42 r1, §42 r2
#[test]
fn cairn_stones_expiry_opens_the_portal() {
    let mut r = row();
    r.param4 = 38;
    let (mut w, m) = world(r, &[]);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 32, m, Some(mon)), 0);
    assert_eq!(w.fake.logged("portal"), 0);
    assert_eq!(srv_hit(&mut w, 32, m, None), 0);
    assert_eq!(w.fake.logged("portal"), 1);
}

// Covers: specs/missiles/bodies.md §2 r1, §2 r2, §2 r3, §2 r4, §2 r5
#[test]
fn volcano_throws_on_its_interval() {
    let mut r = row();
    (r.param1, r.param2, r.param3, r.param4, r.param5) = (0, 0, 2, 128, 30);
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    skill(&mut w, [0, 0, 4, 0, 3]);
    set_data(&mut w, m, 77, 0);
    // e = 3: 2 < 3 < 128 and 3 mod max(calc4 3, 1) = 0.
    set_frames(&mut w, m, 50, 47);
    srv_do(&mut w, 28, m);
    let mut s = Seed::init_low(77);
    let dx = s.roll(9) as i32 - 4;
    let dy = s.roll(9) as i32 - 4;
    let n = others(&w, m);
    assert_eq!(n.len(), 1);
    assert_eq!(tpoint(&w, n[0]), (100 + dx, 100 + dy));
    assert_eq!(w.store.get(m).unwrap().target.0, s.lo as i32);
    // e = 4: not on the interval.
    set_frames(&mut w, m, 50, 46);
    srv_do(&mut w, 28, m);
    assert_eq!(others(&w, m).len(), 1);
    // No skill record: removed.
    w.fake.mb.skills.clear();
    assert_eq!(srv_do(&mut w, 28, m), 2);
}

// Covers: specs/missiles/bodies.md §3 r1, §3 r2, §3 r3, §3 r4, §3 r5, §3 r6
#[test]
fn baal_taunt_control_picks_a_slot() {
    let mut r = row();
    (r.param1, r.param2, r.param3, r.param4) = (25, 3, 45, 0);
    r.submissile1 = 1;
    r.submissile2 = 1;
    r.submissile3 = 0xFFFF;
    let (mut w, m) = world(r.clone(), &[sub()]);
    // Elapsed 24: body 3 only, no draw.
    set_frames(&mut w, m, 100, 76);
    let before = *w.fake.seed(m);
    srv_do(&mut w, 34, m);
    assert_eq!(*w.fake.seed(m), before);
    assert_eq!(others(&w, m).len(), 0);
    // Elapsed 90 = lcm(3, 45) × 1: either slot fires.
    set_frames(&mut w, m, 100, 10);
    srv_do(&mut w, 34, m);
    assert_eq!(*w.fake.seed(m), {
        let mut s = Seed::init_low(100);
        s.step();
        s
    });
    assert_eq!(others(&w, m).len(), 1);
    // No usable slot: removed.
    let mut r0 = r;
    r0.param2 = 0;
    let (mut w, m) = world(r0, &[sub()]);
    assert_eq!(srv_do(&mut w, 34, m), 2);
}

// Covers: specs/missiles/bodies.md §4 r1, §4 r2, §4 r3, §4 r4, §4 r5
#[test]
fn chaos_ice_turns() {
    assert_eq!(chaos_turn(8, 0, true), (8, 2));
    assert_eq!(chaos_turn(8, 0, false), (8, -2));
    assert_eq!(chaos_turn(1, 1, true), (1, 1));
    let mut r = row();
    r.param1 = 3;
    let (mut w, m) = world(r, &[]);
    set_data(&mut w, m, 1234, 8);
    set_frames(&mut w, m, 50, 47);
    srv_do(&mut w, 35, m);
    let mut s = Seed::init_low(1234);
    let bit = s.step() & 1 != 0;
    let (a, b) = chaos_turn(8, 0, bit);
    assert_eq!(tpoint(&w, m), (100 + a, 100 + b));
    assert!(w.fake.calls.contains(&"build".to_string()));
    assert_eq!(
        w.store.get(m).unwrap().target,
        (s.lo as i32, (b << 16) + (a & 0xFFFF))
    );
    // Off the interval: nothing.
    set_frames(&mut w, m, 50, 46);
    w.fake.calls.clear();
    srv_do(&mut w, 35, m);
    assert!(!w.fake.calls.contains(&"build".to_string()));
}

// Covers: specs/missiles/bodies.md §5 r1, §5 r2, §5 r3, §5 r4
#[test]
fn baal_taunt_lightning_scatter() {
    let mut r = row();
    r.shitpar1 = 10;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    assert_eq!(srv_hit(&mut w, 58, m, None), 1);
    let mut s = Seed::init_low(100);
    let tx = 100 - 10 + s.roll(21) as i32;
    let ty = 100 - 10 + s.roll(21) as i32;
    let n = others(&w, m);
    assert_eq!(n.len(), 1);
    assert_eq!(tpoint(&w, n[0]), (tx, ty));
}

// ---------------------------------------------------------------- §6–§11

// Covers: specs/missiles/bodies.md §6 r1, §6 r2, §6 r3, §6 l2 r1, §6 l2 r2, §6 l2 r3, §6 l2 r4, §6 l2 r5, §6 l2 r6
#[test]
fn plague_ring() {
    let mut r = row();
    (r.shitpar1, r.shitpar2, r.shitpar3) = (1, 2, 3);
    r.hitsubmissile1 = 1;
    let mut s = sub();
    (s.param1, s.param2) = (2, 4);
    let (mut w, m) = world(r.clone(), &[s.clone()]);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 2, m, Some(mon)), 3);
    let n = others(&w, m);
    assert_eq!(n.len(), 23);
    // Around the hit unit (110, 100): even directions first.
    assert_eq!(tpoint(&w, n[0]), (110, 102));
    assert_eq!(tpoint(&w, n[1]), (112, 102));
    assert_eq!(tpoint(&w, n[8]), (111, 102));
    // rancidgasepotion (a = 0): the 8 even directions around the missile.
    r.shitpar1 = 0;
    let (mut w, m) = world(r, &[s]);
    srv_hit(&mut w, 2, m, None);
    let n = others(&w, m);
    let got: Vec<_> = n
        .iter()
        .map(|&u| {
            let (x, y) = tpoint(&w, u);
            (x - 100, y - 100)
        })
        .collect();
    assert_eq!(
        got,
        [
            (0, 2),
            (2, 2),
            (2, 0),
            (2, -2),
            (0, -2),
            (-2, -2),
            (-2, 0),
            (-2, 2)
        ]
    );
}

// Covers: specs/missiles/bodies.md §edge-cases-original-bugs r4
#[test]
fn plague_ring_negative_step_is_flagged() {
    let mut r = row();
    r.shitpar1 = (-1i32) as u32;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    srv_hit(&mut w, 2, m, None);
    assert_eq!(
        w.store.unhandled,
        [Unhandled::Fatal {
            addr: 0x005A9370,
            missile: m
        }]
    );
}

// Covers: specs/missiles/bodies.md §7 r1, §7 r2, §7 r3, §7 r4
#[test]
fn fire_wall_maker_on_new_steps() {
    let mut r = row();
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    srv_do(&mut w, 6, m);
    assert_eq!(others(&w, m).len(), 0);
    w.fake.mb.new_step = true;
    srv_do(&mut w, 6, m);
    let n = others(&w, m);
    assert_eq!(n.len(), 1);
    assert_eq!(w.fake.pos[&n[0]], (100, 100));
    w.fake.no_path = true;
    assert_eq!(srv_do(&mut w, 6, m), 2);
}

// Covers: specs/missiles/bodies.md §8 r1, §8 r2, §8 r3, §8 r4, §8 r5, §edge-cases-original-bugs r5, §edge-cases-original-bugs r7
#[test]
fn exploding_javelin_radius() {
    let mut r = row();
    r.shitpar1 = 0;
    let (mut w, m) = world(r.clone(), &[]);
    skill(&mut w, [0, 0, 4, 0, 0]);
    assert_eq!(srv_hit(&mut w, 44, m, None), 1);
    assert!(w.fake.log.contains(&"scan (100, 100) 4 0x8583".to_string()));
    // A negative sHitPar1 is the radius as it is.
    r.shitpar1 = (-3i32) as u32;
    let (mut w, m) = world(r.clone(), &[]);
    srv_hit(&mut w, 44, m, None);
    assert!(w
        .fake
        .log
        .contains(&"scan (100, 100) -3 0x8583".to_string()));
    // Server-hit 3: a unit contact is 0, else server-hit 44.
    let (mut w, m) = world(r, &[]);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 3, m, Some(mon)), 0);
    assert_eq!(w.fake.logged("scan"), 0);
    assert_eq!(srv_hit(&mut w, 3, m, None), 1);
    assert_eq!(w.fake.logged("scan"), 1);
}

// Covers: specs/missiles/bodies.md §9 r1, §9 r2, §9 r3, §9 r4, §9 r5, §9 r6, §9 r7, §9 l2 r1, §9 l2 r2, §9 l2 r3
#[test]
fn meteor_scatter() {
    let mut r = row();
    r.shitpar2 = 1;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r.clone(), &[sub()]);
    skill(&mut w, [0, 0, 3, 0, 0]);
    field(&mut w, 3, 30);
    field(&mut w, 4, 15);
    w.store.get_mut(m).unwrap().level = 3;
    assert_eq!(srv_hit(&mut w, 14, m, None), 1);
    let n = others(&w, m);
    assert_eq!(n.len(), 18);
    assert_eq!(w.fake.pos[&n[0]], (102, 98));
    assert!(n.iter().all(|&u| w.store.get(u).unwrap().total == 60));
    // Step 3 over the 18 points.
    r.shitpar2 = 3;
    let (mut w, m) = world(r, &[sub()]);
    skill(&mut w, [0, 0, 3, 0, 0]);
    srv_hit(&mut w, 14, m, None);
    assert_eq!(others(&w, m).len(), 6);
}

// Covers: specs/missiles/bodies.md §10 r1, §10 r2, §10 r3, §10 r4, §10 r5
#[test]
fn missile_in_air_lands() {
    let mut r = row();
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    set_data(&mut w, m, 42, 0);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 36, m, Some(mon)), 0);
    assert_eq!(others(&w, m).len(), 0);
    assert_eq!(srv_hit(&mut w, 36, m, None), 1);
    let n = others(&w, m);
    assert_eq!(n.len(), 1);
    assert_eq!(w.fake.pos[&n[0]], (100, 100));
    assert_eq!(w.store.get(n[0]).unwrap().target.0, 42);
}

// Covers: specs/missiles/bodies.md §11 r1, §11 r2, §11 r3, §11 r4, §11 r5, §11 r6, §11 r7, §11 l2 r1, §11 l2 r2, §11 l2 r3, §11 l3 r1, §11 l3 r2, §11 l3 r4, §11 l3 r5
#[test]
fn bone_spirit_retargets_once() {
    let mut r = row();
    r.param2 = 15;
    r.range = 128;
    let (mut w, m) = world(r, &[]);
    set_data(&mut w, m, 2, 0);
    set_frames(&mut w, m, 50, 0);
    assert_eq!(srv_hit(&mut w, 10, m, None), 4);
    let d = w.store.get(m).unwrap();
    assert_eq!((d.total, d.current, d.target.0), (128, 128, 6));
    // Frames left with bit 2: 3; the second expiry: 1.
    assert_eq!(srv_hit(&mut w, 10, m, None), 1);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 10, m, Some(mon)), 3);
    // Homing on a unit: another unit passes (4), the target dies (3).
    set_data(&mut w, m, 1, 0);
    assert_eq!(srv_hit(&mut w, 10, m, Some(mon)), 4);
    w.fake.mb.target = Some(mon);
    assert_eq!(srv_hit(&mut w, 10, m, Some(mon)), 3);
    // A barrier under a spirit with no unit: 1.
    w.fake.word = 4;
    assert_eq!(srv_hit(&mut w, 10, m, None), 1);
}

// Covers: specs/missiles/bodies.md §11 l3 r3
#[test]
fn bone_spirit_aims_at_the_found_unit() {
    let mut r = row();
    r.param2 = 15;
    r.range = 20;
    r.levrange = 2;
    let (mut w, m) = world(r, &[]);
    let mon = w.monster;
    w.fake.mb.area = vec![mon];
    set_data(&mut w, m, 2, 0);
    set_frames(&mut w, m, 50, 0);
    assert_eq!(srv_hit(&mut w, 10, m, None), 4);
    let d = w.store.get(m).unwrap();
    assert_eq!((d.total, d.target.0), (20 + 2 * 9, 5));
    assert_eq!(w.fake.paths[&m].target_unit, Some(mon));
    // d = (2 × 10 + 0) / 2 = 10 < 25: rebuilt.
    assert!(w.fake.calls.contains(&"build".to_string()));
}

// ---------------------------------------------------------------- §12–§17

// Covers: specs/missiles/bodies.md §12 r1, §12 r2, §12 r3, §12 r4, §12 r5, §12 r6, §12 r7, §12 r8, §12 r9, §12 r10, §12 r11, §edge-cases-original-bugs r8, §30, §edge-cases-original-bugs r15
#[test]
fn goo_state_first_contact_has_no_values() {
    let (mut w, m) = world(row(), &[]);
    skill(&mut w, [0, 0, 0, 9, 3]);
    field(&mut w, 21, 5);
    w.fake.mb.states_count = 10;
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 16, m, None), 0);
    assert_eq!(srv_hit(&mut w, 16, m, Some(mon)), 0);
    let f = w.game.frame;
    assert!(w
        .fake
        .log
        .contains(&format!("newlist {} 5 {}", mon.0, f + 5)));
    assert_eq!(w.fake.logged("aurafill"), 0);
    assert_eq!(w.fake.logged("changed"), 1);
    // A later contact fills and pushes the expiry.
    assert_eq!(srv_hit(&mut w, 16, m, Some(mon)), 0);
    assert_eq!(w.fake.logged("aurafill"), 1);
    assert_eq!(w.fake.logged("expiry"), 1);
    // Server-hit 19: auralencalc, no unit → 1, result 3.
    assert_eq!(srv_hit(&mut w, 19, m, None), 1);
    assert_eq!(srv_hit(&mut w, 19, m, Some(mon)), 3);
    assert!(w
        .fake
        .log
        .contains(&format!("expiry {} 5 {}", mon.0, f + 9)));
    // An invalid state: 1.
    w.fake.mb.states_count = 5;
    assert_eq!(srv_hit(&mut w, 16, m, Some(mon)), 1);
}

// Covers: specs/missiles/bodies.md §13 r1, §13 r2, §13 r3
#[test]
fn shout_on_allies_only() {
    let (mut w, m) = world(row(), &[]);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 18, m, Some(mon)), 0);
    assert_eq!(w.fake.logged("shout"), 0);
    w.fake.mb.allies.insert(mon);
    assert_eq!(srv_hit(&mut w, 18, m, Some(mon)), 0);
    assert_eq!(w.fake.logged("shout"), 1);
}

// Covers: specs/missiles/bodies.md §14 r1, §14 r2, §14 r3, §14 r4, §14 r5
#[test]
fn grim_ward_start_range() {
    let mut r = row();
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    skill(&mut w, [3, 0, 0, 0, 0]);
    assert_eq!(srv_hit(&mut w, 26, m, None), 1);
    let n = others(&w, m);
    assert_eq!(w.store.get(n[0]).unwrap().total, 5);
}

// Covers: specs/missiles/bodies.md §15 r1, §15 r2, §15 r3, §15 r4, §15 r5
#[test]
fn grim_ward_runs_the_skill_function() {
    let mut r = row();
    (r.param1, r.param2) = (6, 30);
    let (mut w, m) = world(r, &[]);
    set_frames(&mut w, m, 50, 38);
    srv_do(&mut w, 14, m);
    assert!(w.fake.log.contains(&format!("skilldo {} 30 7 10", m.0)));
    set_frames(&mut w, m, 50, 37);
    srv_do(&mut w, 14, m);
    assert_eq!(w.fake.logged("skilldo"), 1);
    assert_eq!(srv_hit(&mut w, 27, m, None), 1);
}

// Covers: specs/missiles/bodies.md §16 r1, §16 r2, §16 r3, §16 r4, §edge-cases-original-bugs r9
#[test]
fn blade_fury_makes_class_zero() {
    let mut r = row();
    r.shitpar1 = 1;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    assert_eq!(srv_hit(&mut w, 52, m, None), 1);
    let n = others(&w, m);
    assert_eq!(w.fake.logged("alloc 0"), 8);
    let got: Vec<_> = n.iter().map(|&u| tpoint(&w, u)).collect();
    assert_eq!(
        got,
        [
            (116, 100),
            (116, 116),
            (100, 116),
            (84, 116),
            (84, 100),
            (84, 84),
            (100, 84),
            (116, 84)
        ]
    );
    assert_eq!(w.store.get(n[1]).unwrap().target, (16, 16));
}

// Covers: specs/missiles/bodies.md §17 r1, §17 r2, §17 r3, §17 r4
#[test]
fn holy_bolt_heals_or_picks() {
    let mut r = row();
    (r.shitpar1, r.shitpar2) = (1, 1);
    r.progoverlay = 5;
    let (mut w, m) = world(r, &[]);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 7, m, Some(mon)), 4);
    w.fake.mb.undead.insert(mon);
    assert_eq!(srv_hit(&mut w, 7, m, Some(mon)), 3);
    // A pet: calc1 = calc2 = 2 → 512, no draw; capped at max life.
    skill(&mut w, [2, 2, 0, 0, 0]);
    w.fake.mb.pets.insert(mon);
    w.fake.stats.insert((mon, 6), 1000);
    w.fake.mb.max_life.insert(mon, 1200);
    let before = *w.fake.seed(m);
    assert_eq!(srv_hit(&mut w, 7, m, Some(mon)), 1);
    assert_eq!(w.fake.stats[&(mon, 6)], 1200);
    assert_eq!(*w.fake.seed(m), before);
    assert!(w.fake.log.contains(&format!("overlay {} 5", mon.0)));
    // A player with sHitPar2 0 is damaged.
    let o = w.owner;
    let mut r = row();
    r.shitpar2 = 0;
    let (mut w, m) = world(r, &[]);
    assert_eq!(srv_hit(&mut w, 7, m, Some(o)), 3);
}

// ---------------------------------------------------------------- §18–§23

// Covers: specs/missiles/bodies.md §18 r1, §18 r2, §18 r3, §18 r4, §18 r5
#[test]
fn trailing_javelin_sides() {
    let mut r = row();
    r.param1 = 3;
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    w.fake.pos.insert(m, (50, 50));
    w.fake.paths.entry(m).or_default().target_point = Some((60, 50));
    w.fake.mb.new_step = true;
    set_frames(&mut w, m, 50, 50);
    srv_do(&mut w, 22, m);
    assert_eq!(w.store.get(m).unwrap().target, (0, 10));
    let n = others(&w, m);
    assert_eq!(tpoint(&w, n[0]), (50, 60));
    assert_eq!(tpoint(&w, n[1]), (50, 40));
}

// Covers: specs/missiles/bodies.md §19 l2 r1, §19 l2 r2, §19 l2 r3, §19 l3 r1, §edge-cases-original-bugs r10
#[test]
fn nova_counts() {
    for (n, want) in [
        (1, 1),
        (2, 10),
        (3, 10),
        (5, 12),
        (9, 16),
        (10, 10),
        (11, 18),
        (18, 24),
        (19, 19),
        (28, 28),
    ] {
        assert_eq!(nova_offsets(n).unwrap().len(), want, "n = {n}");
    }
    assert_eq!(
        nova_offsets(3).unwrap(),
        [
            (14, -14),
            (-18, -8),
            (-8, -18),
            (20, 0),
            (-20, 0),
            (0, 20),
            (0, -20),
            (14, 14),
            (-14, 14),
            (-14, -14)
        ]
    );
    assert!(nova_offsets(40).is_some());
    assert!(nova_offsets(41).is_none());
}

// Covers: specs/missiles/bodies.md §19 r1, §19 r2, §19 r3, §19 l4 r1, §19 l4 r2, §19 l4 r3
#[test]
fn trailing_javelin_explosion_zigzags() {
    let mut r = row();
    (r.shitpar1, r.shitpar2) = (1, 1);
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r.clone(), &[sub()]);
    assert_eq!(srv_hit(&mut w, 45, m, None), 1);
    let n = others(&w, m);
    assert_eq!(n.len(), 1);
    assert_eq!(*w.fake.seed(n[0]), Seed::init_low(114));
    assert!(w.fake.log.contains(&format!("ptype {} 10", n[0].0)));
    assert!(w.fake.log.contains(&format!("pdist {} 50", n[0].0)));
    // sHitPar2 0: no callback.
    r.shitpar2 = 0;
    let (mut w, m) = world(r, &[sub()]);
    srv_hit(&mut w, 45, m, None);
    assert_eq!(w.fake.logged("ptype"), 0);
}

// Covers: specs/missiles/bodies.md §19 l3 r1, §19 l3 r2, §19 l3 r3
#[test]
fn catapult_charged_ball_count() {
    let mut r = row();
    (r.shitpar1, r.shitpar2) = (4, 2);
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    w.store.get_mut(m).unwrap().level = 3;
    srv_hit(&mut w, 38, m, None);
    assert_eq!(others(&w, m).len(), 16);
    assert_eq!(w.fake.logged("ptype"), 16);
}

// Covers: specs/missiles/bodies.md §20 r1, §20 r2, §20 r3, §21 r1, §21 r2, §21 r3, §edge-cases-original-bugs r11
#[test]
fn trail_makers() {
    let mut r = row();
    r.param1 = 3;
    r.submissile1 = 1;
    let (mut w, m) = world(r.clone(), &[sub()]);
    w.fake.mb.new_step = true;
    srv_do(&mut w, 23, m);
    srv_do(&mut w, 24, m);
    assert_eq!(others(&w, m).len(), 2);
    // Server-do 26 every max(Param1, 1) elapsed frames.
    let mut r2 = r.clone();
    r2.param1 = 9;
    let (mut w, m) = world(r2, &[sub()]);
    set_frames(&mut w, m, 50, 41);
    srv_do(&mut w, 26, m);
    assert_eq!(others(&w, m).len(), 1);
    set_frames(&mut w, m, 50, 40);
    srv_do(&mut w, 26, m);
    assert_eq!(others(&w, m).len(), 1);
    // `SubMissile1` = 0 is rejected.
    r.submissile1 = 0;
    let (mut w, m) = world(r, &[]);
    assert_eq!(srv_do(&mut w, 23, m), 2);
    assert_eq!(srv_do(&mut w, 26, m), 2);
    assert_eq!(srv_do(&mut w, 22, m), 2);
}

// Covers: specs/missiles/bodies.md §22 r1, §22 r2, §22 r3, §22 r4, §edge-cases-original-bugs r12
#[test]
fn wake_maker() {
    let mut r = row();
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    set_data(&mut w, m, 3, -4);
    w.fake.mb.new_step = true;
    srv_do(&mut w, 31, m);
    let n = others(&w, m);
    assert_eq!(tpoint(&w, n[0]), (103, 96));
    assert_eq!(tpoint(&w, n[1]), (97, 104));
    // No owner: the no-unit hit inlined, then removed.
    let o = w.owner;
    w.game.remove_unit(o).unwrap();
    assert_eq!(srv_do(&mut w, 31, m), 2);
    assert!(w.fake.log.contains(&format!("clear {}", m.0)));
}

// Covers: specs/missiles/bodies.md §23 r1, §23 r2, §23 r3, §23 r4, §23 r5, §23 r6, §23 r7
#[test]
fn armageddon_control() {
    let mut r = row();
    r.shitpar1 = 3;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    assert_eq!(srv_hit(&mut w, 56, m, None), 1);
    assert!(w.fake.log.contains(&"scan (100, 100) 3 0x8583".to_string()));
    assert_eq!(others(&w, m).len(), 1);
}

// ---------------------------------------------------------------- §24–§30

// Covers: specs/missiles/bodies.md §24 r1, §24 r2, §24 r3
#[test]
fn blaze_spares_its_caster() {
    let (mut w, m) = world(row(), &[]);
    let o = w.owner;
    assert_eq!(srv_hit(&mut w, 8, m, None), 2);
    assert_eq!(srv_hit(&mut w, 8, m, Some(o)), 2);
    w.fake.states.insert((o, 13));
    assert_eq!(srv_hit(&mut w, 8, m, Some(o)), 0);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 8, m, Some(mon)), 2);
}

// Covers: specs/missiles/bodies.md §25 r1, §25 r2, §25 r3, §25 r4, §25 r5, §25 l2 r1, §25 l2 r2, §25 l2 r3, §edge-cases-original-bugs r14
#[test]
fn immolation_fire_disc() {
    let mut r = row();
    r.shitpar1 = 1;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    skill(&mut w, [0, 2, 0, 0, 0]);
    w.fake.mb.calc = 100;
    w.fake.stats.insert((m, 56), 7);
    assert_eq!(srv_hit(&mut w, 9, m, None), 3);
    assert_eq!(w.fake.stats[&(m, 56)], 0);
    let n = others(&w, m);
    let got: Vec<_> = n.iter().map(|&u| w.fake.pos[&u]).collect();
    assert_eq!(
        got,
        [(99, 100), (100, 99), (100, 100), (100, 101), (101, 100)]
    );
    assert!(n.iter().all(|&u| w.store.get(u).unwrap().total == 100));
    assert!(w.fake.log.contains(&"scan (100, 100) 2 0x8583".to_string()));
    // r = 2: 13 cells; a wall on the outward line vetoes one.
    let (mut w, m) = world(row(), &[sub()]);
    w.fake.mb.walls.insert(((101, 100), (102, 100)));
    let mut cx = cx!(w);
    fire_disc(&mut w.game, &mut cx, m, 2, 1, 0);
    assert_eq!(others(&w, m).len(), 12);
}

// Covers: specs/missiles/bodies.md §26 r1, §26 r2, §26 r3, §27 r1, §27 r2, §27 r3, §edge-cases-original-bugs r13
#[test]
fn bat_bolt_and_goo_lay() {
    let mut r = row();
    r.submissile1 = 1;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    w.fake.mb.new_step = true;
    srv_do(&mut w, 9, m);
    assert_eq!(others(&w, m).len(), 1);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 15, m, Some(mon)), 0);
    assert_eq!(others(&w, m).len(), 2);
    // A missing record is fatal for server-do 9.
    w.store.get_mut(m).unwrap().class = 9;
    srv_do(&mut w, 9, m);
    assert_eq!(
        w.store.unhandled,
        [Unhandled::Fatal {
            addr: 0x005AE940,
            missile: m
        }]
    );
}

// Covers: specs/missiles/bodies.md §28 r1, §28 r2, §28 r3, §28 r4, §28 r5, §28 r6, §28 r7
#[test]
fn howl_level_test() {
    let (mut w, m) = world(row(), &[]);
    skill(&mut w, [0; 5]);
    field(&mut w, 21, 5);
    for (code, v) in [(2, 2), (3, 10), (4, 1), (5, 20), (6, 2)] {
        field(&mut w, code, v);
    }
    w.fake.mb.states_count = 10;
    w.store.get_mut(m).unwrap().level = 3;
    let (o, mon) = (w.owner, w.monster);
    w.fake.stats.insert((o, 12), 10);
    w.fake.stats.insert((mon, 12), 15);
    assert_eq!(srv_hit(&mut w, 17, m, Some(mon)), 0);
    assert_eq!(w.fake.logged("terror"), 0);
    w.fake.stats.insert((mon, 12), 14);
    assert_eq!(srv_hit(&mut w, 17, m, Some(mon)), 0);
    assert!(w
        .fake
        .log
        .contains(&format!("terror {} {} 12 24", o.0, mon.0)));
    // Not a monster: 0, nothing.
    assert_eq!(srv_hit(&mut w, 17, m, Some(o)), 0);
    assert_eq!(w.fake.logged("terror"), 1);
}

// Covers: specs/missiles/bodies.md §29 r1, §29 r2, §29 r3, §29 r4, §29 r5, §29 r6
#[test]
fn finger_mage_spider_steps() {
    let mut r = row();
    (r.param1, r.param2, r.param3) = (5, 20, 2);
    let (mut w, m) = world(r, &[]);
    let mon = w.monster;
    w.fake.pos.insert(m, (10, 10));
    w.fake.pos.insert(mon, (25, 5));
    w.fake.mb.target = Some(mon);
    set_frames(&mut w, m, 100, 75);
    srv_do(&mut w, 11, m);
    assert_eq!(tpoint(&w, m), (12, 8));
    // Off the interval: no re-aim.
    w.fake.calls.clear();
    set_frames(&mut w, m, 100, 74);
    srv_do(&mut w, 11, m);
    assert!(!w.fake.calls.contains(&"build".to_string()));
}

// ---------------------------------------------------------------- §31–§38

// Covers: specs/missiles/bodies-2.md §31 r1, §31 r2, §31 r3, §31 r4
#[test]
fn diablo_wall_maker_needs_an_owner() {
    let mut r = row();
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    w.fake.mb.new_step = true;
    srv_do(&mut w, 12, m);
    assert_eq!(others(&w, m).len(), 1);
    let o = w.owner;
    w.game.remove_unit(o).unwrap();
    assert_eq!(srv_do(&mut w, 12, m), 2);
}

// Covers: specs/missiles/bodies-2.md §32 r1, §32 r2, §32 r3, §32 r4, §32 r5
#[test]
fn lightning_fury_first_n() {
    let mut r = row();
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    skill(&mut w, [3, 0, 5, 0, 0]);
    let room = w.fake.room;
    let mut units = Vec::new();
    for i in 0..5 {
        let u = w.game.spawn_unit(UnitType::Monster, room, false).unwrap();
        w.fake.pos.insert(u, (105 + i, 100));
        units.push(u);
    }
    w.fake.mb.area = units.clone();
    assert_eq!(srv_hit(&mut w, 20, m, None), 3);
    assert!(w
        .fake
        .log
        .contains(&"scan (100, 100) 5 0xa783 true".to_string()));
    let n = others(&w, m);
    assert_eq!(n.len(), 3);
    assert_eq!(tpoint(&w, n[2]), (107, 100));
}

// Covers: specs/missiles/bodies-2.md §33 r1, §33 r2, §33 r3, §33 r4, §33 r5, §33 r6, §33 r7, §33 r8, §33 r9, §edge-cases-original-bugs r3
#[test]
fn bone_wall_pieces() {
    let (mut w, m) = world(row(), &[]);
    skill(&mut w, [0; 5]);
    let mon = w.monster;
    let g = guid(&w, mon);
    set_data(&mut w, m, g, 0);
    assert_eq!(srv_do(&mut w, 13, m), 2);
    set_data(&mut w, m, g, 2);
    w.fake.mb.new_step = true;
    // Invalid summon class: 0, no flight.
    assert_eq!(srv_do(&mut w, 13, m), 0);
    w.fake.mb.summon = Some(SummonClass { class: 4, mode: 1 });
    w.fake.mb.summoned = Some(mon);
    field(&mut w, 22, 9);
    w.fake.mb.pet_types = 5;
    srv_do(&mut w, 13, m);
    assert!(w.fake.log.contains(&"summon 4 1 (100, 100) 0".to_string()));
    assert_eq!(w.fake.logged("bind"), 1);
    assert_eq!(w.store.get(m).unwrap().target.1, 1);
}

// Covers: specs/missiles/bodies-2.md §34 r1, §34 r2, §34 r3, §34 r4, §34 r5, §34 r6, §34 r7, §edge-cases-original-bugs r1
#[test]
fn battle_cry_state() {
    let (mut w, m) = world(row(), &[]);
    skill(&mut w, [0, 0, 0, 12, 0]);
    field(&mut w, 21, 10);
    w.fake.mb.states_count = 10;
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 21, m, Some(mon)), 1, "not accepted");
    w.fake.mb.accept = true;
    w.fake.mb.apply_ok = true;
    assert_eq!(srv_hit(&mut w, 21, m, Some(mon)), 0);
    assert!(w.fake.log.contains(&format!("applystate {} 10 12", mon.0)));
    assert_eq!(w.fake.logged("aurafill"), 1);
    assert_eq!(srv_hit(&mut w, 21, m, None), 1);
}

// Covers: specs/missiles/bodies-2.md §35 r1, §35 r2, §35 r3, §35 r4, §35 r5, §35 r6
#[test]
fn fist_delay() {
    let mut r = row();
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    skill(&mut w, [0, 0, 6, 0, 2]);
    set_data(&mut w, m, 1, 99999);
    assert_eq!(srv_hit(&mut w, 22, m, None), 0, "struck unit gone");
    assert_eq!(others(&w, m).len(), 0);
    let mon = w.monster;
    let g = guid(&w, mon);
    set_data(&mut w, m, 1, g);
    w.fake.mb.area = vec![mon, mon, mon];
    assert_eq!(srv_hit(&mut w, 22, m, None), 1);
    assert!(w.fake.log.contains(&format!("damage {} 0", mon.0)));
    assert!(w
        .fake
        .log
        .contains(&"scan (100, 100) 6 0xa683 false".to_string()));
    assert_eq!(others(&w, m).len(), 2);
}

// Covers: specs/missiles/bodies-2.md §36 r1, §36 r2, §36 r3, §36 r4, §36 r5
#[test]
fn panther_orange() {
    let (mut w, m) = world(row(), &[]);
    skill(&mut w, [4, 0, 0, 0, 0]);
    assert_eq!(srv_hit(&mut w, 24, m, None), 1);
    assert!(w.fake.log.contains(&"scan (100, 100) 4 0x8583".to_string()));
}

// Covers: specs/missiles/bodies-2.md §37 r1, §37 r2, §37 r3, §37 l2 r1, §37 l2 r2, §37 l2 r3, §edge-cases-original-bugs r4
#[test]
fn panther_green_ring() {
    let mut r = row();
    r.shitpar1 = 1;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    assert_eq!(srv_hit(&mut w, 25, m, None), 3);
    let got: Vec<_> = others(&w, m)
        .iter()
        .map(|&u| {
            let (x, y) = tpoint(&w, u);
            (x - 100, y - 100)
        })
        .collect();
    assert_eq!(
        got,
        [
            (0, 2),
            (2, 2),
            (2, 0),
            (2, -2),
            (0, -2),
            (-2, -2),
            (-2, 0),
            (-2, 2)
        ]
    );
}

// Covers: specs/missiles/bodies-2.md §38 r1, §38 r2, §38 r3, §38 r4, §38 r5, §38 r6
#[test]
fn grim_ward_scare_radius() {
    let (mut w, m) = world(row(), &[]);
    skill(&mut w, [0; 5]);
    for (code, v) in [(1, 3), (2, 1), (5, 10), (6, 60)] {
        field(&mut w, code, v);
    }
    let g = guid(&w, m);
    set_data(&mut w, m, g, 0);
    let mon = w.monster;
    // d = 3 + 9 = 12; 10² < 12².
    assert_eq!(srv_hit(&mut w, 28, m, Some(mon)), 1);
    assert!(w
        .fake
        .log
        .contains(&format!("terror {} {} 10 60", m.0, mon.0)));
    w.fake.pos.insert(mon, (112, 100));
    srv_hit(&mut w, 28, m, Some(mon));
    assert_eq!(w.fake.logged("terror"), 1);
}

// ---------------------------------------------------------------- §39–§46

// Covers: specs/missiles/bodies-2.md §39 r1, §39 r2, §39 r3, §39 r4, §39 r5
#[test]
fn frozen_orb_bolts() {
    assert_eq!((ORB_C[60], ORB_S[60]), (27, -11));
    assert_eq!(
        ORB_C[..17],
        [30, 29, 29, 28, 27, 26, 24, 23, 21, 19, 16, 14, 11, 8, 5, 2, 0]
    );
    let mut r = row();
    (r.param1, r.param2) = (1, 19);
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    set_data(&mut w, m, 60, 0);
    set_frames(&mut w, m, 50, 45);
    srv_do(&mut w, 15, m);
    let n = others(&w, m);
    assert_eq!(tpoint(&w, n[0]), (127, 89));
    assert_eq!(w.store.get(m).unwrap().target.0, 15);
    set_data(&mut w, m, -70, 0);
    srv_do(&mut w, 15, m);
    let n = others(&w, m);
    assert_eq!(tpoint(&w, n[1]), (100 + ORB_C[6], 100 + ORB_S[6]));
    assert_eq!(w.store.get(m).unwrap().target.0, 25);
}

// Covers: specs/missiles/bodies-2.md §39 l2 r1, §39 l2 r2, §39 l2 r3, §39 l2 r4, §39 l2 r5, §edge-cases-original-bugs r6
#[test]
fn frozen_orb_nova_at_expiry() {
    let mut r = row();
    r.shitpar1 = 4;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    set_frames(&mut w, m, 50, 3);
    assert_eq!(srv_hit(&mut w, 29, m, None), 2);
    assert_eq!(others(&w, m).len(), 0);
    set_frames(&mut w, m, 50, 0);
    assert_eq!(srv_hit(&mut w, 29, m, None), 3);
    let n = others(&w, m);
    assert_eq!(n.len(), 16);
    assert_eq!(w.store.get(n[1]).unwrap().target, (ORB_C[4], ORB_S[4]));
    // Server-hit 5: the same circle without the frames-left test.
    let (mut w, m) = world(
        {
            let mut r = row();
            r.shitpar1 = 4;
            r.hitsubmissile1 = 1;
            r
        },
        &[sub()],
    );
    set_frames(&mut w, m, 50, 3);
    assert_eq!(srv_hit(&mut w, 5, m, None), 3);
    assert_eq!(others(&w, m).len(), 16);
}

// Covers: specs/missiles/bodies-2.md §40 r1, §40 r2, §40 r3
#[test]
fn frozen_orb_nova_curls() {
    let mut r = row();
    (r.param1, r.param2) = (6, 2);
    let (mut w, m) = world(r, &[]);
    set_data(&mut w, m, 30, 0);
    for (current, want) in [(50, (15, 15)), (48, (0, 15)), (46, (-7, 7)), (44, (-7, 7))] {
        set_frames(&mut w, m, 50, current);
        srv_do(&mut w, 16, m);
        assert_eq!(w.store.get(m).unwrap().target, want, "{current}");
    }
}

// Covers: specs/missiles/bodies-2.md §41 r1, §41 r2, §41 r3, §41 r4
#[test]
fn fire_head_heals_its_owner() {
    let mut r = row();
    r.etype = 1;
    r.collidekill = 0;
    let (mut w, m) = world(r, &[]);
    let (o, mon) = (w.owner, w.monster);
    w.fake.stats.insert((o, 6), 100);
    w.fake.mb.max_life.insert(o, 500);
    // Fire 40..40 (equal bounds: no draw).
    w.fake.stats.insert((m, 48), 40);
    w.fake.stats.insert((m, 49), 40);
    assert_eq!(srv_hit(&mut w, 31, m, Some(mon)), 2);
    // Healed by the rolled fire amount (`elem_roll`'s return), not by
    // the `EType` 1 (`missiles.md` §R9.6 return value).
    assert_eq!(w.fake.stats[&(o, 6)], 140);
    assert_eq!(srv_hit(&mut w, 31, m, None), 1);
}

// Covers: specs/missiles/bodies-2.md §43 r1, §43 r2, §43 r3, §43 r4, §43 r5, §edge-cases-original-bugs r7
#[test]
fn tower_chest() {
    let mut r = row();
    (r.param1, r.param2, r.param3) = (150, 2, 5);
    r.range = 400;
    let (mut w, m) = world(r, &[]);
    let room = w.fake.room;
    let chest = w.game.spawn_unit(UnitType::Object, room, false).unwrap();
    let g = guid(&w, chest);
    set_data(&mut w, m, g, 0);
    set_frames(&mut w, m, 400, 250);
    srv_do(&mut w, 18, m);
    assert!(w.fake.log.contains(&format!("chest {}", chest.0)));
    assert_eq!(w.store.get(m).unwrap().target.1, 1);
    assert_eq!(w.fake.logged("gold"), 0);
    w.fake.mb.floor = true;
    set_frames(&mut w, m, 400, 248);
    srv_do(&mut w, 18, m);
    assert_eq!(w.fake.logged("gold"), 1);
    set_frames(&mut w, m, 400, 1);
    srv_do(&mut w, 18, m);
    assert!(w.fake.log.contains(&format!("sound {} 0x5c", chest.0)));
    assert_eq!(srv_hit(&mut w, 33, m, None), 0);
    assert_eq!(w.fake.logged("refresh"), 1);
}

// Covers: specs/missiles/bodies-2.md §44 r1, §44 r2, §44 l2 r1, §44 l2 r2, §44 l2 r3, §44 l2 r4, §44 l2 r5
#[test]
fn radament_redemption() {
    let (mut w, m) = world(row(), &[]);
    w.fake.mb.skills.insert(124, [0; 5]);
    w.fake.mb.fields.insert((124, 1), 16);
    let (room, mon) = (w.room(), w.monster);
    w.game.lists.room_mut(room).unwrap().adjacent = vec![room];
    // The monster at (110, 100) is a corpse (mode 12), within r = 16.
    w.fake.mb.modes.insert(mon, 12);
    set_frames(&mut w, m, 400, 10);
    srv_do(&mut w, 19, m);
    assert!(w
        .fake
        .log
        .contains(&format!("redeem {} 124 1 false", mon.0)));
    set_frames(&mut w, m, 400, 1);
    srv_do(&mut w, 19, m);
    assert!(w.fake.log.contains(&format!("redeem {} 124 1 true", mon.0)));
    set_frames(&mut w, m, 400, 25);
    srv_do(&mut w, 19, m);
    assert_eq!(w.fake.logged("redeem"), 2);
}

fn find_world() -> (World, UnitId, RoomId) {
    let (mut w, m) = world(row(), &[]);
    let room = w.room();
    w.game.lists.room_mut(room).unwrap().adjacent = vec![room];
    (w, m, room)
}

fn find(w: &mut World, room: RoomId, flags: u32, r: i32) -> Vec<UnitId> {
    let filter = crate::missiles::bodies_ext2::FindFilter {
        flags,
        source: Some(w.owner),
        at: (100, 100),
        r,
    };
    let mut cx = cx!(w);
    crate::missiles::bodies_ext2::unit_find(&mut w.game, &mut cx, Some(room), &filter)
}

// Covers: specs/missiles/bodies-2.md §44 l3 r3, §44 l4 r1, §44 l4 r2, §44 l4 r3, §44 l4 r4, §44 l4 r5
#[test]
fn unit_find_default_filter_by_type_mode_and_distance() {
    let (mut w, _, room) = find_world();
    let mon = w.monster;
    // Distance ≤ r passes: (110, 100) at r = 10, not at r = 9.
    w.fake.mb.modes.insert(mon, 12);
    assert_eq!(find(&mut w, room, 0x1002, 10), vec![mon]);
    assert!(find(&mut w, room, 0x1002, 9).is_empty());
    // Without 0x1000 a dead (12) or dying (0) monster is rejected.
    assert!(find(&mut w, room, 0x2, 10).is_empty());
    w.fake.mb.modes.insert(mon, 1);
    assert_eq!(find(&mut w, room, 0x2, 10), vec![mon]);
    assert!(find(&mut w, room, 0x1002, 10).is_empty());
    // 0x4: undead only.
    assert!(find(&mut w, room, 0x6, 10).is_empty());
    w.fake.mb.undead.insert(mon);
    assert_eq!(find(&mut w, room, 0x6, 10), vec![mon]);
    // Players need 0x1 and are never the filter's own unit S.
    assert_eq!(find(&mut w, room, 0x3, 10), vec![mon]);
    // 0x80 / 0x400: unit flags 0x4 / 0x8.
    w.fake.flags.insert(mon, 0);
    assert!(find(&mut w, room, 0x82, 10).is_empty());
    w.fake.flags.insert(mon, 0x4);
    assert_eq!(find(&mut w, room, 0x82, 10), vec![mon]);
    assert!(find(&mut w, room, 0x402, 10).is_empty());
    // 0x40 with the record's limit 0: everything rejected.
    assert!(find(&mut w, room, 0x42, 10).is_empty());
    // 0x100: a unit in a town room rejects; 0x2000 skips town rooms.
    w.fake.town.insert(room);
    assert!(find(&mut w, room, 0x102, 10).is_empty());
    assert!(find(&mut w, room, 0x2002, 10).is_empty());
    assert_eq!(find(&mut w, room, 0x2, 10), vec![mon]);
}

// Covers: specs/missiles/bodies-2.md §44 l2 r4, §44 l3 r1, §44 l3 r2, §44 l3 r4
#[test]
fn unit_find_rooms_and_found_order() {
    let (mut w, _, room) = find_world();
    let filter = crate::missiles::bodies_ext2::FindFilter {
        flags: 0x1002,
        source: None,
        at: (100, 100),
        r: 10,
    };
    let mut cx = cx!(w);
    assert!(
        crate::missiles::bodies_ext2::unit_find(&mut w.game, &mut cx, None, &filter).is_empty()
    );
    let other = w.game.lists.create_room(0).unwrap();
    w.game.lists.activate_room(other).unwrap();
    w.game.lists.room_mut(room).unwrap().adjacent = vec![other, room];
    let far = w
        .game
        .spawn_unit(UnitType::Monster, Some(other), false)
        .unwrap();
    let a = w
        .game
        .spawn_unit(UnitType::Monster, Some(room), false)
        .unwrap();
    w.fake.pos.insert(far, (101, 100));
    w.fake.pos.insert(a, (100, 101));
    let mon = w.monster;
    for u in [far, a, mon] {
        w.fake.mb.modes.insert(u, 12);
    }
    // Not strictly inside (no rectangle): the adjacency array in its
    // order, then each room's unit list (newest first).
    assert_eq!(find(&mut w, room, 0x1002, 10), vec![far, a, mon]);
    // x ± r, y ± r strictly inside the room's rectangle: the room alone.
    w.fake.mb.rects.insert(room, (89, 89, 22, 22));
    assert_eq!(find(&mut w, room, 0x1002, 10), vec![a, mon]);
    // Touching an edge is not strictly inside.
    w.fake.mb.rects.insert(room, (90, 89, 22, 22));
    assert_eq!(find(&mut w, room, 0x1002, 10), vec![far, a, mon]);
}

// Covers: specs/missiles/bodies-2.md §45 r1, §45 r2, §45 r3, §45 r4, §45 r5
#[test]
fn orb_mist_opens_its_object() {
    let (mut w, m) = world(row(), &[]);
    let room = w.fake.room;
    let b = w.game.spawn_unit(UnitType::Object, room, false).unwrap();
    let g = guid(&w, b);
    set_data(&mut w, m, g, 0);
    w.fake.mb.frame_cnt1 = Some(0x500);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 35, m, Some(mon)), 0);
    assert_eq!(srv_hit(&mut w, 35, m, None), 1);
    assert_eq!(w.fake.mb.modes[&b], 1);
    assert_eq!(w.fake.logged("refresh"), 1);
    // Mode 1 now: no second switch.
    srv_hit(&mut w, 35, m, None);
    assert_eq!(w.fake.mb.modes[&b], 1);
}

// Covers: specs/missiles/bodies-2.md §46 r1, §46 r2, §46 r3, §46 r4, §46 l2 r1, §46 l2 r2, §46 l2 r3, §edge-cases-original-bugs r8
#[test]
fn blade_creeper_sits_on_its_owner() {
    let mut r = row();
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    set_frames(&mut w, m, 50, 3);
    assert_eq!(srv_do(&mut w, 20, m), 1);
    // Frames left := 10, then the flight counts one down.
    assert_eq!(w.store.get(m).unwrap().current, 9);
    assert!(w.fake.log.contains(&format!("teleport {} 100 100", m.0)));
    // Distraction: a fog piece on new steps, then as 20.
    w.fake.mb.new_step = true;
    assert_eq!(srv_do(&mut w, 21, m), 1);
    assert_eq!(others(&w, m).len(), 1);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 37, m, Some(mon)), 2);
    assert_eq!(srv_hit(&mut w, 37, m, None), 0);
    let o = w.owner;
    w.fake.mb.dead.insert(o);
    assert_eq!(srv_do(&mut w, 20, m), 2);
}

// ---------------------------------------------------------------- §47–§54

// Covers: specs/missiles/bodies-2.md §47 r1, §47 l2 r1, §47 l2 r2, §47 l2 r3, §47 l2 r4, §47 l2 r5
#[test]
fn imp_spawn_draws_on_the_room_seed() {
    let (mut w, m) = world(row(), &[]);
    w.fake.mb.room_seed = Some(Seed::init_low(5));
    w.fake.mb.mon_list = vec![10, 11, 12];
    w.fake.mb.spawnable = [(10, false), (11, true), (12, false)].into();
    assert_eq!(srv_hit(&mut w, 39, m, None), 1);
    assert!(w.fake.log.contains(&"monster 11 (100, 100)".to_string()));
    let mut s = Seed::init_low(5);
    s.roll(3);
    assert_eq!(w.fake.mb.room_seed, Some(s));
    // None with `isSpawn`: fatal.
    w.fake.mb.spawnable = [(10, false), (11, false), (12, false)].into();
    srv_hit(&mut w, 39, m, None);
    assert_eq!(
        w.store.unhandled,
        [Unhandled::Fatal {
            addr: 0x005B3570,
            missile: m
        }]
    );
}

// Covers: specs/missiles/bodies-2.md §48 r1, §48 r2, §48 r3, §48 l2 r1, §48 l2 r2, §48 l2 r3, §48 l2 r4, §48 l2 r5
#[test]
fn spike_scatter_at_target() {
    let (mut w, m) = world(row(), &[sub()]);
    w.fake.mb.target_pos = Some((50, 50));
    let o = w.owner;
    let mut cx = cx!(w);
    scatter_at_target(&mut w.game, &mut cx, m, Some(o), 1, 8, 2, 7, 10);
    let mut s = Seed::init_low(50);
    let n = others(&w, m);
    assert_eq!(n.len(), 8);
    for &u in &n {
        let px = 48 + s.roll(4) as i32;
        let py = 48 + s.roll(4) as i32;
        assert_eq!(tpoint(&w, u), (px, py));
    }
    assert_eq!(*w.fake.seed(m), s);
    // Server-hit 40 with calc4 5: n = 5, r = 1 → one spike, no draw.
    let mut r = row();
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    skill(&mut w, [0, 0, 0, 0, 5]);
    w.fake.mb.target_pos = Some((50, 50));
    assert_eq!(srv_hit(&mut w, 40, m, None), 1);
    let n = others(&w, m);
    assert_eq!(n.len(), 1);
    assert_eq!(tpoint(&w, n[0]), (50, 50));
    assert_eq!(*w.fake.seed(m), Seed::init_low(50));
}

// Covers: specs/missiles/bodies-2.md §49 r1, §49 r2, §49 r3, §49 r4, §49 r5, §49 r6
#[test]
fn healing_vortex() {
    let mut r = row();
    r.progoverlay = 3;
    r.collidekill = 0;
    let (mut w, m) = world(r, &[]);
    skill(&mut w, [0; 5]);
    w.fake.mb.phys = (100, 100);
    let mon = w.monster;
    w.fake.stats.insert((mon, 6), 50);
    w.fake.mb.max_life.insert(mon, 120);
    let before = *w.fake.seed(m);
    assert_eq!(srv_hit(&mut w, 43, m, Some(mon)), 0);
    assert_eq!(w.fake.stats[&(mon, 6)], 120);
    assert_eq!(*w.fake.seed(m), before);
    assert_eq!(w.fake.logged("overlay"), 1);
    // At max life: no overlay.
    srv_hit(&mut w, 43, m, Some(mon));
    assert_eq!(w.fake.logged("overlay"), 1);
}

// Covers: specs/missiles/bodies-2.md §50 r1, §50 r2, §50 r3, §50 r4, §50 r5, §50 r6, §50 r7, §edge-cases-original-bugs r10
#[test]
fn molten_boulder_bursts_on_large() {
    let mut r = row();
    r.shitpar2 = 1;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    skill(&mut w, [0, 0, 2, 0, 0]);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 47, m, Some(mon)), 2);
    assert_eq!(others(&w, m).len(), 0);
    w.fake.mb.large.insert(mon);
    assert_eq!(srv_hit(&mut w, 47, m, Some(mon)), 1);
    let n = others(&w, m);
    assert_eq!(n.len(), 18);
    assert_eq!(w.store.get(n[0]).unwrap().total, 2);
    assert_eq!(srv_hit(&mut w, 47, m, None), 1);
    assert_eq!(others(&w, m).len(), 36);
}

// Covers: specs/missiles/bodies-2.md §51 r1, §51 r2
#[test]
fn molten_boulder_emerge() {
    let mut r = row();
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    w.fake.paths.entry(m).or_default().target_point = Some((140, 100));
    assert_eq!(srv_hit(&mut w, 48, m, None), 1);
    let n = others(&w, m);
    assert_eq!(tpoint(&w, n[0]), (140, 100));
}

// Covers: specs/missiles/bodies-2.md §52 r1, §52 r2, §52 r3
#[test]
fn plague_vines_trail() {
    let mut r = row();
    r.shitpar1 = 15;
    let (mut w, m) = world(r, &[]);
    let mon = w.monster;
    set_frames(&mut w, m, 100, 16);
    assert_eq!(srv_hit(&mut w, 50, m, Some(mon)), 2);
    set_frames(&mut w, m, 100, 15);
    assert_eq!(srv_hit(&mut w, 50, m, Some(mon)), 0);
    assert_eq!(srv_hit(&mut w, 50, m, None), 0);
}

// Covers: specs/missiles/bodies-2.md §53 r1, §53 r2, §53 r3, §53 r4, §53 r5, §edge-cases-original-bugs r13
#[test]
fn tornado_pulses_from_itself() {
    let (mut w, m) = world(row(), &[]);
    skill(&mut w, [0, 0, 3, 0, 4]);
    field(&mut w, 20, 0x1234);
    let mon = w.monster;
    w.fake.mb.area = vec![mon];
    set_frames(&mut w, m, 50, 46);
    srv_do(&mut w, 27, m);
    assert!(w.fake.log.contains(&"scan (0, 0) 3 0x1234".to_string()));
    set_frames(&mut w, m, 50, 45);
    srv_do(&mut w, 27, m);
    assert_eq!(w.fake.logged("scan"), 1);
}

// Covers: specs/missiles/bodies-2.md §54 r1, §54 r2, §54 r3
#[test]
fn volcano_debris() {
    let mut r = row();
    r.hitsubmissile1 = 1;
    r.hitsubmissile2 = 1;
    r.hitsubmissile3 = 0xFFFF;
    let (mut w, m) = world(r, &[sub()]);
    assert_eq!(srv_hit(&mut w, 51, m, None), 1);
    assert_eq!(others(&w, m).len(), 2);
}

// ---------------------------------------------------------------- §55–§62

// Covers: specs/missiles/bodies-2.md §55 r1, §55 r2, §55 r3, §55 r4
#[test]
fn recycler_heals_at_param1() {
    let mut r = row();
    r.param1 = 45;
    r.progoverlay = 2;
    let (mut w, m) = world(r, &[]);
    skill(&mut w, [20, 0, 0, 0, 0]);
    w.fake.mb.overlay_count = 5;
    let o = w.owner;
    w.fake.stats.insert((o, 6), 50 << 8);
    w.fake.mb.max_life.insert(o, 100 << 8);
    set_frames(&mut w, m, 47, 2);
    srv_do(&mut w, 29, m);
    assert_eq!(w.fake.stats[&(o, 6)], 70 << 8);
    assert_eq!(w.fake.logged("overlay"), 1);
    // Server-do 33: mana (the flight above counted a frame down).
    set_frames(&mut w, m, 47, 2);
    w.fake.stats.insert((o, 8), 10 << 8);
    w.fake.mb.max_mana.insert(o, 40 << 8);
    srv_do(&mut w, 33, m);
    assert_eq!(w.fake.stats[&(o, 8)], 18 << 8);
    // Off the frame: nothing.
    set_frames(&mut w, m, 47, 3);
    srv_do(&mut w, 33, m);
    assert_eq!(w.fake.stats[&(o, 8)], 18 << 8);
}

// Covers: specs/missiles/bodies-2.md §56 r1, §56 r2, §56 r3, §56 r4, §56 r5, §56 r6, §56 l2 r1, §56 l2 r2, §56 l2 r3, §56 l2 r4, §edge-cases-original-bugs r12
#[test]
fn rabies_plague_and_contagion() {
    let mut r = row();
    (r.param1, r.param2) = (4, 7);
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    skill(&mut w, [0; 5]);
    field(&mut w, 21, 3);
    let (o, mon) = (w.owner, w.monster);
    w.fake.mb.lists.insert((o, 3), 777);
    let g = guid(&w, mon);
    set_data(&mut w, m, 1, g);
    w.fake.seeds.insert(mon, Seed::init_low(9));
    set_frames(&mut w, m, 50, 46);
    assert_eq!(srv_do(&mut w, 30, m), 1);
    let mut s = Seed::init_low(9);
    let dx = s.roll(15) as i32 - 7;
    let dy = s.roll(15) as i32 - 7;
    assert_eq!(w.fake.seeds[&mon], s);
    let n = others(&w, m);
    assert_eq!(n.len(), 1);
    assert_eq!(tpoint(&w, n[0]), (100 + dx, 100 + dy));
    assert_eq!(w.store.get(n[0]).unwrap().target.0, 777);
    // The contagion: 10 ≤ t ≤ elem_len.
    let c = n[0];
    w.fake.mb.elem_len = 30;
    let f = w.game.frame;
    set_data(&mut w, c, f + 20, 0);
    assert_eq!(srv_hit(&mut w, 53, c, Some(o)), 2);
    assert!(w.fake.log.contains(&format!("rabies {} 20", o.0)));
    set_data(&mut w, c, f + 5, 0);
    assert_eq!(srv_hit(&mut w, 53, c, Some(o)), 1);
    // Infected unit gone: removed.
    set_data(&mut w, m, 1, 99999);
    assert_eq!(srv_do(&mut w, 30, m), 2);
}

// Covers: specs/missiles/bodies-2.md §57 r1, §57 r2, §57 r3
#[test]
fn tiger_fury_continues_as_guided_arrow() {
    let mut r = row();
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    let o = w.owner;
    w.fake.mb.dead.insert(o);
    // Server-do 7: a dead owner removes the missile.
    assert_eq!(srv_do(&mut w, 32, m), 2);
}

// Covers: specs/missiles/bodies-2.md §58 r1, §58 r2, §58 r3, §58 r4
#[test]
fn baal_spawns_the_entry_class() {
    let (mut w, m) = world(row(), &[]);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 54, m, None), 1);
    assert_eq!(w.fake.logged("spawn"), 0);
    w.fake.mb.entry_param1 = Some(312);
    assert_eq!(srv_hit(&mut w, 54, m, Some(mon)), 1);
    assert_eq!(w.fake.logged("spawn"), 0);
    srv_hit(&mut w, 54, m, None);
    assert!(w.fake.log.contains(&"spawn 312 (100, 100) 1".to_string()));
}

// Covers: specs/missiles/bodies-2.md §59 r1, §59 r2, §59 r3, §59 r4, §59 r5
#[test]
fn baal_inferno_burns_mana() {
    let mut r = row();
    r.shitpar1 = 50;
    let (mut w, m) = world(r, &[]);
    let (o, mon) = (w.owner, w.monster);
    w.fake.stats.insert((o, 8), 256_000);
    assert_eq!(srv_hit(&mut w, 55, m, Some(o)), 2);
    assert_eq!(w.fake.stats[&(o, 8)], 128_000);
    w.fake.stats.insert((o, 8), 1);
    assert_eq!(srv_hit(&mut w, 55, m, Some(o)), 2);
    assert_eq!(w.fake.stats[&(o, 8)], 0);
    assert_eq!(srv_hit(&mut w, 55, m, Some(o)), 1, "no mana");
    assert_eq!(srv_hit(&mut w, 55, m, Some(mon)), 2);
}

// Covers: specs/missiles/bodies-2.md §60
#[test]
fn baal_fx_tyrael_once() {
    let (mut w, m) = world(row(), &[]);
    w.fake.mb.quest_open = true;
    set_frames(&mut w, m, 650, 101);
    srv_do(&mut w, 36, m);
    assert_eq!(w.fake.logged("tyrael"), 0);
    set_frames(&mut w, m, 650, 100);
    srv_do(&mut w, 36, m);
    srv_do(&mut w, 36, m);
    assert_eq!(w.fake.logged("tyrael"), 1);
    assert_eq!(w.fake.logged("refresh"), 1);
    assert_eq!(srv_hit(&mut w, 57, m, None), 0);
    assert_eq!(w.fake.logged("tyrael"), 1);
    // Server-hit 57 at expiry when server-do 36 never ran.
    let (mut w, m) = world(row(), &[]);
    assert_eq!(srv_hit(&mut w, 57, m, None), 0);
    assert_eq!(w.fake.logged("tyrael"), 0, "quest test fails");
    assert_eq!(w.fake.logged("refresh"), 1);
}

// Covers: specs/missiles/bodies-2.md §61 r1, §61 r2
#[test]
fn baal_taunt_poison_ring() {
    let mut r = row();
    r.shitpar1 = 2;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    assert_eq!(srv_hit(&mut w, 59, m, None), 1);
    // b = 2: 8 even directions; a = 2: 8 more at 1, 3, …, 15.
    assert_eq!(others(&w, m).len(), 16);
}

// Covers: specs/missiles/bodies-2.md §62
#[test]
fn unused_bodies() {
    // Server-do 37: SubMissile_j at elapsed Param_j.
    let mut r = row();
    (r.param1, r.param2) = (2, 3);
    r.submissile1 = 1;
    r.submissile2 = 1;
    r.submissile3 = 0;
    let (mut w, m) = world(r, &[sub()]);
    set_frames(&mut w, m, 50, 48);
    srv_do(&mut w, 37, m);
    assert_eq!(others(&w, m).len(), 1);
    // Server-hit 6: a monster of class sHitPar1 in mode sHitPar2.
    let mut r = row();
    (r.shitpar1, r.shitpar2) = (3, 20);
    let (mut w, m) = world(r, &[]);
    w.fake.mb.monstats = 10;
    assert_eq!(srv_hit(&mut w, 6, m, None), 1);
    assert!(w.fake.log.contains(&"spawn 3 (100, 100) 1".to_string()));
    // Server-hit 11: each HitSubMissile > 0, hit-handled with sHitPar1.
    let mut r = row();
    r.hitsubmissile1 = 1;
    r.hitsubmissile2 = 0;
    r.hitsubmissile3 = 1;
    r.hitsubmissile4 = 0xFFFF;
    r.shitpar1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    assert_eq!(srv_hit(&mut w, 11, m, None), 1);
    assert_eq!(w.fake.logged("alloc 1"), 2);
    // Server-hit 23: skill server-do sHitPar1 with the missile as caster.
    let mut r = row();
    r.shitpar1 = 44;
    let (mut w, m) = world(r, &[]);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 23, m, Some(mon)), 1);
    assert_eq!(w.fake.paths[&m].target_unit, Some(mon));
    assert!(w.fake.log.contains(&format!("skilldo {} 44 7 10", m.0)));
}

// Covers: specs/missiles/missiles.md §r6-2-damage-rolls-0x005a89a0-1-14d-confirmed
#[test]
fn full_record_carries_crit_and_bypass_flags() {
    let (mut w, m) = world(row(), &[]);
    let mon = w.monster;
    for (s, v) in [(21, 10), (22, 10), (141, 1), (103, 1), (106, 5)] {
        w.fake.stats.insert((m, s), v);
    }
    let mut cx = cx!(w);
    let rec = crate::missiles::bodies_ext::full_record(&mut cx, m, Some(mon));
    // Deadly strike: physical × 2 and result 0x2000; 103 → hit flags
    // 0x100, 106 → 0x400, 104 (absent) leaves 0x200 clear.
    assert_eq!(rec.physical, 20);
    assert_eq!(rec.result & 0x2000, 0x2000);
    assert_eq!(rec.hit_flags, 0x100 | 0x400);
    // Without a unit no deadly strike or bypass applies.
    let rec = crate::missiles::bodies_ext::full_record(&mut cx, m, None);
    assert_eq!((rec.physical, rec.result, rec.hit_flags), (10, 0, 0));
}
