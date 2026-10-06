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
