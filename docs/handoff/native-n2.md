# Handoff: native-assets session N2 (text and audio kinds)

Branch `claude/native-n2`. Spec: `specs/formats/native-assets.md` §2.3 r4,
§2.4, §2.6, §2.7, §2.8, §2.9 r1, §7.1. No N1 asset-kind API existed on
`origin/claude/native-n1` when this was written, so these are plain
functions (no trait). All live in `d2-native`; all errors are
`toml_kinds::TextError { file, detail }` (Display: `file: detail`).

## API (how N3 calls it)

Every kind has `write_*`, `read_*` and `check_*`. `file` is the canonical
path used in error messages.

| Kind | Write | Read | Check (round trip, §4.3) |
|---|---|---|---|
| cof | `toml_kinds::write_cof(file, &Cof) -> String` (`P.toml`) | `read_cof(file, &str) -> Cof` | `check_cof(file, &Cof)` (C-STRUCT) |
| font | `write_font(file, &FontTable) -> String` | `read_font` | `check_font` |
| ds1 | `write_ds1(file, &Ds1) -> String` | `read_ds1` | `check_ds1` |
| tbl | `tbl::write_tbl(file, &StringTable, encoding: bool) -> TblFiles{tsv,toml}` (converter passes `true`) | `read_tbl(file, tsv, toml) -> StringTable` (`data_start`, `file_size` = 0) | `check_tbl` (C-TBL: struct minus layout fields, plus a lookup of every key); `rebuild_differs(&StringTable)` for the report count (§2.6 r3) |
| wav | `wav::write_wav(file, &Wav) -> Vec<u8>` | `read_wav(file, &[u8]) -> Wav` | `check_wav` (C-WAV) |
| animdata | `animdata::write_animdata(file, &AnimData) -> String` (`P.tsv`) | `read_animdata` | `check_animdata` |
| excel | `tables::write_excel(&[u8]) -> Vec<u8>` (verbatim) | none (the compiler reads the bytes) | `check_excel(file, source, native)` (C-TABLE step 1) |
| overrides | `BinOverrides::to_toml`; `derive_overrides(&CompiledSet, &BinSet)` builds it from the compile and the live `.bin`; path const `OVERRIDES_PATH` | `BinOverrides::from_toml`; `apply(table, record_size, &mut records)` | `check_tables(&CompiledSet, &BinSet, &BinOverrides) -> TableCheck{identical, failures}` (C-TABLE step 2, whole-table byte equality for the 73 runtime tables); `compare_table` for one |

Notes for N3 / N4:
- `derive_overrides` lists only `tables::EXPLAINED` (`monstats` 707
  `NameStr`) and only when the live bytes differ; any other difference is
  a `check_tables` failure. N4's native table source calls
  `BinOverrides::apply` after `compile_all`.
- Writers refuse (never silently drop) a struct they cannot represent
  exactly: tbl non-UTF-8 text, an element on an unused slot, shared or
  orphan slots; animdata names with bytes after the first NUL or a record
  outside its name's bucket; structs whose counts disagree with their
  vectors. N3 records these as `failed:<check>`.
- Strict readers: wrong `native`/`native_version`, unknown or missing key,
  out-of-range integer, hex in the wrong spelling, bad TSV escape or
  header, character above U+00FF in a Latin-1 field are errors naming the file.
- Helpers `toml_kinds::{Out, Tab, tsv_*, latin1, ...}` are `pub(crate)`
  (`Out` writes TOML by hand to keep the §2 key order; `Tab` reads and
  fails on unknown keys). N1 may copy the idea; I did not touch its files.

## Decisions to confirm in the spec
- DS1 `width`/`height` in the sidecar are the `Ds1` struct values (header
  field + 1); a layer has `height` rows of `width` cells. §2.3 r4 says
  "`height + 1` rows of `width + 1` cells", which reads as the raw header
  values: spec wording should say struct values.
- Byte-array fields use hex strings in file byte order: COF `unknown`
  `"0x"+8`, DS1 `unknown_header` `"0x"+16`, `unknown_groups` `"0x"+8`;
  DS1 `trailing` is bare lowercase hex. Object `flags` is `"0x"+8`.
- COF stores `frames`/`directions`; `layers_count` is the `[[layer]]`
  count; `draw_order` is an array of per-direction rows.
- tbl `[encoding]` also holds `[[encoding.slot]]` records (`slot`, `index`,
  `hash`) only for slots whose `index`/`hash` differ from what a rebuild
  gives (element number, full `key_hash`), so unused-slot records and odd
  hashes round-trip exactly. With `[encoding]`, a rebuild that differs
  from `slots` is a load error (§2.6 r3), so a live table that fails
  that makes the conversion fail loudly: this is the PROVISIONAL point
  the full run settles.
- animdata `.tsv`: a name has at most 7 bytes (the parser requires a NUL).

## Done / left
Done (unit tests with synthetic data; `Covers:` claims in the tests):
COF, font, DS1, tbl, wav (spec vector included), animdata, excel copy +
overrides + C-TABLE comparison. Not done: C-TABLE needs game files
(`derive_overrides` and `check_tables` run only on a real `BinSet`; their
building blocks are tested on synthetic monstats records). Queue for the
local run (with N3's converter): `d2-convert convert` then `verify
--deep`; expect 73/73 tables identical with one override, 0 tbl rebuild
differences. No DS1/tbl/animdata test from real files has been run.
