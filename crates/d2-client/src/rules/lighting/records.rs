// Spec: specs/render/lighting.md (§6 light records and the list)
//! Light records (§6.1), their operations (§6.2), the list (§6.3), the
//! per-drawn-frame update (§6.4) and the room-leave rule (§6.4, last
//! paragraph). The client world is read through [`LightWorld`]; fatal
//! cases of 1.14d are [`LightError`]s, never panics.

use super::contribute;
use super::map::LightMap;

/// Owner type of a record without owner (§6.1 `+0x00`).
pub const OWNER_NONE: u32 = 6;
/// Owner GUID of a record without owner (§6.1 `+0x04`).
pub const GUID_NONE: u32 = u32::MAX;
/// Largest radius in sub-tiles (§6.2 r1–r3).
pub const MAX_RADIUS: i32 = 18;

/// Unit types as the original numbers them (owner type, §6.1, §7.2).
pub mod unit_type {
    pub const PLAYER: u32 = 0;
    pub const MONSTER: u32 = 1;
    pub const OBJECT: u32 = 2;
    pub const MISSILE: u32 = 3;
    pub const ITEM: u32 = 4;
    pub const TILE: u32 = 5;
}

/// A room of the client, as the caller identifies it (pointer compare in
/// 1.14d).
pub type RoomId = u32;

/// Fatal cases of 1.14d (§6.2, §6.4, §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LightError {
    /// Remove of a record not in the list (§6.2 r5, fatal).
    #[error("light record not in the list")]
    NotInList,
    /// Die on a kind-2 record (§6.2 r6, fatal).
    #[error("die on a cached (kind 2) light record")]
    DieOnCached,
    /// Room leave: a kind-2 record whose owner is none or not found
    /// (§6.4, fatal 0x591).
    #[error("fatal 0x591: cached light record without a found owner")]
    Fatal0x591,
    /// Room leave: the owner of a kind-2 record has no room. The spec does
    /// not say what `0x00463740` does from no room (open question).
    #[error("cached light record owner has no room (unspecified)")]
    OwnerWithoutRoom,
    /// Cache build of a kind-2 record with no owner or no room (§7.4 r1,
    /// fatal).
    #[error("cached light record built without owner or room")]
    CacheWithoutOwner,
}

/// The owner of a record (§6.1 `+0x00..+0x08`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Owner {
    pub unit_type: u32,
    pub guid: u32,
    /// Lookup flag: owner unit flags `+0xC4` bit 21 (client-only, set C).
    pub client_only: bool,
}

/// What the light code reads from the client world.
pub trait LightWorld {
    /// The owner's precise position (16.16) when the lookup of §6.4 r1
    /// finds it (set C when `client_only`, else set S).
    fn owner_position(&self, owner: &Owner) -> Option<(i32, i32)>;
    /// The owner's sub-tile (§6.4 last paragraph, §7.4 r1): static path
    /// for types 2, 4, 5, dynamic path for 0, 1, 3 (0 without a path).
    /// `None` when the owner is not found.
    fn owner_subtile(&self, owner: &Owner) -> Option<(i32, i32)>;
    /// The owner's room (`0x00620BB0`); `None` when it has none.
    fn owner_room(&self, owner: &Owner) -> Option<RoomId>;
    /// Whether the owner is the local player (`0x00463DE0`, §7.2).
    fn is_local_player(&self, owner: &Owner) -> bool;
    /// The collision point test with mask 0x22 searched from the owner's
    /// room at sub-tile `(x, y)` is non-zero (§7.4 r1).
    fn owner_blocks(&self, owner: &Owner, x: i32, y: i32) -> bool;
    /// The cell lookup `0x00463740(room, x, y)`: the room holding sub-tile
    /// `(x, y)` searched from `room` (§6.4 last paragraph).
    fn cell_room(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId>;
}

/// The kind of a record (§6.1 `+0x0C`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LightKind {
    /// 0: shadowed when `q` = 2.
    Shadowed,
    /// 1: never shadowed.
    Plain,
    /// 2: shadowed with a cached grid.
    Cached,
}

/// A light's position from a unit's precise 16.16 coordinate (§6.1):
/// `(P >> 13) + 4`, in 1/8 sub-tile.
pub fn unit_light_pos(precise: i32) -> i32 {
    (precise >> 13) + 4
}

/// One light record (§6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightRecord {
    pub owner_type: u32,
    pub owner_guid: u32,
    pub lookup_flag: bool,
    pub kind: LightKind,
    /// Position in 1/8 sub-tile (`+0x10`, `+0x14`).
    pub x: i32,
    pub y: i32,
    /// Radius × 8, current (`+0x18`) and target (`+0x1C`).
    pub radius: i32,
    pub target: i32,
    pub dying: bool,
    pub i: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Kind 2: cache valid (`+0x2C`) and the `(2m + 1)²` cache (`+0x30`).
    pub cache_valid: bool,
    pub cache: Vec<i32>,
}

impl LightRecord {
    /// The owner, `None` for owner type 6.
    pub fn owner(&self) -> Option<Owner> {
        (self.owner_type != OWNER_NONE).then_some(Owner {
            unit_type: self.owner_type,
            guid: self.owner_guid,
            client_only: self.lookup_flag,
        })
    }

    fn free_cache(&mut self) {
        self.cache_valid = false;
        self.cache = Vec::new();
    }
}

/// A record handle (the original holds pointers, e.g. unit `+0x64`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LightId(pub u32);

/// The client's light list (§6.3), head first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightList {
    records: Vec<(LightId, LightRecord)>,
    next_id: u32,
    /// The colored-light flag `[0x00712B8C]` (§7.1 r6), static 1.
    pub colored: bool,
}

impl Default for LightList {
    fn default() -> Self {
        LightList {
            records: Vec::new(),
            next_id: 0,
            colored: true,
        }
    }
}

impl LightList {
    pub fn new() -> Self {
        Self::default()
    }

    /// The records, head (newest) first.
    pub fn iter(&self) -> impl Iterator<Item = (LightId, &LightRecord)> {
        self.records.iter().map(|(id, r)| (*id, r))
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn get(&self, id: LightId) -> Option<&LightRecord> {
        self.records.iter().find(|(i, _)| *i == id).map(|(_, r)| r)
    }

    fn get_mut(&mut self, id: LightId) -> Option<&mut LightRecord> {
        self.records
            .iter_mut()
            .find(|(i, _)| *i == id)
            .map(|(_, r)| r)
    }

    /// Create (§6.2 r1): `r` < 1 → no record; `r` ≥ 18 → 18. `pos` is
    /// the light position in 1/8 sub-tile (from the unit:
    /// [`unit_light_pos`] of its precise position). Inserted at the head.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &mut self,
        owner: Option<Owner>,
        pos: (i32, i32),
        kind: LightKind,
        r: i32,
        i: u8,
        red: u8,
        green: u8,
        blue: u8,
    ) -> Option<LightId> {
        if r < 1 {
            return None;
        }
        let r8 = 8 * r.min(MAX_RADIUS);
        let (owner_type, owner_guid, lookup_flag) = match owner {
            Some(o) => (o.unit_type, o.guid, o.client_only),
            None => (OWNER_NONE, GUID_NONE, false),
        };
        let cache = if kind == LightKind::Cached {
            let side = (2 * (r8 >> 3) + 1) as usize;
            vec![0; side * side]
        } else {
            Vec::new()
        };
        let rec = LightRecord {
            owner_type,
            owner_guid,
            lookup_flag,
            kind,
            x: pos.0,
            y: pos.1,
            radius: r8,
            target: r8,
            dying: false,
            i,
            r: red,
            g: green,
            b: blue,
            cache_valid: false,
            cache,
        };
        let id = LightId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.records.insert(0, (id, rec));
        Some(id)
    }

    /// Set radius (§6.2 r2): `r` ≤ 0 → nothing; clamp 18; radius = target
    /// = `8r`; kind 2: cache invalid and freed.
    pub fn set_radius(&mut self, id: LightId, r: i32) {
        if r <= 0 {
            return;
        }
        if let Some(rec) = self.get_mut(id) {
            let r8 = 8 * r.min(MAX_RADIUS);
            rec.radius = r8;
            rec.target = r8;
            if rec.kind == LightKind::Cached {
                rec.free_cache();
            }
        }
    }

    /// Set target (§6.2 r3): `r` ≤ 0 → nothing; clamp 18; target = `8r`.
    pub fn set_target(&mut self, id: LightId, r: i32) {
        if r <= 0 {
            return;
        }
        if let Some(rec) = self.get_mut(id) {
            rec.target = 8 * r.min(MAX_RADIUS);
        }
    }

    /// Set color (§6.2 r4).
    pub fn set_color(&mut self, id: LightId, i: u8, red: u8, green: u8, blue: u8) {
        if let Some(rec) = self.get_mut(id) {
            (rec.i, rec.r, rec.g, rec.b) = (i, red, green, blue);
        }
    }

    /// Get radius (§6.2 r4): radius `>> 3`.
    pub fn radius(&self, id: LightId) -> Option<i32> {
        self.get(id).map(|r| r.radius >> 3)
    }

    /// Remove (§6.2 r5): unlink at once; fatal when absent.
    pub fn remove(&mut self, id: LightId) -> Result<LightRecord, LightError> {
        let pos = self
            .records
            .iter()
            .position(|(i, _)| *i == id)
            .ok_or(LightError::NotInList)?;
        Ok(self.records.remove(pos).1)
    }

    /// Die (§6.2 r6): dying := 1, target := 0; fatal for kind 2.
    pub fn die(&mut self, id: LightId) -> Result<(), LightError> {
        let rec = self.get_mut(id).ok_or(LightError::NotInList)?;
        if rec.kind == LightKind::Cached {
            return Err(LightError::DieOnCached);
        }
        rec.dying = true;
        rec.target = 0;
        Ok(())
    }

    /// Per drawn frame (§6.4, `0x004755A0`): every record head → tail,
    /// update (r1–r4) and contribute to `map` with quality `q` (r5).
    pub fn frame(
        &mut self,
        map: &mut LightMap,
        q: u8,
        world: &impl LightWorld,
    ) -> Result<(), LightError> {
        let colored = self.colored;
        let mut idx = 0;
        while idx < self.records.len() {
            let rec = &mut self.records[idx].1;
            // r1
            if !rec.dying {
                if let Some(owner) = rec.owner() {
                    if let Some((px, py)) = world.owner_position(&owner) {
                        rec.x = unit_light_pos(px);
                        rec.y = unit_light_pos(py);
                    }
                }
            }
            // r2
            if rec.radius != rec.target {
                if rec.kind == LightKind::Cached {
                    rec.free_cache();
                }
                rec.radius += if rec.radius < rec.target { 8 } else { -8 };
            }
            // r3
            if rec.radius > 255 {
                rec.radius = 248;
            }
            // r4
            if rec.dying && rec.radius < 1 {
                self.records.remove(idx);
                continue;
            }
            // r5
            match rec.kind {
                LightKind::Shadowed if q > 1 => contribute::shadowed(map, rec, colored),
                LightKind::Cached => contribute::cached(map, rec, world, colored)?,
                _ => {
                    let local = rec.owner().is_some_and(|o| world.is_local_player(&o));
                    contribute::plain(map, rec, q, local, colored);
                }
            }
            idx += 1;
        }
        Ok(())
    }

    /// A room leaving the client (`0x00475930`, §6.4 last paragraph):
    /// kind-2 records whose cache may cover `leaving` become invalid.
    pub fn room_leaving(
        &mut self,
        leaving: RoomId,
        world: &impl LightWorld,
    ) -> Result<(), LightError> {
        for (_, rec) in self.records.iter_mut() {
            if rec.kind != LightKind::Cached {
                continue;
            }
            let owner = rec.owner().ok_or(LightError::Fatal0x591)?;
            if world.owner_position(&owner).is_none() {
                return Err(LightError::Fatal0x591);
            }
            let room = world
                .owner_room(&owner)
                .ok_or(LightError::OwnerWithoutRoom)?;
            if room == leaving {
                continue;
            }
            let (ux, uy) = world.owner_subtile(&owner).ok_or(LightError::Fatal0x591)?;
            let m = rec.radius >> 3;
            let probes = [(ux + m, uy), (ux - m, uy), (ux, uy + m), (ux, uy - m)];
            if probes
                .iter()
                .any(|&(px, py)| world.cell_room(room, px, py) == Some(leaving))
            {
                rec.cache_valid = false;
            }
        }
        Ok(())
    }
}
