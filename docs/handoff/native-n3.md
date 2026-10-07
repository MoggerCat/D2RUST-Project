# Handoff: native assets, session N3 (converter)

Spec: `specs/formats/native-assets.md` §1, §3, §4, §7.1 tests 2, 3, 6, §7.2.
Branch: `claude/native-n3`.

## CLI

```
cargo run -p d2-convert -- convert --install <D2 dir> --out <dir> [--lang ENG] [--force] [--threads N] [--quiet]
cargo run -p d2-convert -- verify  --out <dir> [--deep --install <D2 dir>] [--quiet]
```

`--game` / `--native` are synonyms of `--install` / `--out` (the spec's
spelling). Progress goes to stderr. Exit 0 pass, 1 failures, 2 not a D2 LoD
install or bad usage. The commit in the manifest comes from the build-time
env `D2_CONVERT_COMMIT` (else `unknown`).

## What is in place

- `crates/d2-native/src/manifest.rs`: `Manifest` (read, write, strict on
  `format_version`, `check_loadable` for §3.4 r2), `files.tsv` rows
  (`FileRow`, `write_files_tsv` sorted, `parse_files_tsv` with a lenient torn
  tail), `check_native_name`. No timestamps. No new dependencies (the
  manifest is hand-written TOML, read with `toml::Table`).
- `tools/d2-convert` (lib + bin):
  - `names.rs`: §1 r3 name set from `mpq::names::known_names`, folded to
    canonical, closure over `Kind::references`.
  - `convert.rs`: §4.1 steps. Workers decode → write to `.work/tmp/<i>/` →
    read back from disk → `Kind::check`; the main thread commits in item
    order by rename into `base/` and appends the `files.tsv` row. Failures
    go to `.work/failed/<path>/` (with `failure.txt`), never `base/`.
    Resume (§4.6): kept rows need status ok, same kind version, same source
    SHA-256, files present at the recorded size; `base/` files with no row
    are deleted at start and end; `--force` redoes all. `manifest.toml` is
    removed at the start of a run and written last. Archive SHA-256s are
    identity only.
  - `verify.rs`: sizes and SHA-256 against `files.tsv`, `files_sha256`,
    orphans, recorded failures; with `--deep --install`, the kind's check
    against the install.
  - `report.rs`: `report.txt` (date, per-kind counts and work seconds,
    failures with first difference, kind notes, unnamed blocks per archive,
    unconverted extensions).
- Tests (`tools/d2-convert/tests/synthetic.rs`, synthetic MPQs from
  `d2_formats::mpq::writer`): §7.1 test 2 (1, 3 and 8 threads give identical
  `files.tsv`, manifest and `base/`), test 3 (one perturbed output byte →
  exactly that file and first difference; a post-hoc edit is found by
  `verify`, quick and deep), test 6 (killed after 1, 17 and 33 files, plus a
  planted orphan and a torn row → rerun equals an uninterrupted run); also
  kind-version redo, `--force`, no-op rerun, exit 2 on a missing archive.

## Not yet wired (N4 / merge)

N1 and N2 had pushed nothing when this was written (their branches equalled
the skeleton), so the converter is written against the `Kind` trait
(`tools/d2-convert/src/kind.rs`) and only `excel` is real (a verbatim copy
with a byte-equality check, claiming every `data/global/excel/*.txt`).

To hook a kind up: implement `Kind` as an adapter over the `d2-native`
reader and writer, and add it to `kinds::builtin()`:

- `name` / `native_version` as in the manifest `kinds`; `claims` by
  canonical path (§1.1); `phase` 0 for `excel`, `tbl` (tables first, §4.1 r3).
- `write(canon, src)`: decode with the `d2-formats` reader (a reader error is
  `Failure::new("decode", …)`), write the native files, return names relative
  to `base/` (§3.2) plus notes for the report (DT1 fallback tiles, `tbl`
  rebuild differences).
- `check(canon, src, native)`: `native` is read back from disk; decode it
  with the native reader, decode `src` again, compare (§4.3); return
  `Failure::new("C-DC6", "<first difference>")` etc.
- `references` / `has_references` for the kinds that name other files
  (DS1 → DT1, COF components, `sounds.txt` names, palette and font paths the
  client builds); the closure of §1 r3 uses them. Tables' own `references`
  (sound names, COF names from tables) are N2's to supply.

Left for N4 / follow-up: C-TABLE step 2 (compile the native table set with
`_bin-overrides.toml` and compare with each live `.bin`; once per run, after
the per-file loop), `Manifest::check_loadable` use in the runtime start
(§3.4, N4's `source.rs`), `--mod` verify (§6 r5), the `kinds` known-versions
map for the runtime, and the §7.2 run itself (queued in `docs/HANDOFF.md` §5).
Work time per kind in the report is the sum of per-file worker time, not
wall time per kind.
