# Handoff: monster creation and initialization (`claude/impl-monster-init`)

Session: cloud implementation (METHODS M14, medium), 2026-10-06. Base:
`claude/bold-ptolemy-jvyvxy` at `a5b323a` (PR #13). Input:
`specs/monsters/init.md` (+ `specs/monsters/umods.tsv`). Scope of every
claim below: this branch, synthetic tests only, no game files (M09).
For the coordinator to fold into `docs/HANDOFF.md` and `docs/PLAN.md`.

## 1. State

**Implemented, unverified** (M02): the spec is a draft; no RNG trace of a
spawn exists (spec open question 4).

`d2_sim::monsters::init` (new submodule; the only edit outside it is the
`pub mod init;` line in `monsters/mod.rs`):

- §4 creation after placement (`create`): placement and allocation
  through the host, region count / coord list seam, alignment
  (`0x005543B0`: Align 1 → 2 + unit flag 0x20000, 2 → 1), normal mods,
  boss mods, party minions seam; flags 0x01, 0x02, 0x08, 0x40.
- §5 type init (`type_init`), §6 stats and skills (`stats_and_skills`),
  §7 level, §8 monlvl base values and `pct`, §9 player bonus (incl. the
  difficulty write), §10 components (16, region variants, 311/312), §11
  monprop, §12 monequip, §13 classic scaling.
- §14.1 normal mods (full table), §14.2 boss mods: **bloodraven only**.
- §16–§21: `mark_unique` / `mark_boss` (`0x005A0320`, §6.3 step 5),
  `random_boss`, `champion_pack_member`, `choose_umods` with
  `pick_champion` / `pick_unique` / `eligible`, `xfer_umods`,
  `boss_minions_and_init`, every umod init function of §19 (1, 2, 4, 5,
  6, 8, 9, 16, 17, 18, 23, 25, 26, 27, 28, 30, 36–39, 41),
  `superunique_init`, `restore_boss`, `restore_minion`.
- §22 dispatcher (`dispatch`, `handle_event7`) with bodies for
  `0x005A25F0`, `0x005A37D0`, `0x005A3800`, `0x005A3840`, `0x005A4230`;
  every other callback logs `Unhandled::Callback` in `MonsterStore`.
- §23 `unique_name` (client draw, pure); §24 `assign_mode`,
  `component_bits`, `components_field`, `write_boss_section`
  (`BitWriter`, low bit first), `LIFE_AT_SPAWN`.
- `UMODS` copied from `umods.tsv` (id, name, init_fn, unique_gate, six
  callbacks); the gate column drives `run_umod_init`.

Tests: 39 (+1 ignored game-file test `real_level_stats`): every
synthetic vector of the spec, the 12 edge cases, draw order replays,
`umods_match_tsv` with a perturbation test (M05, M08). `py
tools/coverage.py`: init.md 111/112 units claimed (unit tier); §1 (the
wrapper entry points, callers' code) is unclaimed.

Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p d2-sim
--all-targets -- -D warnings`, `cargo test -p d2-sim` (551 pass, 4
ignored), `cargo run -p depcheck`, `python3 tools/spec_index.py
--check`, `python3 tools/methods.py check`, `python3 tools/coverage.py
--check`.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/monsters/init/mod.rs` | `MonsterData` (monster data +0x14 block), `MonsterStore`, `CreateRequest`, `GameInfo`, `InitTables` / `Ctx`, stat ids, type/create/unit flags, `monstats_extra` (Sk1–8mode at +0x180), `component_counts` (monstats2 +0x15) | init.md Outputs, §2 |
| `crates/d2-sim/src/monsters/init/calc.rs` | `pct`, `player_bonus`, `area_level`, `monster_level`, `stats_by_level`, `monlvl_dm`, `hp_regen`, `classic_scaling` | §7–§9, §13, §6 step 10 |
| `crates/d2-sim/src/monsters/init/create.rs` | `create`, `type_init`, `stats_and_skills`, `components`, `monprop`, `monequip`, `normal_mods(_for)`, `boss_mods`, `assign_umod` | §4–§6, §10–§12, §14 |
| `crates/d2-sim/src/monsters/init/umods.rs` | `UMODS` (from `umods.tsv`), `AURAS`, umod choice / init / minions / superunique / restore, dispatcher | §16–§22 |
| `crates/d2-sim/src/monsters/init/message.rs` | `unique_name`, 0xAC init-owned fields, `BitWriter` | §23, §24 |
| `crates/d2-sim/src/monsters/init/seams.rs` | `InitHost` | |

## 3. Seams (`InitHost`) and expected providers

State goes through the host (`game()`, `units()`, `monsters()`,
`info()`), because spawning seams re-enter init: a boss minion's
creation runs `create` / `type_init` on the same host. Tables are a
`Ctx` (`Copy`) the host can keep.

| Methods | Provider |
|---|---|
| `stat`, `set_stat` | units/stats: `StatLists::unit_base` / `unit_set`, layer 0 |
| `allocate` | units: `units::lifecycle::allocate`; its `LifecycleHooks::init_kind` must call `monsters::init::type_init` for monsters |
| `place`, `register_spawn`, `party_minions`, `region_variant_count` / `region_variant`, `count_region_boss`, `boss_spawn` (§6.3 steps 1–4 only; step 5 is `mark_boss`, called by init), `spawn_boss_minion` (one §6.5 step-4 spawn; the count draw and class choice of §6.5 steps 1–3 are done here because the boss seed is reached through the host), `spawn_with_guid` | `monsters::population` session |
| `alloc_ai` (`AiStore::entry(u).control = Some(..)`), `ai_install` (`monsters::ai::install`), `link_minion`, `minions`, `boss_owner_data`, `set_ai_flag`, `run_ai_tick` (`0x00573780`) | AI (`monsters::ai`) |
| `give_skill`, `give_aura` | skills |
| `new_inventory`, `has_inventory`, `has_item_at`, `create_equip_item`, `apply_property` | items / treasure / vendors |
| `attach_quest_chain`, `quest_chain`, `boss_quest_hook`, `superunique_quest` | world/quests |
| `set_state`, `set_corpse_noselect`, `set_combat_mode`, `after_type_init`, `post_extra_list`, `set_alignment`, `level_id`, `montype_is`, `umod34_gate`, `set_difficulty` | units / stats / DRLG / data (montype nesting) |

Wiring: the monster timer type-7 handler calls
`monsters::init::handle_event7`; the mode-change code (`0x005A7C20`)
calls `dispatch(.., mode 0 / 1)`, combat `dispatch(.., 3 / 4)`,
missile creation `dispatch(.., Some(missile), 5)`. `MonsterStore::remove`
on unit removal. `NamedIds` (`monteleport` skill, bloodraven `BaseId`)
are resolved by name by the integrator.

Overlap to watch: `population.md` §6.5 step 4 also lists the xfer /
owner data / minion list / flag 0x10 steps; here they run in
`boss_minions_and_init` after each `spawn_boss_minion`. The population
provider must not repeat them.

## 4. Open questions (code TODOs; narrowest reading taken)

1. Boss mods §14.2: only bloodraven (spec OQ6); other cases do nothing.
2. Umods 17, 18, 23, 25 (OQ7): constants read "as fire" (`umods.tsv`
   wording); mana drain ×256 applied to the added value. Umod 26 body
   unread: skill monteleport level 1 mode 4, AI flag 0x20.
3. §24: how an all-zero component field is signalled (presence bit or
   header) is not stated; `components_field` returns `None`. The
   header and the field order around the init-owned fields are not
   built here.
4. Readings taken where the spec is terse: `monster_playercount` = the
   §9 n; superunique step 2's difficulty picks run only inside the
   "fewer than 5 umods" branch; mode-1 callbacks read the unit's mode
   after the change as "new mode"; umod 41's handler re-schedules only
   when the monster is alive; umods 38/39 add the same delta to maxhp
   and hitpoints; +0x5C bit 2 is set from create flag 0x08.
5. Callbacks other than §22's five bodies: logged (OQ8).
6. The first AI setup's draws (OQ2) and allocator draws (OQ3) are the
   providers' (`ai_install`, `allocate`).

## 5. Local run queue entries (for `docs/HANDOFF.md` §5)

- Game files: `D2_GAME_DIR=… cargo test -p d2-sim real_level_stats --
  --ignored`: expect pass (init.md "Real 1.14d values": level, min/max
  HP, AC, XP of six Act 1 classes, L-flag 0 zombie1 row).
- Recording: one population pass with the RNG hook (caller addresses)
  to confirm the Randomness order (OQ4); 0xAC assign decode against
  `write_boss_section` / `component_bits` on
  `traces/raw/20261006-015956-packets.jsonl` and `-022633-` (Recorded
  checks table).
