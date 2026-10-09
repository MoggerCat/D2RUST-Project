# q-fix-d11-stony-objects: hand-back

## Done

- **D11 fixed.** Check `combat-pop-stony-field`: frame 4 objects 2:18–20 (the
  Stony Field arrival room: two class-37 objects and the class-119 waypoint)
  were created in reverse order in d2rs (waypoint, 37, 37; 1.14d: 37, 37,
  waypoint). Cause: the outdoor substitution (`outdoor-tilesub.md` §4.4 r3,
  `0x0066FA10` → `0x0066BF30`) *appended* the pattern file's preset units to
  the room list; `0x0066BF30` *prepends* (the `0x0066BF30` seam in
  `wiring/worldgen/levels.rs` already did). Fix: `drlg/outdoor/tilesub.rs`
  `apply` inserts at the head. Spec note added to `specs/drlg/outdoor-tilesub.md`
  §4.4 r3. Tests: new `apply_prepends_preset_units`; `mutant_tests.rs`
  `apply_reads_the_variant_box` expected the old order and is updated.
- Frame 4 is now equal for every unit (object GUIDs, classes, positions,
  modes, seeds). `cargo nextest run -p d2-sim`: 4704 passed. clippy, fmt,
  `coverage.py --check`, `spec_index.py --check` clean.
- Ledger part: `docs/handoff/ledger/q-fix-d11-stony-objects.tsv` (rows
  `level.a1.4.act-1-wilderness-3`, `drlg.outdoor.tilesub`; validated with
  `tools/coord/ledger.py --parts docs/handoff/ledger --check`). Its
  `last_verdict` stays the suite file's `DIVERGED@4` (the validator derives it
  from checks-status.md); the real verdict is now `DIVERGED@5`.

## Open: the next first difference (frame 5), not fixed (other areas)

The check is still DIVERGED, first at frame 5 (121586 differences, mostly
monster state cascading from the game seed):

1. **Object: chest class 144 missing** at (5121,5148), 1.14d GUID 30 (right
   after the room 16 presets, before room 14's), `PopulateFn` 3, group 33
   `outsideforestobj1` (139/140/144, 25/50/25), `Levels` ObjGrp2 prob 27.
   d2rs's room 16 (DRLG origin 5120,5120) draws slot rolls 5,51,56,58 (none
   hits); 1.14d needs slot 2 ≤ 27 and member roll ≥ 75, so the room seed
   (R) was in a different state when populate ran. Rooms 14, 17, 19, 20 match,
   so suspect R draws between room creation and populate in room 16 (monster
   preset walk 817/818, `place_presets`, ambient) — monsters/population
   (`monster.population`, owner q-fix-real-unit-seed-order).
2. **Object class 61 (invisible, init 13) mode**: 1.14d mode 0, d2rs mode 2
   (`objects-2.md` §17 init 13). `MechWorld::quest_link` (`world/objects.rs:433`)
   is the default stub (`false`), so d2rs always takes the "else mode 2"
   branch; 1.14d links the object to quest chain 4's record. Needs the quest
   control's record lookup wired in (`quests.md` §4.6, `0x00543640`/`0x005436B0`):
   quests area.
3. Monsters: 1:52 class 20 missing, 1:26 class 20/58, the game seed at frame 5.

## Repro

```
export D2_GAME_DIR=$HOME/game CARGO_INCREMENTAL=0
rm -f traces/raw/check-combat-pop-stony-field/d2rs.*   # keep orig.* (Wine recording)
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-pop-stony-field.check --reuse
```
(no `--reuse` re-records 1.14d under Wine: ~10 min; the d2-client build ~10 min cold.)
