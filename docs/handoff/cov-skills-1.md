# cov-skills-1 — coverage of specs/skills/bodies.md and bodies-2.md

Branch `claude/cov-skills-1` (base `claude/specs-staging-7` @ 1633f67). Tests only claim rules with `Covers:`; no spec changed, no test skipped or weakened.

| spec | rules | covered before | covered after |
|---|---|---|---|
| specs/skills/bodies.md | 414 | 212 (any) | 407 |
| specs/skills/bodies-2.md | 292 | 105 | 291 |

Tests: `crates/d2-sim/src/skills/use_/bodies/tests5.rs` (bodies.md, 63 tests) and `tests4.rs` (bodies-2.md, 69 tests, written by the subagent). Fixture additions (cfg(test) only): `bodies/fake.rs`, `skills/fake.rs`, `skills/use_/tests.rs` is read only.

## Code fixes found by tests / spec reading
- `dos.rs` curse (§4.4 step 5): the context stats were initialised to −1; the spec zeroes them, so slots after a stop keep stat 0 (matters for a no-record stop in slot 1: the unit must be skipped).
- `use_/mod.rs` do core (bodies.md §5 step 3): the `item`+`aim` target point is used only when both coordinates are non-zero.
- New `helpers::pet_resync` (§2.17 pet-maximum resync algorithm, was only a no-op seam). Not wired: a provider must call it from `skill_resync` with `player::pets::set_max` and the `pettype` `basemax`.
- bodies-2.md: none found.

## Left (bodies.md)
- §1, §6 text: conventions / intro text (X).
- §2.1 r1: the path-target refresh `0x00553490` is provider-side (wiring `mode_target`, path monsters); no body-level test.
- §2.16 (attack-mode cleanup, 4 rules): production `attack_cleanup` / `weapon_cleanup` are no-op defaults in `wiring/action/pending.rs`; needs inventory/equip wiring (bucket C, for an Opus session).
- §2.17: algorithm done and tested; wiring of `skill_resync` still open (C).
## Left (bodies-2.md)
- §1 (X). Partial claims noted by the subagent: §2.21 msg_a3 layout, §2.22 effect handlers, §2.18/§5.8 result-flag bit wording, edge r13 (absence), r29.

## Gate
`tools/gate.sh`: fmt, coverage, depcheck, clippy (workspace without d2-client), rest tests, doc-tests PASS. FAIL: d2-sim tests only for `world::objects::tests::{routes_match_function_table, route_check_catches_perturbations}` (also fail on the base commit, not touched here); d2-client build fails here (wayland-sys pkg-config missing in this container).
