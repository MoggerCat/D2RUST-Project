# Spec: Items — Quality roll, quality dispatch, uniques, sets, superior, low quality

- **Status:** draft: every rule read from the 1.14d `Game.exe` code
  (addresses per rule, register arguments read from the disassembly); no
  recording of item creation yet (`items/generation.md` Open question 2);
  test vectors are synthetic.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::items::quality`
- **Related specs:** `items/generation.md` (the pipeline that calls the
  dispatch, §1 conventions: quality ids, format, type tests, flags,
  stats; §6.2 class skill mods; §7 sockets; §8 ethereal);
  `items/affixes.md` (magic, rare, crafted, tempered, charm and
  automagic routines); `items/properties.md` (property modes 1, 3, 4);
  `items/treasure.md` §6 (drop quality with magic find, and the
  itemratio row choice used here too); `sim/rng.md` §3, §5.3.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 41–55 |
| Inputs | 56–64 |
| Outputs / state changes | 65–71 |
| Rules | 72–73 |
|   1. Terms | 74–85 |
|   2. Drop quality (owned by `items/treasure.md` §6) | 86–91 |
|   3. Quality roll (`0x00556F60`, D2MOO `ITEMS_RollItemQuality`) | 92–112 |
|   4. Quality dispatch (`0x00557450`) | 113–148 |
|   5. Downgrade (`0x005572A0` normal, `0x00557320` superior, `0x00557380` magic, `0x005573F0` rare) | 149–165 |
|   6. Low quality (`0x005C2FB0` → `0x005C2D40`, format ≥ 1) | 166–179 |
|   7. Superior (`0x005C2AD0` → `0x005C2970`) | 180–198 |
|   8. Unique (`0x005566B0`) | 199–229 |
|   9. Set item (`0x005C2940` → `0x005C25C0`, format ≥ 1) | 230–243 |
| Constants & data dependencies | 244–254 |
| Randomness | 255–267 |
| Edge cases & original bugs | 268–286 |
| Test vectors | 287–299 |
| Provenance | 300–319 |
| Open questions | 320–326 |
<!-- /index -->

## Summary

Once the pipeline has written the base stats, the quality dispatch
(`0x00557450`) decides the item's quality: the request's quality if
given, else a roll on itemratio; then item-type and item-record flags
override it. It runs the routine for that quality; when the routine
fails, the item is cleared, its item seed rewound to the value saved
before the routine, and the next lower quality is tried, down to
normal. It then finishes: ethereal, sockets, automagic. This spec owns
the itemratio quality roll, the overrides, the downgrade chain and the
finishing order, and the unique, set, superior and low-quality routines
(including unique and set item selection and the unique-dropped
flags). The drop quality with magic find, chosen by the treasure walk
before creation, is owned by `items/treasure.md` §6.

## Inputs

| Name | Source |
|---|---|
| item, its base stats, item seed, start seed | `items/generation.md` §2–§4 |
| request: quality (+0x30), index (+0x40), force (+0x2C), ilvl (+0x0C), flags2 (+0x80) | `items/generation.md` Inputs |
| game: difficulty (+0x6D), ladder flags (+0x6A, +0x74), unique-dropped bits (+0x1B24) | game |
| tables | items, itemtypes, itemratio, uniqueitems, setitems, qualityitems, lowqualityitems |

## Outputs / state changes

Quality, file index, identified flag, durability changes, property
stats (through `items/properties.md`), the unique-dropped bit, the
request's quality field (rewritten by downgrades), item seed and start
seed. Result 1 (success) or 0 (the item is removed by the pipeline).

## Rules

### 1. Terms

- Quality ids, format, type tests, helpers and flags:
  `items/generation.md` §1.
- "Save" = read the item seed's current low word (`0x00650E50`).
  "Clear" (`0x00557250`) = prefix and suffix slots 0–2 := 0, rare prefix
  and rare suffix := 0, file index := −1.
- `D2` (dur, max dur): stat 72 := min(F × stat 72, 255) and stat 73 :=
  min(F × base stat 73, 255) for a factor F, applied only when the item
  has durability and format ≥ 1 (the same "has durability" helper,
  `items/generation.md` §1.3).

### 2. Drop quality (owned by `items/treasure.md` §6)

Treasure-class drops arrive with quality 1–7 already chosen by the drop
quality roll (`items/treasure.md` §6, dropping unit's seed). Requests
from other callers may carry 0.

### 3. Quality roll (`0x00556F60`, D2MOO `ITEMS_RollItemQuality`)

Item seed draws (`roll`, `sim/rng.md` §3).

1. Format ≥ 1 and request quality ≠ 0 → return request quality (no draw).
2. Ratio row: as `items/treasure.md` §6 step 3, but the version limit is
   0 for format 0 and 100 otherwise. No row, no item record → fatal.
3. Items `quest` ≠ 0 → return 2.
4. L := request ilvl. Format ≥ 1: if the item is type `misc` (52), L := 1;
   else L := max(L − items `level`, 1).
5. Format ≥ 1: for q in (unique, rare, set, magic, superior, normal)
   (order table `0x006E1138` = 7, 6, 5, 4, 3, 2) with columns (`Unique`,
   `UniqueDivisor`), (`Rare`, …), (`Set`, …), (`Magic`, …), (`HiQuality`,
   …), (`Normal`, …): c := base − L / divisor (signed, toward zero);
   c < 1 → return q without a draw; else roll(c) = 0 → return q.
   (Format 0 uses L unadjusted, no divisor for unique/rare/set and clamps
   c to ≥ 1; unspecified beyond this, `items/generation.md` §1.2.)
6. None hit: return 3 if request flags2 & 0x40, else 1.

The `*Min` columns are not used here.

### 4. Quality dispatch (`0x00557450`)

Arguments: game, item, request. Steps:

1. Item record missing → return 0. Auto affix := 0. Clear (§1) except
   the file index.
2. q := §3. If request quality ≠ 0, q := request quality. Set quality q.
3. Overrides, in order:
   1. itemtype `magic` ≠ 0: quest item → 7; else quality not 4–9 → 4.
   2. itemtype `rare` = 0 and quality 6 → 4.
   3. items `unique` ≠ 0 → 7.
   4. itemtype `normal` ≠ 0 → 2.
4. By the resulting quality (S = save, D(x) = downgrade to x, §5):

| Q | Before | Routine | On failure |
|---|---|---|---|
| 1 low | S | §6 | D(2) |
| 2 normal | — | `items/generation.md` §6.1 | — |
| 3 superior | file index := −1; S | §7 | D(2) |
| 4 magic | S | `items/affixes.md` §6 | D(3) → D(2) |
| 5 set | file index := −1; S | §9 | `D2`×2; D(4) → D(3) → D(2) |
| 6 rare | rare prefix/suffix := 0; S | `items/affixes.md` §7 | D(4) → D(3) → D(2) |
| 7 unique | file index := −1; S | §8 | `D2`×3; D(6) → D(4) → D(3) → D(2) |
| 8 crafted | rare prefix/suffix := 0; S | `items/affixes.md` §8 | D(2) |
| 9 tempered | rare prefix/suffix := 0; S | `items/affixes.md` §9 | D(2) |
| other | | | return 0 |

   "→" means: run the next downgrade only if the previous one's routine
   also failed. D(2) cannot fail.
5. **Finishing** (quality 1–9 only, else return 0), in this order:
   1. format ≥ 100 → ethereal roll (`items/generation.md` §8.1);
   2. quality 1, 2 or 3 → socket roll (`items/generation.md` §7.1);
   3. format ≥ 100 and quality not 5 and not 7 (jump table `0x00557A58`)
      and items `auto prefix` ≠ 0 → automagic (`items/affixes.md` §11).
6. Return 1.

### 5. Downgrade (`0x005572A0` normal, `0x00557320` superior, `0x00557380` magic, `0x005573F0` rare)

D(x) with the saved value s:

1. Clear (§1).
2. Start seed := s; item seed := `{s, 666}`.
3. Run §3 and discard the result (it draws only when format is 0 or the
   request quality is 0 — for a treasure-class drop nothing).
4. Set quality x; request quality := x.
5. For x ≠ 2: s := save (the value after step 3), run x's routine
   (§7 for superior, `items/affixes.md` §6 / §7 for magic / rare) and
   return its result. For x = 2: run the normal routine
   (`items/generation.md` §6.1), success.

Effect: every retry replays the item seed from the same point, so a
downgraded item's draws start where the failed routine's started.

### 6. Low quality (`0x005C2FB0` → `0x005C2D40`, format ≥ 1)

1. File index := roll(lowqualityitems count) (item seed).
2. If the item has durability: m := items `durability` × 33 / 100
   (unsigned), < 1 → 1; stat 72 := roll(m >> 1) **on the unit seed** +
   (m >> 1), 0 → 1; stat 73 := m.
3. Weapon: stat 22 := max(stat 22 × 75 / 100, 2); 21 := max(… , 1); 24 :=
   max(…, 2); 23 := max(…, 1) (signed); if throwable: 159 := max(stat 159
   × 75 / 100, **2**), 160 := max(stat 160 × 75 / 100, **1**). Result 1.
4. Else any armor: stat 31 := max(stat 31 × 75 / 100, 1). Result 1.
5. Else result 0.
6. Class skill mods (`items/generation.md` §6.2) run in all three cases.
   Return the result.

### 7. Superior (`0x005C2AD0` → `0x005C2970`)

1. n := qualityitems count (8 in 1.14d); throwable or items
   `nodurability` ≠ 0 → n := 4 (the first four rows carry no durability
   mods).
2. Loop: draw r := roll(n) (item seed) until r is not marked tried
   (each redraw is a draw). Row r missing → return 0. If the row fits the
   item (§7.1): file index := r; properties, mode 1, of row r
   (`items/properties.md` §2); class skill mods; return 1. Else mark r
   tried; all n tried → return 0.

#### 7.1 Row fits (`0x0065E7D0`)

T := primary type. True if (`weapon` ≠ 0, the item is type `weap`, and T
∉ {staf 26, bow 27, xbow 35, scep 24, wand 25}); or (`armor` ≠ 0, the
item is type `armo`, and T ∉ {shie 2, boot 15, glov 16, belt 19}); or one
of: T = 2 and `shield`; 24 and `scepter`; 25 and `wand`; 26 and `staff`;
27 or 35 and `bow`; 15 and `boots`; 16 and `gloves`; 19 and `belt`.

### 8. Unique (`0x005566B0`)

1. Format 0 only: durability × 5 (not specified further).
2. **Forced request:** idx := request index; outside the uniqueitems
   table → return 0. File index := idx. If the row's `code` ≠ the item
   code → return 1 without properties. Else clear identified, unique
   properties (mode 3), return 1.
3. **Not forced, format ≥ 1:** candidates are the uniqueitems rows i, in
   order, with: `version` < 100 or format ≥ 100; `enabled`; `code` = the
   item's code; ladder: game +0x6A ≠ 0 or game +0x74 ≠ 0 or not
   `ladder`; `lvl` ≤ item level. Each gets weight max(`rarity`, 1) and a
   start = the sum of earlier weights. If request index ≠ 0 and index −
   1 = i, i is the preferred row.
4. No candidate: items `unique` ≠ 0 → return 1 (stays unique, file index
   −1); else file index := −1, return 0.
5. No preferred row: r := roll(total weight) (`roll_range(0, n)`, item
   seed); pick the last candidate whose start ≤ r (scan from the second
   candidate while r ≥ its start).
6. Accept when items `quest` ≠ 0, or the game is none, or (idx ≤ 4096 and
   the game's unique-dropped bit idx is clear). Accepted: row code ≠ item
   code → return 0; file index := idx; mark dropped (§8.1); marked →
   clear identified, unique properties (mode 3), return 1. Not marked or
   not accepted → file index := −1, return 0.

#### 8.1 Unique-dropped bits (`0x00556530`, `0x005564A0`)

The game holds 4,097 bits at +0x1B24 (bit i = word i >> 5, mask 1 << (i &
31)). Marking idx: row `nolimit` set → success without marking.
Otherwise items `quest` = 0 and bit set → failure; idx > 4096 → failure;
else set the bit, success. A test on idx > 4096 reads "dropped".

### 9. Set item (`0x005C2940` → `0x005C25C0`, format ≥ 1)

1. Candidates: setitems rows i with: version (+0x22, filled by
   `data/fixups.md` §6) < 100 or format ≥ 100; `lvl` ≤ item level; `item`
   = the item's code; `set` ≠ 29 or request flags2 & 0x01. Weight
   `rarity`, 0 → 1. Request index − 1 = i (index ≠ 0) → preferred.
2. Preferred → pick it (no draw). Else total = 0 → return 0; else r :=
   roll(total) (item seed); walk candidates subtracting weights until r <
   weight.
3. File index := row; clear identified; set properties (mode 4,
   `items/properties.md` §8); return 1.

No unique-style dropped bits exist for sets.

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| itemratio order | 7, 6, 5, 4, 3, 2 | `0x006E1138` |
| low-quality factors | durability × 33/100, damage and defense × 75/100 | §6 |
| superior rows without durability mods | first 4 | §7 |
| downgrade durability | × 2 (set), × 3 (unique) | §4 |
| unique-dropped bits | 4,097 at game +0x1B24 | §8.1 |
| hellbovine set | 29 | §9 |

## Randomness

Item seed unless noted:

| Routine | Draws in order |
|---|---|
| §3 | ≤ 6 × roll(c) |
| §6 | roll(lowquality count); roll(m >> 1) on the **unit seed**; class skill mods |
| §7 | roll(n) per try (repeats on tried rows); then the properties' draws (mode 1); class skill mods |
| §8 | roll(total rarity) unless preferred; unique properties' draws |
| §9 | roll(total rarity) unless preferred; set properties' draws |
| §5 | rewinds the item seed; §3 replay |

## Edge cases & original bugs

1. A failed unique becomes rare with triple durability; a failed set
   becomes magic with double durability (`D2` runs before the downgrade).
2. Low-quality throwing damage clamps the min to ≥ 2 and the max to ≥ 1
   (swapped bounds, reproduced).
3. Forced unique on a mismatched base: unique quality, file index set,
   no properties.
4. Unique row rarity is read as a 32-bit value at +0x30 (u16 `rarity` plus
   the two unwritten bytes, 0 in 1.14d).
5. The superior tried-array has 10 entries; more than 10 qualityitems
   rows would overflow (1.14d has 8).
6. A not-forced unique with no candidates stays unique only when items
   `unique` is set (quest-style uniques); otherwise it downgrades.
7. A treasure drop with magic find ≤ −100 can carry quality 1–3 for a
   `magic` itemtype (`items/treasure.md` edge case 9); §4 step 3.1 makes
   it 4 (re-read at `0x005574E6`–`0x00557513`: `magic` type, quest test
   `0x00628CD0` → 7, else "magic or better" `0x0062A0F0` false → 4).

## Test vectors

Synthetic, from the rules (`sim/rng.md` generator):

| Input | Expected | Source |
|---|---|---|
| §3, format 101, request quality 6 | 6, no draw | synthetic |
| §3, format 101, quality 0, `misc` item, row Magic 34/3 …: L = 1 | c(unique) = 400 − 1 = 399 → first draw roll(399) | synthetic |
| §5 downgrade, saved s = 0xDEADBEEF | item seed `{3735928559, 666}`, start seed 0xDEADBEEF | synthetic |
| §8 weights: candidates rarity 1, 0, 3 | starts 0, 1, 2; total 5; r = 1 → 2nd, r = 4 → 3rd | synthetic |
| §8.1 idx 4097 | not markable, unique fails (unless `nolimit`) | synthetic |
| §7.1 row (weapon=1), item `axe` | fits; item `staf` with only `weapon` → no | synthetic |

## Provenance

- 1.14d `Game.exe`: dispatch `0x00557450` (jump table `0x00557A50`/
  `0x00557A58`); quality roll `0x00556F60` (order table `0x006E1138`);
  downgrade helpers `0x005572A0`, `0x00557320`, `0x00557380`,
  `0x005573F0`, clear `0x00557250`; low quality `0x005C2D40` (unit-seed
  draw at `0x005C2DE6`); superior `0x005C2970`, `0x0065E7D0`; unique
  `0x005566B0`, `0x00556530`, `0x005564A0`; set `0x005C25C0`; itemratio
  row `0x00637910`.
- D2MOO 1.10f (`ItemMode.cpp sub_6FC4C5F0`, `Items.cpp
  ITEMS_RollItemQuality`, `ItemsMagic.cpp sub_6FC542C0`, `sub_6FC54690`,
  `sub_6FC549F0`) gave the map. Differences in 1.14d: the downgrade chain
  is factored into four helpers with the same effect; low-quality
  durability is ×33/100 (D2MOO: /3) for format ≥ 1; format-1+ uniques get
  no ×5 durability (D2MOO's format-0 rule only); expansion quality
  chances with c < 1 return without a draw (same result as `roll`).
- rng.md §5.3 states the re-init uses the start seed; 1.14d uses the low
  word saved before the failed routine (§5), which is also written as the
  new start seed.

## Open questions

1. No recording confirms the dispatch or any routine (request R1 in the
   session report; `items/generation.md` Open question 2).
2. Which non-treasure callers pass request quality 0 (vendors, quests,
   cube) and so draw §3: owned by those specs; check when written.
