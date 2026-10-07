// Spec: specs/monsters/ai-bodies-7.md (rules the first pass left unclaimed); fakes from the parent test module
use super::act2::{act_row, logged, param_of, set_param_of, world};
use super::act6::{own, sentry_world};
use super::npc::{seed_with, steps_since, unit_mode};
use super::*;
use crate::monsters::ai::bodies7::shadow_usable;

// Covers: specs/monsters/ai-bodies-7.md §7 r3
#[test]
fn jar_jar_with_the_door_open_and_no_home_idles_120() {
    // The door seam ≠ 0 and no home command (H = 0: the unit has no AI
    // control record to hold one): idle 120, nothing else.
    let mut w = world(act_row(81, &[]));
    w.fake.y.hooks.insert("PalaceDoorOpen".into());
    let mon = w.mon;
    w.store.entry(mon).control = None;
    let p = TickParam {
        target: None,
        distance: 0,
        combat: false,
        class: 0,
        class2: 0,
    };
    w.with(|g, cx| run_function(g, cx, AI_TABLE[81].think, mon, &p));
    assert_eq!(w.thinks(), [120]);
    assert!(w.fake.modes().is_empty());
}

// Covers: specs/monsters/ai-bodies-7.md §12 r6
#[test]
fn dark_wanderer_in_another_phase_idles_40() {
    let mut w = world(act_row(91, &[]));
    w.fake.y.wanderer = Some((130, 100));
    set_param_of(&mut w, 0, 5);
    w.think_with(Some(w.player), 5, false);
    assert_eq!(w.thinks(), [40]);
    assert!(w.fake.modes().is_empty());
    assert_eq!(param_of(&w, 0), 5);
}

// Covers: specs/monsters/ai-bodies-7.md §17 r4
#[test]
fn death_sentry_corpse_radius_is_half_of_param3_plus_level_steps_of_param4() {
    // `Skill1` param3 = 10, param4 = 2 at level 5: r = (10 + 4 × 2) / 2 = 9
    // (strict): a corpse 8 from S is taken, one 9 away is not (the lightning
    // or idle follows); the search is made for (S, Skill1, level 5).
    let ds = [30, 0, 50, 16];
    for (kx, taken) in [(118, true), (119, false)] {
        let mut w = sentry_world(104, &ds);
        let mut sk = Skills::decode(&vec![0u8; Skills::SIZE]);
        sk.param3 = 10;
        sk.param4 = 2;
        w.skills = vec![sk.clone(), sk.clone(), sk];
        set_param_of(&mut w, 1, 2);
        let s = w.add_unit(UnitType::Monster, (110, 100));
        let k = w.add_unit(UnitType::Monster, (kx, 100));
        w.fake.secondary = Some((s, 10));
        w.fake.x.corpse = Some(k);
        w.seed(seed_with(1, |v| v[0] >= 50));
        w.think_with(None, 0, false);
        assert!(logged(&w, "corpse search 1 5"));
        let used = w.fake.modes() == [unit_mode(mode::SKILL2, k)];
        assert_eq!(used, taken, "corpse at {kx}");
    }
}

/// A shadow warrior world whose owner is the player; K1 = `k1`, K2 = `k2`
/// (aip8 columns); `Skill` row 1 has charclass 0 (the owner's class) and the
/// given mana fields.
fn shadow(k1: i16, k2: i16, mana: (u16, u16, u16)) -> World {
    let mut row = act_row(105, &[40, 30, 60, 1]);
    row.aip8_n = k1 as u16;
    row.aip8_h = k2 as u16;
    let mut w = world(row);
    own(&mut w);
    let mut sk = Skills::decode(&vec![0u8; Skills::SIZE]);
    sk.mana = mana.0;
    sk.lvlmana = mana.1;
    sk.manashift = mana.2;
    w.skills = vec![sk.clone(), sk.clone(), sk];
    w.fake.x.skill_level.insert(1, 1);
    w
}

fn usable(w: &mut World, skill: i32, c: bool) -> bool {
    let (u, o, t) = (w.mon, w.player, w.player);
    let p = TickParam {
        target: Some(t),
        distance: 5,
        combat: c,
        class: 0,
        class2: 0,
    };
    w.with(|g, cx| shadow_usable(g, cx, u, &p, o, skill, Some(t), c))
}

// Covers: specs/monsters/ai-bodies-7.md §18 l2 r2
#[test]
fn shadow_usable_is_refused_when_the_skill_is_not_allowed() {
    // A summon skill whose id is the unit's own class fails the pet test:
    // refused with no draw.
    let mut w = shadow(1, 64, (0, 0, 0));
    w.skills[1].summon = 999;
    w.fake.class.insert(w.mon, 999);
    w.seed(7);
    assert!(!usable(&mut w, 1, false));
    assert_eq!(steps_since(&w, 7), 0);
    // Not a summon: passes the pet test and goes on to the draws.
    let mut w = shadow(1, 64, (0, 0, 0));
    w.seed(7);
    let _ = usable(&mut w, 1, false);
    assert!(steps_since(&w, 7) >= 1);
    // Also refused: the close-range type needs C, the others need no C.
    let mut w = shadow(1, 64, (0, 0, 0));
    w.skills[1].aitype = 4;
    w.seed(7);
    assert!(!usable(&mut w, 1, false));
    assert_eq!(steps_since(&w, 7), 0);
}

// Covers: specs/monsters/ai-bodies-7.md §18 l2 r4
#[test]
fn shadow_usable_mana_cost_gates_the_skill() {
    // c = ((lvlmana × (lvl − 1) + mana) << manashift) >> 8, at least 0;
    // `roll(100)` > 100 − 160 c / 100 → refused. mana 10, shift 8: c = 10,
    // threshold 84.
    let mut got = Vec::new();
    for lo in [seed_with(1, |v| v[0] == 84), seed_with(1, |v| v[0] == 85)] {
        let mut w = shadow(1, 64, (10, 0, 8));
        w.seed(lo);
        // m below lo → m := 1: a = roll(1) = 0, b ≥ 0: passes if reached.
        got.push(usable(&mut w, 1, false));
    }
    assert_eq!(got, [true, false]);
    // A negative total clamps to 0: no refusal even for a draw of 99.
    let mut w = shadow(1, 64, (0, 0, 0));
    w.skills[1].lvlmana = 0xFFFF; // −1 per level above 1
    w.fake.x.skill_level.insert(1, 5);
    w.seed(seed_with(1, |v| v[0] == 99));
    assert!(usable(&mut w, 1, false));
    // lvlmana × (lvl − 1) counts: lvlmana 5, level 3, mana 0, shift 8 → 10.
    let mut w = shadow(1, 64, (0, 5, 8));
    w.fake.x.skill_level.insert(1, 3);
    w.seed(seed_with(1, |v| v[0] == 85));
    assert!(!usable(&mut w, 1, false));
}

// Covers: specs/monsters/ai-bodies-7.md §18 l2 r5
#[test]
fn shadow_usable_waits_for_the_next_skill_frame() {
    let mut w = shadow(1, 64, (0, 0, 0));
    set_param_of(&mut w, 0, 100);
    w.game.frame = 99;
    w.seed(1);
    assert!(!usable(&mut w, 1, false));
    // The mana draw was made, the pool draws were not.
    assert_eq!(steps_since(&w, 1), 1);
    w.game.frame = 100;
    w.seed(1);
    assert!(usable(&mut w, 1, false));
}

// Covers: specs/monsters/ai-bodies-7.md §18 l2 r6
#[test]
fn shadow_usable_mana_pool_bounds_from_k1_and_k2() {
    // lo := K1 clamped 1..128, hi := K2 clamped 1..256; m < lo or m > 32 hi
    // → m := lo (written back).
    for (k1, k2, m, want) in [
        (5, 64, 3, 5),         // below lo
        (5, 64, 100, 100),     // inside
        (5, 2, 65, 5),         // above 32 × 2
        (5, 2, 64, 64),        // exactly 32 hi: kept
        (500, 64, 3, 128),     // K1 clamped to 128
        (0, 64, 0, 1),         // K1 clamped to 1
        (5, 9999, 8191, 8191), // K2 clamped to 256: 32 × 256 = 8192
        (5, 9999, 8193, 5),
    ] {
        let mut w = shadow(k1, k2, (0, 0, 0));
        set_param_of(&mut w, 1, m);
        w.seed(seed_with(1, |_| true));
        let _ = usable(&mut w, 1, false);
        // After the call m is `want`, plus the step-8 increase (0: c = 0).
        assert_eq!(param_of(&w, 1), want, "K1 {k1} K2 {k2} m {m}");
    }
}

// Covers: specs/monsters/ai-bodies-7.md §18 l2 r7
#[test]
fn shadow_usable_second_roll_must_not_be_below_the_first() {
    // a := roll(m), b := roll(100); b < a → refused. Replay the draws.
    for lo in [1u32, 12345, 3_735_928_559, 4_014_346_870] {
        let m = 50;
        let mut s = Seed::init_low(lo);
        s.roll(100); // the mana-cost roll (cost 0: never refuses)
        let a = s.roll(m);
        let b = s.roll(100);
        let mut w = shadow(1, 64, (0, 0, 0));
        set_param_of(&mut w, 1, m);
        w.seed(lo);
        assert_eq!(usable(&mut w, 1, false), b >= a, "seed {lo}: a {a} b {b}");
    }
    // Both a and b are drawn on every pass: three draws in all.
    let mut w = shadow(1, 64, (0, 0, 0));
    set_param_of(&mut w, 1, 50);
    w.seed(1);
    let _ = usable(&mut w, 1, false);
    assert!(steps_since(&w, 1) >= 3);
}

// Covers: specs/monsters/ai-bodies-7.md §18 l2 r8
#[test]
fn shadow_usable_raises_the_pool_by_the_mana_cost_over_lambda() {
    // m += (320 − λ) × c / (λ + 100), signed: c = 10, λ = 1: 319 × 10 / 101
    // = 31; λ = 220: 100 × 10 / 320 = 3.
    for (lambda, add) in [(1, 31), (220, 3), (400, -80 * 10 / 500)] {
        let mut w = shadow(1, 64, (10, 0, 8));
        set_param_of(&mut w, 1, 40);
        set_param_of(&mut w, 2, lambda);
        // The cost roll must pass (≤ 84) and b ≥ a: a = roll(40) = 0 when
        // the draw is a multiple; pick the first seed that passes.
        let lo = (1u32..)
            .find(|&lo| {
                let mut s = Seed::init_low(lo);
                let r = s.roll(100);
                let a = s.roll(40);
                let b = s.roll(100);
                r <= 84 && b >= a
            })
            .unwrap();
        w.seed(lo);
        assert!(usable(&mut w, 1, false));
        assert_eq!(param_of(&w, 1), 40 + add, "λ {lambda}");
    }
}

// Covers: specs/monsters/ai-bodies-7.md §18 r3
#[test]
fn shadow_warrior_drops_a_far_target() {
    // T := 0 when D > aip1 [40] or the distance to the owner > aip2 [30]:
    // then the pet follow runs (loiter, idle 15) and ends the think before
    // any skill. With T kept (D = 40, dO = 30) the skills run: the attack.
    let setup = |owner_at: (i32, i32)| {
        let mut w = shadow(5, 64, (0, 0, 0));
        w.fake.x.hand.insert((w.player, true), (1, 9));
        w.fake.x.hand.insert((w.player, false), (1, 9));
        w.skills[1].charclass = 0;
        w.fake.x.skill_level.insert(0, 1);
        w.fake.pos.insert(w.player, owner_at);
        w.fake.y.final_point.insert(w.player, owner_at);
        w.fake.y.target_point.insert(w.player, owner_at);
        w.fake.y.last_placed = (300, 300);
        let t = w.add_unit(UnitType::Monster, (120, 100));
        (w, t)
    };
    let attacks = |w: &World, t: UnitId| {
        w.fake
            .modes()
            .iter()
            .any(|m| m == &unit_mode(mode::ATTACK1, t))
    };
    let lo = seed_with(1, |v| v[0] >= 10);
    let run = |owner_at: (i32, i32), d: i32| {
        let (mut w, t) = setup(owner_at);
        w.seed(lo);
        w.think_with(Some(t), d, true);
        let a = attacks(&w, t);
        (a, w.thinks())
    };
    let (kept, _) = run((130, 100), 40);
    assert!(kept, "D = 40 and dO = 30 keep T");
    let (far_d, thinks) = run((130, 100), 41);
    assert!(!far_d, "D > aip1 drops T");
    let _ = thinks;
    let (far_o, thinks) = run((131, 100), 40);
    assert!(!far_o, "dO > aip2 drops T");
    let _ = thinks;
}
