# Handoff: synthetic game data for CI — `claude/synthetic-data`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, base `claude/tender-meitner-mphas3`
at `4b5b0bf`. Repo only, no game files (M09, M16). Task class: test
infrastructure from specs, medium effort (M14).

## 1. State

CI had no game files, so the load path MPQ → `.txt` / `.bin` → typed
tables → server tables was tested only on fragments. It now runs end to
end on a synthetic install generated at test time:

1. **MPQ writer** (`d2_formats::mpq::writer`, behind the new
   `test-support` feature of `d2-formats`): writes format-0 archives that
   `Archive` reads. Hash table (§5 probe placement, deleted entries,
   locales), block table, both encrypted (§4); file data stored, PKWARE
   DCL under COMPRESS (mask 0x08) or IMPLODE (§9, §10; binary and ASCII
   literal modes, dictionary bits 4–6, greedy matching up to 518 bytes,
   end marker), encrypted with or without FIX_KEY (§7), SINGLE_UNIT,
   SECTOR_CRC (§8), optional `(listfile)` (§14), any sector size. A
   sector that does not shrink is stored raw (§8.4). Huffman, ADPCM, zlib
   and bzip2 are not written (no D2 text or table uses them).
2. **Synthetic table set** (`crates/test-fixtures`): headers for every
   called table's `.txt` come from `specs/data/fields.tsv` via
   `d2_data::schema()` (the union of the field lists compiled from each
   file); rows are written from scratch in `content.rs`. No value is
   copied from a Blizzard file: class, item, monster and level names,
   codes and numbers are made up. Only counts and value ranges the load
   checks demand are fixed by the specs (`loading.md` §8, §10.8:
   inventory 32, belts 14, difficultylevels 3, experience 101, arena 1,
   composit 16, armtype 3, superunique hcIdx 0–65, hireling name ids,
   automap names, gamble codes). Plus `string.tbl` / `patchstring.tbl` /
   `expansionstring.tbl` (TBL writer), `AnimData.d2` (writer),
   `soundenviron.txt`.
3. **End to end** (`crates/test-fixtures/tests/`), all in CI, no
   `#[ignore]`:
   - `synthetic_load.rs` (8 tests): stage 1 writes `d2data.mpq` (string
     table + every `.txt`), `d2exp.mpq` (expansion strings, AnimData,
     encrypted FIX_KEY), `patch_d2.mpq` (patch strings,
     `soundenviron.txt` imploded); `compile_all` reads them through
     `ArchiveSet`; stage 2 rewrites `patch_d2.mpq` with the compiled
     `.bin` of all 73 runtime tables, `hitclass.bin`, the 4 code buffers
     and the compile-only by-products; `bin::load` then loads the set.
     Checks: loaded bytes = compiled bytes for every table and buffer,
     `crosscheck::compare_sets` 73/73 identical; no compile diagnostic, no
     calc diagnostic, `links::validate_set` clean (> 100 links); every
     table decodes (`decode_by_name`) and written values read back;
     `fixup::apply` with the archive's AnimData; `.txt` bytes read back
     exactly; two builds give byte-identical archives; M08 perturbations
     (one cell → exactly one byte of the loaded record; one inventory row
     short → `LoadError::Check` on `inventory`; a missing hcIdx → load
     error on `superuniques`).
   - `server_tables.rs` (4 tests): from the loaded and fixed-up set,
     `ItemTables::from_fixed`, `VendorTables::from_fixed`,
     `TreasureClasses::build` (1 + 64 automatic + 3 TCs, no notes;
     monstats TC link 66 = "Synth Act 1"), `StatTable::from_fixed`,
     `StateTable::new`, `SkillTables::from_bin`, `CombatTables::from_bin`,
     `VitalsTables::from_bin` (max level, thresholds, `level_from_exp`
     on the synthetic curve), `UnitData::new`, `DrlgData`, `MazeData`,
     `OutdoorData`, `PopTables` (+ `with_bins`), `WaypointData`,
     `CubeData`.
   - Writer round trips: `d2-formats` `mpq/writer_tests.rs` (11 tests + 2
     property tests: every option combination × sizes around the sector
     boundary, text and noise; compression actually used; stored
     fallback; §13 key recovery finds the written key; probe chains,
     deleted entries, locale preference, case/separator folding; full /
     bad hash tables; listfile; sector sizes; a flipped byte is detected;
     implode ↔ explode for both literal modes and every dictionary size);
     `test-fixtures` `tbl.rs` and `animdata.rs` unit tests.

Not done: a running server game from these tables. No constructor
builds a `SimGame` / `ActionSim` from a `FixedSet` yet (every existing
test assembles `ActionTables`, `DrlgData` and a fake `TileSource` by
hand), and the DRLG needs DS1 / DT1 files, which this fixture does not
write. The `lvlprest` / `lvlsub` / `lvltypes` rows name `.ds1` / `.dt1`
paths that no archive holds.

## 2. Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-formats/src/mpq/writer.rs` | `MpqWriter` (`new`, `sector_size_shift`, `hash_table_count`, `with_listfile`, `add`, `add_file`, `add_deleted`, `to_bytes`, `write`), `FileOptions` (`stored`, `encrypted_fix_key`), `Method`, `Pkware`, `WriteError`, `implode` | `formats/mpq.md` §1, §4–§10 |
| `crates/d2-formats/src/mpq/writer_tests.rs` | round trips through `Archive` | same |
| `crates/test-fixtures/src/tbl.rs` | `write(&[(key, value)])` → `.tbl` bytes | `formats/tbl.md` |
| `crates/test-fixtures/src/animdata.rs` | `Anim`, `write` → `AnimData.d2` bytes | `formats/animdata.md` §2, §4 |
| `crates/test-fixtures/src/synth.rs` | `TxtFile`, `TableSet` (`headers`, `row`, `set`, `render`), `StringSet`, `Synthetic`, `synthetic()` | `data/schema.md`, `txt-format.md` §3–§5 |
| `crates/test-fixtures/src/content.rs` | the made-up rows, strings, anims; constants `CLASSES`, `CLASS_CODES`, `STATS`, `LEVELS`, `MAX_LEVEL`, `SUPERUNIQUES`, `exp_for_level` | `loading.md` §8, §10.6, §10.8 |
| `crates/test-fixtures/src/install.rs` | `write_text_install`, `compile_and_pack`, `bin_files`, `build` → `Install { dir, archives, compiled, loaded }`, `FixtureError`, `string_path` | `loading.md` §1–§4, §10.1 |
| `crates/test-fixtures/tests/{synthetic_load,server_tables}.rs` | the end-to-end tests | `loading.md`, the sim specs named in the file headers |

## 3. How to use the fixtures

From any crate's tests (add `test-fixtures` under `[dev-dependencies]`,
never `[dependencies]` of a game crate):

```rust
let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("my-test");
let mut data = test_fixtures::synth::synthetic();
data.tables.row("monstats", &[("Id", "wolf1"), ("enabled", "1")]); // extend
data.tables.set("charstats", 0, "str", "30");                      // change a cell
let i = test_fixtures::install::build(&dir, &data)?;              // archives + compile + load
let fixed = d2_data::fixup::apply(&i.loaded, &d2_data::fixup::read_animdata(&i.archives)?)?;
```

- Column names are the schema's (`fields.tsv` `column`, case-insensitive);
  an unknown column panics, so a typo cannot silently leave a cell empty.
- A `.txt` with no row gets one all-empty row (E3 forbids zero records).
- Use a distinct directory per test that builds its own install; the
  builds are deterministic, so parallel tests never share one.
- `install::build` returns the error of the first failing stage
  (`FixtureError::{Compile, Load, …}`), which is how the perturbation
  tests assert a load check.
- Other archive layouts: build an `MpqWriter` directly; to put a file
  into the install, add it in `install.rs` (stage 1 or 2).

Content rules kept by `synthetic_set_is_coherent` (fix the rows, not the
test, when it fails): every link resolves (no `LinkMiss`), no
diagnostics of any kind, `links::validate_set` clean. Two rules learnt
on the way: `itemtypes` row 0 has the empty code so an empty `equiv2`
links to 0 rather than −1 (`runtime-maps.md` §2 rejects a pushed −1);
`monmode.txt` / `plrmode.txt` need their `code` column filled (the
`_lookup` lists key on it, else DupCode). `experience` row `L + 1` is
the experience to reach level `L + 1` (`vitals.md` §4.1).

## 4. Changes to existing code (minimal, as allowed)

- `d2-formats/Cargo.toml`: feature `test-support = []`.
- `d2-formats/src/mpq/mod.rs`: `#[cfg(any(test, feature =
  "test-support"))] pub mod writer;` and `#[cfg(test)] mod
  writer_tests;`.
- `d2-formats/src/mpq/crypto.rs`: `encrypt` is compiled under
  `any(test, feature = "test-support")` (was `test`); the writer uses it.
- Workspace `Cargo.toml`: member `crates/test-fixtures`, workspace
  dependency `test-fixtures`. No parser changed; no public signature
  changed. `tools/depcheck` unchanged (the crate is test-only and pulls
  no Bevy).

## 5. Gate (this branch)

`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --
-D warnings` (clean); `cargo clippy -p d2-formats --features
test-support -- -D warnings`; `cargo test -p d2-formats` (164 pass, 1
ignored) and `cargo test -p test-fixtures` (3 unit + 4 + 8); `cargo run
-p depcheck` OK; `python3 tools/spec_index.py --check`; `python3
tools/methods.py check`; `python3 tools/coverage.py --check` (0 errors)
and `--selftest`; `tools/hooks/pre-commit` (no `.mpq` / `.tbl` / `.bin`
staged: every archive and table is generated under
`target/tmp` at test time).

## 6. Next

- A game-creation constructor from a `FixedSet` (`ActionTables`,
  `WorldTables`, providers) would let the e2e test go on to
  `SimGame::join` and one `Host::frame` on synthetic data; it also needs
  a DS1 / DT1 writer for a one-room preset town (both formats have
  specs; this fixture is the place for the writers).
- Coverage: the round-trip tests claim only `mpq.md` §5 and §14; a
  narrower claim set for §6–§10 needs numbered items in `mpq.md` (spec
  work), per the claim policy in `docs/COVERAGE.md`.
