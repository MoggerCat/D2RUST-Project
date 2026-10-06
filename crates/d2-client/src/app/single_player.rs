// Spec: specs/client/bridge.md (§3), specs/world/waypoints.md (§5.1, §6)
//! The single-player game of the app: `d2-server`'s [`SimGame`] on the
//! wired `d2-sim` ([`ActionSim`] with the waypoint world [`ActionWorld`]),
//! behind the in-process host ([`LocalLink`]), started on its own thread
//! ([`ThreadLink`], see there why).
//!
//! What runs is what the wiring has providers for (`docs/HANDOFF.md` §1
//! rows 3j, 3k): the tick driver, the timer queue, and the intents whose
//! handler runs on a real provider (0x49 waypoints). The DRLG is the
//! synthetic two-act one of the bridge's own end-to-end test
//! (`bridge/local_tests.rs`): one 8×8-tile floor room each in Cold Plains
//! (act 0) and Lut Gholein (act 1). Generating levels from the live
//! tables needs the DS1 providers the level types still lack (HANDOFF §2
//! step 7), so no level is generated from game files here.
//!
//! [`GameData::Live`] (with `D2_GAME_DIR`) takes the `levels` and
//! `objects` tables from the user's own files (`d2_data::bin::load`); the
//! waypoint object is then the first `objects` row with operate function
//! 23 and init function 17 (`waypoints.md` §5.1 rule 1).
//! [`GameData::Synthetic`] uses the bridge test's rows instead.
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
use d2_sim::combat::CombatTables;
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
use d2_sim::world::waypoints::{WaypointData, NO_WAYPOINT};

use super::server_thread::{ThreadLink, ThreadStopped};
use crate::bridge::local::{LocalLink, PendingSession};
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
/// Sub-tile x of the waypoint object and of the player (both at y 20).
pub const WAYPOINT_X: i32 = 20;
pub const PLAYER_X: i32 = 42;
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

struct Tiles(BTreeMap<Vec<u8>, Vec<TileInfo>>);

impl TileSource for Tiles {
    fn dt1(&self, path: &[u8]) -> Option<&[TileInfo]> {
        self.0.get(path).map(Vec::as_slice)
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
/// blank and tile-10 entries.
fn tiles() -> Tiles {
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
    Tiles(t)
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

/// Where the game's tables come from.
#[derive(Debug, Clone)]
pub enum GameData {
    Synthetic,
    /// Tables already loaded from the user's files.
    Live(WaypointTables),
}

impl GameData {
    fn tables(&self) -> WaypointTables {
        match self {
            GameData::Synthetic => WaypointTables::synthetic(),
            GameData::Live(t) => t.clone(),
        }
    }
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
    let mut types = Types(BTreeMap::from([
        (COLD_PLAINS, TileRect::new(0, 0, 8, 8)),
        (ACT2_TOWN, TileRect::new(0, 0, 8, 8)),
    ]));
    let mut dungeon = Dungeon::default();
    dungeon.acts[0] =
        Some(Drlg::create(0, 1, 0, 0, false, &drlg, &mut types).map_err(BuildError::Drlg)?);
    dungeon.acts[1] =
        Some(Drlg::create(1, 2, 0, 0, false, &drlg, &mut types).map_err(BuildError::Drlg)?);
    let world = DrlgWorld {
        dungeon,
        data: Arc::new(drlg),
        tiles: Box::new(tiles()),
        types: Box::new(types),
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
                d.generate_level(svc.data, svc.types, l)?;
                let r = d.level_rooms(l)[0];
                d.stream_room(svc, r)
            })
            .ok_or_else(|| BuildError::Setup(format!("act {act} has no DRLG")))?
            .map_err(BuildError::Drlg)?
            .ok_or_else(|| BuildError::Setup(format!("level {level}: no room streamed")))?;
        rooms.push(r);
    }
    let mut spawn = |ty: UnitType, class: u32, room: RoomId, x: i32| {
        let req = AllocRequest {
            ty,
            class,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: ty == UnitType::Player,
        };
        sim.with(&mut game, |g, v| v.allocate(g, &req, x, 20))
            .ok_or_else(|| BuildError::Setup(format!("allocating {ty:?} {class} failed")))
    };
    let waypoint = spawn(
        UnitType::Object,
        wp_tables.object_class,
        rooms[0],
        WAYPOINT_X,
    )?;
    let player = spawn(UnitType::Player, PLAYER_CLASS, rooms[0], PLAYER_X)?;
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
