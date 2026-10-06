// Spec: specs/skills/levels.md, specs/skills/use.md, specs/data/calc-expressions.md
// Tests written against surviving mutants of `cargo mutants` (METHODS
// M08): each asserts what the spec says at the boundary or branch the
// mutant changed. The fake world is `skills::fake`.
use super::fake::*;
use super::*;
use crate::units::UnitType;

/// Skill tables with `skills`, the formula buffer `code` and one blank
/// skilldesc record.
fn tables_with(skills: Vec<Skills>, code: Vec<u8>) -> SkillTables {
    let mut t = skill_tables(skills);
    t.skills_code = code;
    t
}

fn item_entry(skill: i32, base: i32, owner: i32) -> SkillEntry {
    SkillEntry {
        skill,
        base,
        owner_guid: owner,
        ..SkillEntry::default()
    }
}

// ---------------------------------------------------------------- calc

// calc-expressions.md §3.3 op 0x01: an index ≥ the function count (7 for
// skills) pushes 0 and pops nothing. 5 6 9 CALL7 + → 9 + 0 = 9.
#[test]
fn call_past_function_count_pops_nothing() {
    let t = tables_with(
        vec![skill_rec()],
        vec![0x07, 5, 0x07, 6, 0x07, 9, 0x01, 7, 0x10, 0x00],
    );
    let mut f = Fake::default();
    assert_eq!(eval_skill(&mut f, &t, None, 0, 0, 1), 9);
}

// §3.5 skills `rand(a, b)` with no unit: `a ≥ b` → a, else 0 (the
// implementation's narrowest reading, TODO in `levels.rs`).
#[test]
fn rand_without_unit() {
    // rand(5, 3), rand(3, 5), rand(4, 4).
    for (a, b, want) in [(5, 3, 5), (3, 5, 0), (4, 4, 4)] {
        let t = tables_with(vec![skill_rec()], vec![0x07, a, 0x07, b, 0x01, 2, 0x00]);
        let mut f = Fake::default();
        assert_eq!(eval_skill(&mut f, &t, None, 0, 0, 1), want, "{a} {b}");
    }
}

// §3.5 skills `stat(s, mode)`: `s < 0` or `s ≥ itemstatcost count` → 0.
#[test]
fn stat_function_range() {
    let mut f = Fake::default();
    let u = f.add(FUnit::new(UnitType::Player, 0));
    for s in [65535u16, 359, 358] {
        f.units[u].stats.insert((s, 0), 77);
    }
    // stat(-1, 0), stat(359, 0), stat(358, 0).
    for (code, want) in [
        (vec![0x07, 0xFF, 0x07, 0, 0x01, 5, 0x00], 0),
        (vec![0x08, 0x67, 0x01, 0x07, 0, 0x01, 5, 0x00], 0),
        (vec![0x08, 0x66, 0x01, 0x07, 0, 0x01, 5, 0x00], 77),
    ] {
        let t = tables_with(vec![skill_rec()], code);
        assert_eq!(eval_skill(&mut f, &t, Some(u), 0, 0, 1), want);
    }
}

// §3.5 skills `miss(m, c)`: missile special `c` of missile `m` at the
// context level (misscalc 18 `lvl`).
#[test]
fn miss_function() {
    let t = tables_with(vec![skill_rec()], vec![0x07, 0, 0x07, 18, 0x01, 4, 0x00]);
    let mut f = Fake::default();
    assert_eq!(eval_skill(&mut f, &t, None, 0, 0, 7), 7);
}

// §3.5 skills parameter callback: special value `c` of the context skill
// (skillcalc 8 `par1`, 16 `lvl`).
#[test]
fn skill_param_callback() {
    let mut r = skill_rec();
    r.param1 = 42;
    let t = tables_with(vec![r], vec![0x04, 8, 0x04, 16, 0x10, 0x00]);
    let mut f = Fake::default();
    assert_eq!(eval_skill(&mut f, &t, None, 0, 0, 5), 47);
}

// ---------------------------------------------------------------- §1

// `highest_entry`: among non-native entries the larger base wins, the
// first on a tie.
#[test]
fn highest_entry_item_entries() {
    let a = item_entry(3, 2, 7);
    let b = item_entry(3, 5, 8);
    let c = item_entry(3, 2, 9);
    assert_eq!(highest_entry(&[a, b], 3), Some(b));
    assert_eq!(highest_entry(&[b, a], 3), Some(b));
    assert_eq!(highest_entry(&[a, c], 3), Some(a));
}

// `bonus_level`: a class skill of the player's class adds
// `addskill_tab(188)` at layer `page + 8 × class − 1`.
#[test]
fn bonus_level_skill_tab_layer() {
    let mut r = skill_rec();
    r.charclass = 3;
    r.skilldesc = 0;
    let mut t = skill_tables(vec![r]);
    t.skilldesc[0].skillpage = 2;
    let mut f = Fake::default();
    let u = f.add(FUnit::new(UnitType::Player, 3));
    // 2 + 24 − 1 = 25.
    f.units[u].stats.insert((188, 25), 4);
    f.units[u].stats.insert((188, 3), 100);
    let e = item_entry(0, 1, -1);
    assert_eq!(bonus_level(&f, &t, u, &e), 4);
}

// ---------------------------------------------------------------- §2

// skillcalc 12 / 13: Param5, Param6.
#[test]
fn special_par5_par6() {
    let mut r = skill_rec();
    r.param5 = 55;
    r.param6 = 66;
    let t = skill_tables(vec![r]);
    let mut f = Fake::default();
    assert_eq!(special(&mut f, &t, None, 12, 0, 1), 55);
    assert_eq!(special(&mut f, &t, None, 13, 0, 1), 66);
}

/// Tables with skill 0 whose skilldesc 0 names missiles 1, 2, 3 (the
/// second has Range 10, LevRange 2).
fn desc_tables() -> SkillTables {
    let mut r = skill_rec();
    r.skilldesc = 0;
    let mut t = skill_tables(vec![r]);
    t.skilldesc[0].descmissile1 = 1;
    t.skilldesc[0].descmissile2 = 2;
    t.skilldesc[0].descmissile3 = 3;
    t.missiles = vec![missile_rec(); 4];
    for (i, m) in t.missiles.iter_mut().enumerate() {
        m.range = 10 * i as u16;
        m.levrange = 2;
    }
    t
}

// skillcalc 35–37: Range + lvl × LevRange of descmissile 1–3.
#[test]
fn special_missile_range_slots() {
    let t = desc_tables();
    let mut f = Fake::default();
    for (c, want) in [(35, 10 + 6), (36, 20 + 6), (37, 30 + 6)] {
        assert_eq!(special(&mut f, &t, None, c, 0, 3), want, "{c}");
    }
}

// skillcalc 23–25: mastery type 0 reads passivestat 342 / 345, type 1
// 343 / 346, type 2 344 / 347.
#[test]
fn special_mastery_types() {
    let mut r = skill_rec();
    r.passivestat1 = 342;
    r.passivecalc1 = 0;
    r.passivestat2 = 343;
    r.passivecalc2 = 3;
    r.passivestat3 = 347;
    r.passivecalc3 = 6;
    let t = tables_with(
        vec![r],
        vec![0x07, 11, 0x00, 0x07, 22, 0x00, 0x07, 33, 0x00],
    );
    let mut f = Fake::default();
    assert_eq!(special(&mut f, &t, None, 23, 0, 1), 11);
    assert_eq!(special(&mut f, &t, None, 24, 0, 1), 22);
    assert_eq!(special(&mut f, &t, None, 25, 0, 1), 33);
}

// skillcalc 17 / 18 shift elem min / max right by 8; 38 / 39 do not.
#[test]
fn special_elem_shift() {
    let mut r = skill_rec();
    r.emin = 512;
    r.emax = 1024;
    let t = skill_tables(vec![r]);
    let mut f = Fake::default();
    assert_eq!(special(&mut f, &t, None, 17, 0, 1), 2);
    assert_eq!(special(&mut f, &t, None, 18, 0, 1), 4);
    assert_eq!(special(&mut f, &t, None, 38, 0, 1), 512);
    assert_eq!(special(&mut f, &t, None, 39, 0, 1), 1024);
}

// skillcalc 28: `lvl > 0 ? miss_elem_len(descmissile1) : 0`; the length
// at lvl ≤ 0 is the missile's ELen otherwise (§3.4).
#[test]
fn special_missile_len_guard() {
    let mut t = desc_tables();
    t.missiles[1].elen = 25;
    let mut f = Fake::default();
    assert_eq!(special(&mut f, &t, None, 28, 0, 1), 25);
    assert_eq!(special(&mut f, &t, None, 28, 0, 0), 0);
}

// ---------------------------------------------------------------- §3

// `bracket`: one level inside each band (L = 1, 10, 100, 1000, 10000).
#[test]
fn bracket_bands() {
    let l = [1, 10, 100, 1000, 10_000];
    for (lvl, want) in [
        (1, 0),
        (5, 4),
        (12, 7 + 40),
        (20, 7 + 80 + 400),
        (25, 7 + 80 + 600 + 3000),
        (40, 7 + 80 + 600 + 6000 + 120_000),
    ] {
        assert_eq!(bracket(lvl, l), want, "{lvl}");
    }
}

// §3.2: elem_len brackets `lvl ≤ 8`: (lvl − 1) × LL1.
#[test]
fn elem_len_low_band() {
    let mut r = skill_rec();
    r.elen = 100;
    r.elevlen1 = 10;
    r.elevlen2 = 1000;
    let t = skill_tables(vec![r]);
    let mut f = Fake::default();
    assert_eq!(elem_len(&mut f, &t, None, 0, 5), 140);
    assert_eq!(elem_len(&mut f, &t, None, 0, 10), 100 + 70 + 2000);
}

// §3.1 step 4: mastery stat by EType.
#[test]
fn elem_mastery_stats() {
    for (e, s) in [
        (1, Some(329)),
        (2, Some(330)),
        (4, Some(331)),
        (5, Some(332)),
        (12, Some(331)),
        (3, None),
        (0, None),
    ] {
        assert_eq!(elem_mastery_stat(e), s, "{e}");
    }
}

// §3.1 step 3 (Edge case 1): the min synergy (50 %) applies only if
// `v > 256` or `EMinLev1 ≠ 0`; the max synergy always.
#[test]
fn elem_min_synergy_gate() {
    let mk = |emin: u32, lev1: u32| {
        let mut r = skill_rec();
        r.emin = emin;
        r.emax = emin;
        r.eminlev1 = lev1;
        r.edmgsympercalc = 0;
        tables_with(vec![r], vec![0x07, 50, 0x00])
    };
    let mut f = Fake::default();
    let t = mk(256, 0);
    assert_eq!(elem_min(&mut f, &t, None, 0, 1, false), 256);
    assert_eq!(elem_max(&mut f, &t, None, 0, 1, false), 384);
    let t = mk(512, 0);
    assert_eq!(elem_min(&mut f, &t, None, 0, 1, false), 768);
    let t = mk(100, 1);
    assert_eq!(elem_min(&mut f, &t, None, 0, 1, false), 150);
}

// ---------------------------------------------------------------- missiles

/// Missile tables: missile 0 with Param1 42 and damage fields; skill 0
/// with Param1 42; the missile formula buffer `code`.
fn miss_tables(code: Vec<u8>) -> SkillTables {
    let mut r = skill_rec();
    r.param1 = 42;
    let mut t = skill_tables(vec![r]);
    let mut m = missile_rec();
    m.mindamage = 512;
    m.maxdamage = 1024;
    m.emin = 768;
    m.emax = 1280;
    t.missiles = vec![m];
    t.miss_code = code;
    t
}

fn eval_m(code: Vec<u8>, lvl: i32) -> i32 {
    let t = miss_tables(code);
    let mut f = Fake::default();
    eval_missile(&mut f, &t, None, None, 0, 0, lvl)
}

// calc-expressions.md §3.4 / §3.5, missiles family: 4 functions of arity
// 2: min, max, rand (no draw, Open question 1: 0), skill(s, c) on the
// owner; `param(c)` is the missile special value (misscalc 18 `lvl`).
#[test]
fn missile_context_functions() {
    assert_eq!(eval_m(vec![0x07, 3, 0x07, 5, 0x01, 0, 0x00], 1), 3);
    assert_eq!(eval_m(vec![0x07, 3, 0x07, 5, 0x01, 1, 0x00], 1), 5);
    assert_eq!(eval_m(vec![0x07, 0, 0x07, 8, 0x01, 2, 0x00], 1), 0);
    // skill(0, 8): Param1 of skill 0 (no owner: level 0, `par1` is
    // unguarded).
    assert_eq!(eval_m(vec![0x07, 0, 0x07, 8, 0x01, 3, 0x00], 1), 42);
    // Index 4 is past the missiles table: 0, nothing popped (7 + 0).
    assert_eq!(
        eval_m(vec![0x07, 7, 0x07, 1, 0x07, 2, 0x01, 4, 0x10, 0x00], 1),
        2
    );
    assert_eq!(eval_m(vec![0x04, 18, 0x00], 7), 7);
}

// misscalc 24 / 25 shift the physical min / max right by 8; 26 / 27 do
// not; 19 / 20 and 22 / 23 likewise for the elemental values.
#[test]
fn missile_special_shifts() {
    let t = miss_tables(Vec::new());
    let mut f = Fake::default();
    for (c, want) in [
        (24, 2),
        (25, 4),
        (26, 512),
        (27, 1024),
        (19, 3),
        (20, 5),
        (22, 768),
        (23, 1280),
    ] {
        assert_eq!(miss_special(&mut f, &t, None, None, c, 0, 1), want, "{c}");
    }
}

// §3.2: `ELenSymPerCalc ≠ −1` and `p ≠ 0` → `v += pct(v, p, 100)`.
#[test]
fn elem_len_synergy() {
    let mut r = skill_rec();
    r.elen = 100;
    r.elensympercalc = 0;
    let t = tables_with(vec![r], vec![0x07, 50, 0x00]);
    let mut f = Fake::default();
    assert_eq!(elem_len(&mut f, &t, None, 0, 1), 150);
}

// §3.3 step 3: with `SrcDam`, only a wield-type-2 weapon gives the
// weapon's own damage; another weapon reads `mindamage(21)` /
// `maxdamage(22)`.
#[test]
fn phys_srcdam_weapon_source() {
    let mut r = skill_rec();
    r.srcdam = 128;
    let t = skill_tables(vec![r]);
    let mut f = Fake::default();
    let u = f.add(FUnit::new(UnitType::Player, 0).with(21, 5).with(22, 7));
    let i = f.add_item(FItem {
        damage: (10, 20),
        ..FItem::default()
    });
    f.units[u].weapon = Some(i);
    assert_eq!(phys_min(&mut f, &t, Some(u), 0, 1, true), 5);
    assert_eq!(phys_max(&mut f, &t, Some(u), 0, 1, true), 7);
    f.items[i].wield = 2;
    assert_eq!(phys_min(&mut f, &t, Some(u), 0, 1, true), 10);
    assert_eq!(phys_max(&mut f, &t, Some(u), 0, 1, true), 20);
}
