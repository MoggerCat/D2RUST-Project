// Spec: specs/drlg/outdoor.md
//! Act-wide outdoor placement `0x00678AD0` (§2): link tables, the link
//! driver and its linkers, placement cases, checks, Act I border flags,
//! warps and neighbour entries; Act III's jungle placer and Kurast chain
//! (§9.1, §9.2).

use crate::rng::Seed;

use super::super::data::DrlgData;
use super::super::level::Drlg;
use super::super::room::near_gaps;
use super::super::seams::LevelTypes;
use super::super::{LevelIdx, TileRect};
use super::{Orth, Outdoor, OutdoorData, OutdoorError, DRLG_OUTDOOR};

/// Rows per link table (§2.2).
pub const TABLE_ROWS: usize = 15;

/// The linkers of §2.4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Linker {
    Def,
    R4,
    R8,
    Bm,
    Re,
    Fix,
    Rw,
    Vs,
    Os,
    B1,
    Bd,
    B2,
}

/// A link-table row: (linker, level, link row or −1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkRow {
    pub linker: Linker,
    pub level: u32,
    pub link: i32,
}

const fn row(linker: Linker, level: u32, link: i32) -> LinkRow {
    LinkRow {
        linker,
        level,
        link,
    }
}

use Linker::*;

/// A1W `0x006F0750`.
pub const A1W: [LinkRow; 5] = [
    row(Def, 4, -1),
    row(R4, 3, 0),
    row(Bm, 2, 1),
    row(Re, 1, 2),
    row(R4, 17, 1),
];
/// A1M `0x006F0840`.
pub const A1M: [LinkRow; 5] = [
    row(Def, 39, -1),
    row(Def, 26, -1),
    row(Fix, 7, 1),
    row(R4, 6, 2),
    row(R4, 5, 3),
];
/// A2 `0x006F0930`.
pub const A2: [LinkRow; 6] = [
    row(Def, 40, -1),
    row(Rw, 41, 0),
    row(R8, 42, 1),
    row(R8, 43, 2),
    row(R8, 44, 3),
    row(Vs, 45, 4),
];
/// A2C `0x006F0A20`.
pub const A2C: [LinkRow; 1] = [row(Def, 46, -1)];
/// A5U `0x006F0B10`.
pub const A5U: [LinkRow; 2] = [row(Def, 134, -1), row(Def, 136, -1)];
/// A4 `0x006F0C00`.
pub const A4: [LinkRow; 4] = [
    row(Def, 103, -1),
    row(Os, 104, 0),
    row(R4, 105, 1),
    row(R4, 106, 2),
];
/// A4C `0x006F0CF0`.
pub const A4C: [LinkRow; 1] = [row(Def, 108, -1)];
/// A5 `0x006F0DE0`.
pub const A5: [LinkRow; 4] = [
    row(Def, 109, -1),
    row(Def, 110, 0),
    row(B1, 111, 1),
    row(B2, 112, 2),
];
/// A5T `0x006F0ED0`.
pub const A5T: [LinkRow; 1] = [row(Bd, 117, -1)];

/// Driver checks (§2.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    None,
    /// `0x00676DD0`.
    A1W,
    /// `0x00676EB0`.
    A1M,
    /// `0x00676F50`, `0x00676FC0`, `0x006770A0`, `0x00677110`,
    /// `0x00677030`: rows j < i, j ≠ link, must not overlap.
    Simple,
}

/// Rogue check table `0x006F1158`: the indices holding 1.
pub const ROGUE_OK: [usize; 16] = [0, 1, 9, 19, 21, 24, 30, 31, 33, 39, 41, 53, 54, 56, 62, 63];

/// Act I flag rows `0x006F1258`: (level filter, excl1, excl2, r, rNext,
/// flags), filter 0 = any.
pub const ACT1_FLAG_ROWS: [(u32, u32, u32, i32, i32, u32); 15] = [
    (0, 2, 3, 1, 0, 0x4),
    (0, 2, 3, 2, 3, 0x4),
    (0, 3, 17, 2, 1, 0x8),
    (0, 3, 17, 3, 0, 0x8),
    (0, 3, 17, 1, 1, 0x10),
    (0, 3, 17, 3, 3, 0x10),
    (2, 0, 0, 0, 0, 0x8),
    (2, 0, 0, 2, 2, 0x8),
    (2, 0, 0, 3, 0, 0x8),
    (2, 0, 0, 3, 2, 0x8),
    (2, 0, 0, 0, 1, 0x400),
    (2, 0, 0, 1, 1, 0x400),
    (2, 0, 0, 2, 1, 0x200),
    (2, 0, 0, 2, 2, 0x80),
    (2, 0, 0, 3, 2, 0x100),
];

/// B2 offset table (§2.4), index R0 + 2·R0[link].
pub const B2_OFFSETS: [(i32, i32); 4] = [(0, -160), (-96, -64), (-64, -96), (-160, 0)];

/// The link driver's state (§2.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Driver {
    pub c: [TileRect; TABLE_ROWS],
    /// R0..R3 per row.
    pub r: [[i32; TABLE_ROWS]; 4],
    /// The driver's copy of the DRLG seed.
    pub seed: Seed,
    /// The OS linker's transition flag (global `0x0096D66C`).
    pub transition: u32,
}

impl Driver {
    pub fn new(seed: Seed) -> Self {
        Self {
            c: [TileRect::default(); TABLE_ROWS],
            r: [[-1; TABLE_ROWS]; 4],
            seed,
            transition: 0,
        }
    }
}

/// Place A `0x00675DE0` (§2.5).
pub fn place_a(p: TileRect, c: &mut TileRect, case: i32, v: i32) {
    match case {
        0 => {
            (c.x, c.y) = (p.x, p.y + p.h);
            if v == 1 {
                c.x -= 16;
            }
        }
        1 => {
            (c.x, c.y) = (p.x - c.w, p.y);
            match v {
                1 => c.y -= 16,
                2 => c.y += 8,
                _ => {}
            }
        }
        2 => {
            (c.x, c.y) = (p.x + p.w - c.w, p.y - c.h);
            if v == 1 {
                c.x += 16;
            }
        }
        3 => {
            (c.x, c.y) = (p.x + p.w, p.y + p.h - c.h);
            match v {
                1 => c.y += 16,
                2 => c.y -= 8,
                3 => c.y += 8,
                _ => {}
            }
        }
        _ => {}
    }
}

/// Place B `0x00675EB0` (§2.5).
pub fn place_b(p: TileRect, c: &mut TileRect, case: i32, v: i32) {
    match case {
        0 => {
            (c.x, c.y) = (p.x + p.w - c.w, p.y + p.h);
            if v == 1 {
                c.x += 16;
            }
        }
        1 => {
            (c.x, c.y) = (p.x - c.w, p.y + p.h - c.h);
            match v {
                1 => c.y += 16,
                2 => c.y -= 8,
                _ => {}
            }
        }
        2 => {
            (c.x, c.y) = (p.x, p.y - c.h);
            if v == 1 {
                c.x -= 16;
            }
        }
        3 => {
            (c.x, c.y) = (p.x + p.w, p.y);
            match v {
                1 => c.y -= 16,
                2 => c.y += 8,
                3 => c.y -= 8,
                _ => {}
            }
        }
        _ => {}
    }
}

/// Place C `0x00675F80` (§2.5, Act II): the half-size shifts apply only
/// with variant 1.
pub fn place_c(p: TileRect, c: &mut TileRect, case: i32, v: i32) {
    let hx = if v == 1 { c.w / 2 + 8 } else { 0 };
    let hy = if v == 1 { c.h / 2 + 8 } else { 0 };
    match case {
        0 => (c.x, c.y) = (p.x - hx, p.y + p.h),
        1 => (c.x, c.y) = (p.x + hx, p.y + p.h),
        2 => (c.x, c.y) = (p.x - c.w, p.y - hy),
        3 => (c.x, c.y) = (p.x - c.w, p.y + hy),
        4 => (c.x, c.y) = (p.x - hx, p.y - c.h),
        5 => (c.x, c.y) = (p.x + hx, p.y - c.h),
        6 => (c.x, c.y) = (p.x + p.w, p.y - hy),
        7 => (c.x, c.y) = (p.x + p.w, p.y + hy),
        _ => {}
    }
}

/// Gap test `0x0066B800` via `0x0066B860`, margin 0 (§2.6): overlapping
/// unless one gap is ≥ 0 (touching allowed).
pub fn overlaps(a: &TileRect, b: &TileRect) -> bool {
    let (gx, gy) = near_gaps(a, b);
    !(gx >= 0 || gy >= 0)
}

/// Shares an edge `0x0066B880`, margin −1 (§2.7): one gap 0, the other
/// ≤ −1.
pub fn shares_edge(a: &TileRect, b: &TileRect) -> bool {
    let (gx, gy) = near_gaps(a, b);
    (gx == 0 && gy <= -1) || (gy == 0 && gx <= -1)
}

/// Direction from A to B `0x00642240` (`maze.md` §2.6): 0 W, 1 N, 2 E,
/// 3 S, −1 none.
pub fn direction(a: &TileRect, b: &TileRect) -> i32 {
    if b.x < a.x && a.x == b.x + b.w {
        0
    } else if b.x >= a.x && b.x == a.x + a.w {
        2
    } else if b.y < a.y && a.y == b.y + b.h {
        1
    } else if b.y >= a.y && b.y == a.y + a.h {
        3
    } else {
        -1
    }
}

/// One linker call (§2.4). Returns false when its alternatives are used
/// up.
fn run_linker(
    d: &mut Driver,
    rows: &[LinkRow],
    i: usize,
    data: &DrlgData,
) -> Result<bool, OutdoorError> {
    let row = rows[i];
    let first = d.r[1][i] == -1;
    let parent = if row.link >= 0 {
        d.c[row.link as usize]
    } else {
        TileRect::default()
    };
    let mut c = d.c[i];
    match row.linker {
        Def => {
            d.r[0][i] = -1;
            let (x, y) = data.level(row.level)?.offset;
            (c.x, c.y) = (x, y);
        }
        R4 | R8 | Vs => {
            let n = if row.linker == R4 { 4 } else { 8 };
            if first {
                let v = d.seed.mask(n as u32) as i32;
                d.r[0][i] = v;
                d.r[1][i] = v;
            } else {
                let next = (d.r[0][i] + 1) % n;
                if next == d.r[1][i] {
                    return Ok(false);
                }
                d.r[0][i] = next;
            }
            match row.linker {
                R4 => place_a(parent, &mut c, d.r[0][i], 1),
                R8 => place_c(parent, &mut c, d.r[0][i], 1),
                _ => place_c(parent, &mut c, d.r[0][i], 0),
            }
        }
        Bm | Re => {
            let t;
            if first {
                let r0 = d.seed.mask(4) as i32;
                d.r[0][i] = r0;
                d.r[1][i] = r0;
                d.r[3][i] = d.seed.mask(2) as i32;
                t = d.r[3][i];
            } else {
                t = (d.r[2][i] + 1) % 2;
                let r = (d.r[2][i] + d.r[0][i]) % 4;
                if r == d.r[1][i] && t == d.r[3][i] {
                    return Ok(false);
                }
                d.r[0][i] = r;
            }
            d.r[2][i] = t;
            let case = d.r[0][i];
            let v = if row.linker == Bm { 1 } else { 2 };
            if row.linker == Bm {
                (c.w, c.h) = if case % 2 == 1 { (96, 56) } else { (56, 96) };
            }
            if t == 1 {
                place_a(parent, &mut c, case, v);
            } else {
                place_b(parent, &mut c, case, v);
            }
        }
        Fix => {
            d.r[0][i] = 0;
            d.r[1][i] = 0;
            place_a(parent, &mut c, 0, 0);
        }
        Rw => {
            if first {
                let v = d.seed.mask(2) as i32 + 1;
                d.r[0][i] = v;
                d.r[1][i] = v;
            } else {
                let r = if d.r[0][i] == 1 { 2 } else { 1 };
                if r == d.r[1][i] {
                    return Ok(false);
                }
                d.r[0][i] = r;
            }
            if d.r[0][i] == 1 {
                place_a(parent, &mut c, 1, 0);
            } else {
                place_b(parent, &mut c, d.r[0][i], 0);
            }
        }
        Os => {
            d.r[0][i] = 3;
            d.r[1][i] = 3;
            if d.seed.mask(2) == 0 {
                place_b(parent, &mut c, 3, 3);
                d.transition = 0x40_0000;
            } else {
                place_a(parent, &mut c, 3, 3);
                d.transition = 0x80_0000;
            }
        }
        B1 | Bd => {
            let v = d.seed.mask(2) as i32;
            d.r[0][i] = v;
            d.r[1][i] = v;
            (c.w, c.h) = if v == 0 { (64, 160) } else { (160, 64) };
            if row.linker == B1 {
                c.x = parent.x - c.w;
                c.y = parent.y + parent.h - c.h - 16;
            } else {
                let (x, y) = data.level(row.level)?.offset;
                (c.x, c.y) = (x, y);
            }
        }
        B2 => {
            if first {
                let v = d.seed.mask(2) as i32;
                d.r[0][i] = v;
                d.r[1][i] = v;
            } else {
                let r = i32::from(d.r[0][i] == 0);
                if r == d.r[1][i] {
                    return Ok(false);
                }
                d.r[0][i] = r;
            }
            let r0 = d.r[0][i];
            (c.w, c.h) = if r0 == 0 { (64, 160) } else { (160, 64) };
            let link_r0 = if row.link >= 0 {
                d.r[0][row.link as usize]
            } else {
                0
            };
            // TODO(outdoor.md §2.4): an index outside 0..3 (R0[link] not 0
            // or 1) reads past the 4-entry table; not reached in 1.14d
            // (link is the B1 row).
            let (ox, oy) = B2_OFFSETS
                .get((r0 + 2 * link_r0) as usize)
                .copied()
                .unwrap_or((0, 0));
            (c.x, c.y) = (parent.x + ox, parent.y + oy);
        }
    }
    d.c[i] = c;
    Ok(true)
}

/// The driver checks (§2.6).
fn check(d: &Driver, rows: &[LinkRow], i: usize, check: Check) -> bool {
    let link = rows[i].link;
    let others_clear = |d: &Driver| (0..i).all(|j| j as i32 == link || !overlaps(&d.c[j], &d.c[i]));
    match check {
        Check::None => true,
        Check::Simple => others_clear(d),
        Check::A1W => {
            if !others_clear(d) {
                return false;
            }
            match rows[i].level {
                1 if link >= 0 => {
                    let l = link as usize;
                    let idx = d.r[0][i] + 4 * (d.r[2][i] + 2 * (d.r[0][l] + 4 * d.r[2][l]));
                    idx >= 0 && ROGUE_OK.contains(&(idx as usize))
                }
                17 => !rows
                    .iter()
                    .enumerate()
                    .any(|(j, r)| j != i && r.link == link && d.r[0][j] == d.r[0][i]),
                _ => true,
            }
        }
        Check::A1M => {
            if !others_clear(d) {
                return false;
            }
            if i > 0 {
                let mut r0 = d.c[0];
                r0.y -= 200;
                r0.h += 200;
                if overlaps(&r0, &d.c[i]) {
                    return false;
                }
            }
            true
        }
    }
}

impl Outdoor {
    /// `0x00678AD0` (§2.1): place the act's outdoor levels. `types`
    /// allocates levels (`levels.md` §4.2); outdoor inits may be no-ops
    /// there ([`Outdoor::info_mut`]).
    pub fn create_act_levels(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        _od: &OutdoorData,
        types: &mut dyn LevelTypes,
    ) -> Result<(), OutdoorError> {
        match drlg.act {
            0 => {
                self.drive(drlg, data, types, &A1W, Check::A1W, true)?;
                self.drive(drlg, data, types, &A1M, Check::A1M, true)?;
                self.neighbours(drlg, data, 1, 17)?;
            }
            1 => {
                self.drive(drlg, data, types, &A2, Check::Simple, false)?;
                self.drive(drlg, data, types, &A2C, Check::Simple, false)?;
                self.neighbours(drlg, data, 40, 46)?;
            }
            2 => {
                let l = drlg.get_or_alloc_level(data, types, 75)?;
                let def = data.level(75)?;
                let (w, h) = def.size[(drlg.difficulty as usize).min(2)];
                drlg.level_mut(l).rect = TileRect::new(def.offset.0, def.offset.1, w, h);
                self.jungles(drlg, data, types)?;
                self.kurast_chain(drlg, data, types)?;
                self.adjacency(drlg, data, 75, 83)?;
                self.neighbours(drlg, data, 75, 83)?;
            }
            3 => {
                let t1 = self.drive(drlg, data, types, &A4, Check::Simple, false)?;
                self.drive(drlg, data, types, &A4C, Check::Simple, false)?;
                // The global keeps the last OS linker's flag.
                if let Some(l) = drlg.find_level(104) {
                    self.info_mut(l).flags |= t1.transition;
                }
                self.neighbours(drlg, data, 103, 106)?;
            }
            4 => {
                self.drive(drlg, data, types, &A5, Check::None, false)?;
                self.drive(drlg, data, types, &A5T, Check::None, false)?;
                self.adjacency(drlg, data, 111, 112)?;
                self.neighbours(drlg, data, 111, 112)?;
                self.adjacency(drlg, data, 110, 111)?;
                self.adjacency(drlg, data, 109, 110)?;
                self.drive(drlg, data, types, &A5U, Check::Simple, false)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// The link driver `0x006772C0` (§2.3).
    pub fn drive(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        rows: &[LinkRow],
        chk: Check,
        flags_fn: bool,
    ) -> Result<Driver, OutdoorError> {
        let act5 = drlg.act == 4;
        // Step 1: the driver's own copy of the DRLG seed.
        let mut d = Driver::new(drlg.seed);
        let diff = (drlg.difficulty as usize).min(2);
        for (i, r) in rows.iter().enumerate() {
            let (w, h) = data.level(r.level)?.size[diff];
            d.c[i].w = w;
            d.c[i].h = h;
        }
        // Step 3.
        let mut i = 0usize;
        while i < rows.len() && rows[i].level != 0 {
            if !run_linker(&mut d, rows, i, data)? {
                for k in 0..4 {
                    d.r[k][i] = -1;
                }
                // TODO(outdoor.md edge case 2): a first row that runs out of
                // alternatives would index row −1; not reached in 1.14d.
                i = i.checked_sub(1).ok_or(OutdoorError::DriverUnderflow)?;
                continue;
            }
            if check(&d, rows, i, chk) {
                i += 1;
            }
        }
        // Step 4.
        for (i, r) in rows.iter().enumerate() {
            let l = drlg.get_or_alloc_level(data, types, r.level)?;
            drlg.level_mut(l).rect = d.c[i];
            if drlg.level(l).drlg_type == 2 {
                match r.level {
                    1 => {
                        self.preset_direction.insert(1, d.r[0][i]);
                    }
                    40 => {
                        let v = d.r[0].get(i + 1).copied().unwrap_or(-1);
                        self.preset_direction.insert(40, v);
                    }
                    _ => {}
                }
            }
            if r.level == 6 {
                drlg.get_or_alloc_level(data, types, 27)?;
                match d.r[0][i] {
                    1 => {
                        // Site 0x006774DB, the DRLG seed itself.
                        let v = drlg.seed.step();
                        self.preset_direction.insert(27, 2 - (v & 1) as i32);
                    }
                    3 => {
                        // Site 0x006774C0.
                        let v = drlg.seed.step();
                        self.preset_direction.insert(27, 1 - (v & 1) as i32);
                    }
                    _ => {}
                }
            }
            if flags_fn && drlg.level(l).drlg_type == DRLG_OUTDOOR {
                self.act1_flags(l, r.level, &d, i);
            }
            if !act5 && r.link >= 0 {
                let other = rows[r.link as usize].level;
                drlg.set_warp(data, r.level, other, -1, -1)?;
                drlg.set_warp(data, other, r.level, -1, -1)?;
            }
        }
        Ok(d)
    }

    /// Flags function `0x00677180` (§2.6).
    pub fn act1_flags(&mut self, l: LevelIdx, id: u32, d: &Driver, row: usize) {
        let r = d.r[0][row];
        let r_next = d.r[0].get(row + 1).copied().unwrap_or(-1);
        let mut flags = 0;
        for &(filter, e1, e2, a, b, f) in &ACT1_FLAG_ROWS {
            if (filter == 0 || filter == id) && id != e1 && id != e2 && a == r && b == r_next {
                flags |= f;
            }
        }
        self.info_mut(l).flags |= flags;
    }

    /// Adjacency warps `0x006775C0` (§2.7).
    pub fn adjacency(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        a: u32,
        b: u32,
    ) -> Result<(), OutdoorError> {
        for i in a..=b {
            for j in a..=b {
                if i == j {
                    continue;
                }
                // TODO(outdoor.md §2.7): an unallocated level in a..b is
                // not described; skipped.
                let (Some(li), Some(lj)) = (drlg.find_level(i), drlg.find_level(j)) else {
                    continue;
                };
                if shares_edge(&drlg.level(li).rect, &drlg.level(lj).rect) {
                    drlg.set_warp(data, i, j, -1, -1)?;
                }
            }
        }
        Ok(())
    }

    /// Neighbour entries `0x00677680` (§2.7).
    pub fn neighbours(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        a: u32,
        b: u32,
    ) -> Result<(), OutdoorError> {
        for id in a..=b {
            if data.level(id)?.drlg_type != DRLG_OUTDOOR {
                continue;
            }
            // TODO(outdoor.md §2.7): an unallocated outdoor level or
            // neighbour is not described; skipped.
            let Some(l) = drlg.find_level(id) else {
                continue;
            };
            let vis = drlg.vis_array(data, id)?;
            let warp = drlg.warp_array(data, id)?;
            for j in 0..8 {
                if vis[j] == 0 || warp[j] != -1 {
                    continue;
                }
                let Some(n) = drlg.find_level(vis[j]) else {
                    continue;
                };
                let rect = drlg.level(n).rect;
                let entry = Orth {
                    level_id: vis[j],
                    direction: direction(&drlg.level(l).rect, &rect),
                    init: false,
                    rect,
                    preset: drlg.level(n).drlg_type == 2,
                };
                // TODO(outdoor.md §1.4): the insertion order of
                // `0x0066B790` is not stated; head insertion used (the
                // polygon and link probes walk this list in order).
                self.info_mut(l).orth.insert(0, entry);
            }
        }
        Ok(())
    }

    /// Jungle placer `0x00677880` (§9.1), on the DRLG seed itself.
    fn jungles(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
    ) -> Result<(), OutdoorError> {
        let diff = (drlg.difficulty as usize).min(2);
        let (sx, sy) = data.level(76)?.size[diff];
        let docks = drlg
            .find_level(75)
            .map(|l| drlg.level(l).rect)
            .ok_or(OutdoorError::LevelMissing(75))?;
        let (y1, y3) = jungle_offsets(sy);
        // TODO(outdoor.md §9.1, OQ 7): "measured in 32-tile blocks" is not
        // used by the rules given; blocks are kept in tiles.
        let mut blocks = vec![TileRect::new(docks.x, docks.y - sy, sx, sy)];
        for k in 1..=2i32 {
            loop {
                let base = drlg.seed.roll(k) as usize;
                let case = drlg.seed.step() % 5;
                let (ox, oy) = match case {
                    0 => (0, -sy),
                    1 => (-sx, y1),
                    2 => (sx, y1),
                    3 => (-sx, y3),
                    _ => (sx, y3),
                };
                let b = blocks[base];
                let nb = TileRect::new(b.x + ox, b.y + oy, sx, sy);
                if blocks.iter().any(|e| overlaps(e, &nb)) {
                    continue;
                }
                blocks.push(nb);
                break;
            }
        }
        // TODO(outdoor.md §9.1, OQ 7): the attach-point grid (draws
        // roll(2), roll(3), roll(count), roll(4), roll(2), roll(4) at sites
        // 0x00677C43..0x006784D9) and the per-level jungle preset ids are
        // not specified; not drawn here, so Act III creation draws fewer
        // times than 1.14d.
        let mut order: Vec<TileRect> = blocks;
        order.sort_by_key(|a| std::cmp::Reverse(a.y));
        for (n, r) in order.into_iter().enumerate() {
            let l = drlg.get_or_alloc_level(data, types, 76 + n as u32)?;
            drlg.level_mut(l).rect = r;
        }
        Ok(())
    }

    /// Kurast chain `0x00678910` (§9.2).
    fn kurast_chain(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
    ) -> Result<(), OutdoorError> {
        let diff = (drlg.difficulty as usize).min(2);
        let docks = drlg
            .find_level(75)
            .map(|l| drlg.level(l).rect)
            .ok_or(OutdoorError::LevelMissing(75))?;
        let mut y = 0;
        for id in 79..=83 {
            let (w, h) = data.level(id)?.size[diff];
            y -= h;
            let l = drlg.get_or_alloc_level(data, types, id)?;
            drlg.level_mut(l).rect =
                TileRect::new(docks.x + docks.w / 2 - w / 2, docks.y + y, w, h);
        }
        Ok(())
    }
}

/// The jungle case offsets y1, y3 of §9.1 for a block height SY:
/// y1 := ⌊(⌊SY·0x55555555 / 2³²⌋ − SY) / 2⌋, plus 1 if negative;
/// y3 := −2SY/3 truncated toward zero.
pub fn jungle_offsets(sy: i32) -> (i32, i32) {
    let third = ((i64::from(sy) * 0x5555_5555) >> 32) as i32;
    let mut y1 = (third - sy).div_euclid(2);
    if y1 < 0 {
        y1 += 1;
    }
    (y1, -2 * sy / 3)
}

#[cfg(test)]
#[path = "linker_tests.rs"]
mod linker_tests;
