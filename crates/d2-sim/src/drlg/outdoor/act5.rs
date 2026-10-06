// Spec: specs/drlg/outdoor.md, specs/drlg/outdoor-act3-act5.md
//! Act V level build (`outdoor.md` §11, `outdoor-act3-act5.md` §5).

use super::grid::{cell, Gen, Op};
use super::tilesub::BorderCtx;
use super::vertex::{border_piece, corner_piece};
use super::OutdoorError;

/// Ravine step directions D (`0x006F1FD8`, k = 0..11, §11 step 3).
pub const RAVINE_STEPS: [(i32, i32); 12] = [
    (-1, 0),
    (0, -1),
    (1, 0),
    (0, 1),
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (-1, 0),
    (0, -1),
    (1, 0),
    (0, 1),
];

/// Cave rows (`0x006F1F9C`, §11 step 5): (level, F, side, P tall,
/// P wide).
pub const CAVES: [(u32, i32, bool, u32, u32); 3] = [
    (112, 0, false, 913, 914),
    (117, 0, true, 983, 984),
    (117, 0, false, 985, 986),
];

/// One special-preset row (`0x006F2100`, §11 step 9; the unread dword
/// is left out).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecialRow {
    pub level: u32,
    pub tall: u32,
    pub wide: u32,
    pub f: i32,
    pub count: u32,
    pub fatal: bool,
}

const fn row(level: u32, tall: u32, wide: u32, f: i32, count: u32, fatal: bool) -> SpecialRow {
    SpecialRow {
        level,
        tall,
        wide,
        f,
        count,
        fatal,
    }
}

/// The 15 special-preset rows in table order (§11 step 9).
pub const SPECIALS: [SpecialRow; 15] = [
    row(111, 955, 956, 0, 1, true),
    row(112, 955, 956, 0, 1, true),
    row(117, 955, 956, 1, 1, true),
    row(112, 953, 953, -1, 1, true),
    row(117, 954, 954, -1, 1, true),
    row(111, 944, 947, -1, 1, false),
    row(111, 942, 945, -1, 4, false),
    row(111, 943, 946, -1, 4, false),
    row(112, 941, 941, -1, 1, false),
    row(112, 939, 939, -1, 1, false),
    row(112, 940, 940, -1, 5, false),
    row(117, 948, 948, -1, 4, false),
    row(117, 949, 949, -1, 4, false),
    row(117, 950, 950, -1, 4, false),
    row(117, 951, 951, -1, 3, false),
];

/// Fatal error of a special-preset row that placed nothing.
pub const FATAL_SPECIAL: u32 = 0x219;
/// Fatal error of fewer than 3 prisons.
pub const FATAL_PRISONS: u32 = 0x259;

impl Gen<'_> {
    /// Act V `0x0067E600` (§11).
    pub fn act5(&mut self) -> Result<(), OutdoorError> {
        if self.id == 110 {
            // Siege strip `0x0067E560`.
            let s = self.od.preset(865)?.size_x / 8;
            for i in 0..15 {
                let x = self.gw() - s * (i + 1);
                if x < 0 {
                    return Err(OutdoorError::SiegeStrip(865 + i as u32));
                }
                self.stamp(x, 0, 865 + i as u32, 0, false)?;
            }
            return Ok(());
        }
        // Step 1.
        self.link_flags()?;
        // Steps 2..6.
        self.barricade_borders()?;
        self.ravine()?;
        self.entrances()?;
        self.caves()?;
        if self.id == 111 {
            self.connect_to_siege()?;
        }
        // Step 7 (`0x0067E0E0`).
        self.border_sub(BorderCtx::barricade())?;
        // Step 8.
        if self.id == 111 {
            self.prisons()?;
        }
        // Step 9.
        self.act5_specials()
    }

    /// Barricade border walk `0x0067DCF0` (§11 step 2).
    pub fn barricade_borders(&mut self) -> Result<(), OutdoorError> {
        let s = 4 + i32::from(self.id == 117);
        let count = self.info.vertices.len();
        for i in 0..count {
            let v = self.info.vertices[i];
            let n = self.info.vertices[(i + 1) % count];
            let nn = self.info.vertices[(i + 2) % count];
            // `0x0067D280`: sign of each component.
            let (dx, dy) = ((n.x - v.x).signum(), (n.y - v.y).signum());
            let (ndx, ndy) = ((nn.x - n.x).signum(), (nn.y - n.y).signum());
            let (vx, vy) = (v.x & !1, v.y & !1);
            let (nx, ny) = (n.x & !1, n.y & !1);
            if !v.is_preset_link() {
                let piece = border_piece(dx, dy, s);
                // "Until it equals (nx, ny)": polygon edges are
                // axis-aligned, so the cleared ends are a whole number of
                // 2-cell steps apart along (dx, dy).
                // TODO(outdoor.md §11 step 2): ends not on the walk's line
                // (never met, the original would not stop) are not
                // described; not reached by 1.14d polygons.
                let len = (nx - vx).abs().max((ny - vy).abs()) / 2;
                let (mut x, mut y) = (vx, vy);
                for _ in 0..len {
                    x += 2 * dx;
                    y += 2 * dy;
                    if piece != 0 {
                        self.stamp(x, y, piece, -1, false)?;
                    }
                    self.op(2, x, y, Op::Or, cell::BORDER);
                }
            }
            if v.is_link() {
                let x = (v.x.max(n.x) - 4 * dx.abs()) & !1;
                let y = (v.y.max(n.y) - 4 * dy.abs()) & !1;
                self.op(2, x, y, Op::Or, cell::LINK);
                self.op(2, x + 2 * dx.abs(), y + 2 * dy.abs(), Op::Or, cell::LINK);
            }
            let piece = corner_piece(2 * dx, 2 * dy, 2 * ndx, 2 * ndy, s);
            if piece != 0 {
                self.stamp(nx, ny, piece, -1, false)?;
                self.op(2, nx, ny, Op::Or, cell::BORDER);
            }
        }
        if self.id == 111 {
            let (gw, gh) = (self.gw(), self.gh());
            self.op(2, gw - 2, gh - 4, Op::Or, cell::LINK);
            self.op(2, gw - 2, gh - 3, Op::Or, cell::LINK);
        }
        Ok(())
    }

    /// Ravine walk `0x0067DEF0` (§11 step 3).
    pub fn ravine(&mut self) -> Result<(), OutdoorError> {
        let (b, b2, start, end) = if self.id == 117 {
            (957, 969, 982, 981)
        } else {
            (881, 893, 906, 905)
        };
        let (gw, gh) = (self.gw(), self.gh());
        let (mut x, mut y) = (gw - 2, 0);
        let mut steps = 0;
        while (x, y) != (0, gh - 2) {
            let k = self.g(0, x, y) as i32 - b;
            steps += 1;
            if !(0..12).contains(&k) || steps > gw * gh {
                // TODO(outdoor.md §11 step 3): a cell outside the
                // barricade straight/corner range reads outside D, and a
                // walk that never reaches (0, gh − 2) would not end; the
                // original's behaviour is not described. Treated as fatal
                // with the walk's address.
                return Err(OutdoorError::Fatal(0x0067_DEF0));
            }
            self.stamp(x, y, (b2 + k) as u32, -1, false)?;
            let (ddx, ddy) = RAVINE_STEPS[k as usize];
            x += 2 * ddx;
            y += 2 * ddy;
        }
        self.stamp(gw - 2, 0, start, -1, false)?;
        self.stamp(0, gh - 2, end, -1, false)
    }

    /// Entrances `0x0067DB50` (§11 step 4).
    pub fn entrances(&mut self) -> Result<(), OutdoorError> {
        let (gw, gh) = (self.gw(), self.gh());
        let link = |g: &Self, x, y| g.g(2, x, y) & cell::LINK != 0;
        if let Some(x) = (0..gw).find(|&x| link(self, x, 0)) {
            self.stamp(x, 0, 909, -1, false)?;
        }
        if let Some(x) = (0..gw).find(|&x| link(self, x, gh - 2)) {
            self.stamp(x, gh - 2, 908, -1, false)?;
        }
        if let Some(y) = (0..gh).find(|&y| link(self, 0, y)) {
            self.stamp(0, y, 910, -1, false)?;
        }
        if let Some(y) = (0..gh).find(|&y| link(self, gw - 2, y)) {
            self.stamp(gw - 2, y, 907, -1, false)?;
        }
        Ok(())
    }

    /// Caves `0x0067DA70` (§11 step 5).
    pub fn caves(&mut self) -> Result<(), OutdoorError> {
        let (gw, gh) = (self.gw(), self.gh());
        let wide = self.rect.w > self.rect.h;
        for &(level, f, side, tall_p, wide_p) in &CAVES {
            if level != self.id {
                continue;
            }
            if wide {
                self.stamp(if side { gw - 2 } else { 0 }, 2, wide_p, f, false)?;
            } else {
                self.stamp(2, if side { gh - 2 } else { 0 }, tall_p, f, false)?;
            }
        }
        Ok(())
    }

    /// Connect to siege `0x0067E4B0` (§11 step 6, level 111).
    pub fn connect_to_siege(&mut self) -> Result<(), OutdoorError> {
        let (sw, sh) = self.preset_cells(880)?;
        let x = self.gw() - sw;
        let y = self.gh() - sh;
        self.stamp(x, y, 880, -1, false)?;
        self.stamp(x, y - 2, 896, -1, false)
    }

    /// Prison cell value `0x00674120`.
    fn prison_value(&self, x: i32, y: i32) -> u32 {
        if self.g(2, x, y) & cell::PRESET != 0 {
            self.g(0, x, y)
        } else {
            0
        }
    }

    /// One prison test and stamp; true when placed.
    fn prison_at(&mut self, x: i32, y: i32) -> Result<bool, OutdoorError> {
        let v = self.prison_value(x, y);
        if (915..=922).contains(&v) {
            self.stamp(x, y, v + 16, -1, false)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Prisons `0x0067E240` (§11 step 8, level 111).
    pub fn prisons(&mut self) -> Result<(), OutdoorError> {
        let (gw, gh) = (self.gw(), self.gh());
        let mut placed = 0;
        let mut tries = 0;
        while tries < 90 && placed < 3 {
            tries += 1;
            // Helper `0x0045C390`, x then y.
            let x = 2 * self.seed().roll(gw / 2) as i32;
            let y = 2 * self.seed().roll(gh / 2) as i32;
            if self.prison_at(x, y)? {
                placed += 1;
            }
        }
        // Inline, always drawn.
        let sx = self.seed().roll(gw / 2) as i32;
        let sy = self.seed().roll(gh / 2) as i32;
        'scan: for i in 0..gh {
            for j in 0..gw {
                if placed >= 3 {
                    break 'scan;
                }
                let x = (j + 2 * sx) % gw;
                let y = (i + 2 * sy) % gh;
                if self.prison_at(x, y)? {
                    placed += 1;
                }
            }
        }
        if placed < 3 {
            return Err(OutdoorError::Fatal(FATAL_PRISONS));
        }
        Ok(())
    }

    /// Special presets `0x0067E160` (§11 step 9).
    pub fn act5_specials(&mut self) -> Result<(), OutdoorError> {
        let tall = self.rect.w < self.rect.h;
        let id = self.id;
        for r in SPECIALS.iter().filter(|r| r.level == id) {
            let p = if tall { r.tall } else { r.wide };
            let mut last = true;
            for _ in 0..r.count {
                last = self.spawn_preset(p, r.f, 0, 15)?;
            }
            if !last && r.fatal {
                return Err(OutdoorError::Fatal(FATAL_SPECIAL));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "act5_tests.rs"]
mod act5_tests;
