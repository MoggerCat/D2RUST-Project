// Spec: specs/items/bitstream.md (Inputs); specs/items/inventory-moves.md §11
// Spec: specs/items/inventory.md (the sections other than §6–§11)
//! The item bit stream of the deferred item messages on the real item:
//! [`InvDesk::stream_item`] reads the fields the writer
//! (`items::bitstream`) names from their owners (unit record: mode; item
//! store: item data; inventory state: body location, position, the
//! item's own inventory; item tables: items / itemtypes / itemstatcost;
//! stat lists: base, total and the property lists) and
//! [`InvDesk::item_stream`] writes it. Nothing here decides a rule.

use super::inv_world::STAT_SOCKETS;
use super::{InvDesk, InvRest};
use crate::items::bitstream::{self, Kind, StatEntry, StreamItem};
use crate::items::moves::Guid;
use crate::items::{props, stat, ty, ListKey};
use crate::stats::{key_layer, key_stat};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;
use crate::wiring::economy::find_list;

/// Set-list flags (§4.6 rule 1: 0x2040, else 0x40).
const SET_FLAGS: u32 = 0x2040;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// The writer's view of `item` with the sender's flag argument OR-ed
    /// into the item flags and `page` as the page (`inventory-moves.md` §11).
    /// `None`: no item unit, record or item data.
    pub fn stream_item(&self, item: Guid, flags: u32, page: u8) -> Option<StreamItem> {
        let u = self.item_unit(item)?;
        let r = self.econ.units.get(u)?;
        let it = self.econ.items.get(u)?;
        let d = self.state.items.get(&u)?;
        let t = self.econ.tables;
        let rec = t.item(it.record)?;
        let is = |k: u16| t.is_type(it.record, k as i16);
        let stats = &*self.econ.stats;
        let base = |s: u16| stats.unit_base(u, s, 0);
        let total = |s: u16| stats.unit_total(u, s, 0);
        let list = |state: u16, flags: u32| -> Option<Vec<StatEntry>> {
            let l = find_list(stats, u, ListKey { state, flags })?;
            Some(
                stats
                    .base_entries(l)
                    .into_iter()
                    .map(|(k, v)| StatEntry {
                        stat: key_stat(k),
                        param: key_layer(k),
                        value: v,
                    })
                    .collect(),
            )
        };
        let filled = if rec.hasinv != 0 {
            self.state
                .inventories
                .get(&u)
                .map_or(0, |i| i.items().len() as u32)
        } else {
            0
        };
        let mut sets: [Option<Vec<StatEntry>>; 5] = Default::default();
        for (slot, &s) in sets.iter_mut().zip(bitstream::SET_STATES.iter()) {
            *slot = list(s as u16, SET_FLAGS).or_else(|| list(s as u16, ListKey::ITEM.flags));
        }
        // `bitstream.md` §4.1 r2–r3: x / y are the static path's +0x0C /
        // +0x10, the ground sub-tile in modes 3 and 5 (the path
        // provider's record), the grid cell otherwise (the item data).
        let (x, y) = match r.mode {
            3 | 5 => self.econ.hooks.static_position(u).unwrap_or((d.x, d.y)),
            _ => (d.x, d.y),
        };
        Some(StreamItem {
            flags: it.flags | flags,
            alt: false,
            compact: rec.compactsave != 0,
            // The item format (`generation.md` §1.2) is the stream's
            // version field (§4.1 rule 1: 101 in 1.14d expansion games).
            version: it.format,
            mode: r.mode,
            x,
            y,
            body_loc: d.body_loc,
            page,
            code: rec.code,
            base_code: rec.normcode,
            ear_class: it.file_index,
            ear_level: it.ear_level,
            name: it.name,
            filled,
            ilvl: it.ilvl,
            quality: it.quality,
            file_index: it.file_index,
            varinvgfx: t.itype_of(it.record).is_some_and(|y| y.varinvgfx != 0),
            gfx: it.gfx,
            auto_affix: it.auto_affix,
            prefix: it.prefix,
            suffix: it.suffix,
            rare_prefix: it.rare_prefix,
            rare_suffix: it.rare_suffix,
            runeword: self.runeword_name(u),
            kind: Kind {
                armor: is(ty::ARMO),
                weapon: is(ty::WEAP),
                gold: is(ty::GOLD),
                charm: is(ty::CHAR),
                body_part: is(ty::BODY) && !is(ty::PLAY),
                scroll_or_book: is(ty::SCRO) || is(ty::BOOK),
            },
            stackable: rec.stackable != 0,
            quest_diff: rec.quest != 0 && rec.questdiffcheck != 0,
            base_defense: base(stat::ARMORCLASS),
            base_max_dur: base(stat::MAXDURABILITY),
            base_sockets: base(stat::NUMSOCKETS),
            total_dur: total(stat::DURABILITY),
            total_gold: total(stat::GOLD),
            total_quantity: total(stat::QUANTITY),
            total_quest_diff: total(stat::QUESTITEMDIFFICULTY),
            main: list(ListKey::ITEM.state, ListKey::ITEM.flags),
            sets,
            runeword_list: list(bitstream::RUNEWORD_STATE as u16, ListKey::ITEM.flags),
            ..Default::default()
        })
    }

    /// The runeword record's name string id (`bitstream.md` §4.4 rule 1):
    /// the match of `properties.md` §10.1 on the item's own inventory
    /// (class ids in list order) and its socket count (stat 194 as u8);
    /// no record → 0xFFFF.
    pub fn runeword_name(&self, u: UnitId) -> u16 {
        let Some(it) = self.econ.items.get(u) else {
            return 0xFFFF;
        };
        let fillers: Vec<usize> = self
            .state
            .inventories
            .get(&u)
            .map(|inv| {
                inv.items()
                    .iter()
                    .filter_map(|&f| self.econ.items.get(f).map(|x| x.record))
                    .collect()
            })
            .unwrap_or_default();
        let sockets = self.econ.stats.unit_total(u, STAT_SOCKETS, 0) as u8;
        let t = self.econ.tables;
        props::runeword_row(t, it.record, it.quality, sockets, &fillers)
            .and_then(|row| t.runes.get(row))
            .map_or(0xFFFF, |w| w.name_id)
    }

    /// The stream bytes of `item` (`0x006313E0`, network case). Empty
    /// when the item has no stream view or the buffer overflows (§1 rule
    /// 2: the message carries no stream). The writer's changes to the
    /// item (§4.1 rule 8: item level < 1 → 1; §4.3 rule 7: quality
    /// outside 1–9 → 2) are queued in the state and written to the item
    /// store by the next [`InvDesk::apply_write_backs`] (the seam
    /// `MovePending::item_bits` reads the world only); the overflow of
    /// rule 2 comes after them, so they are kept on overflow too.
    pub fn item_stream(&self, item: Guid, flags: u32, page: u8) -> Vec<u8> {
        let Some(view) = self.stream_item(item, flags, page) else {
            return Vec::new();
        };
        let mut w = bitstream::BitWriter::new(bitstream::BUFFER);
        let wb = bitstream::write_into(&mut w, &view, &self.econ.tables.isc);
        if wb.ilvl != view.ilvl || wb.quality != view.quality {
            if let Some(u) = self.item_unit(item) {
                self.state.write_backs.borrow_mut().push((u, wb));
            }
        }
        w.finish().unwrap_or_default()
    }

    /// Writes the queued writer changes (item level, quality) to the item
    /// store, in queue order.
    pub fn apply_write_backs(&mut self) {
        let queued = std::mem::take(self.state.write_backs.get_mut());
        for (u, wb) in queued {
            if let Some(it) = self.econ.items.get_mut(u) {
                it.ilvl = wb.ilvl;
                it.quality = wb.quality;
            }
        }
    }
}
