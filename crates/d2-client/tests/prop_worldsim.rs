// Spec: specs/sim/intents-events.md (§2.2–§2.4, §4 rule 1); specs/sim/tick.md (§3, §4); CLAUDE.md hard rules 6, 7
//! Property tests of the fully wired game over time: `SimGame` on
//! `WorldSim` (the world-generation dispatch around `ActionSim`), built
//! as `d2-client`'s single-player e2e builds it (act 0's DRLG through the
//! level-type dispatcher from the recorded Act I placement seed, a
//! generated preset level with a DS1 monster, the population room pass,
//! the regions on the game seed, a sorceress with the wired skill
//! handlers, a waypoint object), driven by random sequences of valid and
//! invalid C→S game messages interleaved with hundreds of ticks.
//!
//! Properties:
//! 1. no panic, debug overflow or fatal path (`sys.errors`, the world
//!    adapters' errors, the world handlers' faults), and no unbounded
//!    growth: unit counts, the timer queue, the act's room list and the
//!    active rooms, the transport outbox stay under fixed bounds;
//! 2. determinism (hard rule 6): the same game seed and message sequence
//!    run twice give byte-identical S→C output and an identical state
//!    digest after every message and every tick;
//! 3. another game seed changes the digest (it is not trivially
//!    constant);
//! 4. the dispatcher's own rejections (§2.3 rule 3, §2.4 rules 1, 3, 4:
//!    closed gate, wrong size, unit type ≥ 6, point out of range) return
//!    before any handler runs and leave the digest unchanged.
//!
//! The fixture (tables, DS1 / DT1 sources, the seams without a provider)
//! is `e2e_single_player.rs`'s, copied without the bridge and the
//! cube's item world (a second unit world, `docs/handoff/e2e-next.md`
//! finding 1); the NPC / vendor rests and tables are `e2e_support`'s.
//! The world host is `WiredWorld<_, WiredSkills>` (waypoints, the skill
//! handlers in its skill slot, Akara beside the player, the player's
//! buckler and cap). The seam
//! answers are the e2e's (see each); none is behaviour. The messages go
//! straight to the server's dispatcher (`d2_server::dispatch`), as the
//! host's drain hands them over, and the S→C output is what the
//! handlers and the tick queue in the client's buffers.
//!
//! Default case counts are small so `cargo test` stays fast; set
//! `PROPTEST_CASES` to hunt harder.

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard};

use d2_data::bin::BinTable;
use d2_data::fixup::maps::EquivMatrix;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{
    Charstats, Difficultylevels, Experience, Itemratio, Itemstatcost, Itemtypes, Levels,
    Missiles as MissileRow, Monlvl, Monstats, Monstats2, Objects, Record, Skilldesc, Skills,
};
use d2_formats::animdata::{self, AnimData, AnimRecord};
use d2_proto::client::{
    AddStatPoint, BuyItem, EntityAction, InitEntityChat, InteractWithEntity, RightSkill,
    RightSkillOnUnit, SellItem, TakeOrCloseWp,
};
use d2_proto::schema::FieldType;
use d2_proto::{FixedMessage, CLIENT_MESSAGES};
use d2_server::adapters::handlers::skills::wired::WiredSkills;
use d2_server::adapters::handlers::skills::LearnRest;
use d2_server::adapters::handlers::world::{ActionWorld, Outbox, WiredWorld};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use d2_server::buffers::ClientBuffers;
use d2_server::dispatch::{dispatch, gate, is_point, is_unit, kind, Gate, Kind};
use d2_server::seams::{Intents, PlayerGate, Pos, ResultCode, Tick};
use d2_sim::combat::vitals::VitalsTables;
use d2_sim::combat::CombatTables;
use d2_sim::drlg::collision::bits;
use d2_sim::drlg::maze::{Maze, MazeData, MazeRow, Specials};
use d2_sim::drlg::outdoor::{OutdoorData, PresetDef as OutdoorPreset, SubDefs, SubFileMap};
use d2_sim::drlg::preset::{
    Ds1Input, Ds1ObjectInput, Ds1Source, PresetData, PresetDef, PresetTables,
};
use d2_sim::drlg::tiles::{cell, FIXED_LIBRARY};
use d2_sim::drlg::{Drlg, DrlgData, Dungeon, LevelDef, TileInfo, TileSource};
use d2_sim::game::Game;
use d2_sim::items::tables::ItemRec;
use d2_sim::items::{ty, ItemRequest, ItemTables};
use d2_sim::missiles::{param_flags, unit_flag, MissileParams};
use d2_sim::monsters::init::{GameInfo, MonstatsExtra};
use d2_sim::monsters::population::PopTables;
use d2_sim::rng::Seed;
use d2_sim::skills::use_::{MissileAim, ModeTarget, ServerMsg, UseState};
use d2_sim::skills::{SkillEntry, SkillTables, LEVEL_CAP_114D};
use d2_sim::stats::{StatData, StatTable};
use d2_sim::tick::timer::TimerClass;
use d2_sim::treasure::{ItemData, TcEntry, TreasureClass, TreasureClasses};
use d2_sim::units::hooks::{MonsterInfo, Sim as USim, UnitData};
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::modes;
use d2_sim::units::{RoomId, UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, ActionTables, DrlgWorld, KillStep, Pending, SkillEvent};
use d2_sim::wiring::economy::{
    monster_death_drop, DeathDrops, DropSpot, DropTables, FreeSpot, GameFields, ItemSpawn,
};
use d2_sim::wiring::interaction::{skill_events, UseRest};
use d2_sim::wiring::worldgen::{
    SharedTypes, WorldPending, WorldSim, WorldState, WorldTables, WorldTypes,
};
use d2_sim::world::npc::{class, NpcControl};
use d2_sim::world::quests::{PlayerQuests, QuestControl, QuestTables};
use d2_sim::world::waypoints::{WaypointData, NO_WAYPOINT};
use proptest::prelude::*;
use proptest::test_runner::Config;

mod e2e_support;
use e2e_support::{blank, item_tables, monstats as npc_monstats, vendor_tables, Rest};
use e2e_support::{BUC, CAP, N_MONSTATS};

/// Proptest config with `default` cases, or `PROPTEST_CASES` when set.
fn config(default: u32) -> Config {
    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default);
    Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    }
}

// ---- constants -----------------------------------------------------------------------

/// The DRLG init seed of `outdoor.md`'s recorded Act I placement
/// (`dwStartSeed` 4014346869): act 0's levels are allocated by the real
/// level types from it.
const DRLG_SEED: u32 = 644_409_375;
/// The game seed (`rng.md` §5.3).
const GAME_SEED: u32 = 1234;
/// A preset level outside the Act I chain (40 × 18 tiles at (8000,
/// 8000)): the player's level, with a waypoint and the DS1 monster.
const ISLE: u32 = 30;
const ISLE_DEF: u32 = 1104;
/// The preset level the waypoint travels to: also outside the Act I
/// chain (40 × 18 at (9000, 9000)). A chain level (e.g. the Monastery
/// Gate) would generate its outdoor neighbours, which need `lvlsub` rows
/// this fixture does not have.
const GATE: u32 = 31;
const GATE_DEF: u32 = 1105;
/// Akara's position, beside the player.
const NPC_AT: (i32, i32) = (40_022, 40_022);
/// Stat ids (`itemstatcost`).
const GOLD: u16 = 14;
/// Waypoint indices (`levels.txt` `Waypoint`).
const ISLE_WP: u8 = 2;
const GATE_WP: u8 = 1;
/// The ISLE DS1's monster: DS1 sub-tile (12, 10), class 0.
const ISLE_MONSTER: (u32, u32) = (12, 10);
/// The player's position: room (8000, 8000)'s sub-tiles start at
/// (40000, 40000) (`rooms.md` §9.2, 5 sub-tiles per tile).
const PLAYER_AT: (i32, i32) = (40_020, 40_020);
/// The waypoint object's position.
const WP_AT: (i32, i32) = (40_024, 40_020);
/// Skills of the synthetic table: attack, and a right skill (srvst 4,
/// mana 4 + 1 per level, shift 8; `use.md`'s Multiple Shot vector).
const ATTACK: i32 = 0;
const MULTI: i32 = 1;

// ---- seams without a provider --------------------------------------------------

/// The action and world-generation seams no written spec provides yet
/// (positions and a straight-line path, interaction, warp, arrival mode,
/// transport, the DRLG population reads, the COF-name composer and the
/// animation rate, the monster death start's body), the skill use
/// pipeline's rest ([`UseRest`]: the player's skill state, shared with
/// the server's skill seams through [`Book`]), the drop state, and a log
/// of the calls that change something. The answers are the narrowest
/// ones (`Pending`'s defaults) except those the test stages (see each).
#[derive(Default)]
struct TestPending {
    pos: BTreeMap<UnitId, (i32, i32)>,
    interact: BTreeMap<UnitId, (u8, u32)>,
    sent: Vec<(UnitId, Vec<u8>)>,
    log: Vec<String>,
    /// Missile target points (`0x00648AD0`) and the sub-tiles each step
    /// crossed.
    aim: BTreeMap<UnitId, (i32, i32)>,
    crossed: BTreeMap<UnitId, Vec<(i32, i32)>>,
    velocity: BTreeMap<UnitId, i32>,
    /// The point the cast aims its missile at (the skill missile
    /// helpers' record fill, `use.md` §5.4 step 7, is not specified).
    aim_at: (i32, i32),
    book: Book,
    /// The game's drop state (`treasure.md` §3), lent out during a drop.
    drops: Option<DeathDrops>,
}

/// The fixture's COF names (the composer `0x0064F5B0` for units with a
/// unit is `animdata.md` Open question 2): the sorceress casting (SC)
/// and the monster dying (DT). Other modes get no name.
const PLAYER_SC: &[u8; 8] = b"SOSCHTH\0";
const MONSTER_DT: &[u8; 8] = b"M0DTHTH\0";

impl Pending for TestPending {
    fn anim_name(&self, _: UnitId, ty: UnitType, _: u32, mode: u32) -> Option<[u8; 8]> {
        match (ty, mode) {
            (UnitType::Player, 10) => Some(*PLAYER_SC),
            (UnitType::Monster, 0) => Some(*MONSTER_DT),
            _ => None,
        }
    }
    /// The rate formula `0x00623F50` is not written: the AnimData speed
    /// as is (no rate stats in this game).
    fn anim_rate(&self, _: UnitId, speed: Option<u32>) -> i16 {
        speed.map_or(0, |s| s as i16)
    }
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.pos.get(&unit).copied().unwrap_or_default()
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
    fn set_target_point(&mut self, unit: UnitId, x: i32, y: i32) {
        self.aim.insert(unit, (x, y));
    }
    fn target_distance(&self, _: UnitId) -> i32 {
        10
    }
    /// One sub-tile toward the target point on each axis.
    fn step(&mut self, _: &mut Game, unit: UnitId) -> bool {
        let (x, y) = self.position(unit);
        let (tx, ty) = self.aim.get(&unit).copied().unwrap_or((x + 1, y));
        let p = (x + (tx - x).signum(), y + (ty - y).signum());
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
    fn class_has_mode(&self, _: i32, _: u8) -> bool {
        true
    }
    fn skill_list(&self, _: UnitId) -> Vec<SkillEntry> {
        self.book.get().list.clone()
    }
    fn used_skill(&self, _: UnitId) -> Option<SkillEntry> {
        self.book.get().used
    }
    fn unit_event(
        &mut self,
        event: u8,
        unit: Option<UnitId>,
        _: Option<UnitId>,
        _: Option<&mut d2_sim::combat::DamageRecord>,
    ) {
        self.log
            .push(format!("event {event} {:?}", unit.map(|u| u.0)));
    }
    fn reaction(&mut self, a: UnitId, d: UnitId, r: &mut d2_sim::combat::DamageRecord) {
        self.log
            .push(format!("reaction {} {} {:#x}", a.0, d.0, r.result));
    }
    fn kill_step(&mut self, _: &mut Game, step: KillStep, d: UnitId, a: UnitId) {
        self.log.push(format!("kill {step:?} {} {}", d.0, a.0));
    }
    fn set_interact(&mut self, player: UnitId, unit_type: u8, guid: u32) {
        self.interact.entry(player).or_insert((unit_type, guid));
    }
    fn reset_interact(&mut self, player: UnitId) {
        self.interact.remove(&player);
    }
    fn interact_guid(&self, player: UnitId) -> Option<u32> {
        self.interact.get(&player).map(|i| i.1)
    }
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.sent.push((player, msg.to_vec()));
    }
    fn warp(&mut self, _: &mut Game, player: UnitId, level: u32, tile_code: u8) {
        self.log
            .push(format!("warp {} {level} {tile_code}", player.0));
    }
    fn set_player_mode_arrival(&mut self, _: &mut Game, player: UnitId) {
        self.log.push(format!("arrival mode {}", player.0));
    }
    fn level_up_event(&mut self, unit: UnitId) {
        self.log.push(format!("level up {}", unit.0));
    }
    fn skill_event(h: &mut ActionHooks<Self>, sim: &mut USim<'_>, ev: SkillEvent) {
        skill_events::route(h, sim, ev);
    }
    fn action_frame(
        h: &mut ActionHooks<Self>,
        sim: &mut USim<'_>,
        u: UnitId,
        a1: u32,
        a2: u32,
    ) -> u32 {
        h.x.log.push(format!("action frame {} {a1} {a2}", u.0));
        skill_events::action_frame(h, sim, u, a1, a2)
    }
    /// The death start's body is not written: the fixture sets mode DT
    /// (a start function sets its mode, monster spec) and runs the drop
    /// gate and drop it is known to call (`treasure.md` §3.1).
    fn monster_death_start(
        h: &mut ActionHooks<Self>,
        sim: &mut USim<'_>,
        unit: UnitId,
        target: Option<UnitId>,
    ) -> bool {
        h.x.log.push(format!(
            "death start {} target {:?}",
            unit.0,
            target.map(|t| t.0)
        ));
        modes::set_mode(sim, h, unit, 0).expect("mode DT");
        if let Some(mut d) = h.x.drops.take() {
            monster_death_drop(h, sim, &mut d, &mut Spot, unit, target);
            h.x.drops = Some(d);
        }
        true
    }
}

/// The free-spot search `0x0064E810` (collision spec, not written): the
/// start spot as is.
struct Spot;

impl FreeSpot for Spot {
    fn free_spot(
        &mut self,
        room: Option<RoomId>,
        start: (i32, i32),
        _: (i32, i32),
    ) -> Option<DropSpot> {
        Some(DropSpot {
            room,
            x: start.0,
            y: start.1,
        })
    }
}

impl WorldPending for TestPending {
    fn preset_created(&mut self, unit: UnitId, p: &d2_sim::monsters::population::PresetUnit) {
        self.log.push(format!(
            "preset {} class {} at {},{}",
            unit.0, p.class, p.x, p.y
        ));
    }
}

impl Outbox for TestPending {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

/// The skill use pipeline's rest on the timer path (the action frame):
/// the player's skill state from [`Book`] (the same state the server's
/// skill seams use for the message path), the rest narrowest.
impl UseRest for TestPending {
    fn send(&mut self, u: UnitId, msg: ServerMsg) {
        self.log.push(format!("send {} {msg:?}", u.0));
    }
    fn has_player_data(&self, _: UnitId) -> bool {
        true
    }
    fn last_point_frame(&self, _: UnitId) -> i32 {
        0
    }
    fn set_last_point_frame(&mut self, _: UnitId, _: i32) {}
    fn cursor_item(&self, _: UnitId) -> bool {
        false
    }
    fn in_own_inventory(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn within_reach(&self, _: UnitId, _: UnitId) -> bool {
        true
    }
    fn owner(&self, _: UnitId) -> Option<UnitId> {
        None
    }
    fn is_pet(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn is_ally(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn left_skill(&self, _: UnitId) -> Option<SkillEntry> {
        None
    }
    fn right_skill(&self, _: UnitId) -> Option<SkillEntry> {
        self.book.get().right
    }
    fn set_left_skill(&mut self, _: UnitId, _: SkillEntry) {}
    fn set_right_skill(&mut self, _: UnitId, e: SkillEntry) {
        self.book.get().right = Some(e);
    }
    fn find_entry(&self, u: UnitId, skill: i32) -> Option<SkillEntry> {
        self.book.find_entry(u, skill)
    }
    fn find_entry_owned(&self, _: UnitId, _: i32, _: i32) -> Option<SkillEntry> {
        None
    }
    fn owns_skill(&self, u: UnitId, skill: i32) -> bool {
        self.book.find_entry(u, skill).is_some()
    }
    fn set_used_skill(&mut self, _: UnitId, e: Option<SkillEntry>) {
        self.book.get().used = e;
    }
    fn used_skill_flags(&self, _: UnitId) -> u32 {
        0
    }
    fn set_used_skill_flags(&mut self, _: UnitId, _: u32) {}
    fn entry_mode(&self, u: UnitId, e: &SkillEntry) -> u32 {
        self.book.entry_mode(u, e)
    }
    fn attack_param4(&self, _: UnitId) -> i32 {
        0
    }
    fn set_attack_param4(&mut self, _: UnitId, _: i32) {}
    fn use_state(&mut self, _: UnitId, _: &SkillEntry) -> UseState {
        UseState::Usable
    }
    fn dec_quantity(&mut self, _: UnitId, _: i32) {}
    fn shapeshifted(&self, _: UnitId) -> bool {
        false
    }
    fn consume_charges(&mut self, _: UnitId, _: &SkillEntry) -> bool {
        true
    }
    fn pay_life(&mut self, _: UnitId, _: i32) -> bool {
        true
    }
    fn can_dual_wield(&self, _: UnitId) -> bool {
        false
    }
    fn equippable(&self, _: UnitId) -> bool {
        false
    }
    fn bow_equipped(&self, _: UnitId) -> bool {
        false
    }
    fn state_mask(&self, _: UnitId, _: u32) -> bool {
        false
    }
    fn start_mode(&mut self, _: &mut Game, _: UnitId, _: u32, _: ModeTarget<UnitId>) {}
    fn run_to(&mut self, _: UnitId, _: UnitId, _: SkillEntry) {}
    fn target(&self, _: UnitId) -> Option<UnitId> {
        None
    }
    fn clear_target(&mut self, _: UnitId) {}
    fn event_arg(&self, _: UnitId) -> i32 {
        0
    }
    fn set_event_arg(&mut self, _: UnitId, _: i32) {}
    fn step_path(&mut self, _: UnitId) -> i32 {
        0
    }
    fn target_position(&self, _: UnitId) -> Option<(i32, i32)> {
        Some(self.aim_at)
    }
    fn line_clear(&self, _: UnitId, _: (i32, i32), _: u32) -> bool {
        true
    }
    fn set_aura_state(&mut self, _: UnitId, _: u16, _: i32, _: i32) {}
    /// The helpers' record fill is not specified: aimed at the cast's
    /// target point, absolute.
    fn skill_missile_fill(&self, _: UnitId, _: bool, _: MissileAim, p: &mut MissileParams) {
        p.flags |= param_flags::TARGET_ABSOLUTE;
        (p.target_x, p.target_y) = self.aim_at;
    }
    fn srvst(&mut self, index: u16, u: UnitId, skill: i32, lvl: i32) -> i32 {
        self.book.srvst(index, u, skill, lvl)
    }
    fn srvdo(&mut self, i: u16, u: UnitId, s: i32, l: i32, c: bool, it: bool, a: bool) -> i32 {
        self.book.srvdo(i, u, s, l, c, it, a)
    }
}

/// The skill-point calls (`levels.md` §6.4): no class skill (the
/// pre-merge `SkillSeams` default), so 0x3B stops at its check.
impl LearnRest for TestPending {
    fn is_class_skill(&self, _: UnitId, _: i32) -> bool {
        false
    }
    fn add_skill_level(&mut self, _: UnitId, _: i32, _: i32) {}
    fn after_skill_point(&mut self, _: UnitId) {}
}

/// The skill pipeline's state without a provider (skill list, skill
/// bodies: `use.md` OQ10): the player's skill list and a call log,
/// shared with the test (the message path and the timer path read the
/// same state through the action wiring's `UseView`).
#[derive(Default)]
struct Inner {
    list: Vec<SkillEntry>,
    right: Option<SkillEntry>,
    used: Option<SkillEntry>,
    log: Vec<String>,
}

#[derive(Clone, Default)]
struct Book(Arc<Mutex<Inner>>);

impl Book {
    fn get(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap()
    }
}

impl Book {
    fn find_entry(&self, _: UnitId, skill: i32) -> Option<SkillEntry> {
        self.get().list.iter().copied().find(|e| e.skill == skill)
    }
    fn entry_mode(&self, _: UnitId, e: &SkillEntry) -> u32 {
        // Skill entry +8 (`use.md` §4): attack 7 (A1), the right skill
        // 10 (SC).
        if e.skill == MULTI {
            10
        } else {
            7
        }
    }
    fn srvst(&self, index: u16, _: UnitId, skill: i32, lvl: i32) -> i32 {
        self.get().log.push(format!("srvst {index} {skill} {lvl}"));
        1
    }
    /// The do bodies are catalogued only (`use.md` OQ10): logged, result
    /// 0 (the generic `srvmissile` creation of §5.4 step 7 still runs).
    #[allow(clippy::too_many_arguments)]
    fn srvdo(&self, i: u16, _: UnitId, s: i32, l: i32, c: bool, it: bool, a: bool) -> i32 {
        self.get()
            .log
            .push(format!("srvdo {i} {s} {l} {c} {it} {a}"));
        0
    }
}

// ---- DS1 / DT1 sources ---------------------------------------------------------------

#[derive(Default)]
struct Ds1s(BTreeMap<Vec<u8>, Ds1Input>);

impl Ds1Source for Ds1s {
    fn ds1(&self, path: &[u8]) -> Option<&Ds1Input> {
        self.0.get(path)
    }
}

/// A v18 DS1 of `w × h` tiles: one layer of plain floors, an empty
/// shadow layer, monsters (id = class) at DS1 sub-tile positions.
fn ds1(w: u32, h: u32, monsters: &[(u32, u32, u32)]) -> Ds1Input {
    let cells = ((w + 1) * (h + 1)) as usize;
    Ds1Input {
        version: 18,
        width: w,
        height: h,
        act: 0,
        tag_type: 0,
        walls: Vec::new(),
        orientations: Vec::new(),
        floors: vec![vec![cell::FLOOR; cells]],
        shadow: vec![0; cells],
        objects: monsters
            .iter()
            .map(|&(id, x, y)| Ds1ObjectInput {
                kind: 1,
                id,
                x,
                y,
                flags: 0,
            })
            .collect(),
        paths: Vec::new(),
    }
}

fn ds1s() -> Ds1s {
    let (x, y) = ISLE_MONSTER;
    Ds1s(BTreeMap::from([
        (
            format!("def{ISLE_DEF}.ds1").into_bytes(),
            ds1(40, 18, &[(0, x, y)]),
        ),
        (format!("def{GATE_DEF}.ds1").into_bytes(), ds1(40, 18, &[])),
    ]))
}

fn tile(o: u32, main: u32, sub: u32, rarity: u32) -> TileInfo {
    TileInfo {
        orientation: o,
        main,
        sub,
        rarity,
        material: 0,
        subtile_flags: [0; 25],
    }
}

struct Tiles(BTreeMap<Vec<u8>, Vec<TileInfo>>);

impl TileSource for Tiles {
    fn dt1(&self, path: &[u8]) -> Option<&[TileInfo]> {
        self.0.get(path).map(Vec::as_slice)
    }
}

fn tiles() -> Tiles {
    let mut t = BTreeMap::new();
    t.insert(b"floor.dt1".to_vec(), vec![tile(0, 0, 0, 1); 4]);
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

// ---- tables -----------------------------------------------------------------------------

/// Act I sizes, offsets and Vis of `outdoor.md`'s recorded placement,
/// plus [`ISLE`] and the maze level 8; level types 1 and 3 with one DT1.
fn drlg_data() -> DrlgData {
    let mut d = DrlgData {
        levels: vec![LevelDef::default(); 150],
        ..DrlgData::default()
    };
    for l in &mut d.levels {
        l.warp = [-1; 8];
        l.level_type = 1;
    }
    let set = |d: &mut DrlgData, id: usize, ty: u32, size: (i32, i32), off: (i32, i32)| {
        d.levels[id].drlg_type = ty;
        d.levels[id].size = [size; 3];
        d.levels[id].offset = off;
    };
    set(&mut d, 1, 2, (56, 40), (0, 0));
    set(&mut d, 2, 3, (56, 96), (0, 0));
    set(&mut d, 3, 3, (80, 80), (0, 0));
    set(&mut d, 4, 3, (80, 80), (1000, 1000));
    set(&mut d, 5, 3, (80, 80), (0, 0));
    set(&mut d, 6, 3, (80, 80), (0, 0));
    set(&mut d, 7, 3, (80, 80), (0, 0));
    set(&mut d, 8, 1, (200, 200), (1500, 1000));
    set(&mut d, 17, 3, (40, 48), (0, 0));
    set(&mut d, 26, 2, (40, 18), (3000, 1000));
    set(&mut d, 27, 2, (40, 40), (0, 0));
    set(&mut d, 39, 3, (64, 64), (5000, 1148));
    set(&mut d, ISLE as usize, 2, (40, 18), (8000, 8000));
    set(&mut d, GATE as usize, 2, (40, 18), (9000, 9000));
    d.levels[8].level_type = 3;
    d.levels[2].vis = [1, 3, 0, 0, 0, 0, 0, 0];
    d.levels[3].vis = [2, 4, 17, 0, 0, 0, 0, 0];
    d.levels[1].vis = [2, 0, 0, 0, 0, 0, 0, 0];
    d.levels[4].vis = [3, 0, 0, 0, 0, 0, 0, 0];
    d.levels[17].vis = [3, 0, 0, 0, 0, 0, 0, 0];
    let mut files = vec![Vec::new(); 32];
    files[0] = b"floor.dt1".to_vec();
    d.lvltypes = vec![
        vec![Vec::new(); 32],
        files.clone(),
        vec![Vec::new(); 32],
        files,
    ];
    d
}

/// lvlprest rows 0..1199 (`Files` 1, DT1 mask 1), the preset levels'
/// rows (town Files 0, Monastery Gate, Outer Cloister Files 3, ISLE, the
/// travel level) and
/// the outdoor cell row 1103 (16 × 16, Files 2).
fn preset_data() -> PresetData {
    let mut defs: Vec<PresetDef> = (0..1200)
        .map(|i| PresetDef {
            def: i,
            files: 1,
            dt1_mask: 1,
            populate: 1,
            file: Default::default(),
            ..PresetDef::default()
        })
        .collect();
    for d in &mut defs {
        d.file[0] = format!("def{}.ds1", d.def).into_bytes();
    }
    for (i, level, files) in [
        (1100, 1, 0),
        (1101, 26, 1),
        (GATE_DEF, GATE, 1),
        (1102, 27, 3),
        (ISLE_DEF, ISLE, 1),
        (1103, 0, 2),
    ] {
        defs[i as usize].level_id = level;
        defs[i as usize].files = files;
    }
    defs[1103].size_x = 16;
    defs[1103].size_y = 16;
    PresetData {
        defs,
        monpreset_acts: Default::default(),
        monpreset: Vec::new(),
        monstats_count: 1,
        superuniques_count: 0,
        hdm_item: -1,
        tables: PresetTables::spec().expect("preset-tables.tsv"),
    }
}

fn outdoor_data(pd: &PresetData) -> OutdoorData {
    OutdoorData {
        levels: vec![
            SubDefs {
                sub_type: -1,
                sub_theme: -1,
                sub_waypoint: -1,
                sub_shrine: -1,
            };
            150
        ],
        presets: pd
            .defs
            .iter()
            .map(|d| OutdoorPreset {
                size_x: d.size_x as i32,
                size_y: d.size_y as i32,
                files: d.files,
            })
            .collect(),
        subs: Vec::new(),
    }
}

fn maze_data() -> MazeData {
    MazeData {
        rows: vec![MazeRow {
            level: 8,
            rooms: [1; 3],
            size_x: 24,
            size_y: 24,
            merge: 0,
        }],
        prest_files: (0..1200).map(|d| (d, 1)).collect(),
        specials: Specials::shipped(),
    }
}

/// Monster class 0: killable, AI 1 (Idle), no skills, no minions; level
/// 1, 5 life, 100 experience (`noRatio`: the monstats values as they
/// are, `monsters/init.md` §8.1); treasure class 1 (Normal).
fn monster_class() -> Monstats {
    let mut m: Monstats = blank();
    m.killable = true;
    m.noratio = true;
    m.level = 1;
    (m.minhp, m.maxhp, m.exp) = (5, 5, 100);
    m.treasureclass1 = 1;
    m.velocity = 1;
    (m.drain, m.drain_n, m.drain_h) = (100, 100, 100);
    m.montype = 0xFFFF;
    m.ai = 1;
    (m.aidel, m.aidel_n, m.aidel_h) = (15, 15, 15);
    (m.skill1, m.skill2, m.skill3) = (0xFFFF, 0xFFFF, 0xFFFF);
    m.rarity = 1;
    (m.mingrp, m.maxgrp) = (1, 1);
    (m.minion1, m.minion2) = (0xFFFF, 0xFFFF);
    m.enabled = true;
    m.isspawn = true;
    m
}

/// levels.txt: no monsters, act by level id; waypoints at [`ISLE`] and
/// [`GATE`] only.
fn levels() -> Vec<Levels> {
    let mut v: Vec<Levels> = vec![blank(); 150];
    for (i, l) in v.iter_mut().enumerate() {
        for m in [
            &mut l.mon1,
            &mut l.mon2,
            &mut l.mon3,
            &mut l.mon4,
            &mut l.mon5,
            &mut l.mon6,
            &mut l.mon7,
            &mut l.mon8,
            &mut l.mon9,
            &mut l.mon10,
            &mut l.mon11,
            &mut l.mon12,
            &mut l.mon13,
            &mut l.mon14,
            &mut l.mon15,
            &mut l.mon16,
            &mut l.mon17,
            &mut l.mon18,
            &mut l.mon19,
            &mut l.mon20,
            &mut l.mon21,
            &mut l.mon22,
            &mut l.mon23,
            &mut l.mon24,
            &mut l.mon25,
            &mut l.nmon1,
            &mut l.umon1,
        ] {
            *m = 0xFFFF;
        }
        l.act = d2_sim::drlg::act_of_level(i as u32);
        l.waypoint = NO_WAYPOINT;
    }
    v[ISLE as usize].waypoint = ISLE_WP;
    v[GATE as usize].waypoint = GATE_WP;
    v
}

/// A synthetic itemstatcost: 359 stats, no ops, fixed up.
fn stat_data() -> Arc<StatData> {
    let (n, size) = (359, Itemstatcost::SIZE);
    let mut records = vec![0u8; n * size];
    for (s, r) in records.chunks_mut(size).enumerate() {
        for o in [0x32, 0x48, 0x4A, 0x56, 0x58, 0x5A, 0x5C] {
            r[o..o + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }
        r[0..2].copy_from_slice(&(s as u16).to_le_bytes());
    }
    let mut t = BinTable {
        name: "itemstatcost".into(),
        source: "synthetic".into(),
        count: n,
        record_size: size,
        records,
    };
    stat_ops(&mut t);
    Arc::new(StatData {
        stats: StatTable::from_fixed(&t).expect("itemstatcost"),
        ..StatData::default()
    })
}

fn skill_rec() -> Skills {
    let mut s: Skills = blank();
    for f in [
        &mut s.auralencalc,
        &mut s.aurarangecalc,
        &mut s.aurastatcalc1,
        &mut s.calc1,
        &mut s.calc2,
        &mut s.calc3,
        &mut s.calc4,
        &mut s.passivecalc1,
        &mut s.passivecalc2,
        &mut s.passivecalc3,
        &mut s.passivecalc4,
        &mut s.passivecalc5,
        &mut s.petmax,
        &mut s.skpoints,
        &mut s.tohitcalc,
        &mut s.dmgsympercalc,
        &mut s.edmgsympercalc,
        &mut s.elensympercalc,
        &mut s.delay,
        &mut s.perdelay,
    ] {
        *f = 0xFFFF_FFFF;
    }
    s.skilldesc = 0xFFFF;
    s.charclass = 0xFF;
    (s.reqskill1, s.reqskill2, s.reqskill3) = (0xFFFF, 0xFFFF, 0xFFFF);
    s.itypea1 = 0xFFFF;
    s.srvmissile = 0xFFFF;
    s.intown = true;
    s.ingame = true;
    s
}

/// Missile 0: an arrow-like row (default flight, one sub-tile per frame,
/// collide type 3, kill on collision, to-hit), as the action wiring's
/// tests use.
fn arrow() -> MissileRow {
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

/// The right skill: start function 4, do function 8 (the Multiple Shot
/// slot, body catalogued only), `srvmissile` 0 (the generic missile of
/// `use.md` §5.4 step 7).
fn skills() -> SkillTables {
    let mut v = vec![skill_rec(), skill_rec()];
    let m = &mut v[MULTI as usize];
    (m.srvstfunc, m.mana, m.lvlmana, m.manashift) = (4, 4, 1, 8);
    (m.srvdofunc, m.srvmissile) = (8, 0);
    SkillTables {
        skills: v,
        skilldesc: vec![blank::<Skilldesc>()],
        missiles: vec![arrow()],
        skills_code: Vec::new(),
        miss_code: Vec::new(),
        level_cap: LEVEL_CAP_114D,
        stat_count: 359,
    }
}

fn combat_tables() -> CombatTables {
    let mut d: Difficultylevels = blank();
    (d.monsterfreezedivisor, d.monstercolddivisor) = (1, 1);
    (d.lifestealdivisor, d.manastealdivisor) = (1, 1);
    CombatTables {
        charstats: vec![blank::<Charstats>(); 7],
        difficultylevels: vec![d; 3],
        monstats: vec![monster_class()],
        monstats2: vec![blank::<Monstats2>()],
        hitclass: vec![*b"none", *b"hth "],
    }
}

/// experience.txt: max level 3, thresholds 0, 100, 1500 for every class
/// (the kill's 100 experience reaches level 2, `vitals.md` §4.3);
/// charstats: the sorceress (class 1) gets 5 stat points per level (the
/// other per-level columns 0).
fn vitals() -> VitalsTables {
    let row = |v: u32| Experience {
        amazon: v,
        sorceress: v,
        necromancer: v,
        paladin: v,
        barbarian: v,
        druid: v,
        assassin: v,
        ..blank()
    };
    let mut charstats = vec![blank::<Charstats>(); 7];
    charstats[1].statperlevel = 5;
    VitalsTables {
        charstats,
        experience: vec![row(3), row(0), row(100), row(1500)],
    }
}

/// AnimData with the fixture's two names (`animdata.md` §2): the
/// sorceress' cast, 8 frames at speed 256 with a missile event (2) on
/// frame 4; the monster's death, 4 frames at speed 256, no events.
fn anim_data() -> AnimData {
    let mut a = AnimData {
        buckets: vec![Vec::new(); animdata::BUCKETS],
    };
    let mut put = |name: &[u8; 8], frames, event: Option<usize>| {
        let mut events = [0u8; animdata::EVENTS];
        if let Some(i) = event {
            events[i] = 2;
        }
        let len = name.iter().position(|&b| b == 0).unwrap();
        a.buckets[animdata::hash(&name[..len])].push(AnimRecord {
            name: *name,
            frames,
            speed: 256,
            events,
        });
    };
    put(PLAYER_SC, 8, Some(4));
    put(MONSTER_DT, 4, None);
    a
}

/// Items: gold only (`ty::GOLD`, a child of `ty::MISC`); treasure class
/// 1: one pick of gold.
fn drop_tables() -> DropTables {
    let n: usize = 40;
    let words = n.div_ceil(32);
    let mut equiv = EquivMatrix {
        n,
        words,
        bits: vec![0; n * words],
    };
    for i in 1..n {
        equiv.bits[i * words] |= 1;
        equiv.bits[i * words + i / 32] |= 1 << (i % 32);
    }
    let (g, m) = (usize::from(ty::GOLD), usize::from(ty::MISC));
    equiv.bits[g * words + m / 32] |= 1 << (m % 32);
    let mut itemtypes: Vec<Itemtypes> = (0..n)
        .map(|_| {
            let mut t: Itemtypes = blank();
            (t.class, t.staffmods, t.rare) = (0xFF, 0xFF, 1);
            t
        })
        .collect();
    // Gold is always normal quality (itemtypes `Normal`, `treasure.md`
    // §6 step 1).
    itemtypes[g].normal = 1;
    let mut ratio: Itemratio = blank();
    ratio.version = 1;
    let gold = ItemRec {
        code: *b"gld ",
        type_: ty::GOLD as i16,
        level: 1,
        ..ItemRec::default()
    };
    let items = ItemTables {
        items: vec![gold],
        itemtypes,
        equiv,
        itemratio: vec![ratio],
        valshift: vec![0; 359],
        stat_shift: 6,
        stat_mask: 0x3F,
        ..ItemTables::default()
    };
    let treasure_items = items
        .items
        .iter()
        .map(|r| ItemData {
            code: r.code,
            ubercode: r.ubercode,
            ultracode: r.ultracode,
            version: r.version,
            level: r.level,
            type_: r.type_ as u16,
            type2: r.type2 as u16,
            unique: r.unique,
            quest: r.quest,
            spawnable: 1,
        })
        .collect();
    let tc = |name: &[u8], entries: Vec<TcEntry>, total| TreasureClass {
        name: name.to_vec(),
        group: 0,
        level: 0,
        total_classic: total,
        total_expansion: total,
        picks: 1,
        nodrop: 0,
        mods: [0; 6],
        entries,
    };
    let gold_entry = TcEntry {
        start_classic: 0,
        start_expansion: 0,
        id: 0,
        row: 0,
        flags: 0,
        mods: [0; 6],
    };
    DropTables {
        items,
        tcs: TreasureClasses {
            tcs: vec![tc(b"none", Vec::new(), 0), tc(b"gold", vec![gold_entry], 1)],
            group_offset: 0,
            chest: [None; 45],
            notes: Vec::new(),
        },
        treasure_items,
        superuniques: Vec::new(),
    }
}

/// One waypoint object class (0): operate 23, init 17.
fn waypoint_data() -> WaypointData {
    let mut o: Objects = blank();
    o.operatefn = 23;
    o.initfn = 17;
    o.framecnt1 = 15 << 8;
    WaypointData::new(&levels(), &[o])
}

// ---- the cube's item world --------------------------------------------------------------

// ---- the wired game ---------------------------------------------------------------------

type Sim = SimGame<WorldSim<TestPending>, WiredWorld<Rest, WiredSkills>>;

/// The transport client id of the local player.
const CLIENT: u32 = 0;

const ALIVE: PlayerGate = PlayerGate {
    mode: 1,
    uninterruptable: false,
};

fn facts((x, y): (i32, i32)) -> UnitFacts {
    UnitFacts {
        act: 0,
        pos: Pos { x, y },
        owner: None,
    }
}

/// Bounds of property 1. Generous, fixed, independent of the run's
/// length: a leak grows past them over hundreds of ticks.
const MAX_UNITS: usize = 128;
const MAX_TIMERS_PER_UNIT: usize = 8;
const MAX_ROOMS: usize = 64;
const MAX_OUTBOX: usize = 64;

/// The game of `e2e_single_player.rs`'s steps 1–3 on `game_seed`: the
/// generated level, the player's room streamed, the player and the
/// waypoint object allocated there, the client joined, the skills and
/// waypoint handlers wired.
struct Fx {
    sim: Sim,
    out: ClientBuffers,
    player: UnitId,
    /// Akara.
    npc: UnitId,
    /// The ISLE level's DRLG room count.
    level_rooms: usize,
    /// Monsters whose combat inputs were staged (`stage_combat`).
    staged: Vec<UnitId>,
    /// Missiles whose damage was staged.
    armed: Vec<UnitId>,
}

impl Fx {
    fn new(game_seed: u32) -> Self {
        let data = Arc::new(drlg_data());
        let pd = preset_data();
        let types = SharedTypes::new(WorldTypes::new(
            data.clone(),
            Maze::new(maze_data()),
            pd.clone(),
            outdoor_data(&pd),
            Box::new(ds1s()),
            Box::new(SubFileMap::default()),
        ));
        let mut handle = types.clone();
        let drlg = Drlg::create(0, DRLG_SEED, 0, 0, false, &data, &mut handle).expect("act 0");
        let mut dungeon = Dungeon::default();
        dungeon.acts[0] = Some(drlg);
        let world = DrlgWorld {
            dungeon,
            data,
            tiles: Box::new(tiles()),
            types: Box::new(handle),
        };
        let tables = ActionTables {
            missiles: vec![arrow()],
            skills: skills(),
            combat: combat_tables(),
            levels: levels(),
            skill_modes: vec![[0; 3]],
        };
        let book = Book::default();
        let mut hooks = ActionHooks::new(
            Arc::new(tables),
            world,
            Seed::init_low(game_seed),
            TestPending {
                book: book.clone(),
                drops: Some(DeathDrops::new(
                    Arc::new(drop_tables()),
                    GameFields::new(Seed::init_low(game_seed), false),
                )),
                ..TestPending::default()
            },
        );
        hooks.anim_data = Some(Arc::new(anim_data()));
        hooks.vitals = Some(Arc::new(vitals()));
        let wt = WorldTables {
            pop: PopTables::from_records(&levels(), &[monster_class()], &[blank()], &[]),
            monstats: vec![monster_class()],
            monstats2: vec![blank()],
            monlvl: vec![blank::<Monlvl>(); 10],
            levels: levels(),
            difficultylevels: vec![blank::<Difficultylevels>(); 3],
            monstats_extra: vec![MonstatsExtra::default()],
            components: vec![[0; 16]],
            ..WorldTables::default()
        };
        let state = WorldState::new(types, Arc::new(wt), GameInfo::default());
        let unit_data = UnitData {
            // Class 0 (the DS1 monster) moves; the others (Akara) are
            // enabled and stand.
            monsters: (0..N_MONSTATS)
                .map(|c| MonsterInfo {
                    enabled: true,
                    aidel: [15; 3],
                    moves: if c == 0 { 1 << 4 } else { 0 },
                })
                .collect(),
            ..UnitData::default()
        };
        let mut sim = WorldSim::new(stat_data(), unit_data, hooks, state);
        sim.create_regions();
        // The world systems' creation seeds after the regions (a fixture
        // order, `docs/handoff/e2e-next.md` §4).
        let mut seed = sim.action.hooks().game_seed;
        let ctl = NpcControl::new(&npc_monstats(), Vec::new(), false, 0, &mut seed).expect("npc");
        let quests = QuestControl::new(&QuestTables::load().unwrap(), &mut seed).unwrap();
        sim.action.hooks().game_seed = seed;
        let mut game = Game::new();
        game.lists.ensure_act(0).unwrap();

        let (level_rooms, room) = sim
            .action
            .hooks()
            .drlg
            .with_act(0, &mut game.lists, |d, svc| {
                let l = d.get_or_alloc_level(svc.data, svc.types, ISLE)?;
                d.generate_level(svc.data, svc.types, l)?;
                let rooms = d.level_rooms(l);
                let a = *rooms
                    .iter()
                    .find(|&&r| d.room(r).rect == d2_sim::drlg::TileRect::new(8000, 8000, 8, 8))
                    .expect("room at the level origin");
                let active = d.stream_room(svc, a)?.expect("active");
                Ok::<_, d2_sim::drlg::DrlgError>((rooms.len(), active))
            })
            .expect("act 0")
            .expect("generated");

        let mut alloc = |ty, class, (x, y): (i32, i32)| {
            let req = AllocRequest {
                ty,
                class,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: ty == UnitType::Player,
            };
            sim.action
                .with(&mut game, |g, v| v.allocate(g, &req, x, y))
                .expect("allocated")
        };
        let object = alloc(UnitType::Object, 0, WP_AT);
        let player = alloc(UnitType::Player, 1, PLAYER_AT);
        // Akara beside the player (her kind init is the monster spec's).
        let npc = alloc(UnitType::Monster, u32::from(class::AKARA), NPC_AT);
        sim.action.sys.units.get_mut(player).unwrap().mode = 1;
        let rec = sim.action.hooks().waypoints.entry(player).or_default();
        rec.get_mut(0).set(ISLE_WP.into()).unwrap();
        rec.get_mut(0).set(GATE_WP.into()).unwrap();
        // Mana and max mana 4000 (1/256 units); the to-hit inputs of
        // `stage_combat` (attack rating 100, level 1, `hit.md` §3).
        sim.action.with(&mut game, |_, v| {
            v.set_base(player, 8, 4000);
            v.set_base(player, 9, 4000);
            v.set_base(player, 19, 100);
            v.set_base(player, 12, 1);
            v.set_base(player, GOLD, 5000);
        });

        let mut rest = Rest::default();
        rest.quests.insert(player, PlayerQuests::default());
        let action = ActionWorld {
            waypoints: Some(waypoint_data()),
            skills: WiredSkills::default(),
            ..ActionWorld::default()
        };
        let mut world = WiredWorld::new(
            action,
            item_tables(),
            quests,
            ctl,
            vendor_tables(),
            rest,
            1000,
        );
        world.state.add_npc(npc);
        // The player's buckler and cap, made by the economy wiring on the
        // game seed, held in the staged inventory.
        let (buckler, cap) = world.with_economy(&mut game, &mut sim, |econ, _| {
            let mut make = |record: usize| {
                let mut rq = ItemRequest {
                    item: record as i32,
                    ilvl: 1,
                    quality: 2,
                    format: 1,
                    ..ItemRequest::default()
                };
                let spawn = ItemSpawn {
                    room: None,
                    mode: 0,
                    init_flags: 1,
                };
                econ.create_item(&mut rq, false, spawn).expect("item")
            };
            (make(BUC), make(CAP))
        });
        world.rest.inventory.extend([buckler, cap]);
        let mut s: Sim = SimGame::with_world(game, sim, world);
        s.join(CLIENT, Some(player), None, client_state::IN_GAME)
            .unwrap();
        s.set_player(
            player,
            PlayerFields {
                gate: ALIVE,
                data: Some(PlayerData { last_accept: 0 }),
            },
        );
        s.set_unit(player, facts(PLAYER_AT));
        s.set_unit(object, facts(WP_AT));
        s.set_unit(npc, facts(NPC_AT));
        let multi = SkillEntry {
            skill: MULTI,
            base: 10,
            owner_guid: -1,
            ..SkillEntry::default()
        };
        {
            let mut b = book.get();
            b.list = vec![
                SkillEntry {
                    skill: ATTACK,
                    base: 1,
                    owner_guid: -1,
                    ..SkillEntry::default()
                },
                multi,
            ];
            b.right = Some(multi);
        }
        let mut out = ClientBuffers::new();
        out.add_client(CLIENT);
        Fx {
            sim: s,
            out,
            player,
            npc,
            level_rooms,
            staged: Vec::new(),
            armed: Vec::new(),
        }
    }

    /// A listed unit's GUID (−1 once it is gone).
    fn guid(&self, u: UnitId) -> u32 {
        self.sim.game.lists.unit(u).map_or(u32::MAX, |e| e.guid)
    }

    fn pending(&mut self) -> &mut TestPending {
        &mut self.sim.events.action.hooks().x
    }

    /// Stages what the unwritten specs would hold for each unit: its act
    /// and position for the dispatcher's target checks (`UnitFacts`, as
    /// the e2e stages them), and for a new monster the kind init's target
    /// flags (`missiles.md` §R4.2) and its run-time collision bit
    /// (`rooms.md` §10.6), as the e2e's `stage_combat`. The cast aims at
    /// the first monster.
    fn stage(&mut self) {
        let mut all = Vec::new();
        for ty in UnitType::ALL {
            all.extend(self.sim.game.lists.units_of_type(ty));
        }
        all.sort();
        for u in all {
            let pos = self.pending().position(u);
            self.sim.set_unit(u, facts(pos));
        }
        let mut monsters = self.sim.game.lists.units_of_type(UnitType::Monster);
        monsters.sort();
        for m in monsters {
            if m == self.npc || self.staged.contains(&m) {
                continue;
            }
            self.staged.push(m);
            let sim = &mut self.sim;
            sim.events.action.sys.units.get_mut(m).unwrap().flags |=
                unit_flag::IS_VALID_TARGET | unit_flag::CAN_BE_ATTACKED;
            let (x, y) = sim.events.action.hooks().x.position(m);
            let Some(room) = sim.game.lists.unit(m).and_then(|u| u.room()) else {
                continue;
            };
            let game = &sim.game;
            if let Some(c) = sim
                .events
                .action
                .sys
                .hooks
                .drlg
                .collision_mut(game, room, x, y)
            {
                *c |= bits::MONSTER;
            }
            if self.staged.len() == 1 {
                self.pending().aim_at = (x, y);
            }
        }
    }

    /// One message through the dispatcher (`intents-events.md` §2.3,
    /// §2.4): its result code.
    fn send(&mut self, m: &[u8]) -> ResultCode {
        dispatch(
            &mut self.sim,
            &ProtoSizes,
            &mut self.out,
            CLIENT,
            ALIVE,
            m,
            m.len(),
        )
    }

    /// One tick. New missiles get the e2e's damage (10 points, stats 21
    /// and 22 in 1/256): the damage setup `0x0059F900` is the skills
    /// spec's (`Pending`), not written.
    fn tick(&mut self) {
        self.sim.tick(&mut self.out);
        let mut missiles = self.sim.game.lists.units_of_type(UnitType::Missile);
        missiles.sort();
        let sim = &mut self.sim;
        for m in missiles {
            if self.armed.contains(&m) {
                continue;
            }
            self.armed.push(m);
            sim.events.action.with(&mut sim.game, |_, v| {
                v.set_base(m, 21, 2560);
                v.set_base(m, 22, 2560);
            });
        }
    }

    /// Every S→C buffer queued for the client since the last call (the
    /// handlers' and the tick's), in order.
    fn drain(&mut self) -> Vec<Vec<u8>> {
        let mut v = Vec::new();
        while let Some(b) = self.out.pop(CLIENT) {
            v.push(b);
        }
        v
    }

    /// Every error so far: world adapters, level types, action adapters,
    /// the unit dispatch, the world handlers' faults, the tick's queueing
    /// faults.
    fn errors(&self) -> Vec<String> {
        let mut e = self.sim.events.errors();
        e.extend(
            self.sim
                .world
                .action
                .faults
                .iter()
                .map(|f| format!("{f:?}")),
        );
        e.extend(self.sim.world.state.errors.iter().map(|f| format!("{f:?}")));
        e.extend(self.sim.tick_faults.iter().map(|f| format!("{f:?}")));
        e
    }

    /// Live timers in the queue: every bucket and every-tick list.
    fn timer_count(&self) -> usize {
        let t = &self.sim.game.timers;
        TimerClass::RUN_ORDER
            .iter()
            .map(|&c| (0..64).map(|b| t.bucket(c, b).len()).sum::<usize>() + t.every_tick(c).len())
            .sum()
    }

    /// Every unit in the lists, by type then id.
    fn units(&self) -> Vec<UnitId> {
        let mut all = Vec::new();
        for ty in UnitType::ALL {
            let mut v = self.sim.game.lists.units_of_type(ty);
            v.sort();
            all.extend(v);
        }
        all
    }

    /// Property 1's bounds.
    fn check_bounds(&self) -> Result<(), String> {
        let units = self.units().len();
        let timers = self.timer_count();
        let lists = &self.sim.game.lists;
        let mut rooms = 0;
        let mut r = lists.room_first(0);
        while let Some(id) = r {
            rooms += 1;
            r = lists.room_next(id);
        }
        let active = lists.active_rooms(0).len();
        let outbox = self.sim.events.action.sys.hooks.x.sent.len();
        if units > MAX_UNITS
            || timers > MAX_TIMERS_PER_UNIT * units.max(1)
            || rooms > MAX_ROOMS
            || active > rooms
            || outbox > MAX_OUTBOX
        {
            return Err(format!(
                "bounds: units {units}, timers {timers}, rooms {rooms}, active {active}, outbox {outbox}"
            ));
        }
        Ok(())
    }

    /// The state digest (property 2): built from public state only. Per
    /// unit (type, id order): GUID, class, mode, flags, act, room, the
    /// fixture position, the unit seed, the item seed, the animation
    /// fields, its stat list (base and full entries), its timers (event,
    /// args, expire); then the game frame, the game seed, the drop
    /// state's seed, the act's active rooms, the timer queue's current
    /// bucket. Readable lines, so a diff names the first unit that moved.
    fn digest(&mut self) -> String {
        let mut d = String::new();
        let units = self.units();
        let sim = &mut self.sim;
        let game = &sim.game;
        let a = &mut sim.events.action;
        let _ = writeln!(
            d,
            "frame {} bucket {}",
            game.frame,
            game.timers.current_bucket()
        );
        let _ = writeln!(d, "game seed {:?}", a.sys.hooks.game_seed);
        if let Some(drops) = &a.sys.hooks.x.drops {
            let _ = writeln!(d, "drop seed {:?}", drops.fields.seed);
        }
        let _ = writeln!(d, "active {:?}", game.lists.active_rooms(0));
        for u in units {
            let e = game.lists.unit(u).expect("listed");
            let _ = write!(d, "{:?} {:?} guid {} room {:?}", e.ty, u, e.guid, e.room());
            if let Some(r) = a.sys.units.get(u) {
                let _ = write!(
                    d,
                    " class {} mode {} flags {:#x}/{:#x} act {} seed {:?} init {} item {:?} anim {:?} {} {} {}",
                    r.class,
                    r.mode,
                    r.flags,
                    r.flags2,
                    r.act,
                    r.seed,
                    r.init_seed,
                    r.item_seed,
                    r.anim.record,
                    r.anim.frame,
                    r.anim.frame_count,
                    r.anim.speed,
                );
            }
            let _ = write!(d, " pos {:?}", a.sys.hooks.x.pos.get(&u));
            if let Some(l) = a.sys.stats.unit_list(u) {
                let _ = write!(
                    d,
                    " stats {:?} {:?}",
                    a.sys.stats.base_entries(l),
                    a.sys.stats.full_entries(l)
                );
            }
            let t = &game.timers;
            let timers: Vec<_> = t
                .unit_timers(u)
                .into_iter()
                .map(|i| (t.event(i), t.expire(i)))
                .collect();
            let _ = writeln!(d, " timers {timers:?}");
        }
        // The trade world: the item store (records, flags, seeds), the
        // vendor records (stores), the NPC control, the quest state, the
        // world's interaction lists.
        let w = &sim.world;
        let _ = writeln!(d, "items {:?}", w.items);
        let _ = writeln!(d, "vendors {:?}", w.state.vendors);
        let _ = writeln!(d, "interactions {:?}", w.state.lists);
        let _ = writeln!(d, "npc {:?}", w.npc);
        let _ = writeln!(d, "quests {:?}", w.quests);
        d
    }
}

fn hash(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

// ---- messages ---------------------------------------------------------------------------

/// One generated C→S game message (as `prop_handle.rs`): any id, layout
/// fields from a pool built on the game's GUIDs.
#[derive(Clone, Debug)]
struct Gen {
    id: u8,
    picks: Vec<(u8, u32)>,
    raw: Vec<u8>,
    /// 0: exact size; 1: one byte short; 2: one byte long.
    len: u8,
}

fn gen_any() -> impl Strategy<Value = Gen> {
    (
        1u8..0x67,
        prop::collection::vec((any::<u8>(), any::<u32>()), 8),
        prop::collection::vec(any::<u8>(), 0..64),
        prop_oneof![8 => Just(0u8), 1 => Just(1u8), 1 => Just(2u8)],
    )
        .prop_map(|(id, picks, raw, len)| Gen {
            id,
            picks,
            raw,
            len,
        })
}

/// The ids with a handler on this host (skills 0x05–0x11, 0x3A–0x3C;
/// waypoints 0x49) get most of the generated messages.
fn gen() -> impl Strategy<Value = Gen> {
    let owned = prop::sample::select(vec![
        0x05u8, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x3A, 0x3B,
        0x3C, 0x49,
    ]);
    prop_oneof![
        1 => gen_any(),
        2 => (owned, gen_any()).prop_map(|(id, g)| Gen { id, ..g }),
    ]
}

fn pick(name: &str, guids: &[u32], (sel, rnd): (u8, u32)) -> u32 {
    if sel & 0x80 != 0 {
        let guid = match rnd as usize % (guids.len() + 1) {
            i if i < guids.len() => guids[i],
            _ => u32::MAX,
        };
        return match name {
            "x" => PLAYER_AT.0 as u32 + rnd % 101 - 50,
            "y" => PLAYER_AT.1 as u32 + rnd % 101 - 50,
            "type" => rnd % 6,
            "id" | "item" | "unit" | "wp" | "target" | "player" => guid,
            "skill" => rnd % 3,
            "level" => [ISLE, GATE, rnd % 150][rnd as usize % 3],
            "stat" => rnd % 8,
            "left" => rnd % 2,
            _ => rnd % 4,
        };
    }
    const POOL: &[u32] = &[
        0,
        1,
        2,
        5,
        6,
        7,
        0xFF,
        0xFFFF,
        0x7FFF,
        0x8000,
        0x7FFF_FFFF,
        0x8000_0000,
        u32::MAX,
        40_020,
        40_070,
        40_071,
        39_969,
        39_970,
        358,
        359,
    ];
    let n = POOL.len() + guids.len() + 1;
    match sel as usize % n {
        i if i < POOL.len() => POOL[i],
        i if i < POOL.len() + guids.len() => guids[i - POOL.len()],
        _ => rnd,
    }
}

fn build(g: &Gen, guids: &[u32]) -> Vec<u8> {
    let row = &CLIENT_MESSAGES[g.id as usize];
    let mut m = g.raw.clone();
    match row.transport_size.fixed() {
        Some(n) => m.resize(n, 0),
        None if g.id == 0x14 => m.resize(m.len().clamp(4, 275), 0),
        None => m.truncate(0x200 - 1),
    }
    if m.is_empty() {
        m.push(0);
    }
    m[0] = g.id;
    for (f, &p) in row.layout.iter().zip(&g.picks) {
        let Some(off) = f.offset else { continue };
        let off = off as usize;
        let v = pick(f.name, guids, p);
        let (w, mask) = match f.ty {
            FieldType::U8 => (1, 0xFF),
            FieldType::U16 => (2, 0xFFFF),
            FieldType::U32 => (4, u32::MAX),
            FieldType::Bits(n) => (4, (1u32 << n) - 1),
            FieldType::Bit(n) => (4, 1u32 << n),
            _ => continue,
        };
        if off + w > m.len() {
            continue;
        }
        let mut word = [0u8; 4];
        word[..w].copy_from_slice(&m[off..off + w]);
        let old = u32::from_le_bytes(word);
        let v = match f.ty {
            FieldType::Bit(n) => (v & 1) << n,
            _ => v,
        };
        let new = (old & !mask) | (v & mask);
        m[off..off + w].copy_from_slice(&new.to_le_bytes()[..w]);
    }
    match g.len {
        1 if m.len() > 1 => {
            m.pop();
        }
        2 => m.push(0),
        _ => {}
    }
    m
}

fn bytes<M: FixedMessage>(m: &M) -> Vec<u8> {
    let mut b = vec![0; M::SIZE];
    m.write(&mut b);
    b
}

/// One step of a run: a message (or none), then `ticks` ticks.
#[derive(Clone, Debug)]
enum Msg {
    None,
    /// Right skill at the `n`-th monster's position (or a point near the
    /// player when there is none): the e2e's cast.
    CastAt(u8),
    /// Right skill on the `n`-th unit of the game.
    CastOn(u8),
    /// A stat point (`vitals.md` §2) on stat `n % 8`.
    Stat(u8),
    /// Waypoint travel to ISLE or GATE (`waypoints.md` §6).
    Travel(bool),
    /// Talk to Akara (`npc.md` §2), chat (§3), trade (`vendors.md` §4).
    Talk,
    Chat,
    Trade,
    /// Buy the `n`-th store item (`vendors.md` §7.1).
    Buy(u8),
    /// Sell the `n`-th item of the player's staged inventory (§7.2).
    Sell(u8),
    /// Anything, valid or not.
    Any(Gen),
}

#[derive(Clone, Debug)]
struct Op {
    msg: Msg,
    ticks: u8,
}

fn op() -> impl Strategy<Value = Op> {
    let msg = prop_oneof![
        2 => Just(Msg::None),
        2 => any::<u8>().prop_map(Msg::CastAt),
        1 => any::<u8>().prop_map(Msg::CastOn),
        1 => any::<u8>().prop_map(Msg::Stat),
        1 => any::<bool>().prop_map(Msg::Travel),
        1 => Just(Msg::Talk),
        1 => Just(Msg::Chat),
        1 => Just(Msg::Trade),
        1 => any::<u8>().prop_map(Msg::Buy),
        1 => any::<u8>().prop_map(Msg::Sell),
        6 => gen().prop_map(Msg::Any),
    ];
    (msg, 0u8..8).prop_map(|(msg, ticks)| Op { msg, ticks })
}

fn message(fx: &mut Fx, msg: &Msg) -> Option<Vec<u8>> {
    let units = fx.units();
    let nth = |v: &[UnitId], n: u8| (!v.is_empty()).then(|| v[n as usize % v.len()]);
    Some(match msg {
        Msg::None => return None,
        Msg::CastAt(n) => {
            let mut monsters = fx.sim.game.lists.units_of_type(UnitType::Monster);
            monsters.sort();
            let (x, y) = match nth(&monsters, *n) {
                Some(m) => fx.pending().position(m),
                None => (PLAYER_AT.0 - 8, PLAYER_AT.1 - 10),
            };
            bytes(&RightSkill {
                x: x as u16,
                y: y as u16,
            })
        }
        Msg::CastOn(n) => {
            let u = nth(&units, *n)?;
            let e = fx.sim.game.lists.unit(u)?;
            bytes(&RightSkillOnUnit {
                type_: e.ty as u32,
                id: e.guid,
            })
        }
        Msg::Stat(n) => bytes(&AddStatPoint {
            stat: u16::from(*n % 8),
        }),
        Msg::Travel(isle) => {
            let wp = fx.units().into_iter().find_map(|u| {
                let e = fx.sim.game.lists.unit(u)?;
                (e.ty == UnitType::Object).then_some(e.guid)
            })?;
            bytes(&TakeOrCloseWp {
                wp,
                level: if *isle { ISLE } else { GATE } as u16,
            })
        }
        Msg::Talk => bytes(&InteractWithEntity {
            type_: 1,
            id: fx.guid(fx.npc),
        }),
        Msg::Chat => bytes(&InitEntityChat {
            id: fx.guid(fx.npc),
        }),
        Msg::Trade => bytes(&EntityAction {
            action: 1,
            npc: fx.guid(fx.npc),
            item: 0,
        }),
        Msg::Buy(n) => {
            let w = &fx.sim.world;
            let store = w.state.vendors[w.state.vendor_index(class::AKARA)?]
                .store
                .clone();
            let item = nth(&store, *n)?;
            bytes(&BuyItem {
                npc: fx.guid(fx.npc),
                item: fx.guid(item),
                mode: 0,
                cost: 0,
            })
        }
        Msg::Sell(n) => {
            let inv: Vec<UnitId> = fx.sim.world.rest.inventory.iter().copied().collect();
            let item = nth(&inv, *n)?;
            bytes(&SellItem {
                npc: fx.guid(fx.npc),
                item: fx.guid(item),
                tab: 0,
                cost: 0,
            })
        }
        Msg::Any(g) => {
            let guids: Vec<u32> = units
                .iter()
                .filter_map(|&u| Some(fx.sim.game.lists.unit(u)?.guid))
                .collect();
            build(g, &guids)
        }
    })
}

/// Whether the dispatcher itself rejects `m` before any handler
/// (`intents-events.md` §2.3 rule 3, §2.4 rules 1, 3, 4), given the
/// player's staged position; checked against the result code.
fn dispatcher_rejects(fx: &Fx, m: &[u8], code: ResultCode) -> Result<bool, String> {
    let (id, size) = (m[0], m.len());
    let row = &CLIENT_MESSAGES[id as usize];
    if gate(id) == Gate::Dead {
        // The player is alive: the dead gate is closed (§2.3 rule 3).
        return if code == ResultCode::Done {
            Ok(true)
        } else {
            Err(format!("{m:02X?}: closed gate gave {code:?}"))
        };
    }
    let expect = if kind(id) != Kind::Handler {
        return Ok(code == ResultCode::Malformed);
    } else if row.transport_size.fixed().is_some_and(|n| n != size) {
        ResultCode::Malformed
    } else if is_unit(id) && u32::from_le_bytes([m[1], m[2], m[3], m[4]]) >= 6 {
        ResultCode::Invalid
    } else if is_point(id) {
        let p = fx.sim.point_state(CLIENT).expect("staged").player;
        let at = |o: usize| i32::from(u16::from_le_bytes([m[o], m[o + 1]]));
        if (at(1) - p.x).abs() > 50 || (at(3) - p.y).abs() > 50 {
            ResultCode::Refused
        } else {
            return Ok(false);
        }
    } else {
        return Ok(false);
    };
    if code == expect {
        Ok(true)
    } else {
        Err(format!("{m:02X?}: expected {expect:?}, got {code:?}"))
    }
}

/// What a run leaves behind, per message and per tick: the step, the
/// result code, the S→C buffers, the digest.
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    /// The message's id, `None` for a tick.
    id: Option<u8>,
    what: String,
    code: Option<ResultCode>,
    sent: Vec<Vec<u8>>,
    digest: String,
}

/// Runs `ops` on a fresh game: before each message the fixture stages
/// unit facts; after each message and each tick the bounds and the
/// error lists are checked (property 1), the dispatcher's rejections
/// compared (property 4), and a record kept (property 2). After the ops,
/// `quiet` ticks without messages.
fn run(game_seed: u32, ops: &[Op], quiet: u32) -> Result<Vec<Entry>, String> {
    let mut fx = Fx::new(game_seed);
    let mut log = Vec::new();
    let tick = |fx: &mut Fx, log: &mut Vec<Entry>| -> Result<(), String> {
        fx.tick();
        let sent = fx.drain();
        let e = fx.errors();
        if !e.is_empty() {
            return Err(format!("tick {}: {e:?}", fx.sim.game.frame));
        }
        fx.check_bounds()?;
        log.push(Entry {
            id: None,
            what: "tick".into(),
            code: None,
            sent,
            digest: fx.digest(),
        });
        Ok(())
    };
    for op in ops {
        fx.stage();
        if let Some(m) = message(&mut fx, &op.msg) {
            let before = fx.digest();
            let code = fx.send(&m);
            if dispatcher_rejects(&fx, &m, code)? {
                let after = fx.digest();
                if after != before {
                    return Err(format!(
                        "{m:02X?} was rejected ({code:?}) and changed the digest:\n{before}\n---\n{after}"
                    ));
                }
            }
            let e = fx.errors();
            if !e.is_empty() {
                return Err(format!("{m:02X?}: {e:?}"));
            }
            fx.check_bounds()?;
            log.push(Entry {
                id: Some(m[0]),
                what: format!("{m:02X?}"),
                code: Some(code),
                sent: fx.drain(),
                digest: fx.digest(),
            });
        }
        for _ in 0..op.ticks {
            tick(&mut fx, &mut log)?;
        }
    }
    for _ in 0..quiet {
        tick(&mut fx, &mut log)?;
    }
    Ok(log)
}

/// The first record where two runs differ, for the failure message.
fn first_diff(a: &[Entry], b: &[Entry]) -> String {
    match a.iter().zip(b).position(|(x, y)| x != y) {
        Some(i) => format!("record {i}:\n{:#?}\n---\n{:#?}", a[i], b[i]),
        None => format!("lengths {} vs {}", a.len(), b.len()),
    }
}

proptest! {
    #![proptest_config(config(8))]

    /// Properties 1, 2 and 4: hundreds of ticks with valid and invalid
    /// messages run without a panic, an error or growth past the bounds;
    /// a second run of the same seed and sequence is byte-identical.
    #[test]
    fn same_seed_same_messages_same_game(
        seed in any::<u32>(),
        ops in prop::collection::vec(op(), 40..80),
    ) {
        let a = run(seed, &ops, 100).map_err(TestCaseError::fail)?;
        let b = run(seed, &ops, 100).map_err(TestCaseError::fail)?;
        let ticks = a.iter().filter(|r| r.code.is_none()).count();
        prop_assert!(ticks >= 100);
        prop_assert!(a == b, "{}", first_diff(&a, &b));
    }

    /// Property 3: another game seed gives another digest sequence.
    #[test]
    fn other_seed_other_digest(
        seed in any::<u32>(),
        other in any::<u32>(),
        ops in prop::collection::vec(op(), 0..10),
    ) {
        prop_assume!(seed != other);
        let a = run(seed, &ops, 5).map_err(TestCaseError::fail)?;
        let b = run(other, &ops, 5).map_err(TestCaseError::fail)?;
        let (da, db): (Vec<u64>, Vec<u64>) = (
            a.iter().map(|r| hash(&r.digest)).collect(),
            b.iter().map(|r| hash(&r.digest)).collect(),
        );
        prop_assert_ne!(da, db);
    }
}

/// The fixture is live: the first tick populates the level (the DS1's
/// preset monster, `population.md` §11.1), the e2e's cast at it is
/// accepted, the missile flies and the monster dies; the digest moves
/// with the game.
#[test]
fn fixture_reaches_the_wired_paths() {
    let mut fx = Fx::new(GAME_SEED);
    assert_eq!(fx.level_rooms, 15);
    let d0 = fx.digest();
    fx.tick();
    assert!(fx.errors().is_empty(), "{:?}", fx.errors());
    let mut monsters = fx.sim.game.lists.units_of_type(UnitType::Monster);
    monsters.retain(|&m| m != fx.npc);
    assert_eq!(monsters.len(), 1, "the DS1 preset monster");
    assert_ne!(fx.digest(), d0);
    fx.stage();
    let (x, y) = fx.pending().position(monsters[0]);
    let code = fx.send(&bytes(&RightSkill {
        x: x as u16,
        y: y as u16,
    }));
    assert_eq!(code, ResultCode::Done);
    for _ in 0..40 {
        fx.tick();
    }
    assert!(fx.errors().is_empty(), "{:?}", fx.errors());
    let mode = fx
        .sim
        .events
        .action
        .sys
        .units
        .get(monsters[0])
        .map(|r| r.mode);
    assert_eq!(mode, Some(0), "killed (DT)");
    assert_eq!(
        fx.sim
            .events
            .action
            .sys
            .units
            .get(fx.player)
            .map(|r| r.mode),
        Some(1)
    );
}

/// The generated runs reach the wired paths (a spot check of the
/// generator, not a property): over a fixed sample of runs, the handled
/// ids answer 0 and refusals, the dispatcher rejects, missiles fly, and
/// the trade handlers send S→C messages.
#[test]
fn runs_reach_the_handlers() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let ops = prop::collection::vec(op(), 60);
    let mut codes: BTreeMap<(u8, String), u32> = BTreeMap::new();
    let (mut sent, mut missiles, mut ticks) = (BTreeMap::<u8, u32>::new(), 0, 0);
    for seed in 0..4 {
        let ops = ops.new_tree(&mut runner).unwrap().current();
        for r in run(seed, &ops, 50).unwrap() {
            match r.id {
                Some(id) => *codes.entry((id, format!("{:?}", r.code))).or_default() += 1,
                None => ticks += 1,
            }
            for b in r.sent {
                *sent.entry(b[0]).or_default() += 1;
            }
            missiles += r
                .digest
                .lines()
                .filter(|l| l.starts_with("Missile"))
                .count();
        }
    }
    eprintln!("ticks {ticks}, missile-ticks {missiles}\ncodes {codes:?}\nS→C buffers by first id {sent:?}");
    let done = |id: u8| codes.contains_key(&(id, "Some(Done)".to_string()));
    for id in [0x0C, 0x0D, 0x13, 0x2F, 0x38, 0x3A, 0x49] {
        assert!(done(id), "{id:#04X} never accepted");
    }
    for c in ["Some(Malformed)", "Some(Invalid)", "Some(Refused)"] {
        assert!(codes.keys().any(|k| k.1 == c), "no {c}");
    }
    assert!(ticks >= 400 && missiles > 0);
    assert!(
        sent.contains_key(&0x27) && sent.contains_key(&0x2A),
        "{sent:?}"
    );
}
