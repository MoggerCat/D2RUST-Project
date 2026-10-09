// Spec: specs/items/treasure.md §4, §7; specs/world/objects.md §8 (the chest drop `D(Q)`)
//! The chest drop `0x00585B90` (`treasure.md` §4) on the action wiring's
//! units, stats and DRLG: the object `U` is the dropper (its unit seed
//! draws, `treasure.md` Randomness; item level = the area level of its
//! level, §7 rule 3), the operator `R` the recipient, the walk's items
//! real item units in the object's room ([`ItemDrops`], as the monster
//! drop of [`super::death`]).
//!
//! The drop state of a game ([`DeathDrops`]) is held by
//! [`ActionHooks::object_drops`], the `levels` rows are the object
//! tables'; the object code reaches it through
//! [`crate::world::objects::ChestWorld::chest_drop`] on the action
//! wiring's object view. `None` (the default): no drop, as before.

use d2_data::tables::Levels;

use super::death::Spots;
use super::{dropper, recipient, DeathDrops, Economy, FreeSpot, GameFields, ItemDrops};
use crate::treasure::drop::{area_level, chest_drop};
use crate::treasure::TreasureData;
use crate::units::hooks::Sim;
use crate::units::UnitId;
use crate::wiring::action::{ActionHooks, Pending};

/// `0x00585B90(op, Q)` (`treasure.md` §4) for `object` operated by
/// `operator`: the first item dropped, or none. Created items are also
/// appended to [`DeathDrops::placed`].
///
/// The act is the act of the object's room (the act of its level: a
/// level lies in one act). The free-spot search gets the object's room
/// (§7 rule 2: never the room the start lookup found); with the path
/// provider and its field it is the floor drop
/// (`path-placement.md` §9), otherwise `spots`.
#[allow(clippy::too_many_arguments)]
pub fn object_chest_drop<X: Pending, F: FreeSpot>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    d: &mut DeathDrops,
    levels: &[Levels],
    spots: &mut F,
    object: UnitId,
    operator: Option<UnitId>,
    q: u8,
) -> Option<UnitId> {
    let mut seed = sim.units.get(object)?.seed;
    let room = sim.game.lists.unit(object).and_then(|e| e.room());
    let (x, y) = h.path_position(object);
    let level = room
        .and_then(|rm| h.drlg.level_id(sim.game, rm))
        .map_or(0, |l| i32::try_from(l).unwrap_or(i32::MAX));
    let act = room
        .and_then(|rm| sim.game.lists.room(rm))
        .map_or(0, |r| r.act);
    let t = d.tables.clone();
    let mut fields = GameFields::from_action(
        h.game_seed,
        &h.ai_info,
        sim.data.expansion,
        std::mem::take(&mut h.uniques),
    );
    let facts = fields.treasure_facts(super::death::living_players(sim), d.players_setting);
    let area = area_level(levels, level, fields.difficulty, fields.expansion);
    let drop_by = dropper(sim.units, sim.stats, Some(object), area, x, y);
    let rec = operator.map(|r| {
        recipient(
            sim.units,
            sim.stats,
            r,
            h.x.minion_owner(r),
            h.x.party_size(r),
        )
    });
    // §7 rule 2: the start offset when a room exists there.
    let start_room = room.and_then(|rm| h.drlg.find_room(sim.game, rm, x + 2, y + 3));
    let start = match start_room {
        Some(_) => (x + 2, y + 3),
        None => (x, y),
    };
    let data = TreasureData {
        tcs: &t.tcs,
        items: &t.treasure_items,
        itemtypes: &t.items.itemtypes,
        equiv: &t.items.equiv,
        itemratio: &t.items.itemratio,
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
                start_room,
                start,
            },
        )
        .with_unit(object);
        let out = chest_drop(
            &data,
            &facts,
            levels,
            room.is_some(),
            act,
            level,
            q,
            &drop_by,
            &mut seed,
            rec.as_ref(),
            &mut sink,
        );
        (out, sink.placed, sink.failures)
    };
    h.items = items;
    h.game_seed = fields.seed;
    h.uniques = std::mem::take(&mut fields.uniques);
    d.fields = fields;
    if let Some(r) = sim.units.get_mut(object) {
        r.seed = seed;
    }
    d.placed.extend(placed);
    d.failures.extend(failures);
    match out {
        Ok(v) => v,
        Err(e) => {
            d.errors.push(e);
            None
        }
    }
}

/// The free-spot seam of a host without one: no spot (`treasure.md` §7
/// rule 2: no spot → no item). Used by the object view when the path
/// provider or its field is off.
pub struct NoSpot;

impl FreeSpot for NoSpot {
    fn free_spot(
        &mut self,
        _: Option<crate::units::RoomId>,
        _: (i32, i32),
        _: (i32, i32),
    ) -> Option<super::DropSpot> {
        None
    }
}

/// The free-spot seam of a host with no collision search over the floor:
/// the start spot as is (`treasure.md` §7 step 2 without the search).
///
/// PROVISIONAL (M22; REC-235, REC-108): `// d2rs-own, unverified`.
pub struct StartSpot;

impl FreeSpot for StartSpot {
    fn free_spot(
        &mut self,
        room: Option<crate::units::RoomId>,
        start: (i32, i32),
        _: (i32, i32),
    ) -> Option<super::DropSpot> {
        Some(super::DropSpot {
            room,
            x: start.0,
            y: start.1,
        })
    }
    /// The start spot in the start's room.
    fn free_spot_with_start_room(
        &mut self,
        _: Option<crate::units::RoomId>,
        start_room: Option<crate::units::RoomId>,
        start: (i32, i32),
        origin: (i32, i32),
    ) -> Option<super::DropSpot> {
        self.free_spot(start_room, start, origin)
    }
}
