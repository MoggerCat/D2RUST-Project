# Spec: Data — Table-specific compile callbacks

- **Status:** draft: applied to the live `.txt` cells, these rules
  reproduce the callback bytes of all 17,355 calls in the live `cubemain`,
  `monpreset`, `monstats` and `monstats2` bins (Provenance). Implemented
  in `d2-data::compile::callbacks` with the test vectors below as unit
  tests. The Rust code is confirmed by `data-tool tables` (2026-10-05):
  `cubemain`, `monpreset` and `monstats2` byte-identical, `monstats`
  differing only by the `field-types.md` §10 `NameStr` row; CbMiss/CbStop
  count 0.
- **Target version:** 1.14d
- **Crate/module:** `d2-data::compile` (suggested: `compile::callbacks`)
- **Related specs:** `specs/data/field-types.md` (callback call convention
  §8, codes and names §5, linkers §6, compile order §2 and §6.5, bin
  comparison §10), `specs/data/fields.tsv` (the `cb(...)` entries),
  `specs/data/txt-format.md` (cells, §9 errors and diagnostics),
  `specs/data/loading.md` (load order §6, linker sources §7).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–55 |
| Inputs | 56–65 |
| Outputs / state changes | 66–71 |
| Rules | 72–73 |
|   1. Shared rules | 74–112 |
|   2. `cb(cubemain.input)` — recipe inputs | 113–170 |
|   3. `cb(cubemain.output)` — recipe outputs | 171–251 |
|   4. `cb(monstats.skillmode)` — skill modes | 252–275 |
|   5. `cb(monstats2.composit)` — component choices | 276–296 |
|   6. `cb(monpreset.place)` — preset placement | 297–310 |
|   7. Lookups | 311–339 |
|   8. Errors and diagnostics | 340–358 |
|   9. `cubemain` `param` | 359–368 |
| Constants & data dependencies | 369–386 |
| Randomness | 387–390 |
| Edge cases & original bugs | 391–418 |
| Test vectors | 419–497 |
| Provenance | 498–565 |
| Open questions | 566–574 |
<!-- /index -->

## Summary

Five `cb` (type 23) callbacks in the 1.14d field lists parse a cell with
their own grammar and write record bytes outside the field footprints:
`cb(cubemain.input)` and `cb(cubemain.output)` (Horadric Cube recipe
inputs and outputs), `cb(monstats.skillmode)` (the animation mode or
sequence of each monster skill), `cb(monstats2.composit)` (the component
choices of each body part) and `cb(monpreset.place)` (what a preset
places). This spec gives, per callback, the bytes it writes, how it splits
and reads the cell, which linkers it looks up, and what happens on a miss
or a malformed cell. They run inside the record compile of
`field-types.md` §2 and have no other effect.

## Inputs

| Name | Type | Source |
|---|---|---|
| text | none, or the cell's first min(L, 256) bytes | `field-types.md` §8 |
| record | the record being compiled, including bytes earlier fields wrote | `field-types.md` §2 |
| offset | the field-list offset: a slot number here | `fields.tsv` |
| linkers | key → index tables (§7) | tables compiled earlier (`loading.md` §6) |
| `uniqueitems`, `setitems` records | base item code and level of a named unique or set item | their compiled records (§7) |

## Outputs / state changes

- Bytes of the current record only (§2–§6). No linker, code buffer or
  other record changes.
- Diagnostics; error E15 rejects the file (§8).

## Rules

### 1. Shared rules

1. **Calls.** Each callback is called once per record and field, in pass 2
   at its column's position, or after the bound columns with no text when
   its column is missing (`field-types.md` §2, §8). `offset` is the slot
   number given in `fields.tsv` (0–6, 0–2, 0–7, 0–15 or 0).
2. **Order matters.** `skillmode` reads the skill field written by the
   `Skill<n>` column, and `composit` for `S8v` reads the counts the other
   15 calls wrote. A field written later overwrites callback bytes (the
   cube output `ilvl`, §3). In 1.14d every column a callback reads is to
   its left.
3. **Text copy.** A callback works on its own copy of the text. The
   original edits the copy in place (quotes and separators become NUL);
   the helpers below describe the result.
4. **unquote(t).** If t starts with `"`, drop that byte. Then t ends at the
   next `"`. `"lit,med"` → `lit,med`; `rin",mag"` → `rin`.
5. **split(t, S)** → (token, rest). token = the bytes of t before the first
   byte that is in S. rest = the bytes after that byte, or *none* if t has
   no byte in S. An empty rest is not none: `a,` → (`a`, empty);
   `a` → (`a`, none).
6. **code(t)**: `field-types.md` §5.2 (first 4 bytes, space-padded, case
   kept). **Name lookups**: `field-types.md` §5.3 key (first 31 bytes,
   `A`–`Z` lowered; E11 applies) and *find* of §6.2 (no add). **Code
   lookups**: *find* of §6.1 (exact, case-sensitive).
7. **num(t)**: C `strtol(t, base 10)`, which 1.14d's `atol` calls. Skip
   leading bytes 0x09–0x0D and 0x20, read an optional `+` or `-`, then
   decimal digits up to the first other byte. No digits → 0. A value
   outside the i32 range clamps to 2,147,483,647 (or −2,147,483,648 with
   `-`). The result is cut to the field width (low 8 or 16 bits). This is
   not the D2 integer rule (`field-types.md` §4): `" 7x"` → 7, `"+5"` → 5,
   `"-1"` → 0xFF as a u8.
8. **Words.** Modifier words, `usetype` and `useitem` must equal the whole
   token, case-sensitive (`Qty` is not `qty`). `any` and the three portal
   names compare ASCII case-insensitively.
9. **Results ignored.** The original's routines return a success flag that
   the compiler ignores. A parse that stops keeps what it already wrote.
10. **item(c)** = the index of code c in `items.code`, with *found* or *not
    found*; not found gives index 0.

### 2. `cb(cubemain.input)` — recipe inputs

Fields `input 1`–`input 7`, slot k = 0–6. Slot k is 8 bytes at 20 + 8k:

| Slot byte | Size | Content |
|---|---|---|
| +0 | u16 | flags (table below), OR-ed |
| +2 | u16 | item type index, item index, or 0xFFFF |
| +4 | u16 | unique or set number + 1, else 0 |
| +6 | u8 | quality (last quality word wins) |
| +7 | u8 | quantity |

1. No text, or empty text → nothing.
2. t = unquote(text). Empty → nothing.
3. (first, rest) = split(t, `,`).
4. The first matching line decides; length = byte length of first:
   1. length ≤ 4 and first = `any` (case-insensitive) → flags |= 0x0001;
      item = 0xFFFF.
   2. length ≤ 4 and code(first) is in `itemtypes.code` at i → flags |=
      0x0002; item = i.
   3. length ≤ 4 and item(code(first)) is found at i → flags |= 0x0001;
      item = i.
   4. name(first) is in `@uniques` at u → flags |= 0x0041; quality = 7;
      +4 = u + 1; item = item(unique u's `code`).
   5. name(first) is in `@sets` at s → flags |= 0x0041; quality = 5;
      +4 = s + 1; item = item(set item s's `item`).
   6. Otherwise: stop; nothing is written (CbMiss).
5. Modifiers. While rest is not none: (tok, rest) = split(rest, `=,`), then
   by tok:
   - `qty`: if rest is none → E15. Else (v, rest) = split(rest, `,`);
     quantity = num(v).
   - quality word (table below): quality = its number.
   - flag word (table below): flags |= its flag.
   - anything else, including an empty token: stop; the rest of the text
     is ignored (CbStop if a non-empty part is ignored).

A value after `=` is read only for `qty`. After any other word, the value
becomes the next token and stops the parse: `"hax,sock=2,eth"` sets
`sock` and ignores `2,eth`.

| Input flag | Set by | D2MOO name (`CUBEFLAG_IN_`) |
|---|---|---|
| 0x0001 | `any`, item code, unique or set name | `USEANY` |
| 0x0002 | item type code | `ITEMCODE` |
| 0x0004 | `nos` | `NOSOCKET` |
| 0x0008 | `sock` | `SOCKETED` |
| 0x0010 | `eth` | `ETHEREAL` |
| 0x0020 | `noe` | `NOETHEREAL` |
| 0x0040 | unique or set name | `SPECIAL` |
| 0x0080 | `upg` | `UPGRADED` |
| 0x0100 | `bas` | `NORMAL` |
| 0x0200 | `exc` | `EXCEPTIONAL` |
| 0x0400 | `eli` | `ELITE` |
| 0x0800 | `nru` | `NORUNES` |

Quality words (inputs and outputs): `low` 1, `nor` 2, `hiq` 3, `mag` 4,
`set` 5, `rar` 6, `uni` 7, `crf` 8, `tmp` 9.

### 3. `cb(cubemain.output)` — recipe outputs

Fields `output`, `output b`, `output c`, slot k = 0–2. Slot k is 84 bytes
at 76 + 84k:

| Slot byte | Size | Content |
|---|---|---|
| +0 | u16 | flags (table below), OR-ed |
| +2 | u16 | item index or item type index |
| +4 | u16 | unique or set number + 1, else 0 |
| +6 | u8 | quality |
| +7 | u8 | quantity, or socket count |
| +8 | u8 | kind (table below) |
| +9, +10, +11 | u8 | `lvl`, `plvl`, `ilvl` fields; +11 also written here for a unique or set |
| +12 | 3 × u16 | prefix numbers (`pre`), in order |
| +18 | 3 × u16 | suffix numbers (`suf`), in order |
| +24 | 5 × 12 | `mod 1`–`mod 5` fields (not callback bytes) |

1. No text, or empty text → nothing.
2. t = unquote(text) (it may be empty now). (first, rest) = split(t, `,`).
3. The first matching line decides:
   1. first = `Cow Portal` (case-insensitive) → kind = 1.
   2. first = `Pandemonium Portal` (case-insensitive) → kind = 2.
   3. first = `Pandemonium Finale Portal` (case-insensitive) → kind = 3.
   4. first = `usetype` → kind = 0xFF.
   5. first = `useitem` → kind = 0xFE.
   6. length ≤ 4 and item(code(first)) is found at i → kind = 0xFC;
      item = i.
   7. length ≤ 4 and code(first) is in `itemtypes.code` at i → kind =
      0xFD; item = i.
   8. name(first) is in `@uniques` at u → flags |= 0x0008; kind = 0xFC;
      quality = 7; +4 = u + 1; item = item(unique u's `code`); +11 = low
      byte of unique u's `lvl`.
   9. name(first) is in `@sets` at s → the same with quality = 5,
      +4 = s + 1, the set item's `item` and `lvl`. Then stop: the
      modifiers are not read (CbStop if rest is not empty).
   10. Otherwise: stop; nothing is written (CbMiss).
4. Modifiers. p = slot +12, q = slot +18. While rest is not none:
   (tok, rest) = split(rest, `=,`), then by tok:
   - `qty`, `pre`, `suf`, `sock` take a value: if rest is none → E15. Else
     (v, rest) = split(rest, `,`) and n = num(v):
     `qty` → quantity = n; `pre` → u16 n at p, p += 2; `suf` → u16 n at q,
     q += 2; `sock` → flags |= 0x0002 and quantity = n.
   - quality word: quality = its number.
   - flag word (table below): flags |= its flag.
   - `reg`: flags |= 0x0040 and kind = 0xFF.
   - anything else: stop (CbStop if a non-empty part is ignored).
5. `pre` and `suf` have no limit: a 4th `pre` writes the first suffix
   (+18), a 4th `suf` the first `mod 1` bytes (+24, overwritten later by
   the `mod 1` column). A write that would end past the record is E15
   (only `output c` can get there: its 37th `pre` or 34th `suf`).

Input checks item types before item codes; output checks item codes
first. 1.14d codes in both linkers: `axe` and `key`. `axe` as an input is
item type 28; as an output it would be item 1.

| Kind | Meaning | D2MOO name (`CUBEOP_`) |
|---|---|---|
| 0 | nothing matched, or no output | — |
| 1 | `Cow Portal` | `COWPORTAL` |
| 2 | `Pandemonium Portal` | `UBERDUNGEON` |
| 3 | `Pandemonium Finale Portal` | `UBERTRISTRAM` |
| 0xFC | item (code, unique or set) | `ITEMCODE` |
| 0xFD | item type | `ITEMTYPE` |
| 0xFE | `useitem` | `USEITEM` |
| 0xFF | `usetype`, or `reg` | `USETYPE` |

| Output flag | Set by | D2MOO name (`CUBEFLAG_OUT_`) |
|---|---|---|
| 0x0001 | `mod` | `COPYMODS` |
| 0x0002 | `sock` | `SOCKET` |
| 0x0004 | `eth` | `ETHEREAL` |
| 0x0008 | unique or set name | `SPECIAL` |
| 0x0010 | `uns` | `UNSOCKET` |
| 0x0020 | `rem` | `REMOVE` |
| 0x0040 | `reg` | `NORMAL` |
| 0x0080 | `exc` | `EXCEPTIONAL` |
| 0x0100 | `eli` | `ELITE` |
| 0x0200 | `rep` | `REPAIR` |
| 0x0400 | `rch` | `RECHARGE` |

### 4. `cb(monstats.skillmode)` — skill modes

Fields `Sk1mode`–`Sk8mode`, slot i = 0–7. Reads the skill u16 at
368 + 2i (`Skill<i+1>`, `link16(skills.skill)`); writes the mode u8 at
384 + i and the sequence u16 at 392 + 2i.

1. mode = 0; sequence = 0xFFFF.
2. If the skill, read as i16, is < 0, or there is no text → done.
3. If text has ≤ 3 bytes: m = find of code(text) in `monmode_lookup.code`;
   mode = low byte of m. If that byte, read as i8, is ≥ 0 and ≠ 14 → done.
4. mode = 14; sequence = low 16 bits of find of name(text) in
   `monseq.sequence` (miss → 0xFFFF, CbMiss).

- `monmode_lookup.code` holds `DT NU WL GH A1 A2 BL SC S1 S2 S3 S4 DD KB xx
  RN` → 0–15 (1.14d). 14 is `xx`, the sequence mode: `xx` itself falls
  through to the sequence lookup.
- Mode codes are case-sensitive; sequence names are not: `a1` →
  mode 14 and the sequence lookup of `a1`.
- Empty text with a skill ≥ 0: `"    "` misses, so mode = 14 and sequence
  = find(`""`) = 0 (`monseq` record 0 has the empty key). 1.14d has no such
  cell.
- With the `Skill<n>` column right of the mode column, or missing, the
  callback reads 0 and treats it as a skill (reproduced).

### 5. `cb(monstats2.composit)` — component choices

Fields `HDv`, `TRv`, `LGv`, `Rav`, `Lav`, `RHv`, `LHv`, `SHv`, `S1v`–`S8v`,
slot i = 0–15. Writes the count u8 at 21 + i and 12 choice bytes at
38 + 12i; slot 15 also writes the total u8 at 37.

1. count = 0; the 12 choices = 0xFF.
2. No text → done (slot 15 writes no total either).
3. t = unquote(text); n = 0. Repeat:
   (tok, rest) = split(t, `,`). If tok is empty or longer than 4 bytes →
   stop. choice[n] = low byte of find of code(tok) in `compcode.code`
   (miss → 0xFF, CbMiss); n += 1. If rest is none or n = 12 → stop.
   t = rest.
4. count = n. A stop with a non-empty part left is CbStop.
5. Slot 15 only: total = sum over the 16 count bytes 21–36, as the record
   holds them now, of (count − 1) for each count ≥ 2; byte 37 =
   min(total, 254).

Example: `skeleton1` counts 7, 3, 3, 3, 3, 10, 0, 5, 12, 12, 0 × 6 →
total 6 + 2 + 2 + 2 + 2 + 9 + 4 + 11 + 11 = 49.

### 6. `cb(monpreset.place)` — preset placement

Field `Place`, slot 0. Writes the kind u8 at 1 and the index u16 at 2 (the
`Act` field is byte 0).

1. kind = 0; index = 0.
2. No text, or empty text → done.
3. The first name lookup that hits decides: `superuniques.Superunique` →
   kind 2; `monstats.Id` → kind 1; `monplace.code` → kind 0. index = the
   hit. All miss → 0 / 0 (CbMiss).

Superuniques win: 1.14d `Griswold` and `Radament` are also monstats Ids
and get kind 2.

### 7. Lookups

| Linker | Kind | Filled by | Used by |
|---|---|---|---|
| `itemtypes.code` | code | `itemtypes` `code` | §2, §3 |
| `items.code` | code | `weapons`, `armor`, `misc` `code` (`field-types.md` §6.4) | §2, §3 |
| `@uniques` | name, add-always | `uniqueitems` loader, after its compile | §2, §3 |
| `@sets` | name, add-always | `setitems` loader, after its compile | §2, §3 |
| `monmode_lookup.code` | code | `monmode_lookup` (`loading.md` §6) | §4 |
| `monseq.sequence` | name | `monseq` | §4 |
| `compcode.code` | code | `compcode` | §5 |
| `superuniques.Superunique` | name | `superuniques` | §6 |
| `monstats.Id` | name | `monstats` | §6 |
| `monplace.code` | name | `monplace` (compile-only lookup) | §6 |

- `@uniques`: after compiling `uniqueitems`, its loader registers each
  record's `index` text (str at offset 2) in record order with the
  add-always rule of `field-types.md` §6.2. Index = record number; a
  duplicate name finds its first record. 1.14d: 402 records, 394 distinct
  names; `The Stone of Jordan` → 122. `@sets` is the same over `setitems`
  (127 records, all distinct). In `.bin` mode the loaders build them the
  same way from the loaded records.
- Record fields read: unique u `code` (u32 at 40) and `lvl` (u16 at 52);
  set item s `item` (u32 at 40) and `lvl` (u16 at 48).
- `@uniques` and `@sets` may be absent: the original then skips that
  lookup. In the 1.14d order both exist before `cubemain` (steps 23 and 25
  before 72). Every other linker here belongs to an earlier step
  (`loading.md` §6); a missing one is E13 (`field-types.md` §6.5).

### 8. Errors and diagnostics

Error (rejects the file, like `txt-format.md` §9):

| Code | Condition | Reported with | Original 1.14d outcome |
|---|---|---|---|
| E15 | cube `qty` (input or output), `pre`, `suf` or `sock` (output) with no byte after it (rest none); or an output `pre`/`suf` write past the record end | line, column, field | reads through a null pointer; writes into the next record |

Diagnostics (accept, reproduce, report; same output as `txt-format.md` §9):

| Kind | Condition | 1.14d count |
|---|---|---|
| CbMiss | non-empty text whose lookup misses: cube first token (§2 step 4.6, §3 step 3.10), `compcode` token, skill-mode sequence, `Place` | 0 |
| CbStop | a non-empty part of the text is ignored: unknown modifier word, set output with modifiers, composit token empty or longer than 4 bytes, 13th composit token | 0 |

The one 1.14d cell that stops early, `monstats2` record 195 `act2male`
`S1v` `"lit,nil,nil,fez,nil,nil,rol,nil,nil,"`, ignores only an empty
token after its final comma (count 9), so it is not reported.

### 9. `cubemain` `param`

The `param` entry is type 2 (`u32`) with a routine in its link slot
(`fields.tsv`: `cb(cubemain.param)`). The compiler ignores the link slot
of type 2, so the routine never runs and `param` compiles as a plain u32
(`field-types.md` §4). It is not a callback. The dead routine would store
`atol` of a cell starting with `-` or a digit, else the
`itemstatcost` index of the name, else 0; D2MOO 1.10f lists it the same
way.

## Constants & data dependencies

| Constant | Value |
|---|---|
| cube input slots | 7 × 8 bytes at 20 |
| cube output slots | 3 × 84 bytes at 76 |
| cube first-token code limit | length ≤ 4 |
| skill-mode code limit | length ≤ 3 |
| sequence mode | 14 (`xx`) |
| composit choices per slot | 12, unused 0xFF |
| composit token limit | 4 bytes |
| composit total cap | 254 |
| text cap | 256 bytes (`field-types.md` §8) |

Columns read: `cubemain` `input 1`–`7`, `output`, `output b`, `output c`;
`monstats` `Sk1mode`–`Sk8mode` (plus the `Skill1`–`Skill8` bytes);
`monstats2` the 16 `*v` columns; `monpreset` `Place`. Linkers: §7.

## Randomness

None.

## Edge cases & original bugs

All reproduced by default.

1. Set outputs skip their modifiers (§3 step 3.9); D2MOO 1.10f reads them. An
  input set name does read them: `"Civerb's Ward,mag"` ends with
  quality 4.
2. Unique and set outputs write `ilvl` (+11), but the `ilvl` / `b ilvl` /
  `c ilvl` column is to the right and overwrites it (an empty cell writes
  0). A layer that removes that column keeps the item's level.
3. Non-`qty` words with `=v` stop the input parse at `v`; in outputs only
  `qty`, `pre`, `suf`, `sock` take values.
4. An unknown word stops the parse silently in the original; later valid
  words are lost (`"usetype,foo,mag"` → no quality).
5. A first token that is empty becomes code `"    "`: `itemtypes` row 0
  owns it, so input `",qty=2"` → item type 0 (flags 0x0002, qty 2), and
  output `""` (text `""`) → kind 0xFD, item 0. Input text that is empty
  after unquote writes nothing.
6. Text after a second quote is dropped (`rin",mag"` → `rin`).
7. Input and output test item types and item codes in opposite orders
  (§3).
8. A unique or set whose base code is not an item gets item 0, the same
  value as item 0 (`hax`).
9. `pre`/`suf` beyond 3 overwrite neighbouring slot bytes (§3 step 5).
10. Skill-mode bytes depend on the skill column being compiled first (§4).
11. The composit total is written only by the `S8v` call and only from the
  counts present then; a missing `S8v` column leaves it 0.

## Test vectors

1.14d linkers (live tables). Bytes in slot order (§2–§6).

**`cb(cubemain.input)`** (8 slot bytes)

| Text | Bytes | Source |
|---|---|---|
| `any` | `01 00 ff ff 00 00 00 00` | rule |
| `"ANY,nos"` | `05 00 ff ff 00 00 00 00` | rule |
| `"gem2,qty=3"` | `02 00 5d 00 00 00 00 03` | record 15 input 2 |
| `axe` | `02 00 1c 00 00 00 00 00` | record 11 input 1 (type 28, not item 1) |
| `The Stone of Jordan` | `41 00 0a 02 7b 00 07 00` | record 62 input 3 |
| `"Civerb's Ward,mag"` | `41 00 4a 01 01 00 04 00` | rule (set 0, base `lrg` = 330) |
| `"hax,sock=2,eth"` | `09 00 00 00 00 00 00 00` | rule (`2` stops) |
| `",qty=2"` | `02 00 00 00 00 00 00 02` | rule |
| `"rin,qty=300"` | `01 00 0a 02 00 00 00 2c` | rule |
| `"rin,qty= 7x"` | `01 00 0a 02 00 00 00 07` | rule |
| `"rin,qty=-1"` | `01 00 0a 02 00 00 00 ff` | rule |
| `"rin,qty=99999999999"` | `01 00 0a 02 00 00 00 ff` | rule (clamped) |
| `rin",mag"` | `01 00 0a 02 00 00 00 00` | rule |
| `"qqq,mag"` | all 0 | rule (CbMiss) |
| `"rin,qty"` | E15 | rule |

**`cb(cubemain.output)`** (first 24 slot bytes, callback writes only:
+9 and +10 come from the `lvl` columns, and +11 from the `ilvl` column
after the callback)

| Text | Bytes | Source |
|---|---|---|
| `Cow Portal`, `cow portal` | `00`×8, `01`, `00`×15 | record 2 |
| `"usetype,mag,suf=162"` | `00 00 00 00 00 00 04 00 ff 00 00 00 00 00 00 00 00 00 a2 00 00 00 00 00` | record 17 |
| `"useitem,sock=1"` | `02 00 00 00 00 00 00 01 fe 00 …` | record 63 |
| `"useitem,reg"` | `40 00 00 00 00 00 00 00 ff 00 …` | rule |
| `"amu,mag,pre=331"` | `00 00 08 02 00 00 04 00 fc 00 00 00 4b 01 00 …` | record 6 |
| `"pole,mag,pre=191"` | `00 00 22 00 00 00 04 00 fd 00 00 00 bf 00 00 …` | record 19 |
| `"rin,pre=1,pre=2,pre=3,pre=4"` | `00 00 0a 02 00 00 00 00 fc 00 00 00 01 00 02 00 03 00 04 00 00 00 00 00` | rule |
| `The Stone of Jordan` | `08 00 0a 02 7b 00 07 00 fc 00 00 27 00 …` | rule (+11 = `lvl` 39) |
| `"Civerb's Ward,qty=2"` | `08 00 4a 01 01 00 05 00 fc 00 00 0d 00 …` | rule (qty skipped) |
| `"usetype,foo,mag"` | `00`×8, `ff`, `00`×15 | rule |
| `"Useitem"` | all 0 | rule (case; CbMiss) |

**`cb(monstats.skillmode)`** (mode, sequence; skill 5 unless noted)

| Text | Mode | Sequence | Source |
|---|---|---|---|
| `A1` | 4 | `ff ff` | record 45 (skill 321) |
| `KB` | 13 | `ff ff` | rule |
| `a1` | 14 | `ff ff` | rule |
| `xx` | 14 | `ff ff` | rule |
| `seq_skeletonraise` | 14 | `08 00` | record 0 (skill 158) |
| `SEQ_SKELETONRAISE` | 14 | `08 00` | rule |
| empty | 14 | `00 00` | rule |
| any text, skill −1 | 0 | `ff ff` | 5,097 1.14d cells (all empty) |
| none (missing column) | 0 | `ff ff` | rule |

**`cb(monstats2.composit)`** (count, 12 choices)

| Text | Count | Choices | Source |
|---|---|---|---|
| `"lit,med,hvy"` | 3 | `01 02 04` then `ff`×9 | record 0 `TRv` |
| `"lit,,med"` | 1 | `01` then `ff`×11 | rule (CbStop) |
| `"lit,toolong,med"` | 1 | `01` then `ff`×11 | rule (CbStop) |
| `LIT` | 1 | `ff`×12 | rule (CbMiss) |
| `"lit,med,"` | 2 | `01 02` then `ff`×10 | rule |
| empty, or none | 0 | `ff`×12 | rule |
| 13 × `lit` | 12 | `01`×12 | rule (CbStop) |
| `skeleton1` (all 16) | byte 37 = 49 | — | record 0 |

**`cb(monpreset.place)`** (record bytes 1–3)

| Text | Bytes | Source |
|---|---|---|
| `gheed` | `01 93 00` | record 0 (monstats 147) |
| `GHEED` | `01 93 00` | rule |
| `The Countess` | `02 06 00` | record 40 |
| `Griswold` | `02 05 00` | record 39 (also monstats 365) |
| `nonexistent`, empty | `00 00 00` | rule |

## Provenance

1.14d `Game.exe` (1.14.3.71), Ghidra exports in `re/exports/` plus a
headless disassembly of the routines that have no exported function.
Addresses are virtual addresses.

- **Field lists and call sites**: `cubemain` list in `0x00669130` (inputs
  → `0x00669080`, which calls the parser `0x00668600` on slot
  `record + 20 + 8k`; outputs → `0x006690B0`, parser `0x00668A90` on
  `record + 76 + 84k`; both wrappers skip absent or empty text and drop
  the parser's result; code helper `0x00668510`; value helper
  `0x006685C0` for `sock`); `monstats` list in `0x00651210` (callback
  `0x00651150`, code helper `0x00650EB0`); `monstats2` list in
  `0x006571A0` (callback `0x00657010`); `monpreset` list in `0x00659A10`
  (callback `0x00659990`, a bare label in the export, read from its
  disassembly). Each callback takes the six arguments of
  `field-types.md` §8 (text and record in registers, four on the
  stack).
- **Set-output stop**: in `0x00668A90` the set-item branch falls into the
  same exit as the no-match case (return 0) instead of the modifier loop;
  confirmed in the disassembly (`0x00668CEF`).
- **Lookups**: code find `0x006BD130`, name find `0x006BD3C0`, item code
  → index `0x00633640` (linker `0x0096BCC4`, index 0 when not found).
  Linker globals and their owners: item types `0x0096C824`
  (`0x00638D80`), uniques `0x0096C850` / records `0x0096C854` (filled by
  `0x006342B0` with the add-always registration `0x006BD500`), sets
  `0x0096C844` / `0x0096C848` (`0x00634DC0`), monmode lookup
  `0x0096BC5C`, compcode `0x0096BCB0` and monplace `0x0096C6D0`
  (`0x00612750`), monseq `0x0096C7AC` (`0x00659C60`), superuniques
  `0x0096C708` (`0x006552E0`), monstats `0x0096C6AC` (`0x00651210`).
  Uniques record stride 0x14C (code +40, lvl +52), sets 0x1B8 (item +40,
  lvl +48).
- **num**: `atol` `0x00681EBB` calls `strtol` (`0x0068676E`, clamping
  in `0x00686543`).
- **D2MOO 1.10f** (`HoradricCube.cpp`, `MonsterTbls.cpp`) has the same
  grammar and layout. 1.14d differs in: the two Pandemonium portal names
  (D2MOO behind `D2_VERSION_HAS_UBERS`), the set-output modifier skip, and
  no trace messages. Every rule above was read from the 1.14d code.
- **Data check** (2026-10-05, Python over the live files; scratch script,
  not in the repo): compile each record in `field-types.md` §2 order,
  taking field-footprint bytes from the live bin (they already match,
  §10) and running the rules above for the callbacks; compare whole
  records.

| Callback | Calls matching | Records identical |
|---|---|---|
| `cb(cubemain.input)` | 1,057 / 1,057 | `cubemain` 151 / 151 |
| `cb(cubemain.output)` | 453 / 453 | (same) |
| `cb(monpreset.place)` | 229 / 229 | `monpreset` 229 / 229 |
| `cb(monstats.skillmode)` | 5,872 / 5,872 | `monstats` 734 / 734 |
| `cb(monstats2.composit)` | 9,744 / 9,744 | `monstats2` 609 / 609 |

  Branches the data exercises: inputs `any` 5, item type 63, item code
  319, unique 2; outputs item code 78, item type 1, `usetype`/`useitem`
  69, portals 3; skill modes 456 mode codes, 319 sequences, 5,097 skipped
  (skill −1); place superunique 56, monstats 104, monplace 69; composit
  148 records with a total > 0. Mutations fail the check: no composit
  total → 148 calls differ; no skill guard or no 0 / 0xFFFF reset →
  5,097 differ. Not exercised by 1.14d data (code only): set inputs,
  unique and set outputs, the set-output stop, every miss and stop, E15,
  `pre`/`suf` overflow, composit tokens longer than 4 bytes.
- **`monstats` record 707 `NameStr`** (`field-types.md` §10) is not a
  callback byte (offset 6, a `strkey`). Its routine (`0x006117B0` →
  `0x00524D30`: patch, then expansion, then base table; 0 → 5382) has no
  special case, so a `-txt` compile gives 11154. Nothing in the code
  explains the shipped 5382; the §10 inference (a bin built against
  tables without `Lilith`) remains an inference.

## Open questions

1. Answered: `txt-format.md` §9 lists E15, CbMiss and CbStop and places
   E15 in conversion order with E11 / E12.
2. The code-only branches (Provenance) have no 1.14d data or trace behind
   them. A `-txt` run of 1.14d on a `cubemain.txt` with a set output plus
   modifiers, a unique output, and `"rin,qty"` would confirm the set
   stop, the overwritten `ilvl` and the crash.
