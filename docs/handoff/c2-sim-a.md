# c2-sim-a handoff

Coverage session for `specs/sim/intents-events.md` and `specs/sim/rng.md`.

## Counts (unit column of `tools/coverage.py`)
- intents-events: 123 -> 127 of 170
- rng: 37 -> 37 of 58 (the remaining 20 are all exempt candidates below)

## Code fixes
- `d2-server/src/dispatch.rs`: 0x14 chat with strlen 0 now returns 0 (Done)
  with no effect, as `intents-events.md` §2.4 r6 (corrected 2026-10-08) says;
  it returned 2. The existing `chat_string_checks` test asserted the old
  value; it now asserts the spec value and claims §2.4 r6.

## New claims
- §2.4 r6, §3.2 r3, §3.2 r4, §3.3 r4, §3.1 r3, edge-case r8 (tests in
  `crates/d2-server/src/tests/{messages,host,tests_c2sima}.rs`).

## Not done
- Gate: `cargo test -p d2-server` fails in the `world_data_tables`
  integration test with or without my changes (pre-existing); lib tests,
  clippy and `coverage.py --check` pass.
- Rules left (need sim/monster/object handlers; not cheap): §2.4 r8, §7.1 r2,
  §7.3 r1/r3/r4, §7.4 r1, §7.5 r1/r2/r5/r6/r8, §7.6 r5, §7.7 r1, §7.8 r1/r4,
  §7.9 r2/r4/r5, §9 r15/r16, edge-case r10–r13.

## Exempt candidates
specs/sim/rng.md	§3 r6	calling convention of the original (fastcall registers); no behaviour
specs/sim/rng.md	§5.5 row1	client-only globals (automap, particles); render-side, checked against traces/renders
specs/sim/rng.md	§5.5 row2	client-only globals (automap, particles); render-side, checked against traces/renders
specs/sim/rng.md	§7 text	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row1	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row2	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row3	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row4	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row5	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row6	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row7	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row8	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row9	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row11	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row12	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row13	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row15	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row17	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row18	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/rng.md	§7 row19	map of which system draws from which seed; each system spec owns and tests its draws
specs/sim/intents-events.md	§1 text	section intro / narration
specs/sim/intents-events.md	§2.2 text	section intro / narration
specs/sim/intents-events.md	§7 text	section intro / narration
specs/sim/intents-events.md	§7.5 text	section intro / narration
specs/sim/intents-events.md	§7.8 text	section intro / narration
specs/sim/intents-events.md	§9 text	section intro / narration
specs/sim/intents-events.md	§1 r5	lists other flush call sites; no behaviour
specs/sim/intents-events.md	§2.1 r8	1.14d client builders (client code, trace-checked)
specs/sim/intents-events.md	§2.3 r4	host sync timer, never read by the simulation
specs/sim/intents-events.md	§3.1 r2	original client receive code; client specs own it
specs/sim/intents-events.md	§3.4 r2	original client receive code; client specs own it
specs/sim/intents-events.md	§3.4 r3	original client receive code; client specs own it
specs/sim/intents-events.md	§3.2 r5	game types 1/2 and network mode, out of scope for single player
specs/sim/intents-events.md	§3.2 r6	game types 1/2 and network mode, out of scope for single player
specs/sim/intents-events.md	§6 r1	trace-comparison method and recording; checked against traces
specs/sim/intents-events.md	§6 r2	trace-comparison method and recording; checked against traces
specs/sim/intents-events.md	§6 r4	trace-comparison method and recording; checked against traces
specs/sim/intents-events.md	§6 r5	trace-comparison method and recording; checked against traces
