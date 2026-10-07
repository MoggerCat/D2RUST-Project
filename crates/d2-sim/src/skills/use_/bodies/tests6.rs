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

// ---------------------------------------------------------------- §6.9

fn hammer_tables(c: &mut Code) -> crate::skills::SkillTables {
    let mut r = body_rec();
    r.srvmissilea = 1;
    r.param1 = 16;
    tabs(r, Code(c.0.clone()), 2)
}

// Covers: specs/skills/bodies-2b.md §6.9 r1, §6.9 r2
#[test]
fn blessed_hammer_refuses() {
    let t = hammer_tables(&mut Code::new());
    let (mut f, u) = world();
    f.tpos.insert(u, (5, 6));
    // R invalid.
    assert_eq!(b3_lvl18::blessed_hammer(&mut f, &t, u, 9, 1), 0);
    // m <= 0: missile 0 is refused, too.
    let mut t0 = t.clone();
    t0.skills[1].srvmissilea = 0;
    assert_eq!(b3_lvl18::blessed_hammer(&mut f, &t0, u, 1, 1), 0);
    t0.skills[1].srvmissilea = 0xFFFF;
    assert_eq!(b3_lvl18::blessed_hammer(&mut f, &t0, u, 1, 1), 0);
    // No target position.
    f.tpos.clear();
    assert_eq!(b3_lvl18::blessed_hammer(&mut f, &t, u, 1, 1), 0);
    assert_eq!(f.c.units[u].flags & FLAG_40, 0);
    assert!(f.missiles.is_empty());
}

// Covers: specs/skills/bodies-2b.md §6.9 r3, §6.9 r4, §6.9 r5
#[test]
fn blessed_hammer_missile_and_path() {
    let t = hammer_tables(&mut Code::new());
    let (mut f, u) = world();
    f.tpos.insert(u, (5, 6));
    assert_eq!(b3_lvl18::blessed_hammer(&mut f, &t, u, 1, 3), 1);
    assert_eq!(f.c.units[u].flags & FLAG_40, FLAG_40);
    let m = f.missiles[0];
    assert_eq!(
        (m.flags, m.owner, m.origin, m.class, m.target_x, m.target_y),
        (0x20, u, Some(u), 1, 5, 6)
    );
    assert_eq!((m.skill, m.level), (1, 3));
    let log = f.take_log();
    let at = log
        .iter()
        .position(|s| s.ends_with("Type(14)"))
        .expect("type 14");
    assert!(log[at + 1].ends_with("Compute"));
    // No missile created: 0.
    f.no_missiles = true;
    assert_eq!(b3_lvl18::blessed_hammer(&mut f, &t, u, 1, 3), 0);
}

// Covers: specs/skills/bodies-2b.md §6.9 r6
#[test]
fn blessed_hammer_scales_with_concentration() {
    let t = hammer_tables(&mut Code::new());
    let (mut f, u) = world();
    f.tpos.insert(u, (5, 6));
    f.missile_base = vec![(52, 10), (53, 20)];
    // No Concentration: untouched.
    assert_eq!(b3_lvl18::blessed_hammer(&mut f, &t, u, 1, 1), 1);
    let m = f.c.units.len() - 1;
    assert_eq!((f.c.get(m, 52), f.c.get(m, 53)), (10, 20));
    // State 42 with damagepercent 40, Param1 16: c = 40 * 16 / 8 = 80.
    let l = f.alloc_list(0, 0, None).unwrap();
    f.set_list_state(l, 42);
    f.list_set(l, 25, 40);
    f.attach(u, l);
    f.state_on(u, 42, true);
    assert_eq!(b3_lvl18::blessed_hammer(&mut f, &t, u, 1, 1), 1);
    let m = f.c.units.len() - 1;
    assert_eq!((f.c.get(m, 52), f.c.get(m, 53)), (18, 36));
}

// ---------------------------------------------------------------- §6.10

fn freeze_world(
    lo: [u32; 2],
) -> (
    crate::skills::SkillTables,
    crate::combat::CombatTables,
    BodyFake,
    usize,
    [usize; 2],
) {
    let mut c = Code::new();
    let mut r = body_rec();
    r.aurastate = 50;
    r.auratargetstate = 51;
    r.aurastat1 = 20;
    r.aurastatcalc1 = c.f(7);
    r.passivestat1 = 21;
    r.passivecalc1 = c.f(3);
    r.aurarangecalc = c.f(10);
    r.auralencalc = c.f(100);
    r.aurafilter = 0x103;
    let t = tabs(r, c, 1);
    let mut ms = monster_rec();
    ms.coldeffect = 0xFF;
    let mut ms2: Monstats2 = blank();
    ms2.isatt = true;
    let mut ct = combat_tables(vec![ms]);
    ct.monstats2 = vec![ms2];
    let (mut f, u) = world();
    f.tpos.insert(u, (2, 0));
    f.c.hostile = true;
    let mut ms = [0; 2];
    for (i, m) in ms.iter_mut().enumerate() {
        *m = monster(&mut f, (2, i as i32));
        f.c.units[*m].mode = 1;
        f.c.units[*m].seed = crate::rng::Seed::new(lo[i], 0);
        f.c.set(*m, 12, 1);
    }
    f.scan = ms.to_vec();
    (t, ct, f, u, ms)
}

fn seed_with(want: std::ops::Range<u32>) -> u32 {
    (0u32..)
        .find(|&lo| want.contains(&(crate::rng::Seed::new(lo, 0).step() % 100)))
        .unwrap()
}

// Covers: specs/skills/bodies-2b.md §6.10 text, §6.10 r1
#[test]
fn holy_freeze_self_list_has_its_own_remove_callback() {
    let (t, ct, mut f, u, _) = freeze_world([0, 0]);
    assert_eq!(dos2::damage_aura(&mut f, &t, &ct, u, 1, 1, true), 1);
    let l = f.list_of(u, 50).expect("self list");
    assert_eq!(l.callback, callback::HOLY_FREEZE);
    // The srvdo 66 form keeps the default self callback.
    let (t, ct, mut f, u, _) = freeze_world([0, 0]);
    assert_eq!(dos2::damage_aura(&mut f, &t, &ct, u, 1, 1, false), 1);
    assert_ne!(f.list_of(u, 50).unwrap().callback, callback::HOLY_FREEZE);
}

// Covers: specs/skills/bodies-2b.md §6.10 r2, §6.10 l2 r1
#[test]
fn holy_freeze_only_affects_cold_affected_monsters() {
    let (t, mut ct, mut f, u, [m, other]) = freeze_world([0, 0]);
    f.c.units[other].class = 1;
    let mut ms = monster_rec();
    ms.coldeffect = 0;
    ct.monstats.push(ms);
    ct.monstats2.push(blank());
    let p = f.add(FUnit::new(UnitType::Player, 0), (2, 5));
    f.c.units[p].mode = 1;
    f.scan.push(p);
    assert_eq!(dos2::damage_aura(&mut f, &t, &ct, u, 1, 1, true), 1);
    assert!(f.list_of(m, 51).is_some(), "ColdEffect < 0");
    assert!(f.list_of(other, 51).is_none(), "ColdEffect >= 0");
    assert!(f.list_of(p, 51).is_some(), "other unit types pass");
    // The plain form has no such test.
    let (t, mut ct, mut f, u, [m, other]) = freeze_world([0, 0]);
    f.c.units[other].class = 1;
    ct.monstats.push(monster_rec());
    ct.monstats2.push(blank());
    assert_eq!(dos2::damage_aura(&mut f, &t, &ct, u, 1, 1, false), 1);
    assert!(f.list_of(m, 51).is_some() && f.list_of(other, 51).is_some());
}

// Covers: specs/skills/bodies-2b.md §6.10 l2 r2, §6.10 l2 r3
#[test]
fn holy_freeze_applies_the_aura_state_and_the_hit() {
    let (t, ct, mut f, u, [m, _]) = freeze_world([0, 0]);
    assert_eq!(dos2::damage_aura(&mut f, &t, &ct, u, 1, 1, true), 1);
    let l = f.list_of(m, 51).expect("target state");
    assert_eq!(l.stats.get(&20), Some(&7));
    assert_eq!((l.skill, l.lvl), (1, 1));
    // The hit record goes through apply and the reaction on the monster.
    assert!(f.take_log().iter().any(|s| s.contains(&format!("{m}"))));
}

// Covers: specs/skills/bodies-2b.md §6.10 l2 r4
#[test]
fn holy_freeze_shatter_is_a_one_in_five_draw() {
    let (t, ct, mut f, u, [a, b]) = freeze_world([seed_with(0..20), seed_with(20..100)]);
    assert_eq!(dos2::damage_aura(&mut f, &t, &ct, u, 1, 1, true), 1);
    assert!(f.has_state(a, 107), "draw < 20: shatter on");
    assert!(!f.has_state(b, 107), "draw >= 20: shatter off");
}

// Covers: specs/skills/bodies-2b.md §6.10 text
#[test]
fn holy_freeze_remove_callback() {
    let (_, _, mut f, _, [a, _]) = freeze_world([0, 0]);
    f.state_on(a, 50, true);
    f.state_on(a, 107, true);
    helpers3::remove_holy_freeze(&mut f, a, 50);
    assert!(!f.has_state(a, 50) && !f.has_state(a, 107));
    assert!(f.take_log().contains(&format!("anim {a}")));
    // Dead (and not "stay on death"): the state goes off, shatter stays.
    f.state_on(a, 50, true);
    f.state_on(a, 107, true);
    f.alive.remove(&a);
    helpers3::remove_holy_freeze(&mut f, a, 50);
    assert!(!f.has_state(a, 50) && f.has_state(a, 107));
}
