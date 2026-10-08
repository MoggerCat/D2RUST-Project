// Spec: specs/world/quests-act5-2.md §7 (A5Q5 Rite of Passage), §7.8 (objects), §7.5; specs/world/quests.md §8.2
//! The synthetic game's Act V quest objects (task `q-act3-act5-gaps`): the
//! three Ancient statues, the Ancients' altar, the door to the Worldstone
//! Keep, the invisible Ancient and the Arreat Summit door, each with its
//! (operate, init) function indices, and the summit level whose exits wait
//! for the Ancients ([`crate::app::single_player`]'s `warp_quest_gate`).
//!
//! PROVISIONAL (M22; REC-246): the objects are put in a level's room by the
//! caller (the chain levels have no objects of their own) and the summit's
//! exits to 118 and 128 are the chains' neighbours; the original places
//! them from the level's presets. `// d2rs-own, unverified`.

/// The Arreat Summit (`levels.txt` 120).
pub const SUMMIT: u32 = d2_sim::world::quests::act5::q5::SUMMIT;
/// Ancient Statue 3 / 1 / 2 (objects 474 / 475 / 476).
pub const STATUES: [u32; 3] = [474, 475, 476];
/// The Ancients' altar, the door to the Worldstone Keep, the invisible
/// Ancient and the summit door.
pub const ALTAR: u32 = 546;
pub const KEEP_DOOR: u32 = 547;
pub const INVISIBLE_ANCIENT: u32 = 561;
pub const SUMMIT_DOOR: u32 = 564;

/// (class, operate fn, init fn) per `quests-act5.md` §1.4.
pub const OBJECT_ROWS: [(u32, u8, u8); 8] = [
    (474, 62, 63),
    (475, 63, 64),
    (476, 64, 65),
    (ALTAR, 65, 72),
    (KEEP_DOOR, 66, 73),
    (INVISIBLE_ANCIENT, 69, 0),
    (SUMMIT_DOOR, 71, 76),
    // Table padding so the highest class exists.
    (565, 0, 0),
];

/// The summit's objects (class, sub-tiles from the room origin), clear of
/// the warp tiles at (20, 20) and (30, 30).
pub const PRESET_OBJECTS: [(u32, (i32, i32)); 4] = [
    (474, (8, 8)),
    (475, (8, 16)),
    (476, (8, 24)),
    (ALTAR, (16, 8)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_use_the_quest_functions_of_the_spec() {
        assert_eq!(SUMMIT, 120);
        assert!(OBJECT_ROWS.iter().all(|r| r.0 >= 474));
    }
}
