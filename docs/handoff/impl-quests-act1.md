# Handoff: Act I quests A1Q1–A1Q3 callback by callback — `claude/impl-quests-act1`

Cloud implementation session, 2026-10-06, from `b435f5a` (main +
`claude/spec-quests-act1` merged at `6b0be8b`). Spec input: the
`specs/world/quests.md` diff `6b0be8b^1..6b0be8b` (§6.3 iterate form,
§10.1 shorthands and sequence functions, §10.3 respec record, §10.4
A1Q1, §10.5 A1Q2 and A1Q3, five new test vectors, open question 7).
The coordinator folds this file into `docs/HANDOFF.md` / `docs/PLAN.md`.

## State

**Implemented, unverified** (no trace of these quests has run against
this code; the spec rules come from the 1.14d disassembly).

- A1Q1 (chain 1), A1Q2 (chain 2) and A1Q3 (chain 3) now run every
  registered callback from §10.4 / §10.5, plus the active functions of
  chains 1–3, A1Q3's status function, the A1Q1 / A1Q2 timers, the
  Malus object's init and operate, the A1Q3 reset and the imbue grant.
- The sequence functions of chains 1–6 follow the §10.1 walk (own step,
  pass state, `seq_id`); the unlock order from Den of Evil is 1 → 2 → 4
  → 3 → 6.
- Tests: `cargo test -p d2-sim --lib world::quests` (18 new in
  `world/quests/act1_tests.rs`, from every new §10.1 / §10.4 test vector
  and the §10.4 / §10.5 rules, with `// Covers:`), and three wired-host
  sequences in `d2-server` `adapters/handlers/world/tests/quests_act1.rs`
  (`WiredWorld`: real `QuestControl` on `EconomyQuests`, real units and
  stat lists, messages through the host frame): Den of Evil 1 → 5,
  Burial Grounds 0 → 5 (opened because the first player had Den of Evil
  done), Tools of the Trade 0 → 5 + imbue (opened by the 1 → 2 → 4 → 3
  walk), with a real `hdm ` item for message 163.
- Gate (`sh tools/gate.sh`, after `tools/cloud-setup.sh`): every step
  passes except the two test steps, whose failures are all pre-existing
  (below); `test rest (no client)` (d2-server, incl. the wired-host
  sequences) passes.

## Rules that changed in existing code

| Where | Before | Now (spec) |
|---|---|---|
| Sequence functions | any call raised the `seq_id` record from 0 to 1 | §10.1 walk; at game entry chain 1 (state 1) stops the walk, so chain 2 stays 0 until Den of Evil is done or switched off |
| Init | only chain 1 set active / state | chains 2 and 3: active 1, state 0 |
| Status broadcasts of chains 1–3 | 0x5D to every player | iterate I1/J1/K1: only players without x.0 and x.15, or with x.13 / x.14 |
| A1Q1 event 3 | bits only for players who started, on the moving player | I2 for every player without 1.0 / 1.1; leaving town with status 0 gives bit 4 (bug kept); status 2 / 1 broadcasts; GUID removal |
| A1Q1 event 8 | party / log / sound were `unhandled` | I3 (party), I4 (14, `5D 01 00 0C`, 0x28), I5 (sound 35); few-left branch clears callback 2 |
| A1Q1 msg 76 / A1Q2 msg 92 / A1Q3 msg 163 | any NPC; state 5 unconditionally | only the quest's NPC class; state 5 only with x.13 (A1Q3: at state 4); 0x28 / refresh as specified |
| A1Q2 event 8 | `players_near` + sound, no timer | J3 (victim by GUID, fatal if missing), J7, J5, J6, timer 15, game 2.13; callback 8 kept |
| Malus operate | `set_object_opened`, unhandled sound | object mode 2, sound event 19 at level < 8 or not-intro 0, level = **base** stat 12 |
| Imbue grant | flags only | also chain 3's record inactive unless 3.15 (`act1::imbue_granted` now takes the control) |

## Seams added (`QuestWorld`; `QuestRest` for the host)

| Seam | Spec | Provider in the wired host |
|---|---|---|
| `party_members` | `0x00554630`, party list game +0x1D2C (no spec; OQ7) | `QuestRest` (test rest stages it) |
| `base_stat` | `0x006253B0` | `EconomyQuests`: real stat lists |
| `object_mode` / `set_object_mode` (replace `set_object_opened`) | object +0x10, `0x00624690` | `QuestRest` (objects spec) |
| `object_by_guid` | `0x00552F60` type 2 | `EconomyQuests`: real unit lists |
| `players_near` (doc only) | J3's room test (`drlg/rooms.md` §10.4) | `QuestRest` |

`QuestControl::faults` records the fatal asserts the callbacks reach
(`QuestError::Fatal(addr)`, e.g. J3's missing victim) instead of
aborting.

## Open questions / TODOs left in code

1. `TODO(quests §10.8)`: chain 6's sequence own step makes a timer with
   `0x00596580` (period 20); its callback is not specified, so no timer
   is made and `unhandled(6, 0x596580)` is reported.
2. `TODO(quests §10.4)`: "no monster region of level 8 → fatal" has no
   form in `QuestWorld::den_region`.
3. `TODO(quests §10.5)`: A1Q3 event 0 without a player is not described
   (NPC chat always has one; the callback returns).
4. A1Q3's K4 scratch byte (extra +0xA0) is not stored (only K4 reads
   it); A1Q2's extra +4 starts at 0 (the original leaves it
   uninitialized; never read by chain 2).
5. Order in the wired host: the mercenary (`0x00579180`) runs after the
   quest call (existing `WiredWorld::quests` design), so its 0x50 follows
   the refresh's 0x27 / 0x29, while the 1.14d callback calls it before
   the refresh. Needs a trace to decide whether bytes differ.
6. "every player F" is `QuestWorld::players()` (documented as
   `unit-order.md` §7 order); §10.1 names `unit-order.md` §2 r5 (hash
   order, state-7 players skipped). Same in single player; the provider
   should be checked against §2 r5.
7. Unchanged: chains 4–6 still use the generic `start` / `chat_end` /
   `restore` and `set_status_all` (send to all) until §10.6–§10.8 are
   written callback by callback.

## Pre-existing failures (not from this branch)

On the base commit `b435f5a` (checked with the working tree stashed),
`cargo test -p d2-sim --lib` fails in `monsters::ai::tests::*`
(`implemented_matches_catalogue`, `d2moo_only_act1_ais_are_stubs`) and
`skills::*` (`function_tables_match_tsv`, the table-check mutants,
`bodies_check_reports_perturbations`): spec TSVs merged ahead of their
implementations. Untouched here. 7 d2-sim failures in total with this
branch (`--no-fail-fast`), all in `monsters::ai` and `skills`.

`cargo nextest run -p d2-client --no-fail-fast`: 45 failures (bridge
protocol version / dispatch table, e2e, app frame loop, props), the same
45 on the base commit with this branch stashed.

## Local run queue (add to `docs/HANDOFF.md` §5)

1. Record a trace (`tools/trace-recorder`, 1.14d, single player, new
   character) through Den of Evil: Akara 64, leave town, enter level 8,
   clear it, wait 9 updater ticks, Akara 76. Compare the S→C 0x5D / 0x28
   / 0x89 / 0x27 / 0x29 sequence and the slot 1 / slot 41 words with
   `den_of_evil_through_every_state` (expected: `5d 01 00 01`, `5d 01 00
   02`, `5d 01 20 04 LLLL` per kill while ≤ 5 left, `89 00`, `5d 01 00
   05`, slot 1 = `0x2001`, slot 41 = `0x2002`).
2. Same for the Burial Grounds (Kashya 81, area 17, Blood Raven, timer
   15 → `5d 02 00 03`, Kashya 92: 0x28 then 0x27/0x29 vs the 0x50 order,
   item 5 above).
3. Tools of the Trade at character level 7 and 8 (operate the Malus:
   sound 19 / drop; Charsi 163; the chat end's 0x5D status byte, expected
   10 while 3.1 is set).
