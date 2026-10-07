// Spec: specs/combat/vitals.md §4.6, §4.7; specs/sim/units.md §4.5
//! A player's death on the action wiring: the penalties from the DT
//! start, the corpse experience from the DD start and the pickup.

use d2_data::tables::Experience;

use super::*;
use crate::combat::vitals::VitalsTables;
use crate::units::hooks::{Sim, UnitHooks};
use crate::wiring::action::death::{gold_stat, STATE_PLAYERBODY};

const LEVEL_STAT: u16 = 12;
const EXPERIENCE: u16 = 13;

/// Thresholds: level 1 at 500, level 2 at 1500.
fn vitals() -> VitalsTables {
    let row = |v: u32| Experience {
        amazon: v,
        sorceress: v,
        necromancer: v,
        paladin: v,
        barbarian: v,
        druid: v,
        assassin: v,
        ..blank()
    };
    VitalsTables {
        charstats: Vec::new(),
        experience: vec![row(3), row(0), row(500), row(1500)],
    }
}

/// A level-2 player with 1000 experience and 5000 gold, a monster, a
/// corpse unit; single player (game type 3), `DeathExpPenalty` 10.
fn setup() -> (Fx, UnitId, UnitId, UnitId) {
    let mut fx = Fx::new();
    let h = fx.sim.hooks();
    h.vitals = Some(Arc::new(vitals()));
    h.ai_info.game_type = 3;
    Arc::make_mut(&mut h.tables).combat.difficultylevels[0].deathexppenalty = 10;
    let p = fx.spawn(UnitType::Player, 1, fx.a, 10, 10);
    let m = fx.spawn(UnitType::Monster, 0, fx.a, 13, 10);
    let c = fx.spawn(UnitType::Player, 1, fx.a, 10, 11);
    fx.stats(
        p,
        &[(LEVEL_STAT, 2), (EXPERIENCE, 1000), (gold_stat::GOLD, 5000)],
    );
    (fx, p, m, c)
}

fn hooks_on<R>(fx: &mut Fx, f: impl FnOnce(&mut ActionHooks<TestPending>, &mut Sim<'_>) -> R) -> R {
    fx.sim.with(&mut fx.game, |g, v| {
        let mut sim = Sim {
            game: g,
            units: &mut *v.units,
            stats: &mut *v.stats,
            data: v.data,
        };
        f(&mut *v.h, &mut sim)
    })
}

// Covers: specs/combat/vitals.md §4.6, §4.7
#[test]
fn death_penalties_corpse_experience_and_pickup() {
    let (mut fx, p, m, c) = setup();
    // DT start with the monster as K (`0x00580EC0` → `0x00535AB0`).
    fx.sim.hooks().mode_target = Some(m);
    hooks_on(&mut fx, |h, sim| h.player_death(sim, p));
    // Gold: q = 2 × 5000 / 100 = 100; not pvp: drop 4900, gold 0.
    assert_eq!(fx.sim.hooks().x.log, [format!("drop gold {} 4900", p.0)]);
    assert_eq!(fx.stat(p, gold_stat::GOLD), 0);
    assert_eq!(fx.stat(p, gold_stat::GOLDLOST), 100);
    // Experience: loss = 10 × (1500 − 500) / 100 = 100.
    assert_eq!(fx.stat(p, EXPERIENCE), 900);
    assert_eq!(fx.sim.hooks().death.exp_lost[&p], 100);
    // DD start: the corpse holds 75 % of the loss; the field is cleared.
    let guid = fx.sim.sys.units.get(p).unwrap().guid;
    fx.sim.hooks().x.corpse = Some((c, guid));
    hooks_on(&mut fx, |h, sim| h.player_corpse(sim, p));
    assert_eq!(fx.stat(c, EXPERIENCE), 75);
    assert_eq!(fx.sim.hooks().death.exp_lost[&p], 0);
    // Pickup: no `playerbody` → nothing.
    assert_eq!(hooks_on(&mut fx, |h, sim| h.corpse_pickup(sim, p, c)), 0);
    fx.sim.with(&mut fx.game, |_, v| {
        v.set_state(c, STATE_PLAYERBODY as u16, true)
    });
    assert_eq!(hooks_on(&mut fx, |h, sim| h.corpse_pickup(sim, p, c)), 75);
    assert_eq!(fx.stat(p, EXPERIENCE), 975);
    assert_eq!(fx.stat(c, EXPERIENCE), 0);
    assert_eq!(
        fx.sim.hooks().x.log.last(),
        Some(&format!("take back {} {}", p.0, c.0))
    );
}

// Covers: specs/combat/vitals.md §4.6
#[test]
fn player_killer_takes_gold_only() {
    let (mut fx, p, _, q) = setup();
    fx.sim.hooks().mode_target = Some(q);
    hooks_on(&mut fx, |h, sim| h.player_death(sim, p));
    // pvp, q = 100 ≤ gi: drop q, gold unchanged; no experience loss.
    assert_eq!(fx.sim.hooks().x.log, [format!("drop gold {} 100", p.0)]);
    assert_eq!(fx.stat(p, gold_stat::GOLD), 5000);
    assert_eq!(fx.stat(p, gold_stat::GOLDLOST), 100);
    assert_eq!(fx.stat(p, EXPERIENCE), 1000);
    assert!(fx.sim.hooks().death.exp_lost.is_empty());
    // A corpse of a pickup by another player is refused without the
    // loot test.
    fx.sim.hooks().x.corpse = Some((p, 12345));
    fx.sim.with(&mut fx.game, |_, v| {
        v.set_state(p, STATE_PLAYERBODY as u16, true)
    });
    assert_eq!(hooks_on(&mut fx, |h, sim| h.corpse_pickup(sim, q, p)), 0);
}
