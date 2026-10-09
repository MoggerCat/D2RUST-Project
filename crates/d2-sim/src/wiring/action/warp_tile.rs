// Spec: specs/sim/path-placement.md §12.1, §12.2 (warp tiles); specs/drlg/levels.md §10.4 (preset units)
//! Warp tile units and the C→S 0x13 tile case.
//!
//! A level warp is a tile unit (unit type 5, class = the lvlwarp `Id`):
//! the DRLG's warp tile preset (`0x0066E1C0`, §12.1) adds it to the
//! room's preset units, [`View::spawn_warp_tiles`] allocates the units
//! when the room is active, and [`View::warp_tile_message`] runs the walk
//! into the warp (`0x005550B0`, §12.2) for a C→S 0x13 whose unit type is 5.
//!
//! The tile units are allocated by the first walk of `0x005559A0` with
//! the room's other non-monster presets, in list order
//! ([`View::spawn_preset_units`], `drlg/rooms.md` §6 "First spawn").
//! PROVISIONAL (REC-99): no spec says what the 0x13 handler checks before
//! `0x005550B0` (`path-placement.md` §12.2 names only the caller
//! `0x00548C32`). d2rs runs the warp when the tile exists and the
//! player's level is the tile's act; the result is 0 for a warp run,
//! else 1.
// d2rs-own, unverified

use crate::game::Game;
use crate::units::lifecycle::AllocRequest;
use crate::units::{RoomId, UnitId, UnitType};
use crate::wiring::path::{place, PathCtx};

use super::{Pending, View};

/// The unit type of a host-placed monster preset: a level types provider's
/// own preset-list entry (class = monstats class) that population does not
/// read (it takes type 1). PROVISIONAL (REC-124); d2rs-own, unverified.
pub const HOST_MONSTER_PRESET: u32 = 6;
/// Preset unit type for an object the host's level types provider lists
/// (q-a2-duriel, REC-167; d2rs-own, unverified): created through the
/// object state's `create_object` (allocation and the §3 init).
pub const HOST_OBJECT_PRESET: u32 = 7;

/// The unit type of an object preset (DS1 object entries).
pub const OBJECT_PRESET: u32 = 2;

/// The unit type of a tile preset (`levels.md` §10.4).
const TILE_PRESET: u32 = crate::path::warp::TILE_UNIT_TYPE as u32;

impl<X: Pending> View<'_, X> {
    /// Allocates a tile unit for each type-5 preset of the active `room`'s
    /// DRLG room that has none yet (same class and position). Returns the
    /// number allocated.
    pub fn spawn_warp_tiles(&mut self, game: &mut Game, room: RoomId) -> usize {
        let Some(act) = game.lists.room(room).map(|r| r.act) else {
            return 0;
        };
        let found = self.h.drlg.with_act(act, &mut game.lists, |d, svc| {
            let r = d.drlg_room_of(room)?;
            let origin = d.active_room(r)?.subtiles;
            Some((svc.types.preset_units(d, r), origin))
        });
        let Some(Some((units, origin))) = found else {
            return 0;
        };
        units
            .iter()
            .filter(|p| p.unit_type == TILE_PRESET)
            .filter(|p| {
                self.spawn_tile_preset(game, room, (origin.x + p.x, origin.y + p.y), p.class)
            })
            .count()
    }

    /// One type-5 preset: a tile unit at (x, y) unless the room has one
    /// of that class there. True when allocated.
    fn spawn_tile_preset(
        &mut self,
        game: &mut Game,
        room: RoomId,
        (x, y): (i32, i32),
        class: u32,
    ) -> bool {
        let exists = game.lists.room_units(room).into_iter().any(|u| {
            game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Tile)
                && self.units.get(u).is_some_and(|r| r.class == class)
                && self.h.path_position(u) == (x, y)
        });
        if exists {
            return false;
        }
        let req = AllocRequest {
            ty: UnitType::Tile,
            class,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: 0,
            allied: false,
        };
        self.allocate(game, &req, x, y).is_some()
    }

    /// One type-2 preset: an object at (x, y) through the object state's
    /// `create_object`, unless the room has one of that class there. True
    /// when created.
    fn spawn_object_preset(
        &mut self,
        game: &mut Game,
        room: RoomId,
        (x, y): (i32, i32),
        class: u32,
    ) -> bool {
        let exists = game.lists.room_units(room).into_iter().any(|u| {
            game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Object)
                && self.units.get(u).is_some_and(|r| r.class == class)
                && self.h.path_position(u) == (x, y)
        });
        !exists && self.create_object(game, room, class, x, y, 0).is_some()
    }

    /// The first walk of `0x005559A0` (`drlg/rooms.md` §6 "First spawn",
    /// settles REC-99's "where"): every non-monster preset of the active
    /// `room`'s DRLG room in list order (head first; warp tiles are
    /// prepended, `path-placement.md` §12.1 rule 3, so the last-added
    /// warp comes first): type 2 an object ([`Self::spawn_object_preset`]),
    /// type 5 a tile ([`Self::spawn_tile_preset`]). Before the monster
    /// walk. Returns the number created.
    pub fn spawn_preset_units(&mut self, game: &mut Game, room: RoomId) -> usize {
        let Some(act) = game.lists.room(room).map(|r| r.act) else {
            return 0;
        };
        let found = self.h.drlg.with_act(act, &mut game.lists, |d, svc| {
            let r = d.drlg_room_of(room)?;
            let origin = d.active_room(r)?.subtiles;
            Some((svc.types.preset_units(d, r), origin))
        });
        let Some(Some((units, origin))) = found else {
            return 0;
        };
        let mut n = 0;
        for p in &units {
            let at = (origin.x + p.x, origin.y + p.y);
            let made = match p.unit_type {
                OBJECT_PRESET => self.spawn_object_preset(game, room, at, p.class),
                TILE_PRESET => self.spawn_tile_preset(game, room, at, p.class),
                _ => false,
            };
            n += usize::from(made);
        }
        n
    }

    /// Allocates a monster for each [`HOST_MONSTER_PRESET`] entry of the
    /// active `room`'s DRLG room that has none yet (same class), as the
    /// tiles are. Returns the created (unit, class) pairs.
    pub fn spawn_host_monsters(&mut self, game: &mut Game, room: RoomId) -> Vec<(UnitId, u32)> {
        let Some(act) = game.lists.room(room).map(|r| r.act) else {
            return Vec::new();
        };
        let found = self.h.drlg.with_act(act, &mut game.lists, |d, svc| {
            let r = d.drlg_room_of(room)?;
            let origin = d.active_room(r)?.subtiles;
            Some((svc.types.preset_units(d, r), origin))
        });
        let Some(Some((units, origin))) = found else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for p in units.iter().filter(|p| p.unit_type == HOST_MONSTER_PRESET) {
            let exists = game.lists.room_units(room).into_iter().any(|u| {
                game.lists
                    .unit(u)
                    .is_some_and(|e| e.ty == UnitType::Monster)
                    && self.units.get(u).is_some_and(|r| r.class == p.class)
            });
            if exists {
                continue;
            }
            let req = AllocRequest {
                ty: UnitType::Monster,
                class: p.class,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: false,
            };
            if let Some(u) = self.allocate(game, &req, origin.x + p.x, origin.y + p.y) {
                if let Some(r) = self.units.get_mut(u) {
                    r.flags |= crate::missiles::unit_flag::IS_VALID_TARGET
                        | crate::missiles::unit_flag::CAN_BE_ATTACKED;
                }
                out.push((u, p.class));
            }
        }
        out
    }

    /// Creates an object for each [`HOST_OBJECT_PRESET`] entry of the
    /// active `room`'s DRLG room that has none yet (same class).
    pub fn spawn_host_objects(&mut self, game: &mut Game, room: RoomId) {
        let Some(act) = game.lists.room(room).map(|r| r.act) else {
            return;
        };
        let found = self.h.drlg.with_act(act, &mut game.lists, |d, svc| {
            let r = d.drlg_room_of(room)?;
            let origin = d.active_room(r)?.subtiles;
            Some((svc.types.preset_units(d, r), origin))
        });
        let Some(Some((units, origin))) = found else {
            return;
        };
        for p in units.iter().filter(|p| p.unit_type == HOST_OBJECT_PRESET) {
            let exists = game.lists.room_units(room).into_iter().any(|u| {
                game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Object)
                    && self.units.get(u).is_some_and(|r| r.class == p.class)
            });
            if !exists {
                self.create_object(game, room, p.class, origin.x + p.x, origin.y + p.y, 0);
            }
        }
    }

    /// The first tile unit of `class` in the active `room`'s unit list.
    pub fn room_tile(&self, game: &Game, room: RoomId, class: u32) -> Option<UnitId> {
        game.lists.room_units(room).into_iter().find(|&u| {
            game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Tile)
                && self.units.get(u).is_some_and(|r| r.class == class)
        })
    }

    /// The C→S 0x13 tile case: the tile with `guid` and the walk into its
    /// warp for `player`. `None`: no path provider. 0: the warp ran
    /// ([`WarpOutcome::Arrived`]); 1: nothing happened.
    ///
    /// [`WarpOutcome::Arrived`]: crate::path::warp::WarpOutcome::Arrived
    pub fn warp_tile_message(&mut self, game: &mut Game, player: UnitId, guid: u32) -> Option<u32> {
        self.h.paths.as_ref()?;
        let Some(tile) = game.lists.find_unit(UnitType::Tile, guid) else {
            return Some(1);
        };
        let room = game.lists.unit(tile).and_then(|e| e.room());
        let class = self.units.get(tile).map(|r| r.class);
        let (Some(room), Some(class)) = (room, class) else {
            return Some(1);
        };
        let c = PathCtx::of(self, game);
        let r = place::warp_player(c, player, room, class);
        Some(u32::from(
            r != Some(crate::path::warp::WarpOutcome::Arrived),
        ))
    }
}
