# Handoff: native assets, session N4 (runtime switch and mod layers)

Spec: `specs/formats/native-assets.md` §3.4, §5, §6, §7.1 test 7.
Branch: `claude/native-n4`. Synthetic data only: nothing here is checked
against a real 1.14d install yet (rule 10: **unverified**; the §7.2 run is
still queued).

## What is in place

| Piece | Where | What |
|---|---|---|
| Table source seam | `d2-data/src/bin.rs`, `strings.rs` | `trait TableFiles` (`read_excel`, `lod`, `string_table`); `ArchiveSet` implements it. `bin::load_from(&dyn TableFiles, lang)` and `StringTables::load_from` run the unchanged loader and checks; `bin::load` / `StringTables::load` call them. New error variants `LoadError::Source`, `StringsError::Source`. |
| Layer stack (§6) | `d2-native/src/layers.rs` | `LayerStack::open(root)`: `base/`, then the mods of `mods/order.toml` (`order = [...]`, later wins; no file = no mods). Per mod: strict `mod.toml` (`name`, `version`, `native_format_version` = this build's, `requires` listed earlier in the order). Refused at open, naming the file: a mod excel `.txt` or `_bin-overrides`, a mod `data/local/lng/**.tbl.tsv/.toml` (rule 9), a sheet / PL2 / expfield `P.toml` without `P.png` (or the reverse), a `P.dt1.toml` without `P.dt1.d/` (or the reverse). `patches()`: every mod's `files/patches/*` in layer order, then file name. |
| Native source (§3.4, §5) | `d2-native/src/source.rs` | `NativeSource::open(root)`: `manifest.toml` parsed (unknown `format_version` refused), `check_loadable` with `KNOWN_KINDS` (all 13 kinds at version 1), `files_sha256` against `files.tsv`, then the layer stack. Every error says to re-run `d2-convert`. `read_native(p) -> Option<Result<NativeAsset, String>>` (typed loaders, §5 r1): the layer owning the asset's primary file (`P.toml` / `P.pal` / `P.tsv` / `P`) supplies all of its files (pairs never mix layers). `decode_original(p, bytes)` gives the same enum from MPQ bytes. `tables(lang)` → `NativeTables` (a `TableFiles`): strings from the native `tbl`; the native `.txt` set (after the mods' table patches through `d2_data::patch`, layer order) compiled with `compile_all`; `_bin-overrides.toml` applied; each `.bin` and code buffer served as compiled bytes, `sounds.txt` / `soundenviron.txt` verbatim. |
| Server | `d2-server/src/world_data/archive.rs` | `fixed_tables_from(&dyn TableFiles, &AnimData)`, `fixed_tables_native(&NativeSource)` (tables plus the native `animdata.d2.tsv`). |
| Client source | `d2-client/src/assets/path.rs` | `FileSource::read_native` (default: decode the archive bytes) and `is_native`. `impl FileSource for NativeSource`: excel bytes verbatim; **`.wav` reads are `None`** (audio deferred: no sound on native for now); other kinds answer presence only. |
| Client loaders | `d2-client/src/assets.rs` | `NativeSourcePlugin { source }` registers `mpq://` over the native folder (name kept, §5 r2) and an `AssetSourceKind` resource; `D2AssetsPlugin` gives every loader (DS1, DT1, pal, DC6, DCC, PL2, COF, tbl/font) the native source, and they then take the typed asset instead of parsing bytes. Missing native file = load error naming the path (§5 r4). `choose_source` / `default_native_dir` / `SourceChoice` (§3.4 r1 order). |
| `play` flags | `d2-client/src/main.rs` | `--native DIR`, `--source native|mpq`. |

### Source selection (and the release default)

Order: `--native DIR`, then `D2_NATIVE_DIR`, then the default root
(`%APPDATA%\d2rust\native`, `~/.local/share/d2rust/native`,
`~/Library/Application Support/d2rust/native`) **if it holds a
`manifest.toml`**. `--source mpq` / `D2_SOURCE=mpq` forces the archives in a
debug build and is an error in a release build. `--source native` with no
folder found is an error telling the user to convert.

Deviation from §5 r5 (release = native only), on purpose: with nothing
configured, both builds still fall back to the archives (`D2_GAME_DIR`), so
the user's current `play` command keeps working before the first
conversion. Native becomes the default as soon as a native folder is
configured or found. Remove the fallback once play runs on native.

## What `play --native` does today

It opens and checks the folder, builds the native table set and runs
`bin::load_from` over it (all the loader's checks), prints

```
play: native folder <dir> (converter <v>, <n> mod layer(s))
play: native tables loaded (<n> runtime tables)
```

and then **stops with an error**: the play app still reads its levels,
UI art, fonts, palettes and sound through `ArchiveSet` in files this
session does not own. The seams for the integration session:

1. `app/single_player.rs` `GameData::select` / `LiveData::load`: take a
   `SourceChoice`; on native use `fixed_tables_native` (server archive.rs)
   and `bin::load_from(&src.tables(..))`.
2. `world_data/mod.rs` `WorldFiles::load`: takes raw DS1/DT1 bytes; needs a
   typed reader (`FnMut(&str) -> Result<Ds1 | Dt1>`) so the native source
   can feed it (`read_native`). Then add `world_data::archive::load_native`.
3. `app.rs:166` `MpqSourcePlugin { archives }` → `NativeSourcePlugin` on
   native (add it before `DefaultPlugins`, like today).
4. `app/ui.rs` (`FontMeasure::load`, `client_resist_penalties`),
   `app/palette.rs`, `map/mod.rs`: read through `FileSource::read_native`
   instead of `&ArchiveSet`.
5. `app/sound.rs` / `audio/pool.rs`: skip on native (no sound) until audio
   is wired; `FileSource` already answers `None` for `.wav`.

The converter (N3 + native-wire) must also write every kind (`tbl`,
`animdata`, images, …) for a real folder to load; on N3's head only
`excel` was real, so a folder converted from that head fails at the
strings (`no native string table`).

## Tests (synthetic, CI)

- `crates/d2-native/tests/source.rs` (§7.1 test 7):
  `native_equals_mpq_assets_and_tables`: the synthetic install (plus a
  DC6 and a `pal.dat`) converted in-test with the N1/N2 writers; every file
  (DS1, DT1, DC6, pal, tbl, excel) decodes identically from the archive and
  from native, and `bin::load` vs `bin::load_from(native)` give identical
  record bytes for every runtime table, `hitclass`, the code buffers, the
  sound tables and string lookups. `later_mod_wins_and_bad_mods_are_refused`:
  no order = base; `[a, b]` → b; `[a]` → a; a `requires` out of order, a
  sheet without its sidecar, and a mod `.txt` are refused naming the file.
  `bad_manifest_is_refused`: `format_version 2`, a tampered `files.tsv`, no
  manifest; the message names the cause and `d2-convert`.
- `layers.rs` unit tests (copies, half pairs, strict `order.toml` /
  `mod.toml`).
- `d2-client` `assets::tests::native_source_loads_typed_assets` (a
  `pal.dat` through `NativeSourcePlugin` and the Bevy loader equals the
  original; a missing one fails naming it) and `source_choice_order`.
- Not covered yet: a mod table patch end to end (the code path goes
  through `d2_data::patch`, untested here).

Gate on the pushed head: `cargo fmt --all`; `cargo clippy -p d2-native
-p d2-data -p d2-server -p d2-client --all-targets -- -D warnings`;
`cargo test -p d2-native -p d2-data -p d2-server` (all pass; d2-server's
`tests/world_data_tables.rs` shares one install dir per process and races
under `cargo test`'s threads; it passes with `--test-threads=1` or
nextest, unchanged by this branch); `cargo test -p d2-client --lib assets`;
`cargo run -p depcheck`; `python3 tools/coverage.py --check`.

## The user's commands (local, 1.14d files)

```
# 1. convert (once; N3's converter + native-wire's kinds)
cargo run --release -p d2-convert -- convert --install "<D2 dir>" --out "<native dir>"
# 2. verify
cargo run --release -p d2-convert -- verify --deep --install "<D2 dir>" --out "<native dir>"
# 3. play on native
cargo run -p d2-client --release -- play --native "<native dir>" --new sorceress Test
```

Expected: 1 and 2 exit 0, `manifest.toml` has `complete = true`. Step 3
today prints the two `play: native …` lines and then stops with the
"seams" error above. That is the check that the tables load from native.
After the integration seams land, step 3 must look **identical to the MPQ
run** (`play-integrate.md` step 1), except that there is no sound. A
`native assets in …: … run the converter again` error means the folder is
stale or incomplete: re-run step 1.
