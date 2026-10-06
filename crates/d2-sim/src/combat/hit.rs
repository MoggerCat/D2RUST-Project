// Spec: specs/combat/hit.md
//! Attack rating (§1), defense (§2), chance to hit (§3), melee result
//! flags (§4), block chance (§5), block / weapon block / dodge / avoid /
//! evade (§6). Draws: §Randomness of the spec, in that order.

use super::{pct, CombatTables, CombatWorld};
use crate::skills::{eval_skill, weapon_mastery, SkillTables};
use crate::units::UnitType;

/// Result flags (`damage.md` §1, record +0x04).
pub mod result {
    pub const HIT: u16 = 0x0001;
    pub const WILL_DIE: u16 = 0x0002;
    pub const GET_HIT: u16 = 0x0004;
    pub const KNOCKBACK: u16 = 0x0008;
    pub const BLOCK: u16 = 0x0010;
    pub const NO_EVENTS: u16 = 0x0020;
    pub const DODGE: u16 = 0x0080;
    pub const AVOID: u16 = 0x0100;
    pub const EVADE: u16 = 0x0200;
    pub const CRITICAL: u16 = 0x2000;
    pub const SOFT_HIT: u16 = 0x4000;
    pub const WEAPON_BLOCK: u16 = 0x8000;
}

// Stats.
const DEX: u16 = 2;
const LEVEL: u16 = 12;
const ITEM_ARMOR_PERCENT: u16 = 16;
const TOHIT: u16 = 19;
const TOBLOCK: u16 = 20;
const ARMORCLASS: u16 = 31;
const AC_VS_MISSILE: u16 = 32;
const AC_VS_HTH: u16 = 33;
const IGNORETARGETAC: u16 = 115;
const FRACTIONALTARGETAC: u16 = 116;
const PREVENTHEAL: u16 = 117;
const TOHIT_PERCENT: u16 = 119;
const DEMON_TOHIT: u16 = 123;
const UNDEAD_TOHIT: u16 = 124;
const SKILL_ARMOR_PERCENT: u16 = 171;
const ATTACK_VS_MONTYPE: u16 = 179;
const ARMOR_OVERRIDE_PERCENT: u16 = 182;
const PASSIVE_DODGE: u16 = 338;
const PASSIVE_AVOID: u16 = 339;
const PASSIVE_EVADE: u16 = 340;
const PASSIVE_WEAPONBLOCK: u16 = 348;
const MODIFIERLIST_SKILL: u16 = 350;
const MODIFIERLIST_LEVEL: u16 = 351;

// States.
const STATE_PREVENTHEAL: u16 = 52;
const STATE_UNINTERRUPTABLE: u16 = 54;
const STATE_HOLYSHIELD: u16 = 101;

/// Prevent-heal curse length (`0x0057D810`).
pub const PREVENT_HEAL_FRAMES: i32 = 120_000;
/// Block cap (`0x0062280E`).
pub const BLOCK_CAP: i32 = 75;
/// Weapon class that can weapon-block (`ht2`, `0x0057DDB0`).
pub const WEAPON_CLASS_HT2: i32 = 13;

/// Player modes (`sim/units.md`).
const PLAYER_WALK: i32 = 2;
const PLAYER_RUN: i32 = 3;
/// Monster modes.
const MONSTER_WALK: i32 = 2;
const MONSTER_RUN: i32 = 15;

/// Monster flags of `0x005A0180`.
const MON_SUPERUNIQUE: u32 = 2;
const MON_UNIQUE_OR_SUPER: u32 = 0x0A;

/// `attack_rating(player)` = `0x00622560` (§1). Panics for a non-player
/// (a fatal assertion in the original).
pub fn attack_rating<W: CombatWorld>(w: &W, ct: &CombatTables, u: W::Unit) -> i32 {
    assert_eq!(
        w.unit_type(u),
        UnitType::Player,
        "attack_rating of a non-player"
    );
    let f = ct
        .charstats(w.class_id(u))
        .map_or(0, |c| c.tohitfactor as i32);
    w.stat(u, TOHIT, 0)
        .wrapping_add(5i32.wrapping_mul(w.stat(u, DEX, 0).wrapping_sub(7)))
        .wrapping_add(f)
}

/// `defense(unit)` = `0x006223F0` (§2). Mutable for the Holy Shield
/// formula (a `rand` in a formula would draw).
pub fn defense<W: CombatWorld>(w: &mut W, st: &SkillTables, u: W::Unit) -> i32 {
    let base = w.stat(u, ARMORCLASS, 0).wrapping_add(w.stat(u, DEX, 0) / 4);
    let mut p = w
        .item_stat(u, SKILL_ARMOR_PERCENT, 0)
        .wrapping_add(w.item_stat(u, ITEM_ARMOR_PERCENT, 0));
    if w.has_state(u, STATE_HOLYSHIELD) {
        let skill = w
            .state_stat(u, STATE_HOLYSHIELD, MODIFIERLIST_SKILL)
            .unwrap_or(0);
        let level = w
            .state_stat(u, STATE_HOLYSHIELD, MODIFIERLIST_LEVEL)
            .unwrap_or(0);
        if skill > 0 && level > 0 && w.has_shield(u) {
            if let Some(rec) = st.skill(skill) {
                let calc1 = rec.calc1;
                p = p.wrapping_add(eval_skill(w, st, Some(u), calc1, skill, level));
            }
        }
    }
    let bonus = if base > 0 {
        p.wrapping_mul(base) / 100
    } else {
        p.wrapping_mul(base) / -100
    };
    let mut total = base.wrapping_add(bonus);
    let ovr = w.stat(u, ARMOR_OVERRIDE_PERCENT, 0);
    if ovr != 0 {
        total = total.wrapping_add(pct(total, ovr, 100));
    }
    total
}

/// Inputs of §3.3 after §3.1–§3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitTerms {
    pub ar: i32,
    pub pct_ar: i32,
    pub def: i32,
    pub alvl: i32,
    pub dlvl: i32,
}

/// §3.3: the clamped hit chance (5–95). Panics when `alvl + dlvl = 0`
/// (division by zero; Edge case 2).
pub fn hit_chance(t: HitTerms) -> i32 {
    let mut to_hit = t.ar.wrapping_add(pct(t.ar, t.pct_ar, 100));
    let mut def = t.def;
    if def < 0 {
        to_hit = to_hit.wrapping_sub(def);
        def = 0;
    }
    if to_hit < 0 {
        def = def.wrapping_sub(to_hit);
        to_hit = 0;
    }
    if def < 0 {
        def = 0;
    }
    let sum = to_hit.wrapping_add(def);
    let factor = if sum != 0 {
        100i32.wrapping_mul(to_hit).wrapping_div(sum)
    } else {
        100
    };
    let lv = t.alvl.wrapping_add(t.dlvl);
    assert!(
        lv != 0,
        "hit test with alvl + dlvl = 0 (hit.md Edge case 2)"
    );
    let chance = 2i32
        .wrapping_mul(factor)
        .wrapping_mul(t.alvl)
        .wrapping_div(lv);
    chance.clamp(5, 95)
}

/// Monster's `MonType`, or 0.
fn montype<W: CombatWorld>(w: &W, ct: &CombatTables, u: W::Unit) -> i32 {
    ct.monstats(w.class_id(u))
        .map_or(0, |m| i32::from(m.montype as i16))
}

/// Sum of entries of `stat` (at most 128) whose layer matches the
/// defender's montype (`0x0057A830`; hit §3.2 step 5, damage §3.1 step
/// 3.4).
pub(crate) fn montype_bonus<W: CombatWorld>(
    w: &W,
    ct: &CombatTables,
    a: W::Unit,
    d: W::Unit,
    stat: u16,
) -> i32 {
    if w.unit_type(d) != UnitType::Monster {
        return 0;
    }
    let mt = montype(w, ct, d);
    if mt <= 0 {
        return 0;
    }
    w.stat_entries(a, stat, 128)
        .into_iter()
        .filter(|&(layer, _)| w.montype_matches(layer, mt))
        .fold(0i32, |s, (_, v)| s.wrapping_add(v))
}

/// §3.1–§3.2: the terms of the hit test.
pub fn hit_terms<W: CombatWorld>(
    w: &mut W,
    st: &SkillTables,
    ct: &CombatTables,
    a: W::Unit,
    d: W::Unit,
    bonus: i32,
    missile: bool,
) -> HitTerms {
    let dlvl = w.stat(d, LEVEL, 0);
    let alvl = w.stat(a, LEVEL, 0);
    let extra = w.stat(d, if missile { AC_VS_MISSILE } else { AC_VS_HTH }, 0);
    let mut def = defense(w, st, d).wrapping_add(extra);
    let (ar, pct_ar);
    if w.unit_type(a) == UnitType::Player {
        let mut r = attack_rating(w, ct, a);
        // `0x0057D8A0`.
        let d_mon = w.unit_type(d) == UnitType::Monster;
        if w.item_stat(a, IGNORETARGETAC, 0) != 0
            && d_mon
            && !w.monster_flag(d, MON_UNIQUE_OR_SUPER)
            && !w.is_boss(d)
            && !w.is_hireling(d)
        {
            def = 0;
        }
        let mut f = w.item_stat(a, FRACTIONALTARGETAC, 0);
        if f > 0 {
            let strong =
                !d_mon || w.monster_flag(d, MON_SUPERUNIQUE) || w.is_boss(d) || w.is_hireling(d);
            if strong {
                f /= 2;
            }
            f = f.clamp(0, 100);
            def = def.wrapping_sub(pct(def, f, 100));
        }
        let demon = w.item_stat(a, DEMON_TOHIT, 0);
        if demon != 0 && w.is_demon(d) {
            r = r.wrapping_add(demon);
        }
        let undead = w.item_stat(a, UNDEAD_TOHIT, 0);
        if undead != 0 && w.is_undead(d) {
            r = r.wrapping_add(undead);
        }
        let mut p = 0i32;
        if !missile {
            if let Some(weapon) = w.current_weapon(a) {
                p = weapon_mastery(w, st, Some(a), Some(weapon), None, 0);
            }
        }
        p = p
            .wrapping_add(bonus)
            .wrapping_add(w.item_stat(a, TOHIT_PERCENT, 0));
        p = p.wrapping_add(montype_bonus(w, ct, a, d, ATTACK_VS_MONTYPE));
        ar = r;
        pct_ar = p;
    } else {
        ar = w
            .stat(a, TOHIT, 0)
            .wrapping_add(bonus)
            .wrapping_add(5i32.wrapping_mul(w.stat(a, DEX, 0)));
        pct_ar = w.item_stat(a, TOHIT_PERCENT, 0);
    }
    HitTerms {
        ar,
        pct_ar,
        def,
        alvl,
        dlvl,
    }
}

/// `hit_test(attacker, defender, bonus, missile)` = `0x0057D9B0` (§3):
/// one draw from the attacker's seed (`lo′ mod 100 < chance`); on a hit
/// the prevent-heal curse (§3.5).
pub fn hit_test<W: CombatWorld>(
    w: &mut W,
    st: &SkillTables,
    ct: &CombatTables,
    a: Option<W::Unit>,
    d: Option<W::Unit>,
    bonus: i32,
    missile: bool,
) -> bool {
    let (Some(a), Some(d)) = (a, d) else {
        return false;
    };
    let chance = hit_chance(hit_terms(w, st, ct, a, d, bonus, missile));
    let r = (w.seed(a).step() % 100) as i32;
    if r >= chance {
        return false;
    }
    // `0x0057D810`.
    if w.unit_type(a) == UnitType::Player
        && w.unit_type(d) == UnitType::Monster
        && w.item_stat(a, PREVENTHEAL, 0) != 0
    {
        w.curse(
            d,
            a,
            STATE_PREVENTHEAL,
            ARMORCLASS,
            0,
            PREVENT_HEAL_FRAMES,
            0,
            1,
        );
    }
    true
}

/// `melee_result(game, attacker, defender, bonus, range_offset)` =
/// `0x0057EC10` (§4): the u16 result flags.
pub fn melee_result<W: CombatWorld>(
    w: &mut W,
    st: &SkillTables,
    ct: &CombatTables,
    a: Option<W::Unit>,
    d: Option<W::Unit>,
    bonus: i32,
    range_offset: i32,
) -> u16 {
    let (Some(a), Some(d)) = (a, d) else {
        return 0;
    };
    let pm = |t| matches!(t, UnitType::Player | UnitType::Monster);
    if !pm(w.unit_type(a)) || !pm(w.unit_type(d)) || !w.hostile(a, d) {
        return 0;
    }
    // `0x005A4EE0` is true for every monster (kept by not testing it).
    let mut flags = 0u16;
    if w.unit_type(d) == UnitType::Player && w.mode(d) == PLAYER_RUN {
        flags = result::HIT;
    }
    let mut range = 1 + 2 * i32::from(w.unit_type(a) == UnitType::Monster);
    if range_offset != 0 {
        range = range.wrapping_sub(range_offset.wrapping_add(w.melee_range(a)));
    }
    if !w.in_melee_range(a, d, range) {
        return 0;
    }
    if flags & result::HIT == 0 && hit_test(w, st, ct, Some(a), Some(d), bonus, false) {
        flags |= result::HIT;
    }
    if flags & result::HIT != 0 {
        let b = block_or_dodge(w, ct, a, d, false, true);
        flags |= match b {
            BlockResult::Avoid => result::AVOID,
            BlockResult::Dodge => result::DODGE,
            BlockResult::WeaponBlock => result::WEAPON_BLOCK,
            BlockResult::Block => result::BLOCK,
            BlockResult::None | BlockResult::Evade => 0,
        };
        if b != BlockResult::None {
            flags &= !result::HIT;
        }
    }
    if flags & result::BLOCK != 0 {
        flags &= !result::HIT;
    }
    if flags & result::HIT != 0 && !w.has_state(d, STATE_UNINTERRUPTABLE) {
        flags |= result::GET_HIT;
    }
    flags
}

/// `block_chance(unit, expansion)` = `0x00622720` (§5).
pub fn block_chance<W: CombatWorld>(w: &W, ct: &CombatTables, u: W::Unit, expansion: bool) -> i32 {
    match w.unit_type(u) {
        UnitType::Player => {
            if !w.has_shield(u) {
                return 0;
            }
            let bf = ct
                .charstats(w.class_id(u))
                .map_or(0, |c| i32::from(c.blockfactor));
            let mut b = w.stat(u, TOBLOCK, 0).wrapping_add(bf);
            if expansion {
                let lvl = w.stat(u, LEVEL, 0).max(1);
                b = w
                    .stat(u, DEX, 0)
                    .wrapping_sub(15)
                    .wrapping_mul(b)
                    .wrapping_div(2i32.wrapping_mul(lvl));
            }
            b.min(BLOCK_CAP)
        }
        UnitType::Monster => {
            let class = w.class_id(u);
            let no_shld = ct.monstats(class).is_some_and(|m| m.noshldblock);
            let shield = no_shld
                || match class {
                    243 | 310 | 333 => true,
                    359 => false,
                    _ => w.composit_shield(u),
                };
            if shield {
                w.stat(u, TOBLOCK, 0).min(BLOCK_CAP)
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// Outcome of `block_or_dodge` / `dodge` (§6; the original's return
/// values 1, 2, 4, 8, 0x10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockResult {
    None,
    Block,
    Avoid,
    Dodge,
    Evade,
    WeaponBlock,
}

/// `block_or_dodge(game, attacker, defender, avoid, block)` =
/// `0x0057DFB0` (§6.1).
pub fn block_or_dodge<W: CombatWorld>(
    w: &mut W,
    ct: &CombatTables,
    a: W::Unit,
    d: W::Unit,
    avoid: bool,
    block: bool,
) -> BlockResult {
    if !block {
        return dodge(w, a, d, avoid);
    }
    let mut c = block_chance(w, ct, d, w.expansion());
    if c <= 0 {
        return dodge(w, a, d, avoid);
    }
    if w.unit_type(d) == UnitType::Player && w.moving_mode(d) && w.mode(d) != PLAYER_WALK {
        c /= 3;
    }
    let r = (w.seed(d).step() % 100) as i32;
    if r < c {
        BlockResult::Block
    } else {
        dodge(w, a, d, avoid)
    }
}

/// `dodge(attacker, defender, avoid)` = `0x0057DD60` (§6.2).
pub fn dodge<W: CombatWorld>(w: &mut W, _a: W::Unit, d: W::Unit, avoid: bool) -> BlockResult {
    let mode = w.mode(d);
    let moving = match w.unit_type(d) {
        UnitType::Player => mode == PLAYER_WALK || mode == PLAYER_RUN,
        UnitType::Monster => mode == MONSTER_WALK || mode == MONSTER_RUN,
        _ => false,
    };
    if moving {
        let e = w.stat(d, PASSIVE_EVADE, 0);
        if e > 0 && ((w.seed(d).step() % 100) as i32) < e {
            return BlockResult::Evade;
        }
        return BlockResult::None;
    }
    let wb = weapon_block(w, Some(d));
    if wb > 0 && w.weapon_class(d) == WEAPON_CLASS_HT2 && (w.seed(d).roll(100) as i32) < wb {
        return BlockResult::WeaponBlock;
    }
    let (stat, res) = if avoid {
        (PASSIVE_AVOID, BlockResult::Avoid)
    } else {
        (PASSIVE_DODGE, BlockResult::Dodge)
    };
    let v = w.stat(d, stat, 0);
    if v > 0 && (w.seed(d).roll(100) as i32) < v {
        return res;
    }
    BlockResult::None
}

/// `weapon_block(unit)` = `0x0057DCA0` (§6.4): the largest
/// `passive_weaponblock` among at most 32 entries whose layer is ≤ 0 or
/// matches the right- or left-hand item's type. No draws.
pub fn weapon_block<W: CombatWorld>(w: &W, u: Option<W::Unit>) -> i32 {
    let Some(u) = u else {
        return 0;
    };
    let right = w.item_at(u, 4);
    let left = w.item_at(u, 5);
    let mut best: Option<i32> = None;
    for (layer, v) in w.stat_entries(u, PASSIVE_WEAPONBLOCK, 32) {
        let l = i32::from(layer as i16);
        let ok = l <= 0
            || right.is_some_and(|i| w.item_is(i, l))
            || left.is_some_and(|i| w.item_is(i, l));
        if ok {
            best = Some(best.map_or(v, |b| b.max(v)));
        }
    }
    // TODO(hit.md §6.4): the result with no matching entry is not stated;
    // the narrowest reading is 0 (no weapon block, no draw).
    best.unwrap_or(0)
}
