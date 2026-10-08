# q-shrine-overhead: shrine overhead text and state callbacks

Stitching session 2026-10-08. Nothing here is verified against 1.14d (rule 10).
PROVISIONAL note: REC-263 (docs/HANDOFF.md §7).

## Links connected

| # | Link | Was | Now |
|---|---|---|---|
| 1 | Shrine overhead (0x26 type 5, decimal string id) → bubble | q-unit-fx's `OverheadUi` already drew records of non-player units; no test pinned a shrine | Test `a_shrine_overhead_text_draws_above_the_object_for_its_frames` (object key, string id, drawn for 8 · len + 125 frames, freed after) |
| 2 | Shrine state expiry → remove callback | The type-12 walk freed the list but the state bit stayed on forever | `StatHost::list_removed` (ActionHooks) queues the callback; `UnitHooks::lists_expired` (after the walk, `units/dispatch.rs`) runs it: state off; stamina shrine also clamps stamina to max |
| 3 | Timed-state helper refusals | A second use replaced the list; any state id accepted | State outside the table → none; a list of the same state is refreshed (new expiry + timer), not replaced (`skills/bodies.md` §2.7 steps 1, 4) |

Tests: `a_shrine_state_carries_its_stats_and_ends_on_its_tick` (now also state off),
`a_second_shrine_use_refreshes_the_state_instead_of_replacing_it`,
`the_stamina_shrine_callback_clamps_stamina_when_it_ends` (d2-sim),
the overhead test (d2-client).

## PROVISIONAL / left

- Bodies of `0x00583BD0` / `0x00583A40` are unwritten in the specs (REC-263).
- Shrine skill / level in the helper request taken as equal for the refresh.
- Only the default and shrine callbacks are queued from the stat lists; skill
  callbacks stay with their bodies.

## The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-shrine-overhead; git checkout claude/q-shrine-overhead
cargo run -p d2-client --release -- play --new sorceress Test
```

1. Click a shrine outside town: its name text appears above it for about 6 s.
2. After the shrine's duration the effect (armor, resists, ...) ends; use the
   same shrine type again before it ends: the duration restarts, no stacking.
