# fin-sim coverage session

Covered (unit "any" column; before → after): tick.md 49 → 53 of 62;
scenario.md 39 → 49 of 63; pathing.md 168 → 169 of 178. intents-events.md,
rng.md and units.md unchanged (124 / 38 / 70).

New tests: `crates/d2-sim/src/tick/tests_fsim.rs` (§5.5 r4 + text, §5.2 r4,
§7), `crates/conformance/tests/scenario_fsim.rs` (§3 rows 1–5, §3.1 rows
1–4, §3.1 r4), `crates/d2-sim/src/path/walk/tests/fsim.rs` (§12.6).
Visibility of `other_setup`, `other_info`, `owned_path`, `Recorder` in
`path/walk/tests/gaps.rs` widened to `pub(super)`.

Code fixes: none (no test failed against the code).

Left, with reasons:
- units.md §3.3 / §3.4 (compress on room deactivation, inactive records,
  restore): large new wiring (store records, restore lists); not done.
- units.md §4.5: player mode start table; wiring.
- rng.md §7 rows: they name which system draws from which seed; each is
  owned and claimable by the system's own spec. §3 r6 is a calling-convention
  note, §5.5 rows are client-only globals.
- tick.md §5.3, §5.7, §6 (client pass), §1 r6, §8: need the client-pass /
  caller wiring or are host-only (wall clock); not claimed.
- pathing.md §12.2 (type 3 dispatch is in a private function; needs a
  public seam), §12.7/§12.8 text, §9.6 r2, §10 r4–r5.
- intents-events.md: 46 rules, mostly handler wiring; not started.
- scenario.md §1 r2–r3, §4 text/r1/r3/r9, §5 r7, §6 r2 and rows 9–11 need the
  runner and original-side traces.
