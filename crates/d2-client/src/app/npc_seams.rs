// Spec: specs/world/npc.md (§2 distances, §3 axis check), specs/world/quests.md (§1.7, §3)
//! The unit snapshot the play host's NPC and quest seams answer from
//! (`NpcRest::distance`, `axis_check`, `QuestRest::unit_kind`, …): the
//! rest has no game to read, so [`super::single_player::sync_seams`]
//! copies the players, monsters and their positions into a [`Snap`] the
//! rest shares ([`SnapRef`]).
//!
//! d2rs-own, unverified (preview, decisions D1–D3): the unit distance
//! `0x00641530` is not specified as a formula; it is read as the
//! integer Euclidean distance in sub-tiles. REC-52 in `docs/HANDOFF.md`
//! §7 (the NPC rest answers).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use d2_sim::units::{UnitId, UnitType};

/// `NpcRest::axis_check` (`npc.md` §3): the player within 50 sub-tiles
/// of the NPC on both axes.
const AXIS_LIMIT: i32 = 50;

/// One unit as the last sync saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapUnit {
    pub ty: UnitType,
    pub pos: (i32, i32),
    pub act: u8,
    pub guid: u32,
}

/// The players and monsters of the game at the last sync.
#[derive(Debug, Default)]
pub struct Snap {
    pub units: BTreeMap<UnitId, SnapUnit>,
}

/// The snapshot shared by the sim's seams (writer) and the host rest
/// (reader).
pub type SnapRef = Arc<Mutex<Snap>>;

impl Snap {
    /// Integer Euclidean distance in sub-tiles; `i32::MAX` when either
    /// unit is unknown.
    pub fn distance(&self, a: UnitId, b: UnitId) -> i32 {
        let (Some(a), Some(b)) = (self.units.get(&a), self.units.get(&b)) else {
            return i32::MAX;
        };
        let dx = i64::from(a.pos.0 - b.pos.0);
        let dy = i64::from(a.pos.1 - b.pos.1);
        ((dx * dx + dy * dy) as f64).sqrt() as i32
    }

    /// 0 when both axes are within [`AXIS_LIMIT`], else 1.
    pub fn axis_check(&self, player: UnitId, npc: UnitId) -> u32 {
        let (Some(a), Some(b)) = (self.units.get(&player), self.units.get(&npc)) else {
            return 1;
        };
        let near =
            (a.pos.0 - b.pos.0).abs() <= AXIS_LIMIT && (a.pos.1 - b.pos.1).abs() <= AXIS_LIMIT;
        u32::from(!near)
    }

    pub fn of_type(&self, ty: UnitType) -> Vec<UnitId> {
        self.units
            .iter()
            .filter(|(_, u)| u.ty == ty)
            .map(|(&id, _)| id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> Snap {
        let mut s = Snap::default();
        let u = |ty, x, y| SnapUnit {
            ty,
            pos: (x, y),
            act: 0,
            guid: 0,
        };
        s.units.insert(UnitId(1), u(UnitType::Player, 0, 0));
        s.units.insert(UnitId(2), u(UnitType::Monster, 3, 4));
        s.units.insert(UnitId(3), u(UnitType::Monster, 51, 0));
        s
    }

    #[test]
    fn distance_is_euclidean_and_unknown_is_far() {
        let s = snap();
        assert_eq!(s.distance(UnitId(1), UnitId(2)), 5);
        assert_eq!(s.distance(UnitId(1), UnitId(9)), i32::MAX);
    }

    #[test]
    fn axis_check_is_fifty_on_both_axes() {
        let s = snap();
        assert_eq!(s.axis_check(UnitId(1), UnitId(2)), 0);
        assert_eq!(s.axis_check(UnitId(1), UnitId(3)), 1);
        assert_eq!(s.axis_check(UnitId(1), UnitId(9)), 1);
    }
}

/// The 34 list bytes of S→C 0x27 (`npc.md` §2 step 5) from the quest
/// text list (message, menu): count u8 @0, entry k: kind u8 @2+4k,
/// string id u16 @4+4k, the layout the client reads
/// (`client/msg-ui.md` §5 r2.1, §16 r9; `0x00661480` itself is not
/// specified, so this is its inverse: d2rs-own, unverified). At most 7
/// entries (the client build asserts at 8).
pub fn encode_text_list(list: &[(u16, u32)]) -> [u8; 34] {
    let mut out = [0u8; 34];
    let n = list.len().min(7);
    out[0] = n as u8;
    for (k, &(string, kind)) in list.iter().take(n).enumerate() {
        out[2 + 4 * k] = kind as u8;
        out[4 + 4 * k..6 + 4 * k].copy_from_slice(&string.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod encode_tests {
    use super::*;

    #[test]
    fn the_list_round_trips_through_the_client_layout() {
        let b = encode_text_list(&[(64, 0), (300, 2)]);
        assert_eq!(b[0], 2);
        assert_eq!((b[2], u16::from_le_bytes([b[4], b[5]])), (0, 64));
        assert_eq!((b[6], u16::from_le_bytes([b[8], b[9]])), (2, 300));
    }

    #[test]
    fn more_than_seven_entries_are_cut() {
        let l: Vec<(u16, u32)> = (0..10).map(|i| (i, 0)).collect();
        assert_eq!(encode_text_list(&l)[0], 7);
    }
}
