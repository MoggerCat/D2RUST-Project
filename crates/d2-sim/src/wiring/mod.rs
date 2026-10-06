// Spec: specs/sim/tick.md §3, §5.6 (wiring of the module seams)
//! Adapters that connect each module's seams (narrow traits) to their
//! real providers in `d2-sim`. Nothing here decides behaviour: rules stay
//! in the modules; an adapter maps a seam call to a provider call.

pub mod action;
