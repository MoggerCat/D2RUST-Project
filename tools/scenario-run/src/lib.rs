// Spec: specs/tools/scenario.md §4 (the d2rs runner); traces/FORMAT.md §Scenario traces
//! The d2rs side of a differential scenario: builds a wired
//! single-player game from the scenario's seed, difficulty and inline
//! character (the `test-fixtures` game builders on the synthetic or the
//! user's install), injects each step at its tick through the net send
//! and the server's dispatch (`d2_server::transport`, `dispatch`), runs
//! the ticks and writes the scenario trace.
//!
//! What the runner stages that no spec decides yet (each is written to
//! the trace header's `gaps`, never approximated silently):
//!
//! - the start position `char at default`: 5 sub-tiles right of and
//!   below the area's first waypoint object (`synthetic_game.rs`
//!   staging; TODO(spec: unit placement, `drlg/levels.md` §10));
//! - the room presets: only waypoint objects (operate function 23) are
//!   allocated, as the server tests do;
//! - items (`char item`): not created, TODO(spec: item creation from a
//!   code and inventory placement);
//! - `char save`: no loader, TODO(spec: formats/d2s.md) (an error, not a
//!   gap: the character would be wrong);
//! - `record frames`: no renderer in the runner; `rng-draws`: the sim's
//!   seeds have no draw log.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use conformance::packets::{DispatchServer, PacketServer};
use conformance::scenario::script::{
    encode, spawn_position, Spawn, SpawnKind, Start, StepMsg, Stream, UnitRef, World,
};
use conformance::scenario::trace::{Header, Record, TraceFile};
use conformance::scenario::Scenario;
use d2_data::tables::Objects;
use d2_formats::mpq::ArchiveSet;
use d2_server::adapters::handlers::world::{ActionEvents, ActionWorld, Outbox};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use d2_server::seams::{ClientId, MessageSink, PlayerGate, Pos, SessionHandler};
use d2_server::transport::{Classified, Queue, ServerQueues};
use d2_sim::combat::vitals::init_player_stats;
use d2_sim::drlg::{act_of_level, DrlgError};
use d2_sim::game::Game;
use d2_sim::monsters::init::{self, GameInfo, InitHost};
use d2_sim::monsters::population::{placement, spawn as pop_spawn};
use d2_sim::rng::Seed;
use d2_sim::skills::SkillEntry;
use d2_sim::stats::lists::NoHost;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, Pending};
use d2_sim::wiring::economy::GameFields;
use d2_sim::wiring::interaction::{VitalsRest, VitalsView};
use d2_sim::wiring::worldgen::dispatch::WorldSim;
use d2_sim::wiring::worldgen::{WorldPending, WorldState};
use test_fixtures::game::{ActCreation, GameData};

/// The trace header's `tool`.
pub const TOOL: &str = concat!("scenario-run ", env!("CARGO_PKG_VERSION"));

/// The scenario character's client (the local client, id 0).
pub const CLIENT: ClientId = 0;

/// Sub-tiles per tile (`drlg/rooms.md` §9.2).
const SUB: i32 = 5;

/// `objects` operate function of a waypoint (`world/waypoints.md` §5).
const WAYPOINT_OPERATE: u8 = 23;

/// The act-0 town level (`drlg/levels.md` §2 step 2).
const ACT1_TOWN: u32 = 1;

/// Milliseconds per tick (`sim/tick.md` §1): the clock the dispatch
/// sees at tick t is t × 40.
const TICK_MS: u32 = 40;

/// Why a scenario did not run (`scenario.md` §4 rule 10).
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("game data: {0}")]
    Data(String),
    #[error("the d2rs runner cannot run this scenario: {0}")]
    Unsupported(String),
    #[error("building the game: {0}")]
    Build(String),
    #[error("tick {tick}: {message}")]
    Server { tick: u32, message: String },
}

/// Which install the game runs on (the header's `data`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataKind {
    Synthetic,
    Live,
}

/// The game data of a run.
pub struct Data {
    pub game: GameData,
    pub kind: DataKind,
}

impl Data {
    /// The synthetic install (`test-fixtures`), built in `dir`.
    pub fn synthetic(dir: &Path) -> Result<Self, RunError> {
        let i = test_fixtures::install::build(dir, &test_fixtures::synth::synthetic())
            .map_err(|e| RunError::Data(e.to_string()))?;
        Ok(Self {
            game: GameData::from_install(&i).map_err(|e| RunError::Data(e.to_string()))?,
            kind: DataKind::Synthetic,
        })
    }

    /// The user's install in `dir` (`D2_GAME_DIR`).
    pub fn live(dir: &Path) -> Result<Self, RunError> {
        let data = |e: String| RunError::Data(format!("{}: {e}", dir.display()));
        let set = ArchiveSet::open_dir(dir).map_err(|e| data(e.to_string()))?;
        let bins = d2_data::bin::load(&set, d2_data::bin::DEFAULT_LANGUAGE)
            .map_err(|e| data(e.to_string()))?;
        Ok(Self {
            game: GameData::load(bins, &set).map_err(|e| data(e.to_string()))?,
            kind: DataKind::Live,
        })
    }

    fn label(&self) -> &'static str {
        match self.kind {
            DataKind::Synthetic => "synthetic",
            DataKind::Live => "live",
        }
    }
}

/// The seams without a provider (as `test_fixtures::game::Seams`), plus
/// the character's skill list (unit +0xA8), staged from `char skill`.
#[derive(Default)]
pub struct ScenarioSeams {
    pub sent: Vec<(UnitId, Vec<u8>)>,
    pub skills: BTreeMap<UnitId, Vec<SkillEntry>>,
}

impl Pending for ScenarioSeams {
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.sent.push((player, msg.to_vec()));
    }
    fn skill_list(&self, unit: UnitId) -> Vec<SkillEntry> {
        self.skills.get(&unit).cloned().unwrap_or_default()
    }
}

impl WorldPending for ScenarioSeams {}

impl Outbox for ScenarioSeams {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

/// The server game of a run.
pub type Sim = SimGame<WorldSim<ScenarioSeams>, ActionWorld>;

/// No session code (system messages are not scripted by the starters).
pub struct NoSession;

impl SessionHandler for NoSession {
    fn system_message(&mut self, _: ClientId, _: &[u8], _: usize, _: &mut dyn MessageSink) {}
}

struct NoRest;

impl VitalsRest for NoRest {
    fn refresh(&mut self, _: UnitId) {}
    fn level_up_notify(&mut self, _: UnitId) {}
    fn level_up_event(&mut self, _: UnitId) {}
}

/// A finished run.
#[derive(Debug)]
pub struct RunOutput {
    pub trace: TraceFile,
    /// Diagnostics not in the trace (unresolved references, transport
    /// refusals), in order.
    pub notes: Vec<String>,
}

/// The built game and what the run reads from it.
struct Built {
    server: DispatchServer<Sim, ProtoSizes, NoSession>,
    player: UnitId,
    waypoint_classes: BTreeSet<u32>,
    gaps: Vec<String>,
}

/// The game state references resolve against (`scenario.md` §3).
struct View<'a> {
    sim: &'a Sim,
    player: UnitId,
    waypoint_classes: &'a BTreeSet<u32>,
}

impl View<'_> {
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.sim.events.action.sys.hooks.path_position(unit)
    }

    /// Every unit of the unit lists: (type, GUID, id), by (type, GUID).
    fn all(&self) -> Vec<(UnitType, u32, UnitId)> {
        let lists = &self.sim.game.lists;
        let mut out: Vec<(UnitType, u32, UnitId)> = UnitType::ALL
            .iter()
            .flat_map(|&ty| {
                lists
                    .units_of_type(ty)
                    .into_iter()
                    .filter_map(move |id| lists.unit(id).map(|e| (ty, e.guid, id)))
            })
            .collect();
        out.sort_by_key(|&(ty, guid, _)| (ty.index(), guid));
        out
    }

    fn class(&self, id: UnitId) -> u32 {
        self.sim
            .events
            .action
            .sys
            .units
            .get(id)
            .map_or(0, |u| u.class)
    }
}

impl World for View<'_> {
    fn player(&self) -> Option<(u32, i32, i32)> {
        let guid = self.sim.game.lists.unit(self.player)?.guid;
        let (x, y) = self.position(self.player);
        Some((guid, x, y))
    }

    fn units(&self) -> Vec<UnitRef> {
        self.all()
            .into_iter()
            .map(|(ty, guid, id)| {
                let class = self.class(id);
                UnitRef {
                    ty: ty.index() as u8,
                    class,
                    guid,
                    waypoint: ty == UnitType::Object && self.waypoint_classes.contains(&class),
                }
            })
            .collect()
    }
}

fn build(s: &Scenario, data: &Data) -> Result<Built, RunError> {
    // The inline character; a save alone needs the loader.
    let Some(c) = &s.character else {
        return Err(RunError::Unsupported(format!(
            "char save {}: no save loader yet and no inline character, TODO(spec: formats/d2s.md)",
            s.save.as_deref().unwrap_or("")
        )));
    };
    if (c.act, c.area) != (0, ACT1_TOWN) {
        return Err(RunError::Unsupported(format!(
            "char area {} {}: the runner creates act 0 only and starts in its town (level {ACT1_TOWN})",
            c.act, c.area
        )));
    }
    let mut gaps = Vec::new();
    let d = &data.game;
    let b = |e: test_fixtures::game::GameError| RunError::Build(e.to_string());
    let build_err = |e: String| RunError::Build(e);

    // Game creation (`scenario.md` §4 rule 1): the act's DRLG on the
    // map seed, the action hooks on the game seed, the creation fields, then
    // the population regions (as `GameData::world_sim`, with the
    // scenario's difficulty and expansion set before the regions).
    // Seeds (`tools/original-hooks.md` §2 rule 1–2): the game seed starts
    // at {T, 666} and is stepped once (`0x0052C2C6`, the step that fed
    // the overridden +0x7C); the DRLG seed is the save's map ID when the
    // character has one, else the init value I.
    let mut game_seed = Seed::init_low(s.seed);
    game_seed.step();
    let drlg_seed = c.map.unwrap_or(s.init);
    let (drlg_data, types) = d.level_types();
    let creation = match data.kind {
        DataKind::Synthetic => ActCreation::TownOnly,
        DataKind::Live => ActCreation::Full,
    };
    let world = d
        .drlg_world_in(
            drlg_data,
            &types,
            creation,
            drlg_seed,
            s.difficulty.index(),
            c.area,
        )
        .map_err(b)?;
    let mut hooks = ActionHooks::new(
        Arc::new(d.action_tables().map_err(b)?),
        world,
        game_seed,
        ScenarioSeams::default(),
    );
    hooks.anim_data = Some(Arc::new(d.anim.clone()));
    hooks.vitals = Some(Arc::new(d.vitals().map_err(b)?));
    hooks
        .enable_paths()
        .map_err(|e| build_err(format!("paths: {e:?}")))?;
    let info = GameInfo {
        expansion: s.expansion,
        difficulty: s.difficulty.index(),
        ..GameInfo::default()
    };
    let state = WorldState::new(types.clone(), Arc::new(d.world_tables().map_err(b)?), info);
    let mut sim = WorldSim::new(
        Arc::new(d.stat_data().map_err(b)?),
        d.unit_data().map_err(b)?,
        hooks,
        state,
    );
    let mut fields = GameFields::new(game_seed, s.expansion);
    fields.difficulty = s.difficulty.index();
    // Single player: 0x67 byte +0x11 = 3 → game +0x6A (original-hooks
    // §5.2).
    fields.game_type = 3;
    ActionEvents::create_game(&mut sim, &fields);
    sim.create_regions();

    let wp_levels = d.waypoints().map_err(b)?;
    // The area generated and every room streamed.
    let mut game = Game::new();
    game.lists
        .ensure_act(0)
        .map_err(|e| build_err(format!("act 0: {e:?}")))?;
    let rooms = sim
        .action
        .hooks()
        .drlg
        .with_act(0, &mut game.lists, |dr, svc| {
            let lv = dr.get_or_alloc_level(svc.data, svc.types, c.area)?;
            if dr.level_rooms(lv).is_empty() {
                dr.generate_level(svc.data, svc.types, lv)?;
            }
            // The other act 0 levels the character holds a waypoint of
            // are generated too, so a travel there finds its rooms (the
            // synthetic act is created town only).
            for &w in &c.waypoints {
                if w != c.area && act_of_level(w) == 0 && wp_levels.map.index_of_level(w).is_some()
                {
                    let other = dr.get_or_alloc_level(svc.data, svc.types, w)?;
                    if dr.level_rooms(other).is_empty() {
                        dr.generate_level(svc.data, svc.types, other)?;
                    }
                }
            }
            let mut out = Vec::new();
            for r in dr.level_rooms(lv) {
                let active = dr.stream_room(svc, r)?;
                out.push((r, active, dr.room(r).rect));
            }
            Ok::<_, DrlgError>(out)
        })
        .ok_or_else(|| build_err("act 0 has no DRLG".into()))?
        .map_err(|e| build_err(format!("level {}: {e:?}", c.area)))?;

    // Waypoint objects from the room presets (room-relative sub-tiles +
    // the room's origin).
    let objects: Vec<Objects> = d.rows().map_err(b)?;
    let waypoint_classes: BTreeSet<u32> = objects
        .iter()
        .enumerate()
        .filter(|(_, o)| o.operatefn == WAYPOINT_OPERATE)
        .map(|(i, _)| i as u32)
        .collect();
    let mut wanted = Vec::new();
    {
        let t = types.borrow();
        let presets = t.act_presets(0);
        for &(r, active, rect) in &rooms {
            let (Some(active), Some(p)) = (active, presets) else {
                continue;
            };
            for u in p.room_units(r) {
                let class = u32::try_from(u.class).unwrap_or(u32::MAX);
                if u.unit_type == 2 && waypoint_classes.contains(&class) {
                    wanted.push((class, active, rect.x * SUB + u.x, rect.y * SUB + u.y));
                }
            }
        }
    }
    gaps.push("staging: only waypoint objects are allocated from the room presets".into());
    let mut alloc = |game: &mut Game, ty, class, room, (x, y): (i32, i32)| {
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
            .with(game, |g, v| v.allocate(g, &req, x, y))
            .ok_or_else(|| build_err(format!("allocating {ty:?} {class} at ({x}, {y}) failed")))
    };
    let mut placed = Vec::new();
    for &(class, room, x, y) in &wanted {
        placed.push((
            alloc(&mut game, UnitType::Object, class, room, (x, y))?,
            x,
            y,
        ));
    }

    // The character (`scenario.md` §4 rules 1, 3).
    let (room, start) = match c.at {
        Start::Default => {
            let &(_, room, x, y) = wanted.first().ok_or_else(|| {
                RunError::Unsupported(
                    "char at default: the area has no waypoint object to stage from; give `char at <x> <y>`"
                        .into(),
                )
            })?;
            gaps.push(
                "char at default: staged 5 sub-tiles right of and below the first waypoint (TODO(spec: unit placement, drlg/levels.md §10))"
                    .into(),
            );
            (room, (x + 5, y + 5))
        }
        Start::At(x, y) => {
            let (x, y) = (x as i32, y as i32);
            let room = rooms
                .iter()
                .find_map(|&(_, active, r)| {
                    let inside = x >= r.x * SUB
                        && x < (r.x + r.w) * SUB
                        && y >= r.y * SUB
                        && y < (r.y + r.h) * SUB;
                    active.filter(|_| inside)
                })
                .ok_or_else(|| {
                    RunError::Unsupported(format!(
                        "char at {x} {y}: in no room of level {}",
                        c.area
                    ))
                })?;
            (room, (x, y))
        }
    };
    let player = alloc(&mut game, UnitType::Player, u32::from(c.class), room, start)?;
    if let Some(u) = sim.action.sys.units.get_mut(player) {
        u.mode = 1;
    }
    let vitals = d.vitals().map_err(b)?;
    {
        let sys = &mut sim.action.sys;
        let mut v = VitalsView {
            units: &sys.units,
            stats: &mut sys.stats,
            hooks: &mut NoHost,
            rest: &mut NoRest,
        };
        init_player_stats(&mut v, &vitals, player, u32::from(c.act));
        // `char level` (stat 12) and `char stat` (layer 0), after
        // creation.
        v.stats
            .unit_set(&mut NoHost, player, 12, i32::from(c.level), 0);
        for &(stat, value) in &c.stats {
            v.stats.unit_set(&mut NoHost, player, stat, value, 0);
        }
    }
    if !c.skills.is_empty() {
        let list = c
            .skills
            .iter()
            .map(|&(skill, level)| SkillEntry {
                skill: i32::from(skill),
                base: i32::from(level),
                level_bonus: 0,
                owner_guid: -1,
                charges: 0,
                has_charges: false,
            })
            .collect();
        sim.action.hooks().x.skills.insert(player, list);
    }
    let waypoint_data = d.waypoints().map_err(b)?;
    for &level in &c.waypoints {
        match waypoint_data.map.index_of_level(level) {
            Some(index) => sim
                .action
                .hooks()
                .waypoints
                .entry(player)
                .or_default()
                .get_mut(s.difficulty.index())
                .set(u32::from(index))
                .map_err(|e| build_err(format!("waypoint {level}: {e:?}")))?,
            None => gaps.push(format!(
                "char waypoint {level}: no waypoint of that level in this data"
            )),
        }
    }
    if !c.quests.is_empty() {
        gaps.push(format!(
            "char quest: {} quest flag set(s) not applied (TODO(spec: quest flags of a joining character))",
            c.quests.len()
        ));
    }
    if !c.items.is_empty() {
        gaps.push(format!(
            "char item: {} item(s) not created (TODO(spec: item creation from a code and inventory placement))",
            c.items.len()
        ));
    }
    let errors = sim.errors();
    if !errors.is_empty() {
        return Err(build_err(format!("game creation: {errors:?}")));
    }

    // The server game: the waypoint world, the client joined in game
    // with no room (the first tick's client update runs the room switch,
    // `rooms.md` §4.1), the player's gate fields, the objects' facts.
    let world = ActionWorld {
        waypoints: Some(waypoint_data),
        ..ActionWorld::default()
    };
    let mut g: Sim = SimGame::with_world(game, sim, world);
    g.join(CLIENT, Some(player), None, client_state::IN_GAME)
        .map_err(|e| build_err(format!("join: {e}")))?;
    g.set_player(
        player,
        PlayerFields {
            gate: PlayerGate {
                mode: 1,
                uninterruptable: false,
            },
            data: Some(PlayerData { last_accept: 0 }),
        },
    );
    for (unit, x, y) in placed {
        g.set_unit(
            unit,
            UnitFacts {
                act: 0,
                pos: Pos { x, y },
                owner: None,
            },
        );
    }
    Ok(Built {
        server: DispatchServer::new(g, ProtoSizes, NoSession),
        player,
        waypoint_classes,
        gaps,
    })
}

/// A spawn step (`scenario.md` §3.1 rule 2) at (x, y); the GUID of the
/// unit the first call returned.
fn spawn(sim: &mut Sim, sp: &Spawn, x: i32, y: i32) -> Option<u32> {
    let game = &mut sim.game;
    let ev = &mut sim.events;
    // The active room that holds the point.
    let room = game.lists.active_rooms(0).into_iter().find(|&r| {
        ev.action
            .sys
            .hooks
            .drlg
            .subtiles(game, r)
            .is_some_and(|s| x >= s.x && x < s.x + s.w && y >= s.y && y < s.y + s.h)
    })?;
    let class = i32::try_from(sp.class).ok()?;
    let unit = match sp.kind {
        SpawnKind::Normal => ev.population(game, |cx| {
            placement::place_at(cx, room, None, x, y, class, 1, -1, 0).unit()
        })?,
        SpawnKind::RandomBoss => ev.population(game, |cx| {
            let b = pop_spawn::random_boss(cx, room, None, class, true, x, y, false)?;
            pop_spawn::champion_minions(cx, None, b, class);
            Some(b)
        })?,
        SpawnKind::Champion => {
            let b = ev.population(game, |cx| {
                pop_spawn::boss_spawn(cx, room, None, x, y, None, class, false)
            })?;
            let umod = sp.umods[0];
            ev.init(game, |cx, h| init::champion_pack_member(cx, h, b, umod));
            ev.population(game, |cx| pop_spawn::champion_minions(cx, None, b, class));
            b
        }
        SpawnKind::Unique => {
            let b = ev.population(game, |cx| {
                pop_spawn::boss_spawn(cx, room, None, x, y, None, class, false)
            })?;
            ev.init(game, |_, h| {
                for &u in &sp.umods {
                    h.monsters().entry(b).push_umod(u);
                }
            });
            ev.population(game, |cx| {
                pop_spawn::boss_minions_and_init(cx, b, 3, 6, None)
            });
            b
        }
    };
    game.lists.unit(unit).map(|e| e.guid)
}

/// The raw value of (stat, layer 0) in the unit's full stat array, 0
/// when absent or without a list (`tools/original-hooks.md` §4 rule 3:
/// not the unit-total reader).
fn full_value(stats: &d2_sim::stats::StatLists, unit: UnitId, stat: i32) -> i32 {
    stats
        .unit_list(unit)
        .map(|l| stats.full_entries(l))
        .and_then(|e| e.into_iter().find(|&(k, _)| k == stat << 16))
        .map_or(0, |(_, v)| v)
}

/// Every unit's facts from the sim: act and path position.
fn stage_facts(sim: &mut Sim, player: UnitId, waypoint_classes: &BTreeSet<u32>) {
    let view = View {
        sim,
        player,
        waypoint_classes,
    };
    let facts: Vec<(UnitId, UnitFacts)> = view
        .all()
        .into_iter()
        .map(|(_, _, id)| {
            let (x, y) = view.position(id);
            let act = sim.events.action.sys.units.get(id).map_or(0, |u| u.act);
            (
                id,
                UnitFacts {
                    act,
                    pos: Pos { x, y },
                    owner: None,
                },
            )
        })
        .collect();
    for (id, f) in facts {
        sim.set_unit(id, f);
    }
}

/// Runs `s` on `data` (`scenario.md` §4).
pub fn run(s: &Scenario, data: &Data) -> Result<RunOutput, RunError> {
    let Built {
        mut server,
        player,
        waypoint_classes,
        mut gaps,
    } = build(s, data)?;
    let mut notes = Vec::new();
    let mut streams: Vec<String> = vec!["c2s".into()];
    for st in &s.record {
        match st {
            Stream::Frames => gaps.push("record frames: the d2rs runner has no renderer".into()),
            st => streams.push(st.name().into()),
        }
    }
    streams.sort();
    let snapshots = s.snapshot_ticks();
    let mut queues = ServerQueues::new();
    let mut records = Vec::new();
    let mut steps = s.steps.iter().peekable();
    let mut faults_seen = BTreeSet::new();
    let seed = |server: &DispatchServer<Sim, ProtoSizes, NoSession>| {
        let g = server.game.events.action.sys.hooks.game_seed;
        [g.lo, g.hi]
    };
    for t in 0..=s.end {
        // (a) Resolve and inject the steps of tick t; spawns run here.
        let before = seed(&server);
        let mut i = 0;
        while let Some(step) = steps.next_if(|st| st.tick == t) {
            let view = View {
                sim: &server.game,
                player,
                waypoint_classes: &waypoint_classes,
            };
            if let StepMsg::Spawn(sp) = &step.msg {
                let guid = match spawn_position(sp, &view) {
                    Ok((x, y)) => {
                        let g = spawn(&mut server.game, sp, x, y);
                        if g.is_none() {
                            notes.push(format!(
                                "tick {t} step {i}: spawn of {} placed nothing",
                                sp.class
                            ));
                        }
                        Ok(g)
                    }
                    Err(u) => {
                        notes.push(format!("tick {t} step {i}: unresolved: {}", u.why));
                        Err(u.reference)
                    }
                };
                records.push(Record::Spawn { t, i, guid });
                i += 1;
                continue;
            }
            match encode(&step.msg, &view) {
                Ok(bytes) => {
                    match queues.send(&ProtoSizes, CLIENT, &bytes) {
                        Ok(Classified::Queued(_)) => {}
                        Ok(other) => notes.push(format!(
                            "tick {t} step {i}: the transport did not queue it: {other:?}"
                        )),
                        Err(e) => notes.push(format!("tick {t} step {i}: {e}")),
                    }
                    records.push(Record::C2s {
                        t,
                        i,
                        bytes: Ok(bytes),
                    });
                }
                Err(u) => {
                    notes.push(format!("tick {t} step {i}: unresolved: {}", u.why));
                    records.push(Record::C2s {
                        t,
                        i,
                        bytes: Err(u.reference),
                    });
                }
            }
            i += 1;
        }
        // The dispatcher's range checks read the staged unit facts
        // (`SimGame::set_unit`, `intents-events.md` §2.4 rules 3–4):
        // staged from the sim's own units before the drain, as
        // `d2-client/tests/e2e_walk.rs` does.
        if !queues.is_empty() {
            stage_facts(&mut server.game, player, &waypoint_classes);
        }
        // (b) Drain and dispatch; (c) the tick.
        let now = t.wrapping_mul(TICK_MS);
        for d in queues.drain() {
            match d.queue {
                Queue::System => server.system_message(d.client, &d.msg, d.size),
                Queue::Game => {
                    let code = server
                        .game_message(d.client, &d.msg, d.size, now)
                        .map_err(|message| RunError::Server { tick: t, message })?;
                    notes.push(match code {
                        Some(c) => {
                            format!("tick {t}: c2s 0x{:02x} dispatched, result {c}", d.msg[0])
                        }
                        None => format!("tick {t}: c2s 0x{:02x} dropped before dispatch", d.msg[0]),
                    });
                }
                Queue::Admin => notes.push(format!("tick {t}: admin message dropped (no realm)")),
            }
        }
        server.tick();
        let after = seed(&server);
        // (d) The records of tick t.
        let sent = server.take_sent();
        if s.records(Stream::S2c) {
            records.extend(
                sent.into_iter()
                    .filter(|(c, _)| *c == CLIENT)
                    .map(|(client, bytes)| Record::S2c { t, client, bytes }),
            );
        }
        if s.records(Stream::Rng) {
            records.push(Record::Rng { t, before, after });
        }
        if snapshots.contains(&t) {
            let view = View {
                sim: &server.game,
                player,
                waypoint_classes: &waypoint_classes,
            };
            let stats = &server.game.events.action.sys.stats;
            let all = view.all();
            if s.records(Stream::Units) {
                for &(ty, guid, id) in &all {
                    let (x, y) = view.position(id);
                    records.push(Record::Unit {
                        t,
                        ty: ty.index() as u32,
                        guid,
                        class: view.class(id),
                        mode: server
                            .game
                            .events
                            .action
                            .sys
                            .units
                            .get(id)
                            .map_or(0, |u| u.mode),
                        x,
                        y,
                        life: full_value(stats, id, 6),
                        mana: full_value(stats, id, 8),
                    });
                }
            }
            if s.records(Stream::Stats) {
                for &(ty, guid, id) in all.iter().filter(|(ty, ..)| *ty == UnitType::Player) {
                    let mut base: Vec<(u32, u32, i32)> = stats
                        .unit_list(id)
                        .map(|l| stats.base_entries(l))
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(key, value)| {
                            (((key as u32) >> 16) & 0xFFFF, (key as u32) & 0xFFFF, value)
                        })
                        .collect();
                    base.sort_unstable();
                    records.push(Record::Stats {
                        t,
                        ty: ty.index() as u32,
                        guid,
                        base,
                    });
                }
            }
        }
        // d2rs faults (missing wiring): gaps, once each.
        let g = &server.game;
        let faults: Vec<String> = g
            .tick_faults
            .iter()
            .map(|f| format!("{f:?}"))
            .chain(g.world.faults.iter().map(|f| format!("{f:?}")))
            .chain(g.events.errors())
            .collect();
        for f in faults {
            if faults_seen.insert(f.clone()) {
                gaps.push(format!("tick {t}: d2rs fault: {f}"));
            }
        }
    }
    records.push(Record::End { t: s.end });
    Ok(RunOutput {
        trace: TraceFile {
            header: Header {
                side: "d2rs".into(),
                tool: TOOL.into(),
                data: data.label().into(),
                scenario: s.name.clone(),
                scenario_sha256: s.sha256(),
                seed: s.seed,
                init: s.init,
                end: s.end,
                streams,
                gaps,
            },
            records,
        },
        notes,
    })
}
