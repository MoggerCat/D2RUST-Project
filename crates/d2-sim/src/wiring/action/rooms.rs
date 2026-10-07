// Spec: specs/drlg/rooms.md §5–§8, §10; specs/drlg/levels.md §9.1; specs/world/waypoints.md §1 rule 4
//! DRLG room activation ↔ the act room lists: the room lookups the other
//! adapters use (DRLG room of an active room, its level, the town test
//! `0x0061AB00`, the collision grids of §10, the room search
//! `0x00463740`), and the DRLG bodies of tick steps 9 and 10 and of the
//! client room change (`0x00537B50`).
//!
//! The act room list is [`crate::units::UnitLists`]: active rooms are
//! created in it by the DRLG through its [`crate::drlg::ActRooms`]
//! implementation (`drlg/active.rs`).

use crate::drlg::{is_town, Drlg, DrlgRoomId, Services, TileRect};
use crate::game::Game;
use crate::units::{ClientId, RoomId, UnitLists};

use super::{DrlgWorld, WiringError};

/// One room of a client room switch ([`DrlgWorld::client_switches_room`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwitchedRoom {
    /// The active room (act room list).
    pub room: RoomId,
    /// Its DRLG room's tile origin and level id (the 0x07 / 0x08 fields).
    pub tile_x: i32,
    pub tile_y: i32,
    pub level: u32,
    /// Clients in the room after the change (room +0x78).
    pub clients: usize,
}

/// What a client room switch joined and left.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomSwitch {
    /// New adjacency array order.
    pub joined: Vec<SwitchedRoom>,
    /// Old adjacency array order.
    pub left: Vec<SwitchedRoom>,
}

impl DrlgWorld {
    /// The act DRLG and its services over `lists` (the act room list).
    /// `None` when the act has no DRLG yet.
    pub fn with_act<R>(
        &mut self,
        act: u8,
        lists: &mut UnitLists,
        f: impl FnOnce(&mut Drlg, &mut Services<'_>) -> R,
    ) -> Option<R> {
        let drlg = self.dungeon.acts.get_mut(usize::from(act))?.as_mut()?;
        let mut svc = Services {
            data: &self.data,
            tiles: &*self.tiles,
            types: &mut *self.types,
            rooms: lists,
        };
        Some(f(drlg, &mut svc))
    }

    /// The DRLG of an active room's act and the room's DRLG room.
    pub fn drlg_room(&self, game: &Game, room: RoomId) -> Option<(&Drlg, DrlgRoomId)> {
        let act = game.lists.room(room)?.act;
        let drlg = self.dungeon.acts.get(usize::from(act))?.as_ref()?;
        Some((drlg, drlg.drlg_room_of(room)?))
    }

    /// The level id of an active room (`levels.txt` row).
    pub fn level_id(&self, game: &Game, room: RoomId) -> Option<u32> {
        let (d, r) = self.drlg_room(game, room)?;
        Some(d.level(d.room(r).level).id)
    }

    /// `0x0061AB00`: the room's level is a town (`0x006426A0`,
    /// `waypoints.md` §1 rule 4). A room without a DRLG room is not.
    pub fn in_town(&self, game: &Game, room: RoomId) -> bool {
        self.level_id(game, room).is_some_and(is_town)
    }

    /// The sub-tile rectangle of an active room (room +0x4C).
    pub fn subtiles(&self, game: &Game, room: RoomId) -> Option<TileRect> {
        let (d, r) = self.drlg_room(game, room)?;
        d.active_room(r).map(|a| a.subtiles)
    }

    /// The collision mask at sub-tile (x, y) in the act of `room`
    /// (`rooms.md` §10; each sub-tile belongs to one active room's grid).
    pub fn collision(&self, game: &Game, room: RoomId, x: i32, y: i32) -> Option<u16> {
        let act = game.lists.room(room)?.act;
        self.dungeon
            .acts
            .get(usize::from(act))?
            .as_ref()?
            .collision_at(x, y)
    }

    /// Mutable collision mask at (x, y) in the act of `room`.
    pub fn collision_mut(&mut self, game: &Game, room: RoomId, x: i32, y: i32) -> Option<&mut u16> {
        let act = game.lists.room(room)?.act;
        self.dungeon
            .acts
            .get_mut(usize::from(act))?
            .as_mut()?
            .collision_at_mut(x, y)
    }

    /// `0x00463740` (`missiles.md` §R2.3 step 3): the room containing
    /// (x, y) among `near` and its adjacency array, in that order.
    pub fn find_room(&self, game: &Game, near: RoomId, x: i32, y: i32) -> Option<RoomId> {
        let adjacent = game.lists.room(near)?.adjacent.clone();
        std::iter::once(near)
            .chain(adjacent.into_iter().filter(|&r| r != near))
            .find(|&r| self.subtiles(game, r).is_some_and(|t| t.contains(x, y)))
    }

    /// Tick step 9 `0x0061A790`: the room's inactivity counter.
    pub fn room_inactivity(&mut self, game: &Game, room: RoomId) -> Result<u32, WiringError> {
        let act = game.lists.room(room).map_or(0, |r| r.act);
        let drlg = self.act_mut(act)?;
        drlg.room_inactivity(room).map_err(WiringError::Drlg)
    }

    /// Tick step 9 `0x0061A3F0`: the act allows removing the room.
    pub fn allows_removal(&self, act: u8, room: RoomId) -> Result<bool, WiringError> {
        let drlg = self
            .dungeon
            .acts
            .get(usize::from(act))
            .and_then(Option::as_ref)
            .ok_or(WiringError::Drlg(crate::drlg::DrlgError::NotActive))?;
        drlg.allows_removal(room).map_err(WiringError::Drlg)
    }

    /// The rest of `0x0061A910` after tick step 9 unlinked the room
    /// (`rooms.md` §8.2): neighbours' arrays, record free, other flags,
    /// tiles.
    pub fn remove_active_room(
        &mut self,
        lists: &mut UnitLists,
        act: u8,
        room: RoomId,
    ) -> Result<(), WiringError> {
        self.with_act(act, lists, |d, svc| d.remove_active_room(svc, room))
            .unwrap_or(Err(crate::drlg::DrlgError::NotActive))
            .map_err(WiringError::Drlg)
    }

    /// Tick step 10 `0x0061AA20` → `0x00643200` (`levels.md` §9.2).
    pub fn free_inactive_rooms(&mut self, act: u8) -> Result<(), WiringError> {
        let Some(drlg) = self
            .dungeon
            .acts
            .get_mut(usize::from(act))
            .and_then(Option::as_mut)
        else {
            return Ok(());
        };
        drlg.free_inactive_levels(&self.data, &mut *self.types)
            .map_err(WiringError::Drlg)
    }

    /// The server room change of a client (`0x00537B50`; `rooms.md` §4.1,
    /// `levels.md` §9.1) from `old` to `new` active rooms of one act: the
    /// rooms the client joined (new adjacency order) and left (old
    /// adjacency order), each with the fields of its S→C 0x07 / 0x08 (the
    /// DRLG room's tile x, tile y and level id, builders `0x0053BC50`,
    /// `0x0053BC90`) and its client count after the change (room +0x78).
    pub fn client_switches_room(
        &mut self,
        lists: &mut UnitLists,
        act: u8,
        client: ClientId,
        old: Option<RoomId>,
        new: Option<RoomId>,
    ) -> Result<RoomSwitch, WiringError> {
        self.with_act(act, lists, |d, svc| {
            let old = old.and_then(|r| d.drlg_room_of(r));
            let new = new.and_then(|r| d.drlg_room_of(r));
            let (joined, left) = d.client_switches_room(svc, client, old, new)?;
            let side = |rooms: Vec<DrlgRoomId>| -> Vec<SwitchedRoom> {
                rooms
                    .into_iter()
                    .filter_map(|r| {
                        let room = d.room(r);
                        let a = room.active()?;
                        Some(SwitchedRoom {
                            room: a.id,
                            tile_x: room.rect.x,
                            tile_y: room.rect.y,
                            level: d.level(room.level).id,
                            clients: a.clients.len(),
                        })
                    })
                    .collect()
            };
            Ok(RoomSwitch {
                joined: side(joined),
                left: side(left),
            })
        })
        .unwrap_or(Err(crate::drlg::DrlgError::NotActive))
        .map_err(WiringError::Drlg)
    }

    /// Room ready `0x0061A460` (`tick.md` §6 rule 6) of an active room:
    /// `None` when the room has no DRLG room.
    pub fn room_ready(&self, game: &Game, room: RoomId) -> Option<bool> {
        let act = game.lists.room(room)?.act;
        let drlg = self.dungeon.acts.get(usize::from(act))?.as_ref()?;
        drlg.room_ready(room, |r| game.lists.room(r).is_some_and(|e| e.populated))
    }

    fn act_mut(&mut self, act: u8) -> Result<&mut Drlg, WiringError> {
        self.dungeon
            .acts
            .get_mut(usize::from(act))
            .and_then(Option::as_mut)
            .ok_or(WiringError::Drlg(crate::drlg::DrlgError::NotActive))
    }
}
