// Spec: specs/formats/d2s.md §8.1 (item list writing), §8.2 (item list reading); specs/items/bitstream.md §5
//! The player's items through a save: the item list written from the
//! wired host's economy and inventory model ([`WiredWorld::save_items`],
//! `adapters::character::save::item_list` over an inventory desk) and the
//! list read back onto a new player unit ([`WiredWorld::load_items`],
//! `InvDesk::load_entry`). Placement on load is PROVISIONAL (REC-115).

use d2_formats::d2s::ItemEntry;
use d2_sim::items::bitstream::read::read_save_entry;
use d2_sim::items::inventory::UnitKind;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::inventory::load::LoadFault;

use super::{inv_take_sent, ActionEvents, Game, WiredWorld};
use crate::adapters::character::save::item_list;

/// What the load of a save's item list did ([`WiredWorld::load_items`]).
#[derive(Debug, Default)]
pub struct LoadedItems {
    /// Each placed top-level item, in list order.
    pub items: Vec<UnitId>,
    /// What the placements queued (receiving unit, bytes), in send order:
    /// the join's item messages.
    pub sent: Vec<(UnitId, Vec<u8>)>,
    /// Why an entry made no item (record unreadable, no room, no model).
    pub faults: Vec<String>,
}

impl<R, S> WiredWorld<R, S> {
    /// The save entries of `player`'s items (`d2s.md` §8.1 rules 2–4, 10).
    /// `Err`: no inventory model, or an item that does not fit the buffer.
    pub fn save_items<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        player: UnitId,
    ) -> Result<Vec<ItemEntry>, String> {
        let Some(mut inv) = self.inventory.take() else {
            return Err("no inventory model (WiredWorld::inventory is None)".into());
        };
        let out = self.with_economy(game, events, |econ, _| {
            let isc = econ.tables.isc.clone();
            let d = inv.desk(econ);
            item_list(&d, player, &isc).map_err(|e| e.to_string())
        });
        self.inventory = Some(inv);
        out
    }

    /// Makes the items of a save's list on `player` and places them
    /// (`d2s.md` §8.2). The player's inventory is added to the model when
    /// it has none, as [`WiredWorld::start_items`] does.
    pub fn load_items<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        player: UnitId,
        entries: &[ItemEntry],
    ) -> LoadedItems {
        let mut r = LoadedItems::default();
        if entries.is_empty() {
            return r;
        }
        let Some(mut inv) = self.inventory.take() else {
            r.faults
                .push("no inventory model (WiredWorld::inventory is None)".into());
            return r;
        };
        self.with_economy(game, events, |econ, _| {
            let Some((ty, class, guid)) = econ.units.get(player).map(|u| (u.ty, u.class, u.guid))
            else {
                r.faults.push(format!("no player unit {player:?}"));
                return;
            };
            if !inv.state.inventories.contains_key(&player) {
                // The hireling and the golem lists are monsters' (q-save-gaps).
                let kind = if ty == UnitType::Monster {
                    UnitKind::Monster { class }
                } else {
                    UnitKind::Player { class: class as u8 }
                };
                inv.state.add_inventory(player, kind, guid);
            }
            let mut d = inv.desk(econ);
            for (i, e) in entries.iter().enumerate() {
                let rec = match read_save_entry(&e.bytes, d.econ.tables) {
                    Ok(rec) => rec,
                    Err(err) => {
                        r.faults
                            .push(format!("item {i}: unreadable record: {err:?}"));
                        continue;
                    }
                };
                match d.load_entry(player, &rec) {
                    Ok(u) => r.items.push(u),
                    Err(f) => r.faults.push(format!("item {i}: {}", fault(&f))),
                }
            }
            d.sync_out();
            r.sent = inv_take_sent(&mut d)
                .into_iter()
                .filter_map(|(u, b)| Some((u?, b)))
                .collect();
        });
        self.inventory = Some(inv);
        r
    }
}

fn fault(f: &LoadFault) -> &'static str {
    match f {
        LoadFault::NotCreated => "no item unit could be made from the record",
        LoadFault::NoInventory => "the owner has no inventory",
        LoadFault::NoRoom => "no place for the item (freed)",
        LoadFault::StaleRuneword => "runeword flag without a matching runeword (freed)",
    }
}
