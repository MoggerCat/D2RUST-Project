# Spec: Tools — Game-state snapshots and their comparison (1.14d vs d2rs)

- **Status:** draft: format, the 1.14d field sources (every one cited
  from its owning spec) and the comparison rules; the recorder
  (`tools/trace-recorder/record_state.py`), the d2rs export
  (`d2_sim::debug::state`, `d2-client state-dump`) and the comparator
  (`tools/trace-recorder/state_diff.py`) implement it.
- **Target version:** 1.14d (the original side); the format is d2rs-own.
- **Crate/module:** `d2-sim::debug::state` (snapshot of a d2rs game);
  `d2-client` `state-dump` (headless run of the real-install game);
  `tools/trace-recorder/record_state.py`, `state_diff.py`;
  `tools/scenario-diff/` (one-command run of both sides).
- **Related specs:** `tools/original-hooks.md` §3 rule 3 (snapshot
  point), §4 (unit fields); `sim/unit-order.md` §2 (unit lists);
  `sim/path-placement.md` §2 (paths); `sim/units.md` §4 (animation);
  `sim/rng.md` §5.2–§5.3 (game and unit seeds); `sim/stat-lists.md` §1,
  `sim/stats.md` §1–§2 (stat arrays); `tools/scenario.md` (scenarios).

## Summary

One snapshot per server tick holds the game seed and every server unit
with the fields below. Both sides write the same JSON-lines format; the
comparator aligns the two files by game frame and prints the **first
divergence in game state** (frame, unit, field, both values), then the
next N, then per field the first frame it differs. A state divergence
is found at the tick it happens, not where it first changes a pixel
(an RNG-order slip shows as a unit seed at tick 1 instead of a draw row
hundreds of ticks later).

## Inputs

| Name | Type | Source |
|---|---|---|
| 1.14d game | process under the recorder's debugger | `game/Game.exe` (private data repo) |
| d2rs game | `d2_sim` state reached through the wired host | `d2-client state-dump` |
| two snapshot files | JSON lines, §1 | the two writers |

## Outputs / state changes

- Each writer: one file `traces/raw/<name>.<side>.state.jsonl`
  (gitignored; `<side>` = `orig` or `d2rs`).
- The comparator: a report on stdout; exit 0 match, 1 diverged,
  2 partial (no divergence; a field, a frame range or a side's gap not
  compared), 3 error (unreadable file, wrong format, no common frame).
- The d2rs export reads the game only: no RNG draw, no list change, no
  write (CLAUDE.md rule 6). Running it or not gives the same game.

## Rules

### 1. Format `state-1` (JSON lines, key `k`)

1. Line 1: `{"k":"header","format":"state-1","side":"orig"|"d2rs",
   "tool":"<name version>","date":"YYYY-MM-DD","command":"<argv>",
   "fields":[...],"gaps":[...]}`. `fields`: the unit fields of §2 this
   writer fills (others are absent from its units). `gaps`: free text,
   one entry per thing the writer knows it does not match (e.g. a field
   left out because no spec gives its address). The 1.14d writer adds
   `game_exe_sha256` and `args`; the d2rs writer adds `save` and `seed`.
2. One record per snapshot: `{"k":"snap","f":<frame>,"seed":[lo,hi],
   "units":[...]}`. `f` is the game frame of §3. `seed` the game seed
   (§2 table). `units` sorted by (`ut`, `g`) ascending.
3. Last line `{"k":"footer","snaps":<n>,"notes":[...]}`.
4. Numbers are JSON integers (u32 written unsigned, i32 signed, as the
   table says). A field a unit does not have is absent, never `null`
   (e.g. `x`/`y` of an item not on the ground).
5. A reader accepts `format` `state-1` only; unknown keys are ignored
   (later minor additions stay readable).

### 2. Unit fields

Key, meaning and the 1.14d source. Every 1.14d offset is owned by the
spec in the last column (`tools/original-hooks.md` §4 gathers them).
Unit = the unit record; path = unit +0x2C.

<!-- rows -->
| Key | Value | 1.14d read | Owner |
|---|---|---|---|
| `ut` | unit type 0–5 | u32 unit +0x00 | `sim/units.md` §1–§2 |
| `g` | GUID | u32 unit +0x0C | `sim/unit-order.md` §1 |
| `cl` | class (txt row) | u32 unit +0x04 | `sim/units.md` §2 |
| `m` | mode | u32 unit +0x10 | `sim/units.md` §1, §4 |
| `x`, `y` | sub-tile position | types 0, 1, 3: u16 path +0x02, +0x06; types 2, 4, 5: u32 path +0x0C, +0x10; absent when path = 0 | `sim/path-placement.md` §2.1–§2.3 |
| `xf`, `yf` | sub-tile fraction (low 16 bits of the 16.16 position) | types 0, 1, 3: u16 path +0x00, +0x04 | `sim/path-placement.md` §1 r2, §2.3 |
| `tx`, `ty` | path target | types 0, 1, 3: u16 path +0x10, +0x12 | `sim/path-placement.md` §2.3 |
| `d` | direction (0–63 for dynamic, the u8 as stored) | types 0, 1, 3: u8 path +0x64; types 2, 4, 5: u8 path +0x1C | `sim/path-placement.md` §2.2–§2.3 |
| `fr` | animation frame, 8.8 raw | i32 unit +0x44 | `sim/units.md` §4 (field table) |
| `fc` | animation frame count, 8.8 raw | i32 unit +0x48 | `sim/units.md` §4 |
| `sp` | animation speed | i16 unit +0x4C | `sim/units.md` §4 |
| `s` | unit seed `[lo, hi]` | u32 × 2 unit +0x20 | `sim/rng.md` §5.3 |
| `act` | act | u8 unit +0x18 | `sim/units.md` §2 |
| `lv` | level id of the unit's room | room (dynamic path +0x1C, static path +0x00) +0x10 → DRLG room +0x58 → level +0x1D0 | `drlg/rooms.md` §1, `drlg/levels.md` |
| `hp`, `hpx`, `mp`, `mpx`, `st`, `stx` | stats 6, 7, 8, 9, 10, 11 layer 0, raw (8.8) | full array of the extended list at unit +0x5C (`tools/original-hooks.md` §4 r3); absent key → 0; no list → field absent | `sim/stat-lists.md` §1, `sim/stats.md` §2 |
| `str`, `ene`, `dex`, `vit`, `lvl` | stats 0, 1, 2, 3, 12 layer 0 | base array of the list at unit +0x5C (`tools/original-hooks.md` §4 r5); absent key → 0; no list → absent | `sim/stat-lists.md` §1 |
| `own` | owner GUID | type 1 (pet, summon, minion, hireling): AI control (monster data +0x28; 0 → absent) with u32 control +0x28 ≠ 0 → u32 control +0x2C (absent when 0xFFFFFFFF, a released pack); type 3: u32 unit +0x98 when u32 unit +0xC8 bit 0x400, else absent; type 4: item data +0x5C inventory → u32 inventory +0x08 owner unit → u32 +0x0C (absent when either is 0, e.g. on the ground); types 0, 2, 5: absent | `sim/units.md` §2 (owner links), `monsters/ai.md` §3.1, `skills/bodies.md` §6.20, `items/inventory.md` §1.1 |
| `iq`, `if`, `fi`, `il` | item quality, item flags, file index (unique / set / superior index, i32), item level (items only) | u32 item data +0x00, +0x18, i32 +0x28, u32 +0x2C (item data = unit +0x14; absent when the pointer is 0) | `items/bitstream.md` Inputs (item data), `items/generation.md` Outputs |
| `aa`, `pf`, `sf`, `rp`, `rs` | auto affix, magic prefix ids ×3, magic suffix ids ×3, rare prefix, rare suffix (items only) | u16 item data +0x36, 3 × u16 +0x38, 3 × u16 +0x3E, u16 +0x32, u16 +0x34 (the rare pair is D2MOO's `D2ItemDataStrc`; no d2rs-side spec names the offsets: the first check that shows a rare item equal on both sides confirms them) | `items/bitstream.md` Inputs, `items/affixes.md` |
| `ik`, `ss` | item seed `[lo, hi]`, start seed (items only) | u32 × 2 item data +0x04, u32 +0x10 | `items/generation.md` §2 |
| `is` | the item's stat list: `[stat, layer, value]` per entry of the base array, in (stat, layer) order (items only; absent with no list) | the base array of the list at unit +0x5C (`stat-lists.md` §1); d2rs: `StatLists::base_entries` | `sim/stat-lists.md` §1 |

1. Which units: every unit of the server's unit lists (`sim/unit-order.md`
   §2 rule 4: the five per-type hash tables and the tile list), so
   client-side units never appear.
2. Stats are the raw stored values, not the unit-total reader (which
   applies the minimum rule, `sim/stats.md` §4.3).
3. The game seed: u32 × 2 at game +0xD0 (`sim/rng.md` §5.2 table:
   `pGameSeed`, inline in the game record).

### 3. Snapshot point and frame

1. The snapshot of frame N is the state **after tick N**: on 1.14d at
   the tick driver's return `0x0052FD1E` with ESI = game and game +0xA8
   = N (`tools/original-hooks.md` §3 rule 3); on d2rs after the host's
   tick that left `Game::frame` = N, before the next drain.
2. `f` = N. Both sides count frames the same way (`sim/tick.md` §2:
   0 at creation, +1 at the start of every tick), so frames align
   without an offset when both runs start the same game; the comparator
   still takes an explicit `--frame-offset K` (d2rs frame + K = 1.14d
   frame) for runs whose game starts differ.
3. A writer may snapshot every frame or every n-th; it records only the
   first game that ticks (single player).

### 4. Comparison

1. **Same run.** Both headers must be `state-1`; the comparator prints
   both `command` lines. It compares the frames present in both files
   (after the offset); no common frame is an error (exit 3).
2. **Fields.** A field is compared when both headers list it in
   `fields` and `--ignore` does not name it. A field on one side only is
   reported once as "not compared" (verdict at best partial).
3. **Order.** Frame by frame ascending. Within a frame: the game seed
   first, then the units in (`ut`, `g`) order over the union of both
   sides; a unit on one side only is a difference (`missing`: 1.14d
   has it, d2rs not; `extra`: the reverse); within a unit the fields in
   the §2 table order. `--types T,...` limits the unit types compared.
4. **Report.** The first divergence: frame, unit (`type:guid`, class on
   both sides), field, expected (1.14d) and got (d2rs), and both units'
   full records at that frame. Then the next `--next N` divergences
   (default 20) in the same order. Then, per field, the first frame and
   unit where it differs and how many (frame, unit) pairs differ. Then a
   summary: frames compared, units compared, fields not compared, both
   sides' `gaps`.
5. **Verdict.** `MATCH` (exit 0): no divergence, every field of either
   side compared, no gaps. `PARTIAL` (2): no divergence otherwise.
   `DIVERGED` (1): any divergence.
6. `--perturb FRAME:TYPE:GUID:FIELD` (self-test aid) changes that value
   on the d2rs side before comparing: the comparator must then report
   that exact place first.

## Constants & data dependencies

Offsets only (§2); no table is read by the snapshot.

## Randomness

None. The d2rs export reads seeds without stepping them.

## Edge cases & original bugs

1. A unit whose path is 0 (a unit in transit) has none of the path
   fields (`x`, `y`, `xf`, `yf`, `tx`, `ty`, `d`, `lv`) on 1.14d; d2rs
   must leave the same fields out for the same units
   (`sim/path-placement.md` §2.1). An item in an inventory is not such a
   unit: it has a static path whose `x`, `y` are its place (page cell,
   belt slot, body location) and `d` is 0 (recorded, `items-load-mixed`,
   2026-10-09); d2rs reads the place from the inventory model. A static path whose room
   (+0x00) is 0 has no `lv`.
4. `fc`, `sp` are the stored unit +0x48 / +0x4C even when a sequence
   animation (unit +0x30 ≠ 0) runs from +0x34 / +0x3C (`sim/units.md`
   §4): the snapshot records storage, not the value in use.
2. The 1.14d side snapshots in the debugger with the game stopped at
   `0x0052FD1E`: the driver holds the game lock, nothing changes while
   it reads (`tools/original-hooks.md` §3 rule 3).
3. GUIDs are per type (`sim/unit-order.md` §1): a unit is keyed by
   (`ut`, `g`), never by GUID alone.

## Test vectors

| Input | Expected |
|---|---|
| a file compared with itself | `MATCH`, exit 0 |
| a file compared with itself, `--perturb F:T:G:K` for every field of every unit of a synthetic file | `DIVERGED` first at exactly (F, T:G, K) |
| one unit removed on the d2rs side | `missing` at that unit's first frame |
| field listed on one side only | `PARTIAL`, the field named under "not compared" |
| d2rs export run twice on the same game | byte-identical files |

## Provenance

d2rs-own format. The 1.14d reads restate offsets owned by the specs in
the §2 table; nothing here is a new 1.14d fact.

## Open questions

1. ~~The owner of a unit has no 1.14d address in a spec.~~ Answered
   2026-10-09 (pc1-data Step 4 item 21): §2 `own` row, offsets owned by
   `sim/units.md` §2 "Owner links".
