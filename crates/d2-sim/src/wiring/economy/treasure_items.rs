// Spec: specs/items/treasure.md §5.4, §6, §7, §8; specs/items/generation.md Inputs
//! [`DropSink`] on item creation ([`ItemDrops`]), and the walk's dropper
//! and recipient read from unit records and stat lists.

use super::{Economy, EconomyError, ItemSpawn};
use crate::items::ItemRequest;
use crate::stats::StatLists;
use crate::treasure::{DropRequest, DropSink, Dropper, DropperKind, Recipient};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::record::Units;
use crate::units::{RoomId, UnitId, UnitType};

/// Stat ids the walk reads.
const STAT_LEVEL: u16 = 12;
const STAT_GOLD: u16 = 14;
const STAT_GOLDBONUS: u16 = 79;
const STAT_MAGICBONUS: u16 = 80;
const STAT_PLAYERCOUNT: u16 = 100;

/// A drop's room and position (§7 step 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropSpot {
    pub room: Option<RoomId>,
    pub x: i32,
    pub y: i32,
}

/// Seam: §7 step 2, the start offset and the free-spot search
/// `0x0064E810` (`sim/path-placement.md` §7, §9). It receives the
/// economy the walk runs on, so a provider reaches the game's rooms and
/// collision through its hooks (`H`).
pub trait DropPlacer<H> {
    /// The spot of the next item dropped by a unit at (`x`, `y`).
    fn place(&mut self, econ: &mut Economy<'_, H>, x: i32, y: i32) -> Option<DropSpot>;

    /// The item `item` was created at `spot` (§7 step 4): the provider
    /// puts it on the floor there, so the next search sees it (§7 step
    /// 2, "each seeing the previous ones"). Default: nothing.
    fn placed(&mut self, _econ: &mut Economy<'_, H>, _item: UnitId, _spot: DropSpot) {}
}

/// The walk's [`DropSink`]: each request becomes a real item unit.
pub struct ItemDrops<'e, 'a, H, P> {
    pub econ: &'e mut Economy<'a, H>,
    pub placer: P,
    /// Created items with their spots, in creation order (positions
    /// belong to the path spec and are not stored on the unit).
    pub placed: Vec<(UnitId, DropSpot)>,
    /// Creations that returned no item, with the reason.
    pub failures: Vec<EconomyError>,
}

impl<'e, 'a, H, P> ItemDrops<'e, 'a, H, P> {
    pub fn new(econ: &'e mut Economy<'a, H>, placer: P) -> Self {
        Self {
            econ,
            placer,
            placed: Vec::new(),
            failures: Vec::new(),
        }
    }
}

/// The drop request of §7 step 4 as an items request
/// (`generation.md` Inputs).
///
/// TODO(treasure.md §7 step 4): the request's source unit (offset 0x00)
/// is not listed for drops; left none.
pub fn drop_request(req: &DropRequest<DropSpot>) -> (ItemRequest, ItemSpawn) {
    let rq = ItemRequest {
        ilvl: req.item_level,
        item: i32::from(req.id),
        format: req.item_format as u16,
        quality: req.quality,
        index: req.index,
        flags2: u32::from(req.drop_flags),
        ..ItemRequest::default()
    };
    let spawn = ItemSpawn {
        room: req.spot.room,
        mode: u32::from(req.spawn_type),
        init_flags: req.init_flags,
    };
    (rq, spawn)
}

impl<H: LifecycleHooks, P: DropPlacer<H>> DropSink for ItemDrops<'_, '_, H, P> {
    type Spot = DropSpot;
    type Item = UnitId;

    fn place(&mut self, x: i32, y: i32) -> Option<DropSpot> {
        self.placer.place(self.econ, x, y)
    }

    fn create(&mut self, req: DropRequest<DropSpot>) -> Option<UnitId> {
        let (mut rq, spawn) = drop_request(&req);
        match self.econ.create_item(&mut rq, false, spawn) {
            Ok(u) => {
                // PROVISIONAL (REC-281, d2rs-own, unverified): a low or
                // normal quality drop is identified (`generation.md` §1.4:
                // the flag is "set by callers"; no quality success
                // cleared it); magic and better stay unidentified.
                if let Some(i) = self.econ.items.get_mut(u) {
                    if matches!(i.quality, crate::items::q::LOW | crate::items::q::NORMAL) {
                        i.flags |= crate::items::flag::IDENTIFIED;
                    }
                }
                self.placer.placed(self.econ, u, req.spot);
                self.placed.push((u, req.spot));
                Some(u)
            }
            Err(e) => {
                self.failures.push(e);
                None
            }
        }
    }

    /// Stat 14 as stored (base; the setter `0x00530EA0` writes the base).
    fn gold(&self, item: UnitId) -> i32 {
        self.econ.stats.unit_base(item, STAT_GOLD, 0)
    }

    fn set_gold(&mut self, item: UnitId, value: i32) {
        self.econ
            .stats
            .unit_set(self.econ.hooks, item, STAT_GOLD, value, 0);
    }
}

/// The dropping unit `U` (§5.4 step 4, §7 step 3) from its record and
/// stats: a monster's class, `level` (12) and `monster_playercount`
/// (100); a player's base `level`; other units take `area_level` (§4
/// `a`, from the caller: levels are DRLG's). Position: the path spec's,
/// from the caller.
pub fn dropper(
    units: &Units,
    stats: &StatLists,
    unit: Option<UnitId>,
    area_level: i32,
    x: i32,
    y: i32,
) -> Dropper {
    let kind = match unit.and_then(|u| units.get(u).map(|r| (u, r))) {
        None => DropperKind::None,
        Some((u, r)) => match r.ty {
            UnitType::Monster => DropperKind::Monster {
                class: r.class,
                level: stats.unit_total(u, STAT_LEVEL, 0),
                playercount: stats.unit_total(u, STAT_PLAYERCOUNT, 0),
            },
            UnitType::Player => DropperKind::Player {
                level: stats.unit_base(u, STAT_LEVEL, 0),
            },
            _ => DropperKind::Other { area_level },
        },
    };
    Dropper { kind, x, y }
}

/// The recipient `R` (§5.4 step 1, §6 step 4, §8 step 3) from stats:
/// `M` = stat 80 of `R` plus its minion owner's when `R` is a player or
/// monster, else 0; gold find = stat 79 of `R` plus its owner's. The
/// minion owner (`0x0058F0D0`) and the party count (`0x005408E0`) are
/// the caller's (units / party: not written).
pub fn recipient(
    units: &Units,
    stats: &StatLists,
    r: UnitId,
    owner: Option<UnitId>,
    party: Option<i32>,
) -> Recipient {
    let sum = |s: u16| {
        let own = stats.unit_total(r, s, 0);
        own.wrapping_add(owner.map_or(0, |o| stats.unit_total(o, s, 0)))
    };
    let fighter = units
        .get(r)
        .is_some_and(|x| matches!(x.ty, UnitType::Player | UnitType::Monster));
    Recipient {
        party,
        magic_find: if fighter { sum(STAT_MAGICBONUS) } else { 0 },
        gold_find: sum(STAT_GOLDBONUS),
    }
}
