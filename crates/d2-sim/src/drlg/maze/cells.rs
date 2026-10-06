// Spec: specs/drlg/maze.md
//! Cells, sides and links (§2) and the cell primitives (§3): allocate,
//! place test, pick shape, merge, the attach variants, special-room
//! stamps, probes, extreme-cell finders and the random cell.

use std::collections::BTreeMap;

use super::{MazeData, MazeError, MazeRow};
use crate::drlg::level::Drlg;
use crate::drlg::room::{LinkAt, RoomKind};
use crate::drlg::{DrlgRoomId, LevelIdx, TileRect};

/// What a cell link points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    /// A cell (room) of the same level.
    Cell(DrlgRoomId),
    /// Another level (cross-level link `0x0066B790`, §7).
    // TODO(spec: maze.md §7.1, rooms.md): the cross-level link's target
    // is named as the level; its box (for a place test from this cell)
    // is taken as the level rect. Never read in 1.14d paths: the cells
    // carrying such links are locked, so nothing grows from them.
    Level(LevelIdx),
}

/// A link ("orth", §2.2): neighbour, direction, init flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MazeLink {
    pub target: LinkTarget,
    pub dir: u8,
    pub init: bool,
}

/// The maze data of one cell (§2.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    /// lvlprest Def (0 at allocation).
    pub def: u32,
    /// File index (0 at allocation; −1 = rotate / default).
    pub file: i32,
    /// Lock flag (preset-room flag bit 1, D2MOO `HAS_MAP_DS1`).
    pub lock: bool,
    /// Links in the order they were made.
    // TODO(spec: maze.md §2.4): the order of a room's link list
    // (prepend or append) is not stated; it only reaches the preset
    // builder (§9 step 4).
    pub links: Vec<MazeLink>,
}

impl Cell {
    fn linked_to(&self, other: DrlgRoomId) -> bool {
        self.links
            .iter()
            .any(|l| l.target == LinkTarget::Cell(other))
    }
}

/// Shape bits by direction (§3.3): W 1, N 8, E 2, S 4.
pub fn shape_bit(dir: u8) -> u32 {
    match dir {
        0 => 1,
        1 => 8,
        2 => 2,
        3 => 4,
        _ => 0,
    }
}

/// Origin of the cell adjacent to `p` in direction `d` (§2.1); the
/// offsets use the parent's size.
pub fn adjacent(p: &TileRect, d: u8) -> (i32, i32) {
    let x = match d {
        0 | 4 | 7 => p.x - p.w,
        2 | 5 | 6 => p.x + p.w,
        _ => p.x,
    };
    let y = match d {
        1 | 4 | 5 => p.y - p.h,
        3 | 6 | 7 => p.y + p.h,
        _ => p.y,
    };
    (x, y)
}

/// Gaps of the overlap test `0x0066B800` (§2.5).
pub fn gaps(a: &TileRect, b: &TileRect) -> (i32, i32) {
    let dx = if a.x < b.x {
        b.x - a.w - a.x
    } else {
        a.x - b.w - b.x
    };
    let dy = if a.y < b.y {
        b.y - a.h - a.y
    } else {
        a.y - b.h - b.y
    };
    (dx, dy)
}

/// Whether `a` and `b` collide at margin `m` (§2.5).
pub fn collide(a: &TileRect, b: &TileRect, m: i32) -> bool {
    let (dx, dy) = gaps(a, b);
    dx < m && dy < m
}

/// Direction from `a` to `b` (`0x00642240`, §2.6), −1 if not adjacent.
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

/// Pick-shape def by level type and mask (§3.3), before overrides.
/// `rooms_one`: lvlmaze `Rooms[d]` = 1 (`0x00670810`, Ice only).
pub fn shape_def(level_type: u32, mask: u32, rooms_one: bool) -> Result<u32, MazeError> {
    let remap = |t: &[(u32, u32)]| t.iter().find(|e| e.0 == mask).map_or(0, |e| e.1);
    Ok(match level_type {
        3 => 52 + mask,
        4 => 108 + mask,
        7 => 167 + mask,
        8 => 205 + mask,
        10 => 257 + mask,
        13 => 301 + mask,
        14 => remap(&[(5, 356), (6, 355), (9, 357), (10, 354)]),
        15 => remap(&[(5, 360), (6, 359), (9, 361), (10, 358)]),
        23 => remap(&[(5, 659), (6, 660), (9, 661), (10, 662)]),
        17 => 413 + mask,
        18 => 481 + mask,
        19 => 509 + mask,
        22 => 753 + mask,
        24 => 664 + mask,
        25 => 704 + mask,
        28 => 836 + mask,
        32 => remap(&[(5, 1045), (6, 1044), (9, 1043), (10, 1042)]),
        35 => remap(&[
            (1, 1056),
            (2, 1055),
            (3, 1057),
            (4, 1054),
            (8, 1053),
            (12, 1058),
        ]),
        33 if rooms_one => mask,
        33 => 1002 + mask,
        34 => 1058 + mask,
        t => return Err(MazeError::BadLevelType(t)),
    })
}

/// Per-level overrides after the lookup (§3.3): (def, file override).
pub fn shape_override(level_id: u32, def: u32) -> (u32, Option<i32>) {
    match (level_id, def) {
        (52, 361) => (def, Some(2)),
        (54, 361 | 359) => (def, Some(3)),
        (84, 662) => (664, None),
        (85, 661) => (663, None),
        _ => (def, None),
    }
}

/// Generation state of one level: the DRLG, the level, its lvlmaze row
/// and the maze data of every live cell.
pub struct Gen<'a> {
    pub drlg: &'a mut Drlg,
    pub level: LevelIdx,
    pub level_id: u32,
    pub level_type: u32,
    pub row: MazeRow,
    pub data: &'a MazeData,
    pub cells: BTreeMap<DrlgRoomId, Cell>,
}

impl<'a> Gen<'a> {
    pub fn new(
        drlg: &'a mut Drlg,
        level: LevelIdx,
        row: MazeRow,
        data: &'a MazeData,
    ) -> Result<Self, MazeError> {
        let l = drlg.level(level);
        let (level_id, level_type) = (l.id, l.level_type);
        Ok(Self {
            drlg,
            level,
            level_id,
            level_type,
            row,
            data,
            cells: BTreeMap::new(),
        })
    }

    // ---- level, list and seeds -------------------------------------------

    /// The level's rooms in list order (newest first, §2.3).
    pub fn list(&self) -> Vec<DrlgRoomId> {
        self.drlg.level_rooms(self.level)
    }

    /// Level room count (+0x08).
    pub fn count(&self) -> u32 {
        self.drlg.room_count(self.level)
    }

    /// One level-seed step.
    pub fn level_step(&mut self) -> u32 {
        self.drlg.level_mut(self.level).seed.step()
    }

    /// One step of a cell's room seed.
    pub fn room_step(&mut self, c: DrlgRoomId) -> u32 {
        self.drlg.room_mut(c).seed.step()
    }

    pub fn rect(&self, c: DrlgRoomId) -> TileRect {
        self.drlg.room(c).rect
    }

    pub fn cell(&self, c: DrlgRoomId) -> &Cell {
        &self.cells[&c]
    }

    pub fn cell_mut(&mut self, c: DrlgRoomId) -> &mut Cell {
        self.cells.get_mut(&c).expect("live maze cell")
    }

    /// Set def, file and lock (fixed / special cells).
    pub fn set(&mut self, c: DrlgRoomId, def: u32, file: i32) {
        let cell = self.cell_mut(c);
        cell.def = def;
        cell.file = file;
        cell.lock = true;
    }

    /// Lookup rect of a link target.
    fn target_rect(&self, t: LinkTarget) -> TileRect {
        match t {
            LinkTarget::Cell(c) => self.rect(c),
            LinkTarget::Level(l) => self.drlg.level(l).rect,
        }
    }

    // ---- §3.1 allocate, add, free ------------------------------------------

    /// `0x0066B3E0` (§3.1): a new cell of lvlmaze size, not in the list.
    /// Two draws: a level-seed step and the new room's seed step.
    pub fn alloc(&mut self) -> DrlgRoomId {
        let rect = TileRect::new(0, 0, self.row.size_x, self.row.size_y);
        let c = self.drlg.alloc_room(self.level, RoomKind::Preset, rect);
        self.cells.insert(c, Cell::default());
        c
    }

    /// Prepend a cell to the level list and count it (§2.3).
    pub fn add(&mut self, c: DrlgRoomId) {
        self.drlg.link_room(c, LinkAt::Head);
    }

    /// Free a cell that is not in the list (`0x0066C100`).
    pub fn free_unlisted(&mut self, c: DrlgRoomId) {
        self.cells.remove(&c);
        self.drlg.free_room(c);
    }

    /// Unlink a listed cell, decrement the count and free it (§2.3).
    pub fn free_listed(&mut self, c: DrlgRoomId) {
        let next = self.drlg.room(c).next;
        if self.drlg.level(self.level).first_room == Some(c) {
            self.drlg.level_mut(self.level).first_room = next;
        } else {
            let prev = self
                .list()
                .into_iter()
                .find(|&r| self.drlg.room(r).next == Some(c))
                .expect("listed cell has a predecessor");
            self.drlg.room_mut(prev).next = next;
        }
        self.drlg.level_mut(self.level).room_count -= 1;
        self.drlg.room_mut(c).next = None;
        self.free_unlisted(c);
    }

    // ---- §2.4 links ---------------------------------------------------------

    /// `0x0066B5E0` (§2.4): P gets (N, d), N gets (P, (d+2) mod 4), each
    /// only if it has no link to the other yet. Lock is not touched.
    pub fn link(&mut self, p: DrlgRoomId, n: DrlgRoomId, d: u8) {
        if !self.cell(p).linked_to(n) {
            self.cell_mut(p).links.push(MazeLink {
                target: LinkTarget::Cell(n),
                dir: d,
                init: true,
            });
        }
        if !self.cell(n).linked_to(p) {
            self.cell_mut(n).links.push(MazeLink {
                target: LinkTarget::Cell(p),
                dir: (d + 2) % 4,
                init: true,
            });
        }
    }

    /// Cross-level link `0x0066B790` (§7): no init flag, one side only.
    pub fn link_level(&mut self, c: DrlgRoomId, l: LevelIdx, d: u8) {
        self.cell_mut(c).links.push(MazeLink {
            target: LinkTarget::Level(l),
            dir: d,
            init: false,
        });
    }

    // ---- §3.2 place test ----------------------------------------------------

    /// Allocate N and run the place test `0x00670880` next to `p` in
    /// direction `d` (§3.2). A rejected cell is freed: `None`.
    pub fn place(&mut self, p: DrlgRoomId, d: u8) -> Option<DrlgRoomId> {
        let n = self.alloc();
        let pr = self.rect(p);
        let (x, y) = adjacent(&pr, d);
        let rect = {
            let r = &mut self.drlg.room_mut(n).rect;
            r.x = x;
            r.y = y;
            *r
        };
        let hits_link = self
            .cell(p)
            .links
            .iter()
            .any(|l| collide(&rect, &self.target_rect(l.target), 0));
        let hits_room = self
            .list()
            .into_iter()
            .any(|r| r != p && r != n && collide(&rect, &self.rect(r), 0));
        if hits_link || hits_room {
            self.free_unlisted(n);
            None
        } else {
            Some(n)
        }
    }

    // ---- §3.3 pick shape ----------------------------------------------------

    /// `0x006709B0` with "unlock" (§3.3).
    pub fn pick(&mut self, c: DrlgRoomId) -> Result<(), MazeError> {
        let mask = self
            .cell(c)
            .links
            .iter()
            .fold(0, |m, l| m | shape_bit(l.dir));
        let difficulty = self.drlg.difficulty as usize;
        let rooms_one = self.row.rooms.get(difficulty).copied() == Some(1);
        let def = shape_def(self.level_type, mask, rooms_one)?;
        let (def, file) = shape_override(self.level_id, def);
        if def != 0 {
            let cell = self.cell_mut(c);
            cell.def = def;
            cell.file = file.unwrap_or(-1);
            cell.lock = false;
        }
        Ok(())
    }

    // ---- §3.4 merge ---------------------------------------------------------

    /// `0x00670C70` (§3.4) for a new cell N not yet in the list.
    pub fn merge(&mut self, n: DrlgRoomId) -> Result<(), MazeError> {
        if self.cell(n).lock {
            return Ok(());
        }
        let nr = self.rect(n);
        for r in self.list() {
            if r == n || self.cell(r).lock {
                continue;
            }
            let rr = self.rect(r);
            let (dx, dy) = gaps(&nr, &rr);
            if !(dx < 1 && dy < 1 && dx != dy) || self.cell(n).linked_to(r) {
                continue;
            }
            let lo = self.room_step(r);
            if ((lo % 1000) as i32) < self.row.merge {
                let dir = direction(&rr, &nr);
                if dir != -1 {
                    self.link(r, n, dir as u8);
                    self.pick(r)?;
                }
            }
        }
        Ok(())
    }

    // ---- §3.5 attach variants -----------------------------------------------

    /// "grow": place; link P→N, merge N, add N, pick P, pick N.
    pub fn grow(&mut self, p: DrlgRoomId, d: u8) -> Result<Option<DrlgRoomId>, MazeError> {
        let Some(n) = self.place(p, d) else {
            return Ok(None);
        };
        self.link(p, n, d);
        self.merge(n)?;
        self.add(n);
        self.pick(p)?;
        self.pick(n)?;
        Ok(Some(n))
    }

    /// "fixed" `0x00670DE0`: place; link, add, pick P if asked, then N
    /// gets def, file and lock.
    pub fn fixed(
        &mut self,
        p: DrlgRoomId,
        d: u8,
        def: u32,
        file: i32,
        pick_p: bool,
    ) -> Result<Option<DrlgRoomId>, MazeError> {
        let Some(n) = self.place(p, d) else {
            return Ok(None);
        };
        self.link(p, n, d);
        self.add(n);
        if pick_p {
            self.pick(p)?;
        }
        self.set(n, def, file);
        Ok(Some(n))
    }

    /// "special fallback" `0x00670EB0`: "fixed" with pick-P from each
    /// unlocked room in list order until one succeeds.
    pub fn special_fallback(
        &mut self,
        d: u8,
        def: u32,
        file: i32,
    ) -> Result<Option<DrlgRoomId>, MazeError> {
        for r in self.list() {
            if self.cell(r).lock {
                continue;
            }
            if let Some(n) = self.fixed(r, d, def, file, true)? {
                return Ok(Some(n));
            }
        }
        Ok(None)
    }

    /// "blank" `0x00671320` attach: place; link, add, def, file −1,
    /// lock (no pick).
    pub fn blank(&mut self, p: DrlgRoomId, d: u8, def: u32) -> Option<DrlgRoomId> {
        let n = self.place(p, d)?;
        self.link(p, n, d);
        self.add(n);
        self.set(n, def, -1);
        Some(n)
    }

    // ---- §3.6 special-room stamp --------------------------------------------

    /// `0x006724E0` (§3.6) with row `row` of table `kind`. Returns
    /// whether a cell became the special (edge case 4: the caller's
    /// counter advances either way).
    pub fn stamp(&mut self, kind: &'static str, row: usize) -> Result<bool, MazeError> {
        let s = self
            .data
            .specials
            .row(kind, row)
            .ok_or(MazeError::NoSpecialRow(kind, row))?;
        let found = self.list().into_iter().find(|&r| {
            let c = self.cell(r);
            !c.lock && c.def == s.find
        });
        if let Some(r) = found {
            self.set(r, s.special, s.file);
            return Ok(true);
        }
        Ok(self.special_fallback(s.dir, s.special, s.file)?.is_some())
    }

    /// A stamp with a counter: row `r`, then r := (r + 1) mod 4.
    pub fn stamp_next(&mut self, kind: &'static str, r: &mut u32) -> Result<(), MazeError> {
        self.stamp(kind, *r as usize)?;
        *r = (*r + 1) % 4;
        Ok(())
    }

    // ---- §3.7 probe and extreme cells ---------------------------------------

    /// `0x00672340` (§3.7): whether a cell could be placed next to `p` in
    /// direction `d`. Net: P and the list unchanged.
    pub fn probe(&mut self, p: DrlgRoomId, d: u8) -> Result<bool, MazeError> {
        {
            let c = self.cell(p);
            if c.lock || c.links.iter().any(|l| l.dir == d) {
                return Ok(false);
            }
        }
        let Some(n) = self.place(p, d) else {
            return Ok(false);
        };
        self.link(p, n, d);
        self.add(n);
        self.pick(n)?;
        // "net: P unchanged": the freed cell's link leaves P.
        // TODO(spec: maze.md §3.7, rooms.md): what `0x0066C100` does to
        // the neighbour's link list is not described; removal is read
        // from "net: P unchanged".
        self.cell_mut(p)
            .links
            .retain(|l| l.target != LinkTarget::Cell(n));
        self.free_listed(n);
        Ok(true)
    }

    /// The extreme-cell finders (§3.7): walk the list, probing a room only
    /// when there is no best yet or it strictly improves on the best.
    pub fn extreme(&mut self, which: Extreme) -> Result<Option<DrlgRoomId>, MazeError> {
        let mut best: Option<DrlgRoomId> = None;
        for r in self.list() {
            let better = match best {
                None => true,
                Some(b) => {
                    let (rr, br) = (self.rect(r), self.rect(b));
                    match which {
                        Extreme::MinY => rr.y < br.y,
                        Extreme::MaxX => rr.x > br.x,
                        Extreme::MaxY => rr.y > br.y,
                        Extreme::MinX => rr.x < br.x,
                    }
                }
            };
            if better && self.probe(r, which.dir())? {
                best = Some(r);
            }
        }
        Ok(best)
    }

    // ---- §3.8 random cell ---------------------------------------------------

    /// `0x006711A0` (§3.8): `roll(count)` on the level seed, then walk
    /// that many rooms from the head.
    pub fn random_cell(&mut self) -> Option<DrlgRoomId> {
        let count = self.count() as i32;
        let k = self.drlg.level_mut(self.level).seed.roll(count);
        self.list().get(k as usize).copied()
    }

    // ---- §4.3 bounding box --------------------------------------------------

    /// `0x00642520`: bounding box of the listed cells.
    pub fn bounding_box(&self) -> TileRect {
        let list = self.list();
        let mut it = list.iter().map(|&c| self.rect(c));
        let Some(first) = it.next() else {
            return TileRect::default();
        };
        let (mut x0, mut y0) = (first.x, first.y);
        let (mut x1, mut y1) = (first.x + first.w, first.y + first.h);
        for r in it {
            x0 = x0.min(r.x);
            y0 = y0.min(r.y);
            x1 = x1.max(r.x + r.w);
            y1 = y1.max(r.y + r.h);
        }
        TileRect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// Move every listed cell by (dx, dy).
    pub fn shift(&mut self, dx: i32, dy: i32) {
        for c in self.list() {
            let r = &mut self.drlg.room_mut(c).rect;
            r.x += dx;
            r.y += dy;
        }
    }
}

/// The four extreme-cell finders (§3.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extreme {
    /// `0x006723E0`: smallest y, probe N.
    MinY,
    /// `0x00672460`: largest x, probe E.
    MaxX,
    /// `0x006724A0`: largest y, probe S.
    MaxY,
    /// `0x00672420`: smallest x, probe W.
    MinX,
}

impl Extreme {
    pub fn dir(self) -> u8 {
        match self {
            Extreme::MinY => 1,
            Extreme::MaxX => 2,
            Extreme::MaxY => 3,
            Extreme::MinX => 0,
        }
    }
}
