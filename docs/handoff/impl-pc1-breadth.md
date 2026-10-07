# impl-pc1-breadth — PC 1 breadth pass code list

Branch `claude/impl-pc1-breadth`, base `claude/specs-staging-7` @ c0e6db7.

## Landed

1. **Dispatch table** (`bridge.md` §6 r7): shared no-op handler for 0x79,
   0x7F, 0x8B, 0x8C, 0x8D, 0x90 (owner `msg-ui.md` / `msg-units.md` per the
   TSV), 0xAE (`bridge.md`), 0xB3 (`model.md`); 0xB2 no effect (`bridge.md`
   §6 r6). No `TBD` row is left, so no id is unowned.
2. **New handlers**: 0x75 PlayerPartyInfo (`msg/roster.rs`: party → +0x22,
   level → +0x20, u16@11 → +0x30, pet pass, `RosterChanged`); 0x8F Pong
   (`msg/session.rs`, `ClientWorld::ping`); 0xAF / 0xB0 (`ClientWorld::
   connected`).
3. **`L`** (`check.rs`): read from `ClientWorld::ping.rtt`
   (`([0x007A04A4] + 0x32) >> 7`); d2rs sends no ping so it stays 0 → L = 0.
4. **Null object function slots**: `world::objects` already routed operate
   35–38, 60 and init 35, 36, 40 to `Route::Null` (operate returns 0, init
   skipped). The table-check test now accepts `§7.2` / `§3` owners on
   address-0 rows; new test `null_operate_slots_return_zero_and_do_nothing`;
   init test extended to 36 and 40.
5. **Empty-layout rows** (`intents-events.md` §3.5 r4): audit status
   `IdOnly` for 0x7E (the eight 1-byte ids were already `Generated`); new
   test `empty_layout_rows_are_complete`; `docs/handoff/s2c-builders.md`
   row updated.

## Changed expectations (tests corrected to the spec)

- `bridge/tests.rs` `split_and_unowned`: 0x79 / 0x8B now run the no-op
  handler (handled 2, unowned 0).
- `bridge/tests.rs` `dispatch_check_catches_perturbations`: the 0x75
  perturbation (was "unowned") now reports an `Owner` mismatch; a
  `NoHandler` case uses 0x79 with its handler removed.
- `local_tests.rs` `unknown_and_unowned_ids`: 0x79 is handled, not unowned.
- `msg/tests_items_skills.rs` `no_op_and_out_of_scope_ids`: vector uses
  0x79 and expects the no-op handler (spec test vector).
- `objects/tests.rs` `route_mismatches`: address-0 rows owned by `§7.2` /
  `§3` are Null.

## PROVISIONAL

- 0x75 pet pass `0x00478FA0` (msg-units OQ11): palette level `t` is 1 unless
  the type-4 pet record's owner is the local player (stand-in for
  `0x00478E70(local, U) = 0`); stored in `ClientWorld::pet_palette`
  (render state). Marked `// PROVISIONAL` in `msg/roster.rs`. Settled by
  reading `0x00478E70` / a recording with a party.
- 0x8F: `now` is not in the bridge model yet, so `rtt` := 0 and `pong[4]`
  := 0 (matches d2rs, which sends no 0x6D).
- Pet type 4 is taken from the spec text as written.
