// Spec: specs/skills/descriptions.md
//! More pure parts of the skill description functions: the arithmetic of
//! the shared helpers once the caller has read the unit's stats.

use super::skill_desc::etype_color;

/// `pct(v, p, d)` = `v × p / d` truncating toward zero (`combat/damage.md` §0).
pub fn pct(v: i32, p: i32, d: i32) -> i32 {
    v.wrapping_mul(p) / d
}

/// Font choice of the range draw (§2.1 rule 3): `w' = w × 11 / 7`;
/// `w' > x2 − x1` → small font (Font6) and `y − 1`. Returns (small, y).
pub fn range_font(text_width: i32, x1: i32, x2: i32, y: i32) -> (bool, i32) {
    if text_width * 11 / 7 > x2 - x1 {
        (true, y - 1)
    } else {
        (false, y)
    }
}

/// Top-line offset of the two-line draw (§2.2): −6, or −7 for languages 6–9.
pub fn two_line_top_offset(language: i32) -> i32 {
    if (6..=9).contains(&language) {
        -7
    } else {
        -6
    }
}

/// Which lines a two-range function draws (§2.2).
#[derive(Debug, PartialEq, Eq)]
pub enum TwoLine {
    /// A both zero: B alone, drawn as §2.1 in B's color.
    BAlone,
    /// B both zero: A alone in A's color.
    AAlone,
    /// Both ranges, A on top.
    Both,
}

/// Selection rule of §2.2.
pub fn two_line_plan(a: (i32, i32), b: (i32, i32)) -> TwoLine {
    if a == (0, 0) {
        TwoLine::BAlone
    } else if b == (0, 0) {
        TwoLine::AAlone
    } else {
        TwoLine::Both
    }
}

/// Weapon-physical base damage (§2.3 rules 2–3): `b`, `big_b` from the
/// weapon (or the bare-hand `mindamage + 1`, `maxdamage + 2`), `s` the
/// source percent (0 → `src_dam`). Returns the (min, max) added.
pub fn weapon_phys_base(b: i32, big_b: i32, s: i32, src_dam: i32) -> (i32, i32) {
    let s = if s == 0 { src_dam } else { s };
    let (mut b, mut big_b) = (b, big_b);
    if s != 128 {
        b = b * s / 128;
        big_b = big_b * s / 128;
    }
    if s != 0 {
        (b.max(1), big_b.max(2))
    } else {
        (0, 0)
    }
}

/// Weapon-physical final scaling (§2.3 rule 6): returns (min, max).
#[allow(clippy::too_many_arguments)]
pub fn weapon_phys_final(
    min: i32,
    max: i32,
    p: i32,
    min_pct: i32,
    max_pct: i32,
    f1: i32,
    f0: i32,
) -> (i32, i32) {
    let p = if p < -89 { -90 } else { p };
    (
        (min_pct + 100 + p) * min / 100 + f1 + f0,
        (max_pct + 100 + p) * max / 100 + f1 + f0,
    )
}

/// One element of the stat-element sum (§2.4 rule 1): totals with the
/// mastery percent folded in and `mn := min(mn, mx)`.
pub fn element_pair(mn: i32, mx: i32, mastery: i32) -> (i32, i32) {
    let (mut mn, mut mx) = (mn, mx);
    if mastery != 0 {
        mn += pct(mn, mastery, 100);
        mx += pct(mx, mastery, 100);
    }
    (mn.min(mx), mx)
}

/// Poison term of §2.4 rule 2. `override_len` > 0 wins over
/// `poison_length / q` (q < 2 → 1). Returns (min, max).
pub fn poison_pair(
    mn: i32,
    mx: i32,
    mastery: i32,
    override_len: i32,
    poison_length: i32,
    count: i32,
) -> (i32, i32) {
    if mx == 0 {
        return (0, 0);
    }
    let (mut mn, mut mx) = (mn, mx);
    if mastery != 0 {
        mn += pct(mn, mastery, 100);
        mx += pct(mx, mastery, 100);
    }
    let n = if override_len > 0 {
        override_len
    } else {
        poison_length / count.max(1)
    };
    ((n * mn) >> 8, (n * mx) >> 8)
}

/// §2.4 rule 3 clamp of the finished sum.
pub fn clamp_elements(min: i32, max: i32) -> (i32, i32) {
    if max > 0 {
        let min = min.max(1);
        (min, max.max(min + 1))
    } else {
        (min, max)
    }
}

/// Skill elements (§2.5): `(emin, emax)` are the `elem_min` / `elem_max`
/// values (poison: already × length). Returns (min, max, color).
pub fn skill_elements(etype: i32, emin: i32, emax: i32) -> (i32, i32, Option<i32>) {
    (emin >> 8, emax >> 8, etype_color(etype))
}

/// Charge bonus term of §2.9 rule 1 for one state: `(Param1 + (v − 1) ×
/// Param2) × n`, counted only for `prgdam` = 1.
pub fn charge_bonus_term(prgdam: i32, param1: i32, param2: i32, v: i32, n: i32) -> i32 {
    if prgdam == 1 {
        (param1 + (v - 1) * param2) * n
    } else {
        0
    }
}

/// Charge index of §2.9 rule 2: `j = clamp(n, 1, 3)`.
pub fn charge_index(n: i32) -> i32 {
    n.clamp(1, 3)
}

/// Throw damage percent (§2.7 rule 3 / entry 21): clamp below −89.
pub fn throw_percent(str_part: i32, dex_part: i32, damagepercent: i32) -> i32 {
    let p = str_part + dex_part + damagepercent;
    if p < -89 {
        -90
    } else {
        p
    }
}

/// `t + (stat + P) × t / 100` of §2.7 rule 3.
pub fn throw_base(t: i32, stat_pct: i32, p: i32) -> i32 {
    t + (stat_pct + p) * t / 100
}

/// Throw damage tail (§2.7 rule 3): apply `q`, then `max := max(max, min)`.
pub fn throw_apply_q(min: i32, max: i32, q: i32) -> (i32, i32) {
    let min = min + pct(min, q, 100);
    let max = max + pct(max, q, 100);
    (min, max.max(min))
}

/// Missile-potion tail (§2.7 rule 2): `*min := a >> 8`, `*max :=
/// max(b >> 8, *min)`.
pub fn potion_throw(a: i32, b: i32) -> (i32, i32) {
    let mn = a >> 8;
    (mn, (b >> 8).max(mn))
}

/// Entry 12 (blessed hammer) percent: `p × v / 100`.
pub fn entry12(a: i32, b: i32, p: i32) -> (i32, i32) {
    let p = if p < -89 { -90 } else { p };
    (a + p * a / 100, b + p * b / 100)
}

/// Entry 14 (hunger): `a += q × a / 100`.
pub fn entry14(a: i32, b: i32, q: i32) -> (i32, i32) {
    (a + q * a / 100, b + q * b / 100)
}

/// Entry 15 (dragon talon): (min, max) before the charge damage.
#[allow(clippy::too_many_arguments)]
pub fn entry15(k: i32, big_k: i32, kp: i32, s: i32, s2: i32, p: i32) -> (i32, i32) {
    (
        k + pct(k, kp, 100) + s + pct(s, p, 100),
        big_k + pct(big_k, kp, 100) + s2 + pct(s2, p, 100),
    )
}

/// Entry 11 (vengeance) base clamp: `b0 ≥ 1`, `B0 ≥ 2`, `B0 ≥ b0`.
pub fn entry11_base(b0: i32, big_b0: i32) -> (i32, i32) {
    let b0 = b0.max(1);
    (b0, big_b0.max(2).max(b0))
}

/// Entry 11 per-term add: `p += pct(p, m, 100)` when both ≠ 0.
pub fn entry11_term(p: i32, m: i32) -> i32 {
    if p != 0 && m != 0 {
        p + pct(p, m, 100)
    } else {
        p
    }
}

/// `AR` without the state colors (§2.11) for a thrown weapon: type-38
/// primary type zeroes the rating.
pub fn ar_base(rating: i32, primary_type_38: bool) -> i32 {
    if primary_type_38 {
        0
    } else {
        rating
    }
}

/// `descatt` 5 (§4 row 5): keep only the other hand's rating.
pub fn descatt5(v2: i32, c2: i32) -> (i32, i32, i32, i32) {
    (v2, c2, 0, 0)
}

/// Double `SrcDam` scaling of `0x004E99F0` (Edge case 3): applied when
/// both bounds are non-zero.
pub fn driver_scale(mini: i32, maxi: i32, src_dam: i32, e: i32, big_e: i32) -> (i32, i32) {
    if mini != 0 && maxi != 0 {
        (mini * src_dam / 128 + e, maxi * src_dam / 128 + big_e)
    } else {
        (mini, maxi)
    }
}
