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
