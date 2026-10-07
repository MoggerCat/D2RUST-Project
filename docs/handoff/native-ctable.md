# Handoff: native assets, C-TABLE step 2 (`claude/native-ctable`)

Spec: `specs/formats/native-assets.md` §2.8, §4.3, §7.1. Finishes the item
`native-wire.md` left.

## Done

- `crates/d2-native/src/tables.rs`: `run_c_table` (compile the native text
  set read through a reader, derive the overrides from the live `.bin`
  set, `check_tables`), `recheck_c_table` (the same for an existing root;
  the overrides file must equal a fresh derivation, so no stale or
  hand-added entry), `TableRun`, `TableCheck::total`. No change to
  `source.rs` or `layers.rs`.
- `tools/d2-convert/src/ctable.rs` (new): `applicable` (the install has
  live runtime `.bin`s), `run` (after the per-file loop: writes
  `base/data/global/excel/_bin-overrides.toml`, returns its row and a
  `TableSummary`), `recheck` (for `verify --deep`).
- Overrides row in `files.tsv`: kind `excel`, archive `-`, empty source
  SHA-256, one native file with size and SHA. Resume rule: the row is
  **never kept** from an earlier run (it depends on every other excel
  file); it is rederived after the loop of each finished run, and the old
  file is swept as an orphan first. A killed run (`stop_after`) writes
  neither row nor file. `verify` (also without `--deep`) checks its size
  and SHA like any native file; `--deep` skips the source comparison for
  it and runs the table check instead.
- Failure handling: any failing table, a compile error or a failed live
  `.bin` load gives a `failed:C-TABLE` row and failure lines (`complete =
  false`), and no overrides file is written.
- Reporting: `RunSummary::tables`, `report.txt` section `C-TABLE step 2:
  N/N tables identical to the live .bin (k overrides)` with one line per
  table, `verify_full` / `VerifyOutcome::tables`, and `d2-convert verify`
  prints `verify: C-TABLE N/N …` after `all files pass`.
- Tests `tools/d2-convert/tests/c_table.rs` on a synthetic install (full
  text set, live `.bin`s, 708 monstats records, live record 707 `NameStr`
  = 5382): convert gives 73/73 with one override (`record = 707`,
  `0x0615`), report and `files.tsv` row as above, `verify --deep` 73/73,
  a rerun leaves `files.tsv` byte-identical, a changed overrides file and a
  truncated `weapons.txt` fail verify; an install with no difference gives
  0 overrides; one other flipped cell fails `C-TABLE` naming monstats
  record 3 and `complete = false`.
- `docs/HANDOFF.md` §5 local-run entry: expected `report.txt` / `verify`
  lines and the `excel` count (+1).

## Decisions

- Step 2 is skipped (report: "not run") only when the install has no live
  runtime `.bin` at all, which is the case for the converter's other
  hand-made test installs; a real install always runs it. A partial set is
  a `C-TABLE` failure from the load.
- The overrides row counts toward `excel` `converted` in `manifest.toml`.
- `--language` (manifest `ENG`) is lowercased for `bin::load`.

## Not done

- Real-install run (73/73 on 1.14d): queued in HANDOFF §5.
- Audio, `Manifest::check_loadable`, `--mod` verify, runtime `kinds` map:
  `native-n4`.
