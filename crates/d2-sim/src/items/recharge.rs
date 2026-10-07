// Spec: specs/items/generation.md
//! Recharge `0x0055FE80` and set the charges `0x0065C940` (§12.2): every
//! charged skill (stat 204) whose charges are below its maximum is set
//! back to the maximum.

use super::{ItemStats, ListKey, LIST_FLAGS};
use crate::items::props::STATE_RUNEWORD;

/// Stat 204 `item_charged_skill`.
pub const CHARGED_SKILL: u16 = 204;

/// The lists with flag 0x40 that set the charges searches, in order.
// PROVISIONAL (generation.md §12.2 "X's stat lists with flag 0x40 in list
// order"): the item list (state 0), then the runeword list (state 171);
// a charged skill's key is in one list only, so the order shows only when
// both hold it; settled by: recharge of a runeword item with charges on a
// base that has the same charged skill (save capture).
const LISTS: [ListKey; 2] = [
    ListKey::ITEM,
    ListKey {
        state: STATE_RUNEWORD,
        flags: LIST_FLAGS,
    },
];

/// Set the charges `0x0065C940`(X, key, n) on X's own lists: the first
/// stat-204 entry with layer `key` whose maximum mx (entry >> 8) is in
/// 1 … 255 becomes mx × 256 + (min(n′, mx) & 0xFF), n′ := 0 when n < 1.
/// True when written; false when no list holds the entry or mx is out of
/// range (the caller then tries X's own inventory items).
pub fn set_charges<S: ItemStats>(stats: &mut S, key: u16, n: i32) -> bool {
    for list in LISTS {
        if !stats.has_list(list) {
            continue;
        }
        let entry = stats.list_get(list, CHARGED_SKILL, key);
        if entry == 0 {
            continue;
        }
        let mx = entry >> 8;
        if !(1..=255).contains(&mx) {
            return false;
        }
        let n = if n < 1 { 0 } else { n };
        stats.list_set(list, CHARGED_SKILL, key, mx * 256 + (n.min(mx) & 0xFF));
        return true;
    }
    false
}

/// One charged skill §12.2 step 4 recharges: its layer key k′ and the
/// value S→C 0x3E announces ((m & 0xFF) + m × 256).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recharged {
    pub key: u16,
    pub value: i32,
}

/// §12.2 steps 3–4 over X's stat-204 entries `entries` ((layer, value) of
/// the aggregate list, at most 64, in order): for each with charges c
/// below the maximum m, `set` (set the charges of key k to m) is called.
/// Returns the recharged entries; empty = result 0.
pub fn recharge(entries: &[(u16, i32)], mut set: impl FnMut(u16, i32)) -> Vec<Recharged> {
    let mut out = Vec::new();
    for &(k, v) in entries {
        let m = v >> 8;
        let c = v & 0xFF;
        if c < m {
            set(k, m);
            out.push(Recharged {
                key: k,
                value: (m & 0xFF) + m * 256,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::tests::FakeStats;

    // Covers: specs/items/generation.md §12.2 r3, §12.2 r4
    #[test]
    fn recharge_sets_entries_below_max() {
        let entries = [(0x0C1, 5 * 256 + 3), (0x0C2, 4 * 256 + 4), (0x0C3, 2 * 256)];
        let mut set = Vec::new();
        let r = recharge(&entries, |k, m| set.push((k, m)));
        assert_eq!(set, [(0x0C1, 5), (0x0C3, 2)]);
        assert_eq!(
            r,
            [
                Recharged {
                    key: 0x0C1,
                    value: 5 + 5 * 256
                },
                Recharged {
                    key: 0x0C3,
                    value: 2 + 2 * 256
                },
            ]
        );
        assert!(recharge(&[], |_, _| ()).is_empty());
    }

    // Covers: specs/items/generation.md §12.2 text
    #[test]
    fn set_charges_clamps_and_finds_list() {
        let mut s = FakeStats::default();
        s.list_set(LISTS[1], CHARGED_SKILL, 7, 10 * 256 + 2);
        assert!(set_charges(&mut s, 7, 12));
        assert_eq!(s.list_get(LISTS[1], CHARGED_SKILL, 7), 10 * 256 + 10);
        assert!(set_charges(&mut s, 7, 0));
        assert_eq!(s.list_get(LISTS[1], CHARGED_SKILL, 7), 10 * 256);
        assert!(!set_charges(&mut s, 8, 3));
        s.list_set(ListKey::ITEM, CHARGED_SKILL, 9, 300 * 256);
        assert!(!set_charges(&mut s, 9, 3));
        assert_eq!(s.list_get(ListKey::ITEM, CHARGED_SKILL, 9), 300 * 256);
    }
}
