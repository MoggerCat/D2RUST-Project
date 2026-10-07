// Spec: specs/client/bridge.md (§3), specs/world/waypoints.md (§5.1, §6)
//! The single-player game of the app: `d2-server`'s [`SimGame`] on the
//! wired `d2-sim` ([`ActionSim`] with the waypoint world [`ActionWorld`]),
//! behind the in-process host ([`LocalLink`]), started on its own thread
//! ([`ThreadLink`], see there why).
//!
//! What runs is what the wiring has providers for (`docs/HANDOFF.md` §1
//! rows 3j, 3k): the tick driver, the timer queue, and the intents whose
//! handler runs on a real provider (0x49 waypoints). Two acts are created
//! and one room is streamed in Cold Plains (act 0) and Lut Gholein (act 1).
//!
//! [`GameData::Live`] (with `D2_GAME_DIR`, [`LiveData::load`]) takes
//! everything from the user's own files: the `levels` and `objects` tables
//! (`d2_data::bin::load`; the waypoint object is the first `objects` row
//! with operate function 23 and init function 17, `waypoints.md` §5.1
//! rule 1), and the level generation data of `d2_server::world_data`
//! (drlg-data: the level-type table views, every lvlprest / lvlsub DS1
//! and lvltypes DT1, parsed), so the acts are generated through
//! `d2_sim::wiring::worldgen::levels::WorldTypes` (the Maze / Presets /
//! Outdoor dispatcher) with the server's town level ids (`levels.md` §2
//! step 2: 1, 40). [`GameData::Synthetic`] (no game files) uses the
//! bridge test's rows and its synthetic two-act DRLG
//! (`bridge/local_tests.rs`: one 8×8-tile floor room per level).
//!
//! Seams without a provider are [`LocalSeams`]: the narrowest answers
//! (`Pending`'s defaults) plus a store of what the sim itself sets
//! (positions, the interaction target) and the transport outbox. Nothing
//! here decides an outcome: it stages the game the way the server tests
//! do (a sorceress who knows her act's first waypoint, `bridge.md` §3).

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_data::tables::{decode_all, Levels, Objects, Record};
use d2_formats::mpq::ArchiveSet;
use d2_server::adapters::handlers::world::{ActionWorld, Outbox};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame};
use d2_server::host::Host;
use d2_server::host::SystemClock;
use d2_server::seams::{Clock, PlayerGate};
use d2_server::world_data::tables::LevelTables;
use d2_server::world_data::{archive as world_archive, Dt1Files, WorldFiles};
use d2_sim::combat::CombatTables;
use d2_sim::drlg::maze::Maze;
use d2_sim::drlg::room::LinkAt;
use d2_sim::drlg::{
    CellGrid, Drlg, DrlgData, DrlgError, DrlgRoomId, Dungeon, GridPass, LevelDef, LevelIdx,
    LevelTypes, RoomGrids, RoomKind, TileInfo, TileRect, TileSource,
};
use d2_sim::game::Game;
use d2_sim::rng::Seed;
use d2_sim::skills::SkillTables;
use d2_sim::stats::StatData;
use d2_sim::units::hooks::UnitData;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::{RoomId, UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, ActionSim, ActionTables, DrlgWorld, Pending};
use d2_sim::wiring::worldgen::levels::{SharedTypes, WorldTypes};
use d2_sim::world::waypoints::{WaypointData, NO_WAYPOINT};

use d2_sim::drlg::outdoor::{SubFile, SubFiles};
use d2_sim::drlg::preset::{Ds1Input, Ds1Source};

use super::server_thread::{ThreadLink, ThreadStopped};
use crate::bridge::drlg::DrlgSource;
use crate::bridge::local::{LocalLink, PendingSession};
use crate::bridge::world::LevelRow;
use crate::bridge::LOCAL_CLIENT;

/// The game's dispatch and world host.
pub type Sim = SimGame<ActionSim<LocalSeams>, ActionWorld>;

/// The local link over [`Sim`] with clock `C`.
pub type Link<C = SystemClock> = LocalLink<Sim, ProtoSizes, PendingSession, C>;

/// Cold Plains (act 0) and Lut Gholein (act 1): the two generated levels.
pub const COLD_PLAINS: u32 = 3;
pub const ACT2_TOWN: u32 = 40;
/// The default game seed.
pub const DEFAULT_SEED: u32 = 1234;
/// Sub-tile x of the waypoint object and of the player, and their y,
/// from the origin of the first streamed Cold Plains room.
pub const WAYPOINT_X: i32 = 20;
pub const PLAYER_X: i32 = 42;
pub const UNIT_Y: i32 = 20;
/// The player's character class (1, sorceress, as in the server tests).
pub const PLAYER_CLASS: u32 = 1;

/// Errors building the game.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("DRLG: {0:?}")]
    Drlg(DrlgError),
    #[error("{0}")]
    Setup(String),
    #[error("game tables: {0}")]
    Tables(String),
    #[error("level data: {0}")]
    World(#[from] d2_server::world_data::WorldDataError),
    #[error("archives in {dir}: {message}")]
    Archives { dir: String, message: String },
    #[error("no objects row has operate function 23 and init function 17")]
    NoWaypointObject,
    #[error(transparent)]
    Thread(#[from] ThreadStopped),
}

/// The action wiring's seams without a provider. Positions and the
/// interaction target are stored as the sim sets them; messages the sim
/// sends wait in `sent` for the world handlers ([`Outbox`]); warp and
/// arrival mode, whose bodies are unwritten specs, are logged.
#[derive(Debug, Default)]
pub struct LocalSeams {
    pub pos: BTreeMap<UnitId, (i32, i32)>,
    pub interact: BTreeMap<UnitId, (u8, u32)>,
    pub sent: Vec<(UnitId, Vec<u8>)>,
    pub log: Vec<String>,
}

impl Pending for LocalSeams {
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.pos.get(&unit).copied().unwrap_or_default()
    }
    fn place(&mut self, unit: UnitId, x: i32, y: i32) {
        self.pos.insert(unit, (x, y));
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
}

impl Outbox for LocalSeams {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

/// One 8×8-tile floor room per listed level.
struct Types(BTreeMap<u32, TileRect>);

impl LevelTypes for Types {
    fn generate(
        &mut self,
        drlg: &mut Drlg,
        _: &DrlgData,
        level: LevelIdx,
    ) -> Result<(), DrlgError> {
        let id = drlg.level(level).id;
        if let Some(&rect) = self.0.get(&id) {
            let r = drlg.alloc_room(level, RoomKind::Preset, rect);
            drlg.room_mut(r).dt1_mask = 1;
            drlg.link_room(r, LinkAt::Tail);
        }
        Ok(())
    }
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
                g.set(x, y, d2_sim::drlg::tiles::cell::FLOOR);
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
    }
}

/// The synthetic tile library: one floor tile, the fixed library's
/// blank and tile-10 entries (in drlg-data's DT1 provider type).
fn tiles() -> Dt1Files {
    use d2_sim::drlg::tiles::FIXED_LIBRARY;
    let mut t = BTreeMap::new();
    t.insert(b"floor.dt1".to_vec(), vec![tile(0, 0, 0, 1)]);
    let blank = |sub| {
        let mut x = tile(0, 30, sub, 0);
        x.subtile_flags = [0x20; 25];
        x
    };
    t.insert(FIXED_LIBRARY[0].to_vec(), vec![blank(0), blank(1)]);
    t.insert(FIXED_LIBRARY[1].to_vec(), vec![]);
    t.insert(FIXED_LIBRARY[2].to_vec(), vec![tile(10, 0, 0, 0)]);
    Dt1Files(t)
}

fn blank<T: Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

/// The tables the game's waypoint world reads, and the waypoint object's
/// class.
#[derive(Debug, Clone)]
pub struct WaypointTables {
    pub levels: Vec<Levels>,
    pub objects: Vec<Objects>,
    pub object_class: u32,
}

impl WaypointTables {
    /// The bridge test's rows: 150 levels (act 1 from level 40),
    /// waypoints 0 / 1 / 9 at levels 1 / Cold Plains / Lut Gholein, one
    /// waypoint object (class 0).
    pub fn synthetic() -> Self {
        let mut levels = vec![blank::<Levels>(); 150];
        for (i, l) in levels.iter_mut().enumerate() {
            l.waypoint = NO_WAYPOINT;
            l.act = if i >= 40 { 1 } else { 0 };
        }
        levels[1].waypoint = 0;
        levels[COLD_PLAINS as usize].waypoint = 1;
        levels[ACT2_TOWN as usize].waypoint = 9;
        let mut o: Objects = blank();
        o.operatefn = 23;
        o.initfn = 17;
        o.framecnt1 = 15 << 8;
        WaypointTables {
            levels,
            objects: vec![o],
            object_class: 0,
        }
    }

    /// The user's own `levels` and `objects` tables (`loading.md`: the
    /// live `.bin` set, validated).
    pub fn live(archives: &ArchiveSet) -> Result<Self, BuildError> {
        let set =
            d2_data::bin::load(archives, "eng").map_err(|e| BuildError::Tables(e.to_string()))?;
        let table = |name: &str| {
            set.table(name)
                .ok_or_else(|| BuildError::Tables(format!("{name} not loaded")))
        };
        let levels: Vec<Levels> =
            decode_all(table("levels")?).map_err(|e| BuildError::Tables(e.to_string()))?;
        let objects: Vec<Objects> =
            decode_all(table("objects")?).map_err(|e| BuildError::Tables(e.to_string()))?;
        // `waypoints.md` §5.1 rule 1.
        let object_class = objects
            .iter()
            .position(|o| o.operatefn == 23 && o.initfn == 17)
            .ok_or(BuildError::NoWaypointObject)? as u32;
        Ok(WaypointTables {
            levels,
            objects,
            object_class,
        })
    }

    /// The waypoint index of `level`, if it has one.
    fn waypoint(&self, level: u32) -> Option<u8> {
        let wp = self.levels.get(level as usize)?.waypoint;
        (wp != NO_WAYPOINT).then_some(wp)
    }
}

/// Everything the game reads from the user's files, loaded up front.
#[derive(Debug)]
pub struct LiveData {
    pub waypoints: WaypointTables,
    /// drlg-data's level-type table views (`LevelTables::from_fixed`).
    pub levels: LevelTables,
    /// Every DS1 / DT1 the level types read, parsed.
    pub files: WorldFiles,
    /// The archive set itself (the client's other readers: sounds).
    pub archives: Arc<ArchiveSet>,
}

impl LiveData {
    /// Loads the waypoint tables and the level data (`world_data::archive::
    /// load`: a named file that is missing or does not parse is an error;
    /// nothing falls back to synthetic data).
    pub fn load(archives: Arc<ArchiveSet>) -> Result<Self, BuildError> {
        let waypoints = WaypointTables::live(&archives)?;
        let (levels, files) = world_archive::load(&archives)?;
        Ok(LiveData {
            waypoints,
            levels,
            files,
            archives,
        })
    }
}

/// Where the game's tables and levels come from.
#[derive(Debug, Clone)]
pub enum GameData {
    Synthetic,
    /// Tables and level data already loaded from the user's files.
    Live(Arc<LiveData>),
}

impl GameData {
    /// The data of `game_dir` (`$D2_GAME_DIR`): live when a directory is
    /// given and `synthetic` is not asked, else synthetic. A given
    /// directory that does not load is an error, not a fallback.
    pub fn select(game_dir: Option<&std::path::Path>, synthetic: bool) -> Result<Self, BuildError> {
        match game_dir {
            Some(dir) if !synthetic => {
                let archives = ArchiveSet::open_dir(dir).map_err(|e| BuildError::Archives {
                    dir: dir.display().to_string(),
                    message: e.to_string(),
                })?;
                Ok(GameData::Live(Arc::new(LiveData::load(Arc::new(
                    archives,
                ))?)))
            }
            _ => Ok(GameData::Synthetic),
        }
    }

    fn tables(&self) -> WaypointTables {
        match self {
            GameData::Synthetic => WaypointTables::synthetic(),
            GameData::Live(d) => d.waypoints.clone(),
        }
    }
}

/// The level generation the game runs on: table view, tile library, level
/// types, and per act the DRLG init seed and the server's town level id.
struct LevelSource {
    data: Arc<DrlgData>,
    tiles: Box<dyn TileSource>,
    types: Box<dyn LevelTypes>,
    /// (act, init seed, town level id) of each created act.
    acts: [(u8, u32, u32); 2],
}

impl LevelSource {
    /// The bridge test's synthetic DRLG: one 8×8-tile floor room per
    /// level, no town generated at act creation, init seeds 1 and 2.
    fn synthetic() -> Self {
        LevelSource {
            data: Arc::new(synthetic_drlg_data()),
            tiles: Box::new(tiles()),
            types: Box::new(synthetic_types()),
            acts: [(0, 1, 0), (1, 2, 0)],
        }
    }

    /// The user's level data through the level-type dispatcher
    /// (`WorldTypes`, as drlg-data's game-file tests build it). Acts get
    /// the server's town level ids 1 and 40 (`levels.md` §2 step 2) and
    /// the game's init seed (game +0x7C, the same for every act). TODO
    /// (spec: game creation): what sets game +0x7C is not specified; the
    /// app passes its `--seed`.
    fn live(d: &LiveData, init_seed: u32) -> Self {
        let data = Arc::new(d.levels.drlg.clone());
        let types = SharedTypes::new(WorldTypes::new(
            data.clone(),
            Maze::new(d.levels.maze.clone()),
            d.levels.preset.clone(),
            d.levels.outdoor.clone(),
            Box::new(d.files.ds1.clone()),
            Box::new(d.files.subs.clone()),
        ));
        LevelSource {
            data,
            tiles: Box::new(d.files.dt1.clone()),
            types: Box::new(types),
            acts: [(0, init_seed, 1), (1, init_seed, ACT2_TOWN)],
        }
    }
}

/// The synthetic DRLG table view: 150 levels without warps; Cold Plains
/// and Lut Gholein are preset levels of tile library 1 (`floor.dt1`).
fn synthetic_drlg_data() -> DrlgData {
    let mut drlg = DrlgData {
        levels: vec![LevelDef::default(); 150],
        ..DrlgData::default()
    };
    for l in &mut drlg.levels {
        l.warp = [-1; 8];
    }
    let mut files = vec![Vec::new(); 32];
    files[0] = b"floor.dt1".to_vec();
    drlg.lvltypes = vec![vec![Vec::new(); 32], files];
    for id in [COLD_PLAINS, ACT2_TOWN] {
        drlg.levels[id as usize].drlg_type = 2;
        drlg.levels[id as usize].level_type = 1;
    }
    drlg
}

/// The synthetic level types: one 8×8-tile floor room in Cold Plains and
/// one in Lut Gholein.
fn synthetic_types() -> Types {
    Types(BTreeMap::from([
        (COLD_PLAINS, TileRect::new(0, 0, 8, 8)),
        (ACT2_TOWN, TileRect::new(0, 0, 8, 8)),
    ]))
}

/// The live DS1 files of the client DRLG's level types, shared with the
/// game's data (no copy per act build).
struct LiveDs1(Arc<LiveData>);

impl Ds1Source for LiveDs1 {
    fn ds1(&self, path: &[u8]) -> Option<&Ds1Input> {
        self.0.files.ds1.ds1(path)
    }
}

/// The live lvlsub files of the client DRLG's level types.
struct LiveSubs(Arc<LiveData>);

impl SubFiles for LiveSubs {
    fn sub_file(&self, file: &[u8]) -> Option<&SubFile> {
        self.0.files.subs.sub_file(file)
    }
}

/// What the client DRLG copy is built from (`client/model.md` §12 rule
/// 1): the same table view, tile headers and level-type data the game's
/// DRLG reads, with fresh level-type state for every client act (the
/// client never reads the server's DRLG).
pub fn client_drlg_source(data: &GameData) -> DrlgSource {
    match data {
        GameData::Synthetic => DrlgSource {
            data: Arc::new(synthetic_drlg_data()),
            tiles: Arc::new(tiles()),
            types: Arc::new(|| Box::new(synthetic_types())),
        },
        GameData::Live(d) => {
            let live = d.clone();
            let drlg = Arc::new(d.levels.drlg.clone());
            let types_data = drlg.clone();
            DrlgSource {
                data: drlg,
                tiles: Arc::new(d.files.dt1.clone()),
                types: Arc::new(move || {
                    Box::new(WorldTypes::new(
                        types_data.clone(),
                        Maze::new(live.levels.maze.clone()),
                        live.levels.preset.clone(),
                        live.levels.outdoor.clone(),
                        Box::new(LiveDs1(live.clone())),
                        Box::new(LiveSubs(live.clone())),
                    ))
                }),
            }
        }
    }
}

/// The `Levels.txt` fields the client reads (`client/model.md` §11
/// rules 3–4: `Act`, `BlankScreen`; `audio/environment.md` §1 r2:
/// `SoundEnv`), one row per level id, from the game's `levels` table.
pub fn client_level_rows(data: &GameData) -> Vec<LevelRow> {
    data.tables()
        .levels
        .iter()
        .map(|l| LevelRow {
            act: l.act,
            blank_screen: l.blankscreen != 0,
            sound_env: l.soundenv,
        })
        .collect()
}

/// A built game and the units the app and tests address.
pub struct LocalGame {
    pub sim: Sim,
    pub player: UnitId,
    pub waypoint: UnitId,
    /// The waypoint object's GUID.
    pub waypoint_guid: u32,
}

/// Builds the game on `seed`: the DRLG of both acts, one room streamed
/// in each generated level, the waypoint object and the local player in
/// Cold Plains, the player's record joined as [`LOCAL_CLIENT`] (as
/// `tests/e2e_single_player.rs` joins it).
pub fn build(data: &GameData, seed: u32) -> Result<LocalGame, BuildError> {
    let wp_tables = data.tables();
    let mut levels = match data {
        GameData::Synthetic => LevelSource::synthetic(),
        GameData::Live(d) => LevelSource::live(d, seed),
    };
    let mut dungeon = Dungeon::default();
    for (act, init_seed, town) in levels.acts {
        dungeon.acts[usize::from(act)] = Some(
            Drlg::create(
                act,
                init_seed,
                0,
                town,
                false,
                &levels.data,
                levels.types.as_mut(),
            )
            .map_err(BuildError::Drlg)?,
        );
    }
    let world = DrlgWorld {
        dungeon,
        data: levels.data,
        tiles: levels.tiles,
        types: levels.types,
    };
    let tables = ActionTables {
        missiles: Vec::new(),
        skills: SkillTables {
            skills: Vec::new(),
            skilldesc: Vec::new(),
            missiles: Vec::new(),
            skills_code: Vec::new(),
            miss_code: Vec::new(),
            level_cap: 0,
            stat_count: 0,
        },
        combat: CombatTables {
            charstats: Vec::new(),
            difficultylevels: Vec::new(),
            monstats: Vec::new(),
            monstats2: Vec::new(),
            hitclass: Vec::new(),
        },
        levels: Vec::new(),
        skill_modes: Vec::new(),
    };
    let hooks = ActionHooks::new(
        Arc::new(tables),
        world,
        Seed::init_low(seed),
        LocalSeams::default(),
    );
    let mut sim = ActionSim::new(Arc::new(StatData::default()), UnitData::default(), hooks);
    let mut game = Game::new();
    let mut rooms = Vec::new();
    for (act, level) in [(0u8, COLD_PLAINS), (1, ACT2_TOWN)] {
        game.lists
            .ensure_act(act)
            .map_err(|e| BuildError::Setup(format!("act {act}: {e:?}")))?;
        let r = sim
            .hooks()
            .drlg
            .with_act(act, &mut game.lists, |d, svc| {
                let l = d.get_or_alloc_level(svc.data, svc.types, level)?;
                // A town is generated at act creation (`levels.md` §3 step
                // 8); any other level here once.
                if d.level_rooms(l).is_empty() {
                    d.generate_level(svc.data, svc.types, l)?;
                }
                let Some(&r) = d.level_rooms(l).first() else {
                    return Ok(None);
                };
                let rect = d.room(r).rect;
                Ok(d.stream_room(svc, r)?.map(|id| (id, rect)))
            })
            .ok_or_else(|| BuildError::Setup(format!("act {act} has no DRLG")))?
            .map_err(BuildError::Drlg)?
            .ok_or_else(|| BuildError::Setup(format!("level {level}: no room streamed")))?;
        rooms.push(r);
    }
    // Staging, as the server tests do: the units stand in the first
    // streamed room of Cold Plains, at fixed sub-tile offsets from its
    // origin (subtile = tile × 5, `levels.md` §1). The original places a
    // joining player in a spawn room (`levels.md` §10) at a position no
    // spec states yet: not wired, TODO(spec: unit placement).
    let (room0, rect0) = rooms[0];
    let (ox, oy) = (rect0.x * 5, rect0.y * 5);
    let mut spawn = |ty: UnitType, class: u32, room: RoomId, x: i32, y: i32| {
        let req = AllocRequest {
            ty,
            class,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: ty == UnitType::Player,
        };
        sim.with(&mut game, |g, v| v.allocate(g, &req, x, y))
            .ok_or_else(|| BuildError::Setup(format!("allocating {ty:?} {class} failed")))
    };
    let waypoint = spawn(
        UnitType::Object,
        wp_tables.object_class,
        room0,
        ox + WAYPOINT_X,
        oy + UNIT_Y,
    )?;
    let player = spawn(
        UnitType::Player,
        PLAYER_CLASS,
        room0,
        ox + PLAYER_X,
        oy + UNIT_Y,
    )?;
    if let Some(u) = sim.sys.units.get_mut(player) {
        u.mode = 1;
    }
    // The player knows Cold Plains' waypoint (Normal difficulty).
    if let Some(index) = wp_tables.waypoint(COLD_PLAINS) {
        sim.hooks()
            .waypoints
            .entry(player)
            .or_default()
            .get_mut(0)
            .set(u32::from(index))
            .map_err(|e| BuildError::Setup(format!("waypoint {index}: {e:?}")))?;
    }
    let waypoint_guid = game
        .lists
        .unit(waypoint)
        .ok_or_else(|| BuildError::Setup("waypoint unit missing".into()))?
        .guid;
    let mut s: Sim = SimGame::with_events(game, sim);
    s.world.waypoints = Some(WaypointData::new(&wp_tables.levels, &wp_tables.objects));
    // No room yet: the first tick's client update sees the player's room
    // differ and runs the room switch (`rooms.md` §4.1), which registers
    // the client with the DRLG. A client joined with its room already set
    // skips it, and the room inactivity of tick step 9 then frees the
    // player's room under the player (`rooms.md` §7.2, §8).
    s.join(LOCAL_CLIENT, Some(player), None, client_state::IN_GAME)
        .map_err(|e| BuildError::Setup(e.to_string()))?;
    s.set_player(
        player,
        PlayerFields {
            gate: PlayerGate {
                mode: 1,
                uninterruptable: false,
            },
            data: Some(PlayerData { last_accept: 0 }),
        },
    );
    Ok(LocalGame {
        sim: s,
        player,
        waypoint,
        waypoint_guid,
    })
}

/// The units of a started game, for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Started {
    pub player: UnitId,
    pub waypoint: UnitId,
    pub waypoint_guid: u32,
}

/// Builds the game on a new server thread behind a local host with
/// `clock` (`SystemClock` in the app; a manual clock in tests).
pub fn start<C: Clock + Send + 'static>(
    data: GameData,
    seed: u32,
    clock: C,
) -> Result<(ThreadLink<Link<C>>, Started), BuildError> {
    let (tx, rx) = std::sync::mpsc::channel();
    let link = ThreadLink::spawn(move || {
        let g = build(&data, seed)?;
        let _ = tx.send(Started {
            player: g.player,
            waypoint: g.waypoint,
            waypoint_guid: g.waypoint_guid,
        });
        Ok::<_, BuildError>(LocalLink::new(Host::new(
            g.sim,
            ProtoSizes,
            PendingSession::default(),
            clock,
        )))
    })?;
    let started = rx
        .recv()
        .map_err(|_| BuildError::Setup("game not started".into()))?;
    Ok((link, started))
}
