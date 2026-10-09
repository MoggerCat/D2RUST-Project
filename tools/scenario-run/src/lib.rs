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
use d2_server::adapters::handlers::world::{
    preview_inv_parts, ActionEvents, ActionWorld, Outbox, WiredWorld,
};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use d2_server::seams::{ClientId, MessageSink, PlayerGate, Pos, SessionHandler};
use d2_server::transport::{Classified, Queue, ServerQueues};
use d2_server::world_data::tables::drop_tables;
use d2_sim::combat::vitals::init_player_stats;
use d2_sim::drlg::{act_of_level, DrlgError};
use d2_sim::game::Game;
use d2_sim::items::ItemTables;
use d2_sim::monsters::init::GameInfo;
use d2_sim::poke;
use d2_sim::rng::Seed;
use d2_sim::skills::SkillEntry;
use d2_sim::stats::lists::NoHost;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, Pending};
use d2_sim::wiring::economy::{DeathDrops, GameFields};
use d2_sim::wiring::interaction::{VitalsRest, VitalsView};
use d2_sim::wiring::worldgen::dispatch::WorldSim;
use d2_sim::wiring::worldgen::{WorldPending, WorldState};
use test_fixtures::game::{ActCreation, GameData};

pub mod rest;
pub use rest::ScenarioRest;

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
    /// `--save-dir`: the folder holding `char save <name>.d2s`. When the
    /// file is there the d2rs side loads it too (`d2-server` `load_save`),
    /// so both sides start from the same save; else the inline `char`
    /// lines stand for it.
    pub save_dir: Option<std::path::PathBuf>,
}

impl Data {
    /// The synthetic install (`test-fixtures`), built in `dir`.
    pub fn synthetic(dir: &Path) -> Result<Self, RunError> {
        let i = test_fixtures::install::build(dir, &test_fixtures::synth::synthetic())
            .map_err(|e| RunError::Data(e.to_string()))?;
        Ok(Self {
            game: GameData::from_install(&i).map_err(|e| RunError::Data(e.to_string()))?,
            kind: DataKind::Synthetic,
            save_dir: None,
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
            save_dir: None,
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
pub type Sim = SimGame<WorldSim<ScenarioSeams>, WiredWorld<ScenarioRest>>;

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
    /// The item tables `poke item` creates from (`None`: they did not
    /// load; the poke is then a gap).
    items: Result<ItemTables, String>,
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
    // The object control, the next creation seed (`rng.md` §5.2,
    // `objects.md` §2), so objects (presets, `poke object`) get their
    // init and the 0x13 object case reaches their operate; the chest
    // drop's state (`treasure.md` §4) on the action wiring, as the play
    // game holds it.
    sim.create_objects(Arc::new(d.object_tables().map_err(b)?));
    match drop_tables(&d.fixed) {
        Ok(t) => {
            sim.action.hooks().object_drops = Some(Box::new(DeathDrops::new(
                Arc::new(t),
                GameFields::new(Seed::init_low(0), false),
            )))
        }
        Err(e) => gaps.push(format!("drops: no drop tables ({e}): objects drop nothing")),
    }

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
    // The join step after the player's allocation (`units.md` §6.1), as
    // the play host's join (`d2_server::adapters::session`): neutral mode
    // start, the regeneration event 3 every frame (life, stamina, mana;
    // `stat-lists.md` §10.1) and the refresh event 11.
    sim.action
        .sys
        .with(&mut game, |s, hooks| {
            d2_sim::units::modes::player_join(s, hooks, player)
        })
        .map_err(|e| build_err(format!("player join: {e:?}")))?;
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
    let save_loaded = matches!((&data.save_dir, &s.save), (Some(d), Some(n)) if d.join(format!("{n}.d2s")).is_file());
    if !c.items.is_empty() && !save_loaded {
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
    // The wired host (inventory model, item loads, quest records) with the
    // scenario's no-op rest (`rest.rs`). Its quest and NPC controls draw on
    // a scratch copy of the game seed, so the game seed's draws stay as
    // they were before the host had them (TODO(spec: the controls'
    // creation draws in game-creation order)).
    let action = ActionWorld {
        waypoints: Some(waypoint_data),
        ..ActionWorld::default()
    };
    let mut scratch = sim.action.sys.hooks.game_seed;
    let quest_tables = d2_sim::world::quests::QuestTables::load()
        .map_err(|e| build_err(format!("quest tables: {e:?}")))?;
    let quests = d2_sim::world::quests::QuestControl::new(&quest_tables, &mut scratch)
        .map_err(|e| build_err(format!("quest control: {e:?}")))?;
    let npc = d2_sim::world::npc::NpcControl::new(&[], Vec::new(), s.expansion, 0, &mut scratch)
        .map_err(|e| build_err(format!("npc control: {e:?}")))?;
    let items = ItemTables::from_fixed(&d.fixed).map_err(|e| build_err(e.to_string()))?;
    let inv = d2_sim::items::inventory::InvTables::from_fixed(&d.fixed)
        .map_err(|e| build_err(e.to_string()))?;
    let mut world = WiredWorld::new(
        action,
        items,
        quests,
        npc,
        d2_sim::world::vendors::VendorTables::default(),
        ScenarioRest {
            expansion: s.expansion,
            ..ScenarioRest::default()
        },
        0,
    );
    world.inventory = Some(preview_inv_parts(inv));
    let mut g: Sim = SimGame::with_world(game, sim, world);
    // Ground items are announced by the update pass as the play app's
    // host does (`inventory-moves.md` §6.3, §9.1 step 4): a drop from the
    // cursor sends 0x9C action 2 (recorded 2026-10-09,
    // `facts/items/a1-town-item-moves.tsv` n 57–58).
    g.announce_ground = true;
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
    // `char save` with `--save-dir`: the save's own load on the action
    // wiring (character stats, skills, items), as the original side.
    if let (Some(dir), Some(name)) = (&data.save_dir, &s.save) {
        let path = dir.join(format!("{name}.d2s"));
        if path.is_file() {
            load_char_save(&mut g, player, &path, s, &data.game, &mut gaps)?;
        } else {
            gaps.push(format!("char save {name}: {} not found", path.display()));
        }
    }
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
        items: ItemTables::from_fixed(&d.fixed).map_err(|e| e.to_string()),
        gaps,
    })
}

/// Loads `path` (`formats/d2s.md`, `d2s::read` with the scenario's game)
/// onto `player` through `d2_server::adapters::session::load_save`; each
/// step the scenario host has no provider for is a gap.
fn load_char_save(
    g: &mut Sim,
    player: UnitId,
    path: &Path,
    s: &Scenario,
    game: &GameData,
    gaps: &mut Vec<String>,
) -> Result<(), RunError> {
    use d2_formats::d2s;
    let bad = |e: String| RunError::Data(format!("{}: {e}", path.display()));
    let bytes = std::fs::read(path).map_err(|e| bad(e.to_string()))?;
    let expansion = s.expansion;
    let tables = d2_server::world_data::tables::SaveData::from_fixed(&game.fixed, expansion)
        .map_err(|e| bad(e.to_string()))?;
    let name: Vec<u8> = bytes
        .get(0x14..0x24)
        .map(|n| n.iter().copied().take_while(|&c| c != 0).collect())
        .unwrap_or_default();
    let opts = d2s::ReadOptions {
        expansion,
        game: Some(d2s::GameContext {
            client_name: name,
            expansion,
            hardcore: bytes.get(0x24).is_some_and(|b| b & 0x04 != 0),
            difficulty: s.difficulty.index(),
        }),
    };
    let save = d2s::read(&bytes, &opts, &tables).map_err(|e| bad(format!("{e:?}")))?;
    let ctx = d2_server::adapters::character::LoadContext {
        difficulty: s.difficulty.index(),
        map_seed_applies: false,
    };
    let (_, report) = d2_server::adapters::session::load_save(g, player, &save, &ctx)
        .map_err(|e| bad(format!("load: {e:?}")))?;
    // The save's items on the wired host (`d2s.md` §8.2), as the play
    // app's join does (`WiredWorld::load_items`).
    let items_ok = match &save.body {
        Some(body) if !body.items.is_empty() => {
            let loaded = g
                .world
                .load_items(&mut g.game, &mut g.events, player, &body.items);
            for f in &loaded.faults {
                gaps.push(format!("char save: item load: {f}"));
            }
            loaded.faults.is_empty()
        }
        _ => true,
    };
    // Load §2 quests (`world/quests.md` §1.6) into the player's quest
    // record, and `d2s.md` §6 rule 1's NPC fields.
    let mut quests = d2_sim::world::quests::PlayerQuests::default();
    if let Some(body) = &save.body {
        for (d, rec) in body.quests.records.iter().enumerate() {
            match d2_sim::world::quests::QuestFlags::copy_in(rec, true) {
                Ok(f) => quests.flags[d] = f,
                Err(e) => gaps.push(format!("char save: quests {d}: {e}")),
            }
        }
        quests.first_talk = body.npcs.a;
        for (d, &b) in body.npcs.b.iter().enumerate() {
            quests.set_intro_bits(d, b);
        }
    }
    g.world.rest.quests.insert(player, quests);
    for u in report.unapplied {
        let applied =
            (items_ok && u.step == "items") || u.step == "quests" || u.step == "npc fields";
        if !applied {
            gaps.push(format!(
                "char save: load step {:?} not applied (no provider in the scenario host)",
                u.step
            ));
        }
    }
    Ok(())
}

/// A spawn step (`scenario.md` §3.1 rule 2) at (x, y) in act 0; the
/// GUID of the unit the first call returned (`d2_sim::poke::spawn_monster`).
fn spawn(sim: &mut Sim, sp: &Spawn, x: i32, y: i32) -> Option<u32> {
    let kind = match sp.kind {
        SpawnKind::Normal => poke::SpawnKind::Normal,
        SpawnKind::RandomBoss => poke::SpawnKind::RandomBoss,
        SpawnKind::Champion => poke::SpawnKind::Champion,
        SpawnKind::Unique => poke::SpawnKind::Unique,
    };
    let unit = poke::spawn_monster(
        &mut sim.game,
        &mut sim.events,
        0,
        sp.class,
        x,
        y,
        kind,
        &sp.umods,
    )?;
    sim.game.lists.unit(unit).map(|e| e.guid)
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
        items,
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
        // (a) Resolve and inject the steps of tick t; spawns and pokes
        // run here, in script order (`poke.md` §3 rule 1).
        let before = seed(&server);
        let mut i = 0;
        let mut step_records = Vec::new();
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
                step_records.push(Record::Spawn { t, i, guid });
                i += 1;
                continue;
            }
            if let StepMsg::Poke(d) = &step.msg {
                let env = poke::Env {
                    player,
                    waypoint_classes: &waypoint_classes,
                    items: items.as_ref().ok(),
                };
                let g = &mut server.game;
                let r = poke::apply(&mut g.game, &mut g.events, &env, d);
                match &r {
                    poke::PokeResult::Ok(_) => {}
                    poke::PokeResult::Failed => {
                        notes.push(format!("tick {t} step {i}: poke {d}: failed"))
                    }
                    poke::PokeResult::FailedWith(why) => {
                        notes.push(format!("tick {t} step {i}: poke {d}: failed: {why}"))
                    }
                    poke::PokeResult::Unresolved(u) => {
                        notes.push(format!("tick {t} step {i}: poke {d}: unresolved {u}"))
                    }
                    poke::PokeResult::Pending => {
                        // a scenario step runs once: a `goto` walk needs
                        // a runner that steps it every tick (poke.md §6)
                        notes.push(format!(
                            "tick {t} step {i}: poke {d}: gap: goto walks in state-dump and play only"
                        ));
                        gaps.push(format!("poke {} at {t}", d.keyword()));
                    }
                    poke::PokeResult::Gap(why) => {
                        let why = match (d, &items) {
                            (poke::Directive::Item { .. }, Err(e)) => {
                                format!("{why} (item tables: {e})")
                            }
                            _ => why.clone(),
                        };
                        notes.push(format!("tick {t} step {i}: poke {d}: gap: {why}"));
                        gaps.push(format!("poke {} at {t}", d.keyword()));
                    }
                }
                step_records.push(Record::Poke {
                    t,
                    i,
                    d: d.keyword().into(),
                    r: r.code().into(),
                    guid: r.guid(),
                });
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
                    step_records.push(Record::C2s {
                        t,
                        i,
                        bytes: Ok(bytes),
                    });
                }
                Err(u) => {
                    notes.push(format!("tick {t} step {i}: unresolved: {}", u.why));
                    step_records.push(Record::C2s {
                        t,
                        i,
                        bytes: Err(u.reference),
                    });
                }
            }
            i += 1;
        }
        // Within a tick the records go `c2s`, `spawn`, `poke`
        // (FORMAT.md; `scenario.md` §5 rule 3), each in step order.
        step_records.sort_by_key(Record::order);
        records.extend(step_records);
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
            .chain(g.world.action.faults.iter().map(|f| format!("{f:?}")))
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
