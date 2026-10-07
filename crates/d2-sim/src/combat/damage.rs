// Spec: specs/combat/damage.md
//! The damage record (§1), rolling (§3), totals and resistances (§4),
//! application, leech and the timed effects (§5), hit class and hit
//! recovery (§6), the two specified event functions (§8) and durability
//! (§9). Draws: the spec's §Randomness, in that order.

use super::hit::result;
use super::{pct, scale, CombatEntry, CombatTables, CombatWorld, RoomKind};
use crate::skills::{weapon_mastery, SkillTables};
use crate::units::UnitType;

/// Hit flags (record +0x00).
pub mod hitflag {
    pub const SKIP_PHYSICAL: u32 = 0x1;
    pub const SKIP_ROLL: u32 = 0x2;
    pub const LIFE_DRAIN_PRESET: u32 = 0x4;
    pub const MANA_DRAIN_PRESET: u32 = 0x8;
    pub const STAMINA_DRAIN_PRESET: u32 = 0x10;
    pub const ROLLED: u32 = 0x20;
    pub const NO_MISSILE_EVENT: u32 = 0x80;
    pub const BYPASS_UNDEAD: u32 = 0x100;
    pub const BYPASS_DEMONS: u32 = 0x200;
    pub const BYPASS_BEASTS: u32 = 0x400;
    pub const IGNORE_HOSTILITY: u32 = 0x1000;
}

/// The damage record (§1; D2MOO `D2DamageStrc`, 0x70 bytes). Amounts in
/// 1/256 points unless noted; lengths in frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DamageRecord {
    /// +0x00 hit flags ([`hitflag`]).
    pub hit_flags: u32,
    /// +0x04 result flags ([`result`]).
    pub result: u16,
    /// +0x08.
    pub physical: i32,
    /// +0x0C enhanced-damage percent.
    pub enh_pct: i32,
    /// +0x10.
    pub fire: i32,
    /// +0x14.
    pub burn: i32,
    /// +0x18.
    pub burn_len: i32,
    /// +0x1C.
    pub lightning: i32,
    /// +0x20.
    pub magic: i32,
    /// +0x24.
    pub cold: i32,
    /// +0x28 total over the length.
    pub poison: i32,
    /// +0x2C.
    pub poison_len: i32,
    /// +0x30.
    pub cold_len: i32,
    /// +0x34.
    pub freeze_len: i32,
    /// +0x38 percent (players) or amount (monsters).
    pub life_leech: i32,
    /// +0x3C.
    pub mana_leech: i32,
    /// +0x40.
    pub stamina_leech: i32,
    /// +0x44.
    pub stun_len: i32,
    /// +0x48 absorbed life.
    pub absorbed: i32,
    /// +0x4C total.
    pub total: i32,
    /// +0x54 pierce percent of damage reduction (/1024).
    pub pierce_pct: i32,
    /// +0x60 hit class: low nibble weapon class, high nibble element.
    pub hit_class: u32,
    /// +0x64 hit class fixed.
    pub hit_class_fixed: u8,
    /// +0x65 conversion element (`elemtypes` index; 0 none).
    pub conv_elem: i8,
    /// +0x68 conversion percent.
    pub conv_pct: i32,
    /// +0x6C overlay applied to the defender after a melee hit.
    pub overlay: i32,
}

// Stats.
const STR: u16 = 0;
const DEX: u16 = 2;
const LIFE: u16 = 6;
const MAXHP: u16 = 7;
const MANA: u16 = 8;
const MAXMANA: u16 = 9;
const STAMINA: u16 = 10;
const LEVEL: u16 = 12;
const MAXDAMAGE_PERCENT: u16 = 17;
const MINDAMAGE_PERCENT: u16 = 18;
const MINDAMAGE: u16 = 21;
const MAXDAMAGE: u16 = 22;
const SECONDARY_MINDAMAGE: u16 = 23;
const SECONDARY_MAXDAMAGE: u16 = 24;
const DAMAGEPERCENT: u16 = 25;
const ARMORCLASS: u16 = 31;
const NORMAL_DR: u16 = 34;
const MAGIC_DR: u16 = 35;
const DAMAGERESIST: u16 = 36;
const COLDLENGTH: u16 = 56;
const POISONMINDAM: u16 = 57;
const POISONMAXDAM: u16 = 58;
const POISONLENGTH: u16 = 59;
const LIFEDRAINMINDAM: u16 = 60;
const LIFEDRAINMAXDAM: u16 = 61;
const MANADRAINMINDAM: u16 = 62;
const MANADRAINMAXDAM: u16 = 63;
const STAMDRAINMINDAM: u16 = 64;
const STAMDRAINMAXDAM: u16 = 65;
const STUNLENGTH: u16 = 66;
const VELOCITYPERCENT: u16 = 67;
const ATTACKRATE: u16 = 68;
const OTHER_ANIMRATE: u16 = 69;
const HPREGEN: u16 = 74;
const MONSTER_PLAYERCOUNT: u16 = 100;
const POISON_OVERRIDE_LENGTH: u16 = 101;
const BYPASS_UNDEAD: u16 = 103;
const BYPASS_DEMONS: u16 = 104;
const BYPASS_BEASTS: u16 = 106;
const NORMALDAMAGE: u16 = 111;
const HALFFREEZE: u16 = 118;
const DAMAGETARGETAC: u16 = 120;
const DEMONDAMAGE_PERCENT: u16 = 121;
const UNDEADDAMAGE_PERCENT: u16 = 122;
const DEADLYSTRIKE: u16 = 141;
const CANNOTBEFROZEN: u16 = 153;
const DAMAGE_VS_MONTYPE: u16 = 180;
const FIRELENGTH: u16 = 315;
const POISON_COUNT: u16 = 326;
const PASSIVE_POIS_MASTERY: u16 = 332;
const PASSIVE_CRITICAL_STRIKE: u16 = 337;

// States.
const STATE_FREEZE: u16 = 1;
const STATE_POISON: u16 = 2;
const STATE_COLD: u16 = 11;
const STATE_STUNNED: u16 = 21;
const STATE_SANCTUARY: u16 = 47;
const STATE_UNINTERRUPTABLE: u16 = 54;
const STATE_OPENWOUNDS: u16 = 62;
const STATE_DEATH_DELAY: u16 = 92;
const STATE_SHATTER: u16 = 107;
const STATE_BURNING: u16 = 115;
const STATE_SHRINE_RESIST_FIRE: u16 = 131;
const STATE_SHRINE_RESIST_POISON: u16 = 133;

// Timer event types (`sim/tick.md` §5).
const TIMER_AITHINK: u8 = 2;
const TIMER_STATREGEN: u8 = 3;
const TIMER_REMOVESTATE: u8 = 12;

// Unit events (`events.txt`).
pub const EV_DAMAGEDINMELEE: u8 = 1;
pub const EV_DAMAGEDBYMISSILE: u8 = 2;
pub const EV_ATTACKEDINMELEE: u8 = 3;
pub const EV_DOMELEEDAMAGE: u8 = 5;
pub const EV_DOMISSILEDAMAGE: u8 = 6;
pub const EV_DOMELEEATTACK: u8 = 7;
pub const EV_KILL: u8 = 9;
pub const EV_KILLED: u8 = 10;
pub const EV_ABSORBDAMAGE: u8 = 11;

// Monster flags of `0x005A0180`.
const MON_SUPERUNIQUE: u32 = 2;
const MON_UNIQUE: u32 = 8;
const MON_UNIQUE_OR_CHAMPION: u32 = 0x0C;

/// Overlays.
const OVERLAY_LIFE_LEECH: i32 = 151;
const OVERLAY_MANA_LEECH: i32 = 152;
const OVERLAY_CRUSHING_BLOW: i32 = 147;
const OVERLAY_MONSTER_CRIT: i32 = 54;

/// A record field accessor.
type Field = fn(&mut DamageRecord) -> &mut i32;
/// A record field reader.
type Read = fn(&DamageRecord) -> i32;

fn is_monster_not_hireling<W: CombatWorld>(w: &W, u: W::Unit) -> bool {
    w.unit_type(u) == UnitType::Monster && !w.is_hireling(u)
}

// ---------------------------------------------------------------- §3

/// `roll_in_range(unit, min, max, minpct, maxpct, current)` =
/// `0x0057A880` (§3.3).
pub fn roll_in_range<W: CombatWorld>(
    w: &mut W,
    u: W::Unit,
    min: i32,
    max: i32,
    minpct: i32,
    maxpct: i32,
    current: i32,
) -> i32 {
    let mut cur = current;
    if max > 0 {
        let min = min.wrapping_add(pct(min, minpct, 100));
        let max = max.wrapping_add(pct(max, maxpct, 100));
        cur = cur.wrapping_add(min);
        if max > min {
            cur = cur.wrapping_add(w.seed(u).roll(max.wrapping_sub(min)) as i32);
        }
    }
    cur.max(0)
}

/// `element(maxstat, minstat, mastery, minpct, maxpct, current)` =
/// `0x0057A8E0` (§3.3). Stats through the unit getter.
#[allow(clippy::too_many_arguments)]
pub fn element<W: CombatWorld>(
    w: &mut W,
    u: W::Unit,
    maxstat: u16,
    minstat: u16,
    mastery: Option<u16>,
    minpct: i32,
    maxpct: i32,
    current: i32,
) -> i32 {
    let max = w.stat(u, maxstat, 0).wrapping_shl(8);
    if max < 8 {
        return current.max(0);
    }
    let min = w.stat(u, minstat, 0).wrapping_shl(8);
    let (mut lo, mut hi) = (minpct, maxpct);
    if let Some(m) = mastery {
        let v = w.stat(u, m, 0);
        lo = lo.wrapping_add(v);
        hi = hi.wrapping_add(v);
    }
    roll_in_range(w, u, min, max, lo, hi, current)
}

/// `bonuses(unit, get, item, min, max, pct, current, s)` = `0x0057B420`
/// (§3.2). With `get`, `item = None` means the current weapon. `s` is
/// `SrcDam` (128 = unscaled).
#[allow(clippy::too_many_arguments)]
pub fn bonuses<W: CombatWorld>(
    w: &mut W,
    st: &SkillTables,
    u: W::Unit,
    get: bool,
    item: Option<W::Item>,
    min: i32,
    max: i32,
    pct_in: i32,
    current: i32,
    s: i32,
) -> i32 {
    let mut item = item;
    let (mut min, mut max) = (min, max);
    if get {
        if item.is_none() {
            item = w.current_weapon(u);
        }
        match item {
            Some(i) if w.wield_type(i) == 2 => {
                min = w.stat(u, SECONDARY_MINDAMAGE, 0);
                max = w.stat(u, SECONDARY_MAXDAMAGE, 0);
            }
            Some(_) => {
                min = w.stat(u, MINDAMAGE, 0);
                max = w.stat(u, MAXDAMAGE, 0);
            }
            None => {
                min = w.stat(u, MINDAMAGE, 0).max(1);
                max = w.stat(u, MAXDAMAGE, 0).max(2);
            }
        }
        min = min.wrapping_shl(8);
        max = max.wrapping_shl(8);
    }
    // TODO(damage.md §3.2): the getter of `item_normaldamage` is
    // "item/skill" per the rule text; stats 0, 2, 17, 18, 21–25 use the
    // unit getter.
    let n = w.item_stat(u, NORMALDAMAGE, 0).wrapping_shl(8);
    min = min.wrapping_add(n);
    max = max.wrapping_add(n);
    if min < 1 {
        min = 256;
    }
    if max <= min {
        max = min.wrapping_add(256);
    }
    let mut p = pct_in.wrapping_add(w.stat(u, DAMAGEPERCENT, 0));
    if let Some(i) = item {
        let (sb, db) = w.str_dex_bonus(i);
        if sb != 0 {
            p = p.wrapping_add(w.stat(u, STR, 0).wrapping_mul(sb) / 100);
        }
        if db != 0 {
            p = p.wrapping_add(w.stat(u, DEX, 0).wrapping_mul(db) / 100);
        }
        p = p.wrapping_add(weapon_mastery(w, st, Some(u), Some(i), None, 1));
    } else if get {
        p = p.wrapping_add(w.stat(u, STR, 0));
    }
    p = p.max(-90);
    let mut cur = current;
    if max > 0 {
        let min_t = min.wrapping_add(pct(
            min,
            p.wrapping_add(w.stat(u, MINDAMAGE_PERCENT, 0)),
            100,
        ));
        let max_t = max.wrapping_add(pct(
            max,
            p.wrapping_add(w.stat(u, MAXDAMAGE_PERCENT, 0)),
            100,
        ));
        cur = cur.wrapping_add(min_t);
        if max_t > min_t {
            cur = cur.wrapping_add(w.seed(u).roll(max_t.wrapping_sub(min_t)) as i32);
        }
    }
    cur = cur.max(0);
    if s != 128 {
        cur = pct(cur, s, 128);
    }
    cur
}

/// `fill(game, attacker, defender, record, offhand, s)` = `0x0057B7D0`
/// (§3.1). `s = 0` is treated as 128.
#[allow(clippy::too_many_arguments)]
pub fn fill<W: CombatWorld>(
    w: &mut W,
    st: &SkillTables,
    ct: &CombatTables,
    a: W::Unit,
    d: W::Unit,
    rec: &mut DamageRecord,
    offhand: bool,
    s: i32,
) {
    let s = if s == 0 { 128 } else { s };
    w.dual_wield_switch(a, offhand, true);
    rec.hit_flags |= hitflag::ROLLED;
    let a_type = w.unit_type(a);
    let d_mon = w.unit_type(d) == UnitType::Monster;
    // Step 3.
    if (a_type == UnitType::Player || w.alignment(a) == 2) && d_mon {
        let t = w.item_stat(a, DAMAGETARGETAC, 0);
        if t != 0 {
            let ac = w.stat(d, ARMORCLASS, 0).wrapping_add(t).max(0);
            w.set_stat(d, ARMORCLASS, ac);
        }
        if w.is_demon(d) {
            let p = w.item_stat(a, DEMONDAMAGE_PERCENT, 0);
            if p > 0 {
                rec.enh_pct = rec.enh_pct.wrapping_add(p);
            }
        }
        // `0x0057B660`.
        if w.is_undead(d) {
            let blunt = w.current_weapon(a).is_some_and(|i| w.item_is(i, 57));
            let p = (if blunt { 50i32 } else { 0 }).wrapping_add(w.item_stat(
                a,
                UNDEADDAMAGE_PERCENT,
                0,
            ));
            if p > 0 {
                rec.enh_pct = rec.enh_pct.wrapping_add(p);
            }
        }
        // `0x0057B6A0`.
        let mb = super::hit::montype_bonus(w, ct, a, d, DAMAGE_VS_MONTYPE);
        rec.enh_pct = rec.enh_pct.wrapping_add(mb);
    }
    // Step 4.
    if rec.hit_flags & hitflag::SKIP_PHYSICAL == 0 {
        rec.physical = bonuses(w, st, a, true, None, 0, 0, rec.enh_pct, rec.physical, s);
        let mut crit = false;
        let weapon = w.current_weapon(a);
        if !offhand && weapon.is_some() {
            let m = weapon_mastery(w, st, Some(a), weapon, None, 2);
            if m > 0 {
                crit = w.seed(a).roll_range(0, 100) < m;
            }
        }
        if !crit {
            let c = w.stat(a, PASSIVE_CRITICAL_STRIKE, 0);
            if c > 0 {
                crit = w.seed(a).roll_range(0, 100) < c;
            }
        }
        if !crit {
            let ds = w.item_stat(a, DEADLYSTRIKE, 0);
            if ds > 0 {
                crit = w.seed(a).roll_range(0, 100) < ds;
            }
        }
        if crit {
            rec.physical = rec.physical.wrapping_mul(2);
            rec.result |= result::CRITICAL;
        }
    }
    // Step 5: fire, lightning, cold, magic.
    let elems: [(u16, u16, u16, Field); 4] = [
        (49, 48, 329, |r| &mut r.fire),
        (51, 50, 330, |r| &mut r.lightning),
        (55, 54, 331, |r| &mut r.cold),
        (53, 52, 357, |r| &mut r.magic),
    ];
    for (maxs, mins, mastery, field) in elems {
        let cur = *field(rec);
        let mut v = element(w, a, maxs, mins, Some(mastery), 0, 0, cur);
        if s != 128 {
            v = scale(v, s);
        }
        *field(rec) = v;
    }
    // Step 6: drains.
    if is_monster_not_hireling(w, a) {
        if rec.hit_flags & hitflag::LIFE_DRAIN_PRESET == 0 {
            let v = element(
                w,
                a,
                LIFEDRAINMAXDAM,
                LIFEDRAINMINDAM,
                None,
                0,
                0,
                rec.life_leech,
            );
            rec.life_leech = rec.life_leech.wrapping_add(scale(v, s));
        }
        if rec.hit_flags & hitflag::MANA_DRAIN_PRESET == 0 {
            let v = element(
                w,
                a,
                MANADRAINMAXDAM,
                MANADRAINMINDAM,
                None,
                0,
                0,
                rec.mana_leech,
            );
            rec.mana_leech = rec.mana_leech.wrapping_add(scale(v, s));
        }
        if rec.hit_flags & hitflag::STAMINA_DRAIN_PRESET == 0 {
            let v = element(
                w,
                a,
                STAMDRAINMAXDAM,
                STAMDRAINMINDAM,
                None,
                0,
                0,
                rec.stamina_leech,
            );
            rec.stamina_leech = rec.stamina_leech.wrapping_add(scale(v, s));
        }
    } else if a_type == UnitType::Player || (a_type == UnitType::Monster && w.is_hireling(a)) {
        if rec.hit_flags & hitflag::LIFE_DRAIN_PRESET == 0 {
            rec.life_leech = rec.life_leech.wrapping_add(w.stat(a, LIFEDRAINMINDAM, 0));
        }
        if rec.hit_flags & hitflag::MANA_DRAIN_PRESET == 0 {
            rec.mana_leech = rec.mana_leech.wrapping_add(w.stat(a, MANADRAINMINDAM, 0));
        }
        // Unit getter `0x00625480(attacker, s, 0)` (§3.1 step 6).
        if w.stat(a, BYPASS_UNDEAD, 0) != 0 {
            rec.hit_flags |= hitflag::BYPASS_UNDEAD;
        }
        if w.stat(a, BYPASS_DEMONS, 0) != 0 {
            rec.hit_flags |= hitflag::BYPASS_DEMONS;
        }
        if w.stat(a, BYPASS_BEASTS, 0) != 0 {
            rec.hit_flags |= hitflag::BYPASS_BEASTS;
        }
    }
    // Step 7: poison.
    let pmin = w.stat(a, POISONMINDAM, 0);
    let pmax = w.stat(a, POISONMAXDAM, 0);
    let m = if pmax > 0 {
        w.stat(a, PASSIVE_POIS_MASTERY, 0)
    } else {
        0
    };
    let v = roll_in_range(w, a, pmin, pmax, m, m, rec.poison);
    rec.poison = scale(v, s);
    if rec.poison != 0 {
        let o = w.stat(a, POISON_OVERRIDE_LENGTH, 0);
        if o > 0 {
            rec.poison_len = rec.poison_len.wrapping_add(o);
        } else {
            rec.poison_len = rec.poison_len.wrapping_add(w.stat(a, POISONLENGTH, 0));
            let n = w.stat(a, POISON_COUNT, 0);
            if n > 1 {
                rec.poison_len /= n;
            }
        }
    }
    // Step 8–9.
    if rec.cold > 0 {
        rec.cold_len = rec
            .cold_len
            .wrapping_add(scale(w.stat(a, COLDLENGTH, 0), s));
    }
    if rec.stun_len == 0 {
        rec.stun_len = rec
            .stun_len
            .wrapping_add(scale(w.stat(a, STUNLENGTH, 0), s));
    }
    // Step 10: burn (Edge case 5).
    let r1 = w.seed(a).roll(1) as i32;
    let b = rec
        .burn
        .wrapping_mul(s)
        .wrapping_add(316)
        .wrapping_add(r1)
        .max(0);
    rec.burn = rec.burn.wrapping_add(b / 128);
    if rec.burn_len != 0 {
        rec.burn_len = rec
            .burn_len
            .wrapping_add(scale(w.stat(a, FIRELENGTH, 0), s));
    }
    // Step 11.
    let player_or_hireling =
        a_type == UnitType::Player || (a_type == UnitType::Monster && w.is_hireling(a));
    if player_or_hireling && rec.result & result::NO_EVENTS == 0 {
        w.unit_event(EV_ATTACKEDINMELEE, d, a, rec);
    }
    // Step 12: conversion.
    if rec.conv_elem > 0 {
        let c = pct(rec.physical, rec.conv_pct, 100);
        rec.physical = rec.physical.wrapping_sub(c).max(0);
        let mut e = i32::from(rec.conv_elem);
        if e == 10 {
            e = w.seed(a).roll(5) as i32 + 1;
        }
        match e {
            1 => rec.fire = rec.fire.wrapping_add(c),
            2 => rec.lightning = rec.lightning.wrapping_add(c),
            3 => rec.magic = rec.magic.wrapping_add(c),
            4 => {
                rec.cold = rec.cold.wrapping_add(c);
                rec.cold_len = rec.cold_len.max(50);
            }
            5 => {
                rec.poison = rec.poison.wrapping_add(c / 8);
                rec.poison_len = rec.poison_len.max(50);
            }
            11 => {
                rec.fire = rec.fire.wrapping_add(c);
                rec.burn_len = rec.burn_len.max(50);
            }
            12 => {
                rec.cold = rec.cold.wrapping_add(c);
                rec.freeze_len = rec.freeze_len.max(50);
            }
            _ => {}
        }
    }
    // Step 13: monster critical hit `0x005A5560`.
    monster_crit(w, ct, a, d, rec);
    w.dual_wield_switch(a, offhand, false);
}

/// Monster critical hit `0x005A5560` (§3.1 step 13; also the missile
/// area hit, `missiles.md` §R9.6): a monster attacker's `crit` chance
/// (one `lo' % 100` on its seed) doubles the damage fields and marks the
/// hit class.
pub fn monster_crit<W: CombatWorld>(
    w: &mut W,
    ct: &CombatTables,
    a: W::Unit,
    d: W::Unit,
    rec: &mut DamageRecord,
) {
    if w.unit_type(a) == UnitType::Monster {
        let crit = ct.monstats(w.class_id(a)).map_or(0, |m| i32::from(m.crit));
        if crit != 0 && ((w.seed(a).step() % 100) as i32) < crit {
            for f in [
                &mut rec.physical,
                &mut rec.fire,
                &mut rec.lightning,
                &mut rec.magic,
                &mut rec.cold,
                &mut rec.poison,
            ] {
                *f = f.wrapping_mul(2);
            }
            // `0x00554650(record, 0x10)`: `(+0x60 & 0xF0) ≠ 0` writes
            // nothing (§3.1 step 13).
            if rec.hit_class & 0xF0 == 0 {
                rec.hit_class |= 0x10;
                w.overlay(d, OVERLAY_MONSTER_CRIT);
            }
        }
    }
}

/// `start_combat(game, attacker, defender, record, SrcDam)` =
/// `0x0057DBF0` (§3): roll on a hit, then prepend a copy to the
/// attacker's combat list.
pub fn start_combat<W: CombatWorld>(
    w: &mut W,
    st: &SkillTables,
    ct: &CombatTables,
    a: Option<W::Unit>,
    d: Option<W::Unit>,
    rec: &mut DamageRecord,
    srcdam: i32,
) {
    let (Some(a), Some(d)) = (a, d) else {
        return;
    };
    const AVOIDED: u16 = result::DODGE | result::AVOID | result::EVADE | result::WEAPON_BLOCK;
    if rec.result & result::HIT != 0 && rec.result & AVOIDED == 0 {
        if rec.hit_flags & hitflag::SKIP_ROLL == 0 {
            fill(w, st, ct, a, d, rec, false, srcdam);
        }
        totals(w, ct, Some(a), d, rec);
        let mut t = rec
            .physical
            .wrapping_add(rec.fire)
            .wrapping_add(rec.lightning)
            .wrapping_add(rec.magic)
            .wrapping_add(rec.cold)
            .wrapping_add(rec.poison);
        if w.unit_type(a) == UnitType::Monster {
            t = t.wrapping_add(rec.life_leech);
        }
        if (t & !0xFF) > (w.stat(d, LIFE, 0) & !0xFF) {
            rec.result |= result::WILL_DIE;
        }
    }
    // `0x0057CA80`.
    if matches!(w.unit_type(d), UnitType::Player | UnitType::Monster) {
        let entry = CombatEntry {
            attacker: w.ident(a),
            defender: w.ident(d),
            record: *rec,
        };
        w.combat_list(a).insert(0, entry);
    }
}

// ---------------------------------------------------------------- §4

/// One row of the resistance table `0x00732980` (§4.3).
#[derive(Debug, Clone, Copy)]
struct ResRow {
    field: Field,
    resist: Option<u16>,
    max: Option<u16>,
    pierce: Option<u16>,
    absorb_pct: Option<u16>,
    absorb_flat: Option<u16>,
    dr: Dr,
    leech: bool,
    scaled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dr {
    Normal,
    Magic,
    None,
}

#[allow(clippy::too_many_arguments)]
const fn row(
    field: Field,
    resist: Option<u16>,
    max: Option<u16>,
    pierce: Option<u16>,
    absorb: Option<(u16, u16)>,
    dr: Dr,
    leech: bool,
    scaled: bool,
) -> ResRow {
    let (absorb_pct, absorb_flat) = match absorb {
        Some((p, f)) => (Some(p), Some(f)),
        None => (None, None),
    };
    ResRow {
        field,
        resist,
        max,
        pierce,
        absorb_pct,
        absorb_flat,
        dr,
        leech,
        scaled,
    }
}

/// §4.3, rows 0–11.
const RES_ROWS: [ResRow; 12] = [
    row(
        |r| &mut r.physical,
        Some(36),
        None,
        None,
        None,
        Dr::Normal,
        false,
        true,
    ),
    row(
        |r| &mut r.fire,
        Some(39),
        Some(40),
        Some(333),
        Some((142, 143)),
        Dr::Magic,
        false,
        true,
    ),
    row(
        |r| &mut r.lightning,
        Some(41),
        Some(42),
        Some(334),
        Some((144, 145)),
        Dr::Magic,
        false,
        true,
    ),
    row(
        |r| &mut r.cold,
        Some(43),
        Some(44),
        Some(335),
        Some((148, 149)),
        Dr::Magic,
        false,
        true,
    ),
    row(
        |r| &mut r.magic,
        Some(37),
        Some(38),
        None,
        Some((146, 147)),
        Dr::Magic,
        false,
        true,
    ),
    row(
        |r| &mut r.cold_len,
        Some(43),
        Some(44),
        Some(335),
        None,
        Dr::None,
        false,
        false,
    ),
    row(
        |r| &mut r.freeze_len,
        Some(43),
        Some(44),
        Some(335),
        None,
        Dr::None,
        false,
        false,
    ),
    row(
        |r| &mut r.poison_len,
        Some(110),
        None,
        Some(336),
        None,
        Dr::None,
        false,
        false,
    ),
    row(
        |r| &mut r.poison,
        Some(45),
        Some(46),
        Some(336),
        None,
        Dr::None,
        false,
        true,
    ),
    row(
        |r| &mut r.life_leech,
        None,
        None,
        None,
        None,
        Dr::None,
        true,
        true,
    ),
    row(
        |r| &mut r.mana_leech,
        None,
        None,
        None,
        None,
        Dr::None,
        true,
        true,
    ),
    row(
        |r| &mut r.stamina_leech,
        None,
        None,
        None,
        None,
        Dr::None,
        true,
        true,
    ),
];

/// Damage percent `0x0057C060` (§4.2).
pub fn damage_percent<W: CombatWorld>(
    w: &W,
    ct: &CombatTables,
    a: Option<W::Unit>,
    d: W::Unit,
) -> i32 {
    let Some(a) = a else {
        return 100;
    };
    if a == d {
        return 100;
    }
    let a_hire = w.unit_type(a) == UnitType::Monster && w.is_hireling(a);
    let d_hire = w.unit_type(d) == UnitType::Monster && w.is_hireling(d);
    if w.unit_type(d) == UnitType::Player
        && (w.unit_type(a) == UnitType::Player || a_hire || w.is_revived(a))
    {
        17
    } else if d_hire && a_hire {
        25
    } else if w.is_boss(d) && a_hire {
        ct.difficulty(w.difficulty())
            .map_or(100, |r| r.hireablebossdamagepercent as i32)
    } else if w.is_revived(d) && w.is_prime_evil(a) {
        if d_hire {
            200
        } else {
            400
        }
    } else {
        100
    }
}

/// Resistance value `0x0057BE00` (§4.5).
fn resist_value<W: CombatWorld>(
    w: &W,
    ct: &CombatTables,
    a: Option<W::Unit>,
    d: W::Unit,
    row: &ResRow,
    def_mon: bool,
) -> i32 {
    let mut r = row.resist.map_or(0, |s| w.stat(d, s, 0));
    if let Some(p) = row.pierce {
        if r < 100 || !def_mon {
            r = r.wrapping_sub(a.map_or(0, |a| w.stat(a, p, 0)));
        }
    }
    // Leech rows (no resist stat, −1) are "not 36 or 37" and take the
    // penalty (§4.5).
    if !def_mon && row.resist != Some(DAMAGERESIST) && row.resist != Some(37) {
        if w.expansion() {
            let pen = ct
                .difficulty(w.difficulty())
                .map_or(0, |x| x.resistpenalty as i32);
            r = r.wrapping_add(pen);
        } else {
            match w.difficulty() {
                1 => r = r.wrapping_sub(20),
                2 => r = r.wrapping_sub(50),
                _ => {}
            }
        }
    }
    if r > 0 {
        if !def_mon {
            let cap = match row.max {
                Some(m) => w.stat(d, m, 0).wrapping_add(75).min(95),
                None if row.resist == Some(DAMAGERESIST) => 50,
                None => 75,
            };
            r = r.min(cap);
        }
        if row.resist == Some(DAMAGERESIST)
            && a.is_some_and(|a| w.has_state(a, STATE_SANCTUARY))
            && w.is_undead(d)
        {
            r = 0;
        }
    } else {
        r = r.max(-100);
    }
    r
}

/// `totals(game, attacker, defender, record)` = `0x0057C1E0` (§4).
pub fn totals<W: CombatWorld>(
    w: &mut W,
    ct: &CombatTables,
    a: Option<W::Unit>,
    d: W::Unit,
    rec: &mut DamageRecord,
) {
    let att_mon = a.is_some_and(|a| is_monster_not_hireling(w, a));
    let def_mon = is_monster_not_hireling(w, d);
    // §4.1.
    let mut dr_normal = w.stat(d, NORMAL_DR, 0).wrapping_shl(8);
    if dr_normal > 0 && rec.pierce_pct > 0 {
        dr_normal = pct(dr_normal, rec.pierce_pct, 1024);
    }
    let mut dr_magic = w.stat(d, MAGIC_DR, 0).wrapping_shl(8);
    if dr_magic > 0 && rec.pierce_pct > 0 {
        dr_magic = pct(dr_magic, rec.pierce_pct, 1024);
    }
    // Step 1.
    let p = damage_percent(w, ct, a, d);
    if p != 100 {
        for row in RES_ROWS.iter().filter(|r| r.scaled) {
            let f = (row.field)(rec);
            if *f > 0 {
                *f = pct(*f, p, 100);
            }
        }
    }
    // Step 2.
    if let Some(a) = a {
        w.unit_event(EV_ABSORBDAMAGE, d, a, rec);
    }
    // Step 3: `0x0057C140`.
    if rec.cold_len > 0 || rec.freeze_len > 0 {
        if w.item_stat(d, CANNOTBEFROZEN, 0) != 0 {
            rec.cold_len = 0;
            rec.freeze_len = 0;
        } else if w.item_stat(d, HALFFREEZE, 0) != 0 {
            rec.cold_len /= 2;
            rec.freeze_len /= 2;
        }
    }
    if rec.poison_len > 0 && w.has_state(d, STATE_SHRINE_RESIST_POISON) {
        rec.poison_len = 0;
    }
    if rec.burn_len > 0 && w.has_state(d, STATE_SHRINE_RESIST_FIRE) {
        rec.burn_len = 0;
    }
    // Step 4.
    let hf = rec.hit_flags;
    let no_absorb = if w.unit_type(d) == UnitType::Monster {
        let undead = w.is_undead(d);
        let demon = w.is_demon(d);
        (undead && hf & hitflag::BYPASS_UNDEAD != 0)
            || (demon && hf & hitflag::BYPASS_DEMONS != 0)
            || (!undead && !demon && hf & hitflag::BYPASS_BEASTS != 0)
    } else {
        hf & hitflag::BYPASS_BEASTS != 0
    };
    // Step 5.
    for row in &RES_ROWS {
        if row.leech && !att_mon {
            break;
        }
        // §4.6 `0x0057BF80`.
        let mut v = *(row.field)(rec);
        if v <= 0 {
            *(row.field)(rec) = 0;
            continue;
        }
        let mut r = resist_value(w, ct, a, d, row, def_mon);
        if no_absorb {
            r = r.min(0);
        } else {
            v = v.wrapping_sub(match row.dr {
                Dr::Normal => dr_normal,
                Dr::Magic => dr_magic,
                Dr::None => 0,
            });
        }
        if v > 0 && r != 0 {
            v = pct(v, 100 - r.min(100), 100);
        }
        if !no_absorb {
            if let (Some(ap), Some(af)) = (row.absorb_pct, row.absorb_flat) {
                let ab = w.stat(d, ap, 0).min(40);
                if ab > 0 {
                    let x = pct(v, ab, 100);
                    rec.absorbed = rec.absorbed.wrapping_add(x);
                    v = v.wrapping_sub(x);
                }
                let f = w.stat(d, af, 0).wrapping_shl(8);
                if f > 0 {
                    let x = f.min(v);
                    rec.absorbed = rec.absorbed.wrapping_add(x);
                    v = v.wrapping_sub(x);
                }
            }
        }
        *(row.field)(rec) = v;
    }
    // Step 6.
    let mut t = rec
        .physical
        .wrapping_add(rec.fire)
        .wrapping_add(rec.lightning)
        .wrapping_add(rec.magic)
        .wrapping_add(rec.cold)
        .wrapping_add(rec.poison);
    if att_mon {
        t = t.wrapping_add(rec.life_leech);
    }
    rec.total = t;
}

// ---------------------------------------------------------------- §5

/// Rule H `heal(unit, x)` = `0x0057A980` (§5.3): the life added.
pub fn heal<W: CombatWorld>(w: &mut W, u: W::Unit, x: i32) -> i32 {
    if x <= 0 || w.is_dead(u) || w.has_state(u, STATE_DEATH_DELAY) {
        return 0;
    }
    let life = w.stat(u, LIFE, 0);
    let new = life.wrapping_add(x).min(w.stat(u, MAXHP, 0));
    w.set_stat(u, LIFE, new);
    new.wrapping_sub(life)
}

/// Rule M `add_mana(unit, x)` = `0x0057AA00` (§5.3).
pub fn add_mana<W: CombatWorld>(w: &mut W, u: W::Unit, x: i32) -> i32 {
    if x <= 0 {
        return 0;
    }
    let mana = w.stat(u, MANA, 0);
    let new = mana.wrapping_add(x).min(w.stat(u, MAXMANA, 0));
    w.set_stat(u, MANA, new);
    new.wrapping_sub(mana)
}

/// Leech `0x0057C420` (§5.3).
pub fn leech<W: CombatWorld>(
    w: &mut W,
    ct: &CombatTables,
    a: Option<W::Unit>,
    d: W::Unit,
    rec: &mut DamageRecord,
) {
    if rec.life_leech == 0 && rec.mana_leech == 0 {
        return;
    }
    let diff = w.difficulty();
    let drain = if w.unit_type(d) == UnitType::Monster {
        let m = ct.monstats(w.class_id(d));
        let v = m.map_or(0, |m| match diff {
            0 => m.drain,
            1 => m.drain_n,
            _ => m.drain_h,
        });
        let v = i32::from(v);
        if v <= 0 {
            return;
        }
        v
    } else {
        100
    };
    rec.life_leech = rec.life_leech.wrapping_shl(6);
    rec.mana_leech = rec.mana_leech.wrapping_shl(6);
    let Some(a) = a else {
        // TODO(damage.md §5.3 step 3): "or none" takes the mode
        // conversion of a missing attacker; nothing to leech into.
        return;
    };
    let player_rule = match w.unit_type(a) {
        UnitType::Player => {
            if let Some(dl) = ct.difficulty(diff) {
                let (ld, md) = (dl.lifestealdivisor as i32, dl.manastealdivisor as i32);
                if ld != 0 {
                    rec.life_leech /= ld;
                }
                if md != 0 {
                    rec.mana_leech /= md;
                }
            }
            true
        }
        UnitType::Monster if w.is_hireling(a) => true,
        _ => w.converted_type(a) == 0,
    };
    if player_rule {
        if rec.physical <= 0 {
            return;
        }
        let mana = rec.mana_leech != 0;
        if mana {
            let mut m = pct(rec.physical, rec.mana_leech, 100);
            if drain != 100 {
                m = pct(m, drain, 100);
            }
            add_mana(w, a, m / 64);
        }
        if rec.life_leech != 0 {
            let mut l = pct(rec.physical, rec.life_leech, 100);
            if drain != 100 {
                l = pct(l, drain, 100);
            }
            heal(w, a, l / 64);
            let o = if mana && w.seed(a).roll(2) != 0 {
                OVERLAY_MANA_LEECH
            } else {
                OVERLAY_LIFE_LEECH
            };
            w.overlay(a, o);
        } else if mana {
            w.overlay(a, OVERLAY_MANA_LEECH);
        }
    } else {
        rec.life_leech /= 64;
        rec.mana_leech /= 64;
        let l = rec.life_leech.min(rec.physical);
        let m = rec.mana_leech.min(w.stat(d, MANA, 0));
        let s = rec.stamina_leech.min(w.stat(d, STAMINA, 0));
        let mut t = l.wrapping_add(m).wrapping_add(s);
        if t == 0 {
            return;
        }
        if drain != 100 {
            t = pct(t, drain, 100);
        }
        let life = w.stat(a, LIFE, 0);
        t = t.min(w.stat(a, MAXHP, 0).wrapping_sub(life));
        if t > 0 {
            w.set_stat(a, LIFE, life.wrapping_add(t));
            w.overlay(a, OVERLAY_LIFE_LEECH);
        }
    }
}

/// Stun `0x0057AAE0` (§5.5).
pub fn stun<W: CombatWorld>(w: &mut W, ct: &CombatTables, a: W::Unit, d: W::Unit, n: i32) {
    if n <= 0 {
        return;
    }
    let mut n = n;
    if w.unit_type(d) == UnitType::Monster {
        if w.monster_flag(d, MON_UNIQUE) && ((w.seed(a).step() % 100) as i32) < 90 {
            return;
        }
        if w.is_boss(d) {
            return;
        }
        if ct.monstats(w.class_id(d)).is_some_and(|m| m.velocity == 0) {
            return;
        }
        n = if w.is_hireling(d) && n >= 13 {
            13
        } else {
            n.min(250)
        };
    } else {
        n = n.min(250);
    }
    let e = w.frame().wrapping_add(n);
    if w.state_list_expiry(d, STATE_STUNNED).is_some() {
        w.set_state_list_expiry(d, STATE_STUNNED, e);
        w.schedule_timer(d, TIMER_REMOVESTATE, e);
    } else {
        w.create_state_list(d, STATE_STUNNED, a, e);
        w.schedule_timer(d, TIMER_REMOVESTATE, e);
        w.set_state(d, STATE_STUNNED, true);
    }
}

/// The defender's cold effect (`monstats.ColdEffect[difficulty]`, i8;
/// −50 for others).
fn cold_effect<W: CombatWorld>(w: &W, ct: &CombatTables, d: W::Unit) -> i32 {
    if w.unit_type(d) != UnitType::Monster {
        return -50;
    }
    ct.monstats(w.class_id(d)).map_or(0, |m| {
        i32::from(match w.difficulty() {
            0 => m.coldeffect,
            1 => m.coldeffect_n,
            _ => m.coldeffect_h,
        } as i8)
    })
}

/// Cold `0x0057AF80(attacker, length)` (§5.6).
pub fn cold<W: CombatWorld>(w: &mut W, ct: &CombatTables, a: W::Unit, d: W::Unit, length: i32) {
    if length <= 0 {
        return;
    }
    let effect = cold_effect(w, ct, d);
    if effect == 0 {
        return;
    }
    let mon = w.unit_type(d) == UnitType::Monster;
    let mut length = length;
    if mon && effect < 0 {
        let div = ct
            .difficulty(w.difficulty())
            .map_or(0, |x| x.monstercolddivisor as i32);
        if div != 0 {
            length /= div;
        }
    }
    length = length.max(1);
    let e = w.frame().wrapping_add(length);
    match w.state_list_expiry(d, STATE_COLD) {
        None => {
            w.set_state(d, STATE_COLD, true);
            w.create_state_list(d, STATE_COLD, a, e);
            w.schedule_timer(d, TIMER_REMOVESTATE, e);
            for s in [VELOCITYPERCENT, ATTACKRATE, OTHER_ANIMRATE] {
                w.set_state_list_stat(d, STATE_COLD, s, effect);
            }
            w.refresh_anim_rate(d);
        }
        Some(x) if x < e => {
            w.set_state_list_expiry(d, STATE_COLD, e);
            w.schedule_timer(d, TIMER_REMOVESTATE, e);
        }
        Some(_) => {}
    }
    // Shatter.
    let r = (w.seed(d).step() % 100) as i32;
    let dead_col = ct.monstats2(w.class_id(d)).is_some_and(|m| m.deadcol);
    w.set_state(d, STATE_SHATTER, r < 20 && mon && !dead_col);
}

/// Freeze `0x0057B230(defender, attacker, length)` (§5.7). Panics when a
/// monster is frozen with `MonsterFreezeDivisor = 0` (fatal assertion).
pub fn freeze<W: CombatWorld>(w: &mut W, ct: &CombatTables, d: W::Unit, a: W::Unit, length: i32) {
    if length <= 0 {
        return;
    }
    match w.unit_type(d) {
        UnitType::Player => return cold(w, ct, a, d, length),
        UnitType::Monster => {}
        _ => return,
    }
    if w.has_state(d, STATE_UNINTERRUPTABLE) {
        return;
    }
    if w.is_boss(d) || w.monster_flag(d, MON_UNIQUE) || w.is_hireling(d) {
        return cold(w, ct, a, d, length);
    }
    if cold_effect(w, ct, d) >= 0 {
        return;
    }
    let div = ct
        .difficulty(w.difficulty())
        .map_or(0, |x| x.monsterfreezedivisor as i32);
    assert!(div != 0, "MonsterFreezeDivisor 0 (damage.md §5.7)");
    let length = length / div;
    let e = w.frame().wrapping_add(length);
    let expiry = match w.state_list_expiry(d, STATE_FREEZE) {
        Some(x) => {
            let k = x.max(e);
            w.set_state_list_expiry(d, STATE_FREEZE, k);
            k
        }
        None => {
            // TODO(damage.md §5.7): "a new list with state 1"; switching
            // the state on is read from the poison/stun rules.
            w.set_state(d, STATE_FREEZE, true);
            w.create_state_list(d, STATE_FREEZE, a, e);
            e
        }
    };
    w.schedule_timer(d, TIMER_REMOVESTATE, expiry);
    w.cancel_timers(d, TIMER_AITHINK);
    w.schedule_timer(
        d,
        TIMER_AITHINK,
        w.frame().wrapping_add(length).wrapping_add(1),
    );
}

/// Poison `0x0057AC50` (state 2) and burn `0x0057ADD0` (state 115)
/// (§5.8).
fn dot<W: CombatWorld>(w: &mut W, a: W::Unit, d: W::Unit, state: u16, damage: i32, length: i32) {
    if length <= 0 || damage <= 0 {
        return;
    }
    if w.unit_type(d) == UnitType::Monster {
        w.cancel_timers(d, TIMER_STATREGEN);
        w.schedule_timer(d, TIMER_STATREGEN, w.frame().wrapping_add(1));
    }
    let e = w.frame().wrapping_add(length);
    if w.state_list_expiry(d, state).is_some() {
        let regen = w.state_stat(d, state, HPREGEN).unwrap_or(0);
        if regen.wrapping_neg() <= damage {
            w.set_state_list_expiry(d, state, e);
            w.schedule_timer(d, TIMER_REMOVESTATE, e);
            w.set_state_list_stat(d, state, HPREGEN, damage.wrapping_neg());
        }
    } else {
        w.set_state(d, state, true);
        w.create_state_list(d, state, a, e);
        w.set_state_list_stat(d, state, HPREGEN, damage.wrapping_neg());
        w.schedule_timer(d, TIMER_REMOVESTATE, e);
    }
}

/// Poison `0x0057AC50` (§5.8).
pub fn poison<W: CombatWorld>(w: &mut W, a: W::Unit, d: W::Unit, damage: i32, length: i32) {
    dot(w, a, d, STATE_POISON, damage, length);
}

/// Burn `0x0057ADD0` (§5.8).
pub fn burn<W: CombatWorld>(w: &mut W, a: W::Unit, d: W::Unit, damage: i32, length: i32) {
    dot(w, a, d, STATE_BURNING, damage, length);
}

/// `apply(game, attacker, defender, missile, record)` = `0x0057C6C0`
/// (§5.2).
pub fn apply<W: CombatWorld>(
    w: &mut W,
    ct: &CombatTables,
    a: W::Unit,
    d: W::Unit,
    missile: bool,
    rec: &mut DamageRecord,
) {
    // Step 1–3.
    if !w.hostile(a, d) && rec.hit_flags & hitflag::IGNORE_HOSTILITY == 0 {
        rec.result &= !(result::HIT | result::WILL_DIE);
        return;
    }
    match w.room(d) {
        RoomKind::None => {
            rec.result &= !(result::HIT | result::WILL_DIE);
            return;
        }
        RoomKind::Town => {
            let in_town = w.unit_type(a) == UnitType::Monster
                && ct.monstats(w.class_id(a)).is_some_and(|m| m.intown);
            if !in_town {
                rec.result &= !result::WILL_DIE;
                return;
            }
        }
        RoomKind::Field => {}
    }
    if w.unit_type(d) == UnitType::Monster
        && !ct.monstats(w.class_id(d)).is_some_and(|m| m.killable)
    {
        return;
    }
    if w.is_dead(d) {
        return;
    }
    // Step 4–5.
    w.set_last_attacker(d, a);
    if missile {
        totals(w, ct, Some(a), d, rec);
    }
    // Step 6.
    if rec.result & result::HIT != 0 && rec.result & result::NO_EVENTS == 0 {
        if missile {
            if rec.hit_flags & hitflag::ROLLED != 0
                && rec.hit_flags & hitflag::NO_MISSILE_EVENT == 0
            {
                w.unit_event(EV_DOMISSILEDAMAGE, a, d, rec);
            }
            w.unit_event(EV_DAMAGEDBYMISSILE, d, a, rec);
        } else {
            w.unit_event(EV_DOMELEEDAMAGE, a, d, rec);
            w.unit_event(EV_DAMAGEDINMELEE, d, a, rec);
        }
    }
    // Step 7–9.
    rec.physical = rec.physical.min(w.stat(d, LIFE, 0));
    leech(w, ct, Some(a), d, rec);
    if w.unit_type(a) == UnitType::Monster && rec.result & result::HIT != 0 {
        w.monster_hit_hook(a);
    }
    // Step 10–12.
    heal(w, d, rec.absorbed);
    if rec.total > 0 {
        let mut life = w.stat(d, LIFE, 0).wrapping_sub(rec.total);
        if life < 256 {
            life = 0;
        }
        w.set_stat(d, LIFE, life);
    }
    if rec.mana_leech > 0 {
        let mut m = w.stat(d, MANA, 0).wrapping_sub(rec.mana_leech);
        if m < 256 {
            m = 0;
        }
        w.set_stat(d, MANA, m);
    }
    if rec.stamina_leech > 0 {
        let mut s = w.stat(d, STAMINA, 0).wrapping_sub(rec.stamina_leech);
        if s < 256 {
            s = 0;
        }
        w.set_stat(d, STAMINA, s);
    }
    // Step 13.
    stun(w, ct, a, d, rec.stun_len);
    cold(w, ct, a, d, rec.cold_len);
    freeze(w, ct, d, a, rec.freeze_len);
    poison(w, a, d, rec.poison, rec.poison_len);
    burn(w, a, d, rec.burn, rec.burn_len);
    // Step 14.
    if w.stat(d, LIFE, 0) > 0 {
        rec.result &= !result::WILL_DIE;
        if rec.total != 0 && w.unit_type(d) == UnitType::Monster {
            if w.stat(d, HPREGEN, 0) != 0 {
                w.cancel_timers(d, TIMER_STATREGEN);
                w.schedule_timer(d, TIMER_STATREGEN, w.frame().wrapping_add(1));
            }
            w.monster_damaged_hook(d);
        }
    } else {
        rec.result |= result::WILL_DIE;
    }
    // Step 15.
    if rec.result & result::NO_EVENTS == 0 && rec.result & result::WILL_DIE != 0 {
        w.unit_event(EV_KILLED, d, a, rec);
        w.unit_event(EV_KILL, a, d, rec);
    }
}

/// Frees the attacker's combat records for this defender (`0x0057C9F0`).
pub fn free_records<W: CombatWorld>(w: &mut W, a: W::Unit, d: W::Unit) {
    let (ia, id) = (w.ident(a), w.ident(d));
    w.combat_list(a)
        .retain(|e| !(e.attacker == ia && e.defender == id));
}

/// `apply_melee(game, attacker, defender)` = `0x0057D4F0` (§5.1).
pub fn apply_melee<W: CombatWorld>(w: &mut W, ct: &CombatTables, a: W::Unit, d: W::Unit) {
    let id = w.ident(d);
    let Some(mut rec) = w
        .combat_list(a)
        .iter()
        .find(|e| e.defender == id)
        .map(|e| e.record)
    else {
        return;
    };
    let range = 1 + 2 * i32::from(w.unit_type(a) == UnitType::Monster);
    if !w.in_melee_range(a, d, range) {
        free_records(w, a, d);
        return;
    }
    let hit = rec.result & result::HIT != 0;
    let a_type = w.unit_type(a);
    let pm = matches!(a_type, UnitType::Player | UnitType::Monster);
    if hit {
        // Step 4.1 (`0x0057D5AC`): a dead attacker returns at once; the
        // combat record is not freed, no events, thorns or reaction.
        if pm && w.mode(a) == 0 {
            return;
        }
        rec.hit_flags = hitflag::ROLLED;
        apply(w, ct, a, d, false, &mut rec);
        if rec.overlay > 0 {
            w.overlay(d, rec.overlay);
        }
        if rec.hit_class_fixed == 0 && rec.hit_class & 0x0F == 0 {
            rec.hit_class |= w.weapon_hit_class(a);
        }
        durability(w, a, d);
    }
    w.unit_event(EV_DOMELEEATTACK, a, d, &mut rec);
    w.unit_event(EV_ATTACKEDINMELEE, d, a, &mut rec);
    // Step 6: a player or monster attacker that died during steps 4–5
    // returns at once (`0x0057D63F`), as step 4.1; other types (objects
    // included) and live attackers take thorns.
    if hit {
        if pm && w.mode(a) == 0 {
            return;
        }
        w.thorns(a, d, &mut rec);
    }
    w.reaction(a, d, &mut rec);
    free_records(w, a, d);
}

// ---------------------------------------------------------------- §6

/// Element hit-class table `0x006E16D8` (§6.1): cold, fire, lightning,
/// poison.
const ELEMENT_CLASSES: [(u32, Read); 4] = [
    (0x30, |r| r.cold),
    (0x20, |r| r.fire),
    (0x40, |r| r.lightning),
    (0x50, |r| r.poison),
];

/// Element hit class `0x0057CE30` (§6.1): `base` is the weapon class
/// (low nibble); 0 becomes 13 (`over`). Steps the process-wide counter.
pub fn element_hit_class(counter: &mut u8, rec: &DamageRecord, base: u32) -> u32 {
    let i = usize::from(*counter % 4);
    *counter = counter.wrapping_add(1);
    let mut nibble = 0;
    for k in 0..4 {
        let (n, f) = ELEMENT_CLASSES[(i + k) % 4];
        if f(rec) > 0 {
            nibble = n;
            break;
        }
    }
    if nibble == 0 && rec.result & result::CRITICAL != 0 {
        nibble = 0x10;
    }
    let base = if base == 0 { 13 } else { base };
    base | nibble
}

/// The get-hit divisor of a hit class (jump table `0x0057CBF8`), from the
/// `hitclass` code of row `class` (§6.2 step 3).
pub fn get_hit_divisor(hitclass_codes: &[[u8; 4]], class: u32) -> i32 {
    match usize::try_from(class)
        .ok()
        .and_then(|i| hitclass_codes.get(i))
    {
        Some(b"1hss" | b"1ht " | b"bow " | b"xbow") => 8,
        Some(b"2hss" | b"club") => 32,
        Some(b"2hsl") => 64,
        _ => 16,
    }
}

/// Get-hit test `0x0057CB00(unit, record, hitclass)` (§6.2): `true` when
/// the unit does **not** enter get-hit.
pub fn no_get_hit<W: CombatWorld>(
    w: &mut W,
    hitclass_codes: &[[u8; 4]],
    u: W::Unit,
    rec: &DamageRecord,
    hitclass: u32,
) -> bool {
    if w.has_state(u, STATE_FREEZE) {
        return true;
    }
    if (rec.poison != 0 && rec.poison == rec.total) || rec.total < 256 {
        return true;
    }
    let div = get_hit_divisor(hitclass_codes, hitclass);
    let m = w.stat(u, MAXHP, 0);
    if rec.total < m / div {
        return true;
    }
    if rec.total < m / (div / 2) && w.seed(u).mask(2) == 0 {
        return true;
    }
    if rec.total < m / (div / 4) && w.seed(u).mask(4) == 0 {
        return true;
    }
    w.unit_type(u) == UnitType::Monster && !w.monster_has_mode(u, 3)
}

// ---------------------------------------------------------------- §8

/// `(chance stat id, layer)` of an event function argument
/// (`stat id << 16 | layer`).
fn chance_of<W: CombatWorld>(w: &W, a: W::Unit, arg: u32) -> i32 {
    w.item_stat(a, (arg >> 16) as u16, arg as u16)
}

/// Crushing blow, event function 16 (`0x005BFFC0`, §8). `a` owns the
/// event (the attacker), `d` is the other unit. Returns 1 when it ran
/// past the chance, else 0.
pub fn crushing_blow<W: CombatWorld>(
    w: &mut W,
    event: u8,
    a: W::Unit,
    d: W::Unit,
    rec: &mut DamageRecord,
    arg: u32,
) -> i32 {
    let c = chance_of(w, a, arg);
    if c <= 0 {
        return 0;
    }
    if ((w.seed(a).step() % 100) as i32) >= c {
        return 0;
    }
    let mut div = match w.unit_type(d) {
        UnitType::Player => 10,
        UnitType::Monster if w.is_hireling(d) => 10,
        UnitType::Monster => {
            let mut v: i32 = if w.is_boss(d) || w.monster_flag(d, MON_SUPERUNIQUE) {
                8
            } else {
                4
            };
            // TODO(damage.md §8): the player-count term is read as part of
            // the non-hireling monster branch.
            let h = w.player_count_bonus(w.stat(d, MONSTER_PLAYERCOUNT, 0).max(1));
            if h != 0 {
                v = v.wrapping_add(pct(v, h, 100));
            }
            v
        }
        _ => 4,
    };
    if event == EV_DOMISSILEDAMAGE {
        div = div.wrapping_mul(2);
    }
    let life = w.stat(d, LIFE, 0);
    let mut x = if div != 0 { life / div } else { 0 };
    let dr = w.stat(d, DAMAGERESIST, 0).min(100);
    if dr > 0 {
        x = x.wrapping_sub(pct(x, dr, 100));
    }
    let new = life.wrapping_sub(x).max(0);
    w.set_stat(d, LIFE, new);
    if new <= 0 {
        rec.result |= result::WILL_DIE;
    }
    if x > 0 {
        w.overlay(d, OVERLAY_CRUSHING_BLOW);
    }
    1
}

/// Open-wounds level term `0x005BFDA0` (§8).
pub fn open_wounds_base(lvl: i32) -> i32 {
    if lvl <= 1 {
        0
    } else if lvl <= 15 {
        9 * (lvl - 1)
    } else if lvl <= 30 {
        18 * (lvl - 15) + 14 * 9
    } else if lvl <= 45 {
        27 * (lvl - 30) + 15 * 18 + 14 * 9
    } else if lvl <= 60 {
        36 * (lvl - 45) + 15 * (18 + 27) + 14 * 9
    } else {
        45i32
            .wrapping_mul(lvl - 60)
            .wrapping_add(15 * (18 + 27 + 36) + 14 * 9)
    }
}

/// Open wounds, event function 15 (`0x005BFE60`, §8): the hpregen value
/// `h` of the curse (or `None` when the chance failed).
pub fn open_wounds<W: CombatWorld>(
    w: &mut W,
    event: u8,
    a: W::Unit,
    d: W::Unit,
    arg: u32,
) -> Option<i32> {
    let c = chance_of(w, a, arg);
    if c <= 0 || ((w.seed(a).step() % 100) as i32) >= c {
        return None;
    }
    let lvl = w.stat(a, LEVEL, 0).max(1);
    let mut h = open_wounds_base(lvl).wrapping_add(40);
    match w.unit_type(d) {
        UnitType::Player => {
            h /= 4;
            if event == EV_DOMISSILEDAMAGE {
                h /= 2;
            }
        }
        UnitType::Monster if w.monster_flag(d, MON_UNIQUE_OR_CHAMPION) => h /= 2,
        _ => {}
    }
    w.curse(d, a, STATE_OPENWOUNDS, HPREGEN, h.wrapping_neg(), 200, 0, 1);
    Some(h)
}

// ---------------------------------------------------------------- §9

/// Armor durability weights `0x00732B90` (§9): (body location, weight).
pub const DURABILITY_WEIGHTS: [(u8, i32); 7] =
    [(1, 3), (3, 5), (4, 4), (5, 4), (8, 2), (9, 2), (10, 2)];

/// `durability_hit(game, owner, item)` = `0x00559E30` (§9).
pub fn durability_hit<W: CombatWorld>(w: &mut W, owner: W::Unit, item: W::Item) {
    let armor = w.item_is(item, 50);
    if !(armor || w.item_is(item, 45)) || !w.item_has_durability(item) {
        return;
    }
    let chance = if armor {
        10
    } else if w.item_flag_throw(item) {
        if w.expansion() {
            10
        } else {
            return;
        }
    } else {
        4
    };
    if ((w.seed(owner).step() % 100) as i32) < chance {
        w.durability_loss(owner, item);
    }
}

/// Durability `0x0057D3D0` (§9).
pub fn durability<W: CombatWorld>(w: &mut W, a: W::Unit, d: W::Unit) {
    if w.unit_type(a) == UnitType::Player {
        if let Some(weapon) = w.current_weapon(a) {
            durability_hit(w, a, weapon);
        }
    }
    if w.unit_type(d) != UnitType::Player {
        return;
    }
    // TODO(damage.md §9): "with an inventory" is taken as true for every
    // player defender.
    let kept: Vec<Option<W::Item>> = DURABILITY_WEIGHTS
        .iter()
        .map(|&(loc, _)| w.item_at(d, loc).filter(|&i| w.item_is(i, 50)))
        .collect();
    let total: i32 = DURABILITY_WEIGHTS
        .iter()
        .zip(&kept)
        .filter(|(_, k)| k.is_some())
        .map(|(&(_, wt), _)| wt)
        .sum();
    if total <= 0 {
        return;
    }
    let mut i = w.seed(d).roll(7) as usize;
    let mut x = w.seed(d).roll(total) as i32;
    while x >= 0 {
        if let Some(item) = kept[i] {
            let wt = DURABILITY_WEIGHTS[i].1;
            if x < wt {
                durability_hit(w, d, item);
                return;
            }
            x -= wt;
        }
        i = (i + 1) % 7;
    }
}
