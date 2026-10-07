// Spec: specs/world/npc.md
//! NPC interaction: NPC records and the NPC-control seed (§1), talk and
//! chat (§2, §3), menu actions (§4), healing (§5), Cain identify (§6),
//! mercenaries (§7, [`hire`]), NPC services (§8, [`services`]) and the
//! S→C 0x2A / 0x58 / 0x4E / 0x9B messages (§9).
//!
//! Everything outside this spec is reached through two seams:
//! [`NpcWorld`] (game, units, stats, items, quests, messages) and
//! [`NpcVendors`] (store, gamble, prices and repair: `world/vendors.md`,
//! implemented by `world::vendors` in a parallel session; the provider
//! wires that module into this trait).

use d2_data::bin::BinTable;
use d2_data::tables::{Hireling, Monstats, Record};

use crate::rng::Seed;
use crate::units::UnitId;
use crate::world::quests::{QuestFlags, TextList};
use crate::world::{tsv_num, tsv_rows, TsvError};

pub mod hire;
pub mod services;

#[cfg(test)]
mod tests;

pub use hire::{HireList, HireOffer, HireRow, HireSlot, MercInit};
pub use services::{ImbueMods, InvEntry, ItemFacts, Place};

/// Monstats classes named by the spec.
pub mod class {
    pub const CAIN1: u16 = 146;
    pub const GHEED: u16 = 147;
    pub const AKARA: u16 = 148;
    pub const KASHYA: u16 = 150;
    pub const CHARSI: u16 = 154;
    pub const WARRIV1: u16 = 155;
    pub const WARRIV2: u16 = 175;
    pub const ATMA: u16 = 176;
    pub const DROGNAN: u16 = 177;
    pub const FARA: u16 = 178;
    pub const GREIZ: u16 = 198;
    pub const ELZIX: u16 = 199;
    pub const LYSANDER: u16 = 202;
    pub const MESHIF1: u16 = 210;
    pub const CAIN2: u16 = 244;
    pub const CAIN3: u16 = 245;
    pub const CAIN4: u16 = 246;
    pub const ASHEARA: u16 = 252;
    pub const HRATLI: u16 = 253;
    pub const ALKOR: u16 = 254;
    pub const ORMUS: u16 = 255;
    pub const HALBU: u16 = 257;
    pub const MESHIF2: u16 = 264;
    pub const CAIN5: u16 = 265;
    pub const TYRAEL2: u16 = 367;
    pub const JAMELLA: u16 = 405;
    pub const LARZUK: u16 = 511;
    pub const DREHYA: u16 = 512;
    pub const MALAH: u16 = 513;
    pub const NIHLATHAK: u16 = 514;
    pub const QUAL_KEHK: u16 = 515;
    pub const CAIN6: u16 = 520;
}

use class::*;

/// §4 action 1: NPCs with a trade action.
pub const TRADERS: [u16; 16] = [
    GHEED, AKARA, CHARSI, DROGNAN, FARA, ELZIX, LYSANDER, ASHEARA, HRATLI, ALKOR, ORMUS, HALBU,
    JAMELLA, LARZUK, DREHYA, MALAH,
];
/// §4 action 2: NPCs with a gamble action.
pub const GAMBLERS: [u16; 6] = [GHEED, ELZIX, ALKOR, JAMELLA, DREHYA, NIHLATHAK];
/// §5: healers.
pub const HEALERS: [u16; 6] = [AKARA, ATMA, FARA, ORMUS, JAMELLA, MALAH];
/// §6 step 2: the Cains that identify (cain1 excluded).
pub const IDENTIFIERS: [u16; 5] = [CAIN2, CAIN3, CAIN4, CAIN5, CAIN6];
/// §7: mercenary sellers (hire list).
pub const SELLERS: [u16; 4] = [KASHYA, GREIZ, ASHEARA, QUAL_KEHK];
/// §7.4 step 1: NPCs that resurrect.
pub const RESURRECTORS: [u16; 5] = [KASHYA, GREIZ, ASHEARA, TYRAEL2, QUAL_KEHK];

/// §1.1 step 1: records allocated per game.
pub const RECORD_MAX: usize = 64;
/// §2: the distances.
pub const START_DISTANCE: i32 = 50;
pub const TALK_DISTANCE: i32 = 6;
pub const APPROACH_DISTANCE: i32 = 8;
/// §2 rule 2: the AI parameter passed to `0x0058EC00`.
pub const AI_PARAM: u32 = 0x28;
/// §5 step 6: the heal sound attached to the NPC.
pub const SOUND_HEAL: u16 = 10;
/// Unit type of a monster (interact unit type of an NPC).
pub const UNIT_MONSTER: u8 = 1;

/// Stat ids used here.
pub mod stat {
    pub const LIFE: u16 = 6;
    pub const MANA: u16 = 8;
    pub const STAMINA: u16 = 10;
    pub const LEVEL: u16 = 12;
    pub const GOLD: u16 = 14;
    pub const ITEM_194: u16 = 194;
}

/// Node states of an interaction list (§2 start rule 2).
pub mod talk {
    pub const TALKING: u8 = 0;
    pub const CHATTING: u8 = 1;
    pub const TRADING: u8 = 2;
}

/// S→C 0x2A result codes and kinds (§9).
pub mod code {
    pub const BOUGHT: u8 = 0;
    pub const SOLD: u8 = 1;
    pub const REPAIRED: u8 = 2;
    pub const IDENTIFIED: u8 = 3;
    pub const MERC: u8 = 5;
    pub const HEALED: u8 = 6;
    pub const BUY_REFUSED: u8 = 7;
    pub const REFUSED: u8 = 9;
    pub const NO_ROOM: u8 = 10;
    pub const GATE: u8 = 11;
    pub const NO_GOLD: u8 = 12;
    pub const NOTHING_TO_HEAL: u8 = 14;
    pub const NOT_PLACED: u8 = 15;
}

/// Fatal asserts of the original, returned instead of aborting.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NpcError {
    #[error("more than {RECORD_MAX} interact monstats rows")]
    TooManyRecords,
    #[error("no hireling row for seller {seller}, difficulty column {difficulty}")]
    NoHirelingRow { seller: u16, difficulty: u32 },
    #[error("hireling name range {first}..={last} does not fit the 69 hire slots")]
    HireListSize { first: u16, last: u16 },
    #[error("hireling table: {0}")]
    Table(String),
    #[error("vendors.tsv: {0}")]
    Tsv(#[from] TsvError),
}

// ------------------------------------------------------------ §1.2

/// `vendors.tsv` (shared with `vendors.md`), embedded: the NPC table
/// `0x00731184` and every NPC's roles.
pub const VENDORS_TSV: &str = include_str!("../../../../specs/world/vendors.tsv");

const VENDORS_HEADER: &[&str] = &[
    "npc",
    "name",
    "act",
    "trader",
    "force_vendor",
    "cache",
    "refresh_flag",
    "gamble_flag",
    "trade_action",
    "gamble_action",
    "heals",
    "identifies",
    "hire_list",
    "resurrects",
];

/// One NPC table entry (§1.2) with the role columns of `vendors.tsv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcTableRow {
    pub class: u16,
    pub name: String,
    pub act: u8,
    pub trader: u8,
    /// NPC table byte 6.
    pub byte6: u8,
    pub trade_action: bool,
    pub gamble_action: bool,
    pub heals: bool,
    pub identifies: bool,
    pub hire_list: bool,
    pub resurrects: bool,
}

/// Parses `vendors.tsv` strictly (METHODS M07).
pub fn parse_npc_table(text: &str) -> Result<Vec<NpcTableRow>, TsvError> {
    const T: &str = "vendors.tsv";
    let mut out = Vec::new();
    for (line, c) in tsv_rows(T, text, VENDORS_HEADER)? {
        let num = |i: usize, max: u32| -> Result<u32, TsvError> {
            let v = tsv_num(T, line, VENDORS_HEADER[i], c[i])?;
            if v > max {
                return Err(TsvError::Value {
                    table: T,
                    line,
                    column: VENDORS_HEADER[i],
                    value: c[i].to_string(),
                });
            }
            Ok(v)
        };
        out.push(NpcTableRow {
            class: num(0, 0xFFFF)? as u16,
            name: c[1].to_string(),
            act: num(2, 4)? as u8,
            trader: num(3, 1)? as u8,
            byte6: num(4, 1)? as u8,
            trade_action: num(8, 1)? == 1,
            gamble_action: num(9, 1)? == 1,
            heals: num(10, 1)? == 1,
            identifies: num(11, 1)? == 1,
            hire_list: num(12, 1)? == 1,
            resurrects: num(13, 1)? == 1,
        });
    }
    Ok(out)
}

// ------------------------------------------------------------ §1.1

/// One NPC record (§1.1). Only the fields this spec owns; the store
/// fields (+0x04 … +0x40 except +0x10, +0x21) belong to `vendors.md`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NpcRecord {
    /// +0x00 monstats class.
    pub class: u16,
    /// +0x10 hire list (§7.1).
    pub hire: Option<HireList>,
    /// +0x21 hire list made.
    pub hire_made: bool,
    /// +0x22 act 0–4.
    pub act: u8,
    /// +0x23 trader.
    pub trader: u8,
    /// +0x26 NPC table byte 6.
    pub byte6: u8,
}

/// The NPC control block (game +0x1D24) with its records, plus the
/// table data this module reads.
#[derive(Debug, Clone)]
pub struct NpcControl {
    /// The records in `monstats` row order (count = +0x00 = +0x10).
    pub records: Vec<NpcRecord>,
    /// +0x08 NPC-control seed.
    pub seed: Seed,
    /// Global `0x0088CAC4`: GUID of the last NPC whose chat opened (−1
    /// for none). Read only by dead code (§10).
    pub last_chat_npc: u32,
    /// `monstats` flags (`npc`, `interact`) by class.
    flags: Vec<(bool, bool)>,
    /// `hireling` rows in table order.
    pub hirelings: Vec<HireRow>,
    /// Game +0x70 (expansion).
    pub expansion: bool,
    /// Game +0x6D (difficulty 0–2).
    pub difficulty: u8,
}

impl NpcControl {
    /// `0x00536070` at game creation (§1.1): steps the game seed once
    /// (the third seed derived at game creation, `rng.md` §5.2).
    ///
    /// Step 3 (item ids of `cqv` / `aqv`) and the store data of traders
    /// (step 5) belong to `vendors.md` §1; the vendors module attaches
    /// them per record.
    pub fn new(
        monstats: &[Monstats],
        hirelings: Vec<HireRow>,
        expansion: bool,
        difficulty: u8,
        game_seed: &mut Seed,
    ) -> Result<Self, NpcError> {
        let seed = Seed::init_low(game_seed.step());
        let table = parse_npc_table(VENDORS_TSV)?;
        let mut records = Vec::new();
        for (class, m) in monstats.iter().enumerate() {
            if !m.interact {
                continue;
            }
            if records.len() == RECORD_MAX {
                return Err(NpcError::TooManyRecords);
            }
            let class = class as u16;
            let mut r = NpcRecord {
                class,
                ..NpcRecord::default()
            };
            if let Some(t) = table.iter().find(|t| t.class == class) {
                r.act = t.act;
                r.trader = t.trader;
                r.byte6 = t.byte6;
            }
            records.push(r);
        }
        Ok(Self {
            records,
            seed,
            last_chat_npc: u32::MAX,
            flags: monstats.iter().map(|m| (m.npc, m.interact)).collect(),
            hirelings,
            expansion,
            difficulty,
        })
    }

    /// `0x00535F10`: the first record of that class. (`0x00535EA0`
    /// asserts on class 0, the class of the original's empty slots; no
    /// record here has class 0 unless monstats row 0 has `interact`.)
    pub fn record(&self, class: u16) -> Option<&NpcRecord> {
        self.records.iter().find(|r| r.class == class)
    }

    pub fn record_mut(&mut self, class: u16) -> Option<&mut NpcRecord> {
        self.records.iter_mut().find(|r| r.class == class)
    }

    /// `monstats` flag `npc` (bit 8) of a class.
    pub fn is_npc(&self, class: u16) -> bool {
        self.flags.get(usize::from(class)).is_some_and(|f| f.0)
    }

    /// `monstats` flag `interact` (bit 9) of a class.
    pub fn interacts(&self, class: u16) -> bool {
        self.flags.get(usize::from(class)).is_some_and(|f| f.1)
    }
}

// ------------------------------------------------------------ seams

/// The interaction list of an NPC (monster data +0x30 → interaction
/// block; its first field is the list head). Embedded in the monster
/// data of `interact` monsters by the monster spec.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InteractionList {
    /// (player, state) from the list head.
    pub nodes: Vec<(UnitId, u8)>,
}

impl InteractionList {
    pub fn state(&self, player: UnitId) -> Option<u8> {
        self.nodes.iter().find(|n| n.0 == player).map(|n| n.1)
    }

    fn set_state(&mut self, player: UnitId, state: u8) {
        if let Some(n) = self.nodes.iter_mut().find(|n| n.0 == player) {
            n.1 = state;
        }
    }

    fn remove(&mut self, player: UnitId) {
        if let Some(i) = self.nodes.iter().position(|n| n.0 == player) {
            self.nodes.remove(i);
        }
    }
}

/// The seam to the rest of the game. Expected providers in brackets.
pub trait NpcWorld {
    // Game [game creation].
    /// Game +0x78 (item format).
    fn item_format(&self) -> u16;

    // Units [units group, monster spec].
    fn guid(&self, unit: UnitId) -> u32;
    /// A monster by GUID.
    fn monster_by_guid(&self, guid: u32) -> Option<UnitId>;
    /// Any unit by GUID with its unit type (0x2F / 0x30 read only the
    /// GUID, §3).
    fn unit_by_guid(&self, guid: u32) -> Option<(u8, UnitId)>;
    fn monster_class(&self, unit: UnitId) -> Option<u16>;
    /// Unit mode (NPC: 0 death, 12 dead).
    fn mode(&self, unit: UnitId) -> u8;
    fn set_mode(&mut self, unit: UnitId, mode: u8);
    fn clear_unit_flag(&mut self, unit: UnitId, flag: u32);
    /// `0x00641530` unit distance.
    fn distance(&self, a: UnitId, b: UnitId) -> i32;
    /// `0x00548EF0`: 0 when the player is within 50 subtiles of the NPC
    /// on both axes, else its result code.
    fn axis_check(&self, player: UnitId, npc: UnitId) -> u32;
    /// `0x00548A80`: the NPC is in the player's act.
    fn same_act(&self, player: UnitId, npc: UnitId) -> bool;
    /// `0x00548F80` (`intents-events.md` §2.4 rule 4) for a 0x38 NPC
    /// GUID; 0 = accept.
    fn unit_check(&self, player: UnitId, guid: u32) -> u32;
    /// `0x00535060`: 0 when the player is free (no interact unit, no
    /// cursor item, player data +0x4C = 0).
    fn player_busy(&self, player: UnitId) -> u32;
    /// `0x00457490` in the start check (Open question 1).
    fn start_allowed(&self, player: UnitId, npc: UnitId) -> bool;
    /// `0x00594610` for cain1 (Tristram Cain, `quests.md`).
    fn tristram_cain_busy(&self, player: UnitId, npc: UnitId) -> bool;
    /// `0x00648730`: clear a unit's path.
    fn clear_path(&mut self, unit: UnitId);
    /// `0x0058EC00(npc, …, 0x28)` (Open question 2; monster spec).
    fn npc_ai_param(&mut self, npc: UnitId, param: u32);
    /// Cancel the NPC's AI-think events (type 2) and schedule one at
    /// frame + 1 (`tick.md` §5.2–5.4).
    fn reschedule_ai_think(&mut self, npc: UnitId);
    /// `0x00548A50`: player movement toward the target (movement spec).
    fn approach(&mut self, player: UnitId, npc: UnitId);
    /// Monster data +0x30 of an `interact` monster.
    fn interaction(&mut self, npc: UnitId) -> Option<&mut InteractionList>;
    /// The player's interact unit (type, GUID).
    fn interact_unit(&self, player: UnitId) -> Option<(u8, u32)>;
    /// `0x00554120`.
    fn set_interact(&mut self, player: UnitId, unit_type: u8, guid: u32);
    /// `0x00554190` (GUID −1, type 6, flag 0).
    fn reset_interact(&mut self, player: UnitId);
    /// `0x00574EC0(player, kind, arg)`: the player's pet of that kind
    /// (kind 7 = hireling; §7.4, §7.5).
    fn pet(&self, player: UnitId, kind: u8, arg: u8) -> Option<UnitId>;
    /// Pets in the player spec's iteration order (`0x00574DE0`).
    fn pets(&self, player: UnitId) -> Vec<UnitId>;

    // Stats and states [stats spec].
    fn stat(&self, unit: UnitId, stat: u16) -> u32;
    /// The base (unit-list) value.
    fn base_stat(&self, unit: UnitId, stat: u16) -> u32;
    /// Set without a message.
    fn set_stat(&mut self, unit: UnitId, stat: u16, value: u32);
    /// `0x00548520`: set and send (SetStat 0x1D–0x1F).
    fn set_stat_send(&mut self, player: UnitId, stat: u16, value: u32);
    /// `0x00625D10`, `0x00625D60`, `0x00625DB0`.
    fn max_life(&self, unit: UnitId) -> u32;
    fn max_mana(&self, unit: UnitId) -> u32;
    fn max_stamina(&self, unit: UnitId) -> u32;
    /// States count (`states` table).
    fn states_count(&self) -> u16;
    fn has_state(&self, unit: UnitId, state: u16) -> bool;
    /// `0x0063A460`: the state's curable mask is set.
    fn curable(&self, state: u16) -> bool;
    fn has_state_list(&self, unit: UnitId, state: u16) -> bool;
    fn remove_state_list(&mut self, unit: UnitId, state: u16);
    /// `0x00553380`.
    fn attach_sound(&mut self, unit: UnitId, sound: u16);

    // Messages [d2-server transport].
    fn send(&mut self, player: UnitId, msg: &[u8]);

    // Quests [world::quests through the game's QuestControl].
    /// The player's flag record of the game's difficulty.
    fn quest_flags(&self, player: UnitId) -> QuestFlags;
    /// Quest event 0 for player and NPC (`QuestControl::npc_activate`);
    /// the text list it built.
    fn quest_text_list(&mut self, player: UnitId, npc: UnitId) -> TextList;
    /// `0x00661480`: the 34 list bytes of S→C 0x27 (not specified;
    /// `server-messages.tsv` 0x27 is `partial`).
    fn encode_text_list(&self, list: &TextList) -> [u8; 34];
    /// `0x00544520` S→C 0x29 (`QuestControl::send_game_flags`).
    fn send_game_quests(&mut self, player: UnitId);
    /// `0x0053D670` S→C 0x28 (`quests::send_player_flags`).
    fn send_player_quests(&mut self, player: UnitId, unit_type: u8, guid: u32);
    /// `0x00543D50` (`QuestControl::npc_deactivate`).
    fn quest_chat_end(&mut self, player: UnitId, npc: UnitId);
    /// `0x0058FD20` (`quests::act1::respec_offer`).
    fn respec_offer(&mut self, player: UnitId);
    /// `0x0058FD50` (`quests::act1::respec_done`).
    fn respec_done(&mut self, player: UnitId);
    /// `0x00591790` (`quests::act1::imbue_granted`).
    fn imbue_granted(&mut self, player: UnitId);
    /// `0x005877C0` (Act V quest hook, not specified).
    fn socket_granted(&mut self, player: UnitId);
    /// `0x0058BC00` (Act V quest hook, not specified).
    fn personalize_granted(&mut self, player: UnitId);
    /// `0x005467E0(npc, level, from)` (`QuestControl::act_completion`).
    fn act_completion(&mut self, player: UnitId, npc: UnitId, level: u32, from: u32);

    // Player [player spec].
    /// Stat reset `0x00570C80` (`combat/vitals.md` §2.1).
    fn reset_stats(&mut self, player: UnitId);
    /// Skill reset `0x00570360` (`skills/levels.md` §6.5).
    fn reset_skills(&mut self, player: UnitId);
    /// The respec sound (`0x00553380`; the id is not written).
    fn respec_sound(&mut self, player: UnitId);
    fn player_name(&self, player: UnitId) -> Vec<u8>;
    /// `0x0054B830(level, arg)`: act change.
    fn act_change(&mut self, player: UnitId, level: u32, arg: u32);
    /// `0x00660E00`, `0x00660EC0`: activate the level's waypoint
    /// (`waypoints.md`).
    fn activate_waypoint(&mut self, player: UnitId, level: u32);

    // Items [items group, inventory owner].
    /// The player's items in inventory order.
    fn inventory(&self, player: UnitId) -> Vec<InvEntry>;
    /// `0x00562590`.
    fn identify(&mut self, item: UnitId);
    fn cursor_item(&self, player: UnitId) -> Option<UnitId>;
    fn item_facts(&self, item: UnitId) -> ItemFacts;
    /// `0x00563C00`: put the refused item back.
    fn put_back(&mut self, player: UnitId, item: UnitId);
    /// `0x0055EEA0`: remove the input from the cursor.
    fn remove_cursor_item(&mut self, player: UnitId, item: UnitId) -> bool;
    /// `0x0055A2A0`: duplicate an item into the player.
    fn duplicate(&mut self, player: UnitId, item: UnitId) -> Option<UnitId>;
    /// `0x00558270` + `0x00558D90`: fill a drop request from `input`
    /// (read before the input leaves the cursor), apply `mods`, create.
    fn create_imbued(&mut self, player: UnitId, input: UnitId, mods: &ImbueMods) -> Option<UnitId>;
    /// `0x0055FE00`.
    fn item_refresh(&mut self, item: UnitId);
    fn set_item_page(&mut self, item: UnitId, page: u8);
    fn set_item_flag(&mut self, item: UnitId, flag: u32);
    fn personal_name(&self, item: UnitId) -> Vec<u8>;
    fn set_personal_name(&mut self, item: UnitId, name: &[u8]);
    /// Place in the inventory, or drop at a free spot near the player.
    fn place_or_drop(&mut self, player: UnitId, item: UnitId);
    /// `0x0062BC20`.
    fn max_sockets(&self, item: UnitId) -> u32;
    /// `0x0062BCB0`.
    fn add_sockets(&mut self, item: UnitId, n: u32);
    fn item_seed(&mut self, item: UnitId) -> &mut Seed;

    // Mercenaries [mercenary spec].
    /// `0x005B23C0(class, 1, mode, 0)` near `near`.
    fn spawn_mercenary(&mut self, near: UnitId, class: u32, mode: u8) -> Option<UnitId>;
    /// `0x00573270`.
    fn init_mercenary(&mut self, player: UnitId, merc: UnitId, init: &MercInit);
    /// `0x00579AA0`.
    fn revive_mercenary(&mut self, player: UnitId, merc: UnitId);
}

/// The vendors seam (`world/vendors.md`, module `world::vendors`).
pub trait NpcVendors {
    /// `0x00579430(npc, single, gamble)` (`vendors.md` §4). Store
    /// generation draws from `ctl.seed`; asheara's trade open makes the
    /// hire list with [`NpcControl::make_hire_list`] (§7.1).
    fn open_trade(
        &mut self,
        ctl: &mut NpcControl,
        player: UnitId,
        npc: UnitId,
        single: bool,
        gamble: bool,
    ) -> Result<(), NpcError>;
    /// `0x00537190` (`vendors.md` §5.4): drop the player's gamble list.
    fn drop_gamble_list(&mut self, player: UnitId, npc: UnitId);
    /// `vendors.md` §9.1: take `cost` gold; false when not enough.
    fn pay(&mut self, player: UnitId, cost: u32) -> bool;
    /// `0x005761C0` (`vendors.md` §8.2).
    fn repair(&mut self, item: UnitId);
}

// ------------------------------------------------------------ §9

/// `0x0053D740`: S→C 0x2A (15 bytes). Bytes 3–6 are uninitialised
/// stack in the original (edge case 1); d2rs writes 0.
pub fn transaction(kind: u8, code: u8, guid: u32, gold: u32) -> [u8; 15] {
    let mut m = [0u8; 15];
    m[0] = 0x2A;
    m[1] = kind;
    m[2] = code;
    m[7..11].copy_from_slice(&guid.to_le_bytes());
    m[11..15].copy_from_slice(&gold.to_le_bytes());
    m
}

/// `0x0053D8D0`: S→C 0x58 (7 bytes); byte 6 is not written in the
/// original (edge case 10), d2rs writes 0.
pub fn service_result(npc_guid: u32, result: u8) -> [u8; 7] {
    let mut m = [0u8; 7];
    m[0] = 0x58;
    m[1..5].copy_from_slice(&npc_guid.to_le_bytes());
    m[5] = result;
    m
}

/// `0x0053E0E0`: S→C 0x9B after a resurrection (§7.4 step 4).
pub fn resurrect_message() -> [u8; 7] {
    [0x9B, 0xFF, 0xFF, 0, 0, 0, 0]
}

/// 0x2A with kind 0 and GUID −1, gold = stat 14.
fn send_code<W: NpcWorld>(w: &mut W, player: UnitId, code: u8, guid: u32) {
    let gold = w.stat(player, stat::GOLD);
    w.send(player, &transaction(0, code, guid, gold));
}

fn u32_at(msg: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(msg[o..o + 4].try_into().expect("4 bytes"))
}

// ------------------------------------------------------------ §2–§4

impl NpcControl {
    /// `0x0054AA90` → `0x00548B00`: C→S 0x13 for a monster (§2). `None`:
    /// a unit type other than 1 (not this spec). Returns the result code.
    pub fn interact<W: NpcWorld + NpcVendors>(
        &mut self,
        w: &mut W,
        player: UnitId,
        msg: &[u8],
    ) -> Result<Option<u32>, NpcError> {
        if msg.len() != 9 {
            return Ok(Some(3));
        }
        let ty = u32_at(msg, 1);
        if ty > 5 {
            return Ok(Some(2));
        }
        if ty != u32::from(UNIT_MONSTER) {
            return Ok(None);
        }
        let guid = u32_at(msg, 5);
        let Some(npc) = w.monster_by_guid(guid) else {
            return Ok(Some(1));
        };
        let d = w.distance(player, npc);
        if d > START_DISTANCE {
            return Ok(Some(1));
        }
        let class = w.monster_class(npc).unwrap_or(0);
        if self.is_npc(class) && self.interacts(class) {
            w.clear_path(npc);
            w.npc_ai_param(npc, AI_PARAM);
            w.reschedule_ai_think(npc);
        }
        if d > APPROACH_DISTANCE {
            return Ok(Some(0));
        }
        if d > TALK_DISTANCE {
            w.approach(player, npc);
            return Ok(Some(0));
        }
        if w.player_busy(player) != 0 {
            return Ok(Some(0));
        }
        w.clear_path(player);
        self.start(w, player, npc).map(Some)
    }

    /// `0x00573020` → `0x00572C10`: start an interaction (§2).
    fn start<W: NpcWorld + NpcVendors>(
        &mut self,
        w: &mut W,
        player: UnitId,
        npc: UnitId,
    ) -> Result<u32, NpcError> {
        // TODO(npc §2 start rule 1): only the 0x00457490 and "already in
        // the list" failures name a result (1); the others return 0.
        if w.interact_unit(player).is_some() {
            return Ok(0);
        }
        let mode = w.mode(npc);
        if mode == 0 || mode == 12 || w.player_busy(player) == 1 {
            return Ok(0);
        }
        if !w.start_allowed(player, npc) {
            return Ok(1);
        }
        let class = w.monster_class(npc).unwrap_or(0);
        if class == CAIN1 && w.tristram_cain_busy(player, npc) {
            return Ok(0);
        }
        // TODO(npc §2): an NPC without an interaction list is not
        // described; nothing happens.
        let Some(list) = w.interaction(npc) else {
            return Ok(0);
        };
        if list.state(player).is_some() {
            return Ok(1);
        }
        let first = list.nodes.is_empty();
        list.nodes.insert(0, (player, talk::TALKING));
        self.send_hire_list(w, player, npc, first)?;
        let guid = w.guid(npc);
        w.set_interact(player, UNIT_MONSTER, guid);
        let list = w.quest_text_list(player, npc);
        let mut m = Vec::with_capacity(40);
        m.extend_from_slice(&[0x27, 1]);
        m.extend_from_slice(&guid.to_le_bytes());
        m.extend_from_slice(&w.encode_text_list(&list));
        w.send(player, &m);
        w.send_game_quests(player);
        w.send_player_quests(player, UNIT_MONSTER, guid);
        Ok(0)
    }

    /// The common checks of 0x2F and 0x30 (§3).
    fn chat_npc<W: NpcWorld>(w: &mut W, player: UnitId, msg: &[u8]) -> Result<UnitId, u32> {
        if msg.len() != 9 {
            return Err(3);
        }
        let Some((ty, npc)) = w.unit_by_guid(u32_at(msg, 5)) else {
            return Err(1);
        };
        if ty != UNIT_MONSTER || w.interaction(npc).is_none() {
            return Err(3);
        }
        if !w.same_act(player, npc) {
            return Err(2);
        }
        Ok(npc)
    }

    /// `0x0054B930`: C→S 0x2F chat open (§3); `0x00572E60`.
    pub fn chat_open<W: NpcWorld>(&mut self, w: &mut W, player: UnitId, msg: &[u8]) -> u32 {
        let npc = match Self::chat_npc(w, player, msg) {
            Ok(n) => n,
            Err(r) => return r,
        };
        let r = w.axis_check(player, npc);
        if r != 0 {
            return r;
        }
        let list = w.interaction(npc).expect("checked");
        if list.state(player) != Some(talk::TALKING) {
            return 0;
        }
        list.set_state(player, talk::CHATTING);
        self.heal_hook(w, player, npc);
        0
    }

    /// `0x0054B9F0`: C→S 0x30 chat close (§3); `0x00572F20`.
    pub fn chat_close<W: NpcWorld + NpcVendors>(
        &mut self,
        w: &mut W,
        player: UnitId,
        msg: &[u8],
    ) -> u32 {
        let npc = match Self::chat_npc(w, player, msg) {
            Ok(n) => n,
            Err(r) => return r,
        };
        w.quest_chat_end(player, npc);
        let list = w.interaction(npc).expect("checked");
        let Some(state) = list.state(player) else {
            return 0;
        };
        list.remove(player);
        let empty = list.nodes.is_empty();
        if w.interact_unit(player)
            .is_some_and(|(t, _)| t == UNIT_MONSTER)
        {
            w.reset_interact(player);
        }
        if state >= talk::CHATTING && empty {
            w.drop_gamble_list(player, npc);
        }
        0
    }

    /// `0x0054BCA0`: C→S 0x38 menu action (§4) → `0x00579D60`. No 0x2A
    /// is sent here.
    pub fn menu_action<W: NpcWorld + NpcVendors>(
        &mut self,
        w: &mut W,
        player: UnitId,
        msg: &[u8],
    ) -> Result<u32, NpcError> {
        if msg.len() != 13 {
            return Ok(3);
        }
        let (action, guid, item) = (u32_at(msg, 1), u32_at(msg, 5), u32_at(msg, 9));
        let r = w.unit_check(player, guid);
        if r != 0 {
            return Ok(r);
        }
        let Some(npc) = w.monster_by_guid(guid) else {
            return Ok(0);
        };
        let class = w.monster_class(npc).unwrap_or(0);
        if !self.interacts(class) {
            return Ok(0);
        }
        let Some(list) = w.interaction(npc) else {
            return Ok(0);
        };
        let count = list.nodes.len();
        let single = count == 1;
        match action {
            1 | 2 => {
                let gamble = action == 2;
                let allowed = if gamble {
                    GAMBLERS.contains(&class)
                } else {
                    TRADERS.contains(&class)
                };
                if allowed {
                    // `0x00572EA0`: state 1 → 2; skipped when absent
                    // (edge case 7).
                    if let Some(list) = w.interaction(npc) {
                        if list.state(player) == Some(talk::CHATTING) {
                            list.set_state(player, talk::TRADING);
                        }
                    }
                    w.open_trade(self, player, npc, single, gamble)?;
                }
            }
            3 => self.send_hire_list(w, player, npc, count < 2)?,
            _ => self.service(w, player, npc, class, action, item),
        }
        Ok(0)
    }

    /// `0x00579D60` for actions ∉ {1, 2, 3} (§8).
    fn service<W: NpcWorld + NpcVendors>(
        &mut self,
        w: &mut W,
        player: UnitId,
        npc: UnitId,
        class: u16,
        action: u32,
        item: u32,
    ) {
        match class {
            CHARSI | LARZUK | DREHYA => services::item_service(w, player, npc, class, item),
            AKARA => services::respec(w, player, self.difficulty),
            _ => services::act_travel(w, player, npc, class, action, self.expansion),
        }
    }

    // ------------------------------------------------------------ §5

    /// `0x00578E70(npc)`: the heal hook of a chat open.
    fn heal_hook<W: NpcWorld>(&mut self, w: &mut W, player: UnitId, npc: UnitId) {
        let guid = w.guid(npc);
        self.last_chat_npc = guid;
        let class = w.monster_class(npc).unwrap_or(0);
        if HEALERS.contains(&class) {
            heal(w, player, npc);
        }
    }

    // ------------------------------------------------------------ §6

    /// `0x0054BBA0`: C→S 0x34 Cain identify (§6) → `0x00578460`.
    pub fn identify<W: NpcWorld + NpcVendors>(
        &mut self,
        w: &mut W,
        player: UnitId,
        msg: &[u8],
    ) -> u32 {
        if msg.len() != 5 {
            return 3;
        }
        let guid = u32_at(msg, 1);
        let Some(npc) = w.monster_by_guid(guid) else {
            send_code(w, player, code::REFUSED, u32::MAX);
            return 0;
        };
        if w.interact_unit(player) != Some((UNIT_MONSTER, guid)) {
            send_code(w, player, code::REFUSED, u32::MAX);
            return 0;
        }
        let class = w.monster_class(npc).unwrap_or(0);
        if !IDENTIFIERS.contains(&class) {
            return 0;
        }
        let items: Vec<UnitId> = w
            .inventory(player)
            .into_iter()
            .filter(|e| e.flags & 0x10 == 0)
            .filter(|e| matches!(e.place, Place::Grid(0) | Place::Grid(3) | Place::Equipped))
            .map(|e| e.item)
            .collect();
        if items.is_empty() {
            send_code(w, player, code::REFUSED, u32::MAX);
            return 0;
        }
        let f = w.quest_flags(player);
        if !(f.get(4, 0) || f.get(4, 1)) {
            let cost = 100u32.wrapping_mul(items.len() as u32);
            if !w.pay(player, cost) {
                send_code(w, player, code::NO_GOLD, u32::MAX);
                return 0;
            }
        }
        for item in items {
            w.identify(item);
        }
        send_code(w, player, code::IDENTIFIED, u32::MAX);
        0
    }
}

/// `0x00578D30(player, npc)` (§5): heal a player at a healer.
fn heal<W: NpcWorld>(w: &mut W, player: UnitId, npc: UnitId) {
    let guid = w.guid(npc);
    if w.interact_unit(player) != Some((UNIT_MONSTER, guid)) {
        return;
    }
    let mut changed = false;
    let maxes = [
        (stat::LIFE, w.max_life(player)),
        (stat::MANA, w.max_mana(player)),
        (stat::STAMINA, w.max_stamina(player)),
    ];
    for (s, max) in maxes {
        if w.stat(player, s) < max {
            w.set_stat_send(player, s, max);
            changed = true;
        }
    }
    changed |= remove_list(w, player, 2);
    changed |= remove_list(w, player, 1);
    changed |= cure_states(w, player);
    for pet in w.pets(player) {
        // TODO(npc §5 step 5): "life to max" read as step 1's test (only
        // when below max, counted as a change).
        let max = w.max_life(pet);
        if w.stat(pet, stat::LIFE) < max {
            w.set_stat(pet, stat::LIFE, max);
            changed = true;
        }
        changed |= cure_states(w, pet);
        changed |= remove_list(w, pet, 2);
        changed |= remove_list(w, pet, 1);
    }
    if changed {
        w.attach_sound(npc, SOUND_HEAL);
    }
}

fn remove_list<W: NpcWorld>(w: &mut W, unit: UnitId, state: u16) -> bool {
    if w.has_state_list(unit, state) {
        w.remove_state_list(unit, state);
        return true;
    }
    false
}

/// `0x00578C20`: remove every curable state's stat list.
fn cure_states<W: NpcWorld>(w: &mut W, unit: UnitId) -> bool {
    let mut changed = false;
    for s in 0..w.states_count() {
        if w.has_state(unit, s) && w.curable(s) && w.has_state_list(unit, s) {
            w.remove_state_list(unit, s);
            changed = true;
        }
    }
    changed
}

impl HireRow {
    /// Decodes the `hireling` table (fixed up: name ids at +0x114 /
    /// +0x116, `fixups.md` §7).
    pub fn from_table(t: &BinTable) -> Result<Vec<HireRow>, NpcError> {
        if t.name != Hireling::TABLE || t.record_size != Hireling::SIZE {
            return Err(NpcError::Table(format!(
                "{} ({}-byte records) is not hireling",
                t.name, t.record_size
            )));
        }
        Ok(t.iter()
            .map(|r| {
                let h = Hireling::decode(r);
                HireRow {
                    version: h.version,
                    class: h.class,
                    act: h.act,
                    difficulty: h.difficulty,
                    seller: h.seller,
                    gold: h.gold,
                    level: h.level,
                    name_first: u16::from_le_bytes([r[0x114], r[0x115]]),
                    name_last: u16::from_le_bytes([r[0x116], r[0x117]]),
                }
            })
            .collect())
    }
}
