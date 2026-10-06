// Spec: specs/skills/levels.md
//! Skill level (§1), special values (§2), skill damage (§3), mana cost
//! (§4), to-hit (§5) and learning (§6). Integer rules: signed 32-bit,
//! wrapping, truncating division, as the spec states per formula.

use super::calc::{self, CalcContext};
use super::special::{MissSpecial, SkillSpecial, MISS_SPECIALS, SKILL_SPECIALS};
use super::{SkillEntry, SkillTables, SkillUnits};
use crate::combat::pct;
use crate::units::UnitType;
use d2_data::tables::Skills;

/// "No formula" (a calc field of 0xFFFFFFFF).
pub const NO_CALC: u32 = 0xFFFF_FFFF;

// Stat ids (itemstatcost).
const STR: u16 = 0;
const ENERGY: u16 = 1;
const DEX: u16 = 2;
const VIT: u16 = 3;
const LIFE: u16 = 6;
const MANA: u16 = 8;
const LEVEL: u16 = 12;
const MINDAMAGE: u16 = 21;
const DAMAGEPERCENT: u16 = 25;
const ADDCLASSSKILLS: u16 = 83;
const NONCLASSSKILL: u16 = 97;
const SINGLESKILL: u16 = 107;
const ELEMSKILL: u16 = 126;
const ALLSKILLS: u16 = 127;
const ADDSKILL_TAB: u16 = 188;
const MAXDAMAGE_PERCENT: u16 = 17;
const KICKDAMAGE: u16 = 137;

// States.
const STATE_CONCENTRATION: u16 = 42;
const STATE_BLOOD_MANA: u16 = 114;
const STATE_SHRINE_SKILL: u16 = 134;

/// i32 view of a u32 record field.
fn s32(v: u32) -> i32 {
    v as i32
}

/// i32 view of an i16 record field stored as u16.
fn s16(v: u16) -> i32 {
    i32::from(v as i16)
}

// ---------------------------------------------------------------- §1

/// `skill_level(unit, entry, bonus)` = `0x006442A0` (§1).
pub fn skill_level<W: SkillUnits>(
    w: &W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    entry: Option<&SkillEntry>,
    bonus: bool,
) -> i32 {
    let (Some(u), Some(e)) = (unit, entry) else {
        return 0;
    };
    let mut l = e.base;
    if bonus && e.is_native() {
        l = l.wrapping_add(bonus_level(w, t, u, e));
    }
    l.clamp(0, t.level_cap.max(0))
}

/// `bonus_level(unit, skill)` = `0x00644180` (§1), for the skill of
/// `entry` (whose +0x2C level bonus it adds).
///
/// Panics when a class skill of the player's class has no skilldesc
/// record: the original faults there (Edge case 4).
pub fn bonus_level<W: SkillUnits>(w: &W, t: &SkillTables, u: W::Unit, entry: &SkillEntry) -> i32 {
    let skill = entry.skill;
    let Some(rec) = t.skill(skill) else {
        return 0;
    };
    let layer = skill as u16;
    let shrine = if w.has_state(u, STATE_SHRINE_SKILL) {
        2
    } else {
        0
    };
    let mut b = entry
        .level_bonus
        .wrapping_add(shrine)
        .wrapping_add(w.stat(u, ALLSKILLS, 0));
    let charclass = i32::from(rec.charclass as i8);
    if w.unit_type(u) == UnitType::Player {
        let class = w.class_id(u);
        if class == charclass {
            b = b.wrapping_add(w.stat(u, ADDCLASSSKILLS, class as u16));
            let desc = t
                .skilldesc_of(rec)
                .expect("class skill without a skilldesc record (levels.md Edge case 4)");
            let page = i32::from(desc.skillpage as i8);
            if page != 0 {
                let tab = page.wrapping_add(8 * class).wrapping_sub(1);
                b = b.wrapping_add(w.stat(u, ADDSKILL_TAB, tab as u16));
            }
            b = b.wrapping_add(w.stat(u, NONCLASSSKILL, layer).min(3));
        } else {
            b = b.wrapping_add(w.stat(u, NONCLASSSKILL, layer));
        }
    } else if entry.base <= 0 {
        b = b.wrapping_add(w.stat(u, NONCLASSSKILL, layer));
    } else {
        let n = w.stat(u, NONCLASSSKILL, layer);
        if n != 0 {
            b = b.wrapping_add(n.min(3));
        }
    }
    if rec.etype != 0 {
        b = b.wrapping_add(w.stat(u, ELEMSKILL, u16::from(rec.etype)));
    }
    b.wrapping_add(w.stat(u, SINGLESKILL, layer))
}

/// `highest_entry(unit, skill)` = `0x00643810` (§1): the entry the
/// formula `skill()` and `blvl` read.
pub fn highest_entry(list: &[SkillEntry], skill: i32) -> Option<SkillEntry> {
    let mut cur: Option<SkillEntry> = None;
    for e in list.iter().filter(|e| e.skill == skill && !e.has_charges) {
        cur = Some(match cur {
            None => *e,
            Some(_) if e.is_native() => *e,
            Some(c) if c.is_native() => c,
            Some(c) if e.base > c.base => *e,
            Some(c) => c,
        });
    }
    cur
}

// ---------------------------------------------------------------- §2

/// `DM(lvl, a, b)` = `0x00645B20` (§2). Panics on `lvl = −6` (division by
/// zero; the original faults, Edge case 3).
pub fn dm(lvl: i32, a: i32, b: i32) -> i32 {
    let d = lvl.wrapping_add(6);
    assert!(
        d != 0,
        "DM at level -6 divides by zero (levels.md Edge case 3)"
    );
    let q = 110i32.wrapping_mul(lvl).wrapping_div(d);
    let r = q.wrapping_mul(b.wrapping_sub(a)).wrapping_div(100);
    let v = a.wrapping_add(r);
    if v > b {
        b
    } else {
        v
    }
}

/// `lnXY` (§2): `p + (lvl − 1) × q`, wrapping.
pub fn ln(lvl: i32, p: i32, q: i32) -> i32 {
    p.wrapping_add(lvl.wrapping_sub(1).wrapping_mul(q))
}

/// The skills evaluation context (`data/calc-expressions.md` §3.5).
struct SkillCtx<'a, W: SkillUnits> {
    w: &'a mut W,
    t: &'a SkillTables,
    unit: Option<W::Unit>,
    skill: i32,
    lvl: i32,
}

impl<W: SkillUnits> CalcContext for SkillCtx<'_, W> {
    fn function_count(&self) -> u8 {
        7
    }
    fn arity(&self, index: u8) -> u8 {
        if index == 6 {
            3
        } else {
            2
        }
    }
    fn param(&mut self, c: i32) -> i32 {
        // Parameter callback `0x00646BE0`: the context skill's special
        // value `c` (low 8 bits) at the context level.
        special(self.w, self.t, self.unit, c as u8, self.skill, self.lvl)
    }
    fn call(&mut self, index: u8, a: &[i32]) -> i32 {
        match index {
            0 => a[0].min(a[1]),
            1 => a[0].max(a[1]),
            2 => match self.unit {
                Some(u) => calc::rand(self.w.seed(u), a[0], a[1]),
                // TODO(calc-expressions.md §3.5): `rand` with a context
                // but no unit (a skilldesc evaluation) has no seed rule;
                // narrowest reading: no draw, `a` when a ≥ b, else 0.
                None if a[0] >= a[1] => a[0],
                None => 0,
            },
            3 => formula_skill(self.w, self.t, self.unit, a[0], a[1]),
            4 => miss_special(self.w, self.t, None, self.unit, a[1] as u8, a[0], self.lvl),
            5 => match self.unit {
                Some(u) if a[0] >= 0 && a[0] < self.t.stat_count => {
                    self.w.formula_stat(u, a[0] as u16, a[1])
                }
                _ => 0,
            },
            6 => {
                let l = special(self.w, self.t, self.unit, a[1] as u8, self.skill, self.lvl);
                special(self.w, self.t, self.unit, a[2] as u8, a[0], l)
            }
            _ => 0,
        }
    }
}

/// `skill(s, c)` = `0x00646C00` (§2): special value `c` of skill `s` at
/// the unit's level in it (with bonuses); level 0 with no unit or entry.
fn formula_skill<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    s: i32,
    c: i32,
) -> i32 {
    let l = match unit {
        Some(u) => {
            let e = highest_entry(&w.skill_list(u), s);
            skill_level(w, t, unit, e.as_ref(), true)
        }
        None => 0,
    };
    special(w, t, unit, c as u8, s, l)
}

/// `eval(unit, field, skill, lvl)` = `0x00646CA0` (§2): a skills formula
/// field evaluated in the (unit, skill, level) context. Field −1 or an
/// offset past the buffer gives 0.
pub fn eval_skill<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    field: u32,
    skill: i32,
    lvl: i32,
) -> i32 {
    let code = &t.skills_code;
    let mut ctx = SkillCtx {
        w,
        t,
        unit,
        skill,
        lvl,
    };
    calc::eval(code, field, &mut ctx)
}

/// `special(unit, c, skill, lvl)` = `0x00646460` (§2, `skillcalc.tsv`).
pub fn special<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    c: u8,
    skill: i32,
    lvl: i32,
) -> i32 {
    let Some(rec) = t.skill(skill) else {
        return 0;
    };
    let Some(kind) = SKILL_SPECIALS.get(usize::from(c)) else {
        return 0;
    };
    use SkillSpecial as K;
    let guarded = |v: i32| if lvl > 0 { v } else { 0 };
    match *kind {
        K::Ln(p, q) => guarded(ln(lvl, skill_param(rec, p), skill_param(rec, q))),
        K::Dm(p, q, guard) => {
            if guard && lvl <= 0 {
                0
            } else {
                dm(lvl, skill_param(rec, p), skill_param(rec, q))
            }
        }
        K::Par(p) => skill_param(rec, p),
        K::Lvl => lvl,
        K::ElemMin { mastery, shift } => {
            let v = elem_min(w, t, unit, skill, lvl, mastery);
            if shift {
                v >> 8
            } else {
                v
            }
        }
        K::ElemMax { mastery, shift } => {
            let v = elem_max(w, t, unit, skill, lvl, mastery);
            if shift {
                v >> 8
            } else {
                v
            }
        }
        K::ElemLen => elem_len(w, t, unit, skill, lvl),
        K::ToHit => to_hit(w, t, unit, skill, lvl),
        K::Mana => guarded(mana_cost(rec, lvl) >> 8),
        K::Mps => {
            let n = s16(rec.mana).wrapping_add(s16(rec.lvlmana).wrapping_mul(lvl.wrapping_sub(1)));
            let v = n.wrapping_mul(25) / 2;
            guarded(v.wrapping_shl(u32::from(rec.manashift as u8)) >> 8)
        }
        K::Mastery(ty) => mastery(w, t, unit, rec, skill, lvl, ty),
        K::MissElem { slot, max, shift } => {
            if lvl <= 0 {
                return 0;
            }
            let Some(m) = desc_missile(t, rec, slot) else {
                return 0;
            };
            let v = if max {
                miss_elem_max(w, t, None, unit, m, lvl)
            } else {
                miss_elem_min(w, t, None, unit, m, lvl)
            };
            if shift {
                v >> 8
            } else {
                v
            }
        }
        K::MissElemLen { slot } => {
            if lvl <= 0 {
                return 0;
            }
            match desc_missile(t, rec, slot) {
                Some(m) => miss_elem_len(w, t, None, m, lvl),
                None => 0,
            }
        }
        K::MissRange { slot } => match desc_missile(t, rec, slot).and_then(|m| t.missile(m)) {
            Some(m) => s16(m.range).wrapping_add(lvl.wrapping_mul(s16(m.levrange))),
            None => 0,
        },
        K::ULvl => unit.map_or(0, |u| w.stat(u, LEVEL, 0)),
        K::BLvl => match unit {
            Some(u) => {
                let e = highest_entry(&w.skill_list(u), skill);
                skill_level(w, t, unit, e.as_ref(), false)
            }
            None => 0,
        },
        K::Usmc => guarded(mana_cost(rec, lvl)),
        K::Eval(f) => {
            let field = f.of(rec);
            eval_skill(w, t, unit, field, skill, lvl)
        }
    }
}

/// `ParamN` (1-based) of a skills record, as i32.
fn skill_param(rec: &Skills, n: u8) -> i32 {
    s32(match n {
        1 => rec.param1,
        2 => rec.param2,
        3 => rec.param3,
        4 => rec.param4,
        5 => rec.param5,
        6 => rec.param6,
        7 => rec.param7,
        _ => rec.param8,
    })
}

/// `descmissile{slot+1}` of the skill's skilldesc record; `None` when the
/// skilldesc record or the missile is missing.
fn desc_missile(t: &SkillTables, rec: &Skills, slot: u8) -> Option<i32> {
    let d = t.skilldesc_of(rec)?;
    let m = match slot {
        0 => d.descmissile1,
        1 => d.descmissile2,
        _ => d.descmissile3,
    };
    let m = i32::from(m as i16);
    t.missile(m).map(|_| m)
}

/// `mastery(type)` = `0x00647E00` (§2).
fn mastery<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    rec: &Skills,
    skill: i32,
    lvl: i32,
    ty: u8,
) -> i32 {
    if lvl <= 0 {
        return 0;
    }
    let (a, b) = match ty {
        0 => (342, 345),
        1 => (343, 346),
        _ => (344, 347),
    };
    let pairs = [
        (rec.passivestat1, rec.passivecalc1),
        (rec.passivestat2, rec.passivecalc2),
        (rec.passivestat3, rec.passivecalc3),
        (rec.passivestat4, rec.passivecalc4),
        (rec.passivestat5, rec.passivecalc5),
    ];
    for (stat, field) in pairs {
        let s = s16(stat);
        if s == a || s == b {
            return eval_skill(w, t, unit, field, skill, lvl);
        }
    }
    0
}

/// The missiles evaluation context (`data/calc-expressions.md` §3.5).
struct MissCtx<'a, W: SkillUnits> {
    w: &'a mut W,
    t: &'a SkillTables,
    missile_unit: Option<W::Unit>,
    owner: Option<W::Unit>,
    missile: i32,
    lvl: i32,
}

impl<W: SkillUnits> CalcContext for MissCtx<'_, W> {
    fn function_count(&self) -> u8 {
        4
    }
    fn arity(&self, _index: u8) -> u8 {
        2
    }
    fn param(&mut self, c: i32) -> i32 {
        miss_special(
            self.w,
            self.t,
            self.missile_unit,
            self.owner,
            c as u8,
            self.missile,
            self.lvl,
        )
    }
    fn call(&mut self, index: u8, a: &[i32]) -> i32 {
        match index {
            0 => a[0].min(a[1]),
            1 => a[0].max(a[1]),
            // TODO(calc-expressions.md OQ1): missile `rand` has no settled
            // seed; validated buffers contain none (policy 6). No draw.
            2 => 0,
            _ => formula_skill(self.w, self.t, self.owner, a[0], a[1]),
        }
    }
}

/// Missiles evaluator `0x0064B7C0`.
pub fn eval_missile<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    missile_unit: Option<W::Unit>,
    owner: Option<W::Unit>,
    field: u32,
    missile: i32,
    lvl: i32,
) -> i32 {
    let code = &t.miss_code;
    let mut ctx = MissCtx {
        w,
        t,
        missile_unit,
        owner,
        missile,
        lvl,
    };
    calc::eval(code, field, &mut ctx)
}

/// `miss_special(missile_unit, owner, c, missile, lvl)` = `0x0064B340`
/// (§2, `misscalc.tsv`).
pub fn miss_special<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    missile_unit: Option<W::Unit>,
    owner: Option<W::Unit>,
    c: u8,
    missile: i32,
    lvl: i32,
) -> i32 {
    if let Some(m) = missile_unit {
        if w.unit_type(m) != UnitType::Missile {
            return 0;
        }
    }
    let Some(rec) = t.missile(missile) else {
        return 0;
    };
    let Some(kind) = MISS_SPECIALS.get(usize::from(c)) else {
        return 0;
    };
    use MissSpecial as K;
    match *kind {
        K::Par(f) => f.of(rec),
        K::Lvl => lvl,
        K::ElemMin { shift } => {
            let v = miss_elem_min(w, t, missile_unit, owner, missile, lvl);
            if shift {
                v >> 8
            } else {
                v
            }
        }
        K::ElemMax { shift } => {
            let v = miss_elem_max(w, t, missile_unit, owner, missile, lvl);
            if shift {
                v >> 8
            } else {
                v
            }
        }
        K::ElemLen => miss_elem_len(w, t, missile_unit, missile, lvl),
        K::PhysMin { shift } => {
            let v = miss_phys_min(w, t, missile_unit, owner, missile, lvl);
            if shift {
                v >> 8
            } else {
                v
            }
        }
        K::PhysMax { shift } => {
            let v = miss_phys_max(w, t, missile_unit, owner, missile, lvl);
            if shift {
                v >> 8
            } else {
                v
            }
        }
        K::Range => s16(rec.range).wrapping_add(lvl.wrapping_mul(s16(rec.levrange))),
        K::Ln(p, q) => ln(lvl, p.of(rec), q.of(rec)),
        K::Dm(p, q) => dm(lvl, p.of(rec), q.of(rec)),
    }
}

// ---------------------------------------------------------------- §3

/// `bracket(lvl, L1…L5)` = `0x00644B70` (§3).
pub fn bracket(lvl: i32, l: [i32; 5]) -> i32 {
    let m = |a: i32, b: i32| a.wrapping_mul(b);
    let s1 = m(7, l[0]);
    let s2 = s1.wrapping_add(m(8, l[1]));
    let s3 = s2.wrapping_add(m(6, l[2]));
    let s4 = s3.wrapping_add(m(6, l[3]));
    match lvl {
        i32::MIN..=1 => 0,
        2..=8 => m(lvl - 1, l[0]),
        9..=16 => s1.wrapping_add(m(lvl - 8, l[1])),
        17..=22 => s2.wrapping_add(m(lvl - 16, l[2])),
        23..=28 => s3.wrapping_add(m(lvl - 22, l[3])),
        _ => s4.wrapping_add(m(lvl - 28, l[4])),
    }
}

/// The 8/16 length brackets (§3.2).
fn len_bracket(lvl: i32, l: [i32; 3]) -> i32 {
    let m = |a: i32, b: i32| a.wrapping_mul(b);
    if lvl <= 8 {
        m(lvl - 1, l[0])
    } else if lvl <= 16 {
        m(7, l[0]).wrapping_add(m(lvl - 8, l[1]))
    } else {
        m(7, l[0])
            .wrapping_add(m(8, l[1]))
            .wrapping_add(m(lvl.wrapping_sub(16), l[2]))
    }
}

/// Elemental mastery stat by `EType` (byte table `0x00644D38`, §3.1).
pub fn elem_mastery_stat(etype: u8) -> Option<u16> {
    match etype {
        1 => Some(329),
        2 => Some(330),
        4 | 12 => Some(331),
        5 => Some(332),
        _ => None,
    }
}

/// Shared body of `elem_min` / `elem_max` (§3.1).
fn elem<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    skill: i32,
    lvl: i32,
    mastery: bool,
    max: bool,
) -> i32 {
    let Some(rec) = t.skill(skill) else {
        return 0;
    };
    if lvl <= 0 {
        return 0;
    }
    let (base, levs) = if max {
        (
            rec.emax,
            [
                rec.emaxlev1,
                rec.emaxlev2,
                rec.emaxlev3,
                rec.emaxlev4,
                rec.emaxlev5,
            ],
        )
    } else {
        (
            rec.emin,
            [
                rec.eminlev1,
                rec.eminlev2,
                rec.eminlev3,
                rec.eminlev4,
                rec.eminlev5,
            ],
        )
    };
    let mut v = s32(base)
        .wrapping_add(bracket(lvl, levs.map(s32)))
        .wrapping_shl(u32::from(rec.hitshift));
    if rec.edmgsympercalc != NO_CALC {
        // TODO(levels.md §3.1 step 3): whether the gated minimum still
        // evaluates the formula is not stated; d2rs evaluates, then gates
        // the application (only a `rand` in the formula could tell).
        let p = eval_skill(w, t, unit, rec.edmgsympercalc, skill, lvl);
        if p != 0 && (max || v > 256 || rec.eminlev1 != 0) {
            v = v.wrapping_add(pct(v, p, 100));
        }
    }
    if mastery {
        if let (Some(u), Some(stat)) = (unit, elem_mastery_stat(rec.etype)) {
            // TODO(levels.md §3.1): the getter is not named; the unit
            // getter `0x00625480` is the narrowest reading.
            v = v.wrapping_add(pct(v, w.stat(u, stat, 0), 100));
        }
    }
    v
}

/// `elem_min(unit, skill, lvl, mastery)` = `0x00644D50` (§3.1), 1/256
/// points.
pub fn elem_min<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    skill: i32,
    lvl: i32,
    mastery: bool,
) -> i32 {
    elem(w, t, unit, skill, lvl, mastery, false)
}

/// `elem_max(unit, skill, lvl, mastery)` = `0x00644E40` (§3.1).
pub fn elem_max<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    skill: i32,
    lvl: i32,
    mastery: bool,
) -> i32 {
    elem(w, t, unit, skill, lvl, mastery, true)
}

/// `elem_len(unit, skill, lvl)` = `0x00644F20` (§3.2), frames.
pub fn elem_len<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    skill: i32,
    lvl: i32,
) -> i32 {
    let Some(rec) = t.skill(skill) else {
        return 0;
    };
    if lvl <= 0 {
        return 0;
    }
    let l = len_bracket(lvl, [rec.elevlen1, rec.elevlen2, rec.elevlen3].map(s32));
    let mut v = s32(rec.elen).wrapping_add(l);
    if rec.elensympercalc != NO_CALC {
        let p = eval_skill(w, t, unit, rec.elensympercalc, skill, lvl);
        if p != 0 {
            v = v.wrapping_add(pct(v, p, 100));
        }
    }
    v
}

/// `phys_min` / `phys_max` = `0x00647BC0` / `0x00647D00` (§3.3).
fn phys<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    skill: i32,
    lvl: i32,
    use_srcdam: bool,
    max: bool,
) -> i32 {
    let Some(rec) = t.skill(skill) else {
        return if max { 2 } else { 1 };
    };
    if rec.kick {
        // `0x00644C20`.
        let x = match unit {
            Some(u) if w.unit_type(u) == UnitType::Player => w
                .stat(u, DEX, 0)
                .wrapping_add(w.stat(u, STR, 0))
                .wrapping_sub(20),
            Some(u) => 3i32.wrapping_mul(w.stat(u, LEVEL, 0)),
            None => 0,
        }
        .max(1);
        return (if max { x / 3 } else { x / 4 }).wrapping_shl(8);
    }
    let mut s = 0i32;
    if rec.srcdam != 0 && use_srcdam {
        let wv = match unit {
            Some(u) => match w.weapon(u) {
                Some(item) if w.wield_type(item) == 2 => w.item_damage(item, max),
                _ => w.stat(u, if max { MINDAMAGE + 1 } else { MINDAMAGE }, 0),
            },
            None => 0,
        };
        s = i32::from(rec.srcdam).wrapping_mul(wv) / 128;
    }
    let (base, levs) = if max {
        (
            rec.maxdam,
            [
                rec.maxlevdam1,
                rec.maxlevdam2,
                rec.maxlevdam3,
                rec.maxlevdam4,
                rec.maxlevdam5,
            ],
        )
    } else {
        (
            rec.mindam,
            [
                rec.minlevdam1,
                rec.minlevdam2,
                rec.minlevdam3,
                rec.minlevdam4,
                rec.minlevdam5,
            ],
        )
    };
    let mut v = s
        .wrapping_add(s32(base))
        .wrapping_add(bracket(lvl, levs.map(s32)));
    if rec.dmgsympercalc != NO_CALC {
        let p = eval_skill(w, t, unit, rec.dmgsympercalc, skill, lvl);
        if p != 0 {
            v = v.wrapping_add(pct(v, p, 100));
        }
    }
    v.wrapping_shl(u32::from(rec.hitshift))
}

/// `phys_min(unit, skill, lvl, use_srcdam)` = `0x00647BC0` (§3.3).
pub fn phys_min<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    skill: i32,
    lvl: i32,
    use_srcdam: bool,
) -> i32 {
    phys(w, t, unit, skill, lvl, use_srcdam, false)
}

/// `phys_max(unit, skill, lvl, use_srcdam)` = `0x00647D00` (§3.3).
pub fn phys_max<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    skill: i32,
    lvl: i32,
    use_srcdam: bool,
) -> i32 {
    phys(w, t, unit, skill, lvl, use_srcdam, true)
}

/// Missile id and level after the §3.4 unit rules; `None` = result 0.
fn miss_args<W: SkillUnits>(
    w: &W,
    missile_unit: Option<W::Unit>,
    missile: i32,
    lvl: i32,
) -> Option<(i32, i32)> {
    match missile_unit {
        Some(m) if w.unit_type(m) != UnitType::Missile => None,
        Some(m) => {
            let id = if missile < 0 { w.class_id(m) } else { missile };
            let l = if lvl <= 0 { w.missile_level(m) } else { lvl };
            Some((id, l))
        }
        None => Some((missile, lvl)),
    }
}

/// Shared body of the missile damage functions (§3.4).
#[allow(clippy::too_many_arguments)]
fn miss_dmg<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    missile_unit: Option<W::Unit>,
    owner: Option<W::Unit>,
    missile: i32,
    lvl: i32,
    elemental: bool,
    max: bool,
) -> i32 {
    let Some((id, lvl)) = miss_args(w, missile_unit, missile, lvl) else {
        return 0;
    };
    let Some(rec) = t.missile(id) else {
        return 0;
    };
    let (base, levs, sym): (u32, [u32; 5], u32) = match (elemental, max) {
        (false, false) => (
            rec.mindamage,
            [
                rec.minlevdam1,
                rec.minlevdam2,
                rec.minlevdam3,
                rec.minlevdam4,
                rec.minlevdam5,
            ],
            rec.dmgsympercalc,
        ),
        (false, true) => (
            rec.maxdamage,
            [
                rec.maxlevdam1,
                rec.maxlevdam2,
                rec.maxlevdam3,
                rec.maxlevdam4,
                rec.maxlevdam5,
            ],
            rec.dmgsympercalc,
        ),
        (true, false) => (
            rec.emin,
            [
                rec.minelev1,
                rec.minelev2,
                rec.minelev3,
                rec.minelev4,
                rec.minelev5,
            ],
            rec.edmgsympercalc,
        ),
        (true, true) => (
            rec.emax,
            [
                rec.maxelev1,
                rec.maxelev2,
                rec.maxelev3,
                rec.maxelev4,
                rec.maxelev5,
            ],
            rec.edmgsympercalc,
        ),
    };
    let shift = u32::from(rec.hitshift);
    let mut v = s32(base).wrapping_add(bracket(lvl, levs.map(s32)));
    if sym != NO_CALC {
        let p = eval_missile(w, t, missile_unit, owner, sym, id, lvl);
        v = v.wrapping_add(pct(v, p, 100));
    }
    v.wrapping_shl(shift)
}

/// Missile physical min `0x0064AF20` (§3.4), 1/256 points.
pub fn miss_phys_min<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    missile_unit: Option<W::Unit>,
    owner: Option<W::Unit>,
    missile: i32,
    lvl: i32,
) -> i32 {
    miss_dmg(w, t, missile_unit, owner, missile, lvl, false, false)
}

/// Missile physical max `0x0064AFF0` (§3.4).
pub fn miss_phys_max<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    missile_unit: Option<W::Unit>,
    owner: Option<W::Unit>,
    missile: i32,
    lvl: i32,
) -> i32 {
    miss_dmg(w, t, missile_unit, owner, missile, lvl, false, true)
}

/// Missile elemental min `0x0064B100` (§3.4).
pub fn miss_elem_min<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    missile_unit: Option<W::Unit>,
    owner: Option<W::Unit>,
    missile: i32,
    lvl: i32,
) -> i32 {
    miss_dmg(w, t, missile_unit, owner, missile, lvl, true, false)
}

/// Missile elemental max `0x0064B1D0` (§3.4).
pub fn miss_elem_max<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    missile_unit: Option<W::Unit>,
    owner: Option<W::Unit>,
    missile: i32,
    lvl: i32,
) -> i32 {
    miss_dmg(w, t, missile_unit, owner, missile, lvl, true, true)
}

/// Missile elemental length `0x0064B2A0` (§3.4), frames: `lvl ≤ 0` →
/// `ELen`; no synergy.
pub fn miss_elem_len<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    missile_unit: Option<W::Unit>,
    missile: i32,
    lvl: i32,
) -> i32 {
    let Some((id, lvl)) = miss_args(w, missile_unit, missile, lvl) else {
        return 0;
    };
    let Some(rec) = t.missile(id) else {
        return 0;
    };
    if lvl <= 0 {
        return s32(rec.elen);
    }
    s32(rec.elen).wrapping_add(len_bracket(
        lvl,
        [rec.elevlen1, rec.elevlen2, rec.elevlen3].map(s32),
    ))
}

/// `weapon_mastery(unit, item, skill, type)` = `0x00645830` (§3.5): type
/// 0 to-hit, 1 damage, 2 critical strike. `skill = None` reads the used
/// skill.
pub fn weapon_mastery<W: SkillUnits>(
    w: &W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    item: Option<W::Item>,
    skill: Option<i32>,
    ty: u8,
) -> i32 {
    let (Some(u), Some(item)) = (unit, item) else {
        return 0;
    };
    let skill = skill.or_else(|| w.used_skill(u).map(|e| e.skill));
    // Throw path `0x00645720`.
    let throw = w.item_flag_throw(item)
        && skill.and_then(|s| t.skill(s)).is_some_and(|r| {
            r.range == 2 && r.itypea1 != 0xFFFF && w.itype_is(i32::from(r.itypea1), 48)
        });
    let stat = if throw { 345 } else { 342 } + u16::from(ty.min(2));
    w.stat_entries(u, stat, 32)
        .into_iter()
        .filter(|&(layer, _)| w.item_is(item, i32::from(layer)))
        .fold(0, |m, (_, v)| m.max(v))
}

/// Concentration `0x006461D0` (§3.5): with state 42, `(damagepercent of
/// that state's list × Param1 of skill) / 8`; else 0.
pub fn concentration<W: SkillUnits>(w: &W, t: &SkillTables, unit: W::Unit, skill: i32) -> i32 {
    if !w.has_state(unit, STATE_CONCENTRATION) {
        return 0;
    }
    let dp = w
        .state_stat(unit, STATE_CONCENTRATION, DAMAGEPERCENT)
        .unwrap_or(0);
    let p1 = t.skill(skill).map_or(0, |r| s32(r.param1));
    dp.wrapping_mul(p1) / 8
}

/// The item reads of kick damage (§3.5) that belong to the items and
/// stat-lists specs (a seam). Provider: items / stat-lists implementation.
pub trait KickItems: SkillUnits {
    /// `0x00627910`: toggle the weapons' stat lists off (`false`) or merge
    /// only the left weapon's list back (`true`, Edge case 6).
    fn toggle_weapon_lists(&mut self, u: Self::Unit, on: bool);
    /// Items record bytes +0xFE / +0xFF of the boots (kick min / max).
    fn boots_damage(&self, item: Self::Item) -> (i32, i32);
}

/// Kick damage `0x00646280(unit, &min, &max, &pct)` (§3.5).
pub fn kick_damage<W: KickItems>(
    w: &mut W,
    u: W::Unit,
    min: &mut i32,
    max: &mut i32,
    pc: &mut i32,
) {
    let k = w.item_stat(u, KICKDAMAGE, 0);
    *min = min.wrapping_add(k);
    *max = max.wrapping_add(k);
    w.toggle_weapon_lists(u, false);
    if let Some(boots) = w.item_at(u, 9) {
        let (bmin0, bmax0) = w.boots_damage(boots);
        let bmin = k.wrapping_add(bmin0);
        let mut bmax = k.wrapping_add(bmax0);
        let (sb, db) = w.str_dex_bonus(boots);
        let p = pct(w.stat(u, STR, 0), sb, 100)
            .wrapping_add(pct(w.stat(u, DEX, 0), db, 100))
            .wrapping_add(w.stat(u, DAMAGEPERCENT, 0))
            .max(-90);
        if bmin >= bmax {
            bmax = bmin;
        }
        *pc = pc
            .wrapping_add(w.stat(u, MAXDAMAGE_PERCENT, 0))
            .wrapping_add(p);
        *min = min.wrapping_add(bmin);
        *max = max.wrapping_add(bmax);
    }
    w.toggle_weapon_lists(u, true);
}

/// `roll_physical(unit, record, skill, lvl)` = `0x0056E170` (§3.6): adds
/// `a + roll(b − a)` to the record's physical damage (unit seed).
pub fn roll_physical<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: W::Unit,
    record: &mut crate::combat::DamageRecord,
    skill: i32,
    lvl: i32,
) {
    let a = phys_min(w, t, Some(unit), skill, lvl, false);
    let b = phys_max(w, t, Some(unit), skill, lvl, false);
    let r = w.seed(unit).roll(b.wrapping_sub(a)) as i32;
    record.physical = record.physical.wrapping_add(a.wrapping_add(r));
}

/// `roll_elemental(unit, record, skill, lvl)` = `0x0056E0C0` (§3.6):
/// evaluates `elem_len`, `elem_min`, `elem_max` (mastery on) in that
/// order and draws `roll(b − a)`. Returns `(EType, v, len)`.
// TODO(levels.md OQ4): `0x0056C8E0` places `v` and `len` into the damage
// record by `EType`; its field map is unspecified, so the caller places
// them.
pub fn roll_elemental<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: W::Unit,
    skill: i32,
    lvl: i32,
) -> (u8, i32, i32) {
    let len = elem_len(w, t, Some(unit), skill, lvl);
    let a = elem_min(w, t, Some(unit), skill, lvl, true);
    let b = elem_max(w, t, Some(unit), skill, lvl, true);
    let v = a.wrapping_add(w.seed(unit).roll(b.wrapping_sub(a)) as i32);
    let etype = t.skill(skill).map_or(0, |r| r.etype);
    (etype, v, len)
}

// ---------------------------------------------------------------- §4

/// Cost `0x00644B10(skill, lvl)` (§4): `(mana + lvlmana × (lvl − 1)) <<
/// manashift`, 1/256 points; no level check, no `minmana`.
pub fn mana_cost(rec: &Skills, lvl: i32) -> i32 {
    s16(rec.mana)
        .wrapping_add(s16(rec.lvlmana).wrapping_mul(lvl.wrapping_sub(1)))
        .wrapping_shl(u32::from(rec.manashift as u8))
}

/// Shifted cost `0x006459F0` (§4): `max(cost >> 8, 0)`. Panics on an
/// invalid skill (a fatal assertion in the original).
pub fn mana_cost_shifted(t: &SkillTables, skill: i32, lvl: i32) -> i32 {
    let rec = t
        .skill(skill)
        .expect("invalid skill in shifted mana cost (levels.md §4)");
    (mana_cost(rec, lvl) >> 8).max(0)
}

/// The mana effects of §4 that belong to other specs (a seam). Provider:
/// units/stats (stat writes) and the skills-use / items implementation.
pub trait ManaUnits: SkillUnits {
    /// `0x0063A400(unit)` (were-form test for `srvdofunc` 116).
    fn shapeshifted(&self, u: Self::Unit) -> bool;
    /// Charges path `0x0056BEC0`; returns the consume result.
    fn consume_charges(&mut self, u: Self::Unit, entry: &SkillEntry) -> bool;
    /// Blood-mana payment `0x005D2B60(unit, c)`.
    // TODO(levels.md OQ8): the rule is unspecified; the provider decides.
    fn pay_life(&mut self, u: Self::Unit, cost: i32) -> bool;
    /// Sets a stat's base value (here: mana, stat 8).
    fn set_stat(&mut self, u: Self::Unit, stat: u16, value: i32);
}

/// `can_afford(unit, skill)` = `0x00647540` (§4) for the skill of `entry`.
pub fn can_afford<W: ManaUnits>(w: &W, t: &SkillTables, u: W::Unit, entry: &SkillEntry) -> bool {
    if !entry.is_native() {
        return entry.charges > 0;
    }
    let Some(rec) = t.skill(entry.skill) else {
        return false;
    };
    let n = (skill_level(w, t, Some(u), Some(entry), true) - 1).max(0);
    let c = s16(rec.mana)
        .wrapping_add(s16(rec.lvlmana).wrapping_mul(n))
        .wrapping_shl(u32::from(rec.manashift as u8));
    if w.has_state(u, STATE_BLOOD_MANA) {
        w.stat(u, LIFE, 0) >= c
    } else if rec.srvdofunc == 116 && w.shapeshifted(u) {
        true
    } else {
        w.stat(u, MANA, 0) >= c
    }
}

/// `consume_mana(game, unit, skill, lvl)` = `0x0056BFE0` (§4). Returns
/// the original's result (`true` = 1).
pub fn consume_mana<W: ManaUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    skill: i32,
    lvl: i32,
) -> bool {
    let Some(u) = unit else {
        return true;
    };
    if w.unit_type(u) != UnitType::Player {
        return true;
    }
    if let Some(e) = w.used_skill(u) {
        if e.skill == skill && !e.is_native() {
            return w.consume_charges(u, &e);
        }
    }
    let Some(rec) = t.skill(skill) else {
        return false;
    };
    if rec.mana == 0 && rec.lvlmana == 0 {
        return false;
    }
    let c = s16(rec.mana)
        .wrapping_add(s16(rec.lvlmana).wrapping_mul(lvl.wrapping_sub(1).max(0)))
        .wrapping_shl(u32::from(rec.manashift as u8))
        .max(s16(rec.minmana).wrapping_shl(8));
    if w.has_state(u, STATE_BLOOD_MANA) {
        return w.pay_life(u, c);
    }
    let mana = w.stat(u, MANA, 0);
    if mana < c {
        return false;
    }
    w.set_stat(u, MANA, mana.wrapping_sub(c));
    true
}

// ---------------------------------------------------------------- §5

/// `to_hit(unit, skill, lvl)` = `0x006449F0` (§5): the hit-test bonus.
pub fn to_hit<W: SkillUnits>(
    w: &mut W,
    t: &SkillTables,
    unit: Option<W::Unit>,
    skill: i32,
    lvl: i32,
) -> i32 {
    let Some(rec) = t.skill(skill) else {
        return 0;
    };
    if lvl <= 0 {
        return 0;
    }
    if rec.tohitcalc != NO_CALC {
        return eval_skill(w, t, unit, rec.tohitcalc, skill, lvl);
    }
    ln(lvl, s32(rec.tohit), s32(rec.levtohit))
}

// ---------------------------------------------------------------- §6

/// `max_level(skill)` = `0x004AA8B0` (§6): `maxlvl` if > 0, else 20.
pub fn max_level(rec: &Skills) -> i32 {
    let m = s16(rec.maxlvl);
    if m > 0 {
        m
    } else {
        20
    }
}

/// The native entry of `skill` in a list (first in list order).
fn native_entry(list: &[SkillEntry], skill: i32) -> Option<SkillEntry> {
    list.iter()
        .find(|e| e.skill == skill && e.is_native())
        .copied()
}

/// `req_level(unit, skill)` = `0x00644750` (§6): `reqlevel` + base of the
/// native entry; a bad id gives `i32::MAX`.
pub fn req_level<W: SkillUnits>(w: &W, t: &SkillTables, u: W::Unit, skill: i32) -> i32 {
    let Some(rec) = t.skill(skill) else {
        return i32::MAX;
    };
    let base = native_entry(&w.skill_list(u), skill).map_or(0, |e| e.base);
    s16(rec.reqlevel).wrapping_add(base)
}

/// `0x006447D0` (§6 step 3): level and required skills.
pub fn meets_skill_reqs<W: SkillUnits>(w: &W, t: &SkillTables, u: W::Unit, skill: i32) -> bool {
    let Some(rec) = t.skill(skill) else {
        return false;
    };
    if w.stat(u, LEVEL, 0) < req_level(w, t, u, skill) {
        return false;
    }
    let list = w.skill_list(u);
    [rec.reqskill1, rec.reqskill2, rec.reqskill3]
        .into_iter()
        .map(s16)
        .filter(|&r| r >= 0)
        .all(|r| t.skill(r).is_some() && native_entry(&list, r).is_some_and(|e| e.base > 0))
}

/// `0x00644920` (§6 step 4): `InGame`, level and attributes.
pub fn meets_attr_reqs<W: SkillUnits>(w: &W, t: &SkillTables, u: W::Unit, skill: i32) -> bool {
    let Some(rec) = t.skill(skill) else {
        return false;
    };
    rec.ingame
        && w.stat(u, LEVEL, 0) >= req_level(w, t, u, skill)
        && s16(rec.reqstr) <= w.stat(u, STR, 0)
        && s16(rec.reqdex) <= w.stat(u, DEX, 0)
        && s16(rec.reqint) <= w.stat(u, ENERGY, 0)
        && s16(rec.reqvit) <= w.stat(u, VIT, 0)
}

/// The learning effects of §6.4 that belong to other specs (a seam).
/// Provider: units/stats (skill list, stat 5) and `sim/intents-events.md`.
pub trait LearnUnits: SkillUnits {
    /// `0x0056C700`: `skill` is a class skill of the player.
    fn is_class_skill(&self, u: Self::Unit, skill: i32) -> bool;
    /// Spends `cost` from `newskills(5)`, adds a level (`0x00647110`),
    /// refreshes (`0x00646F20`), toggles the passive state and calls
    /// `0x00646D60` (§6.4 step 4); refunds on failure.
    fn add_skill_level(&mut self, u: Self::Unit, skill: i32, cost: i32);
}

/// Outcome of the 0x3B validator `0x00549490` (§6.4 step 2–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillPointCheck {
    Ok,
    /// 2: id out of range, or already at the maximum level.
    Code2,
    /// 3: not a class skill, or a requirement fails.
    Code3,
}

/// §6.4 steps 2–3: validate a skill-point request.
// TODO(levels.md OQ5): what codes 2/3 send to the client is unspecified.
pub fn check_skill_point<W: LearnUnits>(
    w: &W,
    t: &SkillTables,
    u: W::Unit,
    skill: i32,
) -> SkillPointCheck {
    let Some(rec) = t.skill(skill) else {
        return SkillPointCheck::Code2;
    };
    if !w.is_class_skill(u, skill)
        || !meets_skill_reqs(w, t, u, skill)
        || !meets_attr_reqs(w, t, u, skill)
    {
        return SkillPointCheck::Code3;
    }
    if let Some(e) = native_entry(&w.skill_list(u), skill) {
        if skill_level(w, t, Some(u), Some(&e), false) >= max_level(rec) {
            return SkillPointCheck::Code2;
        }
    }
    SkillPointCheck::Ok
}

/// §6.4 step 4 (`0x00570080`): spend a point if `newskills(5)` (base
/// getter) covers the cost. Returns whether a level was added.
pub fn spend_skill_point<W: LearnUnits>(
    w: &mut W,
    t: &SkillTables,
    u: W::Unit,
    skill: i32,
) -> bool {
    let Some(rec) = t.skill(skill) else {
        return false;
    };
    let cost = if rec.skpoints == NO_CALC {
        1
    } else {
        let e = native_entry(&w.skill_list(u), skill);
        let l = e.map_or(0, |e| skill_level(w, t, Some(u), Some(&e), true));
        eval_skill(w, t, Some(u), rec.skpoints, skill, l)
    };
    if w.base_stat(u, 5, 0) < cost {
        return false;
    }
    w.add_skill_level(u, skill, cost);
    true
}
