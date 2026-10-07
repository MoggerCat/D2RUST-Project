# Handoff: the Act II quests callback by callback — `claude/impl-quests-act2`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the ninth fold (`claude/fold-handoff-night`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Ninth set" (PC 1 / PC 2).

Cloud implementation session, 2026-10-06, from `claude/specs-staging`
(`5844674`). Spec: `specs/world/quests-act2.md` (draft, read from the
1.14d `Game.exe`; no Act II recording exists). The coordinator folds this
file into `docs/HANDOFF.md` / `docs/PLAN.md`.

## State

**Implemented, unverified** (no trace of Act II has run against this
code).

- New module `d2_sim::world::quests::act2` (`crates/d2-sim/src/world/
  quests/act2.rs` + `act2/`): the §1.1 shorthands, the §2 record init
  (chains 8–13 active; chain 8 state 1; chain 9 status 13), the §1.4
  sequence chain (8 → 13 → 10 → 11), timers (`act2::Timer`), dispatch.
- Per quest, every registered callback, active and status function the
  spec describes:
  - `q1.rs` A2Q1 Radament (§3): events 0, 2, 3, 8, 10, 11, 13, timer;
    `radament_ai` (§3.6), `use_book_of_skill` (§3.8).
  - `q2.rs` A2Q2 Horadric Staff (§4): Cain selector, chat, messages,
    status fn, the three chests (`scroll_chest`, `staff_chest`,
    `cube_chest`, §1.3 pattern), pick-up / drop, events 3, 9, 10, 13,
    14, `staff_assembled` (§4.9 cube hook).
  - `q3.rs` A2Q3 Tainted Sun (§5): darken, triggers, `act_load`,
    `altar_operate` (operate 24), `altar_init` (init 20), timers.
  - `q4.rs` A2Q4 Arcane Sanctuary (§6): Kaelan's quest-seed draw,
    messages, levels, `tome_operate` (42), harem blocker (event 7 + init
    30), Sanctuary portal (init 29, `portal_operate`), `arcane_hook`,
    the palace-guard / Jerhyn §10 hooks; Jerhyn's spawn only partly
    (below).
  - `q5.rs` A2Q5 Summoner (§7): AI hook `summoner_seen`, chat, kill,
    two-phase timer, messages, events 3, 10, 13.
  - `q6.rs` A2Q6 Seven Tombs (§8): `true_tombs` (§8.1, pure over the
    DRLG seed), chat, levels, status fn, orifice operate / insert /
    hand-in, lair timer and object inits, `lair_warp_open` (warp check
    `0x0059DB20`), `portal_destination` (`0x0059DFD0`), arcane dummy
    list, messages 302 / 430 / 442 / 450 / 444–452, Duriel's kill and
    timer, chat end, events 10, 13, 14, the §10 hooks.
  - `gossip.rs` chains 7, 26, 27, 38 (§9).
- Shared code, additive: `act1::callback` / `active_fn` / `status_fn` /
  `sequence` / `run_timer` route Act II chains to `act2`;
  `TimerFn::Act2`; `act1::Extra::a2`; `QuestControl::new` calls
  `act2::init`; `object_event` class 0x7A → `act2::q4::harem_blocker`.
- Tests: `act2/tests_common.rs`, `tests_q1.rs` … `tests_q6.rs`,
  `tests_gossip.rs` (81 tests, every spec test vector except the live
  `missiles.txt` one, `// Covers:` claims on §1–§10 and edge cases).
- Gate: `cargo test -p d2-sim -p d2-server --no-fail-fast`: only the 10
  known red tests of other sessions (`missiles::tests_bodies`,
  `monsters::ai::tests::specd_here_*`, `skills::use_::tests::*`,
  `skills::mutant_tests::table_check_mutants::*`); clippy `-D warnings`
  and fmt clean; `tools/coverage.py --check` 0 errors;
  `tools/spec_index.py --check` clean.

## Existing tests changed (behavior now implemented, none weakened)

| Test | Before | Now |
|---|---|---|
| `tests::fresh_game_entry` | expected `unhandled 8 0x5991c0` | chain 8's seq runs (state 1 → returns 1; chain 13 stays 0, asserted) |
| `gaps_tests::pick_up_and_drop_reach_only_active_records` | chain 9 inactive at creation; active half saw `unhandled` | chain 9 is active from init (§2), switched off explicitly; active half checks real effects (`tr1 ` → status 1, `vip ` drop clears 10.4) |
| `gaps_tests::player_leaving_with_quest_items` | every non-Act-I event 10 logged unhandled | Act II chains excluded from the log list, and their lists asserted to lose the player |
| `gaps_tests::dispatch_to_all_records` | Act II reach shown by unhandled logs | shown by chain 13 msg 444 setting 14.9 |
| `gaps_tests` object-event table | row `(0x7A, "unhandled 11 0x59b710")` | row removed; 0x7A covered in `tests_q4::harem_blocker` |

## Seams added (`QuestWorld`, default bodies report `unhandled(0xFF, addr)`)

No host provides them yet (`EconomyQuests`, the `d2-server` fake use the
defaults); the quests' test fake implements them.

`client_in_act` (`0x005382B0`), `start_tainted_sun` / `end_tainted_sun`
(`0x0061C450` / `0x0061C4D0`), `quest_chest_gate` (`0x00545850`),
`quest_drop` (drop code + `0x00559A30`), `identify_item` (`0x006280D0`),
`object_treasure` (`0x00585B90`), `drop_gold` (`0x00585970`),
`set_room_portal` (`0x0061AED0`), `spawn_object` (`0x00555230`),
`free_object_collision` (`0x00623830`), `player_busy` (`0x00535060`),
`interact_unit` / `set_interact_unit` (`0x00554120` / `0x00554190`),
`open_insert_dialog` (S→C 0x58, `0x0053D8D0`), `missile_range`
(`missiles.txt` Range), `is_trading` (`0x005678A0`), `remove_unit`
(`0x0052E050`), `npc_hold_chat` (`0x00572DC0` + `0x00573180`),
`spawn_location` (`0x0061B060`), `free_spot_near` (`0x0064E7E0`),
`unit_distance` (`0x005DC5C0`), `living_player_within` (`0x006416D0`),
`npc_intro_heard` / `set_npc_intro` (`0x005723C0` / `0x00572360`).

Callers still to wire (object, item-use, cube, AI, DRLG specs): the
chest / altar / orifice / tome / portal / blocker operate and init
functions, `use_book_of_skill`, `staff_assembled`, `radament_ai`,
`summoner_seen`, the palace / Tyrael hooks, `act_load`, `true_tombs`,
`lair_warp_open` and `portal_destination` behind `WarpCheck::Delegate`.

## Pending (reported through `unhandled` or a `TODO(quests-act2 …)`, 22 TODOs)

- §6.10 Jerhyn's palace spawn: base point of the x + 15 / x − 10, y − 3
  offsets, the spawn's `r` argument and who stores +0x3C are not in the
  spec → `unhandled(11, 0x0059EF70)` when +0x0D ≠ 1.
- §8.11 Tyrael's party tail (`0x0059C9A0` → `0x0059C920`, "per D2MOO,
  not re-read") → reported; members get no bits.
- `0x00538680` (OQ7), `0x005940A0` (chain 7 Cain hook), chain 38's
  active fn `0x005985C0`, status fns of chains 26 / 27 / 38, the §3.6
  branch status ≥ 2 with state < 3, the orifice operate with a busy
  player.
- Chest drop code is set per created item (the seam), not once before
  the count (§1.3); chest item level passed as `None` (not in the spec).
- The live check of `missiles.txt` row 338 Range 440 → period 18
  (`#[ignore]` game-file test) is not written (Local run queue below).

## Open questions (readings chosen; confirm in the spec)

1. Test vector 1 says "table state 2 (msgs 325–334)"; table state 2 of
   chain 8 holds 315–324. Tests follow §1.2 (Atma's 317).
2. Chain 8 event 10 `0x00598980` and chain 7 event 10 bodies are not
   described: removal from the record list (8) / the extra list (7).
3. Do the event-11 handlers of chains 26 / 27 test the NPC class? Only
   the message is tested.
4. §5.7: the altar does not call the quest-chest gate (§1.3 lists it
   under the chest pattern).
5. §5.8: darken always sends status 1 first, so "status 0 → status 2 to
   all" cannot be reached — confirm.
6. "status n, state m" in game-start callbacks: bytes only, no flags
   reset, nothing sent (as Act I's restore).
7. §5.6 / §6.7 ";" scope: "if intro: game 11.13" after the 11.1 block;
   §6.7 grants run in intro games too.
8. §7.2: +0x09 := 0 on every kill (not only when the timer is made);
   the 13.2 iterate inside the "status < 2" branch.
9. §8.3: Tyrael only ever adds table state 2 and stops. §8.11 msg 442:
   only state := 5 depends on 14.13. Msg 430: refresh before state := 2.
10. §8.6 0x44 with a non-orifice class returns 5 (literal "else"); the
    0x58 layout is not in the spec (caller sends it).
11. §8.11 kill party credit: members get the player's test (lacks 14.0,
    14.3, 14.4, 14.5).
12. Pick-up (§4.8): 0x5D also for `tr1 ` with 10.3 set (status
    unchanged).
13. Events 13 / 14 count at most one item per code (`has_item` is a
    yes/no seam); a count seam would be exact.
14. OQ5 of the spec (chain 38's intro record) stays open; the seams keep
    the storage choice to the host.

## Local run queue (HANDOFF §5 C91, S9-A3)

- `missiles.txt` row 338 `horadricstaff` Range: expect 440 → orifice
  timer period (440 − 75) / 20 = 18. Add an `#[ignore]` test reading
  `D2_GAME_DIR` once a missiles table loader is reachable from the quest
  tests.
- Spec OQ10: record a full Act II run (packets + RNG) to verify message
  order: chat-end status, kill timers, Tyrael's portal, Meshif.
