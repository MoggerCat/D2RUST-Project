// Spec: specs/drlg/rooms.md §3.3; specs/sim/path-placement.md §12.1, §12.2; specs/drlg/levels.md §5
//! The rest of the synthetic game's level warps (task `q-levels-warps-all`,
//! `docs/handoff/q-levels-warps-all.md`): chains of flat one-room levels
//! joined by warp pairs, for the Act I fields past Cold Plains (Stony Field stays unbuilt: the waypoint test needs it so), the whole
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

/// The chains, each a level order whose first level exists already. The
/// level ids are `levels.txt` ids (Underground Passage 1 = 10, Dark Wood
/// = 5, Tamoe Highland = 7; Act III 76..=83 then the Durance 100..=102; Act V 110..=118 then
/// 120 and the Worldstone Keep levels 128..=130).
pub const CHAINS: [&[u32]; 3] = [
    &[COLD_PLAINS, 5, 7, 10],
    &[75, 76, 77, 78, 79, 80, 81, 82, 83, 100, 101, 102],
    &[
        ACT5_TOWN, 110, 111, 112, 113, 114, 115, 116, 117, 118, 120, 128, 129, 130,
    ],
];

/// The first synthetic `lvlwarp` `Id` of the chains: after Act IV's.
pub fn first_warp() -> u32 {
    super::synthetic_act4::last_warp() + 1
}

/// One pair of a chain: the tile in `from` (class `on`) leads to `to`;
/// the tile in `to` (class `back`) leads back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: u32,
    pub to: u32,
    pub on: u32,
    pub back: u32,
}

/// Every pair in chain order; ids ascend two at a time.
pub fn edges() -> Vec<Edge> {
    let mut out = Vec::new();
    let mut id = first_warp();
    for chain in CHAINS {
        for w in chain.windows(2) {
            out.push(Edge {
                from: w[0],
                to: w[1],
                on: id,
                back: id + 1,
            });
            id += 2;
        }
    }
    out
}

/// The last synthetic warp id of the chains.
pub fn last_warp() -> u32 {
    first_warp() + 2 * edges().len() as u32 - 1
}

/// Every level that appears in a chain.
pub fn levels() -> impl Iterator<Item = u32> {
    CHAINS.into_iter().flatten().copied()
}

/// The tile classes of `id`: (way back, way on), `None` when no chain has
/// it. A level in two chains (none now) would take the first.
pub fn links(id: u32) -> Option<(Option<u32>, Option<u32>)> {
    let e = edges();
    let back = e.iter().find(|x| x.to == id).map(|x| x.back);
    let on = e.iter().find(|x| x.from == id).map(|x| x.on);
    (back.is_some() || on.is_some()).then_some((back, on))
}

/// The `(vis level, warp class)` pairs of `id` by slot: slot 0 the way
/// back, slot 1 the way on (either may be absent).
pub fn slots(id: u32) -> [Option<(u32, u32)>; 2] {
    let e = edges();
    [
        e.iter().find(|x| x.to == id).map(|x| (x.from, x.back)),
        e.iter().find(|x| x.from == id).map(|x| (x.to, x.on)),
    ]
}

/// Sub-tiles of the way-back and way-on tiles in a level's room.
pub const BACK_XY: i32 = 20;
pub const ON_XY: i32 = 30;

/// The room rectangle (tile x, y) of a chain level that has no room yet:
/// eight-tile cells along a row per chain, clear of the other levels of
/// its act (d2rs-own).
pub fn room_origin(id: u32) -> Option<(i32, i32)> {
    for (c, chain) in CHAINS.iter().enumerate() {
        if let Some(i) = chain.iter().position(|&l| l == id) {
            return Some((8 * i as i32, 40 + 8 * c as i32));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pair_has_two_ids_and_links_are_consistent() {
        let e = edges();
        assert_eq!(e.len(), 3 + 11 + 13);
        assert_eq!(last_warp(), first_warp() + 2 * e.len() as u32 - 1);
        assert_eq!(links(COLD_PLAINS), Some((None, Some(first_warp()))));
        assert_eq!(links(10), Some((Some(first_warp() + 5), None)));
        assert_eq!(links(9), None);
        let s = slots(76);
        assert_eq!(s[0], Some((75, first_warp() + 7)));
        assert_eq!(s[1], Some((77, first_warp() + 8)));
    }
}
