# Handoff: Act II–V monster AI bodies — `claude/impl-ai-acts2-5`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the ninth fold (`claude/fold-handoff-night`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Ninth set" (PC 1 / PC 2).

Cloud implementation session, 2026-10-06. Base `claude/specs-staging` @
`f294bbf`. Repo only, synthetic fakes, no game files. Specs:
`specs/monsters/ai-bodies-2.md`, `ai-bodies-3.md`, `ai-bodies-4.md`,
`ai-bodies-5.md` (and `ai.md` §3, §7, §10 for the shared machinery).

## What was implemented

All 56 AI functions that `ai-functions.tsv` marks `spec'd-here` beyond
the Act I set now have bodies (93 of 148 indices; the other 55 are
`unread` and stay stubs that log `Unhandled::Function`). Every rule is
implemented as written; where the text leaves a choice open the code
takes the narrowest reading with a `TODO(spec: …)` (list in Open
questions).

| Module (`crates/d2-sim/src/monsters/ai/`) | Spec | Functions |
|---|---|---|
| `bodies2.rs` | `ai-bodies-2.md` §2–§16 | PantherJavelin 95, GreaterMummy 22 (scan `0x005F2A00`), Mummy 21, PantherWoman 18, MaggotLarva 38, SandLeaper 17, MaggotEgg 40, PinHead 39, ClawViper 16, Vulture 23, BatDemon 29 + alternate `0x005F4FD0`, SandMaggotQueen 66, Duriel 44, Summoner 53; special-state thinks 10 / 17 `0x005E8020`, 11 init `0x005E80E0` + think `0x005E8140`, 12 `0x005E8340` |
| `bodies3.rs` | `ai-bodies-3.md` §2–§11 | Mosquito 24, ThornHulk 27, ZakarumZealot 48, ZakarumPriest 49, FrogDemon 52 + alternate, FetishShaman 65 + alternate, HighPriest 85, FetishBlowgun 96, WillOWisp 25 (ritual tables), Mephisto 50 |
| `bodies4.rs` | `ai-bodies-4.md` §2–§11 | VileMother 68 (birth), VileDog 69, FingerMage 70, Regurgitator 71, Megademon 89, Diablo 51 + alternate `0x005E8480` (also BaalCrab's `0x005FCF30`), the boss pick §7.1, score §7.2 (and the Baal score of `ai-bodies-5.md` §21.1), choice §7.3, Izual 55, DoomKnight 72, AbyssKnight 73, OblivionKnight 74 |
| `bodies5.rs` | `ai-bodies-5.md` §2–§23 | Minion 116, Imp 122 + init, Succubus 118, BloodLord 125, SuccubusWitch 119, Overseer 120, ReanimatedHorde 114, ClawViperEx 142, DeathMauler 130, PutridDefiler 137, Ancient 133 (Talic / Madawc / Korlic), AncientStatue 132, FrozenHorror 124, SiegeBeast 115, SuicideMinion 117 (also special state 15), BaalMinion 141, BaalTaunt 136, BaalToStairs 138, BaalThrone 134 + init, BaalCrab 135, BaalCrabClone 140 (choice, execution, clone), Nihlathak 128 + init + alternate `0x005E5280` (also the Hireable alternate) |
| `common.rs` | Summaries of `ai-bodies-2.md` / `-5.md`, `ai.md` §5.4, §6 | "wait N", "mode m at (x, y)", "Skill k at U", scan mode 0 over the adjacent rooms, squared distance, alignment pairing, `BaseId`, "has skill", teleport in range, `0x006416D0` / `0x006417F0` distances, the pack scan, the Vulture land / take-off helpers |

Scan callbacks with a full rule (pack, mummy, carrion, heal, fetish,
knight, corpse, minion, rider, host, ally, portal) run in `d2-sim` over
scan mode 0 (`game.lists` adjacent rooms, room unit-list order). The
WillOWisp unit find (`0x0065A950`, OQ3) is a seam returning the units in
the find's order; its callback runs here.

### Seams and tables

- New seam trait `AiActs` (`seams.rs`), part of `AiHost`: unit flags
  read / clear, max life / mana, state groups, states count, stat-list
  flag (curse), hostility, owner and owner record, quest flag, portal
  GUID, monster-data component byte, target unit, target override,
  chain byte, class for level, skill entries / levels / hand skills /
  add / assign / param, skill check with point, corpse search, path
  pattern / move mask / placement / pattern stamp / cell clear / point
  and pattern collision / free point / free spot / room at point / mode
  walk-in-radius / path target / path compute / 64-direction / path stop,
  spawn, queen spawn class (OQ4), kill, remove, clone links, reinit
  class, change-class list, wisp buff, preload (S→C 0xA4), wisp find,
  wave record, room portal flag, and one `quest_call(QuestCall)` for the
  nine quest seams.
- `wiring::action::View` implements it: unit flags, max life, state
  groups and the states count are real (`units.md` §2, `stats.md`,
  `stat-lists.md` §9.3); move mask and path stop go to the existing path
  calls of `Pending`; every other call is a new `Pending::ai_*` method
  whose default is nothing / false / `None`.
- `AiTables` gains `skills` and `missiles` (`aurastate`,
  `auratargetstate`, `attackrank`, `Param5`, `Range`); `skill_modes` is
  now `[u8; 8]` (`Sk1mode`..`Sk8mode`, +0x180..+0x187), so
  `ActionTables::skill_modes` changed type. Test literals `vec![[0; 4]]`
  became `[[0; 8]]` in `d2-sim` and in three `d2-client` tests
  (`e2e_full_loop`, `prop_worldsim`, `e2e_single_player`; mechanical,
  not compiled here: Bevy). `d2-server` and `test-fixtures` check clean.
- `use_skill` (`0x005DEAD0`) returns the mode-change result (Diablo's
  aura, Madawc, Nihlathak test it). `Ctx::class_skill` / `class_aip`
  read any row (Imp's fixed rows 492–495). `AiControl::spawn_class` is
  control +0x3C (Nihlathak).
- `SPECD_HERE` and `IMPLEMENTED` mirror the rebuilt catalogue (93 rows):
  `specd_here_matches_tsv` and `specd_here_check_catches_perturbations`
  are green. The `drehyaiced` 527/528 TODO in `npc.rs` is gone (the
  rebuilt `monstats_rows` lists 527); the game-file test
  `ai_index_of_every_row` reads the TSV directly and needs no change.

## Tests and rule coverage

`tests/act2.rs`..`act5.rs` (86 tests): every spec test vector, the edge
cases, and per-step rules, on the parent module's fake (new `Acts`
knobs). M08 by hand: Vulture `mask(8) + 25`, Diablo W3 71, Baal W10 151
each turned exactly the expected test red; reverted.

Two existing tests were adjusted, not weakened: `stub_ai_logged` used
Mephisto as its unread AI (now Towner, 41); `think_rhythm_table` checked
FrogDemon's target-mode-5 finder through a stub body, now through
`precheck_b` directly (the body runs on after the finder, as §2.3 says).

Unit-tier coverage (`py tools/coverage.py --summary`): `ai-bodies-2.md`
108/109, `ai-bodies-3.md` 70/72, `ai-bodies-4.md` 82/84,
`ai-bodies-5.md` 142/143. Not claimed, with reasons: each file's §1
(scope tables, no rule to test); `ai-bodies-3.md` edge case 6
(Mephisto's dead pick branch and case 0: unreachable by construction);
`ai-bodies-4.md` edge case 4 (a crash in 1.14d with invalid skills, not
reachable).

Verified tier: none. No recording covers any Act II–V AI (each spec's
Open question 1); everything here is **unverified**.

Red on the base and still red (other sessions'): `skills::use_::tests`
(function tables, bodies), `skills::mutant_tests::table_check_mutants`
(2), `missiles::tests_bodies` (2). Everything else in `cargo test -p
d2-sim` passes; clippy, fmt, coverage and spec_index checks pass.

## Open questions (readings taken, `TODO(spec: …)` in the code)

1. `ai-bodies-2.md` §11 step 1: Vulture with T = 0 and a failed landing
   (1.14d reads a null unit): stops.
2. `ai-bodies-2.md` §15, `ai-bodies-3.md` §8 step 1.3, `ai-bodies-4.md`
   §8: `0x005DEAD0(mode, skill, T, x, y)` with both a unit and a point;
   the mode request holds one: Summoner and Izual use T, the HighPriest
   hydra uses the point.
3. `ai-bodies-3.md` §5 step 1.1: a failed teleport check goes on to
   step 1.2 (cooldown spent).
4. `ai-bodies-3.md` §8 step 1.2: the heal-scan callback does not exclude
   the scanner; read literally.
5. `ai-bodies-3.md` §10: ritual slot ≤ 0 reads an uninitialised stack
   entry; (0, 0) used. Ritual points with T = 0 use the own position.
6. `ai-bodies-3.md` §11 case 2: with n = 0 skills, whether the closing
   `50 − K` draw follows the wander; it does not here.
7. `ai-bodies-3.md` §4 step 1: a monster T without an owner record keeps
   Q = T.
8. `ai-bodies-4.md` §5 step 2: "Then s := 3; idle 8" read as following
   both the walk and the near case.
9. `ai-bodies-4.md` §7.1 step 3: with no best, the melee test and path
   compute see unit 0; read as "swap to the alternative".
10. `ai-bodies-4.md` §7.2 step 6: "d58 >> 8 + d55 + …" read as
    (d58 >> 8) + d55 + … (C precedence would shift the whole sum).
11. `ai-bodies-5.md` §3 step 5: "else" read as the I3 draw failing;
    §3: a missing Imp row reads aips 0 (1.14d: null pointer).
12. `ai-bodies-5.md` §6 step 3.1: the aip8 draw only when S was found.
13. `ai-bodies-5.md` §7 step 4: C with a draw ≥ aip6 goes on to step 5.
14. `ai-bodies-5.md` §12 A step 3: the whirlwind request byte +0x15 :=
    100 has no field in the mode request here.
15. `ai-bodies-5.md` §15 step 2: "nearer" read as squared distance.
16. `ai-bodies-5.md` §20 step 7: a missing wave record passes −1 as the
    skill param (1.14d: fatal on the entry).
17. `ai-bodies-5.md` OQ2: the clone's spawn info `0x0063EFA0` (its draws
    and any change of class / point / mode) is not modelled; class 570,
    mode 1 and the ±12 point are used as given.
18. `ai.md` §7.1: `0x005DDFC0` (SC at a point, WillOWisp) read as not
    setting the path step count, like `0x005DE490`.
19. The seams with no provider (all `Pending::ai_*`): the providers are
    the skills session (entries, checks, corpse search), path session
    (patterns, placement, free points, 64-direction), population / init
    (spawn, queen class OQ4, chain byte, class for level), damage
    (kill), stat lists (curse flag, wisp buff, change-class list) and
    quests (`QuestCall`).

## Local run queue (HANDOFF §5 C87; recordings S9-A4)

- `cargo test -p d2-sim --test game_monsters -- --ignored
  ai_index_of_every_row` with `D2_GAME_DIR`: the rebuilt
  `monstats_rows` should now match live (0 differing counts and rows).
- Recordings per spec OQ1 (Act II–V runs with the tick recorder) to move
  the bodies from unverified.
