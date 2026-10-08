// Spec: specs/items/treasure.md §3.1–§3.5, §5, §7; specs/sim/units.md §4.6; specs/sim/rng.md §5.3
//! A dead monster's drop: the gate `0x005A6830` and the monster drop
//! (`treasure.md` §3.1–§3.5) run on the action wiring's units, stats and
//! DRLG, with the walk's items created as real item units through
//! [`ItemDrops`] and placed into their rooms (§7).
//!
//! The death start `0x005A6FF0` that calls the gate has no written body
//! (`units.md` §4.6 names it; `treasure.md` §3.1 names the call), so the
//! action wiring hands it to [`Pending::monster_death_start`]; a host
//! that holds the drop state ([`DeathDrops`]) calls
//! [`monster_death_drop`] from there.
//!
//! Real here: the unit flags and the collision word of the gate, the
//! monster's monstats row, the dropper and recipient stats, the walk on
//! the monster's unit seed, the start offset's room search
//! (`0x00463740`), item creation and allocation on the game seed, into
//! the game's one item store (`ActionHooks::items`, the store every
//! other item system reads: a dropped item can be picked up, sold and
//! cubed). Seams
//! ([`Pending`]): superunique index, champion / unique flags, minion
//! owner, party, the quest TC test. Position: the path record's
//! ([`ActionHooks::path_position`]; [`Pending::position`] without the
//! path provider). The free-spot search `0x0064E810`: with the path
//! provider on and its walk-back field loaded
//! ([`crate::wiring::path::PathState::field`]), the floor drop
//! [`crate::wiring::path::place::floor_drop`] (`path-placement.md` §7,
//! §9) on the game's rooms, and the created item gets its static path
//! and footprint (§2.5) so the next item of the walk sees it; otherwise
//! the [`FreeSpot`] seam, as before the provider.

use std::sync::Arc;

use d2_data::tables::Superuniques;

use super::{
    dropper, recipient, DropPlacer, DropSpot, Economy, EconomyError, GameFields, ItemDrops,
};
use crate::items::ItemTables;
use crate::path::coords::Point;
use crate::treasure::class_pick::{ClassPicks, PickError};
use crate::treasure::drop::{monster_drop, monster_drop_gate, MonsterDrop, MonsterRank};
use crate::treasure::{ItemData, TreasureClasses, TreasureData, TreasureError};
use crate::units::hooks::Sim;
use crate::units::{RoomId, UnitId, UnitType};
use crate::wiring::action::{ActionHooks, Pending, View, WiringError};
use crate::wiring::path::place::floor_drop;

/// The collision mask of the gate (§3.1, `0x0064CB30`).
const GATE_MASK: u16 = 0x801;
/// Type flags of `0x005A0180` (§3.2).
const FLAG_CHAMPION: u32 = 4;
const FLAG_UNIQUE: u32 = 8;

/// The tables a drop reads beyond the action tables (`treasure.md`
/// Inputs).
#[derive(Debug, Clone)]
pub struct DropTables {
    pub items: ItemTables,
    pub tcs: TreasureClasses,
    /// [`crate::treasure::item_list`] order (the items' index order).
    pub treasure_items: Vec<ItemData>,
    pub superuniques: Vec<Superuniques>,
}

/// A game's drop state: tables, game-creation fields, the host's player
/// counts (§5.4) and what the drops left. The items go to the game's one
/// item store (`ActionHooks::items`).
#[derive(Debug)]
pub struct DeathDrops {
    pub tables: Arc<DropTables>,
    /// The fields of the last drop, written back after it. Nothing here
    /// is read: the game seed, the creation fields and the unique bits
    /// (+0x1B24) of a drop are the action wiring's
    /// (`ActionHooks::game_seed`, `ActionHooks::ai_info`,
    /// `UnitData::expansion`, `ActionHooks::uniques`,
    /// [`GameFields::from_action`]), and the seed and the unique bits
    /// go back there (one unique-bit store per game, shared with the
    /// host's economy); `uniques` here is left empty.
    pub fields: GameFields,
    /// Living players and the `players` setting (`treasure.md` Inputs).
    pub living_players: i32,
    pub players_setting: i32,
    /// Created items with their spots, in creation order.
    pub placed: Vec<(UnitId, DropSpot)>,
    /// Item creations that returned no item.
    pub failures: Vec<EconomyError>,
    /// The walk's fatal errors.
    pub errors: Vec<TreasureError>,
    /// The item class picks of the drop helpers
    /// ([`super::drop_helpers`], `objects-2.md` §20): the combined items
    /// array's pick columns. Empty by default (every pick finds no
    /// candidate, so the helpers create nothing).
    pub picks: Arc<ClassPicks>,
    /// The drop helpers' pick fatals (`objects-2.md` §20.4, §20.6).
    pub pick_errors: Vec<PickError>,
}

impl DeathDrops {
    pub fn new(tables: Arc<DropTables>, fields: GameFields) -> Self {
        Self {
            tables,
            fields,
            living_players: 1,
            players_setting: 0,
            placed: Vec::new(),
            failures: Vec::new(),
            errors: Vec::new(),
            picks: Arc::default(),
            pick_errors: Vec::new(),
        }
    }

    /// With the drop helpers' pick rows.
    pub fn with_picks(mut self, picks: Arc<ClassPicks>) -> Self {
        self.picks = picks;
        self
    }
}

/// Seam: the free-spot search `0x0064E810`(room, start, origin, 1,
/// 0x3E01, 0x801, 1) of §7 step 2, used when the path provider or its
/// walk-back field is off. Items of one walk are placed one after
/// another.
pub trait FreeSpot {
    fn free_spot(
        &mut self,
        room: Option<RoomId>,
        start: (i32, i32),
        origin: (i32, i32),
    ) -> Option<DropSpot>;
}

/// §7 step 2 with the start offset done by the caller.
pub(super) struct Spots<'s, F> {
    pub(super) inner: &'s mut F,
    pub(super) room: Option<RoomId>,
    pub(super) start: (i32, i32),
}

/// Item size of the floor drop (§7 step 2, `path-placement.md` §9).
const DROP_SIZE: i32 = 1;

impl<X: Pending, F: FreeSpot> DropPlacer<ActionHooks<X>> for Spots<'_, F> {
    /// With the provider: `0x0064E810` through the floor drop
    /// (`path-placement.md` §9: its rule 1 is §7 step 2's start offset,
    /// the same room lookup), size 1, fallback 1. Without: [`FreeSpot`].
    fn place(
        &mut self,
        econ: &mut Economy<'_, ActionHooks<X>>,
        x: i32,
        y: i32,
    ) -> Option<DropSpot> {
        let h = &mut *econ.hooks;
        let Some(field) = h.paths.as_ref().and_then(|p| p.field.clone()) else {
            return self.inner.free_spot(self.room, self.start, (x, y));
        };
        match floor_drop(
            &h.drlg,
            &field,
            self.room,
            Point::new(x, y),
            DROP_SIZE,
            true,
        ) {
            Ok((Some(room), p)) => Some(DropSpot {
                room: Some(room),
                x: p.x,
                y: p.y,
            }),
            Ok((None, _)) => None,
            Err(e) => {
                h.errors.push(WiringError::Place(e));
                None
            }
        }
    }

    /// With the path records: the path part of `SUNIT_Add` (§2.5) for
    /// the item at its spot (mode 3: static path and footprint, mask
    /// 0x200, in 0x3E01), whether or not the walk-back field chose the
    /// spot (without it the spot is [`FreeSpot`]'s): the item's position
    /// is its path's (`bitstream.md` §4.1 r2, the 0x9C ground x / y).
    /// Without path records: nothing (as before the provider).
    fn placed(&mut self, econ: &mut Economy<'_, ActionHooks<X>>, item: UnitId, spot: DropSpot) {
        if econ.hooks.paths.is_none() {
            return;
        }
        let game: &crate::game::Game = econ.game;
        View::of(econ.units, econ.stats, econ.data, econ.hooks)
            .path_place(game, item, spot.x, spot.y);
    }
}

/// The gate (§3.1) and the drop (§3.2–§3.5) of `unit`, killed with
/// death target `target` (`R`). Returns the created items (also
/// appended to [`DeathDrops::placed`]).
///
/// TODO(treasure.md §3.1): the collision word at a position outside
/// every room grid is read as 0. TODO(treasure.md §7 step 2): the free
/// spot search gets the room the start-offset search found, else the
/// monster's room (with the provider, the floor drop's rule 1 finds the
/// same start from that room: both lookups are the room and its
/// adjacent rooms).
pub fn monster_death_drop<X: Pending, F: FreeSpot>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    d: &mut DeathDrops,
    spots: &mut F,
    unit: UnitId,
    target: Option<UnitId>,
) -> Vec<UnitId> {
    let Some(r) = sim.units.get(unit) else {
        return Vec::new();
    };
    let (ty, class, flags, mut seed) = (r.ty, r.class, r.flags, r.seed);
    if ty != UnitType::Monster {
        return Vec::new();
    }
    let (x, y) = h.path_position(unit);
    let room = sim.game.lists.unit(unit).and_then(|e| e.room());
    let collision = room
        .and_then(|rm| h.drlg.collision(sim.game, rm, x, y))
        .unwrap_or(0);
    match monster_drop_gate(flags, u32::from(collision & GATE_MASK), class) {
        Ok(true) => {}
        Ok(false) => return Vec::new(),
        Err(e) => {
            d.errors.push(e);
            return Vec::new();
        }
    }
    let at = h.tables.clone();
    let Some(m) = at.combat.monstats.get(class as usize) else {
        return Vec::new();
    };
    let t = d.tables.clone();
    let rank = match h.x.superunique(unit) {
        Some(i) => MonsterRank::Superunique(t.superuniques.get(usize::from(i))),
        None if h.x.monster_flag(unit, FLAG_CHAMPION) => MonsterRank::Champion,
        None if h.x.monster_flag(unit, FLAG_UNIQUE) => MonsterRank::Unique,
        None => MonsterRank::Normal,
    };
    let drop_by = dropper(sim.units, sim.stats, Some(unit), 0, x, y);
    let rec = target.map(|r| {
        recipient(
            sim.units,
            sim.stats,
            r,
            h.x.minion_owner(r),
            h.x.party_size(r),
        )
    });
    // §3.3's test reads quest state only; it is asked up front (it is a
    // query) and handed to the TC choice, which uses it only when its
    // other conditions hold.
    let quest_open = match target {
        Some(r) if m.tcquestid != 0 => h.x.quest_tc_open(r, m.tcquestcp),
        _ => false,
    };
    let start_room = room.and_then(|rm| h.drlg.find_room(sim.game, rm, x + 2, y + 3));
    let (spot_room, start) = match start_room {
        Some(rm) => (Some(rm), (x + 2, y + 3)),
        None => (room, (x, y)),
    };
    let mut fields = GameFields::from_action(
        h.game_seed,
        &h.ai_info,
        sim.data.expansion,
        std::mem::take(&mut h.uniques),
    );
    let facts = fields.treasure_facts(d.living_players, d.players_setting);
    let data = TreasureData {
        tcs: &t.tcs,
        items: &t.treasure_items,
        itemtypes: &t.items.itemtypes,
        equiv: &t.items.equiv,
        itemratio: &t.items.itemratio,
    };
    let md = MonsterDrop {
        monstats: m,
        rank,
        find_item: false,
    };
    // The game's one item store, lent out of the hooks for the drop.
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
                room: spot_room,
                start,
            },
        );
        let out = monster_drop(
            &data,
            &facts,
            &md,
            &drop_by,
            &mut seed,
            rec.as_ref(),
            |_| quest_open,
            &mut sink,
        );
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
        Ok(v) => v,
        Err(e) => {
            d.errors.push(e);
            Vec::new()
        }
    }
}
