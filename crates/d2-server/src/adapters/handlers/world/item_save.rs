// Spec: specs/formats/d2s.md §8.1 (item list writing), §8.2 (item list reading), §2.8 (appearance inputs); specs/items/bitstream.md §5
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
use crate::adapters::handlers::items::moves::preview_skills::sync_oskills;
use d2_formats::d2s::appearance::Equipment;

use crate::adapters::character::save::{equipment, item_list, SaveContext};

/// Stats 6 and 8 (`stats.md`): hitpoints and mana.
const HITPOINTS: u16 = 6;
const MANA: u16 = 8;

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
            let mut d = inv.desk(econ);
            let out = item_list(&d, player, &isc).map_err(|e| e.to_string());
            // `items/bitstream.md` Outputs: the writer's changes stay on
            // the items (every later save and message sees them).
            d.apply_write_backs();
            out
        });
        self.inventory = Some(inv);
        out
    }

    /// The appearance inputs of `player`'s items (`d2s-appearance.md`
    /// Inputs, `adapters::character::save::equipment` over an inventory
    /// desk) with `ctx`'s weapon class and states. `Err`: no inventory
    /// model.
    pub fn save_equipment<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        player: UnitId,
        ctx: &SaveContext,
    ) -> Result<Equipment, String> {
        let Some(mut inv) = self.inventory.take() else {
            return Err("no inventory model (WiredWorld::inventory is None)".into());
        };
        let out = self.with_economy(game, events, |econ, _| {
            let d = inv.desk(econ);
            equipment(&d, player, ctx)
        });
        self.inventory = Some(inv);
        Ok(out)
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
        self.load_list(game, events, player, entries, false)
    }

    /// [`Self::load_items`] for a corpse unit's list (`d2s.md` §8.2 rule
    /// 3, §8.3 rule 4): placed by the corpse placement
    /// (`InvDesk::load_corpse_entry`).
    pub fn load_corpse_items<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        corpse: UnitId,
        entries: &[ItemEntry],
    ) -> LoadedItems {
        self.load_list(game, events, corpse, entries, true)
    }

    fn load_list<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        player: UnitId,
        entries: &[ItemEntry],
        corpse: bool,
    ) -> LoadedItems {
        let mut r = LoadedItems::default();
        if entries.is_empty() {
            // A player is allocated with an inventory (`0x0063ABD0`,
            // `inventory.md` §1.3), also with nothing saved: ground pickups
            // and quest items need it (REC-1555).
            if let Some(mut inv) = self.inventory.take() {
                self.with_economy(game, events, |econ, _| {
                    if let Some((UnitType::Player, class, guid)) =
                        econ.units.get(player).map(|u| (u.ty, u.class, u.guid))
                    {
                        if !corpse && !inv.state.inventories.contains_key(&player) {
                            inv.state.add_inventory(
                                player,
                                UnitKind::Player { class: class as u8 },
                                guid,
                            );
                        }
                    }
                });
                self.inventory = Some(inv);
            }
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
            // The skill lists lent to the rest: the item-skill link's 0x22
            // (`inventory.md` §5.5) and the stat 97 / 107 callback's 0x21
            // (`levels.md` §7.1) of the join's item calls, in item order.
            if !corpse {
                super::wired::lend_skills(econ, &mut inv);
            }
            let mut d = inv.desk(econ);
            // `formats/d2s.md` §9 rules 2 and 4: the loader remembers
            // hitpoints and mana before the items and restores them after
            // (item bonuses do not change them: the max-rescale of
            // `stat-lists.md` §7.2 must not move the stored values).
            let remembered = [HITPOINTS, MANA].map(|s| d.econ.stats.unit_total(player, s, 0));
            for (i, e) in entries.iter().enumerate() {
                let rec = match read_save_entry(&e.bytes, d.econ.tables) {
                    Ok(rec) => rec,
                    Err(err) => {
                        r.faults
                            .push(format!("item {i}: unreadable record: {err:?}"));
                        continue;
                    }
                };
                let made = if corpse {
                    d.load_corpse_entry(player, &rec)
                } else {
                    d.load_entry(player, &rec)
                };
                match made {
                    Ok(u) => r.items.push(u),
                    Err(f) => r.faults.push(format!("item {i}: {}", fault(&f))),
                }
                // The stat callback of this item's attach (`levels.md`
                // §7.1), so its 0x21 follows the item's own messages.
                if !corpse {
                    if let Some(mut st) = d.rest.take_skills() {
                        let mut sent = Vec::new();
                        let o = d2_sim::items::moves::Owner::player(guid);
                        if st.lists.contains_key(&o) {
                            sync_oskills(
                                &mut st,
                                o,
                                |sl, stat, skill| {
                                    d.econ.stats.unit_total(sl.unit, stat, skill as u16)
                                },
                                &mut sent,
                            );
                        }
                        d.rest.queue_sent(sent);
                        d.rest.stage_skills(st, d.tables);
                    }
                }
            }
            d.sync_out();
            if !corpse {
                for (s, v) in [HITPOINTS, MANA].into_iter().zip(remembered) {
                    d.econ.stats.unit_set(&mut *d.econ.hooks, player, s, v, 0);
                }
            }
            r.sent = inv_take_sent(&mut d)
                .into_iter()
                .filter_map(|(u, b)| Some((u?, b)))
                .collect();
            if !corpse {
                super::wired::return_skills(econ, &mut inv, false);
            }
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
        LoadFault::RecordFailed => "the record failed to read (entry skipped)",
    }
}
