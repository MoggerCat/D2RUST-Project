// Spec: specs/skills/bodies.md §3.8; specs/skills/bodies-2.md §4.8; specs/skills/use.md §5
//! The Barbarian's skills in the play preview, headless over the synthetic
//! single-player game: a barbarian (class 4) joins, walks out of town by
//! the cave warp (a monster in a town room is not a legal target of
//! Bash, `bodies.md` §3.8 step 1) and casts through C→S 0x0D / 0x0C.
//!
//! Synthetic fills (no game file, `// d2rs-own, unverified`): the skill
//! rows keep the shapes of the real ones (ids 126 Bash, 130 Howl, 132 Leap,
//! 133 Double Swing, 138 Shout, 149 Battle Orders, 151 Whirlwind) but the
//! numbers are made up; the AnimData record of the barbarian's A1.
//! Provisional parts: REC-152 in `docs/HANDOFF.md` §7.

#[path = "app_barbarian/rig.rs"]
mod rig;

use rig::*;

// Covers: specs/skills/bodies.md §3.8
// Covers: specs/skills/use.md §5.3
#[test]
fn bash_on_a_monster_costs_mana_and_hurts_it() {
    let mut r = Rig::new(&[BASH]);
    r.leave_town();
    let m = r.spawn_monster(1);
    r.select_right(BASH);
    let (mana0, life0) = (r.mana(), r.life(m));
    r.right_click_unit(m);
    r.step(30);
    let errors = r.errors();
    assert!(r.mana() < mana0, "Bash spent mana ({errors})");
    assert!(r.life(m) < life0, "Bash hurt the monster ({errors})");
}

// Covers: specs/skills/bodies.md §8.2
// Covers: specs/skills/bodies.md §6.8
// Covers: specs/skills/use.md §5.4
#[test]
fn shout_and_battle_orders_put_their_state_on_the_barbarian() {
    let mut r = Rig::new(&[SHOUT, BATTLE_ORDERS]);
    r.leave_town();
    let me = r.player();
    for (skill, state) in [(SHOUT, SHOUT_STATE), (BATTLE_ORDERS, BATTLE_ORDERS_STATE)] {
        r.select_right(skill);
        let mana0 = r.mana();
        assert!(!r.has_state(me, state), "no state before");
        r.right_click_point(8, 0);
        r.step(30);
        let errors = r.errors();
        assert!(
            r.has_state(me, state),
            "skill {skill}: state {state} ({errors})"
        );
        assert!(r.mana() < mana0, "skill {skill} spent mana ({errors})");
    }
}

// Covers: specs/skills/bodies-2.md §4.8
#[test]
fn double_swing_hits_two_monsters_in_two_swings() {
    let mut r = Rig::new(&[DOUBLE_SWING]);
    r.leave_town();
    let (a, b) = (r.spawn_monster(1), r.spawn_monster(2));
    r.select_right(DOUBLE_SWING);
    let (mana0, la, lb) = (r.mana(), r.life(a), r.life(b));
    r.right_click_unit(a);
    r.step(30);
    let errors = r.errors();
    assert!(r.mana() < mana0, "Double Swing spent mana ({errors})");
    assert!(r.life(a) < la, "the first swing hurt the target ({errors})");
    assert!(
        r.life(b) < lb,
        "the second swing hurt the next monster ({errors})"
    );
}

// Covers: specs/skills/levels.md §6.4
// Covers: specs/client/msg-skills.md §2 r4
#[test]
fn a_learned_mastery_puts_its_passive_stats_in_its_state() {
    let mut r = Rig::with_rows(&[NATURAL_RESISTANCE], &[]);
    r.leave_town();
    r.give_skill_points(1);
    assert_eq!(
        r.state_stat(PASSIVE_STATE, FIRE_RESIST),
        None,
        "not learned"
    );
    r.add_skill_point(NATURAL_RESISTANCE);
    r.step(5);
    let errors = r.errors();
    assert_eq!(
        r.state_stat(PASSIVE_STATE, FIRE_RESIST),
        Some(25),
        "the passive state holds passivestat1 = passivecalc1 ({errors})"
    );
}

// Covers: specs/skills/bodies-2.md §4.6
// Covers: specs/skills/bodies-2.md §4.7
#[test]
fn leap_spends_mana_and_moves_the_barbarian() {
    let mut r = Rig::new(&[LEAP]);
    r.leave_town();
    r.select_right(LEAP);
    let (mana0, from) = (
        r.mana(),
        r.with(|sim, p| sim.events.action.sys.hooks.path_position(p)),
    );
    r.right_click_point(5, 0);
    r.step(40);
    let to = r.with(|sim, p| sim.events.action.sys.hooks.path_position(p));
    let errors = r.errors();
    assert!(r.mana() < mana0, "Leap spent mana ({errors})");
    assert!(
        to.0 > from.0,
        "the barbarian leapt east: {from:?} -> {to:?}"
    );
}

// Covers: specs/skills/bodies-2b.md §8.10
#[test]
fn whirlwind_spends_mana_and_moves_the_barbarian() {
    let mut r = Rig::new(&[WHIRLWIND]);
    r.leave_town();
    r.select_right(WHIRLWIND);
    let (mana0, from) = (
        r.mana(),
        r.with(|sim, p| sim.events.action.sys.hooks.path_position(p)),
    );
    r.right_click_point(8, 0);
    r.step(40);
    let to = r.with(|sim, p| sim.events.action.sys.hooks.path_position(p));
    let errors = r.errors();
    assert!(r.mana() < mana0, "Whirlwind spent mana ({errors})");
    assert!(
        to.0 > from.0,
        "the barbarian whirled east: {from:?} -> {to:?}"
    );
}

// Covers: specs/skills/bodies-2.md §4.7
// The landing of a player's Leap is a knockback (result 9) with a zeroed
// damage record (`bodies-2.md` §4.7): the barbarian lands exactly on the
// aimed point and the monster standing there loses no life.
#[test]
fn leap_lands_on_the_aimed_point_and_deals_no_damage() {
    let mut r = Rig::new(&[LEAP]);
    r.leave_town();
    r.select_right(LEAP);
    let m = r.spawn_monster(4);
    let (life0, from) = (
        r.life(m),
        r.with(|sim, p| sim.events.action.sys.hooks.path_position(p)),
    );
    r.right_click_point(5, 0);
    r.step(40);
    let to = r.with(|sim, p| sim.events.action.sys.hooks.path_position(p));
    let errors = r.errors();
    assert_eq!(
        to,
        (from.0 + 5, from.1),
        "landed on the aimed point ({errors})"
    );
    assert_eq!(r.life(m), life0, "the landing is a knockback only");
}

// Covers: specs/skills/bodies-2b.md §6.11, §6.12
// Covers: specs/skills/bodies-2.md §2.19
#[test]
fn leap_attack_leaps_to_the_monster_and_strikes_it() {
    let mut r = Rig::new(&[LEAP_ATTACK]);
    r.leave_town();
    r.select_right(LEAP_ATTACK);
    let m = r.spawn_monster(6);
    let (mana0, life0) = (r.mana(), r.life(m));
    r.right_click_unit(m);
    r.step(60);
    let errors = r.errors();
    assert!(r.mana() < mana0, "Leap Attack spent mana ({errors})");
    assert!(r.life(m) < life0, "the strike hurt the monster ({errors})");
}

// Covers: specs/skills/bodies-2.md §3.10, §3.11, §2.5, §2.6
#[test]
fn dragon_talon_kick_hurts_a_monster() {
    let mut r = Rig::new(&[DRAGON_TALON]);
    r.leave_town();
    r.select_right(DRAGON_TALON);
    let m = r.spawn_monster(1);
    let life0 = r.life(m);
    r.right_click_unit(m);
    r.step(30);
    let errors = r.errors();
    assert!(r.life(m) < life0, "the kick hurt the monster ({errors})");
}

// Covers: specs/skills/bodies-2b.md §7.20
#[test]
fn dragon_flight_kick_hurts_a_monster() {
    let mut r = Rig::new(&[DRAGON_FLIGHT]);
    r.leave_town();
    r.select_right(DRAGON_FLIGHT);
    let m = r.spawn_monster(1);
    let life0 = r.life(m);
    r.right_click_unit(m);
    r.step(30);
    let errors = r.errors();
    assert!(r.life(m) < life0, "the kick hurt the monster ({errors})");
}
