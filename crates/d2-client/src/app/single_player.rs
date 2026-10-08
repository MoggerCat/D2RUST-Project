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
    decode_all, Charstats, Difficultylevels, Itemstatcost, Levels, Monstats, Objects, Record,
    Shrines, Skills,
};
use d2_formats::animdata::AnimData;
use d2_formats::d2s::{self, D2s, ReadOptions};
use d2_formats::mpq::ArchiveSet;
use d2_native::source::NativeAsset;
use d2_server::adapters::character::LoadContext;
use d2_server::adapters::handlers::skills::wired::WiredSkills;
use d2_server::adapters::handlers::world::{
    preview_cube_parts, preview_inv_parts, ActionEvents, ActionWorld, Outbox, WiredWorld,
};
use d2_server::adapters::session::{load_new_character_with_items, load_save, GameSetup};
use d2_server::adapters::session_flow::{
    create_flags, CharacterLoader, CreateGame, Loaded, SessionFlow,
};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame};
use d2_server::host::Host;
use d2_server::host::SystemClock;
use d2_server::seams::{ClientId, Clock, PlayerGate};
use d2_server::world_data::game::GameTables;
use d2_server::world_data::tables::{drop_tables, hireling_tables, LevelTables, SaveData};
use d2_server::world_data::{self, Dt1Files, WorldFiles};
use d2_sim::combat::vitals::VitalsTables;
use d2_sim::combat::CombatTables;
use d2_sim::drlg::maze::Maze;
use d2_sim::drlg::room::LinkAt;
use d2_sim::drlg::{
    CellGrid, Drlg, DrlgData, DrlgError, DrlgRoomId, Dungeon, GridPass, LevelDef, LevelIdx,
    LevelTypes, PresetUnit, RoomGrids, RoomKind, TileInfo, TileRect, TileSource, WarpDef,
};
use d2_sim::game::Game;
use d2_sim::items::inventory::tables::InvTables;
use d2_sim::items::ItemTables;
use d2_sim::monsters::init::GameInfo;
use d2_sim::rng::Seed;
use d2_sim::skills::SkillTables;
use d2_sim::stats::StatData;
use d2_sim::units::hooks::{MonsterInfo, Sim as USim, UnitData};
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::warp_tile::HOST_MONSTER_PRESET;
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
use super::{
    synthetic_act2, synthetic_act4, synthetic_act5, synthetic_burial, synthetic_chains,
    synthetic_maze, synthetic_tower,
};
use crate::bridge::drlg::DrlgSource;
use crate::bridge::local::{LocalLink, PendingSession};
use crate::bridge::world::{
    LevelRow, MonsterClass, MonsterSetup, ObjectRow, SkillRow, StatSend, UnitRows,
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
/// level warp ([`BLOOD_MOOR_TO_DEN`] / [`DEN_TO_BLOOD_MOOR`]).
pub const DEN_OF_EVIL: u32 = 8;
/// The `lvlwarp` `Id` (and tile class) of the Blood Moor's cave entrance
/// and of the Den's way back (synthetic rows 0 and 1).
pub const BLOOD_MOOR_TO_DEN: u32 = 11;
pub const DEN_TO_BLOOD_MOOR: u32 = 12;
/// Sub-tile of a warp tile in its room (the 40 × 40 sub-tile room's
/// middle) and the synthetic walk-out.
pub const WARP_TILE_XY: i32 = 20;
pub const ACT2_TOWN: u32 = 40;
/// Harrogath (act 4, the fifth act; `levels` row 109).
pub const ACT5_TOWN: u32 = 109;
/// Catacombs Level 4, Andariel's lair (act 0; a flat level in the
/// synthetic world, reached by a level warp: d2rs-own, unverified).
pub const CATACOMBS_4: u32 = 37;
/// The default game seed.
pub const DEFAULT_SEED: u32 = 1234;
/// Game +0x6A of a single-player game: 3 (`rng.md` §5 open question,
/// answered: the client's create message carries 3, stored at +0x6A).
pub const GAME_TYPE: u8 = 3;
/// Sub-tile x and y of the waypoint object from the origin of the town's
/// first room (inside the synthetic 8 × 8-tile room, 40 sub-tiles square).
pub const WAYPOINT_X: i32 = 20;
/// The synthetic chest row's class, operate function and init function
/// (`object-functions.tsv`).
pub const SYNTHETIC_CHEST_CLASS: u32 = 1;
const SYNTHETIC_CHEST_OPERATE: u8 = 4;
const SYNTHETIC_CHEST_INIT: u8 = 3;
/// The synthetic town portal row's class, operate function and init
/// function (`object-functions.tsv`; REC-117).
pub const SYNTHETIC_PORTAL_CLASS: u32 = 59;
const SYNTHETIC_PORTAL_OPERATE: u8 = 15;
const SYNTHETIC_PORTAL_INIT: u8 = 11;
pub const UNIT_Y: i32 = 20;
/// Akara's x in the synthetic town room (sub-tiles from its origin).
pub const AKARA_X: i32 = 28;
/// Kashya's x in the synthetic town room (d2rs-own, unverified: she
/// stands in the Rogue Encampment, 5 sub-tiles from the player's start).
pub const KASHYA_X: i32 = 18;
/// d2rs-own, unverified (q-a2-town): Lut Gholein's NPCs in the synthetic
/// Act II town, Warriv (175), Atma, Drognan, Fara, Greiz, Jerhyn (201),
/// Elzix, Lysander and Meshif (210), in a row at [`ACT2_NPC_Y`], four
/// sub-tiles apart from [`ACT2_NPC_X0`].
pub const ACT2_NPCS: [u16; 9] = [
    d2_sim::world::npc::class::WARRIV2,
    d2_sim::world::npc::class::ATMA,
    d2_sim::world::npc::class::DROGNAN,
    d2_sim::world::npc::class::FARA,
    d2_sim::world::npc::class::GREIZ,
    201,
    d2_sim::world::npc::class::ELZIX,
    d2_sim::world::npc::class::LYSANDER,
    d2_sim::world::npc::class::MESHIF1,
];
pub const ACT2_NPC_X0: i32 = 3;
pub const ACT2_NPC_Y: i32 = 12;
/// The Act II town waypoint (sub-tiles from its room's origin).
pub const ACT2_WAYPOINT_XY: (i32, i32) = (20, 30);

/// The Harrogath waypoint (sub-tiles from its room's origin).
pub const ACT5_WAYPOINT_XY: (i32, i32) = (20, 30);
/// The Harrogath waypoint's index (`levels` `Waypoint`).
const ACT5_WAYPOINT: u8 = 35;
/// Kurast Docks's waypoint object (sub-tiles from its room's origin).
/// d2rs-own, unverified (q-act3-act5-gaps, REC-246).
pub const ACT3_WAYPOINT_XY: (i32, i32) = (20, 30);
/// The Act III and Act V waypoint levels of the synthetic chains
/// ([`synthetic_chains`]) with their `levels` `Waypoint` indexes
/// (`world/waypoints.tsv`; Kurast Docks 18 .. Durance of Hate Level 2 26,
/// Rigid Highlands 31 .. the Worldstone Keep Level 2 38, those the chains have;
/// Harrogath keeps the synthetic index 35 the Act IV portal lights).
/// d2rs-own, unverified (q-act3-act5-gaps, REC-246).
pub const CHAIN_WAYPOINTS: [(u32, u8); 16] = [
    (75, 18),
    (76, 19),
    (77, 20),
    (78, 21),
    (79, 22),
    (80, 23),
    (81, 24),
    (83, 25),
    (101, 26),
    (111, 31),
    (112, 32),
    (113, 33),
    (115, 34),
    (117, 36),
    (118, 37),
    (129, 38),
];

/// Every NPC class of the synthetic game: the Rogue Encampment's Akara,
/// Kashya and Warriv, Lut Gholein's and Harrogath's.
fn synthetic_npc_classes() -> impl Iterator<Item = u16> {
    [
        d2_sim::world::npc::class::AKARA,
        d2_sim::world::npc::class::KASHYA,
        d2_sim::world::npc::class::GHEED,
        d2_sim::world::npc::class::CHARSI,
        // Warriv (act 1): the act travel of Sisters to the Slaughter
        // (`docs/handoff/q-a1-andariel.md`).
        d2_sim::world::npc::class::WARRIV1,
    ]
    .into_iter()
    .chain(ACT2_NPCS)
    .chain(synthetic_act4::NPCS)
    .chain(super::town_npcs::ACT5.iter().map(|&(c, _)| c))
    .chain(super::town_npcs::ACT3.iter().map(|&(c, _)| c))
}
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
}

/// d2rs-own, unverified (preview, D1): the melee reach of every unit in
/// sub-tiles (`0x00622870` reads the unit's size and weapon; not
/// answered here).
const PREVIEW_MELEE_RANGE: i32 = 2;

/// The play host's seam refresh (`SimGame::set_host_sync`): the players
/// and monsters with their allied flag (`UnitLists`), for
/// [`LocalSeams::sides`].
pub fn sync_seams(game: &Game, sim: &mut WorldSim<LocalSeams>) {
    let classes: BTreeMap<UnitId, u32> = [UnitType::Player, UnitType::Monster]
        .into_iter()
        .flat_map(|ty| game.lists.units_of_type(ty))
        .filter_map(|u| Some((u, sim.action.sys.units.get(u)?.class)))
        .collect();
    let hooks = &mut sim.action.sys.hooks;
    // d2rs-own, unverified (q-assassin-gaps, REC-233): a listed pet is on the
    // player side (the summon's alignment effect, `0x005543B0`, is not wired).
    let pets: std::collections::BTreeSet<u32> = hooks
        .pet_lists
        .values()
        .flat_map(|l| l.entries.iter())
        .flat_map(|e| e.nodes.iter().map(|n| n.guid as u32))
        .collect();
    let mut sides = BTreeMap::new();
    for ty in [UnitType::Player, UnitType::Monster] {
        for u in game.lists.units_of_type(ty) {
            if let Some(e) = game.lists.unit(u) {
                let pet = ty == UnitType::Monster && pets.contains(&e.guid);
                sides.insert(u, (ty, e.allied || pet, hooks.path_position(u)));
            }
        }
    }
    hooks.x.sides = sides;
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
            (level == synthetic_act2::DURIELS_LAIR
                && !(self.lair_open && source == self.staff_tomb))
                // `0x0058D090` (`quests-act5-2.md` §7.8): leaving the
                // summit for 118 or 128 waits for the Ancients (d2rs-own,
                // unverified, REC-246: the made-up chain has both exits).
                || (source == super::synthetic_act5::SUMMIT
                    && matches!(level, 118 | 128)
                    && self.summit_closed),
        )
    }
    fn set_summit_open(&mut self, open: bool) {
        self.summit_closed = !open;
    }
    fn set_lair_open(&mut self, open: bool) {
        self.lair_open = open;
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
    fn golem_resummon(h: &mut ActionHooks<Self>, sim: &mut USim<'_>, player: UnitId) -> bool {
        skill_events::golem_resummon(h, sim, player)
    }
    // d2rs-own, unverified (q-amazon, REC-150): the hand class, the item
    // shoots / stack facts of the skill bodies ([`super::weapons`]).
    fn composit_weapon_class(&self, unit: UnitId) -> i32 {
        self.weapons.hand_class(unit)
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
    fn item_shoots(&self, item: UnitId) -> bool {
        self.weapons.facts(item).shoots
    }
    fn item_stackable(&self, item: UnitId) -> bool {
        self.weapons.facts(item).stackable
    }
    fn item_max_stack(&self, item: UnitId) -> i32 {
        self.weapons.facts(item).max_stack
    }
    fn anim_name(&self, _: UnitId, ty: UnitType, class: u32, mode: u32) -> Option<[u8; 8]> {
        super::anim_names::anim_key(self.looks.as_deref()?, ty, class, mode)
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
        let monster = self
            .sides
            .get(&unit)
            .is_some_and(|s| s.0 == UnitType::Monster);
        self.monsters.used_skill(unit, monster)
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
    // PROVISIONAL (world/objects.md §7.1 r3; REC-94): in range. The
    // preview client sends C→S 0x13 only on arrival
    // (`world_view/interact.rs`); the §7.3 r3–r4 approach is
    // `Pending::object_approach`'s default (operate).
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
    /// A host-placed monster of a level's preset list
    /// ([`HOST_MONSTER_PRESET`]): Blood Raven carries chain 2 (`init.md`
    /// §14.3), as her boss mods link it when population creates her.
    fn host_monster_created(&mut self, unit: UnitId, class: u32) {
        if class == synthetic_burial::BLOOD_RAVEN {
            self.monster_quest_chain(unit, synthetic_burial::CHAIN);
        }
        if class == synthetic_act4::IZUAL {
            self.monster_quest_chain(unit, synthetic_act4::IZUAL_CHAIN);
        }
    }
    /// Duriel acts from the world (q-a2-duriel-ai, REC-254): the Lair's
    /// population starts his AI as monster creation does. d2rs-own,
    /// unverified.
    fn host_monster_ai(&self, class: u32) -> bool {
        class == synthetic_act2::DURIEL_CLASS
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
            if id == ACT1_TOWN || id == BLOOD_MOOR {
                drlg.room_mut(r).flags |= d2_sim::drlg::room_flags::WARP_0;
            }
            // The cave entrance pair: Blood Moor slot 1 ↔ Den slot 0.
            if id == BLOOD_MOOR {
                drlg.room_mut(r).flags |= d2_sim::drlg::room_flags::WARP_0 << 1;
            }
            // The Black Marsh pair: Blood Moor slot 2 ↔ Black Marsh slot 0;
            // Black Marsh slot 1 ↔ the Tower (q-a1-tower).
            if id == BLOOD_MOOR {
                drlg.room_mut(r).flags |= d2_sim::drlg::room_flags::WARP_0 << 2;
            }
            if id == synthetic_tower::BLACK_MARSH {
                drlg.room_mut(r).flags |=
                    d2_sim::drlg::room_flags::WARP_0 | (d2_sim::drlg::room_flags::WARP_0 << 1);
            }
            // The Burial Grounds pair: Blood Moor slot 3 ↔ slot 0 there.
            if id == BLOOD_MOOR {
                drlg.room_mut(r).flags |= d2_sim::drlg::room_flags::WARP_0 << 3;
            }
            if id == synthetic_burial::BURIAL_GROUNDS {
                drlg.room_mut(r).flags |= d2_sim::drlg::room_flags::WARP_0;
            }
            // The Act IV line (q-a4): slot 0 back, slot 1 on.
            if let Some((back, on)) = synthetic_act4::links(id) {
                if back.is_some() {
                    drlg.room_mut(r).flags |= d2_sim::drlg::room_flags::WARP_0;
                }
                if on.is_some() {
                    drlg.room_mut(r).flags |= d2_sim::drlg::room_flags::WARP_0 << 1;
                }
            }
            // The remaining chains (q-levels-warps-all): slot 0 back, 1 on.
            for (slot, _, _) in synthetic_chains::slots(id) {
                drlg.room_mut(r).flags |= d2_sim::drlg::room_flags::WARP_0 << slot;
            }
            if id == DEN_OF_EVIL {
                drlg.room_mut(r).flags |= d2_sim::drlg::room_flags::WARP_0;
                // The stairs down to Cave Level 1 (slot 1, q-act1-dungeons).
                drlg.room_mut(r).flags |= d2_sim::drlg::room_flags::WARP_0 << 1;
            }
            drlg.link_room(r, LinkAt::Tail);
        }
        Ok(())
    }
    /// The warp tiles of the cave entrance pair (`path-placement.md`
    /// §12.1 rule 3 shape: unit type 5, class = the lvlwarp `Id`, room
    /// sub-tiles). d2rs-own, unverified.
    fn preset_units(&self, drlg: &Drlg, room: DrlgRoomId) -> Vec<PresetUnit> {
        use synthetic_burial as b;
        use synthetic_tower as t;
        let tile = |class, xy| PresetUnit {
            unit_type: 5,
            class,
            x: xy,
            y: xy,
        };
        let id = drlg.level(drlg.room(room).level).id;
        let mut v = match id {
            t::BLACK_MARSH => vec![
                tile(t::MARSH_TO_BLOOD_MOOR, t::MARSH_BACK_XY),
                tile(t::MARSH_TO_TOWER, t::MARSH_TOWER_XY),
            ],
            BLOOD_MOOR => vec![
                tile(BLOOD_MOOR_TO_DEN, WARP_TILE_XY),
                tile(t::BLOOD_MOOR_TO_MARSH, t::MOOR_MARSH_XY),
                tile(b::BLOOD_MOOR_TO_BURIAL, b::MOOR_BURIAL_XY),
            ],
            // Blood Raven, placed by the host (REC-130).
            b::BURIAL_GROUNDS => vec![
                PresetUnit {
                    unit_type: HOST_MONSTER_PRESET,
                    class: b::BLOOD_RAVEN,
                    x: b::RAVEN_XY,
                    y: b::RAVEN_XY,
                },
                tile(b::BURIAL_TO_BLOOD_MOOR, b::BACK_XY),
            ],
            id if synthetic_act4::index(id).is_some() => {
                use synthetic_act4 as a;
                let (back, on) = a::links(id).unwrap_or((None, None));
                let mut v = Vec::new();
                if id == a::PLAINS_OF_DESPAIR {
                    // Izual, placed by the host (q-a4).
                    v.push(PresetUnit {
                        unit_type: HOST_MONSTER_PRESET,
                        class: a::IZUAL,
                        x: a::IZUAL_XY,
                        y: a::IZUAL_XY,
                    });
                }
                v.extend(back.map(|c| tile(c, a::BACK_XY)));
                v.extend(on.map(|c| tile(c, a::ON_XY)));
                v
            }
            // The Arreat Summit's quest objects (q-act3-act5-gaps); its warp
            // tiles come from the tree below.
            synthetic_act5::SUMMIT => synthetic_act5::PRESET_OBJECTS
                .iter()
                .map(|&(class, xy)| PresetUnit {
                    unit_type: d2_sim::wiring::action::warp_tile::HOST_OBJECT_PRESET,
                    class,
                    x: xy.0,
                    y: xy.1,
                })
                .collect(),
            DEN_OF_EVIL => vec![
                tile(DEN_TO_BLOOD_MOOR, WARP_TILE_XY),
                tile(synthetic_maze::DEN_TO_CAVE, synthetic_maze::DEN_STAIRS_XY),
            ],
            _ => Vec::new(),
        };
        // The tree's tiles (q-levels-warps-all, q-a1-dungeons), also on
        // levels that have their own tiles above.
        v.extend(
            synthetic_chains::slots(id)
                .into_iter()
                .map(|(slot, _, class)| {
                    let (x, y) = synthetic_chains::tile_xy(slot);
                    PresetUnit {
                        unit_type: 5,
                        class,
                        x,
                        y,
                    }
                }),
        );
        v
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
            l.act = if i >= ACT5_TOWN as usize {
                4
            } else if i >= synthetic_act4::FORTRESS as usize {
                3
            } else if i >= 75 {
                2
            } else if i >= 40 {
                1
            } else {
                0
            };
        }
        levels[1].waypoint = 0;
        levels[COLD_PLAINS as usize].waypoint = 1;
        levels[STONY_FIELD as usize].waypoint = 2;
        levels[ACT2_TOWN as usize].waypoint = 9;
        levels[synthetic_act4::FORTRESS as usize].waypoint = 27;
        levels[ACT5_TOWN as usize].waypoint = ACT5_WAYPOINT;
        for (level, wp) in CHAIN_WAYPOINTS {
            levels[level as usize].waypoint = wp;
        }
        let mut o: Objects = blank();
        o.operatefn = 23;
        o.initfn = 17;
        o.framecnt1 = 15 << 8;
        // Class 1: a chest (`objects.md` §5.2, §8.1), placed only on
        // request ([`build_with_chests`]). d2rs-own, unverified.
        let mut chest: Objects = blank();
        chest.operatefn = SYNTHETIC_CHEST_OPERATE;
        chest.initfn = SYNTHETIC_CHEST_INIT;
        chest.framecnt1 = 15 << 8;
        // Class 59: the town portal (`objects.md` §12), padded rows
        // before it are blank. d2rs-own, unverified (REC-117).
        let mut portal: Objects = blank();
        portal.operatefn = SYNTHETIC_PORTAL_OPERATE;
        portal.initfn = SYNTHETIC_PORTAL_INIT;
        portal.framecnt1 = 15 << 8;
        portal.sizex = 1;
        portal.sizey = 1;
        // Class 60: the Moldy Tome of the Forgotten Tower quest
        // (`quests-act1.md` §10.7). d2rs-own, unverified (q-a1-tower).
        let mut tome: Objects = blank();
        tome.operatefn = synthetic_tower::TOME_OPERATE;
        tome.initfn = synthetic_tower::TOME_INIT;
        tome.framecnt1 = 15 << 8;
        let mut objects = vec![o, chest];
        objects.resize(SYNTHETIC_PORTAL_CLASS as usize, blank());
        objects.push(portal);
        debug_assert_eq!(objects.len(), synthetic_tower::TOME_CLASS as usize);
        objects.push(tome);
        // Class 100: Duriel's Lair entrance (a quest object, `quests-act2.md`
        // §8.8; no operate here, the way in is the warp tile), and class
        // 152: the orifice (operate 25, init 21). d2rs-own, unverified
        // (REC-167).
        for (class, operate, init) in [
            (synthetic_act2::LAIR_ENTRANCE_CLASS, 0, 0),
            (synthetic_act2::ORIFICE_CLASS, 25, 21),
            // Tyrael's door: init 38 (`quests-act2.md` §8.8), REC-234.
            (synthetic_act2::TYRAEL_DOOR_CLASS, 0, 38),
        ] {
            let c = class as usize;
            if objects.len() <= c {
                objects.resize(c + 1, blank());
            }
            let mut row: Objects = blank();
            row.operatefn = operate;
            row.initfn = init;
            row.framecnt1 = 15 << 8;
            objects[c] = row;
        }
        // The Act IV endgame objects (q-a4-endgame), by class.
        let last = synthetic_act4::OBJECT_ROWS.iter().map(|r| r.0).max();
        objects.resize(
            last.map_or(0, |c| c as usize + 1).max(objects.len()),
            blank(),
        );
        for &(class, operate, init) in &synthetic_act4::OBJECT_ROWS {
            let row = &mut objects[class as usize];
            row.operatefn = operate;
            row.initfn = init;
            row.framecnt1 = 20 << 8;
        }
        // The Act V quest objects (q-act3-act5-gaps), by class.
        let last = super::synthetic_act5::OBJECT_ROWS.iter().map(|r| r.0).max();
        objects.resize(
            last.map_or(0, |c| c as usize + 1).max(objects.len()),
            blank(),
        );
        for &(class, operate, init) in &super::synthetic_act5::OBJECT_ROWS {
            let row = &mut objects[class as usize];
            row.operatefn = operate;
            row.initfn = init;
            row.framecnt1 = 20 << 8;
        }
        // The Hellforge's animation: mode 1 ends in mode 2 (`objects-2.md`
        // §18.6 needs `Mode2`), frame counts 22 (`quests-act4.md` §1.4).
        // d2rs-own, unverified (q-a4-quest-items, REC-235).
        let forge = &mut objects[synthetic_act4::HELLFORGE as usize];
        forge.mode2 = 1;
        forge.hascollision2 = 1;
        forge.framecnt1 = 22 << 8;
        forge.framecnt3 = 22 << 8;
        WaypointTables {
            levels,
            objects,
            object_class: 0,
        }
    }

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
/// +0x84 = 1, `formats/d2s.md` §2.2 rule 8, `rng.md` §5.2). Synthetic
/// data has no save tables: an error.
pub fn load_character(
    data: &GameData,
    path: &std::path::Path,
    difficulty: u8,
) -> Result<Character, BuildError> {
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
                    GameFiles::archives(Arc::new(archives)),
                ))?)))
            }
            _ => Ok(GameData::Synthetic),
        }
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
    acts: Vec<(u8, u32, u32)>,
}

impl LevelSource {
    /// The bridge test's synthetic DRLG: one 8×8-tile floor room per
    /// level, no town generated at act creation, init seeds 1 and 2.
    fn synthetic() -> Self {
        LevelSource {
            data: Arc::new(synthetic_drlg_data()),
            tiles: Box::new(tiles()),
            types: Box::new(synthetic_level_types()),
            acts: vec![
                (0, 1, 0),
                (1, 2, 0),
                (2, 5, 0),
                (4, 3, 0),
                (synthetic_act4::ACT, 4, 0),
            ],
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
            acts: vec![
                (0, init_seed, 1),
                (1, init_seed, ACT2_TOWN),
                (2, init_seed, synthetic_chains::KURAST_DOCKS),
                (4, init_seed, ACT5_TOWN),
                (synthetic_act4::ACT, init_seed, synthetic_act4::FORTRESS),
            ],
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
    drlg.lvltypes = vec![
        vec![Vec::new(); 32],
        files.clone(),
        vec![Vec::new(); 32],
        files,
    ];
    for id in [
        ACT1_TOWN,
        BLOOD_MOOR,
        COLD_PLAINS,
        STONY_FIELD,
        synthetic_tower::BLACK_MARSH,
        synthetic_burial::BURIAL_GROUNDS,
        DEN_OF_EVIL,
        CATACOMBS_4,
        ACT2_TOWN,
        ACT5_TOWN,
    ]
    .into_iter()
    .chain(synthetic_act4::LEVELS)
    .chain(synthetic_chains::levels())
    {
        drlg.levels[id as usize].drlg_type = 2;
        drlg.levels[id as usize].level_type = 1;
    }
    // Cave Level 1: a maze level (q-act1-dungeons), warp pair with the
    // Den (Den slot 1, cave slot 0).
    {
        let c = &mut drlg.levels[synthetic_maze::CAVE_LEVEL_1 as usize];
        c.drlg_type = 1;
        // Level type 3 (cave): the maze generator's type (`maze.md` §1).
        c.level_type = 3;
        c.size = [(200, 200); 3];
        c.offset = (1500, 1000);
        c.vis[0] = DEN_OF_EVIL;
        c.warp[0] = synthetic_maze::CAVE_TO_DEN as i32;
    }
    drlg.levels[DEN_OF_EVIL as usize].vis[1] = synthetic_maze::CAVE_LEVEL_1;
    drlg.levels[DEN_OF_EVIL as usize].warp[1] = synthetic_maze::DEN_TO_CAVE as i32;
    // The town and the Blood Moor see each other through vis slot 0, a
    // border (warp −1, `drlg/rooms.md` §3.3): each one's room carries
    // flag WARP_0 ([`Types`]).
    drlg.levels[ACT1_TOWN as usize].vis[0] = BLOOD_MOOR;
    drlg.levels[BLOOD_MOOR as usize].vis[0] = ACT1_TOWN;
    // The cave entrance: a warp pair (lvlwarp rows 0 and 1) in slot 1 of
    // the Blood Moor and slot 0 of the Den (`rooms.md` §3.3).
    drlg.levels[BLOOD_MOOR as usize].vis[1] = DEN_OF_EVIL;
    drlg.levels[BLOOD_MOOR as usize].warp[1] = BLOOD_MOOR_TO_DEN as i32;
    drlg.levels[DEN_OF_EVIL as usize].vis[0] = BLOOD_MOOR;
    drlg.levels[DEN_OF_EVIL as usize].warp[0] = DEN_TO_BLOOD_MOOR as i32;
    // The Black Marsh and the Tower line (q-a1-tower): Blood Moor slot 2
    // ↔ Black Marsh slot 0, Black Marsh slot 1 ↔ Tower slot 0, then each
    // Tower level's slot 1 ↔ the next one's slot 0.
    {
        use synthetic_tower as t;
        let l = &mut drlg.levels;
        l[BLOOD_MOOR as usize].vis[2] = t::BLACK_MARSH;
        l[BLOOD_MOOR as usize].warp[2] = t::BLOOD_MOOR_TO_MARSH as i32;
        l[t::BLACK_MARSH as usize].vis[0] = BLOOD_MOOR;
        l[t::BLACK_MARSH as usize].warp[0] = t::MARSH_TO_BLOOD_MOOR as i32;
        l[t::BLACK_MARSH as usize].vis[1] = t::TOWER_LEVELS[0];
        l[t::BLACK_MARSH as usize].warp[1] = t::MARSH_TO_TOWER as i32;
        for (i, &id) in t::TOWER_LEVELS.iter().enumerate() {
            let c = &mut l[id as usize];
            c.drlg_type = 1;
            c.level_type = 3;
            c.size = [(200, 200); 3];
            c.offset = (2000 + 300 * i as i32, 1000);
            let (back, to) = if i == 0 {
                (t::BLACK_MARSH, t::TOWER_TO_MARSH)
            } else {
                (t::TOWER_LEVELS[i - 1], t::up(i - 1))
            };
            c.vis[0] = back;
            c.warp[0] = to as i32;
            if let Some(&next) = t::TOWER_LEVELS.get(i + 1) {
                c.vis[1] = next;
                c.warp[1] = t::down(i) as i32;
            }
        }
    }
    // The Burial Grounds (q-a1-bloodraven): Blood Moor slot 3 ↔ slot 0.
    {
        use synthetic_burial as b;
        let l = &mut drlg.levels;
        l[BLOOD_MOOR as usize].vis[3] = b::BURIAL_GROUNDS;
        l[BLOOD_MOOR as usize].warp[3] = b::BLOOD_MOOR_TO_BURIAL as i32;
        l[b::BURIAL_GROUNDS as usize].vis[0] = BLOOD_MOOR;
        l[b::BURIAL_GROUNDS as usize].warp[0] = b::BURIAL_TO_BLOOD_MOOR as i32;
    }
    // The Act IV line (q-a4): level i slot 1 ↔ level i + 1 slot 0.
    {
        use synthetic_act4 as a;
        for i in 0..a::LEVELS.len() - 1 {
            let (from, to) = (a::LEVELS[i] as usize, a::LEVELS[i + 1] as usize);
            drlg.levels[from].vis[1] = a::LEVELS[i + 1];
            drlg.levels[from].warp[1] = a::on(i) as i32;
            drlg.levels[to].vis[0] = a::LEVELS[i];
            drlg.levels[to].warp[0] = a::back(i) as i32;
        }
    }
    synthetic_act2::add_levels(&mut drlg);
    // The remaining chains (q-levels-warps-all): slot 0 back, slot 1 on.
    for e in synthetic_chains::edges() {
        drlg.levels[e.from as usize].vis[e.slot] = e.to;
        drlg.levels[e.from as usize].warp[e.slot] = e.on as i32;
        drlg.levels[e.to as usize].vis[0] = e.from;
        drlg.levels[e.to as usize].warp[0] = e.back as i32;
    }
    let mut ids = vec![
        BLOOD_MOOR_TO_DEN,
        DEN_TO_BLOOD_MOOR,
        synthetic_maze::DEN_TO_CAVE,
        synthetic_maze::CAVE_TO_DEN,
    ];
    ids.extend(synthetic_tower::BLOOD_MOOR_TO_MARSH..=synthetic_tower::LAST_WARP);
    ids.extend([
        synthetic_burial::BLOOD_MOOR_TO_BURIAL,
        synthetic_burial::BURIAL_TO_BLOOD_MOOR,
    ]);
    ids.extend(synthetic_act2::FIRST_WARP..=synthetic_act2::last_warp());
    ids.extend(synthetic_act4::first_warp()..=synthetic_act4::last_warp());
    ids.extend(synthetic_chains::first_warp()..=synthetic_chains::last_warp());
    drlg.warps = ids
        .iter()
        .map(|&id| WarpDef {
            id: id as i32,
            direction: b'b',
            ..WarpDef::default()
        })
        .collect();
    // ExitWalkX/Y per row: the walk-out after the arrival.
    drlg.warp_exits = ids
        .iter()
        .map(|&id| if id % 2 == 0 { (3, 3) } else { (0, 0) })
        .collect();
    drlg
}

/// Every warp of the synthetic game: (level, destination level, tile
/// class) per vis slot with a warp, in level and slot order. The level
/// holds a tile unit of that class that leads to the destination
/// (q-levels-warps-all).
pub fn synthetic_level_warps() -> Vec<(u32, u32, u32)> {
    let drlg = synthetic_drlg_data();
    let mut out = Vec::new();
    for (id, l) in drlg.levels.iter().enumerate() {
        for (vis, warp) in l.vis.iter().zip(l.warp.iter()) {
            if *warp >= 0 && *vis != 0 {
                out.push((id as u32, *vis, *warp as u32));
            }
        }
    }
    out
}

/// The synthetic level types: one 8×8-tile floor room in the Rogue
/// Encampment (the game entry's town, at tile (16, 0): levels of one act
/// do not overlap), one in the Blood Moor east of it (tile (24, 0), a
/// level border), one in Cold Plains and one in Lut Gholein.
fn synthetic_types() -> Types {
    let mut m = BTreeMap::from([
        (ACT1_TOWN, TileRect::new(16, 0, 8, 8)),
        (BLOOD_MOOR, TileRect::new(24, 0, 8, 8)),
        (COLD_PLAINS, TileRect::new(0, 0, 8, 8)),
        (STONY_FIELD, TileRect::new(0, 16, 8, 8)),
        (DEN_OF_EVIL, TileRect::new(0, 8, 8, 8)),
        (synthetic_tower::BLACK_MARSH, TileRect::new(8, 16, 8, 8)),
        (synthetic_burial::BURIAL_GROUNDS, TileRect::new(0, 24, 8, 8)),
        (ACT2_TOWN, TileRect::new(0, 0, 8, 8)),
        (synthetic_act4::FORTRESS, TileRect::new(0, 0, 8, 8)),
        (synthetic_act4::OUTER_STEPPES, TileRect::new(8, 0, 8, 8)),
        (
            synthetic_act4::PLAINS_OF_DESPAIR,
            TileRect::new(16, 0, 8, 8),
        ),
        (
            synthetic_act4::CITY_OF_THE_DAMNED,
            TileRect::new(24, 0, 8, 8),
        ),
        (synthetic_act4::RIVER_OF_FLAME, TileRect::new(32, 0, 8, 8)),
        // 40 × 40 tiles: the seal bosses stand up to 52 sub-tiles from
        // their seals (`quests-act4.md` §5.4; q-a4-endgame).
        (
            synthetic_act4::CHAOS_SANCTUARY,
            TileRect::new(40, 0, 40, 40),
        ),
        (ACT5_TOWN, TileRect::new(0, 0, 8, 8)),
    ]);
    // The remaining chains' levels (q-levels-warps-all) that have no
    // room yet.
    for id in synthetic_chains::levels() {
        if let (false, Some((x, y))) = (m.contains_key(&id), synthetic_chains::room_origin(id)) {
            m.insert(id, TileRect::new(x, y, 8, 8));
        }
    }
    Types(m)
}

/// The synthetic level types: the flat levels plus the maze level.
fn synthetic_level_types() -> synthetic_maze::SyntheticTypes<Types> {
    synthetic_maze::SyntheticTypes::new(synthetic_types(), Arc::new(synthetic_drlg_data()))
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
            types: Arc::new(|| Box::new(synthetic_level_types())),
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

/// The levels' waypoint indexes (`levels` `Waypoint`,
/// `world/waypoints.md` §1) for the client's waypoint menu.
pub fn client_waypoint_map(data: &GameData) -> d2_sim::world::waypoints::WaypointMap {
    d2_sim::world::waypoints::WaypointMap::new(&data.tables().levels)
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
/// prediction (decision D2); `None` on synthetic data (no charstats rows)
/// or a class past the rows.
pub fn walk_speeds(
    data: &GameData,
    character: &Character,
) -> Result<Option<crate::bridge::predict::Speeds>, BuildError> {
    let GameData::Live(d) = data else {
        return Ok(None);
    };
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

/// Rows of the synthetic `monstats` (classes 0 … 399; Akara is the only
/// NPC).
const SYNTHETIC_MONSTATS: usize = 600;

/// d2rs-own, unverified (preview): the synthetic game's `monstats`, all
/// zero rows with Akara `npc` and `interact` (the town NPC of
/// `docs/handoff/q-quests.md`).
fn synthetic_monstats() -> Vec<Monstats> {
    let mut v: Vec<Monstats> = (0..SYNTHETIC_MONSTATS)
        .map(|_| Monstats::decode(&vec![0u8; Monstats::SIZE]))
        .collect();
    for c in synthetic_npc_classes() {
        let a = &mut v[usize::from(c)];
        a.npc = true;
        a.interact = true;
    }
    // Blood Raven (REC-130): a killable class, so the kill parse runs.
    v[synthetic_burial::BLOOD_RAVEN as usize].killable = true;
    v[synthetic_act4::IZUAL as usize].killable = true;
    v[synthetic_act2::DURIEL_CLASS as usize].killable = true;
    // The Act IV endgame bosses (q-a4-endgame).
    for c in synthetic_act4::BOSSES {
        v[c as usize].killable = true;
    }
    v
}

/// d2rs-own, unverified (preview; REC-130): Kashya's `hireling` rows
/// (Rogue Scout, one per difficulty, version 100 = expansion), so her NPC
/// start has a hire list.
fn synthetic_hire_rows() -> Vec<HireRow> {
    (1..=3)
        .map(|difficulty| HireRow {
            version: 100,
            class: 271,
            act: 1,
            difficulty,
            seller: u32::from(d2_sim::world::npc::class::KASHYA),
            gold: 100,
            level: 1,
            name_first: 100,
            name_last: 104,
        })
        .chain(std::iter::once(HireRow {
            // Greiz's one row, so his hire list can be made (d2rs-own,
            // unverified, q-a2-town): the desert mercenary, names 2000..2002.
            version: 100,
            class: 271,
            act: 2,
            difficulty: 1,
            seller: u32::from(d2_sim::world::npc::class::GREIZ),
            gold: 0, // no gold stat row in the synthetic game (REC-157)
            level: 9,
            name_first: 2000,
            name_last: 2002,
        }))
        .collect()
}

/// The synthetic hire rows' mercenary classes (Kashya's and Greiz's 271,
/// Asheara's 357, Qual-Kehk's 560; made up).
const MERC_CLASSES: [usize; 3] = [271, 357, 560];

/// d2rs-own, unverified (preview): the client's monster rows for the
/// synthetic game, so Akara's S→C 0xAC creates her unit (a class without
/// a row is ignored, `client/msg-units.md` §1.2 r2).
pub fn synthetic_unit_rows() -> UnitRows {
    let raven = synthetic_burial::BLOOD_RAVEN as usize;
    let izual = synthetic_act4::IZUAL as usize;
    let top = synthetic_npc_classes()
        .map(usize::from)
        .max()
        .unwrap_or(0)
        .max(raven)
        .max(izual)
        .max(synthetic_act2::DURIEL_CLASS as usize)
        .max(MERC_CLASSES.into_iter().max().unwrap_or(0));
    let mut monsters = vec![None; top + 1];
    let class_row = |npc| MonsterClass {
        components: [0; 16],
        npc,
        interact: npc,
        setup: Some(crate::bridge::world::MonsterSetup {
            is_att: true,
            is_sel: true,
            ..Default::default()
        }),
    };
    for c in synthetic_npc_classes() {
        monsters[usize::from(c)] = Some(class_row(true));
    }
    monsters[raven] = Some(class_row(false));
    monsters[izual] = Some(class_row(false));
    monsters[synthetic_act2::DURIEL_CLASS as usize] = Some(class_row(false));
    // The hirable mercenaries (q-mercs-acts, REC-157): plain monster rows.
    for c in MERC_CLASSES {
        monsters[c] = Some(class_row(false));
    }
    UnitRows {
        monsters,
        ..UnitRows::default()
    }
}

impl GameParts {
    /// No game files: the waypoint rows, everything else empty; the world
    /// state's level types over the synthetic DRLG view with no preset,
    /// outdoor or maze data.
    fn synthetic(wp: &WaypointTables) -> Result<Self, BuildError> {
        let hire_rows: Vec<HireRow> = synthetic_hire_rows()
            .into_iter()
            .chain(super::town_npcs::synthetic_hire_rows())
            .collect();
        // The maze level's rows and DS1 (q-act1-dungeons).
        let world_types =
            SharedTypes::new(synthetic_maze::maze_types(Arc::new(synthetic_drlg_data())));
        let mut action = empty_action_tables();
        // The unit path needs the monster's `monstats` row (the shape).
        action.combat.monstats = synthetic_monstats();
        action.combat.charstats = synthetic_charstats();
        // The waypoint indexes of the levels (the quest-side
        // `0x005B4FF0` reads them; q-a4-harrogath).
        action.levels = wp.levels.clone();
        Ok(GameParts {
            action,
            stats: super::synthetic_items::stat_data(),
            units: UnitData {
                expansion: GAME_SETUP.expansion,
                monsters: vec![
                    MonsterInfo {
                        enabled: true,
                        aidel: [15; 3],
                        moves: 0,
                    };
                    SYNTHETIC_MONSTATS
                ],
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
                // The portal's quest gate reads a record per level
                // (`objects.md` §12 rule 7): blank, no gate (REC-117).
                leveldefs: vec![blank(); wp.levels.len()],
                ..ObjectTables::default()
            },
            monstats: synthetic_monstats(),
            hire_rows: hire_rows.clone(),
            items: super::synthetic_items::item_tables(),
            vendors: VendorTables::default(),
            anim: None,
            vitals: None,
            bodies: None,
            // The Hellforge's code drops (q-a4-quest-items, REC-235).
            drops: Some(Arc::new(super::synthetic_items::drop_tables())),
            hirelings: Some(super::merc_rows::synthetic_hireling_tables(&hire_rows)),
            inventory: Some(super::synthetic_items::inv_tables(
                &super::synthetic_items::item_tables(),
            )),
            cube: None,
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

/// d2rs-own, unverified: one `charstats` row per player class with the
/// walk / run velocities (6 / 9, the speeds of the preview's prediction),
/// so the synthetic server moves the player (a zero velocity never
/// does: the NPC approach walks, `docs/handoff/q-npc-approach.md`).
fn synthetic_charstats() -> Vec<d2_data::tables::Charstats> {
    use d2_data::tables::Record;
    let mut raw = vec![0u8; d2_data::tables::Charstats::SIZE];
    raw[64] = 6;
    raw[65] = 9;
    (0..7)
        .map(|_| d2_data::tables::Charstats::decode(&raw))
        .collect()
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
    build_with_chests(data, seed, character, &[])
}

/// [`build_with`] plus a synthetic chest (`SYNTHETIC_CHEST_CLASS`) in the
/// town's first room at each sub-tile offset from the room origin
/// (synthetic data only; the end-to-end tests of world objects).
pub fn build_with_chests(
    data: &GameData,
    seed: u32,
    character: Character,
    chests: &[(i32, i32)],
) -> Result<LocalGame, BuildError> {
    build_with_objects(data, seed, character, chests, None)
}

/// The stash object's class (`objects.txt` row 267, `world/objects.md`
/// §16.10 `BANK_CLASS`) and its operate function (32, the bank).
pub const STASH_CLASS: u32 = 267;
const STASH_OPERATE: u8 = 32;

/// [`build_with_chests`] plus, for synthetic data, a stash object
/// ([`STASH_CLASS`]) in the town's first room at the sub-tile offset
/// `stash` (the synthetic `objects` table is padded to the stash row).
/// d2rs-own, unverified: the end-to-end tests of the stash.
pub fn build_with_objects(
    data: &GameData,
    seed: u32,
    character: Character,
    chests: &[(i32, i32)],
    stash: Option<(i32, i32)>,
) -> Result<LocalGame, BuildError> {
    build_with_town(
        data,
        seed,
        character,
        chests,
        stash,
        &super::town_npcs::ACT1,
    )
}

/// [`build_with_objects`] with the synthetic town's NPCs given as (class,
/// sub-tile offset from the room origin) pairs (`super::town_npcs`;
/// synthetic data only, the live town's NPCs come from its presets).
pub fn build_with_town(
    data: &GameData,
    seed: u32,
    character: Character,
    chests: &[(i32, i32)],
    stash: Option<(i32, i32)>,
    npcs: &[(u16, i32)],
) -> Result<LocalGame, BuildError> {
    let mut wp_tables = data.tables();
    if stash.is_some() && matches!(data, GameData::Synthetic) {
        wp_tables
            .objects
            .resize(STASH_CLASS as usize + 1, blank::<Objects>());
        let row = &mut wp_tables.objects[STASH_CLASS as usize];
        row.operatefn = STASH_OPERATE;
        row.framecnt1 = 15 << 8;
    }
    let (mut levels, parts) = match data {
        GameData::Synthetic => (LevelSource::synthetic(), GameParts::synthetic(&wp_tables)?),
        GameData::Live(d) => (LevelSource::live(d, seed), GameParts::live(d)?),
    };
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
    if let GameData::Live(d) = data {
        // The server's animation names follow the client art's name rules.
        hooks.x.looks = crate::world_view::unit_assets::UnitLooks::live(d.archives.as_ref())
            .ok()
            .map(Arc::new);
    }
    hooks.vitals = parts.vitals;
    hooks.bodies = parts.bodies;
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
    let info = GameInfo {
        expansion: GAME_SETUP.expansion,
        difficulty: character.difficulty(),
        game_type: GAME_TYPE,
        ladder: GAME_SETUP.ladder,
        ..GameInfo::default()
    };
    let state = WorldState::new(parts.world_types, Arc::new(parts.world), info);
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
    let mut rooms = Vec::new();
    let mut start_levels = vec![(0u8, ACT1_TOWN), (0, COLD_PLAINS), (1, ACT2_TOWN)];
    // The Moldy Tome stands in the Black Marsh (synthetic data only;
    // q-a1-tower, d2rs-own, unverified).
    if matches!(data, GameData::Synthetic) {
        start_levels.push((0, synthetic_tower::BLACK_MARSH));
        // Harrogath's room (rooms[4]; q-a5-town).
        start_levels.push((4, ACT5_TOWN));
        // The Pandemonium Fortress's room (rooms[5]; q-a4).
        start_levels.push((synthetic_act4::ACT, synthetic_act4::FORTRESS));
        // Kurast Docks's room (q-levels-warps-all).
        start_levels.push((2, synthetic_chains::KURAST_DOCKS));
    }
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
        rooms.push(r);
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
    for &(dx, dy) in chests {
        let chest = AllocRequest {
            class: SYNTHETIC_CHEST_CLASS,
            mode: 0,
            ..req
        };
        sim.action
            .with(&mut game, |g, v| v.allocate(g, &chest, ox + dx, oy + dy))
            .ok_or_else(|| BuildError::Setup("allocating a chest failed".into()))?;
    }
    if let Some((dx, dy)) = stash {
        let req = AllocRequest {
            class: STASH_CLASS,
            mode: 0,
            ..req
        };
        sim.action
            .with(&mut game, |g, v| v.allocate(g, &req, ox + dx, oy + dy))
            .ok_or_else(|| BuildError::Setup("allocating the stash failed".into()))?;
    }
    if let Some(&(marsh, rect)) = rooms.get(3) {
        let tome = AllocRequest {
            class: synthetic_tower::TOME_CLASS,
            room: Some(marsh),
            mode: 0,
            ..req
        };
        let (tx, ty) = synthetic_tower::TOME_XY;
        sim.action
            .with(&mut game, |g, v| {
                v.allocate(g, &tome, rect.x * 5 + tx, rect.y * 5 + ty)
            })
            .ok_or_else(|| BuildError::Setup("allocating the Moldy Tome failed".into()))?;
    }
    let waypoint_guid = game
        .lists
        .unit(waypoint)
        .ok_or_else(|| BuildError::Setup("waypoint unit missing".into()))?
        .guid;
    // The synthetic game's town NPCs (d2rs-own, unverified; the live
    // game's NPCs come from the town presets).
    if matches!(data, GameData::Synthetic) {
        for &(class, dx) in npcs {
            let req = AllocRequest {
                ty: UnitType::Monster,
                class: u32::from(class),
                room: Some(room0),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: true,
            };
            sim.action
                .with(&mut game, |g, v| v.allocate(g, &req, ox + dx, oy + UNIT_Y))
                .ok_or_else(|| BuildError::Setup(format!("allocating NPC {class} failed")))?;
        }
    }
    // Lut Gholein: its NPCs and its waypoint (act 1's room, d2rs-own,
    // unverified, q-a2-town).
    if matches!(data, GameData::Synthetic) {
        let (room2, rect2) = rooms[2];
        let (ox2, oy2) = (rect2.x * 5, rect2.y * 5);
        for (i, &class) in ACT2_NPCS.iter().enumerate() {
            let req = AllocRequest {
                ty: UnitType::Monster,
                class: u32::from(class),
                room: Some(room2),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: true,
            };
            sim.action
                .with(&mut game, |g, v| {
                    v.allocate(g, &req, ox2 + ACT2_NPC_X0 + 4 * i as i32, oy2 + ACT2_NPC_Y)
                })
                .ok_or_else(|| BuildError::Setup(format!("allocating NPC {class} failed")))?;
        }
        let req = AllocRequest {
            ty: UnitType::Object,
            class: wp_tables.object_class,
            room: Some(room2),
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: false,
        };
        sim.action
            .with(&mut game, |g, v| {
                v.allocate(g, &req, ox2 + ACT2_WAYPOINT_XY.0, oy2 + ACT2_WAYPOINT_XY.1)
            })
            .ok_or_else(|| BuildError::Setup("allocating the Act II waypoint failed".into()))?;
    }
    // Harrogath: its NPCs and its waypoint (act 4's room, d2rs-own,
    // unverified, q-a5-town, REC-144).
    if matches!(data, GameData::Synthetic) {
        let (room5, rect5) = rooms[4];
        let (ox5, oy5) = (rect5.x * 5, rect5.y * 5);
        for &(class, dx) in &super::town_npcs::ACT5 {
            let req = AllocRequest {
                ty: UnitType::Monster,
                class: u32::from(class),
                room: Some(room5),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: true,
            };
            sim.action
                .with(&mut game, |g, v| {
                    v.allocate(g, &req, ox5 + dx, oy5 + UNIT_Y)
                })
                .ok_or_else(|| BuildError::Setup(format!("allocating NPC {class} failed")))?;
        }
        let req = AllocRequest {
            ty: UnitType::Object,
            class: wp_tables.object_class,
            room: Some(room5),
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: false,
        };
        sim.action
            .with(&mut game, |g, v| {
                v.allocate(g, &req, ox5 + ACT5_WAYPOINT_XY.0, oy5 + ACT5_WAYPOINT_XY.1)
            })
            .ok_or_else(|| BuildError::Setup("allocating the Act V waypoint failed".into()))?;
    }
    // Pandemonium Fortress: its NPCs and its waypoint (rooms[5], d2rs-own,
    // unverified, q-a4, REC-143).
    if matches!(data, GameData::Synthetic) {
        let (room6, rect6) = rooms[5];
        let (ox6, oy6) = (rect6.x * 5, rect6.y * 5);
        for (i, &class) in synthetic_act4::NPCS.iter().enumerate() {
            let req = AllocRequest {
                ty: UnitType::Monster,
                class: u32::from(class),
                room: Some(room6),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: true,
            };
            let x = ox6 + synthetic_act4::NPC_X0 + synthetic_act4::NPC_STEP * i as i32;
            sim.action
                .with(&mut game, |g, v| {
                    v.allocate(g, &req, x, oy6 + synthetic_act4::NPC_Y)
                })
                .ok_or_else(|| BuildError::Setup(format!("allocating NPC {class} failed")))?;
        }
        let req = AllocRequest {
            ty: UnitType::Object,
            class: wp_tables.object_class,
            room: Some(room6),
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: false,
        };
        let (wx, wy) = synthetic_act4::WAYPOINT_XY;
        sim.action
            .with(&mut game, |g, v| v.allocate(g, &req, ox6 + wx, oy6 + wy))
            .ok_or_else(|| BuildError::Setup("allocating the Act IV waypoint failed".into()))?;
    }
    // Kurast Docks's waypoint (rooms[6], d2rs-own, unverified,
    // q-act3-act5-gaps, REC-246).
    if matches!(data, GameData::Synthetic) {
        if let Some(&(room3, rect3)) = rooms.get(6) {
            let req = AllocRequest {
                ty: UnitType::Object,
                class: wp_tables.object_class,
                room: Some(room3),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: false,
            };
            let (wx, wy) = ACT3_WAYPOINT_XY;
            sim.action
                .with(&mut game, |g, v| {
                    v.allocate(g, &req, rect3.x * 5 + wx, rect3.y * 5 + wy)
                })
                .ok_or_else(|| {
                    BuildError::Setup("allocating the Act III waypoint failed".into())
                })?;
        }
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
    // A new character carries the Horadric Cube (d2rs-own, unverified,
    // REC-244): charstats gives none, and the preview has no Act II quest
    // reward path yet.
    world.start_extra = vec![*b"box "];
    let mut s: Sim = SimGame::with_world(game, sim, world);
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
                super::save_gaps::seed_new_flags(s, player, GAME_SETUP.expansion);
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
                Ok((entry, report)) => {
                    // q-save-full: the save's items, made on the wired host.
                    let items_ok = super::save_full::join_items(s, player, save);
                    super::save_gaps::join_gaps(s, player, save);
                    let log = &mut s.events.action.hooks().x.log;
                    log.extend(
                        report
                            .unapplied
                            .iter()
                            .filter(|u| !(items_ok && u.step == "items"))
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
    pub waypoint: UnitId,
    pub waypoint_guid: u32,
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
    start_with_chests(data, seed, character, clock, Vec::new())
}

/// [`start_with`] with synthetic chests ([`build_with_chests`]).
pub fn start_with_chests<C: Clock + Send + 'static>(
    data: GameData,
    seed: u32,
    character: Character,
    clock: C,
    chests: Vec<(i32, i32)>,
) -> Result<(ThreadLink<Link<C>>, Started), BuildError> {
    start_with_objects(data, seed, character, clock, chests, None)
}

/// [`start_with_chests`] plus a stash ([`build_with_objects`]).
pub fn start_with_objects<C: Clock + Send + 'static>(
    data: GameData,
    seed: u32,
    character: Character,
    clock: C,
    chests: Vec<(i32, i32)>,
    stash: Option<(i32, i32)>,
) -> Result<(ThreadLink<Link<C>>, Started), BuildError> {
    start_with_town(
        data,
        seed,
        character,
        clock,
        chests,
        stash,
        super::town_npcs::ACT1.to_vec(),
    )
}

/// [`start_with_objects`] with the synthetic town's NPCs given
/// ([`build_with_town`]).
pub fn start_with_town<C: Clock + Send + 'static>(
    data: GameData,
    seed: u32,
    character: Character,
    clock: C,
    chests: Vec<(i32, i32)>,
    stash: Option<(i32, i32)>,
    npcs: Vec<(u16, i32)>,
) -> Result<(ThreadLink<Link<C>>, Started), BuildError> {
    let (tx, rx) = std::sync::mpsc::channel();
    let link = ThreadLink::spawn(move || {
        let g = build_with_town(&data, seed, character, &chests, stash, &npcs)?;
        let _ = tx.send(Started {
            waypoint: g.waypoint,
            waypoint_guid: g.waypoint_guid,
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
