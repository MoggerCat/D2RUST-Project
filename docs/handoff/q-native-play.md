# Handoff: q-native-play (play runs from a converted folder)

Spec: `specs/formats/native-assets.md` §5, §7.1 test 7. Branch
`claude/q-native-play`. Synthetic data only: **unverified** against a real
1.14d conversion (rule 10); the local check is below.

## Links connected (the five seams of `native-n4.md`, plus the byte readers)

| Seam | Where | What changed |
|---|---|---|
| Source value | `d2-client/src/assets/game_files.rs` (new) | `GameFiles`: archives **or** native, one value that is both `TableFiles` (tables, strings, `lod`) and `FileSource` (typed `read_native`). On native, excel `.bin`/`.txt` reads are served from the compiled `NativeTables`; `.wav` is `None`. |
| 1. single_player | `app/single_player.rs` | `LiveData.archives: Arc<GameFiles>`; `LiveData::load` uses `bin::load_from`, the `AnimData` via `read_native`, `GameTables::from_loaded`, and `WorldFiles::load_typed`. `GameData::select_native(dir)`. `client_*`, `WaypointTables::live` take `&dyn TableFiles`. |
| 2. WorldFiles | `d2-server/src/world_data/mod.rs`, `archive.rs` | `WorldFiles::load_typed` (typed DS1 / DT1 readers); `load` is now a wrapper (bytes unchanged). `file_name`, `archive::load_native(&NativeSource)`. |
| 3. app.rs | none | `play` never used `MpqSourcePlugin` (that is `view` / `verify`); the Bevy loaders are not on play's path. |
| 4. ui / palette / items / hud | `app/ui.rs`, `palette.rs`, `items.rs`, `hud.rs`, `ui/original.rs`, `world_view/{ui_bind,panel_art,ground_items,unit_assets,automap_view,tile_assets}.rs` | Every place that parsed `read_file` bytes now reads the typed asset: `assets::path::{read_dc6, read_dcc, read_dt1_file, read_font_table, read_palette, read_pl2}` (native: the typed file; archives: decoded bytes). `ActPalettes::live` takes `Pl2::to_bytes()` (new, `d2-formats`). |
| 5. sound | `app/sound.rs` | `sound_table_live` reads `sounds.txt` through `TableFiles`; `.wav` reads are `None` on native, so no sound there. |
| main | `main.rs` | `play --native DIR` (and `D2_NATIVE_DIR`, the default folder) now runs the normal play path through `GameData::select_native`; the "seams" stop is gone. |

## PROVISIONAL / d2rs-own

- `Pl2::to_bytes` writes 0 for the base palette's 4th byte per color
  (`Pl2::parse` drops it; nothing reads it, `composition.md` §4). Archive
  PL2 bytes with a nonzero 4th byte differ in that byte only.
- Sound on native stays off (`sound_table_live` still builds the table,
  the pool finds no `.wav`). Wire audio when it is un-deferred.

## Tests

- `crates/d2-client/tests/play_native.rs` (moved from `tools/d2-convert/tests/` so d2-convert stays free of Bevy): the synthetic install (plus act
  PL2s and a DC6) converted by `d2-convert`, then `GameData::select` vs
  `GameData::select_native`: waypoint tables, every parsed DS1 / DT1, the
  fixed-up tables, `ActPalettes`, a typed DC6 all equal; `.wav` is absent.
- `d2-formats` `pl2_to_bytes_is_the_inverse_of_parse`.

**Not done:** a pixel-compare of the first rendered frame. The synthetic
install has no UI art, fonts or unit art, so a window frame on it is not
meaningful. That check is the user's local run below.

## The user's local check (1.14d files)

```
cargo run --release -p d2-convert -- convert --install "<D2 dir>" --out "<native dir>"
cargo run --release -p d2-convert -- verify --deep --install "<D2 dir>" --out "<native dir>"
# MPQ run, then native run, same seed:
D2_GAME_DIR="<D2 dir>" cargo run -p d2-client --release -- play --source mpq --new sorceress Test --seed 1 --frames 120
cargo run -p d2-client --release -- play --native "<native dir>" --new sorceress Test2 --seed 1 --frames 120
```

Expect: the native run prints `play: game data from native folder …` and
opens the town like the MPQ run (same panels, fonts, tiles, palette), no
sound. An error naming a path means that file is missing from the folder:
re-run the converter. Record both in `docs/HANDOFF.md` §5 (compare the two
frames).
