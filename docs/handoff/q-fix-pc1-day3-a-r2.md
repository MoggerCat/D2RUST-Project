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

## 4. q-fix-real-join-missing-msgs (coordinator's follow-up row, not in the build queue)

`app_frame_loop` (real data) compared the client's applied join messages
with `facts/join/a1-new-sor.tsv`: 115 of 127. The missing ones were not
0x0E (those were all sent). Diffing the two sequences id by id gave:

| Missing | Cause | Fix |
|---|---|---|
| 4 × 0x9C action 0x0E (belt potions) | the start items' belt placement only did the slot placement, not the rest of `0x0055E9B0` (`inventory-moves.md` §7.14: link, mode 2, page 0xFF, command flag 0x400, update list) | `WiredStart::place_belt` runs `handlers::to_belt` (as the vendor's auto-belt); the save loader's belt branch likewise (`d2s.md` §8.2 rule 3 names the same call) |
| 2 × 0x22 (scroll counts) | the inventory desk asked the rest for books `scrollskill` / `bookskill`, which the play host never answers: no scroll or tome ever linked its skill | `EquipWorld::book_skill` reads the desk's own books table (`inventory.md` §5.5 step 1) |
| 1 × 0x21 (staff `StartSkill` o-skill) | the start items ran on a bare desk without the players' skill lists lent | start items run with `lend_skills` / `return_skills` like every wired item call; the o-skill sync runs right after the equip so 0x21 precedes the 0x22s as recorded; 0x21 byte 10 = `bonus_level` (`levels.md` §7.1, new `skills::bonus_level_of`, shared with `bonus_level`) |
| stat 5 drift on load (found by `smoke_save`) | §5.5 step 3 adds q to `newskills` and the learn `0x00570080` is the skill-point spend (`levels.md` §6.4 step 4: −1); d2rs's learn did not spend | the desk's `learn_skill` spends 1 (`skpoints` empty in every row) before adding |
| 0x8F (pong) | transport: d2rs's link sends no 0x6D (`client/model.md` §7 r11.4) | left out of the test's recorded count |
| 0xA8 + 3 × 0x1D at frame 2 | the player has no alignment state 105; no spec names the setter | **open**: PC 1 question in `pc1-data.md` Step 4 |

Now 122 of 126; `app_frame_loop` stays red until the alignment question
is answered (the test's later `(handled, queued)` counts will need
updating then). New real-data test
`app_single_player::the_start_items_messages_follow_the_recorded_join`:
the join's 0x21, 0x22 × 2, 0x9D, 0x9C × 6 equal the recording byte for
byte under `scenario-masks.tsv` (passes). `app_items` updated: the new
character's belt holds four potions, so every column is ready.

Real-data d2-client integration run (`--ignored`, cloud, no GPU): see the
comparison with the previous commit below.

## PROVISIONAL

- **REC-862** (`d2-server` `items/moves/preview_skills.rs`
  `sync_oskills`): the 0x21 bonus (`bonus_level`) is computed without
  state 134 (skill shrine, +2), which the skill stage does not carry; a
  skill shrine active while an o-skill item is worn is not counted.
  Settled by staging the unit's states with its list.
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

## Wrap-up state (2026-10-09)

- **Done:** rows `q-fix-tool-c2s-masks`, `q-fix-p3-npc-nearest-player`,
  `q-fix-p3-npc-interact-gate`, `q-tool-playthrough-quests` (marked DONE
  in `build-queue.tsv`); `q-fix-real-join-missing-msgs` done as far as
  the specs go (§4 above).
- **In progress:** nothing uncommitted.
- **Open:** `app_frame_loop` red at 122 / 126 until the PC 1 item
  "[q-fix-pc1-day3-a-r2] Who gives a player its alignment" is answered
  (then: give players state 105 / stat 172 = 2 at the named place with
  the resend, add the player update's step-7 stat sends
  `intents-events.md` §7.3 r1 step 7 if still missing, and update the
  test's `(handled, queued)` counts).
- **RECs:** REC-860 (quest active test act = `unit_act`), REC-862
  (0x21 bonus without state 134).
- **Live runs for PC1-C:** the two above (packets and a1 town arrival).

Repro (cloud: private data repo assembled to `$HOME/game`, see
`tools/realdata-gate.sh`):

```
python3 tools/trace-recorder/packets_diff.py --selftest
python3 tools/playthrough/playthrough.py --selftest
cargo test -p d2-sim -- nearest_client_player interact_npc npc_wants_interact
D2_GAME_DIR=$HOME/game cargo test --release -p d2-client --test app_frame_loop --test app_single_player --test app_items --test smoke_save -- --ignored
```

Known red on this machine before these changes too (no GPU, other
owners): the GPU lib tests, `a_save_from_the_command_line_joins` and the
class-skill app tests (`app_amazon`, `app_assassin`, … 21 binaries).
