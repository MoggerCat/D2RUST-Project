# Handoff: native-assets session N1 (images)

Scope: `claude/native-n1`, `crates/d2-native`, `specs/formats/native-assets.md`
(draft, no game-file check run yet). Unit tests only (synthetic data); the
§7.2 game-file checks are still queued (M02): every kind below is
**unverified** against a real install.

## Done (all in `crates/d2-native/src`)

| Kind (Rust type) | Module | Native files | Spec |
|---|---|---|---|
| `d2_formats::dc6::Dc6` | `sheet.rs` | `P.png` + `P.toml` | §2.2 |
| `d2_formats::dcc::Dcc` | `sheet.rs` | `P.png` + `P.toml` | §2.2 |
| `d2_formats::dt1::Dt1` | `tileset.rs` | `P.toml` + `P.d/<n>.png` | §2.3 r1–r3 |
| `d2_formats::palette::Palette` (pal.dat) | `pal.rs` | `P.pal` (JASC-PAL) | §2.5 r1 |
| `d2_formats::palette::Pl2` | `pal.rs` | `P.toml` + `P.png` | §2.5 r2 |
| `expfield::ExpFieldData` (own struct, `from_bytes`) | `expfield.rs` | `P.png` + `P.toml` | §2.9 r2 |

`P` is the canonical source path (`data/global/ui/x.dc6`); native names append
to it (`x.dc6.png`). Not done by N1 (N2): COF, font, DS1, tbl, wav, animdata,
excel. DS1 (§2.3 r4) is N2's.

## API for N3 / N4

- `kind::NativeKind` (trait, implemented for every type above):
  `write(&self, p, &ViewPalette) -> Result<Vec<NativeFile>, NativeError>`,
  `read(p, &dyn FileStore) -> Result<Self, NativeError>`,
  `first_difference(&self, &native, p) -> Option<Difference>`.
  `NativeFile { path (relative to base/), bytes }`, files returned sorted by path.
  `ViewPalette` = `[Rgb; 256]`, goes in each PNG's `PLTE` (the converter passes
  act 1 `pal.dat`; `kind::grey_palette()` for tests). PL2 uses its own
  `base_palette` instead. The trait is open: N2's kinds may implement it too.
- `kind::round_trip(&original, p, &view) -> Result<Vec<NativeFile>, CheckError>`
  is the §4.3 check (write, read back, `==`). `CheckError::{Write, Read,
  Mismatch(Difference)}`; `Difference { file, detail }` names the native file
  and e.g. `direction 1 frame 1 pixel (1, 2): index 126 != 127`.
  `kind::check_files(&original, p, &files)` re-checks an edited file set.
- `kind::{FileStore, MemStore}`: readers take a store (in memory, or N4's
  layered source). Missing file → `NativeError` naming the path.
- `png::{write_indexed, read_indexed}`: the §2.1 r2 / §4.2 r2 PNG codec
  (fixed encoder settings; bytes are stable for the pinned `png` version).
- `native_toml::{Fields, hex2/4/8, parse_hex, array}`: strict sidecar reader
  (unknown key, wrong hex spelling, out-of-range ints are errors) and the
  hex writers; reusable by N2.
- ExpField: `ExpFieldData::from_bytes(&[u8])` parses `expfield.d2` (the
  `d2-sim` type is not used so `d2-native` has no `d2-sim` dependency).
- There is no `NativeAsset` enum (it must list N2's types too); N3/N4 add it.

## Decisions (provisional or spec-open)

- Sidecar layout is `[[direction]]` with nested `[[direction.frame]]` (a bare
  `[[direction.frame]]` in TOML cannot carry the direction index).
- Sidecars: `rect` is stored and checked against the layout of the frame
  sizes on read (a modder resizing a frame must fix the sidecar: strict).
  DC6 `width`/`height` are stored too and must equal the rect.
- `sheet = false` also when width or height of the sheet is 0 (e.g. only
  5 × 0 frames).
- DT1: `PROVISIONAL (§2.3 r3)`: assembled images over 2^24 pixels use the
  strip layout (no limit in the spec); `layout`/`image`/`origin` omitted
  for a tile with no blocks; a block whose pixels lie outside its shape
  (non-zero outside the diamond) cannot be written (error).
- DC6 `termination` is `"0x"` + the 4 file bytes in order; DCC
  `optional_data` is a bare lowercase hex string; PL2 groups carry
  `name/start/count` (alpha_blend counts 3 × 256 rows).
- `expfield` with 0 cells writes no PNG (as sheets).

## Left / for others

- §7.1 r2 "different thread order gives an identical files.tsv" and r6/r7:
  N3 / N4.
- Game-file check (§7.2): DT1 fallback tile count, PNG sizes, run time. A
  local session runs it once `d2-convert` exists.
- Spec status/`docs/PLAN.md` checklist not touched by N1 (parallel sessions);
  whoever merges should mark §2.2, §2.3 r1–r3, §2.5, §2.9 r2 as implemented
  (unverified).
