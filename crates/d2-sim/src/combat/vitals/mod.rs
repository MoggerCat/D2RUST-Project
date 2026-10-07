// Spec: specs/combat/vitals.md
//! Vitals: player creation values (§1), spending stat points and the
//! stat reset (§2), level-up (§3), the experience table lookups (§4.1),
//! the experience-on-kill level factor (§4.2), the gain, distribution,
//! add, death penalties and corpse experience (§4.3–§4.7, [`experience`])
//! and the client vitals sync (§5, [`sync`]).
//!
//! Status: implemented, unverified (the spec is a draft; its checks are
//! queued in `docs/handoff/impl-skilluse-vitals.md`). No randomness.
//!
//! Stat reads and writes go through the [`VitalsUnits`] seam (provider:
//! units/stats, `sim/stats.md` §4.2 getters and setters); table data
//! comes from `d2_data` typed records ([`VitalsTables`]).

pub mod experience;
#[cfg(test)]
mod experience_tests;
pub mod sync;
#[cfg(test)]
mod tests;

use super::pct;
use crate::units::UnitType;
use d2_data::bin::BinSet;
use d2_data::tables::{decode_all, Charstats, Experience};

/// Stat ids of the vitals (1.14d itemstatcost rows).
pub mod stat {
    pub use crate::stats::stat::{
        DEXTERITY, ENERGY, HITPOINTS, LEVEL, MANA, MAXHP, MAXMANA, MAXSTAMINA, STAMINA, STRENGTH,
        VITALITY,
    };
    pub const STATPTS: u16 = 4;
    pub const NEWSKILLS: u16 = 5;
    pub const EXPERIENCE: u16 = 13;
    pub const TOHIT: u16 = 19;
    pub const TOBLOCK: u16 = 20;
    pub const LASTEXP: u16 = 29;
    pub const NEXTEXP: u16 = 30;
    pub const VELOCITYPERCENT: u16 = 67;
    pub const ATTACKRATE: u16 = 68;
    pub const OTHER_ANIMRATE: u16 = 69;
}

/// The stat reads, writes and notifications the vitals need (a seam).
/// Provider: units/stats (`sim/stats.md` §4.2), `d2-server` for the
/// client notifications. Every method names its 1.14d address.
pub trait VitalsUnits {
    type Unit: Copy + Eq + std::fmt::Debug;
    /// Unit type (unit +0x00).
    fn unit_type(&self, u: Self::Unit) -> UnitType;
    /// Class id (unit +0x04).
    fn class_id(&self, u: Self::Unit) -> i32;
    /// Base getter `0x006253B0(unit, stat, 0)`.
    fn base_stat(&self, u: Self::Unit, stat: u16) -> i32;
    /// Unit getter `0x00625480(unit, stat, 0)`.
    fn stat(&self, u: Self::Unit, stat: u16) -> i32;
    /// Base set `0x00627260(unit, stat, v, 0)`.
    fn set_base_stat(&mut self, u: Self::Unit, stat: u16, v: i32);
    /// Base add `0x006272B0(unit, stat, v, 0)`.
    fn add_base_stat(&mut self, u: Self::Unit, stat: u16, v: i32);
    /// Max life / mana / stamina `0x00625D10` / `0x00625D60` /
    /// `0x00625DB0`.
    fn max_life(&self, u: Self::Unit) -> i32;
    fn max_mana(&self, u: Self::Unit) -> i32;
    fn max_stamina(&self, u: Self::Unit) -> i32;
    /// The refresh after a strength / dexterity change `0x0064C040`.
    fn refresh(&mut self, u: Self::Unit);
    /// §3 step 7: party roster `0x00536850`, level-up sound
    /// `0x00553380(unit, 2)`, `0x005538D0(game, unit, 0x00570850)`,
    /// `0x0055F500`, `0x0055FDE0(…, 1)`, host callback
    /// `[0x00883D50]+0x2C`, in that order.
    fn level_up_notify(&mut self, u: Self::Unit);
    /// Unit event 12 (`levelup`, §4.3).
    fn level_up_event(&mut self, u: Self::Unit);
}

/// The tables the vitals read.
#[derive(Debug, Clone)]
pub struct VitalsTables {
    pub charstats: Vec<Charstats>,
    /// `experience.txt` rows (`[0x0096C8A8]`): row 0 `MaxLvl`, row `L + 1`
    /// level `L`.
    pub experience: Vec<Experience>,
}

impl VitalsTables {
    /// From a loaded `.bin` set.
    pub fn from_bin(set: &BinSet) -> Result<Self, crate::skills::TablesError> {
        use crate::skills::TablesError;
        let table = |n: &'static str| set.table(n).ok_or(TablesError::Missing(n));
        Ok(Self {
            charstats: decode_all(table("charstats")?)?,
            experience: decode_all(table("experience")?)?,
        })
    }

    /// `charstats` row of a player class.
    pub fn charstats(&self, class: i32) -> Option<&Charstats> {
        usize::try_from(class)
            .ok()
            .and_then(|i| self.charstats.get(i))
    }

    /// Column `class` of experience row `row`. Classes outside 0–6 use
    /// class 0 (§4.1). Panics past the table (the original reads past
    /// it).
    fn exp(&self, class: i32, row: usize) -> u32 {
        let r = self
            .experience
            .get(row)
            .expect("experience row past the table (vitals.md §4.1)");
        match class {
            1 => r.sorceress,
            2 => r.necromancer,
            3 => r.paladin,
            4 => r.barbarian,
            5 => r.druid,
            6 => r.assassin,
            _ => r.amazon,
        }
    }

    /// `max_level(class)` = `0x00611830` (§4.1): row 0.
    pub fn max_level(&self, class: i32) -> u32 {
        self.exp(class, 0)
    }

    /// `threshold(class, L)` = `0x00611800` (§4.1): row `L + 1`, the
    /// experience needed to reach level `L + 1`.
    pub fn threshold(&self, class: i32, level: u32) -> u32 {
        self.exp(class, level as usize + 1)
    }

    /// `level_from_exp(class, exp)` = `0x00611860` (§4.1), unsigned.
    pub fn level_from_exp(&self, class: i32, exp: u32) -> u32 {
        let max = self.max_level(class);
        let mut i = 0;
        while i < max && exp >= self.exp(class, i as usize + 1) {
            i += 1;
        }
        i
    }
}

/// A charstats quarter-point column as 1/256 units: `v << 6`.
fn q(v: i32) -> i32 {
    v.wrapping_shl(6)
}

// ---------------------------------------------------------------- §1

/// The target levels of `act` (`0x006E1520`).
pub const ACT_TARGET_LEVEL: [u32; 5] = [1, 15, 20, 26, 32];

/// `init_player_stats(game, unit, act)` = `0x005706D0` (§1).
pub fn init_player_stats<W: VitalsUnits>(w: &mut W, t: &VitalsTables, u: W::Unit, act: u32) {
    let class = w.class_id(u);
    let Some(c) = t.charstats(class) else {
        return;
    };
    let act = act.min(4);
    let (str_, int, dex, vit) = (
        i32::from(c.str),
        i32::from(c.int),
        i32::from(c.dex),
        i32::from(c.vit),
    );
    let life = (vit + i32::from(c.hpadd)) << 8;
    let stamina = i32::from(c.stamina) << 8;
    for (s, v) in [
        (stat::STRENGTH, str_),
        (stat::ENERGY, int),
        (stat::DEXTERITY, dex),
        (stat::VITALITY, vit),
        (stat::TOHIT, 0),
        (stat::TOBLOCK, 0),
        (stat::HITPOINTS, life),
        (stat::MAXHP, life),
        (stat::MANA, int << 8),
        (stat::MAXMANA, int << 8),
        (stat::STAMINA, stamina),
        (stat::MAXSTAMINA, stamina),
        (stat::LEVEL, 1),
        (stat::NEXTEXP, t.threshold(class, 1) as i32),
        (stat::ATTACKRATE, 100),
        (stat::VELOCITYPERCENT, 100),
        (stat::OTHER_ANIMRATE, 100),
    ] {
        // TODO(vitals.md §1): the write order of the table is not given;
        // d2rs writes in table order (no write has a side effect here).
        w.set_base_stat(u, s, v);
    }
    if act > 0 {
        let idx = if (act as usize) < ACT_TARGET_LEVEL.len() {
            act as usize
        } else {
            0
        };
        set_experience_for_target_level(w, t, u, ACT_TARGET_LEVEL[idx]);
    }
}

/// `0x0057EB10` (§1, D2MOO `SUNITDMG_SetExperienceForTargetLevel`): add
/// `threshold(class, target) − experience` if positive, through §4.3.
// TODO(vitals.md §1): threshold(class, target) is the experience of level
// target + 1 under §4.1; the spec states it so, unconfirmed by a trace.
pub fn set_experience_for_target_level<W: VitalsUnits>(
    w: &mut W,
    t: &VitalsTables,
    u: W::Unit,
    target: u32,
) {
    let class = w.class_id(u);
    let need = t.threshold(class, target);
    let have = w.base_stat(u, stat::EXPERIENCE) as u32;
    if need > have {
        add_experience(w, t, u, need - have);
    }
}

// ---------------------------------------------------------------- §2

/// Handler of client message 0x3A `0x0054BD10` (§2): 0, 2 (a spend
/// failed) or 3 (bad message). Byte +1 is the stat, byte +2 is
/// `count − 1` (Open question 4).
pub fn handle_add_stat_point<W: VitalsUnits>(
    w: &mut W,
    t: &VitalsTables,
    u: W::Unit,
    m: &[u8],
) -> i32 {
    if m.len() != 3 {
        return 3;
    }
    let (s, n1) = (m[1], m[2]);
    if s > 15 || n1 > 99 {
        return 3;
    }
    for _ in 0..=n1 {
        if !spend(w, t, u, s) {
            return 2;
        }
    }
    0
}

/// `spend(unit, s)` = `0x00570D60` (§2): false = fail.
pub fn spend<W: VitalsUnits>(w: &mut W, t: &VitalsTables, u: W::Unit, s: u8) -> bool {
    if w.stat(u, stat::STATPTS) == 0 {
        return false;
    }
    match u16::from(s) {
        s @ (stat::STRENGTH | stat::DEXTERITY) => {
            w.add_base_stat(u, stat::STATPTS, -1);
            w.add_base_stat(u, s, 1);
            w.refresh(u);
            true
        }
        stat::ENERGY => {
            gain_energy(w, t, u, 1);
            true
        }
        stat::VITALITY => {
            gain_vitality(w, t, u, 1);
            true
        }
        _ => false,
    }
}

/// `gain_energy(unit, n)` = `0x00570A80` (§2).
// TODO(vitals.md §2): the rule text could put both mana additions under
// `n > 0`; d2rs reads the condition as covering the current mana only, as
// the closing paragraph states ("lowers the maximum but not the current").
pub fn gain_energy<W: VitalsUnits>(w: &mut W, t: &VitalsTables, u: W::Unit, n: i32) {
    let per = t
        .charstats(w.class_id(u))
        .map_or(0, |c| i32::from(c.manapermagic));
    w.add_base_stat(u, stat::STATPTS, n.wrapping_neg());
    w.add_base_stat(u, stat::ENERGY, n);
    let d = q(per.wrapping_mul(n));
    if n > 0 {
        w.add_base_stat(u, stat::MANA, d);
    }
    w.add_base_stat(u, stat::MAXMANA, d);
    if w.stat(u, stat::MANA) > w.stat(u, stat::MAXMANA) {
        let m = w.stat(u, stat::MAXMANA);
        w.set_base_stat(u, stat::MANA, m);
    }
}

/// `gain_vitality(unit, n)` = `0x00570B60` (§2).
pub fn gain_vitality<W: VitalsUnits>(w: &mut W, t: &VitalsTables, u: W::Unit, n: i32) {
    let (life, stam) = t.charstats(w.class_id(u)).map_or((0, 0), |c| {
        (
            i32::from(c.lifepervitality),
            i32::from(c.staminapervitality),
        )
    });
    w.add_base_stat(u, stat::STATPTS, n.wrapping_neg());
    w.add_base_stat(u, stat::VITALITY, n);
    let dl = q(life.wrapping_mul(n));
    w.add_base_stat(u, stat::MAXHP, dl);
    if n > 0 {
        w.add_base_stat(u, stat::HITPOINTS, dl);
    }
    if w.stat(u, stat::HITPOINTS) > w.stat(u, stat::MAXHP) {
        let m = w.stat(u, stat::MAXHP);
        w.set_base_stat(u, stat::HITPOINTS, m);
    }
    let ds = q(stam.wrapping_mul(n));
    w.add_base_stat(u, stat::MAXSTAMINA, ds);
    if n > 0 {
        w.add_base_stat(u, stat::STAMINA, ds);
    }
    if w.stat(u, stat::STAMINA) > w.stat(u, stat::MAXSTAMINA) {
        let m = w.stat(u, stat::MAXSTAMINA);
        w.set_base_stat(u, stat::STAMINA, m);
    }
}

/// Stat reset `0x00570C80` (§2.1), players only.
pub fn reset_stats<W: VitalsUnits>(w: &mut W, t: &VitalsTables, u: W::Unit) {
    if w.unit_type(u) != UnitType::Player {
        return;
    }
    let Some(c) = t.charstats(w.class_id(u)) else {
        return;
    };
    let starts = [
        (stat::STRENGTH, i32::from(c.str)),
        (stat::ENERGY, i32::from(c.int)),
        (stat::DEXTERITY, i32::from(c.dex)),
        (stat::VITALITY, i32::from(c.vit)),
    ];
    for (s, start) in starts {
        let d = start.wrapping_sub(w.base_stat(u, s));
        match s {
            stat::ENERGY => gain_energy(w, t, u, d),
            stat::VITALITY => gain_vitality(w, t, u, d),
            _ => {
                if d != 0 {
                    w.add_base_stat(u, stat::STATPTS, d.wrapping_neg());
                    w.add_base_stat(u, s, d);
                    w.refresh(u);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- §3

/// Level-up `0x00570880` (§3); returns `d = new − old`.
pub fn level_up<W: VitalsUnits>(w: &mut W, t: &VitalsTables, u: W::Unit) -> i32 {
    let class = w.class_id(u);
    let old = w.base_stat(u, stat::LEVEL);
    let new = t.level_from_exp(class, w.base_stat(u, stat::EXPERIENCE) as u32);
    w.set_base_stat(u, stat::LEVEL, new as i32);
    w.set_base_stat(u, stat::NEXTEXP, t.threshold(class, new) as i32);
    let d = (new as i32).wrapping_sub(old);
    if d <= 0 {
        return d;
    }
    let (lpl, mpl, spl, statpl) = t.charstats(class).map_or((0, 0, 0, 0), |c| {
        (
            i32::from(c.lifeperlevel),
            i32::from(c.manaperlevel),
            i32::from(c.staminaperlevel),
            i32::from(c.statperlevel as i8),
        )
    });
    let maxhp = w
        .base_stat(u, stat::MAXHP)
        .wrapping_add(q(lpl.wrapping_mul(d)));
    w.set_base_stat(u, stat::MAXHP, maxhp);
    if w.stat(u, stat::HITPOINTS) > 0 {
        let m = w.max_life(u);
        w.set_base_stat(u, stat::HITPOINTS, m);
    }
    let maxmana = w
        .base_stat(u, stat::MAXMANA)
        .wrapping_add(q(mpl.wrapping_mul(d)));
    w.set_base_stat(u, stat::MAXMANA, maxmana);
    let m = w.max_mana(u);
    w.set_base_stat(u, stat::MANA, m);
    let maxst = w
        .base_stat(u, stat::MAXSTAMINA)
        .wrapping_add(q(spl.wrapping_mul(d)));
    w.set_base_stat(u, stat::MAXSTAMINA, maxst);
    let m = w.max_stamina(u);
    w.set_base_stat(u, stat::STAMINA, m);
    w.add_base_stat(u, stat::STATPTS, statpl.wrapping_mul(d));
    w.add_base_stat(u, stat::NEWSKILLS, d);
    w.level_up_notify(u);
    d
}

// ---------------------------------------------------------------- §4

/// `T1` (`0x006E1668`): defender at or below the attacker's level.
pub const LEVEL_FACTOR_LOWER: [i32; 11] = [256, 256, 256, 256, 256, 256, 207, 159, 110, 61, 13];
/// `T2` (`0x006E1694`): defender above the attacker's level.
pub const LEVEL_FACTOR_HIGHER: [i32; 11] = [256, 256, 256, 256, 256, 256, 225, 174, 92, 38, 5];

/// Level factor `0x0057E2F0(exp, alvl, dlvl)` (§4.2), signed.
// TODO(vitals.md OQ3): the operand roles of pct(exp, alvl, dlvl) in the
// `dlvl > alvl ≥ 25` branch are read from registers, unconfirmed.
pub fn level_factor(exp: i32, alvl: i32, dlvl: i32) -> i32 {
    let f = if dlvl <= alvl {
        LEVEL_FACTOR_LOWER[alvl.wrapping_sub(dlvl).min(10) as usize]
    } else {
        if alvl >= 25 && dlvl > 0 {
            return pct(exp, alvl, dlvl);
        }
        LEVEL_FACTOR_HIGHER[dlvl.wrapping_sub(alvl).min(10) as usize]
    };
    if f == 256 {
        exp
    } else {
        pct(exp, f, 256)
    }
}

/// The add `0x0057E510` (§4.5) at the gainer's base level, for callers
/// without their own level argument (§1 `0x0057EB10`).
pub fn add_experience<W: VitalsUnits>(w: &mut W, t: &VitalsTables, u: W::Unit, gain: u32) {
    let l0 = w.base_stat(u, stat::LEVEL);
    add_experience_at(w, t, u, l0, gain as i32);
}

/// The add `0x0057E510(game, L0, g)` (§4.5): players only; new := old + g
/// (32-bit), capped at `threshold(class, max_level − 1)` (unsigned);
/// `lastexp` (29) := new − old; experience (13) := new; a level from the
/// new experience ≠ `l0` → level-up (§3) and unit event 12.
pub fn add_experience_at<W: VitalsUnits>(w: &mut W, t: &VitalsTables, u: W::Unit, l0: i32, g: i32) {
    if w.unit_type(u) != UnitType::Player {
        return;
    }
    let class = w.class_id(u);
    let old = w.base_stat(u, stat::EXPERIENCE) as u32;
    let cap = t.threshold(class, t.max_level(class).saturating_sub(1));
    let new = old.wrapping_add(g as u32).min(cap);
    w.set_base_stat(u, stat::LASTEXP, new.wrapping_sub(old) as i32);
    w.set_base_stat(u, stat::EXPERIENCE, new as i32);
    if t.level_from_exp(class, new) as i32 != l0 {
        level_up(w, t, u);
        w.level_up_event(u);
    }
}
