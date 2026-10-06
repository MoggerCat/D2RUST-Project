// Spec: specs/missiles/missiles.md (§R2, §R4, §R5, §R6, §R7; Test vectors), specs/combat/hit.md (Test vectors), specs/drlg/rooms.md §10
//! Missiles ↔ combat, units and the DRLG: a missile created by the real
//! creation code, run every frame by the unit dispatch, flown through a
//! real room's collision grid, hitting a real unit through the real
//! to-hit test and damage application.

use super::*;
use crate::drlg::collision::bits;
use crate::missiles::{create_missile, param_flags, unit_flag, MissileParams};
use crate::stats::stat as st;

const TOHIT: u16 = 19;
const LEVEL_STAT: u16 = 12;
const ARMORCLASS: u16 = 31;
const MINDAMAGE: u16 = 21;
const MAXDAMAGE: u16 = 22;

struct Shot {
    fx: Fx,
    owner: UnitId,
    target: UnitId,
}

/// A monster owner at (10, 10) (AR 300, level 10) and a player at
/// (13, 10) (defense 100, level 8, life 100): to-hit chance 83
/// (`hit.md` vector). The player's flags 2 and 3 (`missiles.md` §R4.2)
/// and its collision bit are what the unwritten kind init and movement
/// specs will set.
fn shot() -> Shot {
    let mut fx = Fx::new();
    let owner = fx.spawn(UnitType::Monster, 0, fx.a, 10, 10);
    let target = fx.spawn(UnitType::Player, 0, fx.a, 13, 10);
    fx.stats(owner, &[(TOHIT, 300), (LEVEL_STAT, 10)]);
    fx.stats(
        target,
        &[
            (ARMORCLASS, 100),
            (LEVEL_STAT, 8),
            (st::MAXHP, 25600),
            (st::HITPOINTS, 25600),
        ],
    );
    fx.sim.sys.units.get_mut(target).unwrap().flags |=
        unit_flag::IS_VALID_TARGET | unit_flag::CAN_BE_ATTACKED;
    fx.mark(target, bits::PLAYER);
    Shot { fx, owner, target }
}

impl Shot {
    /// Missile 0 from the owner toward (tx, 10), damage 10 points.
    fn fire(&mut self, tx: i32) -> UnitId {
        let p = MissileParams {
            owner: Some(self.owner),
            origin: Some(self.owner),
            class: 0,
            flags: param_flags::TARGET_ABSOLUTE,
            target_x: tx,
            target_y: 10,
            ..MissileParams::default()
        };
        let m = self
            .fx
            .sim
            .missiles(&mut self.fx.game, |g, cx| create_missile(g, cx, &p))
            .unwrap()
            .expect("created");
        // The damage setup `0x0059F900` is the skills spec's (pending):
        // the missile's damage stats are set here.
        self.fx.stats(m, &[(MINDAMAGE, 2560), (MAXDAMAGE, 2560)]);
        m
    }

    fn alive(&mut self, m: UnitId) -> bool {
        let in_store = self.fx.sim.hooks().missile_store().get(m).is_some();
        let in_lists = self.fx.game.lists.unit(m).is_some();
        assert_eq!(in_store, in_lists);
        assert_eq!(in_lists, self.fx.sim.sys.units.get(m).is_some());
        in_lists
    }
}

#[test]
fn missile_flies_hits_a_real_unit_and_deals_real_damage() {
    let mut s = shot();
    let (owner, target) = (s.owner, s.target);
    s.fx.seed(owner, seed_giving(10));
    let m = s.fire(13);
    // Allocation (`units.md` §3.1): GUID, unit seed from the game seed,
    // the every-tick type-0 event (`tick.md` §5.3).
    assert_eq!(s.fx.sim.hooks().x.position(m), (10, 10));
    assert_eq!(s.fx.timers(m), [(0, -1)]);
    let frames = s.fx.sim.hooks().missile_store().get(m).unwrap().total;
    assert_eq!(frames, 50);
    // Runs 1 and 2 (frames 1, 2): one sub-tile each, empty grid.
    s.fx.frame();
    s.fx.frame();
    assert!(s.alive(m));
    assert_eq!(s.fx.sim.hooks().x.position(m), (12, 10));
    assert_eq!(s.fx.stat(target, st::HITPOINTS), 25600);
    // Run 3: the player's sub-tile; to-hit draw 10 < 83 on the owner's
    // seed; damage 2560 (equal bounds: no draw); collide-kill → removed.
    s.fx.frame();
    assert!(!s.alive(m));
    assert!(s.fx.game.timers.unit_timers(m).is_empty());
    assert_eq!(s.fx.stat(target, st::HITPOINTS), 25600 - 2560);
    let mut want = seed_giving(10);
    want.step();
    assert_eq!(s.fx.sim.sys.units.get(owner).unwrap().seed, want);
    let t = target.0;
    assert_eq!(
        s.fx.sim.hooks().x.log,
        [
            format!("event 0 Some({t})"),
            format!("event 11 Some({t})"),
            format!("event 2 Some({t})"),
            format!("reaction {} {t} 0x1", owner.0),
        ]
    );
    s.fx.assert_clean();
}

#[test]
fn missed_to_hit_removes_the_missile_without_damage() {
    let mut s = shot();
    s.fx.seed(s.owner, seed_giving(90));
    let m = s.fire(13);
    for _ in 0..3 {
        s.fx.frame();
    }
    assert!(!s.alive(m));
    assert_eq!(s.fx.stat(s.target, st::HITPOINTS), 25600);
    assert_eq!(
        s.fx.sim.hooks().x.log,
        [format!("event 0 Some({})", s.target.0)]
    );
    s.fx.assert_clean();
}

#[test]
fn missile_barrier_in_the_room_grid_removes_without_a_hit() {
    // Room A's tile column 4 is "fill LOS": collision 0x4 on sub-tiles
    // x = 20..24 (`rooms.md` §10.4 step 4). §R4 step 6: removed without
    // the hit handler on the run that enters it.
    let mut s = shot();
    let game = &s.fx.game;
    let a = s.fx.a;
    let grid = |x| s.fx.sim.sys.hooks.drlg.collision(game, a, x, 20);
    assert_eq!(grid(BARRIER_X - 1), Some(0));
    assert_eq!(grid(BARRIER_X), Some(bits::MISSILE_BARRIER));
    // Fly along y = 20, away from the player.
    s.fx.sim.hooks().x.place(s.owner, 10, 20);
    let p = MissileParams {
        owner: Some(s.owner),
        origin: Some(s.owner),
        class: 0,
        flags: param_flags::TARGET_ABSOLUTE,
        target_x: 30,
        target_y: 20,
        ..MissileParams::default()
    };
    let m =
        s.fx.sim
            .missiles(&mut s.fx.game, |g, cx| create_missile(g, cx, &p))
            .unwrap()
            .unwrap();
    let mut runs = 0;
    while s.alive(m) {
        s.fx.frame();
        runs += 1;
        assert!(runs < 60);
    }
    assert_eq!(runs, BARRIER_X - 10);
    assert!(s.fx.sim.hooks().x.log.is_empty());
    s.fx.assert_clean();
}

#[test]
fn missile_expires_after_range_runs() {
    // `missiles.md` §R7 rule 2: N = Range = 50 frames → removed on run 50
    // (expiry hit, no unit, no damage). Flown inside room B's open floor.
    let mut s = shot();
    s.fx.sim.hooks().x.place(s.owner, 41, 30);
    let p = MissileParams {
        owner: Some(s.owner),
        origin: Some(s.owner),
        class: 0,
        flags: param_flags::TARGET_ABSOLUTE,
        target_x: 79,
        target_y: 30,
        ..MissileParams::default()
    };
    let m =
        s.fx.sim
            .missiles(&mut s.fx.game, |g, cx| create_missile(g, cx, &p))
            .unwrap()
            .unwrap();
    let mut runs = 0;
    while s.alive(m) {
        s.fx.frame();
        runs += 1;
        assert!(runs < 60);
    }
    assert_eq!(runs, 50);
    s.fx.assert_clean();
}
