// Spec: specs/client/model.md (§3 r3, open question 2), specs/sim/pathing.md (§8.1–8.5, §9.4, cases M1, D1–D4), specs/ui/controls.md (§6 r7)
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
//! - each server tick the predicted position steps toward the target in a
//!   straight line at the charstats walk / run speed ([`Speeds`]);
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
//! d2rs-own, unverified. PROVISIONAL (client/model.md OQ2; REC-51): the
//! straight line (the server walks the path of `sim/pathing.md` §4), the
//! snap rule, the tick step and the facing (the client turns, §8.5) are
//! not 1.14d facts. Used only by the
//! `play` preview; the strict path never builds one.

use std::sync::OnceLock;

use d2_sim::path::tables::PathTables;
use d2_sim::path::walk::geom::direction_vector;

use super::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use super::world::{ClientWorld, UnitKey, PLAYER};

/// The direction `dir64` (0–63) from precise (16.16) point `from` toward
/// `to` (`sim/pathing.md` §8.3, `tan` table of `path-tables.tsv`, without
/// the path flag 0x200 flip); `None` when the points are equal (§8.4 r1:
/// no direction toward the point the unit is on) or the tables do not
/// parse.
pub fn facing(from: (u32, u32), to: (u32, u32)) -> Option<u8> {
    static TABLES: OnceLock<Option<PathTables>> = OnceLock::new();
    if from == to {
        return None;
    }
    let tables = TABLES.get_or_init(|| PathTables::spec().ok()).as_ref()?;
    Some(direction_vector(tables, from, to).1)
}

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
    /// velocity 0x600 moves 0x6000 a tick). Stats 67 / 96 from items and
    /// skills are not in the client model and are read as their creation
    /// values (d2rs-own, unverified).
    pub fn step(self, run: bool) -> i64 {
        let walk = i64::from(self.walk);
        let p = if run && walk != 0 {
            100 + (100 * i64::from(self.run) / walk - 100)
        } else {
            100
        };
        (walk * 256 * p / 100) * 16
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
    /// The model's (position, server point) at the last observation.
    seen: Option<((u16, u16), (u16, u16))>,
    /// Precise (16.16) predicted position.
    at: Option<(i64, i64)>,
    /// The walk under way.
    walk: Option<Walk>,
    /// The predicted facing `dir64`; kept when the walk ends.
    dir: Option<u8>,
    /// The local player's stamina (stat 10) is zero: the server walks
    /// instead of running (`pathing.md` §9.9, `units.md` §4.5), so the
    /// prediction does too. d2rs-own, unverified (client prediction).
    exhausted: bool,
}

impl Predict {
    pub fn new() -> Self {
        Self::default()
    }

    /// Snaps to the model (D2): a new local player starts at its
    /// position; a change of the model's position (0x15, a check
    /// correction) or server point (0x0F and the other checked unit
    /// messages, `client/model.md` §6 r3) moves the prediction to it,
    /// server point first. The walk target is kept. No local player, or
    /// one with no position: no prediction.
    pub fn observe(&mut self, world: &ClientWorld) {
        let Some(p) = world.local().filter(|p| p.key.unit_type == PLAYER) else {
            *self = Self::default();
            return;
        };
        let Some(pos) = p.position else {
            *self = Self::default();
            return;
        };
        let now = (pos, p.server_point);
        if self.player != Some(p.key) {
            *self = Self {
                player: Some(p.key),
                seen: Some(now),
                at: Some(centre(pos)),
                walk: None,
                dir: None,
                exhausted: false,
            };
            return;
        }
        let Some((old_pos, old_point)) = self.seen else {
            self.seen = Some(now);
            self.at = Some(centre(pos));
            return;
        };
        if p.server_point != old_point && p.server_point.0 != 0 && p.server_point.1 != 0 {
            self.at = Some(centre(p.server_point));
        } else if pos != old_pos {
            self.at = Some(centre(pos));
        }
        self.seen = Some(now);
    }

    /// A walk intent the client sent: the new target.
    pub fn walk(&mut self, walk: Walk) {
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
        let step = speeds.step(walk.run && !self.exhausted);
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

    /// One bridge frame: [`Self::observe`], the walks sent since the last
    /// frame (last one wins), the facing toward the target, then
    /// [`Self::tick`] if the server ticked.
    pub fn frame(
        &mut self,
        world: &ClientWorld,
        walks: impl IntoIterator<Item = Walk>,
        ticked: bool,
        speeds: Speeds,
    ) {
        self.observe(world);
        for w in walks {
            self.walk(w);
        }
        self.face(world);
        if ticked {
            self.tick(world, speeds);
        }
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
        self.walk
            .map(|w| if w.run && !self.exhausted { 3 } else { 2 })
    }
}

/// The walks a [`PredictLink`] recorded, shared with whoever runs
/// [`Predict::frame`] (the play app boxes the link, so it cannot be
/// reached through the bridge's `link_mut`).
#[derive(Clone, Debug, Default)]
pub struct WalkTap(std::sync::Arc<std::sync::Mutex<Vec<Walk>>>);

impl WalkTap {
    /// The walks sent since the last call, in send order.
    pub fn take(&self) -> Vec<Walk> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn push(&self, w: Walk) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(w);
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
        if let Some(w) = walk_of(msg) {
            self.walks.push(w);
        }
        Ok(sent)
    }

    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.inner.pump()
    }

    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.inner.receive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::ClientUnit;

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
        // A server point (0x0F check) snaps; the walk goes on from there.
        w.units.get_mut(&key).unwrap().server_point = (104, 101);
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

    fn walk_point(x: u16, y: u16, run: bool) -> Walk {
        Walk {
            to: WalkTo::Point(x, y),
            run,
        }
    }
}
