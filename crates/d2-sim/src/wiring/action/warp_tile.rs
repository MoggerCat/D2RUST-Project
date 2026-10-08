// Spec: specs/sim/path-placement.md §12.1, §12.2 (warp tiles); specs/drlg/levels.md §10.4 (preset units)
//! Warp tile units and the C→S 0x13 tile case.
//!
//! A level warp is a tile unit (unit type 5, class = the lvlwarp `Id`):
//! the DRLG's warp tile preset (`0x0066E1C0`, §12.1) adds it to the
//! room's preset units, [`View::spawn_warp_tiles`] allocates the units
//! when the room is active, and [`View::warp_tile_message`] runs the walk
//! into the warp (`0x005550B0`, §12.2) for a C→S 0x13 whose unit type is 5.
//!
//! PROVISIONAL (REC-99): no spec says where 1.14d allocates the tile
//! units from the preset list (population `0x005559A0` places only
//! type-1 presets), nor what the 0x13 handler checks before `0x005550B0`
//! (`path-placement.md` §12.2 names only the caller `0x00548C32`). d2rs
//! allocates a tile for every type-5 preset of an active room once, and
//! runs the warp when the tile exists and the player's level is the
//! tile's act; the result is 0 for a warp run, else 1.
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
        let mut n = 0;
        for p in units.iter().filter(|p| p.unit_type == TILE_PRESET) {
            let (x, y) = (origin.x + p.x, origin.y + p.y);
            let exists = game.lists.room_units(room).into_iter().any(|u| {
                game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Tile)
                    && self.units.get(u).is_some_and(|r| r.class == p.class)
                    && self.h.path_position(u) == (x, y)
            });
            if exists {
                continue;
            }
            let req = AllocRequest {
                ty: UnitType::Tile,
                class: p.class,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 0,
                allied: false,
            };
            if self.allocate(game, &req, x, y).is_some() {
                n += 1;
            }
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
