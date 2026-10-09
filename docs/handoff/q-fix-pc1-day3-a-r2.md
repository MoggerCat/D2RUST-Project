# q-fix-pc1-day3-a-r2 — hand-back (2026-10-09)

Branch `claude/q-fix-pc1-day3-a-r2`, from `claude/specs-staging-7`
(gate b63, aac6b711), staging merged in before each push. Three rows of
`docs/handoff/build-queue.tsv`, all done (marked `DONE (q-fix-pc1-day3-a-r2, …)`).

## 1. q-fix-tool-c2s-masks

- `tools/trace-recorder/packets_diff.py` reads
  `specs/tools/scenario-masks-c2s.tsv` with the same strict reader as the
  S→C table (`read_masks(path, side="c2s")`): the id must be a named C→S
  id of `specs/sim/client-messages.tsv` (rows named `-` are not ids).
  `load_masks()` gives `{"s2c": …, "c2s": …}`; `compare_record` and the
  report's `--` marks use the table of the record's stream. New option
  `--masks-c2s TSV`.
- d2rs's zeros in C→S 0x67 stay (no d2rs change).
- Selftest (408 checks): the C→S reader rejects 0x4A (no row), 0xFF and a
  zero length; two 0x67 records that differ only in the stack bytes after
  the game-name NUL (2–16) and the character-name NUL (28–36 for
  "ScnAma") match with 24 masked bytes; the game type byte 0x11 and a byte
  inside the character name are still reported; without the C→S table
  the stack bytes are reported (byte 2).
- Spec test vector row updated (`specs/tools/packets-trace.md`).
- Not changed: `crates/conformance` scenario comparator (its `c2s`
  stream is the script's own messages, identical on both sides, so the
  0x67 rows never apply there).

## 2. q-fix-p3-npc-nearest-player + q-fix-p3-npc-interact-gate

- `QuestControl::npc_wants_interact(w, player, npc, class, interact) ->
  Result<bool, QuestError>`: `interact` (the NPC class's monstats flag)
  false → `Ok(false)` before the picked test and without walking (§6.4
  step 2 before step 3). `records` is already in list order (newest
  first, `QuestControl::new` reverses), so the index walk is newest →
  oldest as §6.4 step 5 says; no order change was needed, a comment says
  so.
- The AI seam `AiSummons::npc_wants_interact` returns bool (default
  false). `View` (`wiring/action/ai.rs`) answers it through the lent quest
  control: new `QuestObjectHost::npc_wants_interact` (default false),
  implemented by `QuestLoan` via a new `LoanCall::NpcWantsInteract`
  (`wiring/economy/quest_objects.rs`); the 0x8A goes out through the
  loan's `QuestRest::send` like every quest send. No lent control →
  false, no send. A not-picked set (1.14d fatal 0x7F4) answers false.
- `nearest_player` per `0x005DDE80`: client players of scan 2 in scan
  order, full-size distance (`distance_full_size`, NPC size subtracted)
  ≤ 15; a non-interact NPC takes the first; an interact NPC calls the test
  per qualifying player and takes the first true; none → the NPC itself.
  "Close" = d < 4. REC-500 PROVISIONAL removed.
- Tests: `world/quests/tests.rs::npc_wants_interact` (fresh player → true
  and one `8a 01 12000000`; slot 0 bit 0 set → false, no send; no
  interact flag → false, no walk, also before the picked check);
  `wiring/path/motion_tests.rs`: `the_nearest_client_player_within_15_is_found`
  rewritten to the full-size rule (16 not found, 15 found),
  `a_non_interact_npc_takes_the_first_player_in_scan_order` (players at
  d 10 then d 3 in scan order → the d 10 one, close=false, test not
  called), `an_interact_npc_takes_the_first_player_the_quest_test_passes`
  (taken + one 8A; none passes → NPC, both tested in order, no 8A; both
  pass → first only; only the near one → taken, close; no lent control →
  NPC). The gate tests use a fake lent host (`Gate`) for the test's
  answer; the real active functions are covered by the quest test.
- `cargo test -p d2-sim`: 4552 lib tests and all integration tests pass;
  `walk_in_radius_points_follow_the_recorded_walks` passes. The "arrival
  e2e" named in the row has no test of that name in `crates/`; the
  Warriv walks against 1.14d are a live run (below).
- `specs/world/quests.md` §6.4 "d2rs:" paragraph updated to the new
  state.

## 3. q-tool-playthrough-quests

- **Field:** staging already had (5a729988, another session) the player
  key `q` in `state-1`: `[slot, word]` for each non-zero slot 0–41 of the
  game difficulty's record (`specs/tools/state-snapshot.md` §2 row `q`,
  `d2_sim::debug::state::HOST_FIELDS`). It carries the same bits as the
  96-byte hex the row asked for (slots 0–41; words 42–47 are not
  exported, and no predicate reads them), so I built on it instead of
  adding a second copy of the record. If the coordinator wants the raw
  96-byte hex as well, that is a small follow-up.
- `tools/playthrough/playthrough.py` 0.2.0: predicate `quest <slot> <bit>
  set|clear` (slot 0–41, bit 0–15), with `need ever`; reads `q` (a slot
  not listed is 0; a player without `q` fails "no quest record").
  `specs/tools/playthrough.md` §2 rule 4, a test vector row, Open
  question 3 answered and replaced by the remaining gap.
- Selftest: a fixed 96-byte record turned into `q`, set / clear of eight
  bits, parse errors (slot 42, bit 16, `on`, arity), `stuck` with "quest
  6.0 clear", `ever` reached at f2, no `q` → stuck, `need ever quest …`
  parsed.
- `traces/playthrough/act1.play`: saves `den` (`--quests 1.1`), `andy`
  (`--quests 6.1`), `east` (`--quests 6.0`); milestones
  `den-of-evil-done` (need 1.1 set, ever 1.0 set), `andariel-done` (ever
  6.0 set), `act2-open` (ever player act == 1, lv == 40, 7.0 set). All
  three need an NPC talk or menu (Akara msg 76, Warriv msg 183, Warriv's
  "Go east"); d2rs's headless input has no hover pick
  (`scenario-diff.md` Open question 4), so the clicks walk: they are
  expected blockers with the bit clear until an NPC-interaction input or
  poke exists. Real-install run: see "Real-data run" below.

## PROVISIONAL

- **REC-860** (`crates/d2-sim/src/world/quests.rs`
  `QuestControl::npc_wants_interact`, `world/quests.md` §6.4 step 4): the
  act is `unit_act(player)`, not the act byte of the player's client
  record: d2rs's sim has no client act. They differ only during an act
  change. Settled when the client record's act is modelled.

## Not done / notes

- `docs/handoff/provisional-index.tsv` not regenerated (the REC-500 line
  is stale; REC-860 is new): the coordinator regenerates it on staging.
- §6.4 consequence (c), an NPC with no room → `0x005DDF20` returns 0:
  d2rs still returns the NPC itself (no null unit in the seam). Outside
  these rows.

## Live runs for PC1-C

1. `python3 tools/scenario-diff/scenario_diff.py traces/checks/packets-town-arrival-ama.check`
   (both sides). Look for: the first `c2s` divergence is no longer C→S
   0x67 (the summary's "masked bytes skipped" counts the 0x67 stack
   bytes); whatever comes first now is the next row.
2. `python3 tools/scenario-diff/scenario_diff.py traces/checks/a1-town-arrival-ama.check`
   (ScnAma, seed 1234, the REC-501 recording). Look for: Warriv's (155)
   unit positions / path targets still equal frame by frame (the
   nearest-player change only affects NPCs with a client player within
   15; Warriv is `interact`, so he now takes the player only when the
   Act I gossip test passes, which sends 8A to a fresh character as the
   recording shows from frame 24).
