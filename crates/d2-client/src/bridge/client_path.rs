// Spec: specs/sim/pathing.md (§1.2–§1.5, §3–§9), specs/sim/path-placement.md (§2.4, §4, §5), specs/client/model.md (§3 rule 3, open question 2)
//! The local player's own path over the client DRLG: the walk
//! prediction's path step (decision D2 in `docs/PLAN.md`).
//!
//! The 1.14d client moves its own player between server messages along
//! its own path, stepped every client update (`client/model.md` §3 rule
//! 3, open question 2). The path code is the server's (`sim/pathing.md`:
//! the request §1.2–§1.5, the compute §3–§7, the per-tick step §9) over
//! the client's rooms, so the client's walk stops at the same wall and
//! goes round the same obstacle as the server's. This module runs that
//! code, `d2-sim`'s, over the client DRLG ([`ClientDrlg`]'s active rooms
//! and collision grids):
//!
//! - [`ClientPath::place`] allocates the player's path at a sub-tile in
//!   its client room (`path-placement.md` §2.4, kind player);
//! - [`ClientPath::request`] is the player mode request (walk 2 / run 3
//!   to a point or a unit, §1.2);
//! - [`ClientPath::tick`] is the player event 0 step (§9.2) of one tick.
//!
//! The player's footprint is stamped on a private copy of the grids it
//! touches (the model's DRLG is not written); other units' footprints are
//! not in the client grids (the model stamps none).
//!
//! d2rs-own, unverified. PROVISIONAL (`client/model.md` OQ2; REC-51,
//! REC-277): that the 1.14d client step `0x00463390` runs these functions
//! with these inputs (the unit's stats read as: velocity percent and
//! stat 96 from the model ([`Own`]) plus the run bonus, stamina from the
//! model, no state, no used skill, no drain), and that client units
//! leave no footprints on the client grid.

use std::collections::BTreeMap;

use d2_sim::drlg::{CollisionGrid, Drlg, TileRect};
use d2_sim::path::record::{alloc_dynamic_path, DynamicKind, DynamicPath};
use d2_sim::path::tables::PathTables;
use d2_sim::path::walk::request::mode;
use d2_sim::path::walk::velocity::{STAT_FASTERMOVE, STAT_VELOCITYPERCENT};
use d2_sim::path::walk::{request, PathWorld, Point, Step, Walk, WalkTarget, WalkUnits};
use d2_sim::path::CollisionRooms;
use d2_sim::rng::Seed;
use d2_sim::units::{ClientId, RoomId, UnitId, UnitType};

use super::predict::{MoveStats, Speeds};

/// The local player in the path context.
const ME: UnitId = UnitId(1);
/// The walk's target unit (a 0x02 / 0x04 walk).
const TARGET: UnitId = UnitId(2);
/// The stamina stat (8.8).
const STAT_STAMINA: u16 = 10;

/// The local player's own stats the path reads from the model: stamina
/// (stat 10) and the velocity stats ([`MoveStats`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Own {
    pub stamina: i32,
    pub moves: MoveStats,
}

/// What the walk goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathTo {
    Point(u16, u16),
    /// A unit of the model: its type, GUID and position.
    Unit(UnitType, u32, (u16, u16)),
}

/// The local player's client path (module doc).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientPath {
    path: Option<DynamicPath>,
    /// The grids the footprint wrote, copied from the client DRLG.
    grids: BTreeMap<RoomId, CollisionGrid>,
    mode: u32,
    run_bonus: i32,
    seed: Seed,
    frame: i32,
}

impl ClientPath {
    /// The path's precise (16.16) position; `None` without a path.
    pub fn position(&self) -> Option<(u32, u32)> {
        self.path.as_ref().map(|p| (p.precise_x, p.precise_y))
    }

    /// The player's mode (2 walk, 3 run, 6 town walk while moving).
    pub fn mode(&self) -> u32 {
        self.mode
    }

    /// Allocates the player's path at sub-tile (x, y) in the active room
    /// of `drlg` that contains it (`path-placement.md` §2.4, kind player;
    /// footprint stamped on the private grids). `false`: no active room
    /// holds the point (no path).
    pub fn place(&mut self, t: &PathTables, drlg: &Drlg, x: u16, y: u16) -> bool {
        self.grids.clear();
        self.path = None;
        self.mode = mode::NEUTRAL;
        let (x, y) = (i32::from(x), i32::from(y));
        let Some(room) = room_at(drlg, x, y) else {
            return false;
        };
        let mut rooms = Rooms {
            drlg,
            grids: &mut self.grids,
        };
        self.path = alloc_dynamic_path(
            t,
            &mut rooms,
            DynamicKind::Player,
            ME,
            Some(room),
            x,
            y,
            false,
        )
        .ok();
        self.path.is_some()
    }

    /// The player mode request (§1.2) of a walk (`run`: mode 3, else 2)
    /// to `to`. `true`: the path moves (a point count above 0).
    pub fn request(
        &mut self,
        t: &PathTables,
        drlg: &Drlg,
        speeds: Speeds,
        own: Own,
        to: PathTo,
        run: bool,
    ) -> bool {
        let m = if run { mode::RUN } else { mode::WALK };
        let target = match to {
            PathTo::Point(x, y) => WalkTarget::Point(Point::new(i32::from(x), i32::from(y))),
            PathTo::Unit(ty, guid, _) => WalkTarget::Unit { ty, guid },
        };
        let mut c = self.ctx(drlg, speeds, own, to);
        matches!(
            request(t, &mut c, ME, None, m, target, false),
            Ok(d2_sim::path::walk::Outcome::Moving(_))
        )
    }

    /// One tick of the player event 0 step (§9.2). `true`: still moving.
    pub fn tick(
        &mut self,
        t: &PathTables,
        drlg: &Drlg,
        speeds: Speeds,
        own: Own,
        to: Option<PathTo>,
    ) -> bool {
        if !matches!(self.mode, mode::WALK | mode::RUN | mode::TOWN_WALK) {
            return false;
        }
        self.frame = self.frame.wrapping_add(1);
        let to = to.unwrap_or(PathTo::Point(0, 0));
        let mut c = self.ctx(drlg, speeds, own, to);
        let mut w = Walk { t, c: &mut c };
        matches!(w.player_event0(ME), Ok(Step::Moving))
    }

    fn ctx<'a>(&'a mut self, drlg: &'a Drlg, speeds: Speeds, own: Own, to: PathTo) -> Ctx<'a> {
        Ctx {
            rooms: Rooms {
                drlg,
                grids: &mut self.grids,
            },
            path: &mut self.path,
            mode: &mut self.mode,
            run_bonus: &mut self.run_bonus,
            seed: &mut self.seed,
            frame: self.frame,
            speeds,
            own,
            to,
        }
    }
}

/// The active room of `drlg` whose sub-tile rect holds (x, y).
fn room_at(drlg: &Drlg, x: i32, y: i32) -> Option<RoomId> {
    drlg.active_rooms()
        .into_iter()
        .find(|&(_, r)| {
            drlg.active_room(r)
                .is_some_and(|a| a.subtiles.contains(x, y))
        })
        .map(|(id, _)| id)
}

/// [`CollisionRooms`] over the client DRLG, with the private grids in
/// front of the DRLG's.
struct Rooms<'a> {
    drlg: &'a Drlg,
    grids: &'a mut BTreeMap<RoomId, CollisionGrid>,
}

impl Rooms<'_> {
    fn active(&self, room: RoomId) -> Option<&d2_sim::drlg::ActiveRoom> {
        self.drlg.active_room(self.drlg.drlg_room_of(room)?)
    }
}

impl CollisionRooms for Rooms<'_> {
    fn subtile_rect(&self, room: RoomId) -> Option<TileRect> {
        self.active(room).map(|a| a.subtiles)
    }

    fn adjacent_count(&self, room: RoomId) -> usize {
        self.active(room).map_or(0, |a| a.adjacency.len())
    }

    fn adjacent(&self, room: RoomId, i: usize) -> Option<RoomId> {
        let n = *self.active(room)?.adjacency.get(i)?;
        self.drlg.active_room(n).map(|a| a.id)
    }

    fn grid(&self, room: RoomId) -> Option<&CollisionGrid> {
        match self.grids.get(&room) {
            Some(g) => Some(g),
            None => self.active(room).map(|a| &a.collision),
        }
    }

    fn grid_mut(&mut self, room: RoomId) -> Option<&mut CollisionGrid> {
        if !self.grids.contains_key(&room) {
            let g = self.active(room)?.collision.clone();
            self.grids.insert(room, g);
        }
        self.grids.get_mut(&room)
    }
}

/// The walk context of the client path.
struct Ctx<'a> {
    rooms: Rooms<'a>,
    path: &'a mut Option<DynamicPath>,
    mode: &'a mut u32,
    run_bonus: &'a mut i32,
    seed: &'a mut Seed,
    frame: i32,
    speeds: Speeds,
    own: Own,
    to: PathTo,
}

impl CollisionRooms for Ctx<'_> {
    fn subtile_rect(&self, room: RoomId) -> Option<TileRect> {
        self.rooms.subtile_rect(room)
    }
    fn adjacent_count(&self, room: RoomId) -> usize {
        self.rooms.adjacent_count(room)
    }
    fn adjacent(&self, room: RoomId, i: usize) -> Option<RoomId> {
        self.rooms.adjacent(room, i)
    }
    fn grid(&self, room: RoomId) -> Option<&CollisionGrid> {
        self.rooms.grid(room)
    }
    fn grid_mut(&mut self, room: RoomId) -> Option<&mut CollisionGrid> {
        self.rooms.grid_mut(room)
    }
}

impl PathWorld for Ctx<'_> {
    fn load_path(&self, unit: UnitId) -> Option<DynamicPath> {
        (unit == ME).then(|| self.path.clone()).flatten()
    }
    fn store_path(&mut self, unit: UnitId, path: &DynamicPath) {
        if unit == ME {
            *self.path = Some(path.clone());
        }
    }
    fn room_in_town(&self, room: RoomId) -> bool {
        let Some(r) = self.rooms.drlg.drlg_room_of(room) else {
            return false;
        };
        let d = self.rooms.drlg;
        d2_sim::drlg::is_town(d.level(d.room(r).level).id)
    }
    fn remove_footprint(&mut self, unit: UnitId, force: bool) -> bool {
        let Some(fp) = self.path.as_ref().filter(|_| unit == ME).map(footprint_of) else {
            return false;
        };
        d2_sim::path::footprint::remove_footprint(
            &mut self.rooms,
            &fp,
            d2_sim::path::RemoveRule::Other,
            force,
        )
    }
    fn add_footprint(&mut self, unit: UnitId) {
        if let Some(fp) = self.path.as_ref().filter(|_| unit == ME).map(footprint_of) {
            d2_sim::path::footprint::add_footprint(&mut self.rooms, &fp);
        }
    }
    fn room_list_remove(&mut self, _: UnitId, _: RoomId) {}
    fn room_list_insert(&mut self, _: UnitId, _: RoomId) {}
    fn queue_for_update(&mut self, _: UnitId) {}
    fn room_clients(&self, _: RoomId) -> Vec<ClientId> {
        Vec::new()
    }
}

fn footprint_of(p: &DynamicPath) -> d2_sim::path::Footprint {
    d2_sim::path::Footprint {
        room: p.room,
        x: p.x(),
        y: p.y(),
        shape: d2_sim::path::FootShape::Pattern(p.pattern),
        mask: p.foot_mask,
    }
}

impl WalkUnits for Ctx<'_> {
    fn unit_type(&self, unit: UnitId) -> UnitType {
        match (unit, self.to) {
            (TARGET, PathTo::Unit(ty, ..)) => ty,
            _ => UnitType::Player,
        }
    }
    fn guid(&self, unit: UnitId) -> u32 {
        match (unit, self.to) {
            (TARGET, PathTo::Unit(_, guid, _)) => guid,
            _ => unit.0,
        }
    }
    fn frame(&self) -> i32 {
        self.frame
    }
    fn mode(&self, unit: UnitId) -> u32 {
        if unit == ME {
            *self.mode
        } else {
            mode::NEUTRAL
        }
    }
    fn find_unit(&self, ty: UnitType, guid: u32) -> Option<UnitId> {
        match self.to {
            PathTo::Unit(t, g, _) if t == ty && g == guid => Some(TARGET),
            _ => None,
        }
    }
    fn position(&self, unit: UnitId) -> Point {
        match (unit, self.to) {
            (ME, _) => self.path.as_ref().map(|p| p.cell()).unwrap_or_default(),
            (TARGET, PathTo::Unit(_, _, (x, y))) => Point::new(i32::from(x), i32::from(y)),
            _ => Point::default(),
        }
    }
    fn stat(&self, unit: UnitId, stat: u16) -> i32 {
        match (unit, stat) {
            (ME, STAT_VELOCITYPERCENT) => {
                self.own.moves.percent
                    + if *self.mode == mode::RUN {
                        *self.run_bonus
                    } else {
                        0
                    }
            }
            (ME, STAT_STAMINA) => self.own.stamina,
            _ => 0,
        }
    }
    fn item_stat(&self, unit: UnitId, stat: u16) -> i32 {
        match (unit, stat) {
            (ME, STAT_FASTERMOVE) => self.own.moves.faster,
            _ => 0,
        }
    }
    fn seed(&mut self, _: UnitId) -> &mut Seed {
        self.seed
    }
    fn set_mode(&mut self, unit: UnitId, m: u32) {
        if unit == ME {
            *self.mode = m;
        }
    }
    fn attach_run_stats(&mut self, _: UnitId, value: i32) {
        *self.run_bonus = value;
    }
    fn charstats_velocity(&self, _: UnitId) -> (i32, i32, i32) {
        (i32::from(self.speeds.walk), i32::from(self.speeds.run), 0)
    }
}
