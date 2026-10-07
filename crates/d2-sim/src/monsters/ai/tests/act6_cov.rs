// Spec: specs/monsters/ai-bodies-6.md (rules the first pass left unclaimed); fakes from the parent test module
use super::act2::{act_row, give_skill, logged, param_of, point_mode, seeded, set_param_of, world};
use super::act6::{grow, own};
use super::npc::{seed_with, steps_since, unit_mode};
use super::*;

/// A melee NecroPet owned by the player: good alignment so the searches
/// use `good_target_search`, owner 3 tiles east.
fn pet() -> World {
    let mut w = world(act_row(67, &[]));
    own(&mut w);
    w.fake.align = 1;
    w.fake.pos.insert(w.player, (103, 100));
    w
}

/// The wander point of `wander'` after `skip` draws of `lo` around the
/// pet's own position (100, 100), radius 4.
fn wander4(lo: u32, skip: usize) -> String {
    let mut s = Seed::init_low(lo);
    for _ in 0..skip {
        s.step();
    }
    let (x, y) = wander_point(&mut s, (100, 100), 4);
    point_mode(mode::WALK, x, y)
}

// Covers: specs/monsters/ai-bodies-6.md §3 r4, §3 r6, §3 r9
#[test]
fn necro_melee_owner_view_replaces_a_far_search_target_and_lunges() {
    // E = 10 > 6 (search distance): S := 0, T0 = the enemy within 36 →
    // S := T0 with M as the search wrote it (0): one draw (the follow's r),
    // no follow (S set, D ≤ 80), no wander, lunge: velocity (0, 0, 12)
    // and a walk at S.
    let (mut w, lo) = seeded(act_row(67, &[]), 1, |v| v[0] >= 15);
    own(&mut w);
    w.fake.align = 1;
    w.fake.pos.insert(w.player, (103, 100));
    let e = w.add_unit(UnitType::Monster, (112, 100));
    w.fake.good = Some((e, 10));
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [unit_mode(mode::WALK, e)]);
    assert_eq!(w.vel_request().steps, 12);
    assert_eq!(steps_since(&w, lo), 1);
    // E = 50: beyond the cap (24) → no search target; T0 is the same
    // enemy but 49 away (≥ 36) → S stays 0 → the quiet follow (r < 15)
    // returns 0 → wander' 4 (r7).
    let (mut w, lo) = seeded(act_row(67, &[]), 1, |v| v[0] < 15);
    own(&mut w);
    w.fake.align = 1;
    w.fake.pos.insert(w.player, (103, 100));
    let e = w.add_unit(UnitType::Monster, (150, 100));
    w.fake.good = Some((e, 50));
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [wander4(lo, 1)]);
}

// Covers: specs/monsters/ai-bodies-6.md §3 r5
#[test]
fn necro_melee_without_direct_reach_drops_the_target() {
    let (mut w, lo) = seeded(act_row(67, &[]), 1, |v| v[0] < 15);
    own(&mut w);
    w.fake.align = 1;
    w.fake.pos.insert(w.player, (103, 100));
    let e = w.add_unit(UnitType::Monster, (104, 100));
    w.fake.good = Some((e, 4));
    w.fake.reach_fails = true;
    w.think_with(None, 0, false);
    // S := 0 by `0x005DC640`: wander', not a lunge at the enemy.
    assert_eq!(w.fake.modes(), [wander4(lo, 1)]);
    // Reachable: the lunge.
    let mut w = pet();
    w.seed(lo);
    let e = w.add_unit(UnitType::Monster, (104, 100));
    w.fake.good = Some((e, 4));
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [unit_mode(mode::WALK, e)]);
}

// Covers: specs/monsters/ai-bodies-6.md §3 r7
#[test]
fn necro_melee_wanders_without_a_target_or_in_town() {
    // No S: wander' 4.
    let (mut w, lo) = seeded(act_row(67, &[]), 1, |v| v[0] < 15);
    own(&mut w);
    w.fake.align = 1;
    w.fake.pos.insert(w.player, (103, 100));
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [wander4(lo, 1)]);
    // S set but the unit's room is a town: wander' 4 as well.
    let mut w = pet();
    w.seed(lo);
    w.fake.town.insert(w.room);
    let e = w.add_unit(UnitType::Monster, (104, 100));
    w.fake.good = Some((e, 4));
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [wander4(lo, 1)]);
}

// Covers: specs/monsters/ai-bodies-6.md §3 r8
#[test]
fn necro_melee_in_contact_attacks_or_idles() {
    // M ≠ 0 (the enemy is in melee range): roll(100) < 80 → A1 at S, else
    // idle 10. Draws: the follow's r, then the roll.
    for (hit, want_attack) in [(true, true), (false, false)] {
        let (mut w, lo) = seeded(act_row(67, &[]), 2, |v| (v[1] < 80) == hit);
        own(&mut w);
        w.fake.align = 1;
        w.fake.pos.insert(w.player, (103, 100));
        let e = w.add_unit(UnitType::Monster, (104, 100));
        w.fake.good = Some((e, 4));
        w.fake.melee.insert(e);
        w.think_with(None, 0, false);
        if want_attack {
            assert_eq!(w.fake.modes(), [unit_mode(mode::ATTACK1, e)]);
        } else {
            assert!(w.fake.modes().is_empty());
            assert_eq!(w.thinks(), [10]);
        }
        assert_eq!(steps_since(&w, lo), 2);
    }
}

// Covers: specs/monsters/ai-bodies-6.md §3 r6
#[test]
fn necro_follow_is_quiet_below_15_with_n_8_else_loud_with_n_7() {
    // Owner 9 east: D = 8. r < 15: quiet, n = 8 → R = 8, D ≤ R → k 2 →
    // quiet returns 0 → wander'. r ≥ 15: not quiet, n = 7 → R = 7 < D →
    // k 1, never the k 2 loiter (idle 15 after a draw ≥ 10).
    let (mut w, lo) = seeded(act_row(67, &[]), 1, |v| v[0] < 15);
    own(&mut w);
    w.fake.align = 1;
    w.fake.pos.insert(w.player, (109, 100));
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [wander4(lo, 1)]);
    let lo = seed_with(2, |v| v[0] >= 15 && v[1] >= 10);
    let mut w = pet();
    w.fake.pos.insert(w.player, (109, 100));
    w.seed(lo);
    w.think_with(None, 0, false);
    assert_ne!(w.thinks(), [15], "k 1 (n = 7), not the k 2 loiter (n = 8)");
    // With n = 8 and r ≥ 15 the same world would have loitered: R = 8.
    let mut w = pet();
    w.fake.pos.insert(w.player, (108, 100));
    w.seed(lo);
    w.think_with(None, 0, false);
    assert_eq!(w.thinks(), [15], "D = 7 ≤ R = 7: k 2, loud: loiter idle 15");
}

// Covers: specs/monsters/ai-bodies-6.md §3 l2 r1, §3 l2 r4
#[test]
fn necro_ranged_owner_view_drops_pet_ignore_targets_and_wanders_without_s() {
    // Ranged (param 0 = 1). No secondary target: S := T0 when T0 is in
    // line and < 20 away; with unit flag 0x40000000 (petIgnore) T0 := 0.
    let lo = seed_with(2, |v| v[0] < 15 && v[1] < 80);
    for (flag, shoots) in [(0u32, true), (0x4000_0000, false)] {
        let mut w = pet();
        w.seed(lo);
        give_skill(&mut w, 1, 50, 10);
        set_param_of(&mut w, 0, 1);
        let e = w.add_unit(UnitType::Monster, (112, 100));
        w.fake.good = Some((e, 10));
        w.fake.x.flags.insert(e, flag);
        w.think_with(None, 0, false);
        if shoots {
            assert_eq!(w.fake.modes(), [unit_mode(10, e)], "flag {flag:#x}");
        } else {
            // No S: wander' 4 after the follow's draw.
            assert_eq!(w.fake.modes(), [wander4(lo, 1)]);
        }
    }
    // S set but the room is a town: wander' 4 (the same draws).
    let mut w = pet();
    w.seed(lo);
    give_skill(&mut w, 1, 50, 10);
    set_param_of(&mut w, 0, 1);
    w.fake.town.insert(w.room);
    let e = w.add_unit(UnitType::Monster, (104, 100));
    w.fake.secondary = Some((e, 4));
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [wander4(lo, 1)]);
    let _ = (
        param_of(&w, 0),
        logged(&w, ""),
        grow as fn(&mut World, usize),
    );
}

// ---- §5 Towner, §7 Hireable -------------------------------------------------

use super::act3::seed_raw;
use super::act6::own as own_player;

/// A hireling of class `cls` (monstats `aip1` = `aip1`) owned by the
/// player, no target, owner at (103, 100).
fn hireling(cls: i32, aip1: i16) -> World {
    let mut w = world(act_row(61, &[aip1]));
    grow(&mut w, 600);
    let mon = w.mon;
    w.fake.class.insert(mon, cls);
    own_player(&mut w);
    w.fake.pos.insert(w.player, (103, 100));
    w
}

fn wander_at(lo: u32, skip: usize, center: (i32, i32), n: i32) -> String {
    let mut s = Seed::init_low(lo);
    for _ in 0..skip {
        s.step();
    }
    let (x, y) = wander_point(&mut s, center, n);
    point_mode(mode::WALK, x, y)
}

// Covers: specs/monsters/ai-bodies-6.md §7 r7, §7 r9
#[test]
fn hireling_collision_wander_threshold_by_class_and_the_late_wander() {
    // B (the unit's cell collides with mask 0x40): `lo' & 127` < 12 for the
    // ranged hirelings (q = 1), < 6 for 338 / 560 / 561 (q = 0) → wander 5.
    // A draw of 8 wanders the first, not the second.
    let lo = seed_raw(1, |v| v[0] & 127 == 8);
    for (cls, aip1, wanders) in [(271, 0, true), (338, 1, false), (560, 1, false)] {
        let mut w = hireling(cls, aip1);
        w.fake.x.pattern_collides = true;
        w.seed(lo);
        // A secondary target in range, so a non-wander goes on to the
        // hireling attack (idle 10: no pet node).
        let s = w.add_unit(UnitType::Monster, (110, 100));
        w.fake.secondary = Some((s, 9));
        w.think_with(None, 0, false);
        if wanders {
            assert_eq!(
                w.fake.modes(),
                [wander_at(lo, 1, (100, 100), 5)],
                "cls {cls}"
            );
        } else {
            assert!(w.fake.modes().is_empty(), "cls {cls}");
            assert_eq!(w.thinks(), [10], "cls {cls}");
        }
    }
    // A draw of 3 wanders every class.
    let lo3 = seed_raw(1, |v| v[0] & 127 == 3);
    let mut w = hireling(338, 1);
    w.fake.x.pattern_collides = true;
    w.seed(lo3);
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [wander_at(lo3, 1, (100, 100), 5)]);
    // Step 9: B with no target and a draw above the threshold still wanders
    // 5 (after the secondary search found nothing); without B it does not.
    let mut w = hireling(338, 1);
    w.fake.x.pattern_collides = true;
    w.seed(lo);
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [wander_at(lo, 1, (100, 100), 5)]);
    let mut w = hireling(338, 1);
    w.seed(lo);
    w.think_with(None, 0, false);
    assert!(w.fake.modes().is_empty(), "no B: no wander (idle 5)");
    assert_eq!(w.thinks(), [5]);
}

// Covers: specs/monsters/ai-bodies-6.md §7 r10
#[test]
fn hireling_in_another_coordinate_area_catches_up_with_move_0() {
    let mut w = hireling(338, 1);
    w.seed(1);
    w.fake.y.coord.insert((103, 100), 7);
    // Q must lie in O's coordinate area to be tried.
    w.fake.y.coord.insert((130, 108), 7);
    w.fake.y.final_point.insert(w.player, (130, 100));
    w.think_with(None, 0, false);
    // Hireling move k 0 (1, 60, 0): each try is velocity (0, 60, 40) and
    // a walk to Q = F + 8 × direction (the run flag is ignored).
    assert_eq!(w.fake.modes(), [point_mode(mode::WALK, 130, 108)]);
    assert_eq!((w.vel_request().speed, w.vel_request().steps), (60, 40));
}

// Covers: specs/monsters/ai-bodies-6.md §7 r11
#[test]
fn hireling_next_to_its_owner_steps_aside() {
    // D2 ≤ 1: h := (5 or p >> 1) − 1 as a byte; velocity (7, 0, 0);
    // wander near O by h.
    for (p, h) in [(0, 4), (18, 8)] {
        let mut w = hireling(338, 1);
        w.seed(1);
        set_param_of(&mut w, 0, p);
        w.fake.pos.insert(w.player, (101, 100));
        w.think_with(None, 0, false);
        assert_eq!(w.fake.modes(), [wander_at(1, 0, (101, 100), h)], "p {p}");
        assert_eq!(w.vel_request().method, 7);
    }
    // Wander fails: delete thinks, escape from O by h fails, walk to F
    // with velocity (0, 0, 40).
    let mut w = hireling(338, 1);
    w.seed(1);
    w.fake.walk_fails = true;
    w.fake.pos.insert(w.player, (101, 100));
    w.fake.y.final_point.insert(w.player, (120, 90));
    w.think_with(None, 0, false);
    assert_eq!(
        w.fake.modes().last().unwrap(),
        &point_mode(mode::WALK, 120, 90)
    );
    assert_eq!(w.vel_request().steps, 40);
    assert!(w.thinks().is_empty(), "thinks deleted");
}

// Covers: specs/monsters/ai-bodies-6.md §7 l2 r1
#[test]
fn hireling_attack_chance_by_class_and_frustration() {
    // ok = draw < a; a = 98 for 338 / 560 / 561, else p + 40 + 2 × level,
    // at most 95. ok → p := 0; a miss → p += 10.
    // (cls, aip1, p, level, a)
    for (cls, aip1, p, level, a) in [
        (338, 1, 0, 20, 98),
        (561, 1, 30, 1, 98),
        (271, 0, 0, 20, 80),
        (271, 0, 10, 20, 90),
        (359, 0, 40, 20, 95),
    ] {
        for (draw, ok) in [(a - 1, true), (a, false)] {
            let mut w = hireling(cls, aip1);
            w.seed(seed_with(1, |v| v[0] == draw as u32));
            set_param_of(&mut w, 0, p);
            w.fake.stats.insert((w.mon, 12), level);
            // S out of reach of the ranged wander (d ≥ 4), in melee range.
            let s = w.add_unit(UnitType::Monster, (110, 100));
            w.fake.secondary = Some((s, 9));
            w.fake.melee.insert(s);
            w.fake.y.hire_id = Some(1);
            w.fake.y.hire_row = Some(HireRow::default());
            w.think_with(None, 0, false);
            let expect = if ok { 0 } else { p + 10 };
            assert_eq!(param_of(&w, 0), expect, "cls {cls} p {p} draw {draw}");
        }
    }
}

// Covers: specs/monsters/ai-bodies-6.md §7 l2 r2
#[test]
fn hireling_melee_attacks_only_within_distance_3() {
    // d from the full-size distance U→S: 2 and in range → hireling skill
    // (A1 fallback); 3 → run to S with flags 1.
    for (sx, runs) in [(103, false), (104, true)] {
        let mut w = hireling(338, 1);
        w.seed(seed_with(1, |v| v[0] < 98));
        let s = w.add_unit(UnitType::Monster, (sx, 100));
        w.fake.secondary = Some((s, 5));
        w.fake.melee.insert(s);
        w.fake.y.hire_id = Some(1);
        w.fake.y.hire_row = Some(HireRow::default());
        w.think_with(None, 0, false);
        let want = if runs { mode::RUN } else { mode::ATTACK1 };
        assert_eq!(w.fake.modes(), [unit_mode(want, s)], "S at x {sx}");
    }
}

// Covers: specs/monsters/ai-bodies-6.md §7 l2 r5
#[test]
fn hireling_ranged_attack_close_in_wanders_or_escapes() {
    let ranged = |w: &mut World| {
        give_skill(w, 1, 50, 10);
        w.fake.y.hire_id = Some(1);
        w.fake.y.hire_row = Some(HireRow::default());
    };
    // d < 4 and the second draw < 50 → wander near O 4, started → end.
    let lo = seed_raw(2, |v| v[0] % 100 < 60 && v[1] % 100 < 50);
    let mut w = hireling(271, 0);
    ranged(&mut w);
    w.seed(lo);
    w.fake.stats.insert((w.mon, 12), 20);
    let s = w.add_unit(UnitType::Monster, (102, 100));
    w.fake.secondary = Some((s, 2));
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [wander_at(lo, 2, (103, 100), 4)]);
    // The wander fails: delete thinks, escape from S by 4 (started) → end.
    let mut w = hireling(271, 0);
    ranged(&mut w);
    w.seed(lo);
    w.fake.stats.insert((w.mon, 12), 20);
    let s = w.add_unit(UnitType::Monster, (102, 100));
    w.fake.secondary = Some((s, 2));
    w.fake.fail_points.insert((0, 0));
    let first = wander_at(lo, 2, (103, 100), 4);
    // Make exactly the wander point fail.
    let pt = first
        .trim_start_matches("mode 2 Point(")
        .trim_end_matches(')')
        .split(", ")
        .map(|v| v.parse::<i32>().unwrap())
        .collect::<Vec<_>>();
    w.fake.fail_points.insert((pt[0], pt[1]));
    w.think_with(None, 0, false);
    let modes = w.fake.modes();
    assert_eq!(modes[0], first);
    assert_eq!(modes.len(), 2, "wander, then the escape from S: {modes:?}");
    assert!(w.thinks().is_empty());
    // d ≥ 4: ok → hireling skill (`Skill1` of roguehire at S); a miss →
    // idle 10 (the ok draw is the first).
    for (ok, want_skill) in [(true, true), (false, false)] {
        let lo = seed_raw(1, |v| (v[0] % 100 < 80) == ok);
        let mut w = hireling(271, 0);
        ranged(&mut w);
        w.seed(lo);
        w.fake.stats.insert((w.mon, 12), 20);
        let s = w.add_unit(UnitType::Monster, (110, 100));
        w.fake.secondary = Some((s, 9));
        w.think_with(None, 0, false);
        if want_skill {
            assert_eq!(w.fake.modes(), [unit_mode(10, s)]);
        } else {
            assert!(w.fake.modes().is_empty());
            assert_eq!(w.thinks(), [10]);
        }
    }
}

// Covers: specs/monsters/ai-bodies-6.md §5 r2, §5 r3
#[test]
fn towner_runs_the_commands_then_the_map_ai_then_idles_12() {
    let home = AiCommand {
        params: [10, 100, 100, 0, 0],
    };
    // Home first: nothing set → the home command is made and the think
    // ends idling 20 (§9.9 step 1).
    let mut w = world(act_row(41, &[]));
    w.think_with(None, 0, false);
    assert_eq!(w.thinks(), [20]);
    // Commands second: a wander command (type 5, count 1, distance 3)
    // wanders and ends the think (no idle 12).
    let (mut w, lo) = seeded(act_row(41, &[]), 1, |_| true);
    w.store.control_mut(w.mon).unwrap().commands = vec![
        home.clone(),
        AiCommand {
            params: [5, 1, 3, 0, 0],
        },
    ];
    w.think_with(None, 0, false);
    assert_eq!(w.fake.modes(), [wander_at(lo, 0, (100, 100), 3)]);
    assert!(w.thinks().is_empty());
    // Map AI third: a node walk when the draw is < 66, else (nothing
    // handled) idle 12 — Towner's own idle, not Npc's 8.
    let nodes = vec![MapNode {
        action: 1,
        x: 110,
        y: 100,
    }];
    for (draw_lt_66, handled) in [(true, true), (false, false)] {
        let (mut w, _) = seeded(act_row(41, &[]), 1, |v| (v[0] < 66) == draw_lt_66);
        let c = w.store.control_mut(w.mon).unwrap();
        c.commands = vec![home.clone()];
        c.map_ai = Some(nodes.clone());
        w.think_with(None, 0, false);
        if handled {
            assert_eq!(w.fake.modes(), [point_mode(mode::WALK, 110, 100)]);
        } else {
            assert!(w.fake.modes().is_empty());
            assert_eq!(w.thinks(), [12]);
        }
    }
    // Nothing at all: idle 12.
    let mut w = world(act_row(41, &[]));
    w.store.control_mut(w.mon).unwrap().commands = vec![home];
    w.think_with(None, 0, false);
    assert_eq!(w.thinks(), [12]);
}
