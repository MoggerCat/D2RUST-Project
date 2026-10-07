# Coverage session cov-skills-2 (skills bodies-2b, bodies-4)

Branch `claude/cov-skills-2`, base `claude/specs-staging-7` @ 1633f67. Buckets A (all unit tests; no bucket B/C work was needed).

## Counts (`python3 tools/coverage.py`, unit tier)

| spec | before | after | left |
|---|---|---|---|
| specs/skills/bodies-2b.md (278 units) | 100 | 276 | 2 |
| specs/skills/bodies-4.md (187 units) | 18 | 185 | 2 |

Tests: `bodies/tests4.rs` (bodies-4, 90 tests), `bodies/tests5.rs` (bodies-2b §8, 55 tests), `bodies/tests6.rs` (bodies-2b §6–§7, 69 tests). Test-fake additions: `Fake.dead_units`, `Fake.aligned` (`skills/fake.rs`); `fail_spawns`, `finds`, `missile_base` (`bodies/fake.rs`).

## Code fixes found by the tests

None. Every test passed against the existing code once the test's own expectation was right; the spec was the reference throughout.

## Rules left, with reason

- bodies-2b §7.11 (Iron Golem start test): the item conditions (type 4, mode 3, `bitfield1` metal bit, identified, not active) live behind the host seam `BodyWorld::golem_item`, whose default (`wiring/action/pending.rs`) is `false`; the fake answers "is an item". The body (`iron_golem_start`) is covered through §7.12; the real predicate is **bucket C** (a host implementation + test) for an Opus session.
- bodies-2b §8.12 text: narration ("The do is srvdo 2").
- bodies-4 §3 text: section intro only ("Continues bodies-3.md §5").
- bodies-4 §4.1 text: skills.txt data fact (Frost Nova / Nova use this body), needs game data.

## Parts of claimed rules the fakes cannot observe

- bodies-2b §8.7 r4 right-skill branch (`BodyFake::right_skill` is None); §8.11 r4 record fields (`apply_melee` consumes the record; checked through life loss and the Berserk start).
- bodies-2b §7.8 r7 "T is a player → override kind 1": unreachable (`can_switch` is false for players, so the attract test fails first).
- bodies-2b §6.14 r3/r5 hit record contents (consumed by `apply_melee`): checked through infection and events only. §7.17 / §7.20 checked through exact life loss and leech amounts.
- bodies-2b §6.12 r1 "no K picked → type-1 timer at F + 4" not exercised.
- bodies-4 §3.2 r7 (`result |= 2` at life 0) and the poison amount/length of §3.1–§3.2: not logged by the fake.
- `BodyFake::stays_on_death` is always false, so the "stay on death" branch of the Holy Freeze remove callback (§6.10) is untested.

## Provisional points

None in either spec (no `PROVISIONAL` marks added).

## Merge note (coordinator)

- Changed expectation: `tests4::lightning_step_zero_or_less_creates_nothing`
  → `..._creates_no_further_missile`, asserting one missile per call (the
  i = 0 missile), per `bodies-4.md` Edge case 1 ("creates no further
  missiles") and the existing `tests3` test of the same decision.
