# Handoff: every `spec'd-here` AI body (Act 1 and the rest of §9) — `claude/impl-ai-act1`

Cloud implementation session, 2026-10-06, task class: implementation
from a clear spec, medium (METHODS M14). Base: `claude/tender-meitner-mphas3`
at `b435f5a` (spec commits `346f245` … `9f64a3a`, "1.14d bodies for …").
Repo only, no game files (M09). Inputs: `specs/monsters/ai.md` §3.3, §6–§9,
Test vectors, Edge cases; `specs/monsters/ai-functions.tsv`.

Task: the four bodies of `9f64a3a` (Smith, Griswold, GoodNpcRanged,
NpcOutOfTown). The coordinator then widened it (message during the
session): on the base, `implemented_matches_catalogue` and
`d2moo_only_act1_ais_are_stubs` failed because the catalogue lists 20
more `spec'd-here` rows than the code had; implement every AI the spec
gives a body for. Done: **all 37 `spec'd-here` rows have bodies**.

## 1. Result (unverified, M02)

| Index | AI | Where | Tests (`ai::tests::…`, all with `// Covers:`) |
|---|---|---|---|
| 4 | Bighead | `monsters/ai/bodies.rs` | `bodies::bighead_vectors` |
| 5 | BloodHawk | `bodies.rs` | `bodies::blood_hawk_vectors` |
| 8 | SandRaider | `bodies.rs` | `bodies::sand_raider_charges_glows_and_hits`, `…_help_circle_and_rest` |
| 10 | CorruptRogue | `bodies.rs` | `bodies::corrupt_rogue_vectors` |
| 11 | Baboon | `bodies.rs` | `bodies::baboon_fights_and_starts_regenerating`, `baboon_regeneration_countdown` |
| 15 | SandMaggot + alternate `0x005F1750` | `bodies.rs` | `bodies::sand_maggot_*` (3) |
| 20 | Scarab | `bodies.rs` | `bodies::scarab_vectors`, `scarab_commands_and_circling` |
| 26 | Arach | `bodies.rs` | `bodies::arach_vectors`, `arach_retreat_and_combat` |
| 28 | Vampire | `bodies.rs` | `bodies::vampire_vectors`, `vampire_bolts_upgrades_and_flight` |
| 30 | Fetish | `bodies.rs` | `bodies::fetish_vectors`, `fetish_rhythm_and_commands` |
| 31 | NpcOutOfTown | `monsters/ai/npc.rs` | `npc::npc_out_of_town_*` (4) |
| 32 | Npc (home, class cases, interaction, commands, map AI) | `npc.rs` | `npc::interaction_*` (4), `bodies::npc_map_ai_vector`, `npc_commands_walk_and_wander`, `npc_command_7_mode_actions`, `npc_class_cases` |
| 33 | HellMeteor | `bodies.rs` | `bodies::hell_meteor_vectors` |
| 37 | SkeletonBow | `bodies.rs` | `bodies::skeleton_bow_vectors` |
| 43 | FoulCrowNest + init `0x005F6630` | `bodies.rs` | `bodies::foul_crow_nest_vectors`, `install_sets_think_and_alternate` |
| 59 | BloodRaven + init `0x005E6300` | `bodies.rs` | `bodies::blood_raven_raise_vector`, `blood_raven_leash_and_moves` |
| 60 | GoodNpcRanged (also special state 5) | `npc.rs` | `npc::good_npc_ranged_*` (3) |
| 64 | SkeletonMage | `bodies.rs` | `bodies::skeleton_mage_vectors` |
| 90 | Griswold | `functions.rs` | `npc::griswold_vectors` |
| 98 | Smith | `functions.rs` | `npc::smith_attacks_or_walks_by_life` |

Every row of the spec's synthetic Test-vector table for these AIs is a
test (CorruptRogue ×2, SkeletonBow ×2, FoulCrowNest, BloodRaven,
SkeletonMage, Arach, Fetish, Vampire, Bighead, BloodHawk, HellMeteor,
Scarab, Npc map AI), plus rule tests for the branches the table does
not reach. `bodies::boundaries` pins six off-by-ones.

Init functions now run (`functions::run_init`, `INIT_IMPLEMENTED`):
`0x005F6630` (FoulCrowNest, also Sarcophagus 45 and MinionSpawner 121,
whose thinks stay stubs) and `0x005E6300` (BloodRaven). Other inits stay
logged stubs.

**Wired host**: `wiring::action::tests::ai::good_npc_ranged_takes_ai_turns`
— a monster with AI 60 on the real unit dispatch and timer queue takes
three turns: idle 10 (1 → 11), idle 10 (11 → 21), then wander 5 (exactly
the §7.2 wander draws; the walk request goes through the real monster
mode set, and no think is left, since the next comes from the walk's
end). The walk start itself is the path spec's (pending), so the anim
mode stays neutral in the fixture.

## 2. Code changes outside the bodies

- **`skill_modes` is `[u8; 4]`** (was 3): `Sk4mode` (+0x183) is read by
  Vampire (§9.22). `ai::skill_modes` reads +0x180..+0x183;
  `ActionTables::skill_modes: Vec<[u8; 4]>`. Mechanical `[[0; 3]]` →
  `[[0; 4]]` in the fixtures, including three `d2-client` tests
  (`e2e_full_loop`, `e2e_single_player`, `prop_worldsim`) and
  `monsters::mutant_tests::skill_modes_reads_bytes_0x180_to_0x183`.
  `Ctx::skill(p, 4)` returns `Skill4`.
- `AiControl::map_ai: Option<Vec<MapNode>>` (control +0x38; who builds it
  is OQ8, so nothing fills it yet); `AiStore::npc_walk_counter` = G
  (`0x0088CADC`, OQ12: kept per AI store, see §4).
- Helpers (`tactics.rs`): `find_command` (`0x0058EEF0`),
  `get_or_create_command` (`0x0058EFA0`), `command_mut`, `path_distance`
  (`0x005DC5C0`), `half_size_distance` (`0x005DC480`), `walk_to_point`
  (`0x005DED90`), `walk_step0` (`0x005DEF30`), `run_to_point`
  (`0x005DEDE0`), `run_near` (`0x005DF680`).
- Seams (`seams.rs`), each forwarded by the wired `View` and given a
  `Pending` default where no provider exists:
  - `AiUnits`: `has_interaction_block`, `in_interaction_list`
    (`0x00572DE0`), `set_life`, `stat` / `set_stat` (real: unit getter /
    set), `set_unit_flag` (real: unit +0xC4), `set_state` (real: the
    state toggle), `path_target` (`0x00553540`, pending).
  - `AiModes`: `start_overlay` (→ `Pending::overlay`), `set_facing`
    (`0x00648820`, pending).
  - `AiWorld`: `footprint_ok` (`0x005FD350`, pending: false).
  - `AiTargets`: `nearest_evil_monster` (SandRaider's scan 1, pending:
    none).
  - New `AiQuests` (+ `PortalNpc`): the NpcOutOfTown portal calls and the
    Npc class-case calls (jerhyn ×3, alkor ×2, ormus ×2, cain5 ×2,
    drehya ×1); provider: the quest session; pending defaults "no quest
    state" (portal setup succeeds, nothing found or brought).

## 3. Catalogue mirror and the two base failures (M05, M08)

- `table::SPECD_HERE` (37 indices) mirrors the `status` column;
  `tests::specd_here_matches_tsv` checks it row by row and
  `specd_here_check_catches_perturbations` shows a changed status (row
  98), a dropped index (60) and an extra index (147) each reported at
  exactly that index. `AI_TABLE` (columns 1–6) needed no change.
- `implemented_matches_catalogue`: unchanged in meaning (IMPLEMENTED =
  the `spec'd-here` rows exactly); it now reads the mirror.
- `rules::d2moo_only_act1_ais_are_stubs` → `no_act1_ai_is_d2moo_only`:
  its assumption is wrong per **§9.14** ("None left: the Act 1 AIs that
  were D2MOO-only are read in 1.14d in §9.15–§9.18 … and §9.23–§9.24"):
  the six rows are `spec'd-here` with a summary, implemented, and their
  think no longer logs a stub.
- `install_sets_think_and_alternate` and
  `mutant_tests::install_init_only_with_a_function_and_both_records`
  assumed every init is a logged stub; per §9.17 / §9.18 two inits have
  bodies. The first now checks FoulCrowNest's init effect (param 0 :=
  frame) and BoneWall's (84) stub; the second picks the first init
  without a body (`INIT_IMPLEMENTED`). `stub_ai_logged` uses Mephisto
  (50, unread) instead of Npc.

M08 by hand, each reverted and each failing exactly its test: first
batch (Smith shift, Griswold 50, portal phase, interaction param 0 > 36,
roguehire, Drehya gate, wired step-3 chance); second batch over the new
bodies: CorruptRogue L, SkeletonBow E < 20, nest idle base, Fetish reset,
run bonus, SandRaider rest k, Baboon bonus /8, SandRaider aidel + 1, Npc
home +9, charsi facing; six survivors (BloodRaven raise cap and run-near
floor, Arach wrap, Scarab > 10, G & 3, map-AI 66) each got a boundary
test in `bodies::boundaries` and then failed it.

## 4. Open questions (spec, not guessed; `TODO(spec: …)` in the code)

- **AI1** §9.32: drehyaiced is class 527 in the text, `528 drehyaiced` in
  the catalogue's `monstats_rows` (index 31). The code uses 527.
- **AI2** §9.31 step 2: the "Else" is read as the first 30 % test's
  (catalogue summary: "under 20: 30% A1, else 30% circle 4, else idle
  10; else 20% wander 5"); without S under 20 the think goes to step 3.
  The other reading changes draws out of town; confirm in `0x005E7AC0`.
- **AI3** §9.31: `0x0061AB00` for a unit without a room (read: out of
  town).
- **AI4** §9.32 step 1: after a failed portal setup, "leave" is followed
  by params 3, 4 := 1, 0 and idle 1, as written.
- **AI5** §9.32: another class with AI 31 does nothing.
- **AI6** §8: where `0x0058EFA0` inserts a created command (read as
  `0x0058EF40`) and where `0x0058EEF0` starts with no current command.
- **AI7** §7.2: the step count of the coordinate walk / run
  (`0x005DED90`, `0x005DEDE0`); 1 is used.
- **AI8** OQ12: G is per AI store here (equal to 1.14d for the first
  game of a fresh process).
- **AI9** §9.26: SandRaider's states go through `0x00639DB0`; wired to
  the state toggle of `stat-lists.md` §9.2.
- **AI10** §9.28: SandMaggot states above 3 take the above-ground steps.
- Spec gaps for target 0 (target mode 1 always has a target): Fetish's
  "life of T", BloodRaven's h and raise point, SkeletonBow's walk in
  radius, run near target 0 (`TODO(spec gap)`).

## 5. Local checks to queue (M02)

No recorded instance of these AIs exists. When town / Act 1 recordings
with these classes exist (Tristram Cain, a rogue hireling out of town,
Blood Raven, a foul crow nest, Act 2 raiders / maggots), compare type-2
delays and unit-seed draws per think with the rules above; settle AI1
with `data-tool` on monstats.txt.

## 6. Gate

See the commit message. Two `skills::mutant_tests::table_check_mutants`
tests (`index_bounds`, `repeated_slot_names_kind`) fail on the base
`b435f5a` too (checked by checkout): `skills/functions.tsv` statuses
changed under them; they belong to the skill-bodies session and are not
touched here.
