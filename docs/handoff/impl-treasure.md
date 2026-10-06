# Handoff: treasure implementation (`d2_sim::treasure`)

Scope: branch `claude/impl-treasure`, from `claude/bold-ptolemy-jvyvxy` at
`4c7a7c7`. Spec: `specs/items/treasure.md` (draft) with
`treasure-quality.tsv` and `treasure-chest-acts.tsv`. Cloud session, repo
only (no game files).

## State

**Implemented, unverified.** Every synthetic test vector and edge case of
the spec passes as a unit test (`cargo test -p d2-sim treasure`: 48
tests, 104 in the crate). The spec's real-data vectors and recordings are
queued below; nothing here is "done" until they pass (M02).

Gate run before push: `cargo fmt --all -- --check`,
`cargo clippy -p d2-sim --all-targets -- -D warnings`,
`cargo test -p d2-sim`, `cargo run -p depcheck`,
`python3 tools/spec_index.py --check`, `python3 tools/methods.py check`:
all pass.

Changes outside the module: one line `pub mod treasure;` in
`crates/d2-sim/src/lib.rs`. No dependency, spec or other-module edits.

Module path: the task put the drops in `d2_sim::treasure`; the spec
header names `d2-data::treasure` (runtime form) and
`d2-sim::items::treasure` (drops). The runtime form (§1) is built here
from `d2-data` typed records, with no I/O. A spec session should update
the header's "Crate/module" line (or move §1 into `d2-data` and move the
code with it).

## Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/treasure/mod.rs` | `TreasureData` (the loaded data a drop reads), `TreasureError`, re-exports | `items/treasure.md` |
| `crates/d2-sim/src/treasure/runtime.rs` | TC runtime form from typed records: `ItemData` / `item_list` (weapons, armor, misc), automatic TCs, `treasureclassex` rows, item strings and parameters, chest table; `TreasureClasses::get` (§2), `chest_tc` | `items/treasure.md` §1–§2 |
| `crates/d2-sim/src/treasure/walk.rs` | the TC walk (slot stack, negative picks, NoDrop, entry search, sub-TCs, item entries, drop flags, classic throwables, gold multiplier and gold find), `player_factor` / `nodrop` (§5.4), `item_level`, `gold_base`; the `DropSink` seam | `items/treasure.md` §5, §7, §8 |
| `crates/d2-sim/src/treasure/softfloat.rs` | integer binary64 (round to nearest even) for the NoDrop formula: add, sub, mul, div, truncation | `items/treasure.md` §5.4 |
| `crates/d2-sim/src/treasure/quality.rs` | drop quality: `LADDER` (= `treasure-quality.tsv`), `ratio_row`, `step_chance`, `roll_quality` | `items/treasure.md` §6 |
| `crates/d2-sim/src/treasure/drop.rs` | monster gate / TC choice / quest TC / level upgrade / `monster_drop` (incl. Find Item, `F` = 1); `area_level`, `chest_tier`, `chest_drop`; `CHEST_ACTS` (= `treasure-chest-acts.tsv`) | `items/treasure.md` §3–§4 |
| `crates/d2-sim/src/treasure/tests.rs` | spec vectors, edge cases, TSV checks (`ladder_matches_tsv`, `chest_acts_match_tsv`) and their perturbation test (`tsv_checks_catch_perturbations`) | |

M05/M08: both TSVs are checked by a test that renders the Rust constant
row by row and compares it with the file (`include_str!` of the spec
TSV); the perturbation test changes one TSV cell and one constant field
and expects exactly that row reported. The soft-float is checked against
the host's IEEE binary64 in tests only (hard rule 6 applies to sim code
paths; the test module allows `float_arithmetic`), with a one-bit
perturbation test.

## Seams (expected providers)

| Seam | Where | Provider |
|---|---|---|
| `DropSink::place` (§7 step 2: start offset when a room exists, free-spot search `0x0064E810`) | `walk.rs` | collision / rooms spec (DRLG/world session); no spec yet (OQ 8) |
| `DropSink::create` (§7 step 4, `0x00558D90`; gold base §8 step 1 via `gold_base`) | `walk.rs` | items session (`d2_sim::items`) |
| `DropSink::gold` / `set_gold` (stat 14; negatives clamped by the caller here) | `walk.rs` | items / stats session |
| `GameFacts` (expansion, difficulty, game type, living players, `players` setting, item format) | `walk.rs` | game state (`d2_sim::game`), `d2-server` host for `S` |
| `Dropper` (`DropperKind`: monster class / `level` stat 12 / stat 100; player base level; object area level via `area_level`; position) | `walk.rs` | units/stats and monsters sessions |
| `Recipient` (`party`: living members of `O`'s party in `O`'s level when `O` is a player; `magic_find` stat 80 + minion owner's; `gold_find` stat 79 + owner's) | `walk.rs` | units/stats session (party `0x005408E0`, owner `0x0058F0D0`) |
| `MonsterRank` (hcIdx/superunique record, champion flag 4, unique flag 8) | `drop.rs` | monsters session |
| `quest_open(TCQuestCP)` callback (§3.3 owner resolution and quest flags 15, 1, `TCQuestCP`) | `drop.rs` | quests session + units (owner, `0x00552F60`) |
| `monster_drop_gate(flags, collision, class)` inputs (unit +0xC4, collision mask 0x801) | `drop.rs` | units + collision |
| chest `has_room`, `act`, `level` | `drop.rs` | objects / world session (operate functions choose `Q`) |
| `EquivMatrix` (itemtypes) | `TreasureData`, `TcSources` | `d2_data::fixup::maps::equiv_matrix` |

## Open questions (each a `TODO` in code where it applies)

1. **OQ-atol** (`runtime.rs` `atol_u16`): CRT `atol` (`0x00681EBB`) beyond
   i32 is not specified; d2rs saturates like `strtol`. No 1.14d value
   gets there.
2. **OQ-quest-magic** (`quality.rs`): §6 step 2 "itemtypes `magic` and
   items `quest` → 7" implemented as a conjunction (one check), not two.
3. **OQ5 NoDrop range** (`walk.rs` `nodrop`, `softfloat.rs`): binary64
   with round to nearest even; any value outside the binary64 normal
   range or i32 (infinite, NaN, `n0 + C` = 0, huge `N`) returns
   `TreasureError::NoDropRange`, since x87 precision control and the
   conversion's out-of-range result are unconfirmed. Unreachable on 1.14d
   data (spec §5.4).
4. **§5.6 with `get` = none**: the new slot's picks and mods are not
   stated; d2rs uses 1 and the parent's mods (never read: the slot ends
   at once).
5. **OQ-gold-equiv** (`walk.rs`): "of type 4 or an equivalent"
   (`0x00629BB0`) uses the item test of `0x00629A90` (type or nonzero
   type2).
6. **Ratio row match** (`quality.rs` `ratio_row`): `Class Specific` and
   `Uber` compared for equality with 0/1 (a stored 2 matches neither).
   The uber test's "type ≠ 38" reads the item's own `type` field.
7. **Itemratio divisor 0**: returns `TreasureError::ZeroDivisor` (the
   original's `idiv` would fault; not in the spec).
8. **§3.1 order**: flag, then collision, then the `bonewall` fatal check,
   read in the spec's sentence order. Whether a bonewall on a blocked
   spot is fatal is unconfirmed.
9. **Chest act**: §4 step 2 "act of the object's level" is a caller input
   (`act`); act > 4 is `TreasureError::Act`.
10. **treasureclassex +0x30/+0x32** (mods slots 5/6): not in
    `fields.tsv` / the typed record, so built as 0 (the spec says always
    0 in 1.14d). A mod that fills them needs the field added.
11. **Item string quotes**: §1.5 step 2 is read as "cut at the next `"`"
    whether or not a leading quote was dropped.
12. **Expansion search over zero entries** selects none (unreachable:
    the slot ends on total 0 first).
13. **OQ7 "living"** players and party members: the caller supplies the
    counts.
14. Fatal errors of the original (0xF3A, 0xF44, 0xFEA, no ratio row,
    bonewall, > 65,534 TCs) are `TreasureError` values, not panics.

## Checks to queue (local run queue, `docs/HANDOFF.md` §5)

The coordinator should copy these into §5:

1. **Runtime form on the live set** (game files; d2-sim has no game-file
   loader dependency, so the test belongs in `crates/conformance` or a
   d2-sim dev-dependency on `d2-formats`): build `TreasureClasses` from
   the live typed records (`d2_data::bin::load` → `decode_all` for
   treasureclassex, itemtypes, weapons, armor, misc, uniqueitems,
   setitems; `equiv_matrix(itemtypes, ItemTypes)`) and check the spec's
   "Real 1.14d" vectors: 1,013 TCs, 160 automatic, 2,742 TC / 660 item /
   2 unique / 0 set entries, 0 notes, 81 `mul`; `Act 1 H2H A` = TC 430
   (group 12, picks 1, nodrop 100, entries 523/218/203/370 with starts
   0/21/37/58, totals 60/60); `ROP (N)`; `Act 1 Champ A` picks −2;
   `get(430, 40)` = 445, `get(430, 0)` = 430, `get(430, 85)` = 471,
   `get(0, 40)` = none; chest table all 45 found, `Act 1 Chest A` = 385;
   chest tiers (normal act 0: `lo` 1, `hi` 12, `s` 4; Hell act 4: `lo`
   0, `hi` 83, `s` 28). Also: the TC count equals
   `d2_data::bin::tc_count`.
2. **Memory dump** (already queued for the spec): compare the dumped TC
   array and chest table with `TreasureClasses` field by field.
3. **Recording** (already queued for the spec): replay recorded walk
   inputs (TC, `Q`, `L`, `F`, seed) through `walk` with a recording
   `DropSink` and compare draw order/count and the creation calls
   (id, quality, index, flags) and NoDrop `N` at `0x0055A9B9`.
