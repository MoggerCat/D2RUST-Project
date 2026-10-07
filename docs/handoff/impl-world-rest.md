# Handoff: world rest (object providers, quest treasure, hireling loader) — `claude/impl-world-rest`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).

Cloud implementation session, 2026-10-07, implementation from specs
(medium, METHODS M14). Base: `claude/specs-staging-5` at `666e2f2`. Repo
only, synthetic tables, no game files (M09): every claim holds on this
branch, on synthetic data. Everything here is **wired, unverified** (M02).

Task (HANDOFF §1 row "3 not implemented", world part): object population,
the providers behind `ChestWorld` / `ShrineWorld` / `MiscWorld` and the
`QuestWorld` calls left on the rest (rows 3aj–3al), the hireling callers.

## 1. Landed

| Item | Where | What |
|---|---|---|
| Chest drop `D(Q)` (`treasure.md` §4, §7) | `d2-sim` `wiring/economy/chest_drop.rs` (`object_chest_drop`, `NoSpot`); `ActionHooks::object_drops: Option<Box<DeathDrops>>` | The object is the dropper (`U`): its unit seed draws, item level = area level of its level (§7 r3), act = its room's act; the operator is the recipient `R` (magic / gold find, party). Tier and area level read the **object tables' `levels`** (`ObjectTables::levels`; `ObjectView` now carries `tables`). Items are real item units in the game's item store, placed by the path provider's floor drop (`path-placement.md` §9) with the object's room as the search room (§7 r2); without the path field the free-spot search finds nothing (`NoSpot`: the walk draws, no item). `None` drop state: `chest_drop` answers none (as before). |
| `ChestWorld` on the action wiring | `wiring/action/objects.rs` | `chest_drop` (above), `unit_type` (unit record), `item_quality` (item store), `room_units` (room unit list order). |
| `ShrineWorld` on the action wiring | same | `stat` (`0x00625480`), `set_stat` (`0x00627260`), `add_base_stat` (`0x006272B0`), `max_life` / `max_mana` / `max_stamina`, `player_level` (stat 12). |
| `MiscWorld` on the action wiring | same | `vital_stat` (6/8/10 total; 7/9/11 through the max getters), `set_vital_stat`, `remove_state_list` (`0x006256B0` + free), `cure_states` (`0x00578C20`, `npc.md` §5 step 4: every state the unit has that is curable and has a list). Wells now heal on the real stat lists. |
| Quest chest treasure `0x00585B90(op, 4)` | `world/quests.rs` (`QuestWorld::object_treasure(object, operator, kind)` — **signature gained the operator**, the operate record's `R`); callers `act2/q2.rs`, `act2/q3.rs` (2), `act3/q2.rs` pass their player; `wiring/economy/quest_host.rs` `HostQuests::object_treasure` | On an object with object data and a game with `object_drops`: `object_chest_drop`, with the economy's **lent** item store, game seed and unique bits swapped into the hooks for the drop and taken back (inside a quest call they live in the `Economy`, not the hooks). Otherwise the rest's answer, as before. |
| `missile_range` | `HostQuests::missile_range` | `missiles.txt` `Range` (u16) of the action tables' row (`quests-act2.md` §8.7); outside the table → `None`. |
| `HirelingTables` production loader | `d2-server` `world_data::tables::{hireling_tables, hireling_tables_by}` | `hireling`, `pettype` row 7, `experience` from the fixed-up set; a missing / malformed table is a `WorldDataError::Table` naming it (M07). Nothing calls it yet: the only production host is the app (`d2-client`, out of scope here); the caller sets `InteractionState::hireling_tables`. |

Small shared edits: `wiring::economy::death::Spots` is `pub(super)`
(reused); `wiring::interaction::tests` is `pub(crate)` under `cfg(test)`
(its `Rest` reused by the action tests); `tests/death.rs` `drop_tables`
is `pub(super)`.

## 2. Tests (all `// Covers:`)

`d2-sim` `wiring::action::tests::objects` (13, 6 new):
`the_chest_drop_walks_on_the_object_seed_and_drops_into_its_room`
(object seed `roll(1)`, game seed two steps, spot (x+2, y+3) in the
object's room, ilvl = `MonLvl1`, plus the three chest seams),
`without_drop_state_or_room_the_chest_drop_is_none` (no draw),
`a_well_heals_life_on_the_players_stat_list` (10 → 60 of 100, charge
2 → 1, mode 1, refill at frame + 751; full player: nothing),
`refill_and_life_shrines_act_on_the_players_stat_list` (codes 1, 2),
`a_quest_chests_treasure_drops_through_the_lent_economy` (item in the
lent store, seed step in the economy's fields),
`host_quests_read_the_missile_range_from_the_action_tables`.
`d2-server` `world_data::tests::hireling_tables_load_from_the_three_tables`
(+ missing-table error, M08).

## 3. Gate (this branch, head after this note)

- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --exclude d2-client --all-targets -- -D warnings`: clean.
- `cargo nextest run --workspace --exclude d2-client --no-fail-fast`:
  4606 run, 4601 passed, **5 failed, all in files this branch does not
  touch**: `d2-sim monsters::ai::tests::{specd_here_matches_tsv,
  specd_here_check_catches_perturbations}` ("index 41: status spec'd-here,
  mirror false": `ai-functions.tsv` vs the AI bodies, another session's
  area) and `scenario-run::scenarios::{starters_parse_and_round_trip,
  every_starter_runs_twice_identically,
  comparator_finds_every_perturbed_record_of_a_real_run}` (a starter
  script does not parse, `tools/scenario-run/tests/scenarios.rs:36`).
- `py tools/coverage.py --check`: 8,893 claims, 0 errors.
- `py tools/spec_index.py --check`: clean.
- `d2-client` not built (known broken on the base; not edited).

## 4. Not done, with the reason

1. **Object population** (`objects.md` §15): no spec (`0x00552610`,
   `PopulateFn` 1–9, objgroup density, shrine / well limits). Gap G1.
2. **Object footprints** (`stamp_footprint`, `free_footprint`,
   `footprint_collides`): still `Pending`; the path primitives exist
   (`path::footprint`, `path::collision::box_value`, `ObjectShape`) but
   no spec maps `0x00623830` / `0x00620A70` / `0x0064D800` onto them.
   Gap G2.
3. **Key test** `0x0055F140` (`objects.md` §8.1 r2, fully stated): needs
   the inventory walk and removal of the host's inventory model
   (`WiredWorld::inventory`, items area), which `ActionHooks` does not
   hold; still `Pending::object_key_test`. Without it locked chests and
   doors never open for a non-assassin.
4. **Code drop / drop item code / quest gold** (`0x00585970`,
   `0x00559A30`): no items-spec body. Gap G3.
5. **Trap monsters, monster spawns, free spot 0x3F11, range test,
   inside-room test, trap damage, player skill start** (§8.2, §8.3):
   owner specs silent on the parts listed in G4; the monster spawns are
   the monsters wiring's (`impl-pc1-final` / `impl-pc2-fixes` areas).
6. **Shrine missiles, hovers, timed states, gems, portal shrine, unique
   shrine, units in range**: G5, G6; the timed-state helper and gems are
   skills / items providers not reachable from the object view.
7. **Quest-chest gate** `0x00545850`: G7. **The rest of the `QuestWorld`
   calls on the rest** (`wire-world-staging.md` §3 item 6): unchanged
   except `object_treasure` and `missile_range` (no spec body or no
   reachable provider for the others).
8. **Hireling callers**: death is wired (`impl-d2s-load-hirelings`,
   `WiredWorld::pet_deaths`). C→S 0x61 swap (G8: item copy
   `0x0055A2A0`), save restore (G8: unit creation place, §10 r3), classic
   act change (G8: `0x0053ACC0` has no owner spec) remain without callers.
9. **Unique bits** (not a spec gap): `ActionHooks::object_drops` keeps
   its own `DeathDrops::fields.uniques`, apart from `WiredWorld::uniques`
   (and from a monster-drop `DeathDrops`); only the quest-treasure path
   uses the economy's bits. A host that sets `object_drops` should share
   one unique-bit store (fold `DeathDrops` into the hooks with the host's
   bits) — follow-up.
10. Nothing sets `ActionHooks::object_drops` in production (the app's game
    is `d2-client`'s); the server's `WiredWorld` does not hold drop
    tables. Follow-up with the app's game creation (HANDOFF §7v (c)).

## 5. Spec gaps (for PC 2)

| Id | File, section | Missing |
|---|---|---|
| G1 | `world/objects.md` §15 | Object population `0x00552610`: `PopulateFn` 1–9 bodies, objgroup density and draws, shrine / well limits per region, draw order against room monster population (`tick.md` §4 names it). |
| G2 | `world/objects.md` §8.2, §10; `sim/path-placement.md` §4 r5, §5.2 | What `0x00623830` (object "free the footprint") and `0x00620A70` ("stamp the footprint") do at the path level (= §5.2 remove with force 0 / 1? add `0x00649400`?); `0x0064D800(room, x, y, sx, sy, mask)` is not in the §4 r5 query table (box query `0x0064CEB0` semantics? result masked how?). `0x0064D800` is also the skill bodies' `box_collides` seam (`skills/bodies.md`), still on `Pending`. |
| G3 | `items/` (generation or treasure) | Bodies of `0x00585970(game, unit, code, quality)` (code drop, `objects.md` §8, `quests-act2.md` §1.3 gold) and `0x00559A30` (drop item code; `quests-act3-2.md` §11.3 gives only the `&level` argument). |
| G4 | `world/objects.md` §8.2 fn 7, §8.3 | `0x00584240` range test (metric, inclusive bound); `0x00582380` "inside the room" (half-open or closed rect); `0x005474C0` trap monster id (OQ3); `0x00582280` / `0x00582420` spawn arguments beyond the flag. |
| G5 | `world/objects.md` §9.2 code 17 | The fallback argument of the portal shrine's `0x0064E7B0` call (and what happens when no spot is found). |
| G6 | `world/objects.md` §9.3 | The missile creator of the storm / exploding / poison shrines and its parameter record: origin, which `param_flags` bits 0x520 / 0 are, target relative to the shrine's position; the walk order and metric of "within `Arg1` of the shrine" (storm). Also §9.1 r3 / §11 r2 "client update" of a stat set (which message, when). |
| G7 | `world/quests-act2.md` §1.3, `quests-act3.md` §4.6 | Body of the quest-chest gate `0x00545850(op)` ("object spec"; not in `objects.md`). |
| G8 | `world/hirelings.md` §6 r3, §10 r3, §11 r3 | The act change `0x0053ACC0` has no owner spec (where `0x00575BC0` and the follow sit in it); the restore's unit creation place (room / point) for `0x005774F0`; the item copy `0x0055A2A0` body for the 0x61 swap. |
