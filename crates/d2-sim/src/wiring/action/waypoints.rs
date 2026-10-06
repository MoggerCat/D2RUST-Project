// Spec: specs/world/waypoints.md §5–§7 (seam `WaypointWorld`); specs/drlg/levels.md §10
//! Waypoints ↔ DRLG levels and warps: [`WaypointView`] implements
//! [`WaypointWorld`]. Real providers: the frame and difficulty, the
//! player's records (kept per player in [`super::ActionHooks`]), objects
//! and players from the unit lists and records with the level of their
//! room (DRLG), room rectangles (active-room sub-tiles), the ENDANIM
//! timer (`tick.md` §5.2), and the spawn search `0x00619E50` →
//! `0x0066B2B0` (`levels.md` §10). The warp itself (act change, free
//! coordinates, placement), object modes, interaction, sounds and
//! messages go to [`Pending`].

use crate::drlg::act_of_level;
use crate::game::Game;
use crate::tick::events::event;
use crate::units::{RoomId, UnitId, UnitType};
use crate::world::waypoints::{ObjectFacts, PlayerFacts, RoomRect, WaypointRecords, WaypointWorld};

use super::{Pending, View, WiringError};

/// The waypoint code's view of a game.
pub struct WaypointView<'a, X> {
    pub game: &'a mut Game,
    pub v: View<'a, X>,
}

impl<X: Pending> WaypointView<'_, X> {
    fn room_and_level(&self, u: UnitId) -> (Option<RoomId>, Option<u32>) {
        let room = self.game.lists.unit(u).and_then(|e| e.room());
        let level = room.and_then(|r| self.v.h.drlg.level_id(self.game, r));
        (room, level)
    }
}

impl<X: Pending> WaypointWorld for WaypointView<'_, X> {
    fn frame(&self) -> i32 {
        self.game.frame
    }
    fn difficulty(&self) -> u8 {
        self.v.data.difficulty
    }
    /// Player data +0x1C: a player unit's records (created empty on first
    /// use; `None` for a unit that is not a player).
    fn records(&mut self, player: UnitId) -> Option<&mut WaypointRecords> {
        let is_player = self
            .game
            .lists
            .unit(player)
            .is_some_and(|e| e.ty == UnitType::Player);
        is_player.then(|| self.v.h.waypoints.entry(player).or_default())
    }
    /// `0x00552F60`.
    fn object(&self, guid: u32) -> Option<(UnitId, ObjectFacts)> {
        let u = self.game.lists.find_unit(UnitType::Object, guid)?;
        let r = self.v.units.get(u)?;
        let (room, level) = self.room_and_level(u);
        let (x, y) = self.v.h.x.position(u);
        Some((
            u,
            ObjectFacts {
                guid,
                class: r.class as u16,
                mode: r.mode as u8,
                room,
                level,
                x,
                y,
            },
        ))
    }
    fn player(&self, player: UnitId) -> PlayerFacts {
        let (room, level) = self.room_and_level(player);
        let (x, y) = self.v.h.x.position(player);
        let r = self.v.units.get(player);
        PlayerFacts {
            guid: self.game.lists.unit(player).map_or(0, |e| e.guid),
            class: r.map_or(0, |r| r.class as u8),
            room,
            level,
            x,
            y,
        }
    }
    /// Room +0x4C..+0x58: the active room's sub-tile rectangle.
    fn room_rect(&self, room: RoomId) -> RoomRect {
        let t = self.v.h.drlg.subtiles(self.game, room).unwrap_or_default();
        RoomRect {
            x: t.x,
            y: t.y,
            width: t.w,
            height: t.h,
        }
    }
    fn set_object_mode(&mut self, object: UnitId, mode: u8) {
        self.v.h.x.set_object_mode(self.game, object, mode);
    }
    /// `0x005417D0`: ENDANIM on the object at `frame`.
    fn schedule_endanim(&mut self, object: UnitId, frame: i32) {
        if let Err(e) =
            self.game
                .schedule_event(object, u32::from(event::END_ANIM), frame, None, 0, 0)
        {
            self.v.unit_error(e.into());
        }
    }
    fn player_busy(&self, player: UnitId) -> bool {
        self.v.h.x.busy(player)
    }
    fn set_interact(&mut self, player: UnitId, unit_type: u8, guid: u32) {
        self.v.h.x.set_interact(player, unit_type, guid);
    }
    fn reset_interact(&mut self, player: UnitId) {
        self.v.h.x.reset_interact(player);
    }
    fn interact_guid(&self, player: UnitId) -> Option<u32> {
        self.v.h.x.interact_guid(player)
    }
    fn hostile_delay(&self, player: UnitId) -> bool {
        self.v.h.x.hostile_delay(player)
    }
    fn attach_sound(&mut self, player: UnitId, ev: u8) {
        self.v.h.x.attach_sound(player, ev);
    }
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.v.h.x.send(player, msg);
    }
    fn warp(&mut self, player: UnitId, level: u32, tile_code: u8) {
        self.v.h.x.warp(self.game, player, level, tile_code);
    }
    /// `0x00619E50(act of level, level, tile code)`: the room the spawn
    /// search `0x0066B2B0` (`levels.md` §10) chooses, as an active room.
    ///
    /// TODO(waypoints.md §7 rule 7): `0x00619E50` is described as "the
    /// same search without the free-coordinate step"; whether it streams
    /// the chosen room like `0x0066B2B0` is not stated. The DRLG search
    /// is run as is (it streams the room). An act without a DRLG gives
    /// none.
    fn spawn_room(&mut self, level: u32, tile_code: u8) -> Option<RoomId> {
        let act = act_of_level(level);
        let r = self
            .v
            .h
            .drlg
            .with_act(act, &mut self.game.lists, |d, svc| {
                d.spawn_room(svc, level, u32::from(tile_code))
            })?;
        match r {
            Ok(p) => p.active,
            Err(e) => {
                self.v.h.errors.push(WiringError::Drlg(e));
                None
            }
        }
    }
    fn set_player_mode_arrival(&mut self, player: UnitId) {
        self.v.h.x.set_player_mode_arrival(self.game, player);
    }
}
