// Spec: specs/drlg/levels.md §3, §5; specs/drlg/maze.md §4, §5.2, §5.4, §5.5, §6.2, §6.3; specs/drlg/rooms.md §3.3
//! The synthetic game's Act 2 dungeons (task `q-a2-dungeons`,
//! `docs/handoff/q-a2-dungeons.md`): the sewers, the Halls of the Dead,
//! the Stony Tomb, the Claw Viper Temple, the Maggot Lair, the Arcane
//! Sanctuary and Tal Rasha's seven tombs, all built by the real maze
//! generator ([`super::synthetic_maze`]) behind warp pairs.
//!
//! The preview world has no Act 2 fields, so where each line starts is
//! d2rs-own, unverified: the sewers, the Halls, the Temple, the Lair,
//! the Stony Tomb and the Sanctuary hang off Lut Gholein, and the seven
//! tombs off a flat stand-in for the Canyon of the Magi (the original
//! reaches them through Dry Hills, Far Oasis, Valley of Snakes, Rocky
//! Waste, the Palace Cellar and the Canyon).

use d2_sim::drlg::{room_flags, DrlgData};

use super::single_player::ACT2_TOWN;

/// Canyon of the Magi (act 1 index), flat: the tombs' parent here.
pub const CANYON: u32 = 46;
pub const SEWERS_1: u32 = 47;
pub const STONY_TOMB_1: u32 = 55;
pub const HALLS_OF_THE_DEAD_1: u32 = 56;
pub const CLAW_VIPER_TEMPLE_1: u32 = 58;
pub const MAGGOT_LAIR_1: u32 = 62;
pub const FIRST_TOMB: u32 = 66;
pub const ARCANE_SANCTUARY: u32 = 74;
/// Duriel's Lair: entered from the tomb holding the orifice, behind the
/// quest gate (q-a2-duriel, d2rs-own, unverified, REC-167).
pub const DURIELS_LAIR: u32 = 73;
/// The orifice's and the lair entrance's `objects` rows (`objects.txt`).
pub const ORIFICE_CLASS: u32 = 152;
pub const LAIR_ENTRANCE_CLASS: u32 = 100;
/// The Lair's population (q-a2-tyrael-door, d2rs-own, unverified,
/// REC-241): Duriel (`monstats` 211), Tyrael (251) and Tyrael's door
/// (`objects` 153), at sub-tiles of the Lair's first room. Duriel's real
/// place is the Lair's own presets.
pub const DURIEL_CLASS: u32 = 211;
pub const TYRAEL_CLASS: u32 = 251;
pub const TYRAEL_DOOR_CLASS: u32 = 153;
pub const DURIEL_XY: (i32, i32) = (28, 14);
pub const TYRAEL_XY: (i32, i32) = (14, 26);
pub const TYRAEL_DOOR_XY: (i32, i32) = (14, 20);

/// A dungeon line: the level it hangs off and the levels in order.
const LINES: [(u32, &[u32]); 14] = [
    (ACT2_TOWN, &[47, 48, 49]),
    (ACT2_TOWN, &[56, 57, 60]),
    (ACT2_TOWN, &[58, 61]),
    (ACT2_TOWN, &[62, 63, 64]),
    (ACT2_TOWN, &[55, 59]),
    (ACT2_TOWN, &[ARCANE_SANCTUARY]),
    (CANYON, &[66]),
    (CANYON, &[67]),
    (CANYON, &[68]),
    (CANYON, &[69]),
    (CANYON, &[70]),
    (CANYON, &[71]),
    (CANYON, &[72]),
    // The Canyon itself, behind Lut Gholein (last: its pair ids follow
    // the dungeons').
    (ACT2_TOWN, &[CANYON]),
];

/// First synthetic Act 2 `lvlwarp` `Id` (after the Tower line's last, 28,
/// and the Burial Grounds pair, 29–30).
pub const FIRST_WARP: u32 = 31;

/// One warp pair: the tile in `from` (class `to_class`) leads to `to`;
/// the tile in `to` (class `back_class`) leads back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: u32,
    pub to: u32,
    pub to_class: u32,
    pub back_class: u32,
}

/// Every pair, ids in line order, each line's entrance first.
pub fn edges() -> Vec<Edge> {
    let mut out = Vec::new();
    let mut id = FIRST_WARP;
    for (parent, levels) in LINES {
        let mut from = parent;
        for &to in levels {
            out.push(Edge {
                from,
                to,
                to_class: id,
                back_class: id + 1,
            });
            id += 2;
            from = to;
        }
    }
    // Every tomb has a way on to the Lair (the DRLG picks the tomb that
    // holds the orifice at run time; the quest gate lets only that one
    // through). The Lair carries one way back per tomb.
    for from in FIRST_TOMB..FIRST_TOMB + 7 {
        out.push(Edge {
            from,
            to: DURIELS_LAIR,
            to_class: id,
            back_class: id + 1,
        });
        id += 2;
    }
    out
}

/// The last synthetic Act 2 warp id.
pub fn last_warp() -> u32 {
    FIRST_WARP + 2 * edges().len() as u32 - 1
}

/// Every level of the dungeons (maze levels).
pub fn dungeon_levels() -> impl Iterator<Item = u32> {
    LINES
        .into_iter()
        .flat_map(|(_, l)| l.iter().copied())
        .filter(|&l| l != CANYON)
        .chain([DURIELS_LAIR])
}

/// The Lair's ways back, one per tomb in tomb order, with the sub-tile
/// of each in the Lair's first room (the first is the usual back tile's).
pub fn lair_backs() -> Vec<(u32, (i32, i32))> {
    const AT: [(i32, i32); 7] = [
        (20, 20),
        (8, 8),
        (32, 8),
        (8, 32),
        (32, 32),
        (20, 8),
        (20, 32),
    ];
    edges()
        .iter()
        .filter(|e| e.to == DURIELS_LAIR)
        .enumerate()
        .map(|(k, e)| (e.back_class, AT[k]))
        .collect()
}

/// Whether `id` is a Tal Rasha tomb.
pub fn is_tomb(id: u32) -> bool {
    (FIRST_TOMB..FIRST_TOMB + 7).contains(&id)
}

/// The lvlmaze `Rooms` of a dungeon level: 6 for the tombs, 61 for the
/// Sanctuary's spiral (`maze.md` §5.5), 1 otherwise.
pub fn base_rooms(id: u32) -> u32 {
    match id {
        ARCANE_SANCTUARY => 61,
        _ if is_tomb(id) => 6,
        _ => 1,
    }
}

/// The tile classes a dungeon maze level carries: the way back and, unless
/// it is its line's last level, the way on.
pub fn maze_links(id: u32) -> Option<(u32, Option<u32>)> {
    let e = edges();
    let back = e.iter().find(|x| x.to == id && id != CANYON)?.back_class;
    let on = e.iter().find(|x| x.from == id).map(|x| x.to_class);
    Some((back, on))
}

/// The warp tiles of a flat level: its way back (if it has a parent)
/// then the edges that leave it, in slot order.
pub fn flat_tiles(id: u32) -> Vec<Edge> {
    let e = edges();
    let mut v: Vec<Edge> = e.iter().filter(|x| x.to == id).copied().collect();
    v.extend(e.iter().filter(|x| x.from == id));
    v
}

/// The `(vis level, warp class)` of slot `k` of a flat level, and the
/// sub-tile of its tile in the level's 8 × 8-tile room (d2rs-own).
pub fn flat_slot(id: u32, k: usize) -> Option<(u32, u32, (i32, i32))> {
    let t = *flat_tiles(id).get(k)?;
    let (other, class) = if t.to == id {
        (t.from, t.back_class)
    } else {
        (t.to, t.to_class)
    };
    let xy = (10 + 18 * (k as i32 % 2), 4 + 8 * (k as i32 / 2));
    Some((other, class, xy))
}

/// The room flags of a flat level's warp slots.
pub fn flat_flags(id: u32) -> u32 {
    (0..flat_tiles(id).len()).fold(0, |f, k| f | (room_flags::WARP_0 << k))
}

/// The levels' `levels.txt` view: the Canyon a flat preset level, the
/// dungeons maze levels (type 17 for the tombs, 13 for the sewers, 19 for
/// the Sanctuary, 3 elsewhere: `maze.md` §4), and the vis / warp slots of
/// every pair. Returns nothing; the warp rows are [`last_warp`]-bound.
pub fn add_levels(drlg: &mut DrlgData) {
    let e = edges();
    // The sewer, tomb and Sanctuary level types (13, 17, 19) have no tile
    // library here, like the cave's.
    if drlg.lvltypes.len() < 20 {
        drlg.lvltypes.resize(20, vec![Vec::new(); 32]);
    }
    {
        let c = &mut drlg.levels[CANYON as usize];
        c.drlg_type = 2;
        c.level_type = 1;
    }
    for (n, id) in dungeon_levels().enumerate() {
        let c = &mut drlg.levels[id as usize];
        c.drlg_type = 1;
        c.level_type = match id {
            47..=49 => 13,
            ARCANE_SANCTUARY => 19,
            _ if is_tomb(id) || id == DURIELS_LAIR || (55..=61).contains(&id) => 17,
            _ => 3,
        };
        c.size = [(200, 200); 3];
        c.offset = (1000 + 300 * n as i32, 2000);
        if id == ARCANE_SANCTUARY {
            // The spiral's 61 cells (`maze.md` §5.5) need room.
            c.size = [(1200, 1200); 3];
            c.offset = (1000, 4000);
        }
    }
    // Flat levels: slot k = tile k of `flat_tiles`; maze levels: slot 0
    // the way back, slot 1 the way on.
    for id in [ACT2_TOWN, CANYON] {
        for k in 0..8 {
            if let Some((other, class, _)) = flat_slot(id, k) {
                let l = &mut drlg.levels[id as usize];
                l.vis[k] = other;
                l.warp[k] = class as i32;
            }
        }
    }
    for (k, x) in e.iter().filter(|x| x.to == DURIELS_LAIR).enumerate() {
        let l = &mut drlg.levels[DURIELS_LAIR as usize];
        l.vis[k] = x.from;
        l.warp[k] = x.back_class as i32;
    }
    for x in &e {
        if x.to != DURIELS_LAIR && maze_links(x.to).is_some() {
            let l = &mut drlg.levels[x.to as usize];
            l.vis[0] = x.from;
            l.warp[0] = x.back_class as i32;
        }
        if maze_links(x.from).is_some() {
            let l = &mut drlg.levels[x.from as usize];
            l.vis[1] = x.to;
            l.warp[1] = x.to_class as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_are_chains_with_unique_warp_ids() {
        let e = edges();
        let mut ids: Vec<u32> = e.iter().flat_map(|x| [x.to_class, x.back_class]).collect();
        assert_eq!(ids.len() as u32, last_warp() - FIRST_WARP + 1);
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 2 * e.len());
        assert_eq!(maze_links(47).map(|l| l.1.is_some()), Some(true));
        assert_eq!(maze_links(49).map(|l| l.1), Some(None));
        assert_eq!(maze_links(CANYON), None);
        // Lut Gholein: 7 children; the Canyon: its way back + 7 tombs.
        assert_eq!(flat_tiles(ACT2_TOWN).len(), 7);
        assert_eq!(flat_tiles(CANYON).len(), 8);
        assert_eq!(dungeon_levels().filter(|&l| is_tomb(l)).count(), 7);
        assert_eq!(lair_backs().len(), 7);
    }
}
