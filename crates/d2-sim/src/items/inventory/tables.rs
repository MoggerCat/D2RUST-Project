// Spec: specs/items/inventory.md (Constants & data dependencies)
//! The table columns the inventory reads, projected from `d2-data` typed
//! records: `inventory` (gridX, gridY), `belts` (numboxes), the combined
//! items array (weapons, armor, misc: invwidth, invheight, reqstr, reqdex,
//! component, belt, autobelt, quest, useable, stackable, maxstack) and
//! `itemtypes` (body, bodyloc1/2, beltable, class, quiver) with the
//! equivalence matrix (`data/runtime-maps.md` §2).

use d2_data::fixup::maps::EquivMatrix;
use d2_data::fixup::FixedSet;
use d2_data::tables::{decode_all, Armor, Belts, Inventory, Itemtypes, Misc, Record, Weapons};

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
    /// `levelreq` (`0x006335F0`, §4.8).
    pub levelreq: u8,
    /// `mindam` / `maxdam` (+0xFE / +0xFF): a shield's smite damage
    /// (`bodies-2.md` §3.4).
    pub mindam: u8,
    pub maxdam: u8,
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
                    levelreq: r.levelreq,
                    mindam: r.mindam,
                    maxdam: r.maxdam,
                }
            }
        }
    )*};
}
inv_item_rec!(Weapons, Armor, Misc);

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
            class: r.class,
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
