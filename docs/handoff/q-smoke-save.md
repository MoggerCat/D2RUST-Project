# q-smoke-save: save flows end to end (readiness check)

Branch `claude/q-smoke-save`. Synthetic tests only; everything d2rs-own,
unverified (rule 10). PROVISIONAL points: REC-282.

## What the smoke tests drive

`crates/d2-client/tests/smoke_save.rs` runs `play::run`'s wiring without a
window: `save::share` (the `SaveHandle` the app holds), the bridge over the
in-process server thread, the preview, the original UI with the Esc menu,
the death and hardcore systems. A character joins, is played on the
server (level, the 16 base stats with unspent stat / skill points, three
class skill levels, a waypoint and a quest flag on each difficulty, gold
in the inventory and the stash, items in the inventory, stash, cube page
and the right hand), then the Esc menu's **Save and Exit Game** is clicked
(the app must stop) and the save `play::run` makes after `app.run()` runs.
The `.d2s` is read with the game context (the same `d2s::read` call as
`LiveData::read_save`) and joined again; `save::read_live` after the load
must equal the state before, and the reloaded character's save must equal
the first file (save time and checksum aside).

| Test | Scenario |
|---|---|
| `every_class_round_trips_through_save_and_exit` | all seven classes, Normal |
| `a_nightmare_character_round_trips` | `--new ... --difficulty nightmare`, two classes |
| `a_mercenary_with_gear_round_trips` | a save with a hireling: restored at the join, hammer put in its hand, block and items back after the reload |
| `a_corpse_with_its_items_survives_save_and_reload` | softcore death, corpse takes the worn hammer, Esc respawn, Save and Exit, reload: the corpse and its item are back and saved again |
| `a_dead_hardcore_character_is_saved_dead_and_refused` | hardcore death: the death's save has HARDCORE \| DEAD, the reader refuses it (code 10), the earlier save is the `.bak`, Esc leaves, the exit save stays dead |
| `each_save_keeps_the_previous_file_as_bak` | `.bak` rotation over three saves, no `.tmp` left, the `.bak` loads |

One comparison rule is the spec's, not a loosening: a load clears every
item's 0x2000 (instore) flag (`d2s.md` §8.2 rule 7: "a file's 0x2000 never
survives a load"), so the before-state's items are compared with that bit
cleared (`loaded_live`).

## Breaks found and fixed (one commit each)

1. **A new character played above Normal wrote a save that does not
   load.** `play --new X --difficulty nightmare`, close, `play --save` →
   "Nightmare not unlocked" (`d2s.md` §2.2 rule 5.4): the client save
   flags had no progression. `save_gaps::seed_new_flags` now seeds the
   least progression that unlocks the played difficulty (REC-282 1).
2. **A softcore corpse and its items were lost after a reload.** The save
   wrote the corpse section, but the join never made the corpse
   (`ActionCharacter` "corpse" unapplied), so the next save dropped it and
   the items with it. Now `save_full::join_corpses` makes a corpse unit
   (`ActionSim::load_corpse`, d2-sim `wiring/action/dying.rs`, sharing
   the death's corpse allocation) and reads the saved list into it with
   the corpse placement of `d2s.md` §8.2 rule 3 (`InvDesk::load_corpse_entry`:
   a worn item put straight at its body location, no equip from the
   cursor; `WiredWorld::load_corpse_items`). Before, the equip path also
   left the item's 0x1 flag set. Where the loaded corpse stands is
   REC-282 2.

## Checked and not a break (the spec's behavior)

- Stamina is full after a load (`d2s-load.md` §9 rule 4: post-load sets
  stamina := maxstamina); life and mana keep the saved values.
- Items lose 0x2000 on load (above).
- The load report still lists the known unapplied `ActionCharacter`
  steps (header, npc fields, item indices, nextexp, quest entry, player
  skills on the synthetic game without vitals tables); the tests fail
  only on a failed load, a dropped item or a quest record error.

## Still broken / not covered

- **Belt**: the synthetic item tables have no potions or belts, so no
  belt item is saved; the belt placement on load is untested here.
- **Hireling rule 8** (`world/hirelings.md` §10 r8: refresh, life := max,
  stats sent after the hireling's items load) does not run; the report's
  "hireling rule 8" line is stale (the items are made now by
  `save_gaps::join_hireling_items`). Not visible in the save.
- The loaded corpse has no room and is not shown to the client (REC-282
  2): the player cannot walk to it and take the items back until a
  recording says where the original puts it.
- `play --synthetic` saves nothing (no save tables); the smoke tests use
  their own synthetic `SaveTables` (32-bit stats, the synthetic item
  tables).
- Hotkeys and mouse-skill item indices pass through as loaded
  (REC-265 6).

## The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/q-smoke-save; git checkout claude/q-smoke-save
cargo run -p d2-client --release -- play --new barbarian NmTest --difficulty nightmare
```
1. Play a little, Esc → Save and Exit Game. Then
   `cargo run -p d2-client --release -- play --save "%USERPROFILE%\Documents\d2rs\saves\NmTest.d2s" --difficulty nightmare`:
   it loads (before: "Nightmare not unlocked").
2. With a softcore character holding a weapon: die, Esc (respawn), Save
   and Exit, load again, Save and Exit again; then load once more: the
   log has no `join: save load: corpse` line, and the weapon is still in
   the save (the corpse is not visible in town yet, REC-282 2). Report any
   `NOT saved` or `join: save load` line.
