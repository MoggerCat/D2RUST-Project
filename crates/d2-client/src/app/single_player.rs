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
//! step 2: 1, 40). There is no other data: without the user's files
//! the game is not built ([`BuildError::NoGameDir`]; the invented
//! synthetic game was retired by q-fixture-migrate, M23).
//!
//! Seams without a provider are [`LocalSeams`] (the action and world
//! wiring's): the narrowest answers (`Pending`'s and `WorldPending`'s
//! defaults) plus a store of what the sim itself sets (positions), the
//! transport outbox, the combat seams' copy of the units
//! ([`sync_seams`]) and the skill pipeline's preview fills
//! ([`super::skill_rest`]: the world's skill slot is `WiredSkills`, on
//! d2-sim's skill lists); and [`super::rest::AppRest`] (the wired
//! host's). Nothing
//! here decides an outcome: it stages the game the way the server tests
//! do (a sorceress who knows her act's first waypoint, `bridge.md` §3).

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::assets::game_files::GameFiles;
use crate::assets::path::{CanonicalPath, FileSource};
use d2_data::bin::TableFiles;
use d2_data::tables::{
    decode_all, Charstats, Difficultylevels, Itemstatcost, Levels, Monstats, Objects, Shrines,
    Skills,
};
use d2_formats::animdata::AnimData;
use d2_formats::d2s::{self, D2s, ReadOptions};
use d2_formats::mpq::ArchiveSet;
use d2_native::source::NativeAsset;
use d2_server::adapters::character::LoadContext;
use d2_server::adapters::handlers::skills::wired::WiredSkills;
use d2_server::adapters::handlers::world::{
    preview_cube_parts, preview_inv_parts, ActionEvents, ActionWorld, Outbox, QuestEnter,
    WiredWorld, WorldHost,
};
use d2_server::adapters::session::{
    initial_portal_flags, load_new_character_with_items, load_save, GameSetup,
};
use d2_server::adapters::session_flow::{
    create_flags, CharacterLoader, CreateGame, Loaded, SessionFlow,
};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame};
use d2_server::host::Host;
use d2_server::host::SystemClock;
use d2_server::seams::{ClientId, Clock, PlayerGate};
use d2_server::world_data::game::GameTables;
use d2_server::world_data::tables::{drop_tables, hireling_tables, LevelTables, SaveData};
use d2_server::world_data::{self, WorldFiles};
use d2_sim::combat::vitals::VitalsTables;
use d2_sim::drlg::maze::Maze;
use d2_sim::drlg::{Drlg, DrlgData, DrlgError, Dungeon, LevelTypes, TileSource};
use d2_sim::game::Game;
use d2_sim::items::inventory::tables::InvTables;
use d2_sim::items::ItemTables;
use d2_sim::monsters::init::GameInfo;
use d2_sim::rng::Seed;
use d2_sim::skills::SkillTables;
use d2_sim::stats::StatData;
use d2_sim::units::hooks::{Sim as USim, UnitData};
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, ActionTables, DrlgWorld, Pending, SkillEvent};
use d2_sim::wiring::economy::{DeathDrops, DropTables, GameFields};
use d2_sim::wiring::interaction::skill_events;
use d2_sim::wiring::worldgen::levels::{SharedTypes, WorldTypes};
use d2_sim::wiring::worldgen::{CreationTables, WorldPending, WorldSim, WorldState, WorldTables};
use d2_sim::world::cube::CubeData;
use d2_sim::world::hirelings::HirelingTables;
use d2_sim::world::npc::HireRow;
use d2_sim::world::objects::ObjectTables;
use d2_sim::world::quests::{PlayerQuests, QuestFlags, QuestTables};
use d2_sim::world::vendors::VendorTables;
use d2_sim::world::waypoints::{WaypointData, NO_WAYPOINT};

use d2_sim::drlg::outdoor::{SubFile, SubFiles};
use d2_sim::drlg::preset::{Ds1Input, Ds1Source};

use super::rest::AppRest;
use super::server_thread::{ThreadLink, ThreadStopped};
use super::skill_rest::SkillStore;
use crate::bridge::drlg::DrlgSource;
use crate::bridge::local::{LocalLink, PendingSession};
use crate::bridge::world::{
    LevelRow, MonsterClass, MonsterSetup, ObjectRow, SkillRow, StatSend, StateRow, UnitRows,
};
use crate::bridge::LOCAL_CLIENT;

/// The game's dispatch and world host.
pub type Sim = SimGame<WorldSim<LocalSeams>, World>;

/// The wired host of the app's game.
pub type World = WiredWorld<AppRest, WiredSkills>;

/// The local link over [`Sim`] with clock `C`.
pub type Link<C = SystemClock> = LocalLink<Sim, ProtoSizes, PendingSession, C>;

/// The Rogue Encampment (act 0's town, where game entry places the
/// player: `sim/path-placement.md` §13 rule 2), Cold Plains (act 0) and
/// Lut Gholein (act 1).
pub const ACT1_TOWN: u32 = 1;
/// The Blood Moor (act 0), east of the synthetic town's room.
pub const BLOOD_MOOR: u32 = 2;
pub const COLD_PLAINS: u32 = 3;
/// Stony Field: a waypoint level not built at game creation (its level
/// init runs on arrival). d2rs-own, unverified (q-waypoint-travel).
pub const STONY_FIELD: u32 = 4;
/// The Den of Evil (act 0): the cave entrance in the Blood Moor is a
/// level warp (its `lvlwarp` id is the `levels` row's `Warp` of the Den slot).
pub const DEN_OF_EVIL: u32 = 8;
pub const ACT2_TOWN: u32 = 40;
/// Harrogath (act 4, the fifth act; `levels` row 109).
pub const ACT5_TOWN: u32 = 109;
/// Kurast Docks (act 2's town, `levels` row 75) and the Pandemonium
/// Fortress (act 3's, row 103).
pub const KURAST_DOCKS: u32 = 75;
/// The stash object's class (`objects.txt` row 267, `world/objects.md`
/// §16.10 `BANK_CLASS`).
pub const STASH_CLASS: u32 = 267;
pub const PANDEMONIUM_FORTRESS: u32 = 103;
/// Catacombs Level 4, Andariel's lair (act 0; a flat level in the
/// synthetic world, reached by a level warp: d2rs-own, unverified).
pub const CATACOMBS_4: u32 = 37;
/// The default game seed.
pub const DEFAULT_SEED: u32 = 1234;

/// The map seed the game is built with (game +0x7C; the DRLG seed of
/// `sim/rng.md` §5.4). `fixed` is `play --seed N`, the fixed-seed switch
/// (game +0x84 := 1): it wins. Otherwise a loaded save whose town byte
/// for the game's difficulty has 0x80 gives its saved map seed
/// (`formats/d2s.md` §2.2 rule 8, +0xAB). Otherwise [`DEFAULT_SEED`].
/// PROVISIONAL (REC-291 -> q-fix-new-char-seed): 1.14d draws a fresh
/// seed for a new character (`time_value`, `rng.md` §5.2; measured: two
/// new characters got 0x63a0b0fd and 0x07013cee,
/// `traces/frontend/frontend-menus/frontend-0005.json`); d2rs keeps the
/// fixed default so dev runs and draw dumps stay reproducible.
pub fn game_seed(character: &Character, fixed: Option<u32>) -> u32 {
    if let Some(n) = fixed {
        return n;
    }
    match character {
        Character::Save(save, ctx) => {
            let t = save.header.towns[usize::from(ctx.difficulty).min(2)];
            if t & 0x80 != 0 {
                save.header.map_seed
            } else {
                DEFAULT_SEED
            }
        }
        _ => DEFAULT_SEED,
    }
}
/// Game +0x6A of a single-player game: 3 (`rng.md` §5 open question,
/// answered: the client's create message carries 3, stored at +0x6A).
pub const GAME_TYPE: u8 = 3;
/// Duriel's Lair (`levels` row 73, act 1).
pub const DURIELS_LAIR: u32 = 73;
/// Durance of Hate levels 1 and 2 (`quests.md` §8.2).
pub const DURANCE_1: u32 = 100;
pub const DURANCE_2: u32 = 101;
/// Duriel's `monstats` row.
pub const DURIEL_CLASS: u32 = 211;
/// Izual's `monstats` row and the quest chain his death links
/// (`init.md` §14.3).
pub const IZUAL: u32 = 256;
pub const IZUAL_CHAIN: u32 = 22;
/// Blood Raven's `monstats` row and her quest chain (`init.md` §14.3).
pub const BLOOD_RAVEN: u32 = 267;
pub const BLOOD_RAVEN_CHAIN: u32 = 2;

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
/// Bit 2 alone, without the expansion bit 20 (`client/model.md` §7 r9;
/// recorded, REC-46: `facts/join/a1-new-classic-ama.tsv`).
pub const CREATE_FLAGS_CLASSIC: u32 = 0x4;

/// The local client's C→S 0x67 for `character` (`client/model.md` §7
/// rule 9): game name empty (byte 1 = 0), game type 3 (client type 0),
/// the character's class and name ([`Character::Save`]: the save's
/// name +0x14; class byte 0, REC-1130), template 0, the game's difficulty,
/// u16@0x25 = 0, the flags of an expansion or a classic character (save
/// status bit 5), @0x2B = @0x2C = 0, language id 0. Bytes after a name's
/// NUL are zero.
pub fn create_request_for(character: &Character) -> CreateGame {
    let (class, name, expansion) = match character {
        Character::New => (PLAYER_CLASS as u8, PLAYER_NAME, GAME_SETUP.expansion),
        Character::Named(c) => (c.class, c.name(), GAME_SETUP.expansion),
        // PROVISIONAL (REC-1130): an existing character picked from the
        // menu leaves the builder's class byte `[0x007A0522]` at 0 (the
        // create screen and `-bar` style switches set it); recorded in
        // 1.14d, checks combat-melee-fallen / combat-potion-midfight
        // (packets, frame 1: byte 18 = 0 for a level-3 Barbarian). The
        // server takes the class from the save (`d2s.md` header +0x28).
        Character::Save(save, _) => (
            0,
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
        difficulty: character.difficulty(),
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

impl Character {
    /// The difficulty the game runs on: a save's load context, else a
    /// named character's own (Normal for [`Character::New`]).
    pub fn difficulty(&self) -> u8 {
        match self {
            Character::New => GAME_SETUP.difficulty,
            Character::Named(c) => c.difficulty,
            Character::Save(_, ctx) => ctx.difficulty,
        }
    }

    /// This character on difficulty `d` (`play --difficulty`). A save's
    /// difficulty is set at load ([`load_character`]); a default new
    /// character becomes the stand-in sorceress as a named one.
    pub fn with_difficulty(self, d: u8) -> Character {
        match self {
            Character::New => {
                let mut name = [0u8; 16];
                name[..PLAYER_NAME.len()].copy_from_slice(PLAYER_NAME);
                Character::Named(NewCharacter {
                    class: PLAYER_CLASS as u8,
                    name,
                    difficulty: d,
                })
            }
            Character::Named(c) => Character::Named(NewCharacter { difficulty: d, ..c }),
            Character::Save(s, mut ctx) => {
                ctx.difficulty = d;
                Character::Save(s, ctx)
            }
        }
    }
}

/// Parses `--difficulty`: `normal`, `nightmare`, `hell` (any case) or 0–2.
pub fn parse_difficulty(s: &str) -> Option<u8> {
    match s.to_ascii_lowercase().as_str() {
        "normal" | "0" => Some(0),
        "nightmare" | "1" => Some(1),
        "hell" | "2" => Some(2),
        _ => None,
    }
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
    /// The game's difficulty 0–2 (`play --difficulty`; Normal by default).
    pub difficulty: u8,
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
    Ok(Character::Named(NewCharacter {
        class,
        name: bytes,
        difficulty: 0,
    }))
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
    /// No game directory given: the game plays only on the user's own
    /// files (`$D2_GAME_DIR`, or `--native DIR`).
    #[error("no game files: put d2-client.exe in the Diablo II 1.14d folder (next to d2data.mpq), pass --game-dir <folder>, or set D2_GAME_DIR")]
    NoGameDir,
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
    /// The game's players and monsters (type, allied, path position),
    /// copied by [`sync_seams`] before each intent and tick: the
    /// hostility, alignment and melee-range seams have no game to read.
    pub sides: BTreeMap<UnitId, (UnitType, bool, (i32, i32))>,
    /// The players and monsters of [`Self::sides`] that are dying or dead
    /// (player modes 0 / 17, monster modes 0 / 12), for the target search.
    pub down: std::collections::BTreeSet<UnitId>,
    /// The unit size (`0x00620510`, the path record's) of the units of
    /// [`Self::sides`], for the full-size distance of the target search.
    pub sizes: BTreeMap<UnitId, i32>,
    /// The skill pipeline's per-unit fields and preview fills (`UseRest`,
    /// `LearnRest`: [`super::skill_rest`]).
    pub skills: SkillStore,
    /// The monsters' mode targets and current skills
    /// ([`super::monster_ai`]).
    pub monsters: super::monster_ai::MonsterAi,
    /// The unit tables of the client art's name rules ([`super::anim_names`]);
    /// none on synthetic data.
    pub looks: Option<Arc<crate::world_view::unit_assets::UnitLooks>>,
    /// The players and monsters for the host rest's NPC and quest seams
    /// (`npc_seams`), written by [`sync_seams`].
    pub snap: super::npc_seams::SnapRef,
    /// The character is hardcore (the save's status bit 0x4, or `play
    /// --hardcore`): client flag 4 of the one local client
    /// ([`super::hardcore`]).
    pub hardcore: bool,
    /// The clients the server dropped, with the reason
    /// (`Pending::drop_client`): the hardcore resurrect drops with 3.
    pub dropped: Vec<(UnitId, u32)>,
    /// Monster chain links and deaths for the quest control, drained once
    /// per tick (`q-a1-tower`, d2rs-own, unverified).
    pub quest_events: Vec<d2_sim::wiring::action::QuestEvent>,
    /// The Act II DRLG's staff-tomb level (0 = not yet known), set when
    /// the acts are created (`q-a2-dungeons`).
    pub staff_tomb: u32,
    /// The lair warp check's answer, published once per tick by the quest
    /// control (`Pending::set_lair_open`, q-a2-duriel).
    pub lair_open: bool,
    /// The Arreat Summit warp check's answer (`Pending::set_summit_open`,
    /// q-act3-act5-gaps); the exits stay closed while it is `true`.
    pub summit_closed: bool,
    /// The quest records' not-intro bytes by chain, published by the quest
    /// control once per tick (`Pending::publish_not_intro`): the not-intro
    /// test `0x005444B0` of population and the missile bodies.
    pub not_intro: BTreeMap<u8, bool>,
    /// The caged barbarians' group states by GUID (counting, portal
    /// GUID), published by the quest control once per tick
    /// (`Pending::publish_rescue`).
    pub rescue: BTreeMap<u32, (bool, Option<u32>)>,
    /// The Durance of Hate warp check's answer
    /// (`Pending::set_durance_open`, q-play-act3); level 100 stays closed
    /// while it is `true`.
    pub durance_closed: bool,
    /// The Golden Bird's +0x00 and the Gidbinn altar's point while Ormus
    /// may activate it (`Pending::set_act3_npc_answers`, q-play-act3).
    pub alkor_bird: bool,
    pub ormus_altar: Option<(i32, i32)>,
    /// The players' hands and the facts of the items in them
    /// ([`super::weapons`], q-amazon).
    pub weapons: super::weapons::Weapons,
}

impl LocalSeams {
    /// Player side: a player or an allied (good-aligned) monster.
    fn player_side(&self, unit: UnitId) -> Option<bool> {
        self.sides
            .get(&unit)
            .map(|&(ty, allied, _)| ty == UnitType::Player || allied)
    }

    /// The good units' target search: the nearest monster of the other
    /// side that is not dying or dead, closer than `range`; ties: the
    /// lower unit id. `full_size`: by the full-size distance (`ai.md` §6
    /// `0x005DC380`, the scanner's size from [`Self::sizes`]), as the scan
    /// 6 callback `0x005DCBD0` (`ai.md` §5.3 rule 2); else by the no-size
    /// distance (`0x005DC530`).
    ///
    /// PROVISIONAL (REC-279 part 2; d2rs-own, unverified): the ranges are
    /// settled (`ai.md` §5.2 step 4: 35; §5.3 scan 6: full-size < 49), but
    /// the scan 6 filter `0x005DC970`, the `nThreat` main / alternative
    /// classes, the line test (mask 4) and `0x005DD510` are not applied
    /// here, nor the scan 5 callback `0x005DCA70`.
    fn nearest_foe(&self, unit: UnitId, range: i32, full_size: bool) -> Option<(UnitId, i32)> {
        let &(_, _, at) = self.sides.get(&unit)?;
        let side = self.player_side(unit)?;
        let size = self.sizes.get(&unit).copied().unwrap_or(0);
        self.sides
            .iter()
            .filter(|&(&u, &(ty, ..))| {
                u != unit
                    && ty == UnitType::Monster
                    && self.player_side(u) != Some(side)
                    && !self.down.contains(&u)
            })
            .map(|(&u, &(_, _, p))| {
                let d = if full_size {
                    d2_sim::monsters::ai::distance_full_size(at, size, p)
                } else {
                    d2_sim::monsters::ai::distance_no_size(at, p)
                };
                (u, d)
            })
            .filter(|&(_, d)| d < range)
            .min_by_key(|&(u, d)| (d, u))
    }
}

/// d2rs-own, unverified (preview, D1): the melee reach of every unit in
/// sub-tiles (`0x00622870` reads the unit's size and weapon; not
/// answered here).
const PREVIEW_MELEE_RANGE: i32 = 2;

/// The good units' main search range (`ai.md` §5.2 step 4: scan 5
/// within 35).
const GOOD_SEARCH_RANGE: i32 = 35;

/// The `0x005DDC30` search window (`ai.md` §5.3, scan 6 callback
/// `0x005DCBD0` rule 2): candidates at full-size distance < 49 (0x31);
/// each caller gates its own distance (the Hireable think: < 25,
/// `ai-bodies-6.md` §7 step 8).
const SECONDARY_SEARCH_RANGE: i32 = 49;

/// The play host's seam refresh (`SimGame::set_host_sync`): the players
/// and monsters with their allied flag (`UnitLists`), for
/// [`LocalSeams::sides`].
pub fn sync_seams(game: &Game, sim: &mut WorldSim<LocalSeams>) {
    let classes: BTreeMap<UnitId, u32> = [UnitType::Player, UnitType::Monster]
        .into_iter()
        .flat_map(|ty| game.lists.units_of_type(ty))
        .filter_map(|u| Some((u, sim.action.sys.units.get(u)?.class)))
        .collect();
    let down: std::collections::BTreeSet<UnitId> = classes
        .keys()
        .copied()
        .filter(|&u| {
            sim.action.sys.units.get(u).is_some_and(|r| match r.ty {
                UnitType::Player => matches!(r.mode, 0 | 17),
                _ => matches!(r.mode, 0 | 12),
            })
        })
        .collect();
    let hooks = &mut sim.action.sys.hooks;
    hooks.x.down = down;
    // d2rs-own, unverified (q-assassin-gaps, REC-233): a listed pet is on the
    // player side (the summon's alignment effect, `0x005543B0`, is not wired).
    let pets: std::collections::BTreeSet<u32> = hooks
        .pet_lists
        .values()
        .flat_map(|l| l.entries.iter())
        .flat_map(|e| e.nodes.iter().map(|n| n.guid as u32))
        .collect();
    let mut sides = BTreeMap::new();
    let mut sizes = BTreeMap::new();
    for ty in [UnitType::Player, UnitType::Monster] {
        for u in game.lists.units_of_type(ty) {
            if let Some(e) = game.lists.unit(u) {
                let pet = ty == UnitType::Monster && pets.contains(&e.guid);
                sides.insert(u, (ty, e.allied || pet, hooks.path_position(u)));
                let size = hooks.paths.as_ref().and_then(|p| p.record(u));
                let size = match size {
                    Some(d2_sim::path::UnitPath::Dynamic(d)) => d.unit_size,
                    _ => 0,
                };
                sizes.insert(u, size);
            }
        }
    }
    hooks.x.sides = sides;
    hooks.x.sizes = sizes;
    let mut units = BTreeMap::new();
    for ty in [UnitType::Player, UnitType::Monster] {
        for u in game.lists.units_of_type(ty) {
            let Some(e) = game.lists.unit(u) else {
                continue;
            };
            let act = e
                .room()
                .and_then(|r| game.lists.room(r))
                .map_or(0, |r| r.act);
            let level = e
                .room()
                .and_then(|r| hooks.drlg.level_id(game, r))
                .unwrap_or(0);
            let class = classes.get(&u).copied().unwrap_or(0);
            units.insert(
                u,
                super::npc_seams::SnapUnit {
                    ty,
                    pos: hooks.path_position(u),
                    act,
                    guid: e.guid,
                    level,
                    class,
                },
            );
        }
    }
    if let Ok(mut snap) = hooks.x.snap.lock() {
        snap.units = units;
    }
    super::skill_rest::sync_shapes(game, sim);
}

impl Pending for LocalSeams {
    fn frame_event_index(&self, unit: UnitId) -> i32 {
        LocalSeams::frame_event_index(self, unit)
    }

    fn set_frame_event_index(&mut self, unit: UnitId, i: i32) {
        LocalSeams::set_frame_event_index(self, unit, i);
    }

    fn body_path_op(
        &mut self,
        unit: UnitId,
        op: d2_sim::skills::use_::bodies::PathOp<UnitId>,
    ) -> i32 {
        LocalSeams::path_op(self, unit, op);
        0
    }

    /// Client flag 4 (`0x00538670`): the local character is hardcore.
    fn client_hardcore(&self, _player: UnitId) -> bool {
        self.hardcore
    }

    /// `0x0052CAF0`: the client is dropped; the preview records it
    /// (`super::hardcore`).
    fn drop_client(&mut self, player: UnitId, reason: u32) {
        self.dropped.push((player, reason));
    }

    fn monster_quest_chain(&mut self, unit: UnitId, chain: u32) {
        if let Ok(chain) = u8::try_from(chain) {
            self.quest_events
                .push(d2_sim::wiring::action::QuestEvent::Link { unit, chain });
        }
    }
    /// d2rs-own, unverified (REC-136): Radament's and the Summoner's AI
    /// calls reach the quest control at the end of the tick.
    fn ai_quest_call(
        &mut self,
        _: &mut Game,
        unit: UnitId,
        call: d2_sim::monsters::ai::QuestCall,
    ) -> bool {
        use d2_sim::monsters::ai::QuestCall;
        use d2_sim::wiring::action::QuestEvent;
        match call {
            QuestCall::RadamentActivated => self
                .quest_events
                .push(QuestEvent::RadamentActivated { unit }),
            QuestCall::SummonerActivated => self.quest_events.push(QuestEvent::SummonerActivated),
            // d2rs-own, unverified (REC-148): the Act V AI calls.
            QuestCall::Shenk => self.quest_events.push(QuestEvent::ShenkActivated { unit }),
            QuestCall::Nihlathak => self.quest_events.push(QuestEvent::NihlathakActivated),
            QuestCall::BaalToStairs => self.quest_events.push(QuestEvent::BaalToStairs),
            QuestCall::AncientsNotActivatable => {
                self.quest_events.push(QuestEvent::AncientsDisarm);
                return false;
            }
            _ => return false,
        }
        true
    }
    /// d2rs-own, unverified (REC-148): Anya's AI asks for the temple portal.
    fn anya_open_portal(&mut self, _: &mut Game, unit: UnitId) {
        self.quest_events
            .push(d2_sim::wiring::action::QuestEvent::AnyaOpenPortal { unit });
    }
    /// `0x0061AEB0`: the Act II staff tomb, the orifice's level.
    fn object_staff_tomb(&self) -> u32 {
        if self.staff_tomb == 0 {
            u32::MAX
        } else {
            self.staff_tomb
        }
    }
    /// `0x00545B80` for level 73 (`quests.md` §8.2, `quests-act2.md`
    /// §8.8): closed until the lair is open, and then only from the tomb
    /// holding the orifice (d2rs-own, unverified, REC-167: the synthetic
    /// Lair has a way in from every tomb).
    fn warp_quest_gate(&self, source: u32, level: u32) -> u32 {
        u32::from(
            (level == DURIELS_LAIR
                && !(self.lair_open && source == self.staff_tomb))
                // `0x0058D090` (`quests-act5-2.md` §7.8): leaving the
                // summit for 118 or 128 waits for the Ancients (d2rs-own,
                // unverified, REC-246: the made-up chain has both exits).
                || (source == d2_sim::world::quests::act5::q5::SUMMIT
                    && matches!(level, 118 | 128)
                    && self.summit_closed)
                // `0x005BBFA0` (`quests.md` §8.2): Durance of Hate 1 waits
                // for the Compelling Orb, except from Durance 2.
                || (level == DURANCE_1 && source != DURANCE_2 && self.durance_closed),
        )
    }
    fn set_summit_open(&mut self, open: bool) {
        self.summit_closed = !open;
    }
    /// `0x005444B0` (`quests.md` §2.3): no record with the chain → true.
    fn quest_not_intro(&self, chain: u8) -> bool {
        self.not_intro.get(&chain).copied().unwrap_or(true)
    }
    fn publish_not_intro(&mut self, records: &[(u8, bool)]) {
        self.not_intro = records.iter().copied().collect();
    }
    /// d2rs-own, unverified (REC-799): the prisoner AI's hooks with an
    /// effect run on the quest control after the tick.
    fn queue_quest_event(&mut self, e: d2_sim::wiring::action::QuestEvent) {
        self.quest_events.push(e);
    }
    fn quest_rescue(&self, guid: u32) -> (bool, Option<u32>) {
        self.rescue.get(&guid).copied().unwrap_or((false, None))
    }
    fn publish_rescue(&mut self, barbarians: &[(u32, bool, Option<u32>)]) {
        self.rescue = barbarians.iter().map(|&(g, c, p)| (g, (c, p))).collect();
    }
    /// d2rs-own, unverified (REC-796): Tyrael's spawn runs on the quest
    /// control after the tick, not inside the missile body.
    fn missile_spawn_tyrael(
        &mut self,
        room: Option<d2_sim::units::RoomId>,
        missile: UnitId,
        x: i32,
        y: i32,
    ) {
        self.quest_events
            .push(d2_sim::wiring::action::QuestEvent::SpawnTyrael {
                room,
                missile,
                x,
                y,
            });
    }
    fn set_lair_open(&mut self, open: bool) {
        self.lair_open = open;
    }
    fn set_durance_open(&mut self, open: bool) {
        self.durance_closed = !open;
    }
    fn set_act3_npc_answers(&mut self, alkor_bird: bool, ormus_altar: Option<(i32, i32)>) {
        self.alkor_bird = alkor_bird;
        self.ormus_altar = ormus_altar;
    }
    /// `0x005BAD20` (`ai-bodies.md` §9.9 alkor): the published answer.
    fn alkor_bird(&mut self, _: &mut Game) -> bool {
        self.alkor_bird
    }
    /// `0x005BAD40`: queued for the quest control (REC-781, d2rs-own,
    /// unverified: after the tick, as REC-129); the published answer
    /// drops at once so the same tick reads it cleared.
    fn alkor_reset(&mut self, _: &mut Game) {
        self.alkor_bird = false;
        self.quest_events
            .push(d2_sim::wiring::action::QuestEvent::AlkorReset);
    }
    /// `0x005B9CA0` (§9.9 ormus): the published altar point.
    fn ormus_altar(&mut self, _: &mut Game) -> Option<(i32, i32)> {
        self.ormus_altar
    }
    /// `0x005B9CD0`: queued as [`Self::alkor_reset`] (REC-781).
    fn ormus_set_altar_mode(&mut self, _: &mut Game) {
        self.ormus_altar = None;
        self.quest_events
            .push(d2_sim::wiring::action::QuestEvent::OrmusAltar);
    }
    /// C→S 0x44 (`quests-act2-2.md` §3.2): queued for the quest control
    /// (REC-167, d2rs-own, unverified: it runs after the tick, not inside
    /// the handler); the handler's own result is 0.
    fn staff_in_orifice(
        &mut self,
        _: &mut Game,
        player: UnitId,
        object: u32,
        item: u32,
        action: u16,
    ) -> Option<u32> {
        self.quest_events
            .push(d2_sim::wiring::action::QuestEvent::InsertItem {
                player,
                object,
                item,
                action,
            });
        Some(0)
    }
    fn take_quest_events(&mut self) -> Vec<d2_sim::wiring::action::QuestEvent> {
        std::mem::take(&mut self.quest_events)
    }
    fn kill_step(
        &mut self,
        _: &mut Game,
        step: d2_sim::wiring::action::KillStep,
        defender: UnitId,
        attacker: UnitId,
    ) {
        if step == d2_sim::wiring::action::KillStep::QuestKill {
            self.quest_events
                .push(d2_sim::wiring::action::QuestEvent::Kill {
                    victim: defender,
                    killer: Some(attacker),
                });
        }
    }
    /// d2rs-own, unverified (REC-108): mode DT and the treasure drop
    /// ([`super::monster_drop::death_start`]).
    fn monster_death_start(
        h: &mut ActionHooks<Self>,
        sim: &mut d2_sim::units::hooks::Sim<'_>,
        unit: UnitId,
        target: Option<UnitId>,
    ) -> bool {
        super::monster_drop::death_start(h, sim, unit, target)
    }
    // The skill timer events and the action frame reach the skill use
    // pipeline (`use.md` §5.2, §7); without them a cast's do step never
    // runs: no mana spent, no missile.
    fn skill_event(h: &mut ActionHooks<Self>, sim: &mut USim<'_>, ev: SkillEvent) {
        skill_events::route(h, sim, ev);
    }
    fn action_frame(
        h: &mut ActionHooks<Self>,
        sim: &mut USim<'_>,
        unit: UnitId,
        a1: u32,
        a2: u32,
    ) -> u32 {
        skill_events::action_frame(h, sim, unit, a1, a2)
    }
    fn monster_skill_start(h: &mut ActionHooks<Self>, sim: &mut USim<'_>, unit: UnitId) -> i32 {
        skill_events::monster_skill_start(h, sim, unit)
    }
    fn monster_sequence_frame(h: &mut ActionHooks<Self>, sim: &mut USim<'_>, unit: UnitId) {
        skill_events::monster_sequence_frame(h, sim, unit);
    }
    fn monster_attack_skill(h: &mut ActionHooks<Self>, sim: &mut USim<'_>, unit: UnitId) {
        skill_events::monster_attack_skill(h, sim, unit);
    }
    fn monster_attack_strike(
        h: &mut ActionHooks<Self>,
        sim: &mut USim<'_>,
        unit: UnitId,
        moving: bool,
    ) {
        skill_events::monster_attack_strike(h, sim, unit, moving);
    }
    fn monster_mode_damage(h: &mut ActionHooks<Self>, sim: &mut USim<'_>, unit: UnitId, mode: u32) {
        skill_events::monster_mode_damage(h, sim, unit, mode);
    }
    fn golem_resummon(h: &mut ActionHooks<Self>, sim: &mut USim<'_>, player: UnitId) -> bool {
        skill_events::golem_resummon(h, sim, player)
    }
    fn passive_refresh_all(h: &mut ActionHooks<Self>, sim: &mut USim<'_>, unit: UnitId) {
        skill_events::passive_refresh_all(h, sim, unit);
    }
    // d2rs-own, unverified (q-amazon, REC-150): the hand class, the item
    // shoots / stack facts of the skill bodies ([`super::weapons`]).
    fn composit_weapon_class(&self, unit: UnitId) -> i32 {
        self.weapons.cof_class(unit)
    }
    fn hand_class(&self, unit: UnitId) -> i32 {
        self.weapons.hand_class(unit)
    }
    fn item_is(&self, item: UnitId, itype: i32) -> bool {
        self.weapons.item_is(item, itype)
    }
    // d2rs-own, unverified (q-weapon-combat, REC-158): combat's weapon is
    // the weapon in use; its damage reaches the wearer through the
    // linked stat list.
    fn current_weapon(&self, unit: UnitId) -> Option<UnitId> {
        self.weapons.weapon(unit)
    }
    fn weapon(&self, unit: UnitId) -> Option<UnitId> {
        self.weapons.weapon(unit)
    }
    fn item_at(&self, unit: UnitId, loc: u8) -> Option<UnitId> {
        self.weapons.item_at(unit, loc)
    }
    // d2rs-own, unverified (q-assassin-gaps, REC-233): a player whose hands
    // the weapon copy knows has an inventory, and a copied item is usable
    // (not broken); the dual-claw test of srvdo 35 (`bodies.md` §8.10).
    fn has_inventory(&self, unit: UnitId) -> bool {
        self.weapons.hands.contains_key(&unit)
    }
    fn item_usable(&self, item: UnitId) -> bool {
        self.weapons.items.contains_key(&item)
    }
    // d2rs-own, unverified (q-skill-gaps, REC-176): the shield and its
    // smite damage from the weapon copy.
    fn shield(&self, unit: UnitId) -> Option<UnitId> {
        self.weapons.shield(unit)
    }
    fn has_shield(&self, unit: UnitId) -> bool {
        self.weapons.shield(unit).is_some()
    }
    fn shield_damage(&self, item: UnitId) -> Option<(i32, i32)> {
        Some(self.weapons.facts(item).dam)
    }
    fn wield_type(&self, item: UnitId) -> i32 {
        self.weapons.facts(item).grip
    }
    fn item_type_class(&self, item: UnitId) -> u32 {
        self.weapons.type_class(item)
    }
    fn item_shoots(&self, item: UnitId) -> bool {
        self.weapons.facts(item).shoots
    }
    fn item_stackable(&self, item: UnitId) -> bool {
        self.weapons.facts(item).stackable
    }
    fn item_max_stack(&self, item: UnitId) -> i32 {
        self.weapons.facts(item).max_stack
    }
    fn anim_name(&self, unit: UnitId, ty: UnitType, class: u32, mode: u32) -> Option<[u8; 8]> {
        let weapon = self.weapons.cof_class(unit);
        super::anim_names::anim_key(self.looks.as_deref()?, ty, class, mode, weapon)
    }
    fn anim_rate(&self, _: UnitId, speed: Option<u32>) -> i16 {
        super::anim_names::anim_rate(speed)
    }
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
    /// d2rs-own, unverified (preview, decision D1; `0x00554200` is not
    /// specified): the player side (players, allied monsters) and the
    /// other monsters may attack each other; nothing else, never itself.
    fn may_attack(&self, attacker: UnitId, defender: UnitId) -> bool {
        attacker != defender
            && matches!(
                (self.player_side(attacker), self.player_side(defender)),
                (Some(a), Some(d)) if a != d
            )
    }
    // d2rs-own, unverified (preview, q-monster-ai; `monster_ai`): a mode
    // request's target, the monster's current skill and the skill pipeline's
    // monster start and per-frame (`use.md` §5.3, OQ6).
    fn set_mode_target(&mut self, unit: UnitId, target: d2_sim::monsters::ai::ModeTarget) {
        self.monsters.set_target(unit, target);
    }
    fn set_current_skill(&mut self, unit: UnitId, skill: i32) -> bool {
        self.monsters.set_current(unit, skill)
    }
    fn class_has_mode(&self, class: i32, mode: u8) -> bool {
        self.monsters.class_has_mode(class, mode)
    }
    fn used_skill(&self, unit: UnitId) -> Option<d2_sim::skills::SkillEntry> {
        self.monsters.used_skill(unit)
    }
    // d2rs-own, unverified (preview, q-a2-charge-jab; REC-274): a monster
    // has no skill list, so its used entry's flags and params (Charge's
    // moving flag, target and hit mode, `skills/bodies-2.md` §5.3) are
    // kept per unit in the skill store.
    fn entry_param(&self, unit: UnitId, _: &d2_sim::skills::SkillEntry, i: u8) -> i32 {
        self.unit_entry_param(unit, i)
    }
    fn set_entry_param_of(&mut self, unit: UnitId, _: &d2_sim::skills::SkillEntry, i: u8, v: i32) {
        self.set_unit_entry_param(unit, i, v);
    }
    fn entry_flags(&self, unit: UnitId, _: &d2_sim::skills::SkillEntry) -> u32 {
        d2_sim::wiring::interaction::UseRest::used_skill_flags(self, unit)
    }
    fn set_entry_flags(&mut self, unit: UnitId, _: &d2_sim::skills::SkillEntry, f: u32) {
        d2_sim::wiring::interaction::UseRest::set_used_skill_flags(self, unit, f);
    }
    /// `0x005DD7F0` step 4 for a good unit: [`LocalSeams::nearest_foe`]
    /// within 35 (`ai.md` §5.2 step 4), no-size distance.
    fn good_target_search(&mut self, _: &mut Game, unit: UnitId, _: bool) -> Option<(UnitId, i32)> {
        self.nearest_foe(unit, GOOD_SEARCH_RANGE, false)
    }
    /// `0x005DDC30`: [`LocalSeams::nearest_foe`] at full-size distance
    /// < 49 (`ai.md` §5.3 scan 6), with the preview's melee flag; none:
    /// distance 0x7FFFFFFF.
    fn secondary_target(&mut self, _: &mut Game, unit: UnitId) -> (Option<UnitId>, i32, bool) {
        match self.nearest_foe(unit, SECONDARY_SEARCH_RANGE, true) {
            Some((t, d)) => (Some(t), d, self.in_melee_range(unit, t, 0)),
            None => (None, 0x7FFF_FFFF, false),
        }
    }
    /// d2rs-own, unverified (preview, D1; `0x00622870`).
    fn melee_range(&self, _: UnitId) -> i32 {
        PREVIEW_MELEE_RANGE
    }
    /// d2rs-own, unverified (preview, D1; `combat/range.md` §7.2 step 3
    /// with the preview reach and no line test): the larger axis
    /// distance of the synced positions within reach + `extra` + 1.
    fn in_melee_range(&self, a: UnitId, d: UnitId, extra: i32) -> bool {
        let (Some(&(_, _, pa)), Some(&(_, _, pd))) = (self.sides.get(&a), self.sides.get(&d))
        else {
            return false;
        };
        let dist = (pa.0 - pd.0).abs().max((pa.1 - pd.1).abs());
        dist <= PREVIEW_MELEE_RANGE + extra + 1
    }
    /// d2rs-own, unverified (preview, D1; `0x006259B0`): allied monsters
    /// and players good (2), every other unit evil (0, the default).
    fn alignment(&self, unit: UnitId) -> u8 {
        if self.player_side(unit) == Some(true) {
            2
        } else {
            0
        }
    }
    /// d2rs-own, unverified (preview, D1; game +0x10F8, `ai.md` OQ6):
    /// one target-node slot per player (the player alone, no pets), in
    /// unit-list order, at most 8 (`ai.md` §5.2 step 5 reads 8).
    fn target_nodes(&self, game: &Game) -> [Vec<UnitId>; 10] {
        let mut nodes: [Vec<UnitId>; 10] = Default::default();
        for (slot, p) in nodes
            .iter_mut()
            .zip(game.lists.units_of_type(UnitType::Player))
            .take(8)
        {
            slot.push(p);
        }
        nodes
    }
    /// `0x00623660`, the operate entry's interact range (`objects.md`
    /// §7.1 rule 3): no written spec gives its test.
    /// Measured (REC-94, `facts/objects/objanim-a1-town.tsv` run r3): 1.14d's client polls
    /// `0x00623660(P, O)` every frame of the walk and sends C→S 0x13 on
    /// the first frame it returns 1 (waypoint 119 at sub-tile offset
    /// (4, 3), stash 267 at (3, 1)); both server calls of that 0x13
    /// (`0x00548B7D`, `0x00584597`) then return 1. So "in range" holds
    /// for every 0x13 the client sends; the test's own formula is not
    /// modelled (`docs/handoff/pc1-data.md` Step 4).
    fn object_in_range(&self, _: &Game, _: UnitId, _: UnitId) -> bool {
        true
    }
    /// d2rs-own, unverified (REC-117): the preview has no quest records,
    /// so every player has one and the portal's quest gate (§12 rule 7)
    /// is the level's own `leveldefs` flag.
    fn object_quest_record(&self, _: UnitId) -> bool {
        true
    }
    /// d2rs-own, unverified (stitch-objects): the preview's interact reach.
    fn object_preview_range(&self) -> Option<i32> {
        Some(crate::world_view::object_click::INTERACT_RANGE)
    }
}

impl WorldPending for LocalSeams {
    /// `0x005444B0` (population's preset swaps, `population.md` §11.3):
    /// the quest control's published answer.
    fn quest_flag(&self, flag: u8) -> bool {
        Pending::quest_not_intro(self, flag)
    }
    /// `0x00544E80` from special monster creation: queued for the quest
    /// control (the Golden Bird's boss choice, `quests-act3.md` §6.2).
    /// PROVISIONAL (REC-780, d2rs-own, unverified): it runs after the tick,
    /// as the other queued quest events (REC-129), not inside the creation.
    fn boss_quest_hook(&mut self, boss: UnitId) {
        self.quest_events
            .push(d2_sim::wiring::action::QuestEvent::BossCreated { unit: boss });
    }
    /// `0x00545B50` (`monsters/init.md` §20.1): queued as
    /// [`Self::boss_quest_hook`] (REC-780).
    fn quest_preset_boss(&mut self, unit: UnitId) {
        self.quest_events
            .push(d2_sim::wiring::action::QuestEvent::PresetBoss { unit });
    }
    /// A host-placed monster of a level's preset list
    /// ([`HOST_MONSTER_PRESET`]): Blood Raven carries chain 2 (`init.md`
    /// §14.3), as her boss mods link it when population creates her.
    fn host_monster_created(&mut self, unit: UnitId, class: u32) {
        if class == BLOOD_RAVEN {
            self.monster_quest_chain(unit, BLOOD_RAVEN_CHAIN);
        }
        if class == IZUAL {
            self.monster_quest_chain(unit, IZUAL_CHAIN);
        }
    }
    /// Duriel acts from the world (q-a2-duriel-ai, REC-254): the Lair's
    /// population starts his AI as monster creation does. d2rs-own,
    /// unverified.
    fn host_monster_ai(&self, class: u32) -> bool {
        class == DURIEL_CLASS
    }
}

impl Outbox for LocalSeams {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
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
    /// The user's own `levels` and `objects` tables (`loading.md`: the
    /// live `.bin` set, validated).
    pub fn live(archives: &dyn TableFiles) -> Result<Self, BuildError> {
        let set = d2_data::bin::load_from(archives, "eng")
            .map_err(|e| BuildError::Tables(e.to_string()))?;
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

/// Every DS1 / DT1 the level types read, through the typed reads of
/// `files` (archives or native alike).
fn load_world_files(files: &GameFiles, levels: &LevelTables) -> Result<WorldFiles, BuildError> {
    let read = |path: &[u8]| -> Result<NativeAsset, world_data::WorldDataError> {
        let name = world_data::file_name(path)?;
        let canon = CanonicalPath::new(&name).map_err(|e| world_data::WorldDataError::Read {
            path: name.clone(),
            detail: e.to_string(),
        })?;
        match files.read_native(&canon) {
            Some(Ok(a)) => Ok(a),
            Some(Err(detail)) => Err(world_data::WorldDataError::Read { path: name, detail }),
            None => Err(world_data::WorldDataError::Read {
                path: name,
                detail: "in no archive".into(),
            }),
        }
    };
    Ok(WorldFiles::load_typed(
        &levels.drlg,
        &levels.preset,
        &levels.outdoor,
        |p| match read(p)? {
            NativeAsset::Ds1(d) => Ok(d),
            _ => Err(world_data::WorldDataError::BadPath(p.to_vec())),
        },
        |p| match read(p)? {
            NativeAsset::Dt1(d) => Ok(d),
            _ => Err(world_data::WorldDataError::BadPath(p.to_vec())),
        },
    )?)
}

/// `ExpField.D2` (`path-placement.md` §7.3): the walk-back field the floor
/// drop of items, monsters' and chests' alike, tests its spots with.
fn load_expfield(files: &GameFiles) -> Result<Arc<d2_sim::path::search::ExpField>, BuildError> {
    use d2_sim::path::search::ExpField;
    let err = |m: String| BuildError::Tables(format!("{}: {m}", ExpField::PATH));
    let path = CanonicalPath::new(ExpField::PATH).map_err(|e| err(e.to_string()))?;
    match files.read_native(&path) {
        Some(Ok(NativeAsset::ExpField(f))) => ExpField::from_cells(f.height, f.width, f.cells)
            .map(Arc::new)
            .ok_or_else(|| err("cell count does not match its header".into())),
        Some(Ok(_)) => Err(err("wrong kind".into())),
        Some(Err(e)) => Err(err(e.to_string())),
        None => Err(err("in no archive".into())),
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
    /// The floor drop's walk-back field (`data\global\ExpField.D2`,
    /// `path-placement.md` §7.3), set on the path provider of every game.
    pub expfield: Arc<d2_sim::path::search::ExpField>,
    /// The archive set itself (the client's other readers: sounds).
    pub archives: Arc<GameFiles>,
}

impl LiveData {
    /// Loads the table sets, the waypoint, drop, hireling and save tables
    /// and the level data (a table or a named file that is missing or does
    /// not parse is an error; nothing falls back to synthetic data).
    pub fn load(archives: Arc<GameFiles>) -> Result<Self, BuildError> {
        let waypoints = WaypointTables::live(archives.as_ref())?;
        let bins = d2_data::bin::load_from(archives.as_ref(), d2_data::bin::DEFAULT_LANGUAGE)
            .map_err(|e| BuildError::Tables(e.to_string()))?;
        let anim = match archives.read_native(
            &CanonicalPath::new(d2_formats::animdata::PATH)
                .map_err(|e| BuildError::Tables(format!("{}: {e}", d2_formats::animdata::PATH)))?,
        ) {
            Some(Ok(NativeAsset::AnimData(a))) => a,
            Some(Ok(_)) => return Err(BuildError::Tables("AnimData.d2: wrong kind".into())),
            Some(Err(e)) => return Err(BuildError::Tables(format!("AnimData.d2: {e}"))),
            None => return Err(BuildError::Tables("AnimData.d2: in no archive".into())),
        };
        let expfield = load_expfield(&archives)?;
        let tables = GameTables::from_loaded(bins, anim)?;
        let levels = LevelTables::from_fixed(&tables.fixed)?;
        let files = load_world_files(&archives, &levels)?;
        Ok(LiveData {
            waypoints,
            levels,
            files,
            drops: Arc::new(drop_tables(&tables.fixed)?),
            hirelings: hireling_tables(&tables.fixed)?,
            save: SaveData::from_fixed(&tables.fixed, GAME_SETUP.expansion)?,
            tables,
            expfield,
            archives,
        })
    }

    /// Reads and checks the save `bytes` on the app's game
    /// (`d2s::read`, `formats/d2s.md` §1–§8, with the header checks of
    /// §2.2 rules 4–5 against [`GAME_SETUP`]: Normal, expansion, not
    /// hardcore). The client's name is the save's own: the client sends
    /// the selected character's name in its C→S 0x67
    /// ([`create_request_for`]).
    pub fn read_save(&self, bytes: &[u8], difficulty: u8) -> Result<D2s, d2s::D2sError> {
        let opts = ReadOptions {
            expansion: GAME_SETUP.expansion,
            game: Some(d2s::GameContext {
                client_name: save_name(bytes).to_vec(),
                expansion: GAME_SETUP.expansion,
                // The character's own mode: a hardcore character plays a
                // hardcore game (a dead one is refused by the header
                // check, `d2s.md` §2.2 r5: permanent death).
                hardcore: super::hardcore::save_is_hardcore(bytes),
                difficulty,
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
/// +0x84 = 1, `formats/d2s.md` §2.2 rule 8, `rng.md` §5.2).
pub fn load_character(
    data: &GameData,
    path: &std::path::Path,
    difficulty: u8,
) -> Result<Character, BuildError> {
    let err = |message: String| BuildError::Save {
        path: path.display().to_string(),
        message,
    };
    let GameData::Live(d) = data;
    let bytes = std::fs::read(path).map_err(|e| err(e.to_string()))?;
    let save = d
        .read_save(&bytes, difficulty)
        .map_err(|e| err(e.to_string()))?;
    Ok(Character::Save(
        Box::new(save),
        LoadContext {
            difficulty,
            map_seed_applies: false,
        },
    ))
}

/// Where the game's tables and levels come from.
#[derive(Debug, Clone)]
pub enum GameData {
    /// Tables and level data already loaded from the user's files.
    Live(Arc<LiveData>),
}

impl GameData {
    /// The data of `game_dir` (`$D2_GAME_DIR`): the user's own files. No
    /// directory is an error naming `D2_GAME_DIR` (nothing plays on
    /// invented data, M23), and so is a directory that does not load.
    pub fn select(game_dir: Option<&std::path::Path>) -> Result<Self, BuildError> {
        let dir = game_dir.ok_or(BuildError::NoGameDir)?;
        let archives = ArchiveSet::open_dir(dir).map_err(|e| BuildError::Archives {
            dir: dir.display().to_string(),
            message: e.to_string(),
        })?;
        Ok(GameData::Live(Arc::new(LiveData::load(Arc::new(
            GameFiles::archives(Arc::new(archives)),
        ))?)))
    }

    /// The data of a converted native folder (`play --native DIR`,
    /// `native-assets.md` §5): the same tables and level files, read from
    /// the native source.
    pub fn select_native(dir: &std::path::Path) -> Result<Self, BuildError> {
        let files = GameFiles::native(dir).map_err(|message| BuildError::Archives {
            dir: dir.display().to_string(),
            message,
        })?;
        Ok(GameData::Live(Arc::new(LiveData::load(Arc::new(files))?)))
    }

    fn tables(&self) -> WaypointTables {
        let GameData::Live(d) = self;
        d.waypoints.clone()
    }
}

/// The level generation the game runs on: table view, tile library, level
/// types, and per act the DRLG init seed and the server's town level id.
struct LevelSource {
    data: Arc<DrlgData>,
    tiles: Box<dyn TileSource>,
    types: Box<dyn LevelTypes>,
    /// The same level-type state as `types`, for the world state's
    /// preset lookups (population reads the rooms the DRLG generated).
    shared: SharedTypes,
    /// (act, init seed, town level id) of each created act.
    acts: Vec<(u8, u32, u32)>,
}

impl LevelSource {
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
            shared: types.clone(),
            types: Box::new(types),
            acts: vec![
                (0, init_seed, 1),
                (1, init_seed, ACT2_TOWN),
                (2, init_seed, KURAST_DOCKS),
                (4, init_seed, ACT5_TOWN),
                (3, init_seed, PANDEMONIUM_FORTRESS),
            ],
        }
    }
}

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
    let GameData::Live(d) = data;
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

/// The `objects.txt` rows of the client object update
/// (`world/objects-client.md` §28 r1): the live table **after its load
/// fix-up** (`data/fixups.md` §13 r2: `FrameCnt0`–`7` in 1/256 frames, as
/// §5's `End(m)` reads them). The raw `.bin` rows hold whole frames: a
/// one-frame mode would clamp at −255 and its frame (`+0x44 >> 8`) read
/// 0xFFFFFF (scene `a1-town-arrival-ama`: every frame dropped).
pub fn client_object_rows(data: &GameData) -> Vec<crate::bridge::objects::ObjClientRow> {
    let GameData::Live(d) = data;
    let fixed: Option<Vec<Objects>> = d
        .tables
        .fixed
        .table("objects")
        .and_then(|t| decode_all(t).ok());
    match fixed {
        Some(rows) => crate::bridge::objects::ObjClientRow::rows(&rows),
        // No fixed table: §13 r2 on the raw rows.
        None => crate::bridge::objects::ObjClientRow::rows(&d.waypoints.objects)
            .into_iter()
            .map(crate::bridge::objects::ObjClientRow::frame_counts_fixed)
            .collect(),
    }
}

/// The `objects.txt` `Name` of each class, for the mouse-over label
/// (`world_view::object_label`; d2rs-own, unverified, REC-239).
pub fn client_object_names(data: &GameData) -> Vec<String> {
    data.tables()
        .objects
        .iter()
        .map(|o| {
            let n = &o.name;
            let end = n.iter().position(|&b| b == 0).unwrap_or(n.len());
            String::from_utf8_lossy(&n[..end]).into_owned()
        })
        .collect()
}

/// The `Levels.txt` fields the client reads (`client/model.md` §11
/// rules 3–4: `Pal`, `Act`, `BlankScreen`; `audio/environment.md` §1 r2:
/// `SoundEnv`), one row per level id, from the game's `levels` table,
/// with the `leveldefs` ambient (`render/lighting.md` §3.1 r2).
pub fn client_level_rows(data: &GameData) -> Vec<LevelRow> {
    // `leveldefs` by level id (`render/lighting.md` §3.1 r2); a missing
    // table or row reads no colour (the environment applies).
    let GameData::Live(d) = data;
    let defs = d
        .tables
        .rows::<d2_data::tables::Leveldefs>()
        .unwrap_or_default();
    data.tables()
        .levels
        .iter()
        .enumerate()
        .map(|(n, l)| LevelRow {
            ambient: defs.get(n).map_or_else(Default::default, |d| {
                crate::rules::lighting::environment::Ambient {
                    i: d.intensity,
                    r: d.red,
                    g: d.green,
                    b: d.blue,
                }
            }),
            critters: crate::bridge::world::Critters {
                cmon: [l.cmon1, l.cmon2, l.cmon3, l.cmon4].map(|c| c as i16),
                cpct: [l.cpct1, l.cpct2, l.cpct3, l.cpct4].map(|c| c as i16),
                camt: [l.camt4, 0, 0, 0],
            },
            pal: l.pal,
            act: l.act,
            blank_screen: l.blankscreen != 0,
            sound_env: l.soundenv,
            draw_edges: l.drawedges != 0,
            rain: l.rain != 0,
            mud: l.mud != 0,
        })
        .collect()
}

/// The levels' waypoint indexes (`levels` `Waypoint`,
/// `world/waypoints.md` §1) for the client's waypoint menu.
pub fn client_waypoint_map(data: &GameData) -> d2_sim::world::waypoints::WaypointMap {
    d2_sim::world::waypoints::WaypointMap::new(&data.tables().levels)
}

/// The `levels` `LevelName` keys by level id (row index), the waypoint
/// rows' text keys ([`crate::ui::original::OriginalUi::set_level_names`]).
pub fn client_level_names(data: &GameData) -> Vec<String> {
    data.tables()
        .levels
        .iter()
        .map(|l| {
            let n = l
                .levelname
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(l.levelname.len());
            String::from_utf8_lossy(&l.levelname[..n]).into_owned()
        })
        .collect()
}

/// `difficultylevels` `ResistPenalty` per row (difficulty), from the
/// user's `.bin` set: the expansion resist penalty of the character
/// panel (`ui/panels.md` §8.9, `0x00611D30`; `panels-2.md` §24 r2), for
/// [`crate::ui::original::OriginalUi::set_resist_penalties`]. The field
/// is read as a signed value.
pub fn client_resist_penalties(archives: &dyn TableFiles) -> Result<Vec<i32>, BuildError> {
    let set =
        d2_data::bin::load_from(archives, "eng").map_err(|e| BuildError::Tables(e.to_string()))?;
    let table = set
        .table("difficultylevels")
        .ok_or_else(|| BuildError::Tables("difficultylevels not loaded".into()))?;
    let rows: Vec<Difficultylevels> =
        decode_all(table).map_err(|e| BuildError::Tables(e.to_string()))?;
    Ok(rows.iter().map(|r| r.resistpenalty as i32).collect())
}

/// The charstats walk / run speeds of `character`'s class
/// (`sim/pathing.md` §8.1 r2, §8.2) for the play preview's walk
/// prediction (decision D2); `None` for a class past the rows.
pub fn walk_speeds(
    data: &GameData,
    character: &Character,
) -> Result<Option<crate::bridge::predict::Speeds>, BuildError> {
    let GameData::Live(d) = data;
    let class = match character {
        Character::New => PLAYER_CLASS as u8,
        Character::Named(c) => c.class,
        Character::Save(save, _) => save.header.class,
    };
    let rows: Vec<d2_data::tables::Charstats> = d
        .tables
        .rows()
        .map_err(|e| BuildError::Tables(e.to_string()))?;
    Ok(rows
        .get(usize::from(class))
        .map(|r| crate::bridge::predict::Speeds {
            walk: r.walkvelocity,
            run: r.runvelocity,
        }))
}

/// The `skills` fields the client skill list reads (`client/msg-skills.md`
/// Inputs: `anim`, `monanim`, `passivestate`; §9–§10: `enhanceable`,
/// `EType`, `skilldesc`, `srvdofunc`; `skills/levels.md` §1, §6:
/// `charclass`, `maxlvl`), one row per skill id, from the user's `skills`
/// table.
pub fn client_skill_rows(archives: &dyn TableFiles) -> Result<Vec<SkillRow>, BuildError> {
    let set =
        d2_data::bin::load_from(archives, "eng").map_err(|e| BuildError::Tables(e.to_string()))?;
    let table = set
        .table("skills")
        .ok_or_else(|| BuildError::Tables("skills not loaded".to_owned()))?;
    let rows: Vec<Skills> = decode_all(table).map_err(|e| BuildError::Tables(e.to_string()))?;
    Ok(rows.iter().map(super::skill_rest::skill_row).collect())
}

/// The client player update's animation lookup
/// ([`super::anim_names::ClientPlayerAnims`]): the user's `AnimData.d2`,
/// the art's token tables and the items tables.
pub fn client_player_anims(
    data: &GameData,
) -> Result<super::anim_names::ClientPlayerAnims, BuildError> {
    let GameData::Live(d) = data;
    let looks = crate::world_view::unit_assets::UnitLooks::live(d.archives.as_ref())
        .map_err(BuildError::Tables)?;
    let inv = InvTables::from_fixed(&d.tables.fixed)
        .map_err(|e| BuildError::Tables(format!("inventory tables: {e}")))?;
    Ok(super::anim_names::ClientPlayerAnims::new(
        Arc::new(looks),
        Arc::new(d.tables.anim.clone()),
        &inv,
    ))
}

/// Each class's `charstats` `Skill 1`…`Skill 10` (`client/msg-skills.md`
/// §2 rule 8), one entry per `charstats` row, from the user's table.
pub fn client_class_skills(archives: &dyn TableFiles) -> Result<Vec<[u16; 10]>, BuildError> {
    let set =
        d2_data::bin::load_from(archives, "eng").map_err(|e| BuildError::Tables(e.to_string()))?;
    let table = set
        .table("charstats")
        .ok_or_else(|| BuildError::Tables("charstats not loaded".to_owned()))?;
    let rows: Vec<Charstats> = decode_all(table).map_err(|e| BuildError::Tables(e.to_string()))?;
    Ok(rows.iter().map(super::skill_rest::class_skills).collect())
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
pub fn client_skill_tables(archives: &dyn TableFiles) -> Result<SkillTables, BuildError> {
    let set =
        d2_data::bin::load_from(archives, "eng").map_err(|e| BuildError::Tables(e.to_string()))?;
    SkillTables::from_bin(&set, d2_sim::skills::LEVEL_CAP_114D)
        .map_err(|e| BuildError::Tables(e.to_string()))
}

/// The unit-message rows of the client (`client/msg-units.md` §1.2 r7:
/// each `monstats` row's `MonStatsEx` link into `monstats2`, whose record
/// bytes +0x15… hold the component choice counts; §1.2 r4: the
/// `itemstatcost` send columns; §1.3 r3 and `client/model.md` §15 r1,
/// `render/lighting.md` OQ 11: `objects.txt` and the `shrines.txt`
/// codes), from the user's tables.
pub fn client_unit_rows(archives: &dyn TableFiles) -> Result<UnitRows, BuildError> {
    let set =
        d2_data::bin::load_from(archives, "eng").map_err(|e| BuildError::Tables(e.to_string()))?;
    let table = |name: &str| {
        set.table(name)
            .ok_or_else(|| BuildError::Tables(format!("{name} not loaded")))
    };
    let err = |e: d2_data::tables::WrongTable| BuildError::Tables(e.to_string());
    let monstats_table = table("monstats")?;
    let monstats: Vec<Monstats> = decode_all(monstats_table).map_err(err)?;
    let monstats2 = table("monstats2")?;
    let monstats2_rows: Vec<d2_data::tables::Monstats2> = decode_all(monstats2).map_err(err)?;
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
            c.no_aura = m.noaura;
            c.min_grp = m.mingrp;
            c.max_grp = m.maxgrp;
            c.in_town = m.intown;
            if let Some(x) = monstats2_rows.get(link as usize) {
                c.light = x.light;
                c.light_rgb = (x.light_r, x.light_g, x.light_b);
            }
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
    // `client/stat-lists.md` §3 r3, r6: `notondead`, `noclear` (+0x14 &
    // 0x80, & 0x10) and the colour call's columns.
    // `missiles/client.md` §C2–§C4: the client create's columns.
    let missiles: Vec<d2_data::tables::Missiles> = decode_all(table("missiles")?).map_err(err)?;
    let missiles = missiles
        .iter()
        .map(|m| crate::bridge::client_missiles::ClientMissileRow {
            vel: i32::from(m.vel),
            vel_lev: i32::from(m.vellev),
            max_vel: i32::from(m.maxvel),
            accel: m.accel as i16,
            range: m.range as i16,
            lev_range: m.levrange as i16,
            sub_loop: m.subloop,
            sub_start: m.substart,
            sub_stop: m.substop,
            activate: i32::from(m.activate),
            init_steps: m.initsteps,
            anim_len: m.animlen,
            anim_speed: m.animspeed,
            light: m.light,
            rgb: (m.red, m.green, m.blue),
            can_slow: m.canslow,
            pierce: m.pierce,
            last_collide: m.lastcollide,
            clt_do_func: m.pcltdofunc,
            loop_anim: m.loopanim != 0,
            flicker: m.flicker,
            collide_type: m.collidetype,
            always_explode: m.alwaysexplode != 0,
            explosion_missile: m.explosionmissile as i16,
            clt_hit_func: m.pclthitfunc as i16,
            clt_sub: [
                m.cltsubmissile1 as i16,
                m.cltsubmissile2 as i16,
                m.cltsubmissile3 as i16,
            ],
            clt_param: [
                m.cltparam1 as i32,
                m.cltparam2 as i32,
                m.cltparam3 as i32,
                m.cltparam4 as i32,
                m.cltparam5 as i32,
            ],
            clt_calc1: m.cltcalc1,
            town: m.town,
            clt_src_town: m.cltsrctown,
            size: m.size,
            rand_start: m.randstart as i32,
            prog_sound: m.progsound as i16,
            param: [m.param1 as i32, m.param2 as i32],
            client_col: m.clientcol != 0,
            collide_kill: m.collidekill != 0,
            collide_friend: m.collidefriend != 0,
            next_hit: m.nexthit != 0,
            next_delay: m.nextdelay,
            can_destroy: m.candestroy,
            clt_hit_sub: [
                m.clthitsubmissile1 as i16,
                m.clthitsubmissile2 as i16,
                m.clthitsubmissile3 as i16,
                m.clthitsubmissile4 as i16,
            ],
            c_hit_par: [m.chitpar1 as i32, m.chitpar2 as i32, m.chitpar3 as i32],
            hit_sub1_server: m.hitsubmissile1 as i16,
            travel_sound: m.travelsound as i16,
            hit_sound: m.hitsound as i16,
            no_multishot: m.nomultishot,
        })
        .collect();
    let states: Vec<d2_data::tables::States> = decode_all(table("states")?).map_err(err)?;
    let states = states
        .iter()
        .map(|s| StateRow {
            dead_bit_only: s.notondead,
            keep_list: s.noclear,
            colorpri: s.colorpri,
            colorshift: s.colorshift,
            light_rgb: (s.light_r, s.light_g, s.light_b),
            meleeonly: s.meleeonly,
        })
        .collect();
    Ok(UnitRows {
        monsters,
        monster_skill_bonus,
        stats,
        objects,
        shrines: shrines.iter().map(|s| s.code).collect(),
        states,
        missiles,
    })
}

/// The animation columns of the client monster rows
/// ([`MonsterClass::anims`], `walk_speed`, `run_speed`): `AnimData.d2`
/// by the class's composite name per mode (the art's name rules,
/// [`super::anim_names::anim_key`]) and the fixed-up `monstats` speeds
/// (`data/fixups.md` §8), from the user's files.
pub fn client_monster_anims(
    archives: &crate::assets::game_files::GameFiles,
    rows: &mut UnitRows,
) -> Result<(), BuildError> {
    let anim = match archives.read_native(
        &CanonicalPath::new(d2_formats::animdata::PATH)
            .map_err(|e| BuildError::Tables(format!("{}: {e}", d2_formats::animdata::PATH)))?,
    ) {
        Some(Ok(NativeAsset::AnimData(a))) => a,
        Some(Ok(_)) => return Err(BuildError::Tables("AnimData.d2: wrong kind".into())),
        Some(Err(e)) => return Err(BuildError::Tables(format!("AnimData.d2: {e}"))),
        None => return Err(BuildError::Tables("AnimData.d2: in no archive".into())),
    };
    let bins = d2_data::bin::load_from(archives, d2_data::bin::DEFAULT_LANGUAGE)
        .map_err(|e| BuildError::Tables(e.to_string()))?;
    let tables = GameTables::from_loaded(bins, anim.clone())?;
    let monstats = tables
        .fixed
        .table("monstats")
        .ok_or_else(|| BuildError::Tables("monstats not loaded".into()))?;
    let looks =
        crate::world_view::unit_assets::UnitLooks::live(archives).map_err(BuildError::Tables)?;
    for (class, row) in rows.monsters.iter_mut().enumerate() {
        let Some(c) = row.as_mut() else {
            continue;
        };
        if class < monstats.count {
            let r = monstats.record(class);
            c.walk_speed = u16::from_le_bytes([r[0x36], r[0x37]]);
            c.run_speed = u16::from_le_bytes([r[0x38], r[0x39]]);
        }
        for (mode, a) in c.anims.iter_mut().enumerate() {
            let key = super::anim_names::anim_key(
                &looks,
                d2_sim::units::UnitType::Monster,
                class as u32,
                mode as u32,
                0,
            );
            *a = match key {
                Some(k) => anim.record(&k).ok().map(|rec| (rec.frames, rec.speed)),
                None => None,
            };
        }
    }
    Ok(())
}

/// A built game and the units the app and tests address. The player
/// exists only after the join (C→S 0x6B): [`local_player`].
pub struct LocalGame {
    pub sim: Sim,
}

/// The tables game creation and the wired host read beyond the DRLG's.
struct GameParts {
    action: ActionTables,
    stats: StatData,
    units: UnitData,
    world: WorldTables,
    objects: ObjectTables,
    monstats: Vec<Monstats>,
    hire_rows: Vec<HireRow>,
    items: ItemTables,
    vendors: VendorTables,
    anim: Option<Arc<AnimData>>,
    monster_sequences: Option<Arc<d2_sim::skills::sequences::MonsterSequences>>,
    vitals: Option<Arc<VitalsTables>>,
    /// The skill bodies' table data (`ActionHooks::bodies`: pet types,
    /// state groups); `None`: synthetic.
    bodies: Option<Arc<d2_sim::skills::use_::bodies::BodyTables>>,
    /// The chest drop's tables; `None`: no drop (synthetic).
    drops: Option<Arc<DropTables>>,
    /// `None`: the mercenary calls report no tables (synthetic).
    hirelings: Option<HirelingTables>,
    /// The inventory tables of the wired host's inventory model (the new
    /// character's start items, `items/generation.md` §10.3); none for
    /// synthetic data (the start items then stay unapplied).
    inventory: Option<InvTables>,
    /// The Horadric Cube's recipes and item columns; none for synthetic
    /// data (the cube then stays a stub).
    cube: Option<CubeData>,
}

impl GameParts {
    /// The user's tables (`GameTables`), the drop and hireling tables of
    /// [`LiveData`], the world state's level types over the live data.
    fn live(d: &LiveData) -> Result<Self, BuildError> {
        let t = &d.tables;
        Ok(GameParts {
            action: t.action_tables()?,
            stats: t.stat_data()?,
            units: t.unit_data(GAME_SETUP.expansion)?,
            world: t.world_tables()?,
            objects: t.object_tables()?,
            monstats: t.rows()?,
            hire_rows: t.hire_rows()?,
            items: t.item_tables()?,
            vendors: t.vendor_tables()?,
            anim: Some(Arc::new(t.anim.clone())),
            monster_sequences: Some(Arc::new(t.monster_sequences()?)),
            vitals: Some(Arc::new(t.vitals()?)),
            bodies: Some(Arc::new(t.body_tables()?)),
            drops: Some(d.drops.clone()),
            hirelings: Some(d.hirelings.clone()),
            inventory: Some(
                InvTables::from_fixed(&t.fixed)
                    .map_err(|e| BuildError::Tables(format!("inventory tables: {e}")))?,
            ),
            cube: Some(
                CubeData::from_fixed(&t.fixed)
                    .map_err(|e| BuildError::Tables(format!("cube tables: {e}")))?,
            ),
        })
    }
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
    let GameData::Live(d) = data;
    let wp_tables = data.tables();
    let (mut levels, parts) = (LevelSource::live(d, seed), GameParts::live(d)?);
    let shared_types = levels.shared.clone();
    let mut dungeon = Dungeon::default();
    for (act, init_seed, town) in levels.acts.iter().copied() {
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
    let seams = LocalSeams {
        monsters: super::monster_ai::MonsterAi::from_tables(
            &parts.action.combat.monstats,
            &parts.action.combat.monstats2,
        ),
        ..LocalSeams::default()
    };
    let mut hooks = ActionHooks::new(Arc::new(parts.action), world, Seed::init_low(seed), seams);
    hooks.anim_data = parts.anim;
    hooks.monster_sequences = parts.monster_sequences;
    // The server's animation names follow the client art's name rules.
    hooks.x.looks = crate::world_view::unit_assets::UnitLooks::live(d.archives.as_ref())
        .ok()
        .map(Arc::new);
    // Init function 17 of the waypoint objects (`waypoints.md` §5.1) on
    // the game's tables.
    hooks.waypoint_init = Some(Arc::new(WaypointData::new(
        &wp_tables.levels,
        &wp_tables.objects,
    )));
    hooks.vitals = parts.vitals;
    hooks.bodies = parts.bodies;
    // The hireling calls (save restore, join follow, act change;
    // `hirelings-2.md` §19) run on the wired host, which holds the
    // hireling lists when the game has `hireling.txt`; without the queue
    // a saved hireling is never restored (`hirelings.md` §10).
    if parts.hirelings.is_some() {
        hooks.hireling_calls = Some(Vec::new());
    }
    // The client vitals sync (`combat/vitals.md` §5.1): life, mana,
    // stamina and position sent to the client at the end of each tick.
    hooks.enable_vitals_sync();
    // d2rs-own, unverified: the preview allocates the death's corpse unit
    // itself (the inventory model has no corpse; PROVISIONAL REC-97).
    hooks.death.allocate_corpses = true;
    // Game entry places through the path provider; on before any unit is
    // allocated.
    hooks
        .enable_paths()
        .map_err(|e| BuildError::Setup(format!("path tables: {e:?}")))?;
    // The floor drop's walk-back field (`path-placement.md` §7.3): monster
    // and chest drops search their spot with it (`treasure.md` §7 step 2).
    if let Some(paths) = hooks.paths.as_mut() {
        paths.field = Some(d.expfield.clone());
    }
    // The inactive store (`units.md` §3.3–§3.4): a room the tick frees
    // keeps its units' records, and its next build restores them.
    hooks.enable_inactive_store();
    let info = GameInfo {
        expansion: GAME_SETUP.expansion,
        difficulty: character.difficulty(),
        game_type: GAME_TYPE,
        ladder: GAME_SETUP.ladder,
        ..GameInfo::default()
    };
    // One level-type state for the DRLG and population (`SharedTypes`):
    // the presets of a generated room are the ones population places
    // (q-fixture-migrate: a second, fresh state left every town without
    // its preset NPCs and objects on the user's files).
    let state = WorldState::new(shared_types, Arc::new(parts.world), info);
    let mut sim = WorldSim::new(Arc::new(parts.stats), parts.units, hooks, state);
    // Game creation (`rng.md` §5.2): the creation fields to their home,
    // then the four seeded controls in order, before any unit.
    let fields = GameFields {
        difficulty: character.difficulty(),
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
    let start_levels = [(0u8, ACT1_TOWN), (0, COLD_PLAINS), (1, ACT2_TOWN)];
    for (act, level) in start_levels {
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
        let _ = r;
    }
    // The Act II DRLG chose its staff tomb at creation (`levels.md` §3).
    if let Some(t) = sim
        .action
        .hooks()
        .drlg
        .with_act(1, &mut game.lists, |d, _| d.staff_tomb)
    {
        sim.action.hooks().x.staff_tomb = t;
    }
    let interact_classes: Vec<u16> = parts
        .monstats
        .iter()
        .enumerate()
        .filter(|(_, m)| m.npc && m.interact)
        .map(|(i, _)| i as u16)
        .collect();
    // The wired host on the created controls.
    let action = ActionWorld {
        waypoints: Some(WaypointData::new(&wp_tables.levels, &wp_tables.objects)),
        // The skill handlers (C→S 0x05–0x11, 0x3A–0x3C) on the action
        // wiring, their open seams on `LocalSeams` (`super::skill_rest`).
        skills: WiredSkills::default(),
        ..ActionWorld::default()
    };
    let rest = AppRest {
        expansion: GAME_SETUP.expansion,
        snap: sim.action.sys.hooks.x.snap.clone(),
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
    // Monster init does not embed the interaction lists yet: the host
    // registers the `interact` NPC units (d2rs-own, unverified).
    world.interact_classes = interact_classes;
    // The play host's inventory model (`play-server` seam, D1 preview
    // fills in `PreviewMoveRest`): the new character's start items.
    world.inventory = parts.inventory.map(preview_inv_parts);
    // The cube (d2rs-own, unverified, REC-119): the user's `cubemain`.
    world.cube = parts.cube.map(preview_cube_parts);
    // No extra start items: a new character gets the charstats slots only
    // (REC-244 settled: 1.14d gives an Amazon stub 8 start items, Wine
    // recording `--auto StubAma`, q-fix-real-start-cube).
    let mut s: Sim = SimGame::with_world(game, sim, world);
    s.announce_ground = true;
    s.set_host_sync(sync_seams);
    s.set_world_sync(super::weapons::sync);
    // The session sequence (`intents-events.md` §8) runs on the client's
    // C→S 0x67 / 0x6B: game creation (the client record, 0x01, 0x00,
    // 0x02; state 1), then the join (this loader, the player's add
    // messages, 0x0B, …, 0x03, 0x53, game entry with its room switch;
    // state 3). The next tick populates the town's rooms, the client's room
    // is ready and the client pass sends 0x04 (`tick.md` §6 rule 6).
    let cold_plains_wp = wp_tables.waypoint(COLD_PLAINS);
    s.set_session(SessionFlow::new(loader(character, cold_plains_wp)));
    Ok(LocalGame { sim: s })
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
        // A save's own class allocates the player: the request's class
        // byte is 0 for an existing character (REC-1130, `create_request_for`).
        let class = match &character {
            Character::Save(save, _) => save.header.class,
            _ => r.class,
        };
        let req = AllocRequest {
            ty: UnitType::Player,
            class: u32::from(class),
            room: None,
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: true,
        };
        let Some(player) = s.events.action.with(&mut s.game, |g, v| {
            // `units.md` §3.1 r4.1: the load draws the player's unit
            // seed (`0x00552DF0`) right after the allocation, before
            // the save's or the start items and the act's DRLG.
            let p = v.allocate(g, &req, 0, 0)?;
            v.init_player_seed(p);
            // `combat/hit.md` §7.1: a player is good (2), its state-105
            // list there before its first 0xAA (`intents-events.md`
            // §7.9 rule 1, recorded). PROVISIONAL (REC-732): the
            // original's call site in the join is not identified.
            v.set_alignment(g, p, 2);
            Some(p)
        }) else {
            s.events
                .action
                .hooks()
                .x
                .log
                .push(format!("join: allocating player class {class} failed"));
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
                        .get_mut(character.difficulty())
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
                // Then the start items (`items/generation.md` §10.3) on
                // the wired host. Their queued 0x9C / 0x9D (`items.sent`)
                // are the join's item messages (rule 3.5), sent after the
                // stat messages.
                let (entry, report, items) = load_new_character_with_items(s, player, r.char_name);
                super::save_gaps::seed_new_flags(
                    s,
                    player,
                    GAME_SETUP.expansion,
                    character.difficulty(),
                );
                let own: Vec<Vec<u8>> = items
                    .sent
                    .iter()
                    .filter(|(u, _)| *u == player)
                    .map(|(_, b)| b.clone())
                    .collect();
                if !own.is_empty() {
                    s.events
                        .action
                        .sys
                        .hooks
                        .session
                        .join_items
                        .insert(player, own);
                }
                // The synthetic game has an inventory model but no vitals
                // tables: its start items are not made (q-a4-quest-items).
                let has_inventory =
                    s.world.inventory.is_some() && s.events.action.hooks().vitals.is_some();
                let log = &mut s.events.action.hooks().x.log;
                log.extend(
                    report
                        .unapplied
                        .iter()
                        .map(|u| format!("join: new character: {u:?}")),
                );
                // Without an inventory model (synthetic data) the report's
                // "start items" step already names why nothing was made.
                if has_inventory {
                    log.extend(
                        items
                            .faults
                            .iter()
                            .map(|f| format!("join: new character: start items: {f}")),
                    );
                }
                (entry, PlayerQuests::default())
            }
            Character::Save(save, ctx) => match load_save(s, player, save, ctx) {
                Ok((mut entry, report)) => {
                    // The skill section's assigns turn the passive states on
                    // with their stat lists (`d2s-load.md` §2 "skills",
                    // before the items).
                    s.events.action.passive_refresh_all(&mut s.game, player);
                    // q-save-full: the save's items, made on the wired host.
                    let items_ok = super::save_full::join_items(s, player, save);
                    let corpses_ok = super::save_full::join_corpses(s, player, save);
                    // The saved hireling (`d2s.md` §1 load order: the
                    // player's items, the corpses, then the hireling,
                    // `hirelings.md` §10): its roomless allocation draws
                    // its unit seed before game entry populates the rooms
                    // (`hirelings-2.md` §16 rule 3), and the monster init
                    // of that allocation (`units.md` §3.1 step 7, its
                    // component and stat rolls) needs the lent world.
                    let (game, world) = (&mut s.game, &mut s.world);
                    s.events.lend_world(|a| world.hireling_calls(game, a));
                    super::save_gaps::join_gaps(s, player, save);
                    // `d2s.md` §2.4 rules 4–6: the hot keys, their item
                    // indices resolved over the loaded inventory list.
                    entry.hotkeys = super::save_gaps::loaded_hotkeys(
                        &save.header.hotkeys,
                        &s.world.item_guids(player),
                    );
                    // `d2s-load.md` §8 rules 1–2: a full save's record
                    // (the join's 0x5F and 0x23 pair, `intents-events.md`
                    // §8.2 rules 3.3, 3.7). A stub's is the loader's.
                    if entry.record.is_none() {
                        let portals = s.events.action.sys.hooks.drlg.data.portal_levels();
                        entry.record = Some(super::save_gaps::loaded_record(
                            &save.header.mouse[..2],
                            &s.world.item_guids(player),
                            initial_portal_flags(&portals),
                        ));
                    }
                    let log = &mut s.events.action.hooks().x.log;
                    log.extend(
                        report
                            .unapplied
                            .iter()
                            .filter(|u| !(items_ok && u.step == "items"))
                            .filter(|u| !(corpses_ok && u.step == "corpse"))
                            // Applied below, with the quest record.
                            .filter(|u| u.step != "npc fields")
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
                        // `d2s.md` §6 rule 1 (reader `0x0056A470`): fields A
                        // and B back into the player's NPC record.
                        quests.first_talk = body.npcs.a;
                        for (d, &b) in body.npcs.b.iter().enumerate() {
                            quests.set_intro_bits(d, b);
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
        // The quest entry `0x00546270` (`world/quests.md` §3: single player
        // takes `0x005344B0`, mode 0; a new character's stub load calls
        // mode 1 first): its messages (0x5E, 0x28, 0x29, 0x89) are the
        // join's rule 3.1 (e), sent after the loader's other messages.
        let new_character = matches!(character, Character::New | Character::Named(_))
            || matches!(&character, Character::Save(save, _) if save.body.is_none());
        quest_entry(s, player, new_character);
        // The point parser reads the staged position (`point_state`); the
        // tick moves it to the path's ([`WorldHost::unit_positions`]).
        s.set_unit(
            player,
            d2_server::adapters::UnitFacts {
                act: 0,
                pos: d2_server::seams::Pos { x: 0, y: 0 },
                owner: None,
            },
        );
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

/// Runs the join's quest entry on the host's quest control and queues its
/// messages for the join ([`d2_sim::wiring::action::switch::SessionState::join_quest`]).
/// An error (the original's fatal assert, for example a game without the
/// quest tables) is logged and the join goes on.
fn quest_entry(s: &mut Sim, player: UnitId, new_character: bool) {
    let modes: &[u8] = if new_character { &[1, 0] } else { &[0] };
    for &mode in modes {
        let r = WorldHost::quests(
            &mut s.world,
            &mut s.game,
            &mut s.events,
            QuestEnter { player, mode },
        );
        if let Some(Err(e)) = r {
            s.events
                .action
                .hooks()
                .x
                .log
                .push(format!("join: quest entry mode {mode}: {e}"));
        }
    }
    let sent: Vec<Vec<u8>> = s
        .world
        .rest
        .take_sent()
        .into_iter()
        .filter(|(u, _)| *u == player)
        .map(|(_, b)| b)
        .collect();
    if !sent.is_empty() {
        s.events
            .action
            .sys
            .hooks
            .session
            .join_quest
            .insert(player, sent);
    }
}

/// The local client's player and its GUID once the join has run (C→S
/// 0x6B drained), else `None`.
pub fn local_player(s: &Sim) -> Option<(UnitId, u32)> {
    let p = s.player_of(LOCAL_CLIENT)?;
    Some((p, s.game.lists.unit(p)?.guid))
}

/// The units of a started game, for the caller (the player exists after
/// the join: [`local_player`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    /// The store prices the host publishes for the shop panel.
    pub prices: crate::ui::original::ShopPrices,
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
            prices: g.sim.world.rest.prices.clone(),
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

    // Covers: specs/formats/d2s.md §2.2 r8
    #[test]
    fn the_map_seed_comes_from_the_switch_then_the_save() {
        let saved = |town: u8, difficulty: u8| {
            let mut s = d2_formats::d2s::D2s::new_stub(b"Seed", 1, 0x20, 0).unwrap();
            s.header.map_seed = 0x2468_ACE0;
            s.header.towns[usize::from(difficulty)] = town;
            Character::Save(
                Box::new(s),
                LoadContext {
                    difficulty,
                    map_seed_applies: false,
                },
            )
        };
        // The town byte's 0x80 for the game's difficulty restores it.
        assert_eq!(game_seed(&saved(0x80, 0), None), 0x2468_ACE0);
        assert_eq!(game_seed(&saved(0x82, 2), None), 0x2468_ACE0);
        // Without 0x80 (or on another difficulty): not restored.
        assert_eq!(game_seed(&saved(0x00, 0), None), DEFAULT_SEED);
        // `--seed N` (game +0x84 = 1) wins.
        assert_eq!(game_seed(&saved(0x80, 0), Some(7)), 7);
        assert_eq!(game_seed(&Character::New, None), DEFAULT_SEED);
        assert_eq!(game_seed(&Character::New, Some(9)), 9);
    }

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

#[cfg(test)]
mod target_search_tests {
    use super::*;

    /// An allied monster (a hireling) of size `size` at (100, 100) and one
    /// hostile monster at (x, 100).
    fn seams(x: i32, size: i32) -> LocalSeams {
        let mut s = LocalSeams::default();
        s.sides
            .insert(UnitId(1), (UnitType::Monster, true, (100, 100)));
        s.sides
            .insert(UnitId(2), (UnitType::Monster, false, (x, 100)));
        s.sizes.insert(UnitId(1), size);
        s
    }

    // Covers: specs/monsters/ai.md §5.3 r2, §6
    #[test]
    fn the_secondary_search_window_is_full_size_distance_below_49() {
        let mut g = Game::default();
        // Size 0: the full-size distance is the axis distance; 48 is
        // found, 49 is not.
        assert_eq!(
            seams(148, 0).secondary_target(&mut g, UnitId(1)),
            (Some(UnitId(2)), 48, false)
        );
        assert_eq!(
            seams(149, 0).secondary_target(&mut g, UnitId(1)),
            (None, 0x7FFF_FFFF, false)
        );
        // The scanner's size comes off each axis: 51 - 3 = 48.
        assert_eq!(
            seams(151, 3).secondary_target(&mut g, UnitId(1)).0,
            Some(UnitId(2))
        );
        assert_eq!(seams(152, 3).secondary_target(&mut g, UnitId(1)).0, None);
    }

    // Covers: specs/monsters/ai.md §5.2 r4
    #[test]
    fn the_good_main_search_stays_within_35() {
        let mut g = Game::default();
        assert_eq!(
            seams(134, 0).good_target_search(&mut g, UnitId(1), false),
            Some((UnitId(2), 34))
        );
        assert_eq!(
            seams(135, 0).good_target_search(&mut g, UnitId(1), false),
            None
        );
    }
}
