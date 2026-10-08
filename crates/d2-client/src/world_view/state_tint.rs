// Spec: specs/client/stat-lists.md (§3 r6.1, r6.3), specs/render/shading.md (§6 r1)
//! The unit state tint of the play preview: a state with a `colorshift`
//! recolours the unit it is on.
//!
//! The spec names the call (`0x004D97F0`, "color", run when a state with
//! `colorshift` ≠ 0 turns on or off) but not its body.
//!
//! PROVISIONAL (REC-245; M22): the call is read as setting the unit
//! palette index (unit `+0x6C`, `shading.md` §6 r1) to `colorshift` on
//! the state's on and to 0 on its off, so the component's `P` is remap
//! map `colorshift − 1` of the act PL2. With several tinted states on,
//! the highest `colorpri` wins; equal priorities take the lowest state
//! id. Settled by a capture of a unit under a tinted state (a shrine
//! buff, Frozen, Poison); until then nothing here counts as done
//! (rule 10).
// d2rs-own, unverified

use std::collections::BTreeSet;

use d2_data::tables::States;

use crate::rules::shading::ShadeTables;
use crate::scene::MapId;

/// `colorpri` and `colorshift` of every `states` row, by state id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateTints {
    rows: Vec<(u8, u8)>,
}

impl StateTints {
    /// The rows of the decoded `states` table (row index = state id).
    pub fn from_tables(states: &[States]) -> Self {
        StateTints {
            rows: states.iter().map(|s| (s.colorpri, s.colorshift)).collect(),
        }
    }

    /// The palette index `p` the states give a unit: the `colorshift` of
    /// the tinted state of highest `colorpri` (lowest id on a tie), 0 for
    /// none (module doc).
    pub fn palette_index(&self, states: &BTreeSet<u8>) -> u8 {
        let mut best: Option<(u8, u8)> = None;
        for &id in states {
            let Some(&(pri, shift)) = self.rows.get(usize::from(id)) else {
                continue;
            };
            if shift != 0 && best.is_none_or(|(p, _)| pri > p) {
                best = Some((pri, shift));
            }
        }
        best.map_or(0, |(_, shift)| shift)
    }

    /// The remap `P` of a unit with `states`: none without a tinted
    /// state, or when the act tables are not resident. A `colorshift`
    /// past the 128 remap maps gives none (the table cannot hold it).
    pub fn remap(&self, states: &BTreeSet<u8>, tables: Option<&ShadeTables>) -> Option<MapId> {
        let p = self.palette_index(states);
        tables?.unit_remap(p).ok().flatten()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.iter().all(|&(_, shift)| shift == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tints() -> StateTints {
        // State 0 untinted; 1: shift 5 pri 3; 2: shift 9 pri 7; 3: shift 4 pri 7;
        // 4: shift 200 (past the maps).
        StateTints {
            rows: vec![(0, 0), (3, 5), (7, 9), (7, 4), (1, 200)],
        }
    }

    // Covers: specs/render/shading.md §6 r1
    #[test]
    fn the_highest_priority_state_gives_the_palette_index() {
        let t = tints();
        assert_eq!(t.palette_index(&BTreeSet::new()), 0);
        assert_eq!(t.palette_index(&BTreeSet::from([0])), 0);
        assert_eq!(t.palette_index(&BTreeSet::from([0, 1])), 5);
        assert_eq!(t.palette_index(&BTreeSet::from([1, 2])), 9);
        // Equal priority: the lowest state id.
        assert_eq!(t.palette_index(&BTreeSet::from([2, 3])), 9);
        // A state outside the table is ignored.
        assert_eq!(t.palette_index(&BTreeSet::from([99])), 0);
    }

    // Covers: specs/render/shading.md §6 r1
    #[test]
    fn a_tint_is_remap_map_shift_minus_one() {
        use crate::scene::MapTable;
        let pl2 =
            d2_formats::palette::Pl2::parse(&super::super::tile_assets::tests::pl2()).unwrap();
        let tables = ShadeTables::push(&mut MapTable::new(), &pl2);
        let t = tints();
        assert_eq!(t.remap(&BTreeSet::new(), Some(&tables)), None);
        assert_eq!(t.remap(&BTreeSet::from([1]), None), None);
        assert_eq!(
            t.remap(&BTreeSet::from([1]), Some(&tables)),
            Some(MapId(tables.remap0.0 + 4))
        );
        assert_eq!(t.remap(&BTreeSet::from([4]), Some(&tables)), None);
    }

    // Covers: specs/render/shading.md §6 r1
    #[test]
    fn a_tinted_state_changes_the_units_palette_shift_in_the_draw_list() {
        use crate::bridge::world::{UnitKey, MONSTER};
        use crate::bridge::ClientUnit;
        use crate::rules::lighting::view::LitRules;
        use crate::rules::lighting::view_tests::{cof, frame_light, layer, request};
        use crate::world_view::preview_light::PreviewLook;
        use crate::world_view::{Unspecified, ViewRules};
        use std::sync::Arc;

        let light = frame_light(0xC8);
        let c = cof(layer(0, 0));
        let mut unit = ClientUnit::new(UnitKey::new(MONSTER, 7));
        unit.position = Some((101, 100));
        let look = PreviewLook {
            tints: Some(Arc::new(tints())),
            tables: Some(light.tables),
            ..PreviewLook::default()
        };
        let rules = LitRules {
            rules: &Unspecified,
            feed: &look,
            light: &light,
        };
        let plain = rules.shade(&unit, &request(&c)).unwrap();
        unit.states.insert(1);
        let tinted = rules.shade(&unit, &request(&c)).unwrap();
        assert_ne!(plain, tinted);
        let remap = MapId(light.tables.remap0.0 + 4);
        assert!(tinted.maps().contains(&remap), "{tinted:?}");
        assert!(!plain.maps().contains(&remap));
        unit.states.remove(&1);
        assert_eq!(rules.shade(&unit, &request(&c)).unwrap(), plain);
    }
}
