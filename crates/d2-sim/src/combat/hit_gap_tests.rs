// Spec: specs/combat/hit.md
// Unit tests for hit.md rules the first test set left unclaimed. Values
// come from the spec's formulas; the fake world is `skills::fake`.
use super::*;
use crate::rng::Seed;
use crate::skills::fake::*;
use crate::skills::SkillTables;
use crate::units::UnitType;

/// A seed whose next step gives `lo′ = x` (lo 0, hi x).
fn seed_giving(x: u32) -> Seed {
    Seed::new(0, x)
}

/// `seed` after `n` steps.
fn stepped(seed: Seed, n: usize) -> Seed {
    let mut s = seed;
    for _ in 0..n {
        s.step();
    }
    s
}

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

// §2: `defense` takes any unit (unlike `attack_rating`, players only),
// and the hit test adds `armorclass_vs_missile` (missiles) or
// `armorclass_vs_hth` (melee) on top.
// Covers: specs/combat/hit.md §2 text
#[test]
fn defense_any_unit_and_per_attack_term() {
    let s = st();
    let c = ct();
    let mut f = world();
    // base = 100 + 50 / 4 = 112; pct 50 → 112 + 56 = 168 for every type.
    let mut units = Vec::new();
    for ty in [
        UnitType::Player,
        UnitType::Monster,
        UnitType::Object,
        UnitType::Missile,
        UnitType::Item,
    ] {
        let u = f.add(
            FUnit::new(ty, 0)
                .with(31, 100)
                .with(2, 50)
                .with(16, 50)
                .with(32, 7)
                .with(33, 11),
        );
        assert_eq!(defense(&mut f, &s, u), 168, "{ty:?}");
        units.push(u);
    }
    // The per-attack term belongs to the hit test, not to `defense`.
    let a = f.add(FUnit::new(UnitType::Monster, 0).with(12, 1));
    let d = units[1];
    assert_eq!(hit_terms(&mut f, &s, &c, a, d, 0, false).def, 168 + 11);
    assert_eq!(hit_terms(&mut f, &s, &c, a, d, 0, true).def, 168 + 7);
}

// §3.2 step 3: melee with a current weapon starts pctAR at the to-hit
// weapon mastery (stat 342, layer = an item type of the weapon);
// missiles and weaponless attackers start at 0.
// Covers: specs/combat/hit.md §3.2 r3
#[test]
fn hit_terms_weapon_mastery_melee_only() {
    let (s, c) = (st(), ct());
    let mut f = world();
    let a = f.add(
        FUnit::new(UnitType::Player, 0)
            .with(19, 100)
            .with(2, 7)
            .with(12, 5),
    );
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(12, 5));
    let axe = f.add_item(FItem {
        types: vec![67],
        ..FItem::default()
    });
    f.units[a].weapon = Some(axe);
    // Entry for type 99 does not match the weapon.
    f.units[a].entries.insert(342, vec![(67, 25), (99, 70)]);
    assert_eq!(hit_terms(&mut f, &s, &c, a, d, 0, false).pct_ar, 25);
    // Step 4 adds onto it.
    f.set(a, 119, 5);
    assert_eq!(hit_terms(&mut f, &s, &c, a, d, 10, false).pct_ar, 40);
    // Missile: no mastery.
    assert_eq!(hit_terms(&mut f, &s, &c, a, d, 10, true).pct_ar, 15);
    // No current weapon: no mastery.
    f.units[a].weapon = None;
    assert_eq!(hit_terms(&mut f, &s, &c, a, d, 10, false).pct_ar, 15);
}

// §5: unit types other than player and monster have block chance 0.
// Covers: specs/combat/hit.md §5 text
#[test]
fn block_chance_other_unit_types() {
    let c = ct();
    let mut f = world();
    for ty in [
        UnitType::Object,
        UnitType::Missile,
        UnitType::Item,
        UnitType::Tile,
    ] {
        let u = f.add(FUnit::new(ty, 0).with(20, 50).with(2, 100).with(12, 1));
        f.units[u].shield = true;
        assert_eq!(block_chance(&f, &c, u, true), 0, "{ty:?}");
        assert_eq!(block_chance(&f, &c, u, false), 0, "{ty:?}");
    }
}

// §6.1 step 1: `block = 0` goes straight to `dodge` — no block chance,
// no block draw, even for a defender with a shield.
// Covers: specs/combat/hit.md §6.1 r1
#[test]
fn block_or_dodge_without_block_is_dodge() {
    let c = ct();
    let mut f = world();
    f.expansion = false;
    let a = f.add(FUnit::new(UnitType::Monster, 0));
    // Standing player, shield, block 50 + 25 = 75; dodge 30, avoid 40.
    let d = f.add(
        FUnit::new(UnitType::Player, 0)
            .with(20, 50)
            .with(338, 30)
            .with(339, 40),
    );
    f.units[d].shield = true;
    assert_eq!(block_chance(&f, &c, d, false), 75);
    // With block = 1 the first draw (29 < 75) blocks.
    f.units[d].seed = seed_giving(29);
    assert_eq!(
        block_or_dodge(&mut f, &c, a, d, false, true),
        BlockResult::Block
    );
    // Block = 0: the same first draw is the dodge draw (29 < 30).
    let s0 = seed_giving(29);
    f.units[d].seed = s0;
    assert_eq!(
        block_or_dodge(&mut f, &c, a, d, false, false),
        BlockResult::Dodge
    );
    assert_eq!(f.units[d].seed, stepped(s0, 1));
    // Avoid flag passes through: 39 < 40 → avoid.
    let s1 = seed_giving(39);
    f.units[d].seed = s1;
    assert_eq!(
        block_or_dodge(&mut f, &c, a, d, true, false),
        BlockResult::Avoid
    );
    assert_eq!(f.units[d].seed, stepped(s1, 1));
    // Dodge fails: none, one draw (no block draw before it).
    let s2 = seed_giving(30);
    f.units[d].seed = s2;
    assert_eq!(
        block_or_dodge(&mut f, &c, a, d, false, false),
        BlockResult::None
    );
    assert_eq!(f.units[d].seed, stepped(s2, 1));
}

// Edge case 6: at most 128 `attack_vs_montype` entries and at most 32
// `passive_weaponblock` entries are read.
// Covers: specs/combat/hit.md §edge-cases-original-bugs r6
#[test]
fn entry_copy_limits() {
    let s = st();
    let mut m = monster_rec();
    m.montype = 4;
    let c = combat_tables(vec![m]);
    let mut f = world();
    let a = f.add(
        FUnit::new(UnitType::Player, 0)
            .with(19, 100)
            .with(2, 7)
            .with(12, 5),
    );
    let d = f.add(FUnit::new(UnitType::Monster, 0).with(12, 5));
    // 129 matching entries of +1: only 128 count.
    f.units[a].entries.insert(179, vec![(4, 1); 129]);
    assert_eq!(hit_terms(&mut f, &s, &c, a, d, 0, false).pct_ar, 128);
    // 32 entries of 10, a 33rd of 90: the 33rd is not read.
    let mut wb = vec![(0u16, 10); 32];
    wb.push((0, 90));
    f.units[d].entries.insert(348, wb);
    assert_eq!(weapon_block(&f, Some(d)), 10);
    f.units[d].entries.get_mut(&348).unwrap()[31] = (0, 90);
    assert_eq!(weapon_block(&f, Some(d)), 90);
}
