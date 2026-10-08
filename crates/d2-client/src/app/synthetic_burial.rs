// Spec: specs/world/quests-act1.md §10.5; specs/monsters/init.md §14.3; specs/sim/path-placement.md §12.1
//! The synthetic game's Sisters' Burial Grounds (task `q-a1-bloodraven`,
//! `docs/handoff/q-a1-bloodraven.md`): level 17, one flat room, reached by
//! a warp pair from the Blood Moor, with Blood Raven placed by the host.
//!
//! PROVISIONAL (M22; REC-136): the original reaches it from Cold Plains
//! and builds an outdoor level; the places of the tiles and of Blood
//! Raven are made up. `// d2rs-own, unverified`.

/// The Sisters' Burial Grounds (act 0, level 17); the quest's trigger
/// level (`quests-act1.md` §10.5 event 3).
pub const BURIAL_GROUNDS: u32 = 17;
/// monstats class 267, `bloodraven`.
pub const BLOOD_RAVEN: u32 = 267;
/// The quest chain her boss mods link (A1Q2, `init.md` §14.3).
pub const CHAIN: u32 = 2;
/// The `lvlwarp` `Id`s (and tile classes) of the Blood Moor's entrance
/// and of the way back (after the Tower line's, `synthetic_tower`).
pub const BLOOD_MOOR_TO_BURIAL: u32 = 29;
pub const BURIAL_TO_BLOOD_MOOR: u32 = 30;
/// Sub-tile of the entrance in the Blood Moor room, of the way back and
/// of Blood Raven in the Burial Grounds room.
pub const MOOR_BURIAL_XY: i32 = 10;
pub const BACK_XY: i32 = 20;
pub const RAVEN_XY: i32 = 8;
