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
//! (`0x00463740`), item creation and allocation on the game seed. Seams
//! ([`Pending`]): position, superunique index, champion / unique flags,
//! minion owner, party, the quest TC test; [`FreeSpot`]: the free-spot
//! search `0x0064E810` (collision spec, `treasure.md` OQ 8).

use std::sync::Arc;

use d2_data::tables::Superuniques;

use super::{
    dropper, recipient, DropPlacer, DropSpot, Economy, EconomyError, GameFields, ItemDrops,
    ItemStore,
};
use crate::items::ItemTables;
use crate::treasure::drop::{monster_drop, monster_drop_gate, MonsterDrop, MonsterRank};
use crate::treasure::{ItemData, TreasureClasses, TreasureData, TreasureError};
use crate::units::hooks::Sim;
use crate::units::{RoomId, UnitId, UnitType};
use crate::wiring::action::{ActionHooks, Pending};

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

/// A game's drop state: tables, game-creation fields, the item store,
/// the host's player counts (§5.4) and what the drops left.
#[derive(Debug)]
pub struct DeathDrops {
    pub tables: Arc<DropTables>,
    /// The unique bits (+0x1B24) the drops read and set. The game seed
    /// and the creation fields of a drop are the action wiring's
    /// (`ActionHooks::game_seed`, `ActionHooks::ai_info`,
    /// `UnitData::expansion`, [`GameFields::from_action`]); only
    /// `uniques` is read here, and the drop's fields are written back
    /// (the seed to the action wiring).
    pub fields: GameFields,
    pub items: ItemStore,
    /// Living players and the `players` setting (`treasure.md` Inputs).
    pub living_players: i32,
    pub players_setting: i32,
    /// Created items with their spots, in creation order.
    pub placed: Vec<(UnitId, DropSpot)>,
    /// Item creations that returned no item.
    pub failures: Vec<EconomyError>,
    /// The walk's fatal errors.
    pub errors: Vec<TreasureError>,
}

impl DeathDrops {
    pub fn new(tables: Arc<DropTables>, fields: GameFields) -> Self {
        Self {
            tables,
            fields,
            items: ItemStore::new(),
            living_players: 1,
            players_setting: 0,
            placed: Vec::new(),
            failures: Vec::new(),
            errors: Vec::new(),
        }
    }
}

/// Seam: the free-spot search `0x0064E810`(room, start, origin, 1,
/// 0x3E01, 0x801, 1) of §7 step 2 (collision spec, not written). Items
/// of one walk are placed one after another.
pub trait FreeSpot {
    fn free_spot(
        &mut self,
        room: Option<RoomId>,
        start: (i32, i32),
        origin: (i32, i32),
    ) -> Option<DropSpot>;
}

/// §7 step 2 with the start offset done by the caller.
struct Spots<'s, F> {
    inner: &'s mut F,
    room: Option<RoomId>,
    start: (i32, i32),
}

impl<F: FreeSpot> DropPlacer for Spots<'_, F> {
    fn place(&mut self, x: i32, y: i32) -> Option<DropSpot> {
        self.inner.free_spot(self.room, self.start, (x, y))
    }
}

/// The gate (§3.1) and the drop (§3.2–§3.5) of `unit`, killed with
/// death target `target` (`R`). Returns the created items (also
/// appended to [`DeathDrops::placed`]).
///
/// TODO(treasure.md §3.1): the collision word at a position outside
/// every room grid is read as 0. TODO(treasure.md §7 step 2): the free
/// spot search gets the room the start-offset search found, else the
/// monster's room.
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
    let (x, y) = h.x.position(unit);
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
        d.fields.uniques.clone(),
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
    let (out, placed, failures) = {
        let mut econ = Economy {
            game: &mut *sim.game,
            units: &mut *sim.units,
            stats: &mut *sim.stats,
            data: sim.data,
            hooks: &mut *h,
            fields: &mut fields,
            tables: &t.items,
            items: &mut d.items,
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
    h.game_seed = fields.seed;
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
