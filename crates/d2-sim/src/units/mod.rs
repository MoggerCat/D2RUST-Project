// Spec: specs/sim/unit-order.md
//! Server units. Phase 3 so far: identity and list bookkeeping
//! ([`lists`]); unit data, stats and behaviour follow with their specs.

pub mod anim;
pub mod dispatch;
#[cfg(test)]
mod gap_tests;
pub mod hooks;
pub mod lifecycle;
pub mod lists;
pub mod modes;
pub mod record;
#[cfg(test)]
mod tests;

pub use lists::{
    ActEntry, ClientEntry, ClientId, GuidCounters, ListError, RoomEntry, RoomId, UnitEntry, UnitId,
    UnitLists, UnitType,
};
