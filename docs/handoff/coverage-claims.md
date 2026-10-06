# Handoff: coverage claims on existing tests (branch `claude/coverage-claims`, 2026-10-06)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Scope: `// Covers:` claims (`docs/COVERAGE.md` §2) added to tests that had
none, in `crates/conformance`, `d2-proto`, `d2-server`, `d2-sim` and
`d2-client`. The branch starts from `claude/bold-ptolemy-jvyvxy` at `ed7236e`
(cloud session, repo only). The diff from `ed7236e` adds `// Covers:` lines
and this file, and nothing else: no test logic, spec text, non-test code,
`docs/HANDOFF.md` or `docs/PLAN.md` was changed. No spec needed new rule
IDs, so `tools/spec_index.py` was not rerun. Task class: mechanical
annotation with judgement, at medium effort (M14). The work was split across
ten agents, each editing a separate set of files.

## 1. Rule for a claim

A test claims a rule only when its assertions check that rule's outcome.
The claim names the narrowest ID that is fully true. These got no claim:
- tests that only call the code behind a rule;
- consistency checks between code and a TSV (table rows are not rule
  units, `COVERAGE.md` §1);
- M08 perturbation tests of a checker;
- tests of d2rs-only errors that no rule states.

In `crates/conformance`, only the replays of recorded traces are claimed:
`recorded_rng_traces_replay_exactly` and `sim_000{6,7,8}_replays_exactly`.
Each claims only what its replay compares against the recording. The
perturbation and well-formedness tests are not claimed.

After review, 10 claims were dropped because the test leaves out a clause
the rule states (commit "drop partial claims"):
- intents-events §3.2 r3: the empty-game timeout;
- intents-events edge case r8: the missing 0x80 handler;
- affixes §3 r4: the version and group filters;
- generation §4 r3: only the armor branch is tested;
- generation §6.2 r5: the refund;
- quality §4 text: the dispatch table;
- ai §9.4 r5 and §9.5 r1: the untested branches;
- quests §9.3: `0x005455F0`.

**Open policy point (for `COVERAGE.md`).** Some rules are one unit with
no narrower ID: a section with only prose or a table, or an item with an
indented sub-list. Agents claimed some of these where a test checks the
unit's main behavior but not every branch. Examples:
- damage.md §4.2, §5.7, §9 text;
- hit.md §4 r5;
- stats.md §5 r3, §8 r2;
- ai.md §1.2, §1.4, §3.2, §7.2, §8;
- skills levels.md §2, §3.5, §4;
- missiles §R4.2, §R6.2, §R5 r2;
- rooms.md §4 text, §9.5 text;
- levels.md §4 r3;
- cube.md §4, §5, §6.2, §7.3, §7.4;
- quests.md §10.1, §10.4, §10.6;
- assets.md §A1, §A2, §A5;
- map-preview §tile-files, §screen-position, §what-is-drawn, §cells.

The existing client claims already do the same. Strict `COVERAGE.md`
wording ("fully true") would drop these. The better fix is to give such
specs numbered items, which is spec work. Then the claims can be narrowed.
Examples:
- damage.md §8 holds the event table, crushing blow and open wounds as one
  unit;
- cube.md's edge cases are unnumbered bullets;
- stats.md §6.3 is the op table.

## 2. Numbers

`python3 tools/coverage.py --summary`, before (`ed7236e`) and after (this
branch). Only specs that changed are listed; all other specs are unchanged.

| Spec | Units | Any before | Any after | Verified before | Verified after |
|---|---:|---:|---:|---:|---:|
| `specs/client/assets.md` | 12 | 3 (25.0%) | 7 (58.3%) | 0 (0.0%) | 1 (8.3%) |
| `specs/client/bridge.md` | 39 | 0 (0.0%) | 18 (46.2%) | 0 (0.0%) | 0 (0.0%) |
| `specs/client/render-pipeline.md` | 16 | 7 (43.8%) | 7 (43.8%) | 0 (0.0%) | 1 (6.2%) |
| `specs/client/ui.md` | 13 | 2 (15.4%) | 8 (61.5%) | 0 (0.0%) | 0 (0.0%) |
| `specs/combat/damage.md` | 126 | 0 (0.0%) | 74 (58.7%) | 0 (0.0%) | 0 (0.0%) |
| `specs/combat/hit.md` | 49 | 0 (0.0%) | 41 (83.7%) | 0 (0.0%) | 0 (0.0%) |
| `specs/data/calc-expressions.md` | 50 | 8 (16.0%) | 12 (24.0%) | 0 (0.0%) | 1 (2.0%) |
| `specs/drlg/levels.md` | 48 | 0 (0.0%) | 28 (58.3%) | 0 (0.0%) | 0 (0.0%) |
| `specs/drlg/rooms.md` | 91 | 0 (0.0%) | 53 (58.2%) | 0 (0.0%) | 0 (0.0%) |
| `specs/items/affixes.md` | 46 | 0 (0.0%) | 18 (39.1%) | 0 (0.0%) | 0 (0.0%) |
| `specs/items/generation.md` | 72 | 0 (0.0%) | 31 (43.1%) | 0 (0.0%) | 0 (0.0%) |
| `specs/items/properties.md` | 37 | 0 (0.0%) | 21 (56.8%) | 0 (0.0%) | 0 (0.0%) |
| `specs/items/quality.md` | 48 | 0 (0.0%) | 23 (47.9%) | 0 (0.0%) | 0 (0.0%) |
| `specs/items/treasure.md` | 85 | 0 (0.0%) | 62 (72.9%) | 0 (0.0%) | 0 (0.0%) |
| `specs/missiles/missiles.md` | 117 | 0 (0.0%) | 55 (47.0%) | 0 (0.0%) | 0 (0.0%) |
| `specs/monsters/ai.md` | 119 | 0 (0.0%) | 39 (32.8%) | 0 (0.0%) | 0 (0.0%) |
| `specs/render/map-preview.md` | 14 | 0 (0.0%) | 11 (78.6%) | 0 (0.0%) | 0 (0.0%) |
| `specs/sim/intents-events.md` | 73 | 0 (0.0%) | 40 (54.8%) | 0 (0.0%) | 0 (0.0%) |
| `specs/sim/rng.md` | 26 | 8 (30.8%) | 9 (34.6%) | 2 (7.7%) | 4 (15.4%) |
| `specs/sim/stat-lists.md` | 86 | 0 (0.0%) | 46 (53.5%) | 0 (0.0%) | 0 (0.0%) |
| `specs/sim/stats.md` | 38 | 0 (0.0%) | 19 (50.0%) | 0 (0.0%) | 0 (0.0%) |
| `specs/sim/tick.md` | 58 | 0 (0.0%) | 38 (65.5%) | 0 (0.0%) | 14 (24.1%) |
| `specs/sim/unit-order.md` | 38 | 0 (0.0%) | 24 (63.2%) | 0 (0.0%) | 12 (31.6%) |
| `specs/sim/units.md` | 41 | 0 (0.0%) | 16 (39.0%) | 0 (0.0%) | 0 (0.0%) |
| `specs/skills/levels.md` | 47 | 0 (0.0%) | 37 (78.7%) | 0 (0.0%) | 3 (6.4%) |
| `specs/world/cube.md` | 49 | 0 (0.0%) | 31 (63.3%) | 0 (0.0%) | 0 (0.0%) |
| `specs/world/quests.md` | 81 | 0 (0.0%) | 54 (66.7%) | 0 (0.0%) | 0 (0.0%) |
| `specs/world/waypoints.md` | 61 | 0 (0.0%) | 36 (59.0%) | 0 (0.0%) | 0 (0.0%) |
| **total** | 2704 | 274 (10.1%) | 1104 (40.8%) | 146 (5.4%) | 180 (6.7%) |

Claims went from 386 to 1520.

Totals:
- unit: 248 → 1072
- game: 144 → 150
- trace: 2 → 30
- verified: 146 → 180 (5.4% → 6.7%)

Per crate (`Covers:` lines / `#[test]` functions):

| Crate | Claims before | Claims after | Tests |
|---|---:|---:|---:|
| `conformance` | 0 | 4 | 11 |
| `d2-proto` | 0 | 12 | 19 |
| `d2-server` | 0 | 34 | 43 |
| `d2-sim` | 11 | 442 | 498 |
| `d2-client` | 76 | 139 | 161 |

**Where the new verified units come from:**
- **Trace tier (runs in CI, recordings in `traces/`):** the
  sim-0006/0007/0008 replays claim tick.md (14 units) and unit-order.md
  (12 units). The rng replay adds rng.md §3 r2 and §3 r4.
- **Game tier (`#[ignore]`, counts only while its latest local run
  passes, `COVERAGE.md` §3):**
  - `d2-client` `frames::all_live_frame_sets_build_and_pack` covers
    render-pipeline §A2 and assets §A3.
  - `d2-sim` `skills::tests::real_skill_vectors` covers levels.md §3.1 r2,
    §3.2 and §4.
  - `skills::special::codes_match_game_tables` covers calc-expressions §5.

  A local session should run these three with `D2_GAME_DIR` set and record
  the results in the HANDOFF §5 queue. This session did not edit HANDOFF.

## 3. Findings from reading the tests against the rules

These are inputs for spec sessions. Nothing was changed for any of them.
- **waypoints.md §7 r2 and edge case 6:** picking the waypoint's own level
  should stop after the interact reset. `close_and_validation` expects
  `warp 1 13`. The spec and the code disagree, so no claim was made.
- **damage.md test vector `pct(0x200000, 50, 0x30000)`:** the spec says 34
  and the §0 rule gives 533. The test asserts 533, and the spec fix is
  already queued.
- **cube.md §6.4 vs §7.1 r4:** `v11_item_level_stored_back` passes without
  the store-back. A vector that mixes a non-zero ilvl and plvl would tell
  the two rules apart.
- **cube.md §2 r1/r2:** `put_item_check` and `cube_check` live in the test
  fake, so they are not claimed.
- **rooms.md edge case 4 / OQ 4:** `client_arrays_sorted` checks a
  placeholder sort by client id.
- **tick.md §5.2 r3:** only one recorded `timer_set` asks for a past frame,
  so the trace tests don't claim this rule or edge case 5.
- **unit-order.md §3.2:** the replay applies the timer cancels from the
  recording, so the free routine's own cancels are not checked by the
  trace.
- **Behavior that tests check but no rule states:**
  - d2rs strictness errors: `MissingDt1`, `MissingWallRemap`,
    `NonAsciiName`, `Fatal::DivideByZero` for a zero `*divisor`, and the
    `CanonicalPath` refusal classes;
  - the atlas and pool refusals;
  - the controls binding errors;
  - the 31-bit wrap in `driver_first_use_and_mask`;
  - `Unhandled::NullSrvDo` / `SrvHit` logging;
  - the map-preview view clipping and verify verdicts.

## 4. Rules with no test at all (next test-writing targets)

`python3 tools/coverage.py` after this branch: every unit no claim names.
Unit IDs are as in `COVERAGE.md` §1.

- `specs/client/assets.md` (5): §a4-residency text, §a4-residency r1, §a4-residency r3, §a6-writes, §b-original-behavior-to-reproduce-not-specified-here
- `specs/client/audio.md` (4): §a1-decode-path text, §a1-decode-path r1, §a1-decode-path r3, §b-original-behavior-to-reproduce-not-specified-here
- `specs/client/bridge.md` (21): §1 r1, §1 r2, §1 r3, §1 r4, §2 r1, §2 r5, §3 r1, §3 r2, §3 r3, §3 r4, §4 r5, §5 r1, §5 r2, §6 r2, §7 r2, §7 r4, §8 r2, §8 r4, §9 r2, §9 r3, §edge-cases-original-bugs
- `specs/client/render-pipeline.md` (9): §a1-layers-of-the-pipeline, §a7-composite-units-cof text, §a7-composite-units-cof r1, §a7-composite-units-cof r2, §a7-composite-units-cof r3, §a7-composite-units-cof r4, §a9-gpu-compute-compositor, §a10-verify-harness-extension, §b-original-behavior-to-reproduce-not-specified-here
- `specs/client/ui.md` (5): §a1-why-not-bevy-ui, §a3-text, §a5-logical-resolution, §b-original-behavior-to-reproduce-not-specified-here, §edge-cases-original-bugs
- `specs/combat/damage.md` (52): §0 text, §1, §2 text, §2 r1, §2 r2, §2 r3, §3 text, §3 r1, §3.1 text, §3.1 r1, §3.1 r3, §3.1 r8, §3.1 r14, §3.2 text, §3.2 r3, §4 text, §4.3, §4.4 r5, §4.6 r1, §4.6 r2, §4.6 r6, §5.1 r2, §5.1 r3, §5.1 r4, §5.1 r5, §5.1 r6, §5.2 text, §5.2 r3, §5.2 r4, §5.2 r5, §5.2 r6, §5.2 r7, §5.2 r8, §5.2 r9, §5.2 r10, §5.2 r12, §5.2 r13, §5.4, §5.5 text, §5.6 r1, §6.2 text, §6.2 r1, §6.2 r5, §6.2 r7, §7.1, §7.2, §8, §9 r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r6, §edge-cases-original-bugs r7
- `specs/combat/hit.md` (8): §2 text, §3.2 r3, §4 text, §4 r6, §5 text, §6.1 r1, §6.2 text, §edge-cases-original-bugs r6
- `specs/combat/vitals.md` (19): §1, §2 text, §2.1, §3 text, §3 r1, §3 r2, §3 r3, §3 r4, §3 r5, §3 r6, §3 r7, §4.1, §4.2, §4.3, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5
- `specs/data/calc-expressions.md` (38): §rules text, §1.1, §1.2, §1.3, §1.4, §2.1, §2.2, §3.1 text, §3.1 r2, §3.4, §4.1 text, §4.1 r1, §4.1 r2, §4.1 r3, §4.1 r4, §4.3 text, §4.3 r1, §4.3 r2, §4.3 r3, §4.4, §4.5 text, §4.5 r1, §4.5 r2, §4.5 r3, §4.5 r4, §4.6 text, §4.6 r1, §4.6 r2, §4.6 l2 r1, §4.6 l2 r2, §4.6 l2 r3, §edge-cases-original-bugs, §d2rs-policy-proposed-not-yet-logged-in-docs-plan-md-1-2-4-6-implemented r1, §d2rs-policy-proposed-not-yet-logged-in-docs-plan-md-1-2-4-6-implemented r2, §d2rs-policy-proposed-not-yet-logged-in-docs-plan-md-1-2-4-6-implemented r3, §d2rs-policy-proposed-not-yet-logged-in-docs-plan-md-1-2-4-6-implemented r4, §d2rs-policy-proposed-not-yet-logged-in-docs-plan-md-1-2-4-6-implemented r5, §d2rs-policy-proposed-not-yet-logged-in-docs-plan-md-1-2-4-6-implemented r6
- `specs/data/callbacks.md` (3): §7, §9, §edge-cases-original-bugs
- `specs/data/field-types.md` (9): §1, §9, §10 text, §10 r1, §10 r2, §10 r3, §10 r4, §10 r5, §edge-cases-original-bugs
- `specs/data/fixups.md` (7): §2 text, §2 r1, §2 r2, §4, §7, §11 text, §edge-cases-original-bugs
- `specs/data/loading.md` (36): §rules text, §1 r1, §1 r2, §1 r3, §3.1 text, §3.1 r1, §3.1 r2, §3.1 r3, §3.2 text, §3.2 r1, §3.2 r2, §3.2 r3, §3.3, §3.4, §3.5, §4.1, §4.2 text, §4.2 r3, §5, §7.1, §7.2, §7.3, §7.4, §9, §10 r1, §10 r2, §10 r4, §10 r6, §10 r7, §edge-cases-original-bugs, §d2-data-policy text, §d2-data-policy r1, §d2-data-policy r2, §d2-data-policy r4, §d2-data-policy r5, §d2-data-policy r6
- `specs/data/patch-layers.md` (6): §1, §8, §10, §11, §12, §edge-cases-original-bugs
- `specs/data/runtime-maps.md` (1): §edge-cases-original-bugs
- `specs/data/schema.md` (6): §3 r1, §3 r2, §3 r3, §3 r4, §4, §edge-cases-original-bugs
- `specs/data/txt-format.md` (11): §1, §5 r1, §6 text, §6 r1, §6 r4, §7 text, §7 r1, §7 r2, §8, §10, §edge-cases-original-bugs
- `specs/drlg/levels.md` (20): §1, §2 r1, §2 r2, §2 r3, §3 text, §3 r1, §3 r5, §3 r6, §3 r7, §4 r2, §5 r2, §6 r2, §7 r1, §7 r5, §9 r5, §10 text, §10 r1, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5
- `specs/drlg/maze.md` (53): §1 r1, §1 r2, §1 r3, §2 r1, §2 r2, §2 r3, §2 r4, §2 r5, §2 r6, §3 text, §3 r1, §3 r2, §3 r3, §4 text, §4 r1, §4 r2, §4 r3, §4 r4, §4 r5, §5 text, §5 r1, §5 r2, §5 r3, §6, §7 text, §7 r1, §7 r2, §7 r3, §7 r4, §7 r5, §7 r6, §7 r7, §7 r8, §8 text, §8 r1, §8 r2, §8 r3, §8 r4, §9 text, §9 r1, §9 r2, §9 r3, §9 r4, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r7, §edge-cases-original-bugs r8, §edge-cases-original-bugs r9, §edge-cases-original-bugs r10
- `specs/drlg/outdoor-tilesub.md` (30): §1 r1, §1 r2, §1 r3, §1 r4, §2.1, §2.2 text, §2.2 r1, §2.2 r2, §2.2 r3, §2.2 r4, §2.3, §3, §4 text, §4.1, §4.2 text, §4.2 r1, §4.2 r2, §4.2 r3, §4.2 r4, §4.3, §4.4 text, §4.4 r1, §4.4 r2, §4.4 r3, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6
- `specs/drlg/outdoor.md` (72): §rules text, §1, §2 text, §2.1, §2.2, §2.3 text, §2.3 r1, §2.3 r2, §2.3 r3, §2.3 r4, §2.4, §2.5, §2.6, §2.7, §3 text, §3 r1, §3 r2, §3 r3, §3 r4, §4, §5.1 text, §5.1 r1, §5.1 r2, §5.1 r3, §5.2, §5.3, §5.4, §5.5, §6 text, §6 r1, §6 r2, §6 r3, §6 r4, §6 r5, §7 text, §7 r1, §7 r2, §7 r3, §7 r4, §7 r5, §7 r6, §7.1, §7.2 r1, §7.2 r2, §7.2 r3, §7.3 r1, §7.3 r2, §7.3 r3, §7.4, §7.5 text, §7.5 r1, §7.5 r2, §7.5 r3, §7.5 r4, §7.6, §8, §9.1, §9.2, §9.3, §10, §11, §12.1, §12.2, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r7, §edge-cases-original-bugs r8, §edge-cases-original-bugs r9
- `specs/drlg/preset.md` (60): §1, §2 r1, §2 r2, §2 r3, §3.1 text, §3.1 r1, §3.1 r2, §3.1 r3, §3.1 r4, §3.2 text, §3.2 r1, §3.2 r2, §3.2 r3, §3.2 r4, §3.3, §4 text, §4 r1, §4 r2, §4 r3, §4 r4, §5.1, §5.2 text, §5.2 r1, §5.2 r2, §5.2 r3, §5.2 r4, §5.2 r5, §5.2 r6, §5.2 r7, §5.2 r8, §5.2 r9, §5.2 r10, §5.2 r11, §5.3, §6 text, §6 r1, §6 r2, §6 r3, §6 r4, §6 r5, §6 r6, §6 r7, §6 r8, §6 r9, §6 r10, §7, §8 text, §8 r1, §8 r2, §8 r3, §9 text, §9 r1, §9 r2, §9 r3, §9 r4, §10, §11, §12, §13, §edge-cases-original-bugs
- `specs/drlg/rooms.md` (38): §1, §2 text, §2 r1, §2 r5, §3 text, §4.2, §4.4 r1, §4.4 r3, §4.4 r4, §4.5, §4.6, §5 text, §5 r1, §5 r7, §5 r8, §6 r4, §9.1, §9.2 text, §9.2 r1, §9.2 r2, §9.2 r3, §9.3 r2, §9.4 text, §9.4 r2, §9.5 r4, §9.6 text, §9.8, §9.9, §9.10, §10.1, §10.2, §10.6, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6
- `specs/formats/animdata.md` (7): §rules text, §1, §4 text, §5, §7, §edge-cases-original-bugs, §expfield-d2
- `specs/formats/cof.md` (1): §rules text
- `specs/formats/dc6.md` (3): §rules text, §placement-informational, §edge-cases-original-bugs
- `specs/formats/dcc.md` (1): §edge-cases-original-bugs
- `specs/formats/ds1.md` (1): §edge-cases-original-bugs
- `specs/formats/dt1.md` (2): §rules text, §edge-cases-original-bugs
- `specs/formats/font-tbl.md` (2): §rules text, §edge-cases-original-bugs
- `specs/formats/mpq-tables.md` (2): §c-huffman-weight-tables, §d-ima-adpcm-tables
- `specs/formats/mpq.md` (20): §rules text, §6, §7, §11 r1, §11 r2, §11 r3, §11 l2 r1, §11 l2 r2, §11 l2 r3, §11 l3 r1, §11 l3 r2, §11 l3 r3, §11 l4 r1, §11 l4 r2, §11 l4 r3, §11 l4 r4, §11 l4 r5, §12 text, §12 r1, §edge-cases-original-bugs
- `specs/formats/tbl.md` (2): §rules text, §edge-cases-original-bugs
- `specs/items/affixes.md` (28): §1 r2, §1 r3, §3 text, §3 r1, §3 r4, §3 r5, §3 r7, §3.1, §4.1 r1, §4.1 r2, §4.1 r3, §4.1 r4, §4.3, §5 text, §5 r0, §6 text, §6 r1, §6 r2, §7 r1, §8 r1, §8 r3, §8 r4, §9, §10 text, §10 r1, §10 r2, §10 r3, §edge-cases-original-bugs r5
- `specs/items/generation.md` (41): §1.1, §1.2, §1.3, §1.4, §1.5, §1.6, §2 r2, §2 r3, §2 r4, §2 r5, §3 text, §3 r6, §3 r7, §3 r8, §3 r9, §3 r10, §4 text, §4 r2, §4 r3, §4 r4, §4 r5, §5 r1, §5 r3, §6.1 text, §6.1 r1, §6.1 r2, §6.1 r4, §6.1 r5, §6.1 r6, §6.2 text, §6.2 r1, §6.2 r2, §6.2 r5, §7.1 text, §7.1 r1, §7.1 r2, §8.1 text, §8.1 r1, §8.1 r2, §8.1 r3, §9 r2
- `specs/items/properties.md` (16): §1 r1, §1 r2, §2, §4.3, §5 r4, §5 r5, §5 r7, §5 r10, §5 r11, §5 r12, §6, §7, §9 text, §9 r1, §9 r3, §9 r4
- `specs/items/quality.md` (25): §1, §2, §3 text, §3 r2, §3 r3, §4 text, §4 r1, §4 r2, §4 r3, §4 r4, §4 r5, §4 r6, §5 text, §5 r1, §5 r3, §5 r5, §6 r4, §6 r5, §6 r6, §7 r1, §8 r1, §9 text, §9 r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5
- `specs/items/treasure.md` (23): §1.1, §1.2, §1.5 text, §3.3, §3.5, §3.6, §4 text, §4 r6, §5.3 text, §5.3 r1, §5.3 r5, §5.3 r6, §5.3 r7, §5.4 text, §5.6, §5.7 r6, §6 text, §6 r4, §7 r1, §7 r2, §edge-cases-original-bugs r1, §edge-cases-original-bugs r6, §edge-cases-original-bugs r8
- `specs/missiles/missiles.md` (62): §r1-data-the-server-keeps-per-missile text, §r1-data-the-server-keeps-per-missile r1, §r1-data-the-server-keeps-per-missile r2, §r1-data-the-server-keeps-per-missile r4, §r1-data-the-server-keeps-per-missile r5, §r2-1-parameter-record-d2moo-d2missilestrc-0x5c-bytes, §r2-2-entry-points, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r2, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r3, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r9, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r11, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r15, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r16, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r18, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r20, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r21, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r23, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r24, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r26, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r27, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r29, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r30, §r2-4-client-message, §r3-per-tick-dispatch text, §r3-per-tick-dispatch r2, §r3-per-tick-dispatch r3, §r3-per-tick-dispatch r4, §r3-per-tick-dispatch r7, §r3-per-tick-dispatch r8, §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 text, §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r1, §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r3, §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r5, §r4-default-flight-server-do-1-0x005b0bc0-0x005ae1f0 r10, §r4-1-movement-in-fixed-point text, §r4-1-movement-in-fixed-point r1, §r4-1-movement-in-fixed-point r4, §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler text, §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r3, §r5-hit-handler-0x005adf10-d2moo-missmode-srvdmghithandler r4, §r6-1-order-1-14d-0x005adf10-step-7 r1, §r6-1-order-1-14d-0x005adf10-step-7 r2, §r6-1-order-1-14d-0x005adf10-step-7 r3, §r7-lifetime-and-expiry r4, §r8-1-pierce-test-at-creation-0x0059f940-1-14d-confirmed text, §r9-2-tsv-columns-srvdo-tsv-srvhit-tsv, §r9-3-seeded-sub-missile-helper-0x005a9820-d2moo-missmode-createmissilewithcollisioncheck-1-14d-confirmed text, §r9-3-seeded-sub-missile-helper-0x005a9820-d2moo-missmode-createmissilewithcollisioncheck-1-14d-confirmed r1, §r9-3-seeded-sub-missile-helper-0x005a9820-d2moo-missmode-createmissilewithcollisioncheck-1-14d-confirmed r2, §r9-3-seeded-sub-missile-helper-0x005a9820-d2moo-missmode-createmissilewithcollisioncheck-1-14d-confirmed r3, §r9-3-seeded-sub-missile-helper-0x005a9820-d2moo-missmode-createmissilewithcollisioncheck-1-14d-confirmed r5, §r9-4-missile-seed-re-initialisations-rng-md-5-3-list, §r10-behaviour-of-the-recorded-missiles text, §r10-behaviour-of-the-recorded-missiles r1, §r10-behaviour-of-the-recorded-missiles r2, §r10-behaviour-of-the-recorded-missiles r3, §r10-behaviour-of-the-recorded-missiles r4, §r11-missiles-txt-columns-and-their-server-use, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r8, §edge-cases-original-bugs r10
- `specs/monsters/ai.md` (80): §1.3 text, §1.7, §2.1 text, §2.1 r1, §2.1 r2, §2.1 r3, §2.1 r4, §2.2 text, §2.2 r2, §2.2 r3, §2.3 r1, §2.3 r2, §2.4 text, §2.4 r2, §2.4 r3, §3.1, §3.3 text, §3.3 r1, §4, §5.1, §5.2 text, §5.2 r1, §5.2 r2, §5.2 r3, §5.2 r4, §5.3, §5.4, §7.1, §9 text, §9.1, §9.3 text, §9.4 text, §9.4 r2, §9.4 r3, §9.4 r4, §9.4 r5, §9.5 text, §9.5 r1, §9.6 text, §9.6 r1, §9.6 r2, §9.6 r3, §9.6 r4, §9.6 r5, §9.6 r6, §9.6 r7, §9.7 text, §9.7 r1, §9.7 r2, §9.7 r3, §9.7 r7, §9.8 text, §9.8 r2, §9.9, §9.10 text, §9.10 r1, §9.10 r2, §9.10 r3, §9.11, §9.12 text, §9.12 r1, §9.12 r2, §9.12 r3, §9.12 r4, §9.13 text, §9.13 r1, §9.13 r2, §9.13 r3, §9.13 r4, §9.13 r5, §9.13 r6, §9.13 r7, §9.14, §10, §edge-cases-original-bugs r3, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r7, §edge-cases-original-bugs r8, §edge-cases-original-bugs r9
- `specs/monsters/init.md` (112): §1, §2, §3, §4 text, §4 r1, §4 r2, §4 r3, §4 r4, §4 r5, §4 r6, §5 text, §5 r1, §5 r2, §5 r3, §5 r4, §5 r5, §5 r6, §5 r7, §6 text, §6 r1, §6 r2, §6 r3, §6 r4, §6 r5, §6 r6, §6 r7, §6 r8, §6 r9, §6 r10, §6 r11, §6 r12, §6 r13, §6 r14, §6 r15, §6 r16, §6 r17, §7 text, §7 r1, §7 r2, §7 r3, §7 r4, §8.1, §8.2 text, §8.2 r1, §8.2 r2, §8.2 r3, §8.2 r4, §9 r1, §9 r2, §9 r3, §10 text, §10 r1, §10 r2, §11, §12 text, §12 r1, §12 r2, §12 r3, §13, §14.1, §14.2, §15, §16 text, §16.1 r1, §16.1 r2, §16.1 r3, §16.2 text, §16.2 r1, §16.2 r2, §16.2 r3, §16.2 r4, §17 text, §17 r1, §17 r2, §17 r3, §17.1, §17.2, §17.3 r1, §17.3 r2, §17.3 r3, §18 text, §18 r1, §18 r2, §19 text, §19.1, §19.2, §19.3, §19.4, §19.5, §19.6, §20 text, §20 r1, §20 r2, §20 r3, §20 r4, §20 r5, §21, §22, §23, §24, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r7, §edge-cases-original-bugs r8, §edge-cases-original-bugs r9, §edge-cases-original-bugs r10, §edge-cases-original-bugs r11, §edge-cases-original-bugs r12
- `specs/monsters/population.md` (165): §1 r1, §1 r2, §1 r3, §2.1 r1, §2.1 r2, §2.1 r3, §2.1 r4, §2.2, §2.3 text, §2.3 r1, §2.3 r2, §2.3 r3, §2.4 text, §2.4 r1, §2.4 r2, §2.4 r3, §2.4 r4, §2.4 r5, §2.5 text, §2.5 r1, §2.5 r2, §2.5 r3, §2.5 r4, §3.1 text, §3.1 r1, §3.1 r2, §3.1 r3, §3.1 r4, §3.1 r5, §3.1 r6, §3.2 text, §3.2 r1, §3.2 r2, §3.2 r3, §3.3, §3.4 r1, §3.4 r2, §4 text, §4 r1, §4 r2, §4 r3, §4 r4, §4 r5, §5 text, §5 r1, §5 r2, §5 r3, §6.1, §6.2 r1, §6.2 r2, §6.2 r3, §6.3 text, §6.3 r1, §6.3 r2, §6.3 r3, §6.3 r4, §6.3 r5, §6.4, §6.5 r1, §6.5 r2, §6.5 r3, §6.5 r4, §7 text, §7 r1, §7 r2, §7 r3, §7 r4, §7 r5, §7 r6, §7 r7, §7 r8, §8 text, §8 r1, §8 r2, §8 r3, §9 text, §9.1 r1, §9.1 r2, §9.1 r3, §9.1 r4, §9.2, §9.3 text, §9.3 r1, §9.3 r2, §9.3 r3, §9.3 r4, §9.4 r1, §9.4 r2, §9.5, §9.6 r1, §9.6 r2, §9.6 r3, §9.6 r4, §9.6 r5, §9.6 r6, §10 text, §10.1 r1, §10.1 r2, §10.2 r1, §10.2 r2, §10.2 r3, §10.2 r4, §10.3 text, §10.3 r1, §10.3 r2, §11.1, §11.2 text, §11.2 r1, §11.2 r2, §11.2 r3, §11.2 r4, §11.3 r1, §11.3 r2, §11.3 r3, §11.4 text, §11.4 r1, §11.4 r2, §11.4 r3, §11.4 r4, §11.4 r5, §11.4 r6, §11.5 text, §11.5 r1, §11.5 r2, §11.5 r3, §11.5 r4, §11.5 r5, §11.6 r1, §11.6 r2, §11.6 r3, §12 text, §12 r1, §12 r2, §12 r3, §12 r4, §12 r5, §12 r6, §13 text, §13 r1, §13 r2, §13 r3, §13 r4, §13 r5, §14 r1, §14 r2, §14 r3, §14 r4, §14 r5, §14 r6, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r7, §edge-cases-original-bugs r8, §edge-cases-original-bugs r9, §edge-cases-original-bugs r10, §edge-cases-original-bugs r11, §edge-cases-original-bugs r12, §edge-cases-original-bugs r13, §edge-cases-original-bugs r14, §edge-cases-original-bugs r15, §edge-cases-original-bugs r16
- `specs/render/map-preview.md` (3): §draw-order r1, §draw-order r3, §edge-cases-original-bugs
- `specs/sim/intents-events.md` (33): §1 text, §1 r5, §2.1 r3, §2.1 r6, §2.2 text, §2.2 r5, §2.3 text, §2.3 r4, §2.3 r5, §2.4 r5, §2.4 r6, §2.4 r8, §2.5, §3.1 r2, §3.2 r3, §3.2 r4, §3.2 r5, §3.2 r6, §3.3 r4, §3.4 r2, §3.4 r3, §4 r1, §4 r2, §4 r3, §4 r4, §4 r5, §6 r1, §6 r2, §6 r3, §6 r4, §6 r5, §edge-cases-original-bugs r1, §edge-cases-original-bugs r8
- `specs/sim/rng.md` (17): §1 r1, §1 r2, §3 r3, §3 r5, §3 r6, §4 r2, §4 r3, §5.2, §5.3, §5.4, §5.5, §6, §7, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4
- `specs/sim/stat-lists.md` (40): §1, §2, §3 r1, §3 r2, §3 r3, §3 r4, §4 r1, §4 r2, §4 r3, §5 text, §5 r2, §6.1 text, §6.1 r2, §6.2, §6.3, §6.4 text, §6.4 r1, §6.4 r2, §6.4 r3, §6.4 r4, §6.4 r5, §6.4 r6, §7.2 text, §7.2 r1, §8.1 r1, §8.1 r2, §8.2 r1, §8.2 r3, §8.3 r1, §8.3 r3, §9.3, §10.1 text, §10.1 l2 r1, §10.1 l2 r4, §10.2, §10.3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r8
- `specs/sim/stats.md` (19): §1 r2, §1 r4, §2 r1, §2 r2, §2 r3, §2 r4, §2 r5, §2 r6, §3, §4.2, §5 text, §6 text, §6.3, §7, §9 r1, §9 r4, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6
- `specs/sim/tick.md` (20): §1 r6, §3 text, §3 r2, §3 r3, §5.1, §5.2 text, §5.2 r4, §5.3, §5.4 r4, §5.5 text, §5.5 r4, §5.6, §5.7, §6 text, §6 r1, §6 r2, §6 r3, §7, §8, §edge-cases-original-bugs r3
- `specs/sim/unit-order.md` (14): §1 r1, §1 r4, §1 r5, §1 r6, §2 text, §3 r2, §4 r1, §5 r1, §6 r1, §7 r1, §7 r3, §9, §10, §edge-cases-original-bugs r2
- `specs/sim/units.md` (25): §1, §2, §3.1 text, §3.1 r2, §3.1 r3, §3.1 r5, §3.1 r7, §3.1 r8, §3.2, §4.1, §4.3, §4.4, §4.5, §4.6, §5 r1, §5 r3, §5 r4, §6 text, §6.1, §6.2, §6.4, §6.5, §7, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5
- `specs/skills/levels.md` (10): §1 l2 r1, §3.1 text, §3.3 text, §6 r4, §6.4 text, §6.4 r1, §6.4 r4, §6.4 r5, §edge-cases-original-bugs r7, §edge-cases-original-bugs r9
- `specs/skills/use.md` (64): §1 text, §1 r1, §1 r2, §1 r3, §1 r4, §1 r5, §1 r6, §2 text, §2 r1, §2 r2, §2 r3, §2 r4, §2 r5, §2 r6, §3 text, §3 r1, §3 r2, §3 r3, §3 r4, §3 r5, §3 r6, §4 text, §4 r1, §4 r2, §4 r3, §4 r4, §5.1, §5.2 text, §5.2 r1, §5.2 r2, §5.2 r3, §5.2 r4, §5.3 r1, §5.3 r2, §5.3 r3, §5.3 r4, §5.3 r5, §5.3 r6, §5.3 r7, §5.4 text, §5.4 r1, §5.4 r2, §5.4 r3, §5.4 r4, §5.4 r5, §5.4 r6, §5.4 r7, §5.4 r8, §5.4 r9, §5.4 r10, §6, §7, §8, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r7, §edge-cases-original-bugs r8, §edge-cases-original-bugs r9, §edge-cases-original-bugs r10, §edge-cases-original-bugs r11
- `specs/world/cube.md` (18): §2 text, §2 r1, §2 r2, §3 r1, §3 r2, §6 text, §6.1 r1, §6.1 r2, §6.1 r3, §6.1 r6, §7 text, §7.1 r1, §7.5 text, §7.6 text, §7.6 r1, §7.6 r5, §10, §edge-cases-original-bugs
- `specs/world/npc.md` (71): §1.1 text, §1.1 r1, §1.1 r2, §1.1 r3, §1.1 r4, §1.1 r5, §1.2, §2 text, §2 r1, §2 r2, §2 r3, §2 r4, §2 l2 r1, §2 l2 r2, §2 l2 r3, §2 l2 r4, §2 l2 r5, §3, §4, §5 text, §5 r1, §5 r2, §5 r3, §5 r4, §5 r5, §5 r6, §6 text, §6 r1, §6 r2, §6 r3, §6 r4, §6 r5, §6 r6, §7 text, §7.1 text, §7.1 r1, §7.1 r2, §7.1 r3, §7.1 r4, §7.2, §7.3 text, §7.3 r1, §7.3 r2, §7.3 r3, §7.3 r4, §7.3 r5, §7.3 r6, §7.3 r7, §7.3 r8, §7.4 text, §7.4 r1, §7.4 r2, §7.4 r3, §7.4 r4, §7.5, §8.1, §8.2, §8.3, §9, §10, §edge-cases-original-bugs text, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r7, §edge-cases-original-bugs r8, §edge-cases-original-bugs r9, §edge-cases-original-bugs r10
- `specs/world/quests.md` (27): §1.2, §1.3, §1.4, §2.1, §2.2, §2.3 r4, §3 r4, §4.1, §4.2, §4.4 r2, §4.5, §5 r2, §5 r4, §6.1 text, §6.2 text, §6.6, §7.1, §9.1, §9.2, §9.3, §9.4, §9.5, §10.2, §10.7, §11, §edge-cases-original-bugs r3, §edge-cases-original-bugs r8
- `specs/world/vendors.md` (108): §1 r1, §1 r2, §1 r3, §1 r4, §1 r5, §2, §3 text, §3 r1, §3 r2, §3 r3, §3 r4, §3 r5, §3.1 text, §3.1 r1, §3.1 r2, §3.1 r3, §3.1 r4, §3.1 r5, §3.2, §3.3, §3.4, §4 text, §4 r1, §4 r2, §4 r3, §5.1 text, §5.1 r1, §5.1 r2, §5.1 r3, §5.1 r4, §5.1 r5, §5.1 r6, §5.1 r7, §5.1 r8, §5.2, §5.3, §5.4, §5.5, §6 text, §6 r1, §6 r2, §6 r3, §6 r4, §6 r5, §7.1 text, §7.1 r1, §7.1 r2, §7.1 r3, §7.1 r4, §7.1 r5, §7.1 r6, §7.1 r7, §7.1 r8, §7.1 r9, §7.1 r10, §7.1 r11, §7.1 r12, §7.2 text, §7.2 r1, §7.2 r2, §7.2 r3, §7.2 r4, §7.2 r5, §7.2 r6, §7.2 r7, §7.2 r8, §7.2 r9, §7.2 r10, §8.1 text, §8.1 r1, §8.1 r2, §8.1 r3, §8.1 r4, §8.1 r5, §8.1 r6, §8.2, §9.1, §9.2 text, §9.2 r0, §9.2 l2 r1, §9.2 l2 r2, §9.2 l2 r3, §9.2 l2 r4, §9.2 l2 r5, §9.2 l2 r6, §9.2 l2 r7, §9.2 l2 r8, §9.2 l2 r9, §9.2 l2 r10, §9.2 l2 r11, §9.2 l2 r12, §9.2 l2 r13, §9.3, §9.4, §edge-cases-original-bugs text, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2, §edge-cases-original-bugs r3, §edge-cases-original-bugs r4, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r7, §edge-cases-original-bugs r8, §edge-cases-original-bugs r9, §edge-cases-original-bugs r10, §edge-cases-original-bugs r11, §edge-cases-original-bugs r12, §edge-cases-original-bugs r13
- `specs/world/waypoints.md` (25): §3 r3, §4 text, §4 r1, §4 r2, §4 r3, §5 r2, §5.2 text, §6.3 r2, §7 r2, §7 r3, §7 r5, §7 r6, §7.1, §8 r1, §8 r2, §8 r3, §8 r4, §8 r5, §9, §10, §edge-cases-original-bugs text, §edge-cases-original-bugs r5, §edge-cases-original-bugs r6, §edge-cases-original-bugs r7, §edge-cases-original-bugs r8
