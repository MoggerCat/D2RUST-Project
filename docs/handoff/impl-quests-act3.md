# Handoff: the Act III quests callback by callback — `claude/impl-quests-act3`

Cloud implementation session, 2026-10-06, from `claude/specs-staging`
(`5844674`). Spec: `specs/world/quests-act3.md` (draft, read from the
1.14d disassembly; no Act III recording exists). The coordinator folds
this file into `docs/HANDOFF.md`.

## State

**Implemented, unverified** (no trace of any Act III quest has run
against this code).

- New module `d2_sim::world::quests::act3` (`act3.rs` + `act3/q0.rs` …
  `q7.rs`, `intro.rs`): every Act III record — A3Q0 Hratli (chain 14),
  A3Q1 Lam Esen (15), A3Q2 Khalim's Will (16), A3Q3 Blade (17), A3Q4
  Golden Bird (18), A3Q5 Blackened Temple (19), A3Q6 Guardian (20), A3Q7
  Dark Wanderer (28), the Act III intro (39): every callback `quests.tsv`
  registers and the ones the spec installs at run time (chat ends, the
  Gidbinn boss and figurine kills), status and active functions, the
  §1.3 sequence chain, the §2 init stores, the three timers (Gidbinn
  boss, Mephisto status, wanderer minions), the quest objects of §1.4
  (tome, decoy, altar, sewer lever and stairs, Khalim's chests, the
  Compelling Orb, stairs R, Hellgate, bridge, Natalya, Hratli dummies,
  wanderer), and the §10 hooks as public functions (`monster_link`,
  `superunique_link`, `choose_bird_boss`, `bird_boss_removed`,
  `council_preset`, `alkor_bird_brought` / `alkor_bird_clear`,
  `altar_position` / `activate_altar`, `wanderer_target` /
  `wanderer_minions`, `will_cubed`, `potion_of_life`, `durance_open`,
  `durance_warp`).
- Quest-seed draws in spec order: Khalim's chest (one step, lo' mod 5 + 5
  gold piles before the parts), the minion timer (one step, lo' & 1).
- Tests: `world/quests/act3_tests.rs` (the `Fake3` wrapper over the
  shared fake with the Act III seams) and `act3_tests/{q1..q6, gossip,
  wiring}.rs`, every Test-vector row of the spec, with `// Covers:`
  claims on every §1–§10 rule and the Act III edge cases.

## Shared-code changes (all additive)

| Where | Change |
|---|---|
| `quests.rs` | `pub mod act3`; `TimerFn::Act3(act3::Timer)`; `act3::init` after `act1::init`; object event 7 of class 0x155 → `act3::bridge_event`, 0x16F → `act3::lever_event`; `object_warp` to a level ≠ 102 → `act3::durance_warp` (`0x005BCFD0`, `quests.md` §8.1) |
| `QuestWorld` | 18 Act III seams with **default bodies** that report the function through `unhandled(0xFF, addr)` and return the neutral value (no implementor had to change): `drop_quest_item` (defaults to `drop_item_at`), `has_act3`, `quest_chest_gate`, `drop_gold_pile`, `chest_treasure`, `spawn_monster_in_room`, `spawn_monster_at_unit`, `kill_monster`, `room_covering`, `player_in_rooms`, `stairs_warp`, `player_near_object`, `free_collision`, `special_monster`, `blocked`, `spawn_object`, `trading`, `weapon_code` |
| `act1.rs` | dispatch arms for chains 14–20, 28, 39 in `callback`, `active_fn`, `status_fn`, `sequence` (15–20), `run_timer`; `Extra::act3` |
| tests | `fresh_game_entry` no longer expects chain 18's sequence function reported; `object_quest_functions_by_class` drops classes 0x155 / 0x16F (now Act III code, tested in `act3_tests`); `player_leaving_with_quest_items` expects the Act III event-10 bodies (list remove; chain 16's bare `ret`) |

Host providers for the new seams are not written: on the wired host
(`EconomyQuests`) they report `unhandled` until a provider session adds
them.

## Readings taken literally (worth a trace)

Each is the literal reading of the spec text; a trace settles them.

1. §3.3 msg 564: the tail (status 13 silent, add GUID, game 17.13, state
   5, seq) runs whether or not +0x00 = 1.
2. §3.3 / §3.4 flag iterate `0x005B73A0` sets 17.2 at state 2 or 3; no
   rule sets 17.3, though event 13 reads it (question 3 below).
3. "status n" without "to all" (events 13 of chains 15–18) is stored
   silently; chain 17 also zeroes the flags byte, chain 18 stores the
   status byte alone.
4. §4.3: chat steps 1, 2, 3 are exclusive (step 1 returns).
5. §4.7 lever event 7: the stairs' end-animation event uses the lever's
   frame rule, frame + (`FrameCnt1` >> 8) (`TODO(quests-act3 §4.7)`).
6. §5.6 decoy operate and §7.7 orb operate: "object mode 0" read as a
   precondition (mode ≠ 0 → return), as in the tome operate.
7. §5.6: only the timer installs the Gidbinn boss's kill callback 8; a
   boss spawned by decoy init 25 (edge case 4) drops no Gidbinn.
8. §5.8 events 3 and 13: "the flag iterate" (no "for all") runs for the
   acting player only; §5.8 event 13 with 19.6 clear runs both branches in
   order (the later one can override the 19.5 result).
9. §6.2 `choose_bird_boss` also tests the unit is in Act III (taken as
   part of `0x00544E80`); §6.5 Cain 527: state 2, +0x09 and callback 2
   depend on state 1, the refresh, +0x18 and the party pass always run;
   the Golden Bird party pass (`0x005BA290`) has no Act III test, the
   Blade's (`0x005B91B0`) has.
10. §6.8: the three game-start tests run in turn (no stop).
11. §7.3 Cain 626: the own seq fn runs on every accepted 626, not only in
    the "21.13 and state ≠ 7" branch.
12. §7.5: the status test runs whether or not the GUID was new.
13. §7.6 r3, §8.5 r1: "for each player from the victim: A, B, C" is three
    walks (as A1Q6), not one walk calling A, B, C per player.
14. §7.6 r3: chains 19 / 20 use the default status rule (init_no 6), so
    "status 4 to all" after the last council kill (state 6) sends `5D 13
    00 0C` to players without 21.13 — the tests assert this.
15. §8.2: messages 657–663 refresh the text even without 22.11; §8.5:
    +0x0C := 2 also without a known Hellgate; +0x00 := 1 even when
    `add_timer` fails (fault pushed).
16. §8.6 bridge event 7: "already in mode 2" is read at the event's
    start, so after mode 1 → 2 the event is scheduled once more.
17. §9.1 init 50 "Hratli from the start exists" = init 49's test (+0x00
    and the +0x0C unit exists); §9.2 the walk-target collision test and
    the minion free-spot search use the wanderer's room.
18. Event 10 of chains 15 (`0x005B7610`) and 18 (`0x005BA990`) is the
    §1.1 list remove (`0x00545530`); the spec states it explicitly only
    for 19 and 20; chain 16's is a bare `ret`.
19. Object functions without their chain's record do nothing (no fatal
    named), except tome init 23 (`0x00544E30`: `QuestError::Fatal`).

## Open questions / TODOs

1. 17.3 (Lam Esen "left town"): which code sets it? Event 13 restores
   state 3 from it (§3.7), but no §3 rule writes it.
2. `0x00538680(client, 3, difficulty)` on Mephisto's credit (spec OQ5):
   reported through `unhandled(20, 0x538680)`.
3. Spec OQ2 (`0x00559A30` `&level`), OQ3 (monstats +0x0D bit 6, used as
   0x40), OQ4 (`0x005A0180` / `0x0063E9F0`, the `special_monster` seam),
   OQ6 (`0x0063BEF0` weapon slot, the `weapon_code` seam) stay open.
4. Natalya / Hratli map AI (`0x005BD040`, `0x005B7230`): no caller in
   1.14d, so +0x24 / +0x18 are never set; the apply branch reports
   `0x0058F000` if ever reached.
5. Missing positions (decoy / altar / Hratli dummies / wanderer without
   a room): `TODO(quests-act3 …)` + `unhandled` where the spec gives no
   form.
6. Host providers for the 18 new `QuestWorld` seams (wired host
   `EconomyQuests` / `QuestRest`) and the callers of the §10 hooks
   (monster creation, NPC map AI, cube `qf2 `, item use `xyz `, object
   operate / init tables, the 0x64 warp check) are not wired.
7. `quests.tsv` column `spec` still says `catalogued` for the Act III
   rows (spec OQ7).

## Gate

`cargo test -p d2-sim -p d2-server`: all pass except the 10 known red
d2-sim tests of other sessions (`missiles::tests_bodies` 2,
`monsters::ai::tests::specd_here_*` 2, `skills::use_::tests::*` 4,
`skills::mutant_tests::table_check_mutants::*` 2); `cargo clippy -p d2-sim
-p d2-server --all-targets -D warnings`, `cargo fmt --check`,
`tools/coverage.py --check`, `tools/spec_index.py --check` pass. 145 new
`act3` tests.

## Local run queue (add to `docs/HANDOFF.md` §5)

1. Record a full Act III run (spec open question 8; `tools/trace-recorder`,
   1.14d, single player): Alkor 564 with a second player in Act III and a
   third elsewhere (17.13 / 17.0 / +5 / `5D 0F 02 00 0000` vs 17.14);
   a Khalim chest (quest-seed draw, gold piles before the part, treasure
   last); a council kill (the 0x5D status byte: 12 by the default rule,
   readings below); two orb hits; Mephisto (timer status 4 after 12
   updater ticks, Hellgate mode, bridge); Natalya's spawn; the Dark
   Wanderer's minion dummies (7 or 8 by lo' & 1). Compare with the
   `act3_tests` expectations named in each test.
