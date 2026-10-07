// Spec: specs/items/treasure.md §9, §9.1, §7 (the quest drop helper `0x00559A30` on the action wiring)
//! The quest drop helper `0x00559A30` (`treasure.md` §9) on the action
//! wiring's units, stats and DRLG, as the chest drop of
//! [`super::object_chest_drop`]: the unit `U` is the dropper (its unit
//! seed draws the class pick; item level from its kind, §9 rule 2), the
//! item a real item unit at the floor drop of `U`'s room and position
//! ([`super::ItemDrops`]). The game's drop state ([`DeathDrops`]) holds
//! the item tables; the `levels` rows are the action tables'.
//!
//! Callers: the quests' `drop_item_at` / `quest_drop`
//! ([`super::HostQuests`]) and the umod callback's drop
//! (`monsters/umod-callbacks.md` §15.2, the world host).

use super::death::Spots;
use super::{dropper, DeathDrops, Economy, FreeSpot, GameFields, ItemDrops};
use crate::treasure::drop::area_level;
use crate::treasure::quest_drop::{quest_drop, PickData, QuestDropArgs};
use crate::units::hooks::Sim;
use crate::units::{UnitId, UnitType};
use crate::wiring::action::{ActionHooks, Pending};

/// `0x00559A30(game, unit, quality, &level, &request, p6, p7)` with the
/// unit's drop code (+0xB8) `drop_code`: the created item, or none. The
/// item is also appended to [`DeathDrops::placed`]; a fatal assert of the
/// helper goes to [`DeathDrops::errors`].
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
    let rec = sim.units.get(unit)?;
    let mut seed = rec.seed;
    let monster = rec.ty == UnitType::Monster;
    let room = sim.game.lists.unit(unit).and_then(|e| e.room());
    let (x, y) = h.path_position(unit);
    let level = room
        .and_then(|rm| h.drlg.level_id(sim.game, rm))
        .map_or(0, |l| i32::try_from(l).unwrap_or(i32::MAX));
    let t = d.tables.clone();
    let mut fields = GameFields::from_action(
        h.game_seed,
        &h.ai_info,
        sim.data.expansion,
        std::mem::take(&mut h.uniques),
    );
    let area = area_level(&h.tables.levels, level, fields.difficulty, fields.expansion);
    let drop_by = dropper(sim.units, sim.stats, Some(unit), area, x, y);
    let args = QuestDropArgs {
        quality,
        drop_code,
        p6,
        p7,
        item_format: fields.treasure_facts(0, 0).item_format,
    };
    // §7 rule 2 (the floor drop): the start offset when a room exists there.
    let start = match room.and_then(|rm| h.drlg.find_room(sim.game, rm, x + 2, y + 3)) {
        Some(_) => (x + 2, y + 3),
        None => (x, y),
    };
    let mut items = std::mem::take(&mut h.items);
    let (out, placed, failures) = {
        let mut econ = Economy {
            game: &mut *sim.game,
            units: &mut *sim.units,
            stats: &mut *sim.stats,
            data: sim.data,
            hooks: &mut *h,
            fields: &mut fields,
            tables: &t.items,
            items: &mut items,
        };
        let mut sink = ItemDrops::new(
            &mut econ,
            Spots {
                inner: spots,
                room,
                start,
            },
        );
        let pick = PickData::of(&t.items, sink.econ.fields.expansion);
        let out = quest_drop(&pick, &drop_by, monster, &mut seed, &args, &mut sink);
        (out, sink.placed, sink.failures)
    };
    h.items = items;
    h.game_seed = fields.seed;
    h.uniques = std::mem::take(&mut fields.uniques);
    d.fields = fields;
    if let Some(r) = sim.units.get_mut(unit) {
        r.seed = seed;
    }
    d.placed.extend(placed);
    d.failures.extend(failures);
    match out {
        Ok(o) => o.item,
        Err(e) => {
            d.errors.push(e);
            None
        }
    }
}
