// Spec: specs/items/treasure.md
//! Treasure classes and drops: the TC runtime form ([`runtime`], §1–§2),
//! monster and chest drops ([`drop`], §3–§4), the TC walk with NoDrop
//! scaling, placement and gold ([`walk`], §5, §7–§8) and drop quality
//! ([`quality`], §6).
//!
//! Item creation, the free-spot search and unit/stat/quest fields are
//! other specs' work; they reach this module through the [`DropSink`]
//! seam and staged inputs ([`GameFacts`], [`Dropper`], [`Recipient`],
//! [`MonsterRank`], the `quest_open` callback).

pub mod drop;
pub mod quality;
pub mod runtime;
pub mod softfloat;
pub mod walk;

#[cfg(test)]
mod tests;

pub use drop::{
    area_level, chest_drop, chest_tier, monster_drop, monster_drop_gate, monster_tc, upgrade_level,
    MonsterDrop, MonsterRank, CHEST_ACTS,
};
pub use quality::{ratio_row, roll_quality, LADDER};
pub use runtime::{item_list, ItemData, TcEntry, TcSources, TreasureClass, TreasureClasses};
pub use walk::{
    gold_base, nodrop, player_factor, select_entry, walk, DropRequest, DropSink, Dropper,
    DropperKind, GameFacts, Recipient, WalkArgs,
};

use d2_data::fixup::maps::EquivMatrix;
use d2_data::tables::{Itemratio, Itemtypes};

/// The loaded data a drop reads.
#[derive(Clone, Copy)]
pub struct TreasureData<'a> {
    pub tcs: &'a TreasureClasses,
    /// [`item_list`] order.
    pub items: &'a [ItemData],
    pub itemtypes: &'a [Itemtypes],
    /// itemtypes equivalence (`runtime-maps.md` §2).
    pub equiv: &'a EquivMatrix,
    pub itemratio: &'a [Itemratio],
}

/// Errors: the original's fatal errors and inputs d2rs does not model.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TreasureError {
    #[error("more than 65,534 treasure classes ({0})")]
    TooManyTcs(usize),
    #[error("name byte {0:#04x} ≥ 0x80 (E11)")]
    NonAsciiName(u8),
    #[error("walk without a treasure class (fatal 0xF3A)")]
    NoTc,
    #[error("walk with an output list and max 0 (fatal 0xF44)")]
    ZeroMax,
    #[error("walk deeper than 64 slots (fatal 0xFEA)")]
    SlotOverflow,
    #[error("no itemratio row matches (fatal)")]
    NoRatioRow,
    #[error("itemratio divisor 0")]
    ZeroDivisor,
    #[error("NoDrop outside the modelled binary64/i32 range (OQ5)")]
    NoDropRange,
    #[error("bonewall without the no-drop flag (fatal)")]
    BonewallDrop,
    #[error("difficulty {0} out of range")]
    Difficulty(u8),
    #[error("act {0} out of range")]
    Act(u8),
}
