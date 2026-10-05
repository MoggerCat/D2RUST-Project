# Spec: Data — Calc expressions (formula fields)

- **Status:** verified by `data-tool tables` (2026-10-05). A scratch model of the rules below compiles every
  formula cell of the 1.14d tables into byte-identical `misscode.bin`,
  `skillscode.bin`, `skilldesccode.bin` and `itemscode.bin`, and reproduces
  all 30,217 formula field values of `missiles.bin`, `skills.bin`,
  `skilldesc.bin`, `weapons.bin`, `armor.bin` and `misc.bin` (Provenance).
  Compiler (§4), folding evaluator and validator (§1.5) implemented in
  `d2-data::calc`; the d2rs compile reproduces the four buffers and every
  formula field byte for byte, and the validator reports exactly the one
  0x02 diagnostic (confirmed by bin cross-check). The run-time evaluator
  (§3) is for `d2-sim`.
- **Target version:** 1.14d
- **Crate/module:** `d2-data::calc` (bytecode decoder, validator, `.txt`
  compiler); `d2-sim::calc` (evaluator, later)
- **Related specs:** `specs/data/loading.md` (§4.3 code files, §6 load
  order, §7.2 compile-only links), `specs/data/txt-format.md` (§6 column
  binding, §7 name keys and callback text, §8 linkers),
  `specs/data/field-types.md` (§8.1 `calc(<buffer>)` fields, §8.2 `param`
  fields). Per-table record specs (skills, skilldesc, missiles, items) take
  the column → offset lists of §1.2 from here.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 49–62 |
| Inputs | 63–73 |
| Outputs / state changes | 74–81 |
| Rules | 82–85 |
|   1. Where formulas live | 86–176 |
|   2. Bytecode | 177–222 |
|   3. Evaluator | 223–328 |
|   4. Compiler | 329–525 |
|   5. Code tables (compile-time links, 1.14d) | 526–560 |
| Constants & data dependencies | 561–578 |
| Randomness | 579–597 |
| Edge cases & original bugs | 598–607 |
| d2rs policy (proposed, not yet logged in `docs/PLAN.md`; 1, 2, 4, 6 implemented) | 608–638 |
| Test vectors | 639–640 |
|   Real 1.14d formulas (`#[ignore]`, need `D2_GAME_DIR`) | 641–680 |
|   Compiler, synthetic (skills family, 1.14d links) | 681–729 |
|   Compiler, other families | 730–744 |
|   Evaluator | 745–776 |
|   Validation | 777–788 |
| Provenance | 789–846 |
| Open questions | 847–881 |
<!-- /index -->

## Summary

Some columns of `skills.txt`, `skilldesc.txt`, `missiles.txt` and the item
tables hold formulas such as `min(24,ln12)` or
`skill('Fire Arrow'.blvl) * 5`. When the game compiles a `.txt` (only in
`-txt` mode), it translates each formula into a short postfix bytecode,
appends it to one of four shared code buffers, and stores the byte offset in
the record (0xFFFFFFFF = no formula). The buffers ship as four raw files
(`*code.bin`). Game code evaluates an expression on a 64-entry stack of
32-bit integers, with callbacks that read skill and missile values and unit
stats. This spec defines where formulas live, the bytecode, the evaluator
and the compiler, exactly enough to rebuild the four 1.14d buffers byte for
byte from the 1.14d `.txt` files and to validate shipped or modded buffers.

## Inputs

| Name | Type | Source |
|---|---|---|
| code buffers | 4 raw files, no header | P: `data\global\excel\<name>.bin` (`loading.md` §4.3) |
| formula fields | u32 per calc column per record | `.bin` records (§1.2) |
| formula text | cell bytes, first 256 | calc columns of the `.txt` (compiler only) |
| name links | skills `skill`, missiles `Missile`, itemstatcost `Stat` | find-or-register linkers, `txt-format.md` §8 (compiler only) |
| code links | skillcalc `code` (73), misscalc `code` (43) | code linkers, `txt-format.md` §8 (compiler only) |
| evaluation context | per family (§3.5) | `d2-sim` |

## Outputs / state changes

- Compiler: per formula cell, an expression (bytes ending in 0x00) or none;
  per family, the buffer; per field, the u32 value.
- Validator: accept or reject a buffer with its fields, plus diagnostics.
- Evaluator: one signed 32-bit result. It changes no state except RNG draws
  made by `rand` (§Randomness) and whatever the callbacks read.

## Rules

All integers are little-endian. "i8/i16/i32" are signed two's complement.

### 1. Where formulas live

#### 1.1 Families

| Family | Code file | 1.14d bytes | Expressions | Compiled from (load step, `loading.md` §6) | Calc columns |
|---|---|---|---|---|---|
| missiles | `misscode.bin` | 196 | 25 | `missiles` (8) | 7 |
| skills | `skillscode.bin` | 5,891 | 853 | `skills` (10) | 36 |
| skilldesc | `skilldesccode.bin` | 4,252 | 867 | `skilldesc` (11) | 42 |
| items | `itemscode.bin` | 158 | 45 | `weapons`, `armor`, `misc` (15–17) | 5 per table |

- skills and skilldesc share every compiler and evaluator rule but have
  separate buffers.
- The record files hold no formula bytes. Their sizes are exactly
  4 + count × record size: skills 204,208 = 4 + 357 × 572; skilldesc 63,652
  = 4 + 221 × 288; missiles 287,284 = 4 + 684 × 420; misc 64,028 =
  4 + 151 × 424. There is no trailing section.
- `d2exp.mpq` and `d2data.mpq` have no code files and no `skillcalc` /
  `misscalc` tables.

#### 1.2 Calc columns and field offsets (1.14d)

Each field is a u32 at the given byte offset of the record. Columns bind
case-insensitively (`txt-format.md` §6).

**missiles** (420-byte records): `SrvCalc1` 128, `CltCalc1` 132,
`SHitCalc1` 136, `CHitCalc1` 140, `DmgCalc1` 144, `DmgSymPerCalc` 224,
`EDmgSymPerCalc` 280.

**skills** (572): `prgcalc1` 56, `prgcalc2` 60, `prgcalc3` 64,
`auralencalc` 96, `aurarangecalc` 100, `aurastatcalc1`–`6` 104, 108, 112,
116, 120, 124, `passivecalc1`–`5` 164, 168, 172, 176, 180, `petmax` 192,
`sumsk1calc`–`sumsk5calc` 208, 212, 216, 220, 224, `cltcalc1`–`3` 276, 280,
284, `perdelay` 296, `calc1`–`calc4` 312, 316, 320, 324, `skpoints` 368,
`delay` 400, `ToHitCalc` 416, `DmgSymPerCalc` 472, `EDmgSymPerCalc` 528,
`ELenSymPerCalc` 548.

**skilldesc** (288): `ddam calc1` 24, `ddam calc2` 28, `p1dmmin` 36,
`p2dmmin` 40, `p3dmmin` 44, `p1dmmax` 48, `p2dmmax` 52, `p3dmmax` 56,
`desccalca1`–`6` 152–172, `dsc2calca1`–`4` 176–188, `dsc3calca1`–`7`
192–216, `desccalcb1`–`6` 220–240, `dsc2calcb1`–`4` 244–256,
`dsc3calcb1`–`7` 260–284 (each run steps by 4).

**items** (424; same for weapons, armor, misc): `calc1` 164, `calc2` 168,
`calc3` 172, `len` 176, `spelldesccalc` 184. In 1.14d only `misc.txt` has
these columns. `weapons.txt` and `armor.txt` lack all five, so all their
fields are 0xFFFFFFFF.

Non-empty cells in 1.14d: missiles 25, skills 853, skilldesc 868 (one is a
single space and compiles to nothing), misc 45.

**Not formulas.** Other fields of type 25 (`CALCTODWORD`) use a different
callback: the item property parameters (magic affixes, uniqueitems, sets,
setitems, gems, runes). They store a number or a skills/montype/states
index (`field-types.md` §8.2). No other table, `properties.txt` included,
has formula columns.

#### 1.3 Field values

- 0xFFFFFFFF: no expression (missing column, empty cell, text that compiles
  to nothing, or a compile failure, §4.1).
- Any other value: the byte offset of the expression's first byte in the
  family's buffer. 0 is valid (the first expression).

#### 1.4 Buffer layout and build order

- A buffer is the concatenation of expressions, each ending with its END
  byte (§2.2). No header, count, alignment or padding.
- Append order = compile order: tables in load order (items: weapons, then
  armor, then misc); records in order; within a record, calc columns in
  `.txt` column order, left to right (`txt-format.md` §7 pass 2).
- Every successful compile is appended, even when its bytes equal an
  earlier expression. Nothing is shared. In 1.14d each expression is
  referenced by exactly one field, and the sorted field values are exactly
  the expression starts.

#### 1.5 Validation (d2rs; the original checks nothing)

1. Decode the buffer from offset 0 as consecutive expressions (§2.2). The
   last one must end exactly at the buffer end. Every opcode must be one the
   compiler can emit: 0x00, 0x01, 0x02, 0x04–0x16. Operands must fit inside
   the buffer.
2. Every formula field must be 0xFFFFFFFF or an expression start.
3. `misscode` must not contain CALL 2 (`rand`; policy 6, Open question 1).
4. Diagnostics, not errors: an expression start referenced by no field or
   by more than one; opcode 0x02 (§4.7); a CALL index without an evaluator
   entry (§3.4).

All four 1.14d buffers pass. The only diagnostic is one 0x02 (skills
`Fire Wall` `EDmgSymPerCalc`, Test vectors).

### 2. Bytecode

#### 2.1 Opcodes

"Pops/pushes" is the stack effect in the evaluator (§3.3). "1.14d" counts
occurrences in the 1,790 expressions of the four 1.14d buffers.

| Op | d2rs name | D2MOO name | Operand | Pops → pushes | Compiler emits | 1.14d |
|---|---|---|---|---|---|---|
| 0x00 | END | `AST_None` | — | stops, result = top | last byte of every expression | 1,790 |
| 0x01 | CALL | `AST_CallbackTable` | u8 function index | arity → 1 | function call | 531 |
| 0x02 | PAREN | `AST_Parenthesis_Open` | — | stops, like END | unclosed `(` only | 1 |
| 0x03 | — | `AST_Callback_Param_Separator` | — | stops | never | 0 |
| 0x04 | PARAM8 | `AST_Callback_Param_UInt8` | u8 (zero-extended) | 0 → 1 | name, value −128…127 | 1,606 |
| 0x05 | PARAM16 | `AST_Callback_Param_UInt16` | i16 (sign-extended) | 0 → 1 | name, value in i16 | 0 |
| 0x06 | PARAM32 | `AST_Callback_Param_UInt32` | i32 | 0 → 1 | name, larger value | 0 |
| 0x07 | INT8 | `AST_Raw_Int8` | i8 | 0 → 1 | constant −128…127 | 1,403 |
| 0x08 | INT16 | `AST_Raw_Int16` | i16 | 0 → 1 | constant in i16 | 234 |
| 0x09 | INT32 | `AST_Raw_Int32` | i32 | 0 → 1 | larger constant | 0 |
| 0x0A | LT | `AST_LessThan` | — | 2 → 1 | `<` | 14 |
| 0x0B | GT | `AST_GreaterThan` | — | 2 → 1 | `>` | 1 |
| 0x0C | LE | `AST_LessOrEqualThan` | — | 2 → 1 | `<=` | 0 |
| 0x0D | GE | `AST_GreaterOrEqualThan` | — | 2 → 1 | `>=` | 0 |
| 0x0E | EQ | `AST_Equal` | — | 2 → 1 | `==` | 0 |
| 0x0F | NE | `AST_NotEqual` | — | 2 → 1 | `!=` | 0 |
| 0x10 | ADD | `AST_Addition` | — | 2 → 1 | `+` | 321 |
| 0x11 | SUB | `AST_Substraction` | — | 2 → 1 | binary `-` | 79 |
| 0x12 | MUL | `AST_Multipliction` | — | 2 → 1 | `*` | 363 |
| 0x13 | DIV | `AST_Division` | — | 2 → 1 | `/` | 108 |
| 0x14 | POW | `AST_Power` | — | 2 → 1 | `^` | 0 |
| 0x15 | NEG | `AST_Negate` | — | 1 → 1 | unary `-` | 23 |
| 0x16 | COND | `AST_Ternary` | — | 3 → 1 | `? :` | 15 |
| 0x17 | — | `AST_Ternary_Colon` | — | stops | never | 0 |
| 0x18–0xFF | — | — | — | stops | never | 0 |

D2MOO's names are listed for cross-reference only; its `UInt8/16/32` names
do not match the 1.14d operand extension.

#### 2.2 Expression boundaries

An expression is a sequence of instructions ending with the first
instruction whose opcode is 0x00. Operand bytes are skipped, so an operand
byte 0x00 does not end it. Operand sizes: 0x01, 0x04, 0x07 → 1 byte; 0x05,
0x08 → 2; 0x06, 0x09 → 4; every other opcode → none. For buffer layout only
0x00 is a delimiter; the evaluator also stops at other opcodes (§3.3).

### 3. Evaluator

#### 3.1 Entry

`eval(family, offset, context)`:
1. If the family's buffer is absent, or `offset ≥ buffer length` (unsigned,
   so 0xFFFFFFFF always fails), the result is 0.
2. Otherwise run §3.3 over the bytes from `offset` to the buffer end.

Callers may test a field for 0xFFFFFFFF before evaluating and use a
fallback. Example: skills `ToHitCalc` = 0xFFFFFFFF → `ToHit +
(level − 1) × LevToHit` (record offsets 408 and 412), and 0 for level ≤ 0
whatever the field. Fallbacks belong to the table specs.

#### 3.2 Stack

- At most 64 values, each i32.
- Pop from an empty stack gives 0 and leaves it empty.
- Push onto a full stack (64 values) is dropped silently.
- Arithmetic wraps modulo 2³². Comparisons are signed.

#### 3.3 Instruction semantics

Repeat while the position is before the end: read the opcode, advance past
it and its operand, act:

| Op | Action |
|---|---|
| 0x01 | `i` = operand. If `i` < the family's function count (§3.4): `k` = that function's arity; pop `k` values (the last argument is popped first, so the arguments are in source order); call the function with them and the context; push its result. If `i` ≥ count, or `k` is not 0–3: push 0 and pop nothing. |
| 0x04–0x06 | Push `param(operand, context)`. Operand: 0x04 u8, zero-extended; 0x05 i16, sign-extended; 0x06 i32. |
| 0x07–0x09 | Push the operand: i8 and i16 sign-extended, i32. |
| 0x0A–0x0F | `b` = pop, `a` = pop. Push 1 if `a < b` / `a > b` / `a ≤ b` / `a ≥ b` / `a = b` / `a ≠ b`, else 0. |
| 0x10–0x12 | `b` = pop, `a` = pop. Push `a + b` / `a − b` / `a × b`. |
| 0x13 | `b` = pop, `a` = pop. `b` = 0 → push 0. Otherwise push `a / b` truncated toward zero. `a` = −2³¹, `b` = −1 makes the original fault (process crash). |
| 0x14 | `b` = pop (exponent), `a` = pop (base). `b` ≤ 0 → push 1. Otherwise push the wrapping product of `b` copies of `a` (equal to wrapping `a^b`). |
| 0x15 | Push −pop (−(−2³¹) = −2³¹). |
| 0x16 | `f` = pop, `t` = pop, `c` = pop. Push `t` if `c` ≠ 0, else `f`. Evaluation continues with the next instruction. |
| any other (0x00, 0x02, 0x03, 0x17–0xFF) | Stop. Result = pop (0 if empty). |

If the position reaches the end without a stop, the result is 0. An
operand that does not fit before the end (CALL's index byte included)
also stops with result 0. (The original checks only that one byte follows
0x04–0x09, so a cut 2- or 4-byte operand reads past the buffer; validated
buffers, §1.5, contain none.)

#### 3.4 Functions and keywords

The compiler recognizes keywords; the evaluator has a function table per
family. Indices are shared.

| Index | Keyword | skills / skilldesc | missiles | items |
|---|---|---|---|---|
| 0 | `min` | arity 2 | arity 2 | arity 2 |
| 1 | `max` | 2 | 2 | 2 |
| 2 | `rand` | 2 | 2 | 2 |
| 3 | `skill` (items: `stat`) | 2 | 2 | 2 |
| 4 | `miss` | 2 | keyword only, no table entry | — |
| 5 | `stat` | 2 | — | — |
| 6 | `sklvl` | 3 | — | — |
| | function count | 7 | 4 | 4 |

- Keywords match case-insensitively (compare at most 32 bytes).
- Compiler arity: skills/skilldesc 3 for index 6, else 2; missiles and
  items 2. These equal the evaluator table arities.
- A missile formula `miss(…)` compiles to CALL 4, which evaluates to 0
  without popping its arguments (§3.3). No 1.14d formula does this.

#### 3.5 Context and callbacks (interface for d2-sim)

The evaluator passes one context to the parameter callback and to every
function. This section fixes the interface; the values behind it belong to
other specs (all still to write): special values (what `ln12`, `dm56`,
`edmn`, `dl12` … mean; §5 gives the indices) to the skills and missiles
specs, stat values to the stats spec, the seed to the RNG spec.

| Family | Context | Parameter callback `param(c)` |
|---|---|---|
| skills, skilldesc | caster unit, skill id, skill level | special value `c` (skillcalc index, low 8 bits) of the context skill at the context level |
| missiles | missile unit, owner unit, missile id, missile level | missile special value `c` (misscalc index) of the context missile |
| items | unit, item unit | always 0 |

"No unit" = the context's unit is absent; "no context" = the caller passed
no context at all. Items functions read only the unit; which unit each
caller passes belongs to the items spec (Open question 6).

Functions:
- `min(a, b)`, `max(a, b)`: signed minimum / maximum.
- `rand(a, b)`: no context → 0. `a ≥ b` → `a`, no RNG draw. Otherwise
  `a + R(b − a + 1)` (§Randomness) on the unit's own seed
  (skills, skilldesc: the caster; items: the unit). Missiles: Open
  question 1.
- skills `skill(s, c)`: `L` = the context unit's level in skill `s`,
  bonuses included (Open question 8); 0 if there is no unit or the unit
  lacks the skill. Result: special value `c` of skill `s` at level `L`.
- skills `miss(m, c)`: missile special value `c` (misscalc index) of
  missile `m`, with the context unit as owner and the context level.
- skills/items `stat(s, mode)`: no unit, `s` < 0 or `s` ≥ itemstatcost
  count → 0. Otherwise the unit's stat `s`, read through one of four
  getters: `s` = 19 (`tohit`) its own getter whatever the mode; else
  mode 1 (`base`), mode 2 (`mod`), any other mode (`accr`, 0) one each
  (stats spec; Open question 5).
- skills `sklvl(s, c1, c2)`: `L` = special value `c1` of the context skill
  at the context level; result = special value `c2` of skill `s` at level
  `L`.
- missiles `skill(s, c)`: as skills `skill`, using the owner unit.

### 4. Compiler

#### 4.1 Cell to field

For each calc column of each record, in the order of §1.4:
1. Missing column → field 0xFFFFFFFF.
2. Text = the cell's first min(L, 256) bytes (`txt-format.md` §7).
3. Compile the text (§4.2–§4.6).
4. Result "fail", or no instruction produced → field 0xFFFFFFFF, nothing
   appended. Otherwise field = current buffer length; append the bytes.

#### 4.2 Characters

- space: 0x09–0x0D, 0x20. digit: `0`–`9`. letter: `A`–`Z`, `a`–`z`.
  alnum: digit or letter. `_` is not alnum.
- Bytes ≥ 0x80: the original passes them to the C library's class tests as
  negative values (undefined; Open question 2). d2rs: a byte ≥ 0x80
  anywhere in the text (the first 256 cell bytes, even after a stop) is an
  error and the table does not compile (Open question 10). 1.14d formula
  cells are ASCII.

#### 4.3 Tokens

At the current position:
1. Skip every space and `"` (0x22). (`"` therefore acts as a space outside
   quoted names, so `"min(24,ln12)"` reads as `min(24,ln12)`.)
2. Read one token:

| Starts with | Token |
|---|---|
| digit | **number**: the maximal digit run. Value = decimal, accumulated as `v = v × 10 + d` in wrapping i32 (`2147483648` → −2³¹, `4294967297` → 1). No sign, point or hex. |
| `'` | **quoted name**: the bytes up to the next `'` or the end of the text. A closing `'` is consumed. The first 255 bytes are kept. Resolved by §4.4. |
| letter | **word**: the maximal alnum run (first 255 bytes kept). Then skip spaces. If the next byte is `(` and the word is a keyword of the family (§3.4), the token is a **function**, and the `(` is consumed. Otherwise it is a **name**, resolved by §4.4 (the skipped spaces stay consumed). |
| `.` | **suffix**: the alnum run after the `.` (may be empty). Resolved by §4.4. Index ≥ 0 → a constant with that index, whatever kind §4.4 returns. Index −1 → **stop**. |
| `(` `)` `,` `+` `-` `*` `/` `^` `?` `:` | that symbol |
| `<`, `>` | `<=` / `>=` when followed by `=`, else `<` / `>` |
| `=`, `!` | `==` / `!=` when followed by `=`, else **stop** |
| anything else, or end of text | **stop** |

3. **Stop** ends tokenizing. The rest of the text is ignored, and
   compilation finishes normally (§4.6 End) with what was read.

A quoted name or name resolves to (index, kind). Index ≥ 0 and kind
constant → a **constant** token; index ≥ 0 and kind parameter → a
**parameter** token; index −1 → a constant token with value 0.

#### 4.4 Name resolution

The **context** is the top entry of the pending-op stack (§4.5) when that
entry is a function, with that function's index; otherwise there is no
context. Only the top entry counts: in `skill(('X').lvl)` the top is `(`,
so `'X'` resolves without context.

Lookups:
- **Name lookup** in a name link: key = the name's first 31 bytes, `A`–`Z`
  lowercased (`txt-format.md` §7 name key). Hit → (index, constant).
  Miss → −1.
- **Code lookup** in a code link: code = the name's first 4 bytes, padded
  with spaces to 4. Exact, case-sensitive match (`txt-format.md` §8).
  Hit → (index, parameter). Miss → −1. Only the first 4 bytes count:
  `par34` finds `par3`, `ln123` finds `ln12`; `LVL` misses.
- **Stat mode**: the whole name, ASCII case-insensitive: `base` →
  (1, parameter), `mod` → (2, parameter), any other name (`accr`
  included) → (0, parameter). Whole-name compare: Open question 9.

| Family | Context | Resolution |
|---|---|---|
| skills, skilldesc | `skill(` or `sklvl(` | skills name lookup; if it misses, skillcalc code lookup |
| | `miss(` | missiles name lookup; if it misses, misscalc code lookup |
| | `stat(` | itemstatcost name lookup; if it misses, stat mode |
| | none, `min(`, `max(`, `rand(` | skillcalc code lookup |
| missiles | `skill(` | skills name lookup (the compile-only skills link, `loading.md` §7.2); if it misses, skillcalc code lookup |
| | `miss(` | missiles name lookup; if it misses, misscalc code lookup |
| | none, other functions | misscalc code lookup |
| items | `stat(` | itemstatcost name lookup; if it misses, stat mode |
| | anything else | (0, parameter): every name compiles to PARAM8 0 |

Links used: skills `skill` column, missiles `Missile`, itemstatcost `Stat`
(find-or-register name links; in 1.14d none has duplicate keys, so link
index = record index); skillcalc and misscalc `code` (code links, §5).

#### 4.5 Parser state

| State | Meaning | Initial |
|---|---|---|
| `out` | instruction bytes so far | empty |
| `ops` | pending-op stack, at most 64 entries: `(`, a function (with its index), or an operator 0x0A–0x16 | empty |
| `count` | number of values the code in `out` leaves on the stack | 0 |
| `pending` | the last relevant token produced a value | false |
| `called` | a function or parameter token was seen | false |

**Strength** of a pending entry, and **number** of an arriving operator:

| Entry / operator | Strength (when pending) | Number (when arriving) |
|---|---|---|
| end of text | — | 0 |
| `(` | 2 | — |
| `,` (separator) | never pending | 3 |
| `<` `>` `<=` `>=` `==` `!=` (0x0A–0x0F) | 15 | its opcode |
| `+` `-` (0x10, 0x11) | 17 | its opcode |
| `*` `/` (0x12, 0x13) | 19 | its opcode |
| `^` (0x14) | 20 | 0x14 |
| unary `-` (0x15) | 21 | 0x15 |
| `?` (0x16) | 22 | 0x16 |
| `:` | never pending | 23 |
| function | never popped by an operator | — |

**Arity:** function: §3.4; 0x0A–0x14: 2; 0x15: 1; 0x16: 3; `(`: 0.

**Emit(op):** if `count` < arity(op): fail. Append the op byte.
`count += 1 − arity`.

**Push value(v, kind):** −128 ≤ v ≤ 127 → 0x07 (constant) or 0x04
(parameter) + 1 byte; −32,768 ≤ v ≤ 32,767 → 0x08 / 0x05 + i16; else
0x09 / 0x06 + i32. `count += 1`.

**Operator(N):**
1. While `ops` is not empty, its top `P` is not a function, and
   strength(P) ≥ N: pop `P` and emit it.
2. N = 3 (`,`) or 23 (`:`): done (nothing is pushed).
3. N = 0 (end): if `out` is empty: fail. Otherwise append 0x00.
4. Otherwise push N (fail if `ops` already holds 64 entries).

#### 4.6 Token actions

| Token | Action | `pending` after |
|---|---|---|
| number, constant | push value (constant) | true |
| parameter | push value (parameter); `called` = true | true |
| function | push the function entry (fail if 64 entries); `called` = true | true |
| `(` | push `(` (fail if 64 entries) | unchanged |
| `)` | close (below) | unchanged |
| `,` | Operator(3) | unchanged |
| `+` | Operator(0x10) | false |
| `-` | Operator(0x11) if `pending`, else Operator(0x15) | false |
| `*` `/` `^` | Operator(0x12 / 0x13 / 0x14) | false |
| `<` `>` `<=` `>=` `==` `!=` | Operator(0x0A / 0x0B / 0x0C / 0x0D / 0x0E / 0x0F) | false |
| `?` | Operator(0x16) | unchanged |
| `:` | Operator(23) | unchanged |

**Close** (`)`):
1. `ops` empty: fail.
2. Look at the top entry. `(` → pop it; done. A function → emit 0x01 (with
   the function's arity), append its index byte, pop it; done. An operator
   → pop and emit it; if `ops` is now empty: fail; repeat step 2.

**End** (after a stop or the end of the text):
1. Operator(0). Step 1 of Operator emits every pending entry above the
   topmost function. A pending `(` there is emitted as 0x02 with arity 0
   (so `count` grows by 1). Functions are never emitted at the end; their
   arguments stay as plain values.
2. **Folding:** if `called` is false, evaluate `out` (§3.3, no callbacks,
   no functions) to `v`, then replace `out` with push value(`v`, constant)
   followed by 0x00. A formula with no name, parameter or function is
   always a single constant.
3. The result is `out`.

**Fail:** any failure above → the field gets 0xFFFFFFFF.

Limits (original): `out` holds at most 1,024 bytes. Appending an op byte
needs `len(out)` < 1,024 before the write; push value needs `len(out)` <
1,024 (8-bit), `len(out)` + 3 < 1,024 (16-bit), `len(out)` + 5 < 1,024
(32-bit); otherwise fail. A 256-byte text produces at most about 512 bytes,
so only the 64-entry `ops` limit is reachable (e.g. 65 nested `(`).

#### 4.7 What the rules amount to (for mod authors and tests)

- Binding, tightest first: unary `-`, `^`, `* /`, `+ -`, comparisons. All
  binary operators are left-associative: `2^3^2` = 64, `10-4-3` = 3,
  `3>2>1` = 0. Unary `-` binds tighter than `^`: `-2^2` = 4.
- `/` truncates toward zero; `x/0` = 0. `a^b` with `b` ≤ 0 is 1.
- `c ? t : f` takes single operands: `c` is only the operand right before
  `?`; `t` and `f` must each be one number, name, function call or
  parenthesized group; an operator after `f` applies to the whole
  conditional. So `lvl < 4 ? 0 : 1` means `lvl < (4 ? 0 : 1)`,
  `1?2:3+10` = 12, `1?2+1:3` fails, `-1?2:3` = −2. Parenthesize the
  condition: `(lvl < 4) ? 0 : 1`. The `:` itself does nothing (Operator(23)
  emits and pushes nothing): `1?2 3` compiles like `1?2:3`.
- After `,`, `?`, `:` and a function name, `-` is binary: `min(-1,2)`,
  `max(1,-2)`, `min((-1),2)` and `1?-2:3` fail. Write `0-1`.
- `--5`, `+5`, `5*` fail (no unary `+`; two unary minus in a row pop each
  other). `- 5` = −5.
- Unknown names compile silently: to constant 0 in skills, skilldesc and
  missiles formulas outside `stat(`; to parameter 0 inside `stat(` and in
  items formulas. Code names: first 4 bytes, case-sensitive. Skill, missile
  and stat names: case-insensitive, first 31 bytes.
- A stop keeps the prefix: `5 $ 3` = 5, `1=1` = 1, `1.5` = 1,
  `skill('Fire Bolt'.LVL)` → `07 24 00` (the function is never closed).
- Missing `)`: an open function is dropped; an open `(` becomes 0x02, which
  stops evaluation where it sits (`lvl*(2` → `04 10 07 02 02 12 00`,
  value 2).
- Extra arguments stay on the stack (`min(1,2,3)` evaluates to
  min(2,3) = 2). A call fails only when the whole stack so far holds fewer
  values than its arity: `min(1)` and `stat('strength')` fail, but
  `1 min(2)` compiles as min(1,2).
- Two values with no operator: the last one wins (`5 3` = 3, `1,2` = 2).

### 5. Code tables (compile-time links, 1.14d)

Code links are built from the `code` column of `skillcalc.txt` and
`misscalc.txt` (P), registered in record order (`txt-format.md` §8). The
compiler needs only the links; the indices below are what the bytecode
holds (PARAM operands, and the code argument of `skill`, `miss`, `sklvl`).

**skillcalc** (73): 0 `ln12` · 1 `dm12` · 2 `ln34` · 3 `dm34` · 4 `ln56` ·
5 `dm56` · 6 `ln78` · 7 `dm78` · 8 `par1` · 9 `par2` · 10 `par3` · 11
`par4` · 12 `par5` · 13 `par6` · 14 `par7` · 15 `par8` · 16 `lvl` · 17
`edmn` · 18 `edmx` · 19 `edln` · 20 `toht` · 21 `mana` · 22 `mps` · 23
`math` · 24 `madm` · 25 `macr` · 26 `m1en` · 27 `m1ex` · 28 `m1el` · 29
`m2en` · 30 `m2ex` · 31 `m2el` · 32 `m3en` · 33 `m3ex` · 34 `m3el` · 35
`m1rn` · 36 `m2rn` · 37 `m3rn` · 38 `edns` · 39 `edxs` · 40 `ulvl` · 41
`blvl` · 42 `usmc` · 43 `m1eo` · 44 `m1ey` · 45 `m2eo` · 46 `m2ey` · 47
`me3o` · 48 `me3y` · 49 `enma` · 50 `exma` · 51 `edma` · 52 `enms` · 53
`exms` · 54 `len` · 55 `clc1` · 56 `clc2` · 57 `clc3` · 58 `clc4` · 59
`rng` · 60 `ast1` · 61 `ast2` · 62 `ast3` · 63 `ast4` · 64 `ast5` · 65
`ast6` · 66 `pst1` · 67 `pst2` · 68 `pst3` · 69 `pst4` · 70 `pst5` · 71
`pets` · 72 `skpt`

**misscalc** (43): 0 `par1` · 1 `par2` · 2 `par3` · 3 `par4` · 4 `par5` ·
5 `cpa1` · 6 `cpa2` · 7 `cpa3` · 8 `cpa4` · 9 `cpa5` · 10 `hpa1` · 11
`hpa2` · 12 `hpa3` · 13 `chp1` · 14 `chp2` · 15 `chp3` · 16 `dpa1` · 17
`dpa2` · 18 `lvl` · 19 `edmn` · 20 `edmx` · 21 `edln` · 22 `edns` · 23
`edxs` · 24 `damn` · 25 `damx` · 26 `dmns` · 27 `dmxs` · 28 `rang` · 29
`sl12` · 30 `sd12` · 31 `sl34` · 32 `sd34` · 33 `cl12` · 34 `cd12` · 35
`cl34` · 36 `cd34` · 37 `shl1` · 38 `shd1` · 39 `chl1` · 40 `chd1` · 41
`dl12` · 42 `dd12`

Codes compare as 4 bytes: a longer name is cut (`par34` → `par3`), a
shorter one is space-padded (`lvl ` is stored so), and the empty name
looks up `"    "`, which misses in 1.14d (no empty skillcalc or misscalc
code).

## Constants & data dependencies

| Constant | Value |
|---|---|
| keywords (index order) | `min`, `max`, `rand`, `skill`, `miss`, `stat`, `sklvl` (skills); first 5 (missiles); `min`, `max`, `rand`, `stat` (items) |
| stat modes | `base` 1, `mod` 2, else 0 |
| strengths | §4.5 (the 1.14d table at 0x6FC874 matches) |
| compile scratch / op stack / value stack | 1,024 bytes / 64 / 64 |
| callback text | first 256 bytes of the cell |
| name key / code width | 31 bytes lowercased / 4 bytes space-padded |
| no formula | 0xFFFFFFFF |

Compile-time dependencies, all loaded earlier in `loading.md` §6: skillcalc
and misscalc code links and the compile-only skills name link (step 1),
itemstatcost name link (step 6), missiles name link (step 8, the table
itself), skills name link (step 10, the table itself, for skills and
skilldesc).

## Randomness

`rand(a, b)` with `a < b` computes `a + R(n)`, `n = b − a + 1` (wrapping
i32), once per evaluation of that CALL. `R(n)` on a seed (lo, hi), two
u32:
1. `n` < 1 (the subtraction wrapped) → 0, no step.
2. Step: `x = lo × 0x6AC690C5 + hi` (64-bit); `lo = x mod 2³²`,
   `hi = x div 2³²`.
3. `n` a power of two → `lo & (n − 1)`; otherwise `lo mod n` (unsigned).

The seed is the unit's own RNG seed (§3.5; D2MOO `D2UnitStrc.pSeed`). No
other instruction draws. The compiler never draws: folding happens only
when no function was called. 1.14d uses `rand` once (skills `Imp Inferno`
`calc1`).

Vectors, seed (1, 0): `rand(1,6)` → 4, seed becomes (0x6AC690C5, 0);
`rand(0,7)` → 5 (power of two); `rand(5,5)` → 5 and
`rand(−2, 2147483647)` → −2, seed unchanged.

## Edge cases & original bugs

The original's quirks are rules here and are reproduced: evaluation
continues after COND (§3.3; D2MOO returns), missile `miss()` (§3.4),
stops, open parens and argument counts (§4.7), name contexts (§4.4),
4-byte codes (§5). Differences (d2rs policy): −2³¹ / −1 gives −2³¹ (the
original crashes, when folding or at run time; unreachable from 1.14d
data); a byte ≥ 0x80 is an error (§4.2); a missile formula with CALL 2 is
rejected (its seed source, Open question 1).

## d2rs policy (proposed, not yet logged in `docs/PLAN.md`; 1, 2, 4, 6 implemented)

1. d2-data loads the four code files as raw bytes (`loading.md`), validates
   them with their fields (§1.5), and exposes each formula field as
   "none" or an offset into its family buffer.
2. The `.txt` compiler implements §4 exactly. Acceptance test: compiling the
   1.14d `.txt` files reproduces the four code files byte for byte and
   every formula field value (30,217).
3. Under `Ruleset::Mod` every table is compiled from its final text
   (`patch-layers.md` §1), so all four buffers are rebuilt from scratch in
   §1.4 order, whatever a layer changed (formula text, or a key a formula
   resolves through: skills `skill`, missiles `Missile`, itemstatcost
   `Stat`, skillcalc / misscalc `code`). There is no append-only mode.
   With no layers the rebuild equals the shipped buffers (policy 2).
   Offsets are not stable across layers; nothing persistent (saves,
   protocol, traces) stores an offset.
4. Strictness: the compiler accepts and reproduces every original outcome
   and reports, per formula cell: `Fail` (a non-empty cell gives
   0xFFFFFFFF, §4.1 step 4), `Stop` (a stop before the end of the text,
   §4.3), `UnknownName` (a name or quoted name resolved to −1, §4.3),
   `OpenParen` (0x02 emitted, §4.6 End), `OpenFunction` (a function
   still open at End). 1.14d: one `OpenParen` (skills `Fire Wall`) and
   one `Fail` (skilldesc `revive`), nothing else. A byte ≥ 0x80 is an
   error (§4.2).
5. Evaluator: §3 exactly (it accepts any bytes; d2-data hands it
   validated buffers); −2³¹ / −1 gives −2³¹ (wrapping) instead of
   crashing; `^` uses wrapping exponentiation by squaring.
6. Missile `rand()`: a missile formula whose compiled bytes contain CALL 2
   is an error until the seed source is settled (Open question 1); no
   1.14d data uses it. The validator rejects it too (§1.5).

## Test vectors

### Real 1.14d formulas (`#[ignore]`, need `D2_GAME_DIR`)

Record = 0-based record index in that table. Field = u32 at the column's
offset (§1.2). Bytes = the buffer from Field to the END byte.

| Table, record, column | Text | Field | Bytes |
|---|---|---|---|
| missiles 12 `firearrow`, `DmgCalc1` | `dl12` | 0 | `04 29 00` |
| missiles 38 `poisonjav`, `SrvCalc1` | `0` | 6 | `07 00 00` |
| missiles 230 `immolationfire`, `EDmgSymPerCalc` | `skill('Fire Arrow'.blvl) * 5` | 24 | `07 07 07 29 01 03 07 05 12 00` |
| missiles 455 `moltenboulderfirepath`, `EDmgSymPerCalc` | `skill('Firestorm'.blvl)*8` | 78 | `08 E1 00 07 29 01 03 07 08 12 00` |
| missiles 648 `viper_poisjav`, `SrvCalc1` | `3` | 193 | `07 03 00` (last; buffer ends at 196) |
| misc 0 `elixir` (`elx`), `calc1` | `5` | 0 | `07 05 00` |
| misc 5 `Stamina Potion` (`vps`), `len` | `750` | 3 | `08 EE 02 00` |
| misc 94 `herb` (`hrb`), `calc1` | `25` | 155 | `07 19 00` (last; 158) |
| weapons, any record, `calc1` | (no column) | 0xFFFFFFFF | — |
| skills 7 `Fire Arrow`, `EDmgSymPerCalc` | `(skill('Exploding Arrow'.blvl)) * par8` | 0 | `07 10 07 29 01 03 04 0F 12 00` |
| skills 12 `Multiple Shot`, `calc1` | `"min(24,ln12)"` | 36 | `07 18 04 00 01 00 00` |
| skills 326 `BearSmite`, `calc2` | `"max(250,ln12)"` | 5,793 | `08 FA 00 04 00 01 01 00` |
| skills 282 `Imp Inferno`, `calc1` | `"rand(par3,par4)"` | 5,512 | `04 0A 04 0B 01 02 00` |
| skills 62 `Hydra`, `passivecalc1` | `stat('passive_fire_mastery'.accr)` | 1,212 | `08 49 01 07 00 01 05 00` |
| skills 123 `Conviction`, `aurastatcalc2` | `"-min(ln34,150)"` | 2,961 | `04 02 08 96 00 01 00 15 00` |
| skills 196 `FingerMageSpider`, `aurastatcalc1` | `-par3 * lvl` | 3,665 | `04 0A 15 04 10 12 00` |
| skills 70 `Raise Skeleton`, `petmax` | `(lvl < 4) ?lvl:(2+lvl/3)` | 1,464 | `04 10 07 04 0A 04 10 07 02 04 10 07 03 13 10 16 00` |
| skills 106 `Zeal`, `calc2` | `((lvl < 5) ? 0 : ((lvl-4) * par4) )+skill('Sacrifice'.blvl)*par8` | 2,508 | `04 10 07 05 0A 07 00 04 10 07 04 11 04 0B 12 16 07 60 07 29 01 03 04 0F 12 10 00` |
| skills 159 `MaggotEgg`, `calc1` | `"(lvl < 5) ? lvl : min(12,5+(lvl-5)/3)"` | 3,547 | `04 10 07 05 0A 04 10 07 0C 07 05 04 10 07 05 11 07 03 13 10 01 00 16 00` |
| skills 51 `Fire Wall`, `EDmgSymPerCalc` | `(skill('Warmth'.blvl)*par8+skill('Inferno'.blvl)*par7` | 950 | `07 25 07 29 01 03 04 0F 12 07 29 07 29 01 03 04 0E 12 10 02 00` |
| skills 78 `Bone Wall`, `calc2` | `par34` | 1,701 | `04 0A 00` |
| skilldesc 94 `firegolem`, `desccalca2` | `sklvl('Holy Fire'.ln56.edmn)` | 1,564 | `07 66 07 04 07 11 01 06 00` |
| skilldesc 166 `summon spirit wolf`, `dsc2calca1` | `(skill('summon fenris'.lvl) > 0) ? skill('summon fenris'.ln12) : 0` | 2,971 | `08 ED 00 07 10 01 03 07 00 0B 08 ED 00 07 00 01 03 07 00 16 00` |
| skilldesc 219 `royal strike`, `desccalca5` | `miss('royalstrikemeteorfire'.edns)*75/256` | 4,201 | `08 37 02 07 16 01 04 07 4B 12 08 00 01 13 00` |
| skilldesc 95 `revive`, `desccalcb2` | ` ` (one space) | 0xFFFFFFFF | — |

Whole-file checks: the four buffers decode into 25 / 853 / 867 / 45
expressions; field values match the expression starts one to one; the
longest expression is 51 bytes (skills); the deepest evaluation stack is 6
(`MaggotEgg` `calc1`). CALL use: skills `min` 46, `max` 1, `rand` 1,
`skill` 327, `stat` 2; skilldesc `min` 17, `skill` 115, `miss` 2,
`sklvl` 6; missiles `skill` 14; items none.

### Compiler, synthetic (skills family, 1.14d links)

Links used: `Fire Bolt` = skill 36, `firebolt` = missile 58, `strength` =
stat 0; codes from §5 (stubs of these suffice; no game files). "fail" =
field 0xFFFFFFFF.

| Text | Bytes |
|---|---|
| (empty), `   `, `""`, `()`, `,`, `min(` | fail (nothing produced) |
| `7` / `-7` / `- 5` | `07 07 00` / `07 F9 00` / `07 FB 00` |
| `300` / `-300` | `08 2C 01 00` / `08 D4 FE 00` |
| `100000` | `09 A0 86 01 00 00` |
| `2147483648` / `4294967297` | `09 00 00 00 80 00` / `07 01 00` |
| `2+3*4` / `(2+3)*4` / `10-4-3` | `07 0E 00` / `07 14 00` / `07 03 00` |
| `2^3^2` / `-2^2` / `2^-1` / `0^0` | `07 40 00` / `07 04 00` / `07 01 00` / `07 01 00` |
| `2*-3` / `7/2` / `-7/2` / `7/-2` / `5/0` | `07 FA 00` / `07 03 00` / `07 FD 00` / `07 FD 00` / `07 00 00` |
| `1<2` / `2>=2` / `2<=1` / `3==3` / `3!=3` / `3>2>1` | `07 01 00` / `07 01 00` / `07 00 00` / `07 01 00` / `07 00 00` / `07 00 00` |
| `lvl<2` / `lvl>2` / `lvl<=2` / `lvl>=2` / `lvl==2` / `lvl!=2` / `lvl^2` | `04 10 07 02` then `0A` / `0B` / `0C` / `0D` / `0E` / `0F` / `14`, then `00` |
| `1?2:3` / `0?2:3` / `1 < 2 ? 5 : 6` / `(1<2)?5:6` | `07 02 00` / `07 03 00` / `07 01 00` / `07 05 00` |
| `1?2:3+10` / `-1?2:3` / `(1<2)?(3+4):5*2` | `07 0C 00` / `07 FE 00` / `07 0E 00` |
| `1?2 3` / `0?2 3` / `(lvl<4)?lvl 3` | `07 02 00` / `07 03 00` / `04 10 07 04 0A 04 10 07 03 16 00` |
| `1?2+1:3`, `1?-2:3`, `1?2`, `--5`, `+5`, `5*`, `lvl+`, `1)` | fail |
| `1=1` / `1.5` / `5 $ 3` / `5 3` / `1:2` / `1,2` / `(` | `07 01 00` / `07 01 00` / `07 05 00` / `07 03 00` / `07 02 00` / `07 02 00` / `07 00 00` |
| `ln12` / `lvl` / `'lvl'` / `'lvl` / `"ln12"` / `.lvl` | `04 00 00` / `04 10 00` / `04 10 00` / `04 10 00` / `04 00 00` / `07 10 00` (a suffix is a constant) |
| `LVL` / `foo` / `a.b` / `''` | `07 00 00` (all four) |
| `par34` | `04 0A 00` |
| `1+ln12` / `ln12"+1"` | `07 01 04 00 10 00` / `04 00 07 01 10 00` |
| `min(3,5)` / `MIN(3,5)` / `min (3,5)` | `07 03 07 05 01 00 00` (all three) |
| `min(-1,2)`, `min((-1),2)`, `min(1)`, `skill(ln12)`, `stat('strength')` | fail |
| `1 min(2)` / `min"(1,2)` | `07 01 07 02 01 00 00` / `07 02 00` (only spaces may precede `(`) |
| 64 × `(` then `1` / 65 × `(` then `1` | `07 01 00` / fail |
| `5 $ \xE9` | error (§4.2) |
| `min(0-1,2)` | `07 00 07 01 11 07 02 01 00 00` |
| `max(2,0-1)` | `07 02 07 00 07 01 11 01 01 00` |
| `min(1,2,3)` | `07 01 07 02 07 03 01 00 00` |
| `min(1,2)min(3,4)` | `07 01 07 02 01 00 07 03 07 04 01 00 00` |
| `max(1,2` | `07 01 07 02 00` |
| `lvl(2)` / `lvl*(2` | `04 10 07 02 00` / `04 10 07 02 02 12 00` |
| `rand(1,6)` | `07 01 07 06 01 02 00` |
| `skill('Fire Bolt'.lvl)` | `07 24 07 10 01 03 00` |
| `skill('Fire Bolt'.blvl` | `07 24 07 29 00` |
| `skill('fire bolt'.LVL)` | `07 24 00` |
| `skill('No Such Skill'.lvl)` | `07 00 07 10 01 03 00` |
| `stat('strength'.base)` / `.BASE` / `.mod` / `.accr` | `07 00 07 01 01 05 00` / `07 00 07 01 01 05 00` / `07 00 07 02 01 05 00` / `07 00 07 00 01 05 00` |
| `stat('strength'.base)+1` | `07 00 07 01 01 05 07 01 10 00` |
| `stat('nosuchstat'.base)` | `04 00 07 01 01 05 00` |
| `miss('firebolt'.edmn)` | `07 3A 07 13 01 04 00` |
| `sklvl('Fire Bolt'.ln12.lvl)` | `07 24 07 00 07 10 01 06 00` |

### Compiler, other families

| Family | Text | Bytes |
|---|---|---|
| missiles | `dl12` / `lvl` | `04 29 00` / `04 12 00` |
| missiles | `miss('firebolt'.dl12)` | `07 3A 07 29 01 04 00` |
| missiles | `skill('Fire Bolt'.sl12)` | `07 24 00` (`sl12` is not a skillcalc code: stop) |
| missiles | `stat('strength'.base)`, `sklvl('Fire Bolt'.ln12.lvl)` | `07 00 00` (not keywords; folded) |
| items | `5` / `1.5` | `07 05 00` / `07 00 00` (`.5` is PARAM 0 as a constant; skills: `07 01 00`) |
| items | `lvl` / `lvl+1` | `04 00 00` / `04 00 07 01 10 00` |
| items | `stat('strength'.base)` | `07 00 07 01 01 03 00` |
| items | `min(stat('strength'.base),10)` | `07 00 07 01 01 03 07 0A 01 00 00` |
| items | `skill(1,2)` | `04 00 07 01 07 02 00` |
| items | `rand(1,3)` | `07 01 07 03 01 02 00` |

### Evaluator

Stub context for these rows: `param(c)` = `c`; skills functions `min`,
`max` real, `skill(s, c)` = 100·s + c, `sklvl(s, a, b)` = 10,000·s + 100·a
+ b; function count 7.

| Bytes | Result | Checks |
|---|---|---|
| `04 10 07 02 12 00` | 32 | PARAM8, MUL |
| Zeal `calc2` bytes (above) | 144,747 | COND then more ops (D2MOO's early return would give 132) |
| Fire Wall bytes (above) | 114,089 | 0x02 stops with the full sum on top |
| `07 66 07 04 07 11 01 06 00` | 1,020,417 | 3-argument CALL, source order |
| `04 02 08 96 00 01 00 15 00` | −2 | CALL then NEG |
| `07 05 07 00 13 00` | 0 | x / 0 |
| `07 F9 07 02 13 00` / `07 07 07 FE 13 00` | −3 / −3 | truncation toward zero |
| `07 FE 07 03 14 00` / `07 02 07 00 14 00` / `07 02 07 FF 14 00` | −8 / 1 / 1 | POW |
| `09 00 00 00 80 15 00` | −2,147,483,648 | NEG wraps |
| `07 FF 07 01 op 00` / `07 02 07 02 op 00` / `07 03 07 02 op 00` | op 0A: 1/0/0; 0B: 0/0/1; 0C: 1/1/0; 0D: 0/1/1; 0E: 0/1/0; 0F: 1/0/1 | signed comparisons, opcode order |
| `07 05 07 07 11 00` / `06 00 00 01 00 00` | −2 / 65,536 | SUB; PARAM32 |
| `07 03 07 64 14 00` / `09 00 00 01 00 09 00 00 01 00 12 00` | −818,408,495 / 0 | POW, MUL wrap |
| `09 00 00 00 80 07 FF 13 00` | −2,147,483,648 | d2rs policy 5 (the original crashes) |
| `10 00` | 0 | pops on an empty stack give 0 |
| `07 05 07 06 16 00` | 6 | COND with a missing condition (0) |
| `07 00 07 01 16 07 05 00` | 5 | COND result then another value |
| `07 05 07 06 01 09 00` | 0 | CALL index ≥ count pushes 0 |
| `04 FF 00` / `05 FF FF 00` / `07 FF 00` | 255 / −1 / −1 | PARAM8 zero-extends, PARAM16 and INT8 sign-extend |
| `07 05 03 07 06 00` / `07 05 17 07 06 00` / `07 05 FF 07 06 00` | 5 | unknown opcodes stop |
| `07 05 02 07 06 10 00` | 5 | PAREN stops |
| `07` / `08 05` / `01` / `07 05` | 0 | cut operand (§3.3); no END |
| 64 × `07 01`, then `07 02 00` | 1 | 65th push dropped |
| any family, offset 0xFFFFFFFF or ≥ buffer length | 0 | entry check |

### Validation

| Input | Expected |
|---|---|
| the four 1.14d buffers with their fields | valid; one 0x02 diagnostic |
| buffer `07 05 00 07` | invalid (last expression cut) |
| buffer `08 05` | invalid (operand past end) |
| buffer `07 05 03 00` | invalid (0x03 not emittable) |
| misscode buffer `07 01 07 02 01 02 00` with its field | invalid (CALL 2, §1.5) |
| buffer `07 05 00`, no field / items buffer `07 01 07 02 01 07 00` with its field / empty buffer, every field 0xFFFFFFFF | valid with an "unreferenced" diagnostic / valid with a CALL-index diagnostic / valid |
| buffer `04 29 00 07 00 00`, fields {0, 3} / {0, 1} / {0, 3, 3} | valid / invalid (1 is not a start) / valid with a "shared" diagnostic |

## Provenance

**1.14d `Game.exe`** (Ghidra exports; constants, jump tables and short
routines read from the PE bytes):

| Address | Role |
|---|---|
| 0x6C1AE0 | compiler: token loop, `pending` / `called` flags, `)` handling, end, folding. Epilogue bytes: success returns end − start (full length including END); failure writes 0 to the first byte and returns 0 |
| 0x6C11C0 | tokenizer: space and `"` skipping, numbers (`v·10 + c − 48`), quoted names, words with keyword test (≥ 0 = function), `.` suffix, two-char operators. Bytes: a character map at 0x6C1798 (codes 0–0x5E) selects 16 handlers (table 0x6C1754); every other byte, a lone `!`/`=` and an unresolved suffix return NULL (stop); quoted names and words set the token to 0x10 + (kind = parameter) |
| 0x6C18A0 | operator handling with the strength table at 0x6FC874 (24 bytes; §4.5) |
| 0x6C1820 | arity: functions via the family callback (default 2), 0x0A–0x14 → 2, 0x15 → 1, 0x16 → 3, else 0 |
| 0x6C19C0 / 0x6C1A50 | push constant / parameter; width by signed range; buffer checks |
| 0x6C0BC0 | evaluator, jump table at 0x6C1164 (opcodes 1–22; 2 and 3 share the stop handler). Confirmed from the bytes: 0x04 reads its operand zero-extended, 0x05, 0x07 and 0x08 sign-extended; 0x0A–0x0F are signed less, greater, less-or-equal, greater-or-equal, equal, not-equal; DIV is signed 32-bit division; pops of an empty stack give 0 |
| 0x6C0B80 / 0x6C0BA0 | push (drops at 64) / pop (0 when empty) |
| 0x611BD0, 0x611C70, 0x6619A0, 0x631530 | field callbacks (skills, skilldesc, missiles, items): 1,024-byte scratch, compile, append or 0xFFFFFFFF |
| 0x6118B0 | append to a growable buffer, returns the start offset |
| 0x611930, 0x661800, 0x631430 | keyword tests (`_strnicmp`, n = 32); strings at 0x6E6388–0x6E63B4 |
| 0x6119E0, 0x631490 | compiler arity (skills: 3 for index 6, else 2; items: 2); missiles pass none |
| 0x6119F0, 0x661880, 0x6314A0 | name resolution per family; 4-byte code builders 0x611760 / 0x6617B0; stat modes at 0x6E63C4 `base`, 0x6E63C0 `mod`, 0x6E63B8 `accr` |
| 0x613F80, 0x661B20, 0x6315D0 | field lists: calc columns and offsets of §1.2 (callback, type 25) |
| 0x612750 | compile-only links: skillcalc / misscalc `code` (type 10), skills `skill` (type 0x11) |
| 0x613E90 | code file load (whole file, size kept) |
| 0x646CA0, 0x646D00, 0x64B7C0, 0x627C20 | evaluator entries (skills, skilldesc, missiles, items): buffer present and offset < size, else 0 |
| 0x6449F0 | skills `ToHitCalc` fallback when the field is 0xFFFFFFFF |
| 0x745774 (7), 0x745828 (4), 0x74463C (4) | function tables (pointer, arity) |
| 0x646BE0, 0x64B6E0, 0x627B40 | parameter callbacks (skill special value 0x646460, missile special value 0x64B340, constant 0) |
| 0x6436E0, 0x6436F0, 0x643700, 0x646C00, 0x643740, 0x643770, 0x646C60 | skills `min`, `max`, `rand`, `skill`, `miss`, `stat`, `sklvl`. `rand` returns 0 when the context pointer is null. `skill`: skill entry 0x643810, level getter 0x6442A0 called with flag 1 |
| 0x64B700–0x64B760, 0x627B50–0x627BB0 | missiles and items functions; items `stat` (0x627BB0) reads the context's first word, getters 0x622560 (`tohit`), 0x6253B0, 0x625500, 0x625480 |
| 0x5BE3F0 (1 of 5 callers of 0x627C20) | items evaluation: second context word = the item (its class id selects the items record whose calc fields are evaluated) |
| 0x45C3E0 | seeded RNG step and range reduction; seed = unit + 0x20, low word first |

**1.14d data** (scratch scripts in the session scratchpad, not committed):
a model of §4 compiled every calc cell of P `missiles.txt`, `skills.txt`,
`skilldesc.txt`, `weapons.txt`, `armor.txt` and `misc.txt` with links built
from P `skills.txt`, `missiles.txt`, `itemstatcost.txt`, `skillcalc.txt` and
`misscalc.txt`. Results:
- All four buffers byte-identical to the P `*code.bin` files (196, 5,891,
  4,252, 158 bytes).
- All 30,217 formula field values equal the P `.bin` values: missiles
  4,788 (684 × 7), skills 12,852 (357 × 36), skilldesc 9,282 (221 × 42),
  items 3,295 (755 bound in misc, 2,540 missing-column fields in weapons and
  armor).
- Exercised by the data: constant folding (items, plain numbers), all three
  name contexts and fallbacks, `.` suffixes, `stat` modes, 3-argument
  `sklvl`, `min` as index 0, unary minus, COND, LT, GT, an unclosed `(`,
  4-byte code truncation (`par34`), a whitespace-only cell, missing columns.
  Not exercised (rules from the code only): POW, LE, GE, EQ, NE, PARAM16/32,
  INT32, failures other than "nothing produced", early stops (ignored
  trailing text), limits. The synthetic vectors come from the same model.

**D2MOO (MIT, 1.10f)** `Fog/src/Calc.cpp` and the D2Common calc callbacks
gave the names and the overall structure. 1.14d differs from D2MOO in four
places, all decided by the 1.14d code: (1) a word is a function when its
keyword index is ≥ 0 (D2MOO: non-zero, which would make `min` a name; the
data has `min(` → `01 00`); (2) evaluation continues after COND (D2MOO
returns); (3) `^` clears `pending` (D2MOO leaves it); (4) PARAM16 is
sign-extended (D2MOO: zero-extended).

## Open questions

1. Missile `rand()` (0x64B720) takes its seed address as context + 0x20,
   while the missile context block is 16 bytes on the stack of the entry
   routine 0x64B7C0, so the 8 seed bytes lie elsewhere in that routine's
   frame (possibly its own argument slots). What they hold, and whether the
   draw is deterministic, was not traced. Unused in 1.14d.
2. Bytes ≥ 0x80 in a formula: behavior of the 1.14d CRT class tests
   (`isspace`, `isdigit`, `isalpha`, `isalnum`) for negative `char`
   values was not examined. No 1.14d formula has one.
3. Compile failures other than "nothing produced" (operand count, `)`
   without `(`, 64 pending entries, buffer limits) are taken from the code;
   no 1.14d cell triggers them. Same for every POW/LE/GE/EQ/NE/PARAM16/
   PARAM32/INT32 path.
4. A missing code file at run time: the entries return 0 when the buffer
   pointer is null; whether the archive read leaves the pointer null (and
   the size defined) on a missing file was not traced. d2rs treats a
   missing code file as a load error (`loading.md` §4.3).
5. The semantics of the special-value functions (0x646460 skills,
   0x64B340 missiles) and of the stat getters behind `stat` are not part of
   this spec (skills, missiles and stats specs).
6. Items context: no items function reads the second word (the item, in
   the one caller checked); which unit each of the 5 callers passes as
   the first word (the one `stat` and `rand` use) was not traced.
7. Behavior when a skills special-value code is ≥ 256 (the callee takes
   the low 8 bits): only reachable with hand-made PARAM16 operands.
8. `skill(s, c)` level: that flag 1 of the 1.14d level getter means "with
   bonuses" is D2MOO's 1.10f reading, unconfirmed for 1.14d.
9. Stat mode (§4.4): d2rs compares the whole name case-insensitively
   (`basex` → 0); a prefix compare would give 1. 1.14d formulas use only
   `.accr`, so the data cannot decide.
10. The refusals of d2rs policy 4 and 6 (byte ≥ 0x80, missile `rand`)
    have no `txt-format.md` §9 code; d2rs reports both as E11 with a
    detail text.
