// Spec: specs/combat/hit.md, specs/combat/damage.md
//! Combat: attack rating, defense, chance to hit, block and avoid
//! ([`hit`], `combat/hit.md`); damage roll, resistances, application,
//! leech and the death trigger ([`damage`], `combat/damage.md`).
//!
//! Status: implemented, unverified (both specs are drafts; their trace
//! checks are queued in `docs/handoff/impl-combat.md`).
//!
//! Unit state and the effects other specs own (stat writes, states and
//! stat lists, timers, unit events, modes) go through the
//! [`CombatWorld`] seam, which extends [`crate::skills::SkillUnits`].
//! Table data comes from `d2_data` typed records ([`CombatTables`]).

pub mod damage;
pub mod events;
pub mod hit;
pub mod range;
pub mod vitals;

use crate::skills::SkillUnits;
use crate::units::UnitType;
use d2_data::bin::BinSet;
use d2_data::tables::{decode_all, Charstats, Difficultylevels, Monstats, Monstats2};

pub use damage::*;
pub use hit::*;

/// `pct(v, p, d)` = `0x00483360` (`damage.md` §0, D2MOO
/// `MONSTERUNIQUE_CalculatePercentage`): `v × p / d` with the original's
/// precision-losing shortcuts for large values. Signed 32-bit, truncating.
pub fn pct(v: i32, p: i32, d: i32) -> i32 {
    if d == 0 {
        return 0;
    }
    let wide = || (i64::from(v) * i64::from(p) / i64::from(d)) as i32;
    if v > 0x10_0000 {
        if d <= v >> 4 {
            v.wrapping_div(d).wrapping_mul(p)
        } else {
            wide()
        }
    } else if p > 0x1_0000 {
        if d <= p >> 4 {
            p.wrapping_div(d).wrapping_mul(v)
        } else {
            wide()
        }
    } else {
        v.wrapping_mul(p).wrapping_div(d)
    }
}

/// `(x × s) / 128` with a 32-bit product, truncating (`damage.md` §0
/// "Scaling by `SrcDam`").
pub fn scale(x: i32, s: i32) -> i32 {
    x.wrapping_mul(s) / 128
}

/// Where a defender stands (`0x00620BB0` room, `0x0061AB00` town).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomKind {
    /// No room.
    None,
    Town,
    Field,
}

/// One entry of an attacker's combat list (`damage.md` §3 step 3, D2MOO
/// `pCombat`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatEntry {
    pub attacker: (UnitType, u32),
    pub defender: (UnitType, u32),
    pub record: DamageRecord,
}

/// The unit queries and effects combat needs beyond [`SkillUnits`] (a
/// seam). Providers, by spec: units/stats (`sim/units.md`,
/// `sim/stats.md`, `sim/stat-lists.md`: modes, ranges, stat writes,
/// states, stat lists, unit events, combat list); `sim/tick.md` (timers);
/// monsters branch (`monsters/`: monster flags and hooks); items
/// (durability, composits). Names give the 1.14d address they stand for.
pub trait CombatWorld: SkillUnits {
    // ------------------------------------------------------------ game
    /// Game frame (game +0xA8).
    fn frame(&self) -> i32;
    /// Expansion game (game +0x70).
    fn expansion(&self) -> bool;
    /// Difficulty 0–2 (game +0x6D).
    fn difficulty(&self) -> usize;
    /// The process-wide element hit-class byte `0x0088CAD0` (`damage.md`
    /// Edge case 7; one per server process).
    fn hit_class_counter(&mut self) -> &mut u8;

    // ------------------------------------------------------- unit reads
    /// Unit type and GUID (combat-list identity).
    fn ident(&self, u: Self::Unit) -> (UnitType, u32);
    /// Unit mode (unit +0x0C).
    fn mode(&self, u: Self::Unit) -> i32;
    /// `0x00622D00`: the unit is in a moving mode.
    fn moving_mode(&self, u: Self::Unit) -> bool;
    /// `0x005A0180(unit, mask)`: monster type flags (2 superunique, 4
    /// champion, 8 unique).
    fn monster_flag(&self, u: Self::Unit, mask: u32) -> bool;
    /// `0x0063E9F0` boss.
    fn is_boss(&self, u: Self::Unit) -> bool;
    /// `0x0063EE90` hireling.
    fn is_hireling(&self, u: Self::Unit) -> bool;
    /// `0x0063E940` demon.
    fn is_demon(&self, u: Self::Unit) -> bool;
    /// `0x0063E990` undead.
    fn is_undead(&self, u: Self::Unit) -> bool;
    /// `0x0063EDC0` prime evil.
    fn is_prime_evil(&self, u: Self::Unit) -> bool;
    /// `0x00451F30`: unit flag 0x80000000 (revived).
    fn is_revived(&self, u: Self::Unit) -> bool;
    /// `0x006259B0` alignment (2 = good).
    fn alignment(&self, u: Self::Unit) -> i32;
    /// `0x00554200(game, attacker, defender)` hostility (`sim/units.md`).
    fn hostile(&self, a: Self::Unit, d: Self::Unit) -> bool;
    /// `0x00622870` melee range of a unit.
    fn melee_range(&self, u: Self::Unit) -> i32;
    /// `0x00622C40(attacker, defender, range)`: in melee range.
    fn in_melee_range(&self, a: Self::Unit, d: Self::Unit, range: i32) -> bool;
    /// `0x0063C8F0(inventory, …)`: a shield is equipped.
    fn has_shield(&self, u: Self::Unit) -> bool;
    /// `0x006225F0` for monster classes other than 243/310/333/359: the
    /// composit item in slot 7 (`SH`) is a code other than `tch ` whose
    /// items record has type 2.
    fn composit_shield(&self, u: Self::Unit) -> bool;
    /// `0x0064F380`: weapon class (13 = `ht2`).
    fn weapon_class(&self, u: Self::Unit) -> i32;
    /// `0x00623C20`: the weapon hit class of a unit.
    fn weapon_hit_class(&self, u: Self::Unit) -> u32;
    /// `0x0057A830`: `layer` matches `montype` in the montype nest bitmap.
    fn montype_matches(&self, layer: u16, montype: i32) -> bool;
    /// `0x00620BB0` / `0x0061AB00`: the unit's room kind.
    fn room(&self, u: Self::Unit) -> RoomKind;
    /// `0x005541B0`: the unit is dead.
    fn is_dead(&self, u: Self::Unit) -> bool;
    /// `0x0046C140(class, mode)`: the monster class has animation `mode`.
    fn monster_has_mode(&self, u: Self::Unit, mode: i32) -> bool;
    /// `0x00645270` (D2MOO `D2COMMON_11013_ConvertMode`): the unit type
    /// its mode converts to, for the leech rules.
    fn converted_type(&self, u: Self::Unit) -> i32;
    /// `0x00629930`: the item has durability.
    fn item_has_durability(&self, item: Self::Item) -> bool;
    /// `0x005738F0(n)`: the player-count life bonus (monsters branch).
    fn player_count_bonus(&self, players: i32) -> i32;

    // --------------------------------------------------------- effects
    /// Sets a stat's base value (life 6, mana 8, stamina 10, armorclass
    /// 31 via `0x00627260`).
    fn set_stat(&mut self, u: Self::Unit, stat: u16, value: i32);
    /// `0x005C0C30(game, event, unit, other, record)`: runs the unit's
    /// event functions for `event` (`events.txt` index).
    fn unit_event(
        &mut self,
        event: u8,
        unit: Self::Unit,
        other: Self::Unit,
        record: &mut DamageRecord,
    );
    /// Switches `state` on or off.
    fn set_state(&mut self, u: Self::Unit, state: u16, on: bool);
    /// The curse helper (`0x0056E970`; skills spec): state `state` for
    /// `frames`, with stat `stat` = `value`, from `owner`.
    #[allow(clippy::too_many_arguments)]
    fn curse(
        &mut self,
        target: Self::Unit,
        owner: Self::Unit,
        state: u16,
        stat: u16,
        value: i32,
        frames: i32,
        skill: i32,
        level: i32,
    );
    /// Expiry frame of the unit's stat list for `state`, `None` without one.
    fn state_list_expiry(&self, u: Self::Unit, state: u16) -> Option<i32>;
    /// Sets that list's expiry.
    fn set_state_list_expiry(&mut self, u: Self::Unit, state: u16, expiry: i32);
    /// Creates and attaches a stat list for `state` (owner, expiry).
    fn create_state_list(&mut self, u: Self::Unit, state: u16, owner: Self::Unit, expiry: i32);
    /// Sets a stat in the list of `state`.
    fn set_state_list_stat(&mut self, u: Self::Unit, state: u16, stat: u16, value: i32);
    /// Schedules timer event `ty` for the unit at `frame` (`sim/tick.md` §5).
    fn schedule_timer(&mut self, u: Self::Unit, ty: u8, frame: i32);
    /// Cancels the unit's timers of event type `ty`.
    fn cancel_timers(&mut self, u: Self::Unit, ty: u8);
    /// Puts overlay `id` on the unit (`0x00621E40`).
    fn overlay(&mut self, u: Self::Unit, id: i32);
    /// `0x00623F50`: refresh the animation rate.
    fn refresh_anim_rate(&mut self, u: Self::Unit);
    /// `0x00621D50`: remember `a` as `d`'s last attacker.
    fn set_last_attacker(&mut self, d: Self::Unit, a: Self::Unit);
    /// `0x005A4390(game, attacker)` after a monster's hit (monsters branch).
    fn monster_hit_hook(&mut self, a: Self::Unit);
    /// `0x005D6410(defender)` after a non-lethal hit on a monster.
    fn monster_damaged_hook(&mut self, d: Self::Unit);
    /// `0x00535D10` (`on`) / `0x00535E20` (off): dual-wield weapon
    /// stat-list switching around a roll.
    fn dual_wield_switch(&mut self, a: Self::Unit, offhand: bool, on: bool);
    /// The attacker's combat list (D2MOO `pCombat`), first = newest.
    fn combat_list(&mut self, u: Self::Unit) -> &mut Vec<CombatEntry>;
    /// The rest of `durability_hit` after a successful draw: durability −
    /// 1 and the item rules (items spec).
    fn durability_loss(&mut self, owner: Self::Unit, item: Self::Item);
    /// Thorns `0x005D10C0` (skills spec).
    fn thorns(&mut self, a: Self::Unit, d: Self::Unit, record: &mut DamageRecord);
    /// Reaction `0x0057CEE0` (`damage.md` §7.1; checked only at the call
    /// level, OQ3; mode changes belong to `sim/units.md`).
    fn reaction(&mut self, a: Self::Unit, d: Self::Unit, record: &mut DamageRecord);
}

/// The tables combat reads, as `d2_data` typed records.
#[derive(Debug, Clone)]
pub struct CombatTables {
    pub charstats: Vec<Charstats>,
    pub difficultylevels: Vec<Difficultylevels>,
    pub monstats: Vec<Monstats>,
    pub monstats2: Vec<Monstats2>,
    /// `hitclass` codes by row (the get-hit divisor, `damage.md` §6.2).
    pub hitclass: Vec<[u8; 4]>,
}

impl CombatTables {
    /// From a loaded `.bin` set.
    pub fn from_bin(set: &BinSet) -> Result<Self, crate::skills::TablesError> {
        use crate::skills::TablesError;
        let table = |n: &'static str| set.table(n).ok_or(TablesError::Missing(n));
        Ok(Self {
            charstats: decode_all(table("charstats")?)?,
            difficultylevels: decode_all(table("difficultylevels")?)?,
            monstats: decode_all(table("monstats")?)?,
            monstats2: decode_all(table("monstats2")?)?,
            hitclass: set
                .hitclass
                .iter()
                .map(|r| [r[0], r[1], r[2], r[3]])
                .collect(),
        })
    }

    /// `charstats` row of a player class.
    pub fn charstats(&self, class: i32) -> Option<&Charstats> {
        usize::try_from(class)
            .ok()
            .and_then(|i| self.charstats.get(i))
    }

    /// `difficultylevels` row (`0x00611D30`).
    pub fn difficulty(&self, d: usize) -> Option<&Difficultylevels> {
        self.difficultylevels.get(d)
    }

    /// `monstats` row of a monster class.
    pub fn monstats(&self, class: i32) -> Option<&Monstats> {
        usize::try_from(class)
            .ok()
            .and_then(|i| self.monstats.get(i))
    }

    /// `monstats2` row of a monster class (via `MonStatsEx`).
    pub fn monstats2(&self, class: i32) -> Option<&Monstats2> {
        self.monstats(class)
            .and_then(|m| self.monstats2.get(usize::from(m.monstatsex)))
    }
}

#[cfg(test)]
mod cov_tests;
#[cfg(test)]
mod damage_tests;
#[cfg(test)]
mod events_tests;
#[cfg(test)]
mod hit_gap_tests;
#[cfg(test)]
mod mutant_tests;
#[cfg(test)]
mod tests;
