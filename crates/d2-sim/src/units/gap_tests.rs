// Spec: specs/sim/units.md; specs/sim/unit-events.tsv
//! Gap tests of `units.md` (M14, M08): one test per rule group the
//! existing tests did not claim, each on a recording fake ([`Probe`]) so
//! the order of steps, the timers left and the hooks called are visible.

use crate::game::Game;
use crate::rng::Seed;
use crate::stats::states::state;
use crate::stats::{stat, StatHost};
use crate::tick::events::{event, monster_dropped_when_frozen};
use crate::tick::run_timer_events;
use crate::tick::timer::TimerClass;

use super::anim;
use super::dispatch::UnitSystem;
use super::hooks::{Sim, UnitHooks};
use super::lifecycle::{allocate, remove, AllocRequest, LifecycleHooks};
use super::modes::{
    self, monster_mode, player_mode, player_start_kind, MonsterModeRecord, Moves, UnitError,
    MONSTER_MODES, MONSTER_MODE_FALLBACK, MONSTER_MOVES,
};
use super::record::{flags, flags2, AnimRecord, Sequence, INITIAL_NODE_INDEX};
use super::tests::{bytes, data, pending, EVENTS_TSV};
use super::{RoomId, UnitId, UnitType};

// ---- recording fake ----------------------------------------------------------------

#[derive(Default)]
struct Probe {
    log: Vec<String>,
    anim: Option<AnimRecord>,
    rate: i16,
    bonus: i32,
    has_path: bool,
    town: bool,
    reject: bool,
    sequence: Option<Sequence>,
    action_result: Option<u32>,
    item_flagged: bool,
    /// The mode the next monster mode function sets (a start function).
    start_mode: Option<u32>,
    start_fails: bool,
    class_record: Option<MonsterModeRecord>,
}

impl Probe {
    fn push(&mut self, s: String) {
        self.log.push(s);
    }
}

impl StatHost for Probe {}

impl UnitHooks for Probe {
    fn anim_record(&mut self, _: &Sim<'_>, _: UnitId) -> Option<AnimRecord> {
        self.anim
    }
    fn anim_rate(&mut self, _: &Sim<'_>, _: UnitId) -> i16 {
        self.rate
    }
    fn frame_bonus(&mut self, _: &Sim<'_>, _: UnitId) -> i32 {
        self.bonus
    }
    fn load_sequence(&mut self, _: &Sim<'_>, _: UnitId) -> Option<Sequence> {
        self.sequence.clone()
    }
    fn has_path(&mut self, _: &Sim<'_>, _: UnitId) -> bool {
        self.has_path
    }
    fn reinit_anim(&mut self, _: &mut Sim<'_>, _: UnitId) {
        self.push("reinit".into());
    }
    fn drop_combat_entries(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let mode = sim.units.get(unit).map(|r| r.mode);
        self.push(format!("combat {mode:?}"));
    }
    fn room_flag(&mut self, _: &Sim<'_>, _: UnitId) -> bool {
        self.town
    }
    fn player_request_check(&mut self, _: &mut Sim<'_>, _: UnitId, _: u32) -> bool {
        !self.reject
    }
    fn player_death(&mut self, _: &mut Sim<'_>, _: UnitId) {
        self.push("death".into());
    }
    fn player_corpse(&mut self, _: &mut Sim<'_>, _: UnitId) {
        self.push("corpse".into());
    }
    fn player_knockback_path(&mut self, _: &mut Sim<'_>, _: UnitId) {
        self.push("kbpath".into());
    }
    fn player_skill_start(&mut self, _: &mut Sim<'_>, _: UnitId) {
        self.push("skillstart".into());
    }
    fn player_movement_step(&mut self, _: &mut Sim<'_>, _: UnitId, a1: u32, a2: u32) -> u32 {
        self.push(format!("step {a1} {a2}"));
        self.action_result.unwrap_or(1)
    }
    fn player_action_frame(&mut self, _: &mut Sim<'_>, _: UnitId, a1: u32, a2: u32) -> u32 {
        self.push(format!("action {a1} {a2}"));
        self.action_result.unwrap_or(1)
    }
    fn player_item_row_flagged(&mut self, _: &Sim<'_>, _: UnitId) -> bool {
        self.item_flagged
    }
    fn player_attack_cleanup(&mut self, _: &mut Sim<'_>, _: UnitId) {
        self.push("cleanup".into());
    }
    fn update_trade(&mut self, _: &mut Sim<'_>, _: UnitId, a1: u32, a2: u32) {
        self.push(format!("trade {a1} {a2}"));
    }
    fn monster_mode_bookkeeping(&mut self, _: &mut Sim<'_>, _: UnitId, mode: u32) {
        self.push(format!("book {mode}"));
    }
    fn monster_class_record(
        &mut self,
        _: &Sim<'_>,
        _: UnitId,
        _: u32,
    ) -> Option<MonsterModeRecord> {
        self.class_record
    }
    fn monster_mode_function(&mut self, sim: &mut Sim<'_>, unit: UnitId, address: u32) -> bool {
        self.push(format!("mfn {address:#x}"));
        if let Some(m) = self.start_mode.take() {
            modes::set_mode(sim, self, unit, m).expect("start sets its mode");
        }
        !self.start_fails
    }
    fn ai_think(&mut self, _: &mut Sim<'_>, _: UnitId, a1: u32, a2: u32) {
        self.push(format!("ai {a1} {a2}"));
    }
    fn monster_umod(&mut self, _: &mut Sim<'_>, _: UnitId, a1: u32, a2: u32) {
        self.push(format!("umod {a1} {a2}"));
    }
    fn ai_reset(&mut self, _: &mut Sim<'_>, _: UnitId, a1: u32, a2: u32) {
        self.push(format!("aireset {a1} {a2}"));
    }
    fn periodic_skills(&mut self, _: &mut Sim<'_>, _: UnitId, a1: u32, a2: u32) {
        self.push(format!("pskills {a1} {a2}"));
    }
    fn missile_do(&mut self, _: &mut Sim<'_>, _: UnitId) {
        self.push("missile".into());
    }
    fn object_event(&mut self, _: &mut Sim<'_>, _: UnitId, ev: u8) {
        self.push(format!("object {ev}"));
    }
    fn item_replenish(&mut self, _: &mut Sim<'_>, _: UnitId) {
        self.push("replenish".into());
    }
    fn free_hover(&mut self, _: &mut Sim<'_>, _: UnitId) {
        self.push("hover".into());
    }
}

impl LifecycleHooks for Probe {
    fn init_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId, req: &AllocRequest) {
        let stats = sim.stats.unit_list(unit).is_some();
        self.push(format!("init {:?} stats={stats}", req.ty));
    }
    fn free_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let listed = sim.game.lists.unit(unit).is_some();
        let timers = sim.game.timers.unit_timers(unit).len();
        let record = sim.units.get(unit).is_some();
        self.push(format!(
            "free listed={listed} timers={timers} record={record}"
        ));
    }
}

type Sys = UnitSystem<Probe>;

fn system() -> Sys {
    UnitSystem::new(crate::stats::tests::data(), data(), Probe::default())
}

fn request(ty: UnitType, class: u32, mode: u32, room: Option<RoomId>) -> AllocRequest {
    AllocRequest {
        ty,
        class,
        room,
        add: true,
        fixed_guid: None,
        mode,
        allied: false,
    }
}

fn alloc(game: &mut Game, sys: &mut Sys, req: &AllocRequest) -> UnitId {
    let mut seed = Seed::init();
    sys.with(game, |sim, hooks| allocate(sim, hooks, &mut seed, req))
        .expect("allocate")
        .expect("unit")
}

/// [`alloc`] without a room; the init's log line is dropped.
fn spawn(game: &mut Game, sys: &mut Sys, ty: UnitType, class: u32, mode: u32) -> UnitId {
    let u = alloc(game, sys, &request(ty, class, mode, None));
    sys.hooks.log.clear();
    u
}

fn room(game: &mut Game, act: u8) -> RoomId {
    game.lists.ensure_act(act).expect("act");
    let r = game.lists.create_room(act).expect("room");
    game.lists.activate_room(r).expect("activate");
    r
}

fn step(game: &mut Game, sys: &mut Sys) {
    game.frame += 1;
    run_timer_events(game, sys);
}

fn at(game: &mut Game, unit: UnitId, ev: u8, expire: i32, a1: u32, a2: u32) {
    game.schedule_event(unit, u32::from(ev), expire, None, a1, a2)
        .expect("schedule");
}

/// The AnimData record of the mode tests: 10 frames, event byte 1 at
/// index 3. At f = 100, speed 256, frame bonus 0, §4.2 gives event 0 at
/// 103, args (1, 0), and event 1 at 110.
fn ten_frames() -> AnimRecord {
    AnimRecord {
        frames: 10,
        byte_0f: 0,
        events: bytes(&[(3, 1)]),
    }
}

const TEN_FRAMES_SCHEDULE: [(u8, i32, u32, u32); 2] = [(0, 103, 1, 0), (1, 110, 0, 0)];

/// A game at frame 100 whose units animate with [`ten_frames`] at speed
/// 256 (they have a path).
fn animated() -> (Game, Sys) {
    let mut game = Game::new();
    game.frame = 100;
    let mut sys = system();
    sys.hooks.anim = Some(ten_frames());
    sys.hooks.rate = 256;
    sys.hooks.has_path = true;
    (game, sys)
}

fn mode_of(sys: &Sys, unit: UnitId) -> u32 {
    sys.units.get(unit).expect("unit").mode
}

fn flags_of(sys: &Sys, unit: UnitId) -> u32 {
    sys.units.get(unit).expect("unit").flags
}

fn types_of(game: &Game, unit: UnitId) -> Vec<u8> {
    pending(game, unit).into_iter().map(|p| p.0).collect()
}

// ---- §1, §2 -----------------------------------------------------------------------------

/// §1: type numbers, timer classes (tile: none, never scheduled), every
/// free routine of types 0–4 cancels all timers, the mode rows.
// Covers: specs/sim/units.md §1
#[test]
fn unit_kinds() {
    let kinds = [
        (UnitType::Player, 0, Some(0)),
        (UnitType::Monster, 1, Some(1)),
        (UnitType::Object, 2, Some(3)),
        (UnitType::Missile, 3, Some(2)),
        (UnitType::Item, 4, Some(4)),
        (UnitType::Tile, 5, None),
    ];
    for (ty, n, class) in kinds {
        assert_eq!(ty.index(), n);
        assert_eq!(TimerClass::of(ty).map(|c| c as u8), class, "{ty:?}");
    }
    let mut game = Game::new();
    let mut sys = system();
    for ty in &UnitType::ALL[..5] {
        let u = spawn(&mut game, &mut sys, *ty, 0, 1);
        at(&mut game, u, event::STAT_REGEN, 5, 0, 0);
        at(&mut game, u, event::REMOVE_STATE, 9, 0, 0);
        at(&mut game, u, event::END_ANIM, -1, 0, 0);
        assert!(pending(&game, u).len() >= 3);
        sys.with(&mut game, |sim, hooks| remove(sim, hooks, u))
            .expect("remove");
        assert!(pending(&game, u).is_empty(), "{ty:?}");
    }
    // Tile: flags |= 0x2; no timer class, so never scheduled.
    let t = spawn(&mut game, &mut sys, UnitType::Tile, 0, 0);
    assert_ne!(flags_of(&sys, t) & 0x2, 0);
    assert!(game.schedule_event(t, 0, 5, None, 0, 0).is_err());
    assert!(game.schedule_event(t, 0, -1, None, 0, 0).is_err());
    assert!(pending(&game, t).is_empty());
    // Mode rows (plrmode.txt, monmode.txt).
    use player_mode as p;
    assert_eq!(
        [
            p::DT,
            p::NU,
            p::WL,
            p::RN,
            p::GH,
            p::TN,
            p::TW,
            p::A1,
            p::A2,
            p::BL
        ],
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    assert_eq!(
        [p::TH, p::DD, p::SEQUENCE, p::KB, p::COUNT],
        [11, 17, 18, 19, 20]
    );
    assert_eq!(player_start_kind(p::COUNT), None);
    use monster_mode as m;
    assert_eq!(
        [m::DT, m::NU, m::GH, m::DD, m::SEQUENCE, m::COUNT],
        [0, 1, 3, 12, 14, 16]
    );
    let mon = spawn(&mut game, &mut sys, UnitType::Monster, 0, 1);
    let r = sys.with(&mut game, |sim, hooks| {
        modes::monster_set_mode(sim, hooks, mon, m::COUNT)
    });
    assert!(matches!(r, Err(UnitError::BadMode { .. })));
}

/// §2: the flag bits, the allocation values of the record and "dead".
// Covers: specs/sim/units.md §2
#[test]
fn unit_record_fields() {
    assert_eq!(
        [
            flags::CHANGED,
            flags::TILE,
            flags::SEED_SET,
            flags::ATTACK_PENDING,
            flags::HOVER_FREED,
            flags::DEAD,
            flags::MODE_CHANGING,
        ],
        [0x1, 0x2, 0x10, 0x40, 0x100, 0x10000, 0x80000]
    );
    assert_eq!([flags2::EXPANSION, flags2::SERVER], [0x2000000, 0x4000000]);
    assert_eq!(INITIAL_NODE_INDEX, 11);
    let mut game = Game::new();
    let mut sys = system();
    let p = spawn(&mut game, &mut sys, UnitType::Player, 3, 0);
    let m = spawn(&mut game, &mut sys, UnitType::Monster, 1, 1);
    for (u, ty, class) in [(p, UnitType::Player, 3), (m, UnitType::Monster, 1)] {
        let rec = sys.units.get(u).expect("unit");
        assert_eq!((rec.ty, rec.class), (ty, class));
        assert_eq!(rec.guid, game.lists.unit(u).expect("listed").guid);
        assert_eq!(rec.stats, sys.stats.unit_list(u));
        assert!(rec.stats.is_some());
        assert_eq!(rec.node_index, 11);
    }
    // Dead (0x005541B0): flag 0x10000; player modes 0, 17; monster 0, 12.
    for mode in 0..player_mode::COUNT {
        let r = sys.units.get_mut(p).expect("player");
        r.mode = mode;
        r.flags &= !flags::DEAD;
        assert_eq!(r.is_dead(), matches!(mode, 0 | 17), "player {mode}");
        r.flags |= flags::DEAD;
        assert!(r.is_dead());
    }
    for mode in 0..monster_mode::COUNT {
        let r = sys.units.get_mut(m).expect("monster");
        r.mode = mode;
        r.flags &= !flags::DEAD;
        assert_eq!(r.is_dead(), matches!(mode, 0 | 12), "monster {mode}");
        r.flags |= flags::DEAD;
        assert!(r.is_dead());
    }
}

// ---- §3 ------------------------------------------------------------------------------------

/// §3.1 steps 2, 3, 5, 7 and the section text: the record of every kind,
/// the act of the room, the per-kind init (mode := argument for types
/// 1–4, not for tiles), and only the missile's init schedules (§6.3).
// Covers: specs/sim/units.md §3.1 text, §3.1 r2, §3.1 r3, §3.1 r5, §3.1 r7
#[test]
fn allocation_steps() {
    for expansion in [false, true] {
        let mut game = Game::new();
        let mut sys = system();
        sys.data.expansion = expansion;
        let r2 = room(&mut game, 2);
        for (ty, class) in [
            (UnitType::Player, 4),
            (UnitType::Monster, 1),
            (UnitType::Object, 9),
            (UnitType::Missile, 12),
            (UnitType::Item, 33),
            (UnitType::Tile, 5),
        ] {
            sys.hooks.log.clear();
            let u = alloc(&mut game, &mut sys, &request(ty, class, 3, Some(r2)));
            let rec = sys.units.get(u).expect("unit").clone();
            // Step 2.
            assert_eq!((rec.ty, rec.class), (ty, class));
            let want2 = flags2::SERVER | if expansion { flags2::EXPANSION } else { 0 };
            assert_eq!(rec.flags2, want2, "{ty:?}");
            // Step 3: the act of the room.
            assert_eq!(rec.act, 2, "{ty:?}");
            // Step 5.
            assert_ne!(rec.flags & flags::SEED_SET, 0);
            assert_eq!(rec.node_index, 11);
            // Step 7: per-kind init.
            let want_mode = match ty {
                UnitType::Monster | UnitType::Object | UnitType::Missile | UnitType::Item => 3,
                _ => 0,
            };
            if ty != UnitType::Player {
                assert_eq!(rec.mode, want_mode, "{ty:?}");
            }
            assert_eq!(rec.flags & flags::TILE != 0, ty == UnitType::Tile);
            assert_eq!(sys.hooks.log.len(), 1, "{ty:?}");
            assert!(sys.hooks.log[0].starts_with(&format!("init {ty:?}")));
            // Events scheduled by the init: the missile's only.
            let want: Vec<(u8, i32, u32, u32)> = match ty {
                UnitType::Missile => vec![(0, -1, 0, 0)],
                _ => vec![],
            };
            assert_eq!(pending(&game, u), want, "{ty:?}");
        }
        // No room: act 0.
        let u = spawn(&mut game, &mut sys, UnitType::Monster, 0, 1);
        assert_eq!(sys.units.get(u).expect("unit").act, 0);
    }
}

/// §3.2: unlink and timer cancel before the free routine, record freed
/// last; immediate: a pending event of the unit never runs.
// Covers: specs/sim/units.md §3.2
#[test]
fn removal_order() {
    let mut game = Game::new();
    let mut sys = system();
    let r = room(&mut game, 0);
    for ty in &UnitType::ALL[..5] {
        let u = alloc(&mut game, &mut sys, &request(*ty, 0, 1, Some(r)));
        let guid = sys.units.get(u).expect("unit").guid;
        at(&mut game, u, event::REMOVE_STATE, 1, 0, 0);
        sys.hooks.log.clear();
        sys.with(&mut game, |sim, hooks| remove(sim, hooks, u))
            .expect("remove");
        assert_eq!(
            sys.hooks.log,
            ["free listed=false timers=0 record=true"],
            "{ty:?}"
        );
        assert!(sys.units.get(u).is_none());
        assert!(sys.stats.unit_list(u).is_none());
        assert!(game.lists.find_unit(*ty, guid).is_none());
        assert!(!game.lists.room_units(r).contains(&u));
        sys.hooks.log.clear();
        step(&mut game, &mut sys);
        assert!(sys.hooks.log.is_empty(), "{ty:?}");
        assert!(sys.errors.is_empty());
    }
}

// ---- §4 ------------------------------------------------------------------------------------

/// §4.1: setting a mode (combat entries first; tile nothing; new mode:
/// write, queue, flag 1, re-init; same mode: queue and flag 1, not for a
/// monster staying in mode 1; nothing scheduled), preparing the
/// animation, and the animated and movement start tails.
// Covers: specs/sim/units.md §4.1
#[test]
fn setting_a_mode() {
    let (mut game, mut sys) = animated();
    let r = room(&mut game, 0);
    let p = alloc(
        &mut game,
        &mut sys,
        &request(UnitType::Player, 0, 0, Some(r)),
    );
    let queued = |game: &Game, u: UnitId| game.lists.unit(u).expect("unit").is_queued();
    game.lists.unqueue_update(p).expect("unqueue");
    sys.units.get_mut(p).expect("p").flags &= !flags::CHANGED;
    sys.hooks.log.clear();
    // New mode.
    sys.with(&mut game, |sim, h| modes::set_mode(sim, h, p, 2))
        .expect("set");
    assert_eq!(sys.hooks.log, ["combat Some(0)", "reinit"]);
    assert_eq!(mode_of(&sys, p), 2);
    assert_ne!(flags_of(&sys, p) & flags::CHANGED, 0);
    assert!(queued(&game, p));
    assert!(pending(&game, p).is_empty());
    // Same mode: queue and flag only.
    game.lists.unqueue_update(p).expect("unqueue");
    sys.units.get_mut(p).expect("p").flags &= !flags::CHANGED;
    sys.hooks.log.clear();
    sys.with(&mut game, |sim, h| modes::set_mode(sim, h, p, 2))
        .expect("set");
    assert_eq!(sys.hooks.log, ["combat Some(2)"]);
    assert_ne!(flags_of(&sys, p) & flags::CHANGED, 0);
    assert!(queued(&game, p));
    // A monster staying in mode 1: neither; in another mode: both.
    let m = alloc(
        &mut game,
        &mut sys,
        &request(UnitType::Monster, 0, 1, Some(r)),
    );
    for (mode, effect) in [(1, false), (2, true), (2, true)] {
        game.lists.unqueue_update(m).expect("unqueue");
        sys.units.get_mut(m).expect("m").flags &= !flags::CHANGED;
        sys.with(&mut game, |sim, h| modes::set_mode(sim, h, m, mode))
            .expect("set");
        assert_eq!(queued(&game, m), effect, "mode {mode}");
        assert_eq!(flags_of(&sys, m) & flags::CHANGED != 0, effect);
    }
    // Tile: combat entries dropped, then nothing.
    let t = alloc(&mut game, &mut sys, &request(UnitType::Tile, 0, 0, Some(r)));
    game.lists.unqueue_update(t).expect("unqueue");
    let before = flags_of(&sys, t);
    sys.hooks.log.clear();
    sys.with(&mut game, |sim, h| modes::set_mode(sim, h, t, 3))
        .expect("set");
    assert_eq!(sys.hooks.log, ["combat Some(0)"]);
    assert_eq!((mode_of(&sys, t), flags_of(&sys, t)), (0, before));
    assert!(!queued(&game, t));

    // Prepare animation (0x005533D0), plain branch with a path.
    let prep = |game: &mut Game, sys: &mut Sys, u: UnitId| {
        sys.with(game, |sim, h| modes::prepare_animation(sim, h, u))
            .expect("prepare");
    };
    {
        let a = &mut sys.units.get_mut(p).expect("p").anim;
        (a.action_frame, a.frame, a.speed) = (5, 999, 7);
    }
    sys.hooks.rate = 128;
    prep(&mut game, &mut sys, p);
    let a = sys.units.get(p).expect("p").anim.clone();
    assert_eq!(
        (a.action_frame, a.frame, a.speed, a.frame_count),
        (0, 0, 128, 10 * 256)
    );
    assert_eq!((a.sequence, a.record), (None, Some(ten_frames())));
    // Without a path: only the AnimData lookup, the rate is not read.
    sys.hooks.has_path = false;
    sys.hooks.rate = 64;
    prep(&mut game, &mut sys, p);
    assert_eq!(sys.units.get(p).expect("p").anim.speed, 128);
    // Sequence modes load the sequence (current frame not reset); other
    // modes drop it.
    let seq = Sequence {
        frame_count: 5 * 256,
        speed: 256,
        events: vec![0, 1, 0, 0, 0],
    };
    sys.hooks.sequence = Some(seq.clone());
    for (u, mode, loaded) in [
        (p, player_mode::SEQUENCE, true),
        (p, player_mode::A1, false),
        (m, monster_mode::SEQUENCE, true),
        (m, 4, false),
    ] {
        let a = &mut sys.units.get_mut(u).expect("u");
        a.mode = mode;
        a.anim.frame = 777;
        a.anim.sequence = None;
        prep(&mut game, &mut sys, u);
        let a = &sys.units.get(u).expect("u").anim;
        assert_eq!(a.sequence.is_some(), loaded, "mode {mode}");
        assert_eq!(a.frame, if loaded { 777 } else { 0 }, "mode {mode}");
    }
    // Monster 14 without a sequence: the plain branch.
    sys.hooks.sequence = None;
    sys.units.get_mut(m).expect("m").mode = monster_mode::SEQUENCE;
    prep(&mut game, &mut sys, m);
    assert_eq!(sys.units.get(m).expect("m").anim.frame, 0);

    // Animated tail: prepare, cancel 0/1 (any argument), §4.2.
    sys.hooks.has_path = true;
    sys.hooks.rate = 256;
    sys.units.get_mut(p).expect("p").mode = player_mode::A1;
    at(&mut game, p, 0, 150, 9, 9);
    at(&mut game, p, 1, 160, 0, 0);
    at(&mut game, p, 2, 105, 0, 0);
    sys.with(&mut game, |sim, h| modes::animate(sim, h, p))
        .expect("animate");
    assert_eq!(
        pending(&game, p),
        [(0, 103, 1, 0), (1, 110, 0, 0), (2, 105, 0, 0)]
    );
    assert_eq!(sys.units.get(p).expect("p").anim.frame, 100 * 256);
    // Movement start: set the mode, cancel 0/1, §4.4.
    sys.with(&mut game, |sim, h| {
        modes::player_start(sim, h, p, player_mode::WL)
    })
    .expect("start");
    assert_eq!(pending(&game, p), [(0, -1, 0, 0), (2, 105, 0, 0)]);
}

/// §4.3: the rate (+0x4C) and the frame bonus are inputs of §4.2: the
/// rate hook's value is the speed, the bonus the start index.
// Covers: specs/sim/units.md §4.3
#[test]
fn rate_and_bonus_are_inputs() {
    let (mut game, mut sys) = animated();
    sys.hooks.anim = Some(AnimRecord {
        frames: 4,
        byte_0f: 0,
        events: bytes(&[(1, 1), (2, 2)]),
    });
    sys.hooks.rate = 128;
    sys.hooks.bonus = 2;
    let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
    sys.with(&mut game, |sim, h| {
        modes::player_start(sim, h, p, player_mode::GH)
    })
    .expect("start");
    // b = 2 skips byte 1; end at f + ⌈(1024 − 512) / 128⌉ = f + 4.
    assert_eq!(pending(&game, p), [(0, 101, 2, 0), (1, 104, 0, 0)]);
    assert_eq!(sys.units.get(p).expect("p").anim.speed, 128);
}

/// §4.4: cancel only the type-0 events, then one every-tick type 0 (0,
/// 0); its callers cancel types 0 and 1 first.
// Covers: specs/sim/units.md §4.4
#[test]
fn every_tick_movement() {
    let mut game = Game::new();
    game.frame = 100;
    let mut sys = system();
    let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
    at(&mut game, p, 0, 150, 1, 2);
    at(&mut game, p, 0, -1, 3, 4);
    at(&mut game, p, 1, 120, 0, 0);
    anim::every_tick_movement(&mut game, p).expect("movement");
    assert_eq!(pending(&game, p), [(0, -1, 0, 0), (1, 120, 0, 0)]);
    // Player movement start: 0 and 1 cancelled.
    sys.with(&mut game, |sim, h| {
        modes::player_start(sim, h, p, player_mode::RN)
    })
    .expect("start");
    assert_eq!(pending(&game, p), [(0, -1, 0, 0)]);
    // Monster mode set of a moving mode (class 0: A1 moves).
    let m = spawn(&mut game, &mut sys, UnitType::Monster, 0, 4);
    at(&mut game, m, 1, 120, 0, 0);
    at(&mut game, m, 0, 130, 5, 5);
    sys.hooks.start_mode = Some(4);
    sys.with(&mut game, |sim, h| modes::monster_set_mode(sim, h, m, 4))
        .expect("set");
    assert_eq!(pending(&game, m), [(0, -1, 0, 0)]);
}

fn set_stamina(game: &mut Game, sys: &mut Sys, p: UnitId, v: i32) {
    sys.with(game, |sim, h| sim.stats.unit_set(h, p, stat::STAMINA, v, 0));
}

/// §4.5: the player start table, event 0's action functions and event
/// 1's end-of-animation rules.
// Covers: specs/sim/units.md §4.5
#[test]
fn player_mode_starts() {
    // Animated starts: GH, BL plain; 7, 8, 10–16, 18 also clear 0x40 and
    // call 0x0056FAF0 after the schedule.
    for mode in [4, 9, 7, 8, 10, 11, 12, 13, 14, 15, 16, 18] {
        let (mut game, mut sys) = animated();
        let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
        sys.units.get_mut(p).expect("p").flags |= flags::ATTACK_PENDING;
        at(&mut game, p, 1, 150, 0, 0);
        sys.hooks.log.clear();
        let ok = sys
            .with(&mut game, |sim, h| modes::player_start(sim, h, p, mode))
            .expect("start");
        assert!(ok);
        assert_eq!(mode_of(&sys, p), mode);
        assert_eq!(pending(&game, p), TEN_FRAMES_SCHEDULE, "mode {mode}");
        let skill = !matches!(mode, 4 | 9);
        assert_eq!(flags_of(&sys, p) & flags::ATTACK_PENDING == 0, skill);
        assert_eq!(
            sys.hooks.log.last().map(String::as_str) == Some("skillstart"),
            skill
        );
    }
    // DT: death bookkeeping, then §4.2.
    let (mut game, mut sys) = animated();
    let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
    sys.units.get_mut(p).expect("p").mode = player_mode::NU;
    sys.hooks.log.clear();
    sys.with(&mut game, |sim, h| modes::player_start(sim, h, p, 0))
        .expect("start");
    assert_eq!(sys.hooks.log[..2], ["death", "combat Some(1)"]);
    assert_eq!(
        (mode_of(&sys, p), pending(&game, p)),
        (0, TEN_FRAMES_SCHEDULE.to_vec())
    );
    // NU, TN: cancel 0/1, mode 1 or 5 in town; nothing scheduled.
    for (mode, town, want) in [(1, false, 1), (1, true, 5), (5, false, 1), (5, true, 5)] {
        let (mut game, mut sys) = animated();
        sys.hooks.town = town;
        let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
        at(&mut game, p, 0, 150, 0, 0);
        at(&mut game, p, 1, 150, 0, 0);
        at(&mut game, p, 2, 150, 0, 0);
        sys.with(&mut game, |sim, h| modes::player_start(sim, h, p, mode))
            .expect("start");
        assert_eq!(mode_of(&sys, p), want);
        assert_eq!(types_of(&game, p), [2]);
    }
    // Movement: RN needs stamina; WL/TW by town; KB sets path values.
    for (mode, stamina, town, want) in [
        (2, 0, false, 2),
        (2, 0, true, 6),
        (6, 0, false, 2),
        (6, 0, true, 6),
        (3, 5, false, 3),
        (3, 5, true, 3),
        (3, 0, false, 2),
        (3, 0, true, 6),
        (19, 0, true, 19),
    ] {
        let (mut game, mut sys) = animated();
        sys.hooks.town = town;
        let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
        set_stamina(&mut game, &mut sys, p, stamina);
        at(&mut game, p, 1, 150, 0, 0);
        sys.hooks.log.clear();
        sys.with(&mut game, |sim, h| modes::player_start(sim, h, p, mode))
            .expect("start");
        assert_eq!(mode_of(&sys, p), want, "{mode} {stamina} {town}");
        assert_eq!(pending(&game, p), [(0, -1, 0, 0)]);
        assert_eq!(sys.hooks.log.contains(&"kbpath".to_string()), mode == 19);
    }
    // DD: corpse, nothing scheduled.
    let (mut game, mut sys) = animated();
    let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
    sys.with(&mut game, |sim, h| modes::player_start(sim, h, p, 17))
        .expect("start");
    assert_eq!(mode_of(&sys, p), 17);
    assert!(sys.hooks.log.contains(&"corpse".to_string()));
    assert!(pending(&game, p).is_empty());
    // A rejected request: nothing.
    sys.hooks.reject = true;
    sys.hooks.log.clear();
    let ok = sys
        .with(&mut game, |sim, h| modes::player_start(sim, h, p, 7))
        .expect("start");
    assert!(!ok);
    assert!(sys.hooks.log.is_empty());
    assert_eq!(mode_of(&sys, p), 17);

    // Event 0: the mode's action function with (a1, a2).
    for mode in 0..player_mode::COUNT {
        let (mut game, mut sys) = animated();
        let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
        sys.units.get_mut(p).expect("p").mode = mode;
        sys.hooks.log.clear();
        sys.with(&mut game, |sim, h| modes::player_event0(sim, h, p, 4, 5))
            .expect("event 0");
        let want: &[&str] = match mode {
            2 | 3 | 6 | 19 => &["step 4 5"],
            7 | 8 | 10..=16 | 18 => &["action 4 5"],
            _ => &[],
        };
        assert_eq!(sys.hooks.log, want, "mode {mode}");
        assert_eq!(mode_of(&sys, p), mode);
    }
    // Result 2 runs the ENDANIM handler at once.
    let (mut game, mut sys) = animated();
    let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
    sys.units.get_mut(p).expect("p").mode = player_mode::A2;
    sys.hooks.action_result = Some(2);
    sys.with(&mut game, |sim, h| modes::player_event0(sim, h, p, 1, 0))
        .expect("event 0");
    assert_eq!(sys.hooks.log[..2], ["action 1 0", "cleanup"]);
    assert_eq!(mode_of(&sys, p), player_mode::NU);

    // Event 1: cleanup for A1, A2, TH or a flagged item row; DT → DD;
    // KB → GH (§4.2); every other mode → neutral.
    for mode in 0..player_mode::COUNT {
        for flagged in [false, true] {
            let (mut game, mut sys) = animated();
            sys.hooks.item_flagged = flagged;
            let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
            sys.units.get_mut(p).expect("p").mode = mode;
            at(&mut game, p, 0, 150, 0, 0);
            sys.hooks.log.clear();
            sys.with(&mut game, |sim, h| modes::player_event1(sim, h, p))
                .expect("event 1");
            let cleanup = flagged || matches!(mode, 7 | 8 | 11);
            assert_eq!(
                sys.hooks.log.first().map(String::as_str) == Some("cleanup"),
                cleanup,
                "mode {mode}"
            );
            let (want_mode, want_pending) = match mode {
                0 => (17, vec![(0, 150, 0, 0)]),
                19 => (4, TEN_FRAMES_SCHEDULE.to_vec()),
                _ => (1, vec![]),
            };
            assert_eq!(mode_of(&sys, p), want_mode, "mode {mode}");
            assert_eq!(pending(&game, p), want_pending, "mode {mode}");
            if mode == 0 {
                assert_eq!(sys.hooks.log.last().map(String::as_str), Some("corpse"));
            }
        }
    }
}

/// The mode table rows of `units.md` §4.6, parsed from the spec:
/// (mode, start, event 0, event 1, schedules, moves).
fn spec_monster_modes() -> Vec<(u32, MonsterModeRecord, Moves)> {
    const SPEC: &str = include_str!("../../../../specs/sim/units.md");
    let hex = |c: &str| {
        let c = c.trim().trim_matches('`');
        if c == "—" {
            0
        } else {
            u32::from_str_radix(c.trim_start_matches("0x"), 16).expect("hex")
        }
    };
    let mut out = Vec::new();
    let mut lines = SPEC
        .lines()
        .skip_while(|l| !l.starts_with("| Mode | Start | Event 0"))
        .skip(2);
    for l in lines.by_ref().take_while(|l| l.starts_with('|')) {
        let c: Vec<&str> = l.split('|').map(str::trim).collect();
        let record = MonsterModeRecord {
            start: hex(c[2]),
            event0: hex(c[3]),
            event1: hex(c[4]),
            schedules: c[5] == "yes",
        };
        let moves = if c[6].starts_with("always") {
            Moves::Always
        } else if c[6] == "no" || c[6] == "—" {
            Moves::No
        } else {
            Moves::Bit
        };
        for m in c[1].split(',') {
            let n = m
                .split_whitespace()
                .next()
                .expect("mode")
                .parse()
                .expect("n");
            out.push((n, record, moves));
        }
    }
    out
}

/// §4.6: the mode table (from the spec text), the mode set's steps, the
/// fallback, events 0/1 and the neutral start's pending-AI test.
// Covers: specs/sim/units.md §4.6
#[test]
fn monster_mode_set() {
    let rows = spec_monster_modes();
    assert_eq!(rows.len(), 16);
    for (mode, rec, moves) in &rows {
        assert_eq!(MONSTER_MODES[*mode as usize], *rec, "mode {mode}");
        assert_eq!(MONSTER_MOVES[*mode as usize], *moves, "mode {mode}");
    }
    // Every mode on class 1 (no move bits), then class 0 (A1 moves).
    for (class, mode) in (0..16).map(|m| (1, m)).chain([(0, 4), (0, 5)]) {
        let (mut game, mut sys) = animated();
        let m = spawn(&mut game, &mut sys, UnitType::Monster, class, 1);
        at(&mut game, m, 0, 150, 0, 0);
        at(&mut game, m, 1, 150, 0, 0);
        if mode != 1 {
            sys.hooks.start_mode = Some(mode);
        }
        sys.hooks.log.clear();
        sys.with(&mut game, |sim, h| modes::monster_set_mode(sim, h, m, mode))
            .expect("set");
        let rec = MONSTER_MODES[mode as usize];
        assert_eq!(
            sys.hooks.log.contains(&format!("book {mode}")),
            mode != 3,
            "mode {mode}"
        );
        if mode != 1 {
            assert_eq!(
                sys.hooks.log[usize::from(mode != 3)],
                format!("mfn {:#x}", rec.start)
            );
        }
        assert_eq!(mode_of(&sys, m), mode);
        assert_ne!(flags_of(&sys, m) & flags::MODE_CHANGING, 0);
        let moving = match MONSTER_MOVES[mode as usize] {
            Moves::Always => true,
            Moves::Bit => class == 0 && mode == 4,
            Moves::No => false,
        };
        let mut want: Vec<(u8, i32, u32, u32)> = match (rec.schedules, moving) {
            (false, _) => vec![],
            (true, true) => vec![(0, -1, 0, 0)],
            (true, false) => TEN_FRAMES_SCHEDULE.to_vec(),
        };
        if mode == 1 {
            want.push((2, 109, 0, 0));
        }
        assert_eq!(pending(&game, m), want, "class {class} mode {mode}");
    }
    // A start returning 0: the neutral start instead.
    let (mut game, mut sys) = animated();
    let m = spawn(&mut game, &mut sys, UnitType::Monster, 1, 1);
    sys.hooks.start_mode = Some(4);
    sys.hooks.start_fails = true;
    sys.with(&mut game, |sim, h| modes::monster_set_mode(sim, h, m, 4))
        .expect("set");
    assert_eq!(mode_of(&sys, m), 1);
    assert_eq!(pending(&game, m), [(2, 109, 0, 0)]);
    // Null functions (per-class record): fallback 0x005A7B30.
    sys.hooks.start_fails = false;
    sys.hooks.class_record = Some(MonsterModeRecord {
        start: 0,
        event0: 0,
        event1: 0,
        schedules: false,
    });
    sys.hooks.log.clear();
    sys.with(&mut game, |sim, h| {
        modes::monster_set_mode(sim, h, m, 4)?;
        modes::monster_event(sim, h, m, false)?;
        modes::monster_event(sim, h, m, true)
    })
    .expect("set");
    let fb = format!("mfn {MONSTER_MODE_FALLBACK:#x}");
    assert_eq!(sys.hooks.log, ["book 4", fb.as_str(), &fb, &fb]);
    // Events 0 and 1 call the current mode's functions; a null one the
    // fallback.
    sys.hooks.class_record = None;
    for (mode, end, want) in [
        (4, false, 0x005A7670),
        (4, true, 0x005A8030),
        (13, false, 0x005A8630),
        (13, true, 0x005A8520),
        (1, true, MONSTER_MODE_FALLBACK),
    ] {
        sys.units.get_mut(m).expect("m").mode = mode;
        sys.hooks.log.clear();
        sys.with(&mut game, |sim, h| modes::monster_event(sim, h, m, end))
            .expect("event");
        assert_eq!(sys.hooks.log, [format!("mfn {want:#x}")]);
    }
    // State 54: fatal assertion.
    let r = sys.with(&mut game, |sim, h| {
        sim.stats.toggle_state(m, state::UNINTERRUPTABLE, true);
        modes::monster_set_mode(sim, h, m, 4)
    });
    assert_eq!(r, Err(UnitError::Uninterruptable));

    // Neutral start: the smallest positive type-2 expire against f.
    let neutral = |game: &mut Game, sys: &mut Sys, m: UnitId| {
        sys.with(game, |sim, h| modes::monster_neutral(sim, h, m))
            .expect("neutral");
    };
    let mut game = Game::new();
    let mut sys = system();
    let a = spawn(&mut game, &mut sys, UnitType::Monster, 1, 1);
    let b = spawn(&mut game, &mut sys, UnitType::Monster, 1, 1);
    let c = spawn(&mut game, &mut sys, UnitType::Monster, 1, 1);
    at(&mut game, a, 2, 105, 0, 0);
    at(&mut game, a, 2, 300, 0, 0);
    at(&mut game, b, 2, 300, 0, 0);
    at(&mut game, c, 2, -1, 0, 0);
    game.frame = 150;
    for u in [a, b, c] {
        neutral(&mut game, &mut sys, u);
    }
    // a: smallest 105 ≤ 150 → scheduled although 300 > f.
    assert_eq!(
        pending(&game, a),
        [(2, 105, 0, 0), (2, 159, 0, 0), (2, 300, 0, 0)]
    );
    assert_eq!(pending(&game, b), [(2, 300, 0, 0)]);
    // An every-tick type 2 (expire −1) is not positive.
    assert_eq!(pending(&game, c), [(2, -1, 0, 0), (2, 159, 0, 0)]);
    assert_eq!(mode_of(&sys, a), 1);
}

// ---- §5, §6 --------------------------------------------------------------------------------

/// §5 rules 1 and 3, edge case 5: players and monsters get (a1, a2) and
/// ignore null entries; missiles ignore the type; objects and items get
/// no arguments and a null entry is fatal.
// Covers: specs/sim/units.md §5 r1, §5 r3, §edge-cases-original-bugs r5
#[test]
fn dispatch_arguments_and_nulls() {
    let mut game = Game::new();
    let mut sys = system();
    let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
    sys.units.get_mut(p).expect("p").mode = player_mode::NU;
    for ev in [2, 4, 7, 10] {
        at(&mut game, p, ev, 1, 7, 7);
    }
    at(&mut game, p, event::PERIODIC_SKILLS, 1, 3, 4);
    at(&mut game, p, event::UPDATE_TRADE, 1, 5, 6);
    let m = spawn(&mut game, &mut sys, UnitType::Monster, 1, 1);
    for ev in [4, 11, 13, 14] {
        at(&mut game, m, ev, 1, 7, 7);
    }
    at(&mut game, m, event::AI_THINK, 1, 7, 8);
    at(&mut game, m, event::MON_UMOD, 1, 1, 2);
    at(&mut game, m, event::AI_RESET, 1, 3, 4);
    step(&mut game, &mut sys);
    let mut log = sys.hooks.log.clone();
    log.sort();
    assert_eq!(
        log,
        [
            "ai 7 8",
            "aireset 3 4",
            "pskills 3 4",
            "trade 5 6",
            "umod 1 2"
        ]
    );
    assert!(sys.errors.is_empty());
    // Missiles: every type runs the server-do function.
    let mi = spawn(&mut game, &mut sys, UnitType::Missile, 0, 0);
    game.timers.cancel_unit_timers(mi);
    for ev in [1, 5, 12, 14] {
        at(&mut game, mi, ev, 2, 0, 0);
    }
    sys.hooks.log.clear();
    step(&mut game, &mut sys);
    assert_eq!(sys.hooks.log, ["missile"; 4]);
    // Objects: types 0–11 to the handler without arguments; 12–14 fatal.
    let o = spawn(&mut game, &mut sys, UnitType::Object, 0, 0);
    for ev in 0..15u8 {
        at(&mut game, o, ev, 3, u32::from(ev) * 7, 1);
    }
    sys.hooks.log.clear();
    step(&mut game, &mut sys);
    let mut log = sys.hooks.log.clone();
    log.sort_by_key(|s| s[7..].parse::<u8>().expect("ev"));
    let want: Vec<String> = (0..12).map(|e| format!("object {e}")).collect();
    assert_eq!(log, want);
    let fatal: Vec<(UnitType, u8)> = sys
        .errors
        .iter()
        .map(|(_, e)| match e {
            UnitError::NullHandler { ty, event } => (*ty, *event),
            e => panic!("{e:?}"),
        })
        .collect();
    let mut fatal_sorted = fatal.clone();
    fatal_sorted.sort();
    assert_eq!(
        fatal_sorted,
        [
            (UnitType::Object, 12),
            (UnitType::Object, 13),
            (UnitType::Object, 14)
        ]
    );
    // Items: 3, 4, 12 have handlers (4 returns at once); others fatal.
    sys.errors.clear();
    let i = spawn(&mut game, &mut sys, UnitType::Item, 0, 0);
    for ev in 0..15u8 {
        at(&mut game, i, ev, 4, 9, 9);
    }
    sys.hooks.log.clear();
    step(&mut game, &mut sys);
    assert_eq!(sys.hooks.log, ["replenish"]);
    let mut fatal: Vec<u8> = sys
        .errors
        .iter()
        .map(|(_, e)| match e {
            UnitError::NullHandler {
                ty: UnitType::Item,
                event,
            } => *event,
            e => panic!("{e:?}"),
        })
        .collect();
    fatal.sort();
    assert_eq!(fatal, [0, 1, 2, 5, 6, 7, 8, 9, 10, 11, 13, 14]);
}

/// §6: expire relative to f; every expire ≤ f becomes f + 1.
// Covers: specs/sim/units.md §6 text
#[test]
fn expire_at_or_before_the_frame() {
    let mut game = Game::new();
    game.frame = 100;
    let mut sys = system();
    let p = spawn(&mut game, &mut sys, UnitType::Player, 0, 0);
    for (e, ev) in [(-50, 2), (50, 4), (100, 7), (101, 10), (102, 13)] {
        at(&mut game, p, ev, e, 0, 0);
    }
    assert_eq!(
        pending(&game, p),
        [
            (2, 101, 0, 0),
            (4, 101, 0, 0),
            (7, 101, 0, 0),
            (10, 101, 0, 0),
            (13, 102, 0, 0)
        ]
    );
}

/// §6.2: monster handlers per type, and the types dropped when frozen.
// Covers: specs/sim/units.md §6.2
#[test]
fn monster_events() {
    let mut game = Game::new();
    game.frame = 100;
    let mut sys = system();
    let m = spawn(&mut game, &mut sys, UnitType::Monster, 1, 1);
    sys.units.get_mut(m).expect("m").mode = 4;
    let run = |game: &mut Game, sys: &mut Sys, ev: u8, a1: u32, a2: u32| {
        sys.hooks.log.clear();
        at(game, m, ev, game.frame + 1, a1, a2);
        step(game, sys);
        assert!(sys.errors.is_empty(), "{:?}", sys.errors);
        sys.hooks.log.clone()
    };
    assert_eq!(run(&mut game, &mut sys, 0, 1, 2), ["mfn 0x5a7670"]);
    assert_eq!(run(&mut game, &mut sys, 1, 1, 2), ["mfn 0x5a8030"]);
    assert_eq!(run(&mut game, &mut sys, 2, 1, 2), ["ai 1 2"]);
    assert_eq!(run(&mut game, &mut sys, 7, 3, 4), ["umod 3 4"]);
    assert_eq!(run(&mut game, &mut sys, 8, 5, 6), ["pskills 5 6"]);
    assert_eq!(run(&mut game, &mut sys, 10, 7, 8), ["aireset 7 8"]);
    for ev in [4, 11, 13, 14] {
        assert!(run(&mut game, &mut sys, ev, 0, 0).is_empty(), "{ev}");
    }
    assert!(pending(&game, m).is_empty());
    // 3: life regeneration, rescheduled at f + 1; a zero rate cancels.
    sys.with(&mut game, |sim, h| {
        sim.stats.unit_set(h, m, stat::MAXHP, 25600, 0);
        sim.stats.unit_set(h, m, stat::HITPOINTS, 1000, 0);
        sim.stats.unit_set(h, m, stat::HPREGEN, 100, 0);
    });
    run(&mut game, &mut sys, 3, 0, 0);
    let f = game.frame;
    assert_eq!(sys.stats.unit_total(m, stat::HITPOINTS, 0), 1100);
    assert_eq!(pending(&game, m), [(3, f + 1, 0, 0)]);
    sys.with(&mut game, |sim, h| {
        sim.stats.unit_set(h, m, stat::HPREGEN, 0, 0)
    });
    step(&mut game, &mut sys);
    assert!(pending(&game, m).is_empty());
    // 6: hover, as the player's.
    let f = game.frame;
    sys.units.get_mut(m).expect("m").hover = Some(f + 3);
    assert!(run(&mut game, &mut sys, 6, 0, 0).is_empty());
    assert_eq!(pending(&game, m), [(6, f + 3, 0, 0)]);
    step(&mut game, &mut sys);
    step(&mut game, &mut sys);
    assert_eq!(sys.hooks.log, ["hover"]);
    let rec = sys.units.get(m).expect("m");
    assert_eq!(rec.hover, None);
    assert_ne!(rec.flags & flags::HOVER_FREED, 0);
    // 12: expired state lists are removed.
    let s = sys.with(&mut game, |sim, h| {
        let s = sim.stats.alloc(0, 0, 0, 1);
        sim.stats.set_state(s, 30);
        sim.stats.set_expire(s, game_frame_plus(sim, 1));
        sim.stats.attach(h, m, s, true);
        s
    });
    run(&mut game, &mut sys, 12, 0, 0);
    assert!(!sys.stats.is_live(s));
    // Frozen: types 0, 1, 2, 6, 7, 9, 10, 11, 13, 14 dropped.
    for ev in 0..15u8 {
        assert_eq!(
            monster_dropped_when_frozen(ev),
            [0, 1, 2, 6, 7, 9, 10, 11, 13, 14].contains(&ev),
            "{ev}"
        );
    }
    sys.with(&mut game, |sim, _| {
        sim.stats.toggle_state(m, state::FREEZE, true);
    });
    assert!(run(&mut game, &mut sys, 7, 0, 0).is_empty());
    assert_eq!(run(&mut game, &mut sys, 8, 1, 1), ["pskills 1 1"]);
}

fn game_frame_plus(sim: &Sim<'_>, d: i32) -> i32 {
    sim.game.frame + d
}

// ---- §7 ------------------------------------------------------------------------------------

/// §7: the scheduler inventory: 269 rows, the column set, the API and
/// type counts, and every cell in its column's grammar.
// Covers: specs/sim/units.md §7
#[test]
fn scheduler_inventory() {
    let mut lines = EVENTS_TSV.lines();
    assert_eq!(
        lines.next(),
        Some("site\tfunction\tapi\ttype\tclasses\texpire\ta1\ta2\tcallback\tproof\towner")
    );
    let rows: Vec<Vec<&str>> = lines
        .filter(|l| !l.is_empty())
        .map(|l| l.split('\t').collect())
        .collect();
    assert_eq!(rows.len(), 269);
    let is_hex = |s: &str| {
        s.strip_prefix("0x")
            .is_some_and(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()))
    };
    let mut api = [0; 3];
    let mut types = [0; 15];
    let named = [
        "f+fc1+1",
        "f+fc1",
        "f+2*fc1",
        "f+parm0+1",
        "f+parm1+1",
        "f+aidel",
        "f+15+rnd35",
        "f+25+rnd250",
        "f+2500/r+1",
        "f+max(2500/r+1,125)",
        "f+1200*m+1",
        "anim",
        "hover",
        "-1",
        "calc",
    ];
    for r in &rows {
        assert_eq!(r.len(), 11, "{r:?}");
        assert!(is_hex(r[0]) && is_hex(r[1]), "{r:?}");
        let (i, cb) = match r[2] {
            "timed" => (0, false),
            "timed_cb" => (1, true),
            "every" => (2, false),
            a => panic!("api {a}"),
        };
        api[i] += 1;
        let ty: usize = r[3].parse().expect("type");
        types[ty] += 1;
        for c in r[4].split(',') {
            assert!(
                ["player", "monster", "object", "missile", "item"].contains(&c),
                "{r:?}"
            );
        }
        for alt in r[5].split('|') {
            let plain = alt
                .strip_prefix("f+")
                .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
            assert!(plain || named.contains(&alt), "expire {alt}");
        }
        for a in [r[6], r[7]] {
            assert!(a == "reg" || a.parse::<u32>().is_ok(), "{r:?}");
        }
        assert_eq!(r[8] != "0", cb, "{r:?}");
        assert!(r[8] == "0" || is_hex(r[8]), "{r:?}");
        assert!(["code", "file"].contains(&r[9]), "{r:?}");
        assert!(!r[10].is_empty());
    }
    assert_eq!(api, [265, 1, 3]);
    assert_eq!(types, [20, 60, 30, 18, 1, 6, 6, 45, 7, 7, 3, 3, 42, 20, 1]);
}
