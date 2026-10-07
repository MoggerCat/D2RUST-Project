// Spec: specs/tools/scenario.md §3, §3.1
//! Reference table rows (§3) and spawn-kind rows (§3.1), checked on the
//! public parser and resolver.

use conformance::scenario::script::{resolve, Ref, UnitRef, World};
use conformance::scenario::Scenario;

struct W;

impl World for W {
    fn player(&self) -> Option<(u32, i32, i32)> {
        Some((7, 100, 200))
    }
    fn units(&self) -> Vec<UnitRef> {
        let u = |ty, class, guid, waypoint| UnitRef {
            ty,
            class,
            guid,
            waypoint,
        };
        vec![
            u(1, 148, 30, false),
            u(1, 148, 12, false),
            u(1, 9, 20, false),
            u(2, 119, 5, true),
            u(2, 119, 4, false),
            u(2, 119, 3, true),
        ]
    }
}

// Covers: specs/tools/scenario.md §3 row1
#[test]
fn ref_player() {
    assert_eq!(resolve(&Ref::Player, &W), Ok(7));
}

// Covers: specs/tools/scenario.md §3 row2
#[test]
fn ref_position_offsets() {
    assert_eq!(resolve(&Ref::X(0), &W), Ok(100));
    assert_eq!(resolve(&Ref::X(5), &W), Ok(105));
    assert_eq!(resolve(&Ref::Y(-3), &W), Ok(197));
}

// Covers: specs/tools/scenario.md §3 row3
#[test]
fn ref_nth_unit_of_type_in_guid_order() {
    let r = |n| Ref::Unit {
        ty: 1,
        class: None,
        n,
    };
    assert_eq!(resolve(&r(0), &W), Ok(12));
    assert_eq!(resolve(&r(1), &W), Ok(20));
    assert_eq!(resolve(&r(2), &W), Ok(30));
    assert!(resolve(&r(3), &W).is_err());
}

// Covers: specs/tools/scenario.md §3 row4
#[test]
fn ref_nth_unit_of_class() {
    let r = |n| Ref::Unit {
        ty: 1,
        class: Some(148),
        n,
    };
    assert_eq!(resolve(&r(0), &W), Ok(12));
    assert_eq!(resolve(&r(1), &W), Ok(30));
    assert!(resolve(&r(2), &W).is_err());
}

// Covers: specs/tools/scenario.md §3 row5
#[test]
fn ref_waypoints_only() {
    assert_eq!(resolve(&Ref::Waypoint(0), &W), Ok(3));
    assert_eq!(resolve(&Ref::Waypoint(1), &W), Ok(5));
    assert!(resolve(&Ref::Waypoint(2), &W).is_err());
}

const BASE: &str = "scenario 1\nname t\ngame 1.14d\nseed 1\ninit 2\ndifficulty normal\nexpansion yes\nend 10\nchar class 1\nchar area 0 1\n";

fn parse(extra: &str) -> Result<Scenario, conformance::scenario::script::ScriptError> {
    Scenario::parse(&format!("{BASE}{extra}\n"))
}

// Covers: specs/tools/scenario.md §3.1 row1
#[test]
fn spawn_normal_takes_no_umods() {
    assert!(parse("at 1 spawn 148 @x+10 @y normal").is_ok());
    assert!(parse("at 1 spawn 148 100 200 normal umod 3").is_err());
}

// Covers: specs/tools/scenario.md §3.1 row2
#[test]
fn spawn_random_boss_takes_no_umods() {
    assert!(parse("at 1 spawn 148 100 200 random-boss").is_ok());
    assert!(parse("at 1 spawn 148 100 200 random-boss umod 3").is_err());
}

// Covers: specs/tools/scenario.md §3.1 row3
#[test]
fn spawn_champion_takes_exactly_one_umod() {
    assert!(parse("at 1 spawn 148 100 200 champion umod 3").is_ok());
    assert!(parse("at 1 spawn 148 100 200 champion").is_err());
    assert!(parse("at 1 spawn 148 100 200 champion umod 3 4").is_err());
}

// Covers: specs/tools/scenario.md §3.1 row4
#[test]
fn spawn_unique_takes_one_to_nine_umods() {
    assert!(parse("at 1 spawn 148 100 200 unique umod 1").is_ok());
    assert!(parse("at 1 spawn 148 100 200 unique umod 1 2 3 4 5 6 7 8 9").is_ok());
    assert!(parse("at 1 spawn 148 100 200 unique umod 1 2 3 4 5 6 7 8 9 10").is_err());
    assert!(parse("at 1 spawn 148 100 200 unique").is_err());
}

// Covers: specs/tools/scenario.md §3.1 r4
#[test]
fn unresolved_spawn_reference_is_reported_not_fatal() {
    let s = parse("at 1 spawn 148 @1:999 @y normal").unwrap();
    let conformance::scenario::script::StepMsg::Spawn(sp) = &s.steps[0].msg else {
        panic!("spawn step");
    };
    let e = conformance::scenario::script::spawn_position(sp, &W).unwrap_err();
    assert_eq!(e.reference, "@1:999");
}
