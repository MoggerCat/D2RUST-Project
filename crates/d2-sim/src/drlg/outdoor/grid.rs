// Spec: specs/drlg/outdoor.md
//! Cell grids (§1.1–§1.2), the generation context, preset stamping and
//! the build list (§5.1), fit tests (§5.2), the shuffled cell list
//! (§5.3) and the placers (§5.4).

use crate::rng::Seed;

use super::super::data::DrlgData;
use super::super::level::Drlg;
use super::super::{LevelIdx, TileRect};
use super::{BuildNode, OutdoorData, OutdoorError, OutdoorLevel, SubFiles};

/// Grid-2 cell bits (§1.2).
pub mod cell {
    pub const BORDER: u32 = 0x1;
    pub const DIRECTION: u32 = 0x2;
    pub const PATH: u32 = 0x80;
    pub const BLANK: u32 = 0x100;
    pub const PRESET: u32 = 0x200;
    pub const LINK: u32 = 0x400;
    pub const WAYPOINT: u32 = 0x800;
    pub const SHRINE: u32 = 0x1000;
    pub const FILE_MASK: u32 = 0xF0000;
    /// Bits that make a cell not "spawn valid" (`0x00674200`).
    pub const NOT_SPAWN: u32 = 0x1B81;
}

/// Grid op codes of `0x0067C4F0` (§1.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Or,
    And,
    Xor,
    Set,
    SetIfZero,
    AndNot,
}

/// One cell grid (gw × gh u32 cells, row-major).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grid {
    pub w: i32,
    pub h: i32,
    pub cells: Vec<u32>,
}

impl Grid {
    /// `0x0067CB80`: zeroed.
    pub fn new(w: i32, h: i32) -> Self {
        let n = (w.max(0) * h.max(0)) as usize;
        Self {
            w,
            h,
            cells: vec![0; n],
        }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }

    /// The cell, or 0 outside the grid.
    pub fn get(&self, x: i32, y: i32) -> u32 {
        if self.contains(x, y) {
            self.cells[(y * self.w + x) as usize]
        } else {
            0
        }
    }

    /// Applies an op to an in-grid cell; outside the grid nothing.
    pub fn op(&mut self, x: i32, y: i32, op: Op, v: u32) {
        if !self.contains(x, y) {
            return;
        }
        let c = &mut self.cells[(y * self.w + x) as usize];
        *c = match op {
            Op::Or => *c | v,
            Op::And => *c & v,
            Op::Xor => *c ^ v,
            Op::Set => v,
            Op::SetIfZero if *c == 0 => v,
            Op::SetIfZero => *c,
            Op::AndNot => *c & !v,
        };
    }
}

/// "Spawn valid" (`0x00674200`).
pub fn spawn_valid(c: u32) -> bool {
    c & cell::NOT_SPAWN == 0
}

/// The picked file of a grid-2 cell (bits 16..19).
pub fn file_of(c: u32) -> i32 {
    ((c >> 16) & 0xF) as i32
}

/// Level generation context: the level, its outdoor info and the data.
pub struct Gen<'a> {
    pub drlg: &'a mut Drlg,
    pub data: &'a DrlgData,
    pub od: &'a OutdoorData,
    pub subs: &'a dyn SubFiles,
    pub level: LevelIdx,
    pub id: u32,
    /// Level rect (tiles).
    pub rect: TileRect,
    pub info: &'a mut OutdoorLevel,
}

impl Gen<'_> {
    /// The level seed (level +0x1C4).
    pub fn seed(&mut self) -> &mut Seed {
        &mut self.drlg.level_mut(self.level).seed
    }

    pub fn gw(&self) -> i32 {
        self.info.gw()
    }

    pub fn gh(&self) -> i32 {
        self.info.gh()
    }

    pub fn g(&self, grid: usize, x: i32, y: i32) -> u32 {
        self.info.grids[grid].get(x, y)
    }

    pub fn op(&mut self, grid: usize, x: i32, y: i32, op: Op, v: u32) {
        self.info.grids[grid].op(x, y, op, v);
    }

    pub fn in_grid(&self, x: i32, y: i32) -> bool {
        self.info.grids[2].contains(x, y)
    }

    /// Size of a preset in cells (`SizeX/8`, `SizeY/8`).
    pub fn preset_cells(&self, p: u32) -> Result<(i32, i32), OutdoorError> {
        let d = self.od.preset(p)?;
        Ok((d.size_x / 8, d.size_y / 8))
    }

    /// Build list `0x00674320` (§5.1 step 1): the next file of `p`.
    pub fn next_file(&mut self, p: u32) -> Result<i32, OutdoorError> {
        let pos = match self.info.build_list.iter().position(|n| n.preset == p) {
            Some(i) => i,
            None => {
                let n = self.od.preset(p)?.files;
                // Site 0x0067438F.
                let r = self.seed().roll(n) as i32;
                self.info.build_list.insert(
                    0,
                    BuildNode {
                        preset: p,
                        files: n,
                        current: r,
                    },
                );
                0
            }
        };
        let node = &mut self.info.build_list[pos];
        if node.files == 0 {
            return Err(OutdoorError::NoFiles(p));
        }
        // Signed remainder (idiv).
        node.current = (node.current + 1) % node.files;
        Ok(node.current)
    }

    /// Stamp a preset `0x006743C0` (§5.1). `f` = −1: from the build list.
    pub fn stamp(
        &mut self,
        x: i32,
        y: i32,
        p: u32,
        f: i32,
        border: bool,
    ) -> Result<(), OutdoorError> {
        let f = if f == -1 { self.next_file(p)? } else { f };
        let (sw, sh) = self.preset_cells(p)?;
        let border_bit = border && (matches!(p, 4..=15) || matches!(p, 364..=375));
        for j in 0..sh {
            for i in 0..sw {
                let (cx, cy) = (x + i, y + j);
                self.op(2, cx, cy, Op::AndNot, cell::FILE_MASK);
                self.op(2, cx, cy, Op::Or, cell::PRESET | ((f as u32) << 16));
                if border_bit {
                    self.op(2, cx, cy, Op::Or, cell::BORDER);
                }
                self.op(0, cx, cy, Op::Set, 0);
            }
        }
        self.op(0, x, y, Op::Set, p);
        Ok(())
    }

    /// Preset fits `0x00674230` (§5.2). `p` = 0 means 1×1.
    pub fn fits(&self, x: i32, y: i32, p: u32, m: i32, flags: u32) -> Result<bool, OutdoorError> {
        let (mut w, mut h) = if p == 0 {
            (1, 1)
        } else {
            self.preset_cells(p)?
        };
        let (mut x, mut y) = (x, y);
        if m != 0 {
            if flags & 1 != 0 {
                y -= m;
                h += m;
            }
            if flags & 2 != 0 {
                w += m;
            }
            if flags & 4 != 0 {
                h += m;
            }
            if flags & 8 != 0 {
                x -= m;
                w += m;
            }
        }
        for j in 0..h {
            for i in 0..w {
                let (cx, cy) = (x + i, y + j);
                if !self.in_grid(cx, cy) || !spawn_valid(self.g(2, cx, cy)) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// Shuffled cell list (§5.3): candidate cells (x + 1, y + 1).
    pub fn shuffle(&mut self) -> Vec<(i32, i32)> {
        let (w, h) = (self.gw() - 2, self.gh() - 2);
        shuffle_cells(self.seed(), w, h)
            .into_iter()
            .map(|(x, y)| (x + 1, y + 1))
            .collect()
    }

    /// SpawnOutdoorLevelPreset `0x00674730` (§5.4).
    pub fn spawn_preset(
        &mut self,
        p: u32,
        f: i32,
        m: i32,
        flags: u32,
    ) -> Result<bool, OutdoorError> {
        for (x, y) in self.shuffle() {
            if self.fits(x, y, p, m, flags)? {
                self.stamp(x, y, p, f, false)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// "S": SpawnOutdoorLevelPreset(p, −1, 0, 15).
    pub fn s(&mut self, p: u32) -> Result<bool, OutdoorError> {
        self.spawn_preset(p, -1, 0, 15)
    }

    /// SpawnRandomDS1 `0x00674920` (§5.4).
    pub fn random_ds1(&mut self, p: u32, f: i32) -> Result<(), OutdoorError> {
        const DX: [i32; 8] = [-1, 0, 0, 1, -1, 1, 1, -1];
        const DY: [i32; 8] = [0, -1, 1, 0, -1, 1, -1, 1];
        for (x, y) in self.shuffle() {
            if self.g(2, x, y) & cell::PATH == 0 {
                continue;
            }
            for k in 0..8 {
                let (nx, ny) = (x + DX[k], y + DY[k]);
                if self.fits(nx, ny, p, 0, 15)? {
                    return self.stamp(nx, ny, p, f, false);
                }
            }
        }
        // TODO(outdoor.md §5.4): read as "no path cell with a fitting
        // neighbour among all candidates" (the scan continues past a path
        // cell whose neighbours do not fit).
        self.spawn_preset(p, f, 0, 15)?;
        Ok(())
    }

    /// "R": RandomDS1(p, −1).
    pub fn r(&mut self, p: u32) -> Result<(), OutdoorError> {
        self.random_ds1(p, -1)
    }

    /// FarAway `0x006744F0` (§5.4).
    pub fn far_away(
        &mut self,
        rect: TileRect,
        p: u32,
        f: i32,
        m: i32,
        flags: u32,
    ) -> Result<bool, OutdoorError> {
        let (gw, gh) = (self.gw(), self.gh());
        let rx = self.seed().roll(gw - 2) as i32;
        let ry = self.seed().roll(gh - 2) as i32;
        let (w, h) = (gw - 2, gh - 2);
        if w <= 0 || h <= 0 {
            // TODO(outdoor.md §5.4): `mod W` with W ≤ 0 divides by zero
            // in the original; not reached by 1.14d level sizes.
            return Ok(false);
        }
        let (cx, cy) = (rect.x + rect.w / 2, rect.y + rect.h / 2);
        let mut best: Option<(i32, i32, i32)> = None;
        // Edge case 6: both loops inclusive.
        for i in 0..=h {
            for j in 0..=w {
                let x = (j + rx) % w + 1;
                let y = (i + ry) % h + 1;
                if !self.fits(x, y, p, m, flags)? {
                    continue;
                }
                let ax = (8 * x - cx + self.rect.x + 4).abs();
                let ay = (8 * y - cy + self.rect.y + 4).abs();
                let d = if ax > ay { ay + 2 * ax } else { ax + 2 * ay } / 2;
                if best.is_none_or(|(_, _, bd)| d > bd) {
                    best = Some((x, y, d));
                }
            }
        }
        match best {
            Some((x, y, _)) => {
                self.stamp(x, y, p, f, false)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Waypoint `0x00674B70` (§5.4).
    pub fn waypoint(&mut self) -> Result<(), OutdoorError> {
        const WP: u32 = super::super::room_flags::WAYPOINT;
        const WP_SMALL: u32 = super::super::room_flags::WAYPOINT_SMALL;
        if self.id == 3 {
            let vis = self.drlg.vis_array(self.data, 3)?;
            let i = vis.iter().position(|&v| v == 2).unwrap_or(8) as u32;
            let mask = 1u32 << (i + 4);
            let (gw, gh) = (self.gw(), self.gh());
            for y in 0..gh {
                for x in 0..gw {
                    if self.g(1, x, y) & mask != 0 && self.g(2, x, y) & cell::LINK != 0 {
                        let x = x.clamp(1, gw - 2);
                        let y = y.clamp(1, gh - 2);
                        self.op(1, x, y, Op::Or, WP_SMALL);
                        self.op(2, x, y, Op::Or, cell::WAYPOINT);
                        return Ok(());
                    }
                }
            }
        }
        for (x, y) in self.shuffle() {
            if spawn_valid(self.g(2, x, y)) {
                self.op(1, x, y, Op::Or, WP);
                self.op(2, x, y, Op::Or, cell::WAYPOINT);
                break;
            }
        }
        Ok(())
    }

    /// Shrines(n) `0x00674E40` (§5.4).
    pub fn shrines(&mut self, n: i32) {
        const BITS: [u32; 4] = [0x1000, 0x2000, 0x4000, 0x8000];
        // Site 0x00674E59, drawn always (edge case 5).
        let mut k = self.seed().mask(4) as usize;
        let mut n = n;
        for (x, y) in self.shuffle() {
            if n <= 0 {
                break;
            }
            if spawn_valid(self.g(2, x, y)) {
                self.op(1, x, y, Op::Or, BITS[k]);
                self.op(2, x, y, Op::Or, cell::SHRINE);
                k = (k + 1) % 4;
                n -= 1;
            }
        }
    }
}

/// The shuffle of §5.3 (and `outdoor-tilesub.md` §2.2, §4.2): entries
/// (k mod W, k div W) for k < A = W·H, then per k one pair
/// `roll(A)`, `roll(A)` and a swap. A ≤ 0: nothing, no draws.
pub fn shuffle_cells(seed: &mut Seed, w: i32, h: i32) -> Vec<(i32, i32)> {
    if w <= 0 || h <= 0 {
        // TODO(outdoor.md §5.3): W and H both negative give A > 0 in the
        // original (entries with a negative modulus); not reached by
        // 1.14d grid sizes. Read as "nothing".
        return Vec::new();
    }
    let a = w * h;
    let mut e: Vec<(i32, i32)> = (0..a).map(|k| (k % w, k / w)).collect();
    for _ in 0..a {
        let i = seed.roll(a) as usize;
        let j = seed.roll(a) as usize;
        e.swap(i, j);
    }
    e
}
