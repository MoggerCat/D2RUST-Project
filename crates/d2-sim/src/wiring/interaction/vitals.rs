// Spec: specs/combat/vitals.md §1–§4; specs/sim/stats.md §4.2; specs/sim/stat-lists.md §7.2, §10.1
//! [`VitalsUnits`] on the real providers: unit records (type, class),
//! the stat lists (base and unit getters, base set and add, the maxima
//! `0x00625D10` / `0x00625D60` / `0x00625DB0`); the refresh and the
//! level-up notifications and event stay a seam ([`VitalsRest`]).
//!
//! Regeneration (timer event 3, `stat-lists.md` §10.1) already runs on
//! the same stat lists in the unit dispatch
//! ([`crate::units::dispatch::player_regen`]); its maxima are the ones
//! [`VitalsView`] reads, so a level-up's new maximum is what the next
//! regeneration tick fills up to.
//!
//! Experience on a kill: [`kill_experience`] composes what the specs
//! write (`vitals.md` §4.2 level factor on the defender's base
//! experience, §4.3 add with its cap and level-up); the kill path that
//! calls it (`damage.md` §7.2 `0x0057CCB0`) has no provider in d2-sim
//! (the action wiring's `Pending::reaction`).

use crate::combat::vitals::{add_experience, level_factor, stat, VitalsTables, VitalsUnits};
use crate::stats::{StatHost, StatLists};
use crate::units::record::Units;
use crate::units::{UnitId, UnitType};

/// The vitals calls no written spec provides yet.
pub trait VitalsRest {
    /// `0x0064C040` after a strength / dexterity change (not specified).
    fn refresh(&mut self, u: UnitId);
    /// `vitals.md` §3 step 7: party roster, level-up sound, the level
    /// broadcast, the item and host callbacks (transport, party, items).
    fn level_up_notify(&mut self, u: UnitId);
    /// Unit event 12 `levelup` (`0x005C0C30` event registry, not in
    /// d2-sim).
    fn level_up_event(&mut self, u: UnitId);
}

/// The vitals' view: unit records, stat lists, the stat host and the
/// rest.
pub struct VitalsView<'a, H, R> {
    pub units: &'a Units,
    pub stats: &'a mut StatLists,
    pub hooks: &'a mut H,
    pub rest: &'a mut R,
}

impl<H: StatHost, R: VitalsRest> VitalsUnits for VitalsView<'_, H, R> {
    type Unit = UnitId;

    fn unit_type(&self, u: UnitId) -> UnitType {
        self.units.get(u).map_or(UnitType::Tile, |r| r.ty)
    }
    fn class_id(&self, u: UnitId) -> i32 {
        self.units.get(u).map_or(0, |r| r.class as i32)
    }
    /// `0x006253B0(unit, stat, 0)`.
    fn base_stat(&self, u: UnitId, s: u16) -> i32 {
        self.stats.unit_base(u, s, 0)
    }
    /// `0x00625480(unit, stat, 0)`.
    fn stat(&self, u: UnitId, s: u16) -> i32 {
        self.stats.unit_total(u, s, 0)
    }
    /// `0x00627260(unit, stat, v, 0)`.
    fn set_base_stat(&mut self, u: UnitId, s: u16, v: i32) {
        self.stats.unit_set(&mut *self.hooks, u, s, v, 0);
    }
    /// `0x006272B0(unit, stat, v, 0)`.
    fn add_base_stat(&mut self, u: UnitId, s: u16, v: i32) {
        self.stats.unit_add(&mut *self.hooks, u, s, v, 0);
    }
    fn max_life(&self, u: UnitId) -> i32 {
        self.stats.max_life(u)
    }
    fn max_mana(&self, u: UnitId) -> i32 {
        self.stats.max_mana(u)
    }
    fn max_stamina(&self, u: UnitId) -> i32 {
        self.stats.max_stamina(u)
    }
    fn refresh(&mut self, u: UnitId) {
        self.rest.refresh(u);
    }
    fn level_up_notify(&mut self, u: UnitId) {
        self.rest.level_up_notify(u);
    }
    fn level_up_event(&mut self, u: UnitId) {
        self.rest.level_up_event(u);
    }
}

/// Experience for `attacker` killing `defender`: the level factor
/// `0x0057E2F0` (`vitals.md` §4.2) on the defender's base experience
/// (stat 13, set at monster init, `monsters/init.md` §6) and both
/// levels (stat 12), then the add function (§4.3: cap, level-up §3,
/// event 12). Returns the gain added; players only.
///
/// TODO(vitals.md OQ2, damage.md OQ7): the rest of `0x0057E480` and the
/// distribution (`ExpRatio`, stat 85, hireling cap, pet credit, party
/// share) are D2MOO structure, not confirmed in 1.14d: not applied.
/// The levels are read as unit totals (the spec says "both levels").
pub fn kill_experience<W: VitalsUnits>(
    w: &mut W,
    t: &VitalsTables,
    attacker: W::Unit,
    defender: W::Unit,
) -> u32 {
    if w.unit_type(attacker) != UnitType::Player {
        return 0;
    }
    let exp = w.base_stat(defender, stat::EXPERIENCE);
    let gain = level_factor(
        exp,
        w.stat(attacker, stat::LEVEL),
        w.stat(defender, stat::LEVEL),
    );
    if gain <= 0 {
        return 0;
    }
    add_experience(w, t, attacker, gain as u32);
    gain as u32
}
