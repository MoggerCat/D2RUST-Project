// Spec: specs/world/quests-act4.md §3 (A4Q1); specs/drlg/levels.md §5; specs/world/npc.md §3
//! The synthetic game's Act IV line (task `q-a4`, `docs/handoff/q-a4.md`):
//! the Pandemonium Fortress town with Tyrael, Jamella, Halbu and Cain, and
//! one flat room each for the Outer Steppes, Plains of Despair, City of
//! the Damned, River of Flame and Chaos Sanctuary, joined by warp pairs in
//! level order. Izual stands in the Plains (host-placed, chain 22).
//!
//! PROVISIONAL (M22; REC-143): the original builds the outdoor levels
//! from the act placer and the Fortress from a town preset; every place
//! here is made up. `// d2rs-own, unverified`.

use d2_sim::world::npc::class;

/// Act index of Act IV (acts are 0-based).
pub const ACT: u8 = 3;
/// The Pandemonium Fortress, the Outer Steppes, the Plains of Despair,
/// the City of the Damned, the River of Flame, the Chaos Sanctuary.
pub const FORTRESS: u32 = 103;
pub const OUTER_STEPPES: u32 = 104;
pub const PLAINS_OF_DESPAIR: u32 = 105;
pub const CITY_OF_THE_DAMNED: u32 = 106;
pub const RIVER_OF_FLAME: u32 = 107;
pub const CHAOS_SANCTUARY: u32 = 108;
/// The line in walking order.
pub const LEVELS: [u32; 6] = [103, 104, 105, 106, 107, 108];

/// Cain (identifier, `CAIN4`), Tyrael, Jamella and Halbu.
pub const NPCS: [u16; 4] = [class::TYRAEL2, class::JAMELLA, class::HALBU, class::CAIN4];
/// Sub-tiles from the Fortress room's origin: the first NPC and the step.
pub const NPC_X0: i32 = 3;
pub const NPC_Y: i32 = 12;
pub const NPC_STEP: i32 = 4;
/// The Fortress waypoint.
pub const WAYPOINT_XY: (i32, i32) = (20, 30);

/// Izual (`monstats` 256) and the Fallen Angel's chain (A4Q1).
pub const IZUAL: u32 = 256;
pub const IZUAL_CHAIN: u32 = 22;
pub const IZUAL_XY: i32 = 8;
/// `monstats` rows the synthetic table needs for this act (Hephasto's
/// class 409 is the highest Act IV class listed in the spec).
pub const MONSTATS_ROWS: usize = 410;

/// The `lvlwarp` `Id`s (= tile classes), after the Burial Grounds' 29 and
/// 30: the way on from `LEVELS[i]` and the way back from `LEVELS[i + 1]`.
pub const fn on(i: usize) -> u32 {
    31 + 2 * i as u32
}
pub const fn back(i: usize) -> u32 {
    32 + 2 * i as u32
}
/// The last synthetic warp id.
pub const LAST_WARP: u32 = back(LEVELS.len() - 2);
/// Sub-tile of the way-back tile and of the way-on tile in a room.
pub const BACK_XY: i32 = 20;
pub const ON_XY: i32 = 30;

/// The index of `id` in [`LEVELS`].
pub fn index(id: u32) -> Option<usize> {
    LEVELS.iter().position(|&l| l == id)
}

/// The tile classes a level carries: (way back, way on).
pub fn links(id: u32) -> Option<(Option<u32>, Option<u32>)> {
    let i = index(id)?;
    Some((
        (i > 0).then(|| back(i - 1)),
        (i + 1 < LEVELS.len()).then(|| on(i)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_links_are_a_chain() {
        assert_eq!(links(103), Some((None, Some(31))));
        assert_eq!(links(105), Some((Some(back(1)), Some(on(2)))));
        assert_eq!(links(108), Some((Some(40), None)));
        assert_eq!(links(7), None);
        assert_eq!(LAST_WARP, 40);
    }
}
