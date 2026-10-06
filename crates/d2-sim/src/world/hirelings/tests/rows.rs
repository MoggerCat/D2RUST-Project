// Spec: specs/world/hirelings.md §1, §2, §9 r1 (tests)
//! Row lookups, the offer and the costs on synthetic rows.

use super::fake::{act1_ice_rows, row};
use crate::world::hirelings::{resurrect_cost, threshold, HirelingRow, HirelingRows};

/// Expansion Act 1 Normal: Fire (Id 0) and Ice (Id 1) at 3 / 36 / 67,
/// then Nightmare Ids 2, 3 at 36; classic rows first.
fn act1() -> HirelingRows {
    let mut rows = vec![row(0, 0, 1, 1, 3), row(0, 0, 1, 1, 25)];
    for id in 0..2 {
        for l in [3, 36, 67] {
            rows.push(row(100, id, 1, 1, l));
        }
    }
    rows.push(row(100, 2, 1, 2, 36));
    rows.push(row(100, 3, 1, 2, 36));
    HirelingRows::new(rows)
}

// Covers: specs/world/hirelings.md §1.2 r2, §1.1 r2
#[test]
fn row_at_picks_bracket_or_first() {
    let t = act1();
    // Id 1 rows are 5, 6, 7 (3 / 36 / 67).
    for (l, want) in [(2, 5), (3, 5), (35, 5), (36, 6), (99, 7)] {
        assert_eq!(t.row_at(true, 1, l), Some(want), "L = {l}");
    }
    // Classic Id 0 at L 30: the second classic bracket.
    assert_eq!(t.row_at(false, 0, 30), Some(1));
    assert_eq!(t.row_at(true, 256, 3), None);
    assert_eq!(t.row_at(false, 1, 3), None);
}

// Covers: specs/world/hirelings.md §1.2 r1
#[test]
fn candidates_are_lowest_brackets() {
    let t = act1();
    assert_eq!(t.candidates(true, 0, 0), vec![2, 5]);
    assert_eq!(t.candidates(true, 0, 1), vec![8, 9]);
    assert_eq!(t.candidates(false, 0, 0), vec![0]);
    assert!(t.candidates(true, 1, 0).is_empty());
    // Act 2 Normal with three subtypes.
    let rows = HirelingRows::new(vec![
        row(100, 6, 2, 1, 9),
        row(100, 6, 2, 1, 43),
        row(100, 7, 2, 1, 9),
        row(100, 8, 2, 1, 9),
    ]);
    assert_eq!(rows.candidates(true, 1, 0), vec![0, 2, 3]);
}

// Covers: specs/world/hirelings.md §1.2 r3
#[test]
fn act_of_name() {
    let mut a = row(100, 0, 1, 1, 3);
    (a.name_first, a.name_last) = (10, 50);
    let mut b = row(100, 6, 2, 1, 9);
    (b.name_first, b.name_last) = (51, 71);
    let t = HirelingRows::new(vec![a, b]);
    assert_eq!(t.act_of_name(10), 0);
    assert_eq!(t.act_of_name(60), 1);
    assert_eq!(t.act_of_name(5), 0);
}

// Covers: specs/world/hirelings.md §2 r1, §2 r2, §2 r3, §2 r4, §2 r6, §2 r7, §2 text
#[test]
fn offer_vector_seed_22752887() {
    // `hirelings.md` Test vectors: slot seed 22752887, 2 candidates,
    // player level 10 → candidate 1 (Id 1), L = 6, price 217. The price
    // fixes the Ice row's gold: 150·145/100 = 217.
    let mut rows = vec![row(100, 0, 1, 1, 3)];
    rows.extend(
        act1_ice_rows()
            .into_iter()
            .map(|r| HirelingRow { gold: 150, ..r }),
    );
    let t = HirelingRows::new(rows);
    let o = t.offer(true, 10, 22_752_887, 0, 0).expect("offer");
    assert_eq!((o.row, o.id, o.level, o.price), (1, 1, 6, 150 * 145 / 100));
    assert_eq!(o.price, 217);
    assert!(t.offer(true, 10, 22_752_887, 3, 0).is_none());
}

// Covers: specs/world/hirelings.md §2 r4, §2 r5, §2 r6
#[test]
fn offer_clamps_and_shift() {
    // Fire N, L 2 (player level 1 gives L < 2 → 2), bracket 3: d = −1.
    let r = HirelingRow {
        str_: 35,
        str_lvl: 10,
        dex: 45,
        dex_lvl: 15,
        hp: 45,
        hp_lvl: 9,
        gold: 100,
        dmg_min: 1,
        dmg_max: 3,
        dmg_lvl: 4,
        defense: 10,
        def_lvl: 3,
        exp_lvl: 100,
        share: -1,
        resist: -5,
        hire_desc: 7,
        ..row(100, 0, 1, 1, 3)
    };
    let t = HirelingRows::new(vec![r]);
    let o = t.offer(true, 1, 1, 0, 0).expect("offer");
    assert_eq!(o.level, 2);
    // `>> 3` rounds toward −∞: 35 + (−10 >> 3) = 35 − 2 = 33.
    assert_eq!(o.strength, 33);
    assert_eq!(o.dexterity, 45 + (-15 >> 3));
    assert_eq!(o.life, 40); // 45 − 9 = 36 → 40
    assert_eq!(o.price, 100); // 100·85/100 < gold → gold
    assert_eq!(o.experience, 3 * 100 * 2 * 2);
    assert_eq!(o.defense, 7);
    assert_eq!(o.min_damage, 0); // 1 + (−4 >> 3) = 0
    assert_eq!(o.max_damage, 2);
    assert_eq!((o.share, o.resist, o.hire_desc), (0, 0, 7));
}

// Covers: specs/world/hirelings.md §9 r1
#[test]
fn resurrect_cost_vectors() {
    assert_eq!(resurrect_cost(30), 6750);
    assert_eq!(resurrect_cost(1), 0);
    assert_eq!(resurrect_cost(81), 49200);
    assert_eq!(resurrect_cost(82), 50000);
}

// Covers: specs/world/hirelings.md §4 r6
#[test]
fn threshold_vectors() {
    // Exp/Lvl 105 (Act 1 Ice): threshold(6) 26460, threshold(7) 41160.
    assert_eq!(threshold(105, 6), 26460);
    assert_eq!(threshold(105, 7), 41160);
    // 32-bit wrap.
    assert_eq!(threshold(i32::MAX, 2), i32::MAX.wrapping_mul(12));
}
