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
- **A done (Act III-V host paths, for q-fix-a3a5-hosts)**:
  - **A1 `0x005DDC30`** (`monsters/ai.md` §5.3 "`0x005DDC30(game, unit;
    &E, &M)`, in full", scan 7 callback `0x005DCC60`, the `0x005DD510`
    row corrected: its scan 7 runs the filter, so the state-146 draw can
    happen there too). ECX game, EDX unit, stack &E, &M (`ret 8`; "second
    argument 0" = no melee flag wanted; 12 sites pass &M, 17 null). Forced
    target first (§5.1 a = 0, s = 1; accepted → no window, no classes, no
    `0x005DD510`); else scan 6 over the unit's room and near-room list in
    list order, each room's unit list; candidates = players (threat 14 →
    main), hirelings, pets, monsters that pass `0x005DC970`, d =
    `0x005DC380(C, unit)` ≤ 48; then `0x005DD510`. Draws: none on the
    scanner's seed (except §5.1's k = 3 for a confused unit); the filter's
    state-146 draw is on the candidate player's seed. Never writes the
    control target: S is a local for that one cast / attack. **Frame 50
    (baal-throne / worldstone)**: monster 1:38 class 476 (SuccubusWitch
    `0x005E2120`) makes one draw more in 1.14d (the `< 66` after S is
    found): d2rs's `secondary_candidates` (`d2-client/src/app/single_player.rs:1107`)
    lists monsters only, so an evil monster never gets the player as S.
    Row `q-fix-pc1late-secondary-target` (also lists the scan's other
    differences in `wiring/action/ai.rs`).
  - **A2a vision record** (`monsters/ai.md` §5.2 steps 2/7, §5.2.1;
    `population.md` §9.6 step 3): monster data +0x50 = the DRLG
    coordinate record (population §3.2 r), set once at creation by
    `0x00552D60` in `0x005B2A00` (cl, else the record at the spawn
    point), cleared on the first room change (`0x00554670`). +0x24 is a
    shared one-shot token: step 2 reads it (flag 0x08 clear) only on the
    LOS-draw-false path, step 7 writes +0x24 := (read == 0) only when
    loaded. Explains `a1-warp-tower-cellar-ama` frames 43–45. Row
    `q-fix-pc1late-vision-token`.
  - **A2b Compelling Orb** (`world/quests-act3.md` §7.7): init 60 →
    `0x005B3090(game, room, x, y, 366, mode 1, flags 0)` = `0x005B2F20`
    with r = −1: one placement test at the orb's point (4496, 1811), no
    ring, no retry; then unit flags 0x20000, GUID → +0x2C, +0x28 := 1.
    Row `q-fix-pc1late-orb-spawn`.
  - **A3 Larzuk's map AI** (`world/quests-act5.md` §3.8; `monsters/ai.md`
    OQ8): built from the DS1 path of object 543 (his start dummy) in the
    room's first population: `0x00555910` creates 543 (init 71 spawns
    Larzuk), then hands the preset path to `0x00545C90` → `0x00587950`,
    which moves it into Larzuk's control +0x38 (+0x17 := 1); no draws.
    Recorded frame 26: one seed step (81 ≥ 66, idle 8); frame 34 walk to
    node 0 (5144, 5037). d2rs never calls `larzuk_map_ai`
    (`apply_map_ai` stub; `warp_tile.rs:170` drops object paths; 459 /
    461 the same). Row `q-fix-pc1late-larzuk-map-ai`.
- **B done (Fallen Shaman draws, for q-fix-monster-ai-2)**:
  `monsters/ai-bodies.md` §9.6 items 8–9 (FallenShaman ordered draw table,
  recorded vector), §9.4 item 8 (Fallen), `monsters/ai.md` Randomness.
  The `0x005DF7DB` circle step is **present** in d2rs
  (`ai/tactics.rs:321`); the missing one is the body's step 6 at
  `0x005F169A` (P(aip2) on the secondary target S). 1.14d frame 7:
  `0x005F1516` (7 < 45, command minions), `0x005F169A` (77 ≥ 60),
  `0x005F16E1` (87 < 100, circle), `0x005DF7DB` (low byte 15 → method 5);
  seed {3519730397, 933551402} → {2437809167, 980491059}. S = the player
  at 14 < 15, but d2rs never offers players as S (same cause as A1), so
  its later draws shift by one. Same row `q-fix-pc1late-secondary-target`.
- **E done (Warriv at town-ama-10k frame 287, for q-fix-npc-interact)**:
  the brief had the sides swapped (1.14d 4228, d2rs 4229, per
  q-tool-replay-diff finding 6 and a local d2rs run). Not the wander
  (unit seed unchanged frames 270–295; `0x005DE200` matches d2rs): the Npc
  interaction's walk in radius (3, 2) of the player, `0x005DE6D0` →
  `0x005DE4E0`, already in `monsters/ai.md` §7.2 (recorded case added;
  `world/npc.md` test vector). d2rs `radius_point`
  (`monsters/ai/tactics.rs:490-513`) uses a rounded D2MOO formula. Row
  `q-fix-pc1late-walk-radius`.

## C — numbered items

| # | Item | Answer | Rows |
|---|---|---|---|

## Recordings (local, `traces/raw/`, not committed)

`check-join-act2-quests-ama/orig.state.jsonl` (fixed `q`),
`pc1late-quest-normalise/` (C, the §1.6 normalisation save).
