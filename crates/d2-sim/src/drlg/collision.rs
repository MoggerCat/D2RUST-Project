// Spec: specs/drlg/rooms.md §10
//! Collision grids of active rooms (`rooms.md` §10): one u16 mask per
//! sub-tile, built from the tile records of the room and its adjacency
//! list (§10.4), updated when a linked tile changes (§10.5).

use super::level::Drlg;
use super::tiles::{rec_flags, RecordKind, TileRecord, TileRef};
use super::{DrlgRoomId, TileRect, SUBTILES};

/// Collision bits (§10.6; D2MOO names).
pub mod bits {
    pub const WALL: u16 = 0x0001;
    pub const VISIBLE: u16 = 0x0002;
    pub const MISSILE_BARRIER: u16 = 0x0004;
    pub const NOPLAYER: u16 = 0x0008;
    pub const PRESET: u16 = 0x0010;
    pub const BLANK: u16 = 0x0020;
    pub const MISSILE: u16 = 0x0040;
    pub const PLAYER: u16 = 0x0080;
    pub const MONSTER: u16 = 0x0100;
    pub const ITEM: u16 = 0x0200;
    pub const OBJECT: u16 = 0x0400;
    pub const DOOR: u16 = 0x0800;
    pub const NO_PATH: u16 = 0x1000;
    pub const PET: u16 = 0x2000;
    pub const BIT_4000: u16 = 0x4000;
    pub const CORPSE: u16 = 0x8000;
}

/// A room's collision grid (`0x0064C900`): sub-tile rect and masks,
/// row stride = width.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollisionGrid {
    pub rect: TileRect,
    pub masks: Vec<u16>,
}

impl CollisionGrid {
    /// A zeroed grid over a sub-tile rect.
    pub fn new(rect: TileRect) -> Self {
        let n = (rect.w.max(0) * rect.h.max(0)) as usize;
        Self {
            rect,
            masks: vec![0; n],
        }
    }

    fn index(&self, sx: i32, sy: i32) -> Option<usize> {
        self.rect
            .contains(sx, sy)
            .then(|| ((sy - self.rect.y) * self.rect.w + (sx - self.rect.x)) as usize)
    }

    /// The mask at sub-tile (sx, sy), if inside.
    pub fn get(&self, sx: i32, sy: i32) -> Option<u16> {
        self.index(sx, sy).map(|i| self.masks[i])
    }

    /// Mutable mask at sub-tile (sx, sy), for units' run-time bits.
    pub fn get_mut(&mut self, sx: i32, sy: i32) -> Option<&mut u16> {
        self.index(sx, sy).map(|i| &mut self.masks[i])
    }

    /// OR (`0x0064C4C0`) or clear (`0x0064C580`) DT1 sub-tile bytes over
    /// the 5×5 block at (ox, oy); DT1 rows are bottom row first.
    fn apply_dt1(&mut self, ox: i32, oy: i32, flags: &[u8; 25], set: bool) {
        for r in 0..5 {
            for c in 0..5 {
                let b = u16::from(flags[(5 * (4 - r) + c) as usize]);
                if let Some(m) = self.get_mut(ox + c, oy + r) {
                    if set {
                        *m |= b;
                    } else {
                        *m &= !b;
                    }
                }
            }
        }
    }

    /// Record flags → bits over the block (`0x0064C700`).
    fn apply_record_flags(&mut self, ox: i32, oy: i32, flags: u32) {
        let mut b = 0;
        if flags & rec_flags::DOOR_OR_EXIT != 0 {
            b |= bits::PRESET;
        }
        if flags & rec_flags::UNWALKABLE != 0 {
            b |= bits::WALL;
        }
        if flags & rec_flags::FILL_LOS != 0 {
            b |= bits::MISSILE_BARRIER;
        }
        if b == 0 {
            return;
        }
        for r in 0..5 {
            for c in 0..5 {
                if let Some(m) = self.get_mut(ox + c, oy + r) {
                    *m |= b;
                }
            }
        }
    }
}

impl Drlg {
    /// Build `0x0064C900` (§10.4) for room `id`'s active room: records of
    /// every room of its adjacency list whose origin lies in this room.
    pub(super) fn build_collision(&mut self, id: DrlgRoomId) {
        let Some(active) = self.room(id).active.as_ref() else {
            return;
        };
        let mut grid = CollisionGrid::new(active.subtiles);
        for &r in &active.adjacency {
            let room = self.room(r);
            let Some(tiles) = room.tiles.as_ref() else {
                continue;
            };
            for kind in [RecordKind::Floor, RecordKind::Wall, RecordKind::Shadow] {
                for rec in tiles.records(kind) {
                    let (ox, oy) = record_origin(room.rect, rec);
                    if !grid.rect.contains(ox, oy) {
                        continue;
                    }
                    grid.apply_dt1(ox, oy, &self.tile_info(rec.tile).subtile_flags, true);
                    grid.apply_record_flags(ox, oy, rec.flags);
                }
            }
        }
        self.room_mut(id)
            .active
            .as_mut()
            .expect("checked")
            .collision = grid;
    }

    /// Update `0x0064C860` (§10.5): in the active room among `n` and its
    /// adjacency list containing the record's origin, clear the old
    /// tile's bytes and OR the new tile's. Record-flag bits are untouched.
    pub(super) fn collision_update(
        &mut self,
        n: DrlgRoomId,
        kind: RecordKind,
        i: usize,
        old: Option<TileRef>,
        new: Option<TileRef>,
    ) {
        let Some(active) = self.room(n).active.as_ref() else {
            return;
        };
        let rec = self
            .room(n)
            .tiles
            .as_ref()
            .expect("record exists")
            .records(kind)[i];
        let (ox, oy) = record_origin(self.room(n).rect, &rec);
        let mut cands = vec![n];
        cands.extend(active.adjacency.iter().copied().filter(|&x| x != n));
        let Some(target) = cands.into_iter().find(|&c| {
            self.room(c)
                .active
                .as_ref()
                .is_some_and(|a| a.collision.rect.contains(ox, oy))
        }) else {
            return;
        };
        let old_flags = old.map(|t| self.tile_info(t).subtile_flags);
        let new_flags = new.map(|t| self.tile_info(t).subtile_flags);
        let grid = &mut self
            .room_mut(target)
            .active
            .as_mut()
            .expect("found")
            .collision;
        if let Some(f) = old_flags {
            grid.apply_dt1(ox, oy, &f, false);
        }
        if let Some(f) = new_flags {
            grid.apply_dt1(ox, oy, &f, true);
        }
    }

    /// The collision mask at sub-tile (sx, sy) in any active room of this
    /// DRLG (each sub-tile belongs to one room's grid).
    pub fn collision_at(&self, sx: i32, sy: i32) -> Option<u16> {
        self.active_index.values().find_map(|&r| {
            self.room(r)
                .active
                .as_ref()
                .and_then(|a| a.collision.get(sx, sy))
        })
    }

    /// Mutable collision mask at sub-tile (sx, sy), for units' run-time
    /// bits (owners: movement and unit specs).
    pub fn collision_at_mut(&mut self, sx: i32, sy: i32) -> Option<&mut u16> {
        let r = self.active_index.values().copied().find(|&r| {
            self.room(r)
                .active
                .as_ref()
                .is_some_and(|a| a.collision.rect.contains(sx, sy))
        })?;
        self.room_mut(r).active.as_mut()?.collision.get_mut(sx, sy)
    }
}

/// Sub-tile origin of a record: 5 · (room tile origin + record position).
fn record_origin(room: TileRect, rec: &TileRecord) -> (i32, i32) {
    ((room.x + rec.x) * SUBTILES, (room.y + rec.y) * SUBTILES)
}
