// Test vectors: specs/combat/events.md (Test vectors, §2, §3) on the body
// fake ([`crate::skills::use_::bodies::fake::BodyFake`]).
use super::events::*;
use super::{CombatTables, DamageRecord};
use crate::skills::fake::{combat_tables, monster_rec, FUnit};
use crate::skills::use_::bodies::fake::BodyFake;
use crate::skills::use_::bodies::tests2::{body_rec, tabs, Code};
use crate::skills::use_::bodies::{BodyWorld, Handler};
use crate::skills::SkillTables;
use crate::units::UnitType;

impl EventWorld for BodyFake {
    fn layer_split(&self) -> (u32, u32) {
        (6, 0x3F)
    }
    fn list_owner(&self, l: usize) -> Option<usize> {
        self.lists[l].owner
    }
    fn guid(&self, u: usize) -> u32 {
        self.c.units[u].guid
    }
    fn terror(&mut self, s: usize, u: usize, skill: i32, a: i32, b: i32) {
        self.c.log.push(format!("terror {s} {u} {skill} {a} {b}"));
    }
    fn point_free(&self, _: usize, at: (i32, i32)) -> bool {
        !self.collides || at == self.path_target
    }
    fn corpse_near(&mut self, _: usize) -> Option<usize> {
        self.found.first().copied()
    }
    fn queue_item_cast(&mut self, u: usize, msg: ItemCastMsg) {
        self.c.log.push(format!("cast {u} {msg:?}"));
    }
    fn raise_test(&self, _: usize) -> bool {
        true
    }
    fn clear_pattern(&mut self, v: usize) {
        self.c.log.push(format!("clearpattern {v}"));
    }
    fn raise_step(&mut self, n: usize, step: RaiseStep<usize>) {
        self.c.log.push(format!("raise {n} {step:?}"));
    }
    fn handlers_of(&self, u: usize) -> Vec<Handler> {
        self.handlers.get(&u).cloned().unwrap_or_default()
    }
    fn remove_handler(&mut self, u: usize, h: &Handler) {
        if let Some(v) = self.handlers.get_mut(&u) {
            if let Some(i) = v.iter().position(|x| x == h) {
                v.remove(i);
            }
        }
    }
}

fn tb<'a>(t: &'a SkillTables, ct: &'a CombatTables) -> EventTables<'a> {
    EventTables {
        skills: t,
        combat: ct,
    }
}

/// Attackable monsters (monstats2 `isatt`, for `apply_state`).
fn monsters(n: usize) -> CombatTables {
    let mut ct = combat_tables(vec![monster_rec(); n]);
    for m in &mut ct.monstats2 {
        m.isatt = true;
    }
    ct
}

fn plain() -> (SkillTables, CombatTables) {
    (tabs(body_rec(), Code::new(), 1), monsters(300))
}

/// k for an item event: stat << 16 | layer.
fn k(stat: u16, layer: u16) -> i32 {
    (i32::from(stat) << 16) | i32::from(layer)
}

fn unit(f: &mut BodyFake, ty: UnitType, stats: &[(u16, u16, i32)]) -> usize {
    let mut u = FUnit::new(ty, 0);
    for &(s, l, v) in stats {
        u.stats.insert((s, l), v);
    }
    f.add(u, (0, 0))
}

fn ev<U>(event: i32, h: Option<U>, o: Option<U>, k: i32, l: i32) -> EventCall<U> {
    EventCall { event, h, o, k, l }
}

// Covers: specs/combat/events.md §2.6, §edge-cases-original-bugs r5
#[test]
fn attacker_takes_records() {
    // Test vector: Fn 12, H level 30, O level 20, v 10.
    let r = attacker_takes_record(12, 10, 30, 20);
    assert_eq!((r.cold, r.cold_len, r.hit_class), (2560, 125, 0x3D));
    assert_eq!(r.result, 0x4021);
    // H not higher: 25.
    assert_eq!(attacker_takes_record(12, 10, 20, 20).cold_len, 25);
    assert_eq!(
        (
            attacker_takes_record(6, 3, 1, 1).physical,
            attacker_takes_record(6, 3, 1, 1).hit_class
        ),
        (768, 0x8D)
    );
    assert_eq!(attacker_takes_record(10, 3, 1, 1).lightning, 768);
    assert_eq!(attacker_takes_record(11, 3, 1, 1).hit_class, 0x2D);
    // O needs unit flag 0x4 and v > 0.
    let (t, ct) = plain();
    let mut f = BodyFake::new();
    let h = unit(&mut f, UnitType::Player, &[(78, 0, 5)]);
    let o = unit(&mut f, UnitType::Monster, &[]);
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            6,
            ev(2, Some(h), Some(o), k(78, 0), 0),
            None
        ),
        0
    );
    f.c.units[o].flags |= 4;
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            6,
            ev(2, Some(h), Some(o), k(78, 0), 0),
            None
        ),
        1
    );
    assert!(f
        .c
        .log
        .iter()
        .any(|l| l.starts_with(&format!("reaction {h} {o}"))));
}

// Covers: specs/combat/events.md §2.11
#[test]
fn freeze_vectors() {
    // Fn 14: v 5, H 20, O 25, event 5: c = 105 → 100; r = 37 → 151.
    assert_eq!(freeze_chance(5, 20, 25, false), 100);
    assert_eq!(freeze_length(100, 37), 151);
    // Event 6: 5 × (16 − 25 + 14 + 10) / 3 = 25; r = 30 → nothing.
    assert_eq!(freeze_chance(5, 20, 25, true), 25);
    // Clamps.
    assert_eq!(freeze_chance(0, 1, 99, false), 0);
    assert_eq!(freeze_length(100, 0), 225);
    assert_eq!(freeze_length(99, 99), 25);
}

// Covers: specs/combat/events.md §2.9
#[test]
fn stupidity_vectors() {
    // Fn 9: H 30, O 40, v 2, event 5: c = 20; r = 4 → d = 4.
    assert_eq!(stupidity_chance(30, 40, 2, false), 20);
    assert_eq!(stupidity_level(20, 4), 4);
    // Event 6 divides a positive c; the clamp is 1…99, d 1…20.
    assert_eq!(stupidity_chance(30, 40, 2, true), 6);
    assert_eq!(stupidity_chance(1, 99, 0, false), 1);
    assert_eq!(stupidity_chance(99, 1, 10, false), 99);
    assert_eq!(stupidity_level(99, 0), 20);
}

// Covers: specs/combat/events.md §2.17
#[test]
fn golem_drain_percent() {
    // Fn 23 f: Param1 0, Param2 100, L 10 → (10 × 110) / 16 = 68.
    let mut r = body_rec();
    r.param1 = 0;
    r.param2 = 100;
    let t = tabs(r, Code::new(), 1);
    assert_eq!(golem_drain_pct(&t, 10, 1), 68);
    assert_eq!(golem_drain_pct(&t, 0, 1), 0);
    assert_eq!(golem_drain_pct(&t, 10, 99), 0);
}

// Covers: specs/combat/events.md §2.18, §edge-cases-original-bugs r3
#[test]
fn energy_shield_row_vector() {
    // Fn 24: w = 2560, p 50, m 5120, q 16 → x = 1280, m = 3840.
    assert_eq!(shield_row(2560, 50, 5120, 16), (1280, 3840));
    // Mana-bound: pct(m, 16, q) caps x; m never goes below 0.
    assert_eq!(shield_row(10_000, 100, 64, 32), (32, 0));
}

// Covers: specs/combat/events.md §2.7, §1
#[test]
fn knockback_draw_against_size() {
    let (t, mut ct) = plain();
    let mut f = BodyFake::new();
    let h = unit(&mut f, UnitType::Player, &[(81, 0, 1)]);
    let o = unit(&mut f, UnitType::Monster, &[]);
    // One `mask(128)` draw on H's seed; < 0x40 for a medium monster.
    let mut s = f.c.units[h].seed;
    let want = (s.mask(128) as i32) < 0x40;
    let mut r = DamageRecord::default();
    let got = call(
        &mut f,
        tb(&t, &ct),
        7,
        ev(5, Some(h), Some(o), k(81, 0), 0),
        Some(&mut r),
    );
    assert_eq!(got == 1, want);
    assert_eq!(r.result & 8 != 0, want);
    assert_eq!(f.c.units[h].seed, s);
    // A large monster: t = 0x20; no record → 0 without a draw.
    ct.monstats2[0].large = true;
    let before = f.c.units[h].seed;
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            7,
            ev(5, Some(h), Some(o), k(81, 0), 0),
            None
        ),
        0
    );
    assert_eq!(f.c.units[h].seed, before);
}

// Covers: specs/combat/events.md §2.10, §2.12
#[test]
fn mana_and_life_after_events() {
    let (t, ct) = plain();
    let mut f = BodyFake::new();
    // Mana 8 at 100, max (stat 9 in the fake) 1000; v 50 % of total 400.
    let h = unit(
        &mut f,
        UnitType::Player,
        &[
            (8, 0, 100),
            (9, 0, 1000),
            (114, 0, 50),
            (6, 0, 500),
            (7, 0, 600),
            (138, 0, 1),
        ],
    );
    let o = unit(&mut f, UnitType::Monster, &[]);
    let mut r = DamageRecord {
        total: 400,
        ..DamageRecord::default()
    };
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            13,
            ev(1, Some(h), Some(o), k(114, 0), 0),
            Some(&mut r)
        ),
        1
    );
    assert_eq!(f.c.units[h].stats[&(8, 0)], 300);
    assert!(f.c.log.contains(&format!("overlay {h} 152")));
    // 17: + (v << 8), capped at max mana.
    call(
        &mut f,
        tb(&t, &ct),
        17,
        ev(9, Some(h), Some(o), k(138, 0), 0),
        None,
    );
    assert_eq!(f.c.units[h].stats[&(8, 0)], 556);
    // 18 needs a demon; 28 does not.
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            18,
            ev(9, Some(h), Some(o), k(138, 0), 0),
            None
        ),
        0
    );
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            28,
            ev(9, Some(h), Some(o), k(138, 0), 0),
            None
        ),
        1
    );
    assert_eq!(f.c.units[h].stats[&(6, 0)], 600);
    // At the maximum: 0.
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            28,
            ev(9, Some(h), Some(o), k(138, 0), 0),
            None
        ),
        0
    );
}

// Covers: specs/combat/events.md §2.13
#[test]
fn slow_caps_and_state_list() {
    let (t, ct) = plain();
    let mut f = BodyFake::new();
    let h = unit(&mut f, UnitType::Player, &[(150, 0, 95)]);
    let o = unit(&mut f, UnitType::Monster, &[]);
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            19,
            ev(5, Some(h), Some(o), k(150, 0), 0),
            None
        ),
        1
    );
    let l = f.list_of(o, 24).expect("slowed list");
    assert_eq!(
        (l.stats[&67], l.stats[&68], l.stats[&69], l.expire),
        (-90, -90, -90, 750)
    );
    assert!(f.c.log.contains(&format!("anim {o}")));
    // A player target: 50; Clay Golem reads stat 150 itself.
    let p = unit(&mut f, UnitType::Player, &[]);
    call(&mut f, tb(&t, &ct), 27, ev(1, Some(h), Some(p), 0, 0), None);
    assert_eq!(f.list_of(p, 24).unwrap().stats[&67], -50);
    // A boss: 50 for 19, 90 for 27.
    let b = unit(&mut f, UnitType::Monster, &[]);
    f.c.units[b].boss = true;
    call(&mut f, tb(&t, &ct), 27, ev(1, Some(h), Some(b), 0, 0), None);
    assert_eq!(f.list_of(b, 24).unwrap().stats[&67], -90);
    // Other unit types: 0.
    let m = unit(&mut f, UnitType::Object, &[]);
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            19,
            ev(5, Some(h), Some(m), k(150, 0), 0),
            None
        ),
        0
    );
}

// Covers: specs/combat/events.md §2.16, §edge-cases-original-bugs r2
#[test]
fn bone_and_cyclone_armor_absorb() {
    let mut r = body_rec();
    r.aurastate = 10;
    r.aurastat1 = 100;
    r.aurastat2 = 101;
    let t = tabs(r, Code::new(), 1);
    let ct = monsters(1);
    let mut f = BodyFake::new();
    let h = unit(&mut f, UnitType::Player, &[]);
    let l = f.alloc_list(0, 0, None).unwrap();
    f.set_list_state(l, 10);
    f.attach(h, l);
    f.list_set(l, 100, 300);
    f.list_set(l, 101, 1000);
    f.list_set(l, 84, 30);
    // Bone Armor: 200 physical absorbed; q = 10, |10 − 30| ≥ 5 → marked.
    let mut rec = DamageRecord {
        physical: 200,
        ..DamageRecord::default()
    };
    call(
        &mut f,
        tb(&t, &ct),
        22,
        ev(4, Some(h), None, 1, 1),
        Some(&mut rec),
    );
    assert_eq!(
        (rec.physical, f.list_get(l, 100), f.list_get(l, 84)),
        (0, 100, 10)
    );
    assert!(f.c.log.contains(&format!("changed {h} 10")));
    // Cyclone Armor: fire, then cold, then lightning.
    f.c.log.clear();
    let mut rec = DamageRecord {
        fire: 30,
        cold: 40,
        lightning: 50,
        ..DamageRecord::default()
    };
    call(
        &mut f,
        tb(&t, &ct),
        25,
        ev(4, Some(h), None, 1, 1),
        Some(&mut rec),
    );
    // a = 100: fire 30, cold 40, then 30 of the lightning's 50.
    assert_eq!((rec.fire, rec.cold, rec.lightning), (0, 0, 20));
    assert_eq!(f.list_get(l, 100), 0);
    // a ≤ 0: the state goes off and the list is freed.
    assert!(f.c.log.contains(&format!("free {h} {l}")));
}

// Covers: specs/combat/events.md §2.21, §edge-cases-original-bugs r6
#[test]
fn reanimate_registers_a_run_once_raise() {
    let (t, ct) = plain();
    let mut f = BodyFake::new();
    let p = unit(&mut f, UnitType::Player, &[]);
    let h = unit(&mut f, UnitType::Monster, &[]);
    f.minion_owner.insert(h, p);
    let o = unit(&mut f, UnitType::Monster, &[]);
    // v = 100 (always), class layer 7.
    f.c.units[h].stats.insert((175, 7), 100);
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            31,
            ev(9, Some(h), Some(o), k(175, 7), 0),
            None
        ),
        1
    );
    let hs = f.handlers_of(o);
    assert_eq!(
        hs,
        [Handler {
            event: 13,
            key_type: 0,
            key: p as i32,
            skill: 7,
            level: p as i32,
            func: RAISE_FUNC,
        }]
    );
    // Champions and uniques (type flags 4, 8) are skipped.
    let u = unit(&mut f, UnitType::Monster, &[]);
    f.c.units[u].flags = 8;
    assert_eq!(
        call(
            &mut f,
            tb(&t, &ct),
            31,
            ev(9, Some(h), Some(u), k(175, 0), 0),
            None
        ),
        0
    );
    // The iteration runs the key-type-0 raise once and drops it.
    let ct2 = monsters(8);
    let before = f.handlers_of(o).len();
    assert_eq!(before, 1);
    run(&mut f, tb(&t, &ct2), 13, Some(o), None, None);
    assert!(f.handlers_of(o).is_empty());
    assert!(f.c.log.iter().any(|l| l == &format!("clearpattern {o}")));
    assert!(f
        .c
        .log
        .iter()
        .any(|l| l.starts_with("raise ") && l.ends_with("Umod21")));
}

// Covers: specs/combat/events.md §3 r1, §3 r3, §3 r4, §3 r7
#[test]
fn item_cast_core_needs_item_effect_and_restores_the_target() {
    let mut r = body_rec();
    r.itemeffect = 0;
    let t = tabs(r, Code::new(), 1);
    let ct = monsters(1);
    let mut f = BodyFake::new();
    let u = unit(&mut f, UnitType::Player, &[]);
    let o = unit(&mut f, UnitType::Monster, &[]);
    // `ItemEffect` 0: nothing, no message.
    assert_eq!(cast(&mut f, tb(&t, &ct), Some(u), 1, 1, Some(o), false), 0);
    assert!(!f.c.log.iter().any(|l| l.starts_with("cast ")));
    // No target unit or a caster with state 54: 0 before the core.
    assert_eq!(cast(&mut f, tb(&t, &ct), Some(u), 1, 1, None, false), 0);
    f.c.units[u].states.push(54);
    assert_eq!(
        cast_point(&mut f, tb(&t, &ct), Some(u), 1, 1, (5, 5), false),
        0
    );
    // Kind 1 (self): the core targets U, then restores the saved point.
    let mut r = body_rec();
    r.itemeffect = 1;
    r.itemtarget = 1;
    let t = tabs(r, Code::new(), 1);
    let mut f = BodyFake::new();
    let u = unit(&mut f, UnitType::Player, &[]);
    let (_, out) = core(&mut f, tb(&t, &ct), u, 1, 1, None, (5, 6), false);
    assert_eq!(out.unit, (0, f.c.units[u].guid));
    let log = f.take_log();
    let paths: Vec<_> = log.iter().filter(|l| l.starts_with("path ")).collect();
    assert_eq!(paths[0], &format!("path {u} TargetPoint(5, 6)"));
    assert_eq!(paths[1], &format!("path {u} TargetUnit(Some({u}))"));
    assert_eq!(
        paths.last().unwrap(),
        &&format!("path {u} TargetPoint(0, 0)")
    );
}
