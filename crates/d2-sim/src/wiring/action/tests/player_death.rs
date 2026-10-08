// Spec: specs/combat/vitals.md §4.6, §4.7; specs/sim/units.md §4.5; specs/world/hirelings-2.md §15 r1
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
    assert_eq!(hooks_on(&mut fx, |h, sim| h.corpse_pickup(sim, p, c)), None);
    fx.sim.with(&mut fx.game, |_, v| {
        v.set_state(c, STATE_PLAYERBODY as u16, true)
    });
    assert_eq!(
        hooks_on(&mut fx, |h, sim| h.corpse_pickup(sim, p, c)),
        Some(75)
    );
    assert_eq!(fx.stat(p, EXPERIENCE), 975);
    assert_eq!(fx.stat(c, EXPERIENCE), 0);
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
    assert_eq!(hooks_on(&mut fx, |h, sim| h.corpse_pickup(sim, q, p)), None);
}

/// `hirelings-2.md` §15 rule 1: the DD start queues the player for the
/// hireling host (`ActionHooks::owner_deaths`) after the corpse creation;
/// without the queue nothing is recorded.
// Covers: specs/world/hirelings-2.md §15 r1
#[test]
fn the_corpse_start_queues_the_owner_for_the_hireling_host() {
    let (mut fx, p, _, c) = setup();
    let guid = fx.sim.sys.units.get(p).unwrap().guid;
    fx.sim.hooks().x.corpse = Some((c, guid));
    hooks_on(&mut fx, |h, sim| h.player_corpse(sim, p));
    assert_eq!(fx.sim.hooks().owner_deaths, None);
    fx.sim.hooks().owner_deaths = Some(Vec::new());
    hooks_on(&mut fx, |h, sim| h.player_corpse(sim, p));
    assert_eq!(fx.sim.hooks().owner_deaths, Some(vec![p]));
}

/// A player with no life starts DT once (the penalties run, every
/// player is told with 0x0D code 8); the ENDANIM turns it into DD (the
/// corpse is allocated with state 7 and its owner, code 9 sent once); a
/// player with life is left alone.
// Covers: specs/combat/vitals.md §4.8
#[test]
fn a_player_with_no_life_dies_and_leaves_a_corpse() {
    let (mut fx, p, _, other) = setup();
    fx.sim.hooks().death.allocate_corpses = true;
    fx.stats(p, &[(6, 100 << 8)]);
    fx.stats(other, &[(6, 100 << 8)]);
    let all = [p, other];
    // Alive: nothing; a player that never had life is left alone.
    assert!(fx.sim.deaths_of(&mut fx.game, &all).is_empty());
    fx.stats(p, &[(6, 0)]);
    let guid = fx.sim.sys.units.get(p).unwrap().guid;
    let changed = fx.sim.deaths_of(&mut fx.game, &all);
    assert_eq!(changed, [p]);
    assert_eq!(fx.sim.sys.units.get(p).unwrap().mode, 0);
    assert_eq!(fx.sim.sys.units.get(other).unwrap().mode, 1);
    // Penalties ran: gold 5000 dropped.
    assert_eq!(fx.stat(p, gold_stat::GOLD), 0);
    let stops = |fx: &mut Fx, code: u8| {
        fx.sim
            .hooks()
            .x
            .sent
            .iter()
            .filter(|(_, m)| m[0] == 0x0D && m[6] == code && m[2..6] == guid.to_le_bytes())
            .count()
    };
    assert_eq!(stops(&mut fx, 8), 2, "both players are told");
    // A second pass starts nothing and tells nothing.
    assert!(fx.sim.deaths_of(&mut fx.game, &all).is_empty());
    assert_eq!(stops(&mut fx, 8), 2);
    // The ENDANIM: DD, corpse.
    hooks_on(&mut fx, |h, sim| {
        crate::units::modes::player_event1(sim, h, p).unwrap()
    });
    assert_eq!(fx.sim.sys.units.get(p).unwrap().mode, 17);
    let changed = fx.sim.deaths_of(&mut fx.game, &all);
    assert_eq!(changed, [p]);
    assert_eq!(stops(&mut fx, 9), 2);
    let (&c, &owner) = fx.sim.hooks().death.owners.iter().next().expect("a corpse");
    // The corpse appears (0x59) and lies dead (0x0D code 9), to both.
    let cguid = fx.sim.sys.units.get(c).unwrap().guid;
    let sent = fx.sim.hooks().x.sent.clone();
    assert_eq!(sent.iter().filter(|(_, m)| m[0] == 0x59).count(), 2);
    assert_eq!(
        sent.iter()
            .filter(|(_, m)| m[0] == 0x0D && m[6] == 9 && m[2..6] == cguid.to_le_bytes())
            .count(),
        2
    );
    assert_eq!(owner, guid);
    assert!(fx.sim.sys.stats.has_state(c, STATE_PLAYERBODY));
    assert_eq!(fx.sim.sys.units.get(c).unwrap().mode, 17);
    // Back on its feet (the 0x41 handler's mode 1): code 7, once.
    fx.sim.sys.units.get_mut(p).unwrap().mode = 1;
    fx.stats(p, &[(6, 100 << 8)]);
    assert_eq!(fx.sim.deaths_of(&mut fx.game, &all), [p]);
    assert_eq!(stops(&mut fx, 7), 2);
    assert!(fx.sim.deaths_of(&mut fx.game, &all).is_empty());
    // The owner takes it back.
    assert!(hooks_on(&mut fx, |h, sim| h.corpse_pickup(sim, p, c)).is_some());
    assert_eq!(
        hooks_on(&mut fx, |h, sim| h.corpse_pickup(sim, other, c)),
        None
    );
}
