// Spec: specs/client/bridge.md (§3), specs/world/waypoints.md (§5.1, §6), specs/sim/path-placement.md (§13), specs/sim/rng.md (§5.2), specs/world/objects.md (§2), specs/world/npc.md (§1.1), specs/world/quests.md (§2), specs/items/treasure.md (§4), specs/world/hirelings.md (Inputs)
//! The single-player game of the app: `d2-server`'s [`SimGame`] on the
//! full wired `d2-sim` world ([`WorldSim`]: the action systems with the
//! world-generation state, population and monster init) with the wired
//! host ([`WiredWorld`]: waypoints, the NPC, vendor, quest and cube
//! systems, the interaction state), behind the in-process host
//! ([`LocalLink`]), started on its own thread ([`ThreadLink`], see there
//! why).
//!
//! Game creation (`rng.md` §5.2) runs at build, before any unit: the
//! creation fields written to their home (`ActionEvents::create_game`:
//! Normal, expansion, the `--seed` as the game seed `{N, 666}` unstepped,
//! the fixed-seed branch of §5.2), then `WorldSim::create_game`'s four
//! game-seed derivations in order (monster regions, object control, NPC
//! control, quest control); the NPC and quest controls go to the wired
//! host. The object control makes S→C 0x03 carry its `dwObjSeed` (game
//! +0x80). With the user's files the drop state of the chest drop
//! (`ActionHooks::object_drops`, `treasure.md` §4: the item, treasure and
//! superunique tables of `d2_server::world_data::tables::drop_tables`)
//! and the hireling tables (`InteractionState::hireling_tables`) are
//! installed too. The game has one store of unique bits (+0x1B24,
//! `ActionHooks::uniques`): the chest drops and the host's economy share
//! it.
//!
//! What runs is what the wiring has providers for (`docs/HANDOFF.md` §1
//! rows 3j, 3k): the tick driver with population, the timer queue, the
//! path provider, and the intents whose handler runs on a real provider
//! (0x49 waypoints, the wired host's ids). Two
//! acts are created and one room is streamed in Cold Plains (act 0) and
//! Lut Gholein (act 1). The local player enters through the server's
//! session flow (`d2_server::adapters::session_flow`,
//! `sim/intents-events.md` §8, `sim/path-placement.md` §13): the client
//! sends C→S 0x67 ([`create_request`]) through the system queue before
//! its first frame ([`super::play::send_create_game`]); the drain runs
//! game creation (§8.1) and the next flush carries 0x01, 0x00, 0x02; the
//! client answers 0x02 with C→S 0x6B (`client/model.md` §7 rule 3, the
//! bridge's own answer), whose drain runs the join (§8.2): the
//! [`Character`] loader creates the player (a new character, or a parsed
//! `.d2s` through `session::load_save`), then the player's 0x59, 0xAA,
//! 0x76, 0x0B, …, 0x03, 0x53, game entry's 0x07, the room switch's 0x07s
//! (with the add messages of the rooms' units), 0x15, 0x7E leave with
//! the next flush, and the following tick's 0x04; the client builds its
//! own DRLG and is in game.
//!
//! [`GameData::Live`] (with `D2_GAME_DIR`, [`LiveData::load`]) takes
//! everything from the user's own files: every table view of the game
//! (`d2_server::world_data::game::GameTables`: the loaded and fixed-up
//! sets; the waypoint object is the first `objects` row
//! with operate function 23 and init function 17, `waypoints.md` §5.1
//! rule 1), the drop, hireling and save tables
//! (`d2_server::world_data::tables`), and the level generation data of `d2_server::world_data`
//! (drlg-data: the level-type table views, every lvlprest / lvlsub DS1
//! and lvltypes DT1, parsed), so the acts are generated through
//! `d2_sim::wiring::worldgen::levels::WorldTypes` (the Maze / Presets /
//! Outdoor dispatcher) with the server's town level ids (`levels.md` §2
//! step 2: 1, 40). [`GameData::Synthetic`] (no game files) uses the
//! bridge test's rows and its synthetic two-act DRLG
//! (`bridge/local_tests.rs`: one 8×8-tile floor room per level), with
//! empty skill, combat, monster, item and vendor tables, no drop state
//! and no hireling tables.
//!
//! Seams without a provider are [`LocalSeams`] (the action and world
//! wiring's): the narrowest answers (`Pending`'s and `WorldPending`'s
//! defaults) plus a store of what the sim itself sets (positions) and
//! the transport outbox; and [`super::rest::AppRest`] (the wired
//! host's). Nothing
//! here decides an outcome: it stages the game the way the server tests
//! do (a sorceress who knows her act's first waypoint, `bridge.md` §3).

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_data::tables::{
    decode_all, Difficultylevels, Itemstatcost, Levels, Monstats, Objects, Record, Shrines, Skills,
};
use d2_formats::animdata::AnimData;
use d2_formats::d2s::{self, D2s, ReadOptions};
use d2_formats::mpq::ArchiveSet;
use d2_server::adapters::character::LoadContext;
use d2_server::adapters::handlers::world::{ActionEvents, ActionWorld, Outbox, WiredWorld};
use d2_server::adapters::session::{load_new_character, load_save, GameSetup};
use d2_server::adapters::session_flow::{
    create_flags, CharacterLoader, CreateGame, Loaded, SessionFlow,
};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame};
use d2_server::host::Host;
use d2_server::host::SystemClock;
use d2_server::seams::{ClientId, Clock, PlayerGate};
use d2_server::world_data::game::GameTables;
use d2_server::world_data::tables::{drop_tables, hireling_tables, LevelTables, SaveData};
use d2_server::world_data::{archive as world_archive, Dt1Files, WorldFiles};
use d2_sim::combat::vitals::VitalsTables;
use d2_sim::combat::CombatTables;
use d2_sim::drlg::maze::{Maze, MazeData};
use d2_sim::drlg::room::LinkAt;
use d2_sim::drlg::{
    CellGrid, Drlg, DrlgData, DrlgError, DrlgRoomId, Dungeon, GridPass, LevelDef, LevelIdx,
    LevelTypes, RoomGrids, RoomKind, TileInfo, TileRect, TileSource,
};
use d2_sim::game::Game;
use d2_sim::items::ItemTables;
use d2_sim::monsters::init::GameInfo;
use d2_sim::rng::Seed;
use d2_sim::skills::SkillTables;
use d2_sim::stats::StatData;
use d2_sim::units::hooks::UnitData;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, ActionTables, DrlgWorld, Pending};
use d2_sim::wiring::economy::{DeathDrops, DropTables, GameFields};
use d2_sim::wiring::worldgen::levels::{SharedTypes, WorldTypes};
use d2_sim::wiring::worldgen::{CreationTables, WorldPending, WorldSim, WorldState, WorldTables};
use d2_sim::world::hirelings::HirelingTables;
use d2_sim::world::npc::HireRow;
use d2_sim::world::objects::ObjectTables;
use d2_sim::world::quests::{PlayerQuests, QuestFlags, QuestTables};
use d2_sim::world::vendors::VendorTables;
use d2_sim::world::waypoints::{WaypointData, NO_WAYPOINT};

use d2_sim::drlg::outdoor::{OutdoorData, SubFile, SubFiles};
use d2_sim::drlg::preset::{Ds1Input, Ds1Source, PresetData};

use super::rest::AppRest;
use super::server_thread::{ThreadLink, ThreadStopped};
use crate::bridge::drlg::DrlgSource;
use crate::bridge::local::{LocalLink, PendingSession};
use crate::bridge::world::{
    LevelRow, MonsterClass, MonsterSetup, ObjectRow, SkillRow, StatSend, UnitRows,
};
use crate::bridge::LOCAL_CLIENT;

/// The game's dispatch and world host.
pub type Sim = SimGame<WorldSim<LocalSeams>, World>;

/// The wired host of the app's game.
pub type World = WiredWorld<AppRest>;

/// The local link over [`Sim`] with clock `C`.
pub type Link<C = SystemClock> = LocalLink<Sim, ProtoSizes, PendingSession, C>;

/// The Rogue Encampment (act 0's town, where game entry places the
/// player: `sim/path-placement.md` §13 rule 2), Cold Plains (act 0) and
/// Lut Gholein (act 1).
pub const ACT1_TOWN: u32 = 1;
pub const COLD_PLAINS: u32 = 3;
pub const ACT2_TOWN: u32 = 40;
/// The default game seed.
pub const DEFAULT_SEED: u32 = 1234;
/// Game +0x6A of a single-player game: 3 (`rng.md` §5 open question,
/// answered: the client's create message carries 3, stored at +0x6A).
pub const GAME_TYPE: u8 = 3;
/// Sub-tile x and y of the waypoint object from the origin of the town's
/// first room (inside the synthetic 8 × 8-tile room, 40 sub-tiles square).
pub const WAYPOINT_X: i32 = 20;
pub const UNIT_Y: i32 = 20;
/// The player's character class (1, sorceress, as in the server tests).
pub const PLAYER_CLASS: u32 = 1;
/// The character's name (0x59 bytes 6..22, zero-padded).
pub const PLAYER_NAME: &[u8] = b"Sorceress";

/// The app's game (S→C 0x01, `intents-events.md` §8.1 rule 3): Normal,
/// expansion, not ladder, the arena flags of every recorded join
/// (0x00100004; the arena record is not modelled).
pub const GAME_SETUP: GameSetup = GameSetup {
    difficulty: 0,
    arena_flags: 0x0010_0004,
    expansion: true,
    ladder: false,
};

/// The local client's C→S 0x67 (`client/model.md` §7 rule 9, builder
/// `0x00477CA0`; checks `intents-events.md` §2.5): the character class
/// and name above, game type 3, Normal, an expansion character's flags
/// 0x00100004, locale 0; passes the server's checks.
pub fn create_request() -> CreateGame {
    create_request_for(&Character::New)
}

/// The 0x67 u32@0x27 of an expansion character: the builder's default
/// 4 | 0x100000 (`client/model.md` §7 rule 9; recorded 0x00100004).
pub const CREATE_FLAGS_EXPANSION: u32 = create_flags::EXPANSION | 0x4;

/// The 0x67 u32@0x27 of a classic character.
///
/// PROVISIONAL (client/model.md §7 r9; REC-46): bit 2 alone, without the
/// expansion bit 20.
pub const CREATE_FLAGS_CLASSIC: u32 = 0x4;

/// The local client's C→S 0x67 for `character` (`client/model.md` §7
/// rule 9): game name empty (byte 1 = 0), game type 3 (client type 0),
/// the character's class and name ([`Character::Save`]: the save's
/// class +0x28 and name +0x14), template 0, the game's difficulty,
/// u16@0x25 = 0, the flags of an expansion or a classic character (save
/// status bit 5), @0x2B = @0x2C = 0, language id 0. Bytes after a name's
/// NUL are zero.
pub fn create_request_for(character: &Character) -> CreateGame {
    let (class, name, expansion) = match character {
        Character::New => (PLAYER_CLASS as u8, PLAYER_NAME, GAME_SETUP.expansion),
        Character::Named(c) => (c.class, c.name(), GAME_SETUP.expansion),
        Character::Save(save, _) => (
            save.header.class,
            save.header.name_bytes(),
            save.header.status & d2_formats::d2s::status::EXPANSION != 0,
        ),
    };
    let mut char_name = [0u8; 16];
    char_name[..name.len()].copy_from_slice(name);
    CreateGame {
        game_type: GAME_TYPE,
        class,
        template: 0,
        difficulty: GAME_SETUP.difficulty,
        char_name,
        arena: 0,
        flags: if expansion {
            CREATE_FLAGS_EXPANSION
        } else {
            CREATE_FLAGS_CLASSIC
        },
        unk_43: 0,
        unk_44: 0,
        locale: 0,
        ..CreateGame::default()
    }
}

/// The character the session flow's loader gives the join (§8.2 rule 2,
/// `0x005345A0`: a new character or a save).
#[derive(Debug, Clone, Default)]
pub enum Character {
    /// A new character of the 0x67 request's class and name, as the save
    /// loader leaves a player (no room, at (0, 0), mode 1), knowing Cold
    /// Plains' waypoint on Normal (the server tests' staging). It is the
    /// stub load (`intents-events.md` §8.2 rule 7,
    /// `d2_server::adapters::session::load_new_character`), so the join
    /// sends 0x5F and the two 0x23.
    #[default]
    New,
    /// [`Character::New`] with the class and name of `d2-client play
    /// --new <class> <name>` (decision D3, `docs/PLAN.md`: a CLI stand-in
    /// for the select / create screens, which are not specified). Held in
    /// memory only: nothing is written to disk.
    Named(NewCharacter),
    /// A parsed `.d2s` loaded onto the new player
    /// (`d2_server::adapters::session::load_save`); `d2-client play
    /// --save` reads one with [`LiveData::read_save`].
    Save(Box<D2s>, LoadContext),
}

/// The class names of `play --new` in class-id order (`charstats` rows
/// 0–6; `items/inventory.md` §1.3 uses the same ids).
pub const CLASS_NAMES: [&str; 7] = [
    "amazon",
    "sorceress",
    "necromancer",
    "paladin",
    "barbarian",
    "druid",
    "assassin",
];

/// Longest character name: the 0x67 name must have a NUL within its 16
/// bytes (`intents-events.md` §2.5 rule 1, `0x0053EFC0(name, 16)`).
pub const MAX_NAME_LEN: usize = 15;

/// The class and name of a new character (`play --new`, decision D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewCharacter {
    /// Class id 0–6 (the 0x67 class u8@0x12; ≥ 7 is refused, §2.5 r1).
    pub class: u8,
    /// The name, NUL-padded (bytes after the name are 0).
    pub name: [u8; 16],
}

impl NewCharacter {
    /// The name bytes up to the first NUL.
    pub fn name(&self) -> &[u8] {
        let n = self.name.iter().position(|&b| b == 0).unwrap_or(16);
        &self.name[..n]
    }
}

/// Why `play --new <class> <name>` was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NewCharacterError {
    #[error("unknown class {0:?}: use 0-6 or one of {names}", names = CLASS_NAMES.join(", "))]
    Class(String),
    #[error("character name {0:?}: {1}")]
    Name(String, &'static str),
}

/// The class id of `s`: a number 0–6 or a [`CLASS_NAMES`] entry (any
/// case).
pub fn parse_class(s: &str) -> Result<u8, NewCharacterError> {
    if let Ok(n) = s.parse::<u8>() {
        return if n < 7 {
            Ok(n)
        } else {
            Err(NewCharacterError::Class(s.to_owned()))
        };
    }
    CLASS_NAMES
        .iter()
        .position(|c| c.eq_ignore_ascii_case(s))
        .map(|i| i as u8)
        .ok_or_else(|| NewCharacterError::Class(s.to_owned()))
}

/// The character of `play --new <class> <name>` (decision D3): class by
/// name or 0–6; the name 1–[`MAX_NAME_LEN`] bytes so the 0x67 check
/// passes (§2.5 r1).
pub fn new_character(class: &str, name: &str) -> Result<Character, NewCharacterError> {
    let class = parse_class(class)?;
    let bad = |why| NewCharacterError::Name(name.to_owned(), why);
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(bad("must be 1 to 15 bytes"));
    }
    // d2rs-own, unverified: the create screen's name rules are not
    // specified (no menu spec); ASCII letters, digits, `-` and `_` keep
    // the name printable in every font and safe in a file name.
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(bad("only ASCII letters, digits, '-' and '_'"));
    }
    let mut bytes = [0u8; 16];
    bytes[..name.len()].copy_from_slice(name.as_bytes());
    Ok(Character::Named(NewCharacter { class, name: bytes }))
}

/// The load result the loader gives when the player unit cannot be
/// allocated. d2rs diagnostic, not an original code (the original's
/// allocation does not fail this way); a save that does not load gives
/// its `formats/d2s.md` §10 result instead (`LoadError::result`). The
/// flow records either as `SessionFault::LoadRefused` and removes the
/// client (its S→C 0xB4 is not sent, `session_flow` module docs).
pub const LOAD_FAILED: u32 = u32::MAX;

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
    /// Game creation's NPC or quest control (`rng.md` §5.2).
    #[error("game creation: {0}")]
    Creation(#[from] d2_sim::wiring::worldgen::CreationError),
    /// `--save`: the file could not be read, or it is not a save the
    /// reader takes (`formats/d2s.md` §1, §10).
    #[error("save {path}: {message}")]
    Save { path: String, message: String },
    #[error(transparent)]
    Thread(#[from] ThreadStopped),
}

/// The action and world wiring's seams without a provider. Positions
/// are stored as the sim sets them (the interaction target is the unit
/// record's, `UnitRecord::interact`); messages the sim
/// sends wait in `sent` for the world handlers ([`Outbox`]); warp and
/// arrival mode, whose bodies are unwritten specs, are logged. The world
/// wiring's seams keep `WorldPending`'s defaults.
#[derive(Debug, Default)]
pub struct LocalSeams {
    pub pos: BTreeMap<UnitId, (i32, i32)>,
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

impl WorldPending for LocalSeams {}

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
        roof_height: 0,
        height: 0,
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
    /// The loaded and fixed-up sets and `AnimData.d2`: every other table
    /// view of the game.
    pub tables: GameTables,
    /// The chest drop's tables (`ActionHooks::object_drops`).
    pub drops: Arc<DropTables>,
    /// `InteractionState::hireling_tables`.
    pub hirelings: HirelingTables,
    /// The `.d2s` reader's tables (`--save`), for the app's expansion game.
    pub save: SaveData,
    /// The archive set itself (the client's other readers: sounds).
    pub archives: Arc<ArchiveSet>,
}

impl LiveData {
    /// Loads the table sets, the waypoint, drop, hireling and save tables
    /// and the level data (a table or a named file that is missing or does
    /// not parse is an error; nothing falls back to synthetic data).
    pub fn load(archives: Arc<ArchiveSet>) -> Result<Self, BuildError> {
        let waypoints = WaypointTables::live(&archives)?;
        let tables = GameTables::load(&archives)?;
        let levels = LevelTables::from_fixed(&tables.fixed)?;
        let files = WorldFiles::load(
            &levels.drlg,
            &levels.preset,
            &levels.outdoor,
            world_archive::reader(&archives),
        )?;
        Ok(LiveData {
            waypoints,
            levels,
            files,
            drops: Arc::new(drop_tables(&tables.fixed)?),
            hirelings: hireling_tables(&tables.fixed)?,
            save: SaveData::from_fixed(&tables.fixed, GAME_SETUP.expansion)?,
            tables,
            archives,
        })
    }

    /// Reads and checks the save `bytes` on the app's game
    /// (`d2s::read`, `formats/d2s.md` §1–§8, with the header checks of
    /// §2.2 rules 4–5 against [`GAME_SETUP`]: Normal, expansion, not
    /// hardcore). The client's name is the save's own: the client sends
    /// the selected character's name in its C→S 0x67
    /// ([`create_request_for`]).
    pub fn read_save(&self, bytes: &[u8]) -> Result<D2s, d2s::D2sError> {
        let opts = ReadOptions {
            expansion: GAME_SETUP.expansion,
            game: Some(d2s::GameContext {
                client_name: save_name(bytes).to_vec(),
                expansion: GAME_SETUP.expansion,
                hardcore: false,
                difficulty: GAME_SETUP.difficulty,
            }),
        };
        d2s::read(bytes, &opts, &self.save)
    }
}

/// The character name of a save's bytes (`formats/d2s.md` §2.1: +0x14,
/// 16 bytes, +0x23 read as 0 (§2.2 rule 4), up to the first NUL); empty
/// for a file shorter than the header.
pub fn save_name(bytes: &[u8]) -> &[u8] {
    let Some(field) = bytes.get(0x14..0x23) else {
        return &[];
    };
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    &field[..end]
}

/// The character of `d2-client play --save <file.d2s>`: the file read
/// and checked with the user's tables ([`LiveData::read_save`]), loaded
/// at the join with [`LoadContext`] of the app's game (Normal; the saved
/// map seed does not apply: the app's game runs on a fixed seed, game
/// +0x84 = 1, `formats/d2s.md` §2.2 rule 8, `rng.md` §5.2). Synthetic
/// data has no save tables: an error.
pub fn load_character(data: &GameData, path: &std::path::Path) -> Result<Character, BuildError> {
    let err = |message: String| BuildError::Save {
        path: path.display().to_string(),
        message,
    };
    let GameData::Live(d) = data else {
        return Err(err(
            "reading a save needs the game's tables (D2_GAME_DIR)".into()
        ));
    };
    let bytes = std::fs::read(path).map_err(|e| err(e.to_string()))?;
    let save = d.read_save(&bytes).map_err(|e| err(e.to_string()))?;
    Ok(Character::Save(
        Box::new(save),
        LoadContext {
            difficulty: GAME_SETUP.difficulty,
            map_seed_applies: false,
        },
    ))
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

/// The synthetic DRLG table view: 150 levels without warps; the Rogue
/// Encampment, Cold Plains and Lut Gholein are preset levels of tile
/// library 1 (`floor.dt1`).
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
    for id in [ACT1_TOWN, COLD_PLAINS, ACT2_TOWN] {
        drlg.levels[id as usize].drlg_type = 2;
        drlg.levels[id as usize].level_type = 1;
    }
    drlg
}

/// The synthetic level types: one 8×8-tile floor room in the Rogue
/// Encampment (the game entry's town, at tile (16, 0): levels of one act
/// do not overlap), one in Cold Plains and one in Lut Gholein.
fn synthetic_types() -> Types {
    Types(BTreeMap::from([
        (ACT1_TOWN, TileRect::new(16, 0, 8, 8)),
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

/// The `objects.txt` rows of the client object update
/// (`world/objects-client.md` §28 r1): the live table; none for the
/// synthetic game (its object update then runs nothing).
pub fn client_object_rows(data: &GameData) -> Vec<crate::bridge::objects::ObjClientRow> {
    match data {
        GameData::Synthetic => Vec::new(),
        GameData::Live(d) => crate::bridge::objects::ObjClientRow::rows(&d.waypoints.objects),
    }
}

/// The `Levels.txt` fields the client reads (`client/model.md` §11
/// rules 3–4: `Pal`, `Act`, `BlankScreen`; `audio/environment.md` §1 r2:
/// `SoundEnv`), one row per level id, from the game's `levels` table.
pub fn client_level_rows(data: &GameData) -> Vec<LevelRow> {
    data.tables()
        .levels
        .iter()
        .map(|l| LevelRow {
            pal: l.pal,
            act: l.act,
            blank_screen: l.blankscreen != 0,
            sound_env: l.soundenv,
            draw_edges: l.drawedges != 0,
        })
        .collect()
}

/// `difficultylevels` `ResistPenalty` per row (difficulty), from the
/// user's `.bin` set: the expansion resist penalty of the character
/// panel (`ui/panels.md` §8.9, `0x00611D30`; `panels-2.md` §24 r2), for
/// [`crate::ui::original::OriginalUi::set_resist_penalties`]. The field
/// is read as a signed value.
pub fn client_resist_penalties(archives: &ArchiveSet) -> Result<Vec<i32>, BuildError> {
    let set = d2_data::bin::load(archives, "eng").map_err(|e| BuildError::Tables(e.to_string()))?;
    let table = set
        .table("difficultylevels")
        .ok_or_else(|| BuildError::Tables("difficultylevels not loaded".into()))?;
    let rows: Vec<Difficultylevels> =
        decode_all(table).map_err(|e| BuildError::Tables(e.to_string()))?;
    Ok(rows.iter().map(|r| r.resistpenalty as i32).collect())
}

/// The `skills` fields the client skill list reads (`client/msg-skills.md`
/// Inputs: `anim`, `monanim`, `passivestate`; §9–§10: `enhanceable`,
/// `EType`, `skilldesc`, `srvdofunc`; `skills/levels.md` §1, §6:
/// `charclass`, `maxlvl`), one row per skill id, from the user's `skills`
/// table.
pub fn client_skill_rows(archives: &ArchiveSet) -> Result<Vec<SkillRow>, BuildError> {
    let set = d2_data::bin::load(archives, "eng").map_err(|e| BuildError::Tables(e.to_string()))?;
    let table = set
        .table("skills")
        .ok_or_else(|| BuildError::Tables("skills not loaded".to_owned()))?;
    let rows: Vec<Skills> = decode_all(table).map_err(|e| BuildError::Tables(e.to_string()))?;
    Ok(rows
        .iter()
        .map(|s| SkillRow {
            anim: s.anim,
            monanim: s.monanim,
            passivestate: s.passivestate,
            maxlvl: s.maxlvl,
            charclass: s.charclass as i8,
            srvdofunc: s.srvdofunc as i16,
            enhanceable: s.enhanceable,
            skilldesc: s.skilldesc,
            etype: s.etype,
        })
        .collect())
}

/// The `monstats` / `monstats2` columns of the client monster set-up
/// (`client/msg-units.md` §1.2 r6): `m` the typed row, `raw` its record
/// (the `Sk`i`mode` bytes +0x180 + i are callback columns), `m2` the
/// `monstats2` record (`isSel` byte +4 bit 3, `shadow` +5 bit 6, `isAtt`
/// +5 bit 1).
fn monster_setup(m: &Monstats, raw: &[u8], m2: &[u8]) -> MonsterSetup {
    let byte = |b: &[u8], o: usize| b.get(o).copied().unwrap_or(0);
    let skill = [
        (m.skill1, m.sk1lvl),
        (m.skill2, m.sk2lvl),
        (m.skill3, m.sk3lvl),
        (m.skill4, m.sk4lvl),
        (m.skill5, m.sk5lvl),
        (m.skill6, m.sk6lvl),
        (m.skill7, m.sk7lvl),
        (m.skill8, m.sk8lvl),
    ];
    let mut skills = [(0i16, 0u8, 0u8); 8];
    for (i, ((s, l), out)) in skill.into_iter().zip(skills.iter_mut()).enumerate() {
        *out = (s as i16, l, byte(raw, 0x180 + i));
    }
    MonsterSetup {
        level: [m.level, m.level_n, m.level_h],
        res: [
            [m.resdm, m.resdm_n, m.resdm_h],
            [m.resma, m.resma_n, m.resma_h],
            [m.resfi, m.resfi_n, m.resfi_h],
            [m.resli, m.resli_n, m.resli_h],
            [m.resco, m.resco_n, m.resco_h],
            [m.respo, m.respo_n, m.respo_h],
        ],
        velocity: m.velocity,
        align: m.align,
        skills,
        is_sel: byte(m2, 4) & 0x08 != 0,
        shadow: byte(m2, 5) & 0x40 != 0,
        is_att: byte(m2, 5) & 0x02 != 0,
    }
}

/// The skills tables and formula buffers of the client's passive refresh
/// (`client/msg-skills.md` §2 r4), from the user's tables.
pub fn client_skill_tables(archives: &ArchiveSet) -> Result<SkillTables, BuildError> {
    let set = d2_data::bin::load(archives, "eng").map_err(|e| BuildError::Tables(e.to_string()))?;
    SkillTables::from_bin(&set, d2_sim::skills::LEVEL_CAP_114D)
        .map_err(|e| BuildError::Tables(e.to_string()))
}

/// The unit-message rows of the client (`client/msg-units.md` §1.2 r7:
/// each `monstats` row's `MonStatsEx` link into `monstats2`, whose record
/// bytes +0x15… hold the component choice counts; §1.2 r4: the
/// `itemstatcost` send columns; §1.3 r3 and `client/model.md` §15 r1,
/// `render/lighting.md` OQ 11: `objects.txt` and the `shrines.txt`
/// codes), from the user's tables.
pub fn client_unit_rows(archives: &ArchiveSet) -> Result<UnitRows, BuildError> {
    let set = d2_data::bin::load(archives, "eng").map_err(|e| BuildError::Tables(e.to_string()))?;
    let table = |name: &str| {
        set.table(name)
            .ok_or_else(|| BuildError::Tables(format!("{name} not loaded")))
    };
    let err = |e: d2_data::tables::WrongTable| BuildError::Tables(e.to_string());
    let monstats_table = table("monstats")?;
    let monstats: Vec<Monstats> = decode_all(monstats_table).map_err(err)?;
    let monstats2 = table("monstats2")?;
    let monsters = monstats
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let link = m.monstatsex as i16;
            if link < 0 || link as usize >= monstats2.count {
                return None;
            }
            let m2 = monstats2.record(link as usize);
            let mut c = MonsterClass::from_record(m2, m.npc, m.interact)?;
            c.setup = Some(monster_setup(m, monstats_table.record(i), m2));
            Some(c)
        })
        .collect();
    let difficulty: Vec<Difficultylevels> = decode_all(table("difficultylevels")?).map_err(err)?;
    let mut monster_skill_bonus = [0i32; 3];
    for (b, d) in monster_skill_bonus.iter_mut().zip(&difficulty) {
        *b = d.monsterskillbonus as i32;
    }
    let isc: Vec<Itemstatcost> = decode_all(table("itemstatcost")?).map_err(err)?;
    let stats = isc
        .iter()
        .map(|r| StatSend {
            bits: r.send_bits,
            param_bits: r.send_param_bits,
            signed: r.signed,
        })
        .collect();
    let objects: Vec<Objects> = decode_all(table("objects")?).map_err(err)?;
    let objects = objects
        .iter()
        .map(|o| ObjectRow {
            subclass: o.subclass,
            shrine_function: o.shrinefunction,
            env_effect: o.enveffect != 0,
            lit: [
                o.lit0, o.lit1, o.lit2, o.lit3, o.lit4, o.lit5, o.lit6, o.lit7,
            ],
            selectable: [
                o.selectable0 != 0,
                o.selectable1 != 0,
                o.selectable2 != 0,
                o.selectable3 != 0,
                o.selectable4 != 0,
                o.selectable5 != 0,
                o.selectable6 != 0,
                o.selectable7 != 0,
            ],
            rgb: (o.red, o.green, o.blue),
        })
        .collect();
    let shrines: Vec<Shrines> = decode_all(table("shrines")?).map_err(err)?;
    Ok(UnitRows {
        monsters,
        monster_skill_bonus,
        stats,
        objects,
        shrines: shrines.iter().map(|s| s.code).collect(),
    })
}

/// A built game and the units the app and tests address. The player
/// exists only after the join (C→S 0x6B): [`local_player`].
pub struct LocalGame {
    pub sim: Sim,
    pub waypoint: UnitId,
    /// The waypoint object's GUID.
    pub waypoint_guid: u32,
}

/// The tables game creation and the wired host read beyond the DRLG's.
struct GameParts {
    action: ActionTables,
    stats: StatData,
    units: UnitData,
    world: WorldTables,
    /// The level-type handle of the world state (preset lookups).
    world_types: SharedTypes,
    objects: ObjectTables,
    monstats: Vec<Monstats>,
    hire_rows: Vec<HireRow>,
    items: ItemTables,
    vendors: VendorTables,
    anim: Option<Arc<AnimData>>,
    vitals: Option<Arc<VitalsTables>>,
    /// The chest drop's tables; `None`: no drop (synthetic).
    drops: Option<Arc<DropTables>>,
    /// `None`: the mercenary calls report no tables (synthetic).
    hirelings: Option<HirelingTables>,
}

impl GameParts {
    /// No game files: the waypoint rows, everything else empty; the world
    /// state's level types over the synthetic DRLG view with no preset,
    /// outdoor or maze data.
    fn synthetic(wp: &WaypointTables) -> Result<Self, BuildError> {
        let presets = PresetData {
            defs: Vec::new(),
            monpreset_acts: Default::default(),
            monpreset: Vec::new(),
            monstats_count: 0,
            superuniques_count: 0,
            hdm_item: -1,
            tables: d2_sim::drlg::preset::PresetTables::spec()
                .map_err(|e| BuildError::Tables(format!("preset-tables.tsv: {e}")))?,
        };
        let world_types = SharedTypes::new(WorldTypes::new(
            Arc::new(synthetic_drlg_data()),
            Maze::new(MazeData::default()),
            presets,
            OutdoorData::default(),
            Box::new(d2_server::world_data::Ds1Files::default()),
            Box::new(d2_sim::drlg::outdoor::SubFileMap::default()),
        ));
        Ok(GameParts {
            action: empty_action_tables(),
            stats: StatData::default(),
            units: UnitData {
                expansion: GAME_SETUP.expansion,
                ..UnitData::default()
            },
            world: WorldTables {
                levels: wp.levels.clone(),
                ..WorldTables::default()
            },
            world_types,
            objects: ObjectTables {
                objects: wp.objects.clone(),
                shrines: Vec::new(),
                levels: wp.levels.clone(),
                ..ObjectTables::default()
            },
            monstats: Vec::new(),
            hire_rows: Vec::new(),
            items: ItemTables::default(),
            vendors: VendorTables::default(),
            anim: None,
            vitals: None,
            drops: None,
            hirelings: None,
        })
    }

    /// The user's tables (`GameTables`), the drop and hireling tables of
    /// [`LiveData`], the world state's level types over the live data.
    fn live(d: &LiveData) -> Result<Self, BuildError> {
        let t = &d.tables;
        Ok(GameParts {
            action: t.action_tables()?,
            stats: t.stat_data()?,
            units: t.unit_data(GAME_SETUP.expansion)?,
            world: t.world_tables()?,
            world_types: live_types(d),
            objects: t.object_tables()?,
            monstats: t.rows()?,
            hire_rows: t.hire_rows()?,
            items: t.item_tables()?,
            vendors: t.vendor_tables()?,
            anim: Some(Arc::new(t.anim.clone())),
            vitals: Some(Arc::new(t.vitals()?)),
            drops: Some(d.drops.clone()),
            hirelings: Some(d.hirelings.clone()),
        })
    }
}

/// Action tables with no rows (the synthetic game reads none).
fn empty_action_tables() -> ActionTables {
    ActionTables {
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
    }
}

/// The level-type dispatcher over the live data (`WorldTypes`, as
/// drlg-data's game-file tests build it).
fn live_types(d: &LiveData) -> SharedTypes {
    SharedTypes::new(WorldTypes::new(
        Arc::new(d.levels.drlg.clone()),
        Maze::new(d.levels.maze.clone()),
        d.levels.preset.clone(),
        d.levels.outdoor.clone(),
        Box::new(d.files.ds1.clone()),
        Box::new(d.files.subs.clone()),
    ))
}

/// Builds the game on `seed`: the DRLG of both acts, game creation
/// (`rng.md` §5.2, module docs), one room streamed
/// in the Rogue Encampment, Cold Plains and Lut Gholein, the waypoint
/// object in the town's room, the path provider on, the wired host on
/// the created controls, and the session flow
/// set: no client record and no player until the client's C→S 0x67 and
/// 0x6B are drained (the loader creates a [`Character::New`]).
pub fn build(data: &GameData, seed: u32) -> Result<LocalGame, BuildError> {
    build_with(data, seed, Character::New)
}

/// [`build`] with the character the join loads (module docs).
pub fn build_with(
    data: &GameData,
    seed: u32,
    character: Character,
) -> Result<LocalGame, BuildError> {
    let wp_tables = data.tables();
    let (mut levels, parts) = match data {
        GameData::Synthetic => (LevelSource::synthetic(), GameParts::synthetic(&wp_tables)?),
        GameData::Live(d) => (LevelSource::live(d, seed), GameParts::live(d)?),
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
    // `rng.md` §5.2, the fixed-seed branch (`--seed N`): the game seed
    // is `{N, 666}`, unstepped.
    let mut hooks = ActionHooks::new(
        Arc::new(parts.action),
        world,
        Seed::init_low(seed),
        LocalSeams::default(),
    );
    hooks.anim_data = parts.anim;
    hooks.vitals = parts.vitals;
    // Game entry places through the path provider; on before any unit is
    // allocated.
    hooks
        .enable_paths()
        .map_err(|e| BuildError::Setup(format!("path tables: {e:?}")))?;
    let info = GameInfo {
        expansion: GAME_SETUP.expansion,
        difficulty: GAME_SETUP.difficulty,
        game_type: GAME_TYPE,
        ladder: GAME_SETUP.ladder,
        ..GameInfo::default()
    };
    let state = WorldState::new(parts.world_types, Arc::new(parts.world), info);
    let mut sim = WorldSim::new(Arc::new(parts.stats), parts.units, hooks, state);
    // Game creation (`rng.md` §5.2): the creation fields to their home,
    // then the four seeded controls in order, before any unit.
    let fields = GameFields {
        difficulty: GAME_SETUP.difficulty,
        game_type: GAME_TYPE,
        ladder: GAME_SETUP.ladder,
        ..GameFields::new(Seed::init_low(seed), GAME_SETUP.expansion)
    };
    ActionEvents::create_game(&mut sim, &fields);
    let quest_tables =
        QuestTables::load().map_err(|e| BuildError::Tables(format!("quest tables: {e}")))?;
    let created = sim.create_game(CreationTables {
        objects: Arc::new(parts.objects),
        monstats: &parts.monstats,
        hirelings: parts.hire_rows,
        quests: &quest_tables,
    })?;
    // The chest drop's state (`treasure.md` §4): its seed, creation
    // fields and unique bits are the action wiring's.
    sim.action.hooks().object_drops = parts.drops.map(|t| {
        Box::new(DeathDrops::new(
            t,
            GameFields::new(Seed::init_low(0), false),
        ))
    });
    let mut game = Game::new();
    let mut rooms = Vec::new();
    for (act, level) in [(0u8, ACT1_TOWN), (0, COLD_PLAINS), (1, ACT2_TOWN)] {
        game.lists
            .ensure_act(act)
            .map_err(|e| BuildError::Setup(format!("act {act}: {e:?}")))?;
        let r = sim
            .action
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
    // The waypoint object stands in the town's first room at a fixed
    // sub-tile offset from its origin (subtile = tile × 5, `levels.md`
    // §1), as the server tests stage it.
    let (room0, rect0) = rooms[0];
    let (ox, oy) = (rect0.x * 5, rect0.y * 5);
    let req = AllocRequest {
        ty: UnitType::Object,
        class: wp_tables.object_class,
        room: Some(room0),
        add: true,
        fixed_guid: None,
        mode: 1,
        allied: false,
    };
    let waypoint = sim
        .action
        .with(&mut game, |g, v| {
            v.allocate(g, &req, ox + WAYPOINT_X, oy + UNIT_Y)
        })
        .ok_or_else(|| BuildError::Setup("allocating the waypoint object failed".into()))?;
    let waypoint_guid = game
        .lists
        .unit(waypoint)
        .ok_or_else(|| BuildError::Setup("waypoint unit missing".into()))?
        .guid;
    // The wired host on the created controls.
    let action = ActionWorld {
        waypoints: Some(WaypointData::new(&wp_tables.levels, &wp_tables.objects)),
        ..ActionWorld::default()
    };
    let rest = AppRest {
        expansion: GAME_SETUP.expansion,
        ..AppRest::default()
    };
    // `WiredWorld::now` (store generation and refresh, `vendors.md` edge
    // case 10) is the host clock in ms, set by each host frame
    // (`host_tick`); 0 until the first frame.
    let mut world = WiredWorld::new(
        action,
        parts.items,
        created.quests,
        created.npc,
        parts.vendors,
        rest,
        0,
    );
    world.state.hireling_tables = parts.hirelings;
    let mut s: Sim = SimGame::with_world(game, sim, world);
    // The session sequence (`intents-events.md` §8) runs on the client's
    // C→S 0x67 / 0x6B: game creation (the client record, 0x01, 0x00,
    // 0x02; state 1), then the join (this loader, the player's add
    // messages, 0x0B, …, 0x03, 0x53, game entry with its room switch;
    // state 3). The next tick populates the town's rooms, the client's room
    // is ready and the client pass sends 0x04 (`tick.md` §6 rule 6).
    let cold_plains_wp = wp_tables.waypoint(COLD_PLAINS);
    s.set_session(SessionFlow::new(loader(character, cold_plains_wp)));
    Ok(LocalGame {
        sim: s,
        waypoint,
        waypoint_guid,
    })
}

/// The character load of the session flow (§8.2 rule 2): the player of
/// the request's class, as the save loader leaves it (no room, at (0, 0),
/// mode 1), then the [`Character`]'s values; the player's quest record
/// (a new one: `quests.md` §1.7) and name go to the wired host's rest.
/// What did not apply is logged in [`LocalSeams::log`].
fn loader(
    character: Character,
    cold_plains_wp: Option<u8>,
) -> CharacterLoader<WorldSim<LocalSeams>, World> {
    Box::new(move |s: &mut Sim, _: ClientId, r: &CreateGame| {
        let req = AllocRequest {
            ty: UnitType::Player,
            class: u32::from(r.class),
            room: None,
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: true,
        };
        let Some(player) = s
            .events
            .action
            .with(&mut s.game, |g, v| v.allocate(g, &req, 0, 0))
        else {
            s.events
                .action
                .hooks()
                .x
                .log
                .push(format!("join: allocating player class {} failed", r.class));
            return Err(LOAD_FAILED);
        };
        if let Some(u) = s.events.action.sys.units.get_mut(player) {
            u.mode = 1;
        }
        let (entry, quests) = match &character {
            Character::New | Character::Named(_) => {
                if let Some(index) = cold_plains_wp {
                    let set = s
                        .events
                        .action
                        .hooks()
                        .waypoints
                        .entry(player)
                        .or_default()
                        .get_mut(0)
                        .set(u32::from(index));
                    if let Err(e) = set {
                        s.events
                            .action
                            .hooks()
                            .x
                            .log
                            .push(format!("join: waypoint {index}: {e:?}"));
                    }
                }
                // §8.2 rule 7: the stub path (start stats, `StartSkill`),
                // so the join sends 0x5F and the two 0x23.
                let (entry, report) = load_new_character(s, player, r.char_name);
                let log = &mut s.events.action.hooks().x.log;
                log.extend(
                    report
                        .unapplied
                        .iter()
                        .map(|u| format!("join: new character: {u:?}")),
                );
                (entry, PlayerQuests::default())
            }
            Character::Save(save, ctx) => match load_save(s, player, save, ctx) {
                Ok((entry, report)) => {
                    let log = &mut s.events.action.hooks().x.log;
                    log.extend(
                        report
                            .unapplied
                            .iter()
                            .map(|u| format!("join: save load: {u:?}")),
                    );
                    // Load §2 quests row (`0x0056A370` → `0x0065C4D0`,
                    // `world/quests.md` §1.6): each 96-byte record copied
                    // into the player's quest record with normalisation.
                    // A stub (no body) keeps the new record (load §1).
                    let mut quests = PlayerQuests::default();
                    if let Some(body) = &save.body {
                        for (d, rec) in body.quests.records.iter().enumerate() {
                            match QuestFlags::copy_in(rec, true) {
                                Ok(f) => quests.flags[d] = f,
                                Err(e) => log.push(format!("join: save load: quests {d}: {e}")),
                            }
                        }
                    }
                    (entry, quests)
                }
                Err(e) => {
                    s.events
                        .action
                        .hooks()
                        .x
                        .log
                        .push(format!("join: save load failed: {e}"));
                    return Err(e.result());
                }
            },
        };
        let name = r.char_name;
        let n = name.iter().position(|&b| b == 0).unwrap_or(name.len());
        let rest = &mut s.world.rest;
        rest.quests.insert(player, quests);
        rest.names.insert(player, name[..n].to_vec());
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
        Ok(Loaded { player, entry })
    })
}

/// The local client's player and its GUID once the join has run (C→S
/// 0x6B drained), else `None`.
pub fn local_player(s: &Sim) -> Option<(UnitId, u32)> {
    let p = s.player_of(LOCAL_CLIENT)?;
    Some((p, s.game.lists.unit(p)?.guid))
}

/// The units of a started game, for the caller (the player exists after
/// the join: [`local_player`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Started {
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
    start_with(data, seed, Character::New, clock)
}

/// [`start`] with the character the join loads.
pub fn start_with<C: Clock + Send + 'static>(
    data: GameData,
    seed: u32,
    character: Character,
    clock: C,
) -> Result<(ThreadLink<Link<C>>, Started), BuildError> {
    let (tx, rx) = std::sync::mpsc::channel();
    let link = ThreadLink::spawn(move || {
        let g = build_with(&data, seed, character)?;
        let _ = tx.send(Started {
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

#[cfg(test)]
mod new_character_tests {
    use super::*;

    // Covers: specs/sim/intents-events.md §2.5 r1
    #[test]
    fn class_by_name_or_number() {
        for (i, name) in CLASS_NAMES.iter().enumerate() {
            assert_eq!(parse_class(name), Ok(i as u8));
            assert_eq!(parse_class(&name.to_uppercase()), Ok(i as u8));
            assert_eq!(parse_class(&i.to_string()), Ok(i as u8));
        }
        for bad in ["7", "255", "-1", "", "paladins", "ama"] {
            assert!(parse_class(bad).is_err(), "{bad:?}");
        }
    }

    // Covers: specs/sim/intents-events.md §2.5 r1
    #[test]
    fn names_fit_the_create_request() {
        let Ok(Character::Named(c)) = new_character("amazon", "Test") else {
            panic!("refused");
        };
        assert_eq!((c.class, c.name()), (0, &b"Test"[..]));
        assert_eq!(c.name[4..], [0; 12]);
        assert!(new_character("druid", &"A".repeat(15)).is_ok());
        for bad in ["", &"A".repeat(16), "two words", "Näme", "a\0b"] {
            assert!(
                matches!(
                    new_character("druid", bad),
                    Err(NewCharacterError::Name(..))
                ),
                "{bad:?}"
            );
        }
        assert!(matches!(
            new_character("monk", "Test"),
            Err(NewCharacterError::Class(_))
        ));
    }

    // Covers: specs/client/model.md §7 r9
    #[test]
    fn the_create_request_carries_the_new_class_and_name() {
        let c = new_character("6", "Shadow_1").unwrap();
        let r = create_request_for(&c);
        assert_eq!(r.class, 6);
        assert_eq!(&r.char_name[..9], b"Shadow_1\0");
        assert_eq!(r.char_name[9..], [0; 7]);
        assert_eq!(r.flags, CREATE_FLAGS_EXPANSION);
        assert_eq!(r.game_type, GAME_TYPE);
    }
}
