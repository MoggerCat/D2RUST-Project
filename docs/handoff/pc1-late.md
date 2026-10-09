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
- **D done (items timing and flags, for q-chk-items-drops /
  q-fix-npc-interact)**:
  - **D1 drop frame** (`items/treasure.md` §3.7 new; `sim/units.md` §4.6
    rule 1 step 3 points there): 1.14d rolls, creates and sends a
    monster's drop **in the death frame**: the kill's mode-0 request
    `0x005A7C20` runs the death start `0x005A6FF0` (mode 0, clean-up,
    treasure gate `0x005A6830`, TC walk, `0x0055A550` → `0x00558D90`);
    the item is added and queued inside `0x00554850` (`0x005549F3` →
    `0x0064C040`), so the same tick's client pass sends 0x69 and 0x9C
    action 0 together; mode end only sets mode 12. Recorded
    (`combat-kill-fallen`): fallen mode 0 and its gold mode 3 in the
    same frame-36 snapshot. d2rs does the same synchronously
    (`reaction::kill` → `death_start` → `monster_death_drop`, then
    `update_pass` in the same tick): **the one-frame lag is the death,
    not the drop** — in a local d2rs run the fallen stands at (5146,
    4266) instead of walking to (5145, 4265) from frame 31 (the check's
    earlier AI divergence, seeds differ from frame 5), so the Fire Bolt
    hits it one frame later (0x69 code 8 at 37 vs 36). No drop-path row.
    PROVISIONAL REC-1453: the `items-drops-nor-*` lags are the same death
    lag; settled by pairing each check's 0x69 code 8 frame with its first
    0x9C on both sides. (Side note: that d2rs `state-dump` run sent no
    0x9C at all; d2rs sets item flag 0x10 on low / normal drops,
    `wiring/economy/treasure_items.rs` ~108, PROVISIONAL REC-281.)
  - **D2 gamble list** (`world/vendors.md` §5.1 step 7 "Exact"): mode 0
    on the NPC's page-0 grid via `0x00560200` (find-free, then mode := 0
    at `0x00560364`), flag 0x10 cleared; `00 12` = mode 0, x 9, y 0 (the
    first item, a ring); d2rs's `place_in_gamble` is a no-op (mode 4,
    (0, 0), `10 00`). Row `q-fix-pc1late-gamble-place`.
- **F2 done (audio from the binary, for q-fix-audio)**
  (`audio/sound-table.md` §6.1 "First tick and the start-tick stamp"
  r1–r4; `client/msg-ui.md` §19 r3): the first sound tick runs with **T
  0** (T := 0 at sound init `0x00482293`, += 1 only at the tick's end
  `0x00482C5A`; start tick = T + delay when requested); it is the first
  loop pass with a client update after S→C 0x04 sets `in_game` (server
  frame 2), so the song and ambience start at T 0. d2rs ticks from
  frame 1 (`audio/driver.rs:529`): row `q-fix-pc1late-audio-first-tick`.
  `cursor\windowopen.wav` = sound 6 `cursor_switch`, requested by every
  screen-message add (`0x0049E585`); at T 0 it is the join's own S→C
  0x5A code 2, which adds an empty line (`0x0049EB10`). d2rs drops the
  add's sound and never consumes EventText: row
  `q-fix-pc1late-screen-message-sound`.

## C — numbered items

| # | Item | Answer | Rows |
|---|---|---|---|

## Recordings (local, `traces/raw/`, not committed)

`check-join-act2-quests-ama/orig.state.jsonl` (fixed `q`),
`pc1late-quest-normalise/` (C, the §1.6 normalisation save).
