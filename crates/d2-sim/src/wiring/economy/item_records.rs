// Spec: specs/world/vendors.md §7.3 step 3 (an item from a save record, `0x00558CB0`); specs/formats/d2s.md §8.2 rules 2, 7; specs/items/generation.md §1.4, §9 step 6
//! An item unit made from one save record read back
//! (`items::bitstream::read`): the allocation (`0x00555230`, `units.md`
//! §3.1, a new GUID) at the record's mode, the decode into it
//! (`0x0062E430`, the inverse of `items/bitstream.md`, `vendors.md` Open
//! question 8), then item flag 0x80000 set and 0x2000 cleared and the
//! replenish timers. Used by the item copy (`vendors.md` §7.3) and the
//! save load (`d2s.md` §8.2).
//!
//! What the record carries goes where the writer read it from: the item
//! data to the item store, unit +0x28 (§4.1 rule 7) to the unit record's
//! init seed, the base values (`0x006253B0`: defense, max durability,
//! sockets) and the totals the writer sent (durability, gold, quantity,
//! quest difficulty) as base stats, the property lists to their keyed
//! lists. The record's position, body location and page are the item
//! data of the inventory state (`wiring::inventory`): the caller places
//! them.

use std::cell::RefCell;

use super::item_stats::{StatCtx, UnitStats};
use super::{Economy, EconomyError};
use crate::items::bitstream::read::ReadItem;
use crate::items::bitstream::{hflag, StatEntry, RUNEWORD_STATE, SET_STATES};
use crate::items::{flag, replenish_timer, stat, Item, ItemStats, ListKey};
use crate::units::hooks::Sim;
use crate::units::lifecycle::{self, AllocRequest, LifecycleHooks};
use crate::units::{RoomId, UnitId, UnitType};

/// Item timer event 3: replenish (`units.md` §6, `generation.md` §9 step 6).
const EVENT_REPLENISH: u32 = 3;
/// Flags of a set list (`items/bitstream.md` §4.6 rule 1: 0x2040, else
/// 0x40).
// TODO(spec: bitstream.md §4.6 rule 1, vendors.md OQ8): the stream does
// not say which of the two flags a set list had; read back as 0x2040.
const SET_FLAGS: u32 = 0x2040;
/// Flags of the runeword list (§4.6 rule 3: state 171, flag 0x40).
const RUNEWORD_FLAGS: u32 = 0x40;

impl<H: LifecycleHooks> Economy<'_, H> {
    /// `0x00558CB0` on a record read back: allocates an item unit of the
    /// record's class in `room` at the record's mode, decodes the record
    /// into it, sets item flag 0x80000 and clears 0x2000 (`d2s.md` §8.2
    /// rule 7, `vendors.md` §7.3 step 3) and schedules the replenish event
    /// (`generation.md` §9 step 6). No item-roll draw: the record holds
    /// the item's fields.
    pub fn item_from_record(
        &mut self,
        rec: &ReadItem,
        room: Option<RoomId>,
    ) -> Result<UnitId, EconomyError> {
        let it = &rec.item;
        let req = AllocRequest {
            ty: UnitType::Item,
            class: rec.record as u32,
            room,
            add: true,
            fixed_guid: None,
            mode: it.mode,
            allied: false,
        };
        let unit = {
            let mut sim = Sim {
                game: &mut *self.game,
                units: &mut *self.units,
                stats: &mut *self.stats,
                data: self.data,
            };
            lifecycle::allocate(&mut sim, &mut *self.hooks, &mut self.fields.seed, &req)?
        }
        .ok_or(EconomyError::NotAllocated)?;
        let mut item = Item::new(rec.record, it.version, ());
        item.ilvl = it.ilvl;
        item.quality = it.quality;
        // The ear record's class is its file index (§3 rule 3).
        item.file_index = if it.flags & hflag::EAR != 0 {
            it.ear_class
        } else {
            it.file_index
        };
        item.prefix = it.prefix;
        item.suffix = it.suffix;
        item.rare_prefix = it.rare_prefix;
        item.rare_suffix = it.rare_suffix;
        item.auto_affix = it.auto_affix;
        item.inv_page = it.page;
        item.gfx = it.gfx;
        item.name = it.name;
        item.ear_level = it.ear_level;
        item.flags = (it.flags | flag::INIT) & !flag::INSTORE;
        let r = self
            .units
            .get_mut(unit)
            .ok_or(EconomyError::NoRecord(unit))?;
        if !it.compact && !it.alt {
            r.init_seed = it.unit28;
        }
        let frame = self.game.frame as u32;
        let event3_at = {
            let ctx = RefCell::new(StatCtx::new(self.stats, self.hooks));
            let mut s = UnitStats::new(&ctx, unit);
            set_record_stats(&mut s, it);
            replenish_timer(&s, false, frame)
        };
        self.items.insert(unit, item);
        if let Some(f) = event3_at {
            // TODO(units.md §6 row 3): the event's arguments are not
            // written; scheduled with 0, 0 (as at creation).
            self.game
                .schedule_event(unit, EVENT_REPLENISH, f as i32, None, 0, 0)?;
        }
        Ok(unit)
    }
}

/// The values the record carries, where the writer read them.
fn set_record_stats(s: &mut impl ItemStats, it: &crate::items::bitstream::StreamItem) {
    if it.compact {
        if it.kind.gold {
            s.set_base(stat::GOLD, 0, it.total_gold);
        }
        if it.quest_diff {
            s.set_base(stat::QUESTITEMDIFFICULTY, 0, it.total_quest_diff);
        }
        return;
    }
    if it.alt {
        return;
    }
    if it.kind.armor {
        s.set_base(stat::ARMORCLASS, 0, it.base_defense);
    }
    if it.kind.armor || it.kind.weapon {
        s.set_base(stat::MAXDURABILITY, 0, it.base_max_dur);
        if it.base_max_dur != 0 {
            s.set_base(stat::DURABILITY, 0, it.total_dur);
        }
    } else if it.kind.gold {
        s.set_base(stat::GOLD, 0, it.total_gold);
    }
    if it.stackable {
        s.set_base(stat::QUANTITY, 0, it.total_quantity);
    }
    if it.flags & hflag::SOCKETED != 0 {
        s.set_base(stat::NUMSOCKETS, 0, it.base_sockets);
    }
    let mut put = |key: ListKey, list: &Option<Vec<StatEntry>>| {
        for e in list.iter().flatten() {
            s.list_set(key, e.stat, e.param, e.value);
        }
    };
    put(ListKey::ITEM, &it.main);
    for (i, list) in it.sets.iter().enumerate() {
        put(
            ListKey {
                state: SET_STATES[i] as u16,
                flags: SET_FLAGS,
            },
            list,
        );
    }
    put(
        ListKey {
            state: RUNEWORD_STATE as u16,
            flags: RUNEWORD_FLAGS,
        },
        &it.runeword_list,
    );
}
