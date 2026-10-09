// Spec: specs/client/model.md (§3 r3, open question 2), specs/sim/pathing.md (§1.2–§1.5, §3–§9, cases M1, D1–D4), specs/ui/controls.md (§6 r7)
//! Provisional own-walk motion of the local player (first playable
//! preview, decision D2 in `docs/PLAN.md`).
//!
//! The server sends the walking player nothing while it walks
//! (`sim/pathing.md` §10 r2), and the client model moves a unit only when
//! a message places it (`client/model.md` §3 r3). How 1.14d moves its
//! own player between messages is `client/model.md` open question 2,
//! settled by the REC-51 recording. Until then this module stands in:
//!
//! - every walk / run C→S message the client sends (0x01–0x04, `ui/controls.md`
//!   §6 r7) sets the target ([`walk_of`], recorded by [`PredictLink`]);
//! - so does the server's walk request for the local player: an S→C
//!   0x0D with code 1 (`client/msg-units.md` §4 r1) is the player mode
//!   request code 0x01, "walk to (r0, r1)" (`client/model.md` §8 r4).
//!   The server sends it for the arrival walk-outs of a warp
//!   (`sim/path-placement.md` §12.2 r5–6), a portal (`world/objects.md`
//!   §12 r11) and a waypoint (`world/waypoints.md` §7 r7), each right
//!   before the 0x15 that places the player at the arrival point; the
//!   walk is held until the player's client room holds its target (so it
//!   starts from the arrival point, after the level change) and a walk
//!   the player sends replaces it ([`Predict::server_walk`]); after a
//!   waypoint travel the client sent (C→S 0x49) the walk is dropped, as
//!   1.14d never walks a waypoint arrival out (REC-288, measured);
//! - each server tick the predicted position steps along the player's own
//!   path over the client DRLG ([`ClientPath`]: the server's path code,
//!   `sim/pathing.md` §1.2–§9, so it stops at the same wall and goes
//!   round the same obstacle); without a client DRLG (or no active room
//!   at the predicted cell) it steps toward the target in a straight line
//!   at the charstats walk / run speed ([`Speeds`]);
//! - every change of the model's local position or server point (0x15
//!   placement, 0x0F / position-check correction) snaps it back to the
//!   model ([`Predict::observe`]);
//! - the facing (`dir64`, [`Predict::facing`]) is the direction of
//!   `sim/pathing.md` §8.3 from the predicted position toward the target
//!   (§8.4 r2, facing §8.5 without the client's turn), set when a walk is
//!   taken and each step; it stays when the walk ends.
//!
//! The server stays the authority (rule 7): nothing here is sent or
//! written to the model; the prediction only feeds the view and the
//! click's screen → world conversion ([`Predict::position`]).
//!
//! d2rs-own, unverified. PROVISIONAL (client/model.md OQ2; REC-51,
//! REC-277 (d)): the client path step (`ClientPath`'s module doc), the
//! straight-line fallback, the
//! snap rule, the tick step and the facing (the client turns, §8.5) are
//! not 1.14d facts. PROVISIONAL (REC-570; REC-288 settled the waypoint
//! case): for a warp or portal arrival, that the 0x0D walk outlives
//! the 0x15 placement after it (the placement's teleport sets the path's
//! point count to 0, `sim/path-placement.md` §6 r4; whether the client
//! then walks on to the request's target is OQ2), and the hold until the
//! target is in the player's room. Measured on a waypoint arrival
//! (`traces/client/model/client-0002.json`): the 1.14d client stands at
//! the 0x15 point with the server and never walks to the 0x0D's x + 3,
//! y + 3, where this module draws it: q-fix-arrival-walkout (warps and
//! portals not recorded yet, REC-570). Used only by the `play` preview; the
//! strict path never builds one.

use std::sync::OnceLock;

use d2_sim::path::tables::PathTables;
use d2_sim::path::walk::geom::direction_vector;
use d2_sim::path::walk::velocity::{STAT_FASTERMOVE, STAT_VELOCITYPERCENT, VELOCITY_PERCENT_FLOOR};

use super::client_path::{ClientPath, Own, PathTo};
use super::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use super::world::{ClientWorld, UnitKey, PLAYER};

/// The direction `dir64` (0–63) from precise (16.16) point `from` toward
/// `to` (`sim/pathing.md` §8.3, `tan` table of `path-tables.tsv`, without
/// the path flag 0x200 flip); `None` when the points are equal (§8.4 r1:
/// no direction toward the point the unit is on) or the tables do not
/// parse.
pub fn facing(from: (u32, u32), to: (u32, u32)) -> Option<u8> {
    if from == to {
        return None;
    }
    let tables = path_tables()?;
    // d2rs-own, unverified: `0x0064FC60` takes the 32-bit `127 × l`, which
    // wraps past about 258 sub-tiles and then indexes the `tan` table out
    // of range; the original only aims at near path points, but the model
    // can hold a position from before a level change. Far points are
    // brought near by halving both deltas (the direction keeps its ratio).
    let mut d = (
        i64::from(to.0) - i64::from(from.0),
        i64::from(to.1) - i64::from(from.1),
    );
    while d.0.abs().max(d.1.abs()) >= FACING_REACH {
        d = (d.0 / 2, d.1 / 2);
    }
    let near = (
        (i64::from(from.0) + d.0) as u32,
        (i64::from(from.1) + d.1) as u32,
    );
    if near == from {
        return None;
    }
    Some(direction_vector(tables, from, near).1)
}

/// The spec path tables (`path-tables.tsv`); `None` when they do not
/// parse.
pub(crate) fn path_tables() -> Option<&'static PathTables> {
    static TABLES: OnceLock<Option<PathTables>> = OnceLock::new();
    TABLES.get_or_init(|| PathTables::spec().ok()).as_ref()
}

/// The precise distance (16.16) below which `127 × l` stays in `i32`.
const FACING_REACH: i64 = 1 << 23;

/// The precise (16.16) centre of a sub-tile, as `u32`.
pub fn cell_centre((x, y): (u16, u16)) -> (u32, u32) {
    ((u32::from(x) << 16) | 0x8000, (u32::from(y) << 16) | 0x8000)
}

/// The charstats speeds of the local player's class (`WalkVelocity`
/// +0x40, `RunVelocity`; `sim/pathing.md` §8.1 r2, §8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Speeds {
    pub walk: u8,
    pub run: u8,
}

impl Speeds {
    /// The precise (16.16 sub-tile) distance of one server tick.
    /// `sim/pathing.md` §8.1 r2: velocity = `WalkVelocity` · 256 · p / 100
    /// with p = 100 walking and 100 + (100 · `RunVelocity` / `WalkVelocity`
    /// − 100) running (§8.2; the run list is skipped when `WalkVelocity`
    /// is 0); §9.4 r2.1 with base 0x400: step = velocity · 16 (case M1:
    /// velocity 0x600 moves 0x6000 a tick). Stats 67 / 96 at their
    /// creation values ([`Self::step_with`]).
    pub fn step(self, run: bool) -> i64 {
        self.step_with(run, MoveStats::CREATION)
    }

    /// [`Self::step`] with the player's stats 67 / 96 `s`
    /// (`sim/pathing.md` §8.1 r2): p = max(f + stat 67 + run bonus, 25),
    /// f = base · raw / (base + raw) of animstat row 4 over stat 96 (0
    /// without it).
    pub fn step_with(self, run: bool, s: MoveStats) -> i64 {
        let walk = i64::from(self.walk);
        let bonus = if run && walk != 0 {
            100 * i64::from(self.run) / walk - 100
        } else {
            0
        };
        let f = match path_tables() {
            Some(t) if s.faster != 0 => {
                let b = i64::from(t.animstat[4][1]);
                b * i64::from(s.faster) / (b + i64::from(s.faster))
            }
            _ => 0,
        };
        let p = (f + i64::from(s.percent) + bonus).max(i64::from(VELOCITY_PERCENT_FLOOR));
        (walk * 256 * p / 100) * 16
    }
}

/// The local player's velocity stats the prediction reads from the model
/// (`seams/movement-prediction.md` §2.6 r2): stat 67 `velocitypercent`
/// total outside the run list (the path adds the run list itself, as
/// the server's does, `sim/pathing.md` §8.2) and stat 96
/// `item_fastermovevelocity` total (§8.1 r2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveStats {
    pub percent: i32,
    pub faster: i32,
}

impl MoveStats {
    /// A player's creation values (`client/msg-units.md` §1.1 r3: stat
    /// 67 := 100; no items).
    pub const CREATION: MoveStats = MoveStats {
        percent: 100,
        faster: 0,
    };

    /// The local player's stats 67 and 96 (`client/stat-lists.md` §1 r3
    /// `total`). A model unit without its base stat 67 (no creation
    /// values: synthetic fixtures) reads the creation base 100 under its
    /// lists. No local player: [`Self::CREATION`].
    pub fn of_local(world: &ClientWorld) -> Self {
        let Some(p) = world.local() else {
            return Self::CREATION;
        };
        let base = if p.stats.contains_key(&STAT_VELOCITYPERCENT) {
            0
        } else {
            Self::CREATION.percent
        };
        MoveStats {
            percent: base + world.total(p.key, STAT_VELOCITYPERCENT, 0),
            faster: world.total(p.key, STAT_FASTERMOVE, 0),
        }
    }
}

/// Where a walk C→S message goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkTo {
    /// 0x01 / 0x03: a sub-tile point.
    Point(u16, u16),
    /// 0x02 / 0x04: a unit (its position each tick).
    Unit(UnitKey),
}

/// One walk or run intent the client sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walk {
    pub to: WalkTo,
    pub run: bool,
}

/// The walk intent of a C→S message (`ui/controls.md` §6 r7: codes 1 / 3
/// walk / run to a point, {id, x u16, y u16}; codes 2 / 4 to a unit, {id,
/// type u32, guid u32}); any other message → `None`.
pub fn walk_of(msg: &[u8]) -> Option<Walk> {
    let (&id, rest) = msg.split_first()?;
    let run = matches!(id, 3 | 4);
    let to = match id {
        1 | 3 if rest.len() == 4 => WalkTo::Point(
            u16::from_le_bytes([rest[0], rest[1]]),
            u16::from_le_bytes([rest[2], rest[3]]),
        ),
        2 | 4 if rest.len() == 8 => {
            let t = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
            let guid = u32::from_le_bytes([rest[4], rest[5], rest[6], rest[7]]);
            WalkTo::Unit(UnitKey::new(u8::try_from(t).ok()?, guid))
        }
        _ => return None,
    };
    Some(Walk { to, run })
}

/// The centre of a sub-tile in precise (16.16) coordinates.
fn centre((x, y): (u16, u16)) -> (i64, i64) {
    ((i64::from(x) << 16) | 0x8000, (i64::from(y) << 16) | 0x8000)
}

/// Floor square root of `n`.
fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = 1u64 << (64 - n.leading_zeros()).div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

/// The local player's predicted position.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Predict {
    /// The local player the prediction is for.
    player: Option<UnitKey>,
    /// The model's (position, rule-8 follow count) at the last
    /// observation.
    seen: Option<((u16, u16), u32)>,
    /// Precise (16.16) predicted position.
    at: Option<(i64, i64)>,
    /// The walk under way.
    walk: Option<Walk>,
    /// The predicted facing `dir64`; kept when the walk ends.
    dir: Option<u8>,
    /// The local player's stamina (stat 10) is zero: the server walks
    /// instead of running (`pathing.md` §9.9, `units.md` §4.5), so the
    /// prediction does too. The 1.14d client ends its run on model stat
    /// 10 = 0, i.e. server raw stamina < 256 (`client/model.md` OQ2,
    /// `seams/movement-prediction.md` §2.5 r3); the server runs while raw
    /// is 1..255, a gap the position check corrects.
    exhausted: bool,
    /// The local player's level at the last observation.
    level: Option<u16>,
    /// The client's act at the last level change.
    act: Option<u8>,
    /// The local player's mode request count (`ClientUnit::mode_requests`)
    /// at the last look.
    requests: u32,
    /// A server walk (S→C 0x0D code 1) whose target is not yet in the
    /// player's client room ([`Self::server_walk`]).
    held: Option<(u16, u16)>,
    /// The client sent a waypoint travel (C→S 0x49) whose arrival walk
    /// request has not come yet ([`Self::waypoint_sent`]).
    waypoint: bool,
    /// The server tick the walk under way started on (kept while a new
    /// click re-targets it; cleared when it ends): the walk animation's
    /// start ([`Self::walk_since`]).
    since: Option<(u64, u32)>,
    /// The player's own path over the client DRLG ([`ClientPath`]): the
    /// step of a walk when the client has a DRLG.
    path: ClientPath,
    /// The walk and the predicted position the path was last stepped
    /// for: a new walk or a snap ([`Self::observe`]) re-places it.
    path_for: Option<(Walk, (i64, i64))>,
}

/// C→S 0x49, waypoint travel (`world/waypoints.md`).
const C2S_WAYPOINT: u8 = 0x49;

/// The player mode request code "walk to (r0, r1)" (`client/model.md`
/// §8 r4, code 0x01).
const CODE_WALK_TO_POINT: u8 = 0x01;

impl Predict {
    pub fn new() -> Self {
        Self::default()
    }

    /// Snaps to the model (D2): a new local player starts at its
    /// position; a change of the model's position (0x15, a check
    /// correction) or a check that took the server's point
    /// (`client/model.md` §6 r8, [`ClientUnit::follows`]) moves the
    /// prediction to it, server point first. The walk target is kept, unless the player's
    /// level changed (a warp, portal or act change: the target is a point
    /// of the old level). A level change the prediction did not walk into
    /// is a placement (0x15 / 0x59): the prediction moves to the model's
    /// position even when that is the cell the model held before (an act
    /// change that lands on the old act's arrival cell). No local player,
    /// or one with no position: no prediction.
    pub fn observe(&mut self, world: &ClientWorld) {
        let Some(p) = world.local().filter(|p| p.key.unit_type == PLAYER) else {
            *self = Self::default();
            return;
        };
        let Some(pos) = p.position else {
            *self = Self::default();
            return;
        };
        let now = (pos, p.follows);
        let level = world.player_level();
        let act = world.act.as_ref().map(|a| a.act);
        if level != self.level {
            // The prediction's own walk across a level border (the room
            // recache of `world_view::walk_room`) is no placement: same
            // act, the predicted cell in the player's room.
            let walked_in = act == self.act
                && self.cell().is_some_and(|(x, y)| {
                    world
                        .local_room()
                        .is_some_and(|r| r.contains(i32::from(x), i32::from(y)))
                });
            self.level = level;
            self.act = act;
            self.walk = None;
            if self.player == Some(p.key) && !walked_in {
                self.at = Some(centre(pos));
                self.seen = Some(now);
                return;
            }
        }
        if self.player != Some(p.key) {
            *self = Self {
                player: Some(p.key),
                seen: Some(now),
                at: Some(centre(pos)),
                walk: None,
                dir: None,
                exhausted: false,
                level: world.player_level(),
                act: world.act.as_ref().map(|a| a.act),
                requests: p.mode_requests,
                held: None,
                waypoint: self.waypoint,
                since: None,
                path: ClientPath::default(),
                path_for: None,
            };
            return;
        }
        let Some((old_pos, old_follows)) = self.seen else {
            self.seen = Some(now);
            self.at = Some(centre(pos));
            return;
        };
        // Only a check that took the server's point (§6 rule 8, the
        // local player's `Checked::Followed`) moves the prediction: a
        // kept check (rule 7) leaves the client's own position as it is,
        // so a waypoint arrival's walk-out stays drawn while its server
        // player stands at the arrival point within the tolerance.
        if p.follows != old_follows && p.server_point.0 != 0 && p.server_point.1 != 0 {
            self.at = Some(centre(p.server_point));
        } else if pos != old_pos {
            self.at = Some(centre(pos));
        }
        self.seen = Some(now);
    }

    /// The server's walk request for the local player (module doc): a
    /// new mode request with code 0x01 (S→C 0x0D code 1) is held; any
    /// other new request drops a held one. A held walk starts once the
    /// player's client room holds its target: after the 0x15 of an
    /// arrival, from the arrival point (a target in another level is
    /// never stepped toward from the old position).
    pub fn server_walk(&mut self, world: &ClientWorld) {
        let Some(p) = world.local().filter(|p| Some(p.key) == self.player) else {
            return;
        };
        if p.mode_requests != self.requests {
            self.requests = p.mode_requests;
            // A waypoint arrival (`world/waypoints.md` §7 r7): 1.14d takes
            // the 0x15 point and never walks to the 0x0D's x + 3, y + 3
            // (the teleport zeroes the path, `sim/path-placement.md` §6
            // r4; measured, REC-288, `traces/client/model/client-0002.json`).
            if std::mem::take(&mut self.waypoint) {
                self.held = None;
                return;
            }
            self.held = p
                .last_mode_request
                .filter(|r| r.code == CODE_WALK_TO_POINT)
                .and_then(|r| {
                    Some((
                        u16::try_from(r.record[0]).ok()?,
                        u16::try_from(r.record[1]).ok()?,
                    ))
                });
        }
        let Some((x, y)) = self.held else {
            return;
        };
        if self.at.is_none()
            || !world
                .local_room()
                .is_some_and(|room| room.contains(i32::from(x), i32::from(y)))
        {
            return;
        }
        self.held = None;
        self.walk = Some(Walk {
            to: WalkTo::Point(x, y),
            run: false,
        });
    }

    /// The client sent a waypoint travel (C→S 0x49): the next server walk
    /// request is that arrival's walk-out, which is not walked.
    pub fn waypoint_sent(&mut self) {
        self.waypoint = true;
    }

    /// A walk intent the client sent: the new target (a held server walk
    /// is dropped).
    pub fn walk(&mut self, walk: Walk) {
        self.held = None;
        self.exhausted = false;
        if self.at.is_some() {
            self.walk = Some(walk);
        }
    }

    /// Stops the walk (the prediction stays where it is).
    pub fn stop(&mut self) {
        self.walk = None;
    }

    /// One server tick: a straight step of [`Speeds::step`] toward the
    /// target's sub-tile centre; reaching it ends the walk. A unit target
    /// not in the model ends the walk.
    pub fn tick(&mut self, world: &ClientWorld, speeds: Speeds) {
        let (Some((x, y)), Some(walk)) = (self.at, self.walk) else {
            return;
        };
        let target = match walk.to {
            WalkTo::Point(tx, ty) => Some((tx, ty)),
            WalkTo::Unit(k) => world.units.get(&k).and_then(|u| u.position),
        };
        let Some(target) = target else {
            self.walk = None;
            return;
        };
        let (tx, ty) = centre(target);
        if let Some(d) = facing((x as u32, y as u32), (tx as u32, ty as u32)) {
            self.dir = Some(d);
        }
        let (dx, dy) = (tx - x, ty - y);
        self.exhausted = world
            .local()
            .is_some_and(|p| p.stats.get(&10).is_some_and(|s| *s == 0));
        let moves = MoveStats::of_local(world);
        if self.path_step(world, speeds, moves, walk, target) {
            return;
        }
        let step = speeds.step_with(walk.run && !self.exhausted, moves);
        let dist = isqrt(dx.unsigned_abs().pow(2) + dy.unsigned_abs().pow(2)) as i64;
        if dist <= step || step <= 0 {
            if step > 0 {
                self.at = Some((tx, ty));
            }
            self.walk = None;
            return;
        }
        self.at = Some((x + dx * step / dist, y + dy * step / dist));
    }

    /// The tick on the player's own path (module doc): with a client DRLG
    /// whose active rooms hold the predicted cell, the path is placed
    /// there and requested for a new walk (or after a snap), then stepped
    /// one tick (`sim/pathing.md` §9.2); its position is the prediction
    /// and its stop ends the walk. `false`: no client path (no DRLG, no
    /// room, no tables), so the straight step runs.
    fn path_step(
        &mut self,
        world: &ClientWorld,
        speeds: Speeds,
        moves: MoveStats,
        walk: Walk,
        target: (u16, u16),
    ) -> bool {
        let (Some(at), Some(cell), Some(t)) = (self.at, self.cell(), path_tables()) else {
            return false;
        };
        let Some(drlg) = world.drlg.as_ref().map(|d| &d.drlg) else {
            return false;
        };
        let to = match walk.to {
            WalkTo::Point(x, y) => PathTo::Point(x, y),
            WalkTo::Unit(k) => {
                let Some(&ty) = d2_sim::units::UnitType::ALL.get(usize::from(k.unit_type)) else {
                    return false;
                };
                PathTo::Unit(ty, k.guid, target)
            }
        };
        // The model's stamina; only zero is read (a run starts as a walk).
        let stamina = world
            .local()
            .and_then(|p| p.stats.get(&10).copied())
            .unwrap_or(1);
        let own = Own { stamina, moves };
        if self.path_for != Some((walk, at)) {
            // A re-target while the path moves (no snap since its last
            // step) recomputes from the current precise position, the
            // fraction kept (`sim/pathing.md` §1.5, edge case 8;
            // `a1-walk-s`: re-placing at the cell centre drifted 3 px).
            // A walk from rest or after a snap places the path anew.
            let moving_here = self.path_for.is_some_and(|(_, a)| a == at);
            if !moving_here && !self.path.place(t, drlg, cell.0, cell.1) {
                self.path_for = None;
                return false;
            }
            if !self.path.request(t, drlg, speeds, own, to, walk.run) {
                // No path: the server's request stands still too.
                self.walk = None;
                self.path_for = None;
                return true;
            }
        }
        let moving = self.path.tick(t, drlg, speeds, own, Some(to));
        if let Some((x, y)) = self.path.position() {
            let now = (i64::from(x), i64::from(y));
            if let Some(d) = facing((at.0 as u32, at.1 as u32), (x, y)) {
                self.dir = Some(d);
            }
            self.at = Some(now);
        }
        if moving {
            self.path_for = self.at.map(|a| (walk, a));
        } else {
            self.walk = None;
            self.path_for = None;
        }
        true
    }

    /// Faces the walk's target from the predicted position (module doc);
    /// no walk, no target or already on it: the facing stays.
    fn face(&mut self, world: &ClientWorld) {
        let (Some((x, y)), Some(walk)) = (self.at, self.walk) else {
            return;
        };
        let target = match walk.to {
            WalkTo::Point(tx, ty) => Some((tx, ty)),
            WalkTo::Unit(k) => world.units.get(&k).and_then(|u| u.position),
        };
        if let Some(d) = target.and_then(|t| facing((x as u32, y as u32), cell_centre(t))) {
            self.dir = Some(d);
        }
    }

    /// One bridge frame: [`Self::observe`], [`Self::server_walk`], the
    /// walks sent since the last frame (last one wins), the facing toward
    /// the target, then [`Self::tick`] if the server ticked.
    pub fn frame(
        &mut self,
        world: &ClientWorld,
        walks: impl IntoIterator<Item = Walk>,
        ticked: bool,
        speeds: Speeds,
    ) {
        self.observe(world);
        self.server_walk(world);
        let mut new_walk = false;
        for w in walks {
            self.walk(w);
            new_walk = true;
        }
        // A walk to a point faces its target from where it started and
        // keeps that facing (`a1-walk-n`: 1.14d dir 32, the start subtile
        // (4873, 4228) to the clicked (4867, 4222); PROVISIONAL REC-517);
        // a walk to a unit follows the unit.
        if new_walk
            || matches!(
                self.walk,
                Some(Walk {
                    to: WalkTo::Unit(_),
                    ..
                })
            )
        {
            self.face(world);
        }
        if ticked {
            self.tick(world, speeds);
        }
        // The animation restarts with the mode (walk ↔ run, `a1-run-*`:
        // the run frames count from the first run click).
        self.since = match (self.mode(), self.since) {
            (None, _) => None,
            (Some(m), Some((t, was))) if was == m => Some((t, m)),
            (Some(m), _) => Some((world.server_ticks, m)),
        };
    }

    /// The server tick the walk under way started on (`sim/units.md`
    /// §4.7 step 7 revision: the walk frame counts from there).
    pub fn walk_since(&self) -> Option<u64> {
        self.since.map(|(t, _)| t)
    }

    /// The predicted precise position (16.16 sub-tiles, the form
    /// `rules::camera::moving_to_client` takes): the hook of the world
    /// view's camera and of the click conversion.
    pub fn position(&self) -> Option<(u32, u32)> {
        self.at.map(|(x, y)| (x as u32, y as u32))
    }

    /// The predicted sub-tile.
    pub fn cell(&self) -> Option<(u16, u16)> {
        self.at.map(|(x, y)| ((x >> 16) as u16, (y >> 16) as u16))
    }

    /// The local player the prediction is for.
    pub fn player(&self) -> Option<UnitKey> {
        self.player
    }

    /// The walk under way, if any.
    pub fn walking(&self) -> Option<Walk> {
        self.walk
    }

    /// The predicted facing `dir64` (0–63, module doc); `None` before the
    /// first walk. Stays after the walk ends (a standing unit keeps its
    /// direction).
    pub fn facing(&self) -> Option<u8> {
        self.dir
    }

    /// The player mode the view shows while the prediction moves: 2
    /// (walk) or 3 (run); `None`: the model's mode.
    pub fn mode(&self) -> Option<u32> {
        // A walk in a town room is the town walk 6 (`pathing.md` §1.5
        // r2), as the client path's mode request sets it.
        let town = self.path.mode() == 6;
        self.walk.map(|w| match (w.run && !self.exhausted, town) {
            (true, _) => 3,
            (false, true) => 6,
            (false, false) => 2,
        })
    }
}

/// The walks a [`PredictLink`] recorded, shared with whoever runs
/// [`Predict::frame`] (the play app boxes the link, so it cannot be
/// reached through the bridge's `link_mut`).
#[derive(Clone, Debug, Default)]
pub struct WalkTap(
    std::sync::Arc<std::sync::Mutex<Vec<Walk>>>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
);

impl WalkTap {
    /// The walks sent since the last call, in send order.
    pub fn take(&self) -> Vec<Walk> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn push(&self, w: Walk) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(w);
    }

    /// Whether a waypoint travel (C→S 0x49) was sent since the last call.
    pub fn take_waypoint(&self) -> bool {
        self.1.swap(false, std::sync::atomic::Ordering::Relaxed)
    }

    /// Records `msg` when it is a walk intent ([`walk_of`]) or a waypoint
    /// travel (C→S 0x49), as [`PredictLink`] does for what it sends: for a
    /// sender that reaches the server another way (a test rig's direct
    /// link).
    pub fn record(&self, msg: &[u8]) {
        if let Some(w) = walk_of(msg) {
            self.push(w);
        }
        if msg.first() == Some(&C2S_WAYPOINT) {
            self.1.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// A [`ServerLink`] that records the walk intents sent through it for
/// [`Predict::frame`]; everything else passes through unchanged.
pub struct PredictLink<L> {
    inner: L,
    walks: WalkTap,
}

impl<L> PredictLink<L> {
    pub fn new(inner: L) -> Self {
        Self {
            inner,
            walks: WalkTap::default(),
        }
    }

    /// The walks sent since the last call, in send order.
    pub fn take_walks(&mut self) -> Vec<Walk> {
        self.walks.take()
    }

    /// A handle on the recorded walks ([`WalkTap::take`] drains the same
    /// list as [`Self::take_walks`]).
    pub fn tap(&self) -> WalkTap {
        self.walks.clone()
    }

    pub fn inner(&self) -> &L {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut L {
        &mut self.inner
    }
}

impl<L: ServerLink> ServerLink for PredictLink<L> {
    fn protocol_version(&self) -> u32 {
        self.inner.protocol_version()
    }

    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        let sent = self.inner.send(queue, msg)?;
        self.walks.record(msg);
        Ok(sent)
    }

    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.inner.pump()
    }

    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.inner.receive()
    }
}

/// Pokes pass through (`state-dump` runs its bridge on a [`PredictLink`]).
impl<L: super::poke::PokeTarget> super::poke::PokeTarget for PredictLink<L> {
    type Error = L::Error;
    fn poke(&mut self, op: &d2_sim::poke::PokeOp) -> Result<d2_sim::poke::PokeResult, L::Error> {
        self.inner.poke(op)
    }
}

/// Scripted messages pass through (`state-dump --send`).
impl<L: super::inject::InjectTarget> super::inject::InjectTarget for PredictLink<L> {
    type Error = L::Error;
    fn inject(
        &mut self,
        msg: &conformance::scenario::script::StepMsg,
    ) -> Result<super::inject::Injected, L::Error> {
        self.inner.inject(msg)
    }
}

/// Snapshots pass through (`state-dump`).
impl<L: super::state::StateSource> super::state::StateSource for PredictLink<L> {
    type Error = L::Error;
    fn state_snapshot(&mut self) -> Result<d2_sim::debug::state::StateSnapshot, L::Error> {
        self.inner.state_snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::ClientUnit;

    // Covers: specs/render/unit-composite.md §3 r1
    /// A walk to a point faces the clicked subtile from the walk's start
    /// (`Predict::frame` aims it only when the walk arrives).
    #[test]
    fn a_point_walk_faces_the_click_from_its_start() {
        let mut p = Predict {
            at: Some(((4873 << 16) + 0x8000, (4228 << 16) + 0x8000)),
            ..Predict::default()
        };
        let w = ClientWorld {
            server_ticks: 22,
            ..ClientWorld::default()
        };
        let walk = Walk {
            to: WalkTo::Point(4867, 4222),
            run: false,
        };
        p.walk = Some(walk);
        p.face(&w);
        // `a1-walk-n`: 1.14d draws dir64 32 (start (4873, 4228) to the
        // clicked (4867, 4222)).
        assert_eq!(p.facing(), Some(32));
    }

    // Covers: specs/sim/pathing.md §1.5 r2
    /// The drawn mode of a predicted walk: the town walk 6 when the client
    /// path's mode request made it one (`a1-walk-n`: 1.14d draws
    /// `soshlittwhth`, d2rs drew mode 2 `wl`); a run stays 3 in town
    /// (`q-facts-scenes.md` finding 3).
    #[test]
    fn a_walk_in_town_is_drawn_in_the_town_walk_mode() {
        let to = WalkTo::Point(1, 2);
        let mode = |run, path| {
            Predict {
                walk: Some(Walk { to, run }),
                path: ClientPath::with_mode(path),
                ..Predict::default()
            }
            .mode()
        };
        assert_eq!(mode(false, 6), Some(6));
        assert_eq!(mode(false, 2), Some(2));
        assert_eq!(mode(true, 3), Some(3));
        assert_eq!(mode(true, 6), Some(3));
        assert_eq!(Predict::default().mode(), None);
    }

    // Synthetic fixture: charstats-shaped speeds (walk 6, run 9; the
    // values of `sim/pathing.md` case V1 / §8.2).
    const SPEEDS: Speeds = Speeds { walk: 6, run: 9 };

    fn world_at(x: u16, y: u16) -> (ClientWorld, UnitKey) {
        let key = UnitKey::new(PLAYER, 7);
        let mut w = ClientWorld::default();
        let mut u = ClientUnit::new(key);
        u.position = Some((x, y));
        u.server_point = (x, y);
        w.units.insert(key, u);
        w.local_player = Some(key);
        (w, key)
    }

    // Covers: specs/sim/pathing.md §8.1 r2, §8.2, §9.4 r2
    #[test]
    fn step_matches_the_velocity_rules() {
        assert_eq!(SPEEDS.step(false), 0x6000);
        // Run: p = 100 + (100·9/6 − 100) = 150 → velocity 0x900.
        assert_eq!(SPEEDS.step(true), 0x9000);
        assert_eq!(Speeds { walk: 0, run: 9 }.step(true), 0);
    }

    // Covers: specs/ui/controls.md §6 r7
    #[test]
    fn walk_of_reads_the_four_walk_codes() {
        assert_eq!(
            walk_of(&[1, 0x34, 0x12, 0x78, 0x56]),
            Some(Walk {
                to: WalkTo::Point(0x1234, 0x5678),
                run: false
            })
        );
        assert_eq!(walk_of(&[3, 1, 0, 2, 0]).map(|w| w.run), Some(true));
        assert_eq!(
            walk_of(&[4, 1, 0, 0, 0, 9, 0, 0, 0]),
            Some(Walk {
                to: WalkTo::Unit(UnitKey::new(1, 9)),
                run: true
            })
        );
        assert_eq!(walk_of(&[2, 0, 1, 0, 0, 9, 0, 0, 0]), None);
        assert_eq!(walk_of(&[5, 1, 0, 2, 0]), None);
        assert_eq!(walk_of(&[1, 1, 0]), None);
        assert_eq!(walk_of(&[]), None);
    }

    #[test]
    fn isqrt_floors() {
        for n in [0u64, 1, 2, 3, 4, 15, 16, 17, 1 << 40, u64::from(u32::MAX)] {
            let r = isqrt(n);
            assert!(r * r <= n && (r + 1) * (r + 1) > n, "{n}");
        }
    }

    // Covers: specs/sim/pathing.md §9.4 r2, §8.1 r2
    #[test]
    fn walks_straight_at_the_walk_speed_and_stops_on_the_target() {
        let (w, _) = world_at(100, 100);
        let mut p = Predict::new();
        p.frame(&w, [walk_point(105, 100, false)], false, SPEEDS);
        assert_eq!(p.position(), Some((0x64_8000, 0x64_8000)));
        for k in 1..=13 {
            p.frame(&w, [], true, SPEEDS);
            assert_eq!(p.position(), Some((0x64_8000 + k * 0x6000, 0x64_8000)));
        }
        p.frame(&w, [], true, SPEEDS);
        assert_eq!(p.position(), Some((0x69_8000, 0x64_8000)));
        assert_eq!(p.cell(), Some((105, 100)));
        assert_eq!(p.walking(), None);
        assert_eq!(p.mode(), None);
    }

    #[test]
    fn frames_without_a_tick_do_not_move() {
        let (w, _) = world_at(100, 100);
        let mut p = Predict::new();
        p.frame(&w, [walk_point(110, 100, true)], false, SPEEDS);
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.cell(), Some((100, 100)));
        assert_eq!(p.mode(), Some(3));
        p.frame(&w, [], true, SPEEDS);
        assert_eq!(p.position(), Some((0x64_8000 + 0x9000, 0x64_8000)));
    }

    #[test]
    fn diagonal_steps_keep_the_speed() {
        let (w, _) = world_at(100, 100);
        let mut p = Predict::new();
        p.frame(&w, [walk_point(120, 120, false)], true, SPEEDS);
        let (x, y) = p.position().unwrap();
        let (dx, dy) = (i64::from(x) - 0x64_8000, i64::from(y) - 0x64_8000);
        assert_eq!(dx, dy);
        // 0x6000 · d / ⌊√(2·d²)⌋ with d = 20 sub-tiles, truncated: 0x6000 / √2.
        let d = 20i64 << 16;
        let dist = isqrt(2 * (d as u64).pow(2)) as i64;
        assert_eq!(dx, 0x6000 * d / dist);
        assert_eq!(dx, 17377);
    }

    // Covers: specs/client/model.md §3 r3, §6 r3
    #[test]
    fn model_changes_snap_the_prediction() {
        let (mut w, key) = world_at(100, 100);
        let mut p = Predict::new();
        p.frame(&w, [walk_point(120, 100, false)], true, SPEEDS);
        p.frame(&w, [], true, SPEEDS);
        assert_eq!(p.position(), Some((0x65_4000, 0x64_8000)));
        // A kept check (rule 7) stores its point but does not move the
        // prediction.
        w.units.get_mut(&key).unwrap().server_point = (104, 101);
        let before = p.position();
        p.frame(&w, [], false, SPEEDS);
        assert_ne!(p.position(), Some((0x68_8000, 0x65_8000)));
        assert!(before.is_some());
        // A check that took the server's point (rule 8) snaps; the walk
        // goes on from there.
        w.units.get_mut(&key).unwrap().server_point = (104, 101);
        w.units.get_mut(&key).unwrap().follows += 1;
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.position(), Some((0x68_8000, 0x65_8000)));
        assert!(p.walking().is_some());
        // A placement (0x15) snaps to the new position.
        w.units.get_mut(&key).unwrap().position = Some((50, 60));
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.position(), Some((0x32_8000, 0x3C_8000)));
        // Nothing changed: no snap.
        p.frame(&w, [], true, SPEEDS);
        assert_ne!(p.position(), Some((0x32_8000, 0x3C_8000)));
    }

    /// A walk under way when the player changes level (a warp or portal
    /// placement) ends: its target is a point of the old level, which may
    /// lie hundreds of sub-tiles away in the new level's coordinates.
    #[test]
    fn a_level_change_ends_the_walk() {
        use super::super::world::ActiveRoom;
        use crate::bridge::drlg::DrlgRoomId;
        let (mut w, key) = world_at(20, 20);
        let room = |level: u16, id: u32, x0: i32| ActiveRoom {
            x0,
            y0: 0,
            w: 40,
            h: 40,
            level,
            room: DrlgRoomId(id),
        };
        w.active_rooms = Some(vec![room(3, 1, 0), room(1, 2, 1000)]);
        w.room_units.place(key, Some(DrlgRoomId(1)));
        let mut p = Predict::new();
        p.frame(&w, [walk_point(30, 20, false)], true, SPEEDS);
        assert!(p.walking().is_some());
        // The placement in level 1, far east.
        w.units.get_mut(&key).unwrap().position = Some((1010, 20));
        w.room_units.place(key, Some(DrlgRoomId(2)));
        p.frame(&w, [], true, SPEEDS);
        assert!(p.walking().is_none());
        assert_eq!(p.cell(), Some((1010, 20)));
    }

    /// Two 40 × 40 rooms: level 3 at x 0, level 1 at x 1000; the player
    /// in the first.
    fn two_levels(x: u16, y: u16) -> (ClientWorld, UnitKey) {
        use super::super::world::ActiveRoom;
        use crate::bridge::drlg::DrlgRoomId;
        let (mut w, key) = world_at(x, y);
        let room = |level: u16, id: u32, x0: i32| ActiveRoom {
            x0,
            y0: 0,
            w: 40,
            h: 40,
            level,
            room: DrlgRoomId(id),
        };
        w.active_rooms = Some(vec![room(3, 1, 0), room(1, 2, 1000)]);
        w.room_units.place(key, Some(DrlgRoomId(1)));
        (w, key)
    }

    /// The player's mode request `(code, x, y)` as the bridge stores it.
    fn request(w: &mut ClientWorld, key: UnitKey, code: u8, x: i32, y: i32) {
        let u = w.units.get_mut(&key).unwrap();
        u.last_mode_request = Some(super::super::world::ModeRequest {
            code,
            record: [x, y, 0, 0, 0, 0, 0],
        });
        u.mode_requests += 1;
    }

    // Covers: specs/client/model.md §8 r4; specs/sim/path-placement.md §12.2 r5; specs/world/objects.md §12 r11
    #[test]
    fn an_arrival_walk_out_starts_at_the_arrival_point() {
        use crate::bridge::drlg::DrlgRoomId;
        let (mut w, key) = two_levels(20, 20);
        let mut p = Predict::new();
        p.frame(&w, [], true, SPEEDS);
        // One receive: the warp's 0x0D code 1 to (1013, 23), then the 0x15
        // at the arrival point (1010, 20) in level 1.
        request(&mut w, key, 1, 1013, 23);
        w.units.get_mut(&key).unwrap().position = Some((1010, 20));
        w.room_units.place(key, Some(DrlgRoomId(2)));
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.walking().map(|w| w.to), Some(WalkTo::Point(1013, 23)));
        assert_eq!(p.cell(), Some((1010, 20)));
        for _ in 0..40 {
            p.frame(&w, [], true, SPEEDS);
        }
        assert_eq!(p.cell(), Some((1013, 23)));
        assert!(p.walking().is_none());
    }

    // Covers: specs/client/model.md §3 r3; specs/client/model.md §11 r3
    #[test]
    fn a_level_change_to_the_same_cell_moves_the_prediction() {
        use crate::bridge::drlg::DrlgRoomId;
        let (mut w, key) = two_levels(20, 20);
        let mut p = Predict::new();
        p.frame(&w, [walk_point(30, 20, false)], true, SPEEDS);
        for _ in 0..40 {
            p.frame(&w, [], true, SPEEDS);
        }
        assert_eq!(p.cell(), Some((30, 20)));
        // An act change places the player at (20, 20) again, now in the
        // other act's room at the same coordinates: the model position
        // does not change.
        w.act = Some(super::super::world::ActLoad {
            act: 4,
            init_seed: 0,
            town_level: 109,
            f8: 0,
        });
        w.room_units.place(key, Some(DrlgRoomId(2)));
        w.active_rooms.as_mut().unwrap()[1].x0 = 0;
        w.active_rooms.as_mut().unwrap()[0].x0 = 1000;
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.cell(), Some((20, 20)));
    }

    // Covers: specs/client/model.md §3 r3
    #[test]
    fn walking_across_a_level_border_is_no_placement() {
        use super::super::world::ActiveRoom;
        use crate::bridge::drlg::DrlgRoomId;
        let (mut w, key) = world_at(35, 20);
        let room = |level: u16, id: u32, x0: i32| ActiveRoom {
            x0,
            y0: 0,
            w: 40,
            h: 40,
            level,
            room: DrlgRoomId(id),
        };
        w.active_rooms = Some(vec![room(1, 1, 0), room(2, 2, 40)]);
        w.room_units.place(key, Some(DrlgRoomId(1)));
        let mut p = Predict::new();
        p.frame(&w, [walk_point(50, 20, false)], true, SPEEDS);
        while p.cell().is_some_and(|c| c.0 < 41) {
            p.frame(&w, [], true, SPEEDS);
        }
        // The view's room recache links the player to the next level's
        // room; the model position stays (35, 20).
        w.room_units.place(key, Some(DrlgRoomId(2)));
        let at = p.cell();
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.cell(), at);
    }

    // Covers: specs/client/model.md §8 r4
    #[test]
    fn a_walk_out_ahead_of_its_placement_is_held() {
        use crate::bridge::drlg::DrlgRoomId;
        let (mut w, key) = two_levels(20, 20);
        let mut p = Predict::new();
        p.frame(&w, [], true, SPEEDS);
        // The 0x0D arrives one frame before the 0x15: its target is in
        // the other level; nothing steps toward it from (20, 20).
        request(&mut w, key, 1, 1013, 23);
        p.frame(&w, [], true, SPEEDS);
        assert!(p.walking().is_none());
        assert_eq!(p.cell(), Some((20, 20)));
        w.units.get_mut(&key).unwrap().position = Some((1010, 20));
        w.room_units.place(key, Some(DrlgRoomId(2)));
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.walking().map(|w| w.to), Some(WalkTo::Point(1013, 23)));
    }

    /// The measured waypoint arrival (REC-288, `traces/client/model/
    /// client-0002.json`, Rogue Encampment → Cold Plains): after the
    /// client's C→S 0x49 the server's 0x0D walk to the arrival point
    /// (x + 3, y + 3) comes with the 0x15 at (5168, 4658); 1.14d takes the 0x15
    /// point and never walks on.
    // Covers: specs/client/model.md §8 r4; specs/world/waypoints.md §7 r7
    #[test]
    fn a_waypoint_arrival_is_not_walked_out() {
        use crate::bridge::drlg::DrlgRoomId;
        let (mut w, key) = two_levels(20, 20);
        w.active_rooms.as_mut().unwrap()[1].x0 = 5150;
        w.active_rooms.as_mut().unwrap()[1].y0 = 4640;
        let mut p = Predict::new();
        p.frame(&w, [], true, SPEEDS);
        let tap = WalkTap::default();
        tap.record(&[0x49, 1, 0, 0, 0, 3, 0, 0, 0]);
        assert!(tap.take_waypoint());
        assert!(!tap.take_waypoint(), "taken once");
        p.waypoint_sent();
        request(&mut w, key, 1, 5168 + 3, 4658 + 3);
        w.units.get_mut(&key).unwrap().position = Some((5168, 4658));
        w.room_units.place(key, Some(DrlgRoomId(2)));
        for _ in 0..40 {
            p.frame(&w, [], true, SPEEDS);
        }
        assert!(p.walking().is_none());
        assert_eq!(p.cell(), Some((5168, 4658)));
        // The next server walk (not a waypoint arrival) is held again.
        request(&mut w, key, 1, 5170, 4660);
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.walking().map(|w| w.to), Some(WalkTo::Point(5170, 4660)));
    }

    // Covers: specs/client/model.md §8 r4
    #[test]
    fn a_player_walk_or_another_request_drops_a_held_walk_out() {
        let (mut w, key) = two_levels(20, 20);
        let mut p = Predict::new();
        p.frame(&w, [], true, SPEEDS);
        request(&mut w, key, 1, 1013, 23);
        p.frame(&w, [walk_point(30, 20, false)], false, SPEEDS);
        assert_eq!(p.walking().map(|w| w.to), Some(WalkTo::Point(30, 20)));
        // Not a walk request: no server walk.
        request(&mut w, key, 1, 25, 25);
        request(&mut w, key, 0x19, 25, 25);
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.walking().map(|w| w.to), Some(WalkTo::Point(30, 20)));
        // The same request again is still a new one.
        request(&mut w, key, 1, 25, 25);
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.walking().map(|w| w.to), Some(WalkTo::Point(25, 25)));
        request(&mut w, key, 1, 25, 25);
        p.frame(&w, [], false, SPEEDS);
        assert_eq!(p.walking().map(|w| w.to), Some(WalkTo::Point(25, 25)));
    }

    #[test]
    fn no_local_player_no_prediction() {
        let mut p = Predict::new();
        let w = ClientWorld::default();
        p.frame(&w, [walk_point(1, 1, false)], true, SPEEDS);
        assert_eq!(p.position(), None);
        assert_eq!(p.walking(), None);
        let (mut w, key) = world_at(10, 10);
        p.observe(&w);
        assert_eq!(p.player(), Some(key));
        w.local_player = None;
        p.observe(&w);
        assert_eq!(p.player(), None);
    }

    #[test]
    fn unit_target_follows_the_unit() {
        let (mut w, _) = world_at(100, 100);
        let npc = UnitKey::new(1, 3);
        let mut u = ClientUnit::new(npc);
        u.position = Some((100, 101));
        w.units.insert(npc, u);
        let mut p = Predict::new();
        let to_npc = Walk {
            to: WalkTo::Unit(npc),
            run: false,
        };
        p.frame(&w, [to_npc], true, SPEEDS);
        p.frame(&w, [], true, SPEEDS);
        p.frame(&w, [], true, SPEEDS);
        assert_eq!(p.cell(), Some((100, 101)));
        assert_eq!(p.walking(), None);
        p.frame(&w, [to_npc], true, SPEEDS);
        w.units.remove(&npc);
        p.frame(&w, [], true, SPEEDS);
        assert_eq!(p.walking(), None);
    }

    // Covers: specs/sim/pathing.md §8.3
    #[test]
    fn facing_follows_the_direction_vector() {
        let c = |x: i32, y: i32| cell_centre(((100 + x) as u16, (100 + y) as u16));
        let o = c(0, 0);
        assert_eq!(facing(o, c(10, 0)), Some(56));
        assert_eq!(facing(o, c(0, -10)), Some(40));
        assert_eq!(facing(o, c(5, 5)), Some(0));
        assert_eq!(facing(o, c(3, 1)), Some(59));
        assert_eq!(facing(o, o), None);
    }

    // Covers: specs/sim/pathing.md §8.3
    #[test]
    fn facing_a_far_point_keeps_the_direction() {
        // A position from before a level change: thousands of sub-tiles.
        let c = |x: u16, y: u16| cell_centre((x, y));
        assert_eq!(facing(c(100, 100), c(5100, 100)), Some(56));
        assert_eq!(
            facing(c(5100, 5100), c(100, 5100)),
            facing(c(110, 100), c(100, 100))
        );
        assert_eq!(facing(c(100, 100), c(3100, 1100)), Some(59));
        assert_eq!(
            facing(c(9000, 9000), c(20, 20)),
            facing(c(110, 110), c(100, 100))
        );
    }

    // Covers: specs/sim/pathing.md §8.3, §8.4
    #[test]
    fn the_walk_sets_the_facing_and_it_stays_when_standing() {
        let (w, _) = world_at(100, 100);
        let mut p = Predict::new();
        p.observe(&w);
        assert_eq!(p.facing(), None);
        // Taken before any tick: faces the target at once.
        p.frame(&w, [walk_point(100, 90, false)], false, SPEEDS);
        assert_eq!(p.facing(), Some(40));
        for _ in 0..40 {
            p.frame(&w, [], true, SPEEDS);
        }
        assert_eq!((p.walking(), p.cell()), (None, Some((100, 90))));
        assert_eq!(p.facing(), Some(40), "kept after the walk ends");
        p.frame(&w, [walk_point(110, 90, true)], true, SPEEDS);
        assert_eq!(p.facing(), Some(56));
    }

    // Covers: specs/sim/pathing.md §9.9
    #[test]
    fn a_run_with_no_stamina_walks() {
        let (mut w, key) = world_at(100, 100);
        w.units.get_mut(&key).unwrap().stats.insert(10, 5 << 8);
        let mut p = Predict::new();
        p.observe(&w);
        p.walk(walk_point(110, 100, true));
        p.tick(&w, SPEEDS);
        assert_eq!(p.mode(), Some(3));
        let run_at = p.position().unwrap().0;
        w.units.get_mut(&key).unwrap().stats.insert(10, 0);
        p.tick(&w, SPEEDS);
        assert_eq!(p.mode(), Some(2));
        let walked = p.position().unwrap().0 - run_at;
        assert_eq!(walked, 6 * 0x1000);
    }

    // Covers: specs/seams/movement-prediction.md §2.5 r3; specs/client/model.md §6
    #[test]
    fn a_raw_stamina_below_256_walks_on_the_client() {
        // Server raw stamina 100 reaches the model as 0x96 stamina
        // 100 >> 8 = 0: the client walks while the server (raw > 0)
        // still runs. The gap is 1.14d's own.
        let raw: i32 = 100;
        let (mut w, key) = world_at(100, 100);
        w.units.get_mut(&key).unwrap().stats.insert(10, raw >> 8);
        let mut p = Predict::new();
        p.observe(&w);
        p.walk(walk_point(110, 100, true));
        p.tick(&w, SPEEDS);
        assert_eq!(p.mode(), Some(2));
    }

    fn walk_point(x: u16, y: u16, run: bool) -> Walk {
        Walk {
            to: WalkTo::Point(x, y),
            run,
        }
    }
}
