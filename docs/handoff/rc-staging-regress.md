# rc-staging-regress — two staging regressions (REC-2050..2054 unused)

Branch claude/rc-staging-regress = staging-7 (cb9a3158) + claude/q-fix-npc-interact merged.
Checks run on a clean container, Wine 2026-10-10, `--orig-cache`, 18 checks
(`a2-npc-*` 13, `interact-*` 3, `sor-hydra`, `ama-valkyrie`), 1,640 frames.

## Equal frames (state + packets)
- before: 1480 / 1640 (state DIVERGED: 6 checks, all "item 4:1 field x: 1.14d N vs d2rs absent").
- after:  1630 / 1640. State is equal on every check (PARTIAL = known own gaps).
  sor-hydra and ama-valkyrie 70/70 before and after.

## Fix: overlay_item_places (state_dump.rs)
587acd04f skipped every item whose owner GUID is none, to leave a monster's
equipment alone. A store's stock is also owner-none (mode STORED, page place
x/y in the inventory model), so the store items lost `x`/`y`/`d`. Now an owner-none
item is skipped only when its mode is EQUIPPED or BELT (a monster's body
location lives in its own static path); STORED items are placed whatever the owner.
Fixes drognan/elzix x2/fara/lysander trade and interact-talk-akara (state 15/30 -> 30/30).

## Regression 1 (goto lands at y+1): did not reproduce
All 13 a2-npc state checks are equal 50/50 on the merged tree with freshly recorded
1.14d (atma-talk included), before any change. The merge-npc-interact note
probably predates a later staging sync; no poke change made. The py `free_cell`/`_settle` and the Rust
`nearest_free`/`settle_landing` are the same algorithm on reading.

## Open (not mine, packets only)
- a2-npc-fara-heal packets: frame 7 stream buf #1 size 66 vs 53. S–M.
- interact-operate-waypoint, interact-talk-akara packets: frame 4 s2c #0 id 0x07 vs 0x15
  (MapReveal room stream, rc-net-s2c area). M.
- Note: poke.py still calls `_settle` from `_goto` once (the duplicate is gone).
Ledger: docs/handoff/ledger/rc-staging-regress.tsv (vendor.drognan PARTIAL).
Disk: the container filled up during release builds; `target/` was cleaned.

Sync note: `tools/coord/sync.sh` after the fix hits 10+ content conflicts (e2e_full_loop,
e2e_walk, vendor_inv, walk/tests, world.rs, object_approach, wired.rs, geom.rs, objects.rs,
npc_items.rs) between the npc-interact branch and newer staging: a merge task, not mine.
The fix itself is one function in state_dump.rs; it cherry-picks cleanly.
