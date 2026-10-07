# Handoff: gap tests for the newly implemented specs — `claude/gaps-new-specs`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud test session, 2026-10-06, task class: tests from specs, medium
(M14). Base: `claude/tender-meitner-mphas3` at `edd9925`. Repo only, no
game files. Inputs: `specs/items/inventory.md`,
`specs/sim/path-placement.md` §7–§12, `specs/render/composition.md`,
`specs/render/capture.md`, and the units the spec-bodies sessions touched
(`monsters/ai.md`, `missiles/missiles.md`, `world/quests.md`,
`skills/use.md`). Path-placement §1–§6, `sim/pathing.md`,
`render/camera.md` and `render/sprite-placement.md` were left to the
parallel implementation sessions.

## 1. Coverage (unit tier, `python3 tools/coverage.py --summary`)

| Spec | Units | Before | After | Uncovered after |
|---|---|---|---|---|
| `specs/items/inventory.md` | 148 | 139 (93.9%) | 145 (98.0%) | 3 |
| `specs/sim/path-placement.md` | 84 | 43 (51.2%) | 46 (54.8%) | 38 (34 in §1–§6, parallel sessions) |
| `specs/world/quests.md` | 81 | 73 (90.1%) | 74 (91.4%) | 7 |
| `specs/render/composition.md` | 16 | 9 (56.2%) | 9 (56.2%) | 7 |
| `specs/render/capture.md` | 9 | 6 (66.7%) | 6 (66.7%) | 3 |
| `specs/monsters/ai.md` | 119 | 108 (90.8%) | 108 (90.8%) | 11 |
| `specs/missiles/missiles.md` | 117 | 104 (88.9%) | 104 (88.9%) | 13 |
| `specs/skills/use.md` | 64 | 64 (100%) | 64 (100%) | 0 |
| total (all specs, any tier) | 3106 | 2660 (85.6%) | 2670 (86.0%) | |

All new claims are unit tier; verified coverage is unchanged (nothing
here runs against 1.14d). `coverage.py --check`: 3,554 claims, 0 errors.

## 2. New tests (new files; one `mod` line each)

- `crates/d2-sim/src/path/gap_tests.rs` (`path/mod.rs`): §7.1 (the
  wrappers: max distance 50 reaches ring 49 and not 50, fallback argument
  of `0x0064E7B0`; `0x0064E7E0` steps by k and never falls back;
  `0x0064E810` needs the walk-back to the origin), §9 text (floor drop is
  one cell against 0x3E01: each of its six bits blocks, 0x8 does not, a
  neighbouring wall does not), §12.1 text (types 10 / 11, the tile
  relative to R, slot from bits 20–25 only).
- `crates/d2-sim/src/items/inventory/gap_tests.rs` (child of `tests.rs`,
  `#[path]` line; the fake gains `no_free_page0` so `0x0063CB00` can
  answer "no"): §1.4 r3, §2.4 r5 (link check kind 1 after the put, before
  the charm re-link), §4.3 text (every row of the hand table, for L = 4,
  5, 11, 12), §4.6 r3 (skip bypasses both requirement tests), edge case
  9 (all 512 occupancies of a 3 × 3 grid × item sizes up to 3 × 3: some
  fitting spot has weight > 0, and the player search fails exactly when
  nothing fits).
- `crates/d2-sim/src/items/moves/tests/gaps.rs`: §7.21 (0x4C is not in
  the handler table and is left alone at any length).
- `crates/d2-server/src/adapters/handlers/world/tests/gaps.rs`:
  `quests.md` §6.2 text (C→S 0x40 frames as one byte; it runs
  `0x00546040`: 0x28 first, 0x52 last).

Perturbation (M08), by hand, each reverted: `FREE_MAX_DISTANCE` 51,
fallback on in `free_point_step`, `ITEM_FLOOR` without 0x2000, the warp
letter swapped, `SWAP_OTHER_TO_PAGE` → 0, the §4.6 step 3 `skip` test
dropped, the §2.3 grid-edge weights set to 0, the §2.4 link check moved
after step 6: each failed exactly the matching new test.

## 3. Code fixes

None. Every new test passed against the existing code.

## 4. Not claimed (why)

### `items/inventory.md` (3)
- §1.1: a record-offset table; d2rs holds Rust structs with no byte
  layout (the cell index `y × width + x` is the one checkable fact, used
  by every grid test).
- §2.4 text: argument list; the "optional inventory (default: the
  owner's)" argument is not modelled (`place_in_page` always takes one).
- §6.1 r4: not implemented (`moves/deferred.rs` `TODO(spec: inventory-moves.md
  §6.1 rule 4, OQ9)`).

### `sim/path-placement.md` §7–§12 (4)
- §10 text, §12.2 text: caller lists.
- §10 r7: the position history is kept out of `d2-sim` by design; only
  absence could be tested.
- Edge cases text ("reproduced by default"); edge r1 (0x27 for a missing
  room) and r8 (§5.3) belong to the §1–§6 path core of the parallel
  sessions: the search tests' grid fake reproduces r1, the real provider
  does not exist yet.

### `world/quests.md` (7)
- §2.3 r4: game +0x10F4 holds the control (a struct field).
- §9.1, §9.2: `reward_item` / `delete_item` are seams with no provider
  (`wiring/economy/quest_items.rs`: "Not wired").
- §9.4: `read_clue` dispatches `bkd ` / `trs `, but the handler's item
  checks are not implemented (`d2-server` lists 0x3E as "§9.4
  (partial)", `NoOwner`). A claim would cover half the rule.
- §10.2: the Act I table (NPCs, messages, rewards, timers) is spread over
  the §10.3–§10.8 tests; no single test checks the table.
- §10.7: D2MOO-derived (the 4 → 5 move is a `TODO(quests §10.7)`).
- §11: catalogue only (OQ8).

### `render/composition.md` (7)
- §1 text, r1–r3: which 1.14d driver is the reference, and why. d2rs
  has one renderer; nothing to compare in a unit test.
- §3 r1 (camera origins and shake: `render/camera.md`, parallel
  session), §3 r3 (world then UI order; skipped world in open mode 3:
  `draw-order.md`, not implemented in `scene`).
- §7: DirectDraw differences, not implemented by design (`scene/frame.rs`
  header).

### `render/capture.md` (3)
- §1, §2, §3: the 1.14d configuration, the `EndScene` hook and the memory
  addresses the recorder reads. They are the recorder's
  (`tools/trace-recorder/record_frames.py`) side against the game; a
  claim belongs on its first recorded run (trace tier). The d2rs reader
  enforces only GDI (`capture_tests.rs` `raw_reader_is_strict`, video
  type 3), not 800 × 600, so a §1 unit claim would overclaim.

### Spec-bodies sessions (`ai.md`, `missiles.md`, `use.md`)
The two notes (`spec-bodies-ai-missiles.md`, `spec-bodies-quests-skills.md`)
added one body (`srvst` 18, `use.md` §8, already claimed) and the
catalogue check `missiles.md §R9.2` (claimed). Every unclaimed unit of
`ai.md` (11) and `missiles.md` (13) is the list of
`docs/handoff/gaps-combat-ai.md` "Not claimed", unchanged: still not
implemented, prose or data-file facts.

## 5. Spec questions

None new.

## 6. Gate

`sh tools/gate.sh all`: GATE PASS (all 13 steps, `d2-client` included).

## 7. Port to the new base

Merged `origin/claude/tender-meitner-mphas3` (the `wire-path-sim` seams):
`path/gap_tests.rs` now uses `path::coords::Point` and `drlg::TileRect`
(`SubPoint` / `RoomRect` are gone); every assertion and claim kept.
After the merge: `coverage.py --check` 3,873 claims, 0 errors;
`path-placement.md` 80/84, `inventory.md` 145/148, `quests.md` 74/81
(any tier); total 2,829/3,106 (91.1%). `cargo check --workspace
--all-targets --keep-going` clean; `sh tools/gate.sh all` GATE PASS.
