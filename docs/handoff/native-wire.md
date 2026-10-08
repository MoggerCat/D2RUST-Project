# Handoff: native assets, wiring session (`claude/native-wire`)

Spec: `specs/formats/native-assets.md` §1.1, §4.3, §7.1, §7.2. Follows
`native-n1.md`, `native-n2.md`, `native-n3.md`.

## Done

- `crates/d2-native/src/asset.rs` (new, below the N3 marker in `lib.rs`):
  `AssetKind` (Dc6, Dcc, Dt1, Pal, Pl2, Cof, Ds1, Tbl, Font, Animdata,
  ExpField). One dispatch type: `claims(canon)`, `write(canon, src)`,
  `check(canon, src, native_files)`, `references` (DS1 → its DT1 names,
  `.tg1` mapped to `.dt1`, developer prefixes cut at `data/global`),
  `native_version` (1), `phase` (tbl first). N1's kinds go through
  `NativeKind` / `check_files`, N2's through the `write_*` / `read_*`
  functions; no N1 or N2 module was changed. Errors are `AssetError
  { check, detail }` (`decode`, `write`, `C-DC6`, `C-STRUCT`, `C-TBL`, …).
- `tools/d2-convert/src/kinds.rs`: `Native(AssetKind)` adapts it to `Kind`;
  `builtin()` registers excel plus the 11 kinds. `SKIPPED_EXTENSIONS`
  gives `.wav` its reason; `report.txt` prints `.wav: N (skipped: audio not
  converted for now (native-assets.md OQ3))`. No audio kind exists.
- Test `tools/d2-convert/tests/native_kinds.rs`: a synthetic MPQ install
  with one file of every kind (test-fixtures for DC6, DCC, DT1, DS1, tbl,
  animdata; hand-built COF, font, pal.dat, PL2, expfield, excel, a `.wav`).
  Convert: 0 failures, `complete`, 1 converted per kind. `verify --deep`
  passes. Then every native file in `base/` is perturbed in turn (one byte):
  `verify` fails naming that file each time.
- `docs/HANDOFF.md` §5: the §7.2 entry now has the runnable command and
  expected output.

## Decisions

- PNG `PLTE` view palette is the grey ramp (`grey_palette()`), not act 1
  `pal.dat`: `Kind::write` has no cross-file context. It is a viewing aid
  only (§2.1 r2) and deterministic. A later change would need a `Kind` hook
  for run-wide context.
- A `.tg1` DS1 entry is turned into a `.dt1` name for the closure (§1 r3);
  names that do not resolve in any archive count as skipped for `dt1`.

## Not done

- **C-TABLE step 2** (compile the native excel set with
  `_bin-overrides.toml`, compare with every live `.bin`). It does not fit
  this session: the overrides file has no source file, so it needs a
  `files.tsv` row and manifest/resume rules of its own; `derive_overrides`
  needs `monstats` record 707, which the synthetic install lacks; so it
  cannot be tested here. Recipe: after the per-file loop, open the install
  (`d2_data::bin::load`), compile with `d2_data::compile_set::compile_all`
  over a reader on `base/data/global/excel/`, `tables::derive_overrides`,
  write `data/global/excel/_bin-overrides.toml` into `base/` (row, kind
  `excel`, no source SHA), `tables::check_tables`; report `identical.len()`
  of 73 and each failure.
- Audio (by decision). `Manifest::check_loadable` use, `--mod` verify, the
  runtime `kinds` map: N4.
- No game-file run has happened: every kind is unverified against a real
  install (queue in `docs/HANDOFF.md` §5).
