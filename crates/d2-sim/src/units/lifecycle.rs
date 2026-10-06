// Spec: specs/sim/units.md §3; specs/sim/rng.md §5.3; specs/sim/stat-lists.md §4.3
//! Unit allocation `0x00555230` (§3.1) and removal `0x00555600` (§3.2).
//! The list bookkeeping is [`crate::game::Game`]'s
//! (`sim/unit-order.md`); the per-kind init and free routines beyond the
//! stat list and the missile setup belong to the kind specs and run
//! through [`LifecycleHooks`].

use crate::game::GameError;
use crate::rng::Seed;
use crate::stats::ValueCallback;

use super::hooks::{Sim, UnitHooks};
use super::modes::{self, UnitError};
use super::record::{flags, flags2, UnitRecord};
use super::{RoomId, UnitId, UnitType};

/// Player classes (`0x00555230` step 1: class < 7).
pub const PLAYER_CLASSES: u32 = 7;

/// The allocation arguments (ECX type, EDX class; x, y, game, room,
/// flags, mode, fixed GUID). Positions belong to the path spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocRequest {
    pub ty: UnitType,
    pub class: u32,
    pub room: Option<RoomId>,
    /// Flags bit 1: `SUNIT_Add`.
    pub add: bool,
    /// Flags bit 2 on a monster: take [`Self::fixed_guid`].
    pub fixed_guid: Option<u32>,
    pub mode: u32,
    /// Counts toward the room's allied count (`unit-order.md` §5.2).
    pub allied: bool,
}

/// The per-kind init and free routines other specs own (§1 table).
#[allow(unused_variables)]
pub trait LifecycleHooks: UnitHooks {
    /// The rest of the per-kind init after d2rs's part (players:
    /// `0x005B1880` unless mode 17; monsters `0x00574250`; objects: data,
    /// `0x00623520`, `0x0054F5D0`; items `0x00623520`; tiles
    /// `0x00623520`), then the path settings of step 8. Provider: the
    /// kind's spec.
    fn init_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId, req: &AllocRequest) {}

    /// The free routine's other calls (§1 table) and `0x005C0A90`,
    /// `0x00571F40` at removal. Provider: the kind's spec.
    fn free_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}
}

/// `0x00555230` (§3.1). Returns `None` when the class is rejected
/// (nothing allocated, no RNG draw). `game_seed` is the game seed of
/// `rng.md` §5.3.
pub fn allocate<H: LifecycleHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    game_seed: &mut Seed,
    req: &AllocRequest,
) -> Result<Option<UnitId>, UnitError> {
    // Step 1.
    match req.ty {
        UnitType::Monster if !sim.data.monster(req.class).is_some_and(|m| m.enabled) => {
            return Ok(None)
        }
        UnitType::Player if req.class >= PLAYER_CLASSES => return Ok(None),
        _ => {}
    }
    if !req.add {
        return Err(UnitError::NotAdded);
    }
    // Step 3: the act of the allocation room's level.
    let act = req
        .room
        .and_then(|r| sim.game.lists.room(r))
        .map_or(0, |r| r.act);
    // Step 4 (rng.md §5.3).
    let mut seed = Seed::init();
    let mut init_seed = 0;
    let mut item_seed = None;
    if req.ty != UnitType::Player {
        init_seed = game_seed.step();
        seed = Seed::init_low(init_seed);
    }
    if req.ty == UnitType::Item {
        let start = game_seed.step();
        item_seed = Some((Seed::init_low(start), start));
    }
    // Step 6.
    let guid = match (req.ty, req.fixed_guid) {
        (UnitType::Monster, Some(g)) => g,
        _ => sim.game.lists.guids.alloc(req.ty),
    };
    // Step 8 (SUNIT_Add) precedes the per-kind init here: d2rs needs the
    // list entry to own timers; neither step reads what the other writes.
    let unit = sim
        .game
        .lists
        .add_unit(req.ty, guid, req.room, req.allied)
        .map_err(GameError::from)?;
    // Steps 2 and 5.
    let mut rec = UnitRecord::new(req.ty, req.class, guid);
    rec.act = act;
    rec.seed = seed;
    rec.init_seed = init_seed;
    rec.item_seed = item_seed;
    rec.flags |= flags::SEED_SET;
    if req.ty == UnitType::Tile {
        rec.flags |= flags::TILE;
    }
    rec.flags2 |= flags2::SERVER;
    if sim.data.expansion {
        rec.flags2 |= flags2::EXPANSION;
    }
    // Step 7 (§1 table): mode := argument for monsters, objects, missiles
    // and items; the tile init (`0x00623520`, flags |= 0x2) sets no mode
    // (units.md §3.1 r7).
    if !matches!(req.ty, UnitType::Player | UnitType::Tile) {
        rec.mode = req.mode;
    }
    sim.units.insert(unit, rec);
    // Step 7: the stat lists (stat-lists.md §4.3) and the missile setup.
    let callback = match req.ty {
        UnitType::Player | UnitType::Monster => Some(Some(ValueCallback::Server)),
        UnitType::Item | UnitType::Missile => Some(None),
        _ => None,
    };
    if let Some(cb) = callback {
        let l = sim
            .stats
            .alloc_extended(hooks, unit, req.ty, guid, req.class, 0, cb);
        if let Some(r) = sim.units.get_mut(unit) {
            r.stats = Some(l);
        }
    }
    if req.ty == UnitType::Missile {
        modes::missile_init(sim, unit)?;
    }
    hooks.init_kind(sim, unit, req);
    Ok(Some(unit))
}

/// `0x00555600` (§3.2): list unlinks and timer cancels
/// ([`crate::game::Game::remove_unit`]), the kind's free routine, the
/// stat list, the record. Immediate.
pub fn remove<H: LifecycleHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    sim.game.remove_unit(unit)?;
    hooks.free_kind(sim, unit);
    sim.stats.free_unit_list(hooks, unit);
    sim.units.remove(unit);
    Ok(())
}
