// Spec: specs/drlg/outdoor.md
//! Acts II and IV level builds: desert (§8), mesas and Chaos Sanctum
//! (§10). Act III is `act3.rs`, Act V `act5.rs`.

use super::grid::Gen;
use super::tilesub::BorderCtx;
use super::OutdoorError;

/// Desert cliff rows `0x006F2390` (§8.3): (P, F, x, y), 5 per row.
pub fn desert_cliff_row(r: u32) -> [(u32, i32, i32, i32); 5] {
    match r {
        0..=2 => {
            // TODO(outdoor.md §8.3): the order of the three wall/path
            // entries in a row is not given; ascending x used (the order
            // decides which first stamp of 377/378 draws its build-list
            // roll first).
            let path_x = 2 + 2 * r as i32;
            let piece = |x: i32| if x == path_x { 378 } else { 377 };
            [
                (376, 1, 0, 4),
                (piece(2), -1, 2, 4),
                (piece(4), -1, 4, 4),
                (piece(6), -1, 6, 4),
                (376, 2, 8, 4),
            ]
        }
        3 | 4 => {
            let (a, b) = if r == 3 { (377, 381) } else { (378, 380) };
            [
                (376, 2, 8, 4),
                (a, -1, 6, 4),
                (382, -1, 4, 4),
                (b, -1, 4, 6),
                (379, 2, 4, 8),
            ]
        }
        _ => {
            // TODO(outdoor.md §8.3): as rows 0–2, ascending y used.
            let path_y = 2 + 2 * (r as i32 - 5);
            let piece = |y: i32| if y == path_y { 381 } else { 380 };
            [
                (379, 1, 4, 0),
                (piece(2), -1, 4, 2),
                (piece(4), -1, 4, 4),
                (piece(6), -1, 4, 6),
                (379, 2, 4, 8),
            ]
        }
    }
}

/// Tomb row `0x006F2610` (§8.4): (P, F, x, y).
pub const TOMB_ROW: [(u32, i32, i32, i32); 9] = [
    (384, 0, 8, 0),
    (383, 2, 6, 0),
    (383, 1, 4, 0),
    (383, 0, 2, 0),
    (387, 0, 0, 0),
    (385, 0, 0, 2),
    (385, 1, 0, 4),
    (385, 2, 0, 6),
    (386, 0, 0, 8),
];

/// Chaos Sanctum piece at index i of the 5×5 layout `0x006F22C0` (§10).
pub fn sanctum_piece(i: i32) -> u32 {
    match i {
        22 => 857,
        11 => 858,
        13 => 859,
        17 => 860,
        7 => 861,
        12 => 862,
        _ => 836,
    }
}

impl Gen<'_> {
    /// Act II `0x0067F980` (§8).
    pub fn act2(&mut self) -> Result<(), OutdoorError> {
        self.link_flags()?;
        self.borders()?;
        match self.id {
            41 => {
                self.town_transition()?;
                self.pb()?;
                self.exit(388)?;
                self.shrines(5);
                self.variants(&[395, 411, 401, 402, 399, 398, 403], false)?;
            }
            42 => {
                self.desert_cliffs()?;
                self.pb()?;
                self.exit(388)?;
                self.waypoint()?;
                self.shrines(5);
                self.variants(&[395, 411, 400, 398], false)?;
                self.variants(&[404, 405, 406, 407], true)?;
            }
            43 => {
                self.desert_cliffs()?;
                self.pb()?;
                self.exit(390)?;
                self.variants(&[396, 397], false)?;
                self.waypoint()?;
                self.shrines(5);
                self.variants(&[411, 399, 398, 403], false)?;
                self.variants(&[395], true)?;
                self.variants(&[395], true)?;
            }
            44 => {
                self.desert_cliffs()?;
                self.pb()?;
                self.exit(412)?;
                self.variants(&[413, 408, 409, 410], false)?;
                self.waypoint()?;
                self.shrines(5);
                self.variants(&[395, 400, 398, 404, 405], false)?;
                self.variants(&[411], true)?;
                self.variants(&[411], true)?;
            }
            45 => self.exit(389)?,
            46 => {
                self.tomb_row()?;
                self.pb()?;
                self.shrines(5);
                self.variants(&[401, 402, 406, 407, 403], false)?;
                self.variants(&[392, 393], true)?;
            }
            134 => {
                self.stamp(4, 4, 394, -1, false)?;
                self.pb()?;
                self.variants(&[401, 402, 406, 407, 403], false)?;
                self.variants(&[392, 393], true)?;
                self.shrines(5);
            }
            _ => {}
        }
        Ok(())
    }

    /// "PB" `0x0067F630`: border substitution types 2, 1, 3, base 364.
    fn pb(&mut self) -> Result<(), OutdoorError> {
        for t in [2, 1, 3] {
            self.border_sub(BorderCtx::wild(t, 364))?;
        }
        Ok(())
    }

    /// "Exit": S(E), fatal if not placed.
    fn exit(&mut self, e: u32) -> Result<(), OutdoorError> {
        if self.s(e)? {
            Ok(())
        } else {
            Err(OutdoorError::ExitNotPlaced(e))
        }
    }

    /// Variants `0x0067F470` (§8.1).
    pub fn variants(&mut self, list: &[u32], iter: bool) -> Result<(), OutdoorError> {
        let n = list.len() as i32;
        let mut r = self.seed().roll(n) as usize;
        for _ in 0..n {
            let p = list[r];
            if iter {
                for f in 0..self.od.preset(p)?.files {
                    self.spawn_preset(p, f, 0, 15)?;
                }
            } else {
                self.spawn_preset(p, -1, 0, 15)?;
            }
            r = (r + 1) % list.len();
        }
        Ok(())
    }

    /// Town transition `0x0067F560` (§8.2).
    fn town_transition(&mut self) -> Result<(), OutdoorError> {
        let Some(e) = self.info.orth.iter().find(|e| e.level_id == 40).copied() else {
            return Ok(());
        };
        if e.direction == 3 {
            let y = self.gh() - 1;
            self.stamp(0, y, 363, -1, false)
        } else {
            let x = self.gw() - 1;
            self.stamp(x, 0, 362, -1, false)
        }
    }

    /// Cliffs `0x0067F5C0` (§8.3).
    fn desert_cliffs(&mut self) -> Result<(), OutdoorError> {
        let r = self.seed().mask(8);
        for (p, f, x, y) in desert_cliff_row(r) {
            self.stamp(x, y, p, f, false)?;
        }
        Ok(())
    }

    /// Tomb row `0x0067F8D0` (§8.4).
    fn tomb_row(&mut self) -> Result<(), OutdoorError> {
        for (p, f, x, y) in TOMB_ROW {
            self.stamp(x, y, p, f, false)?;
        }
        self.stamp(4, 4, 394, -1, false)
    }

    /// Act IV `0x0067E890` (§10).
    pub fn act4(&mut self) -> Result<(), OutdoorError> {
        if self.id == 108 {
            self.link_flags()?;
            for i in 0..25 {
                self.stamp(3 * (i % 5), 3 * (i / 5), sanctum_piece(i), -1, false)?;
            }
            return Ok(());
        }
        if !(104..=106).contains(&self.id) {
            return Ok(());
        }
        self.link_flags()?;
        self.borders()?;
        // `0x0067E660`.
        if self.info.flags & 0x40_0000 != 0 {
            self.stamp(0, 1, 798, -1, false)?;
        }
        if self.info.flags & 0x80_0000 != 0 {
            self.stamp(0, 4, 798, -1, false)?;
        }
        for t in [1, 2, 3] {
            self.border_sub(BorderCtx::wild(t, 799))?;
        }
        // `0x0067E6A0`.
        if self.id == 106 {
            self.s(811)?;
        }
        let (m, t) = match self.id {
            104 => (812, 828),
            105 => (817, 832),
            _ => (823, 832),
        };
        let times = |p: u32, n: u32| (0..n).map(move |_| p);
        let mut seq: Vec<u32> = Vec::new();
        seq.push(m);
        seq.extend(times(m + 1, 2));
        seq.extend(times(m + 2, 2));
        seq.extend(times(m + 3, 2));
        if self.id == 105 {
            seq.push(822);
        }
        seq.extend(times(m + 4, 4));
        seq.push(t);
        seq.extend(times(t + 1, 2));
        seq.extend(times(t + 2, 2));
        seq.extend(times(t + 3, 4));
        for p in seq {
            self.s(p)?;
        }
        Ok(())
    }
}
