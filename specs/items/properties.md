# Spec: Items — Properties to stats, uniques, sets, runewords, socket fillers

- **Status:** draft: every rule read from the 1.14d `Game.exe` code
  (addresses per rule; the function table read from the 1.14d data at
  `0x007462F8`); no recording yet; test vectors are synthetic.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::items::props` (dispatcher, property
  functions, modes); `property-functions.tsv` (this folder) is the
  machine-readable function list (one row per function id)
- **Related specs:** `items/generation.md` (§1 conventions, §8.2
  ethereal apply); `items/quality.md` (§7 superior → mode 1, §8 unique →
  mode 3, §9 set → mode 4); `items/affixes.md` (mode 0); `sim/stats.md`,
  `sim/stat-lists.md` (stat lists, states, flags, set/add, how an item's
  lists reach its owner); `data/fields.tsv` (`properties`, `itemstatcost`,
  `uniqueitems`, `setitems`, `sets`, `runes`, `gems` layouts);
  `data/fixups.md` §5–§6 (gem offsets, set slots and counts).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 47–58 |
| Inputs | 59–66 |
| Outputs / state changes | 67–72 |
| Rules | 73–74 |
|   1. Property record and slots | 75–83 |
|   2. Modes (`0x0065FEC0`, D2MOO `ITEMMODS_AssignProperty`) | 84–103 |
|   3. Dispatcher (`0x0065FD70`; wrapper `0x0065FE10` for format ≥ 1) | 104–113 |
|   4. Shared helpers | 114–163 |
|   5. Property functions | 164–218 |
|   6. Superior (mode 1) and affixes (mode 0) | 219–223 |
|   7. Uniques (mode 3) | 224–227 |
|   8. Set items | 228–238 |
|   9. Socket fillers (`0x0055C2C0`) | 239–258 |
|   10. Runewords | 259–317 |
|   11. Set bonuses (`0x00660120`) | 318–330 |
|   12. Craft property lists (`0x00660240`) | 331–336 |
|   13. Set-item state update (`0x00663CC0`) | 337–392 |
|   14. Format-0 property functions (legacy table `0x00745B58`) | 393–460 |
| Constants & data dependencies | 461–470 |
| Randomness | 471–476 |
| Edge cases & original bugs | 477–486 |
| Test vectors | 487–505 |
| Provenance | 506–525 |
| Open questions | 526–611 |
<!-- /index -->

## Summary

Every magic effect on an item is a property row (properties.txt) applied
with a parameter, a min and a max. A property has up to seven slots;
each slot names a function (1–24, 36), a stat, a "set" flag and a value.
The dispatcher runs the slots in order; the functions roll values from
the item seed and write stats into a stat list of the item. This spec
owns the property record, the modes that list which properties a
source (affix, quality row, gem, unique, set, runeword, craft list)
contributes, the dispatcher, every property function, runeword matching
and activation, socket-filler properties and set bonuses.

## Inputs

| Name | Source |
|---|---|
| property record `{code i32, param i32, min i32, max i32}` | a row of an affix, quality, unique, set, runes, gems or craft table |
| item, owner unit (may be none), state, stat-list flags | the caller |
| properties, itemstatcost, skills tables | `data/fields.tsv` |

## Outputs / state changes

Stats in the target stat list (created on demand), item flags (socketed,
ethereal, runeword), item seed draws, the return value of each function
(passed between slots).

## Rules

### 1. Property record and slots

1. A property record is 16 bytes: `code` (properties row; < 0 = none),
   `param`, `min`, `max`. Affix `modN*`, quality `modN*`, unique
   `propN/parN/minN/maxN`, set `propN…`/`apropNx…`, sets `pcode*/fcode*`,
   runes `t1code*`, gems `*mod*` columns all use this shape.
2. A properties row has slots k = 0…6 with `funcK+1`, `statK+1`, `setK+1`,
   `valK+1` (`data/fields.tsv` `properties`).

### 2. Modes (`0x0065FEC0`, D2MOO `ITEMMODS_AssignProperty`)

Arguments: mode, extra unit, item, source row, property set, apply type.

| Mode | Source | Records | Stop rule |
|---|---|---|---|
| 0 | magic affix row | `mod1`–`mod3` (+0x24, 16 apart) | first `code` < 0 ends |
| 1 | qualityitems row | `mod1`, `mod2` (+0x0C) | first `code` < 0 ends |
| 2 | gems row (gem) | block by property set: 0 weapon (+0x30), 1 helm (+0x60), 2 shield (+0x90); 3 records | first `code` < 0 ends |
| 5 | gems row (rune) | as mode 2 | as mode 2 |
| 3 | uniqueitems row = file index | `prop1`–`prop12` (+0x8C) | all 12 run; `code` < 0 skipped |
| 4 | setitems row = file index | `prop1`–`prop9` (+0x88), then `aprop1a`–`aprop5b` (+0x118, 10 records) | all run; `code` < 0 skipped |

Mode 3/4 do nothing when the file index is outside the table. Every
mode targets the item's own stat list with state 0 and flags 0x40
(owner none), except the set partial records (§8.1). Mode 4 on a format-0
item runs only `prop1`–`prop2` (`0x0065FF6C`: format 0 → two records,
no partial records), each through §14. Modes 6
(runeword, §10) and 7 (craft list, §12) call the dispatcher directly.

### 3. Dispatcher (`0x0065FD70`; wrapper `0x0065FE10` for format ≥ 1)

For one property record: look up the properties row (`code` < 0 or out
of range → nothing). prev := 0. For slot k = 0…6: f := `funcK+1`; f ≥ 37
or table entry `0x007462F8`[f] empty (f = 0, 25–35) → stop. Call f with
(mode, owner, item, record, `setK+1`, `statK+1`, `valK+1`, prev, state,
flags, extra). After slot 0 only, prev := its return value. So every
later slot of the property reuses slot 0's value when its function reads
prev (functions marked prev in the TSV).

### 4. Shared helpers

#### 4.1 Value roll (`0x0065E9E0`)

roll(min..max): max = min → min (no draw). Swap if max < min. Then min +
roll(max − min + 1) on the **item seed** when the target is an item (else
the unit's seed).

#### 4.2 Add to the stat list (`0x0065EA50`)

Arguments: owner, record, set, stat, layer, value, state, flags. Nothing
(return 0) if the record is none, value = 0, or the stat is out of
itemstatcost. List: the owner's list with this state and flags if an
owner is given, else the item's; created if missing (`sim/stat-lists.md`).
v := value << itemstatcost `valshift`. set ≠ 0 → **set** stat (layer) :=
v; and for stat 58 (`poisonmaxdam`): if stat 326 (`poison_count`) is 0,
set it to 1. set = 0 → **add** v; for stat 58 also add 1 to stat 326.
Return value (unshifted).

Register mapping (1.14d, `disasm.py fn` on `0x0065FE10`, `0x0065FD70`,
`0x0065EA50`, `0x0065CBF0`, Open question 3): the dispatcher's second
stack argument is the owner O and its third the item I; every property
function receives I as its first stack argument (the unit of every
§4.1 roll and §4.3 reset) and O in EDX, and passes O as `0x0065EA50`'s
first stack argument with I in ECX, flags in EDX, state as its last
argument. `0x0065CBF0`(EAX O, EBX flags, I, state): the unit is O when
O ≠ none, else I; its list of (state, flags) (`0x00625790`: state ≠ 0 →
the list of that state, state 0 → the first list with those flags);
none → a new list (that unit's pool, flags, expiry 0, owner type 4,
that unit's GUID; GUID −1 when I is none too), attached with reset 1,
state set; a null list is fatal (0x201). Who is O: modes 0–5 and 7 pass
O = none (the wrapper's second argument is 0; the mode's extra unit
goes to the dispatcher's last argument), so they write I's own list
(for a gem or rune: the filler's); §11 passes O = the player; mode 6
(§10.2) passes O = the socketed item and I = the inserted filler.

#### 4.3 Base reset (`0x0065CCC0`)

Called with the slot's stat by functions marked "reset" (mode 1 = only in
mode 1). Uses the items row found by the item's code:

| Stat | Effect |
|---|---|
| 16 (`item_armor_percent`), 31 | armor with `maxac` ≠ 0: stat 31 := max(base 31 + 1, `maxac` + 1) |
| 17 (`item_maxdamage_percent`), 22 | weapon: stat 22 := `maxdam`, 24 := `2handmaxdam`, 160 := `maxmisdam` if throwable (each only when ≠ 0) |
| 18 (`item_mindamage_percent`), 21 | weapon: 21 := `mindam`, 23 := `2handmindam`, 159 := `minmisdam` if throwable |

This makes superior and enhanced-defense items take the top of their
base defense range plus one.

### 5. Property functions

`property-functions.tsv` lists every function: id, 1.14d address, value
source, draws, base reset, stats written, layer, return. Columns:
`value` prev_or_roll = prev if ≠ 0 else §4.1; `layer` param = the
record's `param`, val = the slot's `val`; `returns` added = §4.2's
return. Functions not in the table (0, 25–35, ≥ 37) end the slot loop.
Rules beyond the table:

1. **5 (min damage):** items row = the owner's if the owner is an item,
   else the item's. v := prev or roll. Add to stat 21 unless (weapon and
   `mindam` = 0 and `2handmindam` ≠ 0); add to 23 unless (weapon and
   `2handmindam` = 0 and `mindam` ≠ 0); add to 159 unless (weapon and not
   throwable). For each, if the column ≠ 0 and column + v < 1, the
   amount is 1 − column; amount 0 is skipped. Return v.
2. **6 (max damage):** the same with stats 22 / 24 / 160 and columns
   `maxdam` / `2handmaxdam` / `maxmisdam`; the floor amount is −column.
3. **7 (enhanced damage %):** v := prev or roll. Item target: base reset
   of the min and max damage stats (§4.3 rows 17 and 18); b :=
   max(`maxdam`, `2handmaxdam`); if weapon and b × v / 100 (64-bit,
   toward zero) ≤ 0 → run function 6 with prev 1 and return its result;
   else add stat 18 then stat 17 with v, return v. Non-item target: add
   18 and 17, return 1.
4. **11 (skill on event):** skill := `param` (out of skills → 0).
   chance := `min`, < 1 → 5. level := `max` when > 0; when 0: ((item level
   − skill `reqlevel`) / 4 toward zero) + 1, at least 1, at most the
   skill's `maxlvl` (20 when < 1); when < 0: s := max(99 − reqlevel, 1);
   d := max(−(s / `max`), 1); level := (item level − reqlevel) / d, ≤ 0 → 1.
   Add the stat with layer skill × 64 + (level & 63), value chance.
5. **13 (durability):** after a non-zero add, m := the item's max
   durability (`0x00625E00`); m > 0 → stat 72 := m.
6. **14 (sockets):** cap := min(`invwidth` × `invheight`, 6) (0 → return
   0), then min(cap, max sockets, `items/generation.md` §7.2). n := prev
   if ≥ 1, else roll; n < 1 → `param`. Result := min(max(n, 1), cap) (cap
   < 1 → 0). Set flag 0x800 and **set** stat 194 := result directly on
   the item. Return result. (No quality caps, unlike generation §7.3.)
7. **15 / 16 / 17:** value `min` / `max` / (`param`, or roll when 0;
   0 → return 0). Stat 21 (for 15) → function 5 with prev := value; stat
   22 (for 16, 17) → function 6; other stats → add. Return the value.
8. **18 (by time):** p := clamp(`param`, 0, 3); a := clamp(`min` + 256, 0,
   1023); b := clamp(`max` + 256, 0, 1023); **set** the stat := p + (b ×
   1024 + a) × 4, layer 0. Return b.
9. **19 (charges):** skill := `param` (invalid → 0). level from `max` as
   in rule 4. c := `min`: 0 → 5; < 0 → −min + (−min × level) / 8 (toward
   zero); then c ≤ 1 → 1, c > 254 → 255. r := roll(c − c / 8) (item
   seed). **Set** the stat := c × 256 + ((r + c / 8 + 1) & 0xFF), layer
   (skill << shift) + (level & mask) with the global shift and mask at
   table +0xC6C / +0xC70 (the itemstatcost layer split,
   `sim/stats.md`). Return c.
10. **20:** add stat 152 := 1 (when itemstatcost has > 152 rows). Return 1.
11. **23:** item not yet ethereal and has durability → apply ethereal
    (`items/generation.md` §8.2), return 1; else 0.
12. **12 / 36:** layer := roll(min..max); value := `param` (12) or the
    slot's `val` (36).

### 6. Superior (mode 1) and affixes (mode 0)

No extra rules: the routine that picks the row calls mode 0 or 1 once
per row (`items/quality.md` §7, `items/affixes.md` §3, §7, §8, §11).

### 7. Uniques (mode 3)

`items/quality.md` §8 picks the row; mode 3 runs its 12 records in order.

### 8. Set items

#### 8.1 Item properties (mode 4)

`prop1`–`prop9` go to the item's list (state 0, flags 0x40). The ten
partial records `aprop1a`, `aprop1b`, …, `aprop5b` (k = 0…9): if the
setitems `add func` ≠ 0 → state 165 + k / 2 (`itemset1`–`itemset5`,
table `0x006EDB40`) with flags 0x2040 (`0x006EDB5C`); else state 0 and
flags 0x40. Which state lists are active is decided by the set-bonus
update (§11) and the stat-list rules (`sim/stat-lists.md`).

### 9. Socket fillers (`0x0055C2C0`)

Run when a filler is inserted (the insertion intent is owned by the
inventory spec; `0x00562660`):

1. Filler of type `gem` (20): gems row := items `gemoffset` of the filler
   (`data/fixups.md` §5); mode 2 with property set := the socketed
   item's apply type (`0x00629A40`: items `gemapplytype`, 0 weapon, 1
   helm, 2 shield).
2. Filler of type `rune` (74): the same with mode 5 (extra unit = the
   socketed item).
3. Otherwise, an item of quality 5 (set): `0x00663CC0(owner, item, 0,
   0)`, §13. (`0x0055C2C0`(EDX item, owner, flag) is the stat refresh of
   any item placed with an owner, a player or an item, not only of a
   filler; the gem and rune branches need an item owner and return at
   once otherwise. Rule 3 is the branch a set item takes when it is
   equipped by a player; a set item never is a filler.)
4. The properties land in the filler's own list; they reach the socketed
   item through stat-list linking (`sim/stat-lists.md`).

### 10. Runewords

#### 10.1 Match (`0x0062BED0`)

No runeword for: quality 4–9, quest items, items without an inventory.
Fillers: the socketed items' records in insertion order (≤ 6). The
socket count (stat 194) must equal the filler count. Runes rows in
order: `complete` ≠ 0; `rune1`… (stop at the first < 1) must equal the
fillers in order and cover at least the socket count; none of `etype1`–
`etype3` (stop at 0) may match the item; any of `itype1`–`itype6` (stop
at 0) must match. First matching row wins.

Exact form (1.14d `0x0062BED0`, item → runes record or none):

1. None for: no item; a unit of type 4 with item data whose quality
   (item data +0x00) is 4–9; items `quest` (+0x12A) ≠ 0; no inventory
   (unit +0x60); an empty inventory (no first item, inventory +0x0C).
2. Filler class ids: walk the item's inventory list (first +0x0C, next
   item data +0x64) and take each unit's class id (unit +4). A unit in
   the list that is not an item (type ≠ 4) → none. Count c.
3. c ≠ socket count (`0x006299B0`, u8) → none.
4. For each runes record in row order (0x120 bytes): `complete` (+0x80)
   = 0 → next. For i = 0…5 while `rune`i+1 (+0x98 + 4i) > 0: i > c →
   next record; class id i ≠ `rune`i+1 → next record. The number of
   runes matched must be ≥ c. Then `etype1`–`etype3` (+0x92, i16, stop
   at 0): any match (itemtypes equivalence, `0x00629BB0`) → next
   record. `itype1`–`itype6` (+0x86, stop at 0): the first match →
   **return this record**.
5. No record → none.

The record's +0x82 (the name's string id, `data/fixups.md` §7 runes
row) is what `items/bitstream.md` §4.4 rule 1 sends; d2rs's bit-stream
writer gets the record from this rule (handoff BV5).

Edge: a row with exactly c + 1 runes whose first c runes match compares
rune c + 1 with class-id slot c, which step 2 never wrote (a stack value
left from earlier calls; c ≤ 5 here, as at most 6 runes are read). It
matches only if that stale value equals the rune's class id; d2rs
treats the unset slot as "no class" (no match), Open question 4.

Load: a top-level item with item flag 0x4000000 (runeword) that fails
this match when its character is loaded is deleted when stored or on the
cursor and unequipped when equipped (`0x00563470`, `formats/d2s-load.md`
§6).

#### 10.2 Activation (`0x00562660` → `0x006600A0`)

After a filler is inserted and its properties applied (§9): row := §10.1.
A row with `server` ≠ 0 is skipped unless game +0x74 (ladder) ≠ 0. Else,
if the item has no list with state 171 (`runeword`) and flags 0x40: set
flag 0x4000000; dispatcher mode 6 for `t1code1`… (stop at the first < 0,
max 7), owner = the item, state 171, flags 0x40. Then the replenish
timers (`items/generation.md` §9 step 6). The dispatcher's item argument
is the filler just inserted (`0x00562823`: `0x006600A0(item, filler,
0)`; `0x006600FE`), so every §4.1 roll of a runeword draws on that
filler's item seed and every §4.3 reset reads the filler's items row,
while the stats go into the socketed item's state-171 list (§4.2
register mapping).

### 11. Set bonuses (`0x00660120`)

For an equipped set item (quality 5) with state s (given by the caller,
the owner-list step of §13, step 6): mask := the set slots
(setitems +0x2E) of the owner's equipped set items of the same set,
including this one, excluding items flagged no-equip (0x4000) or broken
(0x100) (`0x0062A370`). c := popcount(mask) (table `0x006EDA40`, masks ≥
64 → 0). n := min(c, set item count − 1) (count: sets +0x0C,
`data/fixups.md` §6). Records `pcode2a`, `pcode2b`, … the first 2n − 2
(code < 0 skipped): mode 4 through the dispatcher with owner = the
player, state s. If c ≥ the set item count: `fcode1`–`fcode8` (stop at the
first < 0).

### 12. Craft property lists (`0x00660240`)

Mode 7 for one record list (owner none, flags 0x40); then, if the item
is flagged ethereal (0x400000), re-apply ethereal (`items/generation.md`
§8.2). The list and when it runs belong to the cube spec.

### 13. Set-item state update (`0x00663CC0`)

`0x00663CC0`(owner O, item I, remove r, repark p) (stdcall, 4
arguments). Callers and arguments: the stat refresh `0x0055C2C0` (§9
rule 3) (0, 0); the repair `0x0055F900` (0, 0); the deactivation
`0x0055C730` (1, 0); the break `0x0055F850` (1, 1); `0x0057F410` (1, 1);
the client equip (`client/stat-lists.md`) (0, 0).

1. O none → return 0. I none, not an item (type ≠ 4), or O without an
   inventory (unit +0x60) → return 0.
2. mask := 0. If I is in O's inventory (`0x0063E070`: item data +0x5C =
   O's inventory) and its node page (item data +0x69, `0x0063E020`) is
   3 (body), mask := the set mask of §11 with I included
   (`0x0062A370(O, I, 1)`).
3. If p ≠ 0 or r = 0: park / unpark I's partial lists (`0x00663A20`(ECX
   I, EDX mask)), step 5.
4. Owner list (`0x00663B40`(ECX O, EDX I, r)), step 6. Return 1.
5. Partial lists of I (`0x00663A20`). Nothing unless I has quality 5
   and a setitems row (file index item data +0x28 below the count;
   `0x0062A490`). f := setitems +0x87 (`add func`, u8). Park and unpark
   are `sim/stat-lists.md` §8.5 on I's own lists of the states
   S[0..5] = 165, 166, 167, 168, 169, 170 (table `0x006EE424`); a state
   without a list on I does nothing.
   - f = 0: nothing.
   - f = 1: n := setitems +0x2E (I's set slot, i16, `0x0062B3A0`). For
     i = 0 … 5, i ≠ n: k := i, minus 1 when i > n; mask bit i set →
     unpark S[k], clear → park S[k].
   - f = 2: b := popcount(mask) (table `0x006EE440`; mask ≥ 64 → 0),
     k := b − 1. Unpark S[0..k−1]; then, when k < 5, park S[k..4]. For
     mask 0 (k = −1) the first park reads the dword before the table
     (`0x006EE420` = 3375), a state no list has (nothing), then parks
     S[0..4].
   - any other f: nothing.
   So the partial records of §8.1 (states 165 + j / 2) are active for
   exactly the equipped set slots (f = 1) or for the first b − 1 pairs
   (f = 2).
6. Owner list (`0x00663B40`). r > 1 (signed) → 0. O or I none, I not an
   item, I's quality ≠ 5 → 0. O an item of quality 4–9 (`0x0062A0F0`) →
   0. set := setitems +0x2C (i16) of I's row (`0x00483440`; no row → 0);
   set < 0 or ≥ the sets count (table +0xC10) → 0. free := −1. For j =
   0 … 5: L := O's list of state S[j] (`0x006256B0`).
   - No L: free := j when free < 0; next j.
   - L's stat 71 (layer 0, `0x00625D00`) ≠ set: next j.
   - Else: r = 0 → remove all of L's stats (`0x00627340`), add stat 71
     := set (`0x00627030(L, 71, set, 0)`), then the set bonuses §11
     (`0x00660120`(ECX O, EDX I, state S[j])); r = 1 → detach L from O
     (`0x006277E0`) and free it (`0x00626CD0`); r < 0 → nothing. Return
     1.
   After the loop: r = 1 or free < 0 → 0. Else a new list
   (`0x006251F0`(O's pool, flags 0, expiry 0, I's unit type, I's GUID
   +0x0C)) attached to O with reset 1 (`0x00626E10`), state S[free]
   (`0x006252D0`); then r = 0 → stat 71 := set and §11 with S[free] (no
   remove-all; the list is new); r < 0 → nothing. Return 1.
   So each equipped set has one owner list (states 165–170, at most six
   sets at once) tagged with stat 71 = the set id, which §11 refills.

### 14. Format-0 property functions (legacy table `0x00745B58`)

The wrapper `0x0065FE10` (every mode except 6, and §11) sends an item
of format 0 (`0x0062A670` < 1) here instead of §3 (format 0: items of
version-0x47 saves only, `items/generation.md` §1.2). No record →
fatal 0x66D. `code` = −1, < 0 or ≥ the properties count (table +0xAC,
268 rows in 1.14d) → nothing. Else (f, s) := the 8-byte entry `code` of
the legacy table (function, stat); f = 0 → nothing; else f(ECX mode,
EDX owner; item, record, s, n, state, flags, extra), where n is the
wrapper's sixth argument (`0x0065FEC0`'s apply-type argument; 0 from
§11 and §12) and owner, state, flags as in §4.2. One function per property code: there are no slots
and no prev value.

The legacy table has 244 entries (codes 0–243); entry 244 is the first
word of the §3 table at `0x007462F8`. So codes 244–267 read §3's
function words as (function, stat) pairs: codes 244 and 257–261 meet a
null function and do nothing; every other code 245–267 calls an address
that is not a legacy function (a §3 property function, which takes nine
arguments, or data), which corrupts the stack: a crash. 1.14d data
gives such codes only to expansion rows, which format-0 rolls skip
(`version` < 100), but a forced unique or set index could still reach
one.

Helpers, all on the item seed and the item (`ECX`/item argument):

- R(min, max): min = max → min, no draw. Else lo := min(min, max), hi
  := max(min, max), n := hi − lo, plus 1 when format ≥ 1; result lo +
  roll(n) (`0x0045C3E0`; n < 1 → 0 with no draw). So for format 0 the
  value max is never rolled (n = hi − lo), and a span of 1 still draws.
- A(stat, v) (`0x0065D070`): base reset §4.3 of the stat when mode = 1
  (or when the caller forces it); v = 0 → return 0. v := v × 256 for
  stats 6–11, 216, 217 (`0x0065CE40`). List as §4.2 (`0x0065CBF0`; none
  → fatal 0x2C5). n ≠ 0 → add −(the list's current value) instead of
  v. Add (`0x00627030`, layer 0). Returns 1.
- `0x0065CF40`(stat, kind): v := R(record min, max); base reset §4.3 of
  the stat when kind ≠ 0 or mode = 1; then A(stat, v), and for stat 58
  also add 1 to stat 326. Returns 1.

Functions (code → stat as stored in the table):

| Function | Codes → stats | Effect |
|---|---|---|
| `0x0065D1C0` | 0→31, 1→32, 2→33, 3→34, 4→36, 6→35, 7→0, 8→2, 9→3, 10→1, 11→9, 13→7, 15→19, 16→20, 17→54, 18→55, 19→56, 20→48, 21→49, 22→50, 23→51, 24→57, 25→58, 26→59, 31→39, 32→40, 33→41, 34→42, 35→43, 36→44, 37→37, 38→38, 39→45, 40→46, 43…50→142…149, 51→73, 53→74, 54→78, 58→79, 59→80, 60→81, 63→11, 64→82, 65→62, 66→60, 72…75→88…91, 88…91→110…113, 92…95→115…118, 97→120, 100…102→123…125, 105→128, 106…113→134…141, 114→150, 115…120→153…158, 181→254 | `0x0065CF40`(s, kind 0) |
| `0x0065D2B0` | 5→16, 12→77, 14→76, 30→114, 52→75, 61→28, 62→27, 96→119, 98→121, 99→122 | `0x0065CF40`(s, kind 1) |
| `0x0065DB90` | 104 → 127 | `0x0065CF40`(127, kind 0) |
| `0x0065E450` | 55–57→93, 76–78→96, 79–81→99, 82–84→102, 85–87→105 | A(s, R) |
| `0x0065D110` | 141–177→214–250, 179→252, 180→253 | v := `param` (0 → return 0); base reset when s is 16–18 or mode = 1; A(s, v) |
| `0x0065DD80` | 195–230 → 268–303 | as function 18 (§5 rule 8) but min > max, `param` > 3, min + 256 or max + 256 > 0x3FF (unsigned) are fatal (0x444, 0x44C, 0x44D, 0x44E) instead of clamped; list set; returns 1 |
| `0x0065E2D0` | 67–71→83–87, 121→179, 122→180 | v := R (0 → return 0); add stat **83** with layer 0–4 for s 83–87, 5 for 179, 6 for 180 |
| `0x0065E230` | 103 → 126 | v := R (0 → 0); add stat 126, layer 1 |
| `0x0065E170` | 123 → 107 | v := R; skill := `param` (outside skills → 0); add stat 107, layer skill |
| `0x0065E070` | 137 → 54, 138 → 57 | no roll: add s := `min`, s + 1 := `max`, s + 2 := `param` (n: negatives of the current values); s = 57 → stat 326 + 1 |
| `0x0065DE40` | 134→48, 135→50, 136→52, 139→159 | no roll: add s := `min`, s + 1 := `max` (n as above) |
| `0x0065DE40` | 140 → 21 | no roll; items row of the owner if it is an item, else of the item: 21 := `min`, 22 := `max` unless (weapon, `maxdam` = 0, `2handmaxdam` ≠ 0); 23 := `min`, 24 := `max` unless (weapon, `2handmaxdam` = 0, `mindam` ≠ 0); throwable → 159 := `min`, 160 := `max` (each A) |
| `0x0065D650` | 27 → 21 | v := R; same items row; A(21, v) unless (weapon, `mindam` = 0, `2handmindam` ≠ 0); A(23, v) unless (weapon, `2handmindam` = 0, `mindam` ≠ 0); A(159, v) when not a weapon or throwable (no floors, unlike §5 rule 1) |
| `0x0065D7D0` | 28 → 22 | the same with 22 (`maxdam` = 0, `2handmaxdam` ≠ 0), 24 (`2handmaxdam` = 0, `mindam` ≠ 0) and 160 |
| `0x0065D950` | 29 | format 0: `0x0065CF40`(17, kind 1), then (18, kind 1): two rolls, base reset each |
| `0x0065D310` | 41 | format 0: `0x0065CF40`(39), (41), (43), (45), kind 0: four rolls |
| `0x0065D4B0` | 42 | format 0: `0x0065CF40`(40), (42), (44), (46), kind 0 |
| `0x0065D220` | 133 → 194 | set flag 0x800 and the socket count := `param` (`0x0062BE00`; no cap, unlike §5 rule 6) |
| `0x0065D270` | 242 | the extra unit if it is an item, else the item: base stats 73 and 72 := 0 |
| `0x0065DBC0` | 243 → 204 | function 19 (§5 rule 9) with the same formulas (charges, level, roll(c − c / 8)), list set; returns 1 |
| `0x0065E440` | 124–132, 178, 182–194, 231–241 | nothing (returns 1) |

(`0x0065D950`, `0x0065D310`, `0x0065D4B0` test the format again and
have a format ≥ 1 branch, unreachable since the table is used only for
format 0.)

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| function table | 37 entries at `0x007462F8`, count `0x00745B54` | §3 |
| set partial states | 165–169, flags 0x2040 | §8.1 |
| runeword state | 171 | §10.2 |
| popcount table | `0x006EDA40` | §11 |
| poison count stat | 326, with stat 58 | §4.2 |

## Randomness

Item seed only (§4.1; function 19's charge roll). Draw order = record
order, slot order inside a record (only slots whose function rolls), so
for an affix: mod1's rolls, mod2's, mod3's.

## Edge cases & original bugs

1. Only slot 0's return is passed on; functions 3, 4, 5, 6 … in later
   slots reuse it (e.g. min/max damage pairs share one roll).
2. A value of 0 adds nothing (§4.2), so a 0 roll leaves no stat.
3. Function 14 ignores the quality caps that generation sockets use.
4. Function 7 on a weapon whose bonus rounds to ≤ 0 adds +1 max damage
   instead of the percentages.
5. A function id 0 in slot 0 makes the whole property do nothing.

## Test vectors

Synthetic, from the rules:

| Input | Expected | Source |
|---|---|---|
| §4.1 min 5, max 5 | 5, no draw | synthetic |
| §4.1 min 15, max 5, item seed `{1, 666}` | 5 + (lo′ mod 11) | synthetic |
| func 19: min 0, level 1, item seed `{4242, 666}` | c 5, roll(5) → value 1283 | synthetic |
| func 19: min −3, level 10 | c 6, value 1541 (seed `{4242, 666}`) | synthetic |
| func 19: min −12, level 20 | c 42, roll(37), value 10771 | synthetic |
| func 19: min 300 | c 255, roll(224), value 65524 | synthetic |
| func 18: param 1, min −50, max 50 | 1254201 | synthetic |
| func 18: param 5, min −300, max 900 | 4190211 (p 3, a 0, b 1023) | synthetic |
| func 10: param 7 | layer 1 + 2 × 8 = 17 | synthetic |
| func 5: weapon `mindam` 3, v −5 | stat 21 += −2 | synthetic |
| func 6: weapon `maxdam` 3, v −5 | stat 22 += −3 | synthetic |
| §11 mask 0b1011, 6-item set | c 3, n 3, records 1–4 | synthetic |

## Provenance

- 1.14d `Game.exe`: modes `0x0065FEC0` (jump table `0x00660084`,
  per-mode accessor `0x0065C730`, `0x0065C660`, `0x0065C6D0`), dispatcher
  `0x0065FD70`, wrapper `0x0065FE10`, function table `0x007462F8` (count
  37 at `0x00745B54`), value roll `0x0065E9E0`, add `0x0065EA50`, list
  `0x0065CBF0`, base reset `0x0065CCC0`, functions per the TSV, skills
  `reqlevel` `0x00644710`, `maxlvl` `0x004AA8B0`, socket fill
  `0x0055C2C0`, insertion `0x00562660`, runeword match `0x0062BED0`
  (disassembled; filler walk `0x0063B2C0`, `0x0063DFD0`, `0x0063DFA0`),
  activation `0x006600A0`, set bonuses `0x00660120`, set mask
  `0x0062A370`, craft list `0x00660240`, ethereal `0x0065E4D0`; state and
  flag tables `0x006EDB40`, `0x006EDB5C`, popcount `0x006EDA40`.
- D2MOO 1.10f `D2Common/src/Items/ItemMods.cpp` (property functions,
  `ITEMMODS_AssignProperty`, `ITEMMODS_UpdateRuneword`,
  `ITEMMODS_UpdateFullSetBoni`, `sub_6FD92CF0`) was the map; every
  function was re-read on 1.14d. Differences: 1.14d has function 36
  (D2MOO: 1.11+ only); 1.14d function 14 re-reads `param` when the roll
  is < 1; runeword `server` rows need the ladder flag in 1.14d.

## Open questions

1. No recording confirms any rule (request R1 in the session report).
2. Answered (2026-10-07, `disasm.py fn` on `0x0055C2C0`, `0x00663CC0`,
   `0x00663A20`, `0x00663B40`; tables `0x006EE424` = 165–170 and
   `0x006EE440` read from the binary): §13. The branch is not a socket
   filler: `0x0055C2C0` refreshes any item placed with an owner, and an
   item of quality 5 (set) takes `0x00663CC0(owner, item, 0, 0)`: the
   item's partial lists (states 165–169) are parked / unparked by the
   equipped-set mask per `add func`, and the owner gets (or reuses) one
   list of state 165–170 tagged with stat 71 = set id, refilled by §11.
   d2rs's no-op (`docs/handoff/impl-items.md`) differs whenever a set
   item is equipped.
3. Answered (2026-10-07, disassembly of the wrapper `0x0065FE10`, the
   dispatcher `0x0065FD70`, all 23 property functions' roll calls, the
   add `0x0065EA50` and `0x0065CBF0`): §4.2 "Register mapping". The
   choice stands as written (owner's list when an owner is given, else
   the item's). Owners per caller: none in modes 0–5 and 7, the player
   in §11, the socketed item in mode 6, whose item argument is the
   inserted filler (§10.2: runeword rolls draw on that filler's item
   seed).
4. §10.1 edge: whether the stale class-id slot can ever equal a rune's
   class id in 1.14d (it would let a socketed item take a runeword one
   rune longer than its sockets). Settle: a stack trace of the slot at
   `0x0062BFBA` for a 2-socket item of a 3-rune word, or accept d2rs's
   "no match".
   Partly answered (2026-10-07, frame layouts from the disassembly): the
   buffer is 6 dwords at ebp−0x2C of `0x0062BED0` (slot k at ebp−0x2C +
   4k; ebp−0x14 is the runes-table pointer, written after the walk), and
   the function has five callers: `0x0053379A` (legacy save reader
   `0x005335E0`), `0x00562802` and `0x006600A9` (insertion and
   activation, §10.2), `0x0056AD78` (character load `0x0056ACE0`),
   `0x0063087E` (bit-stream writer `0x0062FFF0`). For the insertion pair
   the stale slots are fixed by the calls just before them (the
   filler's mode set `0x00624690(filler, 6)`, whose mode change runs
   `0x00624390`):
   - `0x00562802`, slot c for c = 1 … 5: the filler pointer (saved ESI
     of `0x00624390`), the game pointer (saved EBX), the filler's class
     id (`0x00624390`'s local ebp−8, set from unit +4 and left as is by
     `0x00645270` for a unit without a transform state), the unit type
     4 (its local ebp−4), a stack address (its saved EBP).
   - `0x006600A9` (same caller depth, 0x1C bytes deeper): left by the
     first match's own frame and callees: saved EDI / ESI of
     `0x00562660` (filler, item pointers), pushed item pointers or type
     ids of the `0x00629BB0` tests, return and stack addresses.
   So only one stale slot can hold a rune's class id there: slot 3 of a
   3-filler item = the last filler's class id; it matches only a row
   whose 4th rune equals its 3rd. No complete row of the live
   `runes.txt` has that (rows with two equal adjacent runes: Sanctuary
   r18 r18 (runes 1–2), Bone r22 r22 (runes 2–3), Phoenix r26 r26
   (runes 1–2)), so on insertion 1.14d behaves as "no match" with 1.14d
   data. Still open: the three load / save / writer callers, whose slots
   hold whatever deeper frames of earlier calls left (for the load
   loops possibly the filler class ids of a previous item matched at
   the same depth); settle with a stack trace at `0x0062BFBA` on those
   paths, or keep d2rs's "no match" as a Ruleset choice. Struck for the
   binary (2026-10-07: stack contents of earlier frames); recording list
   `docs/handoff/pc2-rec-pc2-items.md` IT-10.
5. Answered (handoff `impl-items` OQ-P1): §5 rules 8 and 9 "**set**" is
   not §4.2. Functions 18 (`0x0065F870`) and 19 (`0x0065F6A0`) take the
   list through the same owner-or-item lookup as §4.2 (`0x0065CBF0`,
   created if missing; no list → return 0), then write the packed value
   with the plain list set `0x00627150` → `0x006270B0` (`sim/stat-lists.md`
   §5 rule 1) at `0x0065F947` / `0x0065F853`: no `valshift` (a value 0
   follows that set rule), no itemstatcost range test, no stat-58 rule.
6. Answered (handoff `impl-items` OQ-P2, `mutants-items-treasure` MT1):
   function 14 (`0x0065F590`) returns 0 and writes nothing (no flag
   0x800, no stat 194) in each of: no property record; the target not an
   item; `invwidth` × `invheight` = 0; and the final cap (after the max
   sockets `0x0062BC20`) < 1 while max(n, 1) ≥ it. Only the write path
   sets the flag (`0x006280D0`) and stat 194 on the item itself
   (`0x00627260`, unit set).
7. Answered (handoff `impl-items` OQ-P3): §10.1's exact form (step 4)
   is the rule: rune i is compared for i = 0 … while `rune`i+1 > 0, and
   a row with more runes than fillers reads the unwritten slot c (Open
   question 4); a row with fewer runes fails "matched ≥ c". So with the
   stale slot read as no class a row matches only with exactly c runes,
   as d2rs does.
8. Answered (handoff `impl-items` OQ-P4): §11's two dispatcher calls
   (`0x006601E1` for `pcode*`, `0x00660220` for `fcode*`, through the
   wrapper `0x0065FE10`) pass state = the caller's s (the routine's first
   stack argument) and **flags 0**, extra 0 (the pushes of the last two
   function arguments at `0x006601D2` / `0x006601D4` and `0x00660211` /
   `0x00660213`), so the bonus stats go into the owner's list (state s,
   flags 0).
