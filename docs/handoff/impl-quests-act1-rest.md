# Handoff: the Act I quest remainders — `claude/impl-quests-act1-rest`

Cloud implementation session, 2026-10-06, from `claude/specs-staging`
(`5844674`). Implements `specs/world/quests-act1-rest.md` (§1–§8) and the
`specs/world/quests.md` changes since `main` (§9.5 class 26, §10.6 step
3.3 marker object, step 15 21 points, the stone operate's cairnstones
missile, §10.7 trap step, §10.8 credit progression and event 3). Parallel
sessions own Acts II, III and IV–V; this branch touches only the Act I
modules plus the shared seams listed below.

The coordinator folds this file into `docs/HANDOFF.md` / `docs/PLAN.md`.

## State

**Implemented, unverified** (no trace of a Cain rescue, a Cairn stone
0x50, a Countess kill or a Catacombs entry after the kill has run against
this code; every rule is from the 1.14d disassembly read in the spec).

| Spec | Code (`d2-sim/src/world/quests/…`) |
|---|---|
| §1.1 gibbet operate `0x00593480` | `act1/q4.rs` `gibbet_operate` |
| §1.2 gibbet quest function `0x00593290` (object event 7, class 26) | `q4::gibbet_event`, called by `object_event` (§9.5); L4 `rescued`, member step `party_rescued`, L5 `completed_now` |
| §2.1 stone value = the object's class | `q4::stone_operate` doc (the caller passes the class) |
| §2.2 stone init `0x005935E0` | `q4::stone_init` |
| §2.3 Tristram-portal timer `0x00592D50` | `TimerFn::TristramPortal`, `q4::tristram_portal_timer` |
| §3 town-Cain marker init `0x005940E0`, Cain leaves Tristram `0x005944F0` | `q4::marker_init`, `q4::cain_leaves_tristram` |
| §4 chest trap step `0x005954F0` | `act1/q5.rs` `trap` (Countess kill, chest init, chest event 7) |
| §5 character progression `0x00538680` | `quests.rs` `progression`, `raise_progression`; A1Q6 credit |
| §6 party list | through the existing `QuestWorld::party_members` |
| §7 0x50 bytes 13–14 | unchanged (0); doc only |
| §8 items 1–9 | item 2 tree order (already literal), 3 marker object anchor + 21 points + fatal without a room, 4 comment (not reachable), 5 event 0 without a player fatal (A1Q3 `0x005916B3`, A1Q4 `0x005925BB`), 6 A1Q6 event 3 keeps states 3–5, 7 O7 comment, 8 / 9 see below |
| §8 item 8 Kashya's order | `wiring::economy::QuestDeferred`: after a queued mercenary reward, the quest call's later sends queue behind it; `Desk::quest_message` and `WiredWorld::quests` run the queue in order (0x28, 0x50 + hireling, 0x27, 0x29) |
| §8 item 9 "every player" order | `wiring::economy::quest_players`: the player list's buckets 0–127, state 7 skipped (`EconomyQuests::players`; `QuestRest::players` is no longer called) |
| `quests.md` §10.6 stone operate | the fifth stone creates missile 288 (owner the player, skill 0, level 1) through `create_missile`, then `refresh_room` (was `create_object`) |

## Seams added

`QuestWorld` (implemented by `EconomyQuests`, the d2-sim fakes and the
d2-server fake) and `QuestRest` (default bodies that report the 1.14d
address through `unhandled(0xFF, …)` and return none, so no host
implementor changed):

| Seam | 1.14d |
|---|---|
| `spawn_monster_flags(room, x, y, class, mode, spread, flags)` | `0x005B2F20` with spawn flags |
| `open_portal(owner, room, x, y, level, class, exact)` | `0x0056D130` |
| `create_missile(owner, skill, level, class, x, y)` | `0x0056EDE0` (§4.1, distance test in the host) |
| `set_missile_target(missile, a, b)` | `0x0064A710` / `0x0064A760` |
| `refresh_room(unit)` | `0x0061AED0(room, 0)` |
| `spawn_object(room, x, y, class, mode)` | `0x00555230` |
| `client_save_flags` / `set_client_save_flags` | client +0x0A via `0x005531C0` |

`QuestWorld::create_object` is no longer called by quest code (the stone
operate's 288 is a missile); it stays in the trait for now.

## Rules that changed in existing code

| Where | Before | Now (spec) |
|---|---|---|
| §9.5 class 26 | `unhandled(4, 0x593290)` | the gibbet quest function |
| A1Q4 event 3 b = 1 | town Cain beside a **monster** +0x6C | beside the marker **object** (type 2) |
| Town Cain spawn point search | i = 0..19 (20 points) | i = 0..=20 (21 points) |
| Stone operate, fifth stone | object 288 | missile 288 owner the player, skill 0, level 1, room refreshed |
| A1Q5 trap step | `unhandled(5, 0x5954f0)` | §4: one trap-firebolt, a towerchestspawner per chest; +0x119 set, which stops the event-7 rescheduling |
| A1Q6 credit | `unhandled(6, 0x538680)` | progression raised on the client's save flags |
| A1Q6 event 3 in Catacombs | state := 3 always | only from states 0–2 |
| A1Q3 / A1Q4 event 0 without a player | silent return | `QuestError::Fatal` |
| `Extra4` | `beside_guid`, `beside_known` | `marker_guid`, `marker_known` plus the new §1 fields |

## Tests

- `world/quests/act1_rest_q4_tests.rs`: gibbet operate / event 7, stone
  init and portal timer, marker init, Cain leaves Tristram, party step,
  the 0x50 vector.
- `world/quests/act1_rest_misc_tests.rs` (11): the trap vectors (first
  try, retry at chest + (5, 5), missing chests, reschedule), the four
  progression vectors, §8 items 2, 5, 6, 7 (with a failing-drop /
  failing-portal world).
- `wiring/interaction/tests/quest_npc.rs`
  `kashya_reward_messages_precede_the_text_refresh`;
  `wiring/economy/tests/quest_players.rs`; existing tests corrected to
  the spec (A1Q5 / A1Q6 logs, stone 288 missile, town Cain at the marker
  object, the d2-server Burial Grounds and Forgotten Tower sequences).

## Open questions / readings (none guessed in code)

1. §4 step 2.3.2: the retry's room (`0x00463740` from C's room) is not
   null-tested by the original; d2rs skips the spawn when no room is
   found. What `0x005B2F20` does with a null room is not specified.
2. An object or marker without a room (gibbet event, Cain leaves
   Tristram, chest in the trap step) is reported as `QuestError::Fatal`
   with the function's address; 1.14d reads the static path without a
   test (invariant, never null on a 1.14d path).
3. §1 L4 (`0x00593130`): the party step is read as inside the
   "neither 4.0 nor 4.1 and in Tristram" test (`quests.md` §10.6 table).
4. §5: a player without a client (`client_save_flags` = none) gets no
   progression; 1.14d always has one.
5. +0x38 (the scroll's GUID, §8 item 1) is not kept: no reader, and the
   drop seam returns no item.
6. Chain 37's unhandled `0x0058F870` still runs before Kashya's
   deferred reward (as before).
7. No host provides the new seams yet (the wired host reports them); the
   gibbet / stone / marker init and operate functions have no host
   dispatcher yet (objects spec not written, `quests-act1-rest.md` open
   question 2).

## Local run queue (add to `docs/HANDOFF.md` §5)

1. Trace a Cain rescue (1.14d, single player): operate the gibbet; expect
   object mode 1, slot 4 = `0x2002`, 0x28 at once; 17 frames later the
   Cain spawn (class 146 at gibbet + (3, 3)), `5d 04 00 06 0000`.
2. Trace a Countess kill with a listed chest: one monster 326 (mode 12)
   at the death position, one missile 332 per chest.
3. Trace entering Catacombs 1 after Andariel's kill (state 4): no 0x5D,
   state kept (§8 item 6).
4. Read the save header after Andariel on Normal: progression bits 8–12
   = 1 (classic) / 1 (expansion) (§5).
