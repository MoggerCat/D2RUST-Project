# q-fix-d2-town-mode — hand-back (2026-10-09)

Task: ledger first divergence D2 (town NPC mode 2 in 1.14d vs 1 in d2rs).

## Done

- **Larzuk (class 511) walks like 1.14d** (REC-1380). Cause: the map-AI
  store `0x00545C90` (`world/quests-act5.md` §5.8) was never called. The
  DS1 path of Larzuk's start dummy (object 543) was dropped, so his Npc AI
  had no map nodes and idled where 1.14d takes a node (mode 2, target
  (5144, 5037), one seed step, frame 24).
  - `LevelTypes::unit_path` / `Presets::unit_path`: peek at a preset unit's
    path (a copy, as `0x006660B0`).
  - `View::spawn_preset_units`: after an object preset of class 459 / 461 /
    543 is created with a path, `QuestObjectHost::map_ai_store` runs on the
    lent quest control (`QuestLoan`), which keeps the path
    (`ActionHooks::map_ai_paths`, handle = index + 1) and calls
    `act5::q1::larzuk_map_ai` / new `act5::q3::map_ai_store` (Anya 459,
    Nihlathak 461).
  - `HostQuests::apply_map_ai` (was an unhandled stub) sets the unit's AI
    `map_ai` nodes.
- Checks, scenario-diff `state` (Wine, 2026-10-09): `a5-harrogath-arrival-ama`,
  `a5-town-arrival-bar`, `milestone-act5-entry`: DIVERGED -> PARTIAL (all
  60 frames equal; `own` / `q` recorded on one side only).
- Tests: `q3::map_ai_store_applies_once_when_in_town`; `cargo nextest run
  -p d2-sim` 4704 pass; clippy and fmt clean; `coverage.py --check`,
  `spec_index.py --check` clean.
- Ledger: `docs/handoff/ledger/q-fix-d2-town-mode.tsv` (`level.a5.109`:
  DIVERGED -> NO-CHECK / PARTIAL).

## Open (not town-mode bugs; routed)

| Check | First difference now | Cause | Owner |
|---|---|---|---|
| `act-travel-lut-ama`, `join-act2-quests-ama` | frame 24/32, class 201 (Jerhyn) m 2 vs 1, no seed draw, target own + (2, 2) | Jerhyn's Npc case calls `0x0059F570`, whose body no spec states (REC-734; pc1-data Step 4 item exists). Needs the function read, then the three `jerhyn_*` / `guard_moving` seams wired to `act2::q4` | q-fix-pc1-proto-items (spec by PC 1) |
| `milestone-nihlathak` | frame 30, class 472 (Halls of Vaught monsters, not a town NPC), m 2 vs 1 with seed draws; d2rs never runs a think for unit 1:28 | monster AI target acquisition, not town mode | q-fix-b-monster-combat |
| `a5-warp-wsk-ama` | frame 40, class 545, m 1 vs 2 (d2rs moves earlier) | monster AI, reverse direction | q-fix-b-monster-combat |

Anya (459) and Nihlathak (461) stores are wired and unit-tested but no
check exercises them yet.

## Repro

```
export D2_GAME_DIR=$HOME/game
python3 tools/scenario-diff/scenario_diff.py traces/checks/a5-harrogath-arrival-ama.check --work /tmp/sd --reuse-orig
```
(`--reuse` also reuses the d2rs side and so hides a rebuild; use `--reuse-orig`.)
