// Spec: specs/sim/pathing.md §1, §3, §8, §9; specs/sim/path-placement.md §5, §6; specs/sim/units.md §4.1, §4.4, §4.5 (wiring of the walk seams)
//! [`PathCtx`]: the one context `path::walk` runs on. It holds the game
//! (lists, timers, frame), the unit records, stat lists and the action
//! state (DRLG, path records, tables, [`Pending`]) and implements
//! [`PathWorld`] (path records, footprints, room lists, clients),
//! [`CollisionRooms`] (the DRLG, [`super::rooms`]), [`WalkUnits`] (unit
//! fields, modes and timers through the unit system, stats, charstats /
//! monstats velocities) and [`PathMotion`] (set position and reset for
//! the teleport of `path-placement.md` §6 rule 4).
//!
//! Entry points: [`PathCtx::walk_to`] (`0x005809D0`, point form),
//! [`walk_message`] (C→S 0x01–0x04, `pathing.md` §1.1), [`player_step`]
//! (event 0 of modes 2, 3, 6, 19: `0x00580C20`), [`unit_step`]
//! (`0x00554CA0`), [`build_path`] (`0x00649970`) and
//! [`PathCtx::teleport`] (`0x00650910`).

use std::sync::Arc;

use crate::drlg::{CollisionGrid, TileRect};
use crate::game::Game;
use crate::path::coords::{to_fp16_center, Point};
use crate::path::footprint::{teleport, PathMotion};
use crate::path::history::PositionHistory;
use crate::path::walk::request::{handle_message, request, Outcome, WalkTarget};
use crate::path::walk::seams::{PathWorld, StartTarget, UsedSkill, WalkUnits};
use crate::path::walk::{Step, Walk, WalkError};
use crate::path::{CollisionRooms, DynamicPath, PathTables, UnitPath};
use crate::rng::Seed;
use crate::units::{ClientId, RoomId, UnitId, UnitType};
use crate::wiring::action::{Pending, View, WiringError};

/// The walk context ([module doc](self)).
pub struct PathCtx<'a, X> {
    pub v: View<'a, X>,
    pub game: &'a mut Game,
}

impl<'a, X: Pending> PathCtx<'a, X> {
    /// A context over a view's parts and `game`.
    pub fn of(v: &'a mut View<'_, X>, game: &'a mut Game) -> Self {
        Self {
            v: View::of(&mut *v.units, &mut *v.stats, v.data, &mut *v.h),
            game,
        }
    }

    /// The path tables (the provider is on whenever a context is used).
    fn tables(&self) -> Arc<PathTables> {
        self.v
            .h
            .paths
            .as_ref()
            .map(|p| p.tables.clone())
            .expect("path provider on")
    }

    fn walk_error(&mut self, e: WalkError) {
        self.v.h.errors.push(WiringError::Walk(e));
    }

    /// Player mode request, point form `0x005809D0(game, unit, no skill,
    /// mode, x, y, 0)` (`pathing.md` §1.2).
    pub fn walk_to(&mut self, unit: UnitId, mode: u32, x: i32, y: i32) -> Option<Outcome> {
        let t = self.tables();
        let target = WalkTarget::Point(Point::new(x, y));
        match request(&t, self, unit, None, mode, target, false) {
            Ok(o) => Some(o),
            Err(e) => {
                self.walk_error(e);
                None
            }
        }
    }

    /// Teleport `0x00650910(path, room, x, y)` (`path-placement.md` §6
    /// rule 4) of a unit with a dynamic path; a static path is set
    /// (`0x00620AE0`) with its footprint moved.
    ///
    /// TODO(spec: path-placement.md §6 r4, §10): `0x00554EA0` on a unit
    /// with a static path (objects, items) is not described; the static
    /// set with the footprint removed and added again is used.
    pub fn teleport(&mut self, unit: UnitId, room: Option<RoomId>, x: i32, y: i32) {
        let missile = self.unit_type(unit) == UnitType::Missile;
        let rec = self
            .v
            .h
            .paths
            .as_ref()
            .and_then(|p| p.record(unit))
            .cloned();
        match rec {
            Some(UnitPath::Dynamic(mut d)) => {
                let r = teleport(self, &mut d, missile, room, x, y);
                self.store_path(unit, &d);
                if let Err(e) = r {
                    self.v.h.errors.push(WiringError::Path(e));
                }
            }
            Some(UnitPath::Static(mut s)) => {
                self.v.path_remove_footprint(unit, true);
                s.set(room, x, y);
                if let Some(p) = self.v.h.paths.as_mut() {
                    p.records.insert(unit, UnitPath::Static(s));
                }
                self.v.path_add_footprint(unit);
            }
            None => {}
        }
    }

    /// Room-change messages `0x00554670(game, unit, 0)` (`pathing.md`
    /// §9.8).
    pub fn room_change_messages(&mut self, unit: UnitId) {
        let Some(mut d) = self.load_path(unit) else {
            return;
        };
        let t = self.tables();
        Walk { t: &t, c: self }.room_change_messages(unit, &mut d);
        self.store_path(unit, &d);
    }

    /// Unit step `0x00554CA0` (`pathing.md` §9.3).
    pub fn step(&mut self, unit: UnitId) -> Option<Step> {
        let mut d = self.load_path(unit)?;
        let t = self.tables();
        let r = Walk { t: &t, c: self }.step(unit, &mut d);
        self.store_path(unit, &d);
        match r {
            Ok(s) => Some(s),
            Err(e) => {
                self.walk_error(e);
                None
            }
        }
    }
}

/// C→S 0x01–0x04 after `intents-events.md` §2.4 accepts it
/// (`pathing.md` §1.1): (x, y) or (type, GUID) in `a`, `b`. The handler
/// result is 0 in every case; the outcome for callers and tests.
pub fn walk_message<X: Pending>(
    v: &mut View<'_, X>,
    game: &mut Game,
    player: UnitId,
    id: u8,
    a: u32,
    b: u32,
) -> (u32, Option<Outcome>) {
    let mut c = PathCtx::of(v, game);
    let t = c.tables();
    match handle_message(&t, &mut c, player, id, a, b) {
        Ok(r) => r,
        Err(e) => {
            c.walk_error(e);
            (0, None)
        }
    }
}

/// C→S 0x5F UpdatePlayerPos (`0x0054CD50`, `pathing.md` §1.6): the
/// handler result and what it did (`None`: a fatal path, logged in
/// `ActionHooks::errors`, result 0).
pub fn resync_message<X: Pending>(
    v: &mut View<'_, X>,
    game: &mut Game,
    player: UnitId,
    msg: &[u8],
) -> (u32, Option<crate::path::walk::resync::Resync>) {
    let mut c = PathCtx::of(v, game);
    let t = c.tables();
    match crate::path::walk::resync::handle_resync(&t, &mut c, player, msg) {
        Ok((r, what)) => (r, Some(what)),
        Err(e) => {
            c.walk_error(e);
            (0, None)
        }
    }
}

/// `0x005809D0(game, player, no skill, mode, x, y, re-entry 1)`: the
/// player mode request with the interrupt gate skipped (`pathing.md`
/// §1.2; C→S 0x41 starts mode 1 this way, `intents-events.md` §9 rule
/// 6). `None`: a fatal path (logged).
pub fn request_skip_gate<X: Pending>(
    v: &mut View<'_, X>,
    game: &mut Game,
    player: UnitId,
    mode: u32,
    x: i32,
    y: i32,
) -> Option<Outcome> {
    let mut c = PathCtx::of(v, game);
    let t = c.tables();
    let target = WalkTarget::Point(Point::new(x, y));
    match request(&t, &mut c, player, None, mode, target, true) {
        Ok(o) => Some(o),
        Err(e) => {
            c.walk_error(e);
            None
        }
    }
}

/// Player event 0 of modes 2, 3, 6, 19: `0x00580C20` (`pathing.md`
/// §9.2); the action result for `units.md` §4.5 (2 = stopped).
pub fn player_step<X: Pending>(v: &mut View<'_, X>, game: &mut Game, unit: UnitId) -> u32 {
    let mut c = PathCtx::of(v, game);
    let t = c.tables();
    match (Walk { t: &t, c: &mut c }).player_event0(unit) {
        Ok(s) => s as u32,
        Err(e) => {
            c.walk_error(e);
            1
        }
    }
}

/// Unit step `0x00554CA0`: false when it returns 2 (no movement).
pub fn unit_step<X: Pending>(v: &mut View<'_, X>, game: &mut Game, unit: UnitId) -> bool {
    PathCtx::of(v, game).step(unit) == Some(Step::Moving)
}

/// Path compute `0x00649970(path, unit, town access)` (`pathing.md` §3)
/// toward the path's target.
// PROVISIONAL (missiles/missiles.md §R2.3 step 16): the missile build
// passes town access 0 (as the re-path, `pathing.md` §9.10); settled by a
// bin read (the missile compute likely ignores it).
pub fn build_path<X: Pending>(v: &mut View<'_, X>, game: &mut Game, unit: UnitId) {
    let mut c = PathCtx::of(v, game);
    let Some(mut d) = c.load_path(unit) else {
        return;
    };
    let t = c.tables();
    let r = crate::path::walk::find::compute(&t, &mut c, &mut d, unit, false);
    c.store_path(unit, &d);
    if let Err(e) = r {
        c.walk_error(e);
    }
}

/// The player part of the per-unit update message `0x0053A500`, for one
/// client of the per-client update (`tick.md` §6.5; `pathing.md` §10
/// rules 3 and 2, in that order): S→C 0x15 when the unit's flags 2 ask
/// for it (`0x00548010`), then, for a player whose mode changed (unit
/// flag 0x1), the walk modes' update function `0x00548180` (0x0F / 0x10;
/// other modes have their own rows, not specified: nothing). Sent through
/// [`Pending::send`] to the client's player. Units other than players
/// with a dynamic path, and clients without a player, get nothing.
///
/// The room clean-up (`tick.md` §3 step 6, `0x00553220`;
/// `intents-events.md` §7.5 step 3, [`crate::wiring::action::View::room_cleanup`])
/// clears unit flag 0x1 and flags 2 bits 0x10000 / 0x800 after the client
/// pass, so each message goes out once per change.
pub fn update_messages<X: Pending>(
    v: &mut View<'_, X>,
    game: &Game,
    client: ClientId,
    unit: UnitId,
) {
    use crate::path::walk::messages::{mode_update, reassign_flag, reassign_player};
    let Some(receiver) = game.lists.client(client).and_then(|c| c.player) else {
        return;
    };
    let Some(r) = v.units.get(unit) else {
        return;
    };
    if r.ty != UnitType::Player {
        return;
    }
    let (ty, guid, mode, flags, flags2) = (r.ty as u8, r.guid, r.mode, r.flags, r.flags2);
    let Some(path) = v.h.paths.as_ref().and_then(|p| p.dynamic(unit)).cloned() else {
        return;
    };
    let own = receiver == unit;
    if let Some(flag) = reassign_flag(flags2, own) {
        let msg = reassign_player(ty, guid, path.x() as u16, path.y() as u16, flag);
        v.h.x.send(receiver, &msg);
    }
    if flags & crate::units::record::flags::CHANGED != 0 {
        if let Some(msg) = mode_update(mode, ty, guid, &path, own) {
            v.h.x.send(receiver, &msg);
        }
    }
}

impl<X: Pending> CollisionRooms for PathCtx<'_, X> {
    fn subtile_rect(&self, room: RoomId) -> Option<TileRect> {
        self.v.h.drlg.subtile_rect(room)
    }
    fn adjacent_count(&self, room: RoomId) -> usize {
        self.v.h.drlg.adjacent_count(room)
    }
    fn adjacent(&self, room: RoomId, i: usize) -> Option<RoomId> {
        self.v.h.drlg.adjacent(room, i)
    }
    fn grid(&self, room: RoomId) -> Option<&CollisionGrid> {
        self.v.h.drlg.grid(room)
    }
    fn grid_mut(&mut self, room: RoomId) -> Option<&mut CollisionGrid> {
        self.v.h.drlg.grid_mut(room)
    }
}

impl<X: Pending> PathWorld for PathCtx<'_, X> {
    fn load_path(&self, unit: UnitId) -> Option<DynamicPath> {
        self.v.h.paths.as_ref()?.dynamic(unit).cloned()
    }
    fn store_path(&mut self, unit: UnitId, path: &DynamicPath) {
        if let Some(d) = self.v.h.paths.as_mut().and_then(|p| p.dynamic_mut(unit)) {
            *d = path.clone();
        }
    }
    fn room_in_town(&self, room: RoomId) -> bool {
        self.v.h.drlg.in_town(self.game, room)
    }
    fn remove_footprint(&mut self, unit: UnitId, force: bool) -> bool {
        self.v.path_remove_footprint(unit, force)
    }
    fn add_footprint(&mut self, unit: UnitId) {
        self.v.path_add_footprint(unit);
    }
    fn room_list_remove(&mut self, unit: UnitId, _: RoomId) {
        if let Err(e) = self.game.lists.room_remove(unit) {
            self.v
                .unit_error(crate::units::modes::UnitError::Game(e.into()));
        }
    }
    /// `0x0064C350` (`unit-order.md` §5.2: d2rs's insert also queues the
    /// unit, which the following `0x0064C040` would do).
    fn room_list_insert(&mut self, unit: UnitId, room: RoomId) {
        if let Err(e) = self.game.lists.room_insert(unit, room) {
            self.v
                .unit_error(crate::units::modes::UnitError::Game(e.into()));
        }
    }
    fn queue_for_update(&mut self, unit: UnitId) {
        if let Err(e) = self.game.lists.queue_update(unit) {
            self.v
                .unit_error(crate::units::modes::UnitError::Game(e.into()));
        }
    }
    /// The active room's client array (`drlg/rooms.md` §7).
    fn room_clients(&self, room: RoomId) -> Vec<ClientId> {
        self.v
            .h
            .drlg
            .drlg_room(self.game, room)
            .and_then(|(d, r)| d.active_room(r))
            .map_or_else(Vec::new, |a| a.clients.clone())
    }
    /// `0x005545C0`: the room still exists and belongs to the unit's act.
    fn room_in_unit_act(&self, unit: UnitId, room: RoomId) -> bool {
        let act = self.v.units.get(unit).map(|r| r.act);
        act.is_some() && self.game.lists.room(room).map(|r| r.act) == act
    }
}

impl<X: Pending> WalkUnits for PathCtx<'_, X> {
    fn frame(&self) -> i32 {
        self.game.frame
    }
    fn unit_type(&self, unit: UnitId) -> UnitType {
        self.v.units.get(unit).map_or(UnitType::Tile, |r| r.ty)
    }
    fn class(&self, unit: UnitId) -> u32 {
        self.v.units.get(unit).map_or(0, |r| r.class)
    }
    fn guid(&self, unit: UnitId) -> u32 {
        self.v.units.get(unit).map_or(0, |r| r.guid)
    }
    fn mode(&self, unit: UnitId) -> u32 {
        self.v.units.get(unit).map_or(0, |r| r.mode)
    }
    /// `0x00552F60`.
    fn find_unit(&self, ty: UnitType, guid: u32) -> Option<UnitId> {
        self.game.lists.find_unit(ty, guid)
    }
    fn position(&self, unit: UnitId) -> Point {
        let (x, y) = self.v.h.path_position(unit);
        Point::new(x, y)
    }
    fn unit_size(&self, unit: UnitId) -> i32 {
        self.v.path_size(unit)
    }
    /// The smallest positive expire of the unit's type-1 timers
    /// (`0x005415A0`).
    fn first_type1_expire(&self, unit: UnitId) -> i32 {
        let q = &self.game.timers;
        q.unit_timers(unit)
            .into_iter()
            .filter_map(|t| {
                let (ev, _, _) = q.event(t)?;
                (ev == crate::tick::events::event::END_ANIM).then_some(q.expire(t)?)
            })
            .filter(|&e| e > 0)
            .min()
            .unwrap_or(0)
    }
    fn has_state(&self, unit: UnitId, state: u16) -> bool {
        self.v.stats.has_state(unit, u32::from(state))
    }
    fn state_stat(&self, unit: UnitId, state: u16, stat: u16) -> i32 {
        self.v.state_stat(unit, state, stat).unwrap_or(0)
    }
    fn stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.v.stat(unit, stat)
    }
    /// `0x00625500` (items; [`Pending::item_stat`], layer 0).
    fn item_stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.v.h.x.item_stat(unit, stat, 0)
    }
    /// Base stat add `0x006272B0`.
    fn add_base_stat(&mut self, unit: UnitId, stat: u16, delta: i32) {
        let v = self.v.stats.unit_base(unit, stat, 0).wrapping_add(delta);
        self.v.set_base(unit, stat, v);
    }
    fn set_base_stat(&mut self, unit: UnitId, stat: u16, value: i32) {
        self.v.set_base(unit, stat, value);
    }
    fn seed(&mut self, unit: UnitId) -> &mut Seed {
        self.v.seed(unit)
    }
    /// `0x00620250` ([`Pending::used_skill`]) and its skills.txt row. The
    /// flags (`0x006446A0`, skill +0x0C) are the used skill entry's
    /// runtime E-flags word, not a skills.txt column (`pathing.md` §8.1).
    fn used_skill(&self, unit: UnitId) -> Option<UsedSkill> {
        let e = self.v.h.x.used_skill(unit)?;
        let row = self.v.h.tables.skills.skills.get(e.skill as usize)?;
        Some(UsedSkill {
            id: e.skill as u16,
            seq_input: i32::from(row.seqinput),
            srvdofunc: i32::from(row.srvdofunc),
            interrupt: row.interrupt,
            skill_flags: self.v.h.x.entry_flags(unit, &e),
        })
    }
    /// Mode set `0x00553570` (`units.md` §4.1) through the unit system.
    fn set_mode(&mut self, unit: UnitId, mode: u32) {
        let r = {
            let mut sim = crate::units::hooks::Sim {
                game: &mut *self.game,
                units: &mut *self.v.units,
                stats: &mut *self.v.stats,
                data: self.v.data,
            };
            crate::units::modes::set_mode(&mut sim, &mut *self.v.h, unit, mode)
        };
        if let Err(e) = r {
            self.v.unit_error(e);
        }
    }
    /// `0x00553990`-style cancel of one event type.
    fn cancel_events(&mut self, unit: UnitId, ty: u8) {
        self.game.timers.cancel_unit_events(unit, ty, None);
    }
    /// `0x00553F00` (`units.md` §4.4).
    fn schedule_event0(&mut self, unit: UnitId) {
        if let Err(e) = crate::units::anim::every_tick_movement(self.game, unit) {
            self.v.unit_error(crate::units::modes::UnitError::Game(e));
        }
    }
    /// The start function of a mode other than 2, 3, 6, 19 (player:
    /// `units.md` §4.5 through `0x005809D0`'s start table; the neutral
    /// re-entry). Targets are not passed: the unit system's starts take
    /// none.
    ///
    /// TODO(units.md §4.5): `player_start` also runs the mode request
    /// check hook; [`crate::wiring::action::ActionHooks`] keeps its
    /// default (accept), so only the start function runs. Monster
    /// starts: the monster mode set (`units.md` §4.6).
    fn start_other_mode(&mut self, unit: UnitId, mode: u32, _: StartTarget) {
        let player = self.unit_type(unit) == UnitType::Player;
        let r = {
            let mut sim = crate::units::hooks::Sim {
                game: &mut *self.game,
                units: &mut *self.v.units,
                stats: &mut *self.v.stats,
                data: self.v.data,
            };
            if player {
                crate::units::modes::player_start(&mut sim, &mut *self.v.h, unit, mode).map(|_| ())
            } else {
                crate::units::modes::monster_set_mode(&mut sim, &mut *self.v.h, unit, mode)
            }
        };
        if let Err(e) = r {
            self.v.unit_error(e);
        }
    }
    fn charstats_velocity(&self, unit: UnitId) -> (i32, i32, i32) {
        self.v.charstats_velocity(unit)
    }
    fn monstats_velocity(&self, unit: UnitId) -> (i32, bool) {
        self.v.monster_velocity(unit)
    }
    /// `0x0063E860` (`path-placement.md` §3).
    fn monster_can_be_in_town(&self, unit: UnitId) -> bool {
        self.v
            .monster_shape(unit)
            .is_some_and(|m| m.can_be_in_town())
    }
    fn set_unit_flag(&mut self, unit: UnitId, bit: u32) {
        if let Some(r) = self.v.units.get_mut(unit) {
            r.flags |= bit;
        }
    }
    /// The player of a client record (`unit-order.md` §7).
    fn client_player(&self, client: ClientId) -> Option<UnitId> {
        self.game.lists.client(client)?.player
    }
    /// [`crate::wiring::path::PathState::history`] (players only).
    fn position_history(&mut self, unit: UnitId) -> Option<&mut PositionHistory> {
        if self.unit_type(unit) != UnitType::Player {
            return None;
        }
        Some(self.v.h.paths.as_mut()?.history.entry(unit).or_default())
    }
}

impl<X: Pending> PathMotion for PathCtx<'_, X> {
    /// Set position `0x0064FB90(Q, hint)` at the cell centre (`pathing.md`
    /// §9.6 rule 8), with the room recache of rule 9.
    fn set_position(&mut self, path: &mut DynamicPath, x: i32, y: i32, hint: Option<RoomId>) {
        let Some(unit) = path.owner else {
            return;
        };
        let t = self.tables();
        Walk { t: &t, c: self }.set_position(
            unit,
            path,
            (to_fp16_center(x), to_fp16_center(y)),
            hint,
        );
    }
    /// Reset `0x006507B0` (`pathing.md` §9.7).
    fn reset(&mut self, path: &mut DynamicPath) {
        let Some(unit) = path.owner else {
            return;
        };
        let t = self.tables();
        Walk { t: &t, c: self }.reset(unit, path);
    }
}
