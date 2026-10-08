// Spec: specs/world/vendors.md §1, §3, §5, §9; specs/data/loading.md §9
//! (q-smoke-town) The synthetic game's vendor tables, over
//! [`super::synthetic_items::item_tables`], so a trade or gamble open in
//! the synthetic play game shows a store: every trader's column
//! (`vendors::column_of`) lists the cap as a permanent item and the
//! buckler as a range entry (Min 1, Max 2), every trader prices with one
//! `npc.txt` row (sell 1024, buy 512, repair 128, max buy 5000, as the
//! `e2e_vendor` fixtures), and the gamble list draws from both. With game
//! files the user's tables give all of it. PROVISIONAL (REC-278): every
//! value is `d2rs-own, unverified`.

use d2_sim::items::{ty, ItemTables};
use d2_sim::world::vendors::{
    column_of, GambleOdds, NpcPrices, TypeRec, VendorItem, VendorTables, COLUMNS, NO_CODE, XXX,
};

use super::synthetic_items::{BUCKLER, CAP};

/// The vendor tables for the synthetic game. `interact` is the
/// `monstats` classes with `interact`, in row order (`vendors.md` §1 r2).
pub fn vendor_tables(items: &ItemTables, interact: Vec<u16>, monstats_rows: usize) -> VendorTables {
    let rows: Vec<VendorItem> = items
        .items
        .iter()
        .map(|r| {
            let mut v = VendorItem {
                code: r.code,
                normcode: r.code,
                ubercode: NO_CODE,
                ultracode: NO_CODE,
                cost: 0,
                type_: r.type_,
                type2: -1,
                level: 1,
                spawnable: 1,
                durability: r.durability,
                minac: r.minac,
                maxac: r.maxac,
                nightmare_upgrade: XXX,
                hell_upgrade: XXX,
                ..VendorItem::default()
            };
            if r.code == CAP {
                v.cost = 100;
                v.perm_store = 1;
                v.columns = [[1, 1, 0, 0, 0]; COLUMNS];
            } else if r.code == BUCKLER {
                v.cost = 80;
                v.columns = [[1, 2, 0, 0, 0]; COLUMNS];
            }
            v
        })
        .collect();
    let gamble: Vec<u32> = items
        .items
        .iter()
        .enumerate()
        .filter(|(_, r)| r.code == CAP || r.code == BUCKLER)
        .map(|(i, _)| i as u32)
        .collect();
    let mut itemtypes = vec![
        TypeRec {
            repair: 1,
            class: 7,
            storepage: 3,
            staffmods: 0xFF,
            ..TypeRec::default()
        };
        items.itemtypes.len()
    ];
    itemtypes[usize::from(ty::HELM)].storepage = 0;
    itemtypes[usize::from(ty::SHIE)].storepage = 0;
    let npc = interact
        .iter()
        .filter(|c| column_of(**c).is_some())
        .map(|&c| NpcPrices {
            class: u32::from(c),
            sell: 1024,
            buy: 512,
            rep: 128,
            quests: [(0, 0, 0, 0); 3],
            max_buy: [5000; 3],
        })
        .collect();
    VendorTables {
        items: rows,
        itemtypes,
        equiv: items.equiv.clone(),
        stat_shift: items.stat_shift,
        stat_mask: items.stat_mask,
        monster_levels: vec![[1, 1, 1]; monstats_rows],
        interact,
        npc,
        gamble_index: Some(gamble),
        gamble_thresholds: vec![2; 100],
        difficulty: vec![GambleOdds::default(); 3],
        ..VendorTables::default()
    }
}
