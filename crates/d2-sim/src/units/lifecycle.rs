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
use super::{ListError, RoomId, UnitId, UnitType};

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

    /// Step 8's list part ([`add`]) linked `unit`: its per-kind init is
    /// over. Provider: whoever tracks units between steps 7 and 8.
    fn added(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// The free routine's other calls (§1 table) and `0x005C0A90`,
    /// `0x00571F40` at removal. Provider: the kind's spec.
    fn free_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// The monster of `class` allocated beside `near` in its room
    /// (`npc.md` §7.3 step 7, `hirelings.md` §3.1: the mercenary's
    /// creation). Provider: the host that owns the path code; `None`
    /// (the default) leaves the creation to the NPC rest.
    fn spawn_near(
        &mut self,
        sim: &mut Sim<'_>,
        near: UnitId,
        class: u32,
        mode: u8,
    ) -> Option<UnitId> {
        None
    }
}

/// `0x00555230` (§3.1): [`allocate_unlinked`] (steps 1–7), then
/// [`add`] (step 8's `SUNIT_Add` list part). Returns `None` when the
/// class is rejected (nothing allocated, no RNG draw). `game_seed` is
/// the game seed of `rng.md` §5.3. An error (unknown room, duplicate
/// GUID) leaves the game, the seed and the GUID counter as they were.
pub fn allocate<H: LifecycleHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    game_seed: &mut Seed,
    req: &AllocRequest,
) -> Result<Option<UnitId>, UnitError> {
    let Some(unit) = allocate_unlinked(sim, hooks, game_seed, req)? else {
        return Ok(None);
    };
    // §3.1 r9: flags bit 1 clear → no `SUNIT_Add`; the unit is returned
    // as it is after step 7, in no list.
    if req.add {
        add(sim, hooks, unit, req)?;
    }
    Ok(Some(unit))
}

/// Step 8's list part: `SUNIT_Add` `0x00554850` (`unit-order.md` §3.1)
/// of a unit [`allocate_unlinked`] returned, in the allocation's room.
/// The path settings of step 8 are the path provider's.
pub fn add<H: LifecycleHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    req: &AllocRequest,
) -> Result<(), UnitError> {
    sim.game
        .lists
        .link_unit(unit, req.room)
        .map_err(GameError::from)?;
    hooks.added(sim, unit);
    Ok(())
}

/// Steps 1–7 of `0x00555230` (§3.1): the unit with its seeds, GUID and
/// per-kind init, in no list (r7.1; [`super::UnitLists::reserve_unit`]):
/// an init never finds the unit by a hash lookup (r7.4) and reads its
/// room from the allocation's argument (r7.2). The GUID is tested free
/// here, so step 8 cannot meet the fatal duplicate (`unit-order.md`
/// §2.1) after the init ran; that error undoes the draws of steps 4
/// and 6.
pub fn allocate_unlinked<H: LifecycleHooks>(
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
    // Step 3: the act of the allocation room's level. An unknown room
    // is refused here, before any draw.
    let act = match req.room {
        Some(r) => {
            sim.game
                .lists
                .room(r)
                .ok_or(ListError::UnknownRoom(r))
                .map_err(GameError::from)?
                .act
        }
        None => 0,
    };
    // A refused `SUNIT_Add` (the fatal duplicate GUID of
    // `unit-order.md` §2.1) undoes the draws of steps 4 and 6.
    let undo = (*game_seed, sim.game.lists.guids.get(req.ty));
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
    // The entry exists from here (it owns timers) but is in no list
    // until step 8 (r7.1).
    // Without `SUNIT_Add` (flags bit 1 clear, §3.1 r9) no list is touched,
    // so a taken GUID is not met either.
    let free = if req.add {
        sim.game.lists.check_guid_free(req.ty, guid)
    } else {
        Ok(())
    };
    if let Err(e) = free {
        *game_seed = undo.0;
        sim.game.lists.guids.set(req.ty, undo.1);
        return Err(GameError::from(e).into());
    }
    let unit = sim.game.lists.reserve_unit(req.ty, guid, req.allied);
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
