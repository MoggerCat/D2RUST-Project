// Spec: specs/monsters/ai.md (Test vectors, Edge cases)
use std::collections::{BTreeMap, BTreeSet};

use d2_data::tables::{Levels, Monstats, Monstats2, Record};

use super::functions::IMPLEMENTED;
use super::table::AI_FUNCTIONS_TSV;
use super::*;
use crate::rng::Seed;
use crate::units::RoomId;

const SEEDS: [u32; 4] = [1, 12345, 3_735_928_559, 4_014_346_870];

#[derive(Default)]
struct Fake {
    seeds: BTreeMap<UnitId, Seed>,
    class: BTreeMap<UnitId, i32>,
    anim: BTreeMap<UnitId, u8>,
    states: BTreeSet<(UnitId, u16)>,
    dead: BTreeSet<UnitId>,
    pos: BTreeMap<UnitId, (i32, i32)>,
    level: i32,
    life: i32,
    ai_state: u32,
    unique: bool,
    walk_fails: bool,
    can_walk_off: bool,
    town: BTreeSet<RoomId>,
    collides: bool,
    melee: BTreeSet<UnitId>,
    nodes: Vec<Vec<UnitId>>,
    secondary: Option<(UnitId, i32)>,
    log: Vec<String>,
    // Knobs added for the rule tests below (defaults keep the behaviour
    // the older tests rely on).
    align: u8,
    champion: bool,
    vision: Option<u32>,
    interacting: bool,
    busy: BTreeSet<UnitId>,
    /// Modes whose start fails.
    fail_modes: BTreeSet<u8>,
    /// A failed mode start falls into the neutral start (§1.3), which
    /// adds a think at frame + this when none is pending later.
    fail_think: Option<i32>,
    blocked_path: bool,
    door: Option<(UnitId, bool)>,
    no_los_draw: bool,
    line_blocked: BTreeSet<UnitId>,
    reach_fails: bool,
    spot: Option<(i32, i32, RoomId)>,
    last_dead: BTreeMap<RoomId, [Option<UnitId>; 4]>,
    forced: Option<(UnitId, i32)>,
    good: Option<(UnitId, i32)>,
    nearest: Option<(UnitId, bool)>,
    special_walk: Option<(UnitId, i32)>,
    corpses: (Option<UnitId>, u32),
    skill_unusable: bool,
    /// Acts by unit (default 0).
    acts: BTreeMap<UnitId, u8>,
    /// `choose_alternative` takes the slot-9 alternative.
    take_alt: bool,
}

impl Fake {
    fn modes(&self) -> Vec<String> {
        self.log
            .iter()
            .filter(|s| s.starts_with("mode"))
            .cloned()
            .collect()
    }
}

impl AiUnits for Fake {
    fn seed(&mut self, unit: UnitId) -> &mut Seed {
        self.seeds.entry(unit).or_default()
    }
    fn class(&self, unit: UnitId) -> i32 {
        self.class.get(&unit).copied().unwrap_or(0)
    }
    fn anim_mode(&self, unit: UnitId) -> u8 {
        self.anim.get(&unit).copied().unwrap_or(mode::NEUTRAL)
    }
    fn has_state(&self, unit: UnitId, state: u16) -> bool {
        self.states.contains(&(unit, state))
    }
    fn clear_uninterruptable(&mut self, _: &mut Game, unit: UnitId) {
        self.states.remove(&(unit, state::UNINTERRUPTABLE));
    }
    fn is_dead(&self, unit: UnitId) -> bool {
        self.dead.contains(&unit)
    }
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.pos.get(&unit).copied().unwrap_or((100, 100))
    }
    fn size(&self, _: UnitId) -> i32 {
        1
    }
    fn act(&self, unit: UnitId) -> u8 {
        self.acts.get(&unit).copied().unwrap_or(0)
    }
    fn level_id(&self, _: &Game, _: UnitId) -> i32 {
        self.level
    }
    fn monster_level(&self, _: UnitId) -> i32 {
        5
    }
    fn life_percent(&self, _: UnitId) -> i32 {
        self.life
    }
    fn add_life(&mut self, _: UnitId, amount: i32) {
        self.log.push(format!("life {amount}"));
    }
    fn ai_state(&self, _: UnitId) -> u32 {
        self.ai_state
    }
    fn alignment(&self, _: UnitId) -> u8 {
        self.align
    }
    fn is_unique(&self, _: UnitId) -> bool {
        self.unique
    }
    fn is_champion(&self, _: UnitId) -> bool {
        self.champion
    }
    fn is_boss(&self, _: UnitId) -> bool {
        false
    }
    fn vision_seen(&self, _: UnitId) -> Option<u32> {
        self.vision
    }
    fn mark_seen(&mut self, _: UnitId) {}
    fn ai_reset(&mut self, unit: UnitId) {
        self.log.push(format!("reset {}", unit.0));
    }
    fn interacting(&self, _: UnitId) -> bool {
        self.interacting
    }
    fn busy(&self, unit: UnitId) -> bool {
        self.busy.contains(&unit)
    }
}

impl AiModes for Fake {
    fn change_mode(&mut self, game: &mut Game, unit: UnitId, m: u8, target: ModeTarget) -> bool {
        self.log.push(format!("mode {m} {target:?}"));
        if (self.walk_fails && matches!(m, mode::WALK | mode::RUN)) || self.fail_modes.contains(&m)
        {
            if let Some(n) = self.fail_think {
                if pending_think(game, unit) <= game.frame {
                    let at = game.frame + n;
                    game.schedule_event(unit, 2, at, None, 0, 0).unwrap();
                }
            }
            return false;
        }
        self.anim.insert(unit, m);
        true
    }
    fn set_anim_mode(&mut self, unit: UnitId, m: u8) {
        self.anim.insert(unit, m);
    }
    fn set_path_steps(&mut self, _: UnitId, steps: i32) {
        self.log.push(format!("steps {steps}"));
    }
    fn path_blocked(&self, _: UnitId) -> bool {
        self.blocked_path
    }
    fn stop_path(&mut self, _: UnitId) {}
    fn set_current_skill(&mut self, _: UnitId, skill: i32) -> bool {
        self.log.push(format!("skill {skill}"));
        skill >= 0
    }
    fn set_skill_flag(&mut self, _: UnitId) {
        self.log.push("skillflag".into());
    }
    fn class_has_mode(&self, _: i32, m: u8) -> bool {
        !(self.can_walk_off && m == mode::WALK)
    }
    fn play_sound(&mut self, _: &mut Game, _: UnitId, sound: u32, to: Option<UnitId>) {
        self.log.push(format!("sound {sound}"));
        if let Some(to) = to {
            self.log.push(format!("sound-to {to:?}"));
        }
    }
    fn knockback_to_gethit(&mut self, _: &mut Game, _: UnitId) {
        self.log.push("gethit".into());
    }
    fn walk_in_radius(&mut self, _: &mut Game, _: UnitId, _: UnitId, a: i32, b: i32) -> bool {
        self.log.push(format!("radius {a} {b}"));
        true
    }
    fn operate_door(&mut self, _: &mut Game, _: UnitId, door: UnitId) {
        self.log.push(format!("door {door:?}"));
    }
}

impl AiWorld for Fake {
    fn in_town(&self, _: &Game, room: RoomId) -> bool {
        self.town.contains(&room)
    }
    fn los_draw(&self, _: &Game, _: RoomId) -> bool {
        !self.no_los_draw
    }
    fn collides(&self, _: &Game, _: UnitId, mask: u16) -> bool {
        self.collides && mask == 0x40
    }
    fn line_blocked(&self, _: &Game, _: UnitId, b: UnitId) -> bool {
        self.line_blocked.contains(&b)
    }
    fn in_melee_range(&self, _: &Game, _: UnitId, b: UnitId) -> bool {
        self.melee.contains(&b)
    }
    fn can_reach_directly(&self, _: &Game, _: UnitId, _: UnitId) -> bool {
        !self.reach_fails
    }
    fn find_spot(&mut self, _: &mut Game, _: UnitId) -> Option<(i32, i32, RoomId)> {
        self.log.push("find_spot".into());
        self.spot
    }
    fn last_dead(&self, _: &Game, room: RoomId) -> [Option<UnitId>; 4] {
        self.last_dead.get(&room).copied().unwrap_or([None; 4])
    }
}

impl AiTargets for Fake {
    fn target_nodes(&self, _: &Game) -> [Vec<UnitId>; 10] {
        let mut out: [Vec<UnitId>; 10] = Default::default();
        for (i, n) in self.nodes.iter().enumerate() {
            out[i] = n.clone();
        }
        out
    }
    fn forced_target(&mut self, _: &mut Game, _: UnitId) -> Option<(UnitId, i32)> {
        self.forced
    }
    fn good_target_search(&mut self, _: &mut Game, _: UnitId, los: bool) -> Option<(UnitId, i32)> {
        self.log.push(format!("good {los}"));
        self.good
    }
    fn choose_alternative(
        &mut self,
        _: &mut Game,
        _: UnitId,
        _: Option<UnitId>,
        alt: UnitId,
    ) -> bool {
        self.log.push(format!("alt {}", alt.0));
        self.take_alt
    }
    fn secondary_target(&mut self, _: &mut Game, _: UnitId) -> (Option<UnitId>, i32, bool) {
        match self.secondary {
            Some((s, d)) => (Some(s), d, false),
            None => (None, 0x7FFF_FFFF, false),
        }
    }
    fn nearest_player(&mut self, _: &mut Game, unit: UnitId) -> (UnitId, bool) {
        self.nearest.unwrap_or((unit, false))
    }
    fn find_door(&mut self, _: &mut Game, _: UnitId) -> Option<UnitId> {
        self.door.map(|(d, _)| d)
    }
    fn door_monster_ok(&self, door: UnitId) -> bool {
        self.door == Some((door, true))
    }
    fn special_walk_target(&mut self, _: &mut Game, _: UnitId) -> Option<(UnitId, i32)> {
        self.log.push("scan 11".into());
        self.special_walk
    }
    fn shaman_corpses(
        &mut self,
        _: &mut Game,
        _: UnitId,
        max_sq: i32,
        own: bool,
    ) -> (Option<UnitId>, u32) {
        self.log.push(format!("corpses {max_sq} {own}"));
        self.corpses
    }
}

impl AiSkills for Fake {
    fn skill_usable(&mut self, _: &mut Game, _: UnitId, skill: i32, _: UnitId) -> bool {
        self.log.push(format!("usable {skill}"));
        !self.skill_unusable
    }
}

/// A monstats row using AI `ai` with Normal aip1..aip5 and `aidel`.
fn monstats(ai: u16, aips: [i16; 5], aidel: u8) -> Monstats {
    let mut r = Monstats::decode(&vec![0u8; Monstats::SIZE]);
    r.ai = ai;
    r.aidel = aidel;
    r.aip1 = aips[0] as u16;
    r.aip2 = aips[1] as u16;
    r.aip3 = aips[2] as u16;
    r.aip4 = aips[3] as u16;
    r.aip5 = aips[4] as u16;
    r.skill1 = 0xFFFF;
    r.skill2 = 0xFFFF;
    r.skill3 = 0xFFFF;
    r
}

struct World {
    game: Game,
    fake: Fake,
    store: AiStore,
    monstats: Vec<Monstats>,
    monstats2: Vec<Monstats2>,
    levels: Vec<Levels>,
    modes: Vec<[u8; 3]>,
    room: RoomId,
    mon: UnitId,
    player: UnitId,
}

impl World {
    fn new(row: Monstats) -> Self {
        let mut game = Game::new();
        game.lists.ensure_act(0).unwrap();
        let room = game.lists.create_room(0).unwrap();
        let mon = game
            .spawn_unit(UnitType::Monster, Some(room), false)
            .unwrap();
        let player = game.spawn_unit(UnitType::Player, Some(room), true).unwrap();
        let mut fake = Fake {
            life: 100,
            ..Fake::default()
        };
        fake.pos.insert(mon, (100, 100));
        fake.pos.insert(player, (105, 100));
        let mut store = AiStore::new();
        store.entry(mon).control = Some(AiControl::default());
        let mut w = Self {
            game,
            fake,
            store,
            monstats: vec![row],
            monstats2: vec![Monstats2::decode(&vec![0u8; Monstats2::SIZE])],
            levels: vec![Levels::decode(&vec![0u8; Levels::SIZE])],
            modes: vec![[0; 3]],
            room,
            mon,
            player,
        };
        let mon = w.mon;
        w.with(|g, cx| install(g, cx, mon, 0));
        w
    }

    fn with<R>(&mut self, f: impl FnOnce(&mut Game, &mut Ctx<'_, Fake>) -> R) -> R {
        let mut cx = Ctx {
            tables: AiTables {
                monstats: &self.monstats,
                monstats2: &self.monstats2,
                levels: &self.levels,
                skill_modes: &self.modes,
            },
            info: GameInfo::default(),
            store: &mut self.store,
            world: &mut self.fake,
        };
        f(&mut self.game, &mut cx)
    }

    fn seed(&mut self, lo: u32) {
        self.fake.seeds.insert(self.mon, Seed::init_low(lo));
    }

    /// Runs the current AI function with a target at distance `d`.
    fn run(&mut self, combat: bool, d: i32) {
        let p = TickParam {
            target: Some(self.player),
            distance: d,
            combat,
            class: 0,
            class2: 0,
        };
        let mon = self.mon;
        self.with(|g, cx| {
            let f = cx.store.control(mon).unwrap().function;
            run_function(g, cx, f, mon, &p)
        });
    }

    /// Next think frames of the monster.
    fn thinks(&self) -> Vec<i32> {
        self.game
            .timers
            .unit_timers(self.mon)
            .into_iter()
            .filter(|&t| self.game.timers.event(t).map(|e| e.0) == Some(EVENT_THINK))
            .filter_map(|t| self.game.timers.expire(t))
            .collect()
    }
}

// Covers: specs/sim/rng.md §2
#[test]
fn draw_vectors() {
    let want = [
        [51, 31, 12, 93],
        [87, 64, 71, 25],
        [53, 46, 20, 96],
        [0, 42, 13, 80],
    ];
    for (s, w) in SEEDS.iter().zip(want) {
        let mut seed = Seed::init_low(*s);
        let got: Vec<u32> = (0..4).map(|_| seed.step() % 100).collect();
        assert_eq!(got, w, "seed {s}");
    }
}

// Covers: specs/monsters/ai.md §7.2
#[test]
fn wander_vectors() {
    let want = [
        ((103, 100), (3_064_641_593, 280_084_454)),
        ((97, 98), (4_008_788_125, 674_806_599)),
        ((103, 100), (4_097_238_796, 611_865_796)),
        ((98, 103), (4_002_178_480, 452_242_291)),
    ];
    for (s, (pt, (lo, hi))) in SEEDS.iter().zip(want) {
        let mut seed = Seed::init_low(*s);
        assert_eq!(wander_point(&mut seed, (100, 100), 3), pt, "seed {s}");
        assert_eq!(seed, Seed::new(lo, hi), "seed {s}");
    }
}

fn last_mode(w: &World) -> String {
    w.fake.modes().last().cloned().unwrap_or_default()
}

fn walk_point(x: i32, y: i32) -> String {
    format!("mode 2 Point({x}, {y})")
}

// Covers: specs/monsters/ai.md §9.3 r1, §9.3 r2, §9.3 r3
#[test]
fn zombie_vectors() {
    let row = || monstats(3, [30, 10, 0, 20, 0], 15);
    let a1 = |w: &World| format!("mode 4 Unit({:?})", w.player);
    let a2 = |w: &World| format!("mode 5 Unit({:?})", w.player);
    // C.
    for (s, want_a1) in SEEDS.iter().zip([false, false, false, true]) {
        let mut w = World::new(row());
        w.seed(*s);
        w.run(true, 1);
        let want = if want_a1 { a1(&w) } else { a2(&w) };
        assert_eq!(last_mode(&w), want, "seed {s}");
    }
    // Not C, D = 5.
    let want = [
        Some((97, 98)),
        Some((99, 103)),
        Some((100, 103)),
        None, // run to T
    ];
    for (s, want) in SEEDS.iter().zip(want) {
        let mut w = World::new(row());
        w.seed(*s);
        w.run(false, 5);
        match want {
            Some((x, y)) => assert_eq!(last_mode(&w), walk_point(x, y), "seed {s}"),
            None => {
                assert_eq!(last_mode(&w), format!("mode 15 Unit({:?})", w.player));
                assert_eq!(w.store.get(w.mon).unwrap().velocity.speed, 100);
            }
        }
    }
    // Not C, D = 12: no draw before the wander.
    let want = [(103, 100), (97, 98), (103, 100), (98, 103)];
    for (s, (x, y)) in SEEDS.iter().zip(want) {
        let mut w = World::new(row());
        w.seed(*s);
        w.run(false, 12);
        assert_eq!(last_mode(&w), walk_point(x, y), "seed {s}");
    }
    // Level 17 always runs; AI state 3 runs without a draw.
    let mut w = World::new(row());
    w.fake.level = 17;
    w.run(false, 12);
    assert_eq!(last_mode(&w), format!("mode 15 Unit({:?})", w.player));
    let mut w = World::new(row());
    w.fake.ai_state = 19;
    w.seed(1);
    w.run(false, 5);
    assert_eq!(w.fake.seeds[&w.mon], Seed::init_low(1));
}

// Covers: specs/monsters/ai.md §9.4 r1
#[test]
fn fallen_vectors() {
    let want = [None, None, None, Some(5)];
    for (s, want) in SEEDS.iter().zip(want) {
        let mut w = World::new(monstats(6, [30, 10, 50, 20, 0], 15));
        w.seed(*s);
        w.run(true, 1);
        match want {
            None => {
                assert!(w.fake.modes().is_empty(), "seed {s}");
                assert_eq!(w.thinks(), [10], "seed {s}");
            }
            Some(m) => {
                assert_eq!(last_mode(&w), format!("mode {m} Unit({:?})", w.player));
                assert!(w.thinks().is_empty(), "the fallen deleted its thinks");
            }
        }
    }
}

// Covers: specs/monsters/ai.md §9.5 r2
#[test]
fn brute_vectors() {
    for (s, m) in SEEDS.iter().zip([4, 5, 5, 4]) {
        let mut w = World::new(monstats(7, [0, 0, 100, 45, 0], 15));
        w.seed(*s);
        w.run(true, 1);
        assert_eq!(last_mode(&w), format!("mode {m} Unit({:?})", w.player));
    }
    // Not C: speed 100 − clamp(life%, 40, 100), method 13.
    let mut w = World::new(monstats(7, [0, 0, 100, 45, 0], 15));
    w.fake.life = 10;
    w.run(false, 9);
    let v = w.store.get(w.mon).unwrap().velocity;
    assert_eq!((v.method, v.speed), (13, 60));
    assert_eq!(last_mode(&w), format!("mode 2 Unit({:?})", w.player));
}

// Covers: specs/monsters/ai.md §9.7 r4, §9.7 r5, §9.7 r6
#[test]
fn quill_rat_vectors() {
    for (s, escape) in SEEDS.iter().zip([true, true, true, false]) {
        let mut w = World::new(monstats(14, [10, 35, 0, 2, 0], 15));
        w.seed(*s);
        w.run(false, 5);
        if escape {
            // Escape from T at (105, 100) by 2: own + sign(own − T)·2.
            assert_eq!(last_mode(&w), walk_point(98, 100), "seed {s}");
        } else {
            assert_eq!(last_mode(&w), format!("mode 5 Unit({:?})", w.player));
        }
    }
    // D ≥ aip1: wander max(aip4, 3).
    let mut w = World::new(monstats(14, [10, 35, 0, 2, 0], 15));
    w.seed(1);
    w.run(false, 10);
    assert_eq!(last_mode(&w), walk_point(103, 100));
}

// Covers: specs/monsters/ai.md §7.2
#[test]
fn circle_vectors() {
    for (s, m) in SEEDS.iter().zip([5, 5, 6, 5]) {
        let mut w = World::new(monstats(3, [0; 5], 15));
        w.seed(*s);
        let (mon, pl) = (w.mon, w.player);
        w.with(|g, cx| circle(g, cx, mon, Some(pl), 4, false));
        assert_eq!(w.store.get(mon).unwrap().velocity.method, m, "seed {s}");
        assert_eq!(w.store.get(mon).unwrap().velocity.steps, 4);
    }
}

// Covers: specs/monsters/ai.md §9.8 r1, §9.8 r3
#[test]
fn corrupt_lancer_vectors() {
    for (s, walk) in SEEDS.iter().zip([true, false, true, true]) {
        let mut w = World::new(monstats(36, [60, 75, 9, 0, 15], 15));
        w.seed(*s);
        w.run(false, 10);
        if walk {
            assert_eq!(last_mode(&w), format!("mode 2 Unit({:?})", w.player));
            assert!(w.fake.log.contains(&"steps 3".to_string()), "seed {s}");
        } else {
            assert!(w.fake.modes().is_empty());
            assert_eq!(w.thinks(), [9]);
        }
    }
    // D > aip5: run with MeleeRng steps, param 0 := 1.
    let mut w = World::new(monstats(36, [60, 75, 9, 0, 15], 15));
    w.monstats2[0].meleerng = 4;
    w.run(false, 20);
    assert_eq!(last_mode(&w), format!("mode 15 Unit({:?})", w.player));
    assert_eq!(w.store.control(w.mon).unwrap().params[0], 1);
}

// Covers: specs/monsters/ai.md §9.2
#[test]
fn idle_ai_thinks_every_200() {
    for ai in [1, 100] {
        let mut w = World::new(monstats(ai, [0; 5], 15));
        w.game.frame = 50;
        w.run(false, 0);
        assert_eq!(w.thinks(), [250]);
    }
}

// ---- §1 scheduling ----------------------------------------------------

// Covers: specs/monsters/ai.md §1.2, §1.3 r2, §1.3 r3
#[test]
fn neutral_start_schedules_aidel() {
    let cases = [
        (15, false, None, vec![115]),
        (0, false, None, vec![115]),
        (15, true, None, vec![145]),
        (15, false, Some(107), vec![107]),
    ];
    for (aidel, stunned, pending, want) in cases {
        let mut w = World::new(monstats(3, [0; 5], aidel));
        w.game.frame = 100;
        if stunned {
            w.fake.states.insert((w.mon, state::STUNNED));
        }
        if let Some(at) = pending {
            w.game.schedule_event(w.mon, 2, at, None, 0, 0).unwrap();
        }
        let mon = w.mon;
        w.with(|g, cx| neutral_mode_start(g, cx, mon));
        assert_eq!(w.thinks(), want);
    }
}

// Covers: specs/monsters/ai.md §1.3 r1, §edge-cases-original-bugs r2
#[test]
fn aidel_difficulty_gate() {
    // Edge case 2: the Normal column unless game +0x6A or +0x74 is set.
    let mut row = monstats(3, [0; 5], 15);
    row.aidel_n = 14;
    let mut w = World::new(row);
    let mut cx = Ctx {
        tables: AiTables {
            monstats: &w.monstats,
            monstats2: &w.monstats2,
            levels: &w.levels,
            skill_modes: &w.modes,
        },
        info: GameInfo {
            difficulty: 1,
            ..GameInfo::default()
        },
        store: &mut w.store,
        world: &mut w.fake,
    };
    assert_eq!(cx.aidel(0), 15);
    cx.info.game_type_ex = 1;
    assert_eq!(cx.aidel(0), 14);
}

// Covers: specs/monsters/ai.md §2.3 r3
#[test]
fn target_mode_1_idle_by_distance() {
    for (d, want) in [
        (Some(40), 125),
        (Some(30), 120),
        (Some(20), 110),
        (None, 125),
    ] {
        let mut w = World::new(monstats(3, [0; 5], 15));
        w.game.frame = 100;
        if let Some(d) = d {
            w.fake.pos.insert(w.player, (100 + d, 100));
            w.fake.nodes = vec![vec![w.player]];
        }
        // aidist 0 → 35: nothing within reach at d ≥ 35; closer players
        // become targets, so test the no-target idle with aidist 1.
        w.monstats[0].aidist = 1;
        let mon = w.mon;
        w.with(|g, cx| think(g, cx, mon));
        assert_eq!(w.thinks(), [want], "d {d:?}");
    }
}

// Covers: specs/monsters/ai.md §2.3 text
#[test]
fn target_mode_4_idles_20() {
    let mut w = World::new(monstats(15, [0; 5], 15));
    w.game.frame = 100;
    let mon = w.mon;
    w.with(|g, cx| think(g, cx, mon));
    assert_eq!(w.thinks(), [120]);
    assert!(w.fake.modes().is_empty());
}

// Covers: specs/monsters/ai.md §1.2
#[test]
fn idle_helpers() {
    let mut w = World::new(monstats(3, [0; 5], 15));
    w.game.frame = 100;
    let mon = w.mon;
    w.with(|g, cx| idle(g, cx, mon, 0));
    assert_eq!(w.thinks(), [101]);
    // Idle in neutral first changes a non-neutral unit's mode.
    w.fake.anim.insert(mon, mode::WALK);
    w.with(|g, cx| idle(g, cx, mon, 5));
    assert_eq!(w.thinks(), [105]);
    assert_eq!(last_mode(&w), format!("mode 1 Unit({mon:?})"));
    // 0x005DE130: pending at +4 kept for N = 10; at +12 replaced.
    let mut w = World::new(monstats(3, [0; 5], 15));
    w.game.frame = 100;
    let mon = w.mon;
    w.game.schedule_event(mon, 2, 104, None, 0, 0).unwrap();
    w.with(|g, cx| idle_if_later(g, cx, mon, 10));
    assert_eq!(w.thinks(), [104]);
    let mut w = World::new(monstats(3, [0; 5], 15));
    w.game.frame = 100;
    let mon = w.mon;
    w.game.schedule_event(mon, 2, 112, None, 0, 0).unwrap();
    w.with(|g, cx| idle_if_later(g, cx, mon, 10));
    assert_eq!(w.thinks(), [110]);
}

// Covers: specs/monsters/ai.md §1.1, §edge-cases-original-bugs r10
#[test]
fn state_54_schedule_cancels_pending_thinks() {
    // Edge case 10.
    let mut w = World::new(monstats(3, [0; 5], 15));
    let mon = w.mon;
    w.game.schedule_event(mon, 2, 50, None, 0, 0).unwrap();
    w.game.schedule_event(mon, 2, 60, None, 0, 0).unwrap();
    w.fake.states.insert((mon, state::UNINTERRUPTABLE));
    w.with(|g, cx| schedule_think(g, cx, mon, 70));
    assert_eq!(w.thinks(), [70]);
    assert!(!w.fake.has_state(mon, state::UNINTERRUPTABLE));
}

// Covers: specs/monsters/ai.md §1.2
#[test]
fn update_ai_callback_rules() {
    let mut w = World::new(monstats(3, [0; 5], 15));
    w.game.frame = 10;
    let mon = w.mon;
    w.game.schedule_event(mon, 2, 25, None, 0, 0).unwrap();
    w.with(|g, cx| update_ai_callback(g, cx, mon));
    assert_eq!(w.thinks(), [12]);
    for m in [mode::DEATH, mode::DEAD, mode::WALK] {
        w.fake.anim.insert(mon, m);
        w.with(|g, cx| update_ai_callback(g, cx, mon));
        assert!(w.thinks().is_empty(), "mode {m}");
    }
    // Base class 110 (vulture1) also in other modes.
    w.monstats[0].baseid = 110;
    w.with(|g, cx| update_ai_callback(g, cx, mon));
    assert_eq!(w.thinks(), [12]);
}

// Covers: specs/monsters/ai.md §1.5 r1, §1.5 r2
#[test]
fn creation_pair_and_room_entry() {
    // §1.5: neutral start +15, then 0x00573780 cancels it and adds +2.
    let mut w = World::new(monstats(3, [0; 5], 15));
    let (mon, room) = (w.mon, w.room);
    w.with(|g, cx| neutral_mode_start(g, cx, mon));
    assert_eq!(w.thinks(), [15]);
    w.with(|g, cx| update_ai_callback(g, cx, mon));
    assert_eq!(w.thinks(), [2]);
    w.game.frame = 40;
    w.with(|g, cx| client_entered_room(g, cx, room));
    assert_eq!(w.thinks(), [42]);
}

// Covers: specs/monsters/ai.md §1.2
#[test]
fn knockback_end_rules() {
    let mut w = World::new(monstats(3, [0; 5], 15));
    let mon = w.mon;
    w.with(|g, cx| knockback_end(g, cx, mon));
    assert!(w.thinks().is_empty());
    assert!(w.fake.log.contains(&"gethit".to_string()));
    w.monstats[0].baseid = 78;
    w.with(|g, cx| knockback_end(g, cx, mon));
    assert_eq!(w.thinks(), [15]);
}

// Covers: specs/monsters/ai.md §1.4
#[test]
fn mode_end_inline_think() {
    // Walk end: neutral, think at once (Idle AI → +200).
    let mut w = World::new(monstats(1, [0; 5], 15));
    let mon = w.mon;
    w.fake.anim.insert(mon, mode::WALK);
    w.with(|g, cx| mode_end(g, cx, mon, mode::WALK));
    assert_eq!(w.thinks(), [200]);
    // Frozen and alive: no think.
    let mut w = World::new(monstats(1, [0; 5], 15));
    let mon = w.mon;
    w.fake.states.insert((mon, state::FREEZE));
    w.with(|g, cx| mode_end(g, cx, mon, mode::RUN));
    assert!(w.thinks().is_empty());
    // An attack end requests neutral.
    let mut w = World::new(monstats(1, [0; 5], 15));
    let mon = w.mon;
    w.with(|g, cx| mode_end(g, cx, mon, mode::ATTACK1));
    assert_eq!(last_mode(&w), format!("mode 1 Unit({mon:?})"));
}

// Covers: specs/monsters/ai.md §1.1, §1.6, §edge-cases-original-bugs r1
#[test]
fn freeze_drops_thinks_and_type_10_resets() {
    let mut w = World::new(monstats(1, [0; 5], 15));
    let mon = w.mon;
    w.game.schedule_event(mon, 2, 1, None, 0, 0).unwrap();
    w.game.schedule_event(mon, 10, 1, None, 0, 0).unwrap();
    w.fake.states.insert((mon, state::FREEZE));
    struct Nothing;
    impl EventDispatch for Nothing {
        fn run_event(&mut self, _: &mut Game, _: &TimerRun) {}
    }
    w.game.frame = 1;
    let mut d = MonsterDispatch {
        cx: Ctx {
            tables: AiTables {
                monstats: &w.monstats,
                monstats2: &w.monstats2,
                levels: &w.levels,
                skill_modes: &w.modes,
            },
            info: GameInfo::default(),
            store: &mut w.store,
            world: &mut w.fake,
        },
        next: &mut Nothing,
    };
    crate::tick::run_timer_events(&mut w.game, &mut d);
    // The think was dropped (nothing rescheduled); the reset ran.
    assert!(w.thinks().is_empty());
    assert!(w.fake.log.contains(&format!("reset {}", mon.0)));
}

// ---- §2–§5 dispatch and targets -------------------------------------

// Covers: specs/monsters/ai.md §2.2 r1
#[test]
fn stun_idles_3() {
    let mut w = World::new(monstats(3, [0; 5], 15));
    let mon = w.mon;
    w.fake.states.insert((mon, state::STUNNED));
    w.with(|g, cx| think(g, cx, mon));
    assert_eq!(w.thinks(), [3]);
}

// Covers: specs/monsters/ai.md §5.2 r5, §5.2 r6, §5.2 r7
#[test]
fn main_search_picks_nearest_qualifying_player() {
    let mut w = World::new(monstats(3, [0; 5], 15));
    let other = w
        .game
        .spawn_unit(UnitType::Player, Some(w.room), true)
        .unwrap();
    w.fake.pos.insert(w.player, (110, 100));
    w.fake.pos.insert(other, (104, 100));
    w.fake.nodes = vec![vec![w.player], vec![other]];
    w.fake.melee.insert(other);
    let mon = w.mon;
    let s = w.with(|g, cx| main_search(g, cx, mon));
    assert_eq!((s.target, s.distance, s.combat), (Some(other), 4, true));
    // A dead head is not chosen but still sets M.
    w.fake.dead.insert(other);
    let s = w.with(|g, cx| main_search(g, cx, mon));
    assert_eq!(s.target, Some(w.player));
    // Players in town do not count.
    let room = w.room;
    w.fake.town.insert(room);
    let s = w.with(|g, cx| main_search(g, cx, mon));
    assert_eq!((s.target, s.distance), (None, target::NO_DISTANCE));
}

// Covers: specs/monsters/ai.md §2.4 r1
#[test]
fn boss_sound_once() {
    let mut w = World::new(monstats(3, [30, 10, 0, 20, 0], 15));
    w.fake.unique = true;
    w.fake.nodes = vec![vec![w.player]];
    let mon = w.mon;
    w.with(|g, cx| think(g, cx, mon));
    assert_eq!(w.fake.log.iter().filter(|s| *s == "sound 16").count(), 1);
    assert_eq!(w.thinks(), [20]);
    w.with(|g, cx| think(g, cx, mon));
    assert_eq!(w.fake.log.iter().filter(|s| *s == "sound 16").count(), 1);
}

// Covers: specs/monsters/ai.md §6, §edge-cases-original-bugs r4
#[test]
fn distances() {
    assert_eq!(distance_no_size((0, 0), (10, 4)), 12);
    assert_eq!(distance_no_size((0, 0), (3, 3)), 4);
    // Full size clamps each axis at 0 (edge case 4).
    assert_eq!(distance_full_size((0, 0), 2, (1, 10)), 8);
    assert_eq!(distance_full_size((0, 0), 2, (0, 0)), 0);
}

// Covers: specs/monsters/ai.md §7.3
#[test]
fn velocity_request() {
    let mut w = World::new(monstats(3, [0; 5], 15));
    let mon = w.mon;
    w.with(|_, cx| {
        set_velocity(cx, mon, 1, 50, 100);
        set_velocity(cx, mon, 0, 0, 5);
        set_velocity(cx, mon, 2, 127, 0);
    });
    let v = w.store.get(mon).unwrap().velocity;
    assert_eq!((v.method, v.speed, v.steps), (7, 50, 5));
    assert_eq!(
        w.store.unhandled,
        [Unhandled::VelocityAssert {
            unit: mon,
            speed: 127
        }]
    );
}

// Covers: specs/monsters/ai.md §7.2
#[test]
fn failed_move_fallback_draw() {
    let mut w = World::new(monstats(3, [0; 5], 15));
    w.fake.walk_fails = true;
    w.seed(4_014_346_870); // 0 < 70 → wander 4 (which fails too)
    let (mon, pl) = (w.mon, w.player);
    let ok = w.with(|g, cx| walk_to(g, cx, mon, Some(pl), 7));
    assert!(ok);
    assert!(w.thinks().is_empty(), "flag 4 deleted the thinks");
    assert_ne!(w.store.control(mon).unwrap().flags & flag::FORCE_LOS, 0);
    w.seed(12345); // 87 ≥ 70 → idle 10
    w.with(|g, cx| walk_to(g, cx, mon, Some(pl), 7));
    assert_eq!(w.thinks(), [10]);
}

// Covers: specs/monsters/ai.md §8
#[test]
fn commands() {
    let mut w = World::new(monstats(6, [0; 5], 15));
    let mon = w.mon;
    w.with(|_, cx| {
        assert_eq!(current_command(cx, mon), None);
        copy_command(
            cx,
            mon,
            AiCommand {
                params: [1, 0, 0, 0, 0],
            },
        );
        copy_command(
            cx,
            mon,
            AiCommand {
                params: [2, 0, 0, 0, 0],
            },
        );
        assert_eq!(current_command(cx, mon).unwrap().params[0], 2);
        free_current_command(cx, mon);
        assert_eq!(current_command(cx, mon).unwrap().params[0], 1);
        free_current_command(cx, mon);
        assert_eq!(current_command(cx, mon), None);
    });
}

// ---- §3 install and tables ----------------------------------------------

// Covers: specs/monsters/ai.md §3.3 r2, §3.3 r3, §3.3 r4
#[test]
fn install_sets_think_and_alternate() {
    let mut w = World::new(monstats(15, [0; 5], 15));
    let mon = w.mon;
    assert_eq!(w.store.control(mon).unwrap().function, 0x005F_1800);
    w.store.control_mut(mon).unwrap().params = [1, 2, 3];
    // Re-install while running: the alternate function, nothing reset.
    w.with(|g, cx| install(g, cx, mon, 0));
    let c = w.store.control(mon).unwrap();
    assert_eq!((c.function, c.params), (0x005F_1750, [1, 2, 3]));
    // An init function is a logged stub; a record without think → Idle.
    let mut w = World::new(monstats(43, [0; 5], 15));
    let mon = w.mon;
    assert!(w.store.unhandled.contains(&Unhandled::Function {
        addr: 0x005F_6630,
        unit: mon
    }));
    w.with(|g, cx| install(g, cx, mon, 0));
    let _ = w;
    let mut w = World::new(monstats(3, [0; 5], 15));
    let mon = w.mon;
    w.with(|g, cx| install(g, cx, mon, 18)); // state ≥ 18: nothing
    assert_eq!(w.store.control(mon).unwrap().function, 0x005E_FE20);
}

// Covers: specs/monsters/ai.md §3.2
#[test]
fn special_states_10_to_12_need_switchai() {
    let mut w = World::new(monstats(3, [0; 5], 15));
    let mon = w.mon;
    w.store.control_mut(mon).unwrap().special_state = 11;
    let r = w.with(|_, cx| cx.record(mon));
    assert_eq!(r, AI_TABLE[3]);
    w.monstats[0].switchai = true;
    let r = w.with(|_, cx| cx.record(mon));
    assert_eq!(r, SPECIAL_TABLE[11]);
    w.store.control_mut(mon).unwrap().special_state = 8;
    w.monstats[0].switchai = false;
    let r = w.with(|_, cx| cx.record(mon));
    assert_eq!(r, SPECIAL_TABLE[8]);
    // An AI index outside 0..147 uses record 0.
    w.store.control_mut(mon).unwrap().special_state = 0;
    w.monstats[0].ai = 0xFFFF;
    let r = w.with(|_, cx| cx.record(mon));
    assert_eq!(r, AI_TABLE[0]);
}

#[test]
fn stub_ai_logged() {
    let mut w = World::new(monstats(32, [0; 5], 15)); // Npc (summarized)
    w.run(false, 0);
    let mon = w.mon;
    assert_eq!(
        w.store.unhandled,
        [Unhandled::Function {
            addr: 0x005E_7130,
            unit: mon
        }]
    );
}

/// Checks `AI_TABLE` against the catalogue: 148 rows, columns index,
/// think, init, alt, target mode. Returns the first disagreement.
fn check_ai_table(tsv: &str, table: &[AiRecord]) -> Result<(), String> {
    let rows: Vec<&str> = tsv.lines().skip(1).filter(|l| !l.is_empty()).collect();
    if rows.len() != table.len() {
        return Err(format!("{} rows, table has {}", rows.len(), table.len()));
    }
    let addr = |s: &str| -> Result<u32, String> {
        if s == "-" {
            return Ok(0);
        }
        u32::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|e| e.to_string())
    };
    for (i, line) in rows.iter().enumerate() {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() != 11 || c[0] != i.to_string() {
            return Err(format!("row {i}: bad row"));
        }
        let rec = AiRecord {
            think: addr(c[2])?,
            init: addr(c[3])?,
            alt: addr(c[4])?,
            target_mode: c[5].parse().map_err(|_| format!("row {i}: target mode"))?,
        };
        if rec != table[i] {
            return Err(format!("index {i}: tsv {rec:x?}"));
        }
    }
    Ok(())
}

// Covers: specs/monsters/ai.md §3.2
#[test]
fn ai_table_matches_tsv() {
    check_ai_table(AI_FUNCTIONS_TSV, &AI_TABLE).unwrap();
    // §3.2 target-mode counts.
    let count = |m| AI_TABLE.iter().filter(|r| r.target_mode == m).count();
    assert_eq!(
        [count(0), count(1), count(2), count(4), count(5)],
        [26, 104, 16, 1, 1]
    );
}

#[test]
fn ai_table_check_catches_perturbations() {
    let bad = AI_FUNCTIONS_TSV.replacen("0x005EFE20", "0x005EFE21", 1);
    let err = check_ai_table(&bad, &AI_TABLE).unwrap_err();
    assert!(err.starts_with("index 3:"), "{err}");
    let bad = AI_FUNCTIONS_TSV.replacen("\t0x005F1750\t4\t", "\t0x005F1750\t5\t", 1);
    let err = check_ai_table(&bad, &AI_TABLE).unwrap_err();
    assert!(err.starts_with("index 15:"), "{err}");
}

#[test]
fn implemented_matches_catalogue() {
    // Every spec'd-here think has a body here, and only those (Npc, 32,
    // is `summarized` and stays a stub).
    let mut spec: Vec<u8> = AI_FUNCTIONS_TSV
        .lines()
        .skip(1)
        .filter(|l| l.ends_with("\tspec'd-here"))
        .map(|l| l.split('\t').next().unwrap().parse().unwrap())
        .collect();
    spec.sort_unstable();
    let mut ours: Vec<u8> = IMPLEMENTED.iter().map(|&(_, i)| i).collect();
    ours.sort_unstable();
    assert_eq!(ours, spec);
    for (addr, i) in IMPLEMENTED {
        assert_eq!(AI_TABLE[i as usize].think, addr, "index {i}");
        assert!(implemented(addr));
    }
}
mod rules;
