// Spec: specs/world/vendors-2.md §7.3 step 3 (an item from a save record, `0x00558CB0`); specs/formats/d2s.md §8.2 rules 2, 7; specs/items/generation.md §1.4, §9 step 6
//! An item unit made from one save record read back
//! (`items::bitstream::read`): the allocation (`0x00555230`, `units.md`
//! §3.1, a new GUID) at the record's mode, the decode into it
//! (`0x0062E430`, the inverse of `items/bitstream.md`, `vendors.md` Open
//! question 8), then item flag 0x80000 set and 0x2000 cleared and the
//! replenish timers. Used by the item copy (`vendors-2.md` §7.3) and the
//! save load (`d2s.md` §8.2).
//!
//! What the record carries goes where the writer read it from: the item
//! data to the item store, the save-only 32 bits (§4.1 rule 7) to the
//! item seed (`sim/rng.md` §5.3, `0x0062CBE0`), the base values (`0x006253B0`: defense, max durability,
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
use crate::items::tables::ItemRec;
use crate::items::{flag, replenish_timer, stat, ty, Item, ItemStats, ListKey};
use crate::rng::Seed;
use crate::units::hooks::Sim;
use crate::units::lifecycle::{self, AllocRequest, LifecycleHooks};
use crate::units::{RoomId, UnitId, UnitType};

/// Stat 57 `poisonmindam` (§7.3.1 rule 3).
const POISONMINDAM: u16 = 57;
/// Item timer event 3: replenish (`units.md` §6, `generation.md` §9 step 6).
const EVENT_REPLENISH: u32 = 3;
/// Flags of a set list read back (`items/bitstream.md` §4.6 rule 6:
/// 0x2040).
const SET_FLAGS: u32 = 0x2040;
/// Flags of the runeword list (§4.6 rule 3: state 171, flag 0x40).
const RUNEWORD_FLAGS: u32 = 0x40;

impl<H: LifecycleHooks> Economy<'_, H> {
    /// `0x00558CB0` on a record read back: allocates an item unit of the
    /// record's class in `room` at the record's mode, decodes the record
    /// into it, sets item flag 0x80000 and clears 0x2000 (`d2s.md` §8.2
    /// rule 7, `vendors-2.md` §7.3 step 3) and schedules the replenish event
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
        // `rng.md` §5.3 "item seed from a save": the full record's 32 bits
        // v (`0x0062CBE0`); the compact reader `0x0062A970` sets 0
        // (`vendors-2.md` §7.3.1 rule 5). Unit init seed +0x28 := v and the
        // unit seed +0x20 / +0x24 := `init_low(v)` = {v, 666}
        // (`0x00650E40`, `rng.md` §5.1). The item data seed +0x04 and the
        // start seed +0x10 keep the allocation's game-seed step.
        let v = if it.compact || it.alt { 0 } else { it.unit28 };
        r.init_seed = v;
        r.seed = Seed::init_low(v);
        let frame = self.game.frame as u32;
        let event3_at = {
            let ctx = RefCell::new(StatCtx::new(self.stats, self.hooks));
            let mut s = UnitStats::new(&ctx, unit);
            let t = self.tables;
            let rebuild = t.item(rec.record).map(|r| Rebuild {
                rec: r.clone(),
                weapon: t.is_type(rec.record, ty::WEAP as i16),
                armor: t.is_type(rec.record, ty::ARMO as i16),
                throwable: t.itype_of(rec.record).is_some_and(|y| y.throwable != 0),
            });
            set_record_stats(&mut s, it, rebuild.as_ref());
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

/// The items-table facts the decoder rebuilds base stats from
/// (`vendors-2.md` §7.3.1).
struct Rebuild {
    rec: ItemRec,
    weapon: bool,
    armor: bool,
    throwable: bool,
}

/// §7.3.1 rule 1: the weapon's base speed and damage of the decoder
/// (`0x0062CBE0`): the items columns, ⌊3v / 4⌋ with floors for quality 1,
/// × 3 / 2 for item flag 0x400000.
fn rebuild_weapon(s: &mut impl ItemStats, rb: &Rebuild, quality: u8, flags: u32) {
    let r = &rb.rec;
    s.set_base(stat::ATTACKRATE, 0, r.speed.wrapping_neg());
    let missile = r.maxmisdam != 0;
    let mut six = [
        (stat::MAXDAMAGE, i32::from(r.maxdam), true, 2),
        (stat::MINDAMAGE, i32::from(r.mindam), true, 1),
        (stat::SECONDARY_MAXDAMAGE, i32::from(r.maxdam2), true, 2),
        (stat::SECONDARY_MINDAMAGE, i32::from(r.mindam2), true, 1),
        (stat::THROW_MINDAMAGE, i32::from(r.minmisdam), missile, 1),
        (stat::THROW_MAXDAMAGE, i32::from(r.maxmisdam), missile, 2),
    ];
    for (id, v, set, floor) in &mut six {
        if !*set {
            continue;
        }
        if quality == 1 {
            *v = (v.wrapping_mul(3) / 4).max(*floor);
        }
        if flags & flag::ETHEREAL != 0 {
            *v = v.wrapping_mul(3) / 2;
        }
        s.set_base(*id, 0, *v);
    }
}

/// §7.3.1 rule 3: an entry for stat 17 raises the base maximum damages,
/// one for stat 18 the base minimum damages, to the items column when
/// below it (the throw damage only for a throwable item).
fn raise_damage(s: &mut impl ItemStats, rb: &Rebuild, max: bool) {
    let r = &rb.rec;
    let cols = if max {
        [
            (stat::MAXDAMAGE, r.maxdam, true),
            (stat::SECONDARY_MAXDAMAGE, r.maxdam2, true),
            (stat::THROW_MAXDAMAGE, r.maxmisdam, rb.throwable),
        ]
    } else {
        [
            (stat::MINDAMAGE, r.mindam, true),
            (stat::SECONDARY_MINDAMAGE, r.mindam2, true),
            (stat::THROW_MINDAMAGE, r.minmisdam, rb.throwable),
        ]
    };
    for (id, col, go) in cols {
        let col = i32::from(col);
        if go && s.base(id, 0) < col {
            s.set_base(id, 0, col);
        }
    }
}

/// The values the record carries, where the writer read them.
fn set_record_stats(
    s: &mut impl ItemStats,
    it: &crate::items::bitstream::StreamItem,
    rebuild: Option<&Rebuild>,
) {
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
    // §7.3.1 rules 1–2: the base values the stream does not carry.
    if let Some(rb) = rebuild {
        if rb.weapon {
            rebuild_weapon(s, rb, it.quality, it.flags);
        } else if rb.armor {
            s.set_base(stat::TOBLOCK, 0, i32::from(rb.rec.block));
            s.set_base(stat::VELOCITYPERCENT, 0, rb.rec.speed.wrapping_neg());
        }
    }
    let mut put = |key: ListKey, list: &Option<Vec<StatEntry>>| {
        for e in list.iter().flatten() {
            if let Some(rb) = rebuild {
                // Rule 3: before the entry is stored.
                match e.stat {
                    stat::MAXDAMAGE_PERCENT => raise_damage(s, rb, true),
                    stat::MINDAMAGE_PERCENT => raise_damage(s, rb, false),
                    _ => {}
                }
            }
            s.list_set(key, e.stat, e.param, e.value);
            // Rule 3: an entry for stat 57 (read with 58, 59) sets
            // `poison_count` to 1 in that list.
            if e.stat == POISONMINDAM {
                s.list_set(key, stat::POISON_COUNT, 0, 1);
            }
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
