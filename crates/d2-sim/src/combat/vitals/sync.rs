// Spec: specs/combat/vitals.md §5 (client vitals sync `0x00548760`)
//! The client vitals sync: what a player's own client is told about its
//! life, mana, stamina, position, gold and experience at the end of each
//! flush (§5.1), from the per-client cache (§5.2) by the steps of §5.3,
//! with the layouts of §5.4 (S→C 0x18, 0x95, 0x96; 0x19–0x1F gold through
//! `items::moves::layouts::gold`; 0x1A–0x1C experience).
//!
//! Pure: the caller reads the current values ([`Current`], with
//! [`life_prediction`] / [`mana_prediction`] for the potion predictions)
//! and owns the cache; [`sync`] decides and builds the bytes.
//!
//! Status: implemented, unverified (no recording holds a 0x18 / 0x95 /
//! 0x96 byte with its tick, `vitals.md` OQ8).

use crate::items::moves::layouts;
use crate::path::walk::messages::walk_verify;

/// The per-client cache record at client +0x48C (`0x00539330`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncCache {
    /// +0x00.
    pub quiet: u32,
    /// +0x04 / +0x06 / +0x08.
    pub life: u16,
    pub mana: u16,
    pub stamina: u16,
    /// +0x0A / +0x0B.
    pub lp: u8,
    pub mp: u8,
    /// +0x0C / +0x0E.
    pub x: u16,
    pub y: u16,
    /// +0x10 / +0x12 (zero-extended bytes).
    pub dx: u16,
    pub dy: u16,
    /// +0x14: gold sent (stat 14).
    pub gold: u32,
    /// +0x18: experience sent (stat 13).
    pub exp: u32,
}

/// The current values of §5.2.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Current {
    /// L = total(6) >> 8.
    pub life: i32,
    /// M = max life >> 8.
    pub max_life: i32,
    /// total(8) >> 8.
    pub mana: i32,
    /// total(10) >> 8.
    pub stamina: i32,
    pub lp: u8,
    pub mp: u8,
    /// Sub-tile position.
    pub x: u16,
    pub y: u16,
    /// (X − path target) & 0xFF; 0 without a path.
    pub dx: u8,
    pub dy: u8,
    /// total(14), total(13).
    pub gold: u32,
    pub exp: u32,
}

/// §5.1 rule 2: force := +0x1B0 ≥ 20, or ≥ 10 with a queued buffer.
pub fn force(update_count: u32, queued: bool) -> bool {
    update_count >= 20 || (update_count >= 10 && queued)
}

/// The low byte of v, 100 when that byte is above 100 (§5.2).
fn pct_byte(v: i32) -> u8 {
    let b = v as u8;
    if b > 100 {
        100
    } else {
        b
    }
}

/// Life prediction `0x005485B0` (§5.2). `potion`: the state 100
/// (`healthpot`) list as (its total of stat 74, its expire frame), when
/// the unit has one. 32-bit arithmetic as the original.
pub fn life_prediction(potion: Option<(i32, i32)>, frame: i32, life_total: i32, m: i32) -> u8 {
    let Some((regen, expire)) = potion else {
        return 0;
    };
    if m == 0 {
        return 0;
    }
    let q = regen
        .wrapping_mul(expire.wrapping_sub(frame))
        .wrapping_add(life_total)
        >> 8;
    pct_byte(q.wrapping_mul(100).wrapping_div(m))
}

/// The inputs of the mana prediction (§5.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ManaInputs {
    /// Max mana (8.8, `0x00625D60`).
    pub max_mana: i32,
    /// charstats `ManaRegen` of the class row; `None`: no valid row.
    pub mana_regen: Option<u8>,
    /// total(27), total(26), total(8).
    pub regen_pct: i32,
    pub regen_add: i32,
    pub mana_total: i32,
}

/// Mana prediction `0x00548640` (§5.2). `potion_expire`: the expire frame
/// of the state 106 (`manapot`) list, when the unit has one.
pub fn mana_prediction(potion_expire: Option<i32>, frame: i32, v: &ManaInputs) -> u8 {
    let (Some(expire), Some(regen)) = (potion_expire, v.mana_regen) else {
        return 0;
    };
    let m = v.max_mana;
    if m == 0 {
        return 0;
    }
    let q = if regen == 0 {
        7500
    } else {
        i32::from(regen) * 25
    };
    let i = (m / q).max(1);
    // The stat 27 scaling is truncated here (not `MulDiv`).
    let i = i
        .wrapping_mul(v.regen_pct.wrapping_add(100))
        .wrapping_div(100)
        .wrapping_add(v.regen_add);
    let v = expire
        .wrapping_sub(frame)
        .wrapping_mul(i)
        .wrapping_add(v.mana_total)
        .wrapping_mul(100)
        .wrapping_div(m);
    pct_byte(v)
}

/// Writes `value`'s low `width` bits LSB first at bit `at` (§5.4).
fn put_bits(buf: &mut [u8], at: &mut usize, width: usize, value: u32) {
    for i in 0..width {
        if (value >> i) & 1 != 0 {
            let b = *at + i;
            buf[b / 8] |= 1 << (b % 8);
        }
    }
    *at += width;
}

/// S→C 0x18 LifeManaUpdate (`0x0053C230`, §5.4): 15 bytes, 115 bits.
#[allow(clippy::too_many_arguments)]
pub fn life_mana_update(
    life: i32,
    mana: i32,
    stamina: i32,
    lp: u8,
    mp: u8,
    x: u16,
    y: u16,
    dx: u8,
    dy: u8,
) -> [u8; 15] {
    let mut m = [0u8; 15];
    let mut at = 0;
    for (w, v) in [
        (8, 0x18u32),
        (15, life as u32),
        (15, mana as u32),
        (15, stamina as u32),
        (7, u32::from(lp)),
        (7, u32::from(mp)),
        (16, u32::from(x)),
        (16, u32::from(y)),
        (8, u32::from(dx)),
        (8, u32::from(dy)),
    ] {
        put_bits(&mut m, &mut at, w, v);
    }
    m
}

/// S→C 0x95 LifeManaUpdate2 (`0x0053C320`, §5.4): 13 bytes, 101 bits.
#[allow(clippy::too_many_arguments)]
pub fn life_mana_update2(
    life: i32,
    mana: i32,
    stamina: i32,
    x: u16,
    y: u16,
    dx: u8,
    dy: u8,
) -> [u8; 13] {
    let mut m = [0u8; 13];
    let mut at = 0;
    for (w, v) in [
        (8, 0x95u32),
        (15, life as u32),
        (15, mana as u32),
        (15, stamina as u32),
        (16, u32::from(x)),
        (16, u32::from(y)),
        (8, u32::from(dx)),
        (8, u32::from(dy)),
    ] {
        put_bits(&mut m, &mut at, w, v);
    }
    m
}

/// Experience `0x0053BDD0(new, old)` (§5.3 step 5): `None` when equal.
pub fn exp_message(new: u32, old: u32) -> Option<Vec<u8>> {
    if new == old {
        return None;
    }
    let d = new.wrapping_sub(old);
    Some(if d > 0xFFFE {
        let mut b = vec![0x1C];
        b.extend_from_slice(&new.to_le_bytes());
        b
    } else if d >= 0xFF {
        let mut b = vec![0x1B];
        b.extend_from_slice(&(d as u16).to_le_bytes());
        b
    } else {
        vec![0x1A, d as u8]
    })
}

/// What one run of the routine did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Synced {
    /// The routine returned 1: the caller resets client +0x1B0 to 0.
    pub done: bool,
    /// The messages in send order.
    pub messages: Vec<Vec<u8>>,
}

/// §5.3 on the cache with the current values.
pub fn sync(cache: &mut SyncCache, cur: &Current, force: bool) -> Synced {
    let mut out = Synced::default();
    // Step 1 (missing client or unit is the caller's).
    if cur.max_life <= 0 {
        return out;
    }
    // Step 2.
    if !force {
        let d = (i32::from(cache.life) - cur.life).wrapping_abs();
        if d.wrapping_mul(100) / cur.max_life < 10 {
            return out;
        }
        if cur.life == 0 && cache.life != 0 {
            return out;
        }
    }
    // Step 3: the cache fields hold the values as stored (u16, u8).
    let (life, mana, stamina) = (cur.life as u16, cur.mana as u16, cur.stamina as u16);
    let msg: Option<Vec<u8>> = if cur.lp != cache.lp || cur.mp != cache.mp {
        Some(
            life_mana_update(
                cur.life,
                cur.mana,
                cur.stamina,
                cur.lp,
                cur.mp,
                cur.x,
                cur.y,
                cur.dx,
                cur.dy,
            )
            .to_vec(),
        )
    } else if life != cache.life || mana != cache.mana {
        Some(
            life_mana_update2(
                cur.life,
                cur.mana,
                cur.stamina,
                cur.x,
                cur.y,
                cur.dx,
                cur.dy,
            )
            .to_vec(),
        )
    } else if stamina != cache.stamina
        || (cache.quiet > 3
            && ((i32::from(cache.x) - i32::from(cur.x)).abs() >= 2
                || (i32::from(cache.y) - i32::from(cur.y)).abs() >= 2))
    {
        Some(walk_verify(cur.stamina as u32, cur.x, cur.y, cur.dx as i8, cur.dy as i8).to_vec())
    } else {
        None
    };
    match msg {
        Some(m) => {
            out.messages.push(m);
            cache.x = cur.x;
            cache.y = cur.y;
            cache.dx = u16::from(cur.dx);
            cache.dy = u16::from(cur.dy);
            cache.quiet = 0;
        }
        None => cache.quiet = cache.quiet.wrapping_add(1),
    }
    // Step 4.
    if cur.gold != cache.gold {
        if let Some(m) = layouts::gold(cur.gold, cache.gold) {
            out.messages.push(m);
        }
        cache.gold = cur.gold;
    }
    // Step 5.
    if let Some(m) = exp_message(cur.exp, cache.exp) {
        out.messages.push(m);
    }
    cache.exp = cur.exp;
    // Step 6.
    cache.life = life;
    cache.mana = mana;
    cache.stamina = stamina;
    cache.lp = cur.lp;
    cache.mp = cur.mp;
    out.done = true;
    out
}

#[cfg(test)]
mod tests;
