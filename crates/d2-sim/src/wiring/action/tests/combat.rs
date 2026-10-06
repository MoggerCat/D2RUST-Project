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
