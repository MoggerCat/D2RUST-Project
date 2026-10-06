// Spec: specs/world/cube.md
//! The Horadric Cube: routing (§1), put-in (§2), transmute (§3–§8) and the
//! portal kinds (§9).
//!
//! Recipes come from the compiled `cubemain` records ([`Recipe::decode`]:
//! the typed [`Cubemain`] fields plus the callback slot bytes of
//! `data/callbacks.md` §2–§3). Items, stats, inventory and item creation
//! belong to other specs and are reached through [`CubeWorld`]. The cube's
//! own draws (type pick, mod chances) go through [`Seed`] in the spec's
//! order.

use d2_data::bin::BinTable;
use d2_data::tables::{
    decode_all, Armor, Cubemain, Experience, Itemstatcost, Misc, Record, Weapons, WrongTable,
};

use crate::rng::Seed;
use crate::units::UnitId;

#[cfg(test)]
mod tests;

/// Item code of the cube (`box `).
pub const CUBE_CODE: [u8; 4] = *b"box ";
/// Inventory page of the cube's contents.
pub const CUBE_PAGE: u8 = 3;
/// Interaction type "cube".
pub const INTERACT_CUBE: u8 = 4;
/// 0x4F buttons.
pub const BUTTON_CLOSE: u16 = 0x17;
pub const BUTTON_TRANSMUTE: u16 = 0x18;
/// Sound events (§Outputs).
pub const SOUND_TRANSMUTE: u8 = 4;
pub const SOUND_REFUSED_PUT: u8 = 19;
pub const SOUND_COW_REFUSED: u8 = 20;
/// Used-mark array size (§6.1).
pub const USED_MARKS: usize = 48;
/// Filler list size (§7).
pub const MAX_FILLERS: usize = 18;
/// Type-pick candidate limit (§7.5).
pub const MAX_CANDIDATES: usize = 256;
/// Quest-item difficulty stat (§6.2 #8).
pub const STAT_QUEST_DIFFICULTY: u16 = 356;
/// Stats the outputs write or read (§7.6).
pub const STAT_LEVEL: u16 = 12;
pub const STAT_QUANTITY: u16 = 70;
pub const STAT_DURABILITY: u16 = 72;
pub const STAT_MAX_DURABILITY: u16 = 73;
pub const STAT_EXTRA_STACK: u16 = 254;

/// Item flags (item data +0x18).
pub mod item_flags {
    pub const IDENTIFIED: u32 = 0x10;
    pub const BROKEN: u32 = 0x100;
    pub const SOCKETED: u32 = 0x800;
    pub const ETHEREAL: u32 = 0x40_0000;
    pub const RUNEWORD: u32 = 0x400_0000;
}

/// Input slot flags (`data/callbacks.md` §2).
pub mod input_flags {
    pub const USEANY: u16 = 0x0001;
    pub const ITEMCODE: u16 = 0x0002;
    pub const NOS: u16 = 0x0004;
    pub const SOCK: u16 = 0x0008;
    pub const ETH: u16 = 0x0010;
    pub const NOE: u16 = 0x0020;
    pub const SPECIAL: u16 = 0x0040;
    pub const UPG: u16 = 0x0080;
    pub const BAS: u16 = 0x0100;
    pub const EXC: u16 = 0x0200;
    pub const ELI: u16 = 0x0400;
    pub const NRU: u16 = 0x0800;
}

/// Output slot flags (`data/callbacks.md` §3).
pub mod output_flags {
    pub const MOD: u16 = 0x0001;
    pub const SOCK: u16 = 0x0002;
    pub const ETH: u16 = 0x0004;
    pub const SPECIAL: u16 = 0x0008;
    pub const UNS: u16 = 0x0010;
    pub const REM: u16 = 0x0020;
    pub const REG: u16 = 0x0040;
    pub const EXC: u16 = 0x0080;
    pub const ELI: u16 = 0x0100;
    pub const REP: u16 = 0x0200;
    pub const RCH: u16 = 0x0400;
}

/// Output kinds (`data/callbacks.md` §3).
pub mod kind {
    pub const NONE: u8 = 0;
    pub const COW_PORTAL: u8 = 1;
    pub const PANDEMONIUM: u8 = 2;
    pub const PANDEMONIUM_FINALE: u8 = 3;
    pub const ITEMCODE: u8 = 0xFC;
    pub const ITEMTYPE: u8 = 0xFD;
    pub const USEITEM: u8 = 0xFE;
    pub const USETYPE: u8 = 0xFF;
}

// --------------------------------------------------------------- recipes

/// One input slot (8 bytes at 20 + 8k).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputSlot {
    pub flags: u16,
    /// Item type index, item index, or 0xFFFF.
    pub item: u16,
    /// Unique or set number + 1, else 0.
    pub special: u16,
    pub quality: u8,
    pub quantity: u8,
}

/// One craft modifier of an output (`mod 1`–`mod 5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CraftMod {
    /// Property index; negative (missed link) = none.
    pub property: i32,
    pub param: u16,
    pub min: u16,
    pub max: u16,
    pub chance: u8,
}

/// One output slot (84 bytes at 76 + 84k).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutputSlot {
    pub flags: u16,
    pub item: u16,
    pub special: u16,
    pub quality: u8,
    pub quantity: u8,
    pub kind: u8,
    pub lvl: u8,
    pub plvl: u8,
    pub ilvl: u8,
    pub pre: [u16; 3],
    pub suf: [u16; 3],
    pub mods: [CraftMod; 5],
}

/// A compiled `cubemain` record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Recipe {
    pub enabled: u8,
    pub ladder: u8,
    pub min_diff: u8,
    pub class: u8,
    pub op: u8,
    pub param: i32,
    pub value: i32,
    pub numinputs: u8,
    pub version: u16,
    pub inputs: [InputSlot; 7],
    pub outputs: [OutputSlot; 3],
}

fn le16(r: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([r[o], r[o + 1]])
}

fn craft_mod(property: u32, param: u16, min: u16, max: u16, chance: u8) -> CraftMod {
    CraftMod {
        // A link32 field; the original reads it as a signed int.
        property: property as i32,
        param,
        min,
        max,
        chance,
    }
}

impl Recipe {
    /// Decodes a 328-byte `cubemain` record: the typed fields of
    /// [`Cubemain`] and the callback slot bytes (`callbacks.md` §2–§3).
    pub fn decode(r: &[u8]) -> Self {
        let c = Cubemain::decode(r);
        let inputs = std::array::from_fn(|k| {
            let o = 20 + 8 * k;
            InputSlot {
                flags: le16(r, o),
                item: le16(r, o + 2),
                special: le16(r, o + 4),
                quality: r[o + 6],
                quantity: r[o + 7],
            }
        });
        let mods = [
            [
                craft_mod(
                    c.mod_1,
                    c.mod_1_param,
                    c.mod_1_min,
                    c.mod_1_max,
                    c.mod_1_chance,
                ),
                craft_mod(
                    c.mod_2,
                    c.mod_2_param,
                    c.mod_2_min,
                    c.mod_2_max,
                    c.mod_2_chance,
                ),
                craft_mod(
                    c.mod_3,
                    c.mod_3_param,
                    c.mod_3_min,
                    c.mod_3_max,
                    c.mod_3_chance,
                ),
                craft_mod(
                    c.mod_4,
                    c.mod_4_param,
                    c.mod_4_min,
                    c.mod_4_max,
                    c.mod_4_chance,
                ),
                craft_mod(
                    c.mod_5,
                    c.mod_5_param,
                    c.mod_5_min,
                    c.mod_5_max,
                    c.mod_5_chance,
                ),
            ],
            [
                craft_mod(
                    c.b_mod_1,
                    c.b_mod_1_param,
                    c.b_mod_1_min,
                    c.b_mod_1_max,
                    c.b_mod_1_chance,
                ),
                craft_mod(
                    c.b_mod_2,
                    c.b_mod_2_param,
                    c.b_mod_2_min,
                    c.b_mod_2_max,
                    c.b_mod_2_chance,
                ),
                craft_mod(
                    c.b_mod_3,
                    c.b_mod_3_param,
                    c.b_mod_3_min,
                    c.b_mod_3_max,
                    c.b_mod_3_chance,
                ),
                craft_mod(
                    c.b_mod_4,
                    c.b_mod_4_param,
                    c.b_mod_4_min,
                    c.b_mod_4_max,
                    c.b_mod_4_chance,
                ),
                craft_mod(
                    c.b_mod_5,
                    c.b_mod_5_param,
                    c.b_mod_5_min,
                    c.b_mod_5_max,
                    c.b_mod_5_chance,
                ),
            ],
            [
                craft_mod(
                    c.c_mod_1,
                    c.c_mod_1_param,
                    c.c_mod_1_min,
                    c.c_mod_1_max,
                    c.c_mod_1_chance,
                ),
                craft_mod(
                    c.c_mod_2,
                    c.c_mod_2_param,
                    c.c_mod_2_min,
                    c.c_mod_2_max,
                    c.c_mod_2_chance,
                ),
                craft_mod(
                    c.c_mod_3,
                    c.c_mod_3_param,
                    c.c_mod_3_min,
                    c.c_mod_3_max,
                    c.c_mod_3_chance,
                ),
                craft_mod(
                    c.c_mod_4,
                    c.c_mod_4_param,
                    c.c_mod_4_min,
                    c.c_mod_4_max,
                    c.c_mod_4_chance,
                ),
                craft_mod(
                    c.c_mod_5,
                    c.c_mod_5_param,
                    c.c_mod_5_min,
                    c.c_mod_5_max,
                    c.c_mod_5_chance,
                ),
            ],
        ];
        let levels = [
            (c.lvl, c.plvl, c.ilvl),
            (c.b_lvl, c.b_plvl, c.b_ilvl),
            (c.c_lvl, c.c_plvl, c.c_ilvl),
        ];
        let outputs = std::array::from_fn(|k| {
            let o = 76 + 84 * k;
            OutputSlot {
                flags: le16(r, o),
                item: le16(r, o + 2),
                special: le16(r, o + 4),
                quality: r[o + 6],
                quantity: r[o + 7],
                kind: r[o + 8],
                lvl: levels[k].0,
                plvl: levels[k].1,
                ilvl: levels[k].2,
                pre: std::array::from_fn(|i| le16(r, o + 12 + 2 * i)),
                suf: std::array::from_fn(|i| le16(r, o + 18 + 2 * i)),
                mods: mods[k],
            }
        });
        Recipe {
            enabled: c.enabled,
            ladder: c.ladder,
            min_diff: c.min_diff,
            class: c.class,
            op: c.op,
            param: c.param as i32,
            value: c.value as i32,
            numinputs: c.numinputs,
            version: c.version,
            inputs,
            outputs,
        }
    }
}

/// Every record of the loaded `cubemain` table, in record order.
pub fn recipes(t: &BinTable) -> Result<Vec<Recipe>, WrongTable> {
    // Checks the table name and record size.
    decode_all::<Cubemain>(t)?;
    Ok(t.iter().map(Recipe::decode).collect())
}

// ------------------------------------------------------------ item data

/// The `items.txt` fields the cube reads (all three item tables).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemRecord {
    pub code: [u8; 4],
    pub normcode: [u8; 4],
    pub ubercode: [u8; 4],
    pub ultracode: [u8; 4],
    pub version: u16,
    pub level: u8,
    pub quest: u8,
    pub questdiffcheck: u8,
    pub stackable: u8,
    pub spawnable: u8,
    pub maxstack: u32,
}

macro_rules! item_record {
    ($r:expr) => {
        ItemRecord {
            code: $r.code,
            normcode: $r.normcode,
            ubercode: $r.ubercode,
            ultracode: $r.ultracode,
            version: $r.version,
            level: $r.level,
            quest: $r.quest,
            questdiffcheck: $r.questdiffcheck,
            stackable: $r.stackable,
            spawnable: $r.spawnable,
            maxstack: $r.maxstack,
        }
    };
}

/// Table data the cube reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CubeData {
    pub recipes: Vec<Recipe>,
    /// Item records by item id: weapons, then armor, then misc.
    pub items: Vec<ItemRecord>,
    /// `itemstatcost` `ValShift` by stat id (the record count is its len).
    pub valshift: Vec<u8>,
    /// `experience.txt` max level of class 0 (`0x00611830(0)`).
    pub max_level: i32,
}

impl CubeData {
    /// From the loaded tables. `experience[0]` is the `MaxLvl` row.
    pub fn new(
        recipes: Vec<Recipe>,
        weapons: &[Weapons],
        armor: &[Armor],
        misc: &[Misc],
        itemstatcost: &[Itemstatcost],
        experience: &[Experience],
    ) -> Self {
        let items = weapons
            .iter()
            .map(|r| item_record!(r))
            .chain(armor.iter().map(|r| item_record!(r)))
            .chain(misc.iter().map(|r| item_record!(r)))
            .collect();
        Self {
            recipes,
            items,
            valshift: itemstatcost.iter().map(|s| s.valshift).collect(),
            max_level: experience.first().map_or(0, |e| e.amazon as i32),
        }
    }

    fn item(&self, class: u32) -> Option<&ItemRecord> {
        self.items.get(class as usize)
    }

    /// The code linker lookup (`0x006BD130`): first item with `code`.
    fn item_index(&self, code: [u8; 4]) -> Option<u32> {
        self.items
            .iter()
            .position(|i| i.code == code)
            .map(|i| i as u32)
    }
}

// ------------------------------------------------------------------ ops

/// Where an op is tested (`cube-ops.tsv` `scope`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpScope {
    Recipe,
    Input0,
    Inputs,
}

/// Which stat reader a stat op uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatRead {
    /// `0x00625480` (`STATLIST_UnitGetStatValue`).
    Value,
    /// `0x006253B0` (`GetUnitBaseStat`).
    Base,
    /// `0x00625560` (`GetUnitStatBonus`).
    Bonus,
}

/// What an op compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpTest {
    Always,
    DayOfMonth,
    DayOfWeek,
    Stat(StatRead, Cmp),
    FileIndexNot,
    QuestDifficulty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmp {
    Ge,
    Le,
    Ne,
    Eq,
}

impl Cmp {
    fn test(self, a: i32, b: i32) -> bool {
        match self {
            Cmp::Ge => a >= b,
            Cmp::Le => a <= b,
            Cmp::Ne => a != b,
            Cmp::Eq => a == b,
        }
    }
}

/// The scope and test of an op (`cube-ops.tsv`; ops ≥ 29 always pass).
pub fn op_info(op: u8) -> (OpScope, OpTest) {
    const CMP: [Cmp; 4] = [Cmp::Ge, Cmp::Le, Cmp::Ne, Cmp::Eq];
    const READ: [StatRead; 3] = [StatRead::Value, StatRead::Base, StatRead::Bonus];
    match op {
        1 => (OpScope::Recipe, OpTest::DayOfMonth),
        2 => (OpScope::Recipe, OpTest::DayOfWeek),
        3..=14 => {
            let i = usize::from(op - 3);
            (OpScope::Recipe, OpTest::Stat(READ[i / 4], CMP[i % 4]))
        }
        15..=26 => {
            let i = usize::from(op - 15);
            (OpScope::Input0, OpTest::Stat(READ[i / 4], CMP[i % 4]))
        }
        27 => (OpScope::Input0, OpTest::FileIndexNot),
        28 => (OpScope::Inputs, OpTest::QuestDifficulty),
        _ => (OpScope::Recipe, OpTest::Always),
    }
}

// ------------------------------------------------------------------ seam

/// An item request (`0x00558D90`, D2MOO `D2ItemDropStrc`; §7.4). Fields
/// not listed are zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemRequest {
    pub player: Option<UnitId>,
    pub level: i32,
    pub class: u32,
    /// Always 4.
    pub spawn_type: u8,
    /// Always 1.
    pub init_flags: u16,
    pub item_format: u16,
    pub quality: u8,
    pub item_index: u16,
    pub prefix: [u16; 3],
    pub suffix: [u16; 3],
    pub flags2: u32,
}

/// A craft property (`0x00660240` argument).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftProperty {
    pub property: i32,
    pub param: i32,
    pub min: i32,
    pub max: i32,
}

/// The seam to the rest of the game. Expected providers in brackets.
pub trait CubeWorld {
    // Game (game creation fields).
    fn expansion(&self) -> bool;
    /// Game +0x6A.
    fn game_type(&self) -> u8;
    /// Game +0x74.
    fn ladder(&self) -> bool;
    fn difficulty(&self) -> u8;
    /// Game +0x78.
    fn item_format(&self) -> u16;
    /// `GetLocalTime`: (day of month, day of week + 1), host-supplied.
    fn local_date(&self) -> (u8, u8);
    /// The game seed (game +0xD0; game).
    fn game_seed(&mut self) -> &mut Seed;

    // Units (units / stats).
    fn player_class(&self, player: UnitId) -> u8;
    /// A stat of a player or item, layer 0 (stats spec).
    fn stat(&self, unit: UnitId, read: StatRead, stat: u16) -> i32;
    fn set_stat(&mut self, unit: UnitId, stat: u16, value: i32);
    /// `0x00553380`: attach a sound event to the player.
    fn attach_sound(&mut self, player: UnitId, event: u8);
    fn send(&mut self, player: UnitId, msg: &[u8]);

    // Interaction (interaction / UI owner).
    /// Active interaction: (unit type, GUID).
    fn interaction(&self, player: UnitId) -> Option<(u8, u32)>;
    /// `0x00554120`; ignored when one is active.
    fn set_interaction(&mut self, player: UnitId, unit_type: u8, guid: u32);
    /// `0x00554190`.
    fn reset_interaction(&mut self, player: UnitId);
    /// `0x0055FA40` (inventory pass).
    fn inventory_pass(&mut self, player: UnitId);
    /// Is the interaction the stash object (type 2, class 0x10B).
    fn interacting_with_stash(&self, player: UnitId) -> bool;
    /// `0x005678A0`: trading (interaction type 0 with a live unit).
    fn trading(&self, player: UnitId) -> bool;

    // Items (items / inventory).
    /// The player's inventory items, in list order.
    fn inventory(&self, player: UnitId) -> Vec<UnitId>;
    /// The item unit with this GUID.
    fn item_by_guid(&self, guid: u32) -> Option<UnitId>;
    fn item_guid(&self, item: UnitId) -> u32;
    /// Item data +0x45.
    fn item_page(&self, item: UnitId) -> u8;
    fn set_item_page(&mut self, item: UnitId, page: u8);
    fn item_mode(&self, item: UnitId) -> u8;
    fn set_item_mode(&mut self, item: UnitId, mode: u8);
    /// Items record index (`None`: record missing).
    fn item_class(&self, item: UnitId) -> Option<u32>;
    fn set_item_class(&mut self, item: UnitId, class: u32);
    /// `0x00629BB0` / `0x00629A90`: is item record `class` of item type
    /// `ty` (type or type 2 through the equivalence matrix).
    fn class_is_type(&self, class: u32, ty: u16) -> bool;
    /// Item data +0 (default 2).
    fn item_quality(&self, item: UnitId) -> u8;
    /// Item data +0x28.
    fn item_file_index(&self, item: UnitId) -> u32;
    /// Item data +0x2C.
    fn item_level(&self, item: UnitId) -> i32;
    fn set_item_level(&mut self, item: UnitId, level: i32);
    /// Item data flags +0x18.
    fn item_flags(&self, item: UnitId) -> u32;
    fn set_item_flag(&mut self, item: UnitId, flag: u32);
    /// `0x006299B0`: stat 194 (sockets).
    fn item_sockets(&self, item: UnitId) -> i32;
    /// `0x0062BC20`: max sockets.
    fn max_sockets(&self, item: UnitId) -> i32;
    /// `0x0062BCB0`: add `n` sockets.
    fn add_sockets(&mut self, item: UnitId, n: i32);
    /// The item's unit seed (+0x20).
    fn item_seed(&mut self, item: UnitId) -> &mut Seed;
    /// Items socketed into `item` (its inventory), in order.
    fn socketed(&self, item: UnitId) -> Vec<UnitId>;
    /// `0x0055A2A0` (`ITEMS_Duplicate`).
    fn duplicate(&mut self, item: UnitId, fillers: bool) -> Option<UnitId>;
    /// `0x00557AB0(game, &item, 0, 0)`: item init; returns the result.
    fn item_init(&mut self, item: UnitId) -> Option<UnitId>;
    /// `0x00558D90(request, 0)`.
    fn create_item(&mut self, request: &ItemRequest) -> Option<UnitId>;
    /// `0x005C1BC0(item, prefix)`: a tempered affix roll (0 = none).
    fn tempered_affix(&mut self, item: UnitId, prefix: bool) -> u16;
    /// Quality 9 with the rare prefix and suffix (`0x00627EA0`,
    /// `0x00628010`, `0x00628070`).
    fn set_tempered(&mut self, item: UnitId, prefix: u16, suffix: u16);
    /// Game unique bitset (+0x1B24).
    fn unique_found(&self, index: u32) -> bool;
    fn set_unique_found(&mut self, index: u32, found: bool);
    /// `0x00558C50`: drop the runeword stat list.
    fn drop_runeword_stats(&mut self, item: UnitId);
    /// `0x00660240(item, &prop, expansion)`.
    fn add_craft_property(&mut self, item: UnitId, prop: &CraftProperty);
    /// `0x0055F900`.
    fn repair(&mut self, item: UnitId);
    /// `0x0055FE80`.
    fn recharge(&mut self, item: UnitId);
    /// `0x00560200(game, player, id, 0, 0, 1, 1, 0)`; true when placed.
    fn place(&mut self, player: UnitId, item: UnitId) -> bool;
    /// `0x00555600`.
    fn free_item(&mut self, item: UnitId);
    /// `0x00564F30` body for one item: queue 0x9D (action 5, flags 0x20,
    /// page 3), remove from the inventory and free (`0x0055DF10`).
    fn remove_cube_item(&mut self, player: UnitId, item: UnitId);
    /// `0x0055BF50` (targeting reset; may queue 0x3F).
    fn targeting_reset(&mut self, player: UnitId);
    /// `0x00549350` (item check shared with another handler): 0 = ok.
    fn put_item_check(&self, player: UnitId, item: u32) -> u32;
    /// `0x00549150`: the cube exists, is stored (mode 0), in the
    /// player's inventory.
    fn cube_check(&self, player: UnitId, cube: u32) -> bool;

    // Quests.
    /// Placed quest outputs: `hst ` → `0x0059E5C0`, `qf2 ` → `0x005B86E0`
    /// (Act II / III quests, not specified yet).
    fn quest_item_hook(&mut self, player: UnitId, item: UnitId, code: [u8; 4]);
    /// Kind 1 (`0x00594140`): [`crate::world::quests::cow_portal`].
    fn cow_portal(&mut self, player: UnitId) -> bool;
}

// ------------------------------------------------------------ transmute

/// One capture entry (§6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capture {
    pub item: Option<UnitId>,
    pub class: i32,
    pub level: i32,
}

/// What a transmute did (for callers and tests).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Transmute {
    /// The record that matched, if any.
    pub record: Option<usize>,
    /// The outputs committed (placed or lost), none when not committed.
    pub committed: bool,
    /// Outputs made (in slot order), committed or discarded.
    pub outputs: Vec<UnitId>,
}

/// ratio(a, b) (§7.1 rule 3).
pub fn ratio(a: i32, b: u8) -> i32 {
    let b = i32::from(b);
    if a <= 0x10_0000 {
        a.wrapping_mul(b) / 100
    } else {
        (a / 100) * b
    }
}

impl CubeData {
    /// A stat op's guard and compare (§5). `None`: passes without a test.
    fn stat_op<W: CubeWorld>(
        &self,
        w: &W,
        unit: UnitId,
        r: &Recipe,
        read: StatRead,
        cmp: Cmp,
    ) -> bool {
        let s = r.param;
        let count = self.valshift.len() as i32;
        if s < 0 || s > count {
            return true;
        }
        // s = count: no record (the range test's off-by-one).
        let Some(&shift) = self.valshift.get(s as usize) else {
            return false;
        };
        let t = r.value >> shift;
        cmp.test(w.stat(unit, read, s as u16), t)
    }

    /// Recipe-scope op (§4 test 7).
    fn recipe_op<W: CubeWorld>(&self, w: &W, player: UnitId, r: &Recipe, date: (u8, u8)) -> bool {
        match op_info(r.op) {
            (OpScope::Recipe, OpTest::DayOfMonth) => {
                let d = i32::from(date.0);
                r.param <= d && d <= r.value
            }
            (OpScope::Recipe, OpTest::DayOfWeek) => i32::from(date.1) == r.value,
            (OpScope::Recipe, OpTest::Stat(read, cmp)) => self.stat_op(w, player, r, read, cmp),
            _ => true,
        }
    }

    /// §4 tests 1–6.
    fn eligible<W: CubeWorld>(&self, w: &W, player: UnitId, r: &Recipe, n: usize) -> bool {
        r.enabled != 0
            && (w.expansion() || r.version < 100)
            && (w.game_type() != 0 || w.ladder() || r.ladder == 0)
            && r.min_diff <= w.difficulty()
            && (r.class == 0xFF || r.class == w.player_class(player))
            && usize::from(r.numinputs) == n
    }

    /// §6.2 tests 1–9 and 11 on one candidate.
    fn item_passes<W: CubeWorld>(
        &self,
        w: &W,
        r: &Recipe,
        k: usize,
        item: UnitId,
        class: u32,
    ) -> bool {
        use input_flags as f;
        let s = &r.inputs[k];
        let rec = self.item(class);
        // 1. item / type
        if s.flags & f::USEANY != 0 {
            if s.item != 0xFFFF {
                if s.flags & f::UPG != 0 {
                    let (Some(rec), Some(slot)) = (rec, self.item(s.item.into())) else {
                        return false;
                    };
                    let (c, sc) = (rec.code, slot.code);
                    let (n, u, l) = (rec.normcode, rec.ubercode, rec.ultracode);
                    let ok = if c == n {
                        sc == n
                    } else if c == u {
                        sc == n || sc == u
                    } else if c == l {
                        sc == n || sc == u || sc == l
                    } else {
                        false
                    };
                    if !ok {
                        return false;
                    }
                } else if class != u32::from(s.item) {
                    return false;
                }
            }
        } else if !w.class_is_type(class, s.item) {
            return false;
        }
        // 2. quality (a quality-9 mismatch calls a getter with no effect)
        if s.quality != 0 && w.item_quality(item) != s.quality {
            return false;
        }
        // 3. unique / set file index
        if s.flags & f::SPECIAL != 0 && w.item_file_index(item) as i32 != i32::from(s.special) - 1 {
            return false;
        }
        // 4. sockets
        if s.flags & f::NOS != 0 {
            if w.item_sockets(item) != 0 {
                return false;
            }
        } else if s.flags & f::SOCK != 0 && w.item_sockets(item) == 0 {
            return false;
        }
        // 5. ethereal
        let eth = w.item_flags(item) & item_flags::ETHEREAL != 0;
        if s.flags & f::NOE != 0 {
            if eth {
                return false;
            }
        } else if s.flags & f::ETH != 0 && !eth {
            return false;
        }
        // 6. base / exceptional / elite
        let tier = if s.flags & f::BAS != 0 {
            Some(rec.map(|r| r.normcode))
        } else if s.flags & f::EXC != 0 {
            Some(rec.map(|r| r.ubercode))
        } else if s.flags & f::ELI != 0 {
            Some(rec.map(|r| r.ultracode))
        } else {
            None
        };
        if let Some(want) = tier {
            if rec.map(|r| r.code) != want {
                return false;
            }
        }
        // 7. no runeword
        if s.flags & f::NRU != 0 && w.item_flags(item) & item_flags::RUNEWORD != 0 {
            return false;
        }
        // 8. op 28: quest item difficulty
        if r.op == 28 {
            if let Some(rec) = rec {
                if rec.quest != 0
                    && rec.questdiffcheck != 0
                    && w.stat(item, StatRead::Value, STAT_QUEST_DIFFICULTY)
                        < i32::from(w.difficulty())
                {
                    return false;
                }
            }
        }
        if k == 0 {
            // 9. output a socket test
            let a = &r.outputs[0];
            if matches!(a.kind, kind::USETYPE | kind::USEITEM)
                && a.flags & output_flags::SOCK != 0
                && w.max_sockets(item) == 0
            {
                return false;
            }
        }
        true
    }

    /// §6.2 test 11: input-scope op 15–27 on a slot-0 candidate.
    fn input_op<W: CubeWorld>(&self, w: &W, r: &Recipe, item: UnitId) -> bool {
        match op_info(r.op) {
            (OpScope::Input0, OpTest::Stat(read, cmp)) => self.stat_op(w, item, r, read, cmp),
            (OpScope::Input0, OpTest::FileIndexNot) => w.item_file_index(item) as i32 != r.value,
            _ => true,
        }
    }

    /// §6.4: capture one slot-0 match.
    fn capture<W: CubeWorld>(
        &self,
        w: &mut W,
        r: &Recipe,
        item: UnitId,
        class: u32,
        cap: &mut [Capture; 3],
    ) {
        let a = &r.outputs[0];
        for c in cap.iter_mut() {
            let mut level = w.item_level(item);
            if level < 1 {
                w.set_item_level(item, 1);
                level = 1;
            }
            c.level = level;
            if a.flags & output_flags::MOD != 0 {
                c.item = Some(item);
            }
            if matches!(a.kind, kind::USETYPE | kind::USEITEM) {
                c.item = Some(item);
                c.class = if a.kind == kind::USETYPE {
                    class as i32
                } else {
                    -1
                };
                // TODO(cube §6.4): "exc → …; else if eli" is read as
                // `if exc {…} else if eli {…}`; the spec's sentence also
                // admits "eli when the exc upgrade fails". No live record
                // sets both.
                let upgrade = if a.flags & output_flags::EXC != 0 {
                    self.item(class).map(|r| r.ubercode)
                } else if a.flags & output_flags::ELI != 0 {
                    self.item(class).map(|r| r.ultracode)
                } else {
                    None
                };
                if let Some(code) = upgrade.filter(|c| c != b"    ") {
                    if let Some(i) = self.item_index(code) {
                        if w.expansion() || self.items[i as usize].version < 100 {
                            c.class = i as i32;
                        }
                    }
                }
            }
        }
    }

    /// `0x005658C0`: input matching (§6). `Some(capture)` on a full match.
    fn match_inputs<W: CubeWorld>(
        &self,
        w: &mut W,
        r: &Recipe,
        cube_items: &[UnitId],
    ) -> Option<[Capture; 3]> {
        let mut cap = [Capture::default(); 3];
        let mut used = [false; USED_MARKS];
        for k in 0..7 {
            let s = r.inputs[k];
            if s.flags & 0x0003 == 0 {
                continue;
            }
            let need = if s.quantity == 0 {
                1
            } else {
                u32::from(s.quantity)
            };
            let mut found = 0u32;
            let mut stack = false;
            for (i, &item) in cube_items.iter().enumerate() {
                let p = i + 1;
                let Some(class) = w.item_class(item) else {
                    continue;
                };
                if self.item(class).is_none() {
                    continue;
                }
                if !self.item_passes(w, r, k, item, class) {
                    continue;
                }
                // 10. not used by an earlier slot
                if p < USED_MARKS && used[p - 1] {
                    continue;
                }
                if k == 0 && !self.input_op(w, r, item) {
                    continue;
                }
                if k == 0 {
                    self.capture(w, r, item, class, &mut cap);
                }
                if p < USED_MARKS {
                    used[p - 1] = true;
                }
                found += 1;
                if self.items[class as usize].stackable != 0 {
                    stack = true;
                }
            }
            let ok = if stack { found >= need } else { found == need };
            if !ok {
                return None;
            }
        }
        Some(cap)
    }

    /// `0x00565930`: item-type pick (§7.5).
    pub fn type_pick<W: CubeWorld>(&self, w: &mut W, ty: u16, level: i32) -> u32 {
        let n = self.items.len() as i32;
        // TODO(cube §7.5): N = 0 is not covered by the spec; 1.14d has
        // hundreds of items.
        if n == 0 {
            return 0;
        }
        let start = w.game_seed().roll(n) as i32;
        let stop = if start - 1 < 0 { n - 1 } else { start - 1 };
        if start == stop {
            return 0;
        }
        let format = w.item_format();
        let mut cand = Vec::new();
        let mut i = start;
        while i != stop {
            let rec = &self.items[i as usize];
            if cand.len() < MAX_CANDIDATES
                && w.class_is_type(i as u32, ty)
                && rec.spawnable != 0
                && (rec.version < 100 || format >= 100)
                && i32::from(rec.level) <= level
            {
                cand.push(i as u32);
            }
            i += 1;
            if i == n {
                i = 0;
            }
        }
        if cand.is_empty() {
            return 0;
        }
        let pick = w.game_seed().roll(cand.len() as i32);
        cand[pick as usize]
    }

    /// §7.1 rule 2–4: the output level.
    fn output_level<W: CubeWorld>(
        &self,
        w: &W,
        player: UnitId,
        o: &OutputSlot,
        cap: &Capture,
    ) -> i32 {
        let l = if o.lvl != 0 {
            i32::from(o.lvl)
        } else {
            let mut l = 0i32;
            if o.plvl != 0 {
                l = l.wrapping_add(ratio(w.stat(player, StatRead::Value, STAT_LEVEL), o.plvl));
            }
            if o.ilvl != 0 {
                l = l.wrapping_add(ratio(cap.level, o.ilvl));
            }
            l
        };
        l.min(self.max_level).max(1)
    }

    /// Max stack: `maxstack` + stat 254, capped at 511 (`0x006295B0`).
    fn max_stack<W: CubeWorld>(&self, w: &W, item: UnitId) -> i32 {
        let base = w
            .item_class(item)
            .and_then(|c| self.item(c))
            .map_or(0, |r| r.maxstack as i32);
        base.wrapping_add(w.stat(item, StatRead::Value, STAT_EXTRA_STACK))
            .min(511)
    }

    fn stackable<W: CubeWorld>(&self, w: &W, item: UnitId) -> bool {
        w.item_class(item)
            .and_then(|c| self.item(c))
            .is_some_and(|r| r.stackable != 0)
    }

    /// `0x00565AB0`: outputs and commit (§7, §8).
    fn outputs<W: CubeWorld>(
        &self,
        w: &mut W,
        player: UnitId,
        r: &Recipe,
        cap: &[Capture; 3],
    ) -> Transmute {
        use output_flags as f;
        let mut success = false;
        let mut craft = true;
        let mut out: [Option<UnitId>; 3] = [None; 3];
        let mut fillers: Vec<UnitId> = Vec::new();
        for j in 0..3 {
            let o = r.outputs[j];
            let remove = o.flags & (f::UNS | f::REM) != 0;
            let mut level = self.output_level(w, player, &o, &cap[j]);
            let mut source = None;
            match o.kind {
                kind::COW_PORTAL => {
                    success = w.cow_portal(player);
                    continue;
                }
                // 1.14d stubs `0x00594270` / `0x00594280` return 0 (§9).
                kind::PANDEMONIUM | kind::PANDEMONIUM_FINALE => {
                    success = false;
                    continue;
                }
                kind::NONE => continue,
                _ => {}
            }
            if o.flags & f::MOD != 0 {
                // TODO(cube §7.3): a `mod` output with no captured item
                // (output a without mod/usetype/useitem) dereferences null
                // in the original; d2rs makes nothing.
                let Some(it) = cap[j].item else { continue };
                source = Some(it);
                w.set_item_page(it, 0xFF);
                w.set_item_mode(it, 4);
                let copy = w.duplicate(it, !remove);
                let class = match o.kind {
                    kind::ITEMCODE => u32::from(o.item),
                    kind::ITEMTYPE => self.type_pick(w, o.item, level),
                    kind::USEITEM => cap[j].class.max(0) as u32,
                    kind::USETYPE => cap[j].class as u32,
                    _ => 0,
                };
                if let Some(copy) = copy {
                    w.set_item_class(copy, class);
                    out[j] = w.item_init(copy);
                }
                if let Some(x) = out[j] {
                    w.set_item_mode(x, 4);
                }
                w.set_item_page(it, CUBE_PAGE);
            } else if o.kind == kind::USEITEM {
                let Some(it) = cap[j].item else { continue };
                source = Some(it);
                w.set_item_page(it, 0xFF);
                w.set_item_mode(it, 4);
                out[j] = w.duplicate(it, !remove);
                if let Some(x) = out[j] {
                    w.set_item_mode(x, 4);
                }
                w.set_item_page(it, CUBE_PAGE);
                if o.quality == 9 {
                    // TODO(cube §7.3): tempered rolls on a failed duplicate
                    // are not covered; d2rs skips them (craft unchanged).
                    if let Some(x) = out[j] {
                        let pre = w.tempered_affix(x, true);
                        let suf = w.tempered_affix(x, false);
                        if pre != 0 && suf != 0 {
                            w.set_tempered(x, pre, suf);
                        } else {
                            craft = false;
                        }
                    }
                }
            } else if matches!(o.kind, kind::USETYPE | kind::ITEMCODE | kind::ITEMTYPE) {
                let class = match o.kind {
                    kind::USETYPE => cap[j].class as u32,
                    kind::ITEMCODE => u32::from(o.item),
                    _ => self.type_pick(w, o.item, level),
                };
                let mut req = ItemRequest {
                    player: Some(player),
                    level,
                    class,
                    spawn_type: 4,
                    init_flags: 1,
                    item_format: w.item_format(),
                    quality: o.quality,
                    item_index: o.special,
                    prefix: o.pre,
                    suffix: o.suf,
                    flags2: (if o.flags & f::SOCK != 0 && o.quantity == 0 {
                        0x10
                    } else {
                        0x08
                    }) | (if o.flags & f::ETH != 0 { 0x04 } else { 0x02 }),
                };
                let mut restore_unique = None;
                if o.kind == kind::USETYPE && o.flags & f::REG != 0 {
                    if let Some(it) = cap[j].item {
                        req.quality = w.item_quality(it);
                        let index = w.item_file_index(it);
                        req.item_index = index.wrapping_add(1) as u16;
                        level = cap[j].level;
                        req.level = level;
                        if req.quality == 7 && !w.unique_found(index) {
                            restore_unique = Some(index);
                        }
                    }
                }
                out[j] = w.create_item(&req);
                if let Some(index) = restore_unique {
                    w.set_unique_found(index, false);
                }
            } else {
                continue;
            }
            let Some(x) = out[j] else { continue };
            // §7.6
            success = true;
            if remove {
                w.drop_runeword_stats(x);
                if o.flags & f::REM != 0 {
                    if let Some(src) = source {
                        for s in w.socketed(src) {
                            if let Some(d) = w.duplicate(s, true) {
                                w.set_item_page(d, 0xFF);
                                w.set_item_mode(d, 4);
                                // TODO(cube §7): more than 18 fillers would
                                // overrun the original's stack array.
                                if fillers.len() < MAX_FILLERS {
                                    fillers.push(d);
                                }
                            }
                        }
                    }
                }
            }
            if craft {
                for m in &o.mods {
                    if m.property < 0 {
                        continue;
                    }
                    if m.chance > 0 && m.chance < 100 {
                        let lo = w.item_seed(x).step();
                        if lo % 100 > u32::from(m.chance) {
                            continue;
                        }
                    }
                    // Sign-extended from 16 bits.
                    let prop = CraftProperty {
                        property: m.property,
                        param: i32::from(m.param as i16),
                        min: i32::from(m.min as i16),
                        max: i32::from(m.max as i16),
                    };
                    w.add_craft_property(x, &prop);
                }
            }
            let qty = i32::from(o.quantity);
            if o.flags & f::REP != 0 {
                if self.stackable(w, x) && qty != 0 {
                    let v = qty.min(self.max_stack(w, x));
                    w.set_stat(x, STAT_QUANTITY, v);
                }
                if w.item_flags(x) & item_flags::BROKEN != 0 {
                    w.repair(x);
                } else {
                    let max = w.stat(x, StatRead::Value, STAT_MAX_DURABILITY);
                    if w.stat(x, StatRead::Base, STAT_DURABILITY) < max {
                        w.set_stat(x, STAT_DURABILITY, max);
                    }
                }
            }
            if o.flags & f::RCH != 0 {
                w.recharge(x);
            }
            if o.flags & f::SOCK != 0 {
                if qty != 0 && w.item_sockets(x) == 0 && w.item_flags(x) & item_flags::SOCKETED == 0
                {
                    let mut s = w.max_sockets(x).min(qty);
                    match w.item_quality(x) {
                        4 | 9 => s = s.min(3),
                        5..=8 => s = s.min(1),
                        _ => {}
                    }
                    if s > 0 {
                        w.set_item_flag(x, item_flags::SOCKETED);
                        w.add_sockets(x, s);
                    }
                }
            } else if qty != 0 && self.stackable(w, x) {
                let v = qty.min(self.max_stack(w, x));
                w.set_stat(x, STAT_QUANTITY, v);
            }
        }
        let made: Vec<UnitId> = out.iter().flatten().copied().collect();
        if !success {
            // §8: made outputs are neither placed nor freed (discarded).
            return Transmute {
                record: None,
                committed: false,
                outputs: made,
            };
        }
        // §8 step 1: every page-3 item goes.
        for item in w.inventory(player) {
            if w.item_page(item) == CUBE_PAGE {
                w.remove_cube_item(player, item);
            }
        }
        w.attach_sound(player, SOUND_TRANSMUTE);
        for &x in &made {
            self.place_output(w, player, x, true);
        }
        for &x in &fillers {
            self.place_output(w, player, x, false);
        }
        Transmute {
            record: None,
            committed: true,
            outputs: made,
        }
    }

    /// §8 steps 3–4.
    fn place_output<W: CubeWorld>(&self, w: &mut W, player: UnitId, x: UnitId, hooks: bool) {
        w.set_item_page(x, CUBE_PAGE);
        if !w.place(player, x) {
            w.free_item(x);
            return;
        }
        w.set_item_flag(x, item_flags::IDENTIFIED);
        if !hooks {
            return;
        }
        if let Some(rec) = w.item_class(x).and_then(|c| self.item(c)) {
            if rec.quest != 0 && (rec.code == *b"hst " || rec.code == *b"qf2 ") {
                w.quest_item_hook(player, x, rec.code);
            }
        }
    }

    /// `0x005665F0`: transmute the player's cube contents (§3).
    pub fn transmute<W: CubeWorld>(&self, w: &mut W, player: UnitId) -> Transmute {
        let cube_items: Vec<UnitId> = w
            .inventory(player)
            .into_iter()
            .filter(|&i| w.item_page(i) == CUBE_PAGE)
            .collect();
        let n = cube_items.len();
        if n == 0 {
            return Transmute::default();
        }
        let date = w.local_date();
        for (i, r) in self.recipes.iter().enumerate() {
            if !self.eligible(w, player, r, n) || !self.recipe_op(w, player, r, date) {
                continue;
            }
            if let Some(cap) = self.match_inputs(w, r, &cube_items) {
                let mut t = self.outputs(w, player, r, &cap);
                t.record = Some(i);
                return t;
            }
        }
        Transmute::default()
    }

    /// `0x00568060` for the cube buttons (§1). `None`: another button with
    /// an active interaction (not the cube's; the trade/UI owner).
    pub fn click_button<W: CubeWorld>(
        &self,
        w: &mut W,
        player: UnitId,
        button: u16,
    ) -> Option<u32> {
        let Some((ty, _)) = w.interaction(player) else {
            w.send(player, &[0x77, 0x0C]);
            return Some(0);
        };
        match button {
            BUTTON_CLOSE | BUTTON_TRANSMUTE if ty != INTERACT_CUBE => Some(3),
            BUTTON_CLOSE => {
                w.reset_interaction(player);
                w.inventory_pass(player);
                Some(0)
            }
            BUTTON_TRANSMUTE => {
                self.transmute(w, player);
                Some(0)
            }
            _ => None,
        }
    }

    /// `0x005BF0C0`: use (open) the cube item (§1).
    pub fn open<W: CubeWorld>(&self, w: &mut W, player: UnitId, cube_guid: u32) {
        if w.interacting_with_stash(player) {
            w.reset_interaction(player);
            w.inventory_pass(player);
            w.send(player, &[0x77, 0x11]);
        }
        w.set_interaction(player, INTERACT_CUBE, cube_guid);
        w.send(player, &[0x77, 0x15]);
        w.inventory_pass(player);
    }

    /// `0x0054B790`: C→S 0x2A put an item into the cube (§2). Returns the
    /// result code.
    pub fn put_in<W: CubeWorld>(&self, w: &mut W, player: UnitId, msg: &[u8]) -> u32 {
        if msg.len() != 9 {
            return 3;
        }
        let item = u32::from_le_bytes(msg[1..5].try_into().expect("4 bytes"));
        let cube = u32::from_le_bytes(msg[5..9].try_into().expect("4 bytes"));
        let r = w.put_item_check(player, item);
        if r != 0 {
            return r;
        }
        if !w.cube_check(player, cube) {
            return 1;
        }
        // §2 step 3 (`0x005628C0`).
        w.targeting_reset(player);
        let is_cube = |w: &W, u: UnitId| {
            w.item_mode(u) == 0
                && w.item_class(u)
                    .and_then(|c| self.item(c))
                    .is_some_and(|r| r.code == CUBE_CODE)
        };
        let Some(cube_unit) = w.item_by_guid(cube).filter(|&u| is_cube(w, u)) else {
            return 3;
        };
        if w.trading(player) && w.item_page(cube_unit) != 0 {
            w.attach_sound(player, SOUND_REFUSED_PUT);
            return 0;
        }
        let Some(it) = w.item_by_guid(item) else {
            return 3;
        };
        if !matches!(w.item_mode(it), 3 | 4) || w.item_mode(cube_unit) != 0 {
            return 3;
        }
        w.set_item_page(it, CUBE_PAGE);
        // The placement result is ignored (edge case; open question 6).
        w.place(player, it);
        0
    }
}
