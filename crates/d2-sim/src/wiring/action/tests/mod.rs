// Spec: specs/sim/tick.md, specs/combat/*, specs/missiles/missiles.md, specs/monsters/ai.md, specs/drlg/rooms.md, specs/world/waypoints.md, specs/world/objects.md (integration of the wired modules)
//! Integration tests: the real modules run together through the action
//! adapters, on synthetic tables and a synthetic DRLG level (two 8×8
//! preset rooms, streamed). Only the seams without a provider
//! ([`Pending`]) are faked: a straight-line path, positions, hostility.

#[cfg(test)]
mod ai;
#[cfg(test)]
mod combat;
#[cfg(test)]
mod death;
#[cfg(test)]
mod e2e;
pub mod fight;
#[cfg(test)]
mod inactive;
#[cfg(test)]
mod missiles;
#[cfg(test)]
mod objects;
#[cfg(test)]
mod player_death;
#[cfg(test)]
mod rooms;
#[cfg(test)]
mod sound;
#[cfg(test)]
mod waypoints;

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_data::tables::{Levels, Missiles as MissileRow};

use super::*;
use crate::drlg::room::LinkAt;
use crate::drlg::tiles::{cell, FIXED_LIBRARY};
use crate::drlg::{
    CellGrid, Drlg, DrlgRoomId, GridPass, LevelDef, LevelIdx, RoomGrids, RoomKind, TileInfo,
    TileRect,
};
use crate::skills::fake::{blank, combat_tables, monster_rec, skill_rec, skill_tables};
use crate::units::hooks::MonsterInfo;
use crate::units::lifecycle::AllocRequest;
use crate::units::{RoomId, UnitId, UnitType};

/// The DRLG init seed of the fixture's act.
pub const INIT: u32 = 644_409_375;
/// The fixture's level (DRLG type 2, level type 1).
pub const LEVEL: u32 = 2;
/// Sub-tile column with a missile barrier (tile x = 4 of room A).
pub const BARRIER_X: i32 = 20;

// ---- seams without a provider -----------------------------------------------

/// A straight-line path: one sub-tile per step along x toward the target
/// point; positions, hostility (everyone but oneself) and a log of the
/// pending calls that matter.
#[derive(Default)]
pub struct TestPending {
    pub pos: BTreeMap<UnitId, (i32, i32)>,
    pub dir: BTreeMap<UnitId, i32>,
    pub crossed: BTreeMap<UnitId, Vec<(i32, i32)>>,
    pub velocity: BTreeMap<UnitId, i32>,
    /// COF names per (unit type, mode) for the AnimData lookup.
    pub names: BTreeMap<(UnitType, u32), [u8; 8]>,
    pub log: Vec<String>,
    /// Messages sent to players' clients, in order.
    pub sent: Vec<(UnitId, Vec<u8>)>,
    /// (operator, object) of every interact-range test, in order.
    pub ranged: std::cell::RefCell<Vec<(UnitId, UnitId)>>,
    /// The corpse `create_corpse` answers and its owner GUID.
    pub corpse: Option<(UnitId, u32)>,
    /// Modes `class_has_mode` denies (every other mode exists).
    pub missing_modes: Vec<u8>,
    /// Used skills (`0x00620250`) by unit.
    pub used: BTreeMap<UnitId, crate::skills::SkillEntry>,
    /// What the monster skill start `0x0056FAF0` answers (logged).
    pub skill_start: i32,
    /// What `object_approach` answers (default: operate).
    pub reach: Option<crate::wiring::action::ObjectReach>,
}

impl Pending for TestPending {
    fn anim_name(&self, _: UnitId, ty: UnitType, _: u32, mode: u32) -> Option<[u8; 8]> {
        self.names.get(&(ty, mode)).copied()
    }
    fn anim_rate(&self, _: UnitId, speed: Option<u32>) -> i16 {
        speed.map_or(0, |s| s as i16)
    }
    fn kill_step(&mut self, _: &mut Game, step: KillStep, d: UnitId, a: UnitId) {
        self.log.push(format!("kill {step:?} {} {}", d.0, a.0));
    }
    fn level_up_event(&mut self, unit: UnitId) {
        self.log.push(format!("level up {}", unit.0));
    }
    fn death_drop_gold(&mut self, _: &mut Game, unit: UnitId, amount: i32) {
        self.log.push(format!("drop gold {} {amount}", unit.0));
    }
    fn create_corpse(&mut self, _: &mut Game, _: UnitId) -> Option<UnitId> {
        self.corpse.map(|c| c.0)
    }
    fn corpse_owner_guid(&self, corpse: UnitId) -> Option<u32> {
        self.corpse.filter(|c| c.0 == corpse).map(|c| c.1)
    }
    /// The death start's body is not written: the fake sets mode DT, as
    /// a start function sets its mode (monster spec).
    fn monster_death_start(
        h: &mut ActionHooks<Self>,
        sim: &mut crate::units::hooks::Sim<'_>,
        unit: UnitId,
        target: Option<UnitId>,
    ) -> bool {
        h.x.log
            .push(format!("death start {} {:?}", unit.0, target.map(|t| t.0)));
        crate::units::modes::set_mode(sim, h, unit, 0).expect("mode DT");
        true
    }
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.pos.get(&unit).copied().unwrap_or((0, 0))
    }
    fn place(&mut self, unit: UnitId, x: i32, y: i32) {
        self.pos.insert(unit, (x, y));
    }
    fn size(&self, _: UnitId) -> i32 {
        1
    }
    fn has_path(&self, _: UnitId) -> bool {
        true
    }
    fn set_velocity(&mut self, unit: UnitId, v: i32) {
        self.velocity.insert(unit, v);
    }
    fn velocity(&self, unit: UnitId) -> i32 {
        self.velocity.get(&unit).copied().unwrap_or(0)
    }
    fn set_target_point(&mut self, unit: UnitId, x: i32, _: i32) {
        let (ux, _) = self.position(unit);
        self.dir.insert(unit, (x - ux).signum());
    }
    fn target_distance(&self, _: UnitId) -> i32 {
        10
    }
    fn step(&mut self, _: &mut Game, unit: UnitId) -> bool {
        let d = self.dir.get(&unit).copied().unwrap_or(1);
        let (x, y) = self.position(unit);
        let p = (x + if d == 0 { 1 } else { d }, y);
        self.pos.insert(unit, p);
        self.crossed.insert(unit, vec![p]);
        true
    }
    fn crossed_subtiles(&self, unit: UnitId) -> Vec<(i32, i32)> {
        self.crossed.get(&unit).cloned().unwrap_or_default()
    }
    fn may_attack(&self, a: UnitId, d: UnitId) -> bool {
        a != d
    }
    fn class_has_mode(&self, _: i32, mode: u8) -> bool {
        !self.missing_modes.contains(&mode)
    }
    fn used_skill(&self, unit: UnitId) -> Option<crate::skills::SkillEntry> {
        self.used.get(&unit).copied()
    }
    fn monster_skill_start(
        h: &mut ActionHooks<Self>,
        _: &mut crate::units::hooks::Sim<'_>,
        unit: UnitId,
    ) -> i32 {
        h.x.log.push(format!("skill start {}", unit.0));
        h.x.skill_start
    }
    fn monster_sequence_frame(
        h: &mut ActionHooks<Self>,
        _: &mut crate::units::hooks::Sim<'_>,
        unit: UnitId,
    ) {
        h.x.log.push(format!("sequence frame {}", unit.0));
    }
    fn unit_event(
        &mut self,
        event: u8,
        unit: Option<UnitId>,
        _: Option<UnitId>,
        _: Option<&mut crate::combat::DamageRecord>,
    ) {
        self.log
            .push(format!("event {event} {:?}", unit.map(|u| u.0)));
    }
    fn reaction(&mut self, a: UnitId, d: UnitId, r: &mut crate::combat::DamageRecord) {
        self.log
            .push(format!("reaction {} {} {:#x}", a.0, d.0, r.result));
    }
    fn set_object_mode(&mut self, _: &mut Game, object: UnitId, mode: u8) {
        self.log.push(format!("object mode {} {mode}", object.0));
    }
    fn object_approach(
        &mut self,
        _: &mut Game,
        _: UnitId,
        _: UnitId,
    ) -> crate::wiring::action::ObjectReach {
        self.reach
            .unwrap_or(crate::wiring::action::ObjectReach::Operate)
    }
    /// Every operator is in interact range of every object (logged).
    fn object_in_range(&self, _: &Game, operator: UnitId, object: UnitId) -> bool {
        self.ranged.borrow_mut().push((operator, object));
        true
    }
    fn object_route(&mut self, _: &mut Game, route: crate::wiring::action::ObjectRoute) {
        self.log.push(format!("object route {route:?}"));
    }
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.sent.push((player, msg.to_vec()));
    }
    fn death_end_action(&mut self, _: &mut Game, unit: UnitId, action: u8, minion: u16) {
        self.log
            .push(format!("death action {} {action} {minion}", unit.0));
    }
}

// ---- DRLG fixture -------------------------------------------------------------------

/// Rooms per level id; floor grids, with a missile-barrier tile column.
struct Types {
    rooms: BTreeMap<u32, Vec<TileRect>>,
}

impl LevelTypes for Types {
    fn generate(
        &mut self,
        drlg: &mut Drlg,
        _: &DrlgData,
        level: LevelIdx,
    ) -> Result<(), DrlgError> {
        let id = drlg.level(level).id;
        for rect in self.rooms.get(&id).cloned().unwrap_or_default() {
            let r = drlg.alloc_room(level, RoomKind::Preset, rect);
            drlg.room_mut(r).dt1_mask = 1;
            drlg.link_room(r, LinkAt::Tail);
        }
        Ok(())
    }

    /// One floor pass: every cell a floor, edges linked; room-local tile
    /// column 4 of a room at x = 0 is "fill LOS" (record flag 0x80 →
    /// collision 0x4, `rooms.md` §10.4 step 4).
    fn room_grids(
        &mut self,
        drlg: &mut Drlg,
        _: &DrlgData,
        room: DrlgRoomId,
    ) -> Result<RoomGrids, DrlgError> {
        let r = drlg.room(room).rect;
        let (w, h) = (r.w as usize + 1, r.h as usize + 1);
        let mut g = CellGrid::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
                let mut v = cell::FLOOR | if edge { cell::LINKED } else { 0 };
                if r.x == 0 && x == 4 {
                    v |= cell::FILL_LOS;
                }
                g.set(x, y, v);
            }
        }
        Ok(RoomGrids {
            passes: vec![GridPass {
                cells: g,
                orientation: None,
                fill_blanks: false,
            }],
            ..RoomGrids::default()
        })
    }
}

fn tile(o: u32, main: u32, sub: u32, rarity: u32) -> TileInfo {
    TileInfo {
        orientation: o,
        main,
        sub,
        rarity,
        material: 0,
        subtile_flags: [0; 25],
        roof_height: 0,
        height: 0,
        light_direction: 0,
    }
}

/// DT1 files by path: open floors (0, 0, 0), walls/shadows of key (1, 0),
/// the fixed library files.
struct Tiles(BTreeMap<Vec<u8>, Vec<TileInfo>>);

impl TileSource for Tiles {
    fn dt1(&self, path: &[u8]) -> Option<&[TileInfo]> {
        self.0.get(path).map(Vec::as_slice)
    }
}

fn tiles() -> Tiles {
    let mut t = BTreeMap::new();
    let mut floor: Vec<TileInfo> = (0..4).map(|_| tile(0, 0, 0, 1)).collect();
    for o in [1, 2, 3, 4, 13] {
        floor.push(tile(o, 1, 0, 1));
    }
    t.insert(b"floor.dt1".to_vec(), floor);
    let blank_tile = |sub| {
        let mut x = tile(0, 30, sub, 0);
        x.subtile_flags = [0x20; 25];
        x
    };
    t.insert(
        FIXED_LIBRARY[0].to_vec(),
        vec![blank_tile(0), blank_tile(1)],
    );
    t.insert(FIXED_LIBRARY[1].to_vec(), vec![]);
    t.insert(FIXED_LIBRARY[2].to_vec(), vec![tile(10, 0, 0, 0)]);
    Tiles(t)
}

fn drlg_data() -> DrlgData {
    let mut d = DrlgData {
        levels: vec![LevelDef::default(); 150],
        ..DrlgData::default()
    };
    for l in &mut d.levels {
        l.warp = [-1; 8];
    }
    let mut files = vec![Vec::new(); 32];
    files[0] = b"floor.dt1".to_vec();
    d.lvltypes = vec![vec![Vec::new(); 32], files];
    for id in [1, LEVEL] {
        d.levels[id as usize].drlg_type = 2;
        d.levels[id as usize].level_type = 1;
    }
    d
}

// ---- tables -------------------------------------------------------------------------------

/// Missile 0: an arrow-like row (default flight, collide type 3, kill on
/// collision, to-hit).
pub fn arrow() -> MissileRow {
    let mut r: MissileRow = blank();
    r.psrvdofunc = 1;
    r.vel = 1;
    r.maxvel = 1;
    r.range = 50;
    r.collidetype = 3;
    r.collidekill = 1;
    r.lastcollide = true;
    r.tohit = 1;
    r.size = 1;
    r
}

/// Monster class 0: AI 1 (Idle), aidel 15, killable.
pub fn monster_class() -> d2_data::tables::Monstats {
    let mut m = monster_rec();
    m.ai = 1;
    m.aidel = 15;
    m.aidel_n = 15;
    m.aidel_h = 15;
    m.skill1 = 0xFFFF;
    m.skill2 = 0xFFFF;
    m.skill3 = 0xFFFF;
    m
}

fn tables() -> ActionTables {
    ActionTables {
        missiles: vec![arrow()],
        skills: skill_tables(vec![skill_rec()]),
        combat: combat_tables(vec![monster_class()]),
        levels: vec![blank::<Levels>(); 150],
        skill_modes: vec![[0; 8]],
        overlay_count: 0,
    }
}

fn unit_data() -> UnitData {
    UnitData {
        monsters: vec![MonsterInfo {
            enabled: true,
            aidel: [15, 15, 15],
            moves: 1 << 4,
        }],
        ..UnitData::default()
    }
}

// ---- the fixture --------------------------------------------------------------------------

/// A game with act 0's DRLG, level 2 generated, rooms A (tiles 0..8) and B
/// (8..16) streamed (active in the act room list), and the action systems.
pub struct Fx {
    pub game: Game,
    pub sim: ActionSim<TestPending>,
    /// Active rooms of A and B.
    pub a: RoomId,
    pub b: RoomId,
}

impl Fx {
    // Shared with the benches (`bench_fixtures::combat`), where clippy
    // sees it as a public type.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self::with_rooms(&[
            (LEVEL, TileRect::new(0, 0, 8, 8)),
            (LEVEL, TileRect::new(8, 0, 8, 8)),
        ])
    }

    /// The rooms of `rooms` (level id, rect) streamed in order.
    pub fn with_rooms(rooms: &[(u32, TileRect)]) -> Self {
        let data = drlg_data();
        let mut by_level: BTreeMap<u32, Vec<TileRect>> = BTreeMap::new();
        for &(l, r) in rooms {
            by_level.entry(l).or_default().push(r);
        }
        let mut types = Types { rooms: by_level };
        let drlg = Drlg::create(0, INIT, 0, 0, false, &data, &mut types).expect("drlg");
        let mut dungeon = crate::drlg::Dungeon::default();
        dungeon.acts[0] = Some(drlg);
        let world = DrlgWorld {
            dungeon,
            data: Arc::new(data),
            tiles: Box::new(tiles()),
            types: Box::new(types),
        };
        let hooks = ActionHooks::new(
            Arc::new(tables()),
            world,
            Seed::init_low(1234),
            TestPending::default(),
        );
        let mut sim = ActionSim::new(crate::stats::tests::data(), unit_data(), hooks);
        let mut game = Game::new();
        game.lists.ensure_act(0).unwrap();
        let mut active = Vec::new();
        let levels: Vec<u32> = {
            let mut v: Vec<u32> = rooms.iter().map(|r| r.0).collect();
            v.dedup();
            v
        };
        for id in levels {
            let r = sim
                .hooks()
                .drlg
                .with_act(0, &mut game.lists, |d, svc| {
                    let l = d.get_or_alloc_level(svc.data, svc.types, id)?;
                    d.generate_level(svc.data, svc.types, l)?;
                    let mut out = Vec::new();
                    for r in d.level_rooms(l) {
                        out.push(d.stream_room(svc, r)?.expect("active"));
                    }
                    Ok::<_, DrlgError>(out)
                })
                .unwrap()
                .unwrap();
            active.extend(r);
        }
        Self {
            game,
            sim,
            a: active[0],
            b: active.get(1).copied().unwrap_or(active[0]),
        }
    }

    /// Allocates a unit (`units.md` §3.1) in `room` at (x, y).
    pub fn spawn(&mut self, ty: UnitType, class: u32, room: RoomId, x: i32, y: i32) -> UnitId {
        let req = AllocRequest {
            ty,
            class,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: ty == UnitType::Player,
        };
        let u = self
            .sim
            .with(&mut self.game, |g, v| v.allocate(g, &req, x, y))
            .expect("allocated");
        if ty == UnitType::Player {
            // Players start in mode 0 (death, `units.md` §2 "dead"); the
            // player mode starts are not run here: neutral.
            self.sim.sys.units.get_mut(u).unwrap().mode = 1;
        }
        u
    }

    /// Sets base stats of a unit.
    pub fn stats(&mut self, u: UnitId, values: &[(u16, i32)]) {
        self.sim.with(&mut self.game, |_, v| {
            for &(s, x) in values {
                v.set_base(u, s, x);
            }
        });
    }

    pub fn stat(&mut self, u: UnitId, s: u16) -> i32 {
        self.sim.with(&mut self.game, |_, v| v.stat(u, s))
    }

    /// Sets a unit's seed (unit +0x20).
    pub fn seed(&mut self, u: UnitId, seed: Seed) {
        self.sim.sys.units.get_mut(u).unwrap().seed = seed;
    }

    /// Marks a unit's run-time collision bit at its position (what the
    /// movement spec will do; `rooms.md` §10.6).
    pub fn mark(&mut self, u: UnitId, bit: u16) {
        let (x, y) = self.sim.hooks().x.position(u);
        let room = self.game.lists.unit(u).unwrap().room().unwrap();
        let game = &self.game;
        *self
            .sim
            .sys
            .hooks
            .drlg
            .collision_mut(game, room, x, y)
            .expect("in a grid") |= bit;
    }

    /// One frame of the timer queue only (frame += 1, step 4).
    pub fn frame(&mut self) {
        self.game.frame += 1;
        crate::tick::run_timer_events(&mut self.game, &mut self.sim);
    }

    /// One full tick (`tick.md` §3).
    pub fn tick(&mut self) {
        crate::tick::tick(&mut self.game, &mut self.sim);
    }

    /// The unit's pending timers as (type, expire), sorted.
    pub fn timers(&self, u: UnitId) -> Vec<(u8, i32)> {
        let mut v: Vec<_> = self
            .game
            .timers
            .unit_timers(u)
            .into_iter()
            .filter_map(|t| Some((self.game.timers.event(t)?.0, self.game.timers.expire(t)?)))
            .collect();
        v.sort();
        v
    }

    /// No adapter or unit-dispatch error so far.
    pub fn assert_clean(&self) {
        assert_eq!(self.sim.sys.hooks.errors, Vec::<WiringError>::new());
        assert!(self.sim.sys.errors.is_empty(), "{:?}", self.sim.sys.errors);
    }
}

/// A seed whose next step gives `lo′ = x` (`rng.md` §3: lo 0, hi x).
pub fn seed_giving(x: u32) -> Seed {
    Seed::new(0, x)
}
