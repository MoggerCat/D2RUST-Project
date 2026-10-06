# Gap tests: drlg, world, tick, unit-order, intents-events

Scope: branch `claude/gaps-drlg-world`, based on `claude/bold-ptolemy-jvyvxy`
at `ac01471` (PR #15). Unit-tier tests only, so no rule became
"verified" (CLAUDE.md rule 10). Each claim names only what its test asserts.

## Coverage (any tier, units covered / units)

| Spec | Before | After |
|---|---|---|
| specs/drlg/levels.md | 28/48 | 40/48 |
| specs/drlg/rooms.md | 53/91 | 80/91 |
| specs/drlg/outdoor.md | 47/72 | 64/72 |
| specs/drlg/preset.md | 43/60 | 46/60 |
| specs/world/quests.md | 54/81 | 72/81 |
| specs/world/waypoints.md | 41/61 | 47/61 |
| specs/world/cube.md | 34/49 | 46/49 |
| specs/sim/intents-events.md | 41/73 | 53/73 |
| specs/sim/tick.md | 38/58 | 43/58 |
| specs/sim/unit-order.md | 24/38 | 36/38 |
| **these 10 specs** | **403/631** | **527/631** |
| repository total (any) | 1815/2704 (67.1%) | 1939/2704 (71.7%) |

## Tests

New files (each `#[cfg(test)] mod gaps_tests;` / `mod gaps;`):
`crates/d2-sim/src/drlg/tests/gaps.rs` (26),
`drlg/outdoor/gaps_tests.rs` (14), `drlg/preset/gaps_tests.rs` (3),
`world/quests/gaps_tests.rs` (18), `world/waypoints/gaps_tests.rs` (6),
`world/cube/gaps_tests.rs` (9), `tick/gaps_tests.rs` (5),
`units/lists/gaps_tests.rs` (11), `crates/d2-server/src/tests/gaps.rs` (10).
The existing `tests.rs` fixtures beside them gained only `pub(super)`.
One claim added to an existing test:
`d2-server/src/adapters/handlers/skills/tests.rs::hold_and_left_forms` now
also claims intents-events §2.4 r5 (it asserts `pierce_idx` +1 per skill
message, none for 0x0B).

## Code fixes

None. No test showed the code deviating from a spec rule it states
unambiguously (see the waypoints question below).

## Not claimed, with reasons

- **Layouts / prose / addresses:** levels §1, §3 text, §10 text; rooms §1;
  outdoor §7 text, §3 text (owned by levels §5); preset §1, §6 text, §13;
  quests §2.3 r4, §11; waypoints edge-cases text; intents §1 text, §2.2
  text; tick §3 r2, §5.5 text, §5.5 r4, §6 text.
- **Not implemented (TODO in code or later phase):** levels §3 r5; rooms
  §4.2, §4.6, §5 r8 (client); outdoor §7.5 text and §12.2 (path floor,
  open q 6), §9.1 (q 7), §9.3 (q 7, 8), §11 (Act V past the siege strip,
  q 9); preset §3.2 r4 (client automap), §12; quests §10.2 (timers 15, 7,
  1), §10.7 (Countess); intents §1 r5, §2.3 r4, §2.4 r6, §2.4 r8, §2.5,
  §3.1 r2, §3.2 r3–r6, edge r8; tick §1 r6, §6 r1–r3, §7, §8 (host-only),
  edge r3.
- **Owned elsewhere / not reachable from the module:** levels §3 r7
  (outdoor), §6 r2 (`maze` private, maze.md §4.3); rooms §2 r5, §6 r4,
  §9.8 (mostly TileSub sites); preset §2 r3 (data loading), §3.1 text
  (via `wiring`), §3.1 r4, §3.2 text; quests §2.1/§2.2 claimed, but §6.2
  text, §9.1, §9.2, §9.4 handlers live in world/item code; waypoints §4
  text, §4 r2, r3, §5.2 text, §7 r6, §8 r1, §8 r3, §9, §10, edge r7;
  cube §6.1 r3, §10; tick §5.2 r4 (monster spec), §5.3 (callers table in
  other specs), §5.6 (dispatch in `units/dispatch.rs`; unclaimed tests
  `tick/tests.rs::dispatch_data_matches_spec` and
  `units/tests.rs::handlers_agree_with_tick_tables` could take it), §5.7
  (units.md); intents §3.4 r2, r3 (client).
- **Unobservable in the sim:** cube §3 r2 (date read once); quests §5 r2.
- **Needs game files or traces:** rooms §9.9 (town DS1/DT1); waypoints §3
  r3; intents §6 r1–r5 (trace definitions); waypoints edge r8 (host
  clock).
- **Byte-level DS1 parsing outside the sim** (sim gets parsed
  `Ds1Input`): preset §5.2 text, r4, r7, r9, r11.
- **Partly tested, not claimed:** levels edge r4 (only "no match reads
  record 0"; the "loop runs out" half cannot occur in this code); outdoor
  §2.4 (the linker "return false" branch reached only for R4/RW); unit-order
  §1 r4, r5 (`units/tests.rs::allocation_seeds_and_rejections` asserts
  most of it; missing: the GUID counter unchanged for a fixed GUID, players
  drawing from counter 0).

## Spec questions

1. **waypoints §7 r2 / edge case 6:** choosing the waypoint's own level is
   said to close the menu without travel, and rule 2 says the stop happens
   "here", but rules 1–3 hold no comparison that detects it (the
   destination bit is set, so rule 3 passes). The code warps, and the
   existing test `waypoints/tests.rs::close_and_validation` asserts that
   warp under a comment citing edge case 6. Which check stops it (object
   room level vs. destination?) Code and test to change together once
   answered. Rules §7 r2 and edge r6 left unclaimed.
2. **levels edge r3 / §5.4:** with 9 warp-room centre slots and no bound
   check, which bytes does a 10th centre write (x into y[0], y[9] into the
   count at +0x228)? Code appends (TODO).
3. **rooms §9.2 text (`0x0066F1A0`):** which `keep` does the only caller
   `0x0066B4C0` pass (decides flag 0x200000)? Code assumes 0.
4. **intents-events §4 r4:** lists S→C 0xAF–0xB4 as out of scope, but
   `server-messages.tsv` marks 0xB1 `none` (size 0, never received per
   §3.1 r3). Test follows the TSV. Should the prose say 0xAF, 0xB0,
   0xB2–0xB4?
5. **outdoor §6 r4:** "each doubled unless its vertex is a preset link"
   plus "grows by 2 in magnitude (a, c)": tests follow the code (double,
   then grow). State the order explicitly.
6. **outdoor §8.3:** rows omit the file (F) and in-row order of the
   377/378/380/381 wall and path pieces; tests check only (P, x, y) sets
   and given F values.
7. **quests §7.1:** "every NPC id is a valid monstats row" needs a
   game-file check (claimed unit test covers lookup and table shape).
