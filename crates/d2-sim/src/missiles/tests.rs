// Spec: specs/missiles/missiles.md (Test vectors, Edge cases)
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use d2_data::tables::Record;

use super::catalogue::{self, seeded_offset, SRVDO_TSV, SRVHIT_TSV, SRV_DO, SRV_HIT};
use super::create::MissileParams;
use super::*;
use crate::rng::Seed;
use crate::tick::run_timer_events;
use crate::units::RoomId;

/// A zeroed `missiles.txt` record with the fields of an arrow-like row.
fn row() -> MissileRow {
    let mut r = MissileRow::decode(&[0u8; 420]);
    r.psrvdofunc = 1;
    r.vel = 20;
    r.maxvel = 20;
    r.range = 50;
    r.collidetype = 3;
    r.collidekill = 1;
    r.lastcollide = true;
    r.size = 1;
    r
}

#[derive(Default)]
struct Path {
    velocity: i32,
    target_unit: Option<UnitId>,
    target_point: Option<(i32, i32)>,
}

/// A fake of every seam: positions, seeds, stats and states by unit; a
/// path that always moves; scripted collision and to-hit results.
#[derive(Default)]
struct Fake {
    room: Option<RoomId>,
    next_seed: u32,
    seeds: BTreeMap<UnitId, Seed>,
    pos: BTreeMap<UnitId, (i32, i32)>,
    stats: BTreeMap<(UnitId, u16), i32>,
    states: BTreeSet<(UnitId, u16)>,
    flags: BTreeMap<UnitId, u32>,
    hostile_off: bool,
    town: BTreeSet<RoomId>,
    no_path: bool,
    paths: BTreeMap<UnitId, Path>,
    stop_moving: bool,
    word: u16,
    crossed: Vec<(i32, i32)>,
    masks: BTreeMap<(i32, i32), u16>,
    units: BTreeMap<(i32, i32), Vec<UnitId>>,
    hits: VecDeque<bool>,
    good: BTreeSet<UnitId>,
    log: Vec<String>,
    /// Step-9 searches (calls of `crossed_subtiles`).
    probes: std::cell::Cell<u32>,
}

impl Fake {
    fn unit_flags(&mut self, u: UnitId) {
        self.flags
            .insert(u, unit_flag::CAN_BE_ATTACKED | unit_flag::IS_VALID_TARGET);
    }
    fn logged(&self, prefix: &str) -> usize {
        self.log.iter().filter(|s| s.starts_with(prefix)).count()
    }
}

impl MissileUnits for Fake {
    fn alloc_missile(
        &mut self,
        game: &mut Game,
        class: u16,
        x: i32,
        y: i32,
        room: RoomId,
        _mode: u8,
    ) -> Option<UnitId> {
        let m = game.spawn_unit(UnitType::Missile, Some(room), false).ok()?;
        self.next_seed += 1;
        self.seeds.insert(m, Seed::init_low(self.next_seed));
        self.pos.insert(m, (x, y));
        self.log.push(format!("alloc {class}"));
        Some(m)
    }
    fn remove_unit(&mut self, game: &mut Game, unit: UnitId) {
        self.log.push(format!("remove {}", unit.0));
        game.remove_unit(unit).unwrap();
    }
    fn seed(&mut self, unit: UnitId) -> &mut Seed {
        self.seeds.entry(unit).or_default()
    }
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.pos.get(&unit).copied().unwrap_or((0, 0))
    }
    fn size(&self, _: UnitId) -> i32 {
        1
    }
    fn stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.stats.get(&(unit, stat)).copied().unwrap_or(0)
    }
    fn base_stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.stat(unit, stat)
    }
    fn set_stat(&mut self, unit: UnitId, stat: u16, value: i32) {
        self.stats.insert((unit, stat), value);
    }
    fn has_state(&self, unit: UnitId, state: u16) -> bool {
        self.states.contains(&(unit, state))
    }
    fn state_stat(&self, unit: UnitId, state: u16, stat: u16) -> Option<i32> {
        self.has_state(unit, state).then(|| self.stat(unit, stat))
    }
    fn alloc_stat_list(&mut self, _: UnitId) {}
    fn unit_flag(&self, unit: UnitId, bit: u32) -> bool {
        self.flags.get(&unit).copied().unwrap_or(0) & bit != 0
    }
    fn set_unit_flag(&mut self, unit: UnitId, bit: u32, on: bool) {
        let f = self.flags.entry(unit).or_default();
        if on {
            *f |= bit;
        } else {
            *f &= !bit;
        }
    }
    fn may_attack(&self, a: UnitId, d: UnitId) -> bool {
        !self.hostile_off && a != d
    }
    fn alignment(&self, unit: UnitId) -> u8 {
        if self.good.contains(&unit) {
            2
        } else {
            0
        }
    }
    fn is_hireling(&self, _: UnitId) -> bool {
        false
    }
    fn apply_justhit(&mut self, _: &mut Game, unit: UnitId, expire: i32) {
        self.log.push(format!("justhit {} {expire}", unit.0));
    }
}

impl MissilePath for Fake {
    fn has_path(&self, _: UnitId) -> bool {
        !self.no_path
    }
    fn set_velocity(&mut self, unit: UnitId, v: i32) {
        self.paths.entry(unit).or_default().velocity = v;
    }
    fn velocity(&self, unit: UnitId) -> i32 {
        self.paths.get(&unit).map_or(0, |p| p.velocity)
    }
    fn set_target_unit(&mut self, unit: UnitId, target: UnitId) {
        self.paths.entry(unit).or_default().target_unit = Some(target);
    }
    fn set_target_point(&mut self, unit: UnitId, x: i32, y: i32) {
        self.paths.entry(unit).or_default().target_point = Some((x, y));
    }
    fn set_footprint_mask(&mut self, _: UnitId, _: u16) {}
    fn set_move_mask(&mut self, _: UnitId, _: u16) {}
    fn build(&mut self, _: &mut Game, _: UnitId) {}
    fn set_acceleration(&mut self, _: UnitId, _: i32, _: i32) {}
    fn target_distance(&self, _: UnitId) -> i32 {
        10
    }
    fn step(&mut self, _: &mut Game, _: UnitId) -> bool {
        !self.stop_moving
    }
    fn collision_word(&self, _: &Game, _: UnitId) -> u16 {
        self.word
    }
    fn crossed_subtiles(&self, _: UnitId) -> Vec<(i32, i32)> {
        self.probes.set(self.probes.get() + 1);
        self.crossed.clone()
    }
}

impl MissileRooms for Fake {
    fn find_room(&self, _: &Game, near: RoomId, _: i32, _: i32) -> Option<RoomId> {
        Some(near)
    }
    fn in_town(&self, _: &Game, room: RoomId) -> bool {
        self.town.contains(&room)
    }
    fn collision_mask(&self, _: &Game, _: RoomId, x: i32, y: i32, _: i32, mask: u16) -> u16 {
        self.masks.get(&(x, y)).copied().unwrap_or(0) & mask
    }
    fn collision_at(&self, _: &Game, _: RoomId, x: i32, y: i32, mask: u16) -> u16 {
        self.masks.get(&(x, y)).copied().unwrap_or(0) & mask
    }
    fn clear_footprint(&mut self, _: &mut Game, unit: UnitId) {
        self.log.push(format!("clear {}", unit.0));
    }
    fn units_at(&self, _: &Game, _: RoomId, x: i32, y: i32) -> Vec<UnitId> {
        self.units.get(&(x, y)).cloned().unwrap_or_default()
    }
}

impl MissileCombat for Fake {
    fn damage_setup(&mut self, _: &mut Game, _: UnitId, _: Option<UnitId>, _: UnitId, _: i32) {
        self.log.push("setup".into());
    }
    fn hit_test(&mut self, _: &mut Game, _: UnitId, _: UnitId, _: i32) -> bool {
        self.hits.pop_front().unwrap_or(true)
    }
    fn srv_dmg(&mut self, _: &mut Game, i: i16, _: UnitId, _: UnitId, _: &mut Damage) {
        self.log.push(format!("srvdmg {i}"));
    }
    fn apply_damage(
        &mut self,
        _: &mut Game,
        _: Option<UnitId>,
        _: UnitId,
        unit: UnitId,
        d: &mut Damage,
    ) {
        self.log.push(format!("damage {} {}", unit.0, d.phys));
    }
    fn hit_by_missile_event(&mut self, _: &mut Game, _: UnitId, unit: Option<UnitId>) {
        self.log.push(format!("event0 {:?}", unit.map(|u| u.0)));
    }
    fn target_damage_bonus(&self, _: UnitId, _: UnitId) -> i32 {
        0
    }
    fn add_target_ac(&mut self, _: &mut Game, _: UnitId, _: i32) {}
}

impl MissileHooks for Fake {
    fn init_callback(&mut self, _: &mut Game, _: UnitId, _: u32, _: u32) {}
    fn unique_mod_missile(&mut self, _: &mut Game, _: UnitId, _: UnitId) {
        self.log.push("umod".into());
    }
}

/// A game with one active room holding a player owner at (100, 100) and
/// a monster at (110, 100).
struct World {
    game: Game,
    fake: Fake,
    store: MissileStore,
    tables: Vec<MissileRow>,
    owner: UnitId,
    monster: UnitId,
}

impl World {
    fn new(r: MissileRow) -> Self {
        let mut game = Game::new();
        game.lists.ensure_act(0).unwrap();
        let room = game.lists.create_room(0).unwrap();
        let owner = game.spawn_unit(UnitType::Player, Some(room), true).unwrap();
        let monster = game
            .spawn_unit(UnitType::Monster, Some(room), false)
            .unwrap();
        let mut fake = Fake {
            room: Some(room),
            ..Fake::default()
        };
        fake.pos.insert(owner, (100, 100));
        fake.pos.insert(monster, (110, 100));
        fake.unit_flags(owner);
        fake.unit_flags(monster);
        Self {
            game,
            fake,
            store: MissileStore::new(),
            tables: vec![r],
            owner,
            monster,
        }
    }

    fn params(&self) -> MissileParams {
        MissileParams {
            owner: Some(self.owner),
            origin: Some(self.owner),
            class: 0,
            flags: param_flags::TARGET_ABSOLUTE,
            target_x: 110,
            target_y: 100,
            ..MissileParams::default()
        }
    }

    fn create(&mut self, p: &MissileParams) -> Option<UnitId> {
        let mut cx = Ctx {
            tables: &self.tables,
            store: &mut self.store,
            world: &mut self.fake,
        };
        create_missile(&mut self.game, &mut cx, p)
    }

    /// One frame: frame += 1, then the timer queue.
    fn frame(&mut self) {
        self.game.frame += 1;
        struct Nothing;
        impl EventDispatch for Nothing {
            fn run_event(&mut self, _: &mut Game, _: &TimerRun) {}
        }
        let mut d = MissileDispatch {
            cx: Ctx {
                tables: &self.tables,
                store: &mut self.store,
                world: &mut self.fake,
            },
            next: &mut Nothing,
        };
        run_timer_events(&mut self.game, &mut d);
    }

    fn alive(&self, m: UnitId) -> bool {
        self.store.get(m).is_some()
    }

    /// Runs frames until the missile is gone; returns the run count.
    fn lifetime(&mut self, m: UnitId) -> i32 {
        let mut n = 0;
        while self.alive(m) {
            self.frame();
            n += 1;
            assert!(n < 1000);
        }
        n
    }

    fn hit(&mut self, m: UnitId, unit: Option<UnitId>, a4: bool) -> i32 {
        let mut cx = Ctx {
            tables: &self.tables,
            store: &mut self.store,
            world: &mut self.fake,
        };
        hit_handler(&mut self.game, &mut cx, m, unit, a4)
    }
}

// ---- R2: creation ---------------------------------------------------

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r5, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r7, §r10-behaviour-of-the-recorded-missiles r5
#[test]
fn creation_velocity_vectors() {
    let p = |level| MissileParams {
        level,
        ..MissileParams::default()
    };
    let mut r = row();
    r.vel = 20;
    assert_eq!(creation_velocity(&r, &p(5), None), 3840);
    r.vel = 10;
    r.vellev = 8;
    assert_eq!(creation_velocity(&r, &p(3), None), 2496);
    r.vellev = 7;
    assert_eq!(creation_velocity(&r, &p(3), None), 2304);
    r.vel = 24;
    r.vellev = 0;
    assert_eq!(creation_velocity(&r, &p(0), Some(50)), 2304);
    // R10.5 speeds.
    r.vel = 8;
    assert_eq!(creation_velocity(&r, &p(0), None), 1536);
    r.vel = 24;
    assert_eq!(creation_velocity(&r, &p(0), None), 4608);
    r.vel = 10;
    r.vellev = 8;
    assert_eq!(creation_velocity(&r, &p(0), None), 1920);
    // Given velocity: << 8 unless flag 0x10.
    let mut q = p(0);
    q.flags = param_flags::VELOCITY;
    q.velocity = 4;
    assert_eq!(creation_velocity(&r, &q, None), 768);
    q.flags |= param_flags::VELOCITY_FIXED;
    q.velocity = 1024;
    assert_eq!(creation_velocity(&r, &q, None), 768);
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r6
#[test]
fn creation_slow_uses_state_87_stat_161() {
    let mut r = row();
    r.vel = 24;
    r.canslow = true;
    let mut w = World::new(r);
    w.fake.states.insert((w.owner, state::SLOWMISSILES));
    w.fake.stats.insert((w.owner, stat::SKILL_HANDOFATHENA), 50);
    let m = w.create(&w.params()).unwrap();
    assert_eq!(w.fake.velocity(m), 2304);
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r4, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r8
#[test]
fn creation_target_at_start_moves_one_subtile() {
    let mut w = World::new(row());
    let mut p = w.params();
    p.flags = 0; // target = start
    let m = w.create(&p).unwrap();
    assert_eq!(w.fake.paths[&m].target_point, Some((101, 101)));
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r8
#[test]
fn creation_target_on_owner_subtile_is_dropped() {
    let mut w = World::new(row());
    let mut p = w.params();
    p.target = Some(w.monster);
    w.fake.pos.insert(w.monster, (100, 100));
    let m = w.create(&p).unwrap();
    assert_eq!(w.fake.paths[&m].target_unit, None);
    assert_eq!(w.fake.paths[&m].target_point, Some((111, 101)));
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r8
#[test]
fn creation_fails_100_subtiles_away() {
    let mut w = World::new(row());
    let mut p = w.params();
    p.target_x = 200;
    assert_eq!(w.create(&p), None);
    assert_eq!(w.fake.logged("alloc"), 0);
    p.target_x = 199;
    assert!(w.create(&p).is_some());
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r1
#[test]
fn creation_fails_without_owner_or_bad_class() {
    let mut w = World::new(row());
    let mut p = w.params();
    p.owner = None;
    assert_eq!(w.create(&p), None);
    let mut p = w.params();
    p.class = 1;
    assert_eq!(w.create(&p), None);
    p.class = -1;
    assert_eq!(w.create(&p), None);
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r14, §edge-cases-original-bugs r4
#[test]
fn creation_without_path_leaves_the_unit_with_its_event() {
    // Edge case 4.
    let mut w = World::new(row());
    w.fake.no_path = true;
    assert_eq!(w.create(&w.params()), None);
    let ms = w.game.lists.units_of_type(UnitType::Missile);
    assert_eq!(ms.len(), 1);
    assert_eq!(w.game.timers.unit_timers(ms[0]).len(), 1);
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r10, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r12, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r13, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r17, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r22
#[test]
fn creation_frames_activate_and_order() {
    let mut r = row();
    r.range = 40;
    r.levrange = 2;
    r.activate = 3;
    let mut w = World::new(r);
    let mut p = w.params();
    p.level = 3;
    p.skill = -5;
    let m = w.create(&p).unwrap();
    let d = w.store.get(m).unwrap();
    assert_eq!((d.total, d.current, d.activate), (46, 46, 43));
    assert_eq!((d.skill, d.level), (0, 3));
    assert_eq!(d.last_collided.map(|l| l.ty), Some(UnitType::Player));
    assert_eq!(w.fake.log, ["alloc 0", "setup"]);
    // Every-tick type-0 event.
    let t = w.game.timers.unit_timers(m);
    assert_eq!(t.len(), 1);
    assert_eq!(w.game.timers.event(t[0]).map(|e| e.0), Some(0));
    assert_eq!(w.game.timers.expire(t[0]), Some(-1));
    assert_eq!(w.fake.stat(m, stat::DAMAGE_FRAMERATE), 0);
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r19, §edge-cases-original-bugs r11
#[test]
fn creation_frames_from_distance() {
    assert_eq!(frames_from_distance(10, 3840), (10u32 << 16) / (3840 << 4));
    assert_eq!(frames_from_distance(0, 3840), (1u32 << 16) / (3840 << 4));
    assert_eq!(frames_from_distance(10, 0), 0);
    // Edge case 11: a negative v divides by a huge unsigned value.
    assert_eq!(frames_from_distance(10, -1), 0);
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r28
#[test]
fn creation_monster_owner_runs_unique_mod_hook() {
    let mut w = World::new(row());
    let mut p = w.params();
    p.owner = Some(w.monster);
    p.origin = Some(w.monster);
    p.target_x = 100;
    w.create(&p).unwrap();
    assert_eq!(w.fake.logged("umod"), 1);
}

// ---- R8: pierce -----------------------------------------------------

// Covers: specs/missiles/missiles.md §r8-1-pierce-test-at-creation-0x0059f940-1-14d-confirmed r2, §r8-1-pierce-test-at-creation-0x0059f940-1-14d-confirmed r3, §edge-cases-original-bugs r7
#[test]
fn pierce_vectors() {
    assert_eq!(pierce_count(67, 0), 4);
    assert_eq!(pierce_count(66, 0), 0);
    assert_eq!(pierce_count(52, 1), 3);
    assert_eq!(pierce_count(80, 5), 0);
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r25, §r8-1-pierce-test-at-creation-0x0059f940-1-14d-confirmed r1, §r8-1-pierce-test-at-creation-0x0059f940-1-14d-confirmed r4, §r8-2-pierce-at-a-hit-0x005ada80
#[test]
fn pierce_set_at_creation_and_used_at_hits() {
    let mut r = row();
    r.pierce = true;
    let mut w = World::new(r);
    w.fake.stats.insert((w.owner, stat::SKILL_PIERCE), 67);
    let m = w.create(&w.params()).unwrap();
    assert_eq!(w.fake.stat(m, stat::PIERCE_IDX), 4);
    // A hit with CollideKill keeps the missile while pierce_idx > 0.
    let mon = w.monster;
    assert_eq!(w.hit(m, Some(mon), false), 1);
    assert_eq!(w.fake.stat(m, stat::PIERCE_IDX), 3);
    // The owner losing its pierce stat stops piercing (R8.2).
    w.fake.stats.insert((w.owner, stat::SKILL_PIERCE), 0);
    w.store.get_mut(m).unwrap().last_collided = None;
    assert_eq!(w.hit(m, Some(mon), false), 2);
}

// Covers: specs/missiles/missiles.md §r8-1-pierce-test-at-creation-0x0059f940-1-14d-confirmed r1
#[test]
fn pierce_not_set_without_owner_stat() {
    let mut r = row();
    r.pierce = true;
    let mut w = World::new(r);
    let m = w.create(&w.params()).unwrap();
    assert!(!w.fake.stats.contains_key(&(m, stat::PIERCE_IDX)));
}

// ---- R4.1: velocity -------------------------------------------------

// Covers: specs/missiles/missiles.md §r4-1-movement-in-fixed-point r2, §r4-1-movement-in-fixed-point r3
#[test]
fn acceleration_vector() {
    let mut v = PathVelocity {
        velocity: 1000,
        max: 4 << 8,
        accel: 10,
        counter: 0,
    };
    let mut seen = Vec::new();
    for _ in 0..16 {
        v.advance();
        seen.push(v.velocity);
    }
    assert_eq!(&seen[..4], &[1000; 4]);
    assert_eq!(seen[4], 1010);
    assert_eq!(seen[9], 1020);
    assert_eq!(seen[14], 1024);
    assert_eq!(v.accel, 0);
    assert_eq!(seen[15], 1024);
    // Below 0 → 0.
    let mut v = PathVelocity {
        velocity: 5,
        max: 100,
        accel: -10,
        counter: 4,
    };
    v.advance();
    assert_eq!(v.velocity, 0);
    // Step vector: ((3840 × 0x400) >> 6) × 4096 >> 12.
    let v = PathVelocity {
        velocity: 3840,
        ..PathVelocity::default()
    };
    assert_eq!(v.step_component(4096), 61440);
    assert_eq!(v.step_component(-4096), -61440);
}

// ---- R7: lifetime ---------------------------------------------------

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r10, §r7-lifetime-and-expiry r1, §r7-lifetime-and-expiry r2, §r7-lifetime-and-expiry r5
#[test]
fn lifetime_is_range_plus_levrange_runs() {
    let mut r = row();
    r.range = 40;
    r.levrange = 2;
    let mut w = World::new(r);
    let mut p = w.params();
    p.level = 3;
    w.game.frame = 677;
    let m = w.create(&p).unwrap();
    // First run in F + 1, removed in F + 46.
    assert_eq!(w.lifetime(m), 46);
    assert_eq!(w.game.frame, 677 + 46);
    assert!(w.game.lists.unit(m).is_none());
    // The expiry hit ran with no unit; no damage.
    assert_eq!(w.fake.logged("event0 None"), 1);
    assert_eq!(w.fake.logged("damage"), 0);
}

// Covers: specs/missiles/missiles.md §r7-lifetime-and-expiry r2
#[test]
fn recorded_full_lifetimes() {
    // Firebolt 50, shafire1 40, rogue1 40 runs (R7.2, recordings).
    for range in [50, 40] {
        let mut r = row();
        r.range = range;
        let mut w = World::new(r);
        let m = w.create(&w.params()).unwrap();
        assert_eq!(w.lifetime(m), i32::from(range));
    }
}

// Covers: specs/missiles/missiles.md §r7-lifetime-and-expiry r2
#[test]
fn lifetime_one_frame_or_less() {
    for range in [0, 1] {
        let mut r = row();
        r.range = range;
        let mut w = World::new(r);
        let m = w.create(&w.params()).unwrap();
        assert_eq!(w.lifetime(m), 1);
    }
}

// Covers: specs/missiles/missiles.md §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r7, §r7-lifetime-and-expiry r3
#[test]
fn collision_tested_from_activate_run() {
    let mut r = row();
    r.range = 50;
    r.activate = 3;
    let mut w = World::new(r);
    w.fake.word = coll::PLAYER;
    w.fake.crossed = vec![(5, 5)];
    let m = w.create(&w.params()).unwrap();
    let mut tested = Vec::new();
    let mut k = 0;
    while w.alive(m) {
        let before = w.fake.probes.get();
        w.frame();
        k += 1;
        if w.fake.probes.get() != before {
            tested.push(k);
        }
    }
    assert_eq!(tested.first(), Some(&3));
    assert_eq!(tested.last(), Some(&49));
    assert_eq!(k, 50);
}

// Covers: specs/missiles/missiles.md §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r2
#[test]
fn stopped_movement_is_an_expiry_hit() {
    let mut w = World::new(row());
    let m = w.create(&w.params()).unwrap();
    w.fake.stop_moving = true;
    w.frame();
    assert!(!w.alive(m));
    assert_eq!(w.fake.logged("event0 None"), 1);
    assert_eq!(w.fake.logged("clear"), 1);
}

// Covers: specs/missiles/missiles.md §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r6, §edge-cases-original-bugs r3
#[test]
fn blocking_word_removes_without_hit() {
    // Edge case 3.
    for word in [coll::WALL, coll::MISSILE_BARRIER] {
        let mut w = World::new(row());
        let m = w.create(&w.params()).unwrap();
        w.fake.word = word;
        w.frame();
        assert!(!w.alive(m));
        assert_eq!(w.fake.logged("event0"), 0);
        assert_eq!(w.fake.logged("clear"), 0);
    }
}

// Covers: specs/missiles/missiles.md §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r4, §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r7
#[test]
fn mode_zero_and_six_never_collide() {
    for mode in [0, 6] {
        let mut r = row();
        r.collidetype = mode;
        let mut w = World::new(r);
        let m = w.create(&w.params()).unwrap();
        w.fake.word = coll::MONSTER;
        w.fake.crossed = vec![(110, 100)];
        w.fake.masks.insert((110, 100), coll::MONSTER);
        w.fake.units.insert((110, 100), vec![w.monster]);
        w.frame();
        assert!(w.alive(m));
        assert_eq!(w.fake.logged("event0"), 0);
    }
}

// Covers: specs/missiles/missiles.md §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r8
#[test]
fn mode_four_removed_on_any_word() {
    let mut r = row();
    r.collidetype = 4;
    let mut w = World::new(r);
    let m = w.create(&w.params()).unwrap();
    w.fake.word = coll::MONSTER;
    w.frame();
    assert!(!w.alive(m));
    assert_eq!(w.fake.logged("event0"), 0);
}

/// Sets up a unit contact on run 1.
fn contact(w: &mut World) {
    w.fake.word = coll::MONSTER;
    w.fake.crossed = vec![(109, 100), (110, 100)];
    w.fake.masks.insert((110, 100), coll::MONSTER);
    w.fake.units.insert((110, 100), vec![w.monster]);
}

// Covers: specs/missiles/missiles.md §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r9, §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r7, §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r8
#[test]
fn unit_hit_damages_and_kills() {
    let mut r = row();
    r.tohit = 1;
    let mut w = World::new(r);
    let m = w.create(&w.params()).unwrap();
    contact(&mut w);
    w.fake.stats.insert((m, stat::MINDAMAGE), 0x100);
    w.fake.stats.insert((m, stat::MAXDAMAGE), 0x200);
    w.frame();
    assert!(!w.alive(m));
    let mon = w.monster.0;
    assert_eq!(w.fake.logged(&format!("event0 Some({mon})")), 1);
    assert_eq!(w.fake.logged(&format!("damage {mon}")), 1);
    assert_eq!(w.fake.logged("clear"), 1);
}

// Covers: specs/missiles/missiles.md §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r5, §edge-cases-original-bugs r2
#[test]
fn missed_to_hit_always_removes() {
    // Edge case 2: even with pierce left and CollideKill 0.
    let mut r = row();
    r.tohit = 1;
    r.collidekill = 0;
    r.pierce = true;
    let mut w = World::new(r);
    w.fake.stats.insert((w.owner, stat::SKILL_PIERCE), 67);
    let m = w.create(&w.params()).unwrap();
    w.fake.hits.push_back(false);
    let mon = w.monster;
    assert_eq!(w.hit(m, Some(mon), false), 2);
    assert_eq!(w.fake.logged("damage"), 0);
    assert_eq!(w.fake.logged("event0"), 1);
}

// Covers: specs/missiles/missiles.md §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r2, §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r8
#[test]
fn no_collidekill_flies_on() {
    let mut r = row();
    r.collidekill = 0;
    let mut w = World::new(r);
    let m = w.create(&w.params()).unwrap();
    let mon = w.monster;
    assert_eq!(w.hit(m, Some(mon), false), 1);
    assert_eq!(w.fake.logged("damage"), 1);
    // LastCollide: the same unit is ignored next.
    assert_eq!(w.hit(m, Some(mon), false), 1);
    assert_eq!(w.fake.logged("damage"), 1);
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r17
#[test]
fn last_collide_skips_owner_first() {
    let mut w = World::new(row());
    let m = w.create(&w.params()).unwrap();
    w.fake.word = coll::PLAYER;
    w.fake.crossed = vec![(100, 100)];
    w.fake.masks.insert((100, 100), coll::PLAYER);
    w.fake.units.insert((100, 100), vec![w.owner]);
    w.frame();
    assert!(w.alive(m));
    assert_eq!(w.fake.logged("event0"), 0);
}

// Covers: specs/missiles/missiles.md §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r2
#[test]
fn friendly_units_ignored_unless_collide_friend() {
    let mut w = World::new(row());
    w.fake.hostile_off = true;
    let m = w.create(&w.params()).unwrap();
    contact(&mut w);
    w.frame();
    assert!(w.alive(m));
    let mut r = row();
    r.collidefriend = 1;
    let mut w = World::new(r);
    w.fake.hostile_off = true;
    let m = w.create(&w.params()).unwrap();
    contact(&mut w);
    w.frame();
    assert!(!w.alive(m));
}

// Covers: specs/missiles/missiles.md §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r2, §edge-cases-original-bugs r9
#[test]
fn mode_one_ignores_good_monsters_after_finding_them() {
    // Edge case 9.
    let mut r = row();
    r.collidetype = 1;
    let mut w = World::new(r);
    let m = w.create(&w.params()).unwrap();
    contact(&mut w);
    w.fake.masks.insert((110, 100), coll::PLAYER);
    w.fake.good.insert(w.monster);
    w.frame();
    assert!(w.alive(m));
    assert_eq!(w.fake.logged("event0"), 0);
}

// Covers: specs/missiles/missiles.md §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r9
#[test]
fn barrier_on_crossed_subtile_is_expiry_hit() {
    let mut w = World::new(row());
    let m = w.create(&w.params()).unwrap();
    w.fake.word = coll::MONSTER;
    w.fake.crossed = vec![(105, 100)];
    w.fake.masks.insert((105, 100), coll::MISSILE_BARRIER);
    w.frame();
    assert!(!w.alive(m));
    assert_eq!(w.fake.logged("event0 None"), 1);
}

// Covers: specs/missiles/missiles.md §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r2, §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r6
#[test]
fn justhit_with_next_hit() {
    let mut r = row();
    r.nexthit = 1;
    r.nextdelay = 7;
    r.collidekill = 0;
    let mut w = World::new(r);
    let m = w.create(&w.params()).unwrap();
    w.game.frame = 100;
    let mon = w.monster;
    w.hit(m, Some(mon), false);
    assert_eq!(w.fake.logged(&format!("justhit {} 107", mon.0)), 1);
    // A unit with state 86 is ignored.
    w.fake.states.insert((mon, state::JUSTHIT));
    w.store.get_mut(m).unwrap().last_collided = None;
    assert_eq!(w.hit(m, Some(mon), false), 1);
    assert_eq!(w.fake.logged("damage"), 1);
}

// Covers: specs/missiles/missiles.md §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r1, §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r8
#[test]
fn explosion_rows_skip_direct_damage() {
    let mut r = row();
    r.explosion = true;
    let mut w = World::new(r);
    let m = w.create(&w.params()).unwrap();
    let mon = w.monster;
    // c = 0 | CollideKill 1 → dies, no damage stage.
    assert_eq!(w.hit(m, Some(mon), false), 2);
    assert_eq!(w.fake.logged("damage"), 0);
}

// Covers: specs/missiles/missiles.md §r7-lifetime-and-expiry r5
#[test]
fn server_hit_stub_logged_on_expiry() {
    let mut r = row();
    r.psrvhitfunc = 1;
    r.range = 1;
    let mut w = World::new(r);
    let m = w.create(&w.params()).unwrap();
    w.frame();
    assert!(!w.alive(m));
    assert_eq!(
        w.store.unhandled,
        [Unhandled::SrvHit {
            index: 1,
            missile: m
        }]
    );
}

// Covers: specs/missiles/missiles.md §r1-data-the-server-keeps-per-missile r3, §r3-per-tick-dispatch r1
#[test]
fn server_do_dispatch_limits() {
    // pSrvDoFunc 0 or negative: nothing happens (R3.1).
    for f in [0u16, 0xFDB4, 53] {
        let mut r = row();
        r.psrvdofunc = f;
        r.range = 1;
        let mut w = World::new(r);
        let m = w.create(&w.params()).unwrap();
        for _ in 0..3 {
            w.frame();
        }
        assert!(w.alive(m));
        assert!(w.store.unhandled.is_empty());
    }
    // A stub and a null entry are logged.
    for (f, want) in [(2u16, false), (4, true)] {
        let mut r = row();
        r.psrvdofunc = f;
        let mut w = World::new(r);
        let m = w.create(&w.params()).unwrap();
        w.frame();
        let u = if want {
            Unhandled::NullSrvDo {
                index: 4,
                missile: m,
            }
        } else {
            Unhandled::SrvDo {
                index: 2,
                missile: m,
            }
        };
        assert_eq!(w.store.unhandled, [u]);
    }
}

// Covers: specs/missiles/missiles.md §r3-per-tick-dispatch r5, §r3-per-tick-dispatch r6, §edge-cases-original-bugs r1
#[test]
fn town_rules() {
    // R3.5: a player owner in town removes even `Town` missiles.
    let mut r = row();
    r.town = true;
    let mut w = World::new(r);
    let m = w.create(&w.params()).unwrap();
    let room = w.fake.room.unwrap();
    w.fake.town.insert(room);
    w.frame();
    assert!(!w.alive(m));
    // A monster owner in town with `Town`: kept.
    let mut r = row();
    r.town = true;
    let mut w = World::new(r);
    let mut p = w.params();
    p.owner = Some(w.monster);
    p.origin = Some(w.monster);
    p.target_x = 100;
    let m = w.create(&p).unwrap();
    let room = w.fake.room.unwrap();
    w.fake.town.insert(room);
    w.frame();
    assert!(w.alive(m));
    // Without `Town`: removed.
    let mut w = World::new(row());
    let mut p = w.params();
    p.owner = Some(w.monster);
    p.origin = Some(w.monster);
    p.target_x = 100;
    let m = w.create(&p).unwrap();
    let room = w.fake.room.unwrap();
    w.fake.town.insert(room);
    w.frame();
    assert!(!w.alive(m));
}

// ---- R6: damage -----------------------------------------------------

// Covers: specs/missiles/missiles.md §r6-2-damage-rolls-0x005a89a0-1-14d-confirmed
#[test]
fn damage_roll_vectors() {
    let mut s = Seed::init();
    assert_eq!(damage_roll(&mut s, 0x100, 0x100, 0), 0x100);
    assert_eq!(s, Seed::init(), "equal bounds draw nothing");
    let mut s = Seed::init();
    let mut t = Seed::init();
    assert_eq!(
        damage_roll(&mut s, 0x300, 0x100, 0),
        0x100 + t.roll(0x200) as i32
    );
    assert_eq!(s, t);
    let mut s = Seed::init();
    assert_eq!(damage_roll(&mut s, 0, 0x100, 0), 0);
    assert_eq!(damage_roll(&mut s, 0x100, 0, 0), 0);
    assert_eq!(s, Seed::init());
    // Mastery 50 %: 0x100 → 0x180 on both bounds.
    let mut s = Seed::init();
    assert_eq!(damage_roll(&mut s, 0x100, 0x100, 50), 0x180);
    // pct with large operands is exact.
    assert_eq!(pct(0x4000_0000, 200), i32::MIN);
    assert_eq!(pct(3_000_000, 100_000), 3_000_000_000_i64 as i32);
}

// Covers: specs/missiles/missiles.md §r6-2-damage-rolls-0x005a89a0-1-14d-confirmed
#[test]
fn damage_rolls_use_missile_seed_in_order() {
    let mut w = World::new(row());
    let m = w.create(&w.params()).unwrap();
    for (min, max) in [
        (stat::MINDAMAGE, stat::MAXDAMAGE),
        (stat::FIREMINDAM, stat::FIREMAXDAM),
        (stat::POISONMINDAM, stat::POISONMAXDAM),
    ] {
        w.fake.stats.insert((m, min), 10);
        w.fake.stats.insert((m, max), 20);
    }
    let seed = *w.fake.seed(m);
    let mut cx = Ctx {
        tables: &w.tables,
        store: &mut w.store,
        world: &mut w.fake,
    };
    let d = fill_damage(&mut cx, m, None);
    let mut s = seed;
    let want = [10 + s.roll(10), 10 + s.roll(10), 10 + s.roll(10)];
    assert_eq!([d.phys, d.fire, d.poison], want.map(|v| v as i32));
    assert_eq!(*w.fake.seed(m), s);
}

// Covers: specs/missiles/missiles.md §r6-2-damage-rolls-0x005a89a0-1-14d-confirmed
#[test]
fn damage_percent_and_deadly_strike() {
    let mut w = World::new(row());
    let m = w.create(&w.params()).unwrap();
    w.fake.stats.insert((m, stat::MINDAMAGE), 100);
    w.fake.stats.insert((m, stat::MAXDAMAGE), 100);
    w.fake.stats.insert((m, stat::DAMAGEPERCENT), -200);
    let mon = w.monster;
    let mut cx = Ctx {
        tables: &w.tables,
        store: &mut w.store,
        world: &mut w.fake,
    };
    assert_eq!(fill_damage(&mut cx, m, Some(mon)).phys, 10);
    cx.world.stats.insert((m, stat::DAMAGEPERCENT), 50);
    cx.world.stats.insert((m, stat::DEADLY_STRIKE), 1);
    let d = fill_damage(&mut cx, m, Some(mon));
    assert_eq!((d.phys, d.crit), (300, true));
    cx.world.stats.insert((m, stat::POISONLENGTH), 100);
    cx.world.stats.insert((m, stat::POISON_COUNT), 3);
    assert_eq!(fill_damage(&mut cx, m, None).poison_length, 33);
}

// Covers: specs/missiles/missiles.md §r6-1-order-1-14d-0x005adf10-step-7 text
#[test]
fn knockback_roll_on_missile_seed() {
    let mut r = row();
    r.knockback = 100;
    r.gethit = true;
    let mut w = World::new(r);
    let m = w.create(&w.params()).unwrap();
    let mon = w.monster;
    let seed = *w.fake.seed(m);
    let mut cx = Ctx {
        tables: &w.tables,
        store: &mut w.store,
        world: &mut w.fake,
    };
    let f = result_flags(&w.game, &mut cx, m, mon);
    use super::hit::result_flag as rf;
    assert_eq!(f, rf::HIT | rf::GETHIT | rf::KNOCKBACK);
    let mut s = seed;
    s.step();
    assert_eq!(*cx.world.seed(m), s);
    // State 54: no get-hit, no draw.
    cx.world.states.insert((mon, state::UNINTERRUPTABLE));
    assert_eq!(result_flags(&w.game, &mut cx, m, mon), rf::HIT);
    assert_eq!(*cx.world.seed(m), s);
}

// ---- R9: catalogues -------------------------------------------------

// Covers: specs/missiles/missiles.md §r9-3-seeded-sub-missile-helper-0x005a9820-d2moo-missmode-createmissilewithcollisioncheck-1-14d-confirmed r4
#[test]
fn seeded_offset_vector() {
    let mut s = Seed::default();
    assert_eq!(seeded_offset(&mut s, 5000, 8, 4), (-3, 1));
}

/// Checks a catalogue table against its TSV: one row per index, address
/// `-` exactly for the null entries. Returns the first disagreement.
fn check_catalogue(tsv: &str, table: &[Option<u32>]) -> Result<(), String> {
    let rows: Vec<&str> = tsv.lines().skip(1).filter(|l| !l.is_empty()).collect();
    if rows.len() != table.len() {
        return Err(format!("{} rows, table has {}", rows.len(), table.len()));
    }
    for (i, line) in rows.iter().enumerate() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 9 {
            return Err(format!("row {i}: {} columns", cols.len()));
        }
        if cols[0] != i.to_string() {
            return Err(format!("row {i}: index {}", cols[0]));
        }
        let addr = match cols[1] {
            "-" => None,
            a => Some(
                u32::from_str_radix(a.trim_start_matches("0x"), 16)
                    .map_err(|e| format!("row {i}: {e}"))?,
            ),
        };
        if addr != table[i] {
            return Err(format!("index {i}: tsv {addr:x?}, table {:x?}", table[i]));
        }
    }
    Ok(())
}

// Covers: specs/missiles/missiles.md §r9-1-tables-dumped-from-game-exe-1-14d-confirmed
#[test]
fn catalogues_match_tsv() {
    check_catalogue(SRVDO_TSV, &SRV_DO).unwrap();
    check_catalogue(SRVHIT_TSV, &SRV_HIT).unwrap();
    // The implemented bodies are non-null entries.
    for i in catalogue::SRV_DO_IMPLEMENTED {
        assert!(SRV_DO[i as usize].is_some());
    }
    // §R9.1 null patterns.
    let null_do: Vec<usize> = (0..53).filter(|&i| SRV_DO[i].is_none()).collect();
    let mut want: Vec<usize> = vec![0, 4];
    want.extend(38..53);
    assert_eq!(null_do, want);
    let null_hit: Vec<usize> = (0..71).filter(|&i| SRV_HIT[i].is_none()).collect();
    let mut want = vec![0, 30, 34, 41, 42, 46, 49];
    want.extend(60..71);
    assert_eq!(null_hit, want);
    assert_eq!(SRV_DO[1], Some(0x005B_0BC0));
}

#[test]
fn catalogue_check_catches_perturbations() {
    // M08: change one address and one null cell; the check names them.
    let bad = SRVDO_TSV.replacen("0x005AE400", "0x005AE401", 1);
    assert_eq!(
        check_catalogue(&bad, &SRV_DO),
        Err("index 2: tsv Some(5ae401), table Some(5ae400)".into())
    );
    let bad = SRVHIT_TSV.replacen("\n30\t-\t", "\n30\t0x1\t", 1);
    assert_eq!(
        check_catalogue(&bad, &SRV_HIT),
        Err("index 30: tsv Some(1), table None".into())
    );
    let mut short = SRV_HIT.to_vec();
    short.pop();
    assert!(check_catalogue(SRVHIT_TSV, &short).is_err());
}

// Covers: specs/missiles/missiles.md §r4-2-collide-types-missile-modes
#[test]
fn collide_mode_table() {
    let masks: Vec<u16> = COLLIDE_MODES.iter().map(|c| c.mask).collect();
    assert_eq!(masks, [0, 0x84, 0x104, 0x184, 0, 0x104, 0x4, 0x40, 0x185]);
    assert_eq!(collide_mode(9), None);
}
