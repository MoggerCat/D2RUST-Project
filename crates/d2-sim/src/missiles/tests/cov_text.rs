// Spec: specs/missiles/bodies.md, specs/missiles/bodies-2.md
//! Coverage additions for the section texts and edge cases of the
//! missile bodies specs (Live notes, seed chaining, row arithmetic).

use super::ext::{others, set_data, set_frames, skill, sub, tpoint, world};
use super::*;

use super::ext::{srv_do, srv_hit};

// Covers: specs/missiles/bodies.md §1 text, §1 r1
#[test]
fn cairn_stones_sparks_window_and_missing_record() {
    // Row cairnstones (288): Param1 40, Param2 2, Param3 17, Param4 38,
    // Param5 100, SubMissile1 cairnstonessky, Range 300.
    let mut r = row();
    (r.param1, r.param2, r.param3, r.param4, r.param5) = (40, 2, 17, 38, 100);
    r.range = 300;
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    // Live: sparks between frames left 260 and 40, every 2 elapsed
    // frames within range 17. Frames left f = 300 - elapsed.
    let mut made = Vec::new();
    for f in 1..=300i16 {
        set_frames(&mut w, m, 300, f);
        let before = others(&w, m).len();
        // Keep the portal out of the way (Param4 38 opens at f <= 140).
        w.store.get_mut(m).unwrap().target.0 = 1;
        srv_do(&mut w, 17, m);
        if others(&w, m).len() > before {
            made.push(f);
        }
    }
    assert!(made.iter().all(|&f| f < 260 && f > 40));
    assert!(made.iter().all(|&f| (300 - f) % 2 == 0));
    assert_eq!(made.first(), Some(&42));
    assert_eq!(made.last(), Some(&258));
    assert_eq!(made.len(), 109);
    // Server-do 17 with no record: removed.
    w.store.get_mut(m).unwrap().class = 9;
    assert_eq!(srv_do(&mut w, 17, m), 2);
}

// Covers: specs/missiles/bodies.md §3 text, §edge-cases-original-bugs r1
#[test]
fn baal_taunt_control_picks_the_same_slot_every_frame() {
    let mut r = row();
    (r.param1, r.param2, r.param3, r.param4) = (25, 3, 45, 0);
    r.submissile1 = 1;
    r.submissile2 = 2;
    let mut s2 = sub();
    s2.psrvdofunc = 0;
    let (mut w, m) = world(r, &[sub(), s2]);
    // Elapsed 90 is a multiple of both intervals (3 and 45): the slot
    // is whatever roll(2) gives, reset from the same x every run.
    let mut classes = Vec::new();
    for _ in 0..4 {
        set_frames(&mut w, m, 200, 110);
        let before = w.store.missiles().count();
        srv_do(&mut w, 34, m);
        assert_eq!(w.store.missiles().count(), before + 1);
        let last = w.store.missiles().last().unwrap();
        classes.push(w.store.get(last).unwrap().class);
    }
    assert!(classes.iter().all(|&c| c == classes[0]), "{classes:?}");
    // The same slot as a fresh seed from x gives.
    let mut s = Seed::init_low(100);
    let j = s.roll(2) as i32;
    assert_eq!(i32::from(classes[0]), j + 1);
}

// Covers: specs/missiles/bodies.md §5 text, §edge-cases-original-bugs r1
#[test]
fn baal_taunt_lightning_ignores_the_unit_and_reseeds_from_x() {
    let mut r = row();
    r.shitpar1 = 10;
    r.hitsubmissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    let mon = w.monster;
    assert_eq!(srv_hit(&mut w, 58, m, Some(mon)), 1);
    assert_eq!(srv_hit(&mut w, 58, m, None), 1);
    let n = others(&w, m);
    assert_eq!(n.len(), 2);
    // The unit argument is not read; equal positions give equal choices.
    assert_eq!(tpoint(&w, n[0]), tpoint(&w, n[1]));
}

// Covers: specs/missiles/bodies.md §6 text
#[test]
fn plague_javelin_live_rows() {
    // plaguejavelin: sHitPar1 1, 2, 3; clouds (Param1 2, Param2 4).
    let mut r = row();
    (r.shitpar1, r.shitpar2, r.shitpar3) = (1, 2, 3);
    r.hitsubmissile1 = 1;
    let mut cloud = sub();
    (cloud.param1, cloud.param2) = (2, 4);
    cloud.range = 10;
    cloud.subloop = 1;
    cloud.substart = 0;
    cloud.substop = 5;
    let (mut w, m) = world(r.clone(), &[cloud.clone()]);
    let mon = w.monster;
    srv_hit(&mut w, 2, m, Some(mon));
    let n = others(&w, m);
    // 8 clouds (even directions) and 15 more, all with 3 loops.
    assert_eq!(n.len(), 8 + 15);
    for &u in &n {
        assert_eq!(w.store.get(u).unwrap().total, 10 + 3 * 5);
    }
    // Param1 2 -> 256, Param2 4 -> 512; step 7 takes 75 %: 192 / 384.
    let vels: Vec<_> = w
        .fake
        .calls
        .iter()
        .filter(|c| c.starts_with("vel ") && *c != "vel 0")
        .cloned()
        .collect();
    assert_eq!(vels.len(), 23);
    assert!(vels[..8].iter().all(|v| v == "vel 192"));
    assert!(vels[8..].iter().all(|v| v == "vel 384"));
    // rancidgasepotion (a = 0): only the 8.
    r.shitpar1 = 0;
    let (mut w, m) = world(r.clone(), &[cloud.clone()]);
    srv_hit(&mut w, 2, m, None);
    assert_eq!(others(&w, m).len(), 8);
    // plaguejavlinexplode: no Param1 / Param2: the pieces do not move.
    cloud.param1 = 0;
    cloud.param2 = 0;
    let (mut w, m) = world(r, &[cloud]);
    srv_hit(&mut w, 2, m, None);
    assert!(w.fake.calls.iter().all(|c| !c.starts_with("vel ") || c == "vel 0"));
    assert_eq!(others(&w, m).len(), 8);
}

// Covers: specs/missiles/bodies.md §edge-cases-original-bugs r3
#[test]
fn volcano_without_an_owner_still_flies() {
    let mut r = row();
    (r.param1, r.param2, r.param3, r.param4, r.param5) = (0, 0, 2, 128, 30);
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    skill(&mut w, [0, 0, 4, 0, 3]);
    set_data(&mut w, m, 77, 0);
    set_frames(&mut w, m, 50, 47);
    let o = w.owner;
    w.game.remove_unit(o).unwrap();
    let before = w.store.get(m).unwrap().current;
    let res = srv_do(&mut w, 28, m);
    assert_ne!(res, 2);
    assert_eq!(others(&w, m).len(), 0, "the throw is skipped");
    assert_eq!(w.store.get(m).unwrap().target.0, 77, "no draw");
    assert_eq!(w.store.get(m).unwrap().current, before - 1, "it flew");
}

// Covers: specs/missiles/bodies-2.md §edge-cases-original-bugs r11
#[test]
fn tiger_fury_never_positions_its_trail() {
    let mut r = row();
    r.submissile1 = 1;
    let (mut w, m) = world(r, &[sub()]);
    w.fake.mb.new_step = true;
    srv_do(&mut w, 32, m);
    // The sub-missile's start is (0, 0), not the missile's (100, 100):
    // nothing is created at the missile.
    assert!(others(&w, m).iter().all(|&u| w.fake.pos[&u] != (100, 100)));
}

// Covers: specs/missiles/bodies-2.md §edge-cases-original-bugs r9
#[test]
fn spawn_for_level_dies_on_empty_or_unspawnable_lists() {
    let (mut w, m) = world(row(), &[]);
    w.fake.mb.room_seed = Some(Seed::init_low(5));
    // No `mon` entries.
    w.fake.mb.mon_list = vec![];
    srv_hit(&mut w, 39, m, None);
    assert_eq!(w.store.unhandled.len(), 1);
    // An invalid class in the list (not in the spawn table).
    let (mut w, m) = world(row(), &[]);
    w.fake.mb.room_seed = Some(Seed::init_low(5));
    w.fake.mb.mon_list = vec![10];
    w.fake.mb.spawnable.clear();
    srv_hit(&mut w, 39, m, None);
    assert_eq!(
        w.store.unhandled,
        [Unhandled::Fatal {
            addr: 0x005B3570,
            missile: m
        }]
    );
}
