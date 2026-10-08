// Spec: specs/drlg/levels.md §5; specs/drlg/rooms.md §3.3; specs/world/quests-act1.md §10.7
//! The synthetic game's Forgotten Tower line (task `q-a1-tower`,
//! `docs/handoff/q-a1-tower.md`): the Black Marsh (flat level) with the
//! Moldy Tome object, the Forgotten Tower and Tower Cellars 1–5 (maze
//! levels, [`super::synthetic_maze`]) joined by warp pairs. Where the
//! Black Marsh sits and how it is reached is d2rs-own, unverified: the
//! original reaches it through the Dark Wood (the preview world has
//! neither), here a warp pair in the Blood Moor.

use super::synthetic_maze;

/// The Black Marsh (act 0, level 6), flat.
pub const BLACK_MARSH: u32 = 6;
/// The Forgotten Tower and Tower Cellar 1–5 (levels 20–25), maze levels.
pub const FORGOTTEN_TOWER: u32 = 20;
pub const TOWER_CELLAR_5: u32 = 25;
/// Tower, Cellar 1, …, Cellar 5.
pub const TOWER_LEVELS: [u32; 6] = [20, 21, 22, 23, 24, 25];

/// The synthetic `lvlwarp` `Id` (= tile class) pairs: Blood Moor ↔ Black
/// Marsh, Black Marsh ↔ Tower (rows after the Den's four, 11–14).
pub const BLOOD_MOOR_TO_MARSH: u32 = 15;
pub const MARSH_TO_BLOOD_MOOR: u32 = 16;
pub const MARSH_TO_TOWER: u32 = 17;
pub const TOWER_TO_MARSH: u32 = 18;
/// Last synthetic warp id.
pub const LAST_WARP: u32 = 28;

/// The stairs down from `TOWER_LEVELS[i]` to `TOWER_LEVELS[i + 1]`.
pub const fn down(i: usize) -> u32 {
    19 + 2 * i as u32
}
/// The way back up from `TOWER_LEVELS[i + 1]` to `TOWER_LEVELS[i]`.
pub const fn up(i: usize) -> u32 {
    20 + 2 * i as u32
}

/// The Moldy Tome's object class: the next synthetic `objects` row after
/// the portal (59), operate function 6 (TowerTome), init function 4.
pub const TOME_CLASS: u32 = 60;
pub const TOME_OPERATE: u8 = 6;
pub const TOME_INIT: u8 = 4;
/// The Countess's `monstats` class (`quests-act1.md` §10.7) and the
/// Tower's quest chain (A1Q5).
pub const COUNTESS_CLASS: u32 = 45;
pub const TOWER_CHAIN: u32 = 5;
/// Sub-tile of the tome in the Black Marsh's room.
pub const TOME_XY: (i32, i32) = (23, 18);
/// Sub-tile of the Black Marsh's tile to the Blood Moor / to the Tower.
pub const MARSH_BACK_XY: i32 = 20;
pub const MARSH_TOWER_XY: i32 = 30;
/// Sub-tile of the Blood Moor's tile to the Black Marsh.
pub const MOOR_MARSH_XY: i32 = 30;

/// Whether `id` is one of the Tower levels.
pub fn is_tower_level(id: u32) -> bool {
    TOWER_LEVELS.contains(&id)
}

/// The tile classes a maze level carries: the way back and, unless it
/// is the line's last level, the way on (a maze level's first room).
pub fn maze_links(id: u32) -> Option<(u32, Option<u32>)> {
    if id == synthetic_maze::CAVE_LEVEL_1 {
        // The way on to Cave Level 2 comes from the Act I tree.
        let on = super::synthetic_chains::slots(id)
            .into_iter()
            .find(|s| s.0 == 1)
            .map(|s| s.2);
        return Some((synthetic_maze::CAVE_TO_DEN, on));
    }
    let Some(i) = TOWER_LEVELS.iter().position(|&l| l == id) else {
        return super::synthetic_act2::maze_links(id);
    };
    let back = if i == 0 { TOWER_TO_MARSH } else { up(i - 1) };
    let on = (i + 1 < TOWER_LEVELS.len()).then(|| down(i));
    Some((back, on))
}

/// The levels the maze generator builds: the cave and the Tower line.
pub fn maze_levels() -> impl Iterator<Item = u32> {
    std::iter::once(synthetic_maze::CAVE_LEVEL_1)
        .chain(TOWER_LEVELS)
        .chain(super::synthetic_act2::dungeon_levels())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tower_line_links_are_a_chain() {
        assert_eq!(maze_links(20), Some((TOWER_TO_MARSH, Some(down(0)))));
        assert_eq!(maze_links(21), Some((up(0), Some(down(1)))));
        assert_eq!(maze_links(25), Some((up(4), None)));
        assert_eq!(maze_links(7), None);
        assert_eq!(down(4), LAST_WARP - 1);
        assert_eq!(up(4), LAST_WARP);
    }
}
