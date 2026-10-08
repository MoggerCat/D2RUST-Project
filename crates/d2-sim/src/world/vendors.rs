// Spec: specs/world/vendors.md
//! Vendors: store lists (§1), store generation (§2–§3), trade and gamble
//! open (§4), gamble lists (§5), refresh (§6), buy and sell (§7), repair
//! (§8) and prices (§9).
//!
//! Table data is a projection of `d2-data` typed records
//! ([`VendorTables::from_fixed`]). The vendor part of an NPC record
//! ([`VendorRecord`], `npc.md` §1.1 +0x08…+0x40) is owned here and is meant
//! to be embedded in the NPC record of `world::npc`; the NPC-control seed
//! stays with that record and is passed in. Items, stats, inventories and
//! messages are reached through [`VendorWorld`]; NPC interaction plumbing
//! (`world::npc`, a parallel session) through the narrow [`NpcLink`].
//! Every draw goes through [`Seed`] in the spec's order (Randomness).

pub mod gamble;
pub mod price;
pub mod store;
pub mod trade;

#[cfg(test)]
mod tests;

use d2_data::bin::BinTable;
use d2_data::fixup::maps::EquivMatrix;
use d2_data::fixup::FixedSet;
use d2_data::tables::{
    decode_all, text, Armor, Automagic, Books, Difficultylevels, Itemstatcost, Itemtypes,
    Lowqualityitems, Magicprefix, Magicsuffix, Misc, Monstats, Npc, Record, Setitems, Skills,
    Uniqueitems, Weapons, WrongTable,
};

use crate::units::UnitId;

pub use price::{cost, PriceCtx, PriceItem};

// ------------------------------------------------------------- constants

/// Vendor columns (§1 rule 1).
pub const COLUMNS: usize = 17;
/// Byte offset of `<Vendor>Min` for column 0; Max, MagicMin, MagicMax,
/// MagicLvl follow 17 bytes apart (§1 rule 1).
pub const COLUMN_BASE: usize = 326;
/// Store item level caps per act in Normal (§2).
pub const ACT_CAPS: [i32; 5] = [12, 20, 28, 36, 45];
/// Failure limit of store generation (§3.3).
pub const FAIL_LIMIT: u32 = 32;
/// Gamble list length (§5.1 step 8).
pub const GAMBLE_ITEMS: u32 = 14;
/// Refresh time in milliseconds (§6 rule 3).
pub const REFRESH_MS: u32 = 240_000;
/// Charged-skill cost base (§9.2 rule 11).
pub const CHARGE_BASE: i32 = 10_000;
/// Town levels by act (§6 rule 1).
pub const TOWNS: [u16; 5] = [1, 40, 75, 103, 109];
/// Spaces: an empty code field.
pub const NO_CODE: [u8; 4] = *b"    ";
/// "No upgrade" code of `NightmareUpgrade` / `HellUpgrade` (§3.1 rule 1).
pub const XXX: [u8; 4] = *b"xxx ";
/// Fixed codes (Constants).
pub const CQV: [u8; 4] = *b"cqv ";
pub const AQV: [u8; 4] = *b"aqv ";
pub const RIN: [u8; 4] = *b"rin ";
pub const AMU: [u8; 4] = *b"amu ";
pub const HP4: [u8; 4] = *b"hp4 ";
pub const HP5: [u8; 4] = *b"hp5 ";
pub const MP4: [u8; 4] = *b"mp4 ";
pub const MP5: [u8; 4] = *b"mp5 ";
/// Low-quality name that marks a destroyed try (§3.1 rule 2).
pub const CRACKED: &[u8] = b"Cracked";

/// Transactions (§9.2).
pub mod tx {
    pub const BUY: u32 = 0;
    pub const SELL: u32 = 1;
    pub const GAMBLE: u32 = 2;
    pub const REPAIR: u32 = 3;
}

/// Monstats classes the vendor code tests.
pub mod class {
    pub const GHEED: u16 = 147;
    pub const AKARA: u16 = 148;
    pub const KASHYA: u16 = 150;
    pub const CHARSI: u16 = 154;
    pub const DROGNAN: u16 = 177;
    pub const FARA: u16 = 178;
    pub const GREIZ: u16 = 198;
    pub const ELZIX: u16 = 199;
    pub const LYSANDER: u16 = 202;
    pub const ASHEARA: u16 = 252;
    pub const HRATLI: u16 = 253;
    pub const ALKOR: u16 = 254;
    pub const ORMUS: u16 = 255;
    pub const HALBU: u16 = 257;
    pub const JAMELLA: u16 = 405;
    pub const LARZUK: u16 = 511;
    pub const DREHYA: u16 = 512;
    pub const MALAH: u16 = 513;
    pub const NIHLATHAK: u16 = 514;
    pub const QUAL_KEHK: u16 = 515;
}

/// Stat ids read or written (`sim/stats.md`).
pub mod stat {
    pub const LEVEL: u16 = 12;
    pub const GOLD: u16 = 14;
    pub const GOLD_BANK: u16 = 15;
    pub const ARMORCLASS: u16 = 31;
    pub const QUANTITY: u16 = 70;
    pub const DURABILITY: u16 = 72;
    pub const MAXDURABILITY: u16 = 73;
    pub const REDUCED_PRICES: u16 = 87;
    pub const ITEM_SINGLESKILL: u16 = 107;
    pub const INDESTRUCTIBLE: u16 = 152;
    pub const CHARGED_SKILL: u16 = 204;
    pub const REPLENISH_DURABILITY: u16 = 252;
    pub const REPLENISH_QUANTITY: u16 = 253;
    pub const EXTRA_STACK: u16 = 254;
}

/// Item flags (item data +0x18; `items/generation.md` §1.4).
pub mod flag {
    pub const NEW: u32 = 0x1;
    pub const TARGET: u32 = 0x2;
    pub const IDENTIFIED: u32 = 0x10;
    pub const BROKEN: u32 = 0x100;
    pub const NOSELL: u32 = 0x1000;
    pub const EAR: u32 = 0x10000;
    pub const STARTITEM: u32 = 0x20000;
    pub const ETHEREAL: u32 = 0x40_0000;
    pub const PERSONALIZED: u32 = 0x100_0000;
}

/// Vendor bits of unit +0xC8 (§3.1 rule 5, §7.1 rule 12).
pub mod unit_flag {
    pub const VENDOR: u32 = 0x4;
    pub const TAKEN: u32 = 0x10;
}

/// Item types the vendor code tests (`itemtypes` rows).
pub mod ty {
    pub const PLAY: u16 = 7;
    pub const BOOK: u16 = 18;
    pub const SCRO: u16 = 22;
    pub const QUEST: u16 = 39;
    pub const BODY: u16 = 40;
    pub const ARMO: u16 = 50;
}

/// Item modes (unit +0x10) the vendor code uses.
pub mod mode {
    pub const STORED: u32 = 0;
    pub const CURSOR: u32 = 4;
}

// --------------------------------------------------- per-NPC constants

/// Per-game copy switch (§1 rules 3–4): the column a trader's record gets.
/// Nihlathak gets Larzuk's 15.
pub fn column_of(class: u16) -> Option<usize> {
    use class::*;
    Some(match class {
        GHEED => 1,
        AKARA => 0,
        CHARSI => 2,
        DROGNAN => 5,
        FARA => 3,
        ELZIX => 9,
        LYSANDER => 4,
        ASHEARA => 10,
        HRATLI => 6,
        ALKOR => 7,
        ORMUS => 8,
        HALBU => 12,
        JAMELLA => 13,
        LARZUK | NIHLATHAK => 15,
        DREHYA => 16,
        MALAH => 14,
        _ => return None,
    })
}

/// Global build switch (§1 rule 2): as [`column_of`], but Nihlathak
/// builds column 16 (rule 4).
pub fn global_column_of(class: u16) -> Option<usize> {
    match class {
        class::NIHLATHAK => Some(16),
        c => column_of(c),
    }
}

/// Classes whose record has gamble lists (+0x0C, §1 rule 5).
pub const GAMBLERS: [u16; 6] = [
    class::GHEED,
    class::ELZIX,
    class::ALKOR,
    class::JAMELLA,
    class::DREHYA,
    class::NIHLATHAK,
];

/// Classes whose record gets +0x24 and +0x25 (§1 rule 5).
pub const FLAGGED: [u16; 9] = [
    class::GHEED,
    class::CHARSI,
    class::FARA,
    class::HRATLI,
    class::ASHEARA,
    class::HALBU,
    class::JAMELLA,
    class::LARZUK,
    class::DREHYA,
];

/// Classes that make a hire list at trade open (§4 rule 2).
pub const HIRE_CLASSES: [u16; 4] = [
    class::KASHYA,
    class::QUAL_KEHK,
    class::GREIZ,
    class::ASHEARA,
];

/// Classes skipped by §4 rule 3.
pub const NO_STORE_REFRESH: [u16; 3] = [class::KASHYA, class::QUAL_KEHK, class::GREIZ];

/// Repairing NPCs (§8.1 rule 2).
pub const REPAIRERS: [u16; 5] = [
    class::CHARSI,
    class::FARA,
    class::HRATLI,
    class::HALBU,
    class::LARZUK,
];

/// Store item level (§2): L_p + 5, capped by the act in Normal.
pub fn store_level(player_level: i32, difficulty: u8, act: u8) -> i32 {
    let ilvl = player_level.wrapping_add(5);
    match (difficulty, ACT_CAPS.get(usize::from(act))) {
        (0, Some(&cap)) => ilvl.min(cap),
        _ => ilvl,
    }
}

// ----------------------------------------------------------------- tables

/// The item columns vendors read (weapons, armor, misc share one layout;
/// Constants).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct VendorItem {
    pub code: [u8; 4],
    pub normcode: [u8; 4],
    pub ubercode: [u8; 4],
    pub ultracode: [u8; 4],
    pub cost: u32,
    pub gamble_cost: u32,
    pub minstack: u32,
    pub maxstack: u32,
    pub bitfield1: u32,
    pub version: u16,
    pub level: u8,
    /// Primary and secondary type (i16 reading of the links).
    pub type_: i16,
    pub type2: i16,
    pub durability: u8,
    pub nodurability: u8,
    pub quest: u8,
    pub stackable: u8,
    pub spawnable: u8,
    pub minac: u32,
    pub maxac: u32,
    pub nightmare_upgrade: [u8; 4],
    pub hell_upgrade: [u8; 4],
    pub perm_store: u8,
    /// Vendor columns: (Min, Max, MagicMin, MagicMax, MagicLvl) per column.
    pub columns: [[u8; 5]; COLUMNS],
}

macro_rules! vendor_item {
    ($name:ident, $t:ty) => {
        fn $name(r: &$t, raw: &[u8]) -> VendorItem {
            VendorItem {
                code: r.code,
                normcode: r.normcode,
                ubercode: r.ubercode,
                ultracode: r.ultracode,
                cost: r.cost,
                gamble_cost: r.gamble_cost,
                minstack: r.minstack,
                maxstack: r.maxstack,
                bitfield1: r.bitfield1,
                version: r.version,
                level: r.level,
                type_: r.type_ as i16,
                type2: r.type2 as i16,
                durability: r.durability,
                nodurability: r.nodurability,
                quest: r.quest,
                stackable: r.stackable,
                spawnable: r.spawnable,
                minac: r.minac,
                maxac: r.maxac,
                nightmare_upgrade: r.nightmareupgrade,
                hell_upgrade: r.hellupgrade,
                perm_store: r.permstoreitem,
                columns: std::array::from_fn(|i| {
                    std::array::from_fn(|f| raw[COLUMN_BASE + 17 * f + i])
                }),
            }
        }
    };
}
vendor_item!(from_weapons, Weapons);
vendor_item!(from_armor, Armor);
vendor_item!(from_misc, Misc);

/// The `itemtypes` columns vendors read (Constants).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TypeRec {
    pub repair: u8,
    pub quiver: u16,
    pub throwable: u8,
    pub autostack: u8,
    pub staffmods: u8,
    pub class: u8,
    pub storepage: u8,
}

impl From<&Itemtypes> for TypeRec {
    fn from(r: &Itemtypes) -> Self {
        TypeRec {
            repair: r.repair,
            quiver: r.quiver,
            throwable: r.throwable,
            autostack: r.autostack,
            staffmods: r.staffmods,
            class: r.class,
            storepage: r.storepage,
        }
    }
}

/// A cost multiplier and addend (`magicprefix` +136/+140, `uniqueitems`
/// +124/+128, `setitems` +56/+60, `skills` +564/+568, `itemstatcost`
/// +16/+20).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CostMod {
    pub mult: i32,
    pub add: i32,
}

/// A unique item row: its cost modifier and the flag byte +0x2C (§7.2
/// rule 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct UniqueCost {
    pub cost: CostMod,
    pub flags: u8,
}

/// A skill row's cost modifier and `reqlevel` (§9.2 (A)–(C)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SkillCost {
    pub cost: CostMod,
    pub reqlevel: i32,
}

/// An `itemstatcost` row's cost columns (§9.2 (B)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct StatCost {
    pub cost: CostMod,
    pub valshift: u8,
    pub encode: u8,
}

/// An `npc.txt` row (§9.2 rule 9, §9.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct NpcPrices {
    pub class: u32,
    pub sell: i32,
    pub buy: i32,
    pub rep: i32,
    /// Quests A, B, C: (flag slot, sell, buy, rep multipliers).
    pub quests: [(u32, i32, i32, i32); 3],
    /// `max buy` per difficulty.
    pub max_buy: [i32; 3],
}

impl From<&Npc> for NpcPrices {
    fn from(r: &Npc) -> Self {
        let i = |v: u32| v as i32;
        NpcPrices {
            class: r.npc,
            sell: i(r.sell_mult),
            buy: i(r.buy_mult),
            rep: i(r.rep_mult),
            quests: [
                (
                    r.questflag_a,
                    i(r.questsellmult_a),
                    i(r.questbuymult_a),
                    i(r.questrepmult_a),
                ),
                (
                    r.questflag_b,
                    i(r.questsellmult_b),
                    i(r.questbuymult_b),
                    i(r.questrepmult_b),
                ),
                (
                    r.questflag_c,
                    i(r.questsellmult_c),
                    i(r.questbuymult_c),
                    i(r.questrepmult_c),
                ),
            ],
            max_buy: [i(r.max_buy), i(r.max_buy_n), i(r.max_buy_h)],
        }
    }
}

/// `difficultylevels` gamble odds (§5.1): rare, set, unique, uber, ultra.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct GambleOdds {
    pub rare: u32,
    pub set: u32,
    pub unique: u32,
    pub uber: i32,
    pub ultra: i32,
}

impl From<&Difficultylevels> for GambleOdds {
    fn from(r: &Difficultylevels) -> Self {
        GambleOdds {
            rare: r.gamblerare,
            set: r.gambleset,
            unique: r.gambleunique,
            uber: r.gambleuber as i32,
            ultra: r.gambleultra as i32,
        }
    }
}

/// Everything the vendor code reads from the tables.
#[derive(Clone, Debug, Default)]
pub struct VendorTables {
    /// The combined items array: weapons, armor, misc (`loading.md` §9).
    pub items: Vec<VendorItem>,
    pub itemtypes: Vec<TypeRec>,
    /// Itemtypes equivalence (`data/runtime-maps.md` §2).
    pub equiv: EquivMatrix,
    /// Magic affix array (suffixes, prefixes, automagic): cost columns.
    pub magic: Vec<CostMod>,
    pub uniques: Vec<UniqueCost>,
    pub setitems: Vec<CostMod>,
    /// `books.costpercharge` per row.
    pub books: Vec<i32>,
    pub skills: Vec<SkillCost>,
    pub stats: Vec<StatCost>,
    /// Layer split `stuff` and its mask (`runtime-maps.md` §3).
    pub stat_shift: u32,
    pub stat_mask: u32,
    /// `monstats` Level, Level(N), Level(H) per class.
    pub monster_levels: Vec<[u16; 3]>,
    /// `monstats` classes with `interact`, in row order (§1 rule 2).
    pub interact: Vec<u16>,
    pub npc: Vec<NpcPrices>,
    pub difficulty: Vec<GambleOdds>,
    /// Low-quality names (`lowqualityitems` Name text).
    pub lowquality: Vec<Vec<u8>>,
    /// Gamble index and thresholds (`runtime-maps.md` §7).
    pub gamble_index: Option<Vec<u32>>,
    pub gamble_thresholds: Vec<u32>,
    /// Mask of `0x006CE270` tested against a unique's flag byte +0x2C
    /// (§7.2 rule 7: entry 2 of the bit table `0x006CE268`, value 4, the
    /// `carry1` bit; V1).
    pub unique_nosell_mask: u8,
}

/// `0x006CE270` (§7.2 rule 7): 4, the `carry1` bit of uniqueitems +0x2C.
pub const UNIQUE_NOSELL_MASK: u8 = 0x04;

/// A table the projection needs is missing or has the wrong layout.
#[derive(Debug, thiserror::Error)]
pub enum TableError {
    #[error("table {0} missing")]
    Missing(&'static str),
    #[error(transparent)]
    Wrong(#[from] WrongTable),
}

fn get<'a>(f: &'a FixedSet, name: &'static str) -> Result<&'a BinTable, TableError> {
    f.table(name).ok_or(TableError::Missing(name))
}

fn typed<T: Record>(f: &FixedSet) -> Result<Vec<T>, TableError> {
    Ok(decode_all::<T>(get(f, T::TABLE)?)?)
}

fn cost_mod(mult: u32, add: u32) -> CostMod {
    CostMod {
        mult: mult as i32,
        add: add as i32,
    }
}

impl VendorTables {
    /// Projects the fixed-up table set (`d2_data::fixup::apply`).
    pub fn from_fixed(f: &FixedSet) -> Result<Self, TableError> {
        let mut items = Vec::new();
        for (r, raw) in typed::<Weapons>(f)?.iter().zip(get(f, "weapons")?.iter()) {
            items.push(from_weapons(r, raw));
        }
        for (r, raw) in typed::<Armor>(f)?.iter().zip(get(f, "armor")?.iter()) {
            items.push(from_armor(r, raw));
        }
        for (r, raw) in typed::<Misc>(f)?.iter().zip(get(f, "misc")?.iter()) {
            items.push(from_misc(r, raw));
        }
        let mut magic: Vec<CostMod> = typed::<Magicsuffix>(f)?
            .iter()
            .map(|r| cost_mod(r.multiply, r.add))
            .collect();
        magic.extend(
            typed::<Magicprefix>(f)?
                .iter()
                .map(|r| cost_mod(r.multiply, r.add)),
        );
        magic.extend(
            typed::<Automagic>(f)?
                .iter()
                .map(|r| cost_mod(r.multiply, r.add)),
        );
        let monstats = typed::<Monstats>(f)?;
        Ok(VendorTables {
            items,
            itemtypes: typed::<Itemtypes>(f)?.iter().map(TypeRec::from).collect(),
            equiv: f.itemtypes_equiv.clone(),
            magic,
            uniques: typed::<Uniqueitems>(f)?
                .iter()
                .zip(get(f, "uniqueitems")?.iter())
                .map(|(r, raw)| UniqueCost {
                    cost: cost_mod(r.cost_mult, r.cost_add),
                    flags: raw[0x2C],
                })
                .collect(),
            setitems: typed::<Setitems>(f)?
                .iter()
                .map(|r| cost_mod(r.cost_mult, r.cost_add))
                .collect(),
            books: typed::<Books>(f)?
                .iter()
                .map(|r| r.costpercharge as i32)
                .collect(),
            skills: typed::<Skills>(f)?
                .iter()
                .map(|r| SkillCost {
                    cost: cost_mod(r.cost_mult, r.cost_add),
                    // An i16 field (§9.2 (C)).
                    reqlevel: i32::from(r.reqlevel as i16),
                })
                .collect(),
            stats: typed::<Itemstatcost>(f)?
                .iter()
                .map(|r| StatCost {
                    cost: cost_mod(r.multiply, r.add),
                    valshift: r.valshift,
                    encode: r.encode,
                })
                .collect(),
            stat_shift: f.stat_stuff,
            stat_mask: f.stat_mask,
            monster_levels: monstats
                .iter()
                .map(|m| [m.level, m.level_n, m.level_h])
                .collect(),
            interact: monstats
                .iter()
                .enumerate()
                .filter(|(_, m)| m.interact)
                .map(|(i, _)| i as u16)
                .collect(),
            npc: typed::<Npc>(f)?.iter().map(NpcPrices::from).collect(),
            difficulty: typed::<Difficultylevels>(f)?
                .iter()
                .map(GambleOdds::from)
                .collect(),
            lowquality: typed::<Lowqualityitems>(f)?
                .iter()
                .map(|r| text(&r.name).to_vec())
                .collect(),
            gamble_index: f.gamble.index.clone(),
            gamble_thresholds: f.gamble.thresholds.to_vec(),
            unique_nosell_mask: UNIQUE_NOSELL_MASK,
        })
    }

    pub fn item(&self, i: usize) -> Option<&VendorItem> {
        self.items.get(i)
    }

    /// The item code map's find (first record with the code).
    pub fn find_code(&self, code: [u8; 4]) -> Option<usize> {
        self.items.iter().position(|r| r.code == code)
    }

    /// The primary type's itemtypes row of item `i`.
    pub fn type_of(&self, i: usize) -> Option<&TypeRec> {
        let t = self.item(i)?.type_;
        usize::try_from(t).ok().and_then(|t| self.itemtypes.get(t))
    }

    /// "Item is type T" (`0x00629BB0`, `items/generation.md` §1.3).
    pub fn is_type(&self, i: usize, t: u16) -> bool {
        let Some(r) = self.item(i) else { return false };
        let t = usize::from(t);
        let eq = |a: i16| a >= 0 && self.equiv.get(a as usize, t);
        eq(r.type_) || (r.type2 > 0 && eq(r.type2))
    }

    /// The `npc.txt` row of a class (`0x00656900`).
    pub fn npc_row(&self, class: u16) -> Option<&NpcPrices> {
        self.npc.iter().find(|r| r.class == u32::from(class))
    }

    /// A code field that names an item: ≠ 0 and ≠ four spaces, and found
    /// in the code map.
    pub fn valid_code(&self, code: [u8; 4]) -> Option<usize> {
        if code == [0; 4] || code == NO_CODE {
            return None;
        }
        self.find_code(code)
    }
}

// ------------------------------------------------------------ store lists

/// One item-list entry (§1 rule 2: 12 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ColumnEntry {
    pub min: u8,
    pub max: u8,
    pub magic_min: u8,
    pub magic_max: u8,
    pub code: [u8; 4],
    pub magic_lvl: u8,
}

/// One column's two lists (§1 rule 2).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Column {
    pub items: Vec<ColumnEntry>,
    pub perm: Vec<[u8; 4]>,
}

impl Column {
    /// Builds column `i` from the combined item array (`0x00536D50`).
    pub fn build(t: &VendorTables, i: usize) -> Self {
        let mut c = Column::default();
        for r in &t.items {
            let [min, max, magic_min, magic_max, magic_lvl] = r.columns[i];
            if r.spawnable == 0 || (max == 0 && magic_max == 0) {
                continue;
            }
            if r.perm_store != 0 {
                c.perm.push(r.code);
            } else {
                c.items.push(ColumnEntry {
                    min,
                    max,
                    magic_min,
                    magic_max,
                    code: r.code,
                    magic_lvl,
                });
            }
        }
        c
    }
}

/// The global lists (`0x00536F80`, once per server start): a column is
/// built for each `interact` class the global switch maps (§1 rule 2);
/// the others stay empty.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GlobalLists {
    pub columns: Vec<Column>,
}

impl GlobalLists {
    pub fn build(t: &VendorTables) -> Self {
        let mut columns = vec![Column::default(); COLUMNS];
        let mut built = [false; COLUMNS];
        for &class in &t.interact {
            if let Some(i) = global_column_of(class) {
                if !built[i] {
                    built[i] = true;
                    columns[i] = Column::build(t, i);
                }
            }
        }
        GlobalLists { columns }
    }
}

// ----------------------------------------------------------------- record

/// One player's gamble list at an NPC (record +0x08 node).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GambleList {
    pub player: u32,
    /// The node's inventory, in placement order.
    pub items: Vec<UnitId>,
}

/// One player's vendor-chain node (record +0x18).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ChainNode {
    pub player: u32,
    pub gamble_mode: bool,
}

/// A node of the NPC event list (record +0x14, §3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventNode {
    pub unit: UnitId,
    pub arg: u32,
    pub kind: u32,
    pub deferred: bool,
}

/// The vendor part of an NPC record (`npc.md` §1.1). `world::npc`
/// embeds it; `class`, `act` and `trader` are copies of the NPC table
/// fields (`npc.md` §1.2).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct VendorRecord {
    pub class: u16,
    pub act: u8,
    pub trader: bool,
    /// The store items of the NPC inventory (+0x04), in creation order.
    pub store: Vec<UnitId>,
    /// +0x08, newest first.
    pub gamble_lists: Vec<GambleList>,
    /// +0x0C.
    pub has_gamble: bool,
    /// +0x14.
    pub events: Vec<EventNode>,
    /// +0x18, newest first.
    pub chain: Vec<ChainNode>,
    /// +0x1C.
    pub has_traded: bool,
    /// +0x20.
    pub store_generated: bool,
    /// +0x24, +0x25 (no reader, `npc.md` Open question 3).
    pub flag24: bool,
    pub flag25: bool,
    /// +0x27.
    pub refresh_pending: bool,
    /// +0x28 (host milliseconds, §6).
    pub store_time: u32,
    /// +0x2C/+0x30 and +0x34/+0x38.
    pub items: Vec<ColumnEntry>,
    pub perm: Vec<[u8; 4]>,
    /// +0x40: NPC GUID of the last trade.
    pub last_npc: u32,
}

impl VendorRecord {
    /// The record's vendor data at creation (`npc.md` §1.1 step 5, §1
    /// rules 3–5): a trader gets the copy of its column and its flags; a
    /// non-trader gets empty lists.
    pub fn new(class: u16, act: u8, trader: bool, globals: &GlobalLists) -> Self {
        let mut r = VendorRecord {
            class,
            act,
            trader,
            ..Default::default()
        };
        if !trader {
            return r;
        }
        if let Some(i) = column_of(class) {
            let c = &globals.columns[i];
            r.items = c.items.clone();
            r.perm = c.perm.clone();
        }
        r.has_gamble = GAMBLERS.contains(&class);
        r.flag24 = FLAGGED.contains(&class);
        r.flag25 = r.flag24;
        r
    }

    /// The player's gamble list here.
    pub fn gamble_list(&self, player: u32) -> Option<&GambleList> {
        self.gamble_lists.iter().find(|g| g.player == player)
    }

    /// The player's vendor-chain node here.
    pub fn chain_node(&self, player: u32) -> Option<&ChainNode> {
        self.chain.iter().find(|n| n.player == player)
    }

    /// `0x00536CB0`: the player's node, created (prepended) on first use.
    pub fn chain_node_mut(&mut self, player: u32) -> &mut ChainNode {
        let i = match self.chain.iter().position(|n| n.player == player) {
            Some(i) => i,
            None => {
                self.chain.insert(
                    0,
                    ChainNode {
                        player,
                        gamble_mode: false,
                    },
                );
                0
            }
        };
        &mut self.chain[i]
    }
}

// ------------------------------------------------------------------ seams

/// The NPC transaction message S→C 0x2A (`npc.md` §9), without the
/// unwritten bytes 3–6. `gold` is the player's stat 14 after the
/// transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transaction {
    pub kind: u8,
    pub code: u8,
    pub guid: u32,
    pub gold: i32,
}

/// No GUID (−1) in a 0x2A.
pub const NO_GUID: u32 = 0xFFFF_FFFF;

/// Seam: NPC interaction plumbing owned by `world::npc` (`npc.md` §2–§4,
/// §7). Expected provider: the NPC module (parallel session).
pub trait NpcLink {
    /// The NPC unit with this GUID (a monster with an interaction list).
    fn npc_by_guid(&self, guid: u32) -> Option<UnitId>;
    /// The NPC unit's monstats class.
    fn npc_class(&self, npc: UnitId) -> u16;
    /// Whether `npc` is the player's interact unit (`npc.md` §2 start
    /// step 4).
    fn is_interact_unit(&self, player: UnitId, npc: UnitId) -> bool;
    /// Whether the NPC's interaction list is empty (§6 rule 3).
    fn interaction_empty(&self, npc: UnitId) -> bool;
    /// Record +0x21 "hire list made" of the class's record.
    fn hire_list_made(&self, class: u16) -> bool;
    fn set_hire_list_made(&mut self, class: u16);
    /// Makes the class's hire list (`npc.md` §7.1). The list draws from
    /// the NPC-control seed, which the trade open holds for store
    /// generation (§4 rule 2): it is lent here as `seed`.
    fn make_hire_list(&mut self, class: u16, seed: &mut crate::rng::Seed);
}

/// Seam: everything else vendors reach (game fields, units, stats,
/// items, inventories, messages). Expected providers: game creation,
/// units / stats, the items group (creation, copy, inventory placement)
/// and the transport.
pub trait VendorWorld: NpcLink {
    // ---- game
    /// Game +0x6D: 0 normal, 1 nightmare, 2 hell.
    fn difficulty(&self) -> u8;
    /// Game +0x70.
    fn expansion(&self) -> bool;
    /// Game +0x78 (item format).
    fn item_format(&self) -> u16;
    /// Game type (§6 rule 2 skips type 3).
    fn game_type(&self) -> u8;

    // ---- units and stats
    fn guid(&self, unit: UnitId) -> u32;
    fn player_by_guid(&self, guid: u32) -> Option<UnitId>;
    fn item_by_guid(&self, guid: u32) -> Option<UnitId>;
    /// Unit total of a stat.
    fn stat(&self, unit: UnitId, id: u16, layer: u16) -> i32;
    /// Base stat.
    fn base_stat(&self, unit: UnitId, id: u16, layer: u16) -> i32;
    /// Sets the base stat.
    fn set_stat(&mut self, unit: UnitId, id: u16, layer: u16, value: i32);
    /// The player's quest slot word of difficulty `d` (`quests.md` §1).
    fn quest_slot(&self, player: UnitId, difficulty: u8, slot: u32) -> u16;
    /// Living players whose room is in `level` (`0x005538D0`).
    fn players_in_level(&self, level: u16) -> i32;
    /// The player's room level (§6 rule 2).
    fn player_level_id(&self, player: UnitId) -> u16;
    /// Quest intro hook on entering a town (`quests.md` §6.7).
    fn town_entered(&mut self, player: UnitId, level: u16);
    /// The player's carried-gold cap (`0x00622E70`) and stash cap
    /// (`0x00623460`).
    fn gold_cap(&self, player: UnitId) -> i32;
    fn stash_cap(&self, player: UnitId) -> i32;
    /// Drops a gold pile at the player (`0x0055A090`).
    fn drop_gold(&mut self, player: UnitId, amount: i32);
    /// Player data +0x6C (GUID of the last bought item).
    fn last_bought(&self, player: UnitId) -> u32;
    fn set_last_bought(&mut self, player: UnitId, guid: u32);
    /// Whether the player holds a cursor item.
    fn has_cursor_item(&self, player: UnitId) -> bool;

    // ---- items
    /// Item creation `0x00559CE0` (owner the NPC of `npc_class`'s record,
    /// mode 4, quality, item level; seeds from the game seed).
    fn create_item(
        &mut self,
        npc_class: u16,
        record: usize,
        quality: u8,
        ilvl: i32,
    ) -> Option<UnitId>;
    /// Copy `0x0055A2A0`.
    fn copy_item(&mut self, item: UnitId) -> Option<UnitId>;
    /// Destroys an item unit.
    fn destroy_item(&mut self, item: UnitId);
    /// The item's combined record index.
    fn item_record(&self, item: UnitId) -> usize;
    fn item_quality(&self, item: UnitId) -> u8;
    fn item_file_index(&self, item: UnitId) -> i32;
    /// Item flags (item data +0x18).
    fn item_flags(&self, item: UnitId) -> u32;
    fn set_item_flags(&mut self, item: UnitId, flags: u32);
    /// Unit +0xC8 bits ([`unit_flag`]).
    fn or_unit_flags(&mut self, item: UnitId, bits: u32);
    /// Item mode (unit +0x10).
    fn item_mode(&self, item: UnitId) -> u32;
    fn set_item_mode(&mut self, item: UnitId, mode: u32);
    fn set_item_page(&mut self, item: UnitId, page: u8);
    /// Has filled sockets (`0x0055F590`).
    fn has_filled_sockets(&self, item: UnitId) -> bool;
    /// The item's price inputs (§9.2); `None` when not an item.
    fn price_item(&self, item: UnitId) -> Option<PriceItem>;
    /// Recharge `0x0055FE80`; broken-item repair `0x0055F900`.
    fn recharge(&mut self, item: UnitId);
    fn repair_broken(&mut self, item: UnitId);
    /// Identify `0x00562590`.
    fn identify(&mut self, item: UnitId);
    /// S→C 0x3E: an item stat to the player.
    fn send_item_stat(&mut self, player: UnitId, item: UnitId, stat: u16);
    /// S→C 0x2A.
    fn send_transaction(&mut self, player: UnitId, t: Transaction);

    // ---- NPC inventories
    /// A new (empty) store inventory for the record (`npc` = the NPC unit
    /// it is also assigned to, §6 rule 3).
    fn new_store_inventory(&mut self, npc_class: u16, npc: Option<UnitId>);
    /// Places an item in the store grid (`0x00560200`) on its page.
    fn place_in_store(&mut self, npc_class: u16, item: UnitId) -> bool;
    /// Removes a store item (`0x00536510`).
    fn remove_store_item(&mut self, npc_class: u16, item: UnitId);
    /// Takes a store item out of the grid (`0x005766D0`: removed, then
    /// re-added to the trade inventory so the client drops it; the caller
    /// sets [`unit_flag::TAKEN`] in between).
    fn take_from_store(&mut self, npc_class: u16, item: UnitId);
    /// New gamble-list inventory; place / remove there.
    fn place_in_gamble(&mut self, npc_class: u16, player: u32, item: UnitId) -> bool;
    fn remove_gamble_item(&mut self, npc_class: u16, player: u32, item: UnitId);
    /// Refresh the NPC inventory (`0x00621000`).
    fn refresh_npc_inventory(&mut self, npc: UnitId);
    /// Adds the item to the NPC's trade inventory (`0x0063CC70` /
    /// `0x00576C30`; the client receives 0x9C action 11).
    fn add_trade_inventory(&mut self, npc_class: u16, item: UnitId);

    // ---- player inventories (§7, §8)
    /// Item belongs to the player (`0x00557FF0`).
    fn owns_item(&self, player: UnitId, item: UnitId) -> bool;
    /// Item is in the player's inventory (§8.1 rule 4).
    fn in_inventory(&self, player: UnitId, item: UnitId) -> bool;
    /// Equipped items over the 13 body locations (`0x0062FE60`).
    fn equipped_items(&self, player: UnitId) -> Vec<UnitId>;
    /// A backpack tome matching the scroll with free space (`0x0055F640`).
    fn find_tome(&mut self, player: UnitId, scroll: UnitId) -> Option<(UnitId, i32)>;
    /// Tome quantity += k (`0x0055F6E0`).
    fn add_to_tome(&mut self, tome: UnitId, k: i32);
    /// A partial stack of the same item (`0x00577700`: equipped first,
    /// then backpack) with its free space.
    fn find_partial_stack(&mut self, player: UnitId, item: UnitId) -> Option<(UnitId, i32)>;
    /// Can go to the belt (`0x00628BA0`); put it there (`0x0055E9B0`).
    fn can_belt(&mut self, player: UnitId, item: UnitId) -> bool;
    fn put_in_belt(&mut self, player: UnitId, item: UnitId) -> bool;
    /// Arrow / bolt auto-equip at buy (`0x00562E00`, Open question 3).
    fn equip_ammo(&mut self, player: UnitId, item: UnitId) -> bool;
    /// Auto-place in the backpack (`0x00560200`).
    fn place_in_backpack(&mut self, player: UnitId, item: UnitId) -> bool;
    /// Takes a cursor item (`0x0055EEA0`).
    fn take_from_cursor(&mut self, player: UnitId, item: UnitId) -> bool;
    /// `0x00576E40`: lowers the matching scroll / book skill quantity
    /// (floor 0, right skill cleared, S→C 0x22).
    fn lower_book_skill(&mut self, player: UnitId, item: UnitId, n: i32);
    /// A stored item: cell := page, item update message, removed
    /// (`0x0055DF10`).
    fn remove_stored(&mut self, player: UnitId, item: UnitId);
    /// Unequip (`0x00560CD0`).
    fn unequip(&mut self, player: UnitId, item: UnitId) -> bool;
}
