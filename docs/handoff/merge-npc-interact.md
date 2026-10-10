# merge-npc-interact — staging-7 merged into claude/q-fix-npc-interact

Merge commits (no rebase) of staging @ 1de329775, then a sync to @ cb9a31582.
Conflicts and how they were resolved:
- `objects.rs`, `quest_objects.rs`, `d2-server .../world/wired.rs`: kept
  both sides (Jerhyn / palace guard hooks and `object_queued` /
  `object_arriving`; staging's `map_ai_store` and `item_picks` / `arriving`).
- `action/ai.rs`: staging's path / coord seams plus the palace guard doc line.
  `tactics.rs`: `radius_point` is the same 1.14d geometry on both sides (took
  staging's). `ai/tests.rs`: kept both Jerhyn vectors.
- `e2e_full_loop.rs`: both extra 0x2C counted (heal + two pick-ups): 29 + store.
- `state_dump.rs`: kept staging's `DialogUi` (the original UI, headless) and
  dropped this branch's `HeadlessDialog`, because keeping both would send the
  B2 0x31 twice.
- 26 `traces/orig-cache/a2-npc-*` files: neither key matches the merged
  recorder. Took staging's; the checks re-recorded 1.14d (cache not refilled).

## Checks
fmt and clippy -D warnings are clean. nextest: sim + server + proto 5232/5232,
client 2498/2498 (client run in batches of 8 test targets because of disk
space). coverage, spec_index and ledger --check: 0 errors.
16 checks (`a2-npc-*`, `interact-*`), equal frames out of 1500:
staging 1de329775 802 -> first merge 833 -> final (cb9a31582) 882.
- interact-operate-stash: state 3 -> 30/30 (0 differences), packets
  DIVERGED@13 -> MATCH. Ledger part: `object.operate.32.bank` EQUAL.
- a2-npc packets +1..+13 each. Packets of all a2-npc checks first differ at
  f3 (MapReveal 0x07); waypoint / akara at f4 (0x07 vs 0x15). This is the room
  stream.

## Open (both are staging-side, not caused by the merge)
- All 13 a2-npc state checks: after `goto unit` the player lands at y+1
  compared with 1.14d (atma 5041 vs 5042). Staging's binary alone gives the
  same result. It comes with the new goto settle (`poke.py` `_settle` /
  `free_cell(target)`; d2rs `poke::settle_landing`). Owner: the poke tool. S–M.
  This hides round 1's npc results. `poke.py` also calls `_settle` twice (a
  duplicated merge hunk); the second call does nothing.
- interact-talk-akara state 30/30 -> DIVERGED@16 since cb9a31582: staging's
  `overlay_item_places` (e757eb6da / 587acd04f, rc-skill-hydra-valk) now
  skips items with no owner GUID, so the store items lose `x` (1.14d `x 0`).
  The code is identical to staging's. Owner: the state-dump / hydra-valk
  session. S.
