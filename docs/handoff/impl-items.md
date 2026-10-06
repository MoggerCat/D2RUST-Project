# Handoff: items implementation (`d2_sim::items`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/impl-items`, based on `claude/bold-ptolemy-jvyvxy` at
`4c7a7c7` (2026-10-06, cloud). Specs: `specs/items/generation.md`,
`quality.md`, `affixes.md`, `properties.md` (+ `property-functions.tsv`),
all drafts.

## State

**Implemented, unverified.** Every rule of the four specs is in code;
every test vector and edge case of the four specs is a unit test (57
tests in `crates/d2-sim/src/items/tests/`), all on synthetic tables and
seam fakes. Nothing is confirmed against 1.14d: the specs' own checks
(item-creation recording R1, Ghidra request G1) are still queued.

Gate run on this branch: `cargo fmt --all -- --check`, `cargo clippy -p
d2-sim --all-targets -- -D warnings`, `cargo test -p d2-sim` (113 pass),
`cargo run -p depcheck`, `python3 tools/spec_index.py --check`,
`python3 tools/methods.py check`: all clean.

Changes outside `crates/d2-sim/src/items/`: one line in
`crates/d2-sim/src/lib.rs` (`pub mod items;`). No dependency, spec,
`tick`, `units`, `game.rs` or `rng.rs` change.

## Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/items/mod.rs` | request (`ItemRequest`, `RequestUnit`, `PlayerInfo`), `Item<S>`, quality/flag/type/stat constants, the seams `ItemStats`, `ItemGame`, `ListKey`, `UniqueBits`, `Fatal` | `items/generation.md` §1, Inputs/Outputs |
| `crates/d2-sim/src/items/tables.rs` | `ItemTables`: projection of `d2_data::tables` typed records + `FixedSet` maps (`from_fixed`); row projections `ItemRec`, `AffixRec`, `RareRec`, `QualityRec`, `UniqueRec`, `SetItemRec`, `SetRec`, `GemRec`, `RuneRec`, `PropertyRec`, `SkillRec`; `is_type` | all four (Constants & data dependencies) |
| `crates/d2-sim/src/items/create.rs` | pipeline `create_item`, base stats `init_item_stats`, elixirs, normal routine, class skill mods, sockets, ethereal, forced/ears/personalized, `replenish_timer` | `items/generation.md` |
| `crates/d2-sim/src/items/quality.rs` | ratio row, quality roll, dispatch + downgrade chain + finishing, low quality, superior, unique (+ dropped bits), set item | `items/quality.md` |
| `crates/d2-sim/src/items/affixes.rs` | alvl, magic roller + fit tests, rare names, magic/rare/crafted/tempered/charm/automagic | `items/affixes.md` |
| `crates/d2-sim/src/items/props.rs` | `FUNCS` (mirror of the TSV), dispatcher, property functions, modes 0–5, socket fillers, runeword match/activation, set bonuses, craft lists | `items/properties.md` |
| `crates/d2-sim/src/items/tests/` | `mod.rs`: `FakeStats`, `FakeGame`, synthetic itemtypes + equivalence; one file per module | |

`property-functions.tsv` is consumed per M05: `FUNCS` drives the generic
functions (value source, base reset, layer) and the test
`funcs_match_tsv` compares every cell with the TSV; its perturbation
test `tsv_check_catches_perturbation` (M08) changes one cell and drops
one row and checks the exact report.

## Public API (for the treasure session and other callers)

- `items::create_item(t, game, &mut req, use_seed, stats, frame) ->
  Result<Created<S>, CreateError>`: the whole pipeline (`0x00558D90`).
  `Created { item, event3_at }`; `event3_at` is the frame for timer
  event 3 if one is due (the caller schedules it, `sim/tick.md` §5).
  `CreateError::{Classic, BadIndex, Failed, NotPlayer, Fatal(Fatal)}`.
  The request's `ilvl` and `quality` are written back as in 1.14d.
- `items::ItemTables::from_fixed(&FixedSet)`: builds the tables once.
- `items::ItemRequest`: the drop request without the allocation fields
  (spawn mode, position, room, init flags stay with the caller's
  allocation). Treasure fills `item`, `ilvl`, `quality`, `index`,
  `flags2`, `format` (= `game.item_format()`).
- `items::replenish_timer(stats, scheduled, frame)`: re-run after
  socketing (`properties.md` §10.2).
- `items::props::{apply_socket_filler, runeword_match,
  activate_runeword, set_bonuses, apply_craft_list}` for the inventory,
  equip and cube specs.

## Seams (each with its expected provider)

| Seam | What | Expected provider |
|---|---|---|
| `ItemStats` | `has_stats`, `stat` (unit total), `base`, `set_base`, `has_list`, `list_set`, `list_add`, `list_get` keyed by `ListKey { state, flags }` | units/stats session (`sim/stats.md`, `sim/stat-lists.md`) |
| `ItemGame` | game seed, difficulty (+0x6D), expansion (+0x70), ladder flags (+0x6A, +0x74), unique-dropped bits (+0x1B24, `UniqueBits`), `item_format` (+0x78) | `game::Game` / units session |
| allocation | `create_item` derives the unit and item seeds itself in the §2.1 order (game seed: `derive`, then one `step`); the caller allocates the unit and passes its stat holder | `sim/units.md` allocation (`0x00555230`); when it exists, move the derivation there |
| `RequestUnit` | the request unit's class, player name, level (stat 12), hardcore flag, staged by the caller | units session |
| itemratio row | `quality::ratio_row` implements `treasure.md` §6 step 3 with a version-limit argument | duplicates the treasure session's ratio row; merge into one when both land |

## Open questions (each has a `TODO` in code naming it)

- **OQ-G1** (`create.rs`, gold): `treasure.md` §8 step 1 "a
  drop-request quantity > 0 replaces it" is read as the quantity
  override (+0x54), not the quantity (+0x34).
- **OQ-G2** (`create.rs`, forced §9 step 2): "Format 0 only: forced
  socket count. Then flag 0x800 := …" — the flag copies are read as
  applying to every forced item.
- **OQ-P1** (`props.rs`, functions 18 and 19): "set the stat" is read as
  §4.2 with set = 1 (valshift applies; a 0 value writes nothing).
- **OQ-P2** (`props.rs`, function 14): when the cap is < 1 the function
  returns 0 without setting the flag or the stat.
- **OQ-P3** (`props.rs`, runeword match §10.1): "must equal the fillers
  in order and cover at least the socket count" is read as every listed
  rune compared with a filler, so a row matches only with exactly n
  runes.
- **OQ-P4** (`props.rs`, set bonuses §11): the flags of the bonus list
  are not in the spec; the caller passes them in the `ListKey`.
- **OQ-A1** (`affixes.rs`, rare §7 / crafted §8 / automagic §11): the
  roller's "force" argument is read as true in these routines (the spec
  writes "force" as the flag's value, and §Randomness says "§3 with
  force" for §11).
- **OQ-Q1** (`quality.md` §8.1 test vector "idx 4097 … unique fails
  (unless `nolimit`)"): by §8 step 6 the accept test (idx ≤ 4096) runs
  before the marking, so `nolimit` alone does not make it succeed; only a
  quest item reaches the marking. Implemented per the rules; test
  `unique_index_4097` documents it. A spec session should reword the
  vector or the rule.
- **OQ-Q2** (`quality.rs`): an itemratio divisor of 0 returns
  `Fatal::DivideByZero` (1.14d would fault); none occurs in 1.14d data.
- Format-0 branches (`generation.md` OQ 1, `affixes.md` OQ 2): not
  implemented; format-0 items run the format ≥ 1 rules.
- Quality-5 socket filler `0x00663CC0` (`properties.md` OQ 2): no-op.
- Readings made where the spec says "stat" without "base": totals
  (`ItemStats::stat`) are read for stat 72 in `D2`, stats 21–24 / 31 /
  159 / 160 in low quality, stat 194 in §7.3, stats 252/253, 254, 152;
  base for stat 73 in `D2` and ethereal (the spec says "base" there).
- Low-quality "`quality 1 (low)` and > 3 → 4" in staffmods tiers is
  implemented literally (tier > 3 becomes 4).

## Checks to queue (`docs/HANDOFF.md` §5, for the coordinator)

1. **Live tables** (local, `game/`): a test that builds
   `ItemTables::from_fixed` from `d2_data::fixup::apply` on the live set
   and checks counts (items = weapons + armor + misc; magic = 747 + 669 +
   36 rows; rare suffix count; 8 qualityitems; skill lists 30 per class).
   It needs `d2-formats` to open the archives, which `d2-sim` does not
   depend on: put it in `crates/conformance` or `d2-data/tests`.
2. **Item-creation recording R1** (`generation.md` OQ 2, `quality.md`
   OQ 1, `affixes.md` OQ 1, `properties.md` OQ 1): every unit-seed and
   item-seed draw between allocation and the return of `0x00558D90`,
   plus a dump of the finished item (quality, file index, affix slots,
   flags, stat-list entries). Replaying it through `create_item` with the
   recorded game seed, request and tables is the acceptance check for
   this module; until then it stays "implemented, unverified".
3. Ghidra G1 (`properties.md` OQ 3): owner vs item list in §4.2.
