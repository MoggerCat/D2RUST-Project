// Spec: specs/drlg/rooms.md §3.3; specs/sim/path-placement.md §12.1, §12.2; specs/drlg/levels.md §5
//! The rest of the synthetic game's level warps (task `q-levels-warps-all`,
//! `docs/handoff/q-levels-warps-all.md`): chains of flat one-room levels
//! joined by warp pairs, for the Act I fields past Cold Plains (Stony Field stays unbuilt: the waypoint test needs it so; Act I's
//! dungeons are a tree, `q-a1-dungeons`), the whole
//! of Act III (Kurast Docks to the Durance of Hate) and Act V (Harrogath
//! to the Worldstone Keep). Each chain's first level exists already (a
//! town or Cold Plains); every level after it gets a way back (slot 0)
//! and, unless last, a way on (slot 1).
//!
//! PROVISIONAL (M22; REC-230): the real worlds join these levels through
//! outdoor placement, stairs and cave mouths with several exits per level;
//! the chain order, the level ids' neighbours and every tile place are
//! made up. `// d2rs-own, unverified`.

use super::single_player::{ACT5_TOWN, COLD_PLAINS};

/// Kurast Docks, the Act III town.
pub const KURAST_DOCKS: u32 = 75;

/// Act I's dungeon tree (task `q-a1-dungeons`, parents aligned with the
/// level links by `q-a1-vis-links`): (parent, parent's vis slot, child).
/// 1.14d's chain is Stony Field → Underground Passage 1 → Dark Wood →
/// Black Marsh → Tamoe Highland → Monastery Gate …; Tristram hangs off the
/// Stony Field, the Burial Grounds off Cold Plains, Cave 2 off Cave 1, the
/// Hole off the Black Marsh, the Pit off Tamoe. (Blood Moor → Cold Plains →
/// Stony Field are outdoor borders, not tiles.) The parent either exists
/// already (Cold Plains, Stony Field, Cave 1; Black Marsh's slot 1 is the
/// Tower's) or is an earlier child; a child's slot 0 is always its way back. Level ids are
/// `levels.txt` ids: Cold Plains 3, Dark Wood 5, Black Marsh 6, Tamoe
/// Highland 7, Underground Passage 1/2 = 10/14, Hole 1/2 = 11/15, Pit
/// 1/2 = 12/16, Cave 2 = 13, Crypt 18, Mausoleum 19, Monastery Gate 26,
/// Outer Cloister 27, Barracks 28, Jail 1–3 = 29–31, Inner Cloister 32,
/// Cathedral 33, Catacombs 1–4 = 34–37, Tristram 38.
const ACT1_TREE: &[(u32, usize, u32)] = &[
    (COLD_PLAINS, 1, 17),
    (9, 1, 13),
    (4, 1, 10),
    (4, 2, 38),
    (10, 1, 14),
    (10, 2, 5),
    (5, 1, 6),
    (6, 2, 7),
    (6, 3, 11),
    (11, 1, 15),
    (7, 1, 26),
    (7, 2, 12),
    (12, 1, 16),
    (17, 1, 18),
    (17, 2, 19),
    (26, 1, 27),
    (27, 1, 28),
    (28, 1, 29),
    (29, 1, 30),
    (30, 1, 31),
    (31, 1, 32),
    (32, 1, 33),
    (33, 1, 34),
    (34, 1, 35),
    (35, 1, 36),
    (36, 1, 37),
];

/// The two straight chains: Act III (Kurast Docks 75 … 83, then the
/// Durance 100..=102) and Act V (110..=118, 120, 128..=130).
const LINES: [&[u32]; 2] = [
    &[75, 76, 77, 78, 79, 80, 81, 82, 83, 100, 101, 102],
    &[
        ACT5_TOWN, 110, 111, 112, 113, 114, 115, 116, 117, 118, 120, 128, 129, 130,
    ],
];

/// The first synthetic `lvlwarp` `Id` of the chains: after Act IV's.
pub fn first_warp() -> u32 {
    super::synthetic_act4::last_warp() + 1
}

/// One pair: the tile in `from` (vis slot `slot`, class `on`) leads to
/// `to`; the tile in `to` (slot 0, class `back`) leads back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: u32,
    pub slot: usize,
    pub to: u32,
    pub on: u32,
    pub back: u32,
}

/// Every pair, Act I's tree first; ids ascend two at a time.
pub fn edges() -> Vec<Edge> {
    let mut pairs: Vec<(u32, usize, u32)> = ACT1_TREE.to_vec();
    for line in LINES {
        pairs.extend(line.windows(2).map(|w| (w[0], 1, w[1])));
    }
    pairs
        .into_iter()
        .enumerate()
        .map(|(i, (from, slot, to))| Edge {
            from,
            slot,
            to,
            on: first_warp() + 2 * i as u32,
            back: first_warp() + 2 * i as u32 + 1,
        })
        .collect()
}

/// The last synthetic warp id of the chains.
pub fn last_warp() -> u32 {
    first_warp() + 2 * edges().len() as u32 - 1
}

/// Every level of a pair, first appearance first (those with no room
/// get one).
pub fn levels() -> impl Iterator<Item = u32> {
    let mut seen = Vec::new();
    for e in edges() {
        for l in [e.from, e.to] {
            if !seen.contains(&l) {
                seen.push(l);
            }
        }
    }
    seen.into_iter()
}

/// The `(vis slot, destination level, tile class)` triples of `id`, way
/// back (slot 0) first, then the ways on in slot order.
pub fn slots(id: u32) -> Vec<(usize, u32, u32)> {
    let mut v: Vec<_> = edges()
        .iter()
        .flat_map(|e| {
            let back = (e.to == id).then_some((0, e.from, e.back));
            let on = (e.from == id).then_some((e.slot, e.to, e.on));
            back.into_iter().chain(on)
        })
        .collect();
    v.sort();
    v
}

/// The sub-tile (x, y) of the tile in vis slot `slot`: the way back and
/// the first way on keep their old places (on the diagonal); further
/// slots stand off it, so the walk to one does not cross another.
pub fn tile_xy(slot: usize) -> (i32, i32) {
    match slot {
        0 => (20, 20),
        1 => (30, 30),
        n => {
            let v = 10 + 4 * (n as i32 - 2).min(4);
            (v, v)
        }
    }
}

/// The room rectangle (tile x, y) of a chain level that has no room yet:
/// eight-tile cells, sixteen to a row, by order of appearance, clear of
/// the other levels of its act (d2rs-own).
pub fn room_origin(id: u32) -> Option<(i32, i32)> {
    let i = levels().position(|l| l == id)? as i32;
    Some((8 * (i % 16), 40 + 8 * (i / 16)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pair_has_two_ids_and_slots_are_consistent() {
        let e = edges();
        assert_eq!(e.len(), ACT1_TREE.len() + 11 + 13);
        assert_eq!(last_warp(), first_warp() + 2 * e.len() as u32 - 1);
        // Dark Wood: way back to Underground Passage 1 (slot 0), on to the
        // Black Marsh (slot 1).
        let dw = slots(5);
        assert_eq!(dw.iter().map(|s| s.0).collect::<Vec<_>>(), [0, 1]);
        assert_eq!((dw[0].1, dw[1].1), (10, 6));
        // 1.14d's Act I order: Stony Field → UP1 → Dark Wood → Black
        // Marsh → Tamoe → Monastery Gate.
        for (from, to) in [(4, 10), (10, 5), (5, 6), (6, 7), (7, 26)] {
            assert!(slots(from).iter().any(|s| s.0 != 0 && s.1 == to));
        }
        assert!(slots(2).is_empty(), "the Blood Moor is no tree level");
        // No (parent, slot) twice.
        for (i, a) in e.iter().enumerate() {
            assert!(e[i + 1..]
                .iter()
                .all(|b| (a.from, a.slot) != (b.from, b.slot)));
        }
        // No level has two parents.
        for id in edges().iter().map(|e| e.to) {
            let s = slots(id);
            assert_eq!(s.iter().filter(|x| x.0 == 0).count(), 1, "level {id}");
        }
    }
}
