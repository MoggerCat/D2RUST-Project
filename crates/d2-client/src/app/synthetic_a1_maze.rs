// Spec: specs/drlg/maze.md §4, §9; specs/drlg/preset.md §4; specs/drlg/levels.md §5; specs/drlg/rooms.md §3.3
//! Act I's tree dungeons as real maze builds (task `q-dungeon-builds`,
//! `docs/handoff/q-dungeon-builds.md`): Cave Level 2, the Underground
//! Passage 1–2, the Holes 1–2 and the Pits 1–2 are built by the maze
//! generator over synthetic `lvlmaze` rows ([`super::synthetic_maze`]),
//! no longer flat 8 × 8-tile rooms. Their warp tiles stay those of the
//! tree ([`super::synthetic_chains`]), placed in the level's first maze
//! room. PROVISIONAL (REC-261): the original takes its exits from the
//! DS1 warp units of the cells; here the first room carries them
//! (`// d2rs-own, unverified`).

/// The tree levels built by the maze generator: Cave 2, Underground
/// Passage 1 / 2, Hole 1 / 2, Pit 1 / 2 (`levels.txt` ids).
pub const LEVELS: [u32; 7] = [13, 10, 14, 11, 15, 12, 16];

/// Whether `id` is one of [`LEVELS`].
pub fn is_level(id: u32) -> bool {
    LEVELS.contains(&id)
}

/// The `lvlmaze` `Rooms` of a level (d2rs-own): the caves and passages
/// are bigger than the holes and pits.
pub fn base_rooms(id: u32) -> u32 {
    match id {
        13 | 10 | 14 => 6,
        _ => 4,
    }
}

/// Fill the `levels` view of the maze levels: maze DRLG type, cave level
/// type (3), a size and an offset clear of the other maze levels.
pub fn add_levels(drlg: &mut d2_sim::drlg::DrlgData) {
    for (n, id) in LEVELS.into_iter().enumerate() {
        let c = &mut drlg.levels[id as usize];
        c.drlg_type = 1;
        c.level_type = 3;
        c.size = [(200, 200); 3];
        c.offset = (4000 + 300 * n as i32, 1000);
    }
}
