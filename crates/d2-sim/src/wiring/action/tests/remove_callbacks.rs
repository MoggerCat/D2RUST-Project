// Spec: specs/missiles/missiles.md R5 step 6.1; specs/skills/bodies.md §6.10; specs/sim/stat-lists.md §8.2 rule 6
//! The remove callbacks that only turn a state off: the missile just-hit
//! list `0x005ADAF0` and the Charge counter `0x005D3310`. Once the list
//! expires the state must be off, or the unit stays "just hit" for good.

use super::*;
use crate::skills::use_::bodies::callback;
use crate::stats::lists::RemoveCallback;
use crate::stats::StatHost;
use crate::units::hooks::{Sim, UnitHooks};

const STATE_JUSTHIT: u32 = 86;
const STATE_CHARGE: u32 = 4;

fn state_off_after_expiry(cb: u32, state: u32) {
    let mut fx = Fx::new();
    let m = fx.spawn(UnitType::Monster, 0, fx.a, 10, 10);
    fx.sim.with(&mut fx.game, |_, v| v.set_state(m, state as u16, true));
    assert!(fx.sim.sys.stats.has_state(m, state));
    fx.sim.with(&mut fx.game, |g, v| {
        let mut sim = Sim {
            game: g,
            units: &mut *v.units,
            stats: &mut *v.stats,
            data: v.data,
        };
        let list = sim.stats.unit_list(m).expect("unit list");
        v.h.list_removed(sim.stats, m, state, list, RemoveCallback(cb));
        v.h.lists_expired(&mut sim, m);
    });
    assert!(!fx.sim.sys.stats.has_state(m, state));
}

// Covers: specs/missiles/missiles.md R5 step 6.1
#[test]
fn the_justhit_callback_turns_state_86_off() {
    state_off_after_expiry(callback::JUSTHIT, STATE_JUSTHIT);
}

// Covers: specs/skills/bodies.md §6.10
#[test]
fn the_charge_callback_turns_its_state_off() {
    state_off_after_expiry(callback::CHARGE, STATE_CHARGE);
}
