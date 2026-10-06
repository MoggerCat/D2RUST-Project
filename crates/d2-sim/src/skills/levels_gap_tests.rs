// Spec: specs/skills/levels.md
// Unit tests for levels.md rules the first test set left unclaimed.
// Values come from the spec's rules; the fake world is `skills::fake`.
use super::fake::*;
use super::*;
use crate::units::UnitType;

fn native(skill: i32, base: i32) -> SkillEntry {
    SkillEntry {
        skill,
        base,
        owner_guid: -1,
        ..SkillEntry::default()
    }
}

// §1 `bonus_level` step 1: a missing skill record (id out of range, or
// negative) gives 0, whatever the unit's bonus stats and the entry's
// level bonus.
// Covers: specs/skills/levels.md §1 l2 r1
#[test]
fn bonus_level_bad_skill_is_zero() {
    let t = skill_tables(vec![skill_rec()]);
    let mut f = Fake::default();
    let u = f.add(FUnit::new(UnitType::Player, 1).with(127, 4));
    f.units[u].states.push(134); // shrine +2
    for id in [1, 57, -1] {
        f.units[u].stats.insert((107, id as u16), 3); // singleskill
        f.units[u].stats.insert((97, id as u16), 3); // nonclassskill
        let e = SkillEntry {
            level_bonus: 5,
            ..native(id, 2)
        };
        assert_eq!(bonus_level(&f, &t, u, &e), 0, "skill {id}");
        // The level is then the base alone.
        assert_eq!(skill_level(&f, &t, Some(u), Some(&e), true), 2);
    }
    // Control: the valid id 0 gets allskills + shrine + level bonus.
    let e = SkillEntry {
        level_bonus: 5,
        ..native(0, 2)
    };
    assert_eq!(bonus_level(&f, &t, u, &e), 5 + 2 + 4);
}

// §6 step 4 (`0x00644920`): `InGame` set; `level ≥ req_level`; `reqstr ≤
// str(0)`, `reqdex ≤ dex(2)`, `reqint ≤ energy(1)`, `reqvit ≤
// vitality(3)`; requirements are i16.
// Covers: specs/skills/levels.md §6 r4
#[test]
fn attribute_requirements() {
    let mut r = skill_rec();
    (r.ingame, r.reqlevel) = (true, 6);
    (r.reqstr, r.reqdex, r.reqint, r.reqvit) = (30, 40, 50, 60);
    let t = skill_tables(vec![r]);
    let mut f = Fake::default();
    let u = f.add(
        FUnit::new(UnitType::Player, 1)
            .with(12, 6)
            .with(0, 30)
            .with(2, 40)
            .with(1, 50)
            .with(3, 60),
    );
    assert!(meets_attr_reqs(&f, &t, u, 0));
    // Each requirement one short fails.
    for (stat, v) in [(12u16, 5), (0, 29), (2, 39), (1, 49), (3, 59)] {
        let old = f.get(u, stat);
        f.set(u, stat, v);
        assert!(!meets_attr_reqs(&f, &t, u, 0), "stat {stat}");
        f.set(u, stat, old);
    }
    // req_level includes the native entry's base.
    f.units[u].skills = vec![native(0, 1)];
    assert!(!meets_attr_reqs(&f, &t, u, 0));
    f.set(u, 12, 7);
    assert!(meets_attr_reqs(&f, &t, u, 0));
    // InGame clear fails.
    let mut t2 = t.clone();
    t2.skills[0].ingame = false;
    assert!(!meets_attr_reqs(&f, &t2, u, 0));
    // Requirements are i16: 0xFFFF is −1, met by a stat of −1.
    let mut t3 = t.clone();
    t3.skills[0].reqstr = 0xFFFF;
    f.set(u, 0, -1);
    assert!(meets_attr_reqs(&f, &t3, u, 0));
    f.set(u, 0, -2);
    assert!(!meets_attr_reqs(&f, &t3, u, 0));
}

// Edge case 9: the cap is the class-0 maximum (99 in 1.14d) for every
// unit: players of any class and monsters alike.
// Covers: specs/skills/levels.md §edge-cases-original-bugs r9
#[test]
fn level_cap_same_for_every_unit() {
    let t = skill_tables(vec![skill_rec()]);
    assert_eq!(t.level_cap, 99);
    let mut f = Fake::default();
    let mut units = Vec::new();
    for class in 0..7 {
        units.push(f.add(FUnit::new(UnitType::Player, class)));
    }
    units.push(f.add(FUnit::new(UnitType::Monster, 0)));
    units.push(f.add(FUnit::new(UnitType::Monster, 300)));
    for u in units {
        for (base, want) in [(98, 98), (99, 99), (100, 99), (150, 99)] {
            let e = native(0, base);
            assert_eq!(skill_level(&f, &t, Some(u), Some(&e), true), want);
        }
    }
}
