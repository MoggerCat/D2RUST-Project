# Spec: Data — Runtime maps built by the table loaders

- **Status:** implemented. Every map below was reproduced byte for byte
  from the live `.bin` files by scratch reimplementations and compared
  with the 1.14d post-load dump (`dump_tables.py`, 2026-10-06; 27 dumped
  maps, the pointer-holding ones compared as indices). `d2-data::fixup`
  (`maps`, `qsort`) builds every map and passes the synthetic vectors
  (2026-10-06); `data-tool dump-compare` on game files is queued
  (`docs/HANDOFF.md` §5).
- **Target version:** 1.14d
- **Crate/module:** `d2-data::fixup` (the maps live next to the fixed
  tables)
- **Related specs:** `data/fixups.md` (record-byte fix-ups, shared
  lookups), `data/loading.md` (§6 load order, §7.4 summary, §8 checks,
  §9 combined arrays), `data/field-types.md` §6 (linkers), `data/fields.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–50 |
| Inputs | 51–55 |
| Outputs / state changes | 56–83 |
| Rules | 84–85 |
|   1. The CRT sort (`qsort`) | 86–117 |
|   2. Type-equivalence matrices (itemtypes, montype) | 118–153 |
|   3. itemstatcost globals and description list | 154–169 |
|   4. states | 170–187 |
|   5. skills: class lists and passive list | 188–202 |
|   6. Items: version-0 list | 203–210 |
|   7. gamble | 211–231 |
|   8. monseq, monpreset, hireling | 232–256 |
|   9. leveldefs, lvlsub | 257–270 |
|   10. automap | 271–299 |
| Constants & data dependencies | 300–312 |
| Randomness | 313–316 |
| Edge cases & original bugs | 317–335 |
| Test vectors | 336–386 |
| Provenance | 387–425 |
| Open questions | 426–441 |
<!-- /index -->

## Summary

Besides rewriting record bytes (`fixups.md`), several loaders build
lookup structures from the loaded records: bit matrices, sorted index
lists, per-class, per-act and per-id indexes, and converted forms of
tables whose records are then freed (gamble, automap). They are built
once, right after their table loads, and are read-only afterwards. This
spec owns their exact contents and the one sort routine they use.

## Inputs

The fixed-up records of the tables named per section (`fixups.md`), in
load order (`loading.md` §6).

## Outputs / state changes

| Map | Built by (step) | 1.14d global | Dump file (`map-*.bin`) | § |
|---|---|---|---|---|
| itemtypes equivalence matrix | itemtypes (2) | `0x96C834`, W `0x96C830` | `itemtypes_equiv` | 2 |
| montype equivalence matrix | montype (3) | `0x96C874`, W `0x96C878` | `montype_equiv` | 2 |
| `stuff`, mask | itemstatcost (6) | `0x96C89C`, `0x96C8A0` | `stat_stuff` | 3 |
| description list | itemstatcost (6) | `0x96C808`, count `0x96C80C` | `isc_desc_list` | 3 |
| 40 flag bitsets, 5 state lists | states (9) | `0x96BCF8`, `0x96BCFC`; `0x96BD9C`–`0x96BDC0` | `states_*` | 4 |
| class lists, counts, passive list | skills (10) | `0x96C7D4`–`0x96C7E4` | `skills_class_*`, `skills_desc_list` (the passive list) | 5 |
| version-0 item list | items (15–17) | `0x96CA78` | `items_f6_list` | 6 |
| sorted gamble items, level thresholds | gamble (32) | block `0x96CAB0` | `gamble_index`, `gamble_levels` | 7 |
| sequence index | monseq (50) | `0x96C7B4`, count `0x96C7B8` | `monseq_index` | 8 |
| per-act ranges | monpreset (54) | `0x96C6DC`, `0x96C6F0` | `monpreset_acts` | 8 |
| first row per hireling id | hireling (55) | `0x96BDD8`, `0x96C1D8` | `hireling_first` | 8 |
| portal level list | leveldefs (59) | `0x96C9F4`, count `0x96C9F8` | `leveldefs_portals` | 9 |
| first row per sub-level type | lvlsub (64) | `0x96CA18` | `lvlsub_type_first` | 9 |
| converted records, level-name ranges | automap (65) | `0x96CA28`/`0x96CA2C`, `0x96C8D0` | `automap_runtime`, `automap_level_index` | 10 |

Owned elsewhere: the item code map, the itemtypes code link and the
unique/set item name links (`loading.md` §7.4, `field-types.md` §6.6);
the combined item, affix, player and object arrays and their counts
(`loading.md` §9; dump `item_counts`, `affix_count`, `rare_count`); the
superunique hcIdx map (`loading.md` §8); the treasure-class form
(`loading.md` §10.6); each set's item list, kept in the sets records
(`fixups.md` §6). Globals are 1.14d addresses for reference; d2rs keeps
the same contents in its own types.

## Rules

### 1. The CRT sort (`qsort`)

The gamble and itemstatcost lists are sorted with `Game.exe`'s C runtime
`qsort` (`0x00685B50`, Visual Studio 2005 CRT) and a comparator that
returns equal for equal keys. It is not stable, so equal keys come out
in the order this exact algorithm leaves them; a stable sort is wrong
(gamble: 93 of 125 positions differ; itemstatcost: 63 of 207).

Positions lo, hi are inclusive; swaps exchange whole elements; elements
are compared only through cmp. Fewer than 2 elements: nothing. Start with
an empty stack, lo = 0, hi = n − 1.

1. size = hi − lo + 1. If size ≤ 8: **short sort**: while hi > lo, let m
   be the first maximum of lo … hi (scan p = lo+1 … hi, m := p only when
   cmp(p, m) > 0), swap m and hi, hi −= 1. Go to 6.
2. mid = lo + size / 2 (integer). If cmp(lo, mid) > 0 swap them; if
   cmp(lo, hi) > 0 swap; if cmp(mid, hi) > 0 swap. (mid stays a
   position.)
3. l = lo, h = hi. Repeat:
   - if mid > l: do l += 1 while l < mid and cmp(l, mid) ≤ 0;
   - if mid ≤ l: do l += 1 while l ≤ hi and cmp(l, mid) ≤ 0;
   - do h −= 1 while h > mid and cmp(h, mid) > 0;
   - if h < l: leave the loop; else swap l, h, and if mid = h then
     mid := l.

   ("do … while": step first, then test.)
4. h += 1. If mid < h: do h −= 1 while h > mid and cmp(h, mid) = 0. Then
   if mid ≥ h: do h −= 1 while h > lo and cmp(h, mid) = 0.
5. If h − lo ≥ hi − l: push (lo, h) if lo < h; if l < hi, lo := l and go
   to 1. Else: push (l, hi) if l < hi; if lo < h, hi := h and go to 1.
6. If the stack is empty, stop; else pop (lo, hi) and go to 1.

### 2. Type-equivalence matrices (itemtypes, montype)

A matrix of n rows (n = record count) of W = ceil(n / 32) u32 words,
zeroed, row-major: bit (i, j) is word i·W + j/32, mask 1 << (j mod 32).
Bit (i, j) := Equiv(i, j) for all i, j in 0 … n − 1 ("is row i of type
j?").

Equiv(i, j), with equivalence links read as i16 from record t:

1. Column test: **itemtypes** j ≤ 0 → 1 (column 0 is set in every row);
   **montype** j ≤ 0 → 0 (column 0 is never set).
2. i ≤ 0 or i ≥ n → 0 (row 0 is column 0 only for itemtypes, empty for
   montype).
3. Depth-first walk with a stack of up to 128 ints, starting with i.
   Until the stack is empty (then 0): pop t;
   - t = j → 1 (so the diagonal is set for 1 ≤ i < n);
   - t ≥ n → 0 for the whole query (no other branch is tried);
   - entries left on the stack > 124 → 0;
   - pushes: if e1(t) > 0, push e1; then if e2(t) ≠ 0, push e2; then
     (montype only) if e2 ≠ 0 and e3(t) ≠ 0, push e3. The last pushed is
     popped first.

| Table | e1, e2, e3 |
|---|---|
| itemtypes (record 0xE4) | `equiv1` +0x04, `equiv2` +0x06 |
| montype (record 0x0C) | `equiv1` +0x02, `equiv2` +0x04, `equiv3` +0x06 |

e2 (and e3) are ignored when e1 ≤ 0, and pushed even when negative; a
negative t reads before the table (Edge cases). 1.14d: itemtypes W = 4
(103 rows), 16 rows have `equiv2`; montype W = 2 (59 rows), one row (37
`undeadfetish`) has `equiv2`, none `equiv3`.

Readers: `0x00629B50` and the item test `0x00629A90` (row = the item's
type, column = the asked type, out-of-range i or j → 0; the item test
also tries the item's second type, items +0x120, when non-zero).

### 3. itemstatcost globals and description list

- `stuff` := record 0 s32 +0x140 if in 1–8, else 6; mask := (1 << stuff)
  − 1. 1.14d: 6, 0x3F.
- **Description list**, after the `fixups.md` §2 pass: collect
  (index i, priority = `descpriority` u16 +0x34) for every stat whose
  `descfunc` (u8 +0x36) ≠ 0, in record order; sort with §1, comparator:
  priority compared as **signed 16-bit**, −1 / 0 / +1. The list is the u16
  stat indices in sorted order; its count is the number collected. Read
  by the client item-description code (`0x004E60A0`).

1.14d: 207 stats. Starts 91 (priority 0), 252, 204 (1), 253 (2), 75, 254,
125, 89, 240, 87, 80 (8), 239. Tie groups in output order: 1: 252, 204;
8: 87, 80; 81: 117, 107, 108, 97; 88: 306, 305, 335, 308, 329, 330, 60,
336, 307, 331, 332, 333, 334; 160: 195, 197, 198, 199, 201, 152, 196.

### 4. states

States record 0x3C; the 40 flag bits are `fields.tsv` `bit` columns at
+0x10 (bit k = byte +0x10 + k/8, mask 1 << (k mod 8)). W = ceil(n / 32).

- **Flag bitsets.** One block of 40 × W u32, zeroed; bitset k (k = 0 …
  39) is words k·W … k·W + W − 1, pointer k at `0x96BCFC + 4k`. Bit s of
  bitset k := state s has flag bit k.
- **Lists** (u16 state indices in record order, then a count): flag bit 4
  `pgsv`, bit 11 `curse`, bit 16 `disguise`, bit 5 `active`, and states
  whose `itemtype` (i16 +0x2A) > 0. Each list buffer has n entries, zeroed;
  only the first count are used.

1.14d (185 states, W = 6): pgsv 122–127 (`progressive_*`); curse 26
states (9 `amplifydamage`, 19 `weaken`, 23, 27, 55, 56, 57, 58, …);
disguise 63, 93, 119, 139, 140, 176, 177; active 47, 62, 88, 143, 144,
150; itemtype 16 `enchant`, 31 `venomclaws`.

### 5. skills: class lists and passive list

Built in the skills loader (`0x00613F80`) before the pettype append
(`fixups.md` §3). Class c := i8 `charclass` (+0x0C); a skill counts when
0 ≤ c < 7. Passive: i16 `passivestate` (+0x94) ≥ 0.
- counts: 7 × i32 (skills per class); max := the largest count;
- class lists: 7 × max u16, zeroed; class c's k-th skill (record order)
  at c·max + k;
- passive list: u16 skill indices in record order, with its count.

Reader `0x006460F0`: class > 6 → −1; 0 ≤ k < count[c] → entry, else −1.
1.14d: 30 skills per class (Amazon 6–35, Sorceress 36–65, Necromancer
66–95, Paladin 96–125, Barbarian 126–155, Druid 221–250, Assassin
251–280); 147 skills have no class; 29 passives.

### 6. Items: version-0 list

After the combined item array (`loading.md` §9): a u16 array with one
slot per item, zeroed, holding in order the indices of the items whose
`version` (u16 +0xF6) is 0; the remaining slots stay 0. No stored
length. Reader `0x006335A0`: index outside 0 … item count − 1 → −1, else
the slot. 1.14d: 361 of 659 items (last 599, 600, 601).

### 7. gamble

Loader `0x00638AE0`, record 0x0C (`code` +0x00). If count = 0 the index
list is absent and every threshold is 0. Otherwise:
1. Each row r in file order: j := item code map find of u32 +0x00; j < 0
   is fatal (`loading.md` §8). Row +0x04 := items[j] u8 `level` (+0xFD),
   +0x08 := j.
2. Sort the 12-byte rows with §1, comparator: u32 +0x04 compared
   **unsigned**, +1 / −1 / 0.
3. Index list: u32 × count, entry i := sorted row i's +0x08.
4. Thresholds: 100 u32; slot 0 := 2 (a literal, Open question 1); slot
   L (1 … 99) := the first sorted position whose level > L, or count.
5. The records are freed. The block at `0x96CAB0` keeps u32 count, the
   index list pointer, then the 100 thresholds.

1.14d (125 rows from X `gamble.bin`, levels 1–52): sorted items start
520 `amu`, 522 `rin`, 25 `ssd`, 175 `ktr`, 47 `jav`, 68 `sbw`, 306 `cap`,
313 `qui`, 14 `clb`, 328 `buc` (level 1), 43 `tkf` (2); the last is 419
`ci1` (52). Thresholds L = 1 … 12: 10 11 17 18 25 27 32 36 39 41 48 54;
124 for L 40–51; 125 from L 52.

### 8. monseq, monpreset, hireling

**monseq** (`0x00659C60`, record 6 bytes, `sequence` i16 +0x00): if the
count is 0, nothing. Else E := last record's sequence + 1; an array of E
entries of 12 bytes, zeroed: (first record, count, count). For each
record in order: entry[sequence]: first := this record if still unset;
both counts += 1. 1.14d: 1,010 rows, E = 60, every entry used (entry 0:
1 row, 1: 15, 59: 23).

**monpreset** (`0x00659A10`, record 4 bytes, `Act` u8 +0x00): five
(first record, count) pairs, act a (0–4) for Act a + 1. cur := 0,
first[0] := record 0, run := 0. For each record: while cur + 1 < Act:
count[cur] := run, cur += 1, first[cur] := this record, run := 0; then
run += 1. At the end count[cur] := run. Rows are expected grouped by
ascending Act; an Act that is lower than the current one counts in the
current act. 1.14d: first rows 0, 47, 106, 145, 173; counts 47, 59, 39,
28, 56.

**hireling** (`0x00655720`, after the `fixups.md` §7 ids and the
`loading.md` §8 checks): two tables of 256 i32, all −1. For each row r in
order with `Id` i32 +0x04 < 256: table := second if `version` (u16 +0x00)
≥ 100, else first; if table[Id] < 0, table[Id] := r. 1.14d: Ids 0–29 in
both tables (version 0: Id 0 → row 0, 1 → 3, …, 29 → 107; version 100:
Id 0 → 12, 1 → 15, 2 → 18, 3 → 20).

### 9. leveldefs, lvlsub

**leveldefs** (`0x0061DD00`, after the load, record 0x9C): the list of
level indices whose `Portal` (u32 +0x8C) ≠ 0, in order, as u32, with its
count. 1.14d: 16 levels: 1, 3, 5, 7, 27, 29, 33, 36, 40, 43, 45, 46, 53,
54, 74, 134.

**lvlsub** (`0x0061F500`, before the path fix of `fixups.md` §12, record
0x15C, `Type` i32 +0x00): M := the largest Type (from 0). If M ≠ 0: an
array of M + 1 u32, zeroed; prev := 0; for each row r in order with
Type t ≠ prev: entry[t] := r, prev := t. Rows are expected grouped by
Type; entry 0 stays 0. 1.14d: 13 entries 0, 1, 2, 3, 4, 6, 10, 16, 17,
21, 28, 31, 33.

### 10. automap

Loader `0x0061FCF0`, record 0x2C. Each record becomes a 0x20-byte record
(zero-filled first); the loaded table is then freed.

| Off | Type | Value |
|---|---|---|
| +0x00 | i32 | LevelName (str +0x00) → index in list A |
| +0x04 | i32 | TileName (str +0x10) → index in list B |
| +0x08 | u8 × 3 | Style, StartSequence, EndSequence (+0x18..+0x1A) |
| +0x0B | u8 | 0 |
| +0x0C | i32 × 4 | Cel1–Cel4 (+0x1C..+0x2B) |
| +0x1C | i32 | cel count: Cels before the first −1 (0–4) |

Name → index, lists A (36) and B (20) and the fatal cases: `loading.md`
§8. A name starting with the character `0` (0x30) gives 0 without a
compare.

**Level-name ranges:** 36 (first, end) i32 pairs, index = list-A index,
all (−1, −1) first. Scanning the converted records in runs of equal
+0x00, a run [f, e) with index L sets pair L := (f, e); a later run of
the same L overwrites an earlier one.

1.14d: 3,286 records. Record 0 `1 Town`/`fl` → (1, 0, style 0, sequences
1–46, cels 0, 1, 2, 3, count 4). Cel counts 1 ×2,942, 2 ×117, 3 ×183,
4 ×44. Ranges: `1 Town` (0, 83), `1 Wilderness` (83, 225), `1 Cave`
(424, 550), `5 Lava` (3,210, 3,286); `None` and `5 Town` (−1, −1). Each
index has one run.

## Constants & data dependencies

| Constant | Value | § |
|---|---|---|
| short-sort limit | 8 elements | 1 |
| equivalence walk stack | 128 ints, overflow test > 124 | 2 |
| state flag bitsets | 40 | 4 |
| player classes | 7 | 5 |
| gamble thresholds | 100 (slot 0 = 2) | 7 |
| monpreset acts | 5 | 8 |
| hireling id tables | 2 × 256; version split at 100 | 8 |
| automap level-name ranges | 36 | 10 |

## Randomness

None.

## Edge cases & original bugs

Reproduced:
- Unstable sort order for equal keys (§1).
- itemtypes column 0 is set in every row; montype column 0 never (§2).
- The equivalence walk gives up on any link ≥ n or a deep stack, even if
  another branch would match (§2).
- Rows of monpreset / lvlsub / monseq out of the expected order are
  counted as §8–§9 say, not rejected.

Out of range in 1.14d (reads or writes outside an array); no 1.14d row
reaches them; d2rs reports a load error:
- an equivalence link < 0 that gets pushed (e1 > 0 with e2 < 0, or e3
  < 0) (§2);
- monseq sequence < 0 or ≥ E (§8);
- monpreset Act > 5 (writes past the five pairs) (§8);
- hireling Id < 0 (§8);
- lvlsub Type < 0 (§9).

## Test vectors

Real 1.14d (`#[ignore]`, `D2_GAME_DIR`): SHA-256 of each map as dumped
(`traces/raw/20261006-004246-tables/map-<name>.bin`). Maps holding
pointers are compared as record indices instead.

| Map | Size (bytes) | SHA-256 / content |
|---|---|---|
| itemtypes_equiv | 1,648 | `c3e681fbb4e036c8a716420d2f7d985a171956778ab68696f80c631d8de93c19` (426 bits) |
| montype_equiv | 472 | `be0573b993f084dfa2da5fe3f20192aeff88ce9b23c9d35fcf683b7842f4b0fe` (99 bits) |
| stat_stuff | 8 | 6, 0x3F |
| isc_desc_list | 414 | `cc75111492fa8a31e1e418c22cd26b4148b410775b8e0e31b1d30e953d35813a` |
| states_bitsets | 960 | `972d2fea453740881ed558a13f2b963dcff1ac3b002e9b103bd9edef8c036470` |
| states_curse | 52 | `63a3cc79999c5a932e3fd8dfa5760962443d34e48c4ae8c1a363df651420c58c` |
| skills_class_counts | 28 | 7 × 30 |
| skills_class_lists | 420 | `9f158e7c484942996e7b5e2c261f954eca96f8f39fe5a5415855c2192ee8c3fd` |
| skills_desc_list (passives) | 58 | 9, 13, 18, 23, 29, 33, 37, 61, 63, 65, 69, 79, 89, 100, 105, 108, 110, 127, 128, 129, 134, 135, 136, 141, 145, 148, 153, 252, 263 |
| items_f6_list | 1,318 | `735e8347683f78da874d3d1a8fd553e7955781584024d89adb3c301b1c3a92ca` |
| gamble_index | 500 | `679765be9b86bd1b6f49d89767316d022196a9360ca1d5ef0b145c6e391639f4` (CRC-32 `0xBF2AD54F`) |
| gamble_levels | 400 | `08b43d79345800c4af8004f5385378919511f64f0e59564b67c6fb893c3deb99` |
| monseq_index | 720 | counts as §8; first = record of the entry's first row |
| monpreset_acts | 40 | first rows 0, 47, 106, 145, 173; counts 47, 59, 39, 28, 56 |
| hireling_first | 2,048 | `0d8fb9d7fb590516cc968bd177165a98f7791de7f2dd55b3fd89d2f3934f696f` |
| leveldefs_portals | 64 | `7e738d6b793eab28fa2b389581d500244a2056c964999450d25be1069245571f` |
| lvlsub_type_first | 52 | `ba596bac52c15c2fde44a8d47b9aac88c5dd73de35be5c3e031b8f34db38e4c6` |
| automap_runtime | 105,152 | `871b0f4ff5e601db868e01b1e3e19adf85e099180f9178324d3ab82f62f063fa` |
| automap_level_index | 288 | `f4d5f80abd4e022936d3a7e080bf9ee27a1e6c9f9926176341963ea8b7a667a7` |

Row vectors (itemtypes): row 28 `axe ` = columns 0, 28, 45 `weap`, 46
`mele` (words 0x10000001, 0x00006000, 0, 0); row 85 `abow` = 0, 27, 45,
47, 59, 60, 85; row 91 `gem0` = 0, 20, 52, 53, 91. montype: row 8
`skeleton` = 1 `undead`, 6 `lowundead`, 8 (word 0x142); row 37
`undeadfetish` = 1, 2 `demon`, 6, 37.

Synthetic (CI-safe):

| Input | Expected |
|---|---|
| sort keys (8 equal), rows 0–7 | 1, 2, 3, 4, 5, 6, 7, 0 |
| sort keys (10 equal), rows 0–9 | 0, 1, …, 9 |
| sort keys 2,1,2,1,2,1,2,1,2,1,2,1 (rows 0–11) | rows 11, 1, 7, 3, 5, 9, 2, 8, 6, 4, 10, 0 |
| itemtypes n = 4, e1 = [0, 0, 1, 2], e2 = 0 | row 3 = bits 0–3; row 2 = 0, 1, 2; row 0 = 0 |
| itemtypes e1(3) = 0, e2(3) = 1 | row 3 = 0, 3 (e2 ignored) |
| itemtypes n = 4, e1(3) = 7 | row 3 = 0, 3; Equiv(3, 1) = 0 (7 ≥ n stops the walk) |
| montype n = 4, e1 = [0, 0, 1, 2] | row 3 = 1, 2, 3; row 0 empty |
| gamble levels 3, 1, 2 | index order of levels 1, 2, 3; thresholds L1 = 1, L2 = 2, L3…99 = 3, slot 0 = 2 |
| gamble count 0 | no list; 100 zero thresholds |
| monpreset Acts 1, 1, 3 | first = 0, 2, 2; counts 2, 0, 1 |
| lvlsub Types 0, 2, 2, 5 | 6 entries: 0, 0, 1, 0, 0, 3 |
| automap LevelName `0abc` | index 0 |

## Provenance

1.14d `Game.exe` (1.14.3.71) Ghidra exports, checked against raw
disassembly for register arguments. D2MOO (1.10f) supplied names and the
data-table offsets (`D2DataTbls.h`), as hints only.

- **Confirmed by post-load dump (`dump_tables.py`, 2026-10-06):** every
  map of §2–§10, byte for byte (`traces/raw/20261006-003056-tables` and
  `20261006-004246-tables`; the second run added `isc_desc_list`,
  `states_*`, `hireling_first`, `leveldefs_portals`, `monseq_index` and
  `lvlsub_type_first` to the tool's `MAPS`). The §1 sort is confirmed by
  two independent lists (gamble, itemstatcost) whose stable order would
  differ in 93 and 63 positions.
- Sort: `_qsort` `0x00685B50`, short sort `0x00685AC0`, swap `0x00685A90`
  (Ghidra library match "Visual Studio 2005 Release"; read instruction by
  instruction). Comparators `0x006379D0` (signed 16-bit), `0x00638AC0`
  (unsigned 32-bit).
- itemtypes `0x00638D80` (matrix `0x00639336`–`0x006393D8`), Equiv
  `0x00638CD0`; montype `0x00658520`, Equiv `0x00658460` (`j < 1 → 0`,
  three links); mask table `0x006CE268`.
- itemstatcost `0x00637A00`: stuff `0x00638208`, list
  `0x0063851C`–`0x006385BC` (`cmp byte [+0x36]`, 4-byte elements).
- states `0x00618100` (block size W·0xA0, 40 pointers at `0x96BCFC`),
  bitset `0x00611D60`, flag lists `0x00611DC0` (bits 4, 0xB, 0x10, 5 into
  `0x96BD9C`/`A4`/`AC`/`B4`, u16 counts at +4), itemtype list
  `0x00611E60` (+0x2A into `0x96BDBC`).
- skills `0x00613F80` (`0x00617966`–`0x00617BAC`); reader `0x006460F0`.
- items `0x006315D0` (version list `0x96CA78`, reader `0x006335A0`).
- gamble `0x00638AE0` (block `0x0096CAB0`, accessor `0x00638CC0`, free
  `0x00638C80`, literal 2 at `0x0096CAB8`).
- monseq `0x00659C60` (`0x00659D29`–`0x00659DD4`); monpreset
  `0x00659A10` (clear `0x00659AF0`); hireling `0x00655720`
  (`0x00656180`–`0x0065625D`: −1 fill, `version ≥ 0x64` → +0x100,
  signed `Id < 0x100`).
- leveldefs `0x0061DE00` → `0x0061DD00` (fatal 0x177 cannot fire: both
  loops count the same rows); lvlsub `0x0061F500`.
- automap `0x0061FCF0` (conversion from `0x0061FEB0`, name lookups
  `0x0061FC10` / `0x0061FC80`, ranges at `0x0096C8D0`).

## Open questions

1. Gamble threshold slot 0 is the literal 2; its meaning and whether
   readers treat it as level 0 depend on the gamble code (not traced).
2. Readers of the montype matrix, the states lists, the monseq and
   lvlsub indexes, the leveldefs list and the hireling tables were not
   traced; their contents are confirmed, their uses belong to the Phase 3
   specs that read them.
3. The treasure-class runtime form (`loading.md` §10.6; the
   `treasureclassex` records are freed) is not dumped or specified here.
4. Where 1.14d reads out of range, d2rs reports the error at the point
   the spec names; for §2 that is when a negative link is pushed, even if
   the walk would end before popping it (1.14d reads out of range only on
   the pop). Equal for every 1.14d row; whether the earlier error can
   reject data 1.14d accepts is not traced.
