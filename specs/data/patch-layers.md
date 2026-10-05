# Spec: Data — Mod patch layers

- **Status:** draft. The format `d2patch 1` is our own design, not
  reverse-engineered. The 1.14d facts it relies on are confirmed (see
  Provenance). A scratch Python model of every rule in this spec produced
  the test vectors and passed the stress tests in §19. Not implemented.
- **Target version:** 1.14d
- **Crate/module:** `d2-data::patch` (lexer, parser, apply, render, diff,
  fmt), `d2-data::patch::catalog` (per-table entries). CLI: `tools/d2patch`.
- **Related specs:** `specs/data/txt-format.md` (reads a `.txt` into cells:
  the base reader), `specs/data/field-types.md` (compiles cells into
  records; linkers; string keys), `specs/data/loading.md` (live sources,
  load order, post-load checks, combined index spaces). Per-table specs
  (still to write) fill catalog entries (§3), field lists and invariants.

## Summary

A mod changes game data only through patch layers (CLAUDE.md rules 8 and
9). A layer is a text file of row and cell operations. d2-data applies an
ordered stack of layers to the user's own tables at load time. Layers work
at the `.txt` level: cells are named by txt column names, and rows by the
table's key column or by row index plus key. Every `set` states the value
it replaces, so drift in the user's base and conflicts between layers are
errors, each with a file, line, column, row and cell. After the last layer,
d2-data compiles every table from the patched text with the 1.14d compiler
rules. That re-resolves every cross-reference, and each broken reference
is blamed on the statement that caused it. Tools render patched tables
back to `.txt`, generate a layer from an edited `.txt` (diff), and
normalize layer files (fmt).

## Inputs

| Name | Type | Source |
|---|---|---|
| base tables | cell tables (§2) | the user's archives, read with `txt-format.md`; sources per `loading.md` §11 |
| catalog | one entry per patchable table (§3) | `d2-data::patch::catalog`, filled by per-table specs |
| stack | `.d2stack` bytes (§6) | mod folder |
| layers | `.d2patch` bytes, keyed by their stack path (§5) | mod folder |

## Outputs / state changes

- On success: the final cell tables of all catalog tables, provenance of
  every column, row and cell, a change log, a removal log, a column log,
  notes, and digests (§12).
- On failure: the report only (§13). A partially patched table is never
  used.
- `apply` takes bytes and returns a value. It reads no files, clock,
  environment or locale. The tools (§17) write only under `game/`, which
  is gitignored.

## Example

Stack file `mods/overhaul/overhaul.d2stack`:

```
d2stack 1
# Application order, first to last. Paths are relative to this file.
layer core/10-items.d2patch
layer core/20-balance.d2patch
```

`core/10-items.d2patch` (line numbers matter for the results below; line 1
is `d2patch 1`):

```
d2patch 1
# core/10-items.d2patch
# Stronger hand axes, a new axe, a new magic prefix and a new cube recipe.

table weapons
set hax mindam 3 -> 4
set hax maxdam 6 -> 8
addcol ovr:tier
set hax ovr:tier [] -> 1
# The new axe starts as a copy of the patched Hand Axe row.
add #306 ov1 like hax sha:68ecce7fb3eab322
set ov1 name [Hand Axe] -> [Bearded Axe]
set ov1 mindam 4 -> 6
set ov1 maxdam 8 -> 11
set ov1 ovr:tier 1 -> 2

table armor
# armor.txt has two mindam columns; @1 is the one the compiler reads.
set buc mindam@1 1 -> 2

table charstats
set Amazon [#walk] 6 -> 7

table monstats
# The Id cr_lancer8 names rows #617 and #723, so the row needs its index.
set #723 cr_lancer8 Level 5 -> 6

table magicprefix
# Sturdy names rows #1, #142 and #143.
set #1 Sturdy levelreq 3 -> 2
add #669 Keen like #12 Jagged sha:31284bff98e59f80
set Keen mod1min 10 -> 12
set Keen mod1max 20 -> 25

table treasureclassex
check [Act 1 Good] group 5
set [Act 1 Good] Picks 1 -> 2

table cubemain
add #151 [Ovr: 2 flawed amethysts -> 1 amethyst]
set [Ovr: 2 flawed amethysts -> 1 amethyst] enabled [] -> 1
set [Ovr: 2 flawed amethysts -> 1 amethyst] version [] -> 100
set [Ovr: 2 flawed amethysts -> 1 amethyst] numinputs [] -> 2
set [Ovr: 2 flawed amethysts -> 1 amethyst] [input 1] [] -> "gfv,qty=2"
set [Ovr: 2 flawed amethysts -> 1 amethyst] output [] -> gsv
```

`core/20-balance.d2patch`, written against base + `10-items`:

```
d2patch 1
# core/20-balance.d2patch: written against base + 10-items.
table weapons
set hax mindam 4 -> 5
```

Result on the 1.14d tables (game-file vector G3):

| Table | Result |
|---|---|
| weapons | 307 rows, 167 columns; the last column is `ovr:tier`. #0 `hax`: mindam 5, maxdam 8, ovr:tier 1. #306 `ov1`: name `Bearded Axe`, mindam 6, maxdam 11, ovr:tier 2; every other cell is copied from `hax` as it was at line 11 (namestr `hax`, type `axe`, level 3, normcode `hax`) |
| armor | #22 `buc`: `mindam@1` 2, `mindam@2` still 0 |
| charstats | `Amazon` `#walk` 7 |
| monstats | #723 Level 6; #617 (also `cr_lancer8`) Level still 5 |
| magicprefix | 670 rows. #1 levelreq 2. #669: Name `Keen`, mod1code `dmg%`, mod1min 12, mod1max 25, itype1 `weap` (the rest copied from #12 `Jagged`) |
| treasureclassex | `Act 1 Good` (#209) Picks 2 |
| cubemain | 152 rows. #151: enabled 1, version 100, numinputs 2, `input 1` = the 11 bytes `"gfv,qty=2"` (quotes included), output `gsv`; all other cells empty |
| report | one note: `note[N01] core/20-balance.d2patch:4:16: weapons hax (#0) mindam: replaces [4] written by core/10-items.d2patch:6` |

If a user's base had `hax` mindam 2, `10-items` would fail instead
(illustration):
`error[A06] core/10-items.d2patch:6:16: weapons hax (#0) mindam: expected [3], found [2] (base patch_d2.mpq:data\global\excel\weapons.txt line 2)`.

## Rules

### 1. Pipeline

1. **`Ruleset::Original`:** no layers. d2-data loads the live `.bin` files
   (`loading.md` §3). The patch engine does not run.
2. **`Ruleset::Mod`:**
   1. Parse the stack and every layer (§4–§6). Any S or P error stops here.
   2. Read the base text of every catalog table (§2). Any base error (B
      code) stops here.
   3. Apply the layers in stack order (§7–§9). The first layer with an A
      error stops here.
   4. Compile every catalog table from its final cells with
      `field-types.md`, in `loading.md` §6 order. `leveldefs` is compiled
      from the final `levels` table. `sounds` and `soundenviron` are parsed
      from their final cells.
   5. Run the reference pass, the post-load checks of `loading.md` §8 and
      the per-table invariants (§11).
   6. Any error: the result is the report only.
- Under `Ruleset::Mod`, every table is compiled from text, including
  tables no layer touches. No index or value read from a `.bin` is used.
  An implementation may load an untouched table from its `.bin` only as an
  optimization whose output is proven byte-identical by a test.
- Layers never patch `.bin` records or typed structs. Reasons (1.14d):
  1. `.bin` records store name keys (`key(name16)`, `key(name32)`) as row
     indices, not text (`field-types.md` §3). For 18 of the 21 name-key
     tables (for example monstats `Id`, skills `skill`), no key text occurs
     anywhere in the `.bin`.
  2. Comment columns are not compiled at all.
  3. Compiling each live `.txt` reproduces its live `.bin` byte for byte
     for 82 tables (`field-types.md` §10). Patched text therefore compiles
     to exactly what 1.14d would compile.

### 2. Cell tables

- **Table name:** the lowercase stem of `data\global\excel\<name>.txt`,
  matching `[a-z0-9]{1,32}`. Only names with a catalog entry (§3) are
  patchable.
- **Base text:** for each catalog table, the first of `patch_d2.mpq`,
  `d2exp.mpq`, `d2data.mpq` that contains `data\global\excel\<name>.txt`
  (MPQ names are case-insensitive, `mpq.md` §3). This is the source order of
  `loading.md` §2 and §11, including its cross-archive sources: `automagic`,
  `rareprefix` and `raresuffix` come from `d2exp`; `inventory` and
  `plrmode` come from `patch_d2`. No `.txt` found: B02.
- **Reading:** `txt-format.md` §2–§5. The header gives the column names.
  The records are the data lines left after removing lines whose first cell
  is exactly `Expansion`. Reader errors E1–E10 are B03 (with the E code).
- **Model.** A cell table holds:
  - columns: an ordered list. Each column has a name (bytes) and an origin:
    `Base` or `AddCol(layer, line)`. Names may repeat and may be empty.
  - rows: an ordered list. Each row has exactly one cell per column and an
    origin: `Base(index, source line)` or `Added(layer, line)`.
  - cells: byte strings with no TAB, CR or LF. There is no trimming, no
    case folding and no number normalization: `0`, `` and `00` are three
    different cells.
  - writers: each cell has one writer: `Base`, `Add(layer, line)`,
    `Copy(layer, line)`, `Set(layer, line)` or `AddCol(layer, line)`.
- **Row index:** row `i` (0-based) is record `i` of the compiled table. In
  a rendered `.txt` (§14), row `i` is on line `i + 2`. In the original 1.14d
  files, rows after an `Expansion` line sit one line lower. Messages give
  both the index and the base source line.
- `layer` in provenance is the layer's path as written in the stack (§6).
- **1.14d (measured):** all 86 catalog tables read without error; every row
  has exactly as many cells as the header; 21,682 rows in total; widest
  header 256 columns (`skills`).

### 3. Catalog (row keys and table policies)

Every patchable table has one catalog entry. A `table` statement naming a
table without an entry is A01.

| Field | Values | Meaning |
|---|---|---|
| `key` | a column name | The table's key column: used by key selectors, as the label of index selectors, and set by `add`. It must occur exactly once in the base header (B01) |
| `kind` | `code`, `name`, `text` | How keys compare for uniqueness (A16, D07). `code`: the first 4 bytes, padded with 0x20 to 4, case-sensitive (`field-types.md` §5.2). `name`: the first 31 bytes, `A`–`Z` mapped to `a`–`z` (§5.3). `text`: all bytes, exact |
| `policy` | `unique`, `multi` | `unique`: a layer may not add a duplicate key or write an empty key. Duplicates already in the base are tolerated. `multi`: no key rule |
| `rows` | `fixed`, `append`, `free` | `fixed`: no `add`, no `remove`. `append`: `add` only. `free`: `add` and `remove` |
| `addable` | list of field names | Non-extension column names that `addcol` may add (§8). Default: empty |
| `fields` | field list, or none | The table's field list (per-table spec). Used only for note N02 |
| `status` | `provisional`, `frozen` | §18 |

**Choosing the key** (the rule per-table specs apply):

1. If the table's 1.14d field list has an own-key field (vocabulary
   `key(code4)`, `key(code1)`, `key(code2)`, `key(str(N))`, `key(name16)`
   or `key(name32)` in `field-types.md` §3), the key is the column that
   field binds. `kind` is `code` for the code types and `name` for the
   others. `policy` is `unique`, except when one key per several rows is the
   table's design (`monseq`: all frames of a sequence share its name):
   then `multi`.
2. Otherwise the key is column 0, `kind` `text`, `policy` `multi`. A table
   whose spec defines a hand-built linker on that column is `unique`
   (`treasureclassex`: treasure-class names, `field-types.md` §6.4).
3. A per-table spec may choose another column. It must say why.

Key selectors compare exact bytes (§7). `kind` matters only for
uniqueness, so a layer cannot create two keys the game's linker would
merge: `[axe ]` collides with `axe` in a `code` table, `FIRE BOLT` with
`Fire Bolt` in a `name` table, and `HAX` does not collide with `hax`
(codes are case-sensitive in 1.14d).

**Rows policy:**

- Default `append`. Appending never moves an existing row.
- `fixed` where the 1.14d loader requires an exact count (`loading.md` §8:
  `inventory` = 32, `difficultylevels` = 3, `belts` 14 or 15), or where the
  table's spec says so.
- `free` only when the table's spec proves that nothing refers to its rows
  by position: no numeric column of another table, no DS1 file, no saved
  data, no protocol message and no engine constant. No table is `free` in
  v1.

**Addable fields.** An added field column changes every existing row's
compiled value from the missing-column default to the empty-cell value.
These differ for `code4` and `key(code4)` (`00 00 00 00` against
`20 20 20 20`), `strkey` (0 against 5382), links whose target has an empty
key (−1 against that row's index) and callbacks (`field-types.md` §3, Edge
cases). A table spec may list a field as addable only after stating that
change. The default list is empty.

The initial entries for all 86 tables are in "Provisional catalog" below.

### 4. Bytes, lines and tokens

These rules hold for layer files and stack files.

**4.1 Bytes.**
- Allowed: 0x20–0x7E, LF (0x0A), and CR (0x0D) only directly before LF.
- Any other byte is P01 at its line and 1-based byte column. That includes
  TAB, every other control byte, 0x7F and every byte ≥ 0x80. A UTF-8 BOM is
  P01 at 1:1. At most one P01 is reported per line (the leftmost).
- A file over 16,777,216 bytes is P13 at 0:0, and nothing else is checked.

**4.2 Lines.**
- Lines are split at LF. A CR before LF belongs to the line end, so LF and
  CR LF files are equivalent. A last line without LF is a line. The empty
  piece after a final LF is not a line. An empty file has no lines.
- Lines are numbered from 1. Columns are 1-based byte offsets.
- A line over 4,096 bytes (line end excluded) is P13 at column 4,097.
- A **blank** line has only spaces. A **comment** line has `#` as its
  first non-space byte. Both are ignored apart from the byte check. There
  are no trailing comments: `#` inside a statement line is data.

**4.3 Tokens.** Each statement line is split into tokens left to right.
Spaces (0x20) between tokens are skipped. Leading and trailing spaces are
ignored.

| At a token start | Token |
|---|---|
| `[` | **bracket string**: the bytes up to the next `]`, which ends it. The text is exactly the bytes between the brackets: no escapes, no trimming. `[]` is the empty text. A line end before `]`, or a `[` inside, is P06 at the opening `[` |
| `]` | P10 at that byte |
| any other byte | **bare token**: the longest run of bytes other than space, `[` and `]`. Never empty |

- After a token, the next byte must be a space or the line end. Otherwise
  P10 at that byte: `lvl[1]` and `[1][4]` are P10.
- A token's text is at most 1,024 bytes. Longer is P13 at the token start.
- Keywords, `->`, `like`, index tokens (`#n`), pins and table names must
  be bare tokens. Every other position takes either kind, and both mean the
  same text: `axe` and `[axe]` are equal.
- Consequences: a value with a space, an edge space or no bytes needs
  brackets (`[Hand Axe]`, `[4 ]`, `[]`). A value cannot contain `[` or `]`,
  and every byte is printable ASCII. In 1.14d none of the 193 `.txt` files
  of the three archives contains `[`, `]`, `@`, `^`, `{`, `|`, `}` or `~`
  in a header or cell. Quotes, backslashes, `#`, `:` and `,` are ordinary
  bytes.

### 5. Layer files (`.d2patch`)

**5.1 Header.** Line 1 must hold exactly two bare tokens: `d2patch` and a
version matching `[1-9][0-9]{0,8}`. Otherwise P02 at 1:1. A version other
than 1 is P03 at the version token. If line 1 has a P01, P02 or P03, no
statement is parsed: the file reports only its P01 errors (all lines) and
its P02 or P03.

**5.2 Statements.** Every other line is blank, a comment, or a statement.
A statement's first token must be one of the bare keywords `table`, `set`,
`check`, `add`, `remove`, `addcol`, `delcol`. Anything else is P04 at
the first token (`Set`, `[set]`, `layer`).

| Statement | Tokens after the keyword |
|---|---|
| `table` | `<table-name>` |
| `set` | `<sel> <col> <old> -> <new>` |
| `check` | `<sel> <col> <value>` |
| `add` | `<index> <key>`, optionally followed by `like <sel>`, optionally followed by `<pin>` |
| `remove` | `<sel>` |
| `addcol` | `<name>` |
| `delcol` | `<col>` |

- `<sel>` is a **selector**. If its first token is a bare token starting
  with `#`, it is an index selector of two tokens: `<index> <label>`.
  Otherwise it is a key selector of one token.
- `<index>`: bare `#` followed by `0` or `[1-9][0-9]*`, with a value at
  most 4,294,967,295. Anything else starting with `#` in an index position
  is P07 (`#01`, `#1x`, `#4294967296`). A key that starts with `#` is
  written bracketed: `[#5]` is a key selector, `#5 [#5]` is row 5 with
  label `#5`.
- `<col>`, `<old>`, `<new>`, `<value>`, `<key>`, `<label>`, `<name>`: any
  token (bare or bracket).
- `<table-name>`: a bare token matching `[a-z0-9]{1,32}`, else P08 (also
  for a bracket string).
- `<pin>`: a bare token `sha:` followed by 16 characters `[0-9a-f]`, else
  P12.
- `<name>` of `addcol` (P11 if it breaks a rule): 1–64 bytes; no `@`; no
  leading or trailing space; and if it contains `:`, it must match the
  extension pattern `[a-z][a-z0-9_]{0,15}:[A-Za-z0-9_]{1,48}`.

**5.3 Shape errors (P05).**
- A required token is missing: P05 at the column after the line's last
  non-space byte.
- An extra token: P05 at the first extra token.
- `->` or `like` expected but another token found: P05 at that token.
- `add`: if token 2 is not a bare token starting with `#`, P05 at token 2.
  If token 4 exists and is not `like`, P05 at token 4. After `like <sel>`,
  one more token must be a pin (else P12); any token after the pin is P05.
- **P09:** a statement other than `table` before the first `table` line,
  at its keyword. A `table` line with a P08 still counts as a table line.

**5.4 Summary in ABNF** (RFC 5234; tokens are separated by `1*SP` and
lexed by §4.3 first; literals are case-sensitive):

```
header    = "d2patch" 1*SP version            ; line 1
version   = %x31-39 *8DIGIT                   ; must be "1" (else P03)
statement = table / set / check / add / remove / addcol / delcol
table     = "table" 1*SP tname
set       = "set" 1*SP sel 1*SP tok 1*SP tok 1*SP "->" 1*SP tok
check     = "check" 1*SP sel 1*SP tok 1*SP tok
add       = "add" 1*SP index 1*SP tok [1*SP "like" 1*SP sel [1*SP pin]]
remove    = "remove" 1*SP sel
addcol    = "addcol" 1*SP tok
delcol    = "delcol" 1*SP tok
sel       = (index 1*SP tok) / keytok
index     = "#" ("0" / %x31-39 *DIGIT)        ; value <= 4294967295
pin       = "sha:" 16(DIGIT / %x61-66)
tname     = 1*32(%x61-7A / DIGIT)
tok       = bracket / bare
keytok    = bracket / (nchar-no-hash *nchar)
bracket   = "[" *(%x20-5A / %x5C / %x5E-7E) "]"
bare      = 1*nchar
nchar     = %x21-5A / %x5C / %x5E-7E          ; printable, no space, no [ ]
nchar-no-hash = %x21-22 / %x24-5A / %x5C / %x5E-7E
```

**5.5 Error policy.** Each line reports at most one P error: the first
found in the order byte check (P01), line length (P13), tokens (P06, P10,
P13, left to right), keyword (P04), P09, then shape (P05, P07, P08, P11,
P12). Parsing continues with the next line. Every P error of every file
of the stack is reported.

### 6. Stack files (`.d2stack`)

- Same byte, line and token rules (P01, P06, P10, P13 apply).
- Line 1: exactly the bare tokens `d2stack` and a version
  `[1-9][0-9]{0,8}`, else S01 at 1:1. A version other than 1 is S02.
- Every other line is blank, a comment, or `layer <path>` (exactly two
  tokens, `layer` bare). Anything else is S03 at its first token.
- `<path>`: relative to the stack file's directory; segments separated by
  `/`; each segment matches `[a-z0-9_.-]+` and is not `.` or `..`; no
  empty segment, so no leading `/`; ends with `.d2patch`; at most 255
  bytes. Otherwise S04 at the path token. Lowercase-only paths make the
  result independent of the file system's case rules.
- The same path twice: S05 at the second line (related: the first).
- More than 1,024 `layer` lines: S07 at the 1,025th `layer` line.
- A missing or unreadable layer file: S06 at the path token. The caller
  reads files; `apply` receives bytes keyed by path.
- The layer order is the line order. It is never computed. A stack with no
  `layer` line is valid and applies nothing.
- A layer is named in every message and in provenance by its path text
  exactly as written in the stack (`core/10-items.d2patch`), never by an
  absolute path.

### 7. Selectors and column references

All resolution happens when a statement executes, against the current
state of the table (base plus every earlier statement).

**7.1 Key selector** with text `K`:
1. A21 if `K` is in `R_T` (§7.4).
2. Candidate rows: rows whose key cell equals `K` byte for byte.
3. Ignore rows marked for removal (§8). If candidates exist but all are
   marked: A12. If there are none: A03. Optional hints (not normative):
   rows equal ignoring ASCII case, renames and removals of `K` in the logs.
4. More than one: A04, with up to 10 indices ascending as related data.
5. Otherwise that row.

**7.2 Index selector** `#n L`:
1. A03 if `n` ≥ the row count (rows marked for removal still count).
2. A12 if row `n` is marked for removal.
3. A05 if row `n`'s key cell is not exactly `L`. Found: the key cell.
   Related: up to 5 ascending indices whose key cell is `L`.
4. Otherwise row `n`.

Index selectors are never subject to A21.

**7.3 Column reference** with text `C`:
1. If `C` contains `@`: the name is the part before the last `@`, and `k`
   is the part after it, which must match `[1-9][0-9]*`. Without `@`: the
   name is `C`, no `k`.
2. `m` = the columns whose name equals the name, byte for byte and
   case-sensitively.
3. A02 if: `m` is empty; or `m` has more than one column and no `k` is
   given (ambiguous; related: their positions); or `k` is given and `m` has
   one column; or `k` > `|m|`; or `k` is malformed.
4. The result is the `k`-th column of `m` from the left, or the only one.

- The empty name is a valid name: `[]` names the one empty-named column of
  1.14d (`weapons` column 18).
- A column whose name contains `@` cannot be referenced. No 1.14d header
  contains `@`, and `addcol` names cannot.
- The **canonical reference** of a column is its name if no other column
  has that name, else `name@k`.

**7.4 Renamed keys** `R_T`. Before a layer executes, for each table `T`,
`R_T` is the set of every old and new text of every `set` statement in
this layer's sections of `T` whose column token text equals `T`'s key
column name. It is computed from the parsed statements, not from state.

### 8. Statements

Checks run in the order listed. The first failing check is the statement's
error, and the statement then has no effect. "Location" is the token whose
column the error reports.

Per layer, the engine keeps:
- `W`: cells written in this layer (by `set`, and the key cell of each
  `add`).
- `TP`: rows used as templates by an `add … like` in this layer.
- per table: whether an `add` or a `remove` was applied in this layer, and
  the names deleted by `delcol` in this layer.
- `KW`: key writes in this layer, in order: (table, value, location) for
  each applied `add` and each applied `set` on the key column.
- marks for removal.

**`table T`**
- A01 if `T` has no catalog entry (location: the name).
- Effect: later statements apply to `T`. One layer may have several
  sections for the same table; they form one sequence in file order.

**`set S C O -> N`**
1. Resolve `S` (§7.1, §7.2; location: the selector's first token).
2. Resolve `C` (§7.3; location: the column token).
3. A08 if the cell is in `W` (location: column token).
4. A22 if the row is in `TP` (location: selector).
5. A06 if the cell is not exactly `O` (location: old token). Expected
   `O`, found the cell, with its writer.
6. A07 if `O` equals `N` (location: new token).
7. A14 if `C` is column 0 and `N` is exactly `Expansion` (location: new
   token). `expansion` and `EXPANSION` are allowed.
8. A16 if `C` is the key column, the policy is `unique` and `N` is empty
   (location: new token).
9. Effect: the cell becomes `N`, writer `Set(layer, line)`; add the cell
   to `W`; append a change-log entry; if `C` is the key column, append to
   `KW`. Note N01 if the previous writer was `Add`, `Copy` or `Set` of
   another layer (location: old token; related: that writer). Note N02 if
   the catalog has the table's field list, `C` is neither the key column
   nor an extension column, and no field binds `C` under
   `txt-format.md` §6 (comment columns, a duplicate shadowed by an earlier
   column, the empty-named column).

**`check S C V`**
1. Resolve `S`, then `C`, as for `set`.
2. A06 if the cell is not exactly `V` (location: value token).
- No effect. It documents and enforces an assumption.

**`add #n K [like S [P]]`**
1. A10 if the table's rows policy is `fixed` (location: keyword).
2. A12 if a `remove` on this table was applied earlier in this layer
   (location: keyword).
3. A09 if `n` is not the current row count (location: index token;
   expected: the row count).
4. With `like`: resolve `S`. A17 if `P` is given and differs from the
   template's pin (§8.1) (location: pin; found: the computed pin).
5. A14 if the key column is column 0 and `K` is exactly `Expansion`
   (location: key token).
6. A16 if the policy is `unique` and `K` is empty (location: key token).
7. Effect: a new row at index `n`, origin `Added(layer, line)`. Without
   `like`, every cell is empty with writer `Add(layer, line)`. With `like`,
   every cell is a copy of the template's current cell, writer
   `Copy(layer, line)`. Then the key cell becomes `K`, writer
   `Add(layer, line)`. Add the key cell to `W`, the template to `TP`, and
   (table, `K`, key token) to `KW`.

**`remove S`**
1. A10 if the rows policy is not `free` (location: keyword).
2. A12 if an `add` on this table was applied earlier in this layer
   (location: keyword).
3. Resolve `S`. A12 also covers a row already marked (§7).
4. A11 if the row is not pristine: its origin must be `Base` and every
   cell's writer `Base` or `AddCol` (location: selector).
5. A19 if this would mark the last unmarked row (location: keyword).
6. Effect: mark the row. Marked rows are deleted at the end of the layer
   (§9), so indices do not change during the layer.

**`addcol X`**
1. A20 if the table already has 280 columns (location: name).
2. A18 if this layer deleted a column of this table whose name equals `X`
   ignoring ASCII case.
3. A13 if a column of the table has a name equal to `X` ignoring ASCII
   case (related: that column's origin).
4. A13 if `X` is not an extension name (no `:`) and is not exactly one of
   the entry's `addable` names.
5. Effect: append a column named `X`, origin `AddCol(layer, line)`. Every
   row gets an empty cell with writer `AddCol(layer, line)`. These cells are
   not in `W` and do not make a row non-pristine.

**`delcol C`**
1. Resolve `C` (location: column token).
2. A15 if the column's origin is `Base`.
3. A18 if the column was added in this layer, or any of its cells is in
   `W`.
4. Effect: delete the column and its cells. Append a column-log entry with
   the deleted non-empty cells. Note N05 if the column was added by another
   layer or any of its cells has an `Add`, `Copy` or `Set` writer of
   another layer.

**8.1 Pin.** The pin of a row is `sha:` followed by the first 16 lowercase
hex digits (8 bytes) of the SHA-256 of its pin stream. The pin stream is,
for each column in table order whose cell in that row is not empty: the
column's canonical reference (§7.3), byte 0x09, the cell, byte 0x0A.
Empty cells contribute nothing, so adding an empty column does not change
any pin. An all-empty row has pin `sha:e3b0c44298fc1c14`.

**8.2 Structural statements.** `add`, `remove`, `addcol`, `delcol`,
`table`, and any `set` whose column token text equals the key column's
name. When one of them fails, every later statement of this layer in a
section of the same table is skipped. One note N03 is emitted at the first
skipped statement's keyword, with the number skipped. Other failures do not
skip anything, and later statements still run.

### 9. Applying a stack

1. Parse the stack file (§6). Parse every layer it lists (§5). If any S or
   P error exists, report all of them and stop.
2. Read the base tables (§2); any B error stops. Every column, row and cell
   starts with origin or writer `Base`.
3. For each layer in stack order:
   1. Compute `R_T` for every table (§7.4). Clear `W`, `TP`, `KW` and the
      per-table flags.
   2. Execute the statements in file order (§8, §8.2).
   3. **Uniqueness (A16).** For each table with policy `unique` that has
      `KW` entries: let `before(k)` be the number of rows whose key
      normalizes (§3 `kind`) to `k` when the layer started, and `after(k)`
      the number of unmarked rows with that key now. For each `KW` entry of
      the table in order, with `k` its normalized value: if
      `after(k) > max(1, before(k))` and `k` was not reported yet, report
      A16 at that entry's location (related: the indices holding `k`).
      Checking at the end allows swaps: `set #0 axe code axe -> ax2` then
      `set #2 ax2 code ax2 -> axe` is valid.
   4. Delete the rows marked for removal, keeping the order of the others,
      and renumber. Append one removal-log entry per deleted row.
   5. If this layer reported any A error: stop. Emit note N04 naming the
      layers not applied. Notes of the failed layer are kept.
4. The final tables go to the compile stage (§11).

Errors inside a layer are all collected (§8.2 limits cascades). Only one
layer can fail, because later layers are built on it.

### 10. Conflicts and drift

| Situation | Outcome |
|---|---|
| A layer's `set` states an old value that the current cell does not have (the user's base differs, or an earlier layer changed the cell) | A06, naming the writer of the current value |
| A later layer states the value an earlier layer wrote | allowed: an explicit override; note N01 and a change-log entry |
| A selected row is missing or ambiguous | A03, A04, A05 |
| A `check` fails | A06 |
| The template row of `like` differs from the pinned one | A17 |
| Two layers append to the same table but are stacked in another order than authored | A09 (indices are pinned) |
| A layer removes a row another layer added or changed | A11 (only pristine rows can be removed) |
| A layer adds a column that exists | A13, naming its creator |
| A layer deletes another layer's column | allowed; note N05 |

- **Policy: errors, never warnings.** Every overwrite of another layer's
  value states that value, so it is always deliberate and visible in
  review. Notes are information only; they never block.
- Each layer is meant to be authored against the rendered state after the
  earlier layers (`render --upto`, §17). The diff tool works this way.

### 11. Compile, references and validation

**11.1 Compile.** Every catalog table is compiled from its final cells
with `field-types.md`: the table's field list, binding by
`txt-format.md` §6, linkers from the final key columns, and string keys
from the installed string tables. Layers cannot change string tables in
v1.

- **Extension columns** (names containing `:`) never bind a field. No
  1.14d field name contains `:` (3,499 names in the 92 recovered field
  lists). They stay in the final cell tables. `Ruleset::Mod` code reads
  them as text by (table, column name, row index).
- Added field columns (`addable`) bind like base columns.
- **Combined index spaces** (`loading.md` §9): appending to `weapons`
  shifts the combined item index of every `armor` and `misc` row, and
  appending to `magicsuffix` shifts `magicprefix` and `automagic`. Compiled
  links resolve by key, so compiled data stays consistent. Nothing that
  outlives one stack (saves, protocol) may store a combined index.

**11.2 Reference findings.** For every compiled link field (`link8`,
`link16`, `link32`) and `strkey` field of every row:

| Code | Severity | Condition |
|---|---|---|
| R01 | error | A non-empty link cell misses (−1), and either its writer is `Add`, `Copy` or `Set` of a layer, or its text matched a key of the target table in the base (under the target linker's comparison) and a layer removed or renamed that key. Related: the cell's writer, and the removal-log or change-log entry |
| R02 | note | A non-empty link cell misses, its writer is `Base`, and the key never existed in the target's base (a 1.14d miss, unchanged; 14 such cells in 1.14d, `field-types.md` §6.3) |
| R03 | error | A non-empty `strkey` cell written by a layer resolves to no string (stored 5382, `field-types.md` §7) |
| R04 | note | The same for a `Base` cell (15 such cells in 1.14d) |
| R05 | error | A post-load check of `loading.md` §8 fails. Related: the layer statements that changed the row count or the checked cells of that table |
| R06 | error | A per-table invariant fails (defined by the table spec). Related: the writer of each offending cell |
| R07 | error | The compiler rejects a cell (for example `txt-format.md` E11, or a callback's parse error). Related: the cell's writer |

A copied cell (`Copy` writer) that inherits a base miss is R01: the new
row's reference is the layer's doing.

### 12. Outputs

| Output | Content |
|---|---|
| tables | final cell tables of all catalog tables, ordered by name |
| provenance | column origins, row origins, cell writers (§2) |
| change log | every applied `set`, in order: layer, line, table, row index, key, column reference, old, new. A `set` on the key column is a rename |
| removal log | layer, line, table, index before deletion, column names and the deleted row's cells |
| column log | layer, line, table, `addcol` or `delcol`, name; for `delcol` the deleted non-empty cells with their row indices |
| notes | N and R notes (§13) |
| table digest | SHA-256 of `render(T)` (§14), as 64 lowercase hex digits |
| data digest | SHA-256 over, for each catalog table in byte order of its name: the name, 0x09, its table digest (64 hex digits), 0x0A |

The data digest identifies the complete patched data set. The protocol
spec (Phase 7) may use it to check that client and server agree.

### 13. Errors and report

**13.1 Record.** Every diagnostic has these fields; they are normative and
tested. Message wording is not.

| Field | Content |
|---|---|
| code, severity | §13.3. Severity is `error` or `note`; there are no warnings |
| file | the layer's stack path (for N04: the failed layer, line 0); the stack file as named by the caller; `base:<archive>:<inner path>` for base findings |
| line, col | 1-based; 0 when not applicable |
| table | table name, or empty |
| row | index and key text at that moment, or empty |
| column | canonical column reference, or empty |
| expected | the value the statement stated (A05 label, A06 old/value, A09 index, A17 pin) |
| found | the current value (A05 key, A06 cell, A09 row count, A17 pin) |
| writer | the writer of the found value: `base` with its source line, or `layer:line` |
| related | further locations or indices (A04 rows, A16 rows, N01 previous writer, R01 log entry, S05 first line) |

Recommended human form, used in this spec:
`<severity>[<code>] <file>:<line>:<col>: <table> <key> (#<index>) <column>: <detail>`.
Values are shown in brackets, so `[]` and edge spaces are visible.

**13.2 Order.**
1. S errors, by line and column.
2. P errors, by the file's stack position, then line, column.
3. B errors, by table name.
4. A errors and N notes of the applied layers, by stack position, line,
   column, code; A16 end-of-layer errors at their locations; N04 last.
5. R findings, by table name (bytes), row index, column position, code.

Stops: after 1–2 if any S or P error; after 3 if any B error; after the
first layer with an A error; R errors leave no tables.

**13.3 Catalogue.** Codes are stable and never reused with another
meaning.

| Code | Meaning |
|---|---|
| P01 | forbidden byte (TAB, control, ≥ 0x80, lone CR, BOM) |
| P02 | line 1 is not a `d2patch <version>` header |
| P03 | unsupported version |
| P04 | unknown keyword |
| P05 | wrong number or kind of tokens |
| P06 | bracket string not closed on its line, or `[` inside it |
| P07 | malformed index token |
| P08 | malformed table name |
| P09 | statement before the first `table` line |
| P10 | token not followed by a space or the line end; `]` at a token start |
| P11 | malformed `addcol` name |
| P12 | malformed pin |
| P13 | limit exceeded: file 16 MiB, line 4,096 bytes, token 1,024 bytes |
| S01 | line 1 is not a `d2stack <version>` header |
| S02 | unsupported stack version |
| S03 | stack line is not `layer <path>` |
| S04 | malformed layer path |
| S05 | duplicate layer path |
| S06 | layer file missing or unreadable |
| S07 | more than 1,024 layers |
| B01 | catalog key column missing from, or repeated in, the base header |
| B02 | no base `.txt` for a catalog table |
| B03 | base `.txt` fails the reader (`txt-format.md` E1–E10; the E code is the detail) |
| A01 | table has no catalog entry |
| A02 | bad column reference: unknown, ambiguous, malformed or unnecessary `@k`, `k` too large |
| A03 | row not found: key matches no row, or index ≥ row count |
| A04 | key matches several rows |
| A05 | index selector's label is not the row's key |
| A06 | cell is not the stated value (`set` old value, `check` value) |
| A07 | `set` does not change the value |
| A08 | cell already written in this layer |
| A09 | `add` index is not the row count |
| A10 | rows policy forbids `add` (`fixed`) or `remove` (not `free`) |
| A11 | `remove` of a row that is not pristine |
| A12 | row marked for removal selected again; `add` and `remove` on one table in one layer |
| A13 | `addcol`: name exists (ASCII case-insensitive) or is not addable |
| A14 | column 0 would hold exactly `Expansion` |
| A15 | `delcol` of a base column |
| A16 | `unique` key: new duplicate, or empty key written |
| A17 | template pin mismatch |
| A18 | column added or written and deleted in one layer, or re-added after deletion |
| A19 | `remove` would leave the table empty |
| A20 | more than 280 columns |
| A21 | key selector uses a key that a key-column `set` of this layer reads or writes |
| A22 | write to a row after it served as a template in this layer |
| R01–R07 | §11.2 |
| D01–D07 | §15 |
| N01 | note: `set` replaced a value written by another layer |
| N02 | note: `set` on a column the compiler does not bind |
| N03 | note: statements skipped after a failed structural statement |
| N04 | note: later layers not applied |
| N05 | note: `delcol` removed a column created or written by another layer |

Severities: every P, S, B, A and D code, and R01, R03, R05, R06, R07, are
errors. N01–N05, R02 and R04 are notes.

### 14. Render

- `render(T)`: the column names joined with TAB, then for each row its
  cells joined with TAB. Every line, the last included, ends with CR LF.
  Bytes are copied exactly. No `Expansion` line is written. No BOM.
- **Identity:** for every base file `f`, `render(read(f))` equals `f` with
  its `Expansion` lines removed. Verified for all 86 catalog tables.
- **Round trip:** `read(render(T)) = T` for every table `apply` returns.
  This holds because cells contain only printable ASCII, column 0 never
  holds `Expansion` (A14), a table never ends up empty (A19), and a header
  never exceeds 280 columns (A20; `txt-format.md` E7).
- Rendered tables contain Blizzard data. Tools write them only under
  `game/` (§17). They are for inspection and authoring, not for the
  original game.

### 15. Diff (edited `.txt` to layer)

Inputs: the before table `B` (the state after a chosen layer, or the
base), the edited file bytes `E`, the table's catalog entry, and the
option `no-like`. Output: canonical layer text (§16), or the first D
error found in the order below.

1. Read `E` with `txt-format.md` §2–§5, giving header `H` and rows `X`.
   A reader error is D06 (detail: its E code and line). Removal of
   `Expansion` lines applies to `E` as to the base.
2. **Columns.** Walk `B`'s columns left to right with a position `j` into
   `H`, starting at 0. If `H[j]` equals the column's name: keep it and
   advance `j`. Otherwise the column is deleted: D02 if its origin is
   `Base`. The names `H[j..]` are new. Each new name must pass the `addcol`
   rules (§5.2 P11, §8 A13) and be unique in `H` ignoring ASCII case, else
   D04. More than 280 names: D07.
3. **Rows.** D01 if `|X| < |B|`; removals are never inferred. D07 if
   `|X| > |B|` and the rows policy is `fixed`. Row `i < |B|` of `X` pairs
   with row `i` of `B'`, where `B'` is `B` with deleted columns dropped and
   an empty cell appended for each new column.
4. **Shift check (D05)** at the first row `i < |B|` with `X_i ≠ B'_i`
   where either:
   - the policy is `unique`, the key changed, and the new key equals
     (normalized) the key of another row `B'_j`, `j < |B|`; or
   - `X_i` equals `B'_(i−1)` or `B'_(i+1)` as a whole row.
   This catches rows inserted, deleted or sorted in a spreadsheet.
5. **Catalog check (D07)**, policy `unique`: a normalized key whose count
   in `X` exceeds `max(1, count in B')`; or an empty key cell in `X` at a
   row `i ≥ |B|` or where `B'_i`'s key was not empty.
6. **Expressible (D03):** every value and column name the output needs
   must be a valid token text: at most 1,024 bytes of 0x20–0x7E, without
   `[` or `]`; column names also without `@`.
7. `R` = the old and new keys of every row `i < |B|` whose key changed.
8. **Lines,** in this order:
   1. `delcol <ref>` for each deleted column, in `B` order (reference in
      `B`).
   2. `addcol <name>` for each new name, in `H` order.
   3. For each `i < |B|` ascending with `X_i ≠ B'_i`: the selector is the
      key selector `key(B'_i)` if that key occurs exactly once among
      `B'`'s keys and is not in `R`; otherwise `#i key(B'_i)`. Emit
      `set <sel> <col> <B'_i[c]> -> <X_i[c]>` for each column `c` in `H`
      order, except the key column, where the cells differ; then the same
      for the key column if it changed. Column references are canonical
      in `H`.
   4. For each new row `n = |B| … |X|−1`: unless `no-like`, pick a
      template among rows `t < |B|` of `X`: `score(t)` = the number of
      columns `c` other than the key where `X_n[c]` is not empty and equals
      `X_t[c]`. Take the highest score, then the lowest `t`. Use it if
      `score ≥ 1` and `2 × score ≥` the number of non-empty non-key cells
      of `X_n`. Emit `add #n key(X_n)`, plus `like <tsel> <pin>` with a
      template: `tsel` is the key selector `key(X_t)` if that key occurs
      exactly once among `X_0 … X_(n−1)` and is not in `R`, else
      `#t key(X_t)`; the pin is that of `X_t` under `H` (§8.1). Then
      `set #n key(X_n) <col> <start[c]> -> <X_n[c]>` for each non-key
      column `c` in `H` order where `X_n[c]` differs from `start[c]` (the
      template's cell, or empty without one).
9. **Output:** `d2patch 1` LF, and if there are lines: an empty line,
   `table <name>`, then the lines, each ending with LF. Without lines the
   output is `d2patch 1` LF only. For several tables, one file holds one
   header and one section per table, sections in byte order of name, each
   preceded by an empty line.

- **Round-trip law:** applying the output as the next layer after `B`
  gives a table whose header is `H` and whose rows are `X`, byte for byte.
  The diff respects A21 (renamed keys use index selectors), A22 (adds come
  after all sets) and A16 (step 5).
- A layer generated this way contains single cells of the user's data
  (old values, keys, labels) but never a whole base row: new rows start
  from a template whenever one shares at least half of their cells.

### 16. Canonical form and `fmt`

**Canonical token:** a value is written bare if its text is non-empty,
every byte is 0x21–0x7E, it has no `[` or `]`, it does not start with
`#`, and it is not exactly `->` or `like`. Otherwise it is bracketed.
Keywords, `->`, `like`, index tokens, pins and table names are always bare.

**Canonical file:**
- LF line ends and a final LF.
- Line 1 is `d2patch 1`.
- Statement lines: canonical tokens joined by single spaces, no
  indentation, no trailing spaces.
- Comment lines: leading and trailing spaces removed; text kept.
- No two consecutive blank lines; no blank line at the end of the file.

**`fmt`** rewrites a file that has no P error into canonical form. It keeps
the statement order, the comments and the positions of blank lines
(collapsing runs). It is idempotent, and the parsed statements of
`fmt(f)` equal those of `f` (line numbers aside). A file with P errors is
left unchanged and the errors are reported.

### 17. Tools (`tools/d2patch`, `anyhow`)

Exit status: 0 = no error (notes allowed), 1 = errors, 2 = usage or I/O
failure.

| Command | Effect |
|---|---|
| `check <stack>` | parse, apply, compile, reference pass; print the report (§13) and the digests (§12) |
| `render <stack> [--upto <layer> \| --base] <table>…` | write `render(T)` of the state after `<layer>` (inclusive; default: all layers) to `game/mod-render/<stack stem>/<table>.txt` |
| `diff <stack> (--upto <layer> \| --base) <table>=<edited.txt>… [--no-like] [-o <file>]` | §15 with `B` = the state after `<layer>`; one output file for all tables; stdout without `-o` |
| `fmt [--check] <file>…` | §16, in place; `--check` changes nothing and exits 1 if a file is not canonical |
| `blame <stack> <table> <sel> [<col>]` | every writer and change-log entry of the row or cell |

- Authoring loop: `render --upto <previous layer>`, edit in a spreadsheet,
  `diff`, `check`, review the layer with `git diff`.
- Tools never write rendered tables or extracted data outside `game/`.
  Recommended: `*.d2patch text eol=lf` in `.gitattributes`, and a
  pre-commit rule rejecting `.txt` and `.bin` files under `mods/`.

### 18. Versioning and migration

- Every file states its format on line 1: `d2patch 1`, `d2stack 1`. The
  engine reads every version from 1 to its current one. A newer version is
  P03 or S02. Unknown statements are never ignored.
- Each new version k ≥ 2 ships with: its spec section; a pure text-to-text
  migration from version k−1 that writes canonical form; and tests that
  applying a migrated file gives the same result as applying the original
  with version k−1 rules. `d2patch upgrade` rewrites files in place.
- A new version is required for any change that can make a valid file
  invalid or change its result: the grammar; a statement's checks or
  effects; the cell model (a `txt-format.md` change that alters cells or
  row numbering); render bytes or digests; the pin stream; and any change to
  a **frozen** catalog entry's `key`, `kind`, a `multi` → `unique` change, a
  tighter rows policy, or a removed `addable` field.
- No new version is needed for changes that only make invalid files valid:
  `unique` → `multi`, a looser rows policy, a new `addable` field, a new
  catalog entry, new error codes for cases that were already errors.
- **Catalog status:** `provisional` entries may change freely. A per-table
  spec freezes its entry when it is reviewed. Every entry must be frozen
  before the first public mod release.
- Pins carry their scheme in the prefix (`sha:` = §8.1). A different pin
  stream gets a different prefix, and the old one stays readable.

### 19. Determinism

- `apply` is a pure function of: the base tables, the catalog, the stack
  bytes and the layer bytes. Diff, render and fmt are pure functions of
  their inputs.
- No result depends on hash-map iteration, directory listing order,
  locale, time, environment, threads or floating point. Use `Vec` and
  `BTreeMap`, or sort explicitly. Case folding is ASCII only.
- **Order independence.** For a layer that applies without error, every
  permutation of its statement lines that also applies without error gives
  identical final tables, except that the order of `addcol` statements on
  one table fixes the order of those columns. Permutations that would
  change a result fail instead: a `set` before its `add` (A03), a column use
  before its `addcol` (A02), swapped `add`s (A09), a write to a template
  after its `add` (A22), selection by a renamed key (A21), a second write to
  a cell (A08).

Required tests:

| Id | Property |
|---|---|
| T-repeat | two runs, in one process and in two processes, give identical tables, logs, report bytes and digests |
| T-perm | seeded random permutations of the statements of valid layers: identical result or an error (model: 1,521 identical, 1,694 rejected, 0 different) |
| T-eol | LF and CR LF versions of a file give the same statements |
| T-render | the render identity and round trip (§14) |
| T-diff | the round-trip law (§15) on seeded random edits (model: 1,183 accepted edits on the 86 tables, all reproduced) |
| T-fmt | `fmt` is idempotent and keeps the statements |
| T-fuzz | the parser never panics on arbitrary bytes and reports only P codes |
| T-platform | the synthetic vectors pass on Windows and Linux without game files |

## Constants & data dependencies

| Constant | Value |
|---|---|
| file headers | `d2patch 1`, `d2stack 1` |
| extensions | `.d2patch`, `.d2stack` |
| limits | file 16,777,216 bytes; line 4,096 bytes; token text 1,024 bytes; 1,024 layers; path 255 bytes; 280 columns per table |
| index range | 0 to 4,294,967,295 |
| table name | `[a-z0-9]{1,32}` |
| path segment | `[a-z0-9_.-]+`, not `.` or `..` |
| extension column | `[a-z][a-z0-9_]{0,15}:[A-Za-z0-9_]{1,48}` |
| `addcol` name | 1–64 bytes, no `@`, no edge space; `:` only in extension names |
| pin | `sha:` + first 16 hex digits of SHA-256 of the pin stream (§8.1) |
| digests | SHA-256, 64 lowercase hex digits (§12) |
| removed-row marker | first cell exactly `Expansion` (`txt-format.md` §5) |
| key comparison | `code`: 4 bytes, 0x20-padded, case-sensitive; `name`: 31 bytes, ASCII lowercased; `text`: exact |

Depends on: `txt-format.md` §2–§6 (reader, binding), `field-types.md`
§3–§8 (compile, linkers, string keys), `loading.md` §2, §6, §8, §9, §11
(sources, order, checks, combined indices).

## Provisional catalog

Measured on the 1.14d base text (§2). Src: archive of the base `.txt`
(P = `patch_d2`, X = `d2exp`). Key: column name (0-based position). Dup:
rows beyond the first per repeated key after `kind` normalization; empty:
rows with an empty key. Every entry is `provisional`, with `addable` empty
and no field list until its table spec exists. "own" keys come from the
own-key field of the 1.14d field list (rule 1 of §3); the others are
column 0 (rule 2).

| Table | Src | Rows | Cols | Key | Kind | Policy | Rows policy | Dup/empty |
|---|---|---|---|---|---|---|---|---|
| arena | X | 1 | 8 | `Arena` (0) | text | multi | append | 0/0 |
| armor | P | 202 | 164 | `code` (17), own | code | unique | append | 0/0 |
| armtype | X | 3 | 2 | `Name` (0) | text | multi | append | 0/0 |
| automagic | X | 36 | 38 | `Name` (0) | text | multi | append | 0/0 |
| automap | X | 3,286 | 13 | `LevelName` (0) | text | multi | append | 3,252/0 |
| belts | X | 14 | 68 | `name` (0) | text | multi | fixed | 1/0 |
| bodylocs | X | 11 | 2 | `Code` (1), own | code | unique | append | 0/1 |
| books | P | 3 | 11 | `Name` (0) | text | multi | append | 2/0 |
| charstats | P | 7 | 79 | `class` (0) | text | multi | append | 0/0 |
| chartemplate | X | 30 | 95 | `Template` (0) | text | multi | append | 16/0 |
| colors | X | 21 | 2 | `Code` (1), own | code | unique | append | 0/0 |
| compcode | P | 115 | 2 | `code` (1), own | code | unique | append | 0/0 |
| composit | X | 16 | 2 | `Name` (0) | text | multi | append | 0/0 |
| cubemain | P | 151 | 105 | `description` (0) | text | multi | append | 0/0 |
| difficultylevels | P | 3 | 23 | `Name` (0) | text | multi | fixed | 0/0 |
| elemtypes | P | 13 | 2 | `Code` (1), own | code | unique | append | 0/1 |
| events | P | 13 | 2 | `event` (0), own | name | unique | append | 0/0 |
| experience | P | 101 | 9 | `Level` (0) | text | multi | append | 0/0 |
| gamble | X | 125 | 2 | `name` (0) | text | multi | append | 0/0 |
| gems | P | 68 | 41 | `name` (0) | text | multi | append | 0/0 |
| hiredesc | X | 9 | 2 | `Code` (1), own | code | unique | append | 0/1 |
| hireling | P | 120 | 73 | `Hireling` (0) | text | multi | append | 116/0 |
| hitclass | X | 14 | 2 | `Code` (1), own | code | unique | append | 0/1 |
| inventory | P | 32 | 73 | `class` (0) | text | multi | fixed | 0/0 |
| itemratio | P | 6 | 20 | `Function` (0) | text | multi | append | 2/0 |
| itemstatcost | P | 359 | 53 | `Stat` (0), own | name | unique | append | 0/0 |
| itemtypes | P | 103 | 37 | `Code` (1), own | code | unique | append | 4/5 |
| levels | P | 137 | 140 | `Name` (0) | text | multi | append | 0/0 |
| lowqualityitems | X | 4 | 1 | `Name` (0) | text | multi | append | 0/0 |
| lvlmaze | P | 81 | 9 | `Name` (0) | text | multi | append | 3/0 |
| lvlprest | P | 1,091 | 25 | `Name` (0) | text | multi | append | 2/0 |
| lvlsub | P | 34 | 24 | `Name` (0) | text | multi | append | 1/0 |
| lvltypes | P | 36 | 37 | `Name` (0) | text | multi | append | 0/0 |
| lvlwarp | X | 88 | 14 | `Name` (0) | text | multi | append | 5/0 |
| magicprefix | P | 669 | 41 | `Name` (0) | text | multi | append | 416/32 |
| magicsuffix | P | 747 | 39 | `Name` (0) | text | multi | append | 458/6 |
| misc | P | 151 | 168 | `code` (13), own | code | unique | append | 0/0 |
| misscalc | P | 43 | 2 | `code` (0), own | code | unique | append | 0/0 |
| missiles | P | 684 | 171 | `Missile` (0), own | name | unique | append | 0/0 |
| monai | P | 148 | 10 | `AI` (0), own | name | unique | append | 0/0 |
| monequip | P | 45 | 13 | `monster` (0) | text | multi | append | 39/0 |
| monitempercent | X | 2 | 5 | `Name` (0) | text | multi | append | 0/0 |
| monlvl | P | 111 | 31 | `Level` (0) | text | multi | append | 0/0 |
| monmode | P | 16 | 3 | `code` (2), own | code | unique | append | 0/0 |
| monplace | P | 37 | 1 | `code` (0), own | name | unique | append | 0/0 |
| monpreset | P | 229 | 2 | `Act` (0) | text | multi | append | 224/0 |
| monprop | P | 13 | 92 | `Id` (0), own | name | unique | append | 0/0 |
| monseq | P | 1,010 | 6 | `sequence` (0), own | name | multi | append | 950/1 |
| monsounds | P | 141 | 41 | `Id` (0), own | name | unique | append | 0/1 |
| monstats | P | 734 | 255 | `Id` (0), own | name | unique | append | 1/0 |
| monstats2 | P | 609 | 126 | `Id` (0), own | name | unique | append | 0/0 |
| montype | P | 59 | 7 | `type` (0), own | name | unique | append | 0/1 |
| monumod | P | 43 | 19 | `uniquemod` (0), own | name | unique | append | 0/0 |
| npc | P | 17 | 19 | `npc` (0) | text | multi | append | 0/0 |
| objects | P | 573 | 160 | `Name` (0) | text | multi | append | 375/0 |
| objgroup | X | 133 | 28 | `GroupName` (0) | text | multi | append | 31/0 |
| objmode | X | 8 | 2 | `Name` (0) | text | multi | append | 0/0 |
| objtype | X | 573 | 3 | `Name` (0) | text | multi | append | 46/0 |
| overlay | P | 293 | 26 | `overlay` (0), own | name | unique | append | 0/0 |
| pettype | P | 20 | 22 | `pet type` (0), own | name | unique | append | 0/0 |
| playerclass | X | 7 | 2 | `Code` (1), own | code | unique | append | 0/0 |
| plrmode | P | 20 | 3 | `Code` (2), own | code | unique | append | 0/0 |
| plrtype | X | 7 | 2 | `Name` (0) | text | multi | append | 0/0 |
| properties | P | 268 | 36 | `code` (0), own | name | unique | append | 0/0 |
| qualityitems | X | 8 | 33 | `nummods` (0) | text | multi | append | 6/0 |
| rareprefix | X | 46 | 16 | `name` (0) | text | multi | append | 1/0 |
| raresuffix | X | 155 | 16 | `name` (0) | text | multi | append | 0/0 |
| runes | P | 169 | 49 | `Name` (0) | text | multi | append | 1/0 |
| setitems | P | 127 | 94 | `index` (0) | text | multi | append | 0/0 |
| sets | P | 32 | 69 | `index` (0), own | name | unique | append | 0/0 |
| shrines | P | 23 | 13 | `Shrine Type` (0) | text | multi | append | 19/0 |
| skillcalc | P | 73 | 2 | `code` (0), own | code | unique | append | 0/0 |
| skilldesc | P | 221 | 114 | `skilldesc` (0), own | name | unique | append | 0/0 |
| skills | P | 357 | 256 | `skill` (0), own | name | unique | append | 0/0 |
| soundenviron | P | 50 | 24 | `Handle` (0) | text | multi | append | 0/0 |
| sounds | P | 4,699 | 25 | `Sound` (0), own | name | unique | append | 0/1 |
| states | P | 185 | 72 | `state` (0), own | name | unique | append | 0/0 |
| storepage | X | 4 | 2 | `Code` (1), own | code | unique | append | 0/0 |
| superuniques | P | 66 | 21 | `Superunique` (0), own | name | unique | append | 0/0 |
| treasureclassex | P | 853 | 33 | `Treasure Class` (0) | text | unique | append | 0/1 |
| uniqueappellation | P | 25 | 1 | `Name` (0) | text | multi | append | 0/0 |
| uniqueitems | P | 402 | 70 | `index` (0) | text | multi | append | 8/1 |
| uniqueprefix | P | 53 | 1 | `Name` (0) | text | multi | append | 0/0 |
| uniquesuffix | P | 69 | 1 | `Name` (0) | text | multi | append | 0/0 |
| uniquetitle | P | 16 | 2 | `Name` (0) | text | multi | append | 15/0 |
| weapons | P | 306 | 166 | `code` (3), own | code | unique | append | 0/0 |

- The 86 tables are the 72 runtime text tables of `loading.md` §6
  (`leveldefs` is patched through `levels`), the 13 compile-only lookup
  tables of `loading.md` §7.2 that are not runtime tables, and
  `soundenviron`.
- Not patchable (no entry, A01): `aiparms`, `monname`, `treasureclass`,
  `cubemod`, `cubetype`, `weaponclass`. 1.14d never reads them
  (`loading.md`, Constants).
- 37 tables have an own key (16 `code`, 21 `name`). 36 of them are
  `unique`; `monseq` is `multi`. `treasureclassex` is the 37th `unique`.
- Notable base duplicates: `monstats` `cr_lancer8` (#617, #723);
  `itemtypes` empty `Code` (#0, #1, #14, #17, #23); `uniqueitems`
  `Azurewrath` ×2, `Rainbow Facet` ×8, all-empty #401; `treasureclassex`
  empty name at #852; `magicprefix` `Sturdy` (#1, #142, #143), `Jagged`
  ×11.

## Randomness

None.

## Edge cases & original bugs

- Base duplicates are tolerated in `unique` tables and addressed by index:
  `set #723 cr_lancer8 Level 5 -> 6`.
- Empty keys: `[]` selects the one row with an empty key (`montype`,
  `sounds`) when it is unique; otherwise use the index
  (`set #401 [] lvl [] -> 1` in `uniqueitems`).
- The empty-named `weapons` column 18 is referenced as `[]`.
- Duplicate columns: `armor` `mindam`/`maxdam` (positions 63/64 and
  161/162), `automap` `Type2` (7, 9), `chartemplate` `SkillName` (9
  columns). The compiler binds the leftmost (`txt-format.md` §6), so a
  `set` on `mindam@2` changes no compiled byte (N02 once the field list is
  known). The copies differ in 54 `armor` rows (#22 `buc`: 1/3 against
  0/0).
- `Expansion` is forbidden only as the exact column-0 text (A14).
  `expansion` and `EXPANSION` are ordinary cells, as in 1.14d (d2exp
  `objgroup` row 97 starts with `EXPANSION`).
- 81 base cells have leading or trailing spaces (for example `armor` name
  `Studded Leather `). They are written bracketed and compared exactly.
- Quotes and backslashes are literal: `cubemain` `input 1` cells such as
  `"hpot,qty=3"` are written bare, quotes included.
- Only 2 base cells hold bytes ≥ 0x80: `objects` #250 `description - not
  loaded` (0x85) and `uniqueitems` #60 `*type` (0x92). Both are comment
  columns. A layer cannot state them; editing them is D03.
- `name` keys compare on 31 bytes: two names that differ only after byte
  31 collide (A16). `sounds` has 33 names longer than 31 bytes; none
  collide.
- A template copy includes comment columns and extension columns.
- Under `Ruleset::Mod` every table is compiled from text. Where the
  compiled text differs from the live `.bin` (`field-types.md` §10:
  `monstats` #707 `NameStr`), the two rulesets differ even for a table no
  layer touches.
- `skills` has 256 columns, so at most 24 can be added (A20).

## Test vectors

### Fixture (synthetic, no game files)

Test catalog:

| Table | Columns | Key | Kind | Policy | Rows | Addable |
|---|---|---|---|---|---|---|
| items | `name`, `code`, `lvl`, `dam`, `dam`, `*note` | `code` | code | unique | append | `speed` |
| recipes | `description`, `enabled`, `output` | `description` | text | multi | free | — |
| flags | `Name`, `Code` | `Code` | code | unique | fixed | — |

Rows (`|` separates cells; `""` is an empty cell):
- items: #0 `Axe|axe|1|3|0|""`, #1 `Club|clb|1|2|0|old`,
  #2 `Axe|ax2|5|7|0|""`, #3 `Big Club|clb|9|8|1|""`.
- recipes: #0 `A|1|"hp1,qty=3"` (the quotes are cell bytes), #1 `B|1|x`,
  #2 `C|0|y`.
- flags: #0 `One|x1`, #1 `Two|x2`.

Base digests:
- `render(items)` =
  `name\tcode\tlvl\tdam\tdam\t*note\r\nAxe\taxe\t1\t3\t0\t\r\nClub\tclb\t1\t2\t0\told\r\nAxe\tax2\t5\t7\t0\t\r\nBig Club\tclb\t9\t8\t1\t\r\n`,
  digest `2c6f65f2ec858aadce86a4a0102fe612f5a2060a8df482caf093b64322f3e5b1`.
- recipes `43649b5f737ec059546013331b45f97283b3b9598e21bb18f2dc4d876aef1287`.
- flags `4378e03538544b4e838daea8af25de69610106e0766e85fafad3778cf814567c`.
- data digest `e17aec6c1ba77dbee55106555ccafc9768364c197c5bf570ff5e47c605ab3827`.

Pins:
- items #0: stream `name\tAxe\ncode\taxe\nlvl\t1\ndam@1\t3\ndam@2\t0\n`,
  pin `sha:84ec3f726bd38615` (SHA-256 `84ec3f726bd386159284740c4b3d0d0c5aec6d4d5f4711c5c6b3b864af2b5f1c`).
- items #3: `sha:bddc5549c005d720`. An all-empty row: `sha:e3b0c44298fc1c14`.

### Parse vectors

Each case is line 3 of `a.d2patch`, after `d2patch 1` and `table items`.

| Line 3 | Result |
|---|---|
| `set axe lvl [1] -> [4 ]` | key `axe`, column `lvl`, old `1`, new `4 ` (trailing space kept) |
| `  set   axe  lvl   1   ->   4   ` | same statement as `set axe lvl 1 -> 4` |
| `set [#5] lvl 1 -> 4` | key selector `#5` |
| `set #5 [#5] lvl 1 -> 4` | index 5, label `#5` |
| `set axe lvl[1] -> 4` | P10 3:12 |
| `set axe lvl [1][4]` | P10 3:16 |
| `set axe lvl 1 -> a]b` | P10 3:19 |
| `set #01 axe lvl 1 -> 4`, `set #1x …`, `set #4294967296 …` | P07 3:5 |
| `set axe [lvl 1 -> 4` | P06 3:9 |
| `set axe lvl [1 -> [4]` | P06 3:13 |
| `set axe lvl 1 -> 4 5` | P05 3:20 |
| `set axe lvl 1 ->` | P05 3:17 |
| `set axe lvl 1 => 4` | P05 3:15 |
| `Set axe lvl 1 -> 4`, `[set] axe lvl 1 -> 4` | P04 3:1 |
| `add 4 spr` | P05 3:5 |
| `add #4` | P05 3:7 |
| `add #4 spr axe` | P05 3:12 |
| `add #4 spr like` | P05 3:16 |
| `add #4 spr like #3` | P05 3:19 |
| `add #4 spr like axe sha:ABCDEF0123456789`, `… sha:0123` | P12 3:21 |
| `add #4 spr like #3 clb sha:bddc5549c005d720 x` | P05 3:45 |
| `remove axe ax2` | P05 3:12 |
| `remove #1` | P05 3:10 |
| `check axe lvl` | P05 3:14 |
| `table Items`, `table [items]` | P08 3:7 |
| `table items extra` | P05 3:13 |
| `table` | P05 3:6 |
| `addcol a b` | P05 3:10 |
| `addcol MOD:bonus`, `addcol mod:bo-nus`, `addcol [a@b]`, `addcol [ x]` | P11 3:8 |
| `set axe lvl 1 -> 4` followed by a TAB | P01 3:19 |
| `set axe lvl 1 -> ` + 1,025 × `x` | P13 3:18 (1,024 × `x` parses) |
| a 4,117-byte line | P13 3:4097 |
| `# comment with [unbalanced` | comment, no error |

Whole files:

| File bytes | Result |
|---|---|
| `EF BB BF` `d2patch 1\n` | P01 1:1 |
| `d2patch 2\n` | P03 1:9 |
| `d2patch 01\n`, `D2PATCH 1\n`, `d2patch\n`, `# x\nd2patch 1\n`, empty file | P02 1:1 |
| `d2patch 1\nset axe lvl 1 -> 4\n` | P09 2:1 |
| `d2patch 1\r\ntable items\r\nset axe lvl 1 -> 4` (no final line end) | valid, 2 statements |
| `…set axe lvl 1 -> 4\r\r\n` on line 3 | P01 3:19 |
| `set axe name Axe -> ` + byte 0xE9 on line 3 | P01 3:21 |

### Apply vectors

Layer `a.d2patch` (or `one.d2patch` then `two.d2patch`): line 1 `d2patch 1`,
line 2 `table items` unless another table is named, statements from line 3
(`/` separates lines). "Applies" means no error and no note unless shown.

| # | Statements | Result |
|---|---|---|
| V1 | `set axe lvl 1 -> 4` | #0 `Axe\|axe\|4\|3\|0\|""`; lvl writer `Set(a,3)`; one change-log entry; digest `13c4b643f616f07034695fa00080af6e0a1b48c1071b67932f7fa8698d476767` |
| V2 | `set clb lvl 1 -> 4` | A04 3:5, related rows 1, 3 |
| V3 | `set #1 clb lvl 1 -> 4` | applies; #1 lvl 4; digest `5970f48c162ef9d7231e275dc39f4a11ecdf810f33532175ab555bbf1d940926` |
| V4 | `set #1 axe lvl 1 -> 4` | A05 3:5, found `clb`, rows labelled `axe`: 0 |
| V5 | `set #4 clb lvl 1 -> 4` | A03 3:5 |
| V6 | `set AXE lvl 1 -> 4` | A03 3:5 |
| V7 | `set axe lvl 2 -> 4` | A06 3:13, expected `2`, found `1`, writer base |
| V8 | `set axe dam 3 -> 4` | A02 3:9, ambiguous (positions 3, 4). `dam@1 3 -> 4` and `dam@2 0 -> 1` apply. `dam@3`, `lvl@1`, `dam@01`, `[]`, `Lvl`: A02 3:9 |
| V9 | `set axe lvl 1 -> 1` | A07 3:18 |
| V10 | `set axe lvl 1 -> 4` / `set axe lvl 4 -> 5` | A08 4:9 |
| V11 | `add #4 spr` / `set spr name [] -> Spear` / `set #4 spr lvl [] -> 2` | #4 `Spear\|spr\|2\|""\|""\|""`; writers name `Set(a,4)`, code `Add(a,3)`, lvl `Set(a,5)`, others `Add(a,3)`; digest `d530958cb6927ea09f8605c00f1962f6c080b3e3395a9a8539e17db86e7d69a7` |
| V12 | `add #5 spr` | A09 3:5, expected `#4` |
| V13 | `add #4 a1` / `add #4 b1` | A09 4:5, expected `#5` |
| V14 | `set #4 spr lvl [] -> 2` / `add #4 spr` | A03 3:5 (line 4 runs; the layer fails) |
| V15 | `add #4 axe` | A16 3:8 |
| V16 | `add #4 [axe ]` | A16 3:8 (`code` comparison pads to 4 bytes) |
| V17 | `add #4 axe12` | applies (`axe1` ≠ `axe `); #4 `""\|axe12\|""\|""\|""\|""` |
| V18 | `add #4 clb` | A16 3:8 (2 in the base, 3 after) |
| V19 | `add #4 []` | A16 3:8 (empty key) |
| V20 | `set #2 ax2 code ax2 -> axe` | A16 3:24 (end of layer) |
| V21 | `set ax2 code ax2 -> ax3` | A21 3:5 |
| V22 | `set #2 ax2 code ax2 -> ax3` / `set ax3 lvl 5 -> 6` | A21 4:5. With `set #2 ax3 lvl 5 -> 6`: applies, #2 `Axe\|ax3\|6\|7\|0\|""` |
| V23 | `set #0 axe code axe -> ax2` / `set #2 ax2 code ax2 -> axe` | applies (swap); keys `ax2`, `clb`, `axe`, `clb` |
| V24 | `add #4 spr like axe sha:84ec3f726bd38615` / `set spr name Axe -> Spear` | #4 `Spear\|spr\|1\|3\|0\|""`; writers name `Set(a,4)`, code `Add(a,3)`, others `Copy(a,3)`; digest `a60483e64d8a39101423c49c690b4d29fe0e895fc8bb1721c584482d2ac2e05f` |
| V25 | `add #4 spr like axe sha:0000000000000000` | A17 3:21, found `sha:84ec3f726bd38615` |
| V26 | `add #4 spr like clb` | A04 3:17 |
| V27 | `add #4 spr like #3 clb sha:bddc5549c005d720` | #4 `Big Club\|spr\|9\|8\|1\|""` |
| V28 | `add #4 spr like axe` / `set axe lvl 1 -> 2` | A22 4:5 |
| V29 | `set axe lvl 1 -> 2` / `add #4 spr like axe` | applies; #4 `Axe\|spr\|2\|3\|0\|""` |
| V30 | `set axe name Axe -> Expansion` | A14 3:21. `-> expansion` applies |
| V31 | `addcol mod:bonus` / `set axe mod:bonus [] -> 5` | header gains `mod:bonus`; #0 `…\|""\|5`, others `…\|""`; digest `4a57cd044d59c450bac4064da6a6260627f4bac26f8599e5a42be3200991a81f` |
| V32 | `addcol speed` / `set axe speed [] -> 10` | applies (`speed` is addable) |
| V33 | `addcol Speed`, `addcol power`, `addcol [x y]` | A13 3:8 (not addable) |
| V34 | `addcol LVL`, `addcol [*note]` | A13 3:8 (exists) |
| V35 | `set axe mod:bonus [] -> 5` / `addcol mod:bonus` | A02 3:9 |
| V36 | `remove axe` / `set axe lvl 1 -> 2` | A10 3:1; N03 4:1, 1 skipped |
| V37 | flags: `add #2 x3` / `set x3 Name [] -> Three` | A10 3:1; N03 4:1 |
| V38 | recipes, one: `remove B`; two: `set #1 C enabled 0 -> 1` | recipes `A\|1\|"hp1,qty=3"`, `C\|1\|y`; digest `43374d0b5edf2b5f24b6a3abdb2b3b1639b0d52f8560237ab78c891856cdea91`. With `set #2 C …` in two: A03 two:3:5 |
| V39 | recipes: `remove B` / `add #3 D` | A12 4:1. Reversed: A12 4:1 |
| V40 | recipes: `remove B` / `set B enabled 1 -> 0` | A12 4:5. Reversed: A11 4:8 |
| V41 | recipes: `remove B` / `remove B` | A12 4:8 |
| V42 | recipes: `remove A` / `remove B` / `remove C` | A19 5:1 |
| V43 | recipes, one: `set B output x -> z`; two: `remove B` | A11 two:3:8. Also with one: `add #3 D`; two: `remove D` |
| V44 | recipes: `set A output "hp1,qty=3" -> "hp1,qty=5"` | output is the 11 bytes `"hp1,qty=5"`; digest `4b33b74c4177af68b21ec335b0770dbc7c8fb4a55116853da13d45e53c4a8769` |
| V45 | one: `set axe lvl 1 -> 4`; two: `set axe lvl 1 -> 6` | A06 two:3:13, found `4`, writer `one.d2patch:3` |
| V46 | one: `set axe lvl 1 -> 4`; two: `set axe lvl 4 -> 6` | applies; N01 two:3:13 (previous writer `one.d2patch:3`); change log (one:3, `1`→`4`), (two:3, `4`→`6`) |
| V47 | `a.d2patch`: `add #5 spr` / `set spr lvl [] -> 2` / `set axe lvl 1 -> 2`; then `b.d2patch`: `set axe lvl 1 -> 3` | A09 a:3:5; N03 a:4:1, 2 skipped; N04 (`b.d2patch` not applied) |
| V48 | `set axe lvl 9 -> 2` / `set ax2 lvl 5 -> 6` / `set clb lvl 1 -> 2` | A06 3:13 and A04 5:5 (line 4 is applied, but the layer fails) |
| V49 | one: `addcol mod:bonus` / `set axe mod:bonus [] -> 5`; two: `delcol mod:bonus` | header and cells equal the base again (digest `2c6f65f2…`); N05 two:3:8 |
| V50 | `delcol lvl` | A15 3:8 |
| V51 | `addcol mod:bonus` / `delcol mod:bonus` | A18 4:8. one: `addcol mod:bonus`; two: `delcol mod:bonus` / `addcol mod:bonus`: N05 two:3:8, A18 two:4:8 |
| V52 | line 2 `table nosuch`, lines 3–4 two `set`s, line 5 `table items`, line 6 `set axe lvl 1 -> 2` | A01 2:7; N03 3:1, 2 skipped; line 6 runs |
| V53 | `set #0 axe code axe -> zz1` / `set zz1 lvl 1 -> 2` | A21 4:5 |
| V54 | `check axe lvl 1` / `check axe lvl 2` | A06 4:15 |
| V55 | `set #4 clb code clb -> x` / `set axe lvl 1 -> 2` | A03 3:5; N03 4:1 (a failed key-column `set` is structural) |

### Stack vector (two layers, render and digests)

`one.d2patch`:
```
d2patch 1
table items
set axe lvl 1 -> 4
addcol mod:bonus
set #1 clb mod:bonus [] -> 7
```
`two.d2patch`:
```
d2patch 1
table items
set axe lvl 4 -> 5
add #4 spr like axe sha:73706f637e0f52e5
set spr name Axe -> Spear

table recipes
set A output "hp1,qty=3" -> "hp1,qty=5"
```
- The pin stream of `axe` at `two` line 4 is
  `name\tAxe\ncode\taxe\nlvl\t5\ndam@1\t3\ndam@2\t0\n` (`mod:bonus` is
  empty) → `sha:73706f637e0f52e5`.
- Report: one note, N01 two:3:13 (previous writer `one.d2patch:3`).
- `render(items)` =
  `name\tcode\tlvl\tdam\tdam\t*note\tmod:bonus\r\nAxe\taxe\t5\t3\t0\t\t\r\nClub\tclb\t1\t2\t0\told\t7\r\nAxe\tax2\t5\t7\t0\t\t\r\nBig Club\tclb\t9\t8\t1\t\t\r\nSpear\tspr\t5\t3\t0\t\t\r\n`,
  digest `4db676519ceba19b534e3329fea06a94600ac7942b31c362b59fd4ff4723f7ce`.
- recipes `4b33b74c4177af68b21ec335b0770dbc7c8fb4a55116853da13d45e53c4a8769`;
  flags unchanged; data digest
  `2bc875e90168b3afccd0bf6aa7999253dc3e2d3478410c7d9cb227bda82bf220`.
- Row #4: origin `Added(two,4)`; writers name `Set(two,5)`, code
  `Add(two,4)`, the other five `Copy(two,4)`. Row #0 `mod:bonus` writer
  `AddCol(one,4)`; #1 `mod:bonus` writer `Set(one,5)`.
- Change log: (one:3 #0 lvl `[1]`→`[4]`), (one:5 #1 mod:bonus
  `[]`→`[7]`), (two:3 #0 lvl `[4]`→`[5]`), (two:5 #4 name
  `[Axe]`→`[Spear]`), (two:8 recipes #0 output).

### Stack-file vectors

| File | Result |
|---|---|
| `d2stack 1\nlayer core/10-items.d2patch\nlayer [core/20-balance.d2patch]\n` | two layers, in that order |
| `d2stack 1\nlayer a.d2patch\nlayer a.d2patch\n` | S05 3:7 (related: line 2) |
| `layer ../a.d2patch`, `layer A.d2patch`, `layer a.txt`, `layer ./a.d2patch`, `layer a//b.d2patch`, `layer /a.d2patch` on line 2 | S04 2:7 each |
| `d2stack 2\n` | S02 1:9 |
| `d2patch 1\n` | S01 1:1 |
| `d2stack 1\nlayers a.d2patch\n` | S03 2:1 |

### Diff vectors

Before: base `items`. Edited file: header
`name|code|lvl|dam|dam|*note|mod:bonus` and rows
`Axe|axe|4|3|0|""|5`, `Club|clb|1|2|0|""|""`, `Axe|ax3|5|7|0|""|""`,
`Big Club|clb|9|8|1|""|""`, `Spear|spr|4|3|0|""|5`, `Mace|mce|3|""|""|""|""`.
Output, exactly (LF after every line):

```
d2patch 1

table items
addcol mod:bonus
set axe lvl 1 -> 4
set axe mod:bonus [] -> 5
set #1 clb *note old -> []
set #2 ax2 code ax2 -> ax3
add #4 spr like axe sha:f1c4537a4b0c139c
set #4 spr name Axe -> Spear
add #5 mce
set #5 mce name [] -> Mace
set #5 mce lvl [] -> 3
```

- `clb` occurs twice, so row 1 uses `#1 clb`. Row 2 is renamed, so it
  uses `#2 ax2` (A21).
- Template of `spr`: row 0 scores 4 (`lvl`, `dam@1`, `dam@2`,
  `mod:bonus`) of 5 non-empty non-key cells. Its pin stream is
  `name\tAxe\ncode\taxe\nlvl\t4\ndam@1\t3\ndam@2\t0\nmod:bonus\t5\n`.
  `mce` has no template (best score 0).
- Applying the output reproduces the edited table; digest
  `8de5415da7311b6742cdacfb2d0244198415167c6c4d1986b43773ca4ae7e076`.

| Edited file (from base `items`) | Result |
|---|---|
| unchanged | `d2patch 1\n` |
| only rows 0–2 | D01 |
| header `name\|code\|X\|lvl\|dam\|dam\|*note` | D02 (`lvl` is a base column) |
| a cell `A]x` | D03 |
| a cell with byte 0xE9 | D03 |
| a new column `x y` | D04 |
| a new column `speed`, all empty | `d2patch 1\n\ntable items\naddcol speed\n` |
| a row `Spear\|spr\|2\|""\|""\|""` inserted at index 1 | D05 at row 2 |
| row 1's code changed to `axe` | D05 at row 1 |
| an appended row with code `axe` | D07 |
| row 0's code emptied | D07 |
| before = state after one: `addcol mod:bonus` / `set axe mod:bonus [] -> 5`; edited header replaces `mod:bonus` with `mod:tier`, row 1 `mod:tier` = 2 | `delcol mod:bonus` / `addcol mod:tier` / `set #1 clb mod:tier [] -> 2`; applying it gives N05 and the edited table |

### Game-file vectors (`#[ignore]`, read `D2_GAME_DIR`)

| Id | Input | Expected |
|---|---|---|
| G1 | every catalog table | reads without error; `render(read(f))` = `f` minus `Expansion` lines; row counts as in the catalog |
| G2 | every catalog table | `diff(read(f), f)` = `d2patch 1\n` |
| G3 | the Example stack | results as in the Example; digests on the reference install: weapons `d9f88badbf3f1bb77fb36d2e538ad462abd17d05373cbe14772f1fd018938baf`, armor `a3bd13b18286ee125b3bc7332d0dc663173439c12fce404aa2df08726e36dfad`, charstats `3e2994616bb359bd55772ad132feeb8023341394aea55ab80206cae6c1f9ac65`, monstats `56c3ef131f2dd21ed40c390035d741da891af1bab374347703da98966fa40708`, magicprefix `63b952e7aa3c56f02d23ab7939ee265bc8d5cb5c3380a02710964864f65f4d8a`, treasureclassex `0a0a4c940f8f3fb9e63b45fa805a02e5a156c92228738dcabfe277376989a7fc`, cubemain `7b17a50943d50f8c037e04f472bb67938d1253ab26f0724f3a648a7e5c6570b6` |
| G4 | monstats `set cr_lancer8 Level 5 -> 6` | A04, rows 617, 723 |
| G5 | monstats `set #618 cr_lancer8 Level 18 -> 19` | A05, found `blunderbore5`, rows labelled `cr_lancer8`: 617, 723 |
| G6 | armor `set buc mindam 1 -> 2` | A02, ambiguous (positions 63, 161) |
| G7 | magicprefix `set Sturdy levelreq 3 -> 2` | A04, rows 1, 142, 143 |
| G8 | magicprefix `add #668 X` | A09, expected `#669` |
| G9 | weapons `add #306 hax` | A16. `add #306 [HAX]` applies (codes are case-sensitive) |
| G10 | skills `set [Fire Bolt] reqlevel 1 -> 2` | applies. `[fire bolt]`: A03 |
| G11 | uniqueitems `set #401 [] lvl [] -> 1` | applies (row 401 is all empty) |
| G12 | itemtypes `set [] Equiv1 [] -> x` | A04, rows 0, 1, 14, 17, 23 |
| G13 | inventory `add #32 x` | A10 |
| G14 | weapons `set hax [] [] -> 1` | applies (the empty-named column 18) |
| G15 | 1,000+ seeded random edits over all catalog tables (cell edits, renames, appended rows, an extension column) | every edit that `diff` accepts round-trips (T-diff); random statement permutations of each output are identical or rejected (T-perm) |

## Design rationale

Three designs were scored (1 = poor, 5 = strong) against the project
requirements:

| Criterion | A: indexed, labelled, old-value-checked | B: line-per-cell, catalog key, optional expect | C: keyed, pinned, lock files |
|---|---|---|---|
| Exactness | 5: byte cells, no escapes needed | 4: codec equality, UTF-8 → 1252 mapping | 4: schema columns only, codec equality |
| Safety | 5: mandatory old values, errors only | 3: optional expects, last-wins warnings | 5: overrides, index pins, locks |
| Simplicity | 4: 5 keywords | 2: directives, placement, TOML, LCS diff | 2: ~60 codes, locks, namespaces, mirrors |
| Author ergonomics | 2: index on every line, no templates | 5: readable blocks, `.copy` | 3: one-line statements, `like` |
| Determinism | 5 | 4 | 5 |
| Versioning | 3: exact versions only | 4: migrations | 5: migrations, schemes |
| Total | 24 | 22 | 24 |

**Base: A.** A and C tie. A wins because it needs only the txt reader to
work: C needs a codec and a schema for every table before any patch can
apply, and its `.bin`-based models cannot be built (bins lack key text).
This spec keeps A's substrate and safety model and grafts C's keyed
addressing and templates and B's readability.

**Taken from A:** patching text cells, never `.bin` or typed records;
byte-exact cells and comparisons; bracket strings without escapes; the
mandatory old value as the single mechanism for drift and conflicts;
errors only, no warnings; pinned `add` indices; append-only tables with
removal opt-in and pristine rows only; `addcol` appending at the end;
`@k` for repeated column names; dropping `Expansion` rows so row `i` sits
on rendered line `i + 2`; A14; the `.d2stack` list; a diff that never
infers removals; the round-trip law.

**Taken from C:** a declared key per table with a uniqueness policy that
tolerates base duplicates; key selectors with an index fallback; one
self-contained statement per line; `like` templates with a row pin;
namespaced extension columns (`ns:name`); `delcol` limited to non-base
columns; blame of broken references through removal and rename logs;
SHA-256 table digests and the determinism proof obligations; reading old
format versions through migrations; `fmt`; template choice in diff.

**Taken from B:** `old -> new` readability; the catalog as part of the
format contract (changes need a version bump); collecting every
independent error of the failing layer; spreadsheet-mistake detection in
diff (D05); output only under `game/`.

**New in this spec (found while verifying the model):** A21 and A22
remove the two silent order dependences that sequential execution would
otherwise have (a key selector that hits a different row after a rename;
a template copied before or after a write to it). With them, no
permutation of a valid layer changes its result without an error (T-perm).
Uniqueness is checked at the end of a layer, so swaps work. Addable field
columns are opt-in because adding a field column changes the compiled
value of every existing row.

**Rejected:**
- B's last-wins with warnings and `--strict`: warnings get ignored; a
  stated old value already makes every override explicit.
- B's optional expectations: unexpressed cells drift silently.
- B's and C's snapshot semantics (all statements against the state before
  the layer): they forbid editing a new row after its `add` and hide
  renames. Sequential execution plus A08, A21 and A22 gives the same order
  independence.
- B's mid-table inserts with an index-shift opt-in: row indices are
  identities (combined index spaces, saved data); appends only.
- B's and C's codec-based value equality: it couples patch validity to
  table codecs, and a codec change would change v1 results.
- C's `.bin` base and schema-only columns: bins store the 21 name keys as
  indices; comment columns would vanish; text compiles to the bins
  exactly.
- C's mirror (derived Id) columns and B's id autofill: hidden writes;
  values are explicit and table invariants (R06) check them.
- C's quoted strings with doubled quotes: 1,356 base cells contain `"`
  (1,927 across all three archives), for example every `cubemain` input
  with a quantity.
- B's backtick quoting and Windows-1252 mapping: non-ASCII occurs only in
  2 unbound comment cells.
- B's kept separator rows: they would break `line = index + 2`.
- C's `layer#k` identities, `requires`, `overrides`, namespace ownership,
  layer ids and lock files; B's TOML manifests: stated old values already
  catch missing dependencies and overrides. Lock files can come later as a
  tool over the digests.
- A's per-section label column and index-only addressing: the
  requirements ask for stable keys.
- A's first-error-only report: a drifted base can fail many independent
  lines, and authors need them all at once.

## Provenance

- **The format** (grammar, semantics, codes, tools) is our own design.
  It was checked by a scratch Python model of this spec (session
  scratchpad, not committed). The model produced every fixture vector above
  and ran the stress tests: 1,183 accepted random diffs on the 86 base
  tables all round-trip; 1,521 valid statement permutations give identical
  results, 1,694 are rejected, none differ.
- **1.14d `Game.exe`** (Ghidra exports in `re/exports/funcs`, checked in
  this session):
  - `0x006BD640` (txt splitter) compares each line's first cell with
    `Expansion` using a length of 10, so only an exact match is removed,
    and it rejects a header of 281 or more columns.
  - `0x006BCE20` (column binding) uses a case-folding hash and a
    case-insensitive compare: field names bind columns ignoring case.
  - The rest comes from the sibling specs, which give their own addresses:
    `.bin`-only normal play (`loading.md` §3, `0x006122F0`), load order,
    post-load checks and combined index spaces (`loading.md` §6, §8, §9),
    and the key comparisons of the code and name linkers (`field-types.md`
    §5–§6, `0x6BD230`, `0x6BD5A0`, `0x4135D0`, `0x4113C0`).
- **Own-key columns:** from the 92 field lists rebuilt from the 1.14d
  loaders by the `field-types.md` session (column, type, offset; dumped in
  that session's scratchpad). This session re-read that dump: 37 tables
  have an own-key field of type 10 (code) or 17/18 (name); the column each
  binds is in the catalog. The 3,499 field names in those lists contain no
  `:`, `@`, `[` or `]` and are at most 25 bytes.
- **1.14d data** (scratch Python scripts over the extracted tables, base
  sources per §2):
  - All 86 catalog tables read, are rectangular, and satisfy the render
    identity. Row counts, column counts, key duplicates and empties are in
    the catalog table.
  - Over all 193 `.txt` files of the three archives (the free-text
    `Aiparms.txt` checked as raw bytes): no header or cell contains `[`,
    `]`, `@`, `^`, `{`, `|`, `}` or `~`; no header contains `:`; no cell
    contains a control byte; the longest cell is 199 bytes (`skills`
    `EDmgSymPerCalc`).
  - Name keys in `.bin` files: for 18 of the 21 name-key tables no key text
    of 4 or more bytes occurs anywhere in the `.bin`; in `missiles` (13 of
    684), `overlay` (8 of 293) and `pettype` (6 of 20) some names also
    appear inside other text fields.
  - Effective base: 81 cells with edge spaces; 2 cells with bytes ≥ 0x80
    (both comment columns); repeated header names only in `armor`,
    `automap` and `chartemplate`; one empty header name (`weapons`
    column 18); no key column is repeated in its header.
  - Real values used in the vectors (hax 3/6, Sturdy levelreq 3, Jagged
    `dmg%` 10/20, cr_lancer8 Level 5 at #617 and #723, blunderbore5 Level 18
    at #618, Act 1 Good group 5 Picks 1 at #209, buc mindam 1/3 against
    0/0) were read from the tables.
- **D2MOO** was not used for this spec beyond what the sibling specs took
  from it.

## Open questions

1. **Catalog entries are provisional.** The rows policy of every table is
   a default, not a confirmed fact: whether 1.14d code tolerates appended
   rows in tables marked `append` (for example `charstats`, `experience`,
   `plrmode`, `monmode`, `bodylocs`, `elemtypes`, `hitclass`, which look
   like engine enumerations) is not examined. Each table spec must confirm
   or tighten its entry.
2. `treasureclassex` is `unique` with `kind` `text`. Whether 1.14d's
   treasure-class name lookup is case-sensitive, and which duplicate wins,
   is not examined (`field-types.md` open question 3 covers the TC linker).
3. Which tables could ever be `free` (nothing refers to their rows by
   position). None is proven.
4. Whether 1.14d code refers to any row of a combined index space by a
   fixed index (`loading.md` §9). An append to `weapons`, `magicsuffix`,
   `raresuffix`, `plrtype` or `objtype` shifts the later parts; the
   `append` default for those five tables assumes no such constant exists.
5. Mod mode needs the complete compiler: calc expressions
   (`calc-expressions.md`), the table-specific callbacks of `monstats`,
   `monstats2`, `monpreset` and `cubemain`, and the TC linker
   (`field-types.md` open questions 1–3). Until then, `Ruleset::Mod`
   cannot compile those parts.
6. The policy for `monstats` #707 `NameStr` when compiling from text
   (`field-types.md` open question 7) decides whether the rulesets differ
   for an untouched `monstats`.
7. `field-types.md` §1 rejects a table whose header columns plus missing
   fields exceed 280. Whether extension columns count toward that limit
   (they bind no field) must be settled in that spec; `skills` has 256
   columns.
8. String-table layers (new item and affix names) are not designed. Until
   they exist, a layer cannot introduce a new string key (R03).
9. The save format (Phase 7) must store keys or its own identities, never
   combined indices (§11.1).
10. A user base that is not 1.14d (for example 1.13c) passes the ownership
   gate. Stated old values and pins detect drift in what a layer touches;
   whether the engine should also refuse such a base is a loader decision.
11. Layers contain single base values (old values, keys, labels, template
    choices). This spec treats them as facts allowed by rule 9; confirm
    with the project owner before the first release.
12. The client–server use of the data digest belongs to the protocol spec.
