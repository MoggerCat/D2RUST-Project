# Game-file tests: formats sweeps and the client asset path

> To be folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` by a docs session; this file stays as the detailed record.

Branch `claude/game-tests-client-assets`, based on
`claude/tender-meitner-mphas3` at `4b5b0bf`. Cloud session (repo only, no
game files). Task class: tests, medium effort. The diff adds two test
files and this note. It changes no library code, spec, `docs/HANDOFF.md`
or `docs/PLAN.md`.

## 1. State

- 17 new `#[ignore]` tests (11 + 6) that read `D2_GAME_DIR`. They compile and pass
  clippy, but **none has run**: no game files in the cloud (M02). Every
  expected value comes from a spec's Status line, Edge cases or Test
  vectors. The local run queue entry in §4 lists them.
- `python3 tools/coverage.py --check`: 3302 claims, 0 errors. Game tier
  187 → 203 units, verified 217 → 233 (8.0% → 8.6%), any tier 2459 → 2460.
  Read the 16 new game units as an upper bound until §4 has run
  (`docs/COVERAGE.md` §3): a failing test is fixed from the observation
  or its claim is removed in the same session.

| spec | units | game before | game after |
|---|---|---|---|
| client/assets.md | 12 | 1 | 5 |
| client/render-pipeline.md | 16 | 3 | 4 |
| formats/animdata.md | 14 | 1 | 8 |
| formats/cof.md | 5 | 3 | 4 |
| formats/dcc.md | 14 | 12 | 13 |
| formats/palette.md | 3 | 2 | 3 |
| formats/tbl.md | 6 | 3 | 4 |

## 2. Code map (tests added)

`crates/d2-formats/tests/game_sweep.rs`. Every sweep walks the **union of
all archives' listfiles** (names folded to lowercase, so each name is
counted once) and reads each name through `ArchiveSet` in search order.

| Test | Checks (source) | Claims |
|---|---|---|
| `dc6_every_file_decodes` | 1,657 files, 29,117 frames, 140 with flip = 1, termination EE×4 1,195 / CD×4 400 / 00×4 62 (`dc6.md` Status) | dc6 §file-header, §frame, §pixel-decoding |
| `dcc_every_file_decodes` | 21,717 files, version 6, < 8 PCD leftover bits per direction, no bottom-up frame, F frames per direction (`dcc.md` Status) | dcc: every Rules section |
| `dt1_every_live_file_decodes` | 254 live files, minor version 6, 6 version-4 files refused, block formats 0x0001 226,996 / 0x1001 110,259 / 0x2005 15,712 (`dt1.md` Status) | dt1 headers, §block-pixels |
| `ds1_every_file_parses` | 2,456 files, versions {3, 8, 12, 13, 15, 16, 17, 18}, 1,997 at v18 (`ds1.md` Status) | ds1 §rules |
| `cof_every_live_file_parses` | 3,605 parse, all version 20; the only failure is `amblxbow.cof`, which is 72 bytes and held only by `d2char.mpq`; `amblxbw.cof` parses; 3 files of 42 bytes with L = F = D = 1 and 3 padding bytes (`cof.md` Status, Edge cases) | cof: all 4 sections |
| `palettes_every_file_parses` | every `.dat` under `data\global\palette` parses, 19 `pal.dat`; 17 `.pl2`, one with 12 text colors and 16 with 13 (`palette.md` Status, Test vectors) | palette: all 3 |
| `font_tables_every_file_parses` | 14 font tables, version 1, 256 records each (`font-tbl.md` Status) | font-tbl headers |
| `string_tables_every_key_resolves` | 33 string tables in 11 `data\local\lng\<lang>` folders, version 1, ASCII keys, every used key's lookup finds a slot with that key (`tbl.md` Status, Test vectors) | tbl §header, §strings, §key-lookup |
| `animdata_real_vectors` | every row of `animdata.md` Test vectors "Real 1.14d" except the COF cross-check, and §1: only `d2exp` and `d2data` hold the file, the set loads the X copy | animdata §1, §2, §3, §4 r1 / r2 / r4, §6 |
| `animdata_matches_every_cof` | the COF cross-check row: 3,558 records, 3,529 with a `.cof`, equal frames / speed / events except the first copies of the 9 differing duplicates | animdata §2 |
| `expfield_layout` | the `expfield.d2` facts: only in `d2data`, 65,546 bytes, u16 266, 256 × 256, the nine cell counts, the centre and row 128 | none (the step rule is not exercised: d2rs has no reader) |

`crates/d2-client/tests/game_assets.rs`:

| Test | Checks | Claims |
|---|---|---|
| `every_listed_name_canonicalizes_and_reads_back` | the C7 path half: every listed name canonicalizes (0 refused), `parse_canonical` takes the canonical form back, `asset_path` agrees, and `read_asset` on the canonical path returns the archive set's bytes | assets §a1 |
| `each_loader_loads_a_real_file` | each of the 8 loaders through a windowless `AssetServer` with `MpqSourcePlugin` (first parsing file per extension, `act1\pal.dat`, `font16.tbl`, `eng\string.tbl`) gives the value the `d2-formats` parser gives | assets §a2 |
| `every_pl2_cof_and_tbl_loads` | the C7 loader half through the `AssetServer`: every `.pl2` loads; every `.cof` except `amblxbow.cof`, which fails naming its path; `data\local\font\**` `.tbl` → `Font`, `data\local\lng\**` → `Strings` | assets §a2, §edge-cases |
| `frame_sets_and_residency_on_a_real_dcc` | real frame sets equal the parsed DCC directions; a `Pool` budgeted to all four sets minus one byte evicts exactly direction 0 (frame 1, lowest key); a frame whose keys are resident loads nothing; a 1-byte budget keeps the current frame's key and logs the overrun; 5 stalls | assets §a3, §a4 r2 |
| `cpu_compositor_on_real_frames` | real DCC, DC6 and DT1 frames, act 1 PL2 light and hue maps and the three alpha tables; overlapping, off-screen and clipped items. `scene::compose` equals a direct per-pixel evaluation of §A4 / §A5 written in the test. The binned walk equals the reference under `verify::compare_indices` and `compare`. `perturb_indices(7)` reports exactly 7 | render-pipeline §a8 |
| `gpu_compositor_on_real_frames` | the same scene through `verify::gpu::Wgpu`: indices and RGBA 0 differing; the reference perturbed by 7 → 7 index bytes differ. Needs a GPU too. No adapter fails the test | none |

`cpu_compositor_on_real_frames` asserts at least one non-zero pixel,
so an empty scene cannot pass by default.

## 3. Findings (written blind, to settle on the first run)

1. **C7 conflicts with the specs.** §5 C7 expects every `.cof` to parse
   and every `data\local\font\**` `.tbl` to load as `Font`. `cof.md` names
   `amblxbow.cof` as junk, and `tbl.md` names `DEFAULT.TBL` and
   `FONTER.TBL` as plain text. The tests follow the specs and exclude
   those three files. **Gap:** `assets.md` does not say what `TblAsset`
   does with a plain-text `.tbl`. The test prints that outcome instead of
   asserting it, and an owner decision belongs in `assets.md` §A2.
2. **C7 named the file `assets_game.rs`.** It is
   `crates/d2-client/tests/game_assets.rs`, as this task asked. The C7
   entry and the §4 command map row "does not exist yet" can point here.
3. **Counting scope.** The spec counts come from `mpq-tool formats`. Its
   scope (distinct names, or per archive) is not stated in `specs/` or
   `docs/`. These tests count distinct names over all archives. If a
   count differs while every file decodes, record the scope in the spec's
   Status line and fix the test.
4. **The COF cross-check wording is ambiguous.** "3,529 of 3,558 names
   have a `.cof`" could count records, and 3,529 is also 3,558 − 29
   duplicates. The test asserts 3,529 records with a `.cof` and prints the
   number of distinct names. If the run gives 3,558 records and 3,529
   distinct names, the spec means distinct names: fix the test and the
   spec wording.
5. **Loose readings.** For SKA11HS, AMA1BOW, 10A1HTH and VMS1HTH, the test
   asserts only the event bytes the spec lists. It does not check that
   the other event bytes are zero, because the spec does not say so.

## 4. Local run queue entry

With `D2_GAME_DIR=<install>`, use release builds because the DCC sweep
decodes about 22k files:

1. `cargo test --release -p d2-formats --test game_sweep -- --ignored
   --nocapture`. Expect 11 passes. Each sweep prints its counts. A
   failure that names a count while every file decodes is finding 3. A
   failure in `animdata_matches_every_cof` on `with_cof` is finding 4.
2. `cargo test --release -p d2-client --test game_assets -- --ignored
   --nocapture --test-threads 1 --skip gpu_compositor_on_real_frames`.
   Expect 5 passes. The printed lines give the listed-name count (0
   refused) and the outcome of the two text `.tbl` files (finding 1).
3. On a machine with a GPU: `cargo test --release -p d2-client --test
   game_assets -- --ignored --nocapture gpu_compositor_on_real_frames`.
   Expect an `adapter: <name>` line, `GPU indices: 0 of 480000 bytes
   differ; GPU: 0 of 480000 pixels differ`, and a pass. The RGBA count
   after the perturbation is printed only, because the real palette can
   map two indices to one color.

Record the results in `docs/HANDOFF.md` §5. A test that fails is either
corrected from the observation or loses its `Covers:` line in the same
session.

## 5. Gate (this branch)

`cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D
warnings`; `cargo test -p d2-formats -p d2-client` (the new tests are
listed as ignored); `cargo run -p depcheck`; `python3 tools/spec_index.py
--check`; `python3 tools/methods.py check`; `python3 tools/coverage.py
--check` and `--selftest`.
