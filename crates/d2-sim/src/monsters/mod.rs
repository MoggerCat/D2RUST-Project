// Spec: specs/monsters/ai.md
// Spec: specs/sim/intents-events.md §7.4, §7.7 (`mode_message`)
//! Monsters. Phase 3 so far: the AI think ([`ai`]).

pub mod ai;
pub mod init;
pub mod mode_message;
#[cfg(test)]
mod mutant_tests;
pub mod population;
