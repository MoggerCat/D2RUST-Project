# Handoff: AI bodies Smith, Griswold, GoodNpcRanged, NpcOutOfTown — `claude/impl-ai-act1`

Cloud implementation session, 2026-10-06, task class: implementation
from a clear spec, medium (METHODS M14). Base: `claude/tender-meitner-mphas3`
at `b435f5a` (spec commit `9f64a3a`, "1.14d bodies for Smith, Griswold,
GoodNpcRanged, NpcOutOfTown"). Repo only, no game files (M09). Inputs:
`specs/monsters/ai.md` §8, §9.9 (interaction handler), §9.30–§9.32, §10;
`specs/monsters/ai-functions.tsv`.

## 1. Result (unverified, M02)

| AI | Index | Address | Where | Tests (`// Covers:`) |
|---|---|---|---|---|
| Smith | 98 | `0x005E3890` | `monsters/ai/functions.rs` `smith` | `ai::tests::npc::smith_attacks_or_walks_by_life` (§9.30 text, r1, r2) |
| Griswold | 90 | `0x005E5AC0` | `functions.rs` `griswold` | `griswold_vectors` (§9.30 l2 r1, r2; the four seed vectors) |
| GoodNpcRanged | 60, special state 5 | `0x005E7AC0` | `monsters/ai/npc.rs` `good_npc_ranged` | `good_npc_ranged_not_neutral_idles_5`, `…_in_town_wanders_or_idles`, `…_out_of_town` (§9.31 text, r1–r3) |
| NpcOutOfTown | 31 | `0x005E7880` | `npc.rs` `npc_out_of_town` (+ portal setup `0x005E77A0`, "leave") | `npc_out_of_town_portal_setup`, `…_gates`, `…_walks_to_the_portal_point`, `…_goes_through_the_portal` (§9.32 text, r1–r8) |
| Npc interaction handler (called by NpcOutOfTown step 5) | – | `0x005E68F0`, home step `0x005E6800`, home check `0x005E6860` | `npc.rs` `npc_interaction_think`, `npc_home` | `interaction_needs_a_block_and_a_player`, `…_talking_or_busy`, `…_greets_close_or_far_players`, `…_home_check_and_walk_around` (§9.9 r1, l2 r1–r7, edge case 11) |

The Npc think itself (32) is still a stub (another session's task); its
step 1 and step 3 are now `npc::npc_home` / `npc::npc_interaction_think`
and can be called from it.

Dispatch: the four addresses are in `functions::IMPLEMENTED` (21 entries)
and `run_function`; GoodNpcRanged is reached both from AI index 60 and
from special state 5 (same address, `SPECIAL_TABLE[5]`).

**Wired host**: `wiring::action::tests::ai::good_npc_ranged_takes_ai_turns`
— a monster with AI 60 on the real unit dispatch and timer queue takes
three turns: idle 10 (frame 1 → 11), idle 10 (11 → 21), then wander 5
(exactly the §7.2 wander draws from the unit seed; the walk request goes
through the real monster mode set, and no think is left, since the next
comes from the walk's end). The walk start itself is the path spec's
(pending), so the anim mode stays neutral in the fixture.

## 2. New helpers and seams

- `monsters::ai` (`tactics.rs`, §8 / §6 / §7.2): `find_command`
  (`0x0058EEF0`), `get_or_create_command` (`0x0058EFA0`), `command_mut`,
  `path_distance` (`0x005DC5C0`), `walk_to_point` (`0x005DED90`),
  `walk_step0` (`0x005DEF30`; for the Npc session's command 4 / 7).
- `AiUnits`: `has_interaction_block` (monster data +0x30),
  `in_interaction_list` (`0x00572DE0`), `set_life` (`0x00627260`, stat 6).
- New trait `AiQuests` (part of `AiHost`), enum `PortalNpc { Cain, Drehya }`:
  `portal_setup`, `spawn_town_portal`, `spawn_outside_portal`,
  `portal_coords`, `drehya_update` (`0x0058AA10`), `drehya_wait`
  (`0x0058A9F0`). Provider: the quest session (`world/quests.md`).
- Wired host (`wiring/action/ai.rs`): `set_life` writes the life base
  (`set_base(HITPOINTS)`); everything else forwards to new `Pending`
  defaults: no interaction block, not in the list, portal setup succeeds
  (nothing to report), no outside portal, no coordinates, no Drehya gate.

## 3. Catalogue mirror (M05, M08)

The spec commit flipped 20 rows to `spec'd-here` (4, 5, 8, 10, 11, 15, 20,
26, 28, 30, 31, 32, 33, 37, 43, 59, 60, 64, 90, 98), which broke two
tests already on the base (`implemented_matches_catalogue`,
`rules::d2moo_only_act1_ais_are_stubs`). Now:

- `table::SPECD_HERE` (37 indices) mirrors the `status` column;
  `tests::specd_here_matches_tsv` checks it row by row, and
  `specd_here_check_catches_perturbations` shows a changed status (row
  98), a dropped index (60) and an extra index (147) each reported at
  exactly that index. `AI_TABLE` (columns 1–6) needed no change: only
  `summary` and `status` changed in the TSV.
- `implemented_matches_catalogue`: IMPLEMENTED ∪ `NOT_IMPLEMENTED_YET`
  = `SPECD_HERE` exactly, disjoint. `NOT_IMPLEMENTED_YET` = [4, 5, 8, 10,
  11, 15, 20, 26, 28, 30, 32, 33, 37, 43, 59, 64]: **a session that lands
  one of these bodies removes its index there** (and adds it to
  `IMPLEMENTED`).
- `rules::d2moo_only_act1_ais_are_stubs` → `no_act1_ai_is_d2moo_only`
  (§9.14 now says "None left"): the six rows are `spec'd-here` with a
  summary, and each one not implemented yet is still a logged stub.

M08 by hand, each reverted and each failing exactly the named test:
Smith `>> 1` → `>> 2` (`smith_…`), Griswold 50 → 52 (`griswold_vectors`),
portal phase `< 8` → `<= 8` (`…_goes_through_the_portal`), interaction
param 0 `> 36` → `> 40` (`interaction_talking_or_busy`), roguehire class
check off (`good_npc_ranged_out_of_town`), the Drehya-only walk gate
applied to Cain (`…_walks_to_the_portal_point`, after adding the Cain
case), step-3 chance 20 → 4 (the wired-host test).

## 4. Open questions (spec, not guessed; `TODO(spec: …)` in the code)

- **AI1** `ai.md` §9.32: drehyaiced is class 527 in the text, `528
  drehyaiced` in the catalogue's `monstats_rows` (index 31). The code uses
  527 (the rule text). Settle on the live monstats.txt.
- **AI2** §9.31 step 2: "If S and E < 20: roll < 30 → attack; end. Else
  roll < 30 → circle 4 at S, else idle 10; end." Read with the catalogue
  summary ("under 20: 30% A1, else 30% circle 4, else idle 10; else 20%
  wander 5, else idle 10"): the Else is the first roll's, and no S under
  20 goes to step 3. The other reading (Else of "S and E < 20", step 3
  only in town) differs in draws and outcome out of town; confirm in
  `0x005E7AC0`.
- **AI3** §9.31: `0x0061AB00` for a unit without a room; read as "out of
  town".
- **AI4** §9.32 step 1: after a failed portal setup, "leave" is followed
  by params 3, 4 := 1, 0 and idle 1 as written (the order of the text);
  confirm that the setup does not end at "leave".
- **AI5** §9.32: a class other than cain1 / drehyaiced with this AI does
  nothing here (the quest functions of any other class are not stated).
- **AI6** §8: where `0x0058EFA0` inserts a created command (read as
  `0x0058EF40`: before the current one, becoming current) and where
  `0x0058EEF0` starts its search when there is no current command (read
  as from the first).
- **AI7** §7.2: the step count of the coordinate walk `0x005DED90` (1 is
  used, as for the unit walks).

## 5. Local checks to queue (M02)

None runnable from game files alone: these bodies have no recorded
instance. When a town recording exists (Tristram Cain after the rescue,
a rogue hireling out of town), compare the type-2 delays (1 / 40 / 20
for NpcOutOfTown; 5 / 10 for GoodNpcRanged) and the unit-seed draws per
think with the rules above; settle AI1 with `data-tool` on monstats.txt.

## 6. Gate

`sh tools/gate.sh` (see the commit message for the summary line).
