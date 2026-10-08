// Spec: specs/sim/path-placement.md §9–§13; specs/world/waypoints.md §7 rule 5; specs/world/hirelings.md §6 r3; specs/world/hirelings-2.md §16 r3 (wiring of the placement seams)
//! The placement seams of `path::place` / `path::warp` on the unit
//! system and the DRLG: [`CollisionView`], [`PlaceHost`] and
//! [`LevelView`] on [`Shared`], three handles to one [`PathCtx`] (the
//! placement code takes them as separate arguments and calls them one
//! after another, so each call borrows the context for its own
//! duration). Entry points: [`place_unit`] (`0x00554EA0`), [`game_entry`]
//! (`0x005394A0`), [`level_warp`]
//! (the same-act part of `0x0053AEC0`), [`warp_player`] (`0x005550B0`);
//! on [`Rooms`] (the DRLG alone): [`floor_drop`] (`0x00555DA0` /
//! `0x0064E810`) and [`coarse_free_box`] (`0x0064E840`).

use std::cell::RefCell;

use crate::drlg::{act_of_level, TileRect};
use crate::path::collision::{box_value, find_room, point_value, size_value};
use crate::path::coords::Point;
use crate::path::place_seams::{
    CollisionView, LevelView, PlaceError, PlaceHost, PlaceMessage, RoomReveal,
};
use crate::path::search::ExpField;
use crate::path::warp::WarpOutcome;
use crate::path::CollisionRooms;
use crate::units::{RoomId, UnitId, UnitType};
use crate::wiring::action::{DrlgWorld, HirelingCall, Pending, WiringError};

use super::walk::PathCtx;

/// One handle to a shared [`PathCtx`].
pub struct Shared<'c, 'a, X>(pub &'c RefCell<PathCtx<'a, X>>);

/// The DRLG rooms alone, for the free-point searches (§7, §8, §9), which
/// call only the §4 queries: its unit methods answer "no path" and its
/// moves do nothing (no search calls them).
pub struct Rooms<'a>(pub &'a DrlgWorld);

impl CollisionView for Rooms<'_> {
    type Room = RoomId;
    type Unit = UnitId;

    fn cell_room(&self, hint: Option<RoomId>, x: i32, y: i32) -> Option<RoomId> {
        find_room(self.0, hint, x, y)
    }
    fn room_rect(&self, room: RoomId) -> TileRect {
        self.0
            .subtile_rect(room)
            .unwrap_or(TileRect::new(0, 0, 0, 0))
    }
    /// §4 rule 2 unmasked (0x27 without a room or grid).
    fn cell_value(&self, room: RoomId, x: i32, y: i32) -> u32 {
        u32::from(point_value(self.0, Some(room), x, y, 0xFFFF))
    }
    fn point_query(&self, room: RoomId, x: i32, y: i32, mask: u32) -> u32 {
        u32::from(point_value(self.0, Some(room), x, y, mask as u16))
    }
    fn size_query(&self, room: RoomId, x: i32, y: i32, size: i32, mask: u32) -> u32 {
        u32::from(size_value(self.0, Some(room), x, y, size, mask as u16))
    }
    fn box_query(&self, room: RoomId, x: i32, y: i32, sx: u32, sy: u32, mask: u32) -> u32 {
        u32::from(box_value(self.0, Some(room), x, y, (sx, sy), mask as u16))
    }
    fn has_path(&self, _: UnitId) -> bool {
        false
    }
    fn unit_room(&self, _: UnitId) -> Option<RoomId> {
        None
    }
    fn unit_size(&self, _: UnitId) -> i32 {
        0
    }
    fn teleport(&mut self, _: UnitId, _: RoomId, _: i32, _: i32) {}
    fn add_player_to_world(&mut self, _: UnitId, _: RoomId, _: i32, _: i32) {}
}

impl<X: Pending> CollisionView for Shared<'_, '_, X> {
    type Room = RoomId;
    type Unit = UnitId;

    fn cell_room(&self, hint: Option<RoomId>, x: i32, y: i32) -> Option<RoomId> {
        Rooms(&self.0.borrow().v.h.drlg).cell_room(hint, x, y)
    }
    fn room_rect(&self, room: RoomId) -> TileRect {
        Rooms(&self.0.borrow().v.h.drlg).room_rect(room)
    }
    fn cell_value(&self, room: RoomId, x: i32, y: i32) -> u32 {
        Rooms(&self.0.borrow().v.h.drlg).cell_value(room, x, y)
    }
    fn point_query(&self, room: RoomId, x: i32, y: i32, mask: u32) -> u32 {
        Rooms(&self.0.borrow().v.h.drlg).point_query(room, x, y, mask)
    }
    fn size_query(&self, room: RoomId, x: i32, y: i32, size: i32, mask: u32) -> u32 {
        Rooms(&self.0.borrow().v.h.drlg).size_query(room, x, y, size, mask)
    }
    fn box_query(&self, room: RoomId, x: i32, y: i32, sx: u32, sy: u32, mask: u32) -> u32 {
        Rooms(&self.0.borrow().v.h.drlg).box_query(room, x, y, sx, sy, mask)
    }
    fn has_path(&self, unit: UnitId) -> bool {
        self.0.borrow().v.h.path_has(unit)
    }
    fn unit_room(&self, unit: UnitId) -> Option<RoomId> {
        let c = self.0.borrow();
        c.v.h.paths.as_ref()?.record(unit)?.room()
    }
    fn unit_size(&self, unit: UnitId) -> i32 {
        self.0.borrow().v.path_size(unit)
    }
    fn teleport(&mut self, unit: UnitId, room: RoomId, x: i32, y: i32) {
        self.0.borrow_mut().teleport(unit, Some(room), x, y);
    }
    /// §2.5 for the game-entry player, `0x00554850(flag 0)`: the
    /// player's path is allocated at the point (§2.4), the unit moved to
    /// the room's list, and the room-changed flag (path flag 0x2) set
    /// (§11).
    fn add_player_to_world(&mut self, unit: UnitId, room: RoomId, x: i32, y: i32) {
        let mut c = self.0.borrow_mut();
        let c = &mut *c;
        if let Err(e) = c.game.lists.change_room(unit, room) {
            c.v.h
                .errors
                .push(WiringError::Unit(crate::units::modes::UnitError::Game(
                    e.into(),
                )));
            return;
        }
        let game: &crate::game::Game = c.game;
        c.v.path_place(game, unit, x, y);
        if let Some(d) = c.v.h.paths.as_mut().and_then(|p| p.dynamic_mut(unit)) {
            d.flags |= crate::path::record::flags::ROOM_CHANGED;
        }
    }
    /// Game entry's `0x005381F0` → `0x00537B50` for the client whose
    /// player is `player` ([`crate::wiring::action::View::room_switch`]);
    /// a player without a client: nothing.
    fn client_room_switch(&mut self, player: UnitId, room: RoomId) {
        let mut c = self.0.borrow_mut();
        let c = &mut *c;
        let client = c
            .game
            .lists
            .clients()
            .into_iter()
            .find(|&k| c.game.lists.client(k).and_then(|e| e.player) == Some(player));
        if let Some(client) = client {
            c.v.room_switch(c.game, client, Some(room));
        }
    }
}

/// S→C 0x07 MapReveal (`sim/server-messages.tsv`: x u16 @1, y u16 @3,
/// level u8 @5; builder `0x0053BC50`).
pub fn map_reveal(x: u16, y: u16, level: u8) -> [u8; 6] {
    let [x0, x1] = x.to_le_bytes();
    let [y0, y1] = y.to_le_bytes();
    [0x07, x0, x1, y0, y1, level]
}

impl<X: Pending> PlaceHost<UnitId> for Shared<'_, '_, X> {
    /// `0x005754B0`: queued for the host holding the pet lists
    /// ([`super::super::action::ActionHooks::pet_follows`]); none → nothing.
    fn pets_follow(&mut self, player: UnitId) {
        if let Some(q) = self.0.borrow_mut().v.h.pet_follows.as_mut() {
            q.push(player);
        }
    }
    fn is_player(&self, unit: UnitId) -> bool {
        self.0
            .borrow()
            .v
            .units
            .get(unit)
            .is_some_and(|r| r.ty == UnitType::Player)
    }
    fn queue_update(&mut self, unit: UnitId) {
        let mut c = self.0.borrow_mut();
        if let Err(e) = c.game.lists.queue_update(unit) {
            c.v.h
                .errors
                .push(WiringError::Unit(crate::units::modes::UnitError::Game(
                    e.into(),
                )));
        }
    }
    fn or_flags2(&mut self, unit: UnitId, bits: u32) {
        if let Some(r) = self.0.borrow_mut().v.units.get_mut(unit) {
            r.flags2 |= bits;
        }
    }
    fn room_change_messages(&mut self, unit: UnitId) {
        self.0.borrow_mut().room_change_messages(unit);
    }
    /// The message bytes go to [`Pending::send`] (the server transport):
    /// 0x07 by its layout, 0x15 / 0x0D by `path::walk::messages`, with
    /// the unit's type and GUID.
    fn send(&mut self, player: UnitId, msg: PlaceMessage<UnitId>) {
        let mut c = self.0.borrow_mut();
        let id = |c: &PathCtx<'_, X>, u: UnitId| {
            c.v.units.get(u).map_or((0, 0), |r| (r.ty as u8, r.guid))
        };
        let bytes: Vec<u8> = match msg {
            PlaceMessage::MapReveal { x, y, level } => map_reveal(x, y, level).to_vec(),
            PlaceMessage::ReassignPlayer { unit, x, y, flag } => {
                let (t, g) = id(&c, unit);
                crate::path::walk::messages::reassign_player(t, g, x, y, flag).to_vec()
            }
            PlaceMessage::PlayerStop {
                unit,
                a,
                x,
                y,
                b,
                life_pct,
            } => {
                let (t, g) = id(&c, unit);
                crate::path::walk::messages::player_stop(t, g, a, x, y, b, life_pct).to_vec()
            }
            PlaceMessage::GameEntryDone => crate::units::messages::GAME_ENTRY_DONE.to_vec(),
        };
        c.v.h.x.send(player, &bytes);
    }
    /// Timer event `event` at frame + `delay`; event 14 with its callback
    /// `0x00554570` (`units.md` §6).
    fn schedule_event(&mut self, unit: UnitId, event: u8, delay: u32) {
        let mut c = self.0.borrow_mut();
        let c = &mut *c;
        let callback = (event == crate::path::place_seams::PLACE_TIMER_EVENT)
            .then_some(crate::units::dispatch::SKILL_COOLDOWN);
        let expire = c.game.frame.wrapping_add(delay as i32);
        if let Err(e) = c
            .game
            .schedule_event(unit, u32::from(event), expire, callback, 0, 0)
        {
            c.v.h
                .errors
                .push(WiringError::Unit(crate::units::modes::UnitError::Game(e)));
        }
    }
    /// `0x00554FD0` (§10 rule 7) into [`super::PathState::history`].
    fn history_write(&mut self, player: UnitId, x: i32, y: i32) {
        if let Some(p) = self.0.borrow_mut().v.h.paths.as_mut() {
            p.history.entry(player).or_default().place_write(x, y);
        }
    }
    fn request_walk(&mut self, player: UnitId, x: i32, y: i32) {
        self.0.borrow_mut().walk_to(player, 2, x, y);
    }
}

impl<X: Pending> LevelView<RoomId> for Shared<'_, '_, X> {
    type Act = u8;

    /// `0x0066B2B0` (`drlg/levels.md` §10) on the act's DRLG: streams the
    /// chosen room; its active room and the position in tiles.
    fn spawn_room(&mut self, act: u8, level: u32, tile_index: u32) -> Option<(RoomId, i32, i32)> {
        let mut c = self.0.borrow_mut();
        let c = &mut *c;
        let r = c.v.h.drlg.with_act(act, &mut c.game.lists, |d, svc| {
            d.spawn_room(svc, level, tile_index)
        })?;
        match r {
            Ok(p) => Some((p.active?, p.x, p.y)),
            Err(e) => {
                c.v.h.errors.push(WiringError::Drlg(e));
                None
            }
        }
    }
    /// `0x006195A0` ([`super::warp_dest::destination`]).
    fn warp_destination(
        &self,
        tile_room: RoomId,
        tile_class: u32,
    ) -> Option<crate::path::place_seams::WarpDestination<RoomId>> {
        super::warp_dest::destination(&mut self.0.borrow_mut(), tile_room, tile_class)
    }
    /// Act +0x08, the act's town level id (`drlg/levels.md` §1 table).
    fn act_start_level(&self, act: u8) -> u32 {
        crate::drlg::TOWN_LEVELS
            .iter()
            .copied()
            .find(|&l| act_of_level(l) == act)
            .unwrap_or(0)
    }
    /// The room's DRLG room tile origin and level id.
    fn room_reveal(&self, room: RoomId) -> RoomReveal {
        let c = self.0.borrow();
        match c.v.h.drlg.drlg_room(c.game, room) {
            Some((d, r)) => {
                let dr = d.room(r);
                RoomReveal {
                    tile_x: dr.rect.x,
                    tile_y: dr.rect.y,
                    level: d.level(dr.level).id,
                }
            }
            None => RoomReveal {
                tile_x: 0,
                tile_y: 0,
                level: 0,
            },
        }
    }
}

/// Runs `f` on three handles of one context.
fn with_shared<X: Pending, R>(
    c: PathCtx<'_, X>,
    f: impl FnOnce(&mut Shared<'_, '_, X>, &mut Shared<'_, '_, X>, &mut Shared<'_, '_, X>) -> R,
) -> R {
    let cell = RefCell::new(c);
    let (mut a, mut b, mut l) = (Shared(&cell), Shared(&cell), Shared(&cell));
    f(&mut a, &mut b, &mut l)
}

/// `0x00554EA0(game, unit, room, x, y, exact, alt)` (§10): `true` placed.
/// Fatal asserts are logged as [`WiringError::Place`].
#[allow(clippy::too_many_arguments)]
pub fn place_unit<X: Pending>(
    c: PathCtx<'_, X>,
    unit: UnitId,
    room: Option<RoomId>,
    x: i32,
    y: i32,
    exact: bool,
    alt: bool,
) -> bool {
    with_shared(c, |cv, host, lv| {
        let r = crate::path::place::place_unit(cv, host, &*lv, unit, room, x, y, exact, alt);
        log(cv, r).unwrap_or(false)
    })
}

/// Game entry `0x005394A0` of a player not yet placed (§11, §13): the
/// spawn point of act `act`'s town (tile index 0, the unit's size), S→C
/// 0x07 for the spawn room, the client's room switch (S→C 0x07 and the
/// add messages of every room of the spawn room's adjacency array,
/// `sim/intents-events.md` §7.8), `0x00554850(flag 0)`, S→C 0x15 with
/// flag 1, S→C 0x7E, all to the player's client ([`Pending::send`]). `true` placed; a fatal
/// assert (no spawn room, no free point, no act) is logged as
/// [`WiringError::Place`] and gives `false`.
///
/// A placed player's join follow `0x005773D0` (`hirelings-2.md` §16
/// rule 3, called at `0x005396C3`) is queued for the host that holds the
/// hireling lists ([`super::super::action::ActionHooks::hireling_calls`];
/// none → nothing).
pub fn game_entry<X: Pending>(c: PathCtx<'_, X>, player: UnitId, act: u8) -> bool {
    with_shared(c, |cv, host, lv| {
        let r = crate::path::place::game_entry(cv, host, lv, player, act);
        let placed = log(cv, r).unwrap_or(false);
        if placed {
            if let Some(q) = cv.0.borrow_mut().v.h.hireling_calls.as_mut() {
                q.push(HirelingCall::JoinFollow(player));
            }
        }
        placed
    })
}

/// The same-act level warp of `0x0053AEC0` (§11, `waypoints.md` §7 rule
/// 5): spawn point (`0x0061B060`) then `0x00554EA0(exact 0, alt 0)`.
/// `None`: the destination is in another act (act change, owner: the
/// act/level-change spec; the caller keeps its `Pending` route).
pub fn level_warp<X: Pending>(
    c: PathCtx<'_, X>,
    player: UnitId,
    level: u32,
    tile_index: u32,
) -> Option<bool> {
    let act = act_of_level(level);
    let own = c
        .game
        .lists
        .unit(player)
        .and_then(|e| e.room())
        .and_then(|r| c.game.lists.room(r))
        .map(|r| r.act);
    if own.is_some_and(|a| a != act) {
        // The act change `0x0053ACC0`: its hireling part (`hirelings.md`
        // §6 rules 3–4) is queued for the host that holds the hireling
        // lists ([`super::super::action::ActionHooks::hireling_calls`]).
        if let Some(q) = c.v.h.hireling_calls.as_mut() {
            q.push(HirelingCall::ActChange(player));
        }
        return None;
    }
    Some(with_shared(c, |cv, host, lv| {
        let r = crate::path::place::level_warp_place(cv, host, lv, player, act, level, tile_index);
        log(cv, r).unwrap_or(false)
    }))
}

/// `0x005550B0(game, player, tile)` (§12.2).
pub fn warp_player<X: Pending>(
    c: PathCtx<'_, X>,
    player: UnitId,
    tile_room: RoomId,
    tile_class: u32,
) -> Option<WarpOutcome> {
    with_shared(c, |cv, host, lv| {
        let r = crate::path::warp::warp_player(cv, host, &*lv, player, tile_room, tile_class);
        log(cv, r)
    })
}

/// Floor drop `0x0064E810(room, &start, origin = from, size, 0x3E01,
/// 0x801, fallback)` through `0x00555DA0` (§9) on `field`: the room
/// (`None`: no place) and the point.
pub fn floor_drop(
    drlg: &DrlgWorld,
    field: &ExpField,
    room: Option<RoomId>,
    from: Point,
    size: i32,
    fallback: bool,
) -> Result<(Option<RoomId>, Point), PlaceError> {
    crate::path::place::floor_drop(&Rooms(drlg), field, room, from, size, fallback)
}

/// Coarse free-box search `0x0064E840(room, &point, n, mask)` (§8).
pub fn coarse_free_box(
    drlg: &DrlgWorld,
    room: RoomId,
    point: &mut Point,
    n: i32,
    mask: u32,
) -> Option<RoomId> {
    crate::path::search::coarse_free_box(&Rooms(drlg), room, point, n, mask)
}

fn log<X: Pending, T>(cv: &mut Shared<'_, '_, X>, r: Result<T, PlaceError>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            cv.0.borrow_mut().v.h.errors.push(WiringError::Place(e));
            None
        }
    }
}
