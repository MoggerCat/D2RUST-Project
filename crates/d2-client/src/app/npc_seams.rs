// Spec: specs/world/npc.md (§2 distances, §3 axis check), specs/world/quests.md (§1.7, §3)
//! The unit snapshot the play host's NPC and quest seams answer from
//! (`NpcRest::distance`, `axis_check`, `QuestRest::unit_kind`, …): the
//! rest has no game to read, so [`super::single_player::sync_seams`]
//! copies the players, monsters and their positions into a [`Snap`] the
//! rest shares ([`SnapRef`]).
//!
//! d2rs-own, unverified (preview, decisions D1–D3): the unit distance
//! `0x00641530` is not specified as a formula; it is read as the
//! integer Euclidean distance in sub-tiles. REC-106 in `docs/HANDOFF.md`
//! §7 (the NPC rest answers).

use d2_sim::path::coords::Point;
use d2_sim::path::tables::PathTables;
use d2_sim::path::walk::geom::unit_distance;
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
    /// The level id of the unit's room (0 without a DRLG room).
    pub level: u32,
    /// The unit's class (monsters: the `monstats` row).
    pub class: u32,
}

/// The players and monsters of the game at the last sync.
#[derive(Debug, Default)]
pub struct Snap {
    pub units: BTreeMap<UnitId, SnapUnit>,
}

/// Unit size used by [`Snap::distance`] (d2rs-own, unverified).
const UNIT_SIZE: i32 = 2;

/// The snapshot shared by the sim's seams (writer) and the host rest
/// (reader).
pub type SnapRef = Arc<Mutex<Snap>>;

impl Snap {
    /// Unit distance `0x00641530` (`pathing.md` §9.5) in sub-tiles, the
    /// distance the walk's arrival check and `npc.md` §2 read;
    /// `i32::MAX` when either unit is unknown. d2rs-own, unverified:
    /// every unit has size 2 (the player's, and Akara's monstats2
    /// `SizeX`).
    pub fn distance(&self, a: UnitId, b: UnitId) -> i32 {
        let (Some(a), Some(b)) = (self.units.get(&a), self.units.get(&b)) else {
            return i32::MAX;
        };
        static TABLES: std::sync::OnceLock<Option<PathTables>> = std::sync::OnceLock::new();
        match TABLES.get_or_init(|| PathTables::spec().ok()) {
            Some(t) => unit_distance(
                t,
                Point::new(a.pos.0, a.pos.1),
                UNIT_SIZE,
                Point::new(b.pos.0, b.pos.1),
                UNIT_SIZE,
            ),
            None => i32::MAX,
        }
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
            level: 0,
            class: 0,
        };
        s.units.insert(UnitId(1), u(UnitType::Player, 0, 0));
        s.units.insert(UnitId(2), u(UnitType::Monster, 3, 4));
        s.units.insert(UnitId(3), u(UnitType::Monster, 51, 0));
        s
    }

    #[test]
    fn distance_is_the_unit_distance_and_unknown_is_far() {
        let s = snap();
        // Both axes under 8: the `dist8_unit` table (`pathing.md` §9.5).
        assert_eq!(s.distance(UnitId(1), UnitId(2)), 4);
        // Past 8 on an axis: 2·max + min of Δ minus the sizes.
        assert_eq!(s.distance(UnitId(1), UnitId(3)), 2 * 49);
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
/// text list (message, menu) in the order the quests added them: count
/// u8 @0, entry k: kind u8 @2+4k, string id u16 @4+4k, the layout the
/// client reads (`client/msg-ui.md` §5 r2.1, §16 r9). At most 7 entries
/// (the client build asserts at 8).
///
/// PROVISIONAL (REC-1634): the add `0x006612F0` is read as a prepend,
/// like the client's list build `0x00661510` (§16 r9), and the writer
/// `0x00661480` as a walk from the head, so the entries go out newest
/// first. Recorded: `items-vendor-akara-buy` frame 15, Akara's list is
/// sent as (64, 11) where the quests add 11, then 64.
pub fn encode_text_list(list: &[(u16, u32)]) -> [u8; 34] {
    let mut out = [0u8; 34];
    let n = list.len().min(7);
    out[0] = n as u8;
    for (k, &(string, kind)) in list.iter().rev().take(n).enumerate() {
        out[2 + 4 * k] = kind as u8;
        out[4 + 4 * k..6 + 4 * k].copy_from_slice(&string.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod encode_tests {
    use super::*;

    #[test]
    fn the_list_goes_out_newest_first() {
        // Akara (`items-vendor-akara-buy` f15): added 11, then 64; sent
        // (64, 11).
        let b = encode_text_list(&[(11, 0), (64, 0)]);
        assert_eq!(b[0], 2);
        assert_eq!((b[2], u16::from_le_bytes([b[4], b[5]])), (0, 64));
        assert_eq!((b[6], u16::from_le_bytes([b[8], b[9]])), (0, 11));
        let b = encode_text_list(&[(64, 0), (300, 2)]);
        assert_eq!((b[2], u16::from_le_bytes([b[4], b[5]])), (2, 300));
        assert_eq!((b[6], u16::from_le_bytes([b[8], b[9]])), (0, 64));
    }

    #[test]
    fn more_than_seven_entries_are_cut() {
        let l: Vec<(u16, u32)> = (0..10).map(|i| (i, 0)).collect();
        assert_eq!(encode_text_list(&l)[0], 7);
    }
}
