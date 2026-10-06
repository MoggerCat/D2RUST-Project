# Handoff: every Act I quest chain callback by callback — `claude/impl-quests-act1`

Cloud implementation session, 2026-10-06. Two rounds:

1. From `b435f5a` (main + `claude/spec-quests-act1` merged at
   `6b0be8b`): the `6b0be8b^1..6b0be8b` diff of `specs/world/quests.md`
   (§10.1 sequence walk, §10.4 A1Q1, §10.5 A1Q2 / A1Q3). Commit
   `86a3a6c`.
2. After merging `origin/claude/tender-meitner-mphas3` @ `dab6dce`
   (spec `claude/spec-quests-act1` @ `946c602`, at the coordinator's
   request): the `6b0be8b..HEAD` diff (§8.1 act-transition values and
   send order, §9.4 `trs `, §9.5 object table, §10.3 intro record, §10.6
   A1Q4, §10.7 A1Q5, §10.8 A1Q6, six more test vectors, open questions
   10–13).

3. Merged `origin/claude/tender-meitner-mphas3` again (the base folded
   the fixture copies of `d2-server/tests/prop_handle.rs` into shared
   test support): the conflict was only this branch's seam stubs inside
   the removed block; the shared fixture already carries them, so the
   file is taken as the base's (all 20 of its tests kept). Gate after the
   merge: every step passes except `test d2-sim + conformance`, whose 9
   failures are the pre-existing ones below; d2-client now passes.

The coordinator folds this file into `docs/HANDOFF.md` / `docs/PLAN.md`.

## State

**Implemented, unverified** (no trace of these quests has run against
this code; the spec rules come from the 1.14d disassembly).

- Every Act I chain runs its registered callbacks from §10: A1Q1–A1Q6
  (`d2-sim/src/world/quests/act1/q1.rs`–`q6.rs`), the Act I intro record
  (chain 37, `act1/intro.rs`), A1Q0 / Flavie / the respec record
  (`act1.rs`). Active functions of chains 1–6 and 37, A1Q3's status
  function, the timers (`0x00590230`, `0x00590BF0`, `0x00593260`,
  `0x005954C0`, `0x00596500`, `0x00596580`), the object functions
  (Malus init / operate, Inifuss tree, Cairn stones, Wirt's body, tower
  tome, tower chests, `0x00595A00`), the class-61 link, the town Cain
  spawn, the Cain cleanup and the act-change hook.
- The sequence functions of chains 1–6 follow the §10.1 walk; the unlock
  order from Den of Evil is 1 → 2 → 4 → 3 → 6 (chain 6 opens through its
  period-20 timer).
- §8.1: 0x61 bytes and send orders from the table (Warriv: 0x28, `61
  02`, intro flags, then chain 6's event 3 (a 1, b 40) and A1Q4's act
  change; Meshif `61 03`; Tyrael `5D 17 02 00 0000` then `61 05`;
  Durance `61 04`). §9.4 `trs `: the 0x50 (u16 13, tomb level − 66).
  §9.5: the object-event table; Act II–V functions are reported.
- Tests: `world/quests/act1_tests.rs` (35, every §10 test vector and the
  §8.1 / §10.1 / §10.3–§10.8 rules, with `// Covers:`); six wired-host
  sequences in `d2-server` `adapters/handlers/world/tests/quests_act1.rs`
  (`WiredWorld`: real `QuestControl` on `EconomyQuests`, real units, stat
  lists and items, messages through the host frame), one per chain from
  its opening state to its last: Den of Evil 1 → 5, Burial Grounds 0 →
  5, Tools of the Trade 0 → 5 + imbue, Search for Cain 1 → 6 (tree,
  real `bks ` / `bkd `, the five stones), Forgotten Tower 0 → 5 (tome,
  Countess, report), Sisters to the Slaughter 0 → 5 (sequence timer,
  Andariel, portal timer, Warriv).
- Gate (`sh tools/gate.sh`, after `tools/cloud-setup.sh`): every step
  passes except the two test steps, whose failures are all pre-existing
  (below).

## Rules that changed in existing code

| Where | Before | Now (spec) |
|---|---|---|
| Sequence functions | any call raised the `seq_id` record from 0 to 1 | §10.1 walk; at game entry chain 1 (state 1) stops it |
| Init | only chain 1 set active / state | chains 2–6: active 1, state 0 |
| Status broadcasts of chains 1–6 | 0x5D to every player | per-quest iterate (I1/J1/K1/L1/O1; M1 without the x.15 test) |
| Start / chat end / restore | one generic shape for chains 1–6 | each chain's own callback (A1Q3 and A1Q4 restore differently) |
| Reward messages 76 / 92 / 118 / 163 / 183 | any NPC; state 5 unconditionally | only the quest's NPC; state 5 (A1Q4: 6) only with x.13; 0x28 / refresh as specified |
| A1Q5 messages 140–145 | state 4 → 5 | only with 5.13 and +0x11A, then the sequence; list B → A |
| A1Q6 kill | gems + unhandled | credit, gems, O3–O6, portal timer, callback 10 added |
| Cow King | 4.10 for level-39 players, 8 `vps ` quality 2 | gated by the killer's 4.10 / 40.0 (26.0 classic); quality 0 |
| Stone-order 0x50 | `unhandled` | `50 0400` + five values (bytes 13–14: 0, OQ5) |
| §8.1 Tyrael | `unhandled` before 0x61 | `5D 17 02 00 0000` |
| Respec done | chain 30 optional | fatal (fault) when absent |
| `add_link` special | always `unhandled` | chain 4 → `0x00592F80`; chains 8 / 12 are `ret 4` stubs |
| Malus operate | `set_object_opened` | object modes, sound 19, base stat 12 |
| Imbue grant | flags only | also chain 3's record inactive unless 3.15 (takes the control) |

## Seams added (`QuestWorld`; `QuestRest` for the host)

| Seam | Spec | Provider in the wired host |
|---|---|---|
| `party_members` | `0x00554630`, party list +0x1D2C (OQ7) | `QuestRest` |
| `base_stat`, `object_by_guid`, `item_code` | `0x006253B0`, `0x00552F60` type 2, `0x00628590` | `EconomyQuests` (real stat lists, unit lists, item store) |
| `object_mode` / `set_object_mode` (replace `set_object_opened`) | object +0x10, `0x00624690` | `QuestRest` |
| `unit_position`, `room_contains`, `room_at`, `free_spot_at` | `0x00620870`, `0x00619730`, `0x00463740`, `0x00545340` | `QuestRest` (paths / DRLG) |
| `spawn_monster`, `or_unit_flags`, `monsters`, `remove_monster`, `drop_preset_monster` | `0x005B2F20`, unit +0xC4, `0x005537D0`, `0x005A7E60` + `0x005A7C20`, `0x00543140` | `QuestRest` (monsters) |
| `npc_chat_clients` | `0x00572DC0` / `0x00573180` | `QuestRest` (NPC) |
| `find_object_near`, `create_object`, `object_anim_length`, `schedule_object_event`, `open_quest_message` | §10.6 stones, `0x0056EDE0` + `0x0061AED0`, `0x00640E90`, `0x005417D0`, `0x005456A0` | `QuestRest` (objects) |
| `players_near` (doc only) | J3's room test | `QuestRest` |

Every other implementor (test fakes in d2-sim, d2-server, d2-client)
got stubs. `QuestControl::faults` records the fatal asserts the
callbacks reach (`QuestError::Fatal(addr)`) instead of aborting.

## Open questions / TODOs left in code

1. OQ11 (A1Q4 objects): the gibbet `0x00593290` (§9.5 class 26, so L4
   and L5 have no caller yet), the operate `0x00593480`, the init
   `0x005935E0`, the portal helper `0x00592D50`, who sets +0x6C/+0x70,
   the stone value source: reported through `unhandled` or left to the
   caller (`stone_operate` takes the value).
2. OQ12 (A1Q5 trap step `0x005954F0`): reported; +0x119 is never set, so
   the chest / victim event 7 keeps rescheduling every 10 frames after
   the kill, as the code reads without the trap.
3. OQ13 (`0x00538680` in the A1Q6 credit): reported.
4. `TODO(quests §10.6)`: tree operate's +0x38 (the dropped scroll's
   GUID): the drop seam returns no item (+0x38 is never read).
5. `TODO(quests §10.6 step 15)`: a beside-monster without a room.
6. `TODO(quests §10.4)`: "no monster region of level 8 → fatal" has no
   form in `den_region`. `TODO(quests §10.5 / §10.6)`: event 0 without a
   player (A1Q3, A1Q4).
7. Read literally, worth a trace: A1Q6 event 3 sets state 3 on every
   Catacombs entry, also from state 4 or 5 (`changed := state < 3, then
   state := 3`); A1Q4 tree operate sets state 4 before the drop and
   keeps it when the drop fails; A1Q6 O7 returns 1 only for a player in
   Catacombs 4 (read as: the first such player gets the portal).
8. A1Q3's K4 scratch (+0xA0) and A1Q2's uninitialized +4 are not
   modelled (never read outside).
9. Wired-host order: the Kashya mercenary (`0x00579180`) runs after the
   quest call, so its 0x50 follows the refresh's 0x27 / 0x29; 1.14d
   calls it before the refresh.
10. "every player F" is `QuestWorld::players()`; §10.1 names
    `unit-order.md` §2 r5 order (state-7 players skipped).
11. Not in scope (Acts II–V, §11): §6.2 barbarians `0x00588C50`, §8.2
    checks, the Act II–V §9.5 functions and `0x005BCFD0` stay reported.

## Pre-existing failures (not from this branch)

The same 55 tests fail on the merged base (`eb3a3e9`, round-2 changes
stashed) and with this branch (`cargo nextest run --no-fail-fast`, the
lists diffed equal): d2-sim 9 — `missiles::tests_bodies` (2),
`monsters::ai` (`implemented_matches_catalogue`,
`d2moo_only_act1_ais_are_stubs`), `skills` (5: function tables, table
check mutants, perturbation checks), spec TSVs merged ahead of their
implementations; d2-client 45 (bridge protocol version / dispatch table,
e2e, app frame loop, props). Clippy, fmt, depcheck, coverage, spec
index, doc-tests and `test rest (no client)` pass.

## Local run queue (add to `docs/HANDOFF.md` §5)

1. Trace (`tools/trace-recorder`, 1.14d, single player, new character)
   through Den of Evil: Akara 64, leave town, enter level 8, clear it,
   wait 9 updater ticks, Akara 76. Compare the 0x5D / 0x28 / 0x89 / 0x27
   / 0x29 sequence and slot 1 / slot 41 with
   `den_of_evil_through_every_state` (`5d 01 00 01`, `5d 01 00 02`, `5d
   01 20 04 LLLL` per kill while ≤ 5 left, `89 00`, `5d 01 00 05`, slot 1
   = `0x2001`, slot 41 = `0x2002`).
2. Burial Grounds: Kashya 81, area 17, Blood Raven, timer 15 → `5d 02 00
   03`; Kashya 92: 0x28, 0x27/0x29 and the 0x50 order (item 9).
3. Tools of the Trade at level 7 and 8 (Malus: sound 19 / drop; Charsi
   163; the chat end's 0x5D status byte, expected 10 while 3.1 is set).
4. Search for Cain: tree, Akara 112, the stone-order 0x50 (bytes 13–14,
   OQ5), the five stones (`89 01`), the Tristram portal; travel to Act II
   without freeing Cain (status 5, 4.14); Akara 118.
5. Forgotten Tower: tome before the tower, Countess kill in Tower Cellar
   5 (sound 37, timer 7 → status 13), the trap spawns (OQ12), a town
   report 140–145.
6. Sisters to the Slaughter: Andariel kill, the portal at counter 10
   (tick T + 18), status 3 at T + 22; Warriv 183; the act change (`61
   02`, chain 6 state 5).
