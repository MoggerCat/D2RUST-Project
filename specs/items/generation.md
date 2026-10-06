# Spec: Items — Creation pipeline, base stats, sockets, ethereal

- **Status:** draft: every rule read from the 1.14d `Game.exe` code
  (addresses per rule, register arguments read from the disassembly); no
  recording of item creation exists yet (`traces/raw/*rng*` holds no draw
  from an item site), so no rule is confirmed on the running game; the
  test vectors are synthetic (computed from the rules and `sim/rng.md`).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::items::create` (pipeline, base stats,
  sockets, ethereal, class skill mods)
- **Related specs:** `items/quality.md` (quality roll, dispatch, the
  unique/set/superior/low-quality routines, the finishing steps that call
  §7 and §8 here); `items/affixes.md` (magic/rare/crafted affixes,
  charms, automagic); `items/properties.md` (properties.txt functions,
  how affix and unique properties become stats); `sim/rng.md` (§5.3 the
  unit and item seeds, §3 draw helpers); `sim/units.md` (unit
  allocation, which derives the seeds); `sim/stats.md`,
  `sim/stat-lists.md` (what "set stat" and "stat list" mean);
  `sim/tick.md` §5 (timer event 3); `data/runtime-maps.md` §2 (item type
  equivalence), §5 (class skill lists); `data/fixups.md` §5 (gem
  offsets); `items/treasure.md` (`claude/phase3-treasure`): §5.7 and §7
  (what a treasure-class drop puts in the request: item, quality,
  index, ilvl, position, flags2), §6 (the drop quality roll with magic
  find, on the dropping unit's seed), §8 (gold: base roll and
  multipliers).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 51–65 |
| Inputs | 66–104 |
| Outputs / state changes | 105–117 |
| Rules | 118–119 |
|   1. Conventions | 120–223 |
|   2. Seeds | 224–242 |
|   3. Pipeline (`0x00558D90`, D2MOO `D2GAME_CreateItemEx`) | 243–285 |
|   4. Base stats (`0x00557AB0`, D2MOO `D2GAME_InitItemStats`) | 286–333 |
|   5. Special item kinds | 334–344 |
|   6. Normal quality and class skill mods | 345–391 |
|   7. Sockets | 392–423 |
|   8. Ethereal | 424–444 |
|   9. Forced requests, ears, names, timers | 445–470 |
| Constants & data dependencies | 471–493 |
| Randomness | 494–512 |
| Edge cases & original bugs | 513–527 |
| Test vectors | 528–543 |
| Provenance | 544–565 |
| Open questions | 566–578 |
<!-- /index -->

## Summary

An item is created from a drop request: an item record index, an item
level, an optional quality and optional overrides. The pipeline
allocates the item unit (which derives two seeds from the game seed),
writes the base stats (durability, defense, damage, quantity, gold) from
the **unit seed**, then hands over to the quality dispatch
(`items/quality.md`), which draws from the **item seed** and finishes
with ethereal, sockets and the automagic affix. Forced requests (loading
a known item, cube outputs with fixed seeds) replace both seeds and
overwrite flags and counts after the rolls. This spec owns the request,
the order of the pipeline, the base-stat rolls, the class skill mods
(staffmods), the generation-time socket and ethereal rolls, and the
generation flags.

## Inputs

The drop request (D2MOO `D2ItemDropStrc`, 0x84 bytes in 1.14d; offsets
given for cross-checks with recordings). This table owns the layout and
what each field does during creation; the values a treasure-class drop
writes are owned by `items/treasure.md` §5.7, §7; other callers (vendors,
cube, quests, NPC gifts, save loading) are owned by their specs:

| Field | Off | Type | Meaning |
|---|---|---|---|
| unit | 0x00 | unit ref | source unit: the dropper, the ear's victim, the personalizing player; may be none |
| game | 0x08 | game ref | |
| ilvl | 0x0C | i32 | item level; < 1 becomes 1 and is written back (§3 step 4) |
| item | 0x14 | i32 | items combined index (`data/loading.md` §9) |
| spawn mode | 0x18 | i32 | unit mode at allocation (3 ground, 4 inventory; `sim/units.md`) |
| x, y | 0x1C, 0x20 | i32 | position |
| room | 0x24 | room ref | |
| init flags | 0x28 | u16 | allocation flags (`sim/units.md`) |
| format | 0x2A | u16 | item format: the game's value (§1.2) |
| force | 0x2C | bool | forced request (§3 step 3, §9) |
| quality | 0x30 | i32 | 0 = roll it; else the quality to use (`items/quality.md` §3, §4) |
| quantity | 0x34 | i32 | forced quantity / gold |
| min dur, max dur | 0x38, 0x3C | i32 | forced durability |
| index | 0x40 | i32 | unique/set preference: record + 1, 0 = none (forced: the record itself); ear/body-part class |
| flags1 | 0x44 | u32 | forced item flags (§1.4); bit 0x800 forces sockets, 0x400000 forces ethereal on random items too |
| seed | 0x48 | u32 | forced unit seed low word |
| item seed | 0x4C | u32 | forced item seed low word (= start seed) |
| ear level | 0x50 | i32 | forced ear level |
| quantity override | 0x54 | i32 | > 0 replaces rolled quantities (§4) |
| name | 0x58 | char[16] | forced ear / personalized name |
| prefix[3], suffix[3] | 0x68, 0x74 | i32 | preferred magic affixes: 1-based index within the prefix (suffix) part, 0 = none, < 0 = suppress (`items/affixes.md` §6) |
| flags2 | 0x80 | u32 | request flags, §1.5 |

Other inputs: the game's difficulty (game +0x6D: 0 normal, 1 nightmare,
2 hell), expansion flag (game +0x70), ladder flags (game +0x6A, +0x74),
the game seed, and the excel tables (items, itemtypes, itemratio,
affix, quality, unique, set tables; `data/fields.tsv` names every
column used below by its name).

## Outputs / state changes

- A new item unit with: item record, format, item level, quality,
  file index (unique/set/superior/low-quality/book index or −1), affix
  slots (`items/affixes.md` §1), flags, base stats and property stats,
  start seed, the two seed states after the last draw.
- Steps of the game seed (two per item, `sim/rng.md` §5.2–§5.3), of the
  item's unit seed and of its item seed.
- For named uniques: the game's unique-dropped bit (`items/quality.md` §8).
- Possibly a scheduled timer event 3 (§9 step 6).
- Failure: no item (the allocated unit is removed); the seed steps
  already made stay made.

## Rules

### 1. Conventions

#### 1.1 Quality ids

| Id | Quality | Id | Quality |
|---|---|---|---|
| 1 | low (inferior) | 6 | rare |
| 2 | normal | 7 | unique |
| 3 | superior | 8 | crafted |
| 4 | magic | 9 | tempered |
| 5 | set | 0 | none ("roll it" in a request) |

#### 1.2 Item format

The format is a u16 on the item. Generated items take the game's value
(game +0x78), set at game creation (`0x00530930` / `0x00530BF0`):
**101 in an expansion game, 2 in a classic game**. Rules below test
"format ≥ 1" (all generated items), "format ≥ 100" (expansion) and
"format = 0". Format 0 occurs only on items decoded from old saves; the
format-0 branches of the generation code (D2MOO's "Old" functions:
`0x005C12F0`, `0x005C0D70`, `0x005C19A0`, `0x005C1E80`, `0x005C2740`,
`0x005C2AF0`, `0x00556D80`, the old property table `0x00745B58`) are not
specified (open question 1).

#### 1.3 Item record and type tests

- The item record is the combined items row (weapons, armor, misc).
  Columns are read by name (`data/fields.tsv`, table `weapons`; the three
  share one layout).
- "Item is type T" (D2MOO `ITEMS_CheckItemTypeId`, `0x00629BB0`): true if
  the item's `type` row is equivalent to T or, when `type2` > 0, if
  `type2` is equivalent to T (`data/runtime-maps.md` §2). Type numbers
  are itemtypes row indices (`weap` 45, `armo` 50, `misc` 52, `tors` 3,
  `helm` 37, `char` 13, `body` 40, `play` 7, `scro` 22, `book` 18, `gold`
  4, `elix` 11, `jewl` 58, `gem` 20, `rune` 74).
- "Primary type" is the `type` column alone (`0x0062B400`).
- The itemtypes row used for type flags (`magic`, `rare`, `normal`,
  `throwable`, `quiver`, `varinvgfx`, `staffmods`, `class`, `maxsock*`)
  is the primary type's row.
- Helpers used throughout:

| Name | 1.14d | Value |
|---|---|---|
| stackable | `0x006289F0` | items `stackable` ≠ 0 |
| throwable | `0x0062BA80` | itemtype `throwable` ≠ 0 |
| quiver | `0x0062E740` | itemtype `quiver` ≠ 0 |
| has durability | `0x00629930` | items `nodurability` = 0, `durability` ≠ 0, the item has a stat list, and stat 152 (`item_indesctructible`) < 1 |
| socketable | `0x00629900` | items `hasinv` ≠ 0 |
| quest item | `0x00628CD0` | items `quest` ≠ 0 |
| min stack | `0x006296F0` | items `minstack` |
| total max stack | `0x006295B0` | items `maxstack` + stat 254 (`item_extra_stack`), capped at 511 |
| spawn stack | `0x00629660` | items `spawnstack` |
| item level | `0x006281E0` | the stored ilvl; a stored value < 1 is set to 1 first |
| quality level (qlvl) | `0x00628740` | items `level` |
| max sockets | `0x0062BC20` | §7.2 |
| is magic or better | `0x0062A0F0` | quality 4 – 9 |

#### 1.4 Item flags (D2MOO names)

| Bit | Name | Set by |
|---|---|---|
| 0x10 | identified | cleared by every affix/unique/set/superior/rare success; set for quest-difficulty items (§5) and by callers |
| 0x100 | broken | forced (§9) |
| 0x800 | socketed | §7 |
| 0x1000 | nosell | forced |
| 0x2000 | instore | every non-forced creation (§3 step 5) |
| 0x8000 | named | ears (§9) |
| 0x10000 | is ear | ears (§6) |
| 0x20000 | start item | forced |
| 0x80000 | init | every creation |
| 0x400000 | ethereal | §8 |
| 0x1000000 | personalized | (read: §9 step 5) |
| 0x4000000 | runeword | `items/properties.md` §10 |

#### 1.5 Request flags (flags2)

| Bit | D2MOO name | Effect |
|---|---|---|
| 0x01 | hellbovine | set items of set 29 may be picked (`items/quality.md` §9) |
| 0x02 | never ethereal | §8 |
| 0x04 | always ethereal | §8 |
| 0x08 | no sockets | §7 |
| 0x10 | always sockets | §7 |
| 0x20 | staffmods use ilvl | §6.2 |
| 0x40 | superior | the quality roll's fallback is superior, not low (`items/quality.md` §3) |

#### 1.6 Stats written here

| Id | Name | Id | Name |
|---|---|---|---|
| 14 | gold | 68 | attackrate |
| 20 | toblock | 70 | quantity |
| 21 | mindamage | 71 | value (elixirs) |
| 22 | maxdamage | 72 | durability |
| 23 | secondary_mindamage | 73 | maxdurability |
| 24 | secondary_maxdamage | 107 | item_singleskill (layer = skill) |
| 31 | armorclass | 159 | item_throw_mindamage |
| 67 | velocitypercent | 160 | item_throw_maxdamage |
| 194 | item_numsockets | 356 | questitemdifficulty |

"Set stat" writes the item's base stat (D2MOO `STATLIST_SetUnitStat`,
`0x00627260`) unless a rule names a stat list; stat-list semantics are
owned by `sim/stat-lists.md`.

### 2. Seeds

1. Allocation (`0x00555230`, owned by `sim/units.md`) derives, in this
   order, the **unit seed** (unit +0x20; one game-seed step, `0x00552DF0`)
   and the **item seed** (item data +0x04; reset to `{1, 666}`, then one
   game-seed step: the start seed := lo′, item seed := `{lo′, 666}`,
   `0x00552E90`) (`sim/rng.md` §5.3).
2. Forced or seeded requests (§3 step 3) then overwrite both.
3. **Unit seed draws:** gold, quantities, durability, defense, variable
   graphics (§4, §5), and the durability of low-quality items
   (`items/quality.md` §6). Nothing else in this pipeline draws from it.
4. **Item seed draws:** everything else: the quality roll, affixes,
   unique/set/superior/low-quality picks, class skill mods, the socket and
   ethereal rolls, property values, elixirs. The two sequences are
   independent; only the order within each matters.
5. The start seed (item data +0x10) is used without drawing by the
   socket count (§7 step 7) and is rewritten by the quality downgrade
   chain (`items/quality.md` §5).

### 3. Pipeline (`0x00558D90`, D2MOO `D2GAME_CreateItemEx`)

Arguments: game, request, "use seed" (true when the caller supplies
seeds).

1. Classic game (game +0x70 = 0): fail if the item record is missing or
   its `version` ≥ 100.
2. Fail if `item` < 0 or ≥ the items count. Allocate the item unit
   (mode, position, room, init flags from the request; §2.1). Set flag
   0x80000 (init). Set the format from the request.
3. If "use seed" or `force`: unit seed := `{seed, 666}` and the unit's
   init seed := `seed`; start seed := `item seed`; item seed :=
   `{item seed, 666}`.
4. `ilvl` < 1 → `ilvl` := 1 (in the request). Store it as the item level.
5. Not forced → set flag 0x2000. Set the inventory page to none (0xFF).
6. **Base stats and quality:** §4 with "quest" = true (it calls the
   quality dispatch). Failure → remove the unit, fail.
7. Forced → §9 steps 1–4.
8. Primary type `play` (player body part) → §9 step 5.
9. Flag 0x1000000 (personalized) set → §9 step 5 (name only).
10. §9 step 6 (replenish timers). Return the item.

**Simple creation** (`0x00559CE0`; vendors, quest rewards, inventory
gifts): ECX source unit, EDX item index; stack game, spawn mode,
quality, no-sockets, never-ethereal, level, use seed, seed, item seed
(`ret 0x24`). Builds a zeroed request: unit, game, item, spawn mode,
quality, seed, item seed as given; x, y, room 0; init flags 1; format :=
the game's (+0x78); flags2 := 0x08 when no-sockets ≠ 0, | 0x02 when
never-ethereal ≠ 0; ilvl := level, where level ≤ 0 becomes 1 at entry
(`0x00559D08`). The code after that holds a unit-based default for
level −1 (the level default below, inlined), but
−1 has already become 1, so it never runs: a caller that wants the
default computes it first. Then the pipeline (§3) with "use seed"; on
success the item gets flag 0x10 (identified, `0x006280D0`).

**Level default** (`0x00558200`, ECX unit, EDX level id): monster →
its `level` total (stat 12); player → its base `level`; any other unit
→ the area level (`0x0061DCA0`, `items/treasure.md` §4 step 2) of the
level of the unit's room (`0x00620BB0`, `0x0061A1B0`), with the unit's
game difficulty and expansion; no unit → EDX = 0 gives 1, EDX ≠ 0 is
fatal (the game lookup `0x00554010` asserts on the null unit). A result
< 1 becomes 1.

### 4. Base stats (`0x00557AB0`, D2MOO `D2GAME_InitItemStats`)

Arguments: game, item, request (may be none), "quest" (true from §3).
"Draw r(n)" below is `roll(n)` on the **unit seed** (`sim/rng.md` §3:
n < 1 draws nothing and gives 0). Branches are exclusive, tested in
order:

1. **Gold** (primary type `gold`): stat 14 := the base amount, one draw
   r(5 × ilvl) (owned by `items/treasure.md` §8 step 1; without a request
   the amount is 1 and nothing is drawn).
2. **Quiver types** (itemtype `quiver` ≠ 0): q := r(total max stack −
   min stack) + min stack; q < 1 → 1; then override > 0 → q := override.
   Set stat 70 := q.
3. Otherwise the item record must exist (else fatal error 0x686).
   1. **Any armor** (type `armo`):
      1. set stat 20 := items `block`;
      2. set stat 67 := −items `speed`;
      3. d := items `durability` (u8); stat 72 := min(r(d >> 1) + (d >> 1), 255);
      4. stat 73 := d;
      5. defense (`0x00556360`): stat 31 := `minac` + r(`maxac` − `minac`
         + 1) (drawn as `roll_range(minac, n)`); a result > `maxac`
         (unsigned) is a fatal error.
   2. **Weapon** (type `weap`):
      1. if stackable: q := r(total max stack − min stack) + min stack;
         override > 0 → q := override; q = 0 → 1; is-magic-or-better →
         1 (normally false here: quality is not set yet); set stat 70 := q;
      2. durability exactly as 3.1.3–3.1.4;
      3. damage (`0x005563D0`), each only when the column ≠ 0, in this
         order: stat 22 := `maxdam`; 21 := `mindam`; 23 := `2handmindam`;
         24 := `2handmaxdam`; if `maxmisdam` ≠ 0: 159 := `minmisdam`,
         160 := `maxmisdam`;
      4. set stat 68 := −items `speed`.
   3. **Other**: if stackable: lo := min stack; hi := spawn stack, but if
      spawn stack < lo or = 0 then hi := max(lo, total max stack); q :=
      r(hi − lo) + lo; override > 0 → q := override; is-magic-or-better
      or q = 0 (unsigned test q < 1) → 1; set stat 70 := q. Then, primary
      type `elix` → elixir (§5.2).
4. **Variable graphics:** itemtype `varinvgfx` = v ≠ 0 → graphics index
   := r(v) (`roll_range(0, v)`).
5. If "quest" and a request is given: result := quality dispatch
   (`items/quality.md` §4, item seed). Then if items `quest` ≠ 0 and
   `questdiffcheck` ≠ 0: §5.3. Return the dispatch result (1 when not
   run).

Quirks reproduced: weapon and misc stack rolls use n = hi − lo, so the
top value (`maxstack`) is never rolled; durability uses d >> 1 for both
the range and the offset, so odd d never rolls d.

### 5. Special item kinds

1. **Body parts and ears** are handled by the normal-quality routine (§6.1).
2. **Elixirs** (`0x0065E8E0`, item seed): k := roll(6) on the table
   {0, 1, 2, 3, 9, 7} (`0x0074638C`, count `0x007463A4`); file index := that
   entry; entry 9 or 7 → one more step, stat 71 := ((lo′ & 3) + 1) × 256;
   else stat 71 := 1.
3. **Quest-difficulty items** (items `quest` and `questdiffcheck` both ≠
   0): find the item's stat list with flag 0x40 (create it if missing),
   set stat 356 := game difficulty there; set flag 0x10 (identified).

### 6. Normal quality and class skill mods

#### 6.1 Normal-quality routine (`0x00556E80`, format ≥ 1)

Run by the quality dispatch for quality 2 (`items/quality.md` §4). Tests
are independent and run in this order:

1. type `char` → charm affixes (`items/affixes.md` §10);
2. type `body` → file index := the request unit's class id if a request
   unit is given, else request `index`;
3. type `play` → the same, plus flag 0x10000;
4. type `scro` → suffix slot 0 := the books row whose `scrollspellcode`
   equals the item code (`0x005C2540`; no row → the books count);
5. type `book` → suffix slot 0 := the row whose `bookspellcode` equals it;
6. class skill mods (§6.2).

No draw except in 1 and 6.

#### 6.2 Class skill mods (staffmods; `0x005C1260`, `0x005C0F90`)

Run after normal, superior, low-quality, magic, rare and crafted
success (the callers are listed in `items/quality.md` and
`items/affixes.md`). Item seed draws; "pct" below is one step, lo′ mod
100.

1. c := itemtype `staffmods` (`0x0062BBD0`); c ≥ 7 → stop. The class's
   skill count (`data/runtime-maps.md` §5) must be > 0, else stop.
   first := the class's first skill id (list entry 0, `0x006460F0`).
2. ilvl := request `ilvl`; bonus := request `ilvl` if flags2 & 0x20, else 0.
3. **Count:** v := pct + bonus: v > 90 → 3; v > 70 → 2; v > 30 → 1;
   else 1 if bonus ≠ 0, else stop (no mods).
4. **Base tier:** ilvl > 36 and format ≥ 100 → 5; else ilvl > 24 → 4;
   > 18 → 3; > 11 → 2; else 1.
5. For each of the count mods, keeping a list of chosen skills:
   1. pct: > 80 → tier+1; > 30 → tier; > 10 → tier−1; else tier−2;
      < 1 → 1; quality 1 (low) and > 3 → 4.
   2. Up to 6 tries: one step, skill := first + 5 × (t − 1) + (lo′ mod
      5). Accept when the skills row is missing, its `itypea1` < 1, or the
      item is type `itypea1`, **and** the skill is not already chosen.
      After 6 rejected tries the last tried skill is used anyway (and not
      added to the chosen list).
   3. Value: if format < 100 or quality ≠ 1: v := pct + bonus / 2 (signed
      /2): ≥ 90 → 3; ≥ 60 → 2; else 1. Expansion low-quality items get 1
      without a draw.
   4. In the item's stat list with flag 0x40 (created if missing): **set**
      stat 107 layer skill := value. A repeated skill overwrites.

### 7. Sockets

#### 7.1 Generation roll (`0x00556B60`)

Run by the finishing steps for qualities 1–3 (`items/quality.md` §4.5).
Arguments: game, item, request.

1. Stop unless quality ≥ 2, socketable, not stackable, and max sockets
   (§7.2) m ≠ 0.
2. Cap by difficulty: normal 3, nightmare 4, hell 6 (m := min(m, cap)).
3. Classic format (< 100) items of type `tors` (body armor) stop here (no
   draw).
4. Draw: p := roll(100) (item seed).
5. flags2 & 0x08 (no sockets) → stop.
6. flags2 & 0x10 (always sockets) → p := 0.
7. If flags1 & 0x800 or p < 33: set flag 0x800; n := (start seed mod m)
   + 1 (unsigned; no draw); apply the socket count n (§7.3).

#### 7.2 Max sockets (`0x0062BC20`)

min(items `gemsockets`, itemtype `maxsock1` if ilvl ≤ 25, `maxsock25` if
ilvl ≤ 40, else `maxsock40`).

#### 7.3 Socket count (`0x0062BCB0`)

Arguments: item, requested n. cap := min(`invwidth` × `invheight`, 6);
cap = 0 → nothing. By quality: magic → min(cap, 4); rare → min(cap, 2);
crafted, tempered → min(cap, 3); set, unique → if the item's stat 194 is
< 1, cap := 1, else n := stat 194 (the current count replaces the
request); others unchanged. cap := min(cap, max sockets). Result := min(max(n,
1), cap); cap < 1 → nothing. Set flag 0x800 and set stat 194 := result.

### 8. Ethereal

#### 8.1 Roll (`0x00556CA0`)

Run by the finishing steps when format ≥ 100 (`items/quality.md` §4.5).

1. Stop if flags2 & 0x02; or the item is neither type `weap` nor `armo`;
   or it has no durability; or quality is 1 (low) or 5 (set); or it is a
   quest item.
2. Draw p := roll(100) (item seed).
3. If flags2 & 0x04, or flags1 & 0x400000, or p < 5: apply (§8.2); then
   if it still has durability: stat 73 := base stat 73 / 2 + 1; stat 72
   := stat 73.

#### 8.2 Apply (`0x0065E4D0`, D2MOO `ITEMMODS_ApplyEthereality`)

Set flag 0x400000. Weapon (type `weap`): each of stats 21, 22, 23, 24,
159, 160 := base × 3 / 2 (signed). Otherwise stat 31 := base × 3 / 2. No
draw. Also used by property function 23 and craft lists
(`items/properties.md`).

### 9. Forced requests, ears, names, timers

1. Forced (§3 step 7): flag 0x10 := flags1 & 0x10; flag 0x1000 := flags1
   & 0x1000.
2. Format 0 only: forced socket count (not specified, §1.2). Then flag
   0x800 := flags1 & 0x800; 0x100 := flags1 & 0x100; 0x20000 := flags1 &
   0x20000.
3. Quantity: primary type `gold` → gold := request quantity (through the
   gold setter `0x00530EA0`); else set stat 70 := request quantity.
4. max dur := min(request max dur, 255) (unsigned); min dur := min(min dur,
   max dur) (unsigned), both written back; stat 72 := min dur; stat 73 :=
   max dur.
5. Ears (primary type `play`): forced → name := request name, ear level
   := request ear level, flag 0x8000 := flags1 & 0x8000. Not forced → the
   request unit must be a player with player data (else fail and remove
   the item); name := its name; ear level := its stat 12 (level);
   flag 0x8000 := the client's hardcore flag (when a client exists).
   Personalized items (flag 0x1000000): name := request name when forced,
   else the request unit's player name (no player data → fatal error).
6. Replenish timers (`0x00558530`, `0x00558580`): for stat 252
   (`item_replenish_durability`), then stat 253
   (`item_replenish_quantity`): value v ≠ 0 and no event 3 scheduled on the
   item → schedule event 3 at game frame + 2500 / v + 1 (signed
   division) (`sim/tick.md` §5). At creation both stats are normally 0;
   socketing re-runs this (`items/properties.md` §10).

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| socket chance | p < 33 of 100 | §7.1 |
| difficulty socket caps | 3 / 4 / 6 | §7.1 |
| socket count cap | min(w × h, 6) | §7.3 |
| ethereal chance | p < 5 of 100 | §8.1 |
| ethereal factor | × 3 / 2 | §8.2 |
| staffmods count thresholds | > 90, > 70, > 30 | §6.2 |
| staffmods tier thresholds | ilvl > 36 (exp.), > 24, > 18, > 11 | §6.2 |
| staffmods value thresholds | ≥ 90, ≥ 60 | §6.2 |
| elixir table | {0, 1, 2, 3, 9, 7} | `0x0074638C` |
| replenish period | 2500 / v frames | §9 |

Columns: items `version`, `type`, `type2`, `level`, `block`, `speed`,
`durability`, `nodurability`, `minac`, `maxac`, `mindam`, `maxdam`,
`2handmindam`, `2handmaxdam`, `minmisdam`, `maxmisdam`, `stackable`,
`minstack`, `maxstack`, `spawnstack`, `hasinv`, `gemsockets`,
`invwidth`, `invheight`, `quest`, `questdiffcheck`; itemtypes `quiver`,
`throwable`, `varinvgfx`, `staffmods`, `maxsock1/25/40`; skills
`itypea1`; books `scrollspellcode`, `bookspellcode`.

## Randomness

Order of draws for one generated (non-forced) item:

| # | Seed | Draw | Decides |
|---|---|---|---|
| 1 | game | step | unit seed (§2.1) |
| 2 | game | step | start seed / item seed (§2.1) |
| 3 | unit | §4 in order: gold r(5·ilvl) (`items/treasure.md` §8) / quiver or stack r(…) / armor durability r(d>>1) then defense r(maxac−minac+1) / weapon stack, durability | base stats |
| 4 | unit | r(varinvgfx) | graphics |
| 5 | item | quality roll, then the quality routine (`items/quality.md` §3–§8, `items/affixes.md`) | quality, affixes, picks, property values |
| 6 | item | §8.1 roll(100) (expansion, when eligible) | ethereal |
| 7 | item | §7.1 roll(100) (qualities 1–3, when eligible) | sockets |
| 8 | item | automagic (`items/affixes.md` §11) | auto affix |

Class skill mods (§6.2) draw inside step 5, at the end of the quality
routine that calls them. Low-quality durability draws from the unit
seed inside step 5 (`items/quality.md` §6).

## Edge cases & original bugs

1. Stack rolls exclude the maximum (§4 quirk); durability rolls exclude
   odd maxima.
2. Body armor never gets generated sockets in classic games (§7.1 step 3).
3. The socket roll draws even when flags2 & 0x08 suppresses sockets.
4. Generated socket counts use the start seed, not a draw: items sharing
   a start seed share counts.
5. Staffmods after 6 rejected skill tries use the last rejected skill and
   may repeat a skill already chosen (its value overwrites).
6. §7.3 for set/unique items with a socket count already present replaces
   the requested n by that count.
7. "Has durability" needs the item to own a stat list; an item without
   one counts as having no durability (ethereal and socket tests skip it).

## Test vectors

Synthetic, computed from the rules and `sim/rng.md` (script logic in the
spec session scratch; reproduce in unit tests):

| Input | Expected | Source |
|---|---|---|
| unit seed `{1000, 666}`, armor `durability` 24, `minac` 3, `maxac` 5 | stat 72 = 18 (roll(12) = 6, + 12), stat 31 = 5 (3 + roll(3) = 2); seed after `{2466107339, 165470233}` | synthetic |
| start seed 0x12345679, max sockets 4 (hell, `gemsockets` 4, ilvl 50) | n = 0x12345679 mod 4 + 1 = 2, then §7.3 | synthetic |
| §7.3: w×h = 3×4, quality 3, max sockets 6, n = 9 | stat 194 = 6 | synthetic |
| §7.3: w×h = 2×2, quality 4, n = 6 | stat 194 = 4 | synthetic |
| §7.2: `gemsockets` 4, itemtype maxsock1/25/40 = 3/4/6, ilvl 25 / 26 / 41 | 3 / 4 / 4 | synthetic |
| §8.2 weapon mindam 10, maxdam 21 | 15, 31 | synthetic |

Real 1.14d vectors need the recording in Open questions 2.

## Provenance

- Read from 1.14d `Game.exe` (Ghidra exports `re/exports`, register
  arguments from `all.asm`): pipeline `0x00558D90`; base stats
  `0x00557AB0` (unit-seed draws at `0x00557AE4`, `0x00557B78`,
  `0x00557C32`, `0x00557CB1`, `0x00557D08`, `0x00557DA3`, `0x00557E0F`;
  all pass `item +0x20` as the seed); defense `0x00556360`; damage
  `0x005563D0`; elixir `0x0065E8E0`; normal routine `0x00556E80`; books
  `0x005C2540`; staffmods `0x005C1260`, `0x005C0F90`; sockets
  `0x00556B60`, `0x0062BC20`, `0x0062BCB0`; ethereal `0x00556CA0`,
  `0x0065E4D0`; timers `0x00558530`, `0x00558580`; seeds `0x00555230`,
  `0x00552DF0`, `0x00552E90`; format at game creation `0x00530AE0`,
  `0x00530D55`; data `0x0074638C` (elixir table).
- D2MOO 1.10f (`D2Game/src/ITEMS/Items.cpp`: `D2GAME_CreateItemEx_6FC4ED80`,
  `D2GAME_InitItemStats_6FC4E520`, `sub_6FC4D6B0`, `ITEMS_MakeEthereal`,
  `sub_6FC52410`, `sub_6FC52650`) was the map; every rule above was
  re-read on 1.14d. Differences found: 1.14d draws all base stats from
  the unit seed as D2MOO does but D2MOO's helper names hide it; the
  1.14d ear branch and personalized names use the request unit's player
  data lookup `0x006221A0`; staffmods pass the request ilvl as the bonus
  only with flags2 & 0x20 (as D2MOO).

## Open questions

1. Format-0 generation branches (legacy items) are unspecified; confirm
   that no 1.14d creation path passes format 0 (check the 20 callers of
   `0x00558D90` for the value written at request +0x2A).
2. No recording confirms any rule here. Needed: an item-creation trace
   (request R1 in the session report): every unit-seed and item-seed
   draw between the allocation and the return of `0x00558D90`, plus a
   dump of the finished item (quality, file index, affix slots, flags,
   stat list entries).
3. The meaning of request `spawn mode`/`init flags` values per caller
   belongs to `sim/units.md` and the treasure spec; not checked here.
