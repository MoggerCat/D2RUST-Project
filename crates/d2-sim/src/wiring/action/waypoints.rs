// Spec: specs/world/waypoints.md §5–§7 (seam `WaypointWorld`); specs/drlg/levels.md §10
//! Waypoints ↔ DRLG levels and warps: [`WaypointView`] implements
//! [`WaypointWorld`]. Real providers: the frame and difficulty, the
//! player's records (kept per player in [`super::ActionHooks`]), objects
//! and players from the unit lists and records with the level of their
//! room (DRLG), room rectangles (active-room sub-tiles), the ENDANIM
//! timer (`tick.md` §5.2), and the spawn search `0x00619E50` →
//! `0x0066B2B0` (`levels.md` §10); with the path provider
//! ([`crate::wiring::path`]) the same-act warp (spawn point, free
//! coordinates, placement) and the arrival mode request; object modes on
//! the object state ([`super::objects`]). The act change, the modes of
//! objects without object data, sounds and messages go to [`Pending`];
//! the player's interact info is the unit record's
//! ([`crate::units::record::InteractInfo`]).

use crate::drlg::act_of_level;
use crate::game::Game;
use crate::tick::events::event;
use crate::units::{RoomId, UnitId, UnitType};
use crate::world::waypoints::{ObjectFacts, PlayerFacts, RoomRect, WaypointRecords, WaypointWorld};

use super::{Pending, View, WiringError};

/// `0x0053AEC0(game, player, level, tile_code)` (`waypoints.md` §7 rule
/// 5): with the path provider, the same-act warp is the spawn point and
/// placement of `path-placement.md` §11
/// ([`crate::wiring::path::place::level_warp`]), an act change is
/// [`crate::wiring::path::act_change::run`]; a warp without the provider
/// (or without the destination act) goes to [`Pending::warp`]. The one
/// level warp of the action wiring (waypoints, quests).
pub fn level_warp<X: Pending>(
    v: &mut View<'_, X>,
    game: &mut Game,
    player: UnitId,
    level: u32,
    tile_code: u8,
) {
    if v.h.paths.is_some() {
        let c = crate::wiring::path::PathCtx::of(v, game);
        if crate::wiring::path::place::level_warp(c, player, level, u32::from(tile_code)).is_some()
        {
            return;
        }
        // Another act: the act change `0x0053ACC0` (`waypoints.md` §11,
        // `wiring::path::act_change`).
        let c = crate::wiring::path::PathCtx::of(v, game);
        if crate::wiring::path::act_change::run(c, player, level, u32::from(tile_code)) {
            return;
        }
    }
    v.h.x.warp(game, player, level, tile_code);
}

/// The waypoint code's view of a game.
pub struct WaypointView<'a, X> {
    pub game: &'a mut Game,
    pub v: View<'a, X>,
}

impl<X: Pending> WaypointView<'_, X> {
    /// The room of a unit `0x00620BB0` (`sim/path-placement.md` §2.1:
    /// the path's room) with the path provider, as the position; without
    /// it, the unit list's room.
    fn room_and_level(&self, u: UnitId) -> (Option<RoomId>, Option<u32>) {
        let room = match &self.v.h.paths {
            Some(p) => p.record(u).and_then(|r| r.room()),
            None => self.game.lists.unit(u).and_then(|e| e.room()),
        };
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
        let (x, y) = self.v.h.path_position(u);
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
        let (x, y) = self.v.h.path_position(player);
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
    /// `0x00624690` on the object state (`objects.md` §4) for an object
    /// with object data; otherwise [`Pending::set_object_mode`].
    fn set_object_mode(&mut self, object: UnitId, mode: u8) {
        if self.v.object_set_mode(self.game, object, mode) {
            return;
        }
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
    /// `0x00535060`: the interact info on the player's unit record
    /// (active), then [`Pending::busy`] (cursor, player data +0x4C).
    fn player_busy(&self, player: UnitId) -> bool {
        self.v.units.get(player).is_some_and(|r| r.interact.active) || self.v.h.x.busy(player)
    }
    /// `0x00554120` on the player's unit record (ignored while active).
    fn set_interact(&mut self, player: UnitId, unit_type: u8, guid: u32) {
        if let Some(r) = self.v.units.get_mut(player) {
            r.interact.set(unit_type, guid);
        }
    }
    /// `0x00554190` on the player's unit record.
    fn reset_interact(&mut self, player: UnitId) {
        if let Some(r) = self.v.units.get_mut(player) {
            r.interact.reset();
        }
    }
    /// `0x00554D00`: the player's interact unit (`world/objects-2.md`
    /// §16.3: P +0x6C ≠ 0 → the unit with GUID P +0x64, else none), as
    /// its GUID; every caller compares it with an existing unit's GUID
    /// (`world/waypoints.md` §6.3 rule 1).
    fn interact_guid(&self, player: UnitId) -> Option<u32> {
        let (_, guid) = self.v.units.get(player)?.interact.get()?;
        Some(guid)
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
    /// `0x0053AEC0` (rule 5): with the path provider, the same-act warp
    /// is the spawn point and placement of `path-placement.md` §11
    /// ([`crate::wiring::path::place::level_warp`]), an act change is
    /// [`crate::wiring::path::act_change::run`]; a warp without the
    /// provider (or without the destination act) goes to [`Pending::warp`].
    fn warp(&mut self, player: UnitId, level: u32, tile_code: u8) {
        level_warp(&mut self.v, self.game, player, level, tile_code);
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
    /// `0x005809D0(game, player, no skill, 2, x, y, 0)` at the player's
    /// own position (rule 7; `pathing.md` §1.2) with the path provider.
    fn set_player_mode_arrival(&mut self, player: UnitId) {
        if self.v.h.paths.is_some() {
            let (x, y) = self.v.h.path_position(player);
            let mut c = crate::wiring::path::PathCtx::of(&mut self.v, self.game);
            c.walk_to(player, 2, x, y);
            return;
        }
        self.v.h.x.set_player_mode_arrival(self.game, player);
    }
}
