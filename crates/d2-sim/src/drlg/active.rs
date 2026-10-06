// Spec: specs/drlg/rooms.md
//! Active rooms (`rooms.md` §5), adjacency arrays (§6), room clients and
//! the inactivity counter (§7), deactivation (§8), and the server room
//! change of a client (§4.1 with `levels.md` §9.1).
//!
//! The act room list itself is `crate::units::UnitLists` (`unit-order.md`
//! §4), reached through [`ActRooms`]; this module implements that seam
//! for `UnitLists`.

use crate::rng::Seed;
use crate::units::{ClientId, RoomId, UnitLists};

use super::collision::CollisionGrid;
use super::level::Drlg;
use super::seams::{ActRooms, Services};
use super::{room_flags, DrlgError, DrlgRoomId, TileRect, SUBTILES};

/// Removal threshold (§7.3): removed when the counter is > 10.
pub const REMOVAL_THRESHOLD: u32 = 10;

/// Active-room flags (+0x34).
pub mod active_flags {
    pub const POPULATED: u32 = 1;
    pub const UNITS_ACTIVE: u32 = 2;
    pub const NO_UPDATE: u32 = 4;
}

/// An active room (`rooms.md` §1, 0x80 bytes): the DRLG side. The act
/// list record is [`ActiveRoom::id`] in the unit lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveRoom {
    /// The act room list record.
    pub id: RoomId,
    /// Active-room seed (+0x6C).
    pub seed: Seed,
    /// Flags at creation (+0x34).
    pub flags: u32,
    /// Sub-tile rect (+0x4C).
    pub subtiles: TileRect,
    /// Adjacency array (+0x00 / +0x24): DRLG rooms with active rooms.
    pub adjacency: Vec<DrlgRoomId>,
    /// Client array (+0x48 / +0x78), sorted (§7.1).
    pub clients: Vec<ClientId>,
    /// Inactivity counter (+0x0C).
    pub inactivity: u32,
    /// Collision grid (§10).
    pub collision: CollisionGrid,
}

impl ActRooms for UnitLists {
    fn create_active_room(&mut self, act: u8, flags: u32) -> RoomId {
        self.ensure_act(act).expect("act index 0..4");
        let id = self.create_room(act).expect("act exists");
        let r = self.room_mut(id).expect("just created");
        r.populated = flags & active_flags::POPULATED != 0;
        r.units_active = flags & active_flags::UNITS_ACTIVE != 0;
        r.no_update = flags & active_flags::NO_UPDATE != 0;
        self.activate_room(id).expect("room exists");
        id
    }

    fn set_adjacent(&mut self, room: RoomId, adjacent: &[RoomId]) {
        if let Some(r) = self.room_mut(room) {
            r.adjacent = adjacent.to_vec();
        }
    }

    fn remove_active_room(&mut self, room: RoomId) -> u32 {
        let Some(r) = self.room(room) else { return 0 };
        let flags =
            u32::from(r.populated) | u32::from(r.units_active) << 1 | u32::from(r.no_update) << 2;
        // TODO(rooms.md §8.2): remaining units get flag 0x800000 and a path
        // update (unit specs); they keep their room link here.
        let _ = self.free_room(room);
        flags
    }
}

impl Drlg {
    /// The DRLG room of an active room record.
    pub fn drlg_room_of(&self, room: RoomId) -> Option<DrlgRoomId> {
        self.active_index.get(&room).copied()
    }

    /// The active room of a DRLG room.
    pub fn active_room(&self, id: DrlgRoomId) -> Option<&ActiveRoom> {
        self.room(id).active.as_ref()
    }

    fn active_mut(&mut self, id: DrlgRoomId) -> Option<&mut ActiveRoom> {
        self.room_mut(id).active.as_mut()
    }

    /// Active room creation `0x006422A0` / `0x00619890` (§5). Nothing if
    /// the tile grid has no floor and no wall records.
    pub(super) fn create_active_room(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
    ) -> Result<(), DrlgError> {
        let has_tiles = self
            .room(id)
            .tiles
            .as_ref()
            .is_some_and(|t| !t.floors.is_empty() || !t.walls.is_empty());
        if !has_tiles {
            return Ok(());
        }
        let r = self.room(id);
        let flags = if r.flags & room_flags::AUTOMAP_REVEAL != 0 {
            active_flags::NO_UPDATE
        } else if r.other_flags & 1 != 0 {
            active_flags::POPULATED
        } else {
            0
        };
        let rect = r.rect;
        let seed = self.room_mut(id).seed.derive();
        let list_id = svc.rooms.create_active_room(self.act, flags);
        let sub = TileRect::new(
            rect.x * SUBTILES,
            rect.y * SUBTILES,
            rect.w * SUBTILES,
            rect.h * SUBTILES,
        );
        self.room_mut(id).active = Some(ActiveRoom {
            id: list_id,
            seed,
            flags,
            subtiles: sub,
            adjacency: Vec::new(),
            clients: Vec::new(),
            inactivity: 0,
            collision: CollisionGrid::new(sub),
        });
        self.active_index.insert(list_id, id);
        // §5.6: fill the new array, then refill every other entry's.
        self.fill_adjacency(svc, id);
        let adj = self
            .room(id)
            .active
            .as_ref()
            .expect("set")
            .adjacency
            .clone();
        for n in adj {
            if n != id {
                self.fill_adjacency(svc, n);
            }
        }
        // §5.7.
        self.build_collision(id);
        Ok(())
    }

    /// Fill `0x0066BD00` (§6.1): the rooms-near order restricted to
    /// rooms with active rooms.
    fn fill_adjacency(&mut self, svc: &mut Services<'_>, id: DrlgRoomId) {
        let near = self.room(id).near.clone().unwrap_or_default();
        let adj: Vec<DrlgRoomId> = near
            .into_iter()
            .filter(|&n| self.room(n).active.is_some())
            .collect();
        self.set_adjacency(svc, id, adj);
    }

    fn set_adjacency(&mut self, svc: &mut Services<'_>, id: DrlgRoomId, adj: Vec<DrlgRoomId>) {
        let ids: Vec<RoomId> = adj
            .iter()
            .map(|&n| self.room(n).active.as_ref().expect("active").id)
            .collect();
        let a = self.active_mut(id).expect("active room");
        a.adjacency = adj;
        let list_id = a.id;
        svc.rooms.set_adjacent(list_id, &ids);
    }

    /// The adjacency array of an active room as act-list ids.
    pub fn adjacent_rooms(&self, id: DrlgRoomId) -> Vec<RoomId> {
        self.active_room(id).map_or_else(Vec::new, |a| {
            a.adjacency
                .iter()
                .map(|&n| self.room(n).active.as_ref().expect("active").id)
                .collect()
        })
    }

    // ---- clients and inactivity (§7) -------------------------------------

    /// Add a client `0x0061A660`: append, then sort ascending.
    ///
    /// TODO(rooms.md OQ 4): the original sorts by client record address;
    /// the client slot is the stand-in.
    pub fn add_room_client(&mut self, id: DrlgRoomId, client: ClientId) {
        if let Some(a) = self.active_mut(id) {
            a.clients.push(client);
            a.clients.sort();
        }
    }

    /// Remove a client `0x0061A700`: last into the hole, sort again.
    pub fn remove_room_client(&mut self, id: DrlgRoomId, client: ClientId) {
        if let Some(a) = self.active_mut(id) {
            if let Some(i) = a.clients.iter().position(|&c| c == client) {
                a.clients.swap_remove(i);
                a.clients.sort();
            }
        }
    }

    /// Inactivity counter `0x0061A790` (§7.2): 0 with clients, else +1;
    /// the new value. Tick step 9's `room_inactivity` hook.
    pub fn room_inactivity(&mut self, room: RoomId) -> Result<u32, DrlgError> {
        let id = self.drlg_room_of(room).ok_or(DrlgError::NotActive)?;
        let a = self.active_mut(id).ok_or(DrlgError::NotActive)?;
        a.inactivity = if a.clients.is_empty() {
            a.inactivity.wrapping_add(1)
        } else {
            0
        };
        Ok(a.inactivity)
    }

    /// Removal test `0x0061A3F0` → `0x0061BA30` (§8.1). Tick step 9's
    /// `act_allows_room_removal` hook.
    pub fn allows_removal(&self, room: RoomId) -> Result<bool, DrlgError> {
        if self.on_client {
            return Err(DrlgError::ClientCopyRemoval);
        }
        let id = self.drlg_room_of(room).ok_or(DrlgError::NotActive)?;
        let r = self.room(id);
        if r.flags & room_flags::PORTAL != 0 {
            return Ok(false);
        }
        // TODO(rooms.md OQ 2): `0x0066C0B0` is not specified; treated as
        // passing.
        if r.status <= 1 {
            return Ok(false);
        }
        let l = r.level;
        if Self::no_removal_level(self.level(l).id)
            && self
                .level_rooms(l)
                .into_iter()
                .any(|x| self.room(x).status <= 1)
        {
            return Ok(false);
        }
        Ok(true)
    }

    /// Room removal after tick step 9 compressed its units (§8.2,
    /// `0x0061A910`, `0x0066B4C0`, `0x0061A840`): neighbours' arrays
    /// (§6.3), act list unlink and free, other flags, tiles freed.
    pub fn remove_active_room(
        &mut self,
        svc: &mut Services<'_>,
        room: RoomId,
    ) -> Result<(), DrlgError> {
        let id = self.drlg_room_of(room).ok_or(DrlgError::NotActive)?;
        let adj = self
            .room(id)
            .active
            .as_ref()
            .expect("indexed")
            .adjacency
            .clone();
        for n in adj {
            if n == id {
                continue;
            }
            let Some(mut arr) = self.active_room(n).map(|a| a.adjacency.clone()) else {
                continue;
            };
            if let Some(p) = arr.iter().position(|&x| x == id) {
                arr.swap_remove(p);
                self.set_adjacency(svc, n, arr);
            }
        }
        let flags = svc.rooms.remove_active_room(room);
        self.active_index.remove(&room);
        let r = self.room_mut(id);
        r.active = None;
        r.other_flags = flags & active_flags::POPULATED;
        if r.flags & room_flags::HAS_ROOM != 0 {
            self.free_room_tiles(svc, id);
        }
        Ok(())
    }

    // ---- client room change (§4.1) ------------------------------------

    /// The server room change of a client (`0x00537B50`; leave
    /// `0x00539DA0` with `new` = none): DRLG statuses (§4.1), level
    /// activity (`levels.md` §9.1), then the client joins every room of
    /// the new adjacency array missing from the old and leaves every room
    /// of the old array missing from the new.
    pub fn client_changes_room(
        &mut self,
        svc: &mut Services<'_>,
        client: ClientId,
        old: Option<DrlgRoomId>,
        new: Option<DrlgRoomId>,
    ) -> Result<(), DrlgError> {
        self.change_status_room(svc, old, new)?;
        self.update_level_activity(svc.data, svc.types, old, new)?;
        let adj = |d: &Self, r: Option<DrlgRoomId>| {
            r.and_then(|r| d.try_room(r))
                .and_then(|r| r.active.as_ref())
                .map(|a| a.adjacency.clone())
                .unwrap_or_default()
        };
        let (old_adj, new_adj) = (adj(self, old), adj(self, new));
        for &r in &new_adj {
            if !old_adj.contains(&r) {
                self.add_room_client(r, client);
            }
        }
        for &r in &old_adj {
            if !new_adj.contains(&r) {
                self.remove_room_client(r, client);
            }
        }
        Ok(())
    }

    /// The active rooms of this DRLG in act-list id order (lookup only).
    pub fn active_rooms(&self) -> Vec<(RoomId, DrlgRoomId)> {
        self.active_index.iter().map(|(&a, &b)| (a, b)).collect()
    }
}
