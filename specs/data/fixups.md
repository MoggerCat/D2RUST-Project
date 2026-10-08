# Spec: Data — Post-load fix-ups (record bytes)

- **Status:** verified: `data-tool dump-compare traces/raw/20261006-021210-tables` (2026-10-06) shows all 70 dumped tables identical to
  1.14d memory after the load, nothing pending, and `fixups_on_live_set`
  passes. Every rule below reproduces the 1.14d tables in
  memory after the excel load byte for byte: scratch reimplementations run
  on the live `.bin` files match the post-load dump (`dump_tables.py`,
  2026-10-06) for all 70 dumped tables. `d2-data::fixup` (`records`,
  `text`) implements every rule and passes the synthetic vectors
  (2026-10-06).
- **Target version:** 1.14d
- **Crate/module:** `d2-data::fixup`
- **Related specs:** `data/loading.md` (§6 load order, §7.4 summary, §8
  fatal checks, §9 combined arrays), `data/runtime-maps.md` (maps the
  loaders build outside the records), `data/field-types.md` (§6 linkers,
  §7 string ids), `data/fields.tsv` (field names and offsets),
  `formats/animdata.md` (monstats speeds), `formats/tbl.md` (string
  tables).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–60 |
| Inputs | 61–70 |
| Outputs / state changes | 71–77 |
| Rules | 78–79 |
|   1. Conventions and shared lookups | 80–116 |
|   2. itemstatcost | 117–147 |
|   3. skills → pettype | 148–156 |
|   4. charstats | 157–163 |
|   5. Items, gems | 164–185 |
|   6. Unique and set items | 186–210 |
|   7. Other string-id rows | 211–224 |
|   8. monstats | 225–291 |
|   9. monequip | 292–309 |
|   10. monumod, missiles | 310–318 |
|   11. levels | 319–338 |
|   12. Tile paths: lvltypes, lvlprest, lvlsub | 339–362 |
|   13. objects | 363–375 |
| Constants & data dependencies | 376–393 |
| Randomness | 394–397 |
| Edge cases & original bugs | 398–420 |
| Test vectors | 421–470 |
| Provenance | 471–520 |
| Open questions | 521–584 |
<!-- /index -->

## Summary

After a table's `.bin` is loaded (`loading.md` §6), its loader rewrites
some of the record bytes before the next table loads: indices and string
ids the compiler leaves 0, links between tables (set attachment, gem
offsets, monequip), derived values (monstats speeds and class chains,
itemstatcost op tables, level monster counts), wide-character copies of
localized names, tile paths, and clamps. This spec owns every such write
into record bytes. Maps a loader builds outside the records (bit
matrices, sorted lists, per-act or per-class indexes) are owned by
`runtime-maps.md`; the fatal checks that run in the same loaders by
`loading.md` §8.

## Inputs

| Name | Type | Source |
|---|---|---|
| loaded records | the `.bin` records of each table, in load order | `loading.md` §4, §6 |
| combined item array | weapons + armor + misc (659 records) | `loading.md` §9 |
| item code map | code → item index | `loading.md` §7.4, `field-types.md` §6.1, §6.6 |
| string tables | patch, expansion, base | `loading.md` §10.1, `tbl.md` |
| `AnimData.d2` | speed by COF name | `formats/animdata.md` |

## Outputs / state changes

The same records with the bytes below rewritten. Each rule runs once, in
its loader, right after that table loads; a rule that reads another
table reads it as already fixed up (load order: `loading.md` §6). Record
counts never change, except the count clamps of §10.

## Rules

### 1. Conventions and shared lookups

Offsets are hex, in the table's own record unless another table is named.
Field names are `fields.tsv` columns. "Items" is the combined item array;
"items[j]" its record j. Record order is `.bin` order.

**String id** of a key: `field-types.md` §7 *without* its last line: an
empty key or a key in no table gives 0, which is stored as 0 unless a
rule names a miss value. Keys are NUL-terminated within their field.

**Wide text** of a key (the same lookup, `0x00524E20` → `0x00524D30`, text
output), as UCS-2 units:
- empty key (first byte 0): the empty string;
- hit (patch → expansion → base): the table's text. Every 1.14d text these
  fix-ups read is ASCII, one unit per byte (other bytes: Open question 1);
- miss: the key, then ` -not xlated call ken w`, converted to wide with a
  capacity of strlen(that string) units; the converter writes at most
  capacity − 1 characters and a 0, so the final `w` is dropped. Result:
  key + ` -not xlated call ken ` (trailing space). Cached per key. Holds
  when no Korean `KAMAP` table is loaded (Open question 2).

**Bounded wide copy** `copy(dst, text, L)`: copy units until the text's 0
or L units, then fill the rest of the L units with 0. A text of L or more
units fills all L with no terminator.

**Item code map find**: exact 4-byte key, binary search (`0x006BD130`); a
miss gives −1.

**Signedness.** "i8/i16/i32" fields are read signed, "u…" unsigned, as
stated per rule; it matters only for out-of-range data (Edge cases).

Order (`loading.md` §6 steps): itemstatcost 6, missiles 8, skills 10,
charstats 12, items 15–17, affixes 18–22, uniqueitems 23, sets/setitems
24–25, gems 26, quality tables 28–29, runes 30, monstats 51, monumod 52,
hireling 55, monequip 57, levels 58, lvltypes 60, lvlprest 61, lvlsub
64, objects 66.

### 2. itemstatcost

Record 0x144 bytes, loader `0x00637A00`. After the count check (`loading.md` §8):

1. Record 0 `stuff` (u32 +0x140, read s32): kept in 1–8, else 6. It goes
   to a global (`runtime-maps.md` §3); the record is not changed.
2. Every record: u16 × 64 at +0x5E..+0xDD := 0xFFFF.
3. For each stat i in order, with op = u8 +0x54, param = u8 +0x55,
   base = u16 +0x56, op stats = u16 +0x58, +0x5A, +0x5C, n = count:
   1. op = 0 or op > 13: op := 0 (byte +0x54) and nothing else for i.
   2. **Op base.** If base < n: record base +0x51 := 1; i is stored in
      the first of record base's 64 slots at +0x5E whose value ≥ n
      (dropped when none is free); then, if op is 4 or 5, record i
      +0x53 := 1. Base ≥ n (an empty link is 0xFFFF): nothing.
   3. **Op stats**, k = 0, 1, 2: s := op stat k. s ≥ n ends the loop
      (later op stats are not read). Otherwise:
      - record i +0x51 := 1;
      - in record s's table of 16 entries at +0xDE (6 bytes each: u16
        base as stored, u16 source stat i, u8 op, u8 param), take the
        first entry whose op byte (+0xE2 + 6m) is 0; if none, go to the
        next k (no flags);
      - write the entry; record s +0x52 := 1;
      - flags on record i, u32 +0x04: if i = 7 or s = 7 set bit 6, else
        if i = 9 or s = 9 bit 7, else if i = 11 or s = 11 bit 8; when one
        was set, also bit 5. (Stats 7, 9, 11: `maxhp`, `maxmana`,
        `maxstamina`.)

Byte meanings (D2MOO names, hints only): +0x51 "is an op base or has op
stats", +0x52 "is another stat's op target", +0x53 "op 4/5 with a valid
base". +0x13E–0x13F stay 0. Source and target may be the same stat.

### 3. skills → pettype

Loader `0x00613F80`, after skills and skilldesc load. For each skill in
record order: p := i8 `pettype` (+0xBE). If 0 ≤ p < pettype count and
pettype record p's u32 count at +0xBC is < 15: u16 at +0xC0 + 2·count :=
skill index; count += 1. A full list drops later skills; the pass goes on.
(The per-class and passive lists built in the same loader:
`runtime-maps.md` §5.)

### 4. charstats

Loader `0x00613260` (call at `0x00613D23`), per record: zero +0x00..+0x0F;
then `copy(+0x00, wide text of class (str +0x20), 16)`, which rewrites
all 32 bytes. A fallback (`<` + key + `>`) exists for a lookup that
returns no text; the 1.14d lookup always returns one, so it never runs.

### 5. Items, gems

The items loader (`0x006315D0`) builds the combined array and the item
code map (`loading.md` §7.4, §9) and the version-0 list
(`runtime-maps.md` §6); it writes no record bytes.

**gems** (record 0xC0, loader `0x00636B70`), two loops after the load:

1. **String id, by a buggy key.** `code` (+0x28) is a `link32(items.code)`,
   so it holds the gem's item index (i32), not its code. For each gem:
   the key is the 3 low bytes of that i32 with each 0x20 turned into 0,
   as a C string (up to its first 0). u16 +0x2C := string id of that key,
   no miss value. 1.14d gem items are 557–642, so every key is
   `[index low byte, 0x02]`, a key in no table: +0x2C = 0 for all 68
   gems. With other data it can hit (index 120 → key `x`).
2. **Gem offsets.** For k = 0 … gem count − 1, in order: items[k] i32
   `gemoffset` (+0xF0) := −1 (k used as an *item* index); then
   j := gem k i32 +0x28; if j ≥ 0, items[j] +0xF0 := k (no upper bound
   check). A later k can overwrite an earlier gem's write when that gem
   names item k; in 1.14d all j ≥ 557 > 67, so it never does. Items not
   reached keep their compiled `gemoffset` (0 in every live record).

### 6. Unique and set items

**uniqueitems** (record 0x14C, loader `0x006342B0`), record n in order:
u16 +0x00 := n; name (str +0x02) added to the uniqueitems name link
(add-always, `field-types.md` §6.2); u16 +0x22 := string id of the name,
miss 5,383.

**sets, setitems** (records 0x128 / 0x1B8, loader `0x00634DC0`). `sets`
gets no write of its own. For setitems record n in order:
1. u16 +0x00 := n; name (+0x02) added to the setitems name link
   (add-always); u16 +0x24 := string id of the name, miss 5,383.
2. **Attachment.** s := i16 `set` (+0x2C); S := sets record s;
   c := i32 S +0x0C (starts at the compiled 0; the loader does not reset
   it). Only if 0 ≤ s < sets count and c < 6:
   - setitems u16 +0x22 := S u16 `version` (+0x04);
   - setitems i16 +0x2E := c (slot 0–5);
   - S u32 +0x110 + 4c := this setitems record (a pointer in 1.14d;
     d2rs stores the index n);
   - S +0x0C := c + 1.

An item that is not attached (bad set, or 7th and later item of a set)
keeps +0x22 and +0x2E as compiled (0), with no message. A set's list is
in setitems record order. 1.14d: every item attaches; sets 16–31 have
version 100, so setitems 62–126 get +0x22 = 100 and the rest 0.

### 7. Other string-id rows

| Table (loader) | Key → id |
|---|---|
| magic affixes (`0x00633730`) | name (+0x00) → u16 +0x20 |
| rare affixes (`0x00633F40`) | name (+0x26) → u16 +0x0C |
| qualityitems (`0x00636770`) | non-empty str +0x2C / +0x4C → u16 +0x6C / +0x6E |
| lowqualityitems (`0x00637500`) | name (+0x00) → u16 +0x20 |
| runes (`0x006394A0`) | name (+0x00) → u16 +0x82 |
| hireling (`0x00655720`) | NameFirst (+0xD3) / NameLast (+0xF3) → u16 +0x114 / +0x116, then the `loading.md` §8 checks |

No miss value: a miss stores 0. The combined affix rows run over the
combined arrays (`loading.md` §9).

### 8. monstats

Record 0x1A8, n = count (734), loader `0x00651210`: count check, then
pass A (`0x00651040`), then pass B (`0x00650F00`). Fields: BaseId i16
+0x02, NextInClass i16 +0x04, Code +0x10, MonStatsEx i16 +0x18, Velocity
i16 +0x32, Run i16 +0x34; results u16 +0x36 (walk), u16 +0x38 (run), u8
+0x4A (chain length), u8 +0x4B (position). The compiled bytes of the
results are 0.

**Pass A, class chain** (reads BaseId as compiled), for each row r:

```
cur = BaseId[r]; steps = 0
loop:
    if cur == r: +0x4B[r] := steps (low 8 bits)
    next = NextInClass[cur]; steps += 1
    if cur == next or steps > 255: stop
    cur = next
    if cur < 0: stop
+0x4A[r] := steps (low 8 bits; 256 → 0)
```

A row missing from its own chain keeps +0x4B (0). No range check on
BaseId or NextInClass (Edge cases).

**Pass B, BaseId repair and speeds**, rows r = 0 … n − 1 in order (a row
sees earlier rows' results):

1. b := BaseId[r]; if b < 0 or b ≥ n: BaseId[r] := r, b := r.
2. w := speed of the AnimData record for (class b, mode 2) (below), u32;
   not found → the default record's 256.
3. If b ≠ r and Velocity[b] > 0: w := (Velocity[r] × w) / Velocity[b]:
   32-bit product (Velocity sign-extended, wrapping), unsigned 32-bit
   division, truncating.
4. +0x36[r] := min(w, 32,767), compared as u32 (a negative i32 gives
   32,767).
5. Run base: if **r** < 410 (row, not b): run := i16 +0x36[b] / 2, signed,
   toward zero, read now (for b = r the value of step 4; for b > r the
   compiled 0). If r ≥ 410: run := speed of the record for (class b,
   mode 15), not found → 256.
6. If b ≠ r and Run[b] > 0: run := (Run[r] × run) / Run[b], as step 3.
7. +0x38[r] := min(run, 32,767) as in step 4.

**COF name for the lookup** (monster, no unit, the composer `0x0064F5B0`
then `animdata.md` §4–§5): token + mode + weapon class, each the first 3
bytes of a 4-byte code with every 0x20 turned into 0 (so a part ends at
its first space or NUL):

| Part | Source |
|---|---|
| token | monstats[b] Code (+0x10), e.g. `SK  ` → `SK` |
| mode | monmode[mode] token (+0x20 of the 0x34-byte record): 2 → `WL`, 15 → `RN` |
| weapon class | monstats2[MonStatsEx[b]] BaseW (+0x10) when MonStatsEx[b] is in 0 … monstats2 count − 1, else `hth` |

The lookup uppercases it: skeleton1 → `SKWL1HS`. Composer rules that do
not apply to modes 2 and 15 (DT/DD, the mode-13 `gh` override):
`animdata.md` Open question 2. 1.14d: every token and mode code has 2
characters and every BaseW 3, so every name has 7.

1.14d data: no BaseId is out of range (step 1 never fires), no Velocity
or Run is negative, no result reaches 32,767. Walk lookups miss for 109
rows (51 names), run lookups (324 rows ≥ 410) for 273 rows (92 names).
Chain lengths 1–11; 28 rows are not in their own chain (e.g. 333
`diabloclone`, 704–709 the uber bosses, 710).

D2MOO (1.10f) clamps a result ≤ 0 to 0; 1.14d has no lower clamp.

### 9. monequip

Record 0x1C, loader `0x00659E60`, after the count check (`loading.md`
§8):
1. Every monstats record: i16 +0x2A := −1.
2. For each monequip row n in order, m := i16 `monster` (+0x00). Only if
   0 ≤ m < monstats count:
   - if monstats[m] i16 +0x2A < 0, set it to n (the first row wins);
   - for k = 0, 1, 2: loc := i8 +0x14 + k, code := u32 +0x08 + 4k.
     loc := 0 if loc < 1, or loc > 10, or (code ≠ 0x20202020 **and**
     code is not in the item code map).

A code of four spaces (an empty `item` cell) is exempt from the code test.
Rows with m out of range are left as they are. 1.14d: monstats 267 → 0,
234 → 1, 357 → 2, 417 → 21, 418 → 25, all others −1; 8 slot-0 locs with an
empty item keep their value (rows 23, 24, 31, 32, 37, 38, 42, 43: loc
3, 1, 3, 1, 3, 1, 3, 1).

### 10. monumod, missiles

- **monumod** (`0x00655030`): a count above 256 is set to 256 (rows from
  256 on are ignored; no error). 1.14d: 43 rows.
- **missiles** (`0x00661B20`): byte +0x183 above 8 is set to 8. 1.14d: no
  row is above 8 (no byte changes).

The superuniques count cut and hcIdx map: `loading.md` §8.

### 11. levels

Record 0x220, loader `0x0061C540`, after the count check (`loading.md`
§8), per record:

1. **Names.** Zero 0x50 bytes at +0x16E and at +0x1BE.
   `copy(+0x16E, wide text of LevelName (str +0xF5), 40)` and
   `copy(+0x1BE, wide text of LevelWarp (str +0x11D), 40)`; then u16
   +0x1BC := 0 and u16 +0x20C := 0. Each holds at most 39 characters.
2. **Monster counts.** u8 +0x33 / +0x34 / +0x35 := the number of entries
   before the first entry < 0 in the 25 i16 at +0x36 (`mon1`–`mon25`) /
   +0x68 (`nmon1`–) / +0x9A (`umon1`–). Entries after a negative one are
   not counted (no 1.14d list has such a gap). `cmon` is not counted.
   (1.14d recomputes the three counts once per record of the table in an
   inner loop; the result is the same.)

1.14d: 274 keys give 218 base, 48 expansion, 3 patch hits and 5 misses
(level 0's two empty keys; the warps of 133–135, e.g. "To The Pandemonium
Run 1 -not xlated ca", cut at 39). The longest hit has 32 characters.

### 12. Tile paths: lvltypes, lvlprest, lvlsub

Path fix (`0x0061E4A0`), applied to one 60-byte str field in place:
1. Replace every `/` with `\` up to the string's end.
2. If the string has more than 1 character, it becomes
   `DATA\GLOBAL\TILES\` + string (formatted with `%s\%s` from the prefix
   `DATA\GLOBAL\TILES` into a 60-byte stack buffer followed by the
   security cookie, then copied back with its
   NUL). Bytes after the new NUL keep their old values.

Strings of 0 or 1 characters (1.14d: the 5,321 `0` placeholders) are
unchanged.

| Table (loader) | Fields | Rows |
|---|---|---|
| lvltypes (`0x0061E520`) | `File 1`–`File 32` (+0x00 + 0x3C·k) | every row |
| lvlprest (`0x0061EBB0`) | `File1`–`File6` (+0x44 + 0x3C·k) | when d2exp exists or the row's `Expansion` (u32 +0x20) is 0, i.e. every row under the d2rs policy (`loading.md` d2-data policy 5) |
| lvlsub (`0x0061F500`) | `File` (+0x04) | every row |

Example: `Act1/Town/Floor.dt1` → `DATA\GLOBAL\TILES\Act1\Town\Floor.dt1`.
The longest 1.14d source string has 40 characters (58 after the fix).
The lvlprest/lvlsub DS1 steps in the same loaders and the lvlsub type
index: `loading.md` Open question 9, `runtime-maps.md` §9.

### 13. objects

Record 0x1C0, loader `0x0063F690`, per record:
1. Zero +0x40..+0xBF, then `copy(+0x40, wide text of Name (str +0x00),
   64)`.
2. `FrameCnt0`–`FrameCnt7` (u32 +0xD8..+0xF7) := value << 8 (32-bit,
   wrapping), i.e. frames in 1/256 units.

1.14d: 566 names hit (421 base, 144 expansion, 1 patch); 7 miss (553
`funeralpire`, 554 `burninglogs`, 555 `stma`, 559 `BBQB`, 560 `btor`,
567 `Zoo`, 568 `Keeper`) and get the miss text. Record 0 `Dummy` → "an
evil force". The longest text has 33 units.

## Constants & data dependencies

| Constant | Value | Rule |
|---|---|---|
| unique/set item name miss | 5,383 | §6 |
| op range | 1–13 | §2 |
| op-base slots / op-stat entries | 64 / 16 | §2 |
| pettype list | 15 | §3 |
| items per set | 6 | §6 |
| monstats run-base row limit | 410 | §8 |
| speed cap | 32,767 | §8 |
| chain step limit | 255 | §8 |
| monequip loc range | 1–10 | §9 |
| monumod / missiles clamps | 256 rows / 8 | §10 |
| wide name lengths | charstats 16, levels 40 (39 used), objects 64 units | §4, §11, §13 |
| tile prefix | `DATA\GLOBAL\TILES\` | §12 |
| miss suffix | ` -not xlated call ken w` (`0x00730520`; last character dropped) | §1 |

## Randomness

None.

## Edge cases & original bugs

Reproduced:
1. gems +0x2C looks up the item index bytes, not the gem code (§5).
2. gems loop 2 resets items 0 … gem count − 1, whatever they are (§5).
3. Set attachment silently skips a 7th item; its +0x2E = 0 looks like slot 0
  (§6).
4. monstats: the run base for rows < 410 reads +0x36 of a later BaseId row
  as 0 (§8).
5. monstats: no lower clamp; negative products become 32,767 (§8).
6. The miss text drops its last character and is cut to the field (§1, §11).
7. levels counts stop at the first negative entry (§11).

Out-of-range data, where 1.14d reads or writes outside an array. No 1.14d
row reaches any of them; d2rs reports a load error (`FixupError`) instead:
8. monstats pass A: BaseId < 0 or ≥ n, or NextInClass ≥ n (§8);
9. gems: an item index j ≥ item count (§5);
10. a tile path longer than 41 characters (§12): the result (18 + n + 1
  bytes) passes both the 60-byte field and the 60-byte buffer, whose
  next bytes are the stack cookie (`[ebp−4]`), so `__security_check_cookie`
  (`0x00681A48`) ends the process at the function's return unless the
  overwritten bytes equal the cookie.

## Test vectors

Real 1.14d (`#[ignore]`, `D2_GAME_DIR`): the rules applied to the live
set must equal the post-load dump.

| Input | Expected | Source |
|---|---|---|
| all fix-ups on the live set | `data-tool dump-compare traces/raw/20261006-004246-tables`: 70/70 tables identical (sets +0x110 pointers excluded) | dump 2026-10-06 |
| itemstatcost, all 359 records after §2 | SHA-256 `562ec3c01a170e095785916c0d71341618903a2a3da900553f6074c7f010a13a` | dump |
| itemstatcost record 7 `maxhp` +0xDE..+0xF5 | `ffff03000900 ffff4c000b00 0c00d8000203 ffff0e010600` (sources 3, 76, 216, 270) | dump |
| itemstatcost flags (+0x04 bits 5–8 added) | 1: 0xA0, 3: 0x160, 76: 0x60, 77: 0xA0, 162: 0x120, 163: 0x120, 216: 0x60, 217: 0xA0, 242: 0x120, 270: 0x60, 271: 0xA0, 295: 0x120 (17 bytes in 12 records) | dump |
| itemstatcost +0x51 / +0x52 / +0x53 | set on 85 / 42 stats / 214, 215, 218, 219; only stat 12 has a +0x5E list (214 … 250, 37 entries) | dump |
| pettype lists | 0: 78, 88, 199; 3 `golem`: 75, 85, 90, 94; 17 `assassintrap`: 257, 261, 262, 271, 272, 276; 1, 7, 9, 18 empty | dump |
| charstats record 2 +0x00 | UCS-2 "Necromancer" + 10 zero bytes | dump |
| items +0xF0 | items 0–67 = −1; 557 = 0, 586 = 24, 610 = 35, 642 = 67; all others 0 | dump |
| gems +0x2C | 0 in all 68 | dump |
| setitems 2 / 62 | +0x2E = 2, +0x22 = 0 / +0x2E = 0, +0x22 = 100; set 0 slots = setitems 0, 1, 2, +0x0C = 3; sets 9 and 18 hold 6 | dump |
| monstats +0x36..+0x39, +0x4A, +0x4B, 6 bytes per row, all 734 rows | SHA-256 `725beb789c46730699e640d8146f2747b16d3dd57dc3b59881fcf49476484822`; sums +0x36 200,281, +0x38 161,888 | dump |
| monstats rows (walk, run, len, pos) | 0: 128, 64, 7, 0; 1: 170, 85, 7, 1; 35 (miss): 256, 128, 4, 1; 333: 256, 128, 1, 0; 421: 120, 80, 2, 1; 443: 138, 329, 5, 2; 671 (b = 235, run from `ZZRNHTH`): 208, 374, 5, 3; 684: 256, 256, 11, 10 | dump |
| monequip 23 slot 0 (empty item, loc 3) | loc stays 3 | dump |
| levels 39 +0x16E / +0x1BE | "The Secret Cow Level" / "To The Secret Cow Level"; +0x33..35 = 3/3/3 | dump |
| levels 133 +0x1BE | "To The Pandemonium Run 1 -not xlated ca" (39 units, unit 39 = 0) | dump |
| levels 131 +0x33/34/35 | 2/10/2 | dump |
| lvlsub 0 `File` | `DATA\GLOBAL\TILES\Act1\Outdoors\BorderCliffs.ds1` | dump |
| objects 553 +0x40 | "funeralpire -not xlated call ken " | dump |
| objects 1 `FrameCnt0`–`2` | 256, 1,792, 256 (file 1, 7, 1) | dump |

Synthetic (CI-safe):

| Input | Expected |
|---|---|
| itemstatcost n = 3; stat 1: op 2, base 0, param 3, op stats (2, 0xFFFF, 0) | rec 0: +0x51 = 1, +0x5E[0] = 1; rec 1: +0x51 = 1; rec 2: entry 0 = (0, 1, 2, 3), +0x52 = 1; op stat 3 not read |
| same with op 14 | op := 0; nothing else |
| stat 5 with op stat 7 | stat 5 flags += 0x60 |
| 8 setitems naming set 0 (count 0) | slots = items 0–5, count 6; items 6, 7: +0x2E = 0, +0x22 = 0 |
| setitems with `set` = −1 or ≥ sets count | not attached |
| gem with +0x28 = −1 | no offset write; key `FF FF FF` → 0 |
| monstats 3 rows, BaseId 0, Next 0→1, 1→2, 2→−1 | +0x4A = 3, 3, 3; +0x4B = 0, 1, 2 (same with 2→2) |
| 2 rows, BaseId 0, Next 0→1, 1→0 | +0x4A = 0 (256 wraps); +0x4B = 254, 255 |
| r ≠ b, Velocity 3 / 2, walk 101 | 151 |
| r ≠ b, Velocity −1 / 2, walk 100 | 0xFFFFFF9C / 2 = 0x7FFFFFCE → 32,767 |
| r ≠ b, Velocity[b] = 0 | walk unscaled |
| r < 410, b > r, compiled +0x36[b] = −3, Run[b] ≤ 0 | run −1 → 32,767 |
| monequip m = −1, loc 12 | row unchanged |
| monequip loc 3, item `xyz ` not in the map / item `    ` | loc 0 / loc 3 |
| levels mon = 5, −1, 7, then −1 | +0x33 = 1; 25 entries ≥ 0 → 25 |
| path `a/b` / `0` / empty | `DATA\GLOBAL\TILES\a\b` / `0` / empty |
| key `abc` missing from every table, 40-unit copy | "abc -not xlated call ken " |
| objects FrameCnt 0x01000001 | 0x00000100 |

## Provenance

1.14d `Game.exe` (1.14.3.71) Ghidra exports (`re/exports/funcs/`),
checked against raw disassembly (capstone) where the decompiler lost
register arguments. D2MOO (1.10f) supplied field names only.

- **Confirmed by post-load dump (`dump_tables.py`, 2026-10-06):** every
  rule of §2–§13. Dumps `traces/raw/20261006-003056-tables` and
  `20261006-004246-tables` (gitignored; the second adds maps). Scratch
  scripts applied each rule to the live `.bin` files (and, for §4, §11,
  §13, the ENG string tables through `d2-formats::tbl`) and compared all
  bytes of the 70 kept tables: identical. `data-tool dump-compare` on the
  same dump: 57 tables identical with today's `d2-data::fixup`; the 13
  others differ exactly in the bytes §2, §4–§6, §8, §9, §11–§13 write
  (itemstatcost's "17 records" are 12 records: bits 5–7 in byte +0x04,
  bit 8 in byte +0x05).
- itemstatcost `0x00637A00`: stuff `0x00638208`, 0xFFFF fill
  `0x00638250`, op test `0x006382B0`, op base `0x006382C2`–`0x00638363`,
  op stats `0x00638369`–`0x006384BA`, flag bits `0x00638431`–`0x0063849C`
  (mask bytes `0x006CE27C`/`80`/`84`, bit 8 via `0x006CE268` into +0x05).
- skills `0x00613F80`, pettype append `0x00617BAC`–`0x00617C15` (pettype
  records/count `0x96C818`/`0x96C820`).
- charstats `0x00613260` → `0x00612600`; lookup `0x00524E20` /
  `0x00524D30` (empty → `0x008829E8`, miss text `0x00524AC0` with suffix
  `0x00730520`, wide conversion `0x00526320`/`0x00526100`, length
  `0x00527D50`, KAMAP flag `0x00882B2C`); bounded copy `0x00526790`.
- gems `0x00636B70` (load call `0x006371AB`); combined items `0x0096CA5C`.
- uniqueitems `0x006342B0`; sets/setitems `0x00634DC0` (calls
  `0x006357E1`, `0x00636651`); name-link add `0x006BD500`; code find
  `0x006BD130`.
- monstats `0x00651210` → `0x00651040` (signed compares, `steps > 0xFF`),
  `0x00650F00` (`imul` + unsigned `div`, signed `> 0` guards, unsigned
  `< 0x7FFF` clamp, `r < 0x19A`, `cdq`/`sar` halving); lookup
  `0x0066A9B0` with no unit, unit type 1; composer `0x0064F5B0`, weapon
  class `0x0064F060` (monstats2 via `0x00451FE0`), mode tokens
  `0x0065B500`.
- monequip `0x00659E60` (load call `0x0065A03B`; monstats count at
  data-table +0xA80, item code map at +0x94).
- monumod `0x00655030`; missiles `0x00661B20`; hireling `0x00655720`.
- levels `0x0061C540` (`0x0061DA70`–`0x0061DB4D`).
- Tile paths: `0x0061E4A0` (`%s\%s` at `0x006D4124`, prefix `0x006E867C`,
  locals 0x40 bytes = 60-byte buffer at `[ebp−0x40]` + cookie at
  `[ebp−4]`, `> 1` length test); callers lvltypes
  `0x0061EA50` (32 fields), lvlprest `0x0061EFA3` (gate `0x00408F20` or
  +0x20 = 0), lvlsub `0x0061F938`.
- objects `0x0063F690` (`0x00640E00`–`0x00640E7E`: memset 0x80, copy
  0x40 units, eight `shl 8`).
- 1.14d differences from D2MOO: monstats lower clamp (§8); the gems key
  is the same quirk in D2MOO's field list (not used as evidence).

## Open questions

1. How the string tables' text becomes UCS-2 for bytes ≥ 0x80 (the hit
   path returns pre-converted text; `tbl.md` treats decoding as
   presentation). No 1.14d text read here has such a byte.
2. With a `KAMAP` table loaded (`0x00882B2C` ≠ 0) the miss text length is
   counted in characters (`0x005276E0`); whether the last character is
   still dropped. Irrelevant for ENG.
   *Answered* (static, 1.14d asm of `0x00524AC0`, `0x00527D50`,
   `0x005276E0`, `0x00526320`, `0x00526100`): yes, for every key whose
   bytes the table treats as single-byte. The miss path measures L =
   strlen(key) + strlen(suffix) bytes, then the capacity n =
   `0x00527D50(text, L)`: L without KAMAP; with KAMAP the number of
   characters in the first L bytes, a byte ≥ 0x20 outside every
   single-byte range of the table (`[0x00882B28]`, `[0x00882B22]`
   ranges) counting as a 2-byte character. The converter `0x00526320`
   (UTF-8 decoding through `0x00526100`, table `0x007309B8`, with or
   without KAMAP) writes at most n − 1 characters and a 0. With every
   byte single-byte, n = L and the final `w` is dropped as without
   KAMAP; each byte counted as a lead byte lowers n by one and cuts one
   more character from the end.
3. Whether anything reads gems +0x2C (always 0); if a reader expects the
   gem name id, the bug's visible effect is unknown.
   *Answered* (static, 2026-10-08): nothing reads it, so the bug has no
   visible effect. The gems array (`[0x0096CA94]`, count `[0x0096CA90]`)
   is read only through `0x006372C0(k)` (k ≥ count or −1 → null; also
   `0x006372A0`, which only frees it, from `0x00619140`). Its 8 callers
   read: `0x00486670` +0x20 (`letter`, widened with limit 6); `0x0062C100` +0x2F
   (`transform`); `0x004C0D20` (2 calls), `0x0055C2C0` (2) and
   `0x004E6850` (2, via `0x004E67D0`) pass the record to `0x0065FEC0`
   (type 2 or 5, 3 mods), which reads only the mod blocks +0x30 / +0x60
   / +0x90 (16-byte entries, `0x0065C730` → `0x0065C6D0`) and hands the
   record to `0x0065FE10` → `0x0065FD70`, which does not read it.
4. Whether runtime code checks the set list or +0x2E for a set item that
   was not attached (no 1.14d item is affected).
5. The texts were checked with the ENG string tables only; other
   languages change the §4/§11/§13 bytes and the §7 ids, not the rules.
6. Inputs this spec does not cover, which d2rs reports as a
   `FixupError` (strict input, no 1.14d row reaches them; 1.14d behavior
   not traced): a sets +0x0C count < 0 at attachment (§6); a gem count
   larger than the item count (§5 loop 2 resets items[k]); a tile path
   field with no NUL in its 60 bytes (§12); a missing monmode row 2 or
   15 (§8); a byte ≥ 0x80 in a miss key or a hit text (Open question 1).
   *Answered* (static, 2026-10-08) except the last (Open question 1);
   d2rs keeps the `FixupError` for all of them (each 1.14d outcome
   corrupts memory or crashes):
   - sets +0x0C = c < 0: the `c < 6` test is signed (`0x006366E3`), so
     the item attaches: +0x22 := version, +0x2E := low 16 bits of c,
     the record pointer is written at S + 0x110 + 4·(c's low 16 bits,
     sign-extended) (`0x006366ED`–`0x006366F8`), i.e. before the slot
     list (S's own bytes for c ≥ −68, earlier memory below), and
     c := c + 1.
   - gem count > item count: loop 2 writes −1 at items + 0x1A8·k + 0xF0
     for every k < gem count with no bound (`0x00637255`), past the
     combined array.
   - tile path field with no NUL: the length scan (`0x0061E4B8`) runs
     into the following bytes of the record; `/` → `\` rewrites them up
     to the first 0; the result is longer than 41 characters, so Edge
     case 10 applies (process ends at the cookie check).
   - monmode row 2 or 15 missing: the mode accessor `0x0065B500` returns
     null for a mode ≥ the monmode count (`0x0065B506`), and the
     composer reads its +0x20 at once (`0x0064F71F`): an access
     violation at load.
