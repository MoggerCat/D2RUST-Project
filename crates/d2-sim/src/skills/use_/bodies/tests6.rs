// Spec: specs/skills/bodies-2b.md §6, §7
//! Tests of the batch 3 bodies of required level 18 and 24 (the rules
//! `tests3` does not claim) on [`super::fake::BodyFake`].

use super::fake::BodyFake;
use super::tests2::{body_rec, monster, tabs, world, Code};
use super::*;
use crate::skills::fake::{blank, combat_tables, monster_rec, FUnit};
use crate::skills::SkillUnits;
use crate::units::UnitType;
use d2_data::tables::{Monstats, Monstats2};

// ---------------------------------------------------------------- §6.3

fn enchant_tables(c: &mut Code) -> crate::skills::SkillTables {
    let mut r = body_rec();
    r.aurastate = 33;
    r.auralencalc = c.f(100);
    r.aurastat1 = 20;
    r.aurastatcalc1 = c.f(5);
    r.aurastat2 = 21;
    r.aurastatcalc2 = c.f(0);
    r.aurastat3 = 22;
    r.aurastatcalc3 = c.f(9);
    r.aurastat4 = 0xFFFF;
    tabs(r, Code(c.0.clone()), 1)
}

// Covers: specs/skills/bodies-2b.md §6.3 r1
#[test]
fn enchant_refuses_bad_records() {
    let mut c = Code::new();
    let ct = combat_tables(vec![monster_rec()]);
    let t = enchant_tables(&mut c);
    let (mut f, u) = world();
    assert_eq!(b3_lvl18::enchant(&mut f, &t, &ct, u, 1, 1), 1);
    // R invalid.
    assert_eq!(b3_lvl18::enchant(&mut f, &t, &ct, u, 9, 1), 0);
    // aurastat1 < -1, >= count; aurastate out of range.
    let bad = |edit: &dyn Fn(&mut d2_data::tables::Skills)| {
        let (mut f, u) = world();
        let mut c = Code::new();
        let mut t = enchant_tables(&mut c);
        edit(&mut t.skills[1]);
        b3_lvl18::enchant(&mut f, &t, &ct, u, 1, 1)
    };
    assert_eq!(bad(&|r| r.aurastat1 = 0xFFFE), 0);
    assert_eq!(bad(&|r| r.aurastat1 = 359), 0);
    assert_eq!(bad(&|r| r.aurastat1 = 358), 1);
    assert_eq!(bad(&|r| r.aurastate = 200), 0);
    assert_eq!(bad(&|r| r.aurastate = 0xFFFF), 0);
    assert_eq!(bad(&|r| r.aurastate = 199), 1);
}

// Covers: specs/skills/bodies-2b.md §6.3 r2, §6.3 r3, §6.3 r4, §6.3 r5
#[test]
fn enchant_applies_the_state_to_an_ally_or_the_caster() {
    let mut c = Code::new();
    let ct = combat_tables(vec![monster_rec()]);
    let t = enchant_tables(&mut c);
    let (mut f, u) = world();
    let ally = f.add(FUnit::new(UnitType::Player, 0), (3, 3));
    let foe = f.add(FUnit::new(UnitType::Player, 0), (4, 4));
    f.allies.insert((u, ally));
    f.c.frame = 50;
    // Target an ally: U = T.
    f.targets.insert(u, ally);
    assert_eq!(b3_lvl18::enchant(&mut f, &t, &ct, u, 1, 3), 1);
    let l = f.list_of(ally, 33).expect("state list on the ally");
    assert_eq!(l.expire, 150);
    assert_eq!((l.skill, l.lvl), (1, 3));
    assert_eq!(l.owner, Some(u));
    assert_eq!(l.callback, callback::DEFAULT);
    // Only non-zero aurastatcalc_i values are set.
    assert_eq!(l.stats.get(&20), Some(&5));
    assert_eq!(l.stats.get(&21), None);
    assert_eq!(l.stats.get(&22), Some(&9));
    assert!(f.list_of(u, 33).is_none());
    assert!(f.take_log().contains(&format!("changed {ally} 33")));
    // A non-ally target: the caster itself.
    f.targets.insert(u, foe);
    assert_eq!(b3_lvl18::enchant(&mut f, &t, &ct, u, 1, 3), 1);
    assert!(f.list_of(u, 33).is_some());
    assert!(f.list_of(foe, 33).is_none());
    assert!(f.take_log().contains(&format!("changed {u} 33")));
}

// ---------------------------------------------------------------- §6.4

// Covers: specs/skills/bodies-2b.md §6.4 r1, §6.4 r2, §6.4 r3, §6.4 r4
#[test]
fn chain_lightning_sets_the_jump_count() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.srvmissilea = 0;
    r.calc1 = c.f(4);
    let t = tabs(r, c, 1);
    let (mut f, u) = world();
    f.tpos.insert(u, (6, 2));
    // R invalid; srvmissilea invalid.
    assert_eq!(b3_lvl18::chain_lightning(&mut f, &t, u, 9, 1), 0);
    let mut t2 = t.clone();
    t2.skills[1].srvmissilea = 0xFFFF;
    assert_eq!(b3_lvl18::chain_lightning(&mut f, &t2, u, 1, 1), 0);
    assert_eq!(f.c.units[u].flags & FLAG_40, 0);
    // Straight missile at the target; M data +0x28 := n.
    assert_eq!(b3_lvl18::chain_lightning(&mut f, &t, u, 1, 1), 1);
    assert_eq!(f.c.units[u].flags & FLAG_40, FLAG_40);
    assert_eq!(f.missiles.len(), 1);
    assert_eq!(
        (
            f.missiles[0].flags,
            f.missiles[0].target_x,
            f.missiles[0].target_y
        ),
        (0x21, 6, 2)
    );
    assert!(f
        .take_log()
        .iter()
        .any(|s| s.contains("MissileData28") && s.ends_with("v: 4 }")));
    // No missile created: 0, but the unit flag is already set.
    f.c.units[u].flags = 0;
    f.no_missiles = true;
    assert_eq!(b3_lvl18::chain_lightning(&mut f, &t, u, 1, 1), 0);
    assert_eq!(f.c.units[u].flags & FLAG_40, FLAG_40);
}

// ---------------------------------------------------------------- §6.5

// Covers: specs/skills/bodies-2b.md §6.5 text, §6.5 r1, §6.5 r2, §6.5 r3, §6.5 r4
#[test]
fn teleport_checks_the_level_rule() {
    let (mut f, u) = world();
    f.tpos.insert(u, (12, 7));
    // R is not read: skill 1 has no record in an empty table, still places.
    f.teleport = Some(1);
    assert_eq!(b3_lvl18::teleport(&mut f, u), 1);
    assert!(f.take_log().contains(&format!("place {u} None (12, 7)")));
    // Level rule none / 0: refused.
    f.teleport = None;
    assert_eq!(b3_lvl18::teleport(&mut f, u), 0);
    f.teleport = Some(0);
    assert_eq!(b3_lvl18::teleport(&mut f, u), 0);
    // 2: refused when the line is blocked, placed otherwise.
    f.teleport = Some(2);
    f.blocked = true;
    assert_eq!(b3_lvl18::teleport(&mut f, u), 0);
    f.blocked = false;
    assert_eq!(b3_lvl18::teleport(&mut f, u), 1);
    // 1 does not look at the line.
    f.teleport = Some(1);
    f.blocked = true;
    assert_eq!(b3_lvl18::teleport(&mut f, u), 1);
    // No room: 0.
    f.room_of.remove(&u);
    assert_eq!(b3_lvl18::teleport(&mut f, u), 0);
}

// ---------------------------------------------------------------- §6.6

fn confuse_setup() -> (
    crate::skills::SkillTables,
    crate::combat::CombatTables,
    BodyFake,
    usize,
    usize,
) {
    let mut c = Code::new();
    let mut r = body_rec();
    r.aurastat1 = 0xFFFF;
    r.auratargetstate = 40;
    r.aurafilter = 0x103;
    r.aurarangecalc = c.f(10);
    r.auralencalc = c.f(100);
    r.aurastat2 = 25;
    r.aurastatcalc2 = c.f(11);
    r.aurastat3 = 26;
    r.aurastatcalc3 = c.f(0);
    r.aurastat4 = 27;
    r.aurastatcalc4 = c.f(13);
    r.aurastat5 = 0xFFFF;
    r.aurastatcalc5 = c.f(15);
    r.aurastat6 = 29;
    r.aurastatcalc6 = c.f(16);
    let t = tabs(r, c, 1);
    let mut ms: Monstats = monster_rec();
    ms.switchai = true;
    let mut ms2: Monstats2 = blank();
    ms2.isatt = true;
    let mut ct = combat_tables(vec![ms]);
    ct.monstats2 = vec![ms2];
    let (mut f, u) = world();
    let m = monster(&mut f, (2, 0));
    f.tpos.insert(u, (2, 0));
    f.scan = vec![m];
    f.c.hostile = true;
    f.c.units[m].mode = 1;
    f.c.set(m, 12, 1);
    (t, ct, f, u, m)
}

// Covers: specs/skills/bodies-2b.md §6.6 r1
#[test]
fn confuse_refuses_bad_records() {
    let (t, ct, mut f, u, _) = confuse_setup();
    assert_eq!(b3_lvl18::confuse(&mut f, &t, &ct, u, 9, 1), 0);
    let try_with = |edit: &dyn Fn(&mut d2_data::tables::Skills)| {
        let (mut t, ct, mut f, u, _) = confuse_setup();
        edit(&mut t.skills[1]);
        b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1)
    };
    assert_eq!(try_with(&|r| r.aurastat1 = 0xFFFE), 0);
    assert_eq!(try_with(&|r| r.aurastat1 = 359), 0);
    assert_eq!(try_with(&|r| r.auratargetstate = 200), 0);
    assert_eq!(try_with(&|r| r.auratargetstate = 0xFFFF), 0);
    assert_eq!(try_with(&|_| {}), 1);
}

// Covers: specs/skills/bodies-2b.md §6.6 r2, §6.6 r3, §6.6 r4, §6.6 r5
#[test]
fn confuse_context_and_scan() {
    let (t, mut ct, mut f, u, m) = confuse_setup();
    ct.difficultylevels[0].aicursedivisor = 4;
    f.c.frame = 1000;
    assert_eq!(b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1), 1);
    // r2: unit flags |= 0x40.
    assert_eq!(f.c.units[u].flags & FLAG_40, FLAG_40);
    // r3: d = 100 / AiCurseDivisor 4.
    let l = f
        .list_of(m, 40)
        .expect("confuse state on the monster")
        .clone();
    assert_eq!(l.expire, 1025);
    assert!(f.take_log().contains(&format!("schedule {m} 10 1025 0 0")));
    // r4: the stats come from aurastat2…, slot 1 is (0, 0): no stat set
    // by the state request; slots 2 and 3 (value 0: not set) and 4; the
    // list stops at the invalid aurastat5.
    assert_eq!(l.stats.get(&25), Some(&11));
    assert_eq!(l.stats.get(&26), None);
    assert_eq!(l.stats.get(&27), Some(&13));
    assert_eq!(l.stats.get(&29), None, "stop at the first invalid stat");
    assert_eq!(l.stats.len(), 2);
    assert_eq!(l.callback, callback::CONFUSE);
    // r5: a unit the scan does not reach is not touched.
    let far = monster(&mut f, (500, 500));
    assert!(f.list_of(far, 40).is_none());
}

// Covers: specs/skills/bodies-2b.md §6.6 text
#[test]
fn confuse_without_a_divisor_keeps_the_duration() {
    let (t, ct, mut f, u, m) = confuse_setup();
    f.c.frame = 10;
    assert_eq!(b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1), 1);
    assert_eq!(f.list_of(m, 40).unwrap().expire, 110);
}

// Covers: specs/skills/bodies-2b.md §6.6 l2 r1
#[test]
fn confuse_test_rejects_the_wrong_targets() {
    let (t, ct, mut f, u, m) = confuse_setup();
    // Not hostile (the scan itself still returns 1).
    f.c.hostile = false;
    assert_eq!(b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1), 1);
    assert!(f.list_of(m, 40).is_none());
    f.c.hostile = true;
    // Dead.
    f.alive.remove(&m);
    b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1);
    assert!(f.list_of(m, 40).is_none());
    f.alive.insert(m);
    // Not a monster.
    let p = f.add(FUnit::new(UnitType::Player, 0), (2, 0));
    f.scan = vec![p];
    b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1);
    assert!(f.list_of(p, 40).is_none());
    // `can_switch(U, 11)` fails for a monster without `switchai`.
    f.scan = vec![m];
    let mut ct2 = ct.clone();
    ct2.monstats[0].switchai = false;
    b3_lvl18::confuse(&mut f, &t, &ct2, u, 1, 1);
    assert!(f.list_of(m, 40).is_none());
}

// Covers: specs/skills/bodies-2b.md §6.6 l2 r2, §6.6 l2 r3
#[test]
fn confuse_slot_one_stays_empty() {
    let (t, ct, mut f, u, m) = confuse_setup();
    assert_eq!(b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1), 1);
    // Stat 0 / value 0 → v = 0 → request stat −1: nothing set for it.
    let l = f.list_of(m, 40).unwrap();
    assert_eq!(l.stats.get(&0), None);
    assert_eq!(l.owner, Some(u));
    assert_eq!((l.skill, l.lvl), (1, 1));
}

// Covers: specs/skills/bodies-2b.md §6.6 l2 r4, §6.6 l2 r5
#[test]
fn confuse_updateanimrate_refreshes_the_animation() {
    let (t, ct, mut f, u, m) = confuse_setup();
    assert_eq!(b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1), 1);
    assert!(!f.take_log().contains(&format!("anim {m}")));
    let (t, ct, mut f, u, m) = confuse_setup();
    f.stat_infos.insert(
        25,
        BodyStat {
            updateanimrate: true,
            maxstat: -1,
            ..BodyStat::default()
        },
    );
    assert_eq!(b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1), 1);
    assert!(f.take_log().contains(&format!("anim {m}")));
}

// Covers: specs/skills/bodies-2b.md §6.6 l2 r6, §6.6 l2 r7
#[test]
fn confuse_turns_the_monster_and_arms_the_timer() {
    let (t, ct, mut f, u, m) = confuse_setup();
    f.c.frame = 7;
    assert_eq!(b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1), 1);
    let log = f.take_log();
    assert!(log.contains(&format!("Alignment {{ u: {m}, a: 1, v: 1 }}")));
    assert!(log.contains(&format!("NodePrepend {{ u: {m}, slot: 9 }}")));
    assert!(log.contains(&format!("TargetOverride {{ m: {m}, kind: 3, guid: 0 }}")));
    assert!(log.contains(&format!("schedule {m} 10 107 0 0")));
    // A monster already in a list is not prepended again.
    let (t, ct, mut f, u, m) = confuse_setup();
    f.node.insert(m, 3);
    assert_eq!(b3_lvl18::confuse(&mut f, &t, &ct, u, 1, 1), 1);
    assert!(!f.take_log().iter().any(|s| s.starts_with("NodePrepend")));
}

// Covers: specs/skills/bodies-2b.md §6.6 text
#[test]
fn confuse_remove_callback_restores_the_alignment() {
    let (_, _, mut f, _, m) = confuse_setup();
    f.state_on(m, 40, true);
    helpers3::remove_alignment(&mut f, m, 40);
    assert!(!f.has_state(m, 40));
    let log = f.take_log();
    assert!(log.contains(&format!("Alignment {{ u: {m}, a: 0, v: 1 }}")));
    assert!(log.contains(&format!("NodeRemove({m})")));
}
