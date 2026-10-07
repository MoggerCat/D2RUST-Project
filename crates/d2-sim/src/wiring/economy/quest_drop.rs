// Spec: specs/items/treasure.md §9, §9.1, §7 (the quest drop helper `0x00559A30` on the action wiring)
//! The quest drop helper `0x00559A30` (`treasure.md` §9) on the action
//! wiring's units, stats and DRLG, as the chest drop of
//! [`super::object_chest_drop`]: the unit `U` is the dropper (its unit
//! seed draws the class pick; item level from its kind, §9 rule 2), the
//! item a real item unit at the floor drop of `U`'s room and position
//! ([`super::ItemDrops`]). The game's drop state ([`DeathDrops`]) holds
//! the item tables; the `levels` rows are the action tables'.
//!
//! The drop itself is [`drop_helpers::source_drop`] (one implementation
//! of `0x00559A30` for the objects, quests and umod callbacks).
//!
//! Callers: the quests' `drop_item_at` / `quest_drop`
//! ([`super::HostQuests`]) and the umod callback's drop
//! (`monsters/umod-callbacks.md` §15.2, the world host).

use std::sync::Arc;

use super::{drop_helpers, DeathDrops, FreeSpot};
use crate::treasure::class_pick::ClassPicks;
use crate::units::hooks::Sim;
use crate::units::UnitId;
use crate::wiring::action::{ActionHooks, Pending};

/// `0x00559A30(game, unit, quality, &level, &request, p6, p7)` with the
/// unit's drop code (+0xB8) `drop_code`: the created item, or none. One
/// implementation: [`drop_helpers::source_drop`] (`objects-2.md` §20.4,
/// the same function), with the action tables' `levels`. The item is also
/// appended to [`DeathDrops::placed`]; a pick fatal goes to
/// [`DeathDrops::pick_errors`]. A drop state without pick rows
/// ([`DeathDrops::picks`] empty) picks from its item tables
/// ([`ClassPicks::of_items`], the same combined array).
#[allow(clippy::too_many_arguments)]
pub fn unit_quest_drop<X: Pending, F: FreeSpot>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    d: &mut DeathDrops,
    spots: &mut F,
    unit: UnitId,
    drop_code: Option<[u8; 4]>,
    quality: u8,
    p6: i32,
    p7: i32,
) -> Option<UnitId> {
    sim.units.get(unit)?;
    let saved = d.picks.rows.is_empty().then(|| {
        let t = &d.tables.items;
        std::mem::replace(
            &mut d.picks,
            Arc::new(ClassPicks::of_items(&t.items, t.parts)),
        )
    });
    let levels = h.tables.clone();
    let code = drop_code.map_or(0, u32::from_le_bytes);
    let (item, _) = drop_helpers::source_drop(
        h,
        sim,
        d,
        &levels.levels,
        spots,
        unit,
        code,
        quality,
        p6,
        p7 != 0,
    );
    if let Some(p) = saved {
        d.picks = p;
    }
    item
}
