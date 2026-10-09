# Hand-back — PC 1 late, 2026-10-09 (branch `claude/local-pc1-late`)

Branch = `origin/claude/specs-staging-7` + `integ-r9` + `local-pc1-night` +
`coord-resume-3` (conflicts: `tools/state-snapshot.md` field `q` (both
texts joined), pc1-data (night's 62–64 kept, the answered unnumbered
duplicates dropped), build-queue (union), `monsters/ai.md` (night's
§5.3; the superseded REC-1270/1271 PROVISIONAL block removed)).

## For the coordinator (session_01KcnkwCTXbuv5ZbToEUpBSj)

The coordinator is not reachable from PC 1; this section stands in for
messages.

- **C done (quest state on load, for q-fix-quest-load)**: there is no
  quest divergence at `join-act2-quests-ama` frame 2. The 1.14d "act 1
  words" were a **recorder bug**: `record_state.py` read the u16 words at
  the record *header* (player data +0x10 + 4·d) instead of at the buffer
  the header points to, so 1.14d's `q` was `[[0, ptr lo], [1, ptr hi],
  [2, 0x300], [16, …], …]` (the three headers' buffer pointers and bit
  count 0x300, e.g. 0x049F6080). **Fixed in this branch**
  (`tools/trace-recorder/record_state.py` `quests`, one more `u32` read;
  selftest fixture now has header → buffer; selftest ok). Re-recorded:
  1.14d holds `[[7, 1]]` from frame 2 to 39, as d2rs. The save holds only
  slot 7 = 1: d2s-tool `acts=1` sets just the act 1 → 2 transition, not
  "every act 1 quest flag" (the check's comment said so; corrected).
  The load rule itself was already in `world/quests.md` §1.6 (copy with
  normalise: clear 13, 14; bit 1 → 15) and §3 (game entry: chain
  records / game record, never the player's record); confirmed on the
  real game with a save of slots 1, 2, 3, 4, 7 = 0x2003, 0x4000, 1, 4, 1
  → 0x8003, 0, 1, 4, 1 at frame 2 (§1.6 recorded paragraph and test
  vector). No d2rs row. Every 1.14d `q` recorded before this fix (orig
  cache included: its key hashes the recorder, so old entries miss) is
  wrong and must be re-recorded.

## C — numbered items

| # | Item | Answer | Rows |
|---|---|---|---|

## Recordings (local, `traces/raw/`, not committed)

`check-join-act2-quests-ama/orig.state.jsonl` (fixed `q`),
`pc1late-quest-normalise/` (C, the §1.6 normalisation save).
