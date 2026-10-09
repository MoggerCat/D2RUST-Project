# q-prov-data: provisional points the game data alone can settle

Session `q-prov-data`, branch `claude/q-prov-data`, cloud, 2026-10-09,
REC block 560–569 (none used). Methods: M23 (real data before green), M25.
Data: private repo `MoggerCat/D2RUST-private-repo` `install/` assembled
into `$HOME/game` (23 files, 0 mismatches); tools `mpq-tool`, `data-tool
excel-dir`. No table contents were copied into the repo; the numbers
below are measured values of a few cells.

## Batch 1 (rows with `settle_kind = data` / `data_alone_may_settle = yes`)

The index's `data` / `yes` flags are keyword sorts, not verdicts: of the
61 rows, most name a routine whose body is unwritten (REC-278/279/287/161,
binary), a recording (REC-291) or a behavior choice (quest links, AI
skill pick); the install's tables cannot choose those. What the data did
settle:

| Row (location) | REC | Result | How checked |
|---|---|---|---|
| `specs/ui/frontend-menus.md:648` | 208 | **settled**: `Patch_D2.mpq` `patchstring.tbl` el. 97 / 98 = Druid / Assassin (the d2exp table is the reverse); spec folded | tbl parse of both archives' tables |
| `specs/ui/frontend-loading.md:130` | 220 | **settled**: `Patch_D2.mpq` has neither loading-screen path; both d2data DC6 are v6, 10 frames 256×256, 666,014 bytes; spec folded | `mpq-tool extract-names`, header read |
| `specs/formats/native-assets.md:704` (OQ6) | – | **settled**: 21 string tables, C-TBL passes on all and the hash rebuild equals the stored slots in every one | new `d2-native/tests/real_tbl.rs` (ignored) |
| `d2-client/src/app/front_host.rs:183` | 189 | **fixed**: per-channel `min(255, d+s)` equals the PL2 additive table for only ~10 % of (d, s) pairs (sky PL2: 6,495 of 65,536), so mode 3 is now `ADD[256·d + s]` in index space, as `blend-modes.md` §1–§2 says | new `front_host_real.rs` compares the composed pixels with the table over the whole logo cel |
| `d2-client/src/app/front_host.rs:10` | 178/189 | **fixed (found by the same test)**: the logo files `FrontEnd\BlackLeft`/`BlackRight`/`FireLeft`/`FireRight` do not exist (real: `D2logo…`, spec §F1.5 r1), `FrontEnd\ShortButtonBlank` does not exist (real: `CharSelect\ShortButtonBlank`), Battle.net uses `WideButtonBlank02` (§F1.4 r3); the Art draw ignored DC6 frame offsets (the logo halves have x −169 / y 47 …) | `every_art_file_the_main_menu_draws_is_in_the_archives` |
| `d2-client/src/ui/panels/cube_items.rs:10` | 119 | **fixed (corner)**: `inventory.bin` record 9 / 25 = (118, 205, 139, 253) / +(80, 60); the estimated fallback (116, 130 …) was off | `prov_data_tables.rs` |
| `d2-client/src/ui/panels/stash_items.rs:12` | 104 | **fixed (corner)**: the classic stash (records 8 / 24, 6×4) is at top 273 / 333, not the expansion corner; expansion record 12 / 28 matched | `prov_data_tables.rs` |
| `d2-sim/src/wiring/economy/quest_host.rs:919` | 166 | **settled**: live `objects` `FrameCnt1` = txt × 256 (fixup `fixups.md` §13), so `>> 8` reads the txt value | `prov_data_tables.rs` (rows 1, 2); full-table scan: bin = txt for all rows before the fixup |

The live rows are used whenever the install's `inventory.bin` loads; the
fallbacks only matter for tests without tables, which now equal the real
rows.

## Left (with the reason)

- Binary-body or choice points, not data: REC-278/279/287/161/150/143/130/
  132/142/166 (quest links by class)/167/174/230 and the `d2-sim`
  wiring rows; REC-111 monster attack skill (monstats gives the
  `Skill1…8` / `Sk1mode` columns, but which entry an attack mode picks is
  the AI routine, `skills/use.md` OQ6); `hireling_drive.rs` (REC-100).
- Recording: REC-291 rows, REC-185 credits, `native-assets.md` OQ5 (the
  DT1 assembled layout needs the §7.2 full conversion run), OQ7 (a
  policy: monstats 707 `NameStr`, `field-types.md` OQ7 says 1.14d cannot
  settle it).
- `specs/render/camera.md:503/571` (REC-61): binary reads.
- Owned by running sessions: seed order, scenes compare, store fill,
  fixture migration, client missiles, q-prov-recording.

## Checks run

`cargo fmt --check`; `cargo clippy -p d2-sim -p d2-server -p d2-client -p
test-fixtures -p d2-native --all-targets -- -D warnings`; `cargo nextest
run` on the same crates; `tools/coverage.py --check`;
`tools/spec_index.py --check`; `tools/provisional_index.py` regenerated.
Real-data runs (`D2_GAME_DIR=$HOME/game … --run-ignored`):
`front_host_real` 2/2, `prov_data_tables` 2/2, `d2-native real_tbl` 1/1.

Gate hints: d2-client needs `libasound2-dev libudev-dev libwayland-dev
libxkbcommon-dev pkg-config` (the cloud image needs `apt-get update`
first; `mesa-vulkan-drivers` has no candidate there).
