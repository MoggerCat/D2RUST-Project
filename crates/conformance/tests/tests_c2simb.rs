// Spec: specs/tools/scenario.md §Edge cases
//! Scenario edge cases checked on the public parser.

use conformance::scenario::Scenario;

fn scenario(extra: &str) -> Scenario {
    let text = format!(
        "scenario 1\nname edge\ngame 1.14d\nseed 1\ninit 1\ndifficulty normal\nexpansion yes\nend 10\n\nchar class 1\nchar area 0 1\nchar at default\n\nrecord units\n{extra}\n"
    );
    Scenario::parse(&text).unwrap()
}

// Covers: specs/tools/scenario.md §edge-cases-original-bugs r4
#[test]
fn snapshot_every_larger_than_end_gives_ticks_0_and_end() {
    let s = scenario("snapshot every 25");
    let t: Vec<u32> = s.snapshot_ticks().into_iter().collect();
    assert_eq!(t, vec![0, 10]);
}
