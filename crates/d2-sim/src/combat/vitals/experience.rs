// Spec: specs/combat/vitals.md §4.3–§4.7
//! Experience on a kill and on death: the player gain `0x0057E480`
//! (§4.3), the distribution `0x0057E990` (§4.4) with the party share
//! `0x0057E6C0`, the add `0x0057E510` (§4.5), the death penalties
//! `0x00535AB0` (§4.6) and the corpse experience (§4.7).
//!
//! The hireling's share (§4.4 step 3) is `world/hirelings.md` §7
//! (`crate::world::hirelings::level`); here it is the seam
//! [`ExpShare::hireling_share`]. No randomness.
//!
//! The party share's x87 arithmetic (`fild` / `fidiv` stored to a float32,
//! then `fild` × float32 truncated by `0x00682FD0`) is reproduced exactly
//! in integers ([`party_quotient`], [`party_share`]): the quotient is
//! rounded to float32 (nearest, ties to even) and the product is exact.
// TODO(spec: sim/stat-lists.md Open question 1): the x87 precision
// control at these calls is not recorded; the product is taken at full
// precision (exact for every level and float32 quotient).

use super::{add_experience_at, level_factor, stat, VitalsTables, VitalsUnits};
use crate::combat::pct;
use crate::units::UnitType;

/// §4.3 rule 1: the experience cap of one gain.
pub const GAIN_CAP: i32 = 0x7F_FFFF;
/// Stat 85 `item_addexperience`.
pub const ADDEXPERIENCE: u16 = 85;
/// Stat 14 `gold`, 15 `goldbank`, 175 `goldlost`.
pub const GOLD: u16 = 14;
pub const GOLDBANK: u16 = 15;
pub const GOLDLOST: u16 = 175;
/// The party share's member limit (a 9th member is a fatal assert).
pub const MAX_PARTY: usize = 8;
/// Squared distance limit of a party member to the defender (§4.4 r6).
pub const PARTY_RANGE_SQ: u32 = 6400;

impl VitalsTables {
    /// `ratio(L)` = `0x00613E60(L)` (§4.3 rule 4): no table → 0; L < 1 →
    /// the `MaxLvl` row's `ExpRatio`; 1 ≤ L ≤ that row's class-0 value →
    /// level L's `ExpRatio` (row L + 1); above → 0.
    pub fn exp_ratio(&self, level: i32) -> i32 {
        let Some(head) = self.experience.first() else {
            return 0;
        };
        if level < 1 {
            return head.expratio as i32;
        }
        if level as u32 > head.amazon {
            return 0;
        }
        self.experience
            .get(level as usize + 1)
            .map_or(0, |r| r.expratio as i32)
    }

    /// The `ExpRatio` step `0x0057E390(g, alvl)` (§4.3 rule 4).
    pub fn apply_exp_ratio(&self, g: i32, alvl: i32) -> i32 {
        if g <= 0 {
            return g;
        }
        let r = self.exp_ratio(alvl);
        let s = self.exp_ratio(0);
        if (s as u32).wrapping_sub(1) >= 31 {
            return g;
        }
        let sh = (r >> s).wrapping_add(s) as u32 & 31;
        let limit = 0x7FFF_FFFFi32 >> sh;
        if g > limit {
            (g >> s).wrapping_mul(r)
        } else {
            r.wrapping_mul(g) >> s
        }
    }
}

/// The gain `0x0057E480` (§4.3) for a **player** gainer `u` (rule 6: a
/// player keeps g; the non-player cap is `world/hirelings.md` §7.2).
pub fn player_gain<W: VitalsUnits>(
    w: &W,
    t: &VitalsTables,
    e: i32,
    u: W::Unit,
    alvl: i32,
    dlvl: i32,
) -> i32 {
    // Rule 1.
    let e = if e > GAIN_CAP {
        GAIN_CAP
    } else if e <= 0 {
        return 1;
    } else {
        e
    };
    // Rule 2: U's class (a player).
    if alvl >= t.max_level(w.class_id(u)) as i32 {
        return 0;
    }
    // Rules 3–5.
    let mut g = level_factor(e, alvl, dlvl);
    g = t.apply_exp_ratio(g, alvl);
    let x = w.stat(u, ADDEXPERIENCE);
    if x != 0 {
        g = g.wrapping_add(pct(g, x, 100));
    }
    g
}

/// The world calls the distribution needs beside [`VitalsUnits`].
pub trait ExpShare: VitalsUnits {
    /// The credited player `0x0057E7B0` (§4.4 step 2) of a monster
    /// attacker: its owner, replaced by the owner of a flag-0x800 stat
    /// list on A, then on D. `None` unless that unit is a player.
    fn credited_player(&self, attacker: Self::Unit, defender: Self::Unit) -> Option<Self::Unit>;
    /// §4.4 step 3: the hireling share (`world/hirelings.md` §7.1 rule
    /// 2) of player `p`'s hireling for `attacker` killing `defender` with
    /// experience `e`.
    fn hireling_share(&mut self, p: Self::Unit, attacker: Self::Unit, defender: Self::Unit, e: i32);
    /// `0x00554630(P)` ≠ 0xFFFF: the player is in a party.
    fn in_party(&self, p: Self::Unit) -> bool;
    /// `0x005405A0` with the callback `0x0057E5A0` (§4.4 rule 6): the
    /// kept members in order (P only when P has no room or no party;
    /// else the party members on P's level, alive, within range of D).
    fn party_members(&self, p: Self::Unit, defender: Self::Unit) -> Vec<Self::Unit>;
}

/// The distribution `0x0057E990(game, A, D)` (§4.4), run by the kill
/// (`combat/damage.md` §7.2 step 2) when D lacks unit flag 0x04000000.
pub fn distribute<W: ExpShare>(w: &mut W, t: &VitalsTables, a: W::Unit, d: W::Unit) {
    // Step 1.
    let a_ty = w.unit_type(a);
    if !matches!(a_ty, UnitType::Player | UnitType::Monster) {
        return;
    }
    let e = w.base_stat(d, stat::EXPERIENCE);
    if e <= 0 {
        return;
    }
    // Step 2.
    let p = if a_ty == UnitType::Player {
        Some(a)
    } else {
        w.credited_player(a, d)
    };
    let Some(p) = p.filter(|&p| w.unit_type(p) == UnitType::Player) else {
        return;
    };
    // Step 3 (base reads).
    let dl = w.base_stat(d, stat::LEVEL);
    w.hireling_share(p, a, d, e);
    // Step 4.
    let pl = w.base_stat(p, stat::LEVEL);
    if w.in_party(p) {
        party_share(w, t, p, d, e, pl, dl);
    } else {
        let g = player_gain(w, t, e, p, pl, dl);
        add_experience_at(w, t, p, pl, g);
    }
}

/// The party share `0x0057E6C0` (§4.4 rule 6).
pub fn party_share<W: ExpShare>(
    w: &mut W,
    t: &VitalsTables,
    p: W::Unit,
    d: W::Unit,
    e: i32,
    pl: i32,
    dl: i32,
) {
    let members = w.party_members(p, d);
    assert!(
        members.len() <= MAX_PARTY,
        "party share: a 9th member (assert 0xF25)"
    );
    // Totals.
    let levels: Vec<i32> = members.iter().map(|&m| w.stat(m, stat::LEVEL)).collect();
    let n = members.len() as i32;
    let s = levels.iter().fold(0i32, |a, &l| a.wrapping_add(l));
    if n <= 0 || s <= 0 {
        return;
    }
    if n == 1 {
        let g = player_gain(w, t, e, p, pl, dl);
        add_experience_at(w, t, p, pl, g);
        return;
    }
    let total = e.wrapping_add(n.wrapping_sub(1).wrapping_mul(e).wrapping_mul(89) / 256);
    let q = party_quotient(total, s);
    for (&m, &l) in members.iter().zip(&levels) {
        let share = party_member_share(l, q);
        let g = player_gain(w, t, share, m, l, dl);
        add_experience_at(w, t, m, l, g);
    }
}

/// A float32 value as `sign · mantissa · 2^-shift` (mantissa < 2^24).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct F32 {
    pub negative: bool,
    pub mantissa: u64,
    pub shift: i32,
}

/// `t / s` (x87 `fild` / `fidiv`) stored to a float32: round to nearest,
/// ties to even. `s` > 0.
pub fn party_quotient(t: i32, s: i32) -> F32 {
    let a = u64::from(t.unsigned_abs());
    let b = u64::from(s.unsigned_abs());
    let negative = (t < 0) != (s < 0);
    if a == 0 {
        return F32 {
            negative,
            mantissa: 0,
            shift: 0,
        };
    }
    // Choose `shift` so 2^23 ≤ a · 2^shift / b < 2^24.
    let mut shift = 23 - (63 - a.leading_zeros() as i32) + (63 - b.leading_zeros() as i32);
    let scaled = |k: i32| -> (u128, u128) {
        let (num, den) = (u128::from(a), u128::from(b));
        if k >= 0 {
            (num << k, den)
        } else {
            (num, den << -k)
        }
    };
    loop {
        let (num, den) = scaled(shift);
        let m = num / den;
        if m < 1 << 23 {
            shift += 1;
        } else if m >= 1 << 24 {
            shift -= 1;
        } else {
            break;
        }
    }
    let (num, den) = scaled(shift);
    let mut m = num / den;
    let r = num % den;
    if 2 * r > den || (2 * r == den && m & 1 == 1) {
        m += 1;
    }
    if m == 1 << 24 {
        m = 1 << 23;
        shift -= 1;
    }
    F32 {
        negative,
        mantissa: m as u64,
        shift,
    }
}

/// `level × q` (`fild` × float32) truncated toward zero by `0x00682FD0`;
/// a value outside i32 gives the x87 integer indefinite 0x80000000.
pub fn party_member_share(level: i32, q: F32) -> i32 {
    let mut p = i128::from(level) * i128::from(q.mantissa);
    if q.negative {
        p = -p;
    }
    let v = if q.shift >= 0 {
        p / (1i128 << q.shift.min(126))
    } else {
        p << (-q.shift).min(64)
    };
    i32::try_from(v).unwrap_or(i32::MIN)
}

/// The gold penalty `0x005357D0` (§4.6 rule 1) as a plan: the amount to
/// drop, the new gold / goldbank (`None` = unchanged) and `goldlost`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoldPenalty {
    pub drop: Option<i32>,
    pub gold: Option<i32>,
    pub goldbank: Option<i32>,
    pub goldlost: i32,
}

/// §4.6 rule 1: level `l` (total), gold `gi` and `gs` (totals), `pvp`,
/// game type `game_type` (game +0x6A) and the stash / gold limits
/// (`0x00623460`, `0x00622E70`).
pub fn gold_penalty(
    l: i32,
    gi: i32,
    gs: i32,
    pvp: bool,
    game_type: u8,
    stash_limit: i32,
    gold_limit: i32,
) -> GoldPenalty {
    let total = gi.wrapping_add(gs);
    let mut q = l.min(20).wrapping_mul(total) / 100;
    let keep = total.wrapping_sub(q);
    let base = l.wrapping_mul(500);
    let mut out = GoldPenalty {
        drop: None,
        gold: None,
        goldbank: None,
        goldlost: 0,
    };
    let bank = |v: i32| if v < 0 || v > stash_limit { 0 } else { v };
    let gold = |v: i32| if v < 0 || v > gold_limit { 0 } else { v };
    if game_type == 3 {
        if keep < base {
            q = total.wrapping_sub(base).max(0);
        }
        q = q.min(gi);
    }
    if pvp {
        if q > gi {
            out.goldbank = Some(bank(gs.wrapping_sub(q.wrapping_sub(gi))));
            out.gold = Some(gold(q));
        }
        out.drop = Some(q);
    } else if q <= gi {
        out.drop = Some(gi.wrapping_sub(q));
        out.gold = Some(0);
    } else {
        out.goldbank = Some(bank(gs.wrapping_sub(q.wrapping_sub(gi))));
        out.gold = Some(0);
    }
    out.goldlost = q.max(0);
    out
}

/// The experience penalty `0x005359F0` (§4.6 rule 2) for player `u` with
/// `DeathExpPenalty` `penalty` of the game's difficulty (skipped by the
/// caller for a player-side killer). Returns the loss stored in client
/// +0x508 (`0x005391E0`), `None` when nothing happens; stat 13 is set.
pub fn death_experience<W: VitalsUnits>(
    w: &mut W,
    t: &VitalsTables,
    u: W::Unit,
    penalty: u32,
) -> Option<i32> {
    let c = w.class_id(u);
    let l = w.stat(u, stat::LEVEL);
    if l <= 1 {
        return None;
    }
    let lo = t.threshold(c, (l - 1) as u32);
    let hi = t.threshold(c, l as u32);
    let x = w.stat(u, stat::EXPERIENCE);
    let mut loss = penalty.wrapping_mul(hi.wrapping_sub(lo)) / 100;
    if loss == 0 {
        return None;
    }
    let mut new = (x as u32).wrapping_sub(loss);
    if new <= lo {
        loss = x.wrapping_sub(lo as i32).max(0) as u32;
        new = lo.wrapping_add(1);
    }
    let loss = loss as i32;
    assert!(loss >= 0, "experience loss < 0 (0x005391E0 assert)");
    w.set_base_stat(u, stat::EXPERIENCE, new as i32);
    Some(loss)
}

/// §4.7 rule 1: the corpse's stat 13 from the client's stored loss v
/// (`pct(v, 75, 100)`, inline: v > 0x100000 → (v / 100) × 75, else
/// v × 75 / 100; 32-bit, toward zero). The caller then clears +0x508.
pub fn corpse_experience(v: i32) -> i32 {
    pct(v, 75, 100)
}

/// §4.7 rule 2: corpse pickup `0x0057FB70` by its own player `p`
/// (`own` = the corpse's owner GUID is P's): the corpse's experience
/// goes back through the add §4.5 at P's base level. Returns the amount.
pub fn corpse_pickup<W: VitalsUnits>(
    w: &mut W,
    t: &VitalsTables,
    p: W::Unit,
    corpse: W::Unit,
    own: bool,
) -> i32 {
    if !own {
        return 0;
    }
    let x = w.stat(corpse, stat::EXPERIENCE);
    if x == 0 {
        return 0;
    }
    w.set_base_stat(corpse, stat::EXPERIENCE, 0);
    let l0 = w.base_stat(p, stat::LEVEL);
    add_experience_at(w, t, p, l0, x);
    x
}
