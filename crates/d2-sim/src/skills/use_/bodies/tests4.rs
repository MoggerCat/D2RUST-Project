// Spec: specs/skills/bodies-4.md
//! Tests of the batch 4 helpers and bodies of `bodies-4.md` on
//! [`super::fake::BodyFake`]. Every expected value is computed from the
//! rule text; the seed draws are followed with [`Seed`] itself, in the
//! order the spec gives.

use super::fake::{stored, BodyFake};
use super::tests2::{body_rec, monster, tabs, world, Code};
use super::*;
use crate::combat::{CombatTables, DamageRecord};
use crate::rng::Seed;
use crate::skills::fake::{combat_tables, monster_rec, FUnit};
use crate::skills::SkillTables;
use crate::units::UnitType;

type Fx = BodyEffect<usize, usize, usize>;

/// The log line of an effect.
fn fx(e: Fx) -> String {
    format!("{e:?}")
}

/// Monster tables of `n` rows with `BaseId` = the row index and no
/// `NextInClass`.
fn ct_ids(n: usize) -> CombatTables {
    let mut rows = vec![monster_rec(); n];
    for (i, m) in rows.iter_mut().enumerate() {
        m.baseid = i as u16;
        m.monstatsex = i as u16;
        m.nextinclass = 0xFFFF;
    }
    combat_tables(rows)
}

/// A monster of `class` at `at`.
fn mon(f: &mut BodyFake, class: i32, at: (i32, i32)) -> usize {
    f.add(FUnit::new(UnitType::Monster, class), at)
}

/// Skill 1 = `r`, with `n` missile rows.
fn tab1(r: d2_data::tables::Skills, c: Code, n: usize) -> SkillTables {
    tabs(r, c, n)
}

fn logged(f: &mut BodyFake, s: &str) -> bool {
    f.c.log.iter().any(|l| l == s)
}

// ---------------------------------------------------------------- §1

// Covers: specs/skills/bodies-4.md §1
#[test]
fn source_of_needs_the_c8_bit_and_local_seeds() {
    let (mut f, u) = world();
    let m = monster(&mut f, (1, 1));
    f.missile_owners.insert(m, u);
    assert_eq!(b4_helpers::source_of(&f, m), None, "bit 0x400 clear");
    f.c8.insert(m, 0x400);
    assert_eq!(b4_helpers::source_of(&f, m), Some(u));
    f.c8.insert(m, 0x3FF);
    assert_eq!(b4_helpers::source_of(&f, m), None);
    // "S" = init_low(x) = {x, 666}.
    assert_eq!(Seed::init_low(9), Seed::new(9, 666));
}

// ---------------------------------------------------------------- §2.1

// Covers: specs/skills/bodies-4.md §2.1
#[test]
fn weapon_roll_is_bonuses_of_the_skill_range() {
    let mut r = body_rec();
    r.mindam = 10;
    r.maxdam = 40;
    let t = tab1(r, Code::new(), 1);
    let (mut f, u) = world();
    let mut rec = DamageRecord::default();
    // R invalid: nothing is rolled.
    b4_helpers::weapon_roll(&mut f, &t, u, 99, 1, &mut rec);
    assert_eq!(rec.physical, 0);
    assert_eq!(f.c.units[u].seed, Seed::new(1, 0));
    // min 10, max 40, no bonus: 10 + roll(30) on the unit's seed; the
    // elemental roll (min = max) draws nothing.
    b4_helpers::weapon_roll(&mut f, &t, u, 1, 1, &mut rec);
    let mut s = Seed::new(1, 0);
    assert_eq!(rec.physical, 10 + s.roll(30) as i32);
    assert_eq!(f.c.units[u].seed, s);
}

// ---------------------------------------------------------------- §2.2

// Covers: specs/skills/bodies-4.md §2.2
#[test]
fn state_list_stacks_a_list_per_call() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.auralencalc = c.f(40);
    r.aurastat1 = 25;
    r.aurastatcalc1 = c.f(7);
    let t = tab1(r, c, 1);
    let (mut f, u) = world();
    let k = mon(&mut f, 0, (1, 1));
    f.c.frame = 100;
    // R invalid: none, nothing allocated.
    assert!(b4_helpers::stack_list(&mut f, &t, u, k, 30, 99, 1).is_none());
    assert!(f.lists.is_empty());
    let l = b4_helpers::stack_list(&mut f, &t, u, k, 30, 1, 3).unwrap();
    let a = &f.lists[l];
    assert_eq!((a.flags, a.expire, a.owner, a.state), (2, 140, Some(u), 30));
    assert_eq!(a.callback, callback::DEFAULT);
    assert_eq!(a.unit, Some(k));
    assert_eq!(a.stats.get(&25), Some(&7), "aura_fill");
    assert!(f.c.units[k].states.contains(&30));
    assert!(logged(&mut f, &format!("timer {k} 12 140")));
    // No existing list is looked for: a second call adds a second one.
    b4_helpers::stack_list(&mut f, &t, u, k, 30, 1, 3).unwrap();
    let n = f
        .lists
        .iter()
        .filter(|l| l.unit == Some(k) && l.state == 30)
        .count();
    assert_eq!(n, 2);
}

// ---------------------------------------------------------------- §2.3

// Covers: specs/skills/bodies-4.md §2.3
#[test]
fn imp_possess_attaches_to_a_tower_or_beast() {
    let ct = ct_ids(500);
    let (mut f, _) = world();
    let imp = mon(&mut f, 492, (1, 1));
    let tower = mon(&mut f, 435, (2, 2));
    let beast = mon(&mut f, 441, (3, 3));
    let other = mon(&mut f, 100, (4, 4));
    f.c.units[imp].flags = 0xFF;
    // Wrong classes: nothing happens.
    b4_helpers::possess(&mut f, &ct, imp, other, 5, 2);
    b4_helpers::possess(&mut f, &ct, other, tower, 5, 2);
    assert!(f.c.log.is_empty() && f.c.units[imp].states.is_empty());
    b4_helpers::possess(&mut f, &ct, imp, tower, 5, 2);
    assert!(f.c.units[imp].states.contains(&143));
    assert_eq!(f.c8.get(&imp).copied().unwrap_or(0) & 0x400, 0x400);
    assert_eq!(f.c8.get(&tower).copied().unwrap_or(0) & 0x400, 0x400);
    assert_eq!(f.c.units[imp].flags, 0xF1, "flags &= ~0xE");
    let log = f.take_log();
    let want = [
        fx(Fx::SourceFields {
            m: imp,
            owner: Some(tower),
        }),
        fx(Fx::SourceFields {
            m: tower,
            owner: Some(imp),
        }),
        format!("deltimers {imp} 5 5"),
        fx(Fx::EveryTick {
            u: imp,
            kind: 5,
            a1: 5,
            a2: 2,
        }),
        format!("ai {imp} 16"),
    ];
    let at = |s: &String| log.iter().position(|l| l == s).expect(s);
    let pos: Vec<usize> = want.iter().map(at).collect();
    assert!(pos.windows(2).all(|w| w[0] < w[1]), "order {pos:?}");
    // A siege beast works too.
    b4_helpers::possess(&mut f, &ct, imp, beast, 5, 2);
    assert!(f.take_log().contains(&format!("ai {imp} 16")));
}

// Covers: specs/skills/bodies-4.md §2.3
#[test]
fn imp_release_kills_below_ten_percent() {
    let ct = ct_ids(500);
    let (mut f, _) = world();
    let imp = mon(&mut f, 492, (1, 1));
    let tower = mon(&mut f, 435, (2, 2));
    f.c.units[imp].flags = 0x1;
    f.c8.insert(imp, 0x400);
    f.c8.insert(tower, 0x400);
    f.c.units[imp].states.push(143);
    // Max life 100 << 8; life 9 %: killed, no AI change.
    f.c.set(imp, 7, 100 << 8);
    f.c.set(imp, 6, 9 << 8);
    b4_helpers::release(&mut f, &ct, imp, tower, 5);
    let log = f.take_log();
    assert!(log.contains(&fx(Fx::KillBy {
        u: imp,
        killer: None,
        b: 1
    })));
    assert!(!log
        .iter()
        .any(|l| l.starts_with("ai ") || l.starts_with("deltimers")));
    assert!(!f.c.units[imp].states.contains(&143), "state 143 off");
    assert_eq!(f.c8[&imp] & 0x400, 0);
    assert_eq!(f.c8[&tower] & 0x400, 0);
    assert_eq!(f.c.units[imp].flags, 1, "flags untouched");
    // A maximum below 256 reads as 0 %: killed as well.
    f.c.set(imp, 7, 255);
    f.c.set(imp, 6, 255);
    b4_helpers::release(&mut f, &ct, imp, tower, 5);
    assert!(f.take_log().contains(&fx(Fx::KillBy {
        u: imp,
        killer: None,
        b: 1
    })));
    // Wrong class pair: nothing.
    let other = mon(&mut f, 100, (3, 3));
    b4_helpers::release(&mut f, &ct, imp, other, 5);
    assert!(f.take_log().is_empty());
}

// Covers: specs/skills/bodies-4.md §2.3
#[test]
fn imp_release_at_ten_percent_frees_the_imp() {
    let ct = ct_ids(500);
    let (mut f, _) = world();
    let imp = mon(&mut f, 492, (1, 1));
    let beast = mon(&mut f, 441, (2, 2));
    f.c.units[imp].flags = 0x1;
    f.c.set(imp, 7, 100 << 8);
    f.c.set(imp, 6, 10 << 8);
    b4_helpers::release(&mut f, &ct, imp, beast, 5);
    let log = f.take_log();
    assert_eq!(f.c.units[imp].flags, 0xF, "flags |= 0xE");
    assert!(log.contains(&format!("deltimers {imp} 5 5")));
    assert!(log.contains(&format!("ai {imp} 0")));
    assert!(!log.iter().any(|l| l.contains("KillBy")));
    // The percent is trunc(100·(life >> 8) / (max >> 8)).
    f.c.set(imp, 6, (10 << 8) - 1);
    assert_eq!(b4_helpers::life_percent(&f, imp), 9);
}

// ---------------------------------------------------------------- §2.4

/// A missile that records what the lightning callbacks write.
#[derive(Default)]
struct PM {
    frames: i32,
    seed: Seed,
    pos: (i32, i32),
    tgt: (i32, i32),
    dir: i32,
    log: Vec<String>,
}

impl JitterMissile for PM {
    type Missile = ();
    fn total_frames(&self, _: ()) -> i32 {
        self.frames
    }
    fn set_frames(&mut self, _: (), v: i32) {
        self.log.push(format!("frames {v}"));
    }
    fn path_target_x(&self, _: ()) -> i32 {
        self.tgt.0
    }
    fn set_seed(&mut self, _: (), s: Seed) {
        self.seed = s;
        self.log.push(format!("seed {} {}", s.lo, s.hi));
    }
    fn set_path_type(&mut self, _: (), ty: u8) {
        self.log.push(format!("type {ty}"));
    }
    fn set_steps(&mut self, _: (), n: i32) {
        self.log.push(format!("steps {n}"));
    }
    fn compute_path(&mut self, _: ()) {
        self.log.push("compute".into());
    }
}

impl PathMissile for PM {
    fn missile_seed(&mut self, _: ()) -> &mut Seed {
        &mut self.seed
    }
    fn missile_position(&self, _: ()) -> (i32, i32) {
        self.pos
    }
    fn path_target(&self, _: ()) -> (i32, i32) {
        self.tgt
    }
    fn missile_dir64(&self, _: (), at: (i32, i32)) -> i32 {
        assert_eq!(at, self.tgt, "dir64 toward the path's target point");
        self.dir
    }
    fn set_point(&mut self, _: (), i: i32, at: (u16, u16)) {
        self.log.push(format!("pt {i} {} {}", at.0, at.1));
    }
    fn set_point_count(&mut self, _: (), n: i32) {
        self.log.push(format!("count {n}"));
    }
}

// Covers: specs/skills/bodies-4.md §2.4 text
#[test]
fn zigzag_callback_writes_points_and_nothing_else() {
    let (a, n, dir) = (5u32, 20, 10);
    let mut m = PM {
        frames: n,
        pos: (100, 200),
        tgt: (130, 170),
        dir,
        ..PM::default()
    };
    zigzag_cb(&mut m, (), a);
    // Hand run of steps 2–6.
    let mut s = Seed::init_low(a);
    let mut sign = if s.step() & 1 == 1 { 1 } else { -1 };
    let mut k = (s.step() % 3) as i32 + 2;
    let (mut d, mut x, mut y) = (dir, 100, 200);
    let mut want = vec!["type 10".to_string(), format!("steps {n}")];
    const X: [i32; 8] = [2, 2, 0, -2, -2, -2, 0, 2];
    const Y: [i32; 8] = [0, 2, 2, 2, 0, -2, -2, -2];
    for i in 0..n {
        d += k * sign;
        let e = (((d & 0x3F) + 4) >> 3) & 7;
        x += X[e as usize];
        y += Y[e as usize];
        want.push(format!("pt {i} {x} {y}"));
        if i % 15 == 0 {
            sign = -sign;
            k = (s.step() % 3) as i32 + 2;
        }
    }
    want.push(format!("count {n}"));
    assert_eq!(m.log, want);
    assert_eq!(m.seed, s, "the missile seed ends where the hand run does");
    // No compute-path call; frames under 78 are left alone.
    assert!(!m
        .log
        .iter()
        .any(|l| l == "compute" || l.starts_with("frames")));
    // 90 frames: capped at 77, written as total and left.
    let mut m = PM {
        frames: 90,
        tgt: (30, 30),
        ..PM::default()
    };
    zigzag_cb(&mut m, (), 1);
    assert_eq!(m.log[0], "frames 77");
    assert!(m.log.contains(&"count 77".to_string()));
}

/// Tables with skill 280 (Royal Strike) carrying `Param2` = 7.
fn royal_tabs() -> SkillTables {
    let mut t = tab1(body_rec(), Code::new(), 600);
    t.skills.resize(281, body_rec());
    t.skills[280].param2 = 7;
    t
}

// Covers: specs/skills/bodies-4.md §2.4 r1, §2.4 r2, §2.4 r3
#[test]
fn lightning_fan_records_and_data() {
    let t = royal_tabs();
    let (mut f, u) = world();
    // Class 568 with step 16: i = 0, 16, 32, 48.
    b4_helpers::zigzag(&mut f, &t, u, 568, (40, 50), 3, 2, 16, 77, false);
    assert_eq!(f.missiles.len(), 4);
    let mut s = Seed::init_low(77);
    for (n, i) in [0usize, 16, 32, 48].into_iter().enumerate() {
        let r = &f.missiles[n];
        assert_eq!((r.flags, r.owner, r.class), (3, u, 568));
        assert_eq!((r.x, r.y, r.skill, r.level), (40, 50, 3, 2));
        let (ox, oy) = ring_offset(i);
        assert_eq!((r.target_x, r.target_y), (ox, oy));
        assert_eq!(r.init, Some((init_cb::ZIGZAG, s.step())), "one S step each");
    }
    // Class 568: data +0x28 := Param2 of skill 280 + 1 on each missile.
    let log = f.take_log();
    let d28 = |m: usize| fx(Fx::MissileData28 { missile: m, v: 8 });
    assert_eq!(
        log.iter()
            .filter(|l| l.starts_with("MissileData28"))
            .count(),
        4
    );
    assert!(log.contains(&d28(4)));
    // Another class: no data write; step 8 gives 8 missiles.
    f.missiles.clear();
    b4_helpers::zigzag(&mut f, &t, u, 100, (40, 50), 3, 2, 8, 77, false);
    assert_eq!(f.missiles.len(), 8);
    assert!(!f.take_log().iter().any(|l| l.starts_with("MissileData28")));
}

// Covers: specs/skills/bodies-4.md §edge-cases-original-bugs r1
#[test]
fn lightning_step_zero_or_less_creates_nothing() {
    let t = royal_tabs();
    let (mut f, u) = world();
    b4_helpers::zigzag(&mut f, &t, u, 568, (40, 50), 3, 2, 0, 77, false);
    b4_helpers::zigzag(&mut f, &t, u, 568, (40, 50), 3, 2, -4, 77, true);
    assert!(f.missiles.is_empty());
}

// ---------------------------------------------------------------- §2.6

// Covers: specs/skills/bodies-4.md §2.6
#[test]
fn whip_state_applies_the_target_state() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.auralencalc = c.f(60);
    r.auratargetstate = 90;
    r.aurastat1 = 25;
    r.aurastatcalc1 = c.f(9);
    let t = tab1(r, c, 1);
    let ct = ct_ids(10);
    let (mut f, u) = world();
    let tg = f.add(FUnit::new(UnitType::Player, 0), (3, 3));
    f.c.frame = 10;
    // R invalid: nothing.
    b4_helpers::whip_state(&mut f, &t, &ct, tg, 99, 1, u);
    assert!(f.lists.is_empty());
    b4_helpers::whip_state(&mut f, &t, &ct, tg, 1, 4, u);
    let l = f.list_of(tg, 90).expect("whip list");
    assert_eq!((l.expire, l.skill, l.lvl, l.owner), (70, 1, 4, Some(u)));
    assert_eq!(l.callback, callback::DEFAULT);
    assert_eq!(l.stats.get(&25), Some(&9), "aura_fill");
}

// Covers: specs/skills/bodies-4.md §2.6
#[test]
fn whip_transform_changes_class_and_marks_it() {
    let mut r = body_rec();
    r.summon = 5;
    r.summode = 3;
    r.sumumod = 7;
    let t = tab1(r, Code::new(), 1);
    let mut rows = vec![monster_rec(); 20];
    for (i, m) in rows.iter_mut().enumerate() {
        m.baseid = i as u16;
        m.nextinclass = 0xFFFF;
    }
    rows[5].nextinclass = 9;
    let ct = combat_tables(rows);
    let (mut f, u) = world();
    let tg = mon(&mut f, 12, (3, 3));
    f.chains.insert(12, 1);
    b4_helpers::transform(&mut f, &t, &ct, u, Some(tg), 1);
    // k = 1: the class of 5 followed one step: 9.
    let log = f.take_log();
    let want = [
        fx(Fx::ClassChange {
            t: tg,
            class: 9,
            mode: 3,
        }),
        format!("ai {tg} 15"),
        format!("moderequest {tg} 3 None"),
        fx(Fx::Umod {
            m: tg,
            umod: 7,
            arg: 0,
        }),
    ];
    let pos: Vec<usize> = want
        .iter()
        .map(|s| log.iter().position(|l| l == s).expect(s))
        .collect();
    assert!(pos.windows(2).all(|w| w[0] < w[1]), "{pos:?}");
    assert!(f.c.units[tg].states.contains(&142));
    let l = f.list_of(tg, 142).expect("changeclass list");
    assert_eq!((l.flags, l.expire, l.owner), (0, 0, Some(u)));
    assert_eq!(l.stats.get(&355), Some(&9), "shortparam1 := c'");
}

// Covers: specs/skills/bodies-4.md §2.6
#[test]
fn whip_transform_refusals() {
    let mut r = body_rec();
    r.summon = 5;
    let t = tab1(r, Code::new(), 1);
    let ct = ct_ids(20);
    let (mut f, u) = world();
    // T none, or not a monster: nothing.
    b4_helpers::transform(&mut f, &t, &ct, u, None, 1);
    let p = f.add(FUnit::new(UnitType::Player, 0), (3, 3));
    b4_helpers::transform(&mut f, &t, &ct, u, Some(p), 1);
    // c < 0 (summon none, no spawn class): nothing.
    let mut r2 = body_rec();
    r2.summon = 0xFFFF;
    let t2 = tab1(r2, Code::new(), 1);
    let tg = mon(&mut f, 12, (3, 3));
    b4_helpers::transform(&mut f, &t2, &ct, u, Some(tg), 1);
    // R invalid: nothing.
    b4_helpers::transform(&mut f, &t, &ct, u, Some(tg), 99);
    assert!(f.c.log.is_empty() && f.lists.is_empty());
    // sumumod outside 1…42: no Umod.
    let mut r3 = body_rec();
    r3.summon = 5;
    r3.sumumod = 43;
    let t3 = tab1(r3, Code::new(), 1);
    b4_helpers::transform(&mut f, &t3, &ct, u, Some(tg), 1);
    assert!(!f.take_log().iter().any(|l| l.starts_with("Umod")));
}

// ---------------------------------------------------------------- §2.7

// Covers: specs/skills/bodies-4.md §2.7
#[test]
fn vine_missiles_cycle_four_offsets() {
    let t = tab1(body_rec(), Code::new(), 20);
    let (mut f, u) = world();
    // Target position fails: 0, nothing made.
    assert_eq!(b4_helpers::vines(&mut f, u, 6, 3, 1, 2), 0);
    assert!(f.missiles.is_empty());
    f.tpos.insert(u, (50, 60));
    assert_eq!(b4_helpers::vines(&mut f, u, 6, 3, 1, 2), 1);
    let offs: Vec<(i32, i32)> = f
        .missiles
        .iter()
        .map(|m| (m.target_x, m.target_y))
        .collect();
    assert_eq!(offs, [(0, 1), (0, -1), (1, 0), (-1, 0), (0, 1), (0, -1)]);
    for (i, m) in f.missiles.iter().enumerate() {
        assert_eq!((m.flags, m.owner, m.class), (3, u, 3));
        assert_eq!((m.x, m.y, m.skill, m.level), (50, 60, 1, 2));
        assert_eq!(m.init, Some((init_cb::JITTER, i as u32)));
    }
    let _ = t;
}

// ================================================================ §3

/// A world whose caster (unit 0, guid 0) is a monster that uses skill 1.
fn mworld() -> (BodyFake, usize) {
    let mut f = BodyFake::new();
    let mut m = FUnit::new(UnitType::Monster, 0);
    let e = crate::skills::SkillEntry {
        skill: 1,
        base: 1,
        owner_guid: -1,
        ..crate::skills::SkillEntry::default()
    };
    m.skills.push(e);
    m.used = Some(e);
    let u = f.add(m, (0, 0));
    (f, u)
}

/// The entry of the used skill of `u`.
fn entry(f: &BodyFake, u: usize) -> crate::skills::SkillEntry {
    f.c.units[u].used.unwrap()
}

// ---------------------------------------------------------------- §3.1

// Covers: specs/skills/bodies-4.md §3.1 r1
#[test]
fn mosquito_start_refusals() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(2);
    r.calc2 = c.f(10);
    let t = tab1(r, c, 1);
    let (mut f, u) = world();
    let tg = mon(&mut f, 0, (5, 5));
    f.targets.insert(u, tg);
    // R invalid.
    assert_eq!(b4_more::mosquito_start(&mut f, &t, u, 99, 1), 0);
    // T none.
    f.targets.clear();
    assert_eq!(b4_more::mosquito_start(&mut f, &t, u, 1, 1), 0);
    // E none.
    f.targets.insert(u, tg);
    let e = entry(&f, u);
    f.c.units[u].used = None;
    assert_eq!(b4_more::mosquito_start(&mut f, &t, u, 1, 1), 0);
    f.c.units[u].used = Some(e);
    assert_eq!(f.entry_param(u, &e, 1), 0, "nothing written");
}

// Covers: specs/skills/bodies-4.md §3.1 r2, §3.1 r3, §3.1 r4, §3.1 text
#[test]
fn mosquito_start_bites_depend_on_the_guids() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(2);
    r.calc2 = c.f(10);
    let t = tab1(r, c, 1);
    let (mut f, u) = world();
    let tg = mon(&mut f, 0, (5, 5));
    f.targets.insert(u, tg);
    let e = entry(&f, u);
    assert_eq!(b4_more::mosquito_start(&mut f, &t, u, 1, 1), 1);
    // S = init_low(T GUID 1 + unit GUID 0); n = 2 + roll_S(8).
    let n = 2 + Seed::init_low(1).roll(8) as i32;
    assert_eq!(f.entry_param(u, &e, 1), n);
    assert_eq!(f.entry_param(u, &e, 2), 1, "T GUID");
    assert_eq!(f.entry_param(u, &e, 3), UnitType::Monster.index() as i32);
    // The same GUIDs give the same count; the unit's own seed is not used.
    f.c.units[u].seed = Seed::new(77, 5);
    b4_more::mosquito_start(&mut f, &t, u, 1, 1);
    assert_eq!(f.entry_param(u, &e, 1), n);
    assert_eq!(f.c.units[u].seed, Seed::new(77, 5));
    // b ≤ a: no draw, n = a.
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(5);
    r.calc2 = c.f(3);
    let t2 = tab1(r, c, 1);
    b4_more::mosquito_start(&mut f, &t2, u, 1, 1);
    assert_eq!(f.entry_param(u, &e, 1), 5);
    // n < 1 → 1.
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(-3);
    r.calc2 = c.f(-3);
    let t3 = tab1(r, c, 1);
    b4_more::mosquito_start(&mut f, &t3, u, 1, 1);
    assert_eq!(f.entry_param(u, &e, 1), 1);
}

// ---------------------------------------------------------------- §3.2

fn bite_tabs() -> SkillTables {
    let mut c = Code::new();
    let mut r = body_rec();
    r.mindam = 3 << 8;
    r.maxdam = 7 << 8;
    r.elen = 25;
    r.calc3 = c.f(50);
    r.param1 = 6;
    tab1(r, c, 1)
}

/// A bite world: the player caster, a monster K (guid 1) in range with
/// the bite entry (3 bites left on K).
fn bite_world(n: i32) -> (BodyFake, usize, usize) {
    let (mut f, u) = world();
    let k = mon(&mut f, 0, (1, 1));
    f.c.in_range = true;
    f.c.hostile = true;
    f.c.set(u, 6, 1000);
    f.c.set(u, 7, 100_000);
    f.c.set(k, 6, 1 << 20);
    f.c.set(k, 8, 1 << 24);
    f.c.set(k, 10, 1 << 24);
    let e = entry(&f, u);
    f.set_entry_param_of(u, &e, 1, n);
    f.set_entry_param_of(u, &e, 2, 1);
    f.set_entry_param_of(u, &e, 3, UnitType::Monster.index() as i32);
    (f, u, k)
}

// Covers: specs/skills/bodies-4.md §3.2 r1, §3.2 r2
#[test]
fn mosquito_refusals() {
    let t = bite_tabs();
    let ct = ct_ids(2);
    let (mut f, u, _) = bite_world(2);
    assert_eq!(b4_more::mosquito(&mut f, &t, &ct, u, 99, 1), 0, "R invalid");
    // Not in melee range.
    f.c.in_range = false;
    assert_eq!(b4_more::mosquito(&mut f, &t, &ct, u, 1, 1), 0);
    f.c.in_range = true;
    // K gone (wrong GUID).
    let e = entry(&f, u);
    f.set_entry_param_of(u, &e, 2, 9);
    assert_eq!(b4_more::mosquito(&mut f, &t, &ct, u, 1, 1), 0);
    f.set_entry_param_of(u, &e, 2, 1);
    // E none.
    f.c.units[u].used = None;
    assert_eq!(b4_more::mosquito(&mut f, &t, &ct, u, 1, 1), 0);
    assert_eq!(f.c.units[u].seed, Seed::new(1, 0), "no draw");
}

// Covers: specs/skills/bodies-4.md §3.2 r3, §3.2 r4, §3.2 r5, §3.2 r6, §3.2 r7
#[test]
fn mosquito_bite_rolls_and_drains() {
    let t = bite_tabs();
    let ct = ct_ids(2);
    let (mut f, u, k) = bite_world(1);
    assert_eq!(b4_more::mosquito(&mut f, &t, &ct, u, 1, 1), 1);
    // a = 3, b = 7 (the ranges >> 8); the physical roll is 768 + roll(1024).
    let mut s = Seed::new(1, 0);
    let phys = 768 + s.roll(1024) as i32;
    // Three draws roll(4): poison, mana leech, stamina leech.
    let _poison = 2 * (3 + s.roll(4) as i32);
    let ml = (3 + s.roll(4) as i32) << 8;
    let sl = (3 + s.roll(4) as i32) << 8;
    // Step 6: life + pct(physical, calc3 = 50, 100).
    assert_eq!(f.c.get(u, 6), 1000 + phys * 50 / 100);
    // Step 7: apply(unit, K): the stamina and mana leech are drained from
    // K (the mana one after the damage code's leech shift << 6).
    assert_eq!(f.c.get(k, 10), (1 << 24) - sl);
    assert_eq!(f.c.get(k, 8), (1 << 24) - (ml << 6));
    // The reaction runs.
    assert!(logged(&mut f, &format!("reaction {u} {k}")));
    // One bite: the frame data is left alone.
    assert_eq!(f.frame_index.get(&u), None);
    assert_eq!(f.frame_count.get(&u), None);
    assert_eq!(f.c.units[u].seed, s);
}

// Covers: specs/skills/bodies-4.md §3.2 r6
#[test]
fn mosquito_life_is_capped_at_the_maximum() {
    let t = bite_tabs();
    let ct = ct_ids(2);
    let (mut f, u, _) = bite_world(1);
    f.c.set(u, 6, 99_990);
    b4_more::mosquito(&mut f, &t, &ct, u, 1, 1);
    assert_eq!(f.c.get(u, 6), 100_000);
}

// Covers: specs/skills/bodies-4.md §3.2 r8
#[test]
fn mosquito_repeats_while_bites_are_left() {
    let t = bite_tabs();
    let ct = ct_ids(2);
    let (mut f, u, _) = bite_world(3);
    f.frame_count.insert(u, 0x350);
    assert_eq!(b4_more::mosquito(&mut f, &t, &ct, u, 1, 1), 1);
    let e = entry(&f, u);
    assert_eq!(f.entry_param(u, &e, 1), 2);
    assert_eq!(f.frame_index[&u], 6, "Param1");
    assert_eq!(f.frame_count[&u], 0x400, "(count & ~0xFF) + 0x100");
}

// ---------------------------------------------------------------- §3.3

/// The first seed `Seed::new(lo, 0)` whose next step mod 5 is `c`.
fn curse_seed(c: u32) -> Seed {
    (0u32..)
        .map(|lo| Seed::new(lo, 0))
        .find(|s| s.clone().step() % 5 == c)
        .unwrap()
}

/// Tables of 92 skills: skills 66, 72, 82, 87, 91 are curses with the
/// states 100…104 and range 50 / length 30 + c.
fn curse_tabs(p7: i16) -> SkillTables {
    let mut c = Code::new();
    let mut t = tab1(body_rec(), Code::new(), 1);
    t.skills.resize(92, body_rec());
    for (i, k) in [66usize, 72, 82, 87, 91].into_iter().enumerate() {
        let mut r = body_rec();
        r.auratargetstate = 100 + i as u16;
        r.aurarangecalc = c.f(50);
        r.auralencalc = c.f(30 + i as i16);
        r.param7 = p7 as _;
        t.skills[k] = r;
    }
    t.skills_code = c.0;
    t
}

/// A caster seeded for curse `c`, a hostile player X at the aim point
/// and one far away.
fn curse_world(c: u32) -> (BodyFake, usize, usize, usize) {
    let (mut f, u) = world();
    f.c.units[u].seed = curse_seed(c);
    f.c.hostile = true;
    f.tpos.insert(u, (10, 10));
    let x = f.add(FUnit::new(UnitType::Player, 0), (10, 10));
    let far = f.add(FUnit::new(UnitType::Player, 0), (90, 90));
    f.c.units[x].flags = 0xE;
    f.c.units[far].flags = 0xE;
    f.scan = vec![x, far];
    f.c.frame = 1000;
    (f, u, x, far)
}

// Covers: specs/skills/bodies-4.md §3.3 r1, §3.3 r2, §3.3 r3, §3.3 r5, §3.3 text
#[test]
fn curse_cast_picks_by_the_unit_seed_and_scans_the_point() {
    let ct = ct_ids(2);
    let t = curse_tabs(0);
    for (c, state) in [(0u32, 100), (1, 101)] {
        let (mut f, u, x, far) = curse_world(c);
        let mut s = f.c.units[u].seed;
        s.step();
        assert_eq!(b4_more::mon_curse_cast(&mut f, &t, &ct, u, 4), 1);
        assert_eq!(f.c.units[u].seed, s, "one step");
        assert_eq!(f.c.units[u].flags & 0x40, 0x40, "flags |= 0x40");
        // Duration d = auralencalc (30 + c) from the frame; the range r
        // = 50 keeps the far unit out.
        let l = f.list_of(x, state).expect("curse list");
        assert_eq!(l.expire, 1000 + 30 + c as i32);
        assert_eq!((l.skill, l.lvl), ([66, 72][c as usize], 4));
        assert!(f.list_of(far, state).is_none());
    }
}

// Covers: specs/skills/bodies-4.md §3.3 r4
#[test]
fn curse_cast_contexts_by_curse() {
    let ct = ct_ids(2);
    let t = curse_tabs(20);
    // c = 0 Amplify Damage: stat 36 = −100. c = 1 Weaken: stat 25 = −50.
    let (mut f, u, x, _) = curse_world(0);
    b4_more::mon_curse_cast(&mut f, &t, &ct, u, 4);
    assert_eq!(f.list_of(x, 100).unwrap().stats.get(&36), Some(&-100));
    let (mut f, u, x, _) = curse_world(1);
    b4_more::mon_curse_cast(&mut f, &t, &ct, u, 4);
    assert_eq!(f.list_of(x, 101).unwrap().stats.get(&25), Some(&-50));
    // c = 2 Life Tap: event 1 = 5, function 1 = 4.
    let (mut f, u, x, _) = curse_world(2);
    b4_more::mon_curse_cast(&mut f, &t, &ct, u, 4);
    let h: Vec<_> = f.handlers[&x]
        .iter()
        .map(|h| (h.event, h.func, h.skill))
        .collect();
    assert_eq!(h, [(5, 4, 82)]);
    // c = 3 Decrepify: (1, 5) and (2, 5); the third pair is empty.
    let (mut f, u, x, _) = curse_world(3);
    b4_more::mon_curse_cast(&mut f, &t, &ct, u, 4);
    let mut h: Vec<_> = f.handlers[&x].iter().map(|h| (h.event, h.func)).collect();
    h.sort();
    assert_eq!(h, [(1, 5), (2, 5)]);
    assert!(f.list_of(x, 103).is_some());
    // c = 4 Lower Resist: upd; 67 := Param7, 25 := −50, 36 := −50,
    // 68 := Param7.
    let (mut f, u, x, _) = curse_world(4);
    b4_more::mon_curse_cast(&mut f, &t, &ct, u, 4);
    let l = f.list_of(x, 104).unwrap();
    assert_eq!(l.stats.get(&67), Some(&20));
    assert_eq!(l.stats.get(&25), Some(&-50));
    assert_eq!(l.stats.get(&36), Some(&-50));
    assert_eq!(l.stats.get(&68), Some(&20));
    assert!(logged(&mut f, &format!("anim {x}")), "upd");
}

// Covers: specs/skills/bodies-4.md §edge-cases-original-bugs r2
#[test]
fn curse_cast_lower_resist_with_param7_zero_applies_nothing() {
    let ct = ct_ids(2);
    let t = curse_tabs(0);
    let (mut f, u, x, _) = curse_world(4);
    assert_eq!(b4_more::mon_curse_cast(&mut f, &t, &ct, u, 4), 1);
    assert!(f.list_of(x, 104).is_none(), "slot-1 value 0: unit skipped");
    // Skill 87's state carries event handlers only (no stats).
    let (mut f, u, x, _) = curse_world(3);
    b4_more::mon_curse_cast(&mut f, &t, &ct, u, 4);
    let l = f.list_of(x, 103).unwrap();
    assert!(l.stats.is_empty());
    assert_eq!(f.handlers[&x].len(), 2);
}

// Covers: specs/skills/bodies-4.md §3.3 r1
#[test]
fn curse_cast_refuses_a_missing_curse_or_state() {
    let ct = ct_ids(2);
    // Skill 91 (c = 4) missing: only 80 skills.
    let mut t = curse_tabs(0);
    t.skills.truncate(80);
    let (mut f, u, _, _) = curse_world(4);
    assert_eq!(b4_more::mon_curse_cast(&mut f, &t, &ct, u, 4), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0);
    // The curse state outside 0…count − 1.
    let mut t = curse_tabs(0);
    t.skills[66].auratargetstate = 500;
    let (mut f, u, _, _) = curse_world(0);
    assert_eq!(b4_more::mon_curse_cast(&mut f, &t, &ct, u, 4), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0);
}

// ---------------------------------------------------------------- §3.4

// Covers: specs/skills/bodies-4.md §3.4 r1
#[test]
fn regurgitator_refuses_what_is_not_a_dead_monster() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(50);
    let t = tab1(r, c, 1);
    let (mut f, u) = mworld();
    assert_eq!(
        b4_more::regurgitator_eat(&mut f, &t, u, 99, 1),
        0,
        "R invalid"
    );
    let tg = mon(&mut f, 0, (2, 2));
    // T none.
    assert_eq!(b4_more::regurgitator_eat(&mut f, &t, u, 1, 1), 0);
    f.targets.insert(u, tg);
    // Alive (mode 1).
    assert_eq!(b4_more::regurgitator_eat(&mut f, &t, u, 1, 1), 0);
    // Dead hireling.
    f.c.units[tg].mode = 12;
    f.c.units[tg].hireling = true;
    assert_eq!(b4_more::regurgitator_eat(&mut f, &t, u, 1, 1), 0);
    // Dead, not a monster.
    let p = f.add(FUnit::new(UnitType::Player, 0), (3, 3));
    f.c.units[p].mode = 12;
    f.targets.insert(u, p);
    assert_eq!(b4_more::regurgitator_eat(&mut f, &t, u, 1, 1), 0);
    assert!(f.c.log.is_empty(), "nothing removed");
}

// Covers: specs/skills/bodies-4.md §3.4 r2, §3.4 r3
#[test]
fn regurgitator_eats_the_corpse_and_heals() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(50);
    let t = tab1(r, c, 1);
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 0, (2, 2));
    f.targets.insert(u, tg);
    f.c.units[tg].mode = 12;
    f.c.set(tg, 7, 1000);
    f.c.set(u, 6, 100);
    f.c.set(u, 7, 10_000);
    assert_eq!(b4_more::regurgitator_eat(&mut f, &t, u, 1, 1), 1);
    // The room notice, then the removal, in that order.
    let log = f.take_log();
    let a = log
        .iter()
        .position(|l| *l == fx(Fx::RoomDelete(tg)))
        .unwrap();
    let b = log
        .iter()
        .position(|l| *l == fx(Fx::RemoveUnit(tg)))
        .unwrap();
    assert!(a < b);
    // h + pct(M_T, 50, 100) = 100 + 500.
    assert_eq!(f.c.get(u, 6), 600);
    // Capped at the maximum.
    f.c.set(u, 7, 300);
    f.c.set(u, 6, 100);
    b4_more::regurgitator_eat(&mut f, &t, u, 1, 1);
    assert_eq!(f.c.get(u, 6), 300);
    // At least 1.
    f.c.set(u, 7, 300);
    f.c.set(u, 6, 0);
    f.c.set(tg, 7, 1);
    b4_more::regurgitator_eat(&mut f, &t, u, 1, 1);
    assert_eq!(f.c.get(u, 6), 1);
    // M_T = 0: removed, life untouched, still 1.
    f.c.set(u, 6, 77);
    f.c.set(tg, 7, 0);
    assert_eq!(b4_more::regurgitator_eat(&mut f, &t, u, 1, 1), 1);
    assert_eq!(f.c.get(u, 6), 77);
}

// ---------------------------------------------------------------- §3.5

// Covers: specs/skills/bodies-4.md §3.5 r1
#[test]
fn wake_sentry_refusals() {
    let mut r = body_rec();
    r.srvmissilea = 2;
    let t = tab1(r, Code::new(), 3);
    let (mut f, u) = world();
    f.tpos.insert(u, (40, 35));
    assert_eq!(b4_more::wake_of_destruction(&mut f, &t, u, 99, 1), 0);
    // m outside 0…count − 1.
    let mut r = body_rec();
    r.srvmissilea = 3;
    let t3 = tab1(r, Code::new(), 3);
    assert_eq!(b4_more::wake_of_destruction(&mut f, &t3, u, 1, 1), 0);
    assert!(f.missiles.is_empty());
    // m = 0 is allowed (0 ≤ m).
    let mut r = body_rec();
    r.srvmissilea = 0;
    let t0 = tab1(r, Code::new(), 3);
    assert_eq!(b4_more::wake_of_destruction(&mut f, &t0, u, 1, 1), 1);
    assert_eq!(f.missiles[0].class, 0);
}

// Covers: specs/skills/bodies-4.md §3.5 r2, §3.5 r3, §3.5 r4
#[test]
fn wake_sentry_aims_a_perpendicular_missile() {
    let mut r = body_rec();
    r.srvmissilea = 2;
    let t = tab1(r, Code::new(), 3);
    let (mut f, u) = world();
    f.pos.insert(u, (10, 20));
    // Target position fails (coordinate 0): 0, flags untouched.
    f.tpos.insert(u, (0, 35));
    assert_eq!(b4_more::wake_of_destruction(&mut f, &t, u, 1, 3), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0);
    f.tpos.insert(u, (40, 35));
    let mm = f.c.units.len();
    assert_eq!(b4_more::wake_of_destruction(&mut f, &t, u, 1, 3), 1);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    let q = &f.missiles[0];
    assert_eq!((q.flags, q.owner, q.origin, q.class), (2, u, Some(u), 2));
    assert_eq!((q.skill, q.level), (1, 3));
    assert_eq!((q.target_x, q.target_y), (30, 15), "(tx − ux, ty − uy)");
    let log = f.take_log();
    assert!(log.contains(&fx(Fx::MissileData28 {
        missile: mm,
        v: -15
    })));
    assert!(log.contains(&fx(Fx::MissileData2C { missile: mm, v: 30 })));
    // No missile made: 0.
    f.no_missiles = true;
    assert_eq!(b4_more::wake_of_destruction(&mut f, &t, u, 1, 3), 0);
}

// ---------------------------------------------------------------- §3.6

// Covers: specs/skills/bodies-4.md §3.6
#[test]
fn imp_inferno_start_uses_calc1() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(5);
    r.calc2 = c.f(9);
    let t = tab1(r, c, 1);
    let (mut f, u) = mworld();
    f.c.frame = 100;
    let e = entry(&f, u);
    assert_eq!(
        b4_more::imp_inferno_start(&mut f, &t, u, 99, 1),
        0,
        "R invalid"
    );
    f.c.units[u].used = None;
    assert_eq!(b4_more::imp_inferno_start(&mut f, &t, u, 1, 1), 0, "E none");
    f.c.units[u].used = Some(e);
    assert_eq!(b4_more::imp_inferno_start(&mut f, &t, u, 1, 1), 1);
    assert_eq!(f.entry_param(u, &e, 1), 105, "F + calc1, not calc2");
    assert!(f.c.units[u].states.contains(&12), "state 12 on");
    // calc1 < 1 → at least 1; state 12 already on is not set again.
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(0);
    let t0 = tab1(r, c, 1);
    f.take_log();
    b4_more::imp_inferno_start(&mut f, &t0, u, 1, 1);
    assert_eq!(f.entry_param(u, &e, 1), 101);
    assert!(!f.take_log().iter().any(|l| l.starts_with("state")));
}

// ---------------------------------------------------------------- §3.7

fn inferno_tabs(m: u16) -> SkillTables {
    let mut c = Code::new();
    let mut r = body_rec();
    r.srvmissilea = m;
    r.calc2 = c.f(6);
    r.calc3 = c.f(4);
    let mut t = tab1(r, c, 3);
    t.missiles[2].vel = 10;
    t.missiles[2].vellev = 16;
    t.missiles[2].param2 = 4;
    t
}

// Covers: specs/skills/bodies-4.md §3.7 r1
#[test]
fn imp_inferno_refusals() {
    let ct = ct_ids(2);
    let (mut f, u) = mworld();
    let t = inferno_tabs(2);
    assert_eq!(
        b4_more::imp_inferno(&mut f, &t, &ct, u, 99, 1),
        0,
        "R invalid"
    );
    for m in [0u16, 3] {
        let t = inferno_tabs(m);
        assert_eq!(b4_more::imp_inferno(&mut f, &t, &ct, u, 1, 1), 0, "m = {m}");
    }
    f.c.units[u].used = None;
    assert_eq!(b4_more::imp_inferno(&mut f, &t, &ct, u, 1, 1), 0, "E none");
    assert!(f.missiles.is_empty());
}

// Covers: specs/skills/bodies-4.md §3.7 r2, §3.7 r3, §3.7 r4, §3.7 r5, §3.7 r6
#[test]
fn imp_inferno_fires_and_keeps_channeling() {
    let mut ct = ct_ids(2);
    ct.monstats2[0].infernoanim = 5;
    let t = inferno_tabs(2);
    let (mut f, u) = mworld();
    f.c.frame = 50;
    f.pos.insert(u, (8, 9));
    let tg = mon(&mut f, 0, (30, 40));
    f.targets.insert(u, tg);
    f.path_target = (30, 40);
    let e = entry(&f, u);
    f.set_entry_param_of(u, &e, 1, 60);
    f.c.units[u].states.push(12);
    let mm = f.c.units.len();
    assert_eq!(b4_more::imp_inferno(&mut f, &t, &ct, u, 1, 3), 1);
    let log = f.take_log();
    // Step 2: path target point := T's position; path target := T.
    let i = log
        .iter()
        .position(|l| *l == format!("path {u} TargetPoint(30, 40)"));
    let j = log
        .iter()
        .position(|l| *l == format!("path {u} TargetUnit(Some({tg}))"));
    assert!(i.unwrap() < j.unwrap());
    // Step 3: flags 0x25, velocity Vel + VelLev·L/8 = 10 + 6, target
    // := the path's target point.
    let q = &f.missiles[0];
    assert_eq!((q.flags, q.owner, q.class), (0x25, u, 2));
    assert_eq!((q.x, q.y, q.target_x, q.target_y), (8, 9, 30, 40));
    assert_eq!((q.skill, q.level, q.velocity), (1, 3, 16));
    // Step 4: n = calc2 + Param2 = 10 as the step counts and frames.
    assert!(log.contains(&format!("path {mm} Steps(10)")));
    assert_eq!(f.mframes[&mm], (10, 10));
    // Step 5: the Inferno animation with R (`MonAnim` ≠ 14: the frame
    // slot gets `InfernoAnim` << 8).
    assert_eq!(f.anim_frame[&u], 5 << 8);
    // Step 6: F < E param 1 and state 12: type-1 timers deleted, type-0
    // timer at F + 3.
    let a = log
        .iter()
        .position(|l| *l == format!("deltimers {u} 1 0"))
        .unwrap();
    let b = log
        .iter()
        .position(|l| *l == format!("schedule {u} 0 53 0 0"))
        .unwrap();
    assert!(a < b);
}

// Covers: specs/skills/bodies-4.md §3.7 r7, §edge-cases-original-bugs r9
#[test]
fn imp_inferno_end_schedules_an_ai_think() {
    let ct = ct_ids(2);
    let t = inferno_tabs(2);
    let (mut f, u) = mworld();
    f.c.frame = 50;
    f.tpos.insert(u, (30, 40));
    let e = entry(&f, u);
    // F ≥ E param 1: the channel is over, but state 12 stays on.
    f.set_entry_param_of(u, &e, 1, 50);
    f.c.units[u].states.push(12);
    assert_eq!(b4_more::imp_inferno(&mut f, &t, &ct, u, 1, 3), 1);
    let log = f.take_log();
    assert!(log.contains(&format!("deltimers {u} 0 0")));
    assert!(
        log.contains(&format!("schedule {u} 2 63 0 0")),
        "type 2 at F + 13"
    );
    assert!(f.c.units[u].states.contains(&12));
    assert!(!log.iter().any(|l| l.starts_with("state")));
    // The same without the state.
    f.c.units[u].states.clear();
    f.set_entry_param_of(u, &e, 1, 500);
    f.take_log();
    b4_more::imp_inferno(&mut f, &t, &ct, u, 1, 3);
    assert!(f.take_log().contains(&format!("schedule {u} 2 63 0 0")));
}

// ---------------------------------------------------------------- §3.8

fn soft_tables() -> CombatTables {
    let mut ms2: d2_data::tables::Monstats2 = crate::skills::fake::blank();
    ms2.corpsesel = true;
    ms2.soft = true;
    let mut ct = combat_tables(vec![monster_rec()]);
    ct.monstats2 = vec![ms2];
    ct
}

// Covers: specs/skills/bodies-4.md §3.8 r1, §3.8 r2, §3.8 text
#[test]
fn baal_corpse_explode_runs_skill_285_on_each_corpse() {
    let ct = soft_tables();
    let mut t = tab1(body_rec(), Code::new(), 1);
    t.skills.resize(286, body_rec());
    t.skills[1].param5 = 4;
    t.skills[1].param6 = 3;
    t.skills[285].param1 = 20;
    t.skills[285].param2 = 5;
    let (mut f, u) = mworld();
    f.pos.insert(u, (12, 13));
    let c1 = mon(&mut f, 0, (13, 13));
    f.c.units[c1].mode = 12;
    f.found = vec![c1];
    f.targets.insert(u, c1);
    assert_eq!(b4_more::baal_corpse_explode(&mut f, &t, &ct, u, 1, 3), 1);
    // Radius Param5 + (L − 1)·Param6 = 10 at the unit's position,
    // flags 0x3002.
    assert_eq!(f.finds.borrow().as_slice(), [((12, 13), 10, 0x3002)]);
    // The callback: path target := the corpse, then srvdo 55 on skill
    // 285 (it marks the corpse).
    let log = f.take_log();
    assert!(log.contains(&format!("path {u} TargetUnit(Some({c1}))")));
    assert!(f.c.units[c1].states.contains(&104));
    // R invalid: radius 0 → skill 285's Param1 + (L − 1)·Param2 = 30.
    f.finds.borrow_mut().clear();
    b4_more::baal_corpse_explode(&mut f, &t, &ct, u, 99, 3);
    assert_eq!(f.finds.borrow()[0].1, 30);
    // L = 0: radius 0 → 20 + (0 − 1)·5 = 15.
    f.finds.borrow_mut().clear();
    b4_more::baal_corpse_explode(&mut f, &t, &ct, u, 1, 0);
    assert_eq!(f.finds.borrow()[0].1, 15);
}

// ---------------------------------------------------------------- §3.9

fn suck_tabs(resultflags: u16, hitflags: u32, hitclass: u8) -> SkillTables {
    let mut c = Code::new();
    let mut r = body_rec();
    r.mindam = 10;
    r.maxdam = 40;
    r.calc1 = c.f(50);
    r.calc2 = c.f(12);
    r.calc3 = c.f(7);
    r.resultflags = resultflags as _;
    r.hitflags = hitflags as _;
    r.hitclass = hitclass as _;
    tab1(r, c, 1)
}

// Covers: specs/skills/bodies-4.md §3.9 r1
#[test]
fn suck_blood_start_refusals() {
    let t = suck_tabs(3, 0, 0);
    let ct = ct_ids(2);
    let (mut f, u) = world();
    let tg = mon(&mut f, 0, (2, 2));
    f.targets.insert(u, tg);
    assert_eq!(b4_more::suck_blood_start(&mut f, &t, &ct, u, 99, 1), 0);
    f.targets.clear();
    assert_eq!(b4_more::suck_blood_start(&mut f, &t, &ct, u, 1, 1), 0);
}

// Covers: specs/skills/bodies-4.md §3.9 r2, §3.9 r3
#[test]
fn suck_blood_start_miss_adds_no_entry() {
    // ResultFlags 2 without the hit bit: the result is the flags (a miss).
    let t = suck_tabs(2, 0, 0);
    let ct = ct_ids(2);
    let (mut f, u) = world();
    let tg = mon(&mut f, 0, (2, 2));
    f.targets.insert(u, tg);
    assert_eq!(b4_more::suck_blood_start(&mut f, &t, &ct, u, 1, 1), 1);
    assert!(f.c.units[u].combat.is_empty());
    assert_eq!(f.c.units[u].seed, Seed::new(1, 0), "no roll on a miss");
}

// Covers: specs/skills/bodies-4.md §3.9 r4
#[test]
fn suck_blood_start_hit_stores_the_leech_record() {
    let t = suck_tabs(3, 0x1000, 5);
    let ct = ct_ids(2);
    let (mut f, u) = world();
    let tg = mon(&mut f, 0, (2, 2));
    f.targets.insert(u, tg);
    assert_eq!(b4_more::suck_blood_start(&mut f, &t, &ct, u, 1, 1), 1);
    let e = &f.c.units[u].combat;
    assert_eq!(e.len(), 1, "start_combat stored the record");
    let r = e[0].record;
    // Hit flags := 2 | HitFlags; hit class := HitClass.
    assert_eq!(r.hit_flags, 0x1002);
    assert_eq!(r.hit_class, 5);
    // weapon_roll: 10 + roll(30); then += 50 %; leech += calc2 / calc3.
    let p0 = 10 + Seed::new(1, 0).roll(30) as i32;
    assert_eq!(r.physical, p0 + p0 * 50 / 100);
    assert_eq!((r.life_leech, r.mana_leech), (12, 7));
    assert_eq!(r.result & 1, 1);
}

// ---------------------------------------------------------------- §3.10

fn suck_do_tabs() -> SkillTables {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(50);
    tab1(r, c, 1)
}

fn hit_record(physical: i32) -> DamageRecord {
    DamageRecord {
        result: 1,
        physical,
        ..DamageRecord::default()
    }
}

// Covers: specs/skills/bodies-4.md §3.10 r1, §3.10 r2
#[test]
fn suck_blood_do_refusals() {
    let t = suck_do_tabs();
    let ct = ct_ids(2);
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 0, (2, 2));
    assert_eq!(
        b4_more::suck_blood(&mut f, &t, &ct, u, 99, 1),
        0,
        "R invalid"
    );
    assert_eq!(b4_more::suck_blood(&mut f, &t, &ct, u, 1, 1), 0, "T none");
    f.targets.insert(u, tg);
    // No pair record: 0, but the unit's flags |= 0x40 first.
    assert_eq!(b4_more::suck_blood(&mut f, &t, &ct, u, 1, 1), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
}

// Covers: specs/skills/bodies-4.md §3.10 r3
#[test]
fn suck_blood_do_heals_the_minion_owner_or_the_monster() {
    let t = suck_do_tabs();
    let ct = ct_ids(2);
    // A minion heals its owner by pct(min(physical, T life), 50, 100).
    let (mut f, u) = mworld();
    let o = mon(&mut f, 0, (5, 5));
    let tg = mon(&mut f, 0, (2, 2));
    f.targets.insert(u, tg);
    f.minion_owner.insert(u, o);
    f.c.set(tg, 6, 200);
    f.c.set(o, 6, 1000);
    f.c.set(o, 7, 5000);
    f.c.set(u, 6, 10);
    f.c.set(u, 7, 5000);
    stored(&mut f, u, tg, hit_record(300));
    assert_eq!(b4_more::suck_blood(&mut f, &t, &ct, u, 1, 1), 1);
    assert_eq!(f.c.get(o, 6), 1100, "d = min(300, 200) = 200");
    assert_eq!(f.c.get(u, 6), 10);
    // Capped at the owner's maximum, and at least 1.
    f.c.set(o, 7, 1050);
    stored(&mut f, u, tg, hit_record(300));
    b4_more::suck_blood(&mut f, &t, &ct, u, 1, 1);
    assert_eq!(f.c.get(o, 6), 1050);
    // Without an owner the monster itself heals.
    f.minion_owner.clear();
    f.c.set(tg, 6, 1000);
    stored(&mut f, u, tg, hit_record(40));
    b4_more::suck_blood(&mut f, &t, &ct, u, 1, 1);
    assert_eq!(f.c.get(u, 6), 30, "10 + pct(40, 50, 100)");
    // A missed pair record heals nobody.
    stored(&mut f, u, tg, DamageRecord::default());
    b4_more::suck_blood(&mut f, &t, &ct, u, 1, 1);
    assert_eq!(f.c.get(u, 6), 30);
}

// Covers: specs/skills/bodies-4.md §3.10 r4, §edge-cases-original-bugs r3
#[test]
fn suck_blood_do_non_monster_heals_no_one_and_applies_melee() {
    let t = suck_do_tabs();
    let ct = ct_ids(2);
    let (mut f, u) = world();
    let tg = mon(&mut f, 0, (2, 2));
    f.targets.insert(u, tg);
    f.c.set(u, 6, 10);
    f.c.set(u, 7, 5000);
    f.c.set(tg, 6, 1000);
    stored(&mut f, u, tg, hit_record(300));
    assert_eq!(b4_more::suck_blood(&mut f, &t, &ct, u, 1, 1), 1);
    assert_eq!(f.c.get(u, 6), 10, "a non-monster heals no unit");
    // apply_melee ran: out of melee range it frees the pair's records.
    assert!(f.c.units[u].combat.is_empty());
}

// ---------------------------------------------------------------- §3.11

// Covers: specs/skills/bodies-4.md §3.11 r1
#[test]
fn cry_help_refusals() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(30);
    let t = tab1(r, c, 1);
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 0, (2, 2));
    f.targets.insert(u, tg);
    assert_eq!(b4_more::cry_help(&mut f, &t, u, 99, 1), 0, "R invalid");
    f.targets.clear();
    assert_eq!(b4_more::cry_help(&mut f, &t, u, 1, 1), 0, "T none");
    // The caster is not a monster.
    let (mut f, p) = world();
    let tg = mon(&mut f, 0, (2, 2));
    f.targets.insert(p, tg);
    assert_eq!(b4_more::cry_help(&mut f, &t, p, 1, 1), 0);
    assert!(f.c.log.is_empty());
}

// Covers: specs/skills/bodies-4.md §3.11 r2, §3.11 r3, §edge-cases-original-bugs r4
#[test]
fn cry_help_commands_the_minions_and_shows_the_overlay() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(30);
    r.srvoverlay = 7;
    let t = tab1(r, c, 1);
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 0, (2, 2));
    f.targets.insert(u, tg);
    f.c.frame = 100;
    assert_eq!(b4_more::cry_help(&mut f, &t, u, 1, 1), 1);
    let log = f.take_log();
    assert!(log.contains(&fx(Fx::MinionCommand {
        unit: u,
        kind: 1,
        ty: UnitType::Monster.index() as i32,
        guid: 1,
        frame: 130,
    })));
    assert!(log.contains(&format!("overlay {tg} 7")));
    // The expiry is at least one frame; an overlay of 0 is not shown.
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(-5);
    r.srvoverlay = 0;
    let t0 = tab1(r, c, 1);
    b4_more::cry_help(&mut f, &t0, u, 1, 1);
    let log = f.take_log();
    assert!(log.iter().any(|l| l.contains("frame: 101")));
    assert!(!log.iter().any(|l| l.starts_with("overlay")));
}

// ---------------------------------------------------------------- §3.12

// Covers: specs/skills/bodies-4.md §3.12
#[test]
fn self_resurrect_revives_and_clears_the_flags() {
    let (mut f, p) = world();
    assert_eq!(b4_more::self_resurrect(&mut f, p), 0, "not a monster");
    assert!(f.c.log.is_empty());
    let (mut f, u) = mworld();
    f.c.set(u, 7, 500);
    f.c.units[u].flags = 0x1;
    assert_eq!(b4_more::self_resurrect(&mut f, u), 1);
    assert_eq!(f.c.get(u, 6), 500, "revive: life := maximum");
    assert_eq!(
        f.c.units[u].flags & 0xE,
        0,
        "flags &= ~0xE after the revive"
    );
    let log = f.take_log();
    let last = log.iter().rposition(|l| l.starts_with("path")).unwrap();
    assert_eq!(log[last], format!("path {u} FootprintMask(256)"));
}

// ---------------------------------------------------------------- §3.13

// Covers: specs/skills/bodies-4.md §3.13 r1, §3.13 r2, §3.13 r3, §3.13 r4
#[test]
fn vine_attack_makes_calc1_vines() {
    let mut c = Code::new();
    let mut r = body_rec();
    r.srvmissilea = 3;
    r.calc1 = c.f(5);
    let t = tab1(r, c, 6);
    let (mut f, u) = world();
    f.tpos.insert(u, (50, 60));
    assert_eq!(b4_more::vine_attack(&mut f, &t, u, 99, 1), 0, "R invalid");
    assert_eq!(b4_more::vine_attack(&mut f, &t, u, 1, 2), 1);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    assert_eq!(f.missiles.len(), 5);
    assert!(f.missiles.iter().all(|m| m.class == 3 && m.level == 2));
    // m outside 1…count − 1 (0 and count): refused before the flag.
    for m in [0u16, 6] {
        let (mut f, u) = world();
        let mut r = body_rec();
        r.srvmissilea = m;
        let t = tab1(r, Code::new(), 6);
        assert_eq!(b4_more::vine_attack(&mut f, &t, u, 1, 1), 0);
        assert_eq!(f.c.units[u].flags & 0x40, 0);
    }
    // n ≤ 0: 0 after the flag, no vine.
    let mut c = Code::new();
    let mut r = body_rec();
    r.srvmissilea = 3;
    r.calc1 = c.f(0);
    let t0 = tab1(r, c, 6);
    let (mut f, u) = world();
    f.tpos.insert(u, (50, 60));
    assert_eq!(b4_more::vine_attack(&mut f, &t0, u, 1, 1), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    assert!(f.missiles.is_empty());
}

// ---------------------------------------------------------------- §3.14

fn whip_tabs(p: i16) -> SkillTables {
    let mut c = Code::new();
    let mut r = body_rec();
    r.calc1 = c.f(p);
    r.auratargetstate = 90;
    r.auralencalc = c.f(60);
    r.summon = 5;
    r.summode = 2;
    tab1(r, c, 1)
}

fn whip_ct() -> CombatTables {
    let mut ct = ct_ids(500);
    ct.monstats2[453].isatt = true;
    ct.monstats2[100].isatt = true;
    ct
}

// Covers: specs/skills/bodies-4.md §3.14 r1, §3.14 r2
#[test]
fn overseer_whip_refusals() {
    let ct = whip_ct();
    let t = whip_tabs(50);
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 453, (2, 2));
    f.targets.insert(u, tg);
    assert_eq!(
        b4_more::overseer_whip(&mut f, &t, &ct, u, 99, 1),
        0,
        "R invalid"
    );
    // auratargetstate outside the states.
    let mut r = body_rec();
    r.auratargetstate = 300;
    let tb = tab1(r, Code::new(), 1);
    assert_eq!(b4_more::overseer_whip(&mut f, &tb, &ct, u, 1, 1), 0);
    // T none / dead.
    f.targets.clear();
    assert_eq!(b4_more::overseer_whip(&mut f, &t, &ct, u, 1, 1), 0);
    f.targets.insert(u, tg);
    f.c.dead_units.insert(tg);
    assert_eq!(b4_more::overseer_whip(&mut f, &t, &ct, u, 1, 1), 0);
    assert!(f.lists.is_empty() && f.c.log.is_empty());
}

// Covers: specs/skills/bodies-4.md §3.14 r3, §3.14 text
#[test]
fn overseer_whip_transforms_a_minion_below_the_roll() {
    let ct = whip_ct();
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 453, (2, 2));
    f.targets.insert(u, tg);
    // r = roll(100) on the unit's seed; p = 0 → r ≥ p: transform.
    let r = Seed::new(1, 0).roll(100) as i32;
    assert_eq!(
        b4_more::overseer_whip(&mut f, &whip_tabs(0), &ct, u, 1, 1),
        1
    );
    assert_eq!(f.c.units[u].seed.lo, {
        let mut s = Seed::new(1, 0);
        s.step();
        s.lo
    });
    assert!(f.take_log().iter().any(|l| l.starts_with("ClassChange")));
    assert!(f.list_of(tg, 90).is_none(), "no whip state");
    // p = r + 1 > r: the whip state instead.
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 453, (2, 2));
    f.targets.insert(u, tg);
    assert_eq!(
        b4_more::overseer_whip(&mut f, &whip_tabs(r as i16 + 1), &ct, u, 1, 1),
        1
    );
    assert!(!f.take_log().iter().any(|l| l.starts_with("ClassChange")));
    assert!(f.list_of(tg, 90).is_some());
}

// Covers: specs/skills/bodies-4.md §3.14 r4
#[test]
fn overseer_whip_other_cases_whip_the_target() {
    let ct = whip_ct();
    // Not a minion: the whip state, and no roll is drawn.
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 100, (2, 2));
    f.targets.insert(u, tg);
    assert_eq!(
        b4_more::overseer_whip(&mut f, &whip_tabs(0), &ct, u, 1, 3),
        1
    );
    assert_eq!(f.c.units[u].seed, Seed::new(1, 0));
    let l = f.list_of(tg, 90).unwrap();
    assert_eq!((l.skill, l.lvl, l.expire), (1, 3, 60));
    // A minion that already has the state is whipped again (refreshed),
    // even with r ≥ p.
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 453, (2, 2));
    f.targets.insert(u, tg);
    f.c.units[tg].states.push(90);
    assert_eq!(
        b4_more::overseer_whip(&mut f, &whip_tabs(0), &ct, u, 1, 1),
        1
    );
    assert!(!f.take_log().iter().any(|l| l.starts_with("ClassChange")));
    assert!(f.list_of(tg, 90).is_some());
}

// ---------------------------------------------------------------- §3.15

// Covers: specs/skills/bodies-4.md §3.15 r1, §3.15 r2, §3.15 r3, §3.15 r4
#[test]
fn imp_fire_missile_adds_the_chain_position() {
    let mut r = body_rec();
    r.srvmissilea = 8;
    let t = tab1(r, Code::new(), 10);
    let (mut f, u) = mworld();
    f.tpos.insert(u, (30, 40));
    assert_eq!(
        b4_more::imp_fire_missile(&mut f, &t, u, 99, 1),
        0,
        "R invalid"
    );
    f.chains.insert(0, 3);
    assert_eq!(b4_more::imp_fire_missile(&mut f, &t, u, 1, 2), 1);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    // m = 8 + 3 = 11: no range check on the sum.
    let q = &f.missiles[0];
    assert_eq!((q.class, q.flags, q.skill, q.level), (11, 0x21, 1, 2));
    assert_eq!((q.target_x, q.target_y), (30, 40));
    // m0 outside 1…count − 1: 0, no new flag work.
    for m in [0u16, 10] {
        let (mut f, u) = mworld();
        let mut r = body_rec();
        r.srvmissilea = m;
        let t = tab1(r, Code::new(), 10);
        assert_eq!(b4_more::imp_fire_missile(&mut f, &t, u, 1, 1), 0);
        assert_eq!(f.c.units[u].flags & 0x40, 0);
    }
}

// ---------------------------------------------------------------- §3.16

fn pregnant_tabs() -> SkillTables {
    let mut r = body_rec();
    r.summon = 7;
    r.summode = 4;
    tab1(r, Code::new(), 1)
}

// Covers: specs/skills/bodies-4.md §3.16 r1, §3.16 r2
#[test]
fn impregnate_refusals() {
    let t = pregnant_tabs();
    let ct = whip_ct();
    let mut ct6 = ct_ids(600);
    for b in [100usize, 546, 551] {
        ct6.monstats2[b].isatt = true;
    }
    let (mut f, u) = mworld();
    assert_eq!(
        b4_more::impregnate(&mut f, &t, &ct, u, 99, 1),
        0,
        "R invalid"
    );
    assert_eq!(f.c.units[u].flags & 0x40, 0, "R invalid: before the flag");
    // T none: 0 after the flag.
    assert_eq!(b4_more::impregnate(&mut f, &t, &ct6, u, 1, 1), 0);
    assert_eq!(f.c.units[u].flags & 0x40, 0x40);
    // The target must be a living monster of another class, not pregnant,
    // not aligned 2.
    let try_with = |setup: &dyn Fn(&mut BodyFake, usize)| {
        let (mut f, u) = mworld();
        let tg = mon(&mut f, 100, (2, 2));
        f.targets.insert(u, tg);
        setup(&mut f, tg);
        let r = b4_more::impregnate(&mut f, &t, &ct6, u, 1, 1);
        (r, f.list_of(tg, 110).is_some())
    };
    assert_eq!(try_with(&|_, _| {}), (1, true));
    assert_eq!(try_with(&|f, t| f.c.units[t].class = 546), (0, false));
    assert_eq!(try_with(&|f, t| f.c.units[t].class = 551), (0, false));
    assert_eq!(try_with(&|f, t| f.c.units[t].states.push(110)), (0, false));
    assert_eq!(try_with(&|f, t| drop(f.c.aligned.insert(t, 2))), (0, false));
    assert_eq!(try_with(&|f, t| drop(f.dead.insert(t))), (0, false));
    assert_eq!(
        try_with(&|f, t| f.c.units[t].kind = UnitType::Player),
        (0, false)
    );
}

// Covers: specs/skills/bodies-4.md §3.16 r3
#[test]
fn impregnate_applies_the_pregnant_state() {
    let t = pregnant_tabs();
    let mut ct = ct_ids(600);
    ct.monstats2[100].isatt = true;
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 100, (2, 2));
    f.targets.insert(u, tg);
    assert_eq!(b4_more::impregnate(&mut f, &t, &ct, u, 1, 4), 1);
    let l = f.list_of(tg, 110).unwrap();
    assert_eq!((l.skill, l.lvl, l.expire, l.owner), (1, 4, 0, Some(u)));
    assert_eq!(
        l.callback,
        callback::PREGNANT,
        "duration 0, callback 0x005D21B0"
    );
    assert!(f.c.units[tg].states.contains(&110));
}

// Covers: specs/skills/bodies-4.md §3.16 text
#[test]
fn pregnant_remove_releases_a_summon_when_dead() {
    let t = pregnant_tabs();
    let ct = ct_ids(600);
    let spawned = |f: &mut BodyFake| -> Vec<String> {
        f.take_log()
            .into_iter()
            .filter(|l| l.starts_with("NearLevel"))
            .collect()
    };
    let (mut f, u) = mworld();
    let tg = mon(&mut f, 100, (2, 2));
    f.c.units[tg].states.push(110);
    f.lists.push(super::fake::FList {
        skill: 1,
        unit: Some(tg),
        state: 110,
        ..Default::default()
    });
    // Alive: the state goes off, nothing spawns.
    b4_more::remove_pregnant(&mut f, &t, &ct, tg, 110, 0);
    assert!(!f.c.units[tg].states.contains(&110));
    assert!(spawned(&mut f).is_empty());
    // Dead: the summon (7, mode 4) near T, spread 1.
    f.c.dead_units.insert(tg);
    b4_more::remove_pregnant(&mut f, &t, &ct, tg, 110, 0);
    let s = spawned(&mut f);
    assert_eq!(s.len(), 1);
    assert!(
        s[0].contains(&format!("unit: {tg}, class: 7, mode: 4, spread: 1")),
        "{s:?}"
    );
    // A list of an invalid skill, or summon / summode out of range:
    // 551 painworm1, mode 1.
    f.lists[0].skill = 99;
    b4_more::remove_pregnant(&mut f, &t, &ct, tg, 110, 0);
    assert!(spawned(&mut f)[0].contains("class: 551, mode: 1"));
    let mut r = body_rec();
    r.summon = 0;
    r.summode = 16;
    let t2 = tab1(r, Code::new(), 1);
    f.lists[0].skill = 1;
    b4_more::remove_pregnant(&mut f, &t2, &ct, tg, 110, 0);
    assert!(spawned(&mut f)[0].contains("class: 551, mode: 1"));
    let _ = u;
}
