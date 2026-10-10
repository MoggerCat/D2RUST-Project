# merge-npc-interact — staging-7 merged into claude/q-fix-npc-interact

Merge commit on `claude/q-fix-npc-interact` (no rebase) of
`origin/claude/specs-staging-7` @ 1de329775.

## Conflicts (7 code files, 26 orig-cache files)

- `objects.rs` / `quest_objects.rs`: both hook sets kept (the Jerhyn / palace
  guard hooks and staging's `map_ai_store`).
- `wiring/action/ai.rs`: staging's path / coord seams kept, plus the palace
  guard doc line; `walk_in_radius` doc from this branch.
- `monsters/ai/tactics.rs`: `radius_point` the same 1.14d geometry on both
  sides; staging's text kept. `tests.rs`: both Jerhyn vectors kept.
- `e2e_full_loop.rs`: both extra 0x2C counted (heal sound + two pick-up
  sounds): `29 + store`.
- `app/state_dump.rs`: staging's `DialogUi` (the headless original UI) kept;
  this branch's `HeadlessDialog` stand-in removed, because keeping both
  would send the B2 0x31 twice. RUN_GAPS names 0x2F / 0x30 / 0x31.
- `traces/orig-cache/a2-npc-*`: neither side's key matches the merged
  recorder (staging's `poke.py` / `autostart.py`). Took staging's; the
  checks re-recorded 1.14d (cache not refilled).

## Checks

fmt, clippy -D warnings (d2-sim, d2-server, d2-proto, d2-client): clean.
nextest: d2-sim + d2-server + d2-proto 5225/5225, d2-client 2492/2492
(run in batches of 8 test targets because of disk space). coverage,
spec_index and ledger --check: 0 errors.

The `a2-npc-*` and `interact-*` checks (16), staging alone -> merged
(equal frames, fresh 1.14d):
- interact-operate-stash: state 3 -> 30/30 (0 differences), packets
  DIVERGED@13 -> MATCH. Ledger part: `object.operate.32.bank` EQUAL.
- a2-npc-elzix-gamble packets 44 -> 45; interact-talk-akara packets 24 -> 25.
- No check got worse. Overall equal frames went from 802 to 833 out of 1500.

## Open

- All 13 `a2-npc-*` state checks diverge on the same thing, and it **predates
  this merge** (staging's binary gives the same result): after `goto unit`, the
  player lands at y+1 compared with 1.14d (atma (5130, 5041) vs d2rs 5042). It
  comes in with staging's new `goto` settle (`poke.py` `_settle` /
  `free_cell(target)`, LAND_MASK; d2rs `poke::settle_landing`). Owner: the poke
  tool (`q-tool-interact-pokes`). Size S–M. It hides the npc-interact results
  from round 1. Also, `poke.py` calls `_settle` twice after the goto placement
  (a duplicated merge hunk on staging). The second call does nothing, but
  someone should remove it.
- The packets of every a2-npc check first differ at frame 3, in the MapReveal
  0x07 room stream. The packets of interact-waypoint / akara first differ at
  frame 4 (0x07 vs 0x15). This is the room stream and not this family.
