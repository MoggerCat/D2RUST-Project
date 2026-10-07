# Handoff: Act IV and Act V quests — `claude/impl-quests-act4-5`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the ninth fold (`claude/fold-handoff-night`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Ninth set" (PC 1 / PC 2).

Cloud implementation session, 2026-10-06, task class: implementation
from a clear spec, medium (METHODS M14). Base: `claude/specs-staging` at
`5844674` (Act IV / V specs merged from PC 2). Repo only, no game files
(M09). Inputs: `specs/world/quests-act4.md`, `quests-act5.md`,
`quests-act5-2.md`, with `quests.md`, `quests-act3.md` §1.1 and
`quests-act2.md` §1.2 for notation. Work split over four parallel
subagents in worktrees (Act IV §3/§4/§6, Act IV §5, Act V part 1, Act V
part 2) on a shared scaffold committed first; merged here.

Parallel sessions implement Act I rest, Act II and Act III. Shared-code
edits are additive. Section 2 lists them, for whoever merges.

## 1. Result (unverified, M02: no Act IV / V recording exists)

| Spec | Module (`d2_sim::world::quests::`) | Tests | Rules claimed |
|---|---|---|---|
| act4 §1.3 sequence, §2 records | `act4` | `act4::q1::tests::sequence_chain` | — |
| act4 §3 The Fallen Angel (chain 22) | `act4::q1` | `act4/q1_tests.rs` (12) | act4 67/75 |
| act4 §4 Hell's Forge (chain 24) | `act4::q3` | `act4/q3_tests.rs` (14) | (same) |
| act4 §5 Terror's End (chain 23) | `act4::q2` | `act4/q2_tests.rs` (20) | (same) |
| act4 §6 Tyrael / Hadriel gossip (21, 29) | `act4::gossip` | `act4/gossip_tests.rs` (2) | (same) |
| act5 §1.3 sequence (31–35), §2 | `act5` | `act5::q1::tests`, `act5::q6::tests::sequence_chain_34_to_36` | — |
| act5 §3 Siege on Harrogath (31) | `act5::q1` | `act5/q1_tests.rs` (10) | act5 47/53 |
| act5 §4 Rescue on Mount Arreat (32), `0x00588C50` | `act5::q2` | `act5/q2_tests.rs` (11) | (same) |
| act5 §5 Prison of Ice (33) | `act5::q3` | `act5/q3_tests.rs` (14) | (same) |
| act5-2 §6 Betrayal of Harrogath (34) | `act5::q4` | `act5/q4_tests.rs` (13) | act5-2 49/50 |
| act5-2 §7 Rite of Passage (35) | `act5::q5` | `act5/q5_tests.rs` (16) | (same) |
| act5-2 §8 Eve of Destruction (36) | `act5::q6` | `act5/q6_tests.rs` (11) | (same) |
| act5-2 §9 Act V intro (40) | `act5::intro` | `act5/intro_tests.rs` (5) | (same) |
| shared notation (act3 §1.1) | `late` | through the above | — |

Every Test-vector row of the three specs is a test. That includes the
Hellforge gem and rune draws from quest seed {12345, 666} (one and two
credited players, normal and hell). It also includes the seal 392
dummy offset, Diablo at the 10th timer firing, the classic 75 / 90 / 95 s
schedule and the Tyrael 676 reward. Unclaimed rules are mostly
conventions (§1.1, §1.2, §1.4, §2) and edge cases for functions with no
caller.

Object init / operate functions, AI hooks and other-system hooks are
`pub fn`s in the quest files (as `act1::q3::malus_operate`). Nothing
outside `world::quests` calls them yet; the object, AI and monster
providers do.

## 2. Shared-code changes (additive; merge notes)

- `quests.rs`:
  - `pub mod act4; act5; late;`.
  - `TimerFn::Act4(act4::Timer)` and `TimerFn::Act5(act5::Timer)`.
  - `request_quest_data` / `send_status` read `act5::barbarians_left` (0x50 bytes 7–8, 0x5D extra).
  - `object_event` routes classes 0x83 / level 108, 0xBD outside Act I, 0x178, 0x1CB–0x1CD and 0x1DA–0x1DC to the act modules.
  - `QuestWorld` gains about 50 default-bodied seams in four marked blocks (`// -- Act IV …` … `// -- end Act V part 2 seams.`). Each default reports `unhandled(0xFE, addr)` and does nothing, so no host had to change.
- `act1.rs`:
  - `Extra` gains `a4` and `a5`.
  - `init`, `sequence`, `callback`, `active_fn`, `status_fn` and `run_timer` route chains 21–24 / 29 and 31–36 / 40 with `act4::owns` / `act5::owns` guards.
  - `status_fn`'s "None" arm narrows from `37..=40` to `37..=39`; 40 is routed to `act5::intro` and still returns `None`.
- `tests.rs` (`Fake`): fields and seam overrides in four marked blocks.
- Existing tests changed. Each change replaces an assertion that a
  function was `unhandled` with the function's specified outcome; none
  was dropped.
  - `tests::fresh_game_entry`: the Act IV / V sequence functions now run (chain 22 holds at 1, chain 31 at 0).
  - `gaps_tests::object_quest_functions_by_class`: classes 0x178, 0xBD, 0x1CB, 0x1CC and 0x1DA–0x1DC.
  - `gaps_tests::player_leaving_with_quest_items`: chains 22, 24 and 31–36 lose P1 from their lists.
  - `mutant_tests::status_message_filter_36_reads_barbarians` and `quest_data_reads_barbarians_only_for_list_36`.
  - `mutant_tests::unspecified_status_function_is_reported`: excludes the now-specified 21, 25, 29, 30, 31, 33, 34.
- Seams that reach the **same 1.14d function** under two names, left
  for the provider session to fold (PLAN decision "Phase 3 system
  seams"):
  - `spawn_object` / `place_object` (`0x00555230`).
  - `refresh_room` / `clear_room_portal_flag` / `unit_room_portal_flag` (`0x0061AED0`).
  - `spawn_superunique` / `spawn_superunique_at_unit` (`0x00545C30`).
  - `create_object_at` / `create_missile_at` (`0x0056EDE0`).
  - `kill_in_place` overlaps the existing `remove_monster` (`0x005A7E60`).
  - Identical duplicates (`unit_distance`, `unit_mode`, `free_object_collision`, `quest_warp`, `player_not_busy`, `reset_interaction`, `game_hook_52e2a0`) were merged at merge time.
- Staging merge (`62e4668`: Act I rest, II, III). The Act IV / V seams were folded into the shared seams where they reach the same function:
  - `spawn_quest_object` (`0x00555230` with flags 1, 0, 0) and `set_room_portal` / `refresh_room` (`0x0061AED0`; `late::unit_room_portal` for "the unit's room").
  - `set_interact_unit(p, None)` (`0x00554190`) and Act II's `free_object_collision`.
  - The Act IV / V `0x006416D0` distance seam is renamed `distance_between`, because staging's `unit_distance` is `0x005DC5C0`. Staging also binds `0x006416D0` as `living_player_within(unit, radius)`. Which reading is right is a spec question (act4 §3.6, act5 §4.6 against quests-act2).
  - `drop_gold_amount` (`0x0055B030`) is distinct from Act II's `drop_gold` (`0x00585970`).
  - `client_idle` (client exists and not busy) stays beside Act II's `player_busy`.
  - `place_object` keeps the general flag form of `0x00555230`.
- Cross-quest links wired after merge:
  - Hadriel reads Terror's End +0x14 (`act4::gossip::diablo_killed`).
  - Dummy 459's init schedules event 7 at frame + 12 while chain 34's +0x87 is set (act5 §5.9).

## 3. Open questions (left `unhandled` / neutral, not guessed)

**Act IV**
1. **OQ2:** the classic end of game runs on `GetTickCount`. d2rs uses 40 ms × (frame − kill frame), the spec's chosen reading. Needs a recording and a `sim/tick.md` exception.
2. **OQ3:** bytes 3–14 of the 0x50 sent to uncredited players are zeros.
3. **OQ4:** `drop_item_at` has no `&level` argument, so the gem / rune slot 50 and the hammer's slot are not passed (`TODO(quests-act4 OQ4)`).
4. **OQ9:** the classic credit's `0x00538680(client, 4, d)` is still `unhandled(23, 0x538680)`, as in A1Q6. Act V uses the `character_progression` seam for the same function. Settle once with the save spec.
5. **OQ5–OQ8:** `0x005A4850`, the ghost removal `0x005E7350`, `0x0061AED0` after the dummy, and the +0xAE0 superunique table are each left to their owner specs (monster, AI, object, data).
6. Hellforge delays use `FrameCnt1` = `FrameCnt3` = 22 as constants. The spec gives the live values, not a reading function. Act IV §5 reads `FrameCnt1` through the `object_frame_count1` seam.
7. The §8 monster / superunique / item creation links are exposed as `link_*` fns for chains 22 and 24 only. Chain 23's links (bases 243, hcIdx 36–38) are not; the caller is monster creation.
8. Readings marked in code:
   - Diablo spawn tries r = −1 / 5 / 10 at one spot.
   - A missing start-point object counts as a failed spawn.
   - Q1 event 3's silent status sits inside the state-2 test.
   - Hadriel's wish to talk tests "26.13 and 26.0 both clear".

**Act V**
1. `nihlathak_dummy_event` (`0x00589540`, part 1 §5.9): "kills him again if he is back" has no test for "back". Still `unhandled`.
2. §5.7 scroll reward: when iced Anya is interacting, `0x00589070` is `unhandled`; the spec doesn't say which player gets S5D(33, 0x20, 0).
3. §5.9 Anya's to-town portal flag arguments are not given; (1, 1, 0) is used, as for the outside portal (`TODO`).
4. Anya AI hooks `0x0058A7D0` … `0x0058AA10` and the map-AI stores `0x0058AD80` / `0x0058AE10` are not mapped to effects (part 1 OQ4).
5. Part 2: Tyrael's last portal `0x0058D7D0` and the Tyrael spawn `0x0058E920` miss their free-spot limit. The spawn also needs flags 0x42, which `spawn_monster` cannot carry. Both are `unhandled(36, …)`.
6. Part 2 §7.8: summit door 564 in mode 0 after the Ancients names no warp function (`unhandled(35, 0x58D6A0)`).
7. Part 2 OQ8: row 40 of `quests.tsv` is `?` and its 15 message rows are not in `quest-messages.tsv`. `act5::intro::init` writes what init `0x0058EA50` stores, and `intro::TABLE` holds the spec's §9 rows. Spec task: fill the TSVs (and the 779-row count in `tables_parse_and_check`), then drop the constant.
8. Readings marked in code:
   - Plain "status n" is a byte write.
   - "status n (silent)" leaves flags alone (`late::status_silent`, from `quests.md` §10.1 status(S)).
   - §4.7 keeps flags 0x20 through "status 2 to all".
   - Freed > 15 → 36.5.
   - §7.6's post-kill steps sit inside not-intro.
   - §8.5 "none → stop" ends step 1 only.
   - Experience stats 13 / 29 are written through `add_stat` (no set-stat function named).

## 4. Local checks to queue (M02, HANDOFF §5 S9-A3)

- **A (player):** record a full Act IV run with packets and RNG (act4 OQ13).
  - Izual and the ghost, the Hellforge drops, the seals and seal bosses.
  - Diablo's spawn and death, in classic (with one uncredited player, OQ3) and in expansion.
  - The Harrogath portal.
  - Expect: the quest-seed draws of act4 Test vectors, the 0x5D / 0x89 sequences and the warp / end frames of OQ2.
- **A (player):** record a full Act V run with packets and RNG (act5 OQ7).
  - Shenk, the rescue portals, the rune reward.
  - Anya's thaw, the scroll and Anya's item draw.
  - The Ancients' experience, Baal and the last portal.

## 5. Gate

On `claude/impl-quests-act4-5`:
- `cargo test -p d2-sim -p d2-server`: green except the 10 known red tests of other sessions (`missiles::tests_bodies::*`, `monsters::ai::tests::specd_here_*`, `skills::use_::tests::*`, `skills::mutant_tests::table_check_mutants::*`); `world::quests` 246 tests pass.
- `cargo clippy -p d2-sim -p d2-server --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `py tools/coverage.py --check`: 0 errors.
- `py tools/spec_index.py --check`: clean.
