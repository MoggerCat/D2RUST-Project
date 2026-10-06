// Spec: specs/world/npc.md; specs/world/vendors.md; specs/skills/use.md; specs/combat/vitals.md (wiring of the interaction seams)
//! The interaction seams on their real providers:
//!
//! - [`npc_vendors`]: `world::npc` ↔ `world::vendors` in both directions
//!   ([`crate::world::npc::NpcVendors`] on the vendor functions,
//!   [`crate::world::vendors::NpcLink`] on [`NpcControl`] and the
//!   interaction lists), and the C→S 0x32 / 0x33 / 0x35 entry points that
//!   need both.
//! - [`npc_world`]: [`crate::world::npc::NpcWorld`] on the unit records,
//!   stat lists, timers, item data (through the economy wiring) and the
//!   quests ([`QuestControl`]); the rest stays a seam ([`NpcRest`]).
//! - [`vendor_world`]: [`crate::world::vendors::VendorWorld`] on the same
//!   providers, item creation through
//!   [`crate::wiring::economy::Economy::create_item`]; the rest is
//!   [`VendorRest`].
//! - [`skill_use`]: the skill use pipeline ([`crate::skills::use_`]) on
//!   the action wiring's units, stats, timers and missiles
//!   ([`UseView`]); missile creation through [`crate::missiles`]; the rest
//!   is [`UseRest`].
//! - [`vitals`]: [`crate::combat::vitals::VitalsUnits`] on unit records
//!   and stat lists ([`VitalsView`]); experience on a kill from the parts
//!   the specs write ([`vitals::kill_experience`]).
//!
//! Ownership: [`Desk`] borrows the economy's parts ([`Economy`]: game,
//! unit records, stat lists, hooks, game fields, item tables and item
//! store), the game's [`QuestControl`], the vendor tables and the
//! [`InteractionState`] (one [`VendorRecord`] per NPC record, the
//! interaction lists of the NPCs). The NPC control block
//! ([`NpcControl`]) stays with the caller: its handlers take the desk as
//! their world.
//!
//! Status: wired, unverified (every spec involved is a draft). Nothing
//! here decides game behaviour: rules stay in the modules; an adapter maps
//! a seam call to a provider call.

pub mod npc_vendors;
pub mod npc_world;
pub mod skill_use;
pub mod vendor_world;
pub mod vitals;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

pub use npc_vendors::VendorDesk;
pub use npc_world::NpcRest;
pub use skill_use::{UseRest, UseView};
pub use vendor_world::VendorRest;
pub use vitals::{VitalsRest, VitalsView};

use super::economy::{Economy, EconomyError, QuestRest};
use crate::units::UnitId;
use crate::world::npc::{InteractionList, NpcControl, NpcError};
use crate::world::quests::{PlayerQuests, QuestControl, QuestError};
use crate::world::vendors::price::PriceFatal;
use crate::world::vendors::{GlobalLists, VendorRecord, VendorTables};

/// What went wrong in an adapter (a provider's error, a fatal assertion
/// of 1.14d), in order.
#[derive(Debug, PartialEq, Eq)]
pub enum InteractionError {
    Economy(EconomyError),
    Npc(NpcError),
    Quest(QuestError),
    Price(PriceFatal),
    /// The NPC class has no NPC record (and so no vendor record).
    NoRecord(u16),
}

/// The interaction state of a game beside the NPC control block: the
/// vendor part of each NPC record (`vendors.md` §1, `npc.md` §1.1, in
/// the record order of [`NpcControl::records`]) and the interaction list
/// of each `interact` NPC (monster data +0x30, `npc.md` §2).
#[derive(Debug, Default)]
pub struct InteractionState {
    pub vendors: Vec<VendorRecord>,
    pub lists: BTreeMap<UnitId, InteractionList>,
    pub errors: Vec<InteractionError>,
}

impl InteractionState {
    /// The vendor records at game creation (`npc.md` §1.1 step 5,
    /// `vendors.md` §1 rules 3–5): one per NPC record, from its class,
    /// act and trader byte.
    pub fn new(ctl: &NpcControl, globals: &GlobalLists) -> Self {
        Self {
            vendors: ctl
                .records
                .iter()
                .map(|r| VendorRecord::new(r.class, r.act, r.trader != 0, globals))
                .collect(),
            lists: BTreeMap::new(),
            errors: Vec::new(),
        }
    }

    /// The interaction list of an `interact` NPC (monster init embeds it,
    /// `npc.md` §2).
    pub fn add_npc(&mut self, npc: UnitId) {
        self.lists.entry(npc).or_default();
    }

    /// Index of the vendor record of `class`.
    pub fn vendor_index(&self, class: u16) -> Option<usize> {
        self.vendors.iter().position(|r| r.class == class)
    }
}

/// The player data the quests need read-only (`QuestRest::quests` is the
/// mutable access; player data is not in d2-sim yet).
pub trait PlayerQuestsRef {
    fn quests_ref(&self, player: UnitId) -> Option<&PlayerQuests>;
}

/// The NPC and vendor world of one call: the economy's parts, the game's
/// quests, the vendor tables, the interaction state and the seams without
/// a provider (`R`).
pub struct Desk<'d, 'a, H, R> {
    pub econ: &'d mut Economy<'a, H>,
    pub quests: &'d mut QuestControl,
    pub vendor_tables: &'d VendorTables,
    pub state: &'d mut InteractionState,
    pub rest: &'d mut R,
    /// Host milliseconds (`GetTickCount`), an input of store generation
    /// and refresh (`vendors.md` edge case 10).
    pub now: u32,
}

impl<'a, H, R: QuestRest> Desk<'_, 'a, H, R> {
    /// The quests' world on the same economy and rest.
    pub(crate) fn quest_world(
        &mut self,
    ) -> (
        &mut QuestControl,
        super::economy::EconomyQuests<'_, 'a, H, R>,
    ) {
        (
            self.quests,
            super::economy::EconomyQuests {
                econ: self.econ,
                rest: self.rest,
            },
        )
    }
}

/// An item probe without stats, for the item functions that read only
/// record fields (`generation.md` §7.2).
pub(crate) struct NoStats;

impl crate::items::ItemStats for NoStats {
    fn has_stats(&self) -> bool {
        false
    }
    fn stat(&self, _: u16, _: u16) -> i32 {
        0
    }
    fn base(&self, _: u16, _: u16) -> i32 {
        0
    }
    fn set_base(&mut self, _: u16, _: u16, _: i32) {}
    fn has_list(&self, _: crate::items::ListKey) -> bool {
        false
    }
    fn list_set(&mut self, _: crate::items::ListKey, _: u16, _: u16, _: i32) {}
    fn list_add(&mut self, _: crate::items::ListKey, _: u16, _: u16, _: i32) {}
    fn list_get(&self, _: crate::items::ListKey, _: u16, _: u16) -> i32 {
        0
    }
}
