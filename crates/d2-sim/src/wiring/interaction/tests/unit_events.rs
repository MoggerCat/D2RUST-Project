// Spec: specs/skills/bodies.md §2.18; specs/combat/events.md §2.20; specs/combat/damage.md §5.4; specs/missiles/missiles.md §R5
//! The unit event registry on the action wiring: `0x005C0C30` runs the
//! handlers of `ActionHooks::handlers` through `combat::events::run` on
//! the skill use view once `ActionHooks::enable_unit_events` is called;
//! combat's events (`damage.md` §5.4) and the missile's event 0 reach it.

use crate::combat::{CombatWorld, DamageRecord};
use crate::missiles::MissileCombat;
use crate::skills::use_::bodies::Handler;
use crate::units::{UnitId, UnitType};

use super::skill_use::{skills, Fx};
use super::stat_data;

/// `restinpeace` (`combat/events.md` §2.20).
const RESTINPEACE: u32 = 172;
/// Function 29: state 172 on O.
const FN_REST_IN_PEACE: i32 = 29;

fn handler(event: u8, key_type: i32, key: i32) -> Handler {
    Handler {
        event,
        key_type,
        key,
        skill: 0,
        level: 1,
        func: FN_REST_IN_PEACE,
    }
}

fn fx() -> (Fx, UnitId, UnitId) {
    let mut fx = Fx::with(stat_data(), skills());
    let p = fx.spawn(UnitType::Player, 10, 10);
    let m = fx.spawn(UnitType::Monster, 12, 10);
    (fx, p, m)
}

fn rest(fx: &Fx, u: UnitId) -> bool {
    fx.sim.sys.stats.has_state(u, RESTINPEACE)
}

// Covers: specs/skills/bodies.md §2.18
#[test]
fn registry_runs_matching_handlers_and_drops_key_type_zero() {
    let (mut fx, p, m) = fx();
    fx.sim.sys.hooks.enable_unit_events();
    // Head first: a one-shot (key type 0) of event 9, a kept one (key
    // type 1) of event 9, one of another event.
    fx.sim.sys.hooks.handlers.insert(
        p,
        vec![handler(9, 0, 5), handler(9, 1, 7), handler(8, 1, 8)],
    );
    let mut rec = DamageRecord::default();
    fx.sim.with(&mut fx.game, |g, v| {
        CombatWorld::unit_event(&mut v.combat(g), 9, p, m, &mut rec)
    });
    assert!(rest(&fx, m));
    assert!(!rest(&fx, p));
    assert_eq!(
        fx.sim.sys.hooks.handlers[&p],
        vec![handler(9, 1, 7), handler(8, 1, 8)]
    );
    fx.assert_clean();
}

// Covers: specs/missiles/missiles.md §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler
#[test]
fn missile_event_zero_reaches_the_registry() {
    let (mut fx, p, m) = fx();
    fx.sim.sys.hooks.enable_unit_events();
    fx.sim.sys.hooks.handlers.insert(m, vec![handler(0, 1, 1)]);
    // O is the missile (here the player stands in for it) and the
    // unit may be none.
    fx.sim.with(&mut fx.game, |g, v| {
        v.hit_by_missile_event(g, p, None);
        v.hit_by_missile_event(g, p, Some(m));
    });
    assert!(rest(&fx, p));
    assert_eq!(fx.sim.sys.hooks.handlers[&m].len(), 1);
}

// Covers: specs/skills/bodies.md §2.18
#[test]
fn without_the_registry_events_stay_on_the_seam() {
    let (mut fx, p, m) = fx();
    fx.sim.sys.hooks.handlers.insert(p, vec![handler(9, 0, 5)]);
    let mut rec = DamageRecord::default();
    fx.sim.with(&mut fx.game, |g, v| {
        CombatWorld::unit_event(&mut v.combat(g), 9, p, m, &mut rec)
    });
    assert!(!rest(&fx, m));
    assert_eq!(fx.sim.sys.hooks.handlers[&p].len(), 1);
}
