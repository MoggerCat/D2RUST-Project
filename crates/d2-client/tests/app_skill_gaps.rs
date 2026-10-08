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
    });
    r.leave_town();
    // The monster class has every mode (the walk test of the curses).
    r.with(|sim, _| sim.events.action.sys.hooks.x.monsters.modes = vec![0xFFFF]);
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
        w.items.insert(item, facts);
        item
    })
}

// Covers: specs/skills/bodies-2.md §3.4
// (Smite with a shield)
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

// Covers: specs/skills/bodies-2.md §3.4
// (step 3.1: no shield, no hit)
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

// Covers: specs/skills/bodies.md §3.7
// Covers: specs/skills/bodies-2.md §3.3
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

/// Edits the combat tables of the running sim.
fn edit_tables(
    r: &mut Rig,
    f: impl FnOnce(&mut d2_sim::wiring::action::ActionTables) + Send + 'static,
) {
    r.with(move |sim, _| {
        let h = &mut sim.events.action.sys.hooks;
        let mut t = (*h.tables).clone();
        f(&mut t);
        h.tables = std::sync::Arc::new(t);
    });
}

/// The corpse test needs a dead monster class that may be raised.
fn corpse_class(r: &mut Rig) {
    edit_tables(r, |t| {
        t.combat.monstats2[0].corpsesel = true;
        t.combat.monstats2[0].revive = true;
        t.combat.monstats2[0].isatt = true;
        t.combat.monstats[0].switchai = true;
    });
}

/// A dead monster (mode 12) `dx` sub-tiles east of the player.
fn spawn_corpse(r: &mut Rig, dx: i32) -> UnitId {
    let m = r.spawn_monster(dx);
    r.with(move |sim, _| {
        sim.events.action.sys.units.get_mut(m).unwrap().mode = 12;
    });
    m
}

fn monsters(r: &mut Rig) -> usize {
    r.with(|sim, _| sim.game.lists.units_of_type(UnitType::Monster).len())
}

/// The curse row: `state` on the target, a 500-tick duration, range 500.
fn curse(state: u16) -> Skills {
    let mut s = row(0, 30);
    s.anim = 10;
    s.auratargetstate = state;
    s.aurastat1 = 0xFFFF;
    s.auralencalc = 4;
    s.aurarangecalc = 4;
    s.aurafilter = 0x583;
    s
}

// Covers: specs/skills/bodies.md §4.4
#[test]
fn a_curse_puts_its_state_on_the_monster() {
    let mut r = game("necromancer", vec![(SKILL, curse(9))]);
    corpse_class(&mut r);
    let m = r.spawn_monster(3);
    r.select_right(SKILL);
    assert!(!r.has_state(m, 9));
    r.right_click_point(3, 0);
    r.step(30);
    assert!(r.has_state(m, 9), "cursed ({})", r.errors());
}

// Covers: specs/skills/bodies-2.md §4.5
#[test]
fn corpse_explosion_hurts_the_monsters_around_the_corpse() {
    let mut s = row(17, 55);
    s.anim = 10;
    s.targetcorpse = true;
    s.aurarangecalc = rig::CALC_8;
    s.aurafilter = 0x583;
    (s.calc1, s.calc2) = (rig::CALC_100, rig::CALC_100);
    let mut r = game("necromancer", vec![(SKILL, s)]);
    corpse_class(&mut r);
    let corpse = spawn_corpse(&mut r, 2);
    let near = r.spawn_monster(3);
    r.select_right(SKILL);
    let life0 = r.life(near);
    r.right_click_unit(corpse);
    r.step(30);
    assert!(r.life(near) < life0, "the blast hurt it ({})", r.errors());
}

// Covers: specs/skills/bodies.md §3.6, §8.14
#[test]
fn raise_skeleton_turns_a_corpse_into_a_pet() {
    let mut s = row(15, 31);
    s.anim = 10;
    s.targetcorpse = true;
    (s.summon, s.summode, s.pettype) = (0, 1, 2);
    s.petmax = rig::CALC_3;
    let mut r = game("necromancer", vec![(SKILL, s)]);
    corpse_class(&mut r);
    let corpse = spawn_corpse(&mut r, 2);
    let before = monsters(&mut r);
    r.select_right(SKILL);
    r.right_click_unit(corpse);
    r.step(30);
    assert!(
        r.pets() > 0,
        "a skeleton follows ({}, {before})",
        r.errors()
    );
}

// Covers: specs/skills/bodies-2b.md §8.6, §8.7
#[test]
fn revive_stands_the_corpse_up_as_a_pet() {
    let mut s = row(21, 58);
    s.anim = 10;
    s.targetcorpse = true;
    (s.summon, s.summode, s.pettype) = (0, 1, 2);
    s.petmax = rig::CALC_3;
    let mut r = game("necromancer", vec![(SKILL, s)]);
    corpse_class(&mut r);
    let corpse = spawn_corpse(&mut r, 2);
    r.select_right(SKILL);
    r.right_click_unit(corpse);
    r.step(30);
    assert!(r.pets() > 0, "a revived pet follows ({})", r.errors());
    let mode = r.with(move |sim, _| sim.events.action.sys.units.get(corpse).map(|u| u.mode));
    assert_ne!(mode, Some(12), "it stands up ({})", r.errors());
}

/// A game of the assassin with `pgsv` progressive states.
fn assassin(rows: Vec<(usize, Skills)>, pgsv: Vec<usize>) -> Rig {
    let learned = rows.iter().map(|r| r.0).collect();
    let mut r = Rig::build(Cfg {
        class: "assassin",
        class_id: 6,
        token: b"AI",
        rows,
        learned,
        pgsv,
    });
    r.leave_town();
    r.with(|sim, _| sim.events.action.sys.hooks.x.monsters.modes = vec![0xFFFF]);
    r
}

/// The claw item type of the tests (`itemtypes` row, made up).
const CLAW: i16 = 77;
const CHARGE_STATE: usize = 8;

fn claw() -> ItemFacts {
    ItemFacts {
        types: vec![CLAW],
        class: class::ONE_HAND_SWING,
        ..ItemFacts::default()
    }
}

/// Tiger Strike: a charge state on a hit, claws required.
fn tiger_strike() -> Skills {
    let mut s = row(23, 34);
    s.aurastate = CHARGE_STATE as u16;
    s.aurastat1 = 10;
    s.itypea1 = CLAW as u16;
    s
}

// Covers: specs/skills/bodies.md §8.8, §2.14
#[test]
fn tiger_strike_with_a_claw_hurts_and_charges() {
    let mut r = assassin(vec![(SKILL, tiger_strike())], vec![CHARGE_STATE]);
    hold(&mut r, true, claw());
    let me = r.player();
    let m = r.spawn_monster(1);
    r.select_right(SKILL);
    let life0 = r.life(m);
    r.right_click_unit(m);
    r.step(30);
    let errors = r.errors();
    assert!(r.life(m) < life0, "the strike hurt it ({errors})");
    assert!(r.state_on(me, CHARGE_STATE as i16), "a charge ({errors})");
}

// Covers: specs/client/stat-lists.md §2
// (rule 8: the weapon-type test of use_state)
#[test]
fn tiger_strike_without_a_claw_is_refused() {
    let mut r = assassin(vec![(SKILL, tiger_strike())], vec![CHARGE_STATE]);
    hold(
        &mut r,
        true,
        ItemFacts {
            types: vec![CLAW + 1],
            class: class::ONE_HAND_SWING,
            ..ItemFacts::default()
        },
    );
    let me = r.player();
    let m = r.spawn_monster(1);
    r.select_right(SKILL);
    let life0 = r.life(m);
    r.right_click_unit(m);
    r.step(30);
    assert_eq!(r.life(m), life0, "no strike ({})", r.errors());
    assert!(!r.state_on(me, CHARGE_STATE as i16), "no charge");
}

// Covers: specs/skills/bodies.md §8.10
// (a finisher strikes through the srvdo 34 body)
#[test]
fn a_finisher_hurts_the_monster_and_charges() {
    let mut s = row(23, 35);
    s.aurastate = CHARGE_STATE as u16;
    s.aurastat1 = 10;
    s.itypea1 = CLAW as u16;
    let mut r = assassin(vec![(SKILL, s)], vec![CHARGE_STATE]);
    hold(&mut r, true, claw());
    let me = r.player();
    let m = r.spawn_monster(1);
    r.select_right(SKILL);
    let life0 = r.life(m);
    r.right_click_unit(m);
    r.step(30);
    let errors = r.errors();
    assert!(r.life(m) < life0, "the finisher hurt it ({errors})");
    assert!(r.state_on(me, CHARGE_STATE as i16), "its charge ({errors})");
}

/// The most missiles alive at once while the skill runs.
fn missiles_seen(r: &mut Rig, ticks: usize) -> usize {
    let mut most = 0;
    for _ in 0..ticks {
        r.step(1);
        most = most.max(r.with(|sim, _| sim.game.lists.units_of_type(UnitType::Missile).len()));
    }
    most
}

// Covers: specs/skills/bodies.md §5
// (the srvmissile path)
#[test]
fn fire_blast_throws_a_missile() {
    let mut s = row(0, 0);
    s.anim = 10;
    s.srvmissile = 0;
    let mut r = assassin(vec![(SKILL, s)], Vec::new());
    r.select_right(SKILL);
    r.right_click_point(8, 0);
    let most = missiles_seen(&mut r, 30);
    assert!(most > 0, "a missile flew ({})", r.errors());
}
