// Spec: specs/items/properties.md (§11 set bonuses), specs/ui/inventory.md (§5 hover state)
//! The set bonus stats of the item tool tip: what a set gives at each
//! number of worn pieces and for the full set, computed with the
//! simulation's own set-bonus rule (`d2_sim::items::props::set_bonuses`)
//! into a recording stat list.
//!
//! PROVISIONAL (REC-242): the tip lists every step of the set (green)
//! and the full-set bonus (orange) whatever the player wears, because the
//! tip builder behind `0x0048DD90` is unwritten; settled by the set item
//! hover capture (`ui/text.md` capture `text-0002`). d2rs-own,
//! unverified.

use std::collections::BTreeMap;

use d2_proto::item_bits::Stat;
use d2_sim::items::props::set_bonuses;
use d2_sim::items::{Item, ItemStats, ItemTables, ListKey};
use d2_sim::rng::Seed;

/// A stat list that records what the property rules write.
#[derive(Default)]
struct Recorder(BTreeMap<(u16, u16), i32>);

impl ItemStats for Recorder {
    fn has_stats(&self) -> bool {
        true
    }
    fn stat(&self, id: u16, layer: u16) -> i32 {
        self.0.get(&(id, layer)).copied().unwrap_or(0)
    }
    fn base(&self, id: u16, layer: u16) -> i32 {
        self.stat(id, layer)
    }
    fn set_base(&mut self, id: u16, layer: u16, value: i32) {
        self.0.insert((id, layer), value);
    }
    fn has_list(&self, _: ListKey) -> bool {
        true
    }
    fn list_set(&mut self, _: ListKey, id: u16, layer: u16, value: i32) {
        self.0.insert((id, layer), value);
    }
    fn list_add(&mut self, _: ListKey, id: u16, layer: u16, value: i32) {
        *self.0.entry((id, layer)).or_default() += value;
    }
    fn list_get(&self, _: ListKey, id: u16, layer: u16) -> i32 {
        self.stat(id, layer)
    }
}

/// The bonuses of the set of setitems row `item_row`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SetBonuses {
    /// Pieces needed for the full set.
    pub count: u32,
    /// `(pieces, stats)` for 2 … `count − 1` pieces.
    pub partial: Vec<(u32, Vec<Stat>)>,
    /// The full-set bonus stats.
    pub full: Vec<Stat>,
}

fn record(t: &ItemTables, item_row: i32, pieces: u32) -> BTreeMap<(u16, u16), i32> {
    let mut item = Item {
        record: 0,
        format: 0,
        ilvl: 0,
        quality: 5,
        file_index: item_row,
        prefix: [0; 3],
        suffix: [0; 3],
        rare_prefix: 0,
        rare_suffix: 0,
        auto_affix: 0,
        flags: 0,
        inv_page: 0xFF,
        gfx: 0,
        unit_seed: Seed { lo: 0, hi: 0 },
        init_seed: 0,
        item_seed: Seed { lo: 0, hi: 0 },
        start_seed: 0,
        name: [0; 16],
        ear_level: 0,
        stats: Recorder::default(),
    };
    let mut owner = Recorder::default();
    let mask = if pieces >= 32 {
        u32::MAX
    } else {
        (1 << pieces) - 1
    };
    set_bonuses(t, &mut item, mask, &mut owner, ListKey::ITEM);
    owner.0
}

fn stats(
    t: &ItemTables,
    now: &BTreeMap<(u16, u16), i32>,
    before: &BTreeMap<(u16, u16), i32>,
) -> Vec<Stat> {
    now.iter()
        .filter_map(|(&(id, layer), &v)| {
            let d = v - before.get(&(id, layer)).copied().unwrap_or(0);
            let shift = t.valshift.get(usize::from(id)).copied().unwrap_or(0);
            let value = i64::from(d) >> shift;
            (value != 0).then_some(Stat {
                stat: id,
                param: u32::from(layer),
                raw: value as u32,
                save_add: 0,
            })
        })
        .collect()
}

/// The bonuses of the set that setitems row `item_row` belongs to;
/// `None` when the row or its set is unknown.
pub fn bonuses(t: &ItemTables, item_row: usize) -> Option<SetBonuses> {
    let set = usize::try_from(t.setitems.get(item_row)?.set).ok()?;
    let count = u32::try_from(t.sets.get(set)?.count).ok()?;
    let row = item_row as i32;
    let mut prev = record(t, row, 1);
    let mut partial = Vec::new();
    for k in 2..count {
        let now = record(t, row, k);
        partial.push((k, stats(t, &now, &prev)));
        prev = now;
    }
    let full = record(t, row, count.max(1));
    Some(SetBonuses {
        count,
        partial,
        full: stats(t, &full, &prev),
    })
}
