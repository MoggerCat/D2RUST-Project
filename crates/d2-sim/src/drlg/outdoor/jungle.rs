// Spec: specs/drlg/outdoor-act3-act5.md
//! Act III jungle placer `0x00677880` (§2): the three jungle records
//! (§2.1, §2.2), the block grids (§2.3), river and attach points with
//! the restart point R (§2.4), river connections (§2.5), attach
//! directions (§2.6) and the code → lvlprest id table (§2.7). The
//! hand-off to levels 76..78 (§2.8 step 3) is `place.rs`. Every draw is
//! on the seed passed in (the DRLG seed itself).

use crate::rng::Seed;

use super::super::TileRect;
use super::place::{jungle_offsets, overlaps};
use super::OutdoorError;

/// Block size in tiles (§2).
pub const BLOCK: i32 = 32;

/// Clearing ids S (`0x006F13BC`), index D >> 4 for 0..15. S[0] (256) is
/// unreachable (edge case 2); 13..15 read 0.
pub const CLEARING_IDS: [u32; 16] = [
    256, 575, 576, 577, 578, 579, 580, 0, 581, 582, 583, 0, 584, 0, 0, 0,
];

/// River-with-exit ids T (`0x006F13F0` + 16·r), rows 1..14 (index r − 1);
/// slot 0 W, 1 E, 2 S, 3 N; 0 = none.
pub const EXIT_IDS: [[u32; 4]; 14] = [
    [0, 545, 546, 547],
    [548, 0, 549, 550],
    [0, 0, 551, 552],
    [553, 554, 0, 555],
    [0, 556, 0, 557],
    [558, 0, 0, 559],
    [0, 0, 0, 560],
    [561, 562, 563, 0],
    [0, 564, 565, 0],
    [566, 0, 567, 0],
    [0, 0, 568, 0],
    [569, 570, 0, 0],
    [0, 571, 0, 0],
    [572, 0, 0, 0],
];

/// One jungle record (§2.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JungleRec {
    /// Rect (+0x00..+0x0C), tiles.
    pub rect: TileRect,
    /// Case it was placed with (+0x10); record 0: 0.
    pub case: i32,
    /// Base record (+0x18); record 0: 0.
    pub base: usize,
    /// Branch records in placement order (+0x14 count, +0x1C..+0x24).
    pub branches: Vec<usize>,
    /// Block column and row (+0x28, +0x2C).
    pub bx: i32,
    pub by: i32,
    /// Block ids (+0x30), row-major from the top-left block.
    pub ids: Vec<u32>,
    /// Clearing count (+0x34).
    pub clearings: i32,
}

/// Fatal ids of §2 / §3.
pub mod fatal {
    pub const NO_IDS: u32 = 0x27;
    pub const CLEARINGS: u32 = 0x47;
    pub const BRANCHES: u32 = 0x63E;
    pub const SPAN_X: u32 = 0x64D;
    pub const SPAN_Y: u32 = 0x64E;
    pub const EXIT_ID: u32 = 0x78C;
    pub const CLEARING_ID: u32 = 0x799;
}

/// River exit bits of D (§2.3): N, S, E, W.
const RIVER_BITS: [i32; 4] = [8, 4, 2, 1];
/// Attach exit bits of D (§2.3, §2.6 direction order 0 N, 1 S, 2 E, 3 W).
const ATTACH_BITS: [i32; 4] = [0x80, 0x40, 0x20, 0x10];
/// Neighbour offsets (column, row) for N, S, E, W.
const DIRS: [(i32, i32); 4] = [(0, -1), (0, 1), (1, 0), (-1, 0)];
/// The opposite of direction i (N ↔ S, E ↔ W).
const OPPOSITE: [usize; 4] = [1, 0, 3, 2];

/// The four W×H block grids of §2.3.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JungleGrid {
    pub w: i32,
    pub h: i32,
    /// A: owner (jungle index + 1).
    pub a: Vec<i32>,
    /// B: river order.
    pub b: Vec<i32>,
    /// C: marks (0 none, 1 link, 2 attach point).
    pub c: Vec<i32>,
    /// D: code.
    pub d: Vec<i32>,
}

impl JungleGrid {
    pub fn new(w: i32, h: i32) -> Self {
        let n = (w.max(0) * h.max(0)) as usize;
        Self {
            w,
            h,
            a: vec![0; n],
            b: vec![0; n],
            c: vec![0; n],
            d: vec![0; n],
        }
    }

    /// Restart point R: all four grids := 0.
    pub fn clear(&mut self) {
        for g in [&mut self.a, &mut self.b, &mut self.c, &mut self.d] {
            g.iter_mut().for_each(|v| *v = 0);
        }
    }

    /// Index of (column, row), or None outside the grid. The outer ring
    /// keeps every read of §2.5–§2.6 inside (§2.3); the guard only keeps
    /// out-of-grid reads from panicking.
    pub fn at(&self, col: i32, row: i32) -> Option<usize> {
        (col >= 0 && row >= 0 && col < self.w && row < self.h)
            .then(|| (row * self.w + col) as usize)
    }

    fn get(g: &[i32], i: Option<usize>) -> i32 {
        i.map_or(0, |i| g[i])
    }

    pub fn a_at(&self, col: i32, row: i32) -> i32 {
        Self::get(&self.a, self.at(col, row))
    }

    pub fn b_at(&self, col: i32, row: i32) -> i32 {
        Self::get(&self.b, self.at(col, row))
    }

    pub fn c_at(&self, col: i32, row: i32) -> i32 {
        Self::get(&self.c, self.at(col, row))
    }

    pub fn d_at(&self, col: i32, row: i32) -> i32 {
        Self::get(&self.d, self.at(col, row))
    }

    fn set_b(&mut self, col: i32, row: i32, v: i32) {
        if let Some(i) = self.at(col, row) {
            self.b[i] = v;
        }
    }

    fn set_c(&mut self, col: i32, row: i32, v: i32) {
        if let Some(i) = self.at(col, row) {
            self.c[i] = v;
        }
    }

    fn or_d(&mut self, col: i32, row: i32, v: i32) {
        if let Some(i) = self.at(col, row) {
            self.d[i] |= v;
        }
    }

    /// River connections (§2.5): whole grid, row-major, no draws.
    pub fn connect_rivers(&mut self) {
        for row in 0..self.h {
            for col in 0..self.w {
                let (ac, bc, cc) = (
                    self.a_at(col, row),
                    self.b_at(col, row),
                    self.c_at(col, row),
                );
                let mut f = 0;
                // Step 1.
                if cc == 1 {
                    let mut best: Option<(usize, i32)> = None;
                    for (k, &(dx, dy)) in DIRS.iter().enumerate() {
                        let (nc, nr) = (col + dx, row + dy);
                        let nb = self.b_at(nc, nr);
                        if nb != 0 && self.a_at(nc, nr) == ac && best.is_none_or(|(_, bb)| nb < bb)
                        {
                            best = Some((k, nb));
                        }
                    }
                    if let Some((k, _)) = best {
                        f = RIVER_BITS[k];
                        let (dx, dy) = DIRS[k];
                        self.or_d(col + dx, row + dy, RIVER_BITS[OPPOSITE[k]]);
                    }
                    for (k, &(dx, dy)) in DIRS.iter().enumerate() {
                        let (nc, nr) = (col + dx, row + dy);
                        if self.c_at(nc, nr) == 1 && self.a_at(nc, nr) != ac {
                            f |= RIVER_BITS[k];
                        }
                    }
                }
                // Step 2 (owners not compared, edge case 5).
                if bc != 0 {
                    for (k, &(dx, dy)) in DIRS.iter().enumerate() {
                        if (self.b_at(col + dx, row + dy) - bc).abs() == 1 {
                            f |= RIVER_BITS[k];
                        }
                    }
                }
                // Step 3.
                self.or_d(col, row, f);
            }
        }
    }

    /// The first direction from `d` (order (d + i) & 3) whose neighbour
    /// passes `test`.
    fn search(
        &self,
        col: i32,
        row: i32,
        d: u32,
        test: impl Fn(usize, i32, i32) -> bool,
    ) -> Option<usize> {
        (0..4u32)
            .map(|i| ((d + i) & 3) as usize)
            .find(|&k| test(k, col + DIRS[k].0, row + DIRS[k].1))
    }

    /// Attach directions (§2.6): whole grid, row-major. Returns false
    /// when an attach point found no river block (restart at R, §2.6
    /// step 2); the draws made so far stay made.
    pub fn attach(&mut self, seed: &mut Seed) -> bool {
        for row in 0..self.h {
            for col in 0..self.w {
                // Site 0x006782AA, before any test.
                let mut d = seed.mask(4);
                let mut u = self.d_at(col, row);
                let empty = u == 0;
                let ac = self.a_at(col, row);
                if self.c_at(col, row) == 2 {
                    // Step 1.
                    let first = self.search(col, row, d, |_, nc, nr| {
                        let nd = self.d_at(nc, nr);
                        self.a_at(nc, nr) == ac && 0 < nd && nd < 15
                    });
                    if let Some(k) = first {
                        u |= ATTACH_BITS[k];
                    }
                    // Step 2.
                    if u == 0 {
                        return false;
                    }
                    // Step 3.
                    if empty {
                        let second = self.search(col, row, d, |_, nc, nr| {
                            self.a_at(nc, nr) != ac && self.c_at(nc, nr) == 2
                        });
                        if let Some(k) = second {
                            u |= ATTACH_BITS[k];
                        }
                        // Site 0x006784AE.
                        let m = seed.mask(2);
                        let more = m == 1 && second.is_none();
                        // Site 0x006784D9.
                        d = seed.mask(4);
                        if more {
                            let third = self.search(col, row, d, |k, nc, nr| {
                                let nd = self.d_at(nc, nr);
                                0 < nd && nd < 15 && u & ATTACH_BITS[k] == 0
                            });
                            if let Some(k) = third {
                                u |= ATTACH_BITS[k];
                            }
                        }
                    }
                    // Step 4.
                    for k in 0..4 {
                        if u & ATTACH_BITS[k] != 0 {
                            let (dx, dy) = DIRS[k];
                            self.or_d(col + dx, row + dy, ATTACH_BITS[OPPOSITE[k]]);
                        }
                    }
                    self.set_c(col, row, 0);
                }
                self.or_d(col, row, u);
            }
        }
        true
    }
}

/// Code → lvlprest id (§2.7, `0x00678670`). A chained lookup whose row
/// is not 1..14 (a preset id from an earlier lookup, edge case 1, or 0)
/// gives a value that is not a lvlprest id: fatal 0x78C (OQ 2).
pub fn code_to_id(d: i32) -> Result<u32, OutdoorError> {
    if d == 0 {
        return Ok(0);
    }
    let l = d & 15;
    if l == 0 {
        // D ≥ 16: a clearing.
        let id = CLEARING_IDS[((d >> 4) & 15) as usize];
        if id == 0 {
            return Err(OutdoorError::Fatal(fatal::CLEARING_ID));
        }
        return Ok(id);
    }
    if d < 16 {
        return Ok(529 + l as u32);
    }
    let mut r = l as u32;
    for (slot, bit) in [0x10, 0x20, 0x40, 0x80].into_iter().enumerate() {
        if d & bit != 0 {
            // TODO(spec edge case 1, OQ 2): rows outside 1..14 read memory
            // past T (0 for slots 0 and 2, ~1.07e9 for 1 and 3; row 0 slot 3
            // is S[16], not given); every such value is fatal 0x78C here.
            let Some(row) = (1..=14).contains(&r).then(|| EXIT_IDS[r as usize - 1]) else {
                return Err(OutdoorError::Fatal(fatal::EXIT_ID));
            };
            r = row[slot];
        }
    }
    if r == 0 {
        return Err(OutdoorError::Fatal(fatal::EXIT_ID));
    }
    Ok(r)
}

/// The jungle placer's result: records in placement order and the level
/// order of §2.8 step 2 (indices into the records, level 76 first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jungles {
    pub recs: Vec<JungleRec>,
    pub order: [usize; 3],
    pub grid: JungleGrid,
}

/// Jungle placer `0x00677880` (§2.2–§2.8 steps 1–2) on `seed` (the DRLG
/// seed itself). `docks` is level 75's rect; `sx`, `sy` the jungle size
/// (leveldefs 76).
pub fn place_jungles(
    seed: &mut Seed,
    docks: TileRect,
    sx: i32,
    sy: i32,
) -> Result<Jungles, OutdoorError> {
    let (sxb, syb) = (sx / BLOCK, sy / BLOCK);
    let (y1, y3) = jungle_offsets(sy);
    // §2.2 step 1.
    let mut recs = vec![JungleRec {
        rect: TileRect::new(docks.x, docks.y - sy, sx, sy),
        ..JungleRec::default()
    }];
    let (mut min_x, mut max_x) = (docks.x, docks.x + sx);
    let (mut min_y, max_y) = (docks.y - sy, docks.y);
    // Step 2.
    let mut k = 1i32;
    while k < 3 {
        // Site 0x00677966 (k = 1 steps and gives 0).
        let base = seed.roll(k) as usize;
        // Site 0x0067799A.
        let case = (seed.step() % 5) as i32;
        let (ox, oy) = match case {
            0 => (0, -sy),
            1 => (-sx, y1),
            2 => (sx, y1),
            3 => (-sx, y3),
            _ => (sx, y3),
        };
        let b = recs[base].rect;
        let rect = TileRect::new(b.x + ox, b.y + oy, sx, sy);
        if recs.iter().any(|r| overlaps(&r.rect, &rect)) {
            continue;
        }
        if recs[base].branches.len() >= 3 {
            return Err(OutdoorError::Fatal(fatal::BRANCHES));
        }
        let new = recs.len();
        recs[base].branches.push(new);
        recs.push(JungleRec {
            rect,
            case,
            base,
            ..JungleRec::default()
        });
        min_x = min_x.min(rect.x);
        min_y = min_y.min(rect.y);
        max_x = max_x.max(rect.x + rect.w);
        k += 1;
    }
    // Step 3.
    if (max_x - min_x) % BLOCK != 0 {
        return Err(OutdoorError::Fatal(fatal::SPAN_X));
    }
    if (max_y - min_y) % BLOCK != 0 {
        return Err(OutdoorError::Fatal(fatal::SPAN_Y));
    }
    // §2.3.
    let mut g = JungleGrid::new((max_x - min_x) / BLOCK + 2, (max_y - min_y) / BLOCK + 2);
    for r in &mut recs {
        r.bx = (r.rect.x - min_x) / BLOCK + 1;
        r.by = (r.rect.y - min_y) / BLOCK + 1;
    }
    // §2.4–§2.6 with the restart point R.
    loop {
        g.clear();
        for k in 0..recs.len() {
            river_and_attach(&mut g, seed, &recs, k, sxb, syb);
        }
        g.connect_rivers();
        if g.attach(seed) {
            break;
        }
    }
    // §2.7 (whole grid, row-major) and §2.8 step 1.
    let mut ids = Vec::with_capacity(g.d.len());
    for &d in &g.d {
        ids.push(code_to_id(d)?);
    }
    for r in &mut recs {
        r.ids = (0..sxb * syb)
            .map(|i| g.at(r.bx + i % sxb, r.by + i / sxb).map_or(0, |j| ids[j]))
            .collect();
        r.clearings = r.ids.iter().filter(|&&id| id > 574).count() as i32;
    }
    // Step 2: bubble passes, swapping when the first y is smaller.
    let mut order = [0usize, 1, 2];
    loop {
        let mut swapped = false;
        for i in 0..2 {
            if recs[order[i]].rect.y < recs[order[i + 1]].rect.y {
                order.swap(i, i + 1);
                swapped = true;
            }
        }
        if !swapped {
            break;
        }
    }
    Ok(Jungles {
        recs,
        order,
        grid: g,
    })
}

/// §2.4 for jungle k (steps 1–4): side, passes, drops.
pub fn river_and_attach(
    g: &mut JungleGrid,
    seed: &mut Seed,
    recs: &[JungleRec],
    k: usize,
    sxb: i32,
    syb: i32,
) {
    let rec = &recs[k];
    let (bx, by) = (rec.bx, rec.by);
    // Step 2.
    let mut s = if k == 0 {
        // Site 0x00677C43.
        seed.mask(2) as i32
    } else {
        i32::from(rec.case % 2 == 1)
    };
    let opposite = |col: i32, s: i32| if s == 0 { col + 1 } else { col - 1 };
    let mut n;
    // Step 3: passes while n < 2.
    loop {
        // 3.1.
        for row in by..by + syb {
            for col in bx..bx + sxb {
                if let Some(i) = g.at(col, row) {
                    g.a[i] = k as i32 + 1;
                    g.b[i] = 0;
                    g.c[i] = 0;
                    g.d[i] = 0;
                }
            }
        }
        // 3.2.
        let mut col = bx + s;
        let last = by + syb - 1;
        // 3.3.
        if k > 0 {
            g.set_c(col, last, 1);
            g.set_c(opposite(col, s), last, 2);
        }
        // 3.4.
        for &br in &rec.branches {
            let (cx, cy) = match recs[br].case {
                0 => (bx, by),
                1 => (bx, by + 3),
                2 => (bx + 1, by + 3),
                3 => (bx, by + 1),
                _ => (bx + 1, by + 1),
            };
            g.set_c(cx, cy, 1);
        }
        // 3.5.
        n = i32::from(k > 0);
        let mut run = 0;
        let mut v = 100 * (k as i32 + 1);
        let mut row = last;
        // 3.6.
        while row >= by {
            g.set_b(col, row, v);
            v += 1;
            let advance = if run == 0 || row == by {
                true
            } else {
                // Site 0x00677E4F.
                let t = seed.step() % 3;
                if t != 0 {
                    true
                } else {
                    run = 0;
                    if s == 0 {
                        col += 1;
                        s = 1;
                    } else {
                        col -= 1;
                        s = 0;
                    }
                    false
                }
            };
            if advance {
                run += 1;
                let opp = opposite(col, s);
                if run >= 2 && row >= 2 && g.c_at(opp, row) == 0 {
                    g.set_c(opp, row, 2);
                    n += 1;
                }
                row -= 1;
            }
        }
        if n >= 2 {
            break;
        }
    }
    // Step 4: drops while n > 3.
    while n > 3 {
        // Site 0x00677F1C: lo' & (n−1) for a power of two, else lo' mod n
        // (`roll`, rng.md §3: the same value).
        let i = seed.roll(n) as usize;
        let hit = (by..by + syb)
            .flat_map(|row| (bx..bx + sxb).map(move |col| (col, row)))
            .filter(|&(col, row)| g.c_at(col, row) == 2)
            .nth(i);
        // TODO(spec §2.4 step 4): fewer than i + 1 marks (n counts a mark a
        // later step overwrote) is not described; nothing is dropped.
        if let Some((col, row)) = hit {
            g.set_c(col, row, 0);
        }
        n -= 1;
    }
}

#[cfg(test)]
#[path = "act3_tests.rs"]
mod act3_tests;
