// Spec: specs/items/generation.md §2, §3, Inputs; specs/sim/units.md §3; specs/sim/rng.md §5.3; specs/sim/tick.md §5
//! Item units: allocation through the unit allocator (`units.md` §3.1),
//! creation through the items pipeline (`generation.md` §3) on the real
//! stat lists, removal (`units.md` §3.2), and the item fields other
//! systems read ([`ItemStore`]).
//!
//! One owner per field: the unit record holds the unit seed (+0x20), the
//! init seed and the item/start seeds (item data +0x04, +0x10); the store
//! holds the rest of the item data; stats live in the unit's stat list.
//! [`Economy::with_item`] assembles an [`Item`] from the three for an
//! items call and writes the seeds back after it.

use std::cell::RefCell;
use std::collections::BTreeMap;

use super::item_stats::{StatCtx, UnitStats};
use super::{EconomyError, GameFields};
use crate::game::Game;
use crate::items::{create, Item, ItemRequest, ItemTables, PlayerInfo, RequestUnit};
use crate::stats::StatLists;
use crate::units::hooks::{Sim, UnitData};
use crate::units::lifecycle::{self, AllocRequest, LifecycleHooks};
use crate::units::record::Units;
use crate::units::{RoomId, UnitId, UnitType};

/// Stat 12 (`level`).
const STAT_LEVEL: u16 = 12;
/// Item timer event 3: replenish (`units.md` §6, `unit-handlers.tsv`).
const EVENT_REPLENISH: u32 = 3;

/// The item data of every item unit (the stats holder is `()`: stats are
/// in the stat lists; seeds are in the unit record).
#[derive(Clone, Debug, Default)]
pub struct ItemStore {
    items: BTreeMap<UnitId, Item<()>>,
}

impl ItemStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, unit: UnitId) -> Option<&Item<()>> {
        self.items.get(&unit)
    }
    pub fn get_mut(&mut self, unit: UnitId) -> Option<&mut Item<()>> {
        self.items.get_mut(&unit)
    }
    pub fn contains(&self, unit: UnitId) -> bool {
        self.items.contains_key(&unit)
    }
    /// Stores the item data of a new item unit (`item_records`).
    pub(super) fn insert(&mut self, unit: UnitId, item: Item<()>) {
        self.items.insert(unit, item);
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// The allocation fields of a request (`generation.md` Inputs: spawn
/// mode, room, init flags; positions belong to the path spec).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemSpawn {
    pub room: Option<RoomId>,
    /// Unit mode at allocation (3 ground, 4 inventory / cursor).
    pub mode: u32,
    /// Allocation flags.
    pub init_flags: u32,
}

/// Moves an item's fields to a new stats holder.
pub(super) fn swap_stats<A, B>(i: Item<A>, stats: B) -> (Item<B>, A) {
    let Item {
        record,
        format,
        ilvl,
        quality,
        file_index,
        prefix,
        suffix,
        rare_prefix,
        rare_suffix,
        auto_affix,
        flags,
        inv_page,
        gfx,
        unit_seed,
        init_seed,
        item_seed,
        start_seed,
        name,
        ear_level,
        realm_data,
        fatal,
        stats: old,
    } = i;
    let item = Item {
        record,
        format,
        ilvl,
        quality,
        file_index,
        prefix,
        suffix,
        rare_prefix,
        rare_suffix,
        auto_affix,
        flags,
        inv_page,
        gfx,
        unit_seed,
        init_seed,
        item_seed,
        start_seed,
        name,
        ear_level,
        realm_data,
        fatal,
        stats,
    };
    (item, old)
}

/// What an items call sees inside [`Economy::with_item`].
pub struct ItemScope<'s, 'r, 'a> {
    pub tables: &'s ItemTables,
    pub game: &'s mut GameFields,
    pub item: &'s mut Item<UnitStats<'r, 'a>>,
    ctx: &'r RefCell<StatCtx<'a>>,
}

impl<'r, 'a> ItemScope<'_, 'r, 'a> {
    /// Another unit's stats on the same lists (e.g. the owner of a set
    /// item, `properties.md` §11).
    pub fn unit_stats(&self, unit: UnitId) -> UnitStats<'r, 'a> {
        UnitStats::new(self.ctx, unit)
    }
}

/// Everything the economy seams work on: the game, the unit records and
/// stat lists, the unit hooks (also the stat host), the game-creation
/// fields, the item tables and the item store.
pub struct Economy<'a, H> {
    pub game: &'a mut Game,
    pub units: &'a mut Units,
    pub stats: &'a mut StatLists,
    pub data: &'a UnitData,
    pub hooks: &'a mut H,
    pub fields: &'a mut GameFields,
    pub tables: &'a ItemTables,
    pub items: &'a mut ItemStore,
}

impl<H: LifecycleHooks> Economy<'_, H> {
    /// Item creation `0x00558D90` (`generation.md` §3) on a new item
    /// unit. Steps 1–2's failures come before the allocation; the unit
    /// is allocated by `0x00555230` (`units.md` §3.1, with its stat list,
    /// `stat-lists.md` §4.3); a failed creation removes it (`generation.md`
    /// Outputs). A due replenish event 3 is scheduled (`tick.md` §5).
    ///
    /// Both the allocator and `create_item` derive the unit and item
    /// seeds from the game seed (`rng.md` §5.3, `generation.md` §2.1); the
    /// allocator runs on a copy of the game seed and the two results
    /// must agree ([`EconomyError::SeedMismatch`] otherwise), so the game
    /// seed steps exactly twice.
    pub fn create_item(
        &mut self,
        rq: &mut ItemRequest,
        use_seed: bool,
        spawn: ItemSpawn,
    ) -> Result<UnitId, EconomyError> {
        let t = self.tables;
        let rec = usize::try_from(rq.item)
            .ok()
            .and_then(|i| t.item(i).map(|r| (i, r)));
        if !self.fields.expansion && rec.is_none_or(|(_, r)| r.version >= 100) {
            return Err(create::CreateError::Classic.into());
        }
        let Some((idx, _)) = rec else {
            return Err(create::CreateError::BadIndex.into());
        };
        // TODO(units.md §3.1 step 8): "flags bit 1" is read as the value
        // 0x1 (drop and cube requests pass init flags 1, and their items
        // are added to the lists).
        let req = AllocRequest {
            ty: UnitType::Item,
            class: idx as u32,
            room: spawn.room,
            add: spawn.init_flags & 1 != 0,
            fixed_guid: None,
            mode: spawn.mode,
            allied: false,
        };
        let mut probe = self.fields.seed;
        let unit = {
            let (mut sim, hooks) = self.split();
            lifecycle::allocate(&mut sim, hooks, &mut probe, &req)?
        }
        .ok_or(EconomyError::NotAllocated)?;
        let frame = self.game.frame as u32;
        let made = {
            let ctx = RefCell::new(StatCtx::new(self.stats, self.hooks));
            create::create_item(
                t,
                self.fields,
                rq,
                use_seed,
                UnitStats::new(&ctx, unit),
                frame,
            )
            .map(|c| (swap_stats(c.item, ()).0, c.event3_at))
        };
        let (item, event3_at) = match made {
            Ok(m) if probe == self.fields.seed => m,
            Ok(_) => {
                self.remove_unit(unit)?;
                return Err(EconomyError::SeedMismatch);
            }
            Err(e) => {
                self.remove_unit(unit)?;
                return Err(e.into());
            }
        };
        let r = self
            .units
            .get_mut(unit)
            .ok_or(EconomyError::NoRecord(unit))?;
        r.seed = item.unit_seed;
        r.init_seed = item.init_seed;
        r.item_seed = Some((item.item_seed, item.start_seed));
        self.items.items.insert(unit, item);
        if let Some(f) = event3_at {
            // TODO(units.md §6 row 3): the event's arguments are not
            // written; scheduled with 0, 0.
            self.game
                .schedule_event(unit, EVENT_REPLENISH, f as i32, None, 0, 0)?;
        }
        Ok(unit)
    }

    /// Frees an item unit (`0x00555600`, `units.md` §3.2) and its item
    /// data.
    pub fn free_item(&mut self, unit: UnitId) -> Result<(), EconomyError> {
        self.items.items.remove(&unit);
        self.remove_unit(unit)
    }

    fn remove_unit(&mut self, unit: UnitId) -> Result<(), EconomyError> {
        let (mut sim, hooks) = self.split();
        lifecycle::remove(&mut sim, hooks, unit)?;
        Ok(())
    }

    /// The unit operations' view of the same state, and the hooks.
    pub fn split(&mut self) -> (Sim<'_>, &mut H) {
        let sim = Sim {
            game: self.game,
            units: self.units,
            stats: self.stats,
            data: self.data,
        };
        (sim, self.hooks)
    }

    /// Runs `f` on the item `unit` assembled from its record (seeds), the
    /// store (item data) and the stat lists (stats); the seeds are
    /// written back afterwards.
    pub fn with_item<R>(
        &mut self,
        unit: UnitId,
        f: impl FnOnce(&mut ItemScope<'_, '_, '_>) -> R,
    ) -> Result<R, EconomyError> {
        let rec = self.units.get(unit).ok_or(EconomyError::NoRecord(unit))?;
        let (seed, init_seed, item_seed) = (rec.seed, rec.init_seed, rec.item_seed);
        let mut stored = self
            .items
            .items
            .remove(&unit)
            .ok_or(EconomyError::NotAnItem(unit))?;
        stored.unit_seed = seed;
        stored.init_seed = init_seed;
        if let Some((s, start)) = item_seed {
            stored.item_seed = s;
            stored.start_seed = start;
        }
        let (stored, out) = {
            let ctx = RefCell::new(StatCtx::new(self.stats, self.hooks));
            let (mut item, ()) = swap_stats(stored, UnitStats::new(&ctx, unit));
            let mut scope = ItemScope {
                tables: self.tables,
                game: self.fields,
                item: &mut item,
                ctx: &ctx,
            };
            let out = f(&mut scope);
            (swap_stats(item, ()).0, out)
        };
        if let Some(r) = self.units.get_mut(unit) {
            r.seed = stored.unit_seed;
            r.init_seed = stored.init_seed;
            r.item_seed = Some((stored.item_seed, stored.start_seed));
        }
        self.items.items.insert(unit, stored);
        Ok(out)
    }

    /// Runs `f` with stat handles on the lists (no item assembled).
    pub fn with_stats<R>(&mut self, f: impl FnOnce(&RefCell<StatCtx<'_>>) -> R) -> R {
        let ctx = RefCell::new(StatCtx::new(self.stats, self.hooks));
        f(&ctx)
    }

    /// The request unit (`generation.md` Inputs, offset 0x00) from the
    /// unit record (class) and stats (stat 12). Player data the d2-sim
    /// units do not hold (name, the client's hardcore flag) comes from
    /// the caller; `player` = `None` means the unit has no player data.
    pub fn request_unit(
        &self,
        unit: UnitId,
        player: Option<([u8; 16], Option<bool>)>,
    ) -> Option<RequestUnit> {
        let r = self.units.get(unit)?;
        Some(RequestUnit {
            class: r.class as i32,
            player: player.map(|(name, hardcore)| PlayerInfo {
                name,
                level: self.stats.unit_total(unit, STAT_LEVEL, 0),
                hardcore,
            }),
        })
    }
}
