// Spec: specs/items/inventory.md (Constants & data dependencies)
//! The table columns the inventory reads, projected from `d2-data` typed
//! records: `inventory` (gridX, gridY), `belts` (numboxes), the combined
//! items array (weapons, armor, misc: invwidth, invheight, reqstr, reqdex,
//! component, belt, autobelt, quest, useable, stackable, maxstack) and
//! `itemtypes` (body, bodyloc1/2, beltable, class, quiver) with the
//! equivalence matrix (`data/runtime-maps.md` §2).

use d2_data::fixup::maps::EquivMatrix;
use d2_data::fixup::FixedSet;
use d2_data::tables::{
    decode_all, Armor, Belts, Books, Inventory, Itemstatcost, Itemtypes, Misc, Record, Weapons,
};

use super::equip::CLASS_NONE;
use crate::items::tables::TableError;

/// An `inventory.bin` record: the grid size (§1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct GridRec {
    pub grid_x: u8,
    pub grid_y: u8,
}

/// The items columns used here (weapons, armor and misc share one layout).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct InvItemRec {
    pub code: [u8; 4],
    /// Primary type (`type`), i16 reading of the link.
    pub type_: i16,
    pub type2: i16,
    pub invwidth: u8,
    pub invheight: u8,
    pub reqstr: u16,
    pub reqdex: u16,
    pub component: u8,
    /// Belt type (`belt`, +0x130): a `belts` record.
    pub belt: u8,
    pub autobelt: u8,
    pub quest: u8,
    pub useable: u8,
    pub stackable: u8,
    pub maxstack: u32,
    /// `2handed` (`0x006289C0`, §4.7).
    pub twohanded: u8,
    /// `1or2handed` (read by `0x0062A1E0`, §4.4 step 4).
    pub onetwohanded: u8,
    /// `pSpell` (+0x94): the item-use entry (`items/use.md` §1 step 2.2).
    pub pspell: u32,
    /// `levelreq` (`0x006335F0`, §4.8).
    pub levelreq: u8,
    /// `mindam` / `maxdam` (+0xFE / +0xFF): a shield's smite damage
    /// (`bodies-2.md` §3.4).
    pub mindam: u8,
    pub maxdam: u8,
    /// `wclass` / `2handedwclass` (weapons +0xC0 / +0xC4): COF weapon
    /// class codes (`render/unit-composite.md` §2.1); `wclass` is also the
    /// hand class `0x00623C60` reads (`bow `, `xbw `, …;
    /// `world/vendors.md` §7.1.1).
    pub wclass: [u8; 4],
    pub wclass2: [u8; 4],
    /// `rangeadder` (weapons +0x104): the melee reach `0x00622870` adds
    /// for a player (`combat/hit.md` §7.3).
    pub rangeadder: u8,
    /// The use fields of `items/use.md` §3.1 (items record +0x98…+0xB0):
    /// `state` (i16), `stat1`–`stat3` (i16, −1 = none), `calc1`–`calc3`
    /// and `len` (offsets into [`ItemUseTables::code`]).
    pub use_state: i16,
    pub use_stat: [i16; 3],
    pub use_calc: [u32; 3],
    pub use_len: u32,
}

macro_rules! inv_item_rec {
    ($($t:ty),*) => {$(
        impl From<&$t> for InvItemRec {
            fn from(r: &$t) -> Self {
                InvItemRec {
                    code: r.code,
                    type_: r.type_ as i16,
                    type2: r.type2 as i16,
                    invwidth: r.invwidth,
                    invheight: r.invheight,
                    reqstr: r.reqstr,
                    reqdex: r.reqdex,
                    component: r.component,
                    belt: r.belt,
                    autobelt: r.autobelt,
                    quest: r.quest,
                    useable: r.useable,
                    stackable: r.stackable,
                    maxstack: r.maxstack,
                    twohanded: r.f_2handed,
                    onetwohanded: r.f_1or2handed,
                    pspell: r.pspell,
                    levelreq: r.levelreq,
                    mindam: r.mindam,
                    maxdam: r.maxdam,
                    wclass: r.wclass,
                    wclass2: r.f_2handedwclass,
                    rangeadder: r.rangeadder,
                    use_state: r.state as i16,
                    use_stat: [r.stat1 as i16, r.stat2 as i16, r.stat3 as i16],
                    use_calc: [r.calc1, r.calc2, r.calc3],
                    use_len: r.len,
                }
            }
        }
    )*};
}
inv_item_rec!(Weapons, Armor, Misc);

/// A `books` row (`items/use.md` §1 step 2.1, `inventory-moves.md`
/// §7.18 step 6): the use entry and the scroll / tome skills.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct InvBookRec {
    /// `pSpell` (+0x04).
    pub pspell: u32,
    /// `scrollskill` (+0x08), `bookskill` (+0x0C): skill ids.
    pub scrollskill: i32,
    pub bookskill: i32,
}

impl From<&Books> for InvBookRec {
    fn from(r: &Books) -> Self {
        InvBookRec {
            pspell: r.pspell,
            scrollskill: r.scrollskill as i32,
            bookskill: r.bookskill as i32,
        }
    }
}

/// The itemtypes columns used here (§1, §3, §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct InvTypeRec {
    pub code: [u8; 4],
    pub body: u8,
    pub bodyloc1: u8,
    pub bodyloc2: u8,
    pub beltable: u8,
    pub quiver: u16,
    /// Class restriction (`class`, +33): a playerclass row, 7 = none.
    pub class: u8,
}

impl From<&Itemtypes> for InvTypeRec {
    fn from(r: &Itemtypes) -> Self {
        InvTypeRec {
            code: r.code,
            body: r.body,
            bodyloc1: r.bodyloc1,
            bodyloc2: r.bodyloc2,
            beltable: r.beltable,
            quiver: r.quiver,
            class: type_class(r.class),
        }
    }
}

/// `inventory.md` §4.2 step 7 revision (PROVISIONAL, REC-513): the
/// bin's `class` of an empty cell is 0xFF (`link8` miss); the class
/// getter's "none" is 7, so every value ≥ 7 is none.
pub fn type_class(raw: u8) -> u8 {
    if raw >= CLASS_NONE {
        CLASS_NONE
    } else {
        raw
    }
}

#[cfg(test)]
mod type_class_tests {
    use super::*;

    // Covers: specs/items/inventory.md §4.2
    /// The 1.14d `itemtypes.bin` values: `abow` 0, `orb ` 1, `h2h ` 6,
    /// `belt` / `helm` / `wand` 0xFF (none).
    #[test]
    fn the_empty_class_cell_is_none() {
        for (raw, class) in [(0, 0), (1, 1), (6, 6), (0xFF, CLASS_NONE), (7, CLASS_NONE)] {
            assert_eq!(type_class(raw), class, "{raw:#x}");
        }
    }
}

/// Everything the inventory reads from the tables.
#[derive(Clone, Debug, Default)]
pub struct InvTables {
    /// `inventory.bin` records (32 in 1.14d; the server uses 0–15).
    pub grids: Vec<GridRec>,
    /// `belts.bin` `numboxes` per record (14 in 1.14d).
    pub belts: Vec<u8>,
    /// The combined items array: weapons, armor, misc.
    pub items: Vec<InvItemRec>,
    pub itemtypes: Vec<InvTypeRec>,
    /// Itemtypes equivalence (`data/runtime-maps.md` §2).
    pub equiv: EquivMatrix,
    /// `books` rows (`items/use.md` §1); empty: no row.
    pub books: Vec<InvBookRec>,
    /// What the item-use entries read besides the item records
    /// (`items/use.md` §3.1).
    pub item_use: ItemUseTables,
}

/// The item-use data of `items/use.md` §3.1 outside the items records.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemUseTables {
    /// The items code buffer the use calcs point into
    /// (`data/calc-expressions.md` §1.1); empty: every calc reads 0.
    pub code: Vec<u8>,
    /// `itemstatcost` `maxstat` (+0x32, i16; −1 = none) by stat id: the
    /// cap of a direct add (§3.1 step 2, `len` ≤ 0).
    pub maxstat: Vec<i16>,
}

fn typed<T: Record>(f: &FixedSet) -> Result<Vec<T>, TableError> {
    let t = f.table(T::TABLE).ok_or(TableError::Missing(T::TABLE))?;
    Ok(decode_all::<T>(t)?)
}

impl InvTables {
    /// Projects the fixed-up table set (`d2_data::fixup::apply`).
    pub fn from_fixed(f: &FixedSet) -> Result<Self, TableError> {
        let mut items: Vec<InvItemRec> = typed::<Weapons>(f)?.iter().map(Into::into).collect();
        items.extend(typed::<Armor>(f)?.iter().map(InvItemRec::from));
        items.extend(typed::<Misc>(f)?.iter().map(InvItemRec::from));
        Ok(InvTables {
            grids: typed::<Inventory>(f)?
                .iter()
                .map(|r| GridRec {
                    grid_x: r.gridx,
                    grid_y: r.gridy,
                })
                .collect(),
            belts: typed::<Belts>(f)?.iter().map(|r| r.numboxes).collect(),
            items,
            itemtypes: typed::<Itemtypes>(f)?.iter().map(Into::into).collect(),
            equiv: f.itemtypes_equiv.clone(),
            books: typed::<Books>(f)?.iter().map(Into::into).collect(),
            item_use: ItemUseTables {
                code: f.items_code.clone(),
                maxstat: typed::<Itemstatcost>(f)?
                    .iter()
                    .map(|r| r.maxstat as i16)
                    .collect(),
            },
        })
    }

    pub fn item(&self, record: usize) -> Option<&InvItemRec> {
        self.items.get(record)
    }

    /// Itemtypes row `t` (`t` < 0 or out of range → none).
    pub fn itemtype(&self, t: i16) -> Option<&InvTypeRec> {
        usize::try_from(t).ok().and_then(|t| self.itemtypes.get(t))
    }

    /// The primary type's itemtypes row (`items/generation.md` §1.3).
    pub fn itype_of(&self, record: usize) -> Option<&InvTypeRec> {
        self.item(record).and_then(|r| self.itemtype(r.type_))
    }

    /// "Item is type T" (`0x00629BB0`, `items/generation.md` §1.3): the
    /// `type` row is equivalent to T or, when `type2` > 0, `type2` is.
    pub fn is_type(&self, record: usize, t: i16) -> bool {
        let Some(r) = self.item(record) else {
            return false;
        };
        let eq = |a: i16| a >= 0 && t >= 0 && self.equiv.get(a as usize, t as usize);
        eq(r.type_) || (r.type2 > 0 && eq(r.type2))
    }

    /// Item size `invwidth` × `invheight` (`None` without a record).
    pub fn size(&self, record: usize) -> Option<(u8, u8)> {
        self.item(record).map(|r| (r.invwidth, r.invheight))
    }

    /// `numboxes` of a `belts` record.
    pub fn numboxes(&self, belt: usize) -> Option<u8> {
        self.belts.get(belt).copied()
    }
}
