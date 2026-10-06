// Spec: specs/sim/unit-order.md
//! Server units. Phase 3 so far: identity and list bookkeeping
//! ([`lists`]); unit data, stats and behaviour follow with their specs.

pub mod lists;

pub use lists::{
    ActEntry, ClientEntry, ClientId, GuidCounters, ListError, RoomEntry, RoomId, UnitEntry, UnitId,
    UnitLists, UnitType,
};
