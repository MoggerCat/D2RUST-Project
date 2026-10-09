// Spec: specs/monsters/ai.md §5.3 (`0x005DDC30`, scan 6 callback `0x005DCBD0`)
//! The secondary target search on the wired units: scan 6's main and
//! alternative slots, its filter and the alternative choice.

use super::*;
use crate::monsters::ai::AiTargets;
use crate::wiring::action::ai_scan::{Scan6, UNIT_FLAG_4};

/// Monstats `threat` of class 0.
fn set_threat(fx: &mut Fx, threat: u8) {
    let mut t = (*fx.sim.hooks().tables).clone();
    t.combat.monstats[0].threat = threat;
    fx.sim.hooks().tables = Arc::new(t);
}

/// A scanner monster at (10, 10), a monster at (12, 10) and a player at
/// (20, 10), each with unit flag 0x4.
fn three(fx: &mut Fx) -> (UnitId, UnitId, UnitId) {
    let m = fx.spawn(UnitType::Monster, 0, fx.a, 10, 10);
    let c = fx.spawn(UnitType::Monster, 0, fx.a, 12, 10);
    let p = fx.spawn(UnitType::Player, 1, fx.a, 20, 10);
    for u in [m, c, p] {
        fx.sim.sys.units.get_mut(u).unwrap().flags |= UNIT_FLAG_4;
    }
    (m, c, p)
}

fn scan(fx: &mut Fx, m: UnitId) -> Scan6 {
    fx.sim.with(&mut fx.game, |g, v| v.scan6(g, m))
}

fn secondary(fx: &mut Fx, m: UnitId) -> (Option<UnitId>, i32, bool) {
    fx.sim.with(&mut fx.game, |g, v| v.secondary_target(g, m))
}

// Covers: specs/monsters/ai.md §5.3
#[test]
fn threat_below_2_is_the_alternative_and_the_main_target_stays() {
    // Rule 3: a monster of threat 0 competes for the alternative; the
    // player (threat 14) for the main slot. Full-size distances (size 1):
    // 2 − 1 = 1 and 10 − 1 = 9. The alternative is within 5 but the
    // trial path keeps the main target (the host's `choose_alternative`,
    // false here).
    let mut fx = Fx::new();
    set_threat(&mut fx, 0);
    let (m, c, p) = three(&mut fx);
    assert_eq!(
        scan(&mut fx, m),
        Scan6 {
            main: Some((p, 9)),
            alt: Some((c, 1)),
        }
    );
    assert_eq!(secondary(&mut fx, m), (Some(p), 9, false));
    fx.assert_clean();
}

#[test]
fn nearer_threatening_monster_takes_the_main_slot() {
    let mut fx = Fx::new();
    set_threat(&mut fx, 5);
    let (m, c, _) = three(&mut fx);
    assert_eq!(
        scan(&mut fx, m),
        Scan6 {
            main: Some((c, 1)),
            alt: None,
        }
    );
    assert_eq!(secondary(&mut fx, m).0, Some(c));
}

#[test]
fn filter_skips_without_flag_4_and_when_not_hostile() {
    // Rule 1: unit flag 0x4 is required; hostility `0x00554200` too.
    let mut fx = Fx::new();
    set_threat(&mut fx, 5);
    let (m, c, p) = three(&mut fx);
    fx.sim.sys.units.get_mut(c).unwrap().flags &= !UNIT_FLAG_4;
    assert_eq!(scan(&mut fx, m).main, Some((p, 9)));
    fx.sim.sys.units.get_mut(c).unwrap().flags |= UNIT_FLAG_4;
    fx.sim.hooks().x.peaceful.push(c);
    assert_eq!(scan(&mut fx, m).main, Some((p, 9)));
}

#[test]
fn nothing_within_49_reports_no_distance() {
    // Rule 2: d ≥ 49 is skipped; none → distance 0x7FFFFFFF (ai.md edge
    // case 16).
    let mut fx = Fx::new();
    let m = fx.spawn(UnitType::Monster, 0, fx.a, 10, 10);
    let p = fx.spawn(UnitType::Player, 1, fx.a, 60, 10);
    fx.sim.sys.units.get_mut(p).unwrap().flags |= UNIT_FLAG_4;
    // 50 − 1 = 49: out.
    assert_eq!(secondary(&mut fx, m), (None, 0x7FFF_FFFF, false));
}
