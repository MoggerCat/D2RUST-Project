# stitch-save: saving the played character

Branch `claude/stitch-save`. Code: `crates/d2-client/src/app/save.rs`;
test: `crates/d2-client/tests/app_save.rs` (synthetic). Everything here is
d2rs-own, unverified (rule 10): no check against 1.14d yet.

## Path traced (before)

| Link | State |
|---|---|
| `d2s::write` (`d2-formats/src/d2s.rs:1276`) | existed, used by tests only |
| appearance / hireling rebuild (`d2-server/adapters/character/save.rs`) | existed, needs an item view the sim does not have |
| live player → `D2s` | **missing**: nothing built a save from the running game |
| `play` → save on close / explicit | **missing** (`play.rs` only loaded) |
| where the file goes | **missing**: `--new` said "not saved" |
| load of stats | wired (`ActionCharacter::set_base_stat`); skills, waypoints, items, header, NPC fields, mouse skills stay *unapplied* (`character.rs:451-590`) |

## Links connected

- `app/save.rs`: `base_save` (loaded save, or a fresh header for `--new`),
  `read_live` (16 base stats + quest flag records from the local player),
  `apply_live`, `write_file` (temp file + rename, previous file kept as
  `<file>.bak`), `save_path`, `share` (`SharedLink` + `SaveHandle`).
- `play::run` wraps the server link in `SharedLink`, inserts `SaveHandle`
  as a Bevy resource, and saves after `app.run()` returns (window close,
  Esc-menu exit, `--frames`): prints `play: saved the character to ...`
  or `the character was NOT saved: ...` (never silent).
- `main.rs`: `--save-dir DIR`; `--new CLASS NAME` writes
  `<DIR or Documents/d2rs/saves>/<NAME>.d2s`; a name already taken, or a
  folder inside `$D2_GAME_DIR`, stops before the window opens.
- Hooks for stitch-hud: `save::request_save_and_exit(&mut MessageWriter<AppExit>)`
  (closes; the save runs on the way out) and `SaveHandle::save()` (explicit
  save, resource `Res<SaveHandle>`, absent when nothing is saved).

## Preview behaviour (all `// d2rs-own, unverified`)

- Only stats 0..=15 and quest records come from the running game; a game
  whose level stat reads 0 (the synthetic game has no stat table) keeps the
  loaded stats instead of zeroing them.
- Skills, waypoints, NPC fields, items, hotkeys, mouse skills, hireling,
  golem pass through from the loaded save unchanged; a new character
  has none of them (no start items, skills 0).
- A `--new` file has `status` = expansion flag only (not NEW: that marks
  the stub), `towns[difficulty] = 0x80`.
- Synthetic data (`--synthetic`) has no save tables: `play` prints that
  the character is not saved.

## What's left

- Sim state that must feed the save once it exists: skill levels,
  waypoints, items (G14/G16), mouse skills, current act/town byte.
- Live round trip of stats is untested (synthetic game has no
  `itemstatcost`): see the local check.
- The Esc menu (stitch-hud) must call the hook above.

## User's local check

```
cargo run -p d2-client --release -- play --new sorceress Tester
```
Walk a bit, close the window. Console: `play: saved the character to
...\Documents\d2rs\saves\Tester.d2s`. Then:
```
cargo run -p d2-client --release -- play --save "%USERPROFILE%\Documents\d2rs\saves\Tester.d2s"
```
Expect: the same name and class; the character panel's level/strength etc.
as the first run's start values (the stats are the only live values saved);
a `Tester.d2s.bak` appears after the second close. Running the first
command again refuses ("already exists"). Report any `NOT saved` line, and
whether `D2_GAME_DIR` saves load in the real game (not a goal yet).
