// Spec: specs/combat/hit.md (Test vectors), specs/combat/damage.md (Test vectors, §5.2), specs/sim/stat-lists.md §10.4
//! Combat ↔ stats and unit fields: the real hit test, damage application
//! and state lists on unit records, stat lists and timers.

use super::*;
use crate::combat::{self as cb, result, CombatWorld, DamageRecord};
use crate::skills::SkillUnits;
use crate::stats::stat as st;
use crate::tick::events::event;

const TOHIT: u16 = 19;
const LEVEL_STAT: u16 = 12;
const ARMORCLASS: u16 = 31;
const NORMAL_DR: u16 = 34;
const DAMAGERESIST: u16 = 36;

/// `hit.md` vector: a monster attacker with AR 300, level 10, against a
/// player with defense 100, level 8: chance 83.
fn duel(fx: &mut Fx) -> (UnitId, UnitId) {
    let a = fx.spawn(UnitType::Monster, 0, fx.a, 10, 10);
    let d = fx.spawn(UnitType::Player, 0, fx.a, 12, 10);
    fx.stats(a, &[(TOHIT, 300), (LEVEL_STAT, 10)]);
    fx.stats(d, &[(ARMORCLASS, 100), (LEVEL_STAT, 8)]);
    (a, d)
}

#[test]
fn hit_test_reads_real_stats_and_draws_on_the_attacker_seed() {
    let mut fx = Fx::new();
    let (a, d) = duel(&mut fx);
    // Seed (1, 0): lo′ = 1791398085, mod 100 = 85 ≥ 83 → miss.
    fx.seed(a, Seed::new(1, 0));
    let hit = fx.sim.combat(&mut fx.game, |w, t| {
        cb::hit_test(w, &t.skills, &t.combat, Some(a), Some(d), 0, false)
    });
    assert!(!hit);
    assert_eq!(
        fx.sim.sys.units.get(a).unwrap().seed,
        Seed::new(0x6AC6_90C5, 0)
    );
    for (x, want) in [(82, true), (83, false)] {
        fx.seed(a, seed_giving(x));
        let hit = fx.sim.combat(&mut fx.game, |w, t| {
            cb::hit_test(w, &t.skills, &t.combat, Some(a), Some(d), 0, false)
        });
        assert_eq!(hit, want, "draw {x}");
    }
    fx.assert_clean();
}

#[test]
fn missile_damage_lowers_real_life_after_resistances() {
    // `damage.md` vector: physical 2560, normal DR 3, damage resist 20
    // (player) → (2560 − 768) × 80 / 100 = 1433.
    let mut fx = Fx::new();
    let (a, d) = duel(&mut fx);
    fx.stats(
        d,
        &[
            (st::MAXHP, 25600),
            (st::HITPOINTS, 25600),
            (NORMAL_DR, 3),
            (DAMAGERESIST, 20),
        ],
    );
    let mut rec = DamageRecord {
        result: result::HIT,
        physical: 2560,
        ..DamageRecord::default()
    };
    fx.sim.combat(&mut fx.game, |w, t| {
        cb::apply(w, &t.combat, a, d, true, &mut rec)
    });
    assert_eq!(rec.total, 1433);
    assert_eq!(fx.stat(d, st::HITPOINTS), 25600 - 1433);
    assert_eq!(rec.result & result::WILL_DIE, 0);
    // §4.4 event 11 (absorb damage) from the totals, then step 6: event 2
    // (damaged by missile) on the defender; no `domissiledamage` without
    // the "rolled" hit flag.
    assert_eq!(
        fx.sim.hooks().x.log,
        [
            format!("event 11 Some({})", d.0),
            format!("event 2 Some({})", d.0)
        ]
    );
    fx.assert_clean();
}

#[test]
fn damage_reschedules_monster_regen_and_kills_at_zero_life() {
    let mut fx = Fx::new();
    let p = fx.spawn(UnitType::Player, 0, fx.a, 10, 10);
    let m = fx.spawn(UnitType::Monster, 0, fx.a, 12, 10);
    fx.stats(
        m,
        &[(st::MAXHP, 5120), (st::HITPOINTS, 5120), (st::HPREGEN, 1)],
    );
    fx.game.frame = 40;
    let mut rec = DamageRecord {
        result: result::HIT,
        physical: 1024,
        ..DamageRecord::default()
    };
    fx.sim.combat(&mut fx.game, |w, t| {
        cb::apply(w, &t.combat, p, m, true, &mut rec)
    });
    // §5.2 step 14: life > 0, hpregen ≠ 0 → type 3 at frame + 1.
    assert_eq!(fx.stat(m, st::HITPOINTS), 4096);
    assert!(fx.timers(m).contains(&(event::STAT_REGEN, 41)));
    // A blow past the remaining life: life 0 (< 256), "will die", events
    // 10 (killed) on the defender and 9 (kill) on the attacker.
    fx.sim.hooks().x.log.clear();
    let mut rec = DamageRecord {
        result: result::HIT,
        physical: 8192,
        ..DamageRecord::default()
    };
    fx.sim.combat(&mut fx.game, |w, t| {
        cb::apply(w, &t.combat, p, m, true, &mut rec)
    });
    assert_eq!(fx.stat(m, st::HITPOINTS), 0);
    assert_ne!(rec.result & result::WILL_DIE, 0);
    assert_eq!(
        fx.sim.hooks().x.log,
        [
            format!("event 11 Some({})", m.0),
            format!("event 2 Some({})", m.0),
            format!("event 10 Some({})", m.0),
            format!("event 9 Some({})", p.0),
        ]
    );
    fx.assert_clean();
}

/// `stat-lists.md` §10.1 step 6: poison (a state-2 list with stat 74
/// below 0) takes a monster's life to 0 in its regeneration event; the
/// monster is killed (`0x0057CCB0`, death mode) by the list's owner, and
/// the death events follow (PROVISIONAL REC-1260: killed 10 on the unit,
/// kill 9 on the owner). Recorded: `check-combat-elements` f78, the quill
/// rat poisoned by the player's javelin dies at life 0.
// Covers: specs/sim/stat-lists.md §10.1 text
#[test]
fn poison_kills_a_monster_at_zero_life_by_its_owner() {
    let mut fx = Fx::new();
    let p = fx.spawn(UnitType::Player, 0, fx.a, 10, 10);
    let m = fx.spawn(UnitType::Monster, 0, fx.a, 12, 10);
    fx.stats(m, &[(st::MAXHP, 5120), (st::HITPOINTS, 100)]);
    const POISON: u16 = 2;
    fx.sim.combat(&mut fx.game, |w, _| {
        w.create_state_list(m, POISON, p, 50);
        w.set_state_list_stat(m, POISON, st::HPREGEN, -87);
        w.set_state(m, POISON, true);
    });
    fx.game
        .schedule_event(m, u32::from(event::STAT_REGEN), 1, None, 0, 0)
        .unwrap();
    fx.sim.hooks().x.log.clear();
    fx.frame();
    assert_eq!(fx.stat(m, st::HITPOINTS), 13);
    assert_ne!(fx.sim.sys.units.get(m).unwrap().mode, 0);
    fx.frame();
    assert_eq!(fx.stat(m, st::HITPOINTS), 0);
    assert_eq!(fx.sim.sys.units.get(m).unwrap().mode, 0);
    let log = &fx.sim.hooks().x.log;
    let killed = format!("event 10 Some({})", m.0);
    let kill = format!("event 9 Some({})", p.0);
    let i = log.iter().position(|l| *l == killed).expect("killed event");
    assert_eq!(log.get(i + 1), Some(&kill));
}

#[test]
fn state_lists_expire_through_the_type_12_event() {
    // Combat's state-list effects on the real stat lists, expired by the
    // unit dispatch's event 12 (`stat-lists.md` §10.4).
    let mut fx = Fx::new();
    let p = fx.spawn(UnitType::Player, 0, fx.a, 10, 10);
    let m = fx.spawn(UnitType::Monster, 0, fx.a, 12, 10);
    const COLD: u16 = 11;
    fx.sim.combat(&mut fx.game, |w, _| {
        w.create_state_list(m, COLD, p, 5);
        w.set_state_list_stat(m, COLD, 67, -50);
        w.set_state(m, COLD, true);
        w.schedule_timer(m, event::REMOVE_STATE, 5);
        assert_eq!(w.state_list_expiry(m, COLD), Some(5));
        assert_eq!(w.state_stat(m, COLD, 67), Some(-50));
        assert!(w.has_state(m, COLD));
        // The list's stat counts on the unit.
        assert_eq!(w.stat(m, 67, 0), -50);
    });
    for _ in 0..5 {
        fx.frame();
    }
    fx.sim.combat(&mut fx.game, |w, _| {
        assert_eq!(w.state_list_expiry(m, COLD), None);
        assert_eq!(w.stat(m, 67, 0), 0);
    });
    fx.assert_clean();
}

// Covers: specs/combat/hit.md §7.1; specs/sim/intents-events.md §7.9 r1
#[test]
fn a_set_alignment_makes_the_state_105_list() {
    // `0x005543B0`: stat 172 in the state-105 list, the state on and its
    // changed bit set (a 0xA8 on the next update); the player's first
    // 0xAA then carries the recorded `-022633` seq 103 bytes.
    let mut fx = Fx::new();
    let p = fx.spawn(UnitType::Player, 0, fx.a, 10, 10);
    fx.sim.with(&mut fx.game, |g, v| {
        assert_eq!(v.state_list(p, 105), None);
        v.set_alignment(g, p, 2);
        let l = v.state_list(p, 105).expect("the list is made");
        assert_eq!(v.stats.base_entries(l).len(), 1);
        assert_eq!(v.state_stat(p, 105, 172), Some(2));
        assert!(v.stats.has_state(p, 105));
        let (_, changed) = v.stats.state_bits(p).expect("state bits");
        assert_ne!(changed[105 / 32] & (1 << (105 % 32)), 0);
        // A second set reuses the list; 0 empties it (the list stays).
        v.set_alignment(g, p, 0);
        assert_eq!(v.state_list(p, 105), Some(l));
        assert!(v.stats.base_entries(l).is_empty());
        // M08: 3 is the original's assertion; nothing changes.
        v.set_alignment(g, p, 3);
        assert!(v.stats.base_entries(l).is_empty());
        v.set_alignment(g, p, 2);
        assert_eq!(v.state_stat(p, 105, 172), Some(2));
        assert!(g.lists.unit(p).is_some_and(|e| e.allied));
    });
    fx.assert_clean();
}
