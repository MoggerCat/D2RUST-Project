// Spec: specs/combat/damage.md, specs/combat/hit.md
// Tests written against surviving mutants of `cargo mutants` (METHODS
// M08): each asserts what the spec says at the boundary or branch the
// mutant changed. Values come from the spec's formulas; the fake world
// is `skills::fake`.
use super::*;
use crate::rng::Seed;
use crate::skills::fake::*;
use crate::skills::SkillTables;
use crate::units::UnitType;

fn st() -> SkillTables {
    skill_tables(vec![skill_rec()])
}

fn ct() -> CombatTables {
    combat_tables(vec![monster_rec()])
}

fn world() -> Fake {
    Fake {
        hostile: true,
        in_range: true,
        expansion: true,
        ..Fake::default()
    }
}

/// `seed` after `n` steps.
fn stepped(seed: Seed, n: usize) -> Seed {
    let mut s = seed;
    for _ in 0..n {
        s.step();
    }
    s
}

/// A seed whose next step gives `lo′ = x`.
fn seed_giving(x: u32) -> Seed {
    Seed::new(0, x)
}

// ---------------------------------------------------------------- §0

// damage.md §0: the thresholds are strict (`v > 0x100000`, `p > 0x10000`)
// and case 3 compares `d` with `p >> 4`.
#[test]
fn pct_thresholds() {
    // v = 0x100000 is not case 2: case 4, the 32-bit product wraps to 0.
    assert_eq!(pct(0x10_0000, 0x1000, 3), 0);
    // p = 0x10000 is not case 3: case 4 again.
    assert_eq!(pct(0x1_0000, 0x1_0000, 3), 0);
    // p > 0x10000 and d > p >> 4 (0x2000): the 64-bit product.
    // 7 × 0x20000 / 0x3000 = 74 (the shortcut would give 10 × 7 = 70).
    assert_eq!(pct(7, 0x2_0000, 0x3000), 74);
    // d ≤ p >> 4: the shortcut (p / d) × v.
    assert_eq!(pct(7, 0x2_0000, 0x2000), 112);
}

// ---------------------------------------------------------------- §3.3

// `roll_in_range`: nothing is added and nothing drawn unless `max > 0`.
#[test]
fn roll_in_range_needs_positive_max() {
    let mut f = world();
    let u = f.add(FUnit::new(UnitType::Player, 0));
    let before = f.units[u].seed;
    assert_eq!(roll_in_range(&mut f, u, -512, 0, 0, 0, 1000), 1000);
    assert_eq!(f.units[u].seed, before);
}

// ---------------------------------------------------------------- §3.2

// Step 2: `min < 1` → 256, so a given min of 1 stays 1 (and `max ≤ min`
// → `max = min + 256`).
#[test]
fn bonuses_min_one_is_kept() {
    let s = st();
    let mut f = world();
    let u = f.add(FUnit::new(UnitType::Player, 0));
    // lo′ = 0 on every step: roll(256) = 0.
    f.units[u].seed = Seed::new(0, 0);
    assert_eq!(bonuses(&mut f, &s, u, false, None, 1, 1, 0, 0, 128), 1);
    assert_eq!(bonuses(&mut f, &s, u, false, None, 0, 1, 0, 0, 128), 256);
}

// ---------------------------------------------------------------- §3.1

/// A player with a weapon of item type 50 (min 1, max 3) against a
/// monster.
fn armed_case(f: &mut Fake) -> (usize, usize) {
    let a = f.add(FUnit::new(UnitType::Player, 0).with(21, 1).with(22, 3));
    let w = f.add_item(FItem {
        types: vec![50],
        ..FItem::default()
    });
    f.units[a].weapon = Some(w);
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(6, 100_000));
    (a, d)
}

/// A seed whose second draw (the crit draw after the physical roll)
/// gives `r = lo′ mod 100 ≥ 1`, and that `r`.
fn crit_seed() -> (Seed, i32) {
    (1u32..)
        .map(|lo| {
            let s = Seed::new(lo, 0);
            let mut t = stepped(s, 1);
            (s, (t.step() % 100) as i32)
        })
        .find(|&(_, r)| r >= 1)
        .unwrap()
}

/// Fills `rec` for the armed case with `offhand`, returning whether the
/// result is critical and how many attacker draws were made.
fn crit_fill(f: &mut Fake, a: usize, d: usize, offhand: bool, seed: Seed) -> (bool, usize) {
    let (s, c) = (st(), ct());
    f.units[a].seed = seed;
    let mut rec = DamageRecord::default();
    fill(f, &s, &c, a, d, &mut rec, offhand, 128);
    let crit = rec.result & result::CRITICAL != 0;
    let n = (0..8)
        .find(|&n| stepped(seed, n) == f.units[a].seed)
        .expect("draw count");
    (crit, n)
}

// §3.1 step 4.1: the weapon-mastery crit needs `offhand = 0`, a current
// weapon and `m > 0`; it draws once and crits on `r < m`. Draws: physical
// roll, crit draw (when made), burn roll(1).
#[test]
fn fill_mastery_crit_threshold() {
    let mut f = world();
    let (a, d) = armed_case(&mut f);
    let (seed, r) = crit_seed();
    // m = r + 1: crit.
    f.units[a].entries.insert(344, vec![(50, r + 1)]);
    assert_eq!(crit_fill(&mut f, a, d, false, seed), (true, 3));
    // m = r: no crit (and no other source).
    f.units[a].entries.insert(344, vec![(50, r)]);
    assert_eq!(crit_fill(&mut f, a, d, false, seed), (false, 3));
    // Off-hand: no mastery draw.
    f.units[a].entries.insert(344, vec![(50, r + 1)]);
    assert_eq!(crit_fill(&mut f, a, d, true, seed), (false, 2));
    // m = 0: no draw.
    f.units[a].entries.clear();
    assert_eq!(crit_fill(&mut f, a, d, false, seed), (false, 2));
}

// §3.1 steps 4.2 and 4.3: `passive_critical_strike` then
// `item_deadlystrike`, each `> 0` → draw, `r < value` → crit.
#[test]
fn fill_passive_and_deadly_crit_threshold() {
    let mut f = world();
    let (a, d) = armed_case(&mut f);
    let (seed, r) = crit_seed();
    f.set(a, 337, r + 1);
    assert_eq!(crit_fill(&mut f, a, d, false, seed), (true, 3));
    f.set(a, 337, r);
    assert_eq!(crit_fill(&mut f, a, d, false, seed), (false, 3));
    f.set(a, 337, 0);
    f.set(a, 141, r + 1);
    assert_eq!(crit_fill(&mut f, a, d, false, seed), (true, 3));
    f.set(a, 141, r);
    assert_eq!(crit_fill(&mut f, a, d, false, seed), (false, 3));
}

/// Fills a fresh record with `hit_flags`, returning it.
fn fill_flags(f: &mut Fake, a: usize, d: usize, hit_flags: u32) -> DamageRecord {
    let (s, c) = (st(), ct());
    let mut rec = DamageRecord {
        hit_flags,
        ..DamageRecord::default()
    };
    fill(f, &s, &c, a, d, &mut rec, false, 128);
    rec
}

// §3.1 step 6, monster attacker (not a hireling): each drain is rolled
// unless its preset flag (4 / 8 / 0x10) is set. min = max: no draw.
#[test]
fn fill_monster_drains_and_presets() {
    let mut f = world();
    let a = f.add(
        FUnit::new(UnitType::Monster, 0)
            .with(60, 2)
            .with(61, 2)
            .with(62, 3)
            .with(63, 3)
            .with(64, 4)
            .with(65, 4),
    );
    let d = f.add(FUnit::new(UnitType::Player, 0));
    let r = fill_flags(&mut f, a, d, 0);
    assert_eq!(
        (r.life_leech, r.mana_leech, r.stamina_leech),
        (512, 768, 1024)
    );
    let r = fill_flags(&mut f, a, d, hitflag::LIFE_DRAIN_PRESET);
    assert_eq!(
        (r.life_leech, r.mana_leech, r.stamina_leech),
        (0, 768, 1024)
    );
    let r = fill_flags(&mut f, a, d, hitflag::MANA_DRAIN_PRESET);
    assert_eq!(
        (r.life_leech, r.mana_leech, r.stamina_leech),
        (512, 0, 1024)
    );
    let r = fill_flags(&mut f, a, d, hitflag::STAMINA_DRAIN_PRESET);
    assert_eq!((r.life_leech, r.mana_leech, r.stamina_leech), (512, 768, 0));
}

// §3.1 step 6, player or hireling attacker: plain percents unless preset,
// and the bypass flags; other attackers: nothing.
#[test]
fn fill_player_and_hireling_drains() {
    let mut f = world();
    let mut stats = FUnit::new(UnitType::Player, 0)
        .with(60, 7)
        .with(61, 9)
        .with(62, 5)
        .with(63, 9)
        .with(103, 1)
        .with(104, 1)
        .with(106, 1);
    let p = f.add(stats.clone());
    stats.kind = UnitType::Monster;
    stats.hireling = true;
    let h = f.add(stats.clone());
    stats.kind = UnitType::Object;
    stats.hireling = false;
    let o = f.add(stats);
    let d = f.add(FUnit::new(UnitType::Monster, 0));
    let bypass = hitflag::BYPASS_UNDEAD | hitflag::BYPASS_DEMONS | hitflag::BYPASS_BEASTS;
    for a in [p, h] {
        let r = fill_flags(&mut f, a, d, 0);
        assert_eq!((r.life_leech, r.mana_leech), (7, 5), "{a}");
        assert_eq!(r.hit_flags & bypass, bypass, "{a}");
        let r = fill_flags(&mut f, a, d, hitflag::LIFE_DRAIN_PRESET);
        assert_eq!((r.life_leech, r.mana_leech), (0, 5), "{a}");
        let r = fill_flags(&mut f, a, d, hitflag::MANA_DRAIN_PRESET);
        assert_eq!((r.life_leech, r.mana_leech), (7, 0), "{a}");
    }
    // Only the bypass stats that are set give their flag.
    f.set(p, 104, 0);
    let r = fill_flags(&mut f, p, d, 0);
    assert_eq!(
        r.hit_flags & bypass,
        hitflag::BYPASS_UNDEAD | hitflag::BYPASS_BEASTS
    );
    f.set(p, 103, 0);
    f.set(p, 106, 0);
    f.set(p, 104, 1);
    let r = fill_flags(&mut f, p, d, 0);
    assert_eq!(r.hit_flags & bypass, hitflag::BYPASS_DEMONS);
    // Flags already set are kept.
    f.set(p, 104, 0);
    let r = fill_flags(&mut f, p, d, hitflag::BYPASS_UNDEAD);
    assert_eq!(r.hit_flags & bypass, hitflag::BYPASS_UNDEAD);
    // An object attacker: no drains, no bypass flags.
    let r = fill_flags(&mut f, o, d, 0);
    assert_eq!(
        (r.life_leech, r.mana_leech, r.hit_flags & bypass),
        (0, 0, 0)
    );
}

// §3.1 step 7: `m = passive_pois_mastery` when `max > 0`, applied to both
// ends. min = max = 256, m = 50: 384, no draw.
#[test]
fn fill_poison_mastery() {
    let mut f = world();
    let a = f.add(
        FUnit::new(UnitType::Player, 0)
            .with(57, 256)
            .with(58, 256)
            .with(332, 50),
    );
    let d = f.add(FUnit::new(UnitType::Monster, 0));
    let r = fill_flags(&mut f, a, d, hitflag::SKIP_PHYSICAL);
    assert_eq!(r.poison, 384);
}

// ---------------------------------------------------------------- §5.6

// Step 6: a monster with `monstats2.deadCol` never shatters.
#[test]
fn cold_shatter_needs_no_deadcol() {
    let mut m = monster_rec();
    m.coldeffect = (-30i8) as u8;
    let mut c = combat_tables(vec![m]);
    c.monstats2[0].deadcol = true;
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let mo = f.add(FUnit::new(UnitType::Monster, 0));
    f.units[mo].seed = seed_giving(19);
    cold(&mut f, &c, a, mo, 41);
    assert_eq!(f.log.last().unwrap(), "state 1 107 false");
}

// §3.1 step 11: event 3 only when the result lacks 0x20.
#[test]
fn fill_event_needs_events() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0));
    let (s, c) = (st(), ct());
    let mut rec = DamageRecord {
        result: result::NO_EVENTS,
        ..DamageRecord::default()
    };
    fill(&mut f, &s, &c, a, d, &mut rec, false, 128);
    assert!(f.log.is_empty(), "{:?}", f.log);
    rec.result = 0;
    fill(&mut f, &s, &c, a, d, &mut rec, false, 128);
    assert_eq!(f.log, ["event 3 1 0"]);
}

// §3.1 step 11: a monster attacker sends event 3 only when it is a
// hireling.
#[test]
fn fill_event_monster_needs_hireling() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Monster, 0));
    let d = f.add(FUnit::new(UnitType::Player, 0));
    fill_flags(&mut f, a, d, 0);
    assert!(f.log.is_empty(), "{:?}", f.log);
    f.units[a].hireling = true;
    fill_flags(&mut f, a, d, 0);
    assert_eq!(f.log, ["event 3 1 0"]);
}

/// Fills a player's record with physical 1000 preset (hit flag 1),
/// conversion `elem` at 50 %.
fn convert(f: &mut Fake, a: usize, d: usize, elem: i8) -> DamageRecord {
    let (s, c) = (st(), ct());
    let mut rec = DamageRecord {
        hit_flags: hitflag::SKIP_PHYSICAL,
        physical: 1000,
        conv_elem: elem,
        conv_pct: 50,
        ..DamageRecord::default()
    };
    fill(f, &s, &c, a, d, &mut rec, false, 128);
    rec
}

// §3.1 step 12: conversion only when +0x65 > 0; c = 500 goes to the
// element's field; poison gets c / 8; burn and freeze set their length
// to at least 50.
#[test]
fn fill_conversion_elements() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0));
    let r = convert(&mut f, a, d, 0);
    assert_eq!(r.physical, 1000);
    let r = convert(&mut f, a, d, 1);
    assert_eq!((r.physical, r.fire), (500, 500));
    let r = convert(&mut f, a, d, 2);
    assert_eq!((r.physical, r.lightning), (500, 500));
    let r = convert(&mut f, a, d, 3);
    assert_eq!((r.physical, r.magic), (500, 500));
    let r = convert(&mut f, a, d, 5);
    assert_eq!((r.physical, r.poison, r.poison_len), (500, 62, 50));
    let r = convert(&mut f, a, d, 11);
    assert_eq!((r.physical, r.fire, r.burn_len), (500, 500, 50));
    let r = convert(&mut f, a, d, 12);
    assert_eq!((r.physical, r.cold, r.freeze_len), (500, 500, 50));
    for e in 6..=9 {
        let r = convert(&mut f, a, d, e);
        assert_eq!(
            (r.physical, r.fire, r.lightning, r.magic, r.cold, r.poison),
            (500, 0, 0, 0, 0, 0),
            "{e}"
        );
    }
}

// §3.1 step 12: element 10 is `roll(5) + 1` on the attacker seed, drawn
// after the burn roll(1). roll(5) = 1 → lightning.
#[test]
fn fill_conversion_random_element() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0));
    let seed = (1u32..)
        .map(|lo| Seed::new(lo, 0))
        .find(|&s| stepped(s, 1).step() % 5 == 1)
        .unwrap();
    f.units[a].seed = seed;
    let r = convert(&mut f, a, d, 10);
    assert_eq!((r.fire, r.lightning), (0, 500));
    assert_eq!(f.units[a].seed, stepped(seed, 2));
}

// §3.1 step 13: the monster crit draws only when `Crit ≠ 0` and doubles
// on `r < Crit`. Draws: burn roll(1), then the crit step.
#[test]
fn fill_monster_crit_threshold() {
    let (seed, r) = crit_seed();
    let s = st();
    for (crit, doubled, draws) in [(0, false, 1), (r, false, 2), (r + 1, true, 2)] {
        let mut m = monster_rec();
        m.crit = crit as u8;
        let c = combat_tables(vec![m]);
        let mut f = world();
        let a = f.add(FUnit::new(UnitType::Monster, 0));
        let d = f.add(FUnit::new(UnitType::Player, 0));
        f.units[a].seed = seed;
        let mut rec = DamageRecord {
            hit_flags: hitflag::SKIP_PHYSICAL,
            physical: 1000,
            ..DamageRecord::default()
        };
        fill(&mut f, &s, &c, a, d, &mut rec, false, 128);
        let want = if doubled { 2000 } else { 1000 };
        assert_eq!(rec.physical, want, "crit {crit}");
        assert_eq!(f.units[a].seed, stepped(seed, draws), "crit {crit}");
    }
}

// ---------------------------------------------------------------- §3

/// `start_combat` with a preset record (hit flags `hf`, result `res`,
/// physical `phys`) from `a` on `d`.
fn start(f: &mut Fake, a: usize, d: usize, hf: u32, res: u16, phys: i32) -> DamageRecord {
    let (s, c) = (st(), ct());
    let mut rec = DamageRecord {
        hit_flags: hf,
        result: res,
        physical: phys,
        ..DamageRecord::default()
    };
    start_combat(f, &s, &c, Some(a), Some(d), &mut rec, 128);
    rec
}

// Step 2: no roll when the result has dodge, avoid, evade or weapon
// block (mask 0x8380).
#[test]
fn start_combat_avoided_results() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(6, 1));
    let r = start(&mut f, a, d, 0, result::HIT, 0x1_0000);
    assert_ne!(r.hit_flags & hitflag::ROLLED, 0);
    assert_ne!(r.result & result::WILL_DIE, 0);
    for flag in [
        result::DODGE,
        result::AVOID,
        result::EVADE,
        result::WEAPON_BLOCK,
    ] {
        let r = start(&mut f, a, d, 0, result::HIT | flag, 0x1_0000);
        assert_eq!(r.hit_flags & hitflag::ROLLED, 0, "{flag:#x}");
        assert_eq!(r.result & result::WILL_DIE, 0, "{flag:#x}");
    }
}

// Step 2.3: the comparison drops the low byte of both sides and is
// strict; a monster attacker adds its life leech.
#[test]
fn start_combat_will_die_compare() {
    let mut f = world();
    let p = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(6, 0x1C0));
    let hf = hitflag::SKIP_ROLL;
    // 0x180 & ~0xFF = 0x100 = 0x1C0 & ~0xFF: not greater.
    let r = start(&mut f, p, d, hf, result::HIT, 0x180);
    assert_eq!(r.result & result::WILL_DIE, 0);
    let r = start(&mut f, p, d, hf, result::HIT, 0x200);
    assert_ne!(r.result & result::WILL_DIE, 0);
    // Life leech counts for a monster attacker only.
    let m = f.add(FUnit::new(UnitType::Monster, 0));
    let pd = f.add(FUnit::new(UnitType::Player, 0).with(6, 0x100));
    let (s, c) = (st(), ct());
    for (a, dies) in [(m, true), (p, false)] {
        let mut rec = DamageRecord {
            hit_flags: hf,
            result: result::HIT,
            life_leech: 0x200,
            ..DamageRecord::default()
        };
        start_combat(&mut f, &s, &c, Some(a), Some(pd), &mut rec, 128);
        assert_eq!(rec.result & result::WILL_DIE != 0, dies, "{a}");
    }
}

// ---------------------------------------------------------------- §4

// §4.2: the first matching row wins.
#[test]
fn damage_percent_rows() {
    let c = ct();
    let mut f = world();
    let p = f.add(FUnit::new(UnitType::Player, 0));
    let mut hire = FUnit::new(UnitType::Monster, 0);
    hire.hireling = true;
    let h = f.add(hire);
    let m = f.add(FUnit::new(UnitType::Monster, 0));
    let mut b = FUnit::new(UnitType::Monster, 0);
    b.boss = true;
    let boss = f.add(b);
    assert_eq!(damage_percent(&f, &c, Some(h), p), 17);
    assert_eq!(damage_percent(&f, &c, Some(m), p), 100);
    assert_eq!(damage_percent(&f, &c, Some(h), boss), 50);
    assert_eq!(damage_percent(&f, &c, Some(m), boss), 100);
    assert_eq!(damage_percent(&f, &c, Some(p), boss), 100);
}

/// `totals` of `rec` from `a` on `d`.
fn totals_of(f: &mut Fake, a: usize, d: usize, rec: DamageRecord) -> DamageRecord {
    let c = ct();
    let mut rec = rec;
    totals(f, &c, Some(a), d, &mut rec);
    rec
}

fn fire(v: i32) -> DamageRecord {
    DamageRecord {
        fire: v,
        ..DamageRecord::default()
    }
}

// §4.1: magic damage reduction is pierced only when the pierce percent
// is > 0: 5 << 8 = 1280 × 512 / 1024 = 640.
#[test]
fn totals_magic_dr_pierce() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(35, 5));
    let r = totals_of(
        &mut f,
        a,
        d,
        DamageRecord {
            pierce_pct: 512,
            ..fire(2560)
        },
    );
    assert_eq!(r.fire, 1920);
    let r = totals_of(&mut f, a, d, fire(2560));
    assert_eq!(r.fire, 1280);
}

// §4.5 step 2: a monster at 100 % is not pierced (`r < 100`).
#[test]
fn resist_monster_immunity_not_pierced() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0).with(333, 20));
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(39, 100));
    assert_eq!(totals_of(&mut f, a, d, fire(1000)).fire, 0);
    f.set(d, 39, 99);
    // 99 − 20 = 79: 1000 × 21 / 100.
    assert_eq!(totals_of(&mut f, a, d, fire(1000)).fire, 210);
}

// §4.5 step 3: classic game, difficulty 1 → −20, difficulty 2 → −50.
#[test]
fn resist_classic_penalty() {
    let mut f = world();
    f.expansion = false;
    let a = f.add(FUnit::new(UnitType::Monster, 0));
    let d = f.add(FUnit::new(UnitType::Player, 0).with(39, 60));
    f.difficulty = 1;
    assert_eq!(totals_of(&mut f, a, d, fire(1000)).fire, 600);
    f.difficulty = 2;
    assert_eq!(totals_of(&mut f, a, d, fire(1000)).fire, 900);
}

// §4.5 step 4: the cap applies to `r > 0` only; r = 0 stays 0 even with
// a max stat below −75.
#[test]
fn resist_cap_only_positive() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Monster, 0));
    let d = f.add(FUnit::new(UnitType::Player, 0).with(40, -100));
    assert_eq!(totals_of(&mut f, a, d, fire(1000)).fire, 1000);
}

// §4.5 step 4: sanctuary zeroes the physical resist (stat 36) of an
// undead defender only.
#[test]
fn resist_sanctuary() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    f.units[a].states.push(47);
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(36, 50).with(39, 50));
    let rec = DamageRecord {
        physical: 1000,
        fire: 1000,
        ..DamageRecord::default()
    };
    let r = totals_of(&mut f, a, d, rec);
    assert_eq!((r.physical, r.fire), (500, 500));
    f.units[d].undead = true;
    let r = totals_of(&mut f, a, d, rec);
    assert_eq!((r.physical, r.fire), (1000, 500));
}

// §4.4 step 3: lengths are touched only when > 0.
#[test]
fn totals_length_guards() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(118, 1));
    let len = |cold_len, freeze_len| DamageRecord {
        cold_len,
        freeze_len,
        ..DamageRecord::default()
    };
    // Half freeze: a freeze length alone is enough.
    let r = totals_of(&mut f, a, d, len(0, 10));
    assert_eq!((r.cold_len, r.freeze_len), (0, 5));
    let r = totals_of(&mut f, a, d, len(10, 0));
    assert_eq!((r.cold_len, r.freeze_len), (5, 0));
    // Burn length and state 131.
    let burn = DamageRecord {
        burn_len: 10,
        ..DamageRecord::default()
    };
    assert_eq!(totals_of(&mut f, a, d, burn).burn_len, 10);
    f.units[d].states.push(131);
    assert_eq!(totals_of(&mut f, a, d, burn).burn_len, 0);
}

// §4.4 step 4: `no_absorb` for a monster defender by its kind and the
// matching bypass flag (seen through a 20 % fire absorb).
#[test]
fn totals_no_absorb_matrix() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(142, 20));
    let bu = hitflag::BYPASS_UNDEAD;
    let bd = hitflag::BYPASS_DEMONS;
    let bb = hitflag::BYPASS_BEASTS;
    // (undead, demon, flags, no_absorb)
    for (undead, demon, hf, none) in [
        (true, false, bu, true),
        (true, false, bd | bb, false),
        (false, true, bd, true),
        (false, true, bu | bb, false),
        (false, false, bb, true),
        (false, false, bu | bd, false),
        (true, true, bb, false),
    ] {
        f.units[d].undead = undead;
        f.units[d].demon = demon;
        let r = totals_of(
            &mut f,
            a,
            d,
            DamageRecord {
                hit_flags: hf,
                ..fire(1000)
            },
        );
        let want = if none { (1000, 0) } else { (800, 200) };
        assert_eq!((r.fire, r.absorbed), want, "{undead} {demon} {hf:#x}");
    }
    // A player defender (monster attacker: damage percent 100): only
    // 0x400.
    let m = f.add(FUnit::new(UnitType::Monster, 0));
    let p = f.add(FUnit::new(UnitType::Player, 0).with(142, 20));
    let r = totals_of(
        &mut f,
        m,
        p,
        DamageRecord {
            hit_flags: bu | bd,
            ..fire(1000)
        },
    );
    assert_eq!((r.fire, r.absorbed), (800, 200));
    let r = totals_of(
        &mut f,
        m,
        p,
        DamageRecord {
            hit_flags: bb,
            ..fire(1000)
        },
    );
    assert_eq!((r.fire, r.absorbed), (1000, 0));
}

// §4.6 step 5: the flat absorb needs `f > 0`; a field left negative by
// damage reduction stays negative (step 6).
#[test]
fn totals_negative_field_kept() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(35, 5));
    let r = totals_of(&mut f, a, d, fire(100));
    assert_eq!((r.fire, r.absorbed), (100 - 1280, 0));
}

// ---------------------------------------------------------------- §5.3

/// A leech case: attacker life 0 / max 100000, mana 0 / max 100000.
fn leecher(kind: UnitType) -> FUnit {
    FUnit::new(kind, 0)
        .with(6, 0)
        .with(7, 100_000)
        .with(8, 0)
        .with(9, 100_000)
}

fn leech_rec(life: i32, mana: i32, physical: i32) -> DamageRecord {
    DamageRecord {
        life_leech: life,
        mana_leech: mana,
        physical,
        ..DamageRecord::default()
    }
}

// Step 1 (Nightmare drain column) and step 3 (player divisors):
// 10 << 6 = 640 / 2 = 320; 1000 × 320 % = 3200; drain 50 → 1600; / 64
// = 25.
#[test]
fn leech_nightmare_drain() {
    let mut m = monster_rec();
    m.drain_n = 50;
    let c = combat_tables(vec![m]);
    let mut f = world();
    f.difficulty = 1;
    let a = f.add(leecher(UnitType::Player));
    let d = f.add(FUnit::new(UnitType::Monster, 0));
    let mut rec = leech_rec(10, 0, 1000);
    leech(&mut f, &c, Some(a), d, &mut rec);
    assert_eq!(f.get(a, 6), 25);
}

// Step 3: a hireling takes the player rule without divisors: 640 % of
// 1000 = 6400 / 64 = 100 life (the monster rule would give 10).
#[test]
fn leech_hireling_player_rule() {
    let c = ct();
    let mut f = world();
    f.difficulty = 1;
    let mut h = leecher(UnitType::Monster);
    h.hireling = true;
    let a = f.add(h);
    let d = f.add(FUnit::new(UnitType::Player, 0));
    let mut rec = leech_rec(10, 0, 1000);
    leech(&mut f, &c, Some(a), d, &mut rec);
    assert_eq!(f.get(a, 6), 100);
}

// Step 4: life only → overlay 151, no draw; mana only → overlay 152.
#[test]
fn leech_player_overlays() {
    let c = ct();
    let mut f = world();
    let a = f.add(leecher(UnitType::Player));
    let d = f.add(FUnit::new(UnitType::Player, 0));
    let before = f.units[a].seed;
    leech(&mut f, &c, Some(a), d, &mut leech_rec(10, 0, 1000));
    assert_eq!((f.get(a, 6), f.get(a, 8)), (100, 0));
    assert_eq!(f.log.last().unwrap(), "overlay 0 151");
    assert_eq!(f.units[a].seed, before);
    f.log.clear();
    leech(&mut f, &c, Some(a), d, &mut leech_rec(0, 10, 1000));
    assert_eq!((f.get(a, 6), f.get(a, 8)), (100, 100));
    assert_eq!(f.log.last().unwrap(), "overlay 0 152");
    assert_eq!(f.units[a].seed, before);
}

// Step 5, monster rule: T = 10 is added with overlay 151; at full life
// T = 0 after the cap: nothing.
#[test]
fn leech_monster_rule() {
    let c = ct();
    let mut f = world();
    let a = f.add(leecher(UnitType::Monster).with(6, 100).with(7, 1000));
    let d = f.add(FUnit::new(UnitType::Player, 0));
    leech(&mut f, &c, Some(a), d, &mut leech_rec(10, 0, 1000));
    assert_eq!(f.get(a, 6), 110);
    assert_eq!(f.log, ["set 0 6 110", "overlay 0 151"]);
    f.log.clear();
    f.set(a, 6, 1000);
    leech(&mut f, &c, Some(a), d, &mut leech_rec(10, 0, 1000));
    assert!(f.log.is_empty(), "{:?}", f.log);
}

// ---------------------------------------------------------------- §5.6 / §5.7

// §5.6 step 2: the Nightmare column of ColdEffect.
#[test]
fn cold_effect_nightmare() {
    let mut m = monster_rec();
    m.coldeffect_n = (-30i8) as u8;
    let c = combat_tables(vec![m]);
    let mut f = world();
    f.difficulty = 1;
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0));
    cold(&mut f, &c, a, d, 40);
    assert!(
        f.log.contains(&"liststat 1 11 67 -30".to_string()),
        "{:?}",
        f.log
    );
}

// §5.6 step 3: only a monster's length is divided (Hell divisor 4).
#[test]
fn cold_divisor_monsters_only() {
    let c = ct();
    let mut f = world();
    f.difficulty = 2;
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Player, 0));
    cold(&mut f, &c, a, d, 40);
    assert_eq!(f.log[1], "list 1 11 0 40");
}

// §5.6 step 5: an existing list is extended only when its expiry < e.
#[test]
fn cold_existing_list_expiry() {
    let c = ct();
    for (x, extended) in [(39, true), (40, false), (41, false)] {
        let mut f = world();
        let a = f.add(FUnit::new(UnitType::Player, 0));
        let d = f.add(FUnit::new(UnitType::Player, 0));
        f.units[d].state_expiry.insert(11, x);
        cold(&mut f, &c, a, d, 40);
        let want: Vec<String> = if extended {
            vec!["expiry 1 11 40".into(), "timer 1 12 40".into()]
        } else {
            vec![]
        };
        assert_eq!(f.log[..f.log.len() - 1], want[..], "{x}");
    }
}

// §5.6 step 6: r = lo′ mod 100, shatter on r < 20.
#[test]
fn cold_shatter_draw() {
    let mut m = monster_rec();
    m.coldeffect = (-30i8) as u8;
    let c = combat_tables(vec![m]);
    for (x, on) in [(2019, true), (20, false), (19, true)] {
        let mut f = world();
        let a = f.add(FUnit::new(UnitType::Player, 0));
        let d = f.add(FUnit::new(UnitType::Monster, 0));
        f.units[d].seed = seed_giving(x);
        cold(&mut f, &c, a, d, 40);
        assert_eq!(f.log.last().unwrap(), &format!("state 1 107 {on}"), "{x}");
    }
}

// §5.7: a player defender gets cold; a boss, unique or hireling monster
// gets cold; another monster's length is divided by the Hell freeze
// divisor (4).
#[test]
fn freeze_targets() {
    let mut m = monster_rec();
    m.coldeffect_h = (-50i8) as u8;
    let c = combat_tables(vec![m]);
    let cold_log = |f: &Fake| f.log.first().cloned();
    let mut f = world();
    f.difficulty = 2;
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let p = f.add(FUnit::new(UnitType::Player, 0));
    freeze(&mut f, &c, p, a, 40);
    assert_eq!(cold_log(&f).as_deref(), Some("state 1 11 true"));
    for kind in 0..3 {
        let mut u = FUnit::new(UnitType::Monster, 0);
        match kind {
            0 => u.boss = true,
            1 => u.flags = 8,
            _ => u.hireling = true,
        }
        let mut f = world();
        f.difficulty = 2;
        let a = f.add(FUnit::new(UnitType::Player, 0));
        let d = f.add(u);
        freeze(&mut f, &c, d, a, 40);
        assert_eq!(cold_log(&f).as_deref(), Some("state 1 11 true"), "{kind}");
    }
    let mut f = world();
    f.difficulty = 2;
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0));
    freeze(&mut f, &c, d, a, 40);
    assert_eq!(f.log[..2], ["state 1 1 true", "list 1 1 0 10"]);
}

// ---------------------------------------------------------------- §5.2

/// `apply` of a melee hit record with `total` on a defender with life
/// 1000.
fn apply_case(
    f: &mut Fake,
    c: &CombatTables,
    a: usize,
    d: usize,
    rec: DamageRecord,
) -> DamageRecord {
    let mut rec = rec;
    apply(f, c, a, d, false, &mut rec);
    rec
}

// Step 2: no room clears result 1 and 2 only; town returns unless the
// attacker is a monster with `inTown`.
#[test]
fn apply_room_rules() {
    let mut m = monster_rec();
    m.intown = true;
    let c = combat_tables(vec![m.clone(), monster_rec()]);
    let rec = DamageRecord {
        result: result::HIT | result::WILL_DIE | result::BLOCK,
        total: 100,
        ..DamageRecord::default()
    };
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Player, 0).with(6, 1000));
    f.units[d].room = RoomKind::None;
    assert_eq!(apply_case(&mut f, &c, a, d, rec).result, result::BLOCK);
    f.units[d].room = RoomKind::Town;
    let mon_in_town = f.add(FUnit::new(UnitType::Monster, 0));
    let mon_out = f.add(FUnit::new(UnitType::Monster, 1));
    // Player (class 0 is an inTown monster class): returns.
    let r = apply_case(&mut f, &c, a, d, rec);
    assert_eq!((r.result, f.get(d, 6)), (result::HIT | result::BLOCK, 1000));
    let r = apply_case(&mut f, &c, mon_out, d, rec);
    assert_eq!((r.result, f.get(d, 6)), (result::HIT | result::BLOCK, 1000));
    apply_case(&mut f, &c, mon_in_town, d, rec);
    assert_eq!(f.get(d, 6), 900);
}

// Steps 11–12: only totals / leeches > 0 subtract; the result is zeroed
// below 256 (256 itself stays).
#[test]
fn apply_subtract_bounds() {
    let c = ct();
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(
        FUnit::new(UnitType::Player, 0)
            .with(6, 100)
            .with(8, 100)
            .with(10, 100),
    );
    let r = apply_case(&mut f, &c, a, d, DamageRecord::default());
    assert_eq!((f.get(d, 6), f.get(d, 8), f.get(d, 10)), (100, 100, 100));
    assert_eq!(r.result & result::WILL_DIE, 0);
    // The mana leech is shifted by leech (§5.3 step 6): 16 << 6 = 1024.
    f.set(d, 6, 1256);
    f.set(d, 8, 1280);
    f.set(d, 10, 1256);
    let rec = DamageRecord {
        total: 1000,
        mana_leech: 16,
        stamina_leech: 1000,
        ..DamageRecord::default()
    };
    apply_case(&mut f, &c, a, d, rec);
    assert_eq!((f.get(d, 6), f.get(d, 8), f.get(d, 10)), (256, 256, 256));
}

// Step 14: the monster-damaged hook only for a monster defender.
#[test]
fn apply_damaged_hook_monsters_only() {
    let c = ct();
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Player, 0).with(6, 10_000));
    let rec = DamageRecord {
        total: 1000,
        ..DamageRecord::default()
    };
    apply_case(&mut f, &c, a, d, rec);
    assert!(
        !f.log.iter().any(|l| l.starts_with("mondamaged")),
        "{:?}",
        f.log
    );
}

// ---------------------------------------------------------------- §5.1

/// Stores a hit record from `a` on `d` and runs `apply_melee`.
fn melee(f: &mut Fake, a: usize, d: usize, rec: DamageRecord) {
    let c = ct();
    let entry = CombatEntry {
        attacker: f.ident(a),
        defender: f.ident(d),
        record: rec,
    };
    f.units[a].combat.push(entry);
    apply_melee(f, &c, a, d);
}

// Step 4.3: overlay only when +0x6C > 0. Step 6: a non-player
// non-monster attacker in mode 0 still gets thorns.
#[test]
fn apply_melee_overlay_and_thorns() {
    let mut f = world();
    let a = f.add(FUnit::new(UnitType::Player, 0));
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(6, 10_000));
    let hit = DamageRecord {
        result: result::HIT,
        ..DamageRecord::default()
    };
    melee(&mut f, a, d, hit);
    assert!(
        !f.log.iter().any(|l| l.starts_with("overlay")),
        "{:?}",
        f.log
    );
    let mut ms = FUnit::new(UnitType::Missile, 0);
    ms.mode = 0;
    let m = f.add(ms);
    f.log.clear();
    melee(&mut f, m, d, hit);
    assert!(f.log.contains(&"thorns 2 1".to_string()), "{:?}", f.log);
}

// ---------------------------------------------------------------- §6

// §6.1: the critical nibble 0x10 only when no element matched and the
// result has 0x2000.
#[test]
fn element_hit_class_crit_fallback() {
    let mut counter = 0u8;
    let rec = fire(10);
    assert_eq!(element_hit_class(&mut counter, &rec, 2), 0x22);
    assert_eq!(
        element_hit_class(&mut counter, &DamageRecord::default(), 2),
        2
    );
    let crit = DamageRecord {
        result: result::CRITICAL,
        ..fire(10)
    };
    assert_eq!(element_hit_class(&mut counter, &crit, 2), 0x22);
}

// §6.2 step 2: poison alone (poison = total) skips get-hit; mixed damage
// goes on to the size tests (1000 ≥ 1000 / 4: get-hit).
#[test]
fn no_get_hit_poison_only() {
    let c = ct();
    let mut f = world();
    let u = f.add(FUnit::new(UnitType::Player, 0).with(7, 1000));
    let rec = DamageRecord {
        poison: 100,
        total: 1000,
        ..DamageRecord::default()
    };
    assert!(!no_get_hit(&mut f, &c.hitclass, u, &rec, 0));
    let rec = DamageRecord {
        poison: 1000,
        total: 1000,
        ..DamageRecord::default()
    };
    assert!(no_get_hit(&mut f, &c.hitclass, u, &rec, 0));
}
