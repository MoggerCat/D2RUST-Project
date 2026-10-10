# rc-client-melee-approach: hand-back (NOT fixed)

Checks: `play_smoke the_live_run` fails before and after (no EQUAL counts
changed). No ledger rows settled, no code change kept beyond the import below.

## What changed on this branch
- The code of `claude/q-fix-rd-misc` (crates only; its docs conflict with
  staging-7's generated ledgers) is copied in so `the_live_run` reaches
  the finding at all. Without it the test dies earlier ("run leg: dropped
  {6D: 01}"). Merge q-fix-rd-misc properly when it lands.
- My attempt in `bridge/predict.rs` is reverted (it made the result worse,
  3 divergences instead of 2).

## Findings (measured with a per-step trace of `kill()`)
1. The test sends a raw C->S 0x06. The preview's client melee range
   (`bridge/combat.rs::melee_range`) always answers "in range", so the
   click path never walks. The local server host (`app/single_player.rs`
   `in_melee_range`: larger axis distance <= 2 + 2 + 1, no line test)
   runs the player to the monster (`use_on_unit` -> `run_to_unit`, mode 3).
   The server sends the own client nothing for it (`mode_update`:
   `own_client` -> None), so the client prediction stays put.
   In 1.14d the click would test `0x00622C40` (reach `0x00622870`, distance
   `0x00641530`, line `0x00622AA0`, 0x804) and walk first (C->S 0x01..0x04
   plus the pending record run by `0x00481600`, model.md §8 r6); a raw 0x06
   out of reach never leaves the real client.
2. Prediction attempt: record 0x06/0x0D as `Walk::Unit` run when the
   target is beyond Chebyshev 5. The client run starts and tracks the
   server for ~15 ticks, then its path ends early (drawn (5056,4213),
   server keeps to (5061,4209), monster (5066,4205)): the client path
   engine reports "not moving" with the unit target ~10 sub-tiles away
   (a stale target position at path end; the server's engine kept going
   with the monster's moved position). Then the second 0x06 (server in
   reach) made the client run again (its reach test used the stale
   position) and drifted further.
   Suspect: `ClientPath` unit-target re-path (`PathTo::Unit` position is
   fixed at request; the sim re-reads the live unit), see
   `bridge/client_path.rs` `ctx.to` / `target_position`. Not proven.

## Open (sizes)
- Make the client unit-target path follow a moving monster as the server's
  does (small-medium; start with a unit test: path to a unit that moves
  away mid-run, compare with `d2_sim::path::walk` on the same grid).
- Then re-add the 0x06/0x0D approach prediction (small, ~40 lines in
  `predict.rs`: `attack_of`, `WalkTo::Attack`, resolved in `frame`).
- Proper fix = real `melee_range` + pending-attack loop (`0x00481600`) in
  the client (large; `PreviewInteract` only fires interact, not attacks).
