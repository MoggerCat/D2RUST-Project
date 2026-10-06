# Spec: Data — Mod patch layers

- **Status:** draft; our own format (`d2patch 1`). Vectors rechecked
  2026-10-05 (Provenance). Implemented 2026-10-05 on branch
  `claude/patch-layers` (`d2-data::patch`, `data-tool patch`): every
  synthetic vector and both properties pass (`cargo test -p d2-data
  patch`). Verified on the 1.14d set 2026-10-06: `cargo test --release
  -p d2-data --test patch_game -- --ignored` passes G1–G8 (5 tests), and
  `data-tool patch check` on the Example stack exits 0 with the one N01
  note (data digest `66010ecda7c8df5b…`). Implementation readings of
  unstated details: open question 5.
- **Target version:** 1.14d
- **Crate/module:** `d2-data::patch`; CLI `data-tool patch`
- **Related specs:** `txt-format.md` (reader, binding, diagnostics),
  `field-types.md` (conversion, linkers, string keys), `loading.md`
  (sources, load checks, d2-data policy), `fields.tsv`, `tables.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 46–54 |
| Inputs | 55–58 |
| Outputs / state changes | 59–64 |
| Example | 65–121 |
| Rules | 122–123 |
|   1. Pipeline | 124–132 |
|   2. Tables and keys | 133–164 |
|   3. Syntax | 165–204 |
|   4. Selectors and columns | 205–218 |
|   5. Applying a stack | 219–253 |
|   6. Conflicts and drift | 254–266 |
|   7. Compile and checks | 267–284 |
|   8. Report | 285–295 |
|   9. Render, digests, diff | 296–331 |
|   10. Tools (`data-tool patch`, `anyhow`) | 332–342 |
|   11. Versioning and determinism | 343–354 |
|   12. Deferred | 355–361 |
| Constants & data dependencies | 362–365 |
| Randomness | 366–369 |
| Edge cases & original bugs | 370–377 |
| Test vectors | 378–473 |
| Provenance | 474–485 |
| Open questions | 486–509 |
<!-- /index -->

## Summary

Mods change game data only through patch layers (CLAUDE.md rules 8, 9):
row and cell statements applied in stack order to the user's own excel
`.txt` cells under `Ruleset::Mod`, then compiled by the verified compiler
(`crates/d2-data/src/compile*.rs`). Every `set` states the value it
replaces, so drift and conflicts are errors. Tools render patched tables
and turn an edited `.txt` into a layer.

## Inputs

Base tables (§2), `tables.tsv`, `fields.tsv`, stack and layer bytes.

## Outputs / state changes

Patched cell tables with provenance (row origins, cell writers), change
log, digests (§9), compiled set (§7); or the report only (§8). `apply`
reads no files, clock, environment or locale.

## Example

`mods/overhaul/overhaul.d2stack`:

```
d2stack 1
layer core/10-items.d2patch
layer core/20-balance.d2patch
```

`core/10-items.d2patch`:

```
d2patch 1
# Stronger hand axes, a new axe and a new magic prefix.

table weapons
set hax mindam 3 -> 4
set hax maxdam 6 -> 8
# The new axe copies the patched Hand Axe row.
add #306 ov1 like hax sha:527afb9693739d22
set ov1 level 3 -> 9
set ov1 mindam 4 -> 6
set ov1 maxdam 8 -> 11

table armor
# Two mindam columns; the compiler binds the first.
set buc mindam@1 1 -> 2

table magicprefix
set #1 Sturdy levelreq 3 -> 2
add #669 Keen like #12 Jagged sha:31284bff98e59f80
set Keen mod1min 10 -> 12
set Keen mod1max 20 -> 25

table treasureclassex
check [Act 1 Good] group 5
set [Act 1 Good] Picks 1 -> 2
```

`core/20-balance.d2patch`:

```
d2patch 1
# Written against base + 10-items.
table weapons
set hax mindam 4 -> 5
```

Result (G3): `hax` mindam 5, maxdam 8; #306 `ov1` level 9, 6–11, other
cells as `hax` at line 8; `buc` `mindam@1` 2, `mindam@2` 0; #1 levelreq
2; #669 `Keen` `dmg%` 12–25, rest from #12; #209 Picks 2. Report:
`note[N01] core/20-balance.d2patch:4:16: weapons hax (#0) mindam: replaces
[4] written by core/10-items.d2patch:5`. With `hax` mindam 2 in the base,
`10-items` fails: A06 5:16 (expected [3], found [2], base line 2), A13
8:23 (pin), N03 9:1 (3 skipped), N04.

## Rules

### 1. Pipeline

`Ruleset::Original` uses the live `.bin` set, no layers. `Ruleset::Mod`
stops after the first stage with errors: parse stack and layers (S, P);
read base tables (B); apply layers in order (A; the first failing layer
stops); compile and check (C). Layers edit `.txt` cells, never `.bin`
files or records (`loading.md` "d2-data policy" 4); every table compiles
from text, touched or not. Beyond Phase 2 "patches apply": §12.

### 2. Tables and keys

- **Tables:** one per distinct `txt_name` of a called `tables.tsv` row,
  named by its lowercase stem (85 in 1.14d; others A01). All lists of one
  txt (`levels`/`leveldefs`, `_lookup`) compile from the same cells.
- **Base:** `data\global\excel\<name>.txt` via the archive set, the file
  `compile_all` reads (`loading.md` §2, §11). Absent: B01; reader error
  (`txt-format.md` §2–§5): B02.
- **Cells:** header names (may repeat or be empty); row `i` = record `i`;
  cells are bytes compared exactly. Row origin `Base(line)` or
  `Added(layer, line)`; cell writer `Base`, or `Add`, `Copy`, `Set` with
  (layer, line); layers are named by stack path.

| Table | Key column | Kind | Policy |
|---|---|---|---|
| its txt feeds a list with a `key(code4)`, `key(name16)` or `key(name32)` entry (`tables.tsv` `key_column`) | the column that entry binds | `code` for `key(code4)`, else `name` | `unique`; `monseq` `multi` |
| `treasureclassex` | 0 (in `@tc`, `field-types.md` §6.4) | `name` | `unique` |
| others | 0 | — | `multi` |

- Kinds compare as linkers do (`field-types.md` §5.2–§5.3): `code` =
  first 4 bytes, space-padded, case-sensitive; `name` = first 31 bytes,
  ASCII-lowercased.
- **Scope:** uniqueness spans the tables filling one linker (`fields.tsv`
  `link`; `weapons`, `armor`, `misc` share `items.code`). `unique`: no new
  duplicate in the scope, no empty key (A12); base duplicates are
  tolerated and addressed by index.
- B03: the key entry binds no column, or another column's name equals the
  key column's ignoring case (none in 1.14d).
- **Rows:** `fixed` (`add` is A10) where `loading.md` §8 or §10.8 pins the
  count: `arena`, `armtype`, `belts`, `composit`, `difficultylevels`,
  `experience`, `inventory`. Else `append`: rows only at the end.

### 3. Syntax

- **Bytes:** 0x20–0x7E, LF, CR before LF; any other byte (TAB, BOM, lone
  CR…) is P01 at line and byte column, leftmost, one per line. A file over
  16 MiB: P12 at 0:0.
- **Lines** split at LF (CR LF = LF; an unterminated last line counts).
  Blank lines and comments (`#` first non-space byte) only get P01. A
  statement line over 4,096 bytes: P12 at 4,097.
- **Tokens,** split on spaces: `[` starts a bracket string, the exact
  bytes to the next `]` (`[]` empty; line end or `[` first: P06 at the
  `[`); `]` at a token start: P10; else a bare token (no space, `[`, `]`).
  A token must end at a space or the line end (else P10 there); over 1,024
  bytes: P12. Keywords, `->`, `like`, indices, pins and table names are
  bare; elsewhere `axe` = `[axe]`, and values with spaces, edge spaces or
  no bytes need brackets.

```
d2patch 1                        line 1, else P02 at 1:1; other version P03
table <tname>                    [a-z0-9]{1,32}, else P08
set   <sel> <col> <old> -> <new>
check <sel> <col> <value>
add   <index> <key> [like <sel> [<pin>]]
sel   = <index> <label> | <key>  index form iff bare and starting with #
index = #0 | #[1-9][0-9]*        <= 4294967295, else P07
pin   = sha: + 16 [0-9a-f]       else P11
```

- P04: not a bare keyword (also deferred `remove`, `addcol`, `delcol`).
  P09: a statement before the first `table`. P05: a missing token (at the
  column after the last non-space byte), an extra token, `->` or `like`
  expected, an `add` index not a bare `#…` token. `[#5]` is key `#5`.
- If line 1 fails, other lines report only P01. One P error per line, the
  first of: P01, P12, token errors, P04, P09, shape errors.
- **Stack file:** same rules; line 1 `d2stack 1` (S01 at 1:1; version:
  S02); then blanks, comments or `layer <path>` (else S03 at the first
  token). Path: relative, `/`-separated segments `[a-z0-9_.-]+` (not
  empty, `.`, `..`), ending `.d2patch`, ≤ 255 bytes (else S04). Repeat:
  S05. Unreadable: S06 (all three at the path). 1,025th layer: S07. Layer
  order = line order.

### 4. Selectors and columns

Resolved when the statement runs, against the current table.
- **Key `K`:** A14 if `K` is in `R_T`; rows keyed exactly `K`: none A03,
  several A04 (related: up to 10), else that row.
- **Index `#n L`:** A03 if `n` ≥ row count; A05 if row `n`'s key is not
  exactly `L` (found: it; related: up to 5 rows keyed `L`).
- **Column `C`:** with `@`, the name precedes the last `@` and `k`
  (`[1-9][0-9]*`) follows. `m` = columns named exactly so. A02 if `m` is
  empty, `|m|` > 1 without `k`, `k` with `|m|` = 1, `k` > `|m|`, or `k`
  malformed. **Canonical:** the name if unique, else `name@k`.
- **`R_T`:** the old and new texts of the layer's `set`s on `T` whose
  column token text is the key column name.

### 5. Applying a stack

Per layer: `W` (cells written by `set`s, added key cells), `TP` (template
rows), `KW` (key writes: scope, value, location). The first failing check
is the statement's error; it then has no effect. Locations in parentheses.

- **`table T`:** A01 if not patchable (name).
- **`set S C O -> N`:** resolve `S` (first token), `C` (column); A08 cell
  in `W` (column); A15 row in `TP` (selector); A06 cell ≠ `O` (old;
  expected, found, writer); A07 `O` = `N`; A11 column 0 and `N` =
  `Expansion`; A12 `C` is a `unique` key column and `N` is empty (A07,
  A11, A12: new). Effect: cell = `N`, writer `Set`, into `W`; change log
  (layer, line, table, index, key, column, old, new); key column: into
  `KW`. N01 if the previous writer was another layer's (old). N02 if no
  list of this txt binds `C` (column).
- **`check S C V`:** resolve; A06 if the cell ≠ `V` (value).
- **`add #n K [like S [P]]`:** A10 `fixed` (keyword); A09 `n` ≠ row count
  (index); with `like`, resolve `S` and A13 if `P` ≠ its pin (pin); A11
  key column 0 and `K` = `Expansion`; A12 `unique` and `K` empty (A11,
  A12: key). Effect: row `n`, origin `Added`; cells empty (writer `Add`)
  or copies of the template's current cells (`Copy`); key cell `K`
  (`Add`) into `W`; template into `TP`; key into `KW`.
- **Pin:** `sha:` + first 16 hex digits of SHA-256 over, per non-empty
  cell in column order: canonical reference, 0x09, cell, 0x0A.
- **Structural:** `table`, `add`, a `set` whose column token text is the
  key column name. When one fails, the layer's later statements on that
  table are skipped, with one N03 (count) at the first.

Per layer: compute `R_T`, clear `W`, `TP`, `KW`, run the statements in
order. Then for each `KW` entry, `k` = its normalized value: if the scope
has more than `max(1, b)` rows keyed `k` (`b`: at layer start) and `k` is
unreported, A12 at the entry (related: (table, index) pairs). Checking at
the end allows swaps. Any A error stops the stack; N04 (line 0) names the
layers not applied.

### 6. Conflicts and drift

| Situation | Outcome |
|---|---|
| Old value differs (base drift, or an earlier layer) | A06, naming the writer |
| Old value is an earlier layer's | allowed; N01 |
| Row missing, ambiguous, relabelled; template changed | A03–A05; A13 |
| Appending layers reordered | A09 |
| A key change breaks a reference | C02 per cell |

Errors only, never warnings. Each layer is authored against the state
after the earlier ones (`render --upto`, `diff`).

### 7. Compile and checks

1. Run `compile_set::compile_all` with a reader returning, per `txt_name`,
   the base archive and `render(T)` of the patched table; compile the
   unpatched base the same way. Row `i` is line `i + 2`: errors and
   diagnostics (`txt-format.md` §9) map to (table, row, column). Add
   `StrMiss` for each non-empty `strkey` cell in no string table
   (`field-types.md` §7).
2. **C01:** `compile_all` fails (E1–E14). **C02:** a diagnostic with none
   of its kind at the same table, row and column in the base compile
   (added rows: always); related: the writer, or for a `Base` cell the
   key-column `set`s on the tables filling the field's linker. **C03:** a
   live-set check fails (`loading.md` §8, §10.8); related: the table's
   `add`s.
3. Match by row, never line (base files skip a line per removed
   `Expansion`). The compiled records then replace the live `.bin`
   records, fix-ups included (`loading.md` §7.4).

### 8. Report

Fields (normative, unlike wording): code; severity (N notes, others
errors); file (layer stack path, stack file, or
`base:<archive>:<path>`); line, col (1-based, 0 if none); table; row
(index, key); column (canonical); expected; found; writer (`base` + line,
or `layer:line`); related. Human form `<severity>[<code>]
<file>:<line>:<col>: <table> <key> (#<index>) <column>: <detail>`, values
bracketed. Order: S; P (stack position, line, column); B (table); A and N
(stack position, line, column, code; N04 last); C (table, row, column).

### 9. Render, digests, diff

- **`render(T)`:** header then rows, cells joined by TAB, lines ending CR
  LF, no `Expansion` line. `render(read(f))` = `f` minus `Expansion`
  lines; `read(render(T))` = `T`.
- **Digests:** table = SHA-256 of `render(T)`, hex; data = SHA-256 over,
  per table by name: name, 0x09, table digest, 0x0A.
- **Canonical token:** bare if non-empty, bytes 0x21–0x7E without `[`,
  `]`, no leading `#`, not `->` or `like`; else bracketed.

**Diff** (before table `B`, edited bytes `E`): a canonical layer, or the
first D error:
1. Reader error: D04. Header ≠ `B`'s: D02. Rows `X`: `|X|` < `|B|` D01;
   `|X|` > `|B|` and `fixed` D06.
2. D05 at the first `i` < `|B|` with `X_i` ≠ `B_i` where a `unique` key
   changed to another base row's key (normalized), or `X_i` = `B_(i±1)`:
   spreadsheet inserts, deletes, sorts (swaps are hand-written).
3. D06 (`unique`): a normalized key more than `max(1, count in B)` times
   in `X`; an empty key in a new row or where `B_i`'s was not empty.
4. D03: a needed value over 1,024 bytes, outside 0x20–0x7E, or with `[`,
   `]`.
5. `R` = old and new keys of renamed rows. Emit:
   - per changed `i` < `|B|`, ascending: `set <sel> <col> <B_i[c]> ->
     <X_i[c]>` per differing column in header order, key last; `sel` =
     `key(B_i)` if unique in `B` and not in `R`, else `#i key(B_i)`.
   - per new row `n`: `add #n key(X_n)`, plus `like <tsel> <pin of X_t>`
     if a row `t` < `|B|` shares ≥ 1 and ≥ half of `X_n`'s non-empty
     non-key cells (most, then lowest `t`; not with `--no-like`); `tsel` =
     `key(X_t)` if unique in `X_0…X_(n−1)` and not in `R`, else `#t
     key(X_t)`. Then `set #n key(X_n) <col> <old> -> <new>` per non-key
     cell differing from the template (or from empty).
6. Output `d2patch 1` LF, and with lines: an empty line, `table <name>`,
   the lines (LF each). Several tables: one section each, by name.
- **Round trip:** applying the output after `B` gives rows `X` exactly.
  Layers hold single base values, never whole base rows.

### 10. Tools (`data-tool patch`, `anyhow`)

- `check <stack>`: parse, apply, compile; print report and data digest.
- `render <stack> [--base | --upto <layer>] <table>…`: write `render(T)`
  to `game/mod-render/<stack stem>/<table>.txt`.
- `diff <stack> (--base | --upto <layer>) <table>=<edited.txt>…
  [--no-like] [-o <file>]`: §9 diff from that state.

Game files from `D2_GAME_DIR`. Exit 0 no error, 1 errors, 2 usage or I/O.
Rendered tables go only under `game/`.

### 11. Versioning and determinism

- Line 1 names the version; the engine reads 1 to its current one (newer:
  P03, S02). Unknown statements are never ignored.
- A change that can invalidate a valid file or change its result needs a
  new version with a text-to-text migration; one that only validates
  invalid files (deferred statements, `fixed` → `append`) does not.
- `apply`, `render`, `diff` are pure: no hash-map order, directory order,
  locale, time, threads or floats.
- **Order independence:** permuting a valid layer gives identical cells
  or fails (A03, A08, A09, A14, A15).

### 12. Deferred

`remove`; `addcol`/`delcol` (extension columns, field columns a txt lacks,
`schema.md` §5); string-table layers (a new string key is C02);
`soundenviron.txt` (not in `tables.tsv`), runtime reads of patched
`sounds`; `fmt`, `blame`, lock files; protocol use of digests (Phase 7).

## Constants & data dependencies

Limits in §3; 1,024 layers; path 255 bytes.

## Randomness

None.

## Edge cases & original bugs

1. Duplicate columns (`armor` `mindam`/`maxdam` at 63/64 and 161/162,
  `automap` `Type2`, `chartemplate` `SkillName`): the leftmost binds, so
  `mindam@2` is N02. `weapons` column 18 is `[]`.
2. Appending to `weapons` shifts the combined index of every `armor` and
  `misc` row (`loading.md` §9); links resolve by key.

## Test vectors

Digests: first 16 hex digits. Cells separated by `;`.

**Fixture:** `items` = `name code lvl dam dam *note`, key `code`,
code/unique/append: #0 `Axe;axe;1;3;0;`, #1 `Club;clb;1;2;0;old`, #2
`Axe;ax2;5;7;0;`, #3 `Big Club;clb;9;8;1;`. `gear` = `name code`, key
`code`, code/unique/fixed, same scope: #0 `Cap;cap`, #1 `Belt;blt`.
`recipes` = `description enabled output`, key column 0, text/multi/append:
#0 `A;1;"hp1,qty=3"`, #1 `B;1;x`, #2 `C;0;y`. Unbound: `dam@2`, `*note`.
Digests: items `2c6f65f2ec858aad`, data `29a76856ffd1a6a9`. Pins: items
#0 (stream `name\tAxe\ncode\taxe\nlvl\t1\ndam@1\t3\ndam@2\t0\n`)
`sha:84ec3f726bd38615`, #3 `sha:bddc5549c005d720`.

**Parse** (line 3, after `d2patch 1`, `table items`): `set axe lvl [1] ->
[4 ]`: new `4 `. P10: `set axe lvl[1] -> 4` 3:12. P07: `set #01 axe lvl 1
-> 4` 3:5. P06: `set axe [lvl 1 -> 4` 3:9. P05: `set axe lvl 1 -> 4 5`
3:20, `set axe lvl 1 ->` 3:17, `add #4 spr like #3` 3:19. P04: `Set axe
lvl 1 -> 4` 3:1. P11: `add #4 spr like axe sha:0123` 3:21. P01: TAB
after `set axe lvl 1 -> 4` 3:19. P12: a 1,025-byte token after `set axe
lvl 1 -> ` 3:18. Files: `d2patch 2` P03 1:9;
`D2PATCH 1` P02 1:1; `d2patch 1\nset axe lvl 1 -> 4` P09 2:1.

**Apply** (`a.d2patch`, line 2 `table items` unless named; `/` separates
lines; "applies" = no error or note unless shown):

| # | Statements | Result |
|---|---|---|
| V1 | `set axe lvl 1 -> 4` | #0 `Axe;axe;4;3;0;`, `Set(a,3)`; `13c4b643f616f070` |
| V2 | `set clb lvl 1 -> 4`; `set #1 axe lvl 1 -> 4`; `set AXE lvl 1 -> 4` | A04 3:5 (1, 3); A05 3:5; A03 3:5 |
| V3 | `set axe lvl 2 -> 4`; `set axe lvl 1 -> 1`; `set axe name Axe -> Expansion` | A06 3:13 (found `1`); A07 3:18; A11 3:21 |
| V4 | `set axe dam 3 -> 4` | A02 3:9; `dam@1 3 -> 4` applies; `dam@2 0 -> 1`: N02 3:9 |
| V5 | `set axe lvl 1 -> 4` / `set axe lvl 4 -> 5` | A08 4:9 |
| V6 | `add #4 spr` / `set spr name [] -> Spear` / `set #4 spr lvl [] -> 2` | #4 `Spear;spr;2;;;`; `d530958cb6927ea0` |
| V7 | `add #5 spr`; `set #4 spr lvl [] -> 2` / `add #4 spr` | A09 3:5; A03 3:5 |
| V8 | `add #4 [axe ]`; `add #4 []`; `add #4 cap` | A12 3:8 (last: gear 0, items 4) |
| V9 | `set #2 ax2 code ax2 -> axe`; the same after `set #0 axe code axe -> ax2` | A12 3:24 (items 0, 2); applies (swap) |
| V10 | `set ax2 code ax2 -> ax3`; `set #2 ax2 code ax2 -> ax3` / `set ax3 lvl 5 -> 6` | A14 3:5; A14 4:5 |
| V11 | `add #4 spr like axe sha:84ec3f726bd38615` / `set spr name Axe -> Spear` | #4 `Spear;spr;1;3;0;`; `a60483e64d8a3910`; pin `sha:0000000000000000`: A13 3:21 |
| V12 | `add #4 spr like axe` / `set axe lvl 1 -> 2` | A15 4:5; reversed: #4 `Axe;spr;2;3;0;` |
| V13 | gear: `add #2 x` / `set x name [] -> X` | A10 3:1; N03 4:1 (1) |
| V14 | recipes: `add #3 A` / `set A enabled 1 -> 0` | A04 4:5; reversed applies |
| V15 | one: `set axe lvl 1 -> 4`; two: `set axe lvl 1 -> 6`; or two: `… 4 -> 6` | A06 two:3:13 (writer `one.d2patch:3`); N01 two:3:13 |
| V16 | a: `add #5 spr` / `set spr lvl [] -> 2` / `set axe lvl 1 -> 2`; b: any | A09 a:3:5; N03 a:4:1 (2); N04 |
| V17 | `set axe lvl 9 -> 2` / `set ax2 lvl 5 -> 6` / `set clb lvl 1 -> 2` | A06 3:13, A04 5:5 |
| V18 | `table nosuch` / two `set`s / `table items` / `set axe lvl 1 -> 2` | A01 2:7; N03 3:1 (2) |
| V19 | `set #4 clb code clb -> x` / `set axe lvl 1 -> 2` | A03 3:5; N03 4:1 |
| V20 | `add #4 x1` / `set #4 x1 code x1 -> x2` | A08 4:11 |

**Stack:** one: `set axe lvl 1 -> 4` / `set #1 clb lvl 1 -> 7`. two: `set
axe lvl 4 -> 5` / `add #4 spr like axe sha:73706f637e0f52e5` / `set spr
name Axe -> Spear` / (empty) / `table recipes` / `set A output
"hp1,qty=3" -> "hp1,qty=5"`. Report: N01 two:3:13. Items #0 lvl 5, #1 lvl
7, #4 `Spear;spr;5;3;0;`; recipes #0 output is the 11 bytes
`"hp1,qty=5"`; items `a564579cb18890b5`, data `22b98c96c8a9960d`.
Stack files: a repeated `layer a.d2patch` on line 3: S05 3:7; `layer
../a.d2patch` on line 2: S04 2:7.

**Diff** from base items, edited rows `Axe;axe;4;3;0;`, `Club;clb;1;2;0;`,
`Axe;ax3;5;7;0;`, `Big Club;clb;9;8;1;`, `Spear;spr;4;3;0;`,
`Mace;mce;3;;;`:

```
d2patch 1

table items
set axe lvl 1 -> 4
set #1 clb *note old -> []
set #2 ax2 code ax2 -> ax3
add #4 spr like axe sha:da7b748ddf3353a9
set #4 spr name Axe -> Spear
add #5 mce
set #5 mce name [] -> Mace
set #5 mce lvl [] -> 3
```

It reproduces the edit (N02 5:12; `bea8bb33aac58fd2`). Unchanged:
`d2patch 1\n`. Rows 0–2 only: D01. `*note` renamed: D02. A cell `A]x`:
D03. A row inserted at 1: D05 row 2. A new row coded `axe `: D06.

**Game files** (`#[ignore]`):

| Id | Input | Expected |
|---|---|---|
| G1 | all 85 tables | 21,632 rows; render identity; no-edit diff = `d2patch 1\n` |
| G2 | empty stack | compiles, no C finding |
| G3 | the Example | weapons `ef2ca1073f89cf91`, armor `a3bd13b18286ee12`, magicprefix `63b952e7aa3c56f0`, treasureclassex `0a0a4c940f8f3fb9` |
| G4 | monstats `set cr_lancer8 Level 5 -> 6`; armor `set buc mindam 1 -> 2`; armor `add #202 hax` | A04 (617, 723); A02 (63, 161); A12 (`items.code`) |
| G5 | treasureclassex `add #853 [act 1 good]`; skills `set [fire bolt] reqlevel 1 -> 2` | A12 (209); A03 |
| G6 | weapons `set hax type axe -> zzz`; `set hax namestr hax -> nosuchkey` | C02 LinkMiss; C02 StrMiss |
| G7 | itemtypes `set #28 axe Code axe -> zz9` | C02 at `Base` cells (weapons `type`, affix `itype*`, …), related: that `set` |
| G8 | gamble `add #125 X` | C03 (empty code is no item) |

**Properties:** §11 order independence and the §9 round trip on seeded
random input; the parser never panics.

## Provenance

- Design: text cells (`.bin` keeps name keys only as indices; compiled
  text equals the live bins); one mandatory old value, so errors replace
  warnings; append-only rows (indices are identities); A08, A14, A15 and
  end-of-layer uniqueness make valid layers order independent.
- Checked 2026-10-05 with uncommitted scratch code: a Python model gave
  the synthetic vectors and G1, G3–G5 (71 permutations identical, 3,097
  rejected, 0 different; 600 random diffs round-trip or reject); a Rust
  run of `compile_all` gave G2 (C01, C02), G3, G6, G7; G8 follows from
  `loading.md` §8.

## Open questions

1. `append` is a default; new rows in engine enumerations (`charstats`,
   `plrmode`, …) are unexamined.
2. Mod compiles lack the `monstats`, `monstats2`, `monpreset`, `cubemain`
   callback bytes (`field-types.md` §8.3) and the #707 `NameStr` policy.
3. `treasureclassex` rows after the empty #852 are no TCs, and a name
   equal to an automatic TC (`bow3`) is shadowed (`loading.md` §10.6).
4. Layers hold single base values: confirm this fits rule 9.
5. Details the rules leave open, implemented as follows (2026-10-05,
   `claude/patch-layers`); confirm or restate: (a) the 1,024-byte token
   limit counts a bracket token's content, not its brackets; (b) line 1
   `d2patch <v>` / `d2stack <v>` with `v` ≠ `1` is P03 / S02, any other
   line 1 P02 / S01; (c) a `table` line with a shape error still ends
   the P09 zone; (d) a `layer` line with a missing or extra token is S03,
   a bracketed path S04; S07 is at the path; (e) N03 counts every
   skipped statement, a repeated `table T` included, at its keyword; (f)
   A12 end-of-layer related rows and A04/A05 related rows are
   `<table> <index>`, sorted by table name then index (V8 lists `gear 0,
   items 4`); A02 related are the columns named exactly so (G4); (g) C
   findings name the base file, the row's base line (0 for added rows)
   and column + 1; a code buffer that fails validation is also C03; (h)
   D findings name the edited row (`line` = row + 2).
