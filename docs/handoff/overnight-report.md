# Overnight report — 2026-10-09 (cloud, toward 1.14d match)

Run: 2026-10-08 ~22:00 UTC to 2026-10-09 06:00 UTC, coordinator plus up
to 8 cloud sessions (Sonnet by default; Opus for rendering seams).
Rules: `docs/handoff/overnight-loop.md`. Staging: `claude/specs-staging-7`.
Every merge went through the full gate (fmt, clippy -D warnings on
d2-sim / d2-server / d2-client / test-fixtures, nextest, coverage,
spec_index) before it was pushed, with one exception (see "Process").

## What merged (in order)

| Branch | What it brought |
|---|---|
| q-fix-audio-sounds | unit, mode, voice, footstep, state, missile, item and NPC sounds from the real tables |
| q-cloud-game (3 rounds) | object FrameCnt ×256; per-object animation phase from the client seed (REC-440); exporter tick alignment; component file weapon class per COF layer (REC-441); dir64 export; call-only cel slots; town NPCs think and walk their DS1 paths (REC-442) |
| q-fix-render-rest (many rows) | light list, quest byte glow, level 120 summit (REC-420), missile sight test, flat missiles, shaken-camera pick, player light colour from states, Den of Evil lights (REC-450), client missile create / update / path step (REC-451) and functions 1/4/5/6/8/11/23/25/43/49/60/63 (~600 of the install's missile rows), hit functions as non-zero (REC-452, a seam found on real data) |
| q-fix-items-play | equip-click dispatch, death drops, TP scroll refusal, vendor seams, save records, client item reader = server reader, format-0 gaps (code 242), REC-289 |
| q-fixture-migrate | real-data rigs G1–G3 (37/44 on the install), ScriptedClock at construction |
| q-fix-real-next | docs only: controls order to PC 1, stash repro superseded |
| q-fix-real-skills | Leap / Leap Attack / Whirlwind (REC-460) / sentries (REC-461) on the install |
| q-fix-audit-rest | belt tables, store grid (client), click sounds, NPC dialog capture, grid facts, join quest entry, client order, provisional cleanups |
| q-fix-ui-play-wiring | waypoint menu gates, HUD small items, belt hover, skill tree tips, shop store tip |

Final staging head: `c50da245` (gate: 7,433 tests, 0 failed; workspace
clippy without d2-client and depcheck clean). **Not merged:** q-facts-scenes
(`e86302ac`, walk/run scenes in 8 directions): it conflicts with
q-cloud-game's exporter changes in `tools/trace-recorder/facts_render.py`,
`specs/tools/facts-render.md` and `facts/render/sprites.tsv`. First item
next run: merge it, regenerating `sprites.tsv` with the tool.

## Found against real 1.14d

- **Rogue Encampment arrival (`a1-town-arrival-ama`), draw for draw:**
  equal through row 107 (was 96 at the start of the night). Floors,
  objects and the player block all match. First difference: row 108,
  shadow pass order, because Kashya walks a different path.
- **Root cause behind it:** the unit seed order from game start differs.
  1.14d hands the in-between game-seed steps to town objects; d2rs gives
  them to the starting items. From object guid 7 on, no d2rs unit seed
  appears in 1.14d's draws. Row `q-fix-real-unit-seed-order`; do this
  first next run, before any more pose work.
- Town NPCs never thought before tonight (three causes in d2-sim, all
  fixed). The chickens (ck, GUIDs 93–95) are missing: `q-fix-real-town-critters`.
- Store items all arrive at cell (0,0) on the install: the server's
  store fill (`place_in_store`, 0x00560200) is a stub. Repro test is
  ignored as a known bug. Server task, not yet a row owner.
- Player snap-back at the town gate (`q-fix-real-gate-snapback`); the
  `leave_town` retry limit was raised 12 → 40 as a **temporary** measure
  (marked in the code); revert once fixed.

## Real-data test status

- CI set (no game files): 7,433 tests, all passing at the final staging head.
- d2-client ignored set on the install (render-rest's run before the
  final batch): 187 of 216 pass; the 29 failures are all in the known 71
  from the start of the night (42 of those 71 now pass).
- fixture-migrate: G1 26/33, G2 4/4, G3 7/7; G4–G7 not started. The full
  150-test migrated set was last measured at 86/150 before G1–G3.

## What is left (M24: remaining work, sizes, uncertainty)

| Work | Size | Uncertainty |
|---|---|---|
| Unit seed order from game start (q-fix-real-unit-seed-order) | M | medium: the draw order is recorded; mapping which units take which steps needs the trace |
| Then continue the town arrival compare from row 108 (NPC pose, critters) | M per round | low per difference, unknown count |
| Server store fill on the inventory model (store items all at cell 0,0) | M | low (spec exists) |
| Merge q-facts-scenes (exporter conflict) | S | low |
| fixture-migrate G4–G7 + F1–F3 fixes (client path, equip 2h, start belt) | L | medium |
| Remaining client missile functions (2, 3, 65, 59, 7, 9, singles), §9.4 cell walk, unit hits | M | medium |
| Protocol one-type refactor (~100 call sites) | L | low |
| Gate snap-back, missile damage setup (needs spec) | S / M | high for missile damage until PC 1 specs it |

## Waits on PC 1

`docs/handoff/pc1-data.md` §Step 4 now has items 1–16. New tonight:
controls default order (11), format-0 wrapper questions (12),
state-param sign / stamina scale / progression call sites / loader
conditions (13–16). Also: arcane stars initial tick (draw-order-2 OQ 12,
HANDOFF §5 entry 103), missile damage setup (0x0059F900 → 0x0064B860),
the 0x00553160 think-restart gate (REC-442).

## Process

- One push went to staging with a failed gate (04:15 UTC, `e4331da2`):
  my push command did not check the gate result. The failure was a
  latent oracle bug in `prop_transport::dispatch_any` (a random 1-byte
  0x15 made the test's `msg[3..]` panic; the server already rejected it
  correctly). Fixed with a fixed regression test and pushed green
  (`29cba6e6`) within the next gate. Since then every push runs through
  one script that pushes only on `GATE fail=0`, and the gate also fails
  on leftover conflict markers (one had reached HANDOFF.md earlier).
- REC ids: blocks per session are now recorded in overnight-loop.md;
  next free REC-490.
