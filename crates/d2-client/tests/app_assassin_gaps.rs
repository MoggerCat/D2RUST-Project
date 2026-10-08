// Spec: specs/skills/bodies.md (§2.14, §8.8, §8.10), specs/monsters/ai-bodies-6.md (§14)
//! Assassin gaps in the play host (q-assassin-gaps), headless over the
//! synthetic single-player game: the finisher spends the charges, dual
//! claws strike twice, and a laid trap shoots its skill at a monster in
//! range until its shots are spent. Rows are test-local
//! (`// d2rs-own, unverified`); provisional notes: REC-233.

#[path = "app_skill_gaps/rig.rs"]
mod rig;

use std::sync::Arc;

use d2_client::app::weapons::{class, ItemFacts};
use d2_data::tables::Skills;
use d2_formats::animdata;
use d2_sim::bench_fixtures::combat as fx;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use rig::{Cfg, Rig};

const CHARGE: usize = 3;
const FINISH: usize = 4;
const CHARGE_STATE: usize = 8;
/// The claw item type of the tests (`itemtypes` row, made up) and `weap`.
const CLAW: i16 = 77;
const WEAP: i16 = 45;

fn row(srvst: u16, srvdo: u16) -> Skills {
    let mut s = fx::skill_rec();
    s.anim = 7;
    s.range = 1;
    s.srvstfunc = srvst;
    s.srvdofunc = srvdo;
    s.hitshift = 8;
    s.srcdam = 128;
    s
}

fn assassin(rows: Vec<(usize, Skills)>) -> Rig {
    let learned = rows.iter().map(|r| r.0).collect();
    let mut r = Rig::build(Cfg {
        class: "assassin",
        class_id: 6,
        token: b"AI",
        rows,
        learned,
        pgsv: vec![CHARGE_STATE],
    });
    r.leave_town();
    r.with(|sim, _| sim.events.action.sys.hooks.x.monsters.modes = vec![0xFFFF]);
    r
}

fn claw() -> ItemFacts {
    ItemFacts {
        types: vec![CLAW, WEAP],
        class: class::ONE_HAND_SWING,
        ..ItemFacts::default()
    }
}

/// Puts a claw in the right hand (`right`) or the left.
fn hold(r: &mut Rig, right: bool) -> UnitId {
    r.with(move |sim, p| {
        let req = AllocRequest {
            ty: UnitType::Item,
            class: 0,
            room: None,
            add: false,
            fixed_guid: None,
            mode: 1,
            allied: false,
        };
        let item = sim
            .events
            .action
            .with(&mut sim.game, |g, v| v.allocate(g, &req, 0, 0))
            .expect("an item unit");
        // The weapon copy is set by hand; the synthetic game's inventory
        // model would replace it at the next sync (q-a4-quest-items).
        sim.world.inventory = None;
        let w = &mut sim.events.action.sys.hooks.x.weapons;
        let h = w.hands.entry(p).or_default();
        if right {
            h.right = Some(item);
            h.weapon = Some(item);
        } else {
            h.left = Some(item);
        }
        w.items.insert(item, claw());
        item
    })
}

fn charge_skill() -> Skills {
    let mut s = row(23, 34);
    s.aurastate = CHARGE_STATE as u16;
    s.aurastat1 = 10;
    s.itypea1 = CLAW as u16;
    // The finisher this charge feeds: a strike of the same skill.
    s.srvprgfunc1 = 34;
    s.srvprgfunc2 = 34;
    s.srvprgfunc3 = 34;
    s
}

fn plain_attack_row() -> Skills {
    row(1, 1)
}

// Covers: specs/skills/bodies.md §2.14, §8.10
// (the finisher after a charge: extra strike, charge state removed)
#[test]
fn a_finisher_spends_the_charge() {
    let mut r = assassin(vec![(CHARGE, charge_skill()), (FINISH, plain_attack_row())]);
    hold(&mut r, true);
    let me = r.player();
    let m = r.spawn_monster(1);
    tough(&mut r, m);
    r.select_right(CHARGE);
    r.right_click_unit(m);
    r.step(30);
    assert!(
        r.state_on(me, CHARGE_STATE as i16),
        "charged ({})",
        r.errors()
    );
    let life0 = r.life(m);
    r.select_right(FINISH);
    r.right_click_unit(m);
    r.step(30);
    assert!(r.life(m) < life0, "the finisher hurt it ({})", r.errors());
    assert!(
        !r.state_on(me, CHARGE_STATE as i16),
        "the charge is spent ({})",
        r.errors()
    );
}

/// The attack animation fires its event twice (frames 3 and 5).
fn two_events(r: &mut Rig) {
    r.with(|sim, _| {
        let mut a = (**sim.events.action.sys.hooks.anim_data.as_ref().unwrap()).clone();
        for b in &mut a.buckets {
            for rec in b.iter_mut() {
                if rec.events[3] == 1 {
                    rec.events[5] = 1;
                }
            }
        }
        sim.events.action.sys.hooks.anim_data = Some(Arc::new(a));
    });
}

fn claws_strike(both: bool) -> i32 {
    let mut s = row(23, 35);
    s.aurastate = CHARGE_STATE as u16;
    s.aurastat1 = 10;
    s.itypea1 = CLAW as u16;
    let mut r = assassin(vec![(CHARGE, s)]);
    hold(&mut r, true);
    if both {
        hold(&mut r, false);
    }
    two_events(&mut r);
    let m = r.spawn_monster(1);
    tough(&mut r, m);
    r.select_right(CHARGE);
    let life0 = r.life(m);
    r.right_click_unit(m);
    r.step(40);
    life0 - r.life(m)
}

// Covers: specs/skills/bodies.md §8.10
// (dual claws: the second claw strikes on the next frame event)
#[test]
fn dual_claws_strike_twice() {
    let one = claws_strike(false);
    let two = claws_strike(true);
    assert!(one > 0, "a single claw strikes");
    assert!(two > one, "two claws hurt more: {two} vs {one}");
}

/// A monster that survives the strong player of the rig.
fn tough(r: &mut Rig, m: UnitId) {
    r.with(move |sim, _| {
        sim.events.action.with(&mut sim.game, |_, v| {
            v.set_base(m, d2_sim::stats::stat::MAXHP, 100_000 << 8);
            v.set_base(m, d2_sim::stats::stat::HITPOINTS, 100_000 << 8);
        });
    });
}

const TRAP: usize = 3;
const SHOT: usize = 4;
/// The trap's monster class (made up: a copy of class 0).
const TRAP_CLASS: u16 = 1;
const SHOTS: i32 = 3;

/// A game with a trap skill (srvdo 45) whose monster shoots `SHOT`.
fn trap_game() -> Rig {
    let mut lay = row(0, 45);
    lay.summon = TRAP_CLASS;
    lay.summode = 1;
    lay.pettype = 2;
    lay.anim = 10;
    lay.calc4 = rig::CALC_3;
    let shot = row(1, 1);
    let mut r = assassin(vec![(TRAP, lay), (SHOT, shot)]);
    r.with(|sim, _| {
        let h = &mut sim.events.action.sys.hooks;
        let mut t = (*h.tables).clone();
        let (m, m2) = (t.combat.monstats[0].clone(), t.combat.monstats2[0].clone());
        let mut m = m;
        m.skill1 = SHOT as u16;
        m.monstatsex = 1;
        t.combat.monstats.push(m);
        t.combat.monstats2.push(m2);
        t.skill_modes.push([4; 8]);
        h.tables = Arc::new(t);
        h.x.monsters.modes = vec![0xFFFF, 0xFFFF];
        let mut looks = (**h.x.looks.as_ref().unwrap()).clone();
        let row0 = looks.monsters[&0];
        looks.monsters.insert(1, row0);
        h.x.looks = Some(Arc::new(looks));
        // The trap's attack animation: 8 frames, the attack event on 3.
        let mut a = (**h.anim_data.as_ref().unwrap()).clone();
        let mut name = [0u8; 8];
        name[..7].copy_from_slice(b"ZOA1HTH");
        let mut events = [0u8; animdata::EVENTS];
        events[3] = 1;
        a.buckets[animdata::hash(&name[..7])].push(animdata::AnimRecord {
            name,
            frames: 8,
            speed: 256,
            events,
        });
        h.anim_data = Some(Arc::new(a));
        let d = &mut sim.events.action.sys.data;
        let info = d.monsters[0];
        d.monsters.push(info);
    });
    r
}

fn traps(r: &mut Rig) -> Vec<UnitId> {
    r.with(|sim, _| {
        sim.events
            .action
            .sys
            .hooks
            .sentries
            .keys()
            .copied()
            .collect()
    })
}

fn shots_left(r: &mut Rig) -> Option<i32> {
    r.with(|sim, _| {
        sim.events
            .action
            .sys
            .hooks
            .sentries
            .values()
            .next()
            .map(|s| s.shots)
    })
}

// Covers: specs/monsters/ai-bodies-6.md §14
// (a trap with no monster in range keeps its shots)
#[test]
fn a_trap_without_a_target_holds_its_shots() {
    let mut r = trap_game();
    r.select_right(TRAP);
    r.right_click_point(5, 0);
    r.step(40);
    assert_eq!(traps(&mut r).len(), 1, "a trap is laid ({})", r.errors());
    assert_eq!(shots_left(&mut r), Some(SHOTS), "no shot without a target");
}

// Covers: specs/monsters/ai-bodies-6.md §14
// Covers: specs/skills/bodies.md §8.3
// (the trap shoots a monster in range, spends a shot each time, and dies
// after the last)
#[test]
fn a_trap_shoots_in_range_then_dies() {
    let mut r = trap_game();
    r.select_right(TRAP);
    r.right_click_point(5, 0);
    r.step(40);
    let trap = traps(&mut r)[0];
    let m = r.spawn_monster(6);
    r.with(move |sim, _| {
        let a = &mut sim.events.action;
        a.with(&mut sim.game, |_, v| {
            // The trap's strength (`base_stats` has no `monlvl` row here).
            v.set_base(trap, 12, 30);
            v.set_base(trap, 19, 1000);
            v.set_base(trap, 21, 20);
            v.set_base(trap, 22, 30);
            v.set_base(m, d2_sim::stats::stat::MAXHP, 1000 << 8);
            v.set_base(m, d2_sim::stats::stat::HITPOINTS, 1000 << 8);
        });
    });
    let life0 = r.life(m);
    r.step(200);
    let errors = r.errors();
    assert!(r.life(m) < life0, "the trap hurt it ({errors})");
    assert!(
        traps(&mut r).is_empty(),
        "the trap is spent and gone ({errors})"
    );
}

/// A game whose trap shoots a missile skill (`srvmissile` row 0) on the
/// missile event (2) of its attack animation.
fn missile_trap_game() -> Rig {
    let mut lay = row(0, 45);
    lay.summon = TRAP_CLASS;
    lay.summode = 1;
    lay.pettype = 2;
    lay.anim = 10;
    lay.calc4 = rig::CALC_3;
    let mut shot = row(1, 0);
    shot.srvmissile = 0;
    shot.anim = 7;
    let mut r = assassin(vec![(TRAP, lay), (SHOT, shot)]);
    r.with(|sim, _| {
        let h = &mut sim.events.action.sys.hooks;
        let mut t = (*h.tables).clone();
        let (m, m2) = (t.combat.monstats[0].clone(), t.combat.monstats2[0].clone());
        let mut m = m;
        m.skill1 = SHOT as u16;
        m.monstatsex = 1;
        t.combat.monstats.push(m);
        t.combat.monstats2.push(m2);
        t.skill_modes.push([4; 8]);
        // The lightning bolt: fast and long (the arrow row crawls).
        // d2rs-own, unverified: not the real `missiles.txt` row.
        t.missiles[0].vel = 16;
        t.missiles[0].maxvel = 16;
        t.missiles[0].range = 200;
        // A spell bolt: no to-hit roll (`ToHit` 0), so whether it hits does
        // not hang on the trap's seed, which any other unit's creation moves
        // (the arrow row's `ToHit` 1 missed once the synthetic town had one
        // more NPC). d2rs-own, unverified.
        t.missiles[0].tohit = 0;
        h.tables = Arc::new(t);
        h.x.monsters.modes = vec![0xFFFF, 0xFFFF];
        let mut looks = (**h.x.looks.as_ref().unwrap()).clone();
        let row0 = looks.monsters[&0];
        looks.monsters.insert(1, row0);
        h.x.looks = Some(Arc::new(looks));
        let mut a = (**h.anim_data.as_ref().unwrap()).clone();
        let mut name = [0u8; 8];
        name[..7].copy_from_slice(b"ZOA1HTH");
        let mut events = [0u8; animdata::EVENTS];
        events[3] = 2;
        a.buckets[animdata::hash(&name[..7])].push(animdata::AnimRecord {
            name,
            frames: 8,
            speed: 256,
            events,
        });
        h.anim_data = Some(Arc::new(a));
        let d = &mut sim.events.action.sys.data;
        let info = d.monsters[0];
        d.monsters.push(info);
    });
    r
}

// Covers: specs/monsters/ai-bodies-6.md §14
// Covers: specs/skills/bodies.md §5
// (a laid trap fires its missile: shots drop, the missile flies and hits)
#[test]
fn a_lightning_sentry_fires_its_missile() {
    let mut r = missile_trap_game();
    r.select_right(TRAP);
    r.right_click_point(5, 0);
    r.step(40);
    let trap = traps(&mut r)[0];
    let m = r.spawn_monster(6);
    r.with(move |sim, _| {
        let a = &mut sim.events.action;
        a.with(&mut sim.game, |g, v| {
            v.set_base(trap, 12, 30);
            v.set_base(trap, 19, 1000);
            v.set_base(trap, 21, 20);
            v.set_base(trap, 22, 30);
            v.set_base(m, d2_sim::stats::stat::MAXHP, 1000 << 8);
            v.set_base(m, d2_sim::stats::stat::HITPOINTS, 1000 << 8);
            // The synthetic monster has no path footprint: stamp the
            // unit-collision bit its sub-tile would carry (d2rs-own).
            let (x, y) = v.h.path_position(m);
            let room = g.lists.unit(m).and_then(|e| e.room());
            d2_sim::path::footprint::stamp_size(&mut v.h.drlg, room, x, y, 1, 0x100);
        });
    });
    let life0 = r.life(m);
    let mut flew = 0;
    let mut farthest = 0;
    for _ in 0..60 {
        r.step(1);
        // The host has no missile damage setup yet (`0x0059F900`, skills
        // spec): the missile gets its damage here, as the e2e tests do.
        let (n, x) = r.with(|sim, _| {
            let ms = sim.game.lists.units_of_type(UnitType::Missile);
            let mut x = 0;
            sim.events.action.with(&mut sim.game, |_, v| {
                for &mi in &ms {
                    v.set_base(mi, 21, 2560);
                    v.set_base(mi, 22, 2560);
                    x = x.max(v.h.path_position(mi).0);
                }
            });
            (ms.len(), x)
        });
        flew = flew.max(n);
        farthest = farthest.max(x);
    }
    let errors = r.errors();
    assert!(flew > 0, "a missile flew ({errors})");
    assert!(
        shots_left(&mut r).is_none_or(|s| s < SHOTS),
        "a shot was spent ({errors})"
    );
    assert!(farthest <= 27, "the bolt ended at the monster ({farthest})");
    assert!(r.life(m) < life0, "the missile hit ({errors})");
}
