// Spec: specs/skills/bodies.md §2
//! Coverage tests of the shared helpers (§2) on [`BodyFake`].

use super::fake::BodyFake;
use super::tests2::{body_rec, monster, tabs, world, Code};
use super::*;
use crate::combat::{CombatEntry, DamageRecord};
use crate::skills::fake::{combat_tables, monster_rec, FItem, FUnit};
use crate::skills::{SkillEntry, SkillUnits};
use crate::units::UnitType;

// Covers: specs/skills/bodies.md §2.1 text, §2.1 r2
#[test]
fn target_is_the_path_target_unless_it_is_the_unit() {
    let (mut f, u) = world();
    let m = monster(&mut f, (5, 5));
    assert_eq!(target(&f, u), None);
    f.targets.insert(u, m);
    assert_eq!(target(&f, u), Some(m));
    f.targets.insert(u, u);
    assert_eq!(target(&f, u), None);
}

// Covers: specs/skills/bodies.md §2.2
#[test]
fn pair_record_is_the_first_entry_of_the_pair() {
    let (mut f, u) = world();
    let a = monster(&mut f, (1, 1));
    let b = monster(&mut f, (2, 2));
    let ident = |f: &BodyFake, x: usize| (f.c.units[x].kind, f.c.units[x].guid);
    let mk = |att, def, hit: i32| CombatEntry {
        attacker: att,
        defender: def,
        record: DamageRecord {
            enh_pct: hit,
            ..DamageRecord::default()
        },
    };
    let (iu, ia, ib) = (ident(&f, u), ident(&f, a), ident(&f, b));
    f.c.units[u].combat = vec![mk(iu, ia, 1), mk(iu, ib, 2), mk(iu, ib, 3)];
    assert_eq!(helpers::pair_record(&mut f, u, a), Some(0));
    assert_eq!(helpers::pair_record(&mut f, u, b), Some(1));
    assert_eq!(helpers::pair_damage(&mut f, u, b).unwrap().enh_pct, 2);
    // No entry for the reverse pair.
    assert_eq!(helpers::pair_record(&mut f, b, u), None);
}

// Covers: specs/skills/bodies.md §2.3
#[test]
fn bow_class_and_missile_choice() {
    let (mut f, u) = world();
    for (c, bow) in [(0, false), (1, true), (2, false), (7, true), (8, false)] {
        f.composit_class = c;
        assert_eq!(is_bow(&f, u), bow, "class {c}");
    }
    // Player: hand class 1 arrows, 7 bolts, else −1.
    f.hand_class = 1;
    assert_eq!(bow_missile(&f, u), (0, None));
    f.hand_class = 7;
    assert_eq!(bow_missile(&f, u), (31, None));
    f.hand_class = 3;
    assert_eq!(bow_missile(&f, u), (-1, None));
    // Magic arrow beats explosive arrow and replaces the level.
    f.hand_class = 7;
    f.c.set(u, 158, 4);
    assert_eq!(bow_missile(&f, u), (41, Some(4)));
    f.c.set(u, 157, 6);
    assert_eq!(bow_missile(&f, u), (27, Some(6)));
    // Another unit type shoots arrows whatever its hand class.
    let m = monster(&mut f, (1, 1));
    assert_eq!(bow_missile(&f, m), (0, None));
    f.c.set(m, 158, 2);
    assert_eq!(bow_missile(&f, m), (41, Some(2)));
}

fn player_with_stack(f: &mut BodyFake, u: usize) -> usize {
    let i = f.c.add_item(FItem {
        throw: true,
        ..FItem::default()
    });
    f.c.units[u].weapon = Some(i);
    f.c.units[u].items.insert(4, i);
    i
}

// Covers: specs/skills/bodies.md §2.3
#[test]
fn ammunition_check_and_quantity_rule() {
    let (mut f, u) = world();
    let m = monster(&mut f, (1, 1));
    assert!(has_ammo(&mut f, m), "non-players always have ammo");
    // No weapon → 0.
    assert!(!has_ammo(&mut f, u));
    let i = player_with_stack(&mut f, u);
    // `any`: the stack counts. Quantity 0 and a stack maximum > 0 → 0.
    f.max_stack.insert(i, 10);
    assert!(!has_ammo(&mut f, u));
    f.item_stats.insert((i, 70), 5);
    assert!(has_ammo(&mut f, u));
    // No used skill → 0.
    let used = f.c.units[u].used.take();
    assert!(!has_ammo(&mut f, u));
    f.c.units[u].used = used;
    // has_qty: quantity 0 but item_throwable ≠ 0 → quantity := 0, 1.
    f.item_stats.insert((i, 70), 0);
    f.item_stats.insert((i, 125), 1);
    assert!(has_qty(&mut f, i));
    assert!(f.take_log().contains(&format!("itemstat {i} 70 0")));
    f.item_stats.insert((i, 125), 0);
    assert!(!has_qty(&mut f, i));
    // Maximum stack ≤ 0 → 1.
    f.max_stack.insert(i, 0);
    assert!(has_qty(&mut f, i));
    // A shooting weapon: only W itself counts (any = 0); a stack in the
    // other hand is not accepted.
    f.max_stack.insert(i, 10);
    f.shoots.insert(i);
    let q = f.c.add_item(FItem {
        throw: true,
        ..FItem::default()
    });
    f.c.units[u].items.insert(5, q);
    f.item_stats.insert((q, 70), 9);
    assert!(!has_ammo(&mut f, u));
    // Used skill 0 (Attack) and W has item_magicarrow → 1.
    f.c.units[u].used = Some(SkillEntry {
        skill: 0,
        ..f.c.units[u].used.unwrap()
    });
    f.item_stats.insert((i, 157), 1);
    assert!(has_ammo(&mut f, u));
    // Without magic arrows, W with quantity → 1.
    f.item_stats.insert((i, 157), 0);
    f.item_stats.insert((i, 70), 2);
    assert!(has_ammo(&mut f, u));
}

// Covers: specs/skills/bodies.md §2.4
#[test]
fn skill_missile_record_by_kind() {
    let (mut f, u) = world();
    f.pos.insert(u, (100, 200));
    assert!(helpers::skill_missile(
        &mut f,
        3,
        u,
        1,
        4,
        (2, -3),
        (50, 60),
        false,
        false
    ));
    let r = f.missiles[0];
    assert_eq!((r.flags, r.class, r.skill, r.level), (0x21, 3, 1, 4));
    assert_eq!((r.x, r.y, r.target_x, r.target_y), (102, 197, 50, 60));
    assert_eq!(r.origin, None);
    assert!(helpers::skill_missile(
        &mut f,
        3,
        u,
        1,
        4,
        (2, -3),
        (50, 60),
        false,
        true
    ));
    let r = f.missiles[1];
    assert_eq!((r.flags, r.origin), (0x420, Some(u)));
    // A monster with tohit: straight gets flag 0x1000 and the bonus; the
    // lob does not.
    let m = monster(&mut f, (10, 10));
    f.c.set(m, 19, 33);
    assert!(helpers::skill_missile(
        &mut f,
        3,
        m,
        1,
        4,
        (0, 0),
        (5, 5),
        false,
        false
    ));
    let r = f.missiles[2];
    assert_eq!((r.flags, r.attack_bonus), (0x21 | 0x1000, 33));
    assert!(helpers::skill_missile(
        &mut f,
        3,
        m,
        1,
        4,
        (0, 0),
        (5, 5),
        false,
        true
    ));
    assert_eq!(
        (f.missiles[3].flags, f.missiles[3].attack_bonus),
        (0x420, 0)
    );
    // tx or ty = 0 → the target position; none → no missile.
    assert!(!helpers::skill_missile(
        &mut f,
        3,
        u,
        1,
        4,
        (0, 0),
        (0, 7),
        false,
        false
    ));
    f.tpos.insert(u, (8, 9));
    assert!(helpers::skill_missile(
        &mut f,
        3,
        u,
        1,
        4,
        (0, 0),
        (0, 7),
        false,
        false
    ));
    let r = f.missiles[4];
    assert_eq!((r.target_x, r.target_y), (8, 9));
    // A target position with a 0 coordinate fails.
    f.tpos.insert(u, (8, 0));
    assert!(!helpers::skill_missile(
        &mut f,
        3,
        u,
        1,
        4,
        (0, 0),
        (0, 0),
        false,
        false
    ));
    // quant: a player pays one; no quantity → no missile.
    let n = f.missiles.len();
    assert!(!helpers::skill_missile(
        &mut f,
        3,
        u,
        1,
        4,
        (0, 0),
        (5, 5),
        true,
        false
    ));
    assert_eq!(f.missiles.len(), n);
    let i = player_with_stack(&mut f, u);
    f.item_stats.insert((i, 70), 3);
    assert!(helpers::skill_missile(
        &mut f,
        3,
        u,
        1,
        4,
        (0, 0),
        (5, 5),
        true,
        false
    ));
    assert_eq!(f.item_stats[&(i, 70)], 2);
    // The missile itself failing → none.
    f.no_missiles = true;
    assert!(!helpers::skill_missile(
        &mut f,
        3,
        u,
        1,
        4,
        (0, 0),
        (5, 5),
        false,
        false
    ));
}

// Covers: specs/skills/bodies.md §2.5 r4
#[test]
fn dec_quantity_returns_zero_without_a_stack() {
    let (mut f, u) = world();
    let m = monster(&mut f, (1, 1));
    assert_eq!(helpers::dec_quantity(&mut f, m), 0, "non-player");
    // No item in either hand.
    assert_eq!(helpers::dec_quantity(&mut f, u), 0);
    // A weapon that is not a stack and no other item.
    let w = f.c.add_item(FItem::default());
    f.c.units[u].weapon = Some(w);
    f.c.units[u].items.insert(4, w);
    assert_eq!(helpers::dec_quantity(&mut f, u), 0);
    // A stack that is not the weapon: weapon_only → skipped.
    let q = f.c.add_item(FItem {
        throw: true,
        ..FItem::default()
    });
    f.c.units[u].items.insert(5, q);
    assert_eq!(helpers::dec_quantity(&mut f, u), 0);
    // A bow (item type 27) as the weapon lifts weapon_only: the quiver
    // in the other hand is used.
    f.c.items[w].types.push(27);
    f.item_stats.insert((q, 70), 4);
    assert_eq!(helpers::dec_quantity(&mut f, u), 4);
    assert_eq!(f.item_stats[&(q, 70)], 3);
}

fn calc_tables() -> (crate::skills::SkillTables, i16) {
    let mut c = Code::new();
    let mut r = body_rec();
    r.aurastat1 = 68;
    r.aurastatcalc1 = c.f(10);
    r.aurastat2 = 25;
    r.aurastatcalc2 = c.f(0);
    r.aurastat3 = 26;
    r.aurastatcalc3 = c.f(-4);
    r.aurastat4 = 5000;
    r.aurastatcalc4 = c.f(7);
    r.aurastat5 = 0xFFFF;
    r.aurastatcalc5 = c.f(7);
    r.aurastat6 = 27;
    r.aurastatcalc6 = c.f(6);
    r.passivestat1 = 30;
    r.passivecalc1 = c.f(3);
    r.passivestat2 = 31;
    r.passivecalc2 = c.f(0);
    r.passivestat5 = 32;
    r.passivecalc5 = c.f(9);
    (tabs(r, c, 1), 1)
}

// Covers: specs/skills/bodies.md §2.6
#[test]
fn aura_and_passive_fills_set_nonzero_valid_stats() {
    let (t, sk) = calc_tables();
    let (mut f, u) = world();
    let l = f.alloc_list(0, 0, None).unwrap();
    helpers::aura_fill(&mut f, &t, u, l, i32::from(sk), 1);
    assert_eq!(f.list_get(l, 68), 10);
    assert_eq!(f.list_get(l, 69), 10, "attackrate also sets 69");
    assert_eq!(f.list_get(l, 26), -4);
    assert_eq!(f.list_get(l, 27), 6);
    // Zero values, invalid and none stats are not written.
    assert!(!f.lists[l].stats.contains_key(&25));
    assert!(!f.lists[l].stats.contains_key(&5000));
    assert_eq!(f.lists[l].stats.len(), 4);
    assert!(f.take_log().contains(&format!("anim {u}")));
    let p = f.alloc_list(0, 0, None).unwrap();
    helpers::passive_fill(&mut f, &t, u, p, i32::from(sk), 1);
    assert_eq!(f.list_get(p, 30), 3);
    assert_eq!(f.list_get(p, 32), 9);
    assert_eq!(f.lists[p].stats.len(), 2);
    // An invalid skill does nothing (not even the refresh).
    let z = f.alloc_list(0, 0, None).unwrap();
    f.take_log();
    helpers::aura_fill(&mut f, &t, u, z, 99, 1);
    helpers::passive_fill(&mut f, &t, u, z, -1, 1);
    assert!(f.lists[z].stats.is_empty());
    assert!(f.take_log().is_empty());
}

fn ct2() -> crate::combat::CombatTables {
    let mut ct = combat_tables(vec![monster_rec(), monster_rec(), monster_rec()]);
    ct.monstats2[0].isatt = true;
    ct.monstats[1].npc = true;
    ct.monstats[2].monstatsex = 2;
    ct
}

// Covers: specs/skills/bodies.md §2.7 text, §2.7 r1, §2.7 r2
#[test]
fn apply_state_refuses_bad_states_and_non_attackable_monsters() {
    let ct = ct2();
    let (mut f, u) = world();
    f.states = 10;
    let mk = |target, state| StateRequest {
        source: u,
        target,
        skill: 5,
        level: 2,
        duration: 30,
        stat: 7,
        value: 11,
        state,
        callback: 0,
    };
    // r1: a state outside 0 ≤ s < states count.
    assert!(apply_state(&mut f, &ct, mk(u, -1)).is_none());
    assert!(apply_state(&mut f, &ct, mk(u, 10)).is_none());
    // r2: monsters: npc, or no isAtt → none (class 1 is npc; class 2
    // reads the blank monstats2 row 2 → isatt clear; class 3 has no
    // record).
    let ok = f.add(FUnit::new(UnitType::Monster, 0), (1, 1));
    let npc = f.add(FUnit::new(UnitType::Monster, 1), (1, 1));
    let noatt = f.add(FUnit::new(UnitType::Monster, 2), (1, 1));
    let none = f.add(FUnit::new(UnitType::Monster, 3), (1, 1));
    for m in [npc, noatt, none] {
        assert!(apply_state(&mut f, &ct, mk(m, 3)).is_none());
    }
    // The request fields are used as laid out.
    f.c.frame = 100;
    let l = apply_state(&mut f, &ct, mk(ok, 3)).expect("list");
    let l = &f.lists[l];
    assert_eq!((l.state, l.skill, l.lvl), (3, 5, 2));
    assert_eq!(l.stats.get(&7), Some(&11));
    assert_eq!((l.expire, l.flags & 2), (130, 2));
    assert_eq!(l.callback, callback::DEFAULT);
    assert_eq!(l.unit, Some(ok));
    let m2 = apply_state(
        &mut f,
        &ct,
        StateRequest {
            callback: 0x1234,
            state: 4,
            stat: -1,
            ..mk(ok, 4)
        },
    )
    .unwrap();
    assert_eq!(f.lists[m2].callback, 0x1234);
    assert!(f.lists[m2].stats.is_empty());
}

fn scan_world() -> (BodyFake, usize, usize, usize, usize) {
    let (mut f, u) = world();
    let a = f.add(FUnit::new(UnitType::Player, 0), (10, 0));
    let b = monster(&mut f, (0, 6));
    let c = monster(&mut f, (30, 40));
    f.scan = vec![u, a, b, c];
    (f, u, a, b, c)
}

fn run_scan(
    f: &mut BodyFake,
    u: usize,
    at: (i32, i32),
    r: i32,
    flt: u32,
    noaura: bool,
    ret: i32,
) -> (i32, Vec<usize>) {
    let t = crate::skills::fake::skill_tables(vec![]);
    let ct = ct2();
    let mut seen = Vec::new();
    let n = scan_unit(f, &t, &ct, u, at, r, flt, noaura, &mut |_, x| {
        seen.push(x);
        if x == 1 {
            0
        } else {
            ret
        }
    });
    (n, seen)
}

// Covers: specs/skills/bodies.md §2.12 text, §2.12 r1, §2.12 r3
#[test]
fn scan_unit_visits_accepted_units_in_range() {
    let (mut f, u, a, b, _c) = scan_world();
    // r1: r ≤ 0 → nothing; no room → nothing.
    assert_eq!(run_scan(&mut f, u, (0, 0), 0, 3, false, 1), (0, vec![]));
    assert_eq!(run_scan(&mut f, u, (0, 0), -5, 3, false, 1), (0, vec![]));
    f.room_of.remove(&u);
    assert_eq!(run_scan(&mut f, u, (0, 0), 100, 3, false, 1), (0, vec![]));
    f.room_of.insert(u, 1);
    // r3: the source is excluded; d² ≤ r² (10² = 100: the player at
    // (10, 0) is exactly in); (30, 40) is 50 away.
    let (n, seen) = run_scan(&mut f, u, (0, 0), 10, 3, false, 1);
    assert_eq!(seen, vec![a, b]);
    // `a` has unit id 1 and its callback returns 0: only `b` counts.
    assert_eq!(n, 1);
    // One short of the player: d² = 100 > 81.
    assert_eq!(run_scan(&mut f, u, (0, 0), 9, 3, false, 1).1, vec![b]);
    // x or y = 0 means the source's position; an explicit centre moves it.
    f.pos.insert(u, (30, 0));
    assert_eq!(run_scan(&mut f, u, (0, 0), 7, 3, false, 1).1, vec![]);
    assert_eq!(
        run_scan(&mut f, u, (30, 40), 5, 3, false, 1).1,
        vec![f.scan[3]]
    );
    f.pos.insert(u, (0, 0));
    // f = 0 is replaced by 0x583, which needs the unit flags 0x4 and 0x8.
    assert_eq!(run_scan(&mut f, u, (0, 0), 10, 0, false, 1).1, vec![]);
    for x in [a, b] {
        f.c.units[x].flags = 0xC;
    }
    assert_eq!(run_scan(&mut f, u, (0, 0), 10, 0, false, 1).1, vec![a, b]);
    // f only selects what `accepts` accepts: monsters only.
    assert_eq!(run_scan(&mut f, u, (0, 0), 10, 2, false, 1).1, vec![b]);
    // Town rooms are skipped with 0x2000 only.
    let t = f.add(FUnit::new(UnitType::Monster, 0), (1, 1));
    f.scan_town = vec![t];
    assert_eq!(
        run_scan(&mut f, u, (0, 0), 10, 3, false, 1).1,
        vec![a, b, t]
    );
    assert_eq!(
        run_scan(&mut f, u, (0, 0), 10, 3 | 0x2000, false, 1).1,
        vec![a, b]
    );
    // noaura skips a monster whose class has monstats `noaura`.
    let mut ct = ct2();
    ct.monstats[0].noaura = true;
    let tb = crate::skills::fake::skill_tables(vec![]);
    let mut seen = Vec::new();
    scan_unit(&mut f, &tb, &ct, u, (0, 0), 10, 3, true, &mut |_, x| {
        seen.push(x);
        1
    });
    assert_eq!(seen, vec![a], "noaura drops the class-0 monsters");
    seen.clear();
    scan_unit(&mut f, &tb, &ct, u, (0, 0), 10, 3, false, &mut |_, x| {
        seen.push(x);
        1
    });
    assert_eq!(seen, vec![a, b, t]);
}

// Covers: specs/skills/bodies.md §2.12 text, §2.12 r1
#[test]
fn scan_point_uses_the_target_point_and_keeps_f() {
    let (mut f, u, a, b, _c) = scan_world();
    let (t, ct) = (crate::skills::fake::skill_tables(vec![]), ct2());
    let mut seen = Vec::new();
    // No target position: returns 0 and calls nothing.
    assert_eq!(
        scan_point(&mut f, &t, &ct, 3, u, 100, &mut |_, x| {
            seen.push(x);
            1
        }),
        0
    );
    assert!(seen.is_empty());
    // Centre (0, 6): the source is not excluded; f = 0 is not replaced
    // (nothing is accepted).
    f.tpos.insert(u, (0, 6));
    assert_eq!(
        scan_point(&mut f, &t, &ct, 0, u, 7, &mut |_, x| {
            seen.push(x);
            1
        }),
        1
    );
    assert!(seen.is_empty());
    assert_eq!(
        scan_point(&mut f, &t, &ct, 3, u, 7, &mut |_, x| {
            seen.push(x);
            1
        }),
        1
    );
    assert_eq!(seen, vec![u, b], "the source at distance 6 is visited");
    seen.clear();
    f.tpos.insert(u, (10, 0));
    scan_point(&mut f, &t, &ct, 3, u, 0, &mut |_, x| {
        seen.push(x);
        1
    });
    assert_eq!(seen, vec![a], "r = 0 keeps d² = 0 only");
}

// ---------------------------------------------------------------- §3

/// A world where the player (unit 0) attacks a player in run mode
/// (always hit), hostile and in melee range, with a fixed damage roll.
fn duel() -> (BodyFake, usize, usize) {
    let (mut f, u) = world();
    let mut d = FUnit::new(UnitType::Player, 1);
    d.mode = 3;
    let m = f.add(d, (1, 0));
    f.c.hostile = true;
    f.c.in_range = true;
    f.targets.insert(u, m);
    f.c.set(u, 21, 100);
    f.c.set(u, 22, 100);
    (f, u, m)
}

fn last_entry(f: &mut BodyFake, u: usize) -> CombatEntry {
    f.c.units[u].combat[0].clone()
}

fn ct3() -> crate::combat::CombatTables {
    combat_tables(vec![monster_rec(), monster_rec()])
}

// Covers: specs/skills/bodies.md §3.1 r1, §3.1 r2, §3.1 r3, §3.1 r4
#[test]
fn attack_start_frame_meleeonly_and_ammo() {
    let t = tabs(body_rec(), Code::new(), 1);
    let ct = ct3();
    let (mut f, u, m) = duel();
    // r1: the animation frame := frame bonus << 8.
    f.frame_bonus_v = 3;
    assert_eq!(starts::attack(&mut f, &t, &ct, u, 1), 1);
    assert_eq!(f.anim_frame[&u], 768);
    // r4: a plain attack (no bow, no meleeonly state) needs nothing else.
    assert!(f.c.units[u].combat.is_empty());
    // r3: a bow without ammunition is refused and the cleanups run.
    f.composit_class = 1;
    f.take_log();
    assert_eq!(starts::attack(&mut f, &t, &ct, u, 1), 0);
    let log = f.take_log();
    let i = log.iter().position(|l| *l == format!("attackcleanup {u}"));
    assert_eq!(
        log.iter().position(|l| *l == format!("weaponcleanup {u}")),
        i.map(|i| i + 1)
    );
    assert!(i.is_some());
    // …with ammunition, 1.
    player_with_stack(&mut f, u);
    f.item_stats.insert((0, 70), 9);
    assert_eq!(starts::attack(&mut f, &t, &ct, u, 1), 1);
    assert!(f.take_log().iter().all(|l| !l.contains("cleanup")));
    // r2: a unit in a meleeonly (group 38) state runs the shape start,
    // even with a bow without ammunition.
    f.item_stats.insert((0, 70), 0);
    f.max_stack.insert(0, 10);
    f.state_flags.insert((139, group::MELEEONLY));
    f.c.units[u].states.push(139);
    f.targets.remove(&u);
    assert_eq!(starts::attack(&mut f, &t, &ct, u, 1), 0, "no target");
    f.targets.insert(u, m);
    assert_eq!(starts::attack(&mut f, &t, &ct, u, 1), 1);
    assert_eq!(f.c.units[u].combat.len(), 1);
    assert_eq!(last_entry(&mut f, u).record.hit_class, 1);
}

// Covers: specs/skills/bodies.md §3.2
#[test]
fn kick_makes_a_forced_hit_record() {
    let mut r = body_rec();
    r.mindam = 5;
    r.hitshift = 3;
    let t = tabs(r, Code::new(), 1);
    let ct = ct3();
    let (mut f, u, m) = duel();
    f.targets.remove(&u);
    assert_eq!(starts::kick(&mut f, &t, &ct, u, 1), 0);
    f.targets.insert(u, m);
    // The body hands `start_combat` the forced-hit record: compare with
    // the same call made by hand on a copy of the world.
    let mut g = f.clone();
    let mut want = DamageRecord {
        hit_flags: 2,
        result: 9,
        physical: 5 << 3,
        hit_class: 1,
        ..DamageRecord::default()
    };
    crate::combat::start_combat(&mut g.c, &t, &ct, Some(u), Some(m), &mut want, 128);
    assert_eq!(starts::kick(&mut f, &t, &ct, u, 1), 1);
    let e = last_entry(&mut f, u);
    assert_eq!(e.record, want);
    assert_eq!((e.record.hit_class, e.record.hit_flags & 2), (1, 2));
    assert_eq!(e.record.result & 9, 9, "hit + knockback");
    // An invalid skill reads no record (the original is fatal).
    assert_eq!(starts::kick(&mut f, &t, &ct, u, 99), 0);
}

// Covers: specs/skills/bodies.md §3.3
#[test]
fn unsummon_needs_the_owner_and_an_unsummonable_pet() {
    let (mut f, u) = world();
    let m = monster(&mut f, (5, 5));
    f.targets.insert(u, m);
    assert_eq!(starts::unsummon(&mut f, u), 0, "not the owner");
    f.minion_owner.insert(m, u);
    assert_eq!(starts::unsummon(&mut f, u), 0, "pet type not unsummonable");
    f.unsummon_ok = true;
    assert_eq!(starts::unsummon(&mut f, u), 1);
    assert_eq!(f.entries[&(u, 1, 1)], f.c.units[m].guid as i32);
    // No used skill entry → 0.
    f.c.units[u].used = None;
    assert_eq!(starts::unsummon(&mut f, u), 0);
    f.c.units[u].used = f.c.units[u].skills.first().copied();
    // The caster must be a player, the target a monster.
    let p = f.add(FUnit::new(UnitType::Player, 2), (9, 9));
    f.targets.insert(u, p);
    f.minion_owner.insert(p, u);
    assert_eq!(starts::unsummon(&mut f, u), 0);
    f.targets.remove(&u);
    assert_eq!(starts::unsummon(&mut f, u), 0);
    f.targets.insert(m, m);
    f.minion_owner.insert(u, m);
    f.c.units[m].skills = f.c.units[u].skills.clone();
    f.c.units[m].used = f.c.units[u].used;
    f.targets.insert(m, u);
    assert_eq!(starts::unsummon(&mut f, m), 0, "a monster caster");
}

// Covers: specs/skills/bodies.md §3.4
#[test]
fn arrow_bolt_and_throw_starts_need_ammunition() {
    let t = tabs(body_rec(), Code::new(), 1);
    let ct = ct3();
    for slot in [4u16, 65] {
        let (mut f, u) = world();
        assert_eq!(run_start(&mut f, &t, &ct, slot, u, 1, 1), Some(0));
        let log = f.take_log();
        assert!(log.contains(&format!("attackcleanup {u}")), "slot {slot}");
        assert!(log.contains(&format!("weaponcleanup {u}")), "slot {slot}");
        let i = player_with_stack(&mut f, u);
        f.item_stats.insert((i, 70), 3);
        assert_eq!(run_start(&mut f, &t, &ct, slot, u, 1, 1), Some(1));
        assert!(f.take_log().iter().all(|l| !l.contains("cleanup")));
    }
}

// Covers: specs/skills/bodies.md §3.5
#[test]
fn jab_start_needs_a_target() {
    let t = tabs(body_rec(), Code::new(), 1);
    let (mut f, u) = world();
    let m = monster(&mut f, (5, 5));
    assert_eq!(starts::jab(&mut f, &t, u, 1), 0);
    f.targets.insert(u, m);
    assert_eq!(starts::jab(&mut f, &t, u, 1), 1);
    assert_eq!(starts::jab(&mut f, &t, u, 77), 0, "R invalid");
}

fn corpse_world() -> (BodyFake, usize, usize, crate::combat::CombatTables) {
    let mut ct = combat_tables(vec![monster_rec()]);
    ct.monstats2[0].corpsesel = true;
    ct.monstats2[0].soft = true;
    let (mut f, u) = world();
    let mut c = FUnit::new(UnitType::Monster, 0);
    c.mode = 12;
    let m = f.add(c, (4, 4));
    f.targets.insert(u, m);
    (f, u, m, ct)
}

// Covers: specs/skills/bodies.md §3.6
#[test]
fn raise_start_needs_a_raisable_corpse() {
    let (mut f, u, m, mut ct) = corpse_world();
    assert_eq!(starts::raise(&mut f, &ct, u), 1);
    // In a town room → 0.
    f.town.insert(1);
    assert_eq!(starts::raise(&mut f, &ct, u), 0);
    f.town.clear();
    // Not dead (mode 12), a udead state (group 33), no corpseSel, no
    // Velocity, not a monster, no target.
    f.c.units[m].mode = 1;
    assert_eq!(starts::raise(&mut f, &ct, u), 0);
    f.c.units[m].mode = 12;
    f.state_flags.insert((77, group::UDEAD));
    f.c.units[m].states.push(77);
    assert_eq!(starts::raise(&mut f, &ct, u), 0);
    f.c.units[m].states.clear();
    ct.monstats[0].velocity = 0;
    assert_eq!(starts::raise(&mut f, &ct, u), 0);
    ct.monstats[0].velocity = 1;
    ct.monstats2[0].corpsesel = false;
    assert_eq!(starts::raise(&mut f, &ct, u), 0);
    ct.monstats2[0].corpsesel = true;
    f.targets.remove(&u);
    assert_eq!(starts::raise(&mut f, &ct, u), 0);
}

// Covers: specs/skills/bodies.md §3.9
#[test]
fn find_potion_start_needs_a_soft_unmarked_corpse() {
    let (mut f, u, m, mut ct) = corpse_world();
    assert_eq!(starts::find_potion(&mut f, &ct, u), 1);
    f.c.units[m].states.push(118);
    assert_eq!(starts::find_potion(&mut f, &ct, u), 0, "corpse_noselect");
    f.c.units[m].states.clear();
    ct.monstats2[0].soft = false;
    assert_eq!(starts::find_potion(&mut f, &ct, u), 0);
    ct.monstats2[0].soft = true;
    f.c.units[m].mode = 1;
    assert_eq!(starts::find_potion(&mut f, &ct, u), 0);
    f.c.units[m].mode = 12;
    f.targets.remove(&u);
    assert_eq!(starts::find_potion(&mut f, &ct, u), 0);
}

// Covers: specs/skills/bodies.md §3.10
#[test]
fn andrial_spray_start_stores_the_target_position() {
    let (mut f, u) = world();
    let m = monster(&mut f, (123, -45));
    assert_eq!(starts::andrial_spray(&mut f, u), 0);
    f.targets.insert(u, m);
    assert_eq!(starts::andrial_spray(&mut f, u), 1);
    assert_eq!((f.entries[&(u, 1, 1)], f.entries[&(u, 1, 2)]), (123, -45));
    f.c.units[u].used = None;
    assert_eq!(starts::andrial_spray(&mut f, u), 0);
}

/// The skills record the Sacrifice / Bash tests use: formulas 40, 3,
/// −25 and 30 in calc1–calc4, EType 3, SrcDam `src`.
fn strike_tables(src: u8, hitflags: u32, hitclass: u32) -> (crate::skills::SkillTables, Code) {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(40);
    r.calc2 = c.f(3);
    r.calc3 = c.f(-25);
    r.calc4 = c.f(30);
    r.etype = 3;
    r.srcdam = src;
    r.resultflags = 0x0400;
    r.hitflags = hitflags;
    r.hitclass = hitclass;
    let t = tabs(r, Code(c.0.clone()), 1);
    (t, c)
}

// Covers: specs/skills/bodies.md §3.7
#[test]
fn sacrifice_start_rolls_a_melee_hit_and_always_starts_combat() {
    use crate::combat::{bonuses, melee_result, start_combat};
    let ct = ct3();
    let (t, _) = strike_tables(0, 2, 0);
    // Refusals: R invalid, no used skill, no target, not in range.
    let (mut f, u, m) = duel();
    assert_eq!(starts::sacrifice(&mut f, &t, &ct, u, 99, 1), 0);
    f.c.in_range = false;
    assert_eq!(starts::sacrifice(&mut f, &t, &ct, u, 1, 1), 0);
    f.c.in_range = true;
    f.targets.remove(&u);
    assert_eq!(starts::sacrifice(&mut f, &t, &ct, u, 1, 1), 0);
    f.targets.insert(u, m);
    let used = f.c.units[u].used.take();
    assert_eq!(starts::sacrifice(&mut f, &t, &ct, u, 1, 1), 0);
    f.c.units[u].used = used;
    assert!(f.c.units[u].combat.is_empty());
    // A hit: physical from bonuses(get, pct = calc1, s = SrcDam), the
    // conversion (EType ≠ 0, calc4 > 0), roll_elemental, ResultFlags,
    // hit flags = HitFlags | 1; replayed by hand on a copy.
    let mut g = f.clone();
    let bonus = crate::skills::to_hit(&mut g, &t, Some(u), 1, 1);
    let mut want = DamageRecord {
        result: melee_result(&mut g.c, &t, &ct, Some(u), Some(m), bonus, 0),
        ..DamageRecord::default()
    };
    assert_eq!(want.result & 1, 1, "run mode: always a hit");
    want.physical = bonuses(&mut g.c, &t, u, true, None, 0, 0, 40, 0, 0);
    want.conv_pct = 30;
    want.conv_elem = 3;
    crate::skills::roll_elemental(&mut g, &t, u, &mut want, 1, 1);
    want.result |= 0x0400;
    want.hit_flags = 2 | 1;
    start_combat(&mut g.c, &t, &ct, Some(u), Some(m), &mut want, 128);
    assert_eq!(starts::sacrifice(&mut f, &t, &ct, u, 1, 1), 1);
    assert_eq!(last_entry(&mut f, u).record, want);
    assert_eq!(f.c.units[u].seed, g.c.units[u].seed, "same draws");
    // SrcDam is passed on to bonuses (and 0 stays 0).
    let (t2, _) = strike_tables(64, 2, 0);
    let mut g = f.clone();
    g.c.units[u].combat.clear();
    f.c.units[u].combat.clear();
    assert_eq!(starts::sacrifice(&mut f, &t2, &ct, u, 1, 1), 1);
    let want_phys = bonuses(&mut g.c, &t2, u, true, None, 0, 0, 40, 0, 64);
    assert_ne!(want_phys, 0);
    assert_eq!(last_entry(&mut f, u).record.physical, {
        let mut r = DamageRecord {
            physical: want_phys,
            ..DamageRecord::default()
        };
        crate::combat::totals(&mut g.c, &ct, Some(u), m, &mut r);
        r.physical
    });
    // A miss: nothing but the melee result is stored, and combat still
    // starts (an entry exists).
    f.c.hostile = false;
    f.c.units[u].combat.clear();
    assert_eq!(starts::sacrifice(&mut f, &t, &ct, u, 1, 1), 1);
    let e = last_entry(&mut f, u);
    assert_eq!(
        (e.record.result, e.record.physical, e.record.hit_flags),
        (0, 0, 0)
    );
}

// Covers: specs/skills/bodies.md §3.8
#[test]
fn bash_start_attackrate_list_record_pair_bonus_and_aura_state() {
    use crate::combat::{melee_result, start_combat};
    let ct = ct3();
    let (mut t, _) = strike_tables(0, 2, 7);
    // aurastate 40 with aurastat1 = 25 (value calc2 = 3).
    t.skills[1].aurastate = 40;
    t.skills[1].aurastat1 = 25;
    t.skills[1].aurastatcalc1 = t.skills[1].calc2;
    // Refusals: R invalid, no target, target in a town room.
    let (mut f, u, m) = duel();
    assert_eq!(starts::bash(&mut f, &t, &ct, u, 99, 1), 0);
    f.town.insert(1);
    assert_eq!(starts::bash(&mut f, &t, &ct, u, 1, 1), 0);
    f.town.clear();
    f.targets.remove(&u);
    assert_eq!(starts::bash(&mut f, &t, &ct, u, 1, 1), 0);
    f.targets.insert(u, m);
    assert!(f.lists.is_empty() && f.c.units[u].combat.is_empty());
    // The full run, replayed by hand on a copy.
    let mut g = f.clone();
    let bonus = crate::skills::to_hit(&mut g, &t, Some(u), 1, 1);
    let mut want = DamageRecord {
        result: melee_result(&mut g.c, &t, &ct, Some(u), Some(m), bonus, 0),
        ..DamageRecord::default()
    };
    assert_eq!(want.result & 1, 1);
    want.result |= 0x0400;
    want.hit_flags |= 2;
    want.hit_class = 7;
    want.enh_pct = 40;
    want.conv_pct = 30;
    want.conv_elem = 3;
    crate::skills::roll_elemental(&mut g, &t, u, &mut want, 1, 1);
    start_combat(&mut g.c, &t, &ct, Some(u), Some(m), &mut want, 128);
    want.physical += 3 << 8;
    f.take_log();
    assert_eq!(starts::bash(&mut f, &t, &ct, u, 1, 1), 1);
    assert_eq!(
        last_entry(&mut f, u).record,
        want,
        "pair bonus after the roll"
    );
    // The temporary attack-rate list: flags 4, expire 0, no state.
    let tmp = f
        .lists
        .iter()
        .find(|l| l.stats.get(&68) == Some(&-25))
        .unwrap();
    assert_eq!(
        (tmp.flags, tmp.expire, tmp.owner, tmp.unit),
        (4, 0, Some(u), Some(u))
    );
    assert!(f.take_log().contains(&format!("anim {u}")));
    // The aurastate list: state 40, flags 4, default callback, on the
    // unit, filled by aura_fill; the state is on.
    let a = f.list_of(u, 40).expect("aura list");
    assert_eq!((a.flags, a.expire, a.callback), (4, 0, callback::DEFAULT));
    assert_eq!(a.stats.get(&25), Some(&3));
    assert!(f.has_state(u, 40));
    // A second run reuses the list.
    let n = f.lists.len();
    assert_eq!(starts::bash(&mut f, &t, &ct, u, 1, 1), 1);
    assert_eq!(f.lists.iter().filter(|l| l.state == 40).count(), 1);
    assert_eq!(f.lists.len(), n + 1, "only the attack-rate list is new");
    // SrcDam 0 → 128; a miss stores a bare record (no hit work).
    f.c.hostile = false;
    f.c.units[u].combat.clear();
    assert_eq!(starts::bash(&mut f, &t, &ct, u, 1, 1), 1);
    let e = last_entry(&mut f, u);
    assert_eq!(
        (e.record.result, e.record.hit_class, e.record.enh_pct),
        (0, 0, 0)
    );
    // aurastate invalid (−1): no state list.
    let mut t3 = t.clone();
    t3.skills[1].aurastate = 0xFFFF;
    let (mut f, u, _) = duel();
    assert_eq!(starts::bash(&mut f, &t3, &ct, u, 1, 1), 1);
    assert!(f.lists.iter().all(|l| l.state != 40));
}

// ---------------------------------------------------------------- §4

fn hit_entry(f: &mut BodyFake, u: usize, m: usize, result: u16) {
    let e = CombatEntry {
        attacker: (f.c.units[u].kind, f.c.units[u].guid),
        defender: (f.c.units[m].kind, f.c.units[m].guid),
        record: DamageRecord {
            result,
            ..DamageRecord::default()
        },
    };
    f.c.units[u].combat.insert(0, e);
}

fn life_of(f: &BodyFake, u: usize) -> i32 {
    f.c.get(u, 6)
}

fn pgsv_list(f: &mut BodyFake, u: usize, state: i32, skill: i32) -> usize {
    f.state_flags.insert((state, group::PGSV));
    f.c.units[u].states.push(state as u16);
    let l = f.alloc_list(0, 0, None).unwrap();
    f.set_list_state(l, state);
    f.attach(u, l);
    f.list_set(l, 350, skill);
    l
}

// Covers: specs/skills/bodies.md §4.1 r1, §4.1 r2, §4.1 r3, §4.1 r4
#[test]
fn attack_do_flag_potion_bow_and_target() {
    let t = tabs(body_rec(), Code::new(), 1);
    let ct = ct3();
    let (mut f, u, m) = duel();
    // r4: no target → 0; r1: unit flags |= 0x40 first.
    f.targets.remove(&u);
    assert_eq!(dos::attack(&mut f, &t, &ct, u, 1, 1), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    // r2: a missile potion weapon (item type 38) → 0 before any attack.
    f.targets.insert(u, m);
    f.c.units[u].flags = 0;
    let w = f.c.add_item(FItem {
        types: vec![38],
        ..FItem::default()
    });
    f.c.units[u].weapon = Some(w);
    assert_eq!(dos::attack(&mut f, &t, &ct, u, 1, 1), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    assert!(f.c.units[u].combat.is_empty());
    f.c.units[u].weapon = None;
    // r3: a bow shoots: arrows (class 0) cost one quantity (quant = 1);
    // the call returns 1 whatever the missile creation gave.
    f.composit_class = 1;
    f.tpos.insert(u, (40, 50));
    assert_eq!(dos::attack(&mut f, &t, &ct, u, 3, 7), 1);
    assert!(f.missiles.is_empty(), "no quantity: no missile");
    let i = player_with_stack(&mut f, u);
    f.item_stats.insert((i, 70), 2);
    assert_eq!(dos::attack(&mut f, &t, &ct, u, 3, 7), 1);
    let r = f.missiles[0];
    assert_eq!((r.class, r.skill, r.level, r.flags), (0, 3, 7, 0x21));
    assert_eq!((r.target_x, r.target_y), (40, 50));
    assert_eq!(f.item_stats[&(i, 70)], 1);
    // Magic arrows (class 27) replace the level and cost nothing.
    f.c.set(u, 157, 5);
    assert_eq!(dos::attack(&mut f, &t, &ct, u, 3, 7), 1);
    let r = f.missiles[1];
    assert_eq!((r.class, r.level), (27, 5));
    assert_eq!(f.item_stats[&(i, 70)], 1);
    // Bolts: hand class 7 → class 31.
    f.c.set(u, 157, 0);
    f.hand_class = 7;
    assert_eq!(dos::attack(&mut f, &t, &ct, u, 3, 7), 1);
    assert_eq!(f.missiles[2].class, 31);
    // A bow never melees.
    assert!(f.c.units[u].combat.is_empty());
}

// Covers: specs/skills/bodies.md §4.1 text, §4.1 r5, §4.1 r6, §4.1 r7, §4.1 r8
#[test]
fn attack_do_melee_pipeline_and_wolf_bear_path() {
    use crate::combat::{apply_melee, fill, melee_result, start_combat};
    let t = tabs(body_rec(), Code::new(), 1);
    let ct = ct3();
    let (mut f, u, m) = duel();
    f.c.set(u, 325, 17);
    f.c.set(m, 6, 100_000);
    // A progressive-charge list: the finisher detaches and frees it
    // (record of k missing → step 6) and clears the group.
    let p = pgsv_list(&mut f, u, 122, 99);
    // Replay of steps 5–8 by hand on a copy.
    let mut g = f.clone();
    let mut want = DamageRecord {
        result: melee_result(&mut g.c, &t, &ct, Some(u), Some(m), 17, 0),
        ..DamageRecord::default()
    };
    want.hit_flags |= 2;
    helpers::charges_before(&mut g, &t, u, &mut want);
    fill(&mut g.c, &t, &ct, u, m, &mut want, false, 128);
    helpers::charges_after(&mut g, &t, u, &mut want);
    start_combat(&mut g.c, &t, &ct, Some(u), Some(m), &mut want, 128);
    let stored = g.c.units[u].combat[0].record;
    assert_eq!(stored, want);
    apply_melee(&mut g.c, &ct, u, m);
    assert!(life_of(&g, m) < 100_000, "the hit lands");
    f.take_log();
    assert_eq!(dos::attack(&mut f, &t, &ct, u, 1, 1), 1);
    assert_eq!(life_of(&f, m), life_of(&g, m), "r5–r6, r8: same damage");
    assert_eq!(f.c.units[u].seed, g.c.units[u].seed);
    assert!(
        f.c.units[u].combat.is_empty(),
        "apply_melee frees the record"
    );
    // r8: the finisher ran: flag 0x40, list freed, group cleared.
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    assert!(f.lists[p].freed);
    assert!(f.take_log().contains(&format!("cleargroup {u} 4")));
    // r7: wolf / bear (group 38): the melee is applied at once, with no
    // finisher.
    let (mut f, u, m) = duel();
    f.c.set(m, 6, 100_000);
    let p = pgsv_list(&mut f, u, 122, 99);
    f.state_flags.insert((139, group::MELEEONLY));
    f.c.units[u].states.push(139);
    f.take_log();
    assert_eq!(dos::attack(&mut f, &t, &ct, u, 1, 1), 1);
    assert!(life_of(&f, m) < 100_000, "applied at once");
    assert!(!f.lists[p].freed);
    assert!(!f.take_log().iter().any(|l| l.starts_with("cleargroup")));
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
}

/// Skills record for the melee-with-state do (§4.2): overlay 9, target
/// state 30 (length calc 50, aurastat1 25 = 4), self state 31.
fn melee_state_tables() -> crate::skills::SkillTables {
    let mut c = Code::new();
    let mut r = body_rec();
    r.srvoverlay = 9;
    r.auratargetstate = 30;
    r.aurastate = 31;
    r.auralencalc = c.f(50);
    r.aurastat1 = 25;
    r.aurastatcalc1 = c.f(4);
    tabs(r, c, 1)
}

// Covers: specs/skills/bodies.md §4.2 text, §4.2 r1, §4.2 r2
#[test]
fn melee_state_overlay_and_dispatch() {
    let t = melee_state_tables();
    let ct = ct3();
    let (mut f, u, m) = duel();
    f.c.set(m, 6, 100_000);
    // Slot 2 is this body (`functions.tsv`).
    assert_eq!(
        run_do(&mut f, &t, &ct, 2, u, 99, 1),
        Some(0),
        "r1: R invalid"
    );
    // r2: no target → 0, flag not set.
    f.targets.remove(&u);
    assert_eq!(dos::melee_state(&mut f, &t, &ct, u, 1, 1), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0);
    // No pair record → return 1 and the rest is skipped (no flag 0x40,
    // no state on the unit).
    f.targets.insert(u, m);
    assert_eq!(run_do(&mut f, &t, &ct, 2, u, 1, 1), Some(1));
    assert_eq!(f.c.units[u].flags & 0x40, 0);
    assert!(!f.has_state(u, 31));
    // A pair record that is a miss: no overlay, but the rest goes on.
    hit_entry(&mut f, u, m, 0);
    f.take_log();
    assert_eq!(dos::melee_state(&mut f, &t, &ct, u, 1, 1), 1);
    assert!(!f.take_log().iter().any(|l| l.starts_with("overlay")));
    assert!(f.has_state(u, 31));
    // A hit: overlay 9 on the target.
    f.c.units[u].combat.clear();
    hit_entry(&mut f, u, m, 1);
    f.take_log();
    dos::melee_state(&mut f, &t, &ct, u, 1, 1);
    assert!(f.take_log().contains(&format!("overlay {m} 9")));
    // The overlay counts 1…overlay count inclusive.
    for (ov, shown) in [(0u16, false), (200, true), (201, false)] {
        let mut t2 = t.clone();
        t2.skills[1].srvoverlay = ov;
        t2.skills[1].auratargetstate = 0xFFFF;
        f.c.units[u].combat.clear();
        hit_entry(&mut f, u, m, 1);
        f.take_log();
        dos::melee_state(&mut f, &t2, &ct, u, 1, 1);
        let log = f.take_log();
        assert_eq!(
            log.contains(&format!("overlay {m} {ov}")),
            shown,
            "overlay {ov}"
        );
    }
    // No target with an overlay in range → 0 (also for the target state).
    f.targets.remove(&u);
    assert_eq!(dos::melee_state(&mut f, &t, &ct, u, 1, 1), 0);
}

// Covers: specs/skills/bodies.md §4.2 r3
#[test]
fn melee_state_target_state_list() {
    let mut t = melee_state_tables();
    t.skills[1].srvoverlay = 0;
    t.skills[1].aurastate = 0xFFFF;
    let ct = ct3();
    let (mut f, u, m) = duel();
    f.c.frame = 100;
    // T none → 0.
    f.targets.remove(&u);
    assert_eq!(dos::melee_state(&mut f, &t, &ct, u, 1, 1), 0);
    f.targets.insert(u, m);
    // p none → return 1, nothing done.
    assert_eq!(dos::melee_state(&mut f, &t, &ct, u, 1, 1), 1);
    assert!(f.lists.is_empty());
    // p a miss: nothing for the target state.
    hit_entry(&mut f, u, m, 0);
    dos::melee_state(&mut f, &t, &ct, u, 1, 1);
    assert!(f.lists.is_empty());
    // p a hit: a new list (flags 2, expire F + len, owner T), timer 12,
    // state on, callback default, aura_fill formulas on T.
    f.c.units[u].combat.clear();
    hit_entry(&mut f, u, m, 1);
    f.take_log();
    assert_eq!(dos::melee_state(&mut f, &t, &ct, u, 1, 1), 1);
    let l = f.list_of(m, 30).expect("target list").clone();
    assert_eq!((l.flags, l.expire, l.owner), (2, 150, Some(m)));
    assert_eq!(
        (l.callback, l.stats.get(&25)),
        (callback::DEFAULT, Some(&4))
    );
    assert!(f.has_state(m, 30));
    assert!(f.take_log().contains(&format!("timer {m} 12 150")));
    // An existing list keeps its old expiry; a length below 1 counts as 1.
    f.c.frame = 120;
    f.c.units[u].combat.clear();
    hit_entry(&mut f, u, m, 1);
    dos::melee_state(&mut f, &t, &ct, u, 1, 1);
    assert_eq!(f.list_of(m, 30).unwrap().expire, 150);
    assert_eq!(f.lists.iter().filter(|l| l.state == 30).count(), 1);
    let mut c = Code::new();
    t.skills[1].auralencalc = c.f(-7);
    t.skills_code = c.0.clone();
    t.skills[1].aurastat1 = 0xFFFF;
    let (mut f, u, m) = duel();
    f.c.frame = 100;
    hit_entry(&mut f, u, m, 1);
    dos::melee_state(&mut f, &t, &ct, u, 1, 1);
    assert_eq!(f.list_of(m, 30).unwrap().expire, 101);
}

// Covers: specs/skills/bodies.md §4.2 r4, §4.2 r5
#[test]
fn melee_state_self_state_and_the_melee() {
    let mut t = melee_state_tables();
    t.skills[1].srvoverlay = 0;
    t.skills[1].auratargetstate = 0xFFFF;
    let ct = ct3();
    let (mut f, u, m) = duel();
    f.c.set(m, 6, 100_000);
    hit_entry(&mut f, u, m, 1);
    assert_eq!(dos::melee_state(&mut f, &t, &ct, u, 1, 1), 1);
    // A self list: flags 4, expire 0, callback default, aura_fill on the
    // unit, state on; reused by a second run.
    let l = f.list_of(u, 31).expect("self list");
    assert_eq!((l.flags, l.expire, l.callback), (4, 0, callback::DEFAULT));
    assert_eq!((l.owner, l.stats.get(&25)), (Some(u), Some(&4)));
    assert!(f.has_state(u, 31));
    // r5: flag 0x40 and apply_melee (the hit pair record is consumed).
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    assert!(f.c.units[u].combat.is_empty());
    hit_entry(&mut f, u, m, 1);
    dos::melee_state(&mut f, &t, &ct, u, 1, 1);
    assert_eq!(f.lists.iter().filter(|l| l.state == 31).count(), 1);
    // aurastate outside 1…count − 1: no list (0, count, none).
    for st in [0u16, 200, 0xFFFF] {
        let mut t2 = t.clone();
        t2.skills[1].aurastate = st;
        let (mut f, u, m) = duel();
        assert_eq!(dos::melee_state(&mut f, &t2, &ct, u, 1, 1), 1);
        assert!(f.lists.is_empty(), "aurastate {st}");
        let _ = m;
    }
    // r5: T none → 0 after the self state and the flag.
    let (mut f, u, _) = duel();
    f.targets.remove(&u);
    assert_eq!(dos::melee_state(&mut f, &t, &ct, u, 1, 1), 0);
    assert!(f.has_state(u, 31));
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
}

// Covers: specs/skills/bodies.md §4.3 r6
#[test]
fn buff_registers_up_to_three_aura_events() {
    let mut r = body_rec();
    r.aurastate = 20;
    r.auraevent1 = 5;
    r.auraeventfunc1 = 3;
    r.auraevent2 = 6;
    r.auraeventfunc2 = 4;
    r.auraevent3 = 0xFFFF;
    r.auraeventfunc3 = 9;
    let t = tabs(r, Code::new(), 1);
    let ct = ct3();
    let (mut f, u) = world();
    f.take_log();
    assert_eq!(dos::buff(&mut f, &t, &ct, u, 1, 1), 1);
    let log = f.take_log();
    let at = |s: &str| log.iter().position(|l| l == s);
    // Unregister (1, state) first, then the pairs in order; the third
    // (event −1) ends the loop.
    let (un, h1, h2) = (
        at(&format!("unhandle {u} 1 20")).expect("unregister"),
        at(&format!("handler {u} 5 3 20")).expect("first"),
        at(&format!("handler {u} 6 4 20")).expect("second"),
    );
    assert!(un < h1 && h1 < h2);
    assert_eq!(log.iter().filter(|l| l.starts_with("handler")).count(), 2);
    assert!(log.contains(&format!("changed {u} 20")));
    // auraevent1 < 0: nothing is unregistered or registered, even when
    // events 2 / 3 are set.
    let mut t2 = t.clone();
    t2.skills[1].auraevent1 = 0xFFFF;
    dos::buff(&mut f, &t2, &ct, u, 1, 1);
    let log = f.take_log();
    assert!(!log
        .iter()
        .any(|l| l.starts_with("handler") || l.starts_with("unhandle {u} 1")));
    // A function outside 1…31 registers nothing.
    let mut t3 = t.clone();
    t3.skills[1].auraeventfunc1 = 40;
    t3.skills[1].auraeventfunc2 = 0;
    dos::buff(&mut f, &t3, &ct, u, 1, 1);
    assert!(!f.take_log().iter().any(|l| l.starts_with("handler")));
}

/// A curse skill (record 1): state `st`, aurastat1 = 25 (calc 5), length
/// 40, range 100, filter 3 (players and monsters).
fn curse_tables(st: u16) -> crate::skills::SkillTables {
    let mut c = Code::new();
    let mut r = body_rec();
    r.auratargetstate = st;
    r.aurastat1 = 25;
    r.aurastatcalc1 = c.f(5);
    r.auralencalc = c.f(40);
    r.aurarangecalc = c.f(100);
    r.aurafilter = 3;
    // The other five slots: calc formulas for the tests to enable.
    for x in [
        &mut r.aurastatcalc2,
        &mut r.aurastatcalc3,
        &mut r.aurastatcalc4,
        &mut r.aurastatcalc5,
        &mut r.aurastatcalc6,
    ] {
        *x = c.f(7);
    }
    tabs(r, c, 1)
}

fn curse_ct() -> crate::combat::CombatTables {
    let mut ct = combat_tables(vec![monster_rec(), monster_rec()]);
    ct.monstats2[0].isatt = true;
    ct.monstats[0].switchai = true;
    ct
}

/// The caster (unit 0) aiming at (10, 10) and one monster there with the
/// flags 0xE, alive and hostile.
fn curse_world() -> (BodyFake, usize, usize) {
    let (mut f, u) = world();
    f.tpos.insert(u, (10, 10));
    let mut m = FUnit::new(UnitType::Monster, 0);
    m.type_flags = Some(0);
    m.flags = 0xE;
    let m = f.add(m, (10, 10));
    f.scan = vec![m];
    f.c.hostile = true;
    (f, u, m)
}

// Covers: specs/skills/bodies.md §4.4 text, §4.4 l2 r3, §4.4 l2 r4, §4.4 l2 r9
#[test]
fn curse_unit_conditions_and_state_list() {
    let (t, ct) = (curse_tables(30), curse_ct());
    let (mut f, u, m) = curse_world();
    f.c.frame = 100;
    // Slot 30 is the curse body.
    assert_eq!(run_do(&mut f, &t, &ct, 30, u, 1, 1), Some(1));
    let l = f.list_of(m, 30).expect("curse list").clone();
    assert_eq!(
        (l.expire, l.callback, l.skill, l.lvl),
        (140, callback::DEFAULT, 1, 1)
    );
    assert_eq!(l.stats.get(&25), Some(&5), "stat1 = value1");
    assert!(f.has_state(m, 30));
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    // r3: flags 0x4, 0x8 and 0x2 must all be set; alive; hostile; a
    // monster needs a walk mode and no type flag 0x20.
    let reject = |mut f: BodyFake, why: &str| {
        let u = 0;
        assert_eq!(dos::curse(&mut f, &t, &ct, u, 1, 1), 1, "{why}");
        assert!(f.lists.is_empty(), "{why}");
    };
    for bit in [0x4u32, 0x8, 0x2] {
        let (mut f, _, m) = curse_world();
        f.c.units[m].flags = 0xE & !bit;
        reject(f, &format!("flags without {bit:#x}"));
    }
    let (mut f, _, m) = curse_world();
    f.alive.remove(&m);
    reject(f, "dead");
    let (mut f, _, _) = curse_world();
    f.c.hostile = false;
    reject(f, "not hostile");
    let (mut f, _, m) = curse_world();
    f.c.units[m].no_walk = true;
    reject(f, "no walk mode");
    let (mut f, _, m) = curse_world();
    f.c.units[m].type_flags = Some(0x20);
    reject(f, "possessed");
    // A player target needs neither the walk mode nor the type flag.
    let (mut f, u, _) = curse_world();
    let mut p = FUnit::new(UnitType::Player, 1);
    p.flags = 0xE;
    p.no_walk = true;
    p.type_flags = Some(0x20);
    let p = f.add(p, (10, 10));
    f.scan = vec![p];
    assert_eq!(dos::curse(&mut f, &t, &ct, u, 1, 1), 1);
    assert!(f.list_of(p, 30).is_some());
    // r4: a Monster that cannot be afflicted (apply_state refuses an npc)
    // gets no list and the scan still returns 1.
    let mut ct = ct;
    ct.monstats[1].npc = true;
    let (mut f, u, m) = curse_world();
    f.c.units[m].class = 1;
    assert_eq!(dos::curse(&mut f, &t, &ct, u, 1, 1), 1);
    assert!(f.lists.is_empty());
}

// Covers: specs/skills/bodies.md §4.4 l2 r2, §4.4 l2 r6, §4.4 l2 r7
#[test]
fn curse_unit_scales_and_sets_stats() {
    let ct = curse_ct();
    // r2: stat1 a resistance, value ≤ 0, base ≥ 100 on a non-player →
    // value / 5; v1 = 0 skips the unit; no aurastat1 goes on with 0.
    let mut c = Code::new();
    let mut r = body_rec();
    r.auratargetstate = 30;
    r.aurastat1 = 39;
    r.aurastatcalc1 = c.f(-50);
    r.auralencalc = c.f(40);
    r.aurarangecalc = c.f(100);
    r.aurafilter = 3;
    r.aurastat2 = 26;
    r.aurastatcalc2 = c.f(7);
    r.aurastat3 = 27;
    r.aurastatcalc3 = c.f(0);
    r.aurastat4 = 28;
    r.aurastatcalc4 = c.f(-100);
    r.aurastat5 = 0xFFFF;
    r.aurastatcalc5 = c.f(9);
    r.aurastat6 = 29;
    r.aurastatcalc6 = c.f(9);
    let t = tabs(r, c, 1);
    let (mut f, u, m) = curse_world();
    f.c.set(m, 39, 120);
    dos::curse(&mut f, &t, &ct, u, 1, 1);
    let l = f.list_of(m, 30).expect("list");
    assert_eq!(l.stats.get(&39), Some(&-10), "−50 / 5");
    // r6: stats 2…6: valid, non-zero values set; a zero value is not
    // set; an invalid stat (−1) ends the list, so slot 6 is never read.
    assert_eq!(l.stats.get(&26), Some(&7));
    assert!(!l.stats.contains_key(&27));
    assert_eq!(l.stats.get(&28), Some(&-100));
    assert!(!l.stats.contains_key(&29));
    // The same value with a base resistance below 100 is unchanged.
    let (mut f, u, m) = curse_world();
    f.c.set(m, 39, 99);
    dos::curse(&mut f, &t, &ct, u, 1, 1);
    assert_eq!(f.list_of(m, 30).unwrap().stats.get(&39), Some(&-50));
    // A zero v1 skips the unit.
    let mut t0 = t.clone();
    let mut c = Code(t0.skills_code.clone());
    t0.skills[1].aurastatcalc1 = c.f(0);
    t0.skills_code = c.0;
    let (mut f, u, _) = curse_world();
    dos::curse(&mut f, &t0, &ct, u, 1, 1);
    assert!(f.lists.is_empty());
    // aurastat1 = −1 (none) goes on with v1 = 0: the state is applied
    // with no stat and slots 2…6 are not read (the stop is at slot 1).
    let mut tn = t.clone();
    tn.skills[1].aurastat1 = 0xFFFF;
    let (mut f, u, m) = curse_world();
    dos::curse(&mut f, &tn, &ct, u, 1, 1);
    assert!(f.list_of(m, 30).expect("applied").stats.is_empty());
    // r7: a stat with `updateanimrate` refreshes the target's animation.
    let (mut f, u, m) = curse_world();
    f.stat_infos.insert(
        26,
        BodyStat {
            updateanimrate: true,
            maxstat: -1,
            ..BodyStat::default()
        },
    );
    f.take_log();
    dos::curse(&mut f, &t, &ct, u, 1, 1);
    assert!(f.take_log().contains(&format!("anim {m}")));
    let (mut f, u, m) = curse_world();
    f.take_log();
    dos::curse(&mut f, &t, &ct, u, 1, 1);
    assert!(!f.take_log().contains(&format!("anim {m}")));
}

// Covers: specs/skills/bodies.md §4.4 l2 r8
#[test]
fn curse_unit_registers_the_first_three_event_pairs() {
    let ct = curse_ct();
    let mut t = curse_tables(30);
    let s = &mut t.skills[1];
    s.auraevent1 = 5;
    s.auraeventfunc1 = 3;
    s.auraevent2 = 6;
    s.auraeventfunc2 = 0;
    s.auraevent3 = 7;
    s.auraeventfunc3 = 4;
    let (mut f, u, m) = curse_world();
    f.take_log();
    dos::curse(&mut f, &t, &ct, u, 1, 1);
    let log = f.take_log();
    let at = |s: String| log.iter().position(|l| *l == s);
    let un = at(format!("unhandle {m} 1 30")).expect("unregister");
    let a = at(format!("handler {m} 5 3 30")).expect("pair 1");
    let c = at(format!("handler {m} 7 4 30")).expect("pair 3");
    assert!(un < a && a < c);
    // Pair 2 has function 0: refused by register. Three attempts → two
    // handlers.
    assert_eq!(log.iter().filter(|l| l.starts_with("handler")).count(), 2);
    // Event 1 < 0, or function 1 = 0: nothing at all (even for pairs 2
    // and 3).
    for (e1, f1) in [(0xFFFFu16, 3u16), (5, 0)] {
        let mut t2 = t.clone();
        t2.skills[1].auraevent1 = e1;
        t2.skills[1].auraeventfunc1 = f1;
        let (mut f, u, m) = curse_world();
        f.take_log();
        dos::curse(&mut f, &t2, &ct, u, 1, 1);
        let log = f.take_log();
        assert!(!log.iter().any(|l| l.starts_with("handler")));
        assert!(!log.contains(&format!("unhandle {m} 1 30")));
    }
}

// Covers: specs/skills/bodies.md §4.4 l2 r1, §4.4 l2 r5
#[test]
fn curse_unit_ai_curses_switch_the_monster_ai() {
    let mut ct = curse_ct();
    ct.difficultylevels[0].aicursedivisor = 4;
    for (st, k) in [(23u16, 10), (56, 11)] {
        let t = curse_tables(st);
        let (mut f, u, m) = curse_world();
        f.c.frame = 50;
        f.take_log();
        assert_eq!(dos::curse(&mut f, &t, &ct, u, 1, 1), 1);
        let l = f.list_of(m, i32::from(st)).expect("ai curse list");
        // r5: the AI curse callback; the AI control switched to k.
        assert_eq!(l.callback, callback::AI_CURSE);
        assert!(f.take_log().contains(&format!("ai {m} {k}")));
        // Duration: 40 / AiCurseDivisor.
        assert_eq!(f.list_of(m, i32::from(st)).unwrap().expire, 60);
        // A non-AI curse state keeps the full length and sets no AI.
        let t = curse_tables(30);
        let (mut f, u, m) = curse_world();
        f.c.frame = 50;
        f.take_log();
        dos::curse(&mut f, &t, &ct, u, 1, 1);
        assert_eq!(f.list_of(m, 30).unwrap().expire, 90);
        assert!(!f.take_log().iter().any(|l| l.starts_with("ai ")));
    }
    // r1: the unit must be a monster of alignment ≠ 1 that can switch.
    let t = curse_tables(23);
    let none = |f: BodyFake, why: &str| {
        let mut f = f;
        assert_eq!(dos::curse(&mut f, &t, &ct, 0, 1, 1), 1);
        assert!(f.lists.is_empty(), "{why}");
    };
    let (mut f, u, _) = curse_world();
    let mut p = FUnit::new(UnitType::Player, 1);
    p.flags = 0xE;
    let p = f.add(p, (10, 10));
    f.scan = vec![p];
    let _ = u;
    none(f, "player");
    let (mut f, _, m) = curse_world();
    f.c.units[m].align = 1;
    none(f, "alignment 1");
    let (mut f, _, m) = curse_world();
    f.c.units[m].type_flags = Some(2);
    none(f, "superunique");
    let (mut f, _, m) = curse_world();
    f.c.units[m].type_flags = Some(8);
    none(f, "unique");
    let (mut f, _, m) = curse_world();
    f.c.units[m].class = 1;
    none(f, "class without switchai (and npc)");
    let (mut f, _, m) = curse_world();
    f.c.units[m].states.push(54);
    none(f, "uninterruptable");
}

// Covers: specs/skills/bodies.md §4.4 text
#[test]
fn can_switch_follows_the_original_conditions() {
    let mut ct = curse_ct();
    let (mut f, _, m) = curse_world();
    for k in [10, 11, 12, 19, 0, 5] {
        assert!(dos::can_switch(&mut f, &ct, m, k), "k = {k}");
    }
    assert!(!dos::can_switch(&mut f, &ct, m, 20), "k > 19");
    // Base class 492 with state 143 (attached) → 0.
    ct.monstats[0].baseid = 492;
    assert!(dos::can_switch(&mut f, &ct, m, 19));
    f.c.units[m].states.push(143);
    assert!(!dos::can_switch(&mut f, &ct, m, 19));
    f.c.units[m].states.clear();
    ct.monstats[0].baseid = 0;
    // State 54, no switchai, boss, no walk mode.
    f.c.units[m].states.push(54);
    assert!(!dos::can_switch(&mut f, &ct, m, 19));
    f.c.units[m].states.clear();
    ct.monstats[0].switchai = false;
    assert!(!dos::can_switch(&mut f, &ct, m, 19));
    ct.monstats[0].switchai = true;
    ct.monstats[0].boss = true;
    assert!(!dos::can_switch(&mut f, &ct, m, 19));
    ct.monstats[0].boss = false;
    f.c.units[m].no_walk = true;
    assert!(!dos::can_switch(&mut f, &ct, m, 19));
    f.c.units[m].no_walk = false;
    // Unit flag 0x4 unset and dead → 0; alive or flag set → ok.
    f.c.units[m].flags = 0;
    f.alive.remove(&m);
    assert!(!dos::can_switch(&mut f, &ct, m, 19));
    f.alive.insert(m);
    assert!(dos::can_switch(&mut f, &ct, m, 19));
    f.alive.remove(&m);
    f.c.units[m].flags = 4;
    assert!(dos::can_switch(&mut f, &ct, m, 19));
    f.alive.insert(m);
    // Superunique (2) / unique (8) type flags → 0, for every k.
    for fl in [2u32, 8] {
        f.c.units[m].type_flags = Some(fl);
        for k in [10, 19, 0] {
            assert!(!dos::can_switch(&mut f, &ct, m, k), "flag {fl} k {k}");
        }
    }
    // A non-monster never switches.
    let p = f.add(FUnit::new(UnitType::Player, 0), (1, 1));
    assert!(!dos::can_switch(&mut f, &ct, p, 19));
}

// ---------------------------------------------------------------- §5

// Covers: specs/skills/bodies.md §5 text, §5 r1, §5 r2, §5 r3, §5 r4, §5 r5
#[test]
fn srvmissile_path_of_the_do_core() {
    use crate::skills::use_::tests::{caster, tables, F, U};
    use crate::skills::use_::{do_core, MissileAim};
    let t = tables(
        4,
        &[
            (1, &|r| r.srvmissile = 0),
            (2, &|r| {
                r.srvmissile = 0;
                r.lob = true;
            }),
            (3, &|r| r.srvmissile = 5),
        ],
    );
    // r1: srvmissile < 0 or without a missiles record: skipped (the
    // result is the do function's, here none → 0).
    for skill in [0, 3] {
        let mut f = F::new();
        let p = caster(&mut f, skill, 1, 0);
        assert_eq!(do_core(&mut f, &t, p, skill, 1, false, false, false), 0);
        assert!(f.units[p].flags & 0x40 == 0 && f.take_log().is_empty());
    }
    // r2, r4, r5: flags |= 0x40, the straight / lob creation, result 1
    // whatever the creation gave (the seam returns nothing).
    for (skill, lob) in [(1, false), (2, true)] {
        let mut f = F::new();
        let p = caster(&mut f, skill, 1, 0);
        assert_eq!(do_core(&mut f, &t, p, skill, 1, false, false, false), 1);
        assert_eq!(f.units[p].flags & 0x40, 0x40);
        assert_eq!(
            f.take_log(),
            [format!("missile {p} {skill} 1 0 {lob} None")]
        );
    }
    // r3: item and aim: the target position gives the offset and the aim
    // point (2 × target − unit); no item or no aim → position 0.
    let mut f = F::new();
    let p = caster(&mut f, 1, 1, 0);
    let mut tg = U::new(UnitType::Monster, 0);
    tg.pos = (140, 160);
    let tg = f.add(tg);
    f.units[p].target = Some(tg);
    f.units[p].pos = (100, 100);
    for (item, aim, at) in [
        (
            true,
            true,
            MissileAim::At {
                offset: (40, 60),
                aim: (180, 220),
            },
        ),
        (true, false, MissileAim::None),
        (false, true, MissileAim::None),
    ] {
        f.take_log();
        assert_eq!(do_core(&mut f, &t, p, 1, 1, false, item, aim), 1);
        assert_eq!(f.take_log(), [format!("missile {p} 1 1 0 false {at:?}")]);
    }
    // A target position with a 0 coordinate fails (`0x0056D2C0`).
    f.units[tg].pos = (0, 160);
    f.take_log();
    do_core(&mut f, &t, p, 1, 1, false, true, true);
    assert_eq!(f.take_log(), [format!("missile {p} 1 1 0 false None")]);
}
