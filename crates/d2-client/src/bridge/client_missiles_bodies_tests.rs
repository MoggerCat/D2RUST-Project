// Spec: specs/missiles/client.md (§C3 r19, r21, §C7 r3, §C13 function 2), specs/missiles/client-bodies.md (§B3 r1, §B4 function 3, §B5 r4, r6)
//! The client missile bodies of [`super::bodies`] and the motion record
//! of the create, on synthetic rows.

use super::*;
use crate::bridge::world::UnitOrigin;
use crate::rules::unit_composite::motion;
use d2_data::tables::{Missiles, Record, Skilldesc};
use d2_sim::skills::SkillTables;

fn row(f: u16) -> ClientMissileRow {
    ClientMissileRow {
        range: 40,
        anim_len: 3,
        anim_speed: 16,
        clt_do_func: f,
        ..ClientMissileRow::default()
    }
}

fn at(x: i32, y: i32, class: u32) -> CreateRecord {
    CreateRecord {
        flags: flag::POSITION,
        class,
        x,
        y,
        level: 3,
        ..CreateRecord::default()
    }
}

fn env(rows: &[ClientMissileRow]) -> Env<'_> {
    Env {
        rows,
        lights: true,
        skills: None,
        monsters: &[],
    }
}

// Covers: specs/missiles/client.md §c13-function-bodies-specified-here
#[test]
fn function_2_holds_blood_on_screen_and_ends_it_off_screen() {
    let rows = vec![ClientMissileRow::default(), row(FN_BLOOD)];
    let mut w = ClientWorld::default();
    let k = create(&mut w, &rows, &at(100, 100, 1), true)
        .unwrap()
        .unwrap();
    // No drawn frame yet: the origin is unknown.
    assert!(update(&mut w, &rows, k, true).is_err());
    let m = w.objclient.missiles[&k];
    let p = crate::rules::camera::moving_to_client(m.pos.0, m.pos.1);
    // On screen: (10, 10) from the origin getters.
    w.unit_origin = Some(UnitOrigin {
        x: p.x - 10,
        y: p.y - 10,
        width: 800,
        play_height: 560,
    });
    update(&mut w, &rows, k, true).unwrap();
    // Frames left := 128, then the step counts one down.
    assert_eq!(w.objclient.missiles[&k].current, 127);
    // The box is inclusive: y up to H − 40 + 64.
    w.unit_origin = Some(UnitOrigin {
        x: p.x,
        y: p.y - 624,
        width: 800,
        play_height: 560,
    });
    update(&mut w, &rows, k, true).unwrap();
    assert_eq!(w.objclient.missiles[&k].current, 127);
    // At the animation end: flat and path velocity 0.
    let m = w.objclient.missiles.get_mut(&k).unwrap();
    (m.frame, m.velocity) = (2 << 8, 99);
    update(&mut w, &rows, k, true).unwrap();
    let m = w.objclient.missiles[&k];
    assert!(m.flat && m.velocity == 0);
    // Off screen (one pixel past the right edge): end(none, 0).
    w.unit_origin = Some(UnitOrigin {
        x: p.x - 801,
        y: p.y,
        width: 800,
        play_height: 560,
    });
    update(&mut w, &rows, k, true).unwrap();
    assert!(!w.objclient.set_c.contains_key(&k));
}

fn blank<T: Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

/// Skills tables whose `misscode` holds the constant 2 at offset 0
/// (`data/calc-expressions.md`: 0x07 n pushes n, 0x00 ends).
fn tables() -> SkillTables {
    SkillTables {
        skills: Vec::new(),
        skilldesc: vec![blank::<Skilldesc>()],
        missiles: vec![blank::<Missiles>(); 3],
        skills_code: Vec::new(),
        miss_code: vec![0x07, 2, 0x00],
        level_cap: 99,
        stat_count: 359,
    }
}

// Covers: specs/missiles/client-bodies.md §b4-do-bodies-emitters
// Covers: specs/missiles/client-bodies.md §b3-shared-create-helpers
#[test]
fn function_3_drops_a_looping_sub_missile_on_each_new_sub_tile() {
    let mut r = row(FN_SUB_AT_STEP);
    r.clt_sub = [2, -1, -1];
    let mut child = row(FN_DEFAULT_STEP);
    (child.range, child.sub_loop, child.sub_start, child.sub_stop) = (4, 1, 1, 3);
    let t = tables();
    for (calc, loops) in [(0, true), (99, false)] {
        r.clt_calc1 = calc;
        let rows = vec![ClientMissileRow::default(), r, child];
        let e = Env {
            skills: Some(&t),
            ..env(&rows)
        };
        let mut w = ClientWorld::default();
        let k = create(&mut w, &rows, &at(100, 100, 1), true)
            .unwrap()
            .unwrap();
        // No new sub-tile: nothing made.
        update_with(&mut w, &e, k).unwrap();
        assert_eq!(w.objclient.set_c.len(), 1);
        // The flag of the last step (a stopped missile keeps it, §B2 r4).
        w.objclient.missiles.get_mut(&k).unwrap().new_step = true;
        update_with(&mut w, &e, k).unwrap();
        let kids: Vec<_> = w
            .objclient
            .set_c
            .iter()
            .filter(|(_, u)| u.class == 2)
            .map(|(c, u)| (*c, u.position))
            .collect();
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].1, Some((100, 100)));
        let c = w.objclient.missiles[&kids[0].0];
        // v = 2 > 0: flag 8 with R+0x34 = 2 × 3 − 2 = 4 loops: F = 4 +
        // (3 − 1) × 4; no formula (offset past the buffer) → v = 0, F = 4.
        assert_eq!(c.total, if loops { 12 } else { 4 }, "calc {calc}");
        assert_eq!((c.velocity, c.skill, c.level), (0, 0, 3));
    }
    // Without the skills tables the evaluator cannot run.
    let rows = vec![ClientMissileRow::default(), r, child];
    let mut w = ClientWorld::default();
    let k = create(&mut w, &rows, &at(100, 100, 1), true)
        .unwrap()
        .unwrap();
    w.objclient.missiles.get_mut(&k).unwrap().new_step = true;
    assert!(update(&mut w, &rows, k, true).is_err());
}

// Covers: specs/missiles/client-bodies.md §b5-do-bodies-animation-steering-timed-effects
#[test]
fn function_59_lives_while_its_height_is_in_its_window() {
    let rows = vec![ClientMissileRow::default(), row(FN_HEIGHT_WINDOW)];
    let made = |vz: i32, d2c: i32| {
        let mut w = ClientWorld::default();
        let k = create(&mut w, &rows, &at(100, 100, 1), true)
            .unwrap()
            .unwrap();
        let m = w.objclient.missiles.get_mut(&k).unwrap();
        (m.motion.vel[2], m.d2c, m.d28) = (vz, d2c, 0);
        update(&mut w, &rows, k, true).unwrap();
        (w, k)
    };
    // z = 10 ≪ above d2C = 0: removed.
    let (w, k) = made(10 << 11, 0);
    assert!(!w.objclient.set_c.contains_key(&k));
    // Inside the window: stepped.
    let (w, k) = made(10 << 11, 20 << 11);
    assert_eq!(w.objclient.missiles[&k].current, 39);
    // Falling below the limits stops the record at z = 0 (§8 r5): inside.
    let (w, k) = made(-(10 << 11), 0);
    assert_eq!(w.objclient.missiles[&k].motion.pos[2], 0);
    assert!(w.objclient.set_c.contains_key(&k));
}

/// Two `baalfx spirit` missiles: U (d28 = `k`) and m (d28 = U's GUID,
/// d2C = `b`), m's seed {lo, 666}.
fn spirits(rows: &[ClientMissileRow], k: i32, b: i32, lo: u32) -> (ClientWorld, UnitKey) {
    let mut w = ClientWorld::default();
    let u = create(&mut w, rows, &at(95, 100, 1), true)
        .unwrap()
        .unwrap();
    let m = create(&mut w, rows, &at(100, 100, 1), true)
        .unwrap()
        .unwrap();
    w.objclient.missiles.get_mut(&u).unwrap().d28 = k;
    let mm = w.objclient.missiles.get_mut(&m).unwrap();
    (mm.d28, mm.d2c) = (u.guid as i32, b);
    w.objclient.set_c.get_mut(&m).unwrap().seed = Some((lo, 666));
    (w, m)
}

// Covers: specs/missiles/client-bodies.md §b5-do-bodies-animation-steering-timed-effects
// Covers: specs/missiles/client.md §c14-seeds-capture-only
#[test]
fn function_65_wakes_circles_and_leaves_for_the_fixed_point() {
    let rows = vec![ClientMissileRow::default(), row(FN_SPIRIT)];
    let e = env(&rows);
    // r3 (b = 1, k = 1 < 2: no rnd(25)): a quarter turn around U.
    let (mut w, m) = spirits(&rows, 1, 1, 5);
    update_with(&mut w, &e, m).unwrap();
    // (dx, dy) = (5, 0), negated when m's GUID has bit 1.
    let want = if m.guid & 2 != 0 {
        (100, 95)
    } else {
        (100, 105)
    };
    assert_eq!(w.objclient.missiles[&m].target_point, want);
    assert_eq!(w.objclient.set_c[&m].seed, Some((5, 666)), "no draw");
    // r2 (b = 0, k = 0): one seed step; lo' mod 10 = 0 wakes it.
    let (wake, sleep) = {
        let mut wake = None;
        let mut sleep = None;
        for lo in 1..200u32 {
            let mut s = Seed::new(lo, 666);
            if s.step().is_multiple_of(10) {
                wake.get_or_insert(lo);
            } else {
                sleep.get_or_insert(lo);
            }
        }
        (wake.unwrap(), sleep.unwrap())
    };
    let (mut w, m) = spirits(&rows, 0, 0, sleep);
    update_with(&mut w, &e, m).unwrap();
    assert_eq!(w.objclient.missiles[&m].d2c, 0);
    let (mut w, m) = spirits(&rows, 0, 0, wake);
    update_with(&mut w, &e, m).unwrap();
    let mut s = Seed::new(wake, 666);
    s.step();
    let up = s.roll(2) != 0;
    let mm = w.objclient.missiles[&m];
    assert_eq!((mm.d2c, mm.velocity), (1, 0xF00));
    assert_eq!(mm.motion.flags & motion::RESTARTED, motion::RESTARTED);
    assert_eq!(mm.motion.vel, if up { [0, 0, 1 << 11] } else { [0; 3] });
    assert_eq!(w.objclient.set_c[&m].seed, Some((s.lo, s.hi)));
    // r1 (b = 0 ≠ k = 2): rnd(25) = 0 → d2C := 2, off to (15135, 5900).
    let lo = (1..500u32)
        .find(|&lo| Seed::new(lo, 666).roll(25) == 0)
        .unwrap();
    let (mut w, m) = spirits(&rows, 2, 0, lo);
    update_with(&mut w, &e, m).unwrap();
    let mm = w.objclient.missiles[&m];
    assert_eq!(
        (mm.d2c, mm.velocity, mm.target_point),
        (2, 0xF00, (15135, 5900))
    );
    // No U: step only.
    let (mut w, m) = spirits(&rows, 2, 0, lo);
    w.objclient.missiles.get_mut(&m).unwrap().d28 = 9999;
    update_with(&mut w, &e, m).unwrap();
    assert_eq!(w.objclient.missiles[&m].d2c, 0);
    assert_eq!(w.objclient.set_c[&m].seed, Some((lo, 666)));
}

// Covers: specs/missiles/client.md §c3-create-allocation-and-frames
// Covers: specs/missiles/client.md §c7-default-step-0x004d30c0-function-1
// Covers: specs/render/unit-composite.md §8 r2, §8 r5
#[test]
fn an_arc_missile_ends_when_its_motion_lands() {
    let rows = vec![ClientMissileRow::default(), row(FN_DEFAULT_STEP)];
    let mut w = ClientWorld::default();
    // Without flag 0x100: restarted, never "not moving".
    let k = create(&mut w, &rows, &at(100, 100, 1), true)
        .unwrap()
        .unwrap();
    assert_eq!(w.objclient.missiles[&k].motion.flags, motion::RESTARTED);
    update(&mut w, &rows, k, true).unwrap();
    assert!(w.objclient.missiles[&k].motion.moving());
    // Flag 0x100: the timed arc over the 40 frames (height 6), then one
    // motion update at the create.
    let mut rec = at(100, 100, 1);
    rec.flags |= flag::ARC | flag::TARGET_ABSOLUTE;
    (rec.tx, rec.ty, rec.arc_height) = (103, 104, 6);
    let k = create(&mut w, &rows, &rec, true).unwrap().unwrap();
    let m = w.objclient.missiles[&k];
    assert_eq!(m.direction, ((103 + 104) & 63) as u8);
    assert_eq!((m.motion.flags, m.motion.ticks_left), (motion::TIMED, 39));
    let n = 40;
    let vz = (-6 * 2048 - (-0x1000 * n * n / 2)) / n;
    assert_eq!(m.motion.acc[2], -0x1000);
    assert_eq!(m.motion.pos[2], (6 << 11) + vz);
    assert_eq!(m.motion.vel[2], vz - 0x1000);
    // Landed (ticks run out): the next update ends it (§C7 r3) although
    // frames are left.
    w.objclient.missiles.get_mut(&k).unwrap().motion.ticks_left = 0;
    update(&mut w, &rows, k, true).unwrap();
    assert!(!w.objclient.set_c.contains_key(&k));
}

// Covers: specs/missiles/client.md §c9-end-0x004d2d70-m-u-forced
#[test]
fn the_explosion_takes_the_missiles_motion_position() {
    let mut r = row(FN_DEFAULT_STEP);
    (r.always_explode, r.explosion_missile) = (true, 2);
    let mut boom = row(FN_DEFAULT_STEP);
    boom.explosion_missile = -1;
    let rows = vec![ClientMissileRow::default(), r, boom];
    let mut w = ClientWorld::default();
    let p = UnitKey::new(PLAYER, 1);
    let mut u = crate::bridge::world::ClientUnit::new(p);
    u.position = Some((100, 100));
    w.units.insert(p, u);
    let mut rec = at(100, 100, 1);
    rec.owner = Some(p);
    let k = create(&mut w, &rows, &rec, true).unwrap().unwrap();
    w.objclient.missiles.get_mut(&k).unwrap().motion.pos = [1, 2, 3 << 11];
    let x = end(&mut w, &rows, k, false, true).unwrap().unwrap();
    let xm = w.objclient.missiles[&x].motion;
    // Done then restarted: flag 8, flag 1 clear.
    assert_eq!((xm.pos, xm.flags), ([1, 2, 3 << 11], motion::RESTARTED));
}

/// One active room (90, 90)–(110, 110) of `level` (1 = the Rogue
/// Encampment, a town).
fn room_world(level: u16) -> ClientWorld {
    use crate::bridge::drlg::DrlgRoomId;
    use crate::bridge::world::ActiveRoom;
    ClientWorld {
        active_rooms: Some(vec![ActiveRoom {
            x0: 90,
            y0: 90,
            w: 20,
            h: 20,
            level,
            room: DrlgRoomId(1),
        }]),
        ..ClientWorld::default()
    }
}

// Covers: specs/missiles/client.md §c6-per-update-dispatch-0x004d2c70
// Covers: specs/missiles/client.md §c7-default-step-0x004d30c0-function-1
#[test]
fn town_rooms_remove_missiles_without_town_and_clamp_src_town() {
    let mut town_row = row(FN_DEFAULT_STEP);
    town_row.town = true;
    let rows = vec![ClientMissileRow::default(), row(FN_DEFAULT_STEP), town_row];
    // §C6 r4: no `Town` in a town room → removed; with `Town` stepped.
    let mut w = room_world(1);
    let gone = create(&mut w, &rows, &at(100, 100, 1), true)
        .unwrap()
        .unwrap();
    let kept = create(&mut w, &rows, &at(100, 100, 2), true)
        .unwrap()
        .unwrap();
    update(&mut w, &rows, gone, true).unwrap();
    update(&mut w, &rows, kept, true).unwrap();
    assert!(!w.objclient.set_c.contains_key(&gone));
    assert_eq!(w.objclient.missiles[&kept].current, 39);
    // Out of town: stepped.
    let mut w = room_world(2);
    let k = create(&mut w, &rows, &at(100, 100, 1), true)
        .unwrap()
        .unwrap();
    update(&mut w, &rows, k, true).unwrap();
    assert_eq!(w.objclient.missiles[&k].current, 39);
    // §C7 r8: `CltSrcTown` 5, no owner: frames left 39 ≥ 5 → 5, frame
    // (`AnimLen` 8 − 5) << 8; a looping row keeps its frame.
    for looping in [false, true] {
        let mut r = row(FN_DEFAULT_STEP);
        (r.clt_src_town, r.anim_len, r.anim_speed, r.loop_anim) = (5, 8, 0, looping);
        let rows = vec![ClientMissileRow::default(), r];
        let mut w = ClientWorld::default();
        let k = create(&mut w, &rows, &at(100, 100, 1), true)
            .unwrap()
            .unwrap();
        update(&mut w, &rows, k, true).unwrap();
        let m = w.objclient.missiles[&k];
        assert_eq!(m.current, 5);
        assert_eq!(m.frame, if looping { 0 } else { 3 << 8 });
        // An owner out of town: no clamp.
        let mut w = ClientWorld::default();
        let p = UnitKey::new(PLAYER, 1);
        let mut u = crate::bridge::world::ClientUnit::new(p);
        u.position = Some((100, 100));
        w.units.insert(p, u);
        let mut rec = at(100, 100, 1);
        rec.owner = Some(p);
        let k = create(&mut w, &rows, &rec, true).unwrap().unwrap();
        update(&mut w, &rows, k, true).unwrap();
        assert_eq!(w.objclient.missiles[&k].current, 39);
    }
}

// Covers: specs/missiles/client-bodies.md §b5-do-bodies-animation-steering-timed-effects
// Covers: specs/ui/controls.md §6 r9
#[test]
fn function_7_steers_toward_a_hostile_living_target_in_range() {
    use crate::bridge::world::{ClientUnit, MonsterClass, MonsterSetup, MONSTER};
    let mut r = row(FN_GUIDED);
    (r.vel, r.clt_param) = (8, [2, 0, 0, 0, 0]);
    let rows = vec![ClientMissileRow::default(), r];
    let monsters = vec![
        Some(MonsterClass::default()),
        Some(MonsterClass {
            setup: Some(MonsterSetup {
                align: 1,
                ..MonsterSetup::default()
            }),
            ..MonsterClass::default()
        }),
    ];
    let e = Env {
        monsters: &monsters,
        ..env(&rows)
    };
    let p = UnitKey::new(PLAYER, 1);
    let t = UnitKey::new(MONSTER, 9);
    let run = |class: u32, at_t: (u16, u16), mode: u32, d28: i32| {
        let mut w = ClientWorld::default();
        let mut u = ClientUnit::new(p);
        u.position = Some((100, 100));
        w.units.insert(p, u);
        let mut u = ClientUnit::new(t);
        (u.position, u.class, u.mode) = (Some(at_t), class, mode);
        w.units.insert(t, u);
        let mut rec = at(100, 100, 1);
        rec.flags |= flag::TARGET_RELATIVE;
        (rec.owner, rec.target, rec.tx) = (Some(p), Some(t), 1);
        rec.owner_dir64 = Some(0);
        let k = create(&mut w, &rows, &rec, true).unwrap().unwrap();
        let m = w.objclient.missiles.get_mut(&k).unwrap();
        // Aim away first, so a re-path shows.
        (m.dir_vec, m.d28) = ((-4096, 0), d28);
        update_with(&mut w, &e, k).unwrap();
        (w.objclient.missiles[&k].dir_vec, w)
    };
    // Hostile, alive, distance 10 at elapsed 0: re-path toward T (east).
    let (v, _) = run(0, (110, 100), 1, 0);
    assert!(v.0 > 0 && v.1 == 0, "{v:?}");
    // Too near (distance 2 < 4), an ally (`Align` 1), or dead: no re-path.
    assert_eq!(run(0, (102, 100), 1, 0).0, (-4096, 0));
    assert_eq!(run(1, (110, 100), 1, 0).0, (-4096, 0));
    assert_eq!(run(0, (110, 100), 12, 0).0, (-4096, 0));
    // A town room removes it (even with `Town`, which passes §C6 r4).
    r.town = true;
    let rows = vec![ClientMissileRow::default(), r];
    let e = Env {
        monsters: &monsters,
        ..env(&rows)
    };
    let mut w = room_world(1);
    let k = create(&mut w, &rows, &at(100, 100, 1), true)
        .unwrap()
        .unwrap();
    w.objclient.missiles.get_mut(&k).unwrap().velocity = 0;
    update_with(&mut w, &e, k).unwrap();
    assert!(!w.objclient.set_c.contains_key(&k));
    // d28 bit 0: a target unit gone from the model is dropped.
    let (_, mut w) = run(0, (110, 100), 1, 1);
    w.units.remove(&t);
    let k = *w.objclient.missiles.keys().next().unwrap();
    update_with(&mut w, &e, k).unwrap();
    assert_eq!(w.objclient.missiles[&k].target_unit, None);
}

// Covers: specs/missiles/client-bodies.md §b5-do-bodies-animation-steering-timed-effects
#[test]
fn function_9_drops_its_meteor_and_grows_its_light() {
    use d2_data::tables::Skills;
    let mut r = row(FN_METEOR);
    (r.light, r.clt_sub, r.clt_param) = (2, [2, 3, -1], [5, 3, 2, 0, 0]);
    let mut child = row(FN_DEFAULT_STEP);
    child.light = 0;
    let rows = vec![ClientMissileRow::default(), r, child, child];
    // Skill 0's calc1 at offset 0: the constant 6 → q = 10.
    let mut t = tables();
    t.skills = vec![blank::<Skills>()];
    t.skills_code = vec![0x07, 6, 0x00];
    let e = Env {
        skills: Some(&t),
        ..env(&rows)
    };
    let mut w = ClientWorld::default();
    let k = create(&mut w, &rows, &at(100, 100, 1), true)
        .unwrap()
        .unwrap();
    update_with(&mut w, &e, k).unwrap();
    // Elapsed 0: S1 then S2 (> 0), motion (−c·a, 0, b·a) ≪ = (−10, 0,
    // 15) ≪, velocity (2, 0, −3) ≪ (a, b, c = 5, 3, 2).
    for class in [2, 3] {
        let kids: Vec<_> = w
            .objclient
            .set_c
            .iter()
            .filter(|(_, u)| u.class == class)
            .map(|(c, _)| *c)
            .collect();
        assert_eq!(kids.len(), 1, "class {class}");
        let mm = w.objclient.missiles[&kids[0]].motion;
        assert_eq!(mm.pos, [-10 << 11, 0, 15 << 11]);
        assert_eq!(mm.vel, [2 << 11, 0, -3 << 11]);
    }
    // The light (radius 2) grows by one on (elapsed + 1) mod 10 = 0, up
    // to n = 6.
    let id = super::light_of(&w, k).unwrap();
    for _ in 1..9 {
        update_with(&mut w, &e, k).unwrap();
    }
    assert_eq!(w.lights.radius(id), Some(2));
    update_with(&mut w, &e, k).unwrap();
    assert_eq!(w.lights.radius(id), Some(3));
    // No skills tables: an error; a skill outside the table: removed.
    assert!(update(&mut w, &rows, k, true).is_err());
    w.objclient.missiles.get_mut(&k).unwrap().skill = 5;
    update_with(&mut w, &e, k).unwrap();
    assert!(!w.objclient.set_c.contains_key(&k));
}

// Covers: specs/ui/controls.md §6 r9
#[test]
fn hostility_between_client_units() {
    use crate::bridge::combat::hostile_between;
    use crate::bridge::world::{ClientUnit, MonsterClass, MonsterSetup, ITEM, MONSTER};
    let monsters = vec![
        Some(MonsterClass::default()),
        Some(MonsterClass {
            setup: Some(MonsterSetup {
                align: 1,
                ..MonsterSetup::default()
            }),
            ..MonsterClass::default()
        }),
    ];
    let mut w = ClientWorld::default();
    let (p, q) = (UnitKey::new(PLAYER, 1), UnitKey::new(PLAYER, 2));
    let (foe, ally) = (UnitKey::new(MONSTER, 3), UnitKey::new(MONSTER, 4));
    for (k, class) in [(p, 0), (q, 0), (foe, 0), (ally, 1)] {
        let mut u = ClientUnit::new(k);
        u.class = class;
        w.units.insert(k, u);
    }
    let h = |a, b| hostile_between(&w, &monsters, a, b);
    assert!(h(p, foe) && h(foe, p) && h(ally, foe));
    assert!(!h(p, ally) && !h(p, p) && !h(p, q) && !h(foe, foe));
    assert!(h(p, UnitKey::new(ITEM, 5)));
}
