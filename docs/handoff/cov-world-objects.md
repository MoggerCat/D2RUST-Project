# Coverage session: world objects (batch 6)

Branch `claude/cov-world-objects` (base `claude/specs-staging-7` @ bf602bf).
Spec files: `world/object-population.md`, `world/objects.md`,
`world/objects-2.md`, `world/cube.md` (not `objects-client.md`).
Buckets A and B of `docs/handoff/uncovered-rules-sort.md` batch 6.

## Rules covered (any tier; all `unit`)

| Spec | rules | covered before | covered after | left |
|---|---|---|---|---|
| world/object-population.md | 58 | 0 (the 13 existing tests had malformed `/// Covers: object-population.md …` lines, so none counted) | 57 (58 incl. the new exemption of the heading-text unit) | 0 claimable (see below) |
| world/objects.md | 129 | 98 | 126 | 3 |
| world/objects-2.md | 79 | 44 | 74 | 5 |
| world/cube.md | 66 | 59 | 63 | 3 |

Exemptions added to `docs/coverage-exempt.tsv` (heading/pointer text only):
`object-population.md §edge-cases-original-bugs text`,
`objects-2.md §edge-cases-original-bugs text`,
`cube.md §edge-cases-original-bugs text`.

### New tests (all in `crates/d2-sim`)

- `world/objects/populate/tests.rs`: claim syntax repaired on the existing
  13 tests; new tests for §1 r1, §3 (towns, level count, theme gate result),
  §4 (every threshold p = 0/5/10, pick draws, n = 0, t < n), §5 r6/r7,
  §6 (Fit A/B/C bounds and box queries, random/oriented/spread spot draw
  order), §7.1 (T per class, count < 1, argument-row sizes, a reference
  model of the cluster walk over 6 accept limits × 2 densities × 39 seeds),
  §7.2 (tries, forced health, cap, init-health vs forced, waypoint-shrine
  rows through populate_room), §7.3/7.6 (exact blocked draws, last-unit
  rule, flies), §7.4, §7.5, §7.7, §7.8, §7.9, objects-2 §22 r1 (mode 0).
  `Fake` got `box_log`/`block`/`room_at_fn`/`free_all`/unit-list fields.
- `world/objects/misc/tests.rs`: portal partner/destination/quest hook
  (§12 r6, r8–r11), removal rules (r12, edge 29), class-59 quest-flag gate
  (r7, both difficulties), well "used" by write (objects-2 §24 r5), locked
  door without operator (§24 r6), well refill queue/flag (§24 r7).
- `world/objects/mech/tests.rs`: trap door (§16.2), teleport pad (§16.7),
  obelisk insert / power-up table (§19, new code), burn (§18 text),
  stairs adjacency (§16.11, edge 16/17), gold placeholder drift (edge 18),
  gate/soul inert (edge 19/21), pad ignores own mode (edge 15).
- `wiring/action/tests/objects.rs`: C→S 0x13 object case results
  (objects §7.3), curable-state removal (objects-2 §21).
- `world/cube/tests.rs`: date read once (§3 r1, r2), slot item class (§6.1 r3).
- Claims added to existing tests: objects-2 §23 r1/r3, §24 r1–r4, §22 r1,
  edge cases 1, 3–6; objects.md edge 13, 14, 16–24, 26–29;
  cube §8 l2 r1 (sim inventory host test), edge 15 (server test line).

## Code fixes found by tests

1. **Preset 580 allocated in the preset mode, spec says mode 0**
   (`objects.md` §6 table, edge 22, `objects-2.md` §22 r4):
   `world/objects.rs create_preset` now allocates class 371 with mode 0.
   The old test asserted `Allocate(.., 1)` (the bug); changed to mode 0.
2. **C→S 0x13 object case ignored the operate entry's result 0**
   (`objects.md` §7.3 r5: result 0 → 3): `wiring/action/objects.rs
   object_message` now returns `ObjectCase::Code(3)` for result 0. (The
   action wiring finds the object by the same GUID lookup, so it cannot
   reach this today; the branch is literal.)
3. **`objects-2.md` §19 obelisk completion was missing** although listed A:
   new `mech.rs obelisk_insert` / `power_up` (+ three `MechWorld` seams
   `remove_cursor_item`, `item_subtype`, `power_up_add_stat`). Not yet
   called by the quest item-to-object path (`quests/act2/q6.rs
   item_to_object` "reported"): hand-off to the quests session.
4. Test-only: `route_mismatches` in `world/objects/tests.rs` failed on the
   base (spec commit e44dcaa1 gave null slots an owner `§…` with address 0);
   helper and perturbation string updated to the new table.
5. `stats/tests.rs` synthetic state table gained curable states 45, 46
   (flag 12) for the §21 test.

## Spec note (no spec changed)

`object-population.md` §7.5 closes with "at most 9 objects per call (the
first of each call is not counted)", but its own steps 3–5 do not count the
first object of **each cluster**; the code follows the steps (walk
allocations ≤ 8, first object of each of j clusters extra). One of the two
statements is wrong; resolve with a recording (open question).

## Rules left, with reason

- `objects.md` §12 r14 — hostile-flag walk (`0x00535B10`
  `iterate_players`): provider-side (party/hostile list); `MiscWorld`
  default 0, no sim data for the player-list hostile flag. Opus/provider.
- `objects.md` §14 r2 — flag 0x400 sound (`0x00571740`) needs the server
  sound-event queue; 0x100 hover message is client.
- `objects.md` edge 25 — trap monster family base is the chest-trap
  provider (wiring), no test seam in the sim.
- `objects-2.md` §22 r2, r3, r4 — allocation-mode table of call sites:
  worldgen preset path, quest objects, portal creators sit in other
  modules; population (mode 0), fire objects (1) and the preset forward
  are verified but the whole table is not.
- `objects-2.md` §23 r2 — client side of 0x4D (`client/model.md`).
- `objects-2.md` edge 7 — "locked doors never occur" (narration).
- `cube.md` §8 l2 r2 — the 0x9C action 4 of the placed outputs in the
  player's unit update (needs a tick-level test across inventory wiring).
- `cube.md` §8 l2 r3 — sound event queue (see `objects.md` §14 r2).
- `cube.md` §10 — statement about C→S 0x4C (stub test exists in
  `d2-server` items tests; exempt per sort).
