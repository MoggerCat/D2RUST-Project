// Spec: specs/render/lighting.md (§7 contribution of one record)
//! The contribution of one light record to the light map: shared rules
//! (§7.1), plain (§7.2), shadowed (§7.3) and cached (§7.4). Integer math
//! only.

use super::map::LightMap;
use super::records::{unit_type, LightError, LightRecord, LightWorld};

/// Side of the shading grid (§7.3).
pub const GRID: i32 = 64;
/// Centre of the shading grid (§7.3).
pub const CENTRE: i32 = 32;
/// The blocker value `B` of a light-blocking cell (§7.3 r1).
pub const WALL: i32 = 16;

/// Octagonal distance (§7.1 r4, `0x004740D0`, `0x00474080`).
pub fn oct(a: i32, b: i32) -> i32 {
    (983 * a.max(b) + 407 * a.min(b)) >> 10
}

/// The reciprocal table `T[i] = 65536 / i`, `T[0] = 0` (§7.1 r6,
/// `0x007B0A68`).
pub fn recip(i: u8) -> i32 {
    if i == 0 {
        0
    } else {
        65536 / i32::from(i)
    }
}

/// Add `v` > 0 to cell `(gx, gy)` (§7.1 r5–r6, `0x004747C0`); cells
/// outside 0…47 are skipped. `src` is the record's R, G, B.
pub fn add(map: &mut LightMap, gx: i32, gy: i32, v: i32, src: (u8, u8, u8), colored: bool) {
    if v <= 0 {
        return;
    }
    let Some(c) = map.cell_mut(gx, gy) else {
        return;
    };
    let old = i64::from(c.i);
    let new_i = (old + i64::from(v)).min(255) as u8;
    if colored {
        let t = i64::from(recip(new_i));
        let mix = |cur: u8, s: u8| -> u8 {
            (((i64::from(cur) * old + i64::from(s) * i64::from(v)) * t) >> 16).min(255) as u8
        };
        c.r = mix(c.r, src.0);
        c.g = mix(c.g, src.1);
        c.b = mix(c.b, src.2);
    } else {
        c.r = 0;
        c.g = 0;
        c.b = 0;
    }
    c.i = new_i;
}

/// The window of §7.1 r2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub x0: i32,
    pub y0: i32,
    /// `n = 2r >> 3 = 2m`.
    pub n: i32,
}

/// §7.1 r1–r2: `None` when `r` ∉ 1…255 or the window misses the map.
pub fn window(map: &LightMap, x: i32, y: i32, r: i32) -> Option<Window> {
    if !(1..=255).contains(&r) {
        return None;
    }
    // `x mod 8` with C semantics (positions are non-negative in practice).
    let x0 = (x - x % 8) - r;
    let y0 = (y - y % 8) - r;
    let n = (2 * r) >> 3;
    let (ox, oy) = map.origin;
    if x0 >> 3 > ox + 48 || (x0 >> 3) + n + 1 < ox || y0 >> 3 > oy + 48 || (y0 >> 3) + n + 1 < oy {
        return None;
    }
    Some(Window { x0, y0, n })
}

/// §7.1 r3–r6 over window `w`: `weight(j, i, b)` turns the base value
/// `b` of row `j`, column `i` into the added value `v` (or `None`).
fn spread(
    map: &mut LightMap,
    rec: &LightRecord,
    r: i32,
    w: Window,
    colored: bool,
    weight: impl Fn(i32, i32, i32) -> Option<i32>,
) {
    let k = (i32::from(rec.i) << 16) / r;
    let (ox, oy) = map.origin;
    for j in 0..=w.n {
        for i in 0..=w.n {
            let cx = w.x0 + 8 * i;
            let cy = w.y0 + 8 * j;
            let d = oct((rec.x - cx).abs(), (rec.y - cy).abs());
            let b = ((r - d) * k) >> 16;
            if let Some(v) = weight(j, i, b) {
                add(
                    map,
                    (cx >> 3) - ox,
                    (cy >> 3) - oy,
                    v,
                    (rec.r, rec.g, rec.b),
                    colored,
                );
            }
        }
    }
}

/// The radius used by the plain contribution (§7.2): at `q` = 0 a remote
/// player's `r` ≥ 16 → 16, a monster's → 8, a missile → none.
pub fn plain_radius(rec: &LightRecord, q: u8, owner_is_local_player: bool) -> Option<i32> {
    let r = rec.radius;
    if q != 0 {
        return Some(r);
    }
    match rec.owner_type {
        unit_type::PLAYER if !owner_is_local_player && r >= 16 => Some(16),
        unit_type::MONSTER => Some(8),
        unit_type::MISSILE => None,
        _ => Some(r),
    }
}

/// Plain contribution (§7.2, `0x004748D0`).
pub fn plain(
    map: &mut LightMap,
    rec: &LightRecord,
    q: u8,
    owner_is_local_player: bool,
    colored: bool,
) {
    let Some(r) = plain_radius(rec, q, owner_is_local_player) else {
        return;
    };
    let Some(w) = window(map, rec.x, rec.y, r) else {
        return;
    };
    spread(map, rec, r, w, colored, |_, _, b| Some(b));
}

/// The 64 × 64 planes of §7.3: blockers `B` and shade `S`, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadeGrid {
    pub b: Vec<i32>,
    pub s: Vec<i32>,
}

impl Default for ShadeGrid {
    fn default() -> Self {
        ShadeGrid {
            b: vec![0; (GRID * GRID) as usize],
            s: vec![0; (GRID * GRID) as usize],
        }
    }
}

impl ShadeGrid {
    fn at(row: i32, col: i32) -> usize {
        (row * GRID + col) as usize
    }

    /// `v(c)` = `B[c]` when non-zero, else `S[c]` (§7.3).
    pub fn v(&self, row: i32, col: i32) -> i32 {
        let c = Self::at(row, col);
        if self.b[c] != 0 {
            self.b[c]
        } else {
            self.s[c]
        }
    }

    pub fn shade(&self, row: i32, col: i32) -> i32 {
        self.s[Self::at(row, col)]
    }

    pub fn set_blocker(&mut self, row: i32, col: i32, v: i32) {
        self.b[Self::at(row, col)] = v;
    }

    /// One cell of §7.3 r3: grid `(row, col)` is sub-tile `(sx, sy)`;
    /// `(x, y)` the light position.
    fn shade_cell(&mut self, row: i32, col: i32, sx: i32, sy: i32, x: i32, y: i32) {
        let dx = x - (8 * sx + 4);
        let dy = y - (8 * sy + 4);
        let sgx = if dx < 0 { -1 } else { 1 };
        let sgy = if dy < 0 { -1 } else { 1 };
        let (ax, ay) = (dx.abs(), dy.abs());
        let s = if dx == 0 && dy == 0 {
            return;
        } else if dx == 0 {
            self.v(row + sgy, col)
        } else if dy == 0 {
            self.v(row, col + sgx)
        } else if ay <= ax {
            let f = (ay << 8) / ax;
            ((256 - f) * self.v(row, col + sgx) + f * self.v(row + sgy, col + sgx)) >> 8
        } else {
            let f = (ax << 8) / ay;
            ((256 - f) * self.v(row + sgy, col) + f * self.v(row + sgy, col + sgx)) >> 8
        };
        self.s[Self::at(row, col)] = s;
    }

    /// Rings 2…m (§7.3 r2–r4) around grid centre = sub-tile `centre`, rays
    /// from the light position `(x, y)`. Rings 0 and 1 are not written.
    pub fn shade_rings(&mut self, m: i32, centre: (i32, i32), x: i32, y: i32) {
        for t in 2..=m {
            for s in 0..=t {
                let offs = [
                    (-s, -t),
                    (s, -t),
                    (-s, t),
                    (s, t),
                    (t, -s),
                    (t, s),
                    (-t, -s),
                    (-t, s),
                ];
                for (ox, oy) in offs {
                    self.shade_cell(CENTRE + oy, CENTRE + ox, centre.0 + ox, centre.1 + oy, x, y);
                }
            }
        }
    }
}

/// The shade grid of a shadowed record (§7.3 r1–r4), `None` when §7.1
/// r1–r2 give nothing.
pub fn shadow_grid(map: &LightMap, rec: &LightRecord) -> Option<ShadeGrid> {
    let r = rec.radius;
    window(map, rec.x, rec.y, r)?;
    let m = r >> 3;
    let (lx, ly) = (rec.x >> 3, rec.y >> 3);
    let (ox, oy) = map.origin;
    let mut g = ShadeGrid::default();
    for row in CENTRE - m..=CENTRE + m {
        for col in CENTRE - m..=CENTRE + m {
            let gx = lx + col - CENTRE - ox;
            let gy = ly + row - CENTRE - oy;
            let blocked = map.cell(gx, gy).is_none_or(|c| c.blocks != 0);
            g.set_blocker(row, col, if blocked { WALL } else { 0 });
        }
    }
    g.shade_rings(m, (lx, ly), rec.x, rec.y);
    Some(g)
}

/// §7.3 r5: the value added for base `b` at shade `s`.
pub fn shaded_value(b: i32, s: i32) -> Option<i32> {
    (s < WALL).then(|| (b * (8 - (s >> 1))) >> 3)
}

/// Shadowed contribution (§7.3, `0x00474D70`, kind 0 at `q` = 2).
pub fn shadowed(map: &mut LightMap, rec: &LightRecord, colored: bool) {
    let Some(g) = shadow_grid(map, rec) else {
        return;
    };
    let r = rec.radius;
    let m = r >> 3;
    let Some(w) = window(map, rec.x, rec.y, r) else {
        return;
    };
    spread(map, rec, r, w, colored, |j, i, b| {
        shaded_value(b, g.shade(CENTRE - m + j, CENTRE - m + i))
    });
}

/// Build the kind-2 cache (§7.4 r1): `B` over all 64 × 64 cells from
/// `blocks` at sub-tile `(ux + col − 32, uy + row − 32)`, rings 2…m, then
/// the `(2m + 1)²` window of `S` row by row. The ray geometry uses the
/// grid's own sub-tiles (centre `(ux, uy)`).
pub fn build_cache(
    rec: &LightRecord,
    owner_subtile: (i32, i32),
    blocks: impl Fn(i32, i32) -> bool,
) -> Vec<i32> {
    let m = rec.radius >> 3;
    let (ux, uy) = owner_subtile;
    let mut g = ShadeGrid::default();
    for row in 0..GRID {
        for col in 0..GRID {
            if blocks(ux + col - CENTRE, uy + row - CENTRE) {
                g.set_blocker(row, col, WALL);
            }
        }
    }
    g.shade_rings(m, (ux, uy), rec.x, rec.y);
    let mut out = Vec::with_capacity(((2 * m + 1) * (2 * m + 1)) as usize);
    for row in CENTRE - m..=CENTRE + m {
        for col in CENTRE - m..=CENTRE + m {
            out.push(g.shade(row, col));
        }
    }
    out
}

/// Cached contribution (§7.4, kind 2, every `q`): §7.1 r1, the cache
/// build when invalid, then §7.1 r2–r6 with `S` from the cache.
pub fn cached(
    map: &mut LightMap,
    rec: &mut LightRecord,
    world: &impl LightWorld,
    colored: bool,
) -> Result<(), LightError> {
    let r = rec.radius;
    if !(1..=255).contains(&r) {
        return Ok(());
    }
    if !rec.cache_valid {
        let owner = rec.owner().ok_or(LightError::CacheWithoutOwner)?;
        world
            .owner_room(&owner)
            .ok_or(LightError::CacheWithoutOwner)?;
        let sub = world
            .owner_subtile(&owner)
            .ok_or(LightError::CacheWithoutOwner)?;
        rec.cache = build_cache(rec, sub, |x, y| world.owner_blocks(&owner, x, y));
        rec.cache_valid = true;
    }
    cached_contribution(map, rec, colored);
    Ok(())
}

/// §7.4 r2: §7.1 r2–r6 with `S` from a record's built cache (§7.4 r1,
/// `cache_valid` set); nothing when §7.1 r1–r2 give nothing.
pub fn cached_contribution(map: &mut LightMap, rec: &LightRecord, colored: bool) {
    let r = rec.radius;
    if !(1..=255).contains(&r) {
        return;
    }
    let Some(w) = window(map, rec.x, rec.y, r) else {
        return;
    };
    let side = 2 * (r >> 3) + 1;
    spread(map, rec, r, w, colored, |j, i, b| {
        let s = rec.cache[(j * side + i) as usize];
        shaded_value(b, s)
    });
}
