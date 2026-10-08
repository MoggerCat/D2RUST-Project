// Spec: specs/skills/bodies.md (§3.7, §7.6, §8.11), specs/skills/bodies-2.md (§3.3, §3.4), specs/skills/use.md (§5)
//! Skill gaps of the Paladin, Necromancer and Assassin in the play host
//! (q-skill-gaps), headless over the synthetic single-player game: the
//! player leaves the camp (a monster in a town room is no target and
//! takes no damage), a monster / corpse stands beside, and the right
//! skill is used through C→S 0x0D / 0x0C. Each test asserts an outcome
//! (damage, state, pet or missile). Rows are test-local
//! (`// d2rs-own, unverified`); provisional notes: REC-176.

#[path = "app_skill_gaps/rig.rs"]
mod rig;

use d2_client::app::weapons::{class, ItemFacts};
use d2_data::tables::Skills;
use d2_sim::bench_fixtures::combat as fx;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use rig::{Cfg, Rig};

const SKILL: usize = 3;

/// A skill row that starts at once, costs nothing and works in the camp.
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

fn game(class: &'static str, rows: Vec<(usize, Skills)>) -> Rig {
    let (class_id, token): (i32, &'static [u8; 2]) = match class {
        "paladin" => (3, b"PA"),
        "necromancer" => (2, b"NE"),
        _ => (6, b"AI"),
    };
    let learned = rows.iter().map(|r| r.0).collect();
    let mut r = Rig::build(Cfg {
        class,
        class_id,
        token,
        rows,
        learned,
        pgsv: Vec::new(),
        setup: None,
    });
    r.leave_town();
    r
}

/// Gives the player an item in the right hand (else the left).
fn hold(r: &mut Rig, right: bool, facts: ItemFacts) -> UnitId {
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
        let w = &mut sim.events.action.sys.hooks.x.weapons;
        let h = w.hands.entry(p).or_default();
        if right {
            h.right = Some(item);
            h.weapon = Some(item);
        } else {
            h.left = Some(item);
        }
        w.items.insert(item, facts);
        item
    })
}

// Covers: specs/skills/bodies-2.md §3.4 (Smite with a shield)
#[test]
fn smite_with_a_shield_hurts_the_monster() {
    let mut r = game("paladin", vec![(SKILL, row(0, 150))]);
    hold(
        &mut r,
        false,
        ItemFacts {
            shield: true,
            dam: (20, 30),
            ..ItemFacts::default()
        },
    );
    let m = r.spawn_monster(1);
    r.select_right(SKILL);
    let life0 = r.life(m);
    r.right_click_unit(m);
    r.step(30);
    assert!(r.life(m) < life0, "Smite hurt it ({})", r.errors());
}

// Covers: specs/skills/bodies-2.md §3.4 step 3.1 (no shield: no hit)
#[test]
fn smite_without_a_shield_does_nothing() {
    let mut r = game("paladin", vec![(SKILL, row(0, 150))]);
    let m = r.spawn_monster(1);
    r.select_right(SKILL);
    let life0 = r.life(m);
    r.right_click_unit(m);
    r.step(30);
    assert_eq!(r.life(m), life0, "{}", r.errors());
}

// Covers: specs/skills/bodies.md §3.7, bodies-2.md §3.3
#[test]
fn sacrifice_hurts_the_monster_and_the_caster() {
    let mut s = row(29, 64);
    s.calc2 = rig::CALC_8;
    let mut r = game("paladin", vec![(SKILL, s)]);
    let p = r.player();
    let m = r.spawn_monster(1);
    r.select_right(SKILL);
    let (life0, mine0) = (r.life(m), r.life(p));
    r.right_click_unit(m);
    r.step(30);
    let errors = r.errors();
    assert!(r.life(m) < life0, "the monster is hurt ({errors})");
    assert!(r.life(p) < mine0, "the caster is hurt ({errors})");
}

// Covers: specs/skills/bodies.md §7.6, §8.11
#[test]
fn zeal_strikes_the_monster() {
    let mut s = row(37, 13);
    s.calc1 = rig::CALC_3;
    let mut r = game("paladin", vec![(SKILL, s)]);
    hold(
        &mut r,
        true,
        ItemFacts {
            class: class::ONE_HAND_SWING,
            ..ItemFacts::default()
        },
    );
    let m = r.spawn_monster(1);
    r.select_right(SKILL);
    let life0 = r.life(m);
    r.right_click_unit(m);
    r.step(60);
    assert!(r.life(m) < life0, "Zeal hurt it ({})", r.errors());
}
