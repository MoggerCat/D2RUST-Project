// Spec: specs/drlg/outdoor-tilesub.md
//! lvlsub tile substitution (DrlgTileSub): rows and files (§1), border
//! substitution on the level grids from the level seed (§2), the
//! sub-theme pick at outdoor room creation (§3) and the room
//! substitutions at tile build from the room seed (§4).

use crate::rng::Seed;

use super::super::tiles::CellGrid;
use super::grid::{cell, shuffle_cells, spawn_valid, Gen, Op};
use super::{OutdoorData, OutdoorError, OutdoorRoom, SubFile, SubFiles, SubGroup};

/// Default skip style S (§2.2 step 1).
pub const SKIP_STYLE: i32 = 62;

/// Style-map result meaning "any" / "no stamp".
pub const STYLE_ANY: i32 = -5;

/// File of a replacement's stamp (§2.3): the stamp makes no build-list
/// roll (recorded: Blood Moor and Cold Plains). PROVISIONAL (§2.3,
/// REC-404): file 0, the build list untouched.
pub const SUB_STAMP_FILE: i32 = 0;

/// One lvlsub row (§1.2), file order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubRow {
    pub type_: i32,
    pub file: Vec<u8>,
    pub check_all: i32,
    pub bord_type: i32,
    pub dt1_mask: u32,
    pub grid_size: i32,
    /// `Prob0..4`, `Trials0..4`, `Max0..4` by sub theme.
    pub prob: [i32; 5],
    pub trials: [i32; 5],
    pub max: [i32; 5],
}

impl OutdoorData {
    /// Rows of type t (`0x0061FA90`, §1.1): from the first row of the
    /// type while `Type` = t.
    pub fn sub_rows(&self, t: i32) -> Result<&[SubRow], OutdoorError> {
        let first = self
            .subs
            .iter()
            .position(|r| r.type_ == t)
            .ok_or(OutdoorError::NoSubRows(t))?;
        // Edge case 4: the original reads the record after the last row;
        // the slice ends at the table end here.
        let n = self.subs[first..]
            .iter()
            .take_while(|r| r.type_ == t)
            .count();
        Ok(&self.subs[first..first + n])
    }
}

/// A row's file, loaded (§1.3): missing → error; no groups → fatal.
pub fn load<'s>(subs: &'s dyn SubFiles, row: &SubRow) -> Result<&'s SubFile, OutdoorError> {
    let f = subs
        .sub_file(&row.file)
        .ok_or_else(|| OutdoorError::MissingSubFile(row.file.clone()))?;
    if f.groups.is_empty() {
        return Err(OutdoorError::SubFileNoGroups(row.file.clone()));
    }
    Ok(f)
}

/// Callback set of a border substitution (§2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Callbacks {
    /// Acts I, II, IV: link test, 1×1 fit test.
    Wild,
    /// Act V (`0x0067E0E0`): custom test `0x0067E080`, style map
    /// `0x0067E000`.
    Barricade,
}

/// Border substitution context (§2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BorderCtx {
    pub t: i32,
    pub base: u32,
    pub skip: i32,
    pub callbacks: Callbacks,
}

impl BorderCtx {
    /// Acts I, II, IV (`0x006752A0`).
    pub fn wild(t: i32, base: u32) -> Self {
        Self {
            t,
            base,
            skip: -1,
            callbacks: Callbacks::Wild,
        }
    }

    /// Act V (`0x0067E0E0`): type 12, base 0.
    pub fn barricade() -> Self {
        Self {
            t: 12,
            base: 0,
            skip: -1,
            callbacks: Callbacks::Barricade,
        }
    }
}

/// Act V style map `0x0067E000` (§2.3): (style, lo, hi, P, P snow).
pub const STYLE_MAP: [(u32, u32, u32, i32, i32); 10] = [
    (49, 1, 16, 915, 987),
    (49, 31, 46, 915, 987),
    (48, 1, 1, 883, 959),
    (48, 2, 3, 881, 957),
    (48, 4, 4, 884, 960),
    (48, 5, 5, 895, 971),
    (48, 6, 7, 893, 969),
    (48, 8, 8, 896, 972),
    (48, 30, 30, 0, 0),
    (48, 31, 31, -5, -5),
];

/// The Act V style map: style must be 48 or 49 and a row must match.
pub fn style_map(style: u32, v: u32, snow: bool) -> Result<i32, OutdoorError> {
    if style != 48 && style != 49 {
        return Err(OutdoorError::StyleMap(style, v));
    }
    STYLE_MAP
        .iter()
        .find(|r| r.0 == style && (r.1..=r.2).contains(&v))
        .map(|r| {
            let p = if snow { r.4 } else { r.3 };
            if p > 0 {
                p + (v - r.1) as i32
            } else {
                p
            }
        })
        .ok_or(OutdoorError::StyleMap(style, v))
}

/// One replacement of a border substitution (§2.2 step 3): lvlsub type,
/// row index within the type's rows, group index, the snapped cell, the
/// variant roll and the level seed's low word after it. Not original
/// state: a record of what the generator did, read by the checks that
/// compare a build with the recorded draws (`outdoor.md` Test vectors,
/// "Cold Plains grid").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubHit {
    pub t: i32,
    pub row: usize,
    pub group: usize,
    pub x: i32,
    pub y: i32,
    pub variant: i32,
    pub lo: u32,
}

/// Outcome of one group (`0x0066F690`).
enum GroupEnd {
    NextGroup,
    StopRow,
}

impl Gen<'_> {
    /// Border substitution `0x00670750` (§2.2).
    pub fn border_sub(&mut self, mut ctx: BorderCtx) -> Result<(), OutdoorError> {
        let od = self.od;
        let subs = self.subs;
        for (r, row) in od.sub_rows(ctx.t)?.iter().enumerate() {
            let file = load(subs, row)?;
            if ctx.skip == -1 {
                ctx.skip = SKIP_STYLE;
            }
            let count = file.groups.len() as i32;
            // Site 0x0066F9C9.
            let g0 = if row.bord_type == 0 {
                self.seed().roll(count) as i32
            } else {
                0
            };
            for j in 0..count {
                let gi = ((g0 + j) % count) as usize;
                if let GroupEnd::StopRow = self.border_group(&ctx, (r, row), file, gi)? {
                    break;
                }
            }
        }
        Ok(())
    }

    /// One group `0x0066F690` (§2.2 step 3).
    fn border_group(
        &mut self,
        ctx: &BorderCtx,
        (r, row): (usize, &SubRow),
        file: &SubFile,
        gi: usize,
    ) -> Result<GroupEnd, OutdoorError> {
        let g = file.groups[gi];
        let off = if ctx.t == 1 && self.info.flags & 0xC != 0 {
            -1
        } else {
            1
        };
        let w = self.gw() - row.grid_size * g.w + off;
        let h = self.gh() - row.grid_size * g.h + 1;
        if w * h <= 0 {
            return Ok(GroupEnd::NextGroup);
        }
        let small = ctx.t == 1 && (2..=7).contains(&self.id) && w < 6 && h < 6;
        // Sites 0x0066F78D / 0x0066F7B8, 0x0066F7E9.
        let entries = shuffle_cells(self.seed(), w, h);
        for (x, y) in entries {
            if small && (x, y) == (2, 2) {
                continue;
            }
            if self.sub_test(ctx, row, file, g, x, y)? {
                // Sites 0x0066F8DB / 0x0066F905.
                let v = self.seed().roll(g.variants) as i32;
                let (sx, sy) = Self::snap(row, x, y);
                let lo = self.seed().lo;
                self.info.sub_hits.push(SubHit {
                    t: ctx.t,
                    row: r,
                    group: gi,
                    x: sx,
                    y: sy,
                    variant: v,
                    lo,
                });
                self.sub_replace(ctx, row, file, g, x, y, (v + 1) * (g.w + 1))?;
                match row.bord_type {
                    0 => return Ok(GroupEnd::StopRow),
                    1 => return Ok(GroupEnd::NextGroup),
                    _ => {}
                }
            }
        }
        Ok(GroupEnd::NextGroup)
    }

    fn snap(row: &SubRow, x: i32, y: i32) -> (i32, i32) {
        let gs = row.grid_size;
        if gs == 0 {
            // TODO(outdoor-tilesub.md §2.3): GridSize 0 divides by zero in
            // the original; no 1.14d row has it.
            return (x, y);
        }
        (x - x % gs, y - y % gs)
    }

    /// Test `0x0066F3B0` (§2.3).
    fn sub_test(
        &self,
        ctx: &BorderCtx,
        row: &SubRow,
        file: &SubFile,
        g: SubGroup,
        x: i32,
        y: i32,
    ) -> Result<bool, OutdoorError> {
        let (x, y) = Self::snap(row, x, y);
        let snow = self.id == 117;
        for j in 0..g.h {
            for i in 0..g.w {
                let (cx, cy) = (x + i * row.grid_size, y + j * row.grid_size);
                let f = file.floor_at(g.x + i, g.y + j);
                let w = file.wall_at(0, g.x + i, g.y + j);
                let c = self.g(0, cx, cy) as i32;
                let not_link = self.g(2, cx, cy) & cell::LINK == 0;
                let pass = match ctx.callbacks {
                    Callbacks::Barricade => {
                        let m = style_map(w >> 20 & 0x3F, w >> 8 & 0xFF, snow)?;
                        m == STYLE_ANY || (m == c && not_link)
                    }
                    Callbacks::Wild if w & 1 != 0 => {
                        let s = (w >> 8 & 0xFF) as i32 - 1;
                        (s == ctx.skip || c == ctx.base as i32 + s) && not_link
                    }
                    Callbacks::Wild if f & 2 != 0 => {
                        self.in_grid(cx, cy) && spawn_valid(self.g(2, cx, cy))
                    }
                    Callbacks::Wild => true,
                };
                if !pass {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// Replace `0x0066F520` (§2.3) with pattern x offset `xoff`.
    #[allow(clippy::too_many_arguments)]
    fn sub_replace(
        &mut self,
        ctx: &BorderCtx,
        row: &SubRow,
        file: &SubFile,
        g: SubGroup,
        x: i32,
        y: i32,
        xoff: i32,
    ) -> Result<(), OutdoorError> {
        let (x, y) = Self::snap(row, x, y);
        let snow = self.id == 117;
        for j in 0..g.h {
            for i in 0..g.w {
                let (cx, cy) = (x + i * row.grid_size, y + j * row.grid_size);
                let f = file.floor_at(g.x + i + xoff, g.y + j);
                let w = file.wall_at(0, g.x + i + xoff, g.y + j);
                if w & 1 != 0 {
                    let s = (w >> 8 & 0xFF) as i32 - 1;
                    let p = match ctx.callbacks {
                        Callbacks::Barricade => style_map(w >> 20 & 0x3F, w >> 8 & 0xFF, snow)?,
                        Callbacks::Wild => ctx.base as i32 + s,
                    };
                    if p != STYLE_ANY && s != ctx.skip {
                        // TODO(outdoor-tilesub.md §2.3): P ≤ 0 (style map row
                        // (48, 30, 30) gives 0) would stamp lvlprest row 0 or
                        // a negative id; skipped.
                        if p > 0 {
                            // No build-list roll (recorded, §2.3); the
                            // file is PROVISIONAL (outdoor-tilesub.md §2.3,
                            // REC-404).
                            self.stamp(cx, cy, p as u32, SUB_STAMP_FILE, true)?;
                        }
                    }
                } else if f & 2 != 0 {
                    // Keep cell `0x00674160`.
                    self.op(0, cx, cy, Op::Set, 0);
                    self.op(2, cx, cy, Op::Set, 0);
                } else {
                    // Blank cell `0x006741A0`.
                    self.op(0, cx, cy, Op::Set, 0);
                    self.op(2, cx, cy, Op::Set, cell::BLANK);
                }
            }
        }
        Ok(())
    }
}

/// Sub-theme pick `0x006706A0` (§3): one room-seed step per row of type
/// `t`; returns (picked mask, DT1 mask bits to OR into the room).
pub fn pick_sub_themes(
    od: &OutdoorData,
    seed: &mut Seed,
    t: i32,
    theme: i32,
) -> Result<(u32, u32), OutdoorError> {
    if t == -1 || theme == -1 {
        return Ok((0, 0));
    }
    let h = theme_index(theme)?;
    let (mut mask, mut dt1) = (0, 0);
    for (k, row) in od.sub_rows(t)?.iter().enumerate() {
        // Site 0x006706D7.
        let r = (seed.step() % 100) as i32;
        if r < row.prob[h] {
            mask |= 1u32 << k;
            dt1 |= row.dt1_mask;
        }
    }
    Ok((mask, dt1))
}

fn theme_index(theme: i32) -> Result<usize, OutdoorError> {
    if (0..5).contains(&theme) {
        Ok(theme as usize)
    } else {
        Err(OutdoorError::SubTheme(theme))
    }
}

/// The room side of a room substitution: size (8 × 8 for outdoor rooms),
/// tile origin, and its grids (one wall layer).
pub struct RoomSub<'a> {
    pub w: i32,
    pub h: i32,
    pub tile_x: i32,
    pub tile_y: i32,
    pub room: &'a mut OutdoorRoom,
}

/// Room substitution `0x006707A0` (§4): rows of type `t` selected by
/// `mask` (bit k = row k), theme `theme`, room seed `seed`.
pub fn room_substitution(
    od: &OutdoorData,
    subs: &dyn SubFiles,
    seed: &mut Seed,
    rs: &mut RoomSub<'_>,
    t: i32,
    theme: i32,
    mask: u32,
) -> Result<(), OutdoorError> {
    if t == -1 {
        return Ok(());
    }
    let mut mask = mask;
    for row in od.sub_rows(t)? {
        if mask == 0 {
            break;
        }
        if mask & 1 != 0 {
            let h = theme_index(theme)?;
            let file = load(subs, row)?;
            if row.check_all != 0 {
                check_all(seed, rs, row, file, h);
            } else {
                scattered(seed, rs, row, file, h);
            }
        }
        mask >>= 1;
    }
    Ok(())
}

/// CheckAll `0x0066FF50` (§4.1).
fn check_all(seed: &mut Seed, rs: &mut RoomSub<'_>, row: &SubRow, file: &SubFile, h: usize) {
    for &g in &file.groups {
        let (gw, gh) = (rs.w - g.w + 1, rs.h - g.h + 1);
        if gw <= 0 || gh <= 0 {
            continue;
        }
        match file.method {
            1 => {
                for y in 1..gh {
                    for x in 1..gw {
                        if fixed_test(rs, file, g, x, y) {
                            apply(rs, file, g, x, y, 0);
                        }
                    }
                }
            }
            2 => {
                for y in 0..gh {
                    for x in 0..gw {
                        if random_test(rs, file, g, x, y) {
                            let r = (seed.step() % 100) as i32;
                            // Edge case 2: the opposite comparison of §3.
                            if row.prob[h] < r {
                                let v = seed.roll(g.variants) as i32;
                                apply(rs, file, g, x, y, (v + 1) * (g.w + 1));
                            }
                        }
                    }
                }
            }
            // TODO(outdoor-tilesub.md §4.1): other methods are not
            // described; nothing.
            _ => {}
        }
    }
}

/// Scattered `0x00670170` (§4.2).
fn scattered(seed: &mut Seed, rs: &mut RoomSub<'_>, row: &SubRow, file: &SubFile, h: usize) {
    let count = file.groups.len() as i32;
    if count == 0 {
        return;
    }
    for _ in 0..row.max[h].max(0) {
        // Site 0x006701DB (edge case 6: drawn even if the group does not
        // fit).
        let g = file.groups[seed.roll(count) as usize];
        let (aw, ah) = (rs.w - g.w, rs.h - g.h);
        if aw <= 0 || ah <= 0 {
            continue;
        }
        let trials = row.trials[h];
        if trials == -1 {
            // Sites 0x006702A4 / 0x006702C3, 0x006702EB.
            for (x, y) in shuffle_cells(seed, aw, ah) {
                if fixed_test(rs, file, g, x + 1, y + 1) {
                    apply(rs, file, g, x + 1, y + 1, 0);
                    break;
                }
            }
        } else if trials > 0 {
            for _ in 0..trials {
                // Sites 0x00670408, 0x0067044D.
                let x = seed.roll(aw) as i32 + 1;
                let y = seed.roll(ah) as i32 + 1;
                if fixed_test(rs, file, g, x, y) {
                    apply(rs, file, g, x, y, 0);
                    break;
                }
            }
        }
    }
}

/// Fixed test `0x0066FCF0` (§4.3).
pub fn fixed_test(rs: &RoomSub<'_>, file: &SubFile, g: SubGroup, x: i32, y: i32) -> bool {
    for j in 0..g.h {
        for i in 0..g.w {
            let pf = file.floor_at(g.x + i, g.y + j);
            let pw = file.wall_at(0, g.x + i, g.y + j);
            let has_wall0 = !file.walls.is_empty();
            if pf & 2 != 0 || (has_wall0 && pw & 1 != 0) {
                let rf = rs.room.floor.get(x + i, y + j);
                let rw = rs.room.wall.get(x + i, y + j);
                if rf & 2 == 0 || rf & 0x3F0_FF00 != 0 || rw & 1 != 0 {
                    return false;
                }
            }
        }
    }
    true
}

/// Random test `0x0066FE00` (§4.3).
pub fn random_test(rs: &RoomSub<'_>, file: &SubFile, g: SubGroup, x: i32, y: i32) -> bool {
    const M: u32 = 0x3F0_FF00;
    for j in 0..g.h {
        for i in 0..g.w {
            let (px, py) = (g.x + i, g.y + j);
            let (rx, ry) = (x + i, y + j);
            if file.tile_type_at(0, px, py) != rs.room.tile_type.get(rx, ry) {
                return false;
            }
            if file.floor.is_some() {
                let pf = file.floor_at(px, py);
                let rf = rs.room.floor.get(rx, ry);
                if pf & 2 != 0 && (rf & 2 == 0 || rf & M != pf & M) {
                    return false;
                }
            }
            if !file.walls.is_empty() {
                let pw = file.wall_at(0, px, py);
                let rw = rs.room.wall.get(rx, ry);
                if pw & 1 != 0 && (rw & 1 == 0 || rw & M != pw & M) {
                    return false;
                }
            }
        }
    }
    true
}

fn set(g: &mut CellGrid, x: i32, y: i32, v: u32) {
    if x >= 0 && y >= 0 && (x as usize) < g.width && (y as usize) < g.height {
        g.set(x as usize, y as usize, v);
    }
}

/// Apply `0x0066FAD0` (§4.4) at room cell (x, y), pattern x offset `o`.
pub fn apply(rs: &mut RoomSub<'_>, file: &SubFile, g: SubGroup, x: i32, y: i32, o: i32) {
    // Step 1: roof list growth.
    let mut roofs = 0;
    for j in 0..g.h {
        for i in 0..g.w {
            if file.shadow_at(g.x + i + o, g.y + j) & 0x800_0000 != 0 {
                roofs += 1;
            }
        }
    }
    rs.room.roof_count += roofs;
    // Step 2.
    for j in 0..g.h {
        for i in 0..g.w {
            let (px, py) = (g.x + i + o, g.y + j);
            let (rx, ry) = (x + i, y + j);
            if file.floor.is_some() {
                let f = file.floor_at(px, py);
                if f & 2 != 0 {
                    set(&mut rs.room.floor, rx, ry, f | 0x80);
                }
            }
            // The room has one wall layer and one tile-type grid (k = 0).
            if !file.walls.is_empty() {
                let w = file.wall_at(0, px, py);
                if w & 1 != 0 {
                    set(&mut rs.room.wall, rx, ry, w);
                }
                let t = file.tile_type_at(0, px, py);
                if t != 0 {
                    set(&mut rs.room.tile_type, rx, ry, t);
                }
            }
            let s = file.shadow_at(px, py);
            if s & 0x800_0000 != 0 {
                rs.room
                    .shadows
                    .push((rs.tile_x + x + i, rs.tile_y + y + j, s));
            }
        }
    }
    // Step 3: preset units of the match pattern box (edge case 3).
    let (bx0, by0) = (g.x * 5, g.y * 5);
    let (bx1, by1) = ((g.x + g.w) * 5, (g.y + g.h) * 5);
    for u in &file.units {
        if bx0 < u.x && u.x < bx1 && by0 < u.y && u.y < by1 {
            let mut nu = *u;
            nu.x = 5 * x + u.x - bx0;
            nu.y = 5 * y + u.y - by0;
            // `0x0066BF30` prepends: the list ends up in reverse of the
            // file list (measured: check combat-pop-stony-field).
            rs.room.units.insert(0, nu);
        }
    }
}
