// Spec: specs/drlg/levels.md §11.1–§11.4
//! Logical rooms (`levels.md` §11, D2MOO `DrlgDrlgLogic.cpp`): the
//! coordinate lists a room gets at the end of its tile fill (§11.2), the
//! grid build of preset rooms with lvlprest `Logicals` (§11.3) and the
//! lookups population reads (§11.4). Building draws nothing.

use super::level::Drlg;
use super::room::near_gaps;
use super::tiles::{CellGrid, TileRecord};
use super::{DrlgRoomId, TileRect, SUBTILES};

/// Info flag 1: one record for the whole room (§11.2 step 2).
pub const INFO_ONE: u32 = 1;
/// Info flag 2: built from grids (§11.3).
pub const INFO_GRID: u32 = 2;
/// Index-cell bits (§11.3, `levels.md` Constants).
pub const VISITED: u32 = 0x1000_0000;
pub const NODE: u32 = 0x2000_0000;
pub const INDEX_MASK: u32 = 0x0FFF_FFFF;

/// A coordinate record (0x30 bytes, §11.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoordRec {
    /// x0, y0, x1, y1 in level tiles (x1 / y1 exclusive) (+0x00).
    pub boxr: [i32; 4],
    /// The clipped box population reads (+0x10).
    pub clipped: [i32; 4],
    /// Node flag (+0x20).
    pub node: bool,
    /// Index (+0x28).
    pub index: u32,
}

/// The grids the §11.3 build reads, cut like the tile-fill grids
/// ((W+1) × (H+1), room-relative): wall layer 0's orientation grid
/// (preset room data +0x60), floor layer 0 (+0xB0) and wall layer 0
/// (+0x10).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogicGrids {
    pub orientation: CellGrid,
    pub floor: CellGrid,
    pub wall: CellGrid,
}

/// Logical-room info (0x34 bytes, DRLG room +0x64, §11.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogicInfo {
    /// [`INFO_ONE`] or [`INFO_GRID`] (+0x00).
    pub flags: u32,
    /// Records allocated as one array (+0x04).
    pub array_len: u32,
    /// Index grid, (W+1) × (H+1), row-major; flag 2 only (+0x08).
    pub index_grid: Vec<u32>,
    /// Record grid: per cell, the position in [`LogicInfo::list`] of its
    /// record; flag 2 only (+0x1C).
    pub record_grid: Vec<usize>,
    /// The records in `next` order, head first (+0x30).
    pub list: Vec<CoordRec>,
}

impl Drlg {
    /// `0x0066CCB0` (§11.2 step 2): one record for the whole room; the
    /// level counter := 1.
    pub(super) fn build_logic_one(&mut self, id: DrlgRoomId) {
        let r = self.room(id).rect;
        let l = self.room(id).level;
        // Always 1, overwriting the counter (`levels.md` edge case 6).
        self.level_mut(l).coord_counter = 1;
        let b = [r.x, r.y, r.x + r.w, r.y + r.h];
        self.room_mut(id).logic = Some(LogicInfo {
            flags: INFO_ONE,
            array_len: 1,
            index_grid: Vec::new(),
            record_grid: Vec::new(),
            list: vec![CoordRec {
                boxr: b,
                clipped: b,
                node: false,
                index: 1,
            }],
        });
    }

    /// `0x0066D110` (§11.3): the grid build.
    pub(super) fn build_logic_grid(&mut self, id: DrlgRoomId, g: &LogicGrids) {
        let rect = self.room(id).rect;
        let l = self.room(id).level;
        let (cw, ch) = ((rect.w + 1) as usize, (rect.h + 1) as usize);
        // Step 1: info (flag 2) and a zeroed index grid.
        let mut index = vec![0u32; cw * ch];
        // Step 2 (tree marks, `0x0066C7F0`): ORs 8 into the wall layer 0
        // grid for wall records with flag 0x4 and no layer bits. Wall
        // records always carry layer bits, and no later step reads that
        // grid, so nothing is done here.
        // Step 3.
        if self.level(l).coord_counter == 0 {
            self.level_mut(l).coord_counter = 1;
        }
        let start = self.level(l).coord_counter;
        // Step 4.
        let blockers = self.blocker_grid(id, cw, ch);
        // Step 5: regions.
        let mut counter = start;
        for y in 0..ch {
            for x in 0..cw {
                if index[y * cw + x] & VISITED != 0 {
                    continue;
                }
                counter = counter.wrapping_add(1);
                let mut m = (counter & INDEX_MASK) | VISITED;
                let v = g.floor.get(x as i32, y as i32);
                if v & 0x01E0_FF00 == 0x01E0_0000 || v & 0x8000_0000 != 0 {
                    m |= NODE;
                }
                fill(
                    &mut index,
                    &blockers,
                    &g.orientation,
                    rect,
                    (x as i32, y as i32, -1),
                    m,
                );
            }
        }
        // Step 7.
        let array_len = counter.wrapping_sub(start).wrapping_add(1);
        self.level_mut(l).coord_counter = counter.wrapping_add(array_len);
        // Step 8: rectangles, in scan order (prepended below).
        let mut cell_rec: Vec<Option<usize>> = vec![None; cw * ch];
        let mut made: Vec<CoordRec> = Vec::new();
        for y in 0..ch {
            for x in 0..cw {
                if cell_rec[y * cw + x].is_some() {
                    continue;
                }
                let v = index[y * cw + x];
                let free = |cx: usize, cy: usize, cr: &[Option<usize>]| {
                    index[cy * cw + cx] == v && cr[cy * cw + cx].is_none()
                };
                let mut x1 = x;
                while x1 < cw && free(x1, y, &cell_rec) {
                    x1 += 1;
                }
                let mut y1 = y + 1;
                while y1 < ch && (x..x1).all(|cx| free(cx, y1, &cell_rec)) {
                    y1 += 1;
                }
                let k = made.len();
                for cy in y..y1 {
                    for cx in x..x1 {
                        cell_rec[cy * cw + cx] = Some(k);
                    }
                }
                let boxr = [
                    x as i32 + rect.x,
                    y as i32 + rect.y,
                    x1 as i32 + rect.x,
                    y1 as i32 + rect.y,
                ];
                let (ex, ey) = (rect.x + rect.w, rect.y + rect.h);
                let clipped = if boxr[0] >= ex || boxr[1] >= ey {
                    [0; 4]
                } else {
                    [boxr[0], boxr[1], boxr[2].min(ex), boxr[3].min(ey)]
                };
                made.push(CoordRec {
                    boxr,
                    clipped,
                    node: v & NODE != 0,
                    index: v & INDEX_MASK,
                });
            }
        }
        // List: the rectangles prepended (reverse scan order), then the
        // first element of the step-7 array (zeroed); the rest of the
        // array is never linked.
        let n = made.len();
        let mut list: Vec<CoordRec> = made.into_iter().rev().collect();
        list.push(CoordRec::default());
        let record_grid = cell_rec
            .into_iter()
            .map(|k| n - 1 - k.expect("every cell gets a record"))
            .collect();
        // Step 9 (wall record +0x10 := its record) is read by drawing
        // code only; tile records carry no such field here.
        self.room_mut(id).logic = Some(LogicInfo {
            flags: INFO_GRID,
            array_len,
            index_grid: index,
            record_grid,
            list,
        });
        // Step 10.
        self.merge_logic(id);
    }

    /// Step 4 (`0x0066C870`): the blocker grid, (W+1) × (H+1).
    fn blocker_grid(&self, id: DrlgRoomId, cw: usize, ch: usize) -> Vec<bool> {
        let room = self.room(id);
        let rect = room.rect;
        let mut b = vec![false; cw * ch];
        let blocks = |r: &TileRecord| {
            r.flags & LAYER_BITS == 0x4000 && r.kind != ROOF && r.flags & OBJECT_WALL == 0
        };
        if let Some(t) = &room.tiles {
            for r in t.walls.iter().filter(|r| blocks(r)) {
                // TODO(spec: levels.md §11.3 r4): own wall records are
                // room-relative 0..=W / 0..=H; the original's local grid
                // (1,024 cells) has no bound check. Out-of-grid records
                // are skipped here.
                if (0..cw as i32).contains(&r.x) && (0..ch as i32).contains(&r.y) {
                    b[r.y as usize * cw + r.x as usize] = true;
                }
            }
        }
        for &n in room.near.as_deref().unwrap_or(&[]) {
            if n == id {
                continue;
            }
            let nr = self.room(n);
            let Some(t) = &nr.tiles else { continue };
            for &(kind, i) in &t.other_links {
                let r = &t.records(kind)[i];
                let (px, py) = (nr.rect.x + r.x, nr.rect.y + r.y);
                if blocks(r) && rect.contains_closed(px, py) {
                    b[(py - rect.y) as usize * cw + (px - rect.x) as usize] = true;
                }
            }
        }
        b
    }

    /// Step 10 (`0x0066D040`): merge with the rooms near.
    fn merge_logic(&mut self, id: DrlgRoomId) {
        let rect = self.room(id).rect;
        let near: Vec<DrlgRoomId> = self.room(id).near.clone().unwrap_or_default();
        for n in near {
            if n == id || self.room(n).logic.is_none() {
                continue;
            }
            let (gx, gy) = near_gaps(&rect, &self.room(n).rect);
            if !(gx < 1 && gy < 1) {
                continue;
            }
            for x in rect.x..=rect.x + rect.w {
                self.merge_cell(id, n, x, rect.y);
                self.merge_cell(id, n, x, rect.y + rect.h);
            }
            for y in rect.y..=rect.y + rect.h {
                self.merge_cell(id, n, rect.x, y);
                self.merge_cell(id, n, rect.x + rect.w, y);
            }
        }
    }

    /// `0x0066CF60` (step 10, one cell in tiles).
    fn merge_cell(&mut self, id: DrlgRoomId, n: DrlgRoomId, x: i32, y: i32) {
        if !self.room(n).rect.contains_closed(x, y) {
            return;
        }
        let (Some(a), Some(b)) = (self.coord_at_tile(n, x, y), self.coord_at_tile(id, x, y))
        else {
            return;
        };
        let same_level =
            self.level(self.room(n).level).id == self.level(self.room(id).level).id;
        if a.index != 0 && b.index != 0 && same_level && a.index != b.index && a.node == b.node
        {
            self.rename_logic(id, b.index, a.index);
        }
    }

    /// Step 11 (`0x0066C770`): rename `old` to `new` in a grid-built room
    /// and, if any record changed, in its rooms near of the same level id.
    fn rename_logic(&mut self, id: DrlgRoomId, old: u32, new: u32) {
        let Some(info) = self.room_mut(id).logic.as_mut() else {
            return;
        };
        if info.flags != INFO_GRID {
            return;
        }
        let mut any = false;
        for r in info.list.iter_mut().filter(|r| r.index == old) {
            r.index = new;
            any = true;
        }
        if !any {
            return;
        }
        let lid = self.level(self.room(id).level).id;
        let near: Vec<DrlgRoomId> = self.room(id).near.clone().unwrap_or_default();
        for n in near {
            if n != id && self.level(self.room(n).level).id == lid {
                self.rename_logic(n, old, new);
            }
        }
    }

    /// The record at a tile point (§11.4 without the sub-tile scaling).
    fn coord_at_tile(&self, id: DrlgRoomId, x: i32, y: i32) -> Option<CoordRec> {
        let room = self.room(id);
        let info = room.logic.as_ref()?;
        if info.flags == INFO_ONE {
            return info.list.first().copied();
        }
        let (cx, cy) = (x - room.rect.x, y - room.rect.y);
        let cw = room.rect.w + 1;
        if cx < 0 || cy < 0 || cx >= cw || cy > room.rect.h {
            return None;
        }
        let k = info.record_grid[(cy * cw + cx) as usize];
        Some(info.list[k])
    }

    /// `0x0066CF30` (§11.4): the record list of the room's info, head
    /// first; `None` without info (fatal 0x2CD in the original).
    pub fn coord_first(&self, id: DrlgRoomId) -> Option<&[CoordRec]> {
        self.room(id).logic.as_ref().map(|l| l.list.as_slice())
    }

    /// `0x0066CEB0` (§11.4): the record at a sub-tile point: one-record
    /// room → its record; else the record grid at (x/5 − X, y/5 − Y)
    /// (C division). `None` without info or for a point outside the grid
    /// (the original reads outside it).
    pub fn coord_at(&self, id: DrlgRoomId, x: i32, y: i32) -> Option<CoordRec> {
        self.coord_at_tile(id, x / SUBTILES, y / SUBTILES)
    }
}

/// T1 `0x006EEEA0`: rule row by orientation (§11.3 step 6.3).
const T1: [i32; 20] = [-1, 0, 1, 2, 2, 0, 1, 3, 0, 1, 0, 1, 4, -1, 4, 0, 0, 0, 0, 0];
/// T2 `0x006EEE38`: fill rules (26 entries).
const T2: [u32; 26] = [
    23, 0, 5, 21, 17, 15, 3, 0, 9, 7, 39, 0, 0, 5, 3, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 0,
];
/// The five dwords before T2 (`0x006EEE24`–`0x006EEE34`), read for
/// T1 = −1: indexes −5..−1 (d = −1..3).
const BEFORE_T2: [u32; 5] = [0xFFFF_FFFF, 0, 0, 0xFFFF_FFFF, 0];
/// Direction offsets `0x006EEE14`: +x, +y, −x, −y.
const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
/// Record layer bits; roof type; object-wall flag (§11.3 step 4).
const LAYER_BITS: u32 = 0x1_C000;
const ROOF: u32 = 15;
const OBJECT_WALL: u32 = 0x800;

/// The step-6.3 rule for orientation `o` entered in direction `d`.
fn rule(o: u32, d: i32) -> u32 {
    // TODO(spec: levels.md §11.3 r6): orientations above 19 read past
    // T1 (no bound check, values not given); treated as rule 0 here.
    let Some(&t1) = T1.get(o as usize) else {
        return 0;
    };
    let i = d + 5 * t1 + 1;
    if i < 0 {
        BEFORE_T2[(i + 5) as usize]
    } else {
        T2[i as usize]
    }
}

/// Fill `0x0066C3D0` (step 6). The recursion is run on an explicit stack
/// in the original's call order: a call's neighbour fills run before its
/// step-6.5 continuation, which is the call's last action.
fn fill(
    index: &mut [u32],
    blockers: &[bool],
    orient: &CellGrid,
    rect: TileRect,
    at: (i32, i32, i32),
    m: u32,
) {
    let cw = (rect.w + 1) as usize;
    let mut stack = vec![at];
    while let Some((x, y, d)) = stack.pop() {
        // 6.1
        if !rect.contains_closed(rect.x + x, rect.y + y) {
            continue;
        }
        let c = y as usize * cw + x as usize;
        if index[c] & VISITED != 0 {
            continue;
        }
        // 6.2
        if !blockers[c] {
            index[c] |= m;
            for k in (0..4).rev() {
                let (dx, dy) = DIRS[k];
                stack.push((x + dx, y + dy, k as i32));
            }
            continue;
        }
        // 6.3–6.5
        let r = rule(orient.get(x, y), d);
        if r & 1 != 0 {
            index[c] |= m;
        }
        if r & 32 != 0 {
            stack.push((x + 1, y + 1, -1));
        }
        let calls = [(2, 2), (4, 3), (8, 0), (16, 1)];
        for k in (0..4).rev() {
            let (bit, not_d) = calls[k];
            if r & bit != 0 && d != not_d {
                let (dx, dy) = DIRS[k];
                stack.push((x + dx, y + dy, k as i32));
            }
        }
    }
}
