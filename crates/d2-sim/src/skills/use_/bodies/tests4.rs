// Spec: specs/skills/bodies-2.md
//! Coverage tests of the batch 3 helpers and bodies (`bodies-2.md` §2–§5
//! and its edge cases) on [`super::fake::BodyFake`]. Values are computed
//! by hand from the rules.

use super::fake::{BodyFake, FList};
use super::tests2::{body_rec, monster, tabs, world, Code};
use super::*;
use crate::combat::{CombatTables, DamageRecord};
use crate::rng::Seed;
use crate::skills::fake::{combat_tables, monster_rec, FItem, FUnit};
use crate::skills::{SkillEntry, SkillTables, SkillUnits};
use crate::units::UnitType;

/// A live list of `state` on `u` with `stats`.
fn give_list(f: &mut BodyFake, u: usize, state: i32, stats: &[(i32, i32)]) -> usize {
    let mut l = FList {
        state,
        unit: Some(u),
        ..FList::default()
    };
    for &(k, v) in stats {
        l.stats.insert(k, v);
    }
    f.lists.push(l);
    f.lists.len() - 1
}

/// The seed after one `step`.
fn stepped(s: Seed) -> Seed {
    let mut s = s;
    s.step();
    s
}

/// A seed for the unit whose first draw `step() % 100` falls in `want`.
fn seed_with(want: std::ops::Range<u32>) -> Seed {
    let lo = (0u32..)
        .find(|&lo| want.contains(&(Seed::new(lo, 0).step() % 100)))
        .unwrap();
    Seed::new(lo, 0)
}

/// A seed for which a melee hit of `u` on `m` is a plain hit.
fn hit_seed(f: &BodyFake, t: &SkillTables, ct: &CombatTables, u: usize, m: usize) -> Seed {
    (1u32..)
        .map(|lo| Seed::new(lo, 0))
        .find(|&s| {
            let mut g = f.clone();
            g.c.units[u].seed = s;
            crate::combat::melee_result(&mut g.c, t, ct, Some(u), Some(m), 100_000, 0) & 1 != 0
        })
        .unwrap()
}

fn hit_world() -> (BodyFake, usize, usize) {
    let (mut f, u) = world();
    let m = monster(&mut f, (1, 0));
    f.targets.insert(u, m);
    f.c.hostile = true;
    f.c.in_range = true;
    f.c.set(u, 12, 1);
    f.c.set(m, 12, 1);
    f.c.set(u, 19, 100_000);
    (f, u, m)
}

// ---------------------------------------------------------------- §2.2

// Covers: specs/skills/bodies-2.md §2.2 text, §2.2 r1, §2.2 r2
#[test]
fn minion_damage_reads_the_owners_skill() {
    let mut r = body_rec();
    r.mindam = 10;
    r.maxdam = 20;
    r.hitshift = 8;
    r.tohit = 100;
    r.levtohit = 10;
    let t = tabs(r, Code::new(), 1);
    let mut mr = monster_rec();
    mr.skilldamage = 1;
    let ct = combat_tables(vec![mr]);
    let (mut f, o) = world();
    let m = monster(&mut f, (5, 5));
    // No minion owner: S stays zero.
    assert_eq!(minion_damage(&mut f, &t, &ct, m), (0, 0, 0));
    f.minion_owner.insert(m, o);
    // Owner has no entry of the skill: level 0 (to-hit of level 0 is 0).
    f.c.units[o].skills.clear();
    assert_eq!(minion_damage(&mut f, &t, &ct, m), (10, 20, 0));
    // Base level 5, native entry: level_bonus is added (owner GUID −1).
    let e = SkillEntry {
        skill: 1,
        base: 5,
        owner_guid: -1,
        level_bonus: 3,
        ..SkillEntry::default()
    };
    f.c.units[o].skills = vec![e];
    assert_eq!(minion_damage(&mut f, &t, &ct, m), (10, 20, 100 + 7 * 10));
    // An item-granted entry (owner GUID ≠ −1) takes no bonus level.
    f.c.units[o].skills[0].owner_guid = 7;
    assert_eq!(minion_damage(&mut f, &t, &ct, m), (10, 20, 100 + 4 * 10));
    // The level is capped at the maximum character level.
    f.c.units[o].skills[0].owner_guid = -1;
    f.c.units[o].skills[0].base = 500;
    f.c.units[o].skills[0].level_bonus = 0;
    let cap = t.level_cap;
    assert_eq!(
        minion_damage(&mut f, &t, &ct, m).2,
        100 + (cap - 1) * 10,
        "capped"
    );
    // k out of range (0 or above the count) leaves S zero.
    let mut mr0 = monster_rec();
    mr0.skilldamage = 0;
    let ct0 = combat_tables(vec![mr0]);
    assert_eq!(minion_damage(&mut f, &t, &ct0, m), (0, 0, 0));
    let mut mr3 = monster_rec();
    mr3.skilldamage = 3; // count is 2
    let ct3 = combat_tables(vec![mr3]);
    assert_eq!(minion_damage(&mut f, &t, &ct3, m), (0, 0, 0));
    // The count itself is accepted (a skill without a record: min 1, max 2
    // >> 8 = 0).
    let mut mr2 = monster_rec();
    mr2.skilldamage = 2;
    let ct2 = combat_tables(vec![mr2]);
    assert_eq!(minion_damage(&mut f, &t, &ct2, m).0, 0);
    // A non-monster has none.
    assert_eq!(minion_damage(&mut f, &t, &ct, o), (0, 0, 0));
}

// ---------------------------------------------------------------- §2.5

fn kick_world(mindam: u32, maxdam: u32) -> (BodyFake, SkillTables, CombatTables, usize, usize) {
    let mut r = body_rec();
    r.mindam = mindam;
    r.maxdam = maxdam;
    r.hitshift = 8;
    let t = tabs(r, Code::new(), 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u) = world();
    let m = monster(&mut f, (1, 0));
    f.targets.insert(u, m);
    (f, t, ct, u, m)
}

// Covers: specs/skills/bodies-2.md §2.5 text, §2.5 r1, §2.5 r2, §2.5 r3, §2.5 r4, §2.5 r5, §2.5 r6
#[test]
fn kick_damage_rolls_the_physical_part() {
    // Fixed damage 10: lo = hi, no draw. E = 50 %: 2560 + 1280 = 3840.
    let (mut f, t, ct, u, m) = kick_world(10, 10);
    let seed0 = f.c.units[u].seed;
    let mut rec = DamageRecord {
        enh_pct: 50,
        ..DamageRecord::default()
    };
    helpers3::kick_damage(&mut f, &t, &ct, u, &mut rec, m, 1, 1);
    assert_eq!(rec.physical, 3840);
    assert_eq!(rec.hit_flags & 3, 3, "hit flags |= 3");
    assert_eq!(f.c.units[u].seed, seed0, "d < 1: no draw");
    // Kick stats: KICKDAMAGE 4 adds to both, shifted by 8, with E2 = E.
    let (mut f, t, ct, u, m) = kick_world(10, 10);
    f.c.set(u, 137, 4);
    let mut rec = DamageRecord {
        enh_pct: 50,
        ..DamageRecord::default()
    };
    helpers3::kick_damage(&mut f, &t, &ct, u, &mut rec, m, 1, 1);
    let kick = (4 << 8) + (4 << 8) * 50 / 100;
    assert_eq!(rec.physical, 3840 + kick);
    // Min 10, max 20: d = 2560 > 0, one draw x = roll(d) on the unit's
    // seed; physical += lo + x.
    let (mut f, t, ct, u, m) = kick_world(10, 20);
    let mut s = f.c.units[u].seed;
    let x = s.roll(2560) as i32;
    let mut rec = DamageRecord::default();
    helpers3::kick_damage(&mut f, &t, &ct, u, &mut rec, m, 1, 1);
    assert_eq!(rec.physical, 2560 + x);
    assert_eq!(f.c.units[u].seed, s, "exactly one draw");
    // A power-of-two range uses `lo' & (d − 1)`: min 0, max 1 (<< 8).
    let (mut f, t, ct, u, m) = kick_world(0, 1);
    let mut s = f.c.units[u].seed;
    let x = s.roll(256) as i32;
    assert_eq!(x as u32, stepped(f.c.units[u].seed).lo & 255);
    let mut rec = DamageRecord::default();
    helpers3::kick_damage(&mut f, &t, &ct, u, &mut rec, m, 1, 1);
    assert_eq!(rec.physical, x);
}

// Covers: specs/skills/bodies-2.md §2.5 r6, §2.5 r7, §2.5 r8
#[test]
fn kick_damage_toggles_the_weapons() {
    let (mut f, t, ct, u, m) = kick_world(10, 10);
    f.inventory = true;
    let w = f.c.add_item(FItem {
        types: vec![45],
        ..FItem::default()
    });
    let o = f.c.add_item(FItem {
        types: vec![45],
        ..FItem::default()
    });
    f.c.units[u].weapon = Some(w);
    f.c.units[u].items.insert(5, o);
    f.take_log();
    let mut rec = DamageRecord::default();
    helpers3::kick_damage(&mut f, &t, &ct, u, &mut rec, m, 1, 1);
    let log = f.take_log();
    let off = |i: usize| format!("ItemLists {{ u: {u}, item: {i}, on: false }}");
    let on = |i: usize| format!("ItemLists {{ u: {u}, item: {i}, on: true }}");
    let pos = |s: &str| log.iter().position(|l| l == s);
    assert!(pos(&off(w)).is_some() && pos(&off(o)).is_some(), "{log:?}");
    assert!(pos(&on(w)).is_some(), "the weapon is toggled back on");
    assert!(pos(&on(o)).is_none(), "the other hand is not (Edge case 2)");
    assert!(pos(&off(w)) < pos(&on(w)));
    // A weapon of another item type is neither toggled off nor on.
    let (mut f, t, ct, u, m) = kick_world(10, 10);
    f.inventory = true;
    let w = f.c.add_item(FItem {
        types: vec![46],
        ..FItem::default()
    });
    f.c.units[u].weapon = Some(w);
    f.take_log();
    helpers3::kick_damage(&mut f, &t, &ct, u, &mut DamageRecord::default(), m, 1, 1);
    assert!(
        !f.take_log().iter().any(|l| l.starts_with("ItemLists")),
        "type 46 is not a weapon of item type 45"
    );
}

// ---------------------------------------------------------------- §2.7

// Covers: specs/skills/bodies-2.md §2.7
#[test]
fn knock_chance_column_by_target_kind() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(11);
    r.calc2 = c.f(22);
    r.calc3 = c.f(33);
    r.calc4 = c.f(44);
    let t = tabs(r, c, 1);
    let (mut f, u) = world();
    let m = monster(&mut f, (1, 1));
    // Ordinary monster: calc1, or the given value (no formula).
    assert_eq!(knock_chance(&mut f, &t, u, m, 1, 1, None), 11);
    assert_eq!(knock_chance(&mut f, &t, u, m, 1, 1, Some(100)), 100);
    // Type flag 8 (unique): calc2.
    f.c.units[m].flags = 8;
    assert_eq!(knock_chance(&mut f, &t, u, m, 1, 1, None), 22);
    assert_eq!(knock_chance(&mut f, &t, u, m, 1, 1, Some(100)), 22);
    // Boss: calc3 (before the unique test).
    f.c.units[m].boss = true;
    assert_eq!(knock_chance(&mut f, &t, u, m, 1, 1, Some(100)), 33);
    // A hireling: calc4; a player: calc4.
    f.c.units[m].hireling = true;
    assert_eq!(knock_chance(&mut f, &t, u, m, 1, 1, Some(100)), 44);
    let p = f.add(FUnit::new(UnitType::Player, 0), (2, 2));
    assert_eq!(knock_chance(&mut f, &t, u, p, 1, 1, None), 44);
    // The level is passed through unchanged (vestigial clamp): calc with
    // the skill level is the same for every L.
    assert_eq!(knock_chance(&mut f, &t, u, p, 1, 40, None), 44);
}

// ---------------------------------------------------------------- §2.8

// Covers: specs/skills/bodies-2.md §2.8
#[test]
fn set_uninterruptable_ends_death_delay() {
    let (mut f, u) = world();
    let m = monster(&mut f, (3, 3));
    set_uninterruptable(&mut f, m, true);
    assert!(f.c.has_state(m, 54));
    // v = 1 never touches death_delay.
    f.c.units[m].states.push(92);
    set_uninterruptable(&mut f, m, true);
    assert!(f.c.has_state(m, 92));
    f.take_log();
    // v = 0 with death_delay and no last attacker: kill (u, 0, 1).
    set_uninterruptable(&mut f, m, false);
    assert!(!f.c.has_state(m, 54));
    assert!(!f.c.has_state(m, 92), "death_delay off");
    let log = f.take_log();
    assert!(
        log.iter()
            .any(|l| l == &format!("Kill {{ u: {m}, a: 0, b: 1 }}")),
        "{log:?}"
    );
    assert!(
        log.iter().any(|l| l == &format!("deltimers {m} 2 0")),
        "a monster's type-2 timers are deleted"
    );
    // A player: no timer deletion; without death_delay nothing else.
    set_uninterruptable(&mut f, u, false);
    let log = f.take_log();
    assert!(!log.iter().any(|l| l.starts_with("deltimers")), "{log:?}");
    assert!(!log.iter().any(|l| l.starts_with("Kill")));
}

// ---------------------------------------------------------------- §2.9

// Covers: specs/skills/bodies-2.md §2.9
#[test]
fn nearest_takes_the_first_of_equal_distances() {
    let t = tabs(body_rec(), Code::new(), 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u) = world();
    f.c.hostile = true;
    let far = monster(&mut f, (6, 0));
    let a = monster(&mut f, (0, 3));
    let b = monster(&mut f, (3, 0));
    let rejected = monster(&mut f, (1, 0));
    for x in [far, a, b, rejected] {
        f.c.units[x].flags = 0xC; // scan filter 0x80 / 0x400 flag tests
    }
    f.scan = vec![far, a, b, rejected];
    let ok = |_: &mut BodyFake, x: usize| x != rejected;
    // d² 36, 9, 9: the first of the equally near units (strictly <).
    assert_eq!(nearest(&mut f, &t, &ct, u, 10, &ok), Some(a));
    f.scan = vec![far, b, a, rejected];
    assert_eq!(nearest(&mut f, &t, &ct, u, 10, &ok), Some(b));
    // The test rejects every unit: none. r = 0 scans nothing.
    assert_eq!(nearest(&mut f, &t, &ct, u, 10, &|_, _| false), None);
    assert_eq!(nearest(&mut f, &t, &ct, u, 0, &ok), None);
}

// ---------------------------------------------------------------- §2.11

// Covers: specs/skills/bodies-2.md §2.11 text, §2.11 r0
#[test]
fn prog_count_picks_the_calc_by_the_stat() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.progressive = true;
    r.aurastate = 60;
    r.aurastat1 = 100;
    r.prgcalc1 = c.f(5);
    r.prgcalc2 = c.f(6);
    r.prgcalc3 = c.f(7);
    let t = tabs(r, c, 1);
    let (mut f, u) = world();
    // No list of the state: prgcalc1.
    assert_eq!(prog_count(&mut f, &t, u, 1, 1), 5);
    // list[aurastat1] clamped to 1…3 → prgcalc_n.
    let l = give_list(&mut f, u, 60, &[(100, 0)]);
    assert_eq!(prog_count(&mut f, &t, u, 1, 1), 5, "0 clamps to 1");
    f.lists[l].stats.insert(100, 2);
    assert_eq!(prog_count(&mut f, &t, u, 1, 1), 6);
    f.lists[l].stats.insert(100, 3);
    assert_eq!(prog_count(&mut f, &t, u, 1, 1), 7);
    f.lists[l].stats.insert(100, 9);
    assert_eq!(prog_count(&mut f, &t, u, 1, 1), 7, "clamped to 3");
    // Not progressive: prgcalc1 in every case.
    let mut r = body_rec();
    r.progressive = false;
    r.aurastate = 60;
    r.aurastat1 = 100;
    let mut c = Code::new();
    r.prgcalc1 = c.f(5);
    r.prgcalc2 = c.f(6);
    let t2 = tabs(r, c, 1);
    f.lists[l].stats.insert(100, 2);
    assert_eq!(prog_count(&mut f, &t2, u, 1, 1), 5);
    // Invalid skill: 0.
    assert_eq!(prog_count(&mut f, &t, u, 99, 1), 0);
}

// ---------------------------------------------------------------- §2.12

// Covers: specs/skills/bodies-2.md §2.12 text, §2.12 r1, §2.12 r2, §2.12 r3, §2.12 r4
#[test]
fn claw_hit_adds_enhanced_damage_and_stores_the_entry() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(40);
    let t = tabs(r, c, 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u, m) = hit_world();
    f.c.units[u].seed = hit_seed(&f, &t, &ct, u, m);
    f.c.set(u, 325, 100_000);
    helpers3::claw_hit(&mut f, &t, &ct, u, m, 1, 1);
    let e = f.c.units[u].combat.last().expect("start_combat stored");
    assert_eq!(e.record.enh_pct, 40, "enh % += eval(calc1)");
    assert_eq!(e.record.hit_flags & 2, 2, "hit flags |= 2");
    assert_eq!(e.record.result & 1, 1);
    // A miss still stores an entry (r4): no tohit at all, not in range.
    let (mut f, u, m) = hit_world();
    f.c.in_range = false;
    helpers3::claw_hit(&mut f, &t, &ct, u, m, 1, 1);
    let e = f.c.units[u].combat.last().expect("also on a miss");
    assert_eq!(e.record.result & 1, 0);
    assert_eq!(e.record.enh_pct, 0);
    // R invalid: nothing.
    let (mut f, u, m) = hit_world();
    helpers3::claw_hit(&mut f, &t, &ct, u, m, 99, 1);
    assert!(f.c.units[u].combat.is_empty());
}

// ---------------------------------------------------------------- §2.14

// Covers: specs/skills/bodies-2.md §2.14 text, §2.14 r1, §2.14 r2, §2.14 r3
#[test]
fn wear_quantity_and_durability() {
    let (mut f, u) = world();
    let wpn = f.c.add_item(FItem {
        types: vec![45],
        durability: true,
        ..FItem::default()
    });
    f.c.units[u].seed = seed_with(0..10);
    f.item_stats.insert((wpn, 72), 20);
    f.take_log();
    // r < chance: durability 20 − 5 = 15, stat message.
    helpers3::wear(&mut f, u, wpn, 10, 5);
    assert_eq!(f.item_stats[&(wpn, 72)], 15);
    // r ≥ chance: nothing (one draw consumed either way).
    f.c.units[u].seed = seed_with(50..100);
    let before = f.c.units[u].seed;
    helpers3::wear(&mut f, u, wpn, 10, 5);
    assert_eq!(f.item_stats[&(wpn, 72)], 15);
    assert_eq!(f.c.units[u].seed, stepped(before), "one draw");
    // d ≤ 0: the item breaks.
    f.c.units[u].seed = seed_with(0..10);
    f.take_log();
    helpers3::wear(&mut f, u, wpn, 10, 15);
    let log = f.take_log();
    assert!(
        log.iter()
            .any(|l| l == &format!("BreakItem {{ u: {u}, item: {wpn} }}")),
        "{log:?}"
    );
    // Not item type 45: nothing, no draw.
    let other = f.c.add_item(FItem {
        types: vec![46],
        durability: true,
        ..FItem::default()
    });
    let before = f.c.units[u].seed;
    helpers3::wear(&mut f, u, other, 100, 1);
    assert_eq!(f.c.units[u].seed, before);
    // A stack with quantity 0: no draw; with quantity > 0 one draw.
    let stack = f.c.add_item(FItem {
        types: vec![45],
        throw: true,
        ..FItem::default()
    });
    helpers3::wear(&mut f, u, stack, 100, 1);
    assert_eq!(f.c.units[u].seed, before, "quantity 0: no draw");
    f.item_stats.insert((stack, 70), 3);
    f.c.units[u].items.insert(4, stack);
    f.c.units[u].weapon = Some(stack);
    helpers3::wear(&mut f, u, stack, 100, 1);
    assert_eq!(f.c.units[u].seed, stepped(before), "one draw");
    assert_eq!(f.item_stats[&(stack, 70)], 2, "dec_quantity");
}

// ---------------------------------------------------------------- §2.17

/// The stored record of `a` against `d` (kept when `a` is dead-moded, so
/// `apply_melee` returns before it frees the entry).
fn stored_record(f: &BodyFake, a: usize, d: usize) -> Option<DamageRecord> {
    let id = crate::combat::CombatWorld::ident(&f.c, d);
    f.c.units[a]
        .combat
        .iter()
        .find(|e| e.defender == id)
        .map(|e| e.record)
}

// Covers: specs/skills/bodies-2.md §2.17
#[test]
fn skill_result_and_set_len() {
    // ResultFlags bit 1 (value 2) set: result := ResultFlags, no roll.
    let mut r = body_rec();
    r.resultflags = 2 | 8;
    let t = tabs(r, Code::new(), 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u, m) = hit_world();
    let seed0 = f.c.units[u].seed;
    let mut rec = DamageRecord::default();
    let res = helpers3::skill_result(&mut f, &t, &ct, u, m, 1, 1, &mut rec, 0);
    assert_eq!((res, rec.result), (10, 10));
    assert_eq!(f.c.units[u].seed, seed0, "no roll");
    // Bit clear: melee_result, hit → | ResultFlags; miss → as rolled.
    let mut r = body_rec();
    r.resultflags = 8;
    let t = tabs(r, Code::new(), 1);
    let (mut f, u, m) = hit_world();
    f.c.units[u].seed = hit_seed(&f, &t, &ct, u, m);
    let res = helpers3::skill_result(&mut f, &t, &ct, u, m, 1, 1, &mut rec, 0);
    assert_eq!(res & 9, 9, "hit | ResultFlags");
    f.c.in_range = false;
    let res = helpers3::skill_result(&mut f, &t, &ct, u, m, 1, 1, &mut rec, 0);
    assert_eq!((res, rec.result), (0, 0), "a miss keeps no ResultFlags");
    // set_len by EType.
    let len = |etype: u8| {
        let mut r = body_rec();
        r.etype = etype;
        let t = tabs(r, Code::new(), 1);
        let mut rec = DamageRecord::default();
        helpers3::set_len(&t, &mut rec, 77, 1);
        (
            rec.cold_len,
            rec.poison_len,
            rec.stun_len,
            rec.burn_len,
            rec.freeze_len,
        )
    };
    assert_eq!(len(4), (77, 0, 0, 0, 0));
    assert_eq!(len(5), (0, 77, 0, 0, 0));
    assert_eq!(len(9), (0, 0, 77, 0, 0));
    assert_eq!(len(11), (0, 0, 0, 77, 0));
    assert_eq!(len(12), (0, 0, 0, 0, 77));
    assert_eq!(len(1), (0, 0, 0, 0, 0));
}

// ---------------------------------------------------------------- §2.18

// Covers: specs/skills/bodies-2.md §2.18
#[test]
fn plague_infects_once_and_spreads() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.auratargetstate = 50;
    r.srvmissilea = 1;
    r.aurastat1 = 100;
    r.aurastatcalc1 = c.f(9);
    let t = tabs(r, c, 2);
    let (mut f, u) = world();
    let tg = monster(&mut f, (4, 4));
    f.c.frame = 1000;
    // L < 1: nothing.
    assert_eq!(helpers3::plague(&mut f, &t, u, tg, 50, 1, 0), 0);
    assert!(f.lists.is_empty() && f.missiles.is_empty());
    assert_eq!(helpers3::plague(&mut f, &t, u, tg, 50, 1, 3), 1);
    // Infect: flags 2, expire F + len, state on T, callback 0x0056E900,
    // timer 12 at e, aura stats filled.
    let l = f.state_list(tg, 50).expect("list of the state");
    let li = &f.lists[l];
    assert_eq!((li.flags, li.expire, li.owner), (2, 1050, Some(tg)));
    assert_eq!(li.callback, callback::DEFAULT);
    assert!(f.c.has_state(tg, 50));
    assert!(f.c.log.contains(&format!("timer {tg} 12 1050")));
    assert_eq!(f.list_get(l, 100), 9, "aura_fill on the caster's formulas");
    // Spreader: flags 0x8000, owner = origin = T, class m, range len, then
    // the caster's type / GUID in the missile data.
    assert_eq!(f.missiles.len(), 1);
    let q = f.missiles[0];
    assert_eq!(
        (q.flags, q.class, q.skill, q.level, q.range),
        (0x8000, 1, 1, 3, 50)
    );
    assert_eq!((q.owner, q.origin), (tg, Some(tg)));
    let log = f.take_log();
    assert!(log
        .iter()
        .any(|s| s.starts_with("MissileData28") && s.contains("v: 0")));
    assert!(log
        .iter()
        .any(|s| s.starts_with("MissileData2C") && s.contains("v: 0")));
    // Already infected: no new list, no spreader.
    f.missiles.clear();
    assert_eq!(helpers3::plague(&mut f, &t, u, tg, 50, 1, 3), 1);
    assert!(f.missiles.is_empty());
    assert_eq!(f.lists.len(), 1);
    // A state outside 0…count: no infection (the count itself is accepted).
    let mut r = body_rec();
    r.auratargetstate = 201;
    let t2 = tabs(r, Code::new(), 1);
    helpers3::plague(&mut f, &t2, u, tg, 50, 1, 3);
    assert_eq!(f.lists.len(), 1);
}

// ---------------------------------------------------------------- §2.20

// Covers: specs/skills/bodies-2.md §2.20
#[test]
fn base_roll_stats_by_weapon() {
    let (mut f, u) = world();
    // No weapon: max(21, 1) << 8 = 256, max(22, 2) << 8 = 512: roll(256).
    let mut s = f.c.units[u].seed;
    let x = s.roll(256) as i32;
    assert_eq!(helpers3::base_roll(&mut f, u), 256 + x);
    assert_eq!(f.c.units[u].seed, s, "one draw");
    // A weapon (not wield type 2): stats 21 / 22.
    f.c.set(u, 21, 10);
    f.c.set(u, 22, 20);
    f.c.set(u, 23, 3);
    f.c.set(u, 24, 3);
    let w = f.c.add_item(FItem::default());
    f.c.units[u].weapon = Some(w);
    f.c.units[u].seed = Seed::new(1, 0);
    let mut s = Seed::new(1, 0);
    let x = s.roll(2560) as i32;
    assert_eq!(helpers3::base_roll(&mut f, u), 2560 + x);
    // Wield type 2: stats 23 / 24; max ≤ min → max = min + 256.
    f.c.items[w].wield = 2;
    f.c.units[u].seed = Seed::new(1, 0);
    let mut s = Seed::new(1, 0);
    let x = s.roll(256) as i32;
    assert_eq!(helpers3::base_roll(&mut f, u), 768 + x);
    // A weapon with a zero minimum: 256 (the "< 1" rule), no max(…, 1).
    f.c.set(u, 23, 0);
    f.c.set(u, 24, 0);
    f.c.units[u].seed = Seed::new(1, 0);
    let mut s = Seed::new(1, 0);
    let x = s.roll(256) as i32;
    assert_eq!(helpers3::base_roll(&mut f, u), 256 + x);
}

// ---------------------------------------------------------------- §2.21

// Covers: specs/skills/bodies-2.md §2.21
#[test]
fn next_unit_and_count_units() {
    let t = tabs(body_rec(), Code::new(), 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u) = world();
    f.c.hostile = true;
    let ms: Vec<usize> = (0..4)
        .map(|i| {
            let m = monster(&mut f, (i + 1, 0));
            f.c.units[m].flags = 0xC;
            m
        })
        .collect();
    // GUIDs are the fake's indices (1..=4); scan order is reversed.
    f.scan = ms.iter().rev().copied().collect();
    let c = helpers3::count_units(&mut f, &t, &ct, u, (0, 0), 20, 0);
    assert_eq!(c, 4);
    // Smallest GUID above g; the count covers every accepted unit.
    let (k, n) = helpers3::next_unit(&mut f, &t, &ct, u, (0, 0), 20, 0, ms[1] as u32);
    assert_eq!((k, n), (Some(ms[2]), 4));
    // Nothing above: wraps to the smallest GUID.
    let (k, n) = helpers3::next_unit(&mut f, &t, &ct, u, (0, 0), 20, 0, ms[3] as u32);
    assert_eq!((k, n), (Some(ms[0]), 4));
    // Out of range units are not counted.
    let (_, n) = helpers3::next_unit(&mut f, &t, &ct, u, (0, 0), 1, 0, 0);
    assert_eq!(n, 1);
    let (k, n) = helpers3::next_unit(&mut f, &t, &ct, u, (0, 0), 0, 0, 0);
    assert_eq!((k, n), (None, 0));
}

// ---------------------------------------------------------------- §2.22

// Covers: specs/skills/bodies-2.md §2.22
#[test]
fn pack_alignment_and_target_list_helpers() {
    let (mut f, _u) = world();
    let leader = monster(&mut f, (2, 2));
    let minion = monster(&mut f, (3, 3));
    // T's owner is a monster: another unit → leave the leader's list.
    f.minion_owner.insert(minion, leader);
    f.take_log();
    helpers3::leave_pack(&mut f, minion);
    assert_eq!(f.take_log(), [format!("LeaveLeader({minion})")]);
    // T itself is its pack owner → the pack is dissolved.
    f.minion_owner.insert(leader, leader);
    helpers3::leave_pack(&mut f, leader);
    assert_eq!(f.take_log(), [format!("DissolvePack({leader})")]);
    // Owner not a monster / no owner: nothing.
    f.minion_owner.insert(minion, 0);
    helpers3::leave_pack(&mut f, minion);
    f.minion_owner.remove(&minion);
    helpers3::leave_pack(&mut f, minion);
    assert!(f.take_log().is_empty());
    // Alignment: the callbacks set 0 (evil) and unlink the target list.
    remove_alignment(&mut f, minion, 53);
    let log = f.take_log();
    assert!(log.contains(&format!("Alignment {{ u: {minion}, a: 0, v: 1 }}")));
    assert!(log.contains(&format!("NodeRemove({minion})")));
    // Target-list prepend only for a player / monster in no list.
    node_prepend(&mut f, minion, 8);
    assert_eq!(
        f.take_log(),
        [format!("NodePrepend {{ u: {minion}, slot: 8 }}")]
    );
    f.node.insert(minion, 8);
    node_prepend(&mut f, minion, 9);
    let item = f.add(FUnit::new(UnitType::Item, 0), (0, 0));
    node_prepend(&mut f, item, 9);
    assert!(f.take_log().is_empty());
}

// ---------------------------------------------------------------- §2.24

// Covers: specs/skills/bodies-2.md §2.24
#[test]
fn frenzy_charge_stacks_to_the_level() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.aurastate = 70;
    r.auralencalc = c.f(100);
    let t = tabs(r, c, 1);
    let (mut f, u) = world();
    let e = f.c.units[u].used.unwrap();
    f.c.frame = 500;
    // E param 1 = 0 (the last swing missed): nothing.
    assert_eq!(frenzy_charge(&mut f, &t, u, 1, 3), 0);
    assert!(f.lists.is_empty());
    f.set_entry_param_of(u, &e, 1, 1);
    assert_eq!(frenzy_charge(&mut f, &t, u, 1, 2), 1);
    let l = f.state_list(u, 70).expect("list");
    let li = &f.lists[l];
    assert_eq!((li.flags, li.expire, li.owner), (2, 600, Some(u)));
    assert_eq!(li.callback, callback::DEFAULT);
    assert_eq!(f.list_get(l, 169), 1);
    let log = f.take_log();
    assert!(log.contains(&format!("timer {u} 12 600")), "{log:?}");
    assert!(log.contains(&format!("changed {u} 70")));
    f.c.frame = 510;
    frenzy_charge(&mut f, &t, u, 1, 2);
    frenzy_charge(&mut f, &t, u, 1, 2);
    assert_eq!(f.list_get(l, 169), 2, "min(count + 1, L)");
    assert_eq!(f.lists[l].expire, 610);
    assert_eq!(f.lists.len(), 1, "the same list is reused");
    // Invalid skill: 0.
    assert_eq!(frenzy_charge(&mut f, &t, u, 99, 2), 0);
}

// Covers: specs/skills/bodies-2.md §2.24
#[test]
fn frenzy_swing_hit_and_miss() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(30);
    r.hitflags = 0x10;
    r.hitclass = 3;
    let t = tabs(r, c, 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u, m) = hit_world();
    let e = f.c.units[u].used.unwrap();
    f.c.units[u].seed = hit_seed(&f, &t, &ct, u, m);
    f.c.units[u].mode = 0; // keeps the stored entry after apply_melee
    assert_eq!(frenzy_swing(&mut f, &t, &ct, u, Some(m), 1, 1), 1);
    let rec = stored_record(&f, u, m).expect("entry");
    assert_eq!(
        (rec.enh_pct, rec.hit_flags & 0x10, rec.hit_class),
        (30, 0x10, 3)
    );
    assert_eq!(f.entry_param(u, &e, 1), 1, "E param 1 := 1 on a hit");
    assert!(f
        .take_log()
        .iter()
        .any(|s| s == &format!("path {u} TargetUnit(Some({m}))")));
    // A miss: param 1 := 0.
    let (mut f, u, m) = hit_world();
    f.c.in_range = false;
    f.set_entry_param_of(u, &e, 1, 1);
    assert_eq!(frenzy_swing(&mut f, &t, &ct, u, Some(m), 1, 1), 1);
    assert_eq!(f.entry_param(u, &e, 1), 0);
    // T none or R invalid: 0.
    assert_eq!(frenzy_swing(&mut f, &t, &ct, u, None, 1, 1), 0);
    assert_eq!(frenzy_swing(&mut f, &t, &ct, u, Some(m), 99, 1), 0);
}

// ---------------------------------------------------------------- §2.26

// Covers: specs/skills/bodies-2.md §2.26 text, §2.26 r1, §2.26 r2, §2.26 r3, §2.26 r4
#[test]
fn blade_pulse_hits_every_unit_in_range() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.aurarangecalc = c.f(6);
    r.mindam = 4;
    r.maxdam = 4;
    r.hitshift = 8;
    r.hitflags = 0x10;
    r.hitclass = 5;
    r.resultflags = 2 | 1; // bit 1 set: no roll, always the flags
    r.aurafilter = 0x8783;
    let t = tabs(r, c, 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u) = world();
    f.c.hostile = true;
    f.c.in_range = true;
    let near = monster(&mut f, (2, 0));
    let far = monster(&mut f, (9, 0));
    for x in [near, far] {
        f.c.units[x].flags = 0xC;
    }
    f.c.units[u].mode = 0;
    f.scan = vec![near, far];
    f.take_log();
    assert_eq!(blade_pulse(&mut f, &t, &ct, u, 1, 1), 1);
    let rec = stored_record(&f, u, near).expect("the near unit is hit");
    assert_eq!(rec.hit_flags & 0x12, 0x12, "hit flags 2 | HitFlags");
    assert_eq!(rec.hit_class, 5);
    assert_eq!(rec.physical, 4 << 8, "roll_physical");
    assert_eq!(rec.result & 3, 3);
    assert!(
        stored_record(&f, u, far).is_none(),
        "r = eval(aurarangecalc)"
    );
    // Invalid skill: 0.
    assert_eq!(blade_pulse(&mut f, &t, &ct, u, 99, 1), 0);
}

// ---------------------------------------------------------------- §3

// Covers: specs/skills/bodies-2.md §3.1 text, §3.1 r1, §3.1 r2, §3.1 r3, §3.1 r4, §3.1 r5
#[test]
fn jab_rolls_on_a_hit_and_stores_the_entry() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(25);
    r.etype = 1;
    r.calc4 = c.f(50);
    r.emin = 7;
    r.emax = 7;
    r.hitshift = 8;
    let t = tabs(r, c, 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u, m) = hit_world();
    f.c.units[u].seed = hit_seed(&f, &t, &ct, u, m);
    f.c.units[u].mode = 0; // keeps the stored entry after apply_melee
    assert_eq!(b3_lvl01::jab(&mut f, &t, &ct, u, 1, 1), 1);
    let rec = stored_record(&f, u, m).expect("start_combat stored");
    assert_eq!(rec.result & 1, 1);
    assert_eq!(rec.enh_pct, 25, "enhanced damage % := eval(calc1)");
    assert_eq!((rec.conv_pct, rec.conv_elem), (50, 1), "EType conversion");
    // A miss: no rolls, nothing stored for the apply.
    let (mut f, u, _m) = hit_world();
    f.c.in_range = false;
    let seed0 = f.c.units[u].seed;
    assert_eq!(b3_lvl01::jab(&mut f, &t, &ct, u, 1, 1), 1);
    assert_eq!(f.c.units[u].seed, seed0, "no roll_elemental on a miss");
    // R invalid / T none: 0.
    assert_eq!(b3_lvl01::jab(&mut f, &t, &ct, u, 99, 1), 0);
    f.targets.clear();
    assert_eq!(b3_lvl01::jab(&mut f, &t, &ct, u, 1, 1), 0);
}

// Covers: specs/skills/bodies-2.md §3.2 text, §3.2 r1, §3.2 r2, §3.2 r3, §3.2 r4, §3.2 r5, §3.2 r6
#[test]
fn charged_bolt_makes_n_jittered_missiles() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(3);
    r.srvmissilea = 1;
    let t = tabs(r, c, 2);
    let (mut f, u) = world();
    f.tpos.insert(u, (12, 7));
    assert_eq!(b3_lvl01::charged_bolt(&mut f, &t, u, 1, 4), 1);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40, "unit flags |= 0x40");
    assert_eq!(f.missiles.len(), 3);
    for (i, q) in f.missiles.iter().enumerate() {
        assert_eq!(q.flags, 0x21);
        assert_eq!((q.owner, q.class, q.skill, q.level), (u, 1, 1, 4));
        assert_eq!((q.x, q.y), (0, 0), "the unit's position");
        assert_eq!((q.target_x, q.target_y), (12, 7));
        assert_eq!(q.init, Some((init_cb::JITTER, i as u32)));
    }
    // A zero coordinate: that i makes nothing; still returns 1.
    f.missiles.clear();
    f.tpos.insert(u, (12, 0));
    assert_eq!(b3_lvl01::charged_bolt(&mut f, &t, u, 1, 4), 1);
    assert!(f.missiles.is_empty());
    // n ≤ 0: returns 1, nothing.
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(0);
    r.srvmissilea = 1;
    let t0 = tabs(r, c, 2);
    f.tpos.insert(u, (12, 7));
    assert_eq!(b3_lvl01::charged_bolt(&mut f, &t0, u, 1, 4), 1);
    assert!(f.missiles.is_empty());
    // Missile out of range (≥ count): 0; the flag was set before.
    f.c.units[u].flags = 0;
    let mut r = body_rec();
    r.srvmissilea = 5;
    let t5 = tabs(r, Code::new(), 2);
    assert_eq!(b3_lvl01::charged_bolt(&mut f, &t5, u, 1, 4), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40, "flag set before the test");
    // R invalid: 0.
    assert_eq!(b3_lvl01::charged_bolt(&mut f, &t, u, 99, 4), 0);
}

// Covers: specs/skills/bodies-2.md §3.6 text, §3.6 r1, §3.6 r2, §3.6 r3, §3.6 r4, §3.6 r5
#[test]
fn raven_spawns_without_a_target_node() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.summon = 0;
    r.summode = 1;
    r.pettype = 3;
    r.petmax = c.f(4);
    r.calc2 = c.f(2);
    let t = tabs(r, c, 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u) = world();
    f.monlvl = vec![crate::skills::fake::blank::<d2_data::tables::Monlvl>(); 10];
    for row in &mut f.monlvl {
        row.ac = 7;
        row.th = 11;
    }
    f.c.units[u].flags = 0;
    assert_eq!(b3_lvl01::raven(&mut f, &t, &ct, u, 1, 3), 1);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    let log = f.take_log();
    assert!(log.iter().any(|s| s.starts_with("monster 1 (0, 0) 0 1 -1")));
    let m = 1; // the spawned monster
    assert!(log.contains(&format!("set {m} 74 0")), "hpregen := 0");
    assert!(log.contains(&format!("set {m} 12 2")), "base_stats level");
    assert!(log.contains(&format!("add {m} 31 7")));
    assert!(log.contains(&format!("add {m} 19 11")));
    assert!(log
        .iter()
        .any(|s| s.starts_with("PetAdd") && s.contains("t: 3, max: 4")));
    assert!(!log.iter().any(|s| s.starts_with("NodeInsert")), "no node");
    assert_eq!(f.c.units[m].flags & 0x4, 0, "m flags &= ~0x4");
    // pettype out of range: 0, no flag; spawn failure: 0 with the flag.
    f.c.units[u].flags = 0;
    f.pettypes = 3;
    assert_eq!(b3_lvl01::raven(&mut f, &t, &ct, u, 1, 3), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0);
    f.pettypes = 15;
    f.no_monsters = true;
    assert_eq!(b3_lvl01::raven(&mut f, &t, &ct, u, 1, 3), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    // R invalid: 0.
    assert_eq!(b3_lvl01::raven(&mut f, &t, &ct, u, 99, 3), 0);
}

// Covers: specs/skills/bodies-2.md §3.7 r1, §3.7 r2, §3.7 r3, §3.7 r4
#[test]
fn firestorm_is_a_fan_with_one_straight_missile() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(4);
    r.srvmissilea = 1;
    let t = tabs(r, c, 2);
    let (mut f, u) = world();
    f.tpos.insert(u, (12, 7));
    assert_eq!(b3_lvl01::firestorm(&mut f, &t, u, 1, 5), 1);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    let log = f.take_log();
    let _ = log;
    assert_eq!(f.missiles.len(), 4, "1 straight + (n − 1) jittered");
    assert_eq!(f.missiles[0].init, None, "the straight skill_missile");
    for (i, q) in f.missiles[1..].iter().enumerate() {
        assert_eq!(q.init, Some((init_cb::JITTER, i as u32)), "argument 0…n−2");
        assert_eq!((q.target_x, q.target_y), (12, 7));
    }
    // n ≤ 0: 0 (after the flag); an invalid missile: 0 (before the flag).
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(0);
    r.srvmissilea = 1;
    let t0 = tabs(r, c, 2);
    assert_eq!(b3_lvl01::firestorm(&mut f, &t0, u, 1, 5), 0);
    f.c.units[u].flags = 0;
    let mut r = body_rec();
    r.srvmissilea = 9;
    let t9 = tabs(r, Code::new(), 2);
    assert_eq!(b3_lvl01::firestorm(&mut f, &t9, u, 1, 5), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0);
    assert_eq!(b3_lvl01::firestorm(&mut f, &t, u, 99, 5), 0);
}

// Covers: specs/skills/bodies-2.md §3.8
#[test]
fn psychic_hammer_start_tests_target_and_towns() {
    let (mut f, u) = world();
    assert_eq!(b3_lvl01::psychic_hammer_start(&mut f, u), 0, "T none");
    let m = monster(&mut f, (3, 3));
    f.targets.insert(u, m);
    assert_eq!(b3_lvl01::psychic_hammer_start(&mut f, u), 1);
    // T's room in town → 0; the unit's room in town → 0.
    f.room_of.insert(m, 2);
    f.town.insert(2);
    assert_eq!(b3_lvl01::psychic_hammer_start(&mut f, u), 0);
    f.room_of.insert(m, 1);
    f.town.clear();
    f.town.insert(1);
    f.room_of.insert(m, 2);
    assert_eq!(b3_lvl01::psychic_hammer_start(&mut f, u), 0);
    f.town.clear();
    // A target that is neither player nor monster: 0.
    let o = f.add(FUnit::new(UnitType::Object, 0), (4, 4));
    f.targets.insert(u, o);
    assert_eq!(b3_lvl01::psychic_hammer_start(&mut f, u), 0);
    let p = f.add(FUnit::new(UnitType::Player, 0), (5, 5));
    f.targets.insert(u, p);
    assert_eq!(b3_lvl01::psychic_hammer_start(&mut f, u), 1);
}

// Covers: specs/skills/bodies-2.md §3.9 text, §3.9 r1, §3.9 r2, §3.9 r3, §3.9 r4, §3.9 r5, §3.9 r6, §3.9 r7
#[test]
fn psychic_hammer_applies_then_flags_the_reaction() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.mindam = 1;
    r.maxdam = 1;
    r.hitshift = 8;
    r.calc1 = c.f(100);
    let t = tabs(r, c, 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u) = world();
    let m = monster(&mut f, (3, 3));
    f.targets.insert(u, m);
    f.c.hostile = true;
    f.c.set(m, 6, 10_000);
    let seed0 = f.c.units[u].seed;
    assert_eq!(b3_lvl01::psychic_hammer(&mut f, &t, &ct, u, 1, 1), 1);
    // k = calc1 = 100 > 0: one draw, r < 100 → knockback; life ≥ 256 →
    // no will-die; result |= 1.
    let (a, d, res) = *f.c.reactions.last().expect("reaction");
    assert_eq!((a, d, res), (u, m, 1 | 8));
    assert!(f.c.get(m, 6) < 10_000, "the damage was applied first");
    // roll_physical (1 draw) + the knock draw.
    assert_ne!(f.c.units[u].seed, seed0);
    // Life below 256 after the apply: will die (|= 2).
    f.c.set(m, 6, 300);
    b3_lvl01::psychic_hammer(&mut f, &t, &ct, u, 1, 1);
    let (_, _, res) = *f.c.reactions.last().unwrap();
    assert_eq!(res & 2, 2, "life < 256 → will die");
    // k ≤ 0: no knock draw, no knockback bit.
    let mut r = body_rec();
    r.mindam = 1;
    r.maxdam = 1;
    r.hitshift = 8;
    let t0 = tabs(r, Code::new(), 1);
    f.c.set(m, 6, 10_000);
    b3_lvl01::psychic_hammer(&mut f, &t0, &ct, u, 1, 1);
    let (_, _, res) = *f.c.reactions.last().unwrap();
    assert_eq!(res & 8, 0);
    // The start test failing (T none): 0, nothing applied.
    f.targets.clear();
    let n = f.c.reactions.len();
    assert_eq!(b3_lvl01::psychic_hammer(&mut f, &t, &ct, u, 1, 1), 0);
    assert_eq!(f.c.reactions.len(), n);
    assert_eq!(b3_lvl01::psychic_hammer(&mut f, &t, &ct, u, 99, 1), 0);
}

// Covers: specs/skills/bodies-2.md §3.11 text, §3.11 r1, §3.11 r2, §3.11 r3, §3.11 r4, §3.11 r5, §3.11 r6
#[test]
fn dragon_talon_do_counts_down_the_kicks() {
    let mut r = body_rec();
    r.param1 = 5;
    r.param2 = 7;
    let t = tabs(r, Code::new(), 1);
    let ct = combat_tables(vec![monster_rec()]);
    let (mut f, u, m) = hit_world();
    let e = f.c.units[u].used.unwrap();
    f.c.units[u].mode = 0; // keeps the kick's entry after apply_melee
    f.c.units[u].seed = hit_seed(&f, &t, &ct, u, m);
    f.set_entry_param_of(u, &e, 1, 3);
    f.c.units[u].flags = 0x40;
    f.take_log();
    assert_eq!(b3_lvl01::dragon_talon(&mut f, &t, &ct, u, 1, 1), 1);
    // n = 3 − 1 = 2 > 0: stored, flag 0x40 cleared, rewind(100), no knock.
    assert_eq!(f.entry_param(u, &e, 1), 2);
    assert_eq!(f.c.units[u].flags & 0x40, 0);
    assert!(f.take_log().contains(&format!("rewind {u} 100")));
    let rec = stored_record(&f, u, m).expect("kick entry");
    assert_eq!((rec.result & 8, rec.hit_class), (0, 1), "no knockback yet");
    // The last kick: ordinary target → chance 100 (no formula) → knock.
    f.c.units[u].combat.clear();
    f.set_entry_param_of(u, &e, 1, 1);
    f.c.units[u].seed = hit_seed(&f, &t, &ct, u, m);
    assert_eq!(b3_lvl01::dragon_talon(&mut f, &t, &ct, u, 1, 1), 1);
    assert_eq!(f.entry_param(u, &e, 1), 0);
    let rec = stored_record(&f, u, m).expect("kick entry");
    assert_eq!(rec.result & 0xC, 0xC, "get-hit and knockback");
    assert!(!f.take_log().contains(&format!("rewind {u} 100")));
    // A unique target with calc2 = 0: k < 1, no draw, no knock.
    f.c.units[u].combat.clear();
    f.c.units[m].flags = 8;
    f.set_entry_param_of(u, &e, 1, 1);
    f.c.units[u].seed = hit_seed(&f, &t, &ct, u, m);
    b3_lvl01::dragon_talon(&mut f, &t, &ct, u, 1, 1);
    let rec = stored_record(&f, u, m).expect("kick entry");
    assert_eq!(rec.result & 8, 0);
    // Param 1 − 1 < 0: 0. T none / R invalid: 0.
    f.set_entry_param_of(u, &e, 1, 0);
    assert_eq!(b3_lvl01::dragon_talon(&mut f, &t, &ct, u, 1, 1), 0);
    assert_eq!(b3_lvl01::dragon_talon(&mut f, &t, &ct, u, 99, 1), 0);
    f.targets.clear();
    assert_eq!(b3_lvl01::dragon_talon(&mut f, &t, &ct, u, 1, 1), 0);
}

// ---------------------------------------------------------------- §2.1

/// A monster (class 0) with a base list (flag 1) holding a stale stat, and
/// a one-row monlvl; returns the world, the monster, the list and tables.
fn mode_world(
    edit: impl FnOnce(&mut d2_data::tables::Monstats),
) -> (BodyFake, usize, usize, SkillTables, CombatTables) {
    let mut ms = monster_rec();
    ms.noratio = true;
    ms.a1mind = 10;
    ms.a1maxd = 20;
    ms.a1th = 100;
    ms.a2mind = 30;
    ms.a2maxd = 40;
    ms.a2th = 110;
    ms.s1mind = 50;
    ms.s1maxd = 60;
    ms.s1th = 120;
    edit(&mut ms);
    let ct = combat_tables(vec![ms]);
    let t = tabs(body_rec(), Code::new(), 1);
    let (mut f, _u) = world();
    f.monlvl = vec![crate::skills::fake::blank::<d2_data::tables::Monlvl>(); 1];
    let m = monster(&mut f, (4, 4));
    let l = give_list(&mut f, m, 0, &[(99, 1)]);
    f.lists[l].flags = 1;
    (f, m, l, t, ct)
}

fn stats3(f: &BodyFake, l: usize) -> (i32, i32, i32) {
    (f.list_get(l, 21), f.list_get(l, 22), f.list_get(l, 19))
}

// Covers: specs/skills/bodies-2.md §2.1 text, §2.1 r1, §2.1 r2, §2.1 r3, §2.1 r4, §2.1 r5, §2.1 r6, §2.1 r7
#[test]
fn mode_damage_rewrites_the_base_stats() {
    let (mut f, m, l, t, ct) = mode_world(|_| {});
    // Step 3: all stats of the base list are removed first; mode 5 → A2,
    // 7 / 8 → S1, other (4, and 6 block in 1.14d) → A1; noRatio columns.
    mode_damage(&mut f, &t, &ct, m, 5);
    assert_eq!(f.list_get(l, 99), 0, "stale stats removed");
    assert_eq!(stats3(&f, l), (30, 40, 110));
    mode_damage(&mut f, &t, &ct, m, 7);
    assert_eq!(stats3(&f, l), (50, 60, 120));
    mode_damage(&mut f, &t, &ct, m, 8);
    assert_eq!(stats3(&f, l), (50, 60, 120));
    mode_damage(&mut f, &t, &ct, m, 4);
    assert_eq!(stats3(&f, l), (10, 20, 100));
    mode_damage(&mut f, &t, &ct, m, 6);
    assert_eq!(stats3(&f, l), (10, 20, 100), "block takes the A1 row");
    // Step 1: not a monster / no list / no record: nothing.
    let (mut f2, m2, l2, t2, ct2) = mode_world(|_| {});
    f2.lists[l2].flags = 2;
    mode_damage(&mut f2, &t2, &ct2, m2, 4);
    assert_eq!(f2.list_get(l2, 99), 1, "no base list: untouched");
    f2.lists[l2].flags = 1;
    mode_damage(&mut f2, &t2, &ct2, 0, 4);
    assert_eq!(f2.list_get(l2, 99), 1, "a player is not a monster");
    let ct_none = combat_tables(vec![]);
    mode_damage(&mut f2, &t2, &ct_none, m2, 4);
    assert_eq!(f2.list_get(l2, 99), 1, "no monstats record");
    // Without noRatio: pct(monlvl DM, value, 100) and TH likewise.
    let (mut f, m, l, t, ct) = mode_world(|ms| ms.noratio = false);
    f.monlvl[0].dm = 150;
    f.monlvl[0].th = 200;
    mode_damage(&mut f, &t, &ct, m, 4);
    assert_eq!(stats3(&f, l), (15, 30, 200));
    // The L-flag picks the L-DM / L-TH columns.
    f.l_flag = true;
    f.monlvl[0].l_dm = 300;
    f.monlvl[0].l_th = 50;
    mode_damage(&mut f, &t, &ct, m, 4);
    assert_eq!(stats3(&f, l), (30, 60, 50));
}

// Covers: specs/skills/bodies-2.md §2.1 r5, §2.1 r7
#[test]
fn mode_damage_difficulty_and_player_count() {
    // Classic (not expansion), d = 1, Align ≠ 1: 10/12 and 10/15.
    let (mut f, m, l, t, ct) = mode_world(|ms| {
        ms.a1mind_n = 120;
        ms.a1maxd_n = 240;
        ms.a1th_n = 150;
    });
    f.c.difficulty = 1;
    mode_damage(&mut f, &t, &ct, m, 4);
    assert_eq!(stats3(&f, l), (100, 200, 100));
    // Align = 1 (neutral) is exempt.
    let (mut f, m, l, t, ct) = mode_world(|ms| {
        ms.a1mind_n = 120;
        ms.a1maxd_n = 240;
        ms.a1th_n = 150;
        ms.align = 1;
    });
    f.c.difficulty = 1;
    mode_damage(&mut f, &t, &ct, m, 4);
    assert_eq!(stats3(&f, l), (120, 240, 150));
    // Expansion, 3 players on Nightmare: mult = 16, v += trunc(v·16/128).
    let (mut f, m, l, t, ct) = mode_world(|ms| {
        ms.a1mind_n = 80;
        ms.a1maxd_n = 130;
        ms.a1th_n = 100;
    });
    f.c.difficulty = 1;
    f.c.expansion = true;
    f.c.set(m, 100, 3);
    mode_damage(&mut f, &t, &ct, m, 4);
    assert_eq!(stats3(&f, l), (90, 146, 112));
    // Normal difficulty: no multiplier whatever the count.
    f.c.difficulty = 0;
    f.c.set(m, 100, 5);
    mode_damage(&mut f, &t, &ct, m, 4);
    assert_eq!(stats3(&f, l), (10, 20, 100));
}

// Covers: specs/skills/bodies-2.md §2.1 r8, §2.1 r9
#[test]
fn mode_damage_element_slots() {
    // Slot 1 cold on mode 5 (El1 columns, scaled by the monlvl DM), slot 2
    // poison on mode 5 reading the El1 columns too (Edge case 1), slot 3
    // on another mode: skipped.
    let (mut f, m, l, t, ct) = mode_world(|ms| {
        ms.noratio = false;
        ms.el1mode = 5;
        ms.el1type = 4;
        ms.el1pct = 100;
        ms.el1mind = 4;
        ms.el1maxd = 8;
        ms.el1dur = 30;
        ms.el2mode = 5;
        ms.el2type = 5;
        ms.el2pct = 100;
        ms.el3mode = 4;
        ms.el3type = 1;
        ms.el3pct = 100;
    });
    f.monlvl[0].dm = 150;
    f.monlvl[0].th = 100;
    mode_damage(&mut f, &t, &ct, m, 5);
    assert_eq!(
        (f.list_get(l, 54), f.list_get(l, 55), f.list_get(l, 56)),
        (6, 12, 30)
    );
    assert_eq!(
        (f.list_get(l, 57), f.list_get(l, 58), f.list_get(l, 59)),
        (60, 120, 60),
        "poison: 10·min, 10·max, 2·len"
    );
    assert_eq!(f.list_get(l, 48), 0, "slot 3 has another mode");
    // Expansion with mult ≠ 0: min, max, len each += trunc(v·mult/128).
    f.c.expansion = true;
    f.c.difficulty = 1;
    f.c.set(m, 100, 3);
    f.lists[l].stats.clear();
    f.monlvl[0].dm_n = 100;
    let mut ct2 = ct.clone();
    ct2.monstats[0].el1pct_n = 100;
    ct2.monstats[0].el2pct_n = 100;
    ct2.monstats[0].el1mind_n = 64;
    ct2.monstats[0].el1maxd_n = 128;
    ct2.monstats[0].el1dur_n = 64;
    mode_damage(&mut f, &t, &ct2, m, 5);
    assert_eq!(
        (f.list_get(l, 54), f.list_get(l, 55), f.list_get(l, 56)),
        (72, 144, 72)
    );
    // noRatio: min = max = 0 and len = El1Dur (fire has no length).
    let (mut f, m, l, t, ct) = mode_world(|ms| {
        ms.el1mode = 5;
        ms.el1type = 11;
        ms.el1pct = 100;
        ms.el1mind = 4;
        ms.el1maxd = 8;
        ms.el1dur = 30;
    });
    mode_damage(&mut f, &t, &ct, m, 5);
    assert_eq!(
        (f.list_get(l, 316), f.list_get(l, 317), f.list_get(l, 315)),
        (0, 0, 30)
    );
    // The chance: p < 100 draws once on the unit's seed; r ≥ p skips.
    let (mut f, m, l, t, ct) = mode_world(|ms| {
        ms.el1mode = 5;
        ms.el1type = 11;
        ms.el1pct = 50;
        ms.el1dur = 30;
    });
    f.c.units[m].seed = seed_with(0..50);
    mode_damage(&mut f, &t, &ct, m, 5);
    assert_eq!(f.list_get(l, 315), 30, "r < p: applied");
    f.lists[l].stats.clear();
    f.c.units[m].seed = seed_with(50..100);
    let before = f.c.units[m].seed;
    mode_damage(&mut f, &t, &ct, m, 5);
    assert_eq!(f.list_get(l, 315), 0, "r ≥ p: skipped");
    assert_eq!(f.c.units[m].seed, stepped(before), "exactly one draw");
    // Type 10 (random): one more draw, t = roll(5) + 1; len 0 → 25.
    let (mut f, m, l, t, ct) = mode_world(|ms| {
        ms.el1mode = 5;
        ms.el1type = 10;
        ms.el1pct = 100;
        ms.el1dur = 0;
    });
    let mut s = f.c.units[m].seed;
    let ty = s.roll(5) as i32 + 1;
    mode_damage(&mut f, &t, &ct, m, 5);
    assert_eq!(f.c.units[m].seed, s, "one draw (pct 100 draws none)");
    if ty == 4 {
        assert_eq!(f.list_get(l, 56), 25);
    }
    if ty == 5 {
        assert_eq!(f.list_get(l, 59), 50);
    }
    if ty == 9 {
        assert_eq!(f.list_get(l, 66), 25);
    }
}

// Covers: specs/skills/bodies-2.md §3.1 r2
#[test]
fn jab_of_a_monster_sets_the_s1_damage() {
    let t = tabs(body_rec(), Code::new(), 1);
    let (mut f, m, l, _t, ct) = mode_world(|_| {});
    let p = 0; // the player of the world
    f.targets.insert(m, p);
    f.c.set(m, 12, 1);
    f.c.set(p, 12, 1);
    f.c.hostile = true;
    f.c.in_range = true;
    assert_eq!(b3_lvl01::jab(&mut f, &t, &ct, m, 1, 1), 1);
    assert_eq!(stats3(&f, l), (50, 60, 120), "mode_damage(unit, 8)");
}
