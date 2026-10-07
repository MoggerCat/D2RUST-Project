# Spec: Items — Treasure classes and drops

- **Status:** draft. The TC runtime form (§1) was rebuilt from the live
  1.14d `.bin` files by a measurement script and gives the 1,013 TCs of
  `loading.md` §10.6 and `@treasureclass` (`data-tool links`) with 0
  unresolved item strings; every drop rule (§2–§8) is read from the 1.14d
  `Game.exe` disassembly. No recording of a drop exists yet (Open
  questions 1–3); the runtime form has not been compared with a memory
  dump (Open question 4).
- **Target version:** 1.14d
- **Crate/module:** `d2-data::treasure` (runtime form, §1) and
  `d2-sim::items::treasure` (drops, §2–§8)
- **Related specs:** `specs/sim/rng.md` (generator, `roll`; §7 names the
  dropping unit's seed), `specs/data/loading.md` §10.6 (TC list order and
  count), `specs/data/field-types.md` §6 (linkers and lookups),
  `specs/data/runtime-maps.md` §2 (item-type equivalence),
  `specs/data/fields.tsv` (record layouts of every table read here),
  `treasure-quality.tsv`, `treasure-chest-acts.tsv` (this folder). Owned
  elsewhere: item creation once id, quality, level and flags are chosen
  (items spec, `claude/phase3-items`); monster death sequence, monster
  level, champion/unique flags, superunique id and the
  `monster_playercount` stat (monsters spec, `claude/phase3-monsters`);
  object operate functions that decide how often a chest drops and with
  which forced quality (objects spec); the free-spot search (collision
  spec); quest flags (quests spec); the Find Item skill (skills spec).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 51–61 |
| Inputs | 62–78 |
| Outputs / state changes | 79–85 |
| Rules | 86–87 |
|   1. TC runtime form (load step 46) | 88–216 |
|   2. TC by id and level (`0x00654E00`) | 217–223 |
|   3. Monster drop | 224–282 |
|   4. Chest drop (`0x00585B90`) | 283–309 |
|   5. The TC walk (`0x0055A6D0`) | 310–416 |
|   6. Drop quality (`0x00558640`) | 417–448 |
|   7. Creation inputs and placement (`0x0055A550`) | 449–473 |
|   8. Gold amount | 474–489 |
|   9. Quest drop helper (`0x00559A30`) | 490–571 |
| Constants & data dependencies | 572–601 |
| Randomness | 602–618 |
| Edge cases & original bugs | 619–642 |
| Test vectors | 643–672 |
| Provenance | 673–697 |
| Open questions | 698–857 |
<!-- /index -->

## Summary

A treasure class (TC) is a weighted list of items and other TCs with a
pick count, a NoDrop weight and quality modifiers. At load, 1.14d turns
`treasureclassex` plus 160 automatic TCs into a runtime array (§1). When
a monster dies (§3), a chest opens (§4) or Find Item succeeds (§3.6),
the game picks a TC and walks it (§5): each pick draws from the dropping
unit's seed, may hit NoDrop (scaled by player count), may descend into a
sub-TC, and for an item rolls its quality (§6) and creates it next to the
dropping unit (§7). Gold piles get their amount in §8.

## Inputs

| Name | Type | Source |
|---|---|---|
| tables | `treasureclassex`, `itemtypes`, `weapons`/`armor`/`misc`, `uniqueitems`, `setitems`, `itemratio`, `monstats`, `superuniques`, `levels` records | `loading.md` (live `.bin`) |
| dropping unit `U` | monster or object; its unit seed (unit +0x20) | caller |
| recipient `R` | killer / operator unit or none (D2MOO `pPlayer`) | caller |
| game | expansion flag (game +0x70, u32), difficulty (game +0x6D, u8 0–2), game type (game +0x6A), players | game state |
| players setting `S` | 0–8, set by the `players` command (`0x00535780` ignores values > 8) | host |
| forced quality `Q` | 0 = roll (§6), else an item quality code 1–8 | caller |
| level input `L` | i32, used only by §6 | caller |
| mode `F` | 0 = normal, 1 = Find Item (NoDrop off) | caller |

Item quality codes: 1 inferior, 2 normal, 3 superior, 4 magic, 5 set,
6 rare, 7 unique, 8 crafted. Unit types: 0 player, 1 monster, 2 object,
4 item.

## Outputs / state changes

- `U`'s seed advances by the draws of §Randomness.
- Item units created through the items spec, placed by §7, with gold
  amounts per §8; at most `max` per walk (§5.1).
- The walk returns the created items (chests: the first, §4).

## Rules

### 1. TC runtime form (load step 46)

#### 1.1 Layout

TC records (0x2C bytes, array `0x0096C5EC`, count `0x0096C5F0`; index =
`@treasureclass` index, `field-types.md` §6.4):

| Field | Off | Type | Meaning |
|---|---|---|---|
| group | 0x00 | u16 | 0 = none |
| level | 0x02 | u16, compared as i16 | |
| count | 0x04 | i32 | entries |
| total classic | 0x08 | i32 | sum of classic probs |
| total expansion | 0x0C | i32 | sum of expansion probs |
| picks | 0x10 | i32 | never 0 (§1.4) |
| nodrop | 0x14 | i32 | |
| mods | 0x1A | 6 × u16 | magic, rare, set, unique, slot 5, slot 6 |
| entries | 0x28 | ptr | `count` entries |

Entries (0x1C bytes):

| Field | Off | Type | Meaning |
|---|---|---|---|
| start classic | 0x00 | i32 | total classic before this entry |
| start expansion | 0x04 | i32 | total expansion before this entry |
| id | 0x08 | u16 | item index (`field-types.md` §6.4), or TC index when flag 4 |
| row | 0x0A | u16 | flags 1/2: uniqueitems / setitems record; else gold multiplier (§1.5) |
| flags | 0x0C | u8 | 0x01 unique item, 0x02 set item, 0x04 TC, 0x10 not in classic |
| mods | 0x0E | 6 × u16 | as the TC's, used only in classic games (§5.7) |

Appending an entry with prob `p` and flag "expansion only" `e`: its starts
are the current totals; total expansion += `p`; total classic += `p`
unless `e` (`0x00654080`, `0x006540C0`; the latter can also change an
existing entry's prob and shift later starts, which the loader never
does).

#### 1.2 Order

TC 0 (empty name, no entries), the automatic TCs (§1.3), then
`treasureclassex` rows (§1.4); list and name order: `loading.md` §10.6.
Each TC's name is added to `@treasureclass` when its record is created,
so a name lookup during §1.4 sees only TCs created before (`field-types.md`
§6.5): a forward reference to a later TC is a miss (1.14d: none). The
loader is fatal if the TC count exceeds 65,534 (`0x0065A390`).

#### 1.3 Automatic TCs (`0x006541C0`)

For each itemtypes record `t` in record order with `treasureclass`
(+0x1D) ≠ 0 (1.14d: records 27 `bow`, 45 `weap`, 46 `mele`, 50 `armo`,
85 `abow`), add 1 to the group offset `A` (§1.4), then for `Lv` = 3, 6,
…, 96: create TC `<code><Lv>` (itemtypes code with pad spaces removed,
decimal `Lv`, e.g. `bow3`) with group 0, level `Lv` − 3, picks 1, nodrop
0, mods 0. Append, for each item index `i` in order (weapons, armor,
misc), an entry when all hold:

1. the item has `quest` = 0 and `spawnable` ≠ 0;
2. the item is of type `t` (its `type` or nonzero `type2` is equivalent to
   `t`, `runtime-maps.md` §2; `0x00629A90`);
3. `t` is record 38 (`tpot`), or the item is not of type 38;
4. `Lv` − 3 < item `level` ≤ `Lv`.

Entry: id `i`, prob = max(itemtypes[item `type`].`rarity`, 1); flag 0x10
and expansion only when item `version` ≥ 100. Measured (model of this
section on the live set): 160 TCs, 763 entries; 85 TCs have classic total
0 and 44 (all `abow`/empty levels) have expansion total 0.

#### 1.4 `treasureclassex` rows (`0x006547D0`)

Rows in record order until the first empty `Treasure Class`. Per row a
new TC:

| TC field | From |
|---|---|
| group | 0 if `group` = 0, else `group` + `A` (u16; 1.14d `A` = 5) |
| level | `level` |
| picks | `Picks`, or 1 if `Picks` = 0 |
| nodrop | `NoDrop` |
| mods | `Magic`, `Rare`, `Set`, `Unique` (record +0x28…+0x2E); slots 5, 6 from record +0x30/+0x32, which no column fills (always 0 in 1.14d) |

Then `Item1`…`Item10` with `Prob1`…`Prob10` in order, stopping at the
first empty item cell; each goes through §1.5.

#### 1.5 Item strings (`0x00654440`)

For one item string `s` with prob `p`:

1. `p` < 1: no entry.
2. If `s` starts with `"`, drop it; then cut `s` at its next `"` (the
   live cells keep their quotes, e.g. `"gld,mul=1280"`).
3. Split at the first `,`: name, then a parameter list (may be absent).
4. Resolve the name, first match wins:
   1. length ≤ 4: item code (padded with spaces to 4 bytes, `0x00650EB0`)
      found in `items.code` (case-sensitive) → item entry; flag 0x10 and
      expansion only when the item's `version` ≥ 100.
   2. `@treasureclass` name lookup (case-insensitive, `field-types.md`
      §6.2) with index ≥ 1 → TC entry, flag 4; if that TC's total
      classic is 0 (at this moment): flag 0x10 and expansion only.
   3. uniqueitems `index` name with record index ≥ **1** → id = item index
      of its `code` (+0x28), row = the record, flags 0x11, expansion only.
      Record 0 (`The Gnasher`) can never be named (original quirk).
   4. setitems `index` name with record index ≥ 0 → id from its `item`
      code (+0x28), row = the record, flags 0x12, expansion only.
   5. none: no entry, prob not counted.
   Unique and set name linkers are add-always over every record
   (`0x006342B0`, `0x00634DC0`), so index = record number.
5. Parameters, left to right, each `key=value` up to the next `,`; value
   = `atol` (`0x00681EBB`) cut to u16:

   | Key | Sets |
   |---|---|
   | `mul`, `ma`, `mg` | entry row (gold multiplier) |
   | `cm` / `cr` / `cs` / `cu` / `ce` / `cg` | entry mods slot 1 / 2 / 3 / 4 / 5 / 6 |

   Keys match exactly. A part without `=` or an unknown key ends the
   parameter list; the entry stays.

Measured on 1.14d: 2,742 TC entries, 660 item entries (81 with `mul`:
1280, 1536 or 2048), 2 unique entries (`ROP (N)`, `ROP (H)` →
`Annihilus`, record 381), 0 set entries, 0 misses, no other parameter
key. 184 TCs have negative picks, 300 a nonzero NoDrop, 240 nonzero
mods.

#### 1.6 Chest TC table (`0x0065A2C0`)

45 TC pointers at `0x0096C5F4`, index (difficulty × 5 + act) × 3 + tier,
filled with the TC named `Act <act+1><D> Chest <T>`, `D` ∈ {``, ` (N)`,
` (H)`}, `T` ∈ {`A`, `B`, `C`}; a missing name stores none. 1.14d: all
45 found (e.g. `Act 1 Chest A` = TC 385).

### 2. TC by id and level (`0x00654E00`)

`get(id, lvl)`: none if `id` = 0 or `id` ≥ TC count. Else start at TC
`id`; if `lvl` > 0 and its group ≠ 0, step forward while the next TC has
the same group and the next TC's level (i16) ≤ `lvl`. Result: the last TC
reached. The starting TC's own level is never compared.

### 3. Monster drop

#### 3.1 Gate (`0x005A6830`, called from the death handler `0x005A6FF0`)

No drop when unit flag bit 17 (unit +0xC4, `0x20000`) is set, or when the
collision at the monster's position (room, x, y, mask 0x801;
`0x0064CB30`) is nonzero. A monster of class 344 (`bonewall`) without
the flag is a fatal error (d2rs: error). Then §3.2 with `F` = 0 and the
death mode change (target = `R`).

Exact order in `0x005A6830` (handoff `impl-treasure` question 8): no unit
→ fatal 0x1ED; flag bit 17 → no drop; class 344 → fatal 0x1F8, **before**
the collision test (so a bone wall on a blocked spot is fatal too);
collision ≠ 0 → no drop; the monster has no room → fatal 0x1FF; its
monstats record missing (`0x00451F80`) → fatal 0x204; else §3.2.

#### 3.2 Which TC (`0x005A6600`)

Columns `TreasureClass1..4` for the game difficulty (monstats +0x86 +
8 × difficulty + 2 × column):

1. Superunique (hcIdx ≠ −1, `0x005A03A0`): its `TC` for the difficulty
   (superuniques +0x2C + 2 × difficulty); if the record is missing,
   column 3.
2. Else champion (type flag 4, `0x005A0180`): column 2.
3. Else unique (type flag 8): column 3.
4. Else column 1.

#### 3.3 Quest TC

Replace with column 4 when all hold: monstats `TCQuestId` ≠ 0; column 4 ≠
0; `R` exists; and the quest owner `P` is a player whose quest flags for
the difficulty (`0x00543520`) have neither flag 15 (completed before),
flag 1 (reward pending) nor flag `TCQuestCP` set (`0x0065C310` ≠ 1 for
each). `P` = `R` if `R` is a player. Else `O` = `R`'s minion owner
(`0x0058F0D0`); if `R` passes `0x0063A690` and has a stat list with flag
0x800, `O` is replaced by the unit that list names (owner type and GUID,
`0x00552F60`), even when that lookup finds none; `P` = `O` if `O` exists,
else `R`.

#### 3.4 Level upgrade (`0x0055AFA0`)

`lvl` = the monster's `level` stat (12) when the game is expansion,
difficulty ≥ 1, `U` is a monster and its monstats has neither `noRatio`
(bit 2) nor `boss` (bit 6); else 0. TC = `get(id, lvl)` (§2); none → no
drop.

#### 3.5 Walk arguments

§5 with `U` = monster, `R` = death target, `Q` = 0, `L` = item level of
§7 (monster `level` stat, at least 1; `0x00558200`), `F`, no output list,
`max` = 6.

#### 3.6 Find Item (`0x005A8000`)

The Find Item skill (`0x005D8780`) calls §3.2 with the corpse as `U`, the
caster as `R` and `F` = 1, skipping §3.1. It computes a value 1–4 from a
second `roll(100)` that `0x005A8000` ignores.

### 4. Chest drop (`0x00585B90`)

Called with the operate context (object `U`, operator `R`) and `Q`:

1. No room → no drop (returns none).
2. `act` = act of the object's level (0–4); area level `a(l)` = levels
   `MonLvlEx`[difficulty] (expansion) or `MonLvl`[difficulty] (classic),
   i16; 1 for level id ≤ 0, ≥ count or difficulty ≥ 3 (`0x0061DCA0`).
3. `lo` = `a(first)`, `hi` = `a(last)` of `treasure-chest-acts.tsv`
   (`0x006E1988`), `s` = (|`hi` − `lo`| + 1) / 3 (integer), `cur` =
   `a(object level)`.
4. Tier = 0 if `cur` < `lo` + `s`; else 1 if `cur` < `lo` + 2`s`; else 2.
5. TC = chest table[difficulty][act][tier] (§1.6; each index clamped to
   0…max); none → no drop.
6. §5 with `U`, `R`, `Q`, `L` = **tier** (not a level; original quirk),
   `F` = 0, a 6-slot output list, `max` = 6. Returns the first item.

Callers and `Q` (decision logic: objects/quests specs):

| Caller | `Q` |
|---|---|
| `0x00585CE0` | 0 |
| `0x00585E00` | 4, three calls |
| `0x00585F60` (chest operate, 11 sites) | 0, 4, 5, 6, 7, or computed (`0x005860CE` `ebx`+7; `0x00586148` `eax`+6; `0x0058633B`, `0x00586365` `ebx`) |
| `0x00586410`, `0x00586520`, `0x005866C0`, `0x00586850`, `0x005868A0`, `0x005869F0` | 0 |
| `0x00599C10`, `0x00599CF0`, `0x00599DF0`, `0x0059A7E0` (2), `0x005B8860`, `0x005B8940`, `0x005B8A20` | 4 |

### 5. The TC walk (`0x0055A6D0`)

#### 5.1 Arguments

game, `U`, `R`, TC (pointer, not none: fatal 0xF3A), `Q`, `L`, `F`,
output list, count out, `max`. Without an output list, `max` < 1 becomes
6; with a list, `max` = 0 is fatal (0xF44). The count starts at 0.

#### 5.2 Slot stack

Up to 64 slots of {TC, picks left, mods[6]}. Slot 0 = {TC, max(|picks|,
1), the TC's mods}. Classic games use total classic and start classic;
expansion games total expansion and start expansion ("total" and "start"
below).

Walk: `k` = 0. Process slot `k` (§5.3) until it ends; then if `k` = 0 the
walk ends, else `k` −= 1 and process that slot again (its remaining
picks).

#### 5.3 Pick loop of a slot

A slot ends at once if its TC is none, its total is 0, or its picks left
are 0. Each pick:

1. `T` = total of the slot's TC; `T` = 0 ends the slot.
2. **Negative picks** (TC picks < 0): `r` = |picks| − picks left; `r` ≥
   `T` ends the slot. No draw, no NoDrop.
3. **Positive picks:** `N` = NoDrop of §5.4 (0 when the TC's nodrop is
   0). If `F` = 0: `r` = `roll(T + N)` on `U`'s seed (inline; no step when
   `T + N` < 1, `r` = 0); if `r` < `N`: picks left −= 1, no item, next
   pick; else `r` −= `N`. If `F` = 1: `r` = `roll(T)`.
4. Picks left −= 1.
5. Select entry `e` by `r` (§5.5); none → next pick.
6. TC entry → §5.6; item entry → §5.7.
7. Loop while picks left ≠ 0.

#### 5.4 NoDrop scaling

Only when the TC's nodrop `n0` ≠ 0, in this order (no draws):

1. Party `p` = 1, unless `R` exists, `O` = `R` if `R` is a player else
   `R`'s minion owner, `O` is a player, and the living members of `O`'s
   party in `O`'s level (`0x005408E0`: `O` alone when it has no party or
   no room) number ≥ 2: then `p` = min(that, 8).
2. Players `P` (`0x00535790`): living players in the game; when game type
   is 1, 2 or 3 (3 = single player, `intents-events.md`), max(that,
   `S`).
3. `n` = `p` + (`P` − `p`) / 2 (signed, toward zero).
4. If `U` is a monster: `n` = min(`n`, max(`U`'s stat 100
   `monster_playercount`, 1)).
5. If `n` ≤ 1: `N` = `n0`. Else, with `C` = the TC's total:
   `x` = `n0` / (`n0` + `C`); `p'` = `x` multiplied by `x` (`n` − 1)
   times, left to right; `q` = 1 − `p'`; if `q` = 0: `N` = 0; else `N` =
   trunc(((1 − `q`) × `C`) / `q`).

The original computes step 5 in x87 floating point (`0x0055A935`–
`0x0055A9B9`, conversion `0x00682FD0`: round to double, truncate). Hard
rule 6 forbids host floating point in `d2-sim`, so d2rs evaluates the
same IEEE binary64 operations (round to nearest even, this order) with an
integer soft-float. The exact value is `C`·`n0`^`n` / ((`n0` + `C`)^`n` −
`n0`^`n`); on the live data every (nodrop, total) pair (23 expansion, 15
classic) gives the same `N` for `n` = 2…8 in binary64, binary32 and exact
rational arithmetic (floor), so the FPU precision mode (Open question 5)
does not change 1.14d results, and the rational form is a valid check.

#### 5.5 Entry selection

- **Expansion** (binary search over start expansion, `c` entries):
  `lo` = 0, `hi` = `c`. While `lo` < `hi`: `m` = `lo` + (`hi` − `lo`) / 2;
  if start[`m`] < `r`: `lo` = `m` + 1; else if start[`m`] > `r`: `hi` =
  `m`; else select `m` and stop. After the loop select max(`lo` − 1, 0).
- **Classic** (linear): the first `i` from 0 whose flags lack 0x10 and
  that is the last entry or has start classic[`i` + 1] > `r`; if none,
  no entry.

#### 5.6 TC entry

`X` = `get(e.id, 0)` (§2: no upgrade). If the current slot's picks left
> 0, `k` += 1 (new slot above); else the current slot is reused. `k` ≥ 64
is fatal (0xFEA). New slot: {`X`, max(|`X`.picks|, 1), mods[j] =
max(previous slot mods[j], `X`.mods[j]) (unsigned)}; processing continues
in it.

#### 5.7 Item entry

1. `e.id` = 0xFFFF: next pick.
2. Classic game only: no item record, or item `version` ≥ 100 → next
   pick. Then raise the current slot's mods to max(slot, `e.mods`) per
   slot (they stay raised for the slot's later picks).
3. Quality and index: flag 1 → quality 7, index `e.row` + 1; flag 2 → 5,
   `e.row` + 1; else index 0 and quality `Q`, or §6 when `Q` = 0.
4. Drop flags `d` = 0. If slot mod 5 ≠ 0: one step of `U`'s seed;
   (`lo'` & 0x3FF) < mod 5 → `d` = 0x04. If slot mod 6 ≠ 0: one step;
   (`lo'` & 0x3FF) < mod 6 → `d` |= 0x10. (Meaning of the bits: items
   spec; in 1.14d both mods are 0 in expansion games, §1.4.)
5. Classic game, item type `throwable` (itemtypes +0x10): a per-walk
   counter `t` (from 0) is read, then incremented; if the value read was <
   10: picks left += 1, next pick (the draws above stay spent). Otherwise
   the item becomes code `lsd` (long sword).
6. Create (§7) with id, quality, index, `d`. Failure → next pick.
7. Gold: if the item is of type 4 `gold` or an equivalent
   (`0x00629BB0`) and `e.row` ≠ 0: gold = (gold × `e.row`) >> 8 (i32
   multiply, arithmetic shift).
8. Store in the output list, count += 1; count ≥ `max` ends the walk
   (step 9 is skipped for that item).
9. Gold find (§8).

### 6. Drop quality (`0x00558640`)

Inputs: item id, `L`, game, `U`, `R`, the slot mods. Draws are `roll`
(`0x0045C3E0`) on `U`'s seed.

1. Item record or its type record missing → 0 (no quality).
2. itemtypes `normal` → 2. Items `unique` → 7. itemtypes `magic` and
   items `quest` → 7.
3. Ratio row (`0x00637910`): among itemratio records with
   `Class Specific` = (itemtypes `class` < 7) and `Uber` = (item of type 45
   `weap` or 50 `armo`, `code` = `ubercode` or `ultracode`, type ≠ 38,
   `quest` = 0; `0x0062B4D0`) and `Version` ≤ 100 (unsigned), the one
   with the highest `Version`, the later record on a tie. None → fatal.
   The difficulty argument is ignored. 1.14d: always a `Version` 1 row,
   also in classic games.
4. `D` = `L` − item `level`. Magic find `M` = `R`'s stat 80
   (`item_magicbonus`) plus its minion owner's when `R` is a player or
   monster, else 0 (`0x005585D0`).
5. If `M` ≤ −100: skip to step 7.
6. Steps 1–5 of `treasure-quality.tsv`, in order. For a step with a gate,
   the step runs only when the itemtypes flag is set (step 4 then returns
   4 without a draw). Chance: `b` = (ratio − `D` / divisor) × 128
   (signed division toward zero). If `M` ≠ 0: `f` = `M` + 100; with an
   MF factor `k`, `f` = `M` × `k` / (`M` + `k`) + 100 when `M` + 100 >
   110 (`0x00558610`); "linear" keeps `f`; `b` = `b` × 100 / `f`. `b` =
   max(`b`, min). Chance = `b` − (`b` × slot mod) / 1024 (toward zero).
   Chance ≤ 0 → result; else `roll(chance)` < 128 → result.
7. Superior (step 6): `h` = (`HiQuality` − `D` / `HiQualityDivisor`) ×
   128; `h` ≤ 0 or `roll(h)` < 128 → 3.
8. Normal (step 7): `n` = (`Normal` − `D` / `NormalDivisor`) × 128; `n` ≤
   0 or `roll(n)` < 128 → 2; else 1.

### 7. Creation inputs and placement (`0x0055A550`)

1. Item id < 0 → nothing.
2. Position: the floor drop `0x00555DA0`(room of `U` (`0x00620BB0`),
   `U`'s position (`x`, `y`; unit coordinates `0x00620870`,
   `sim/path-placement.md` §2.1: a monster's dynamic path sub-tile,
   (0, 0) only when it has no path), size 1, fallback 1)
   (`sim/path-placement.md` §9): start = (`x` + 2, `y` + 3) if a room
   exists there (`0x00463740`), else (`x`, `y`); the free-spot search
   `0x0064E810` gets `U`'s room as its room argument (never the room the
   start lookup found, `0x00555DEC`) and gives the final room and
   position; none → nothing. Items of one walk are placed one after
   another, each seeing the previous ones.
3. Item level: `U` none → 1; monster → its `level` stat; player → base
   `level`; else area level of `U`'s level (§4 `a`); at least 1.
4. Drop request to the items spec (`0x00558D90`): id, quality, index,
   item level, room and position, spawn type 3, init flags 1, the game's
   item format (game +0x78), drop flags `d`, plus 0x01 when `U` is
   monster class 391 (`hellbovine`).
   Spawn type 3 with init flag 1 makes the allocation add the item to the
   world at the request's room and position (`sim/units.md` §3.1 step 8 →
   `sim/path-placement.md` §2.5: static path, footprint, room list) inside
   `0x00558D90`, before its base stats; so the next drop's search sees
   it.

### 8. Gold amount

1. Base (in item creation `0x00557AB0`, item type 4): `g` =
   `roll(5 × ilvl)` on the new item unit's seed (unit +0x20) + ilvl; `g`
   ≤ 0 → 1; a drop-request quantity > 0 replaces it.
   "Quantity" here is the request's quantity override +0x54 (not +0x34;
   `0x00557AF9`, handoff `impl-items` OQ-G1); with no request the gold is 1
   and nothing is drawn.
2. TC multiplier: §5.7 step 7.
3. Gold find (`0x005589A0`), when `R` exists and the item's type is
   exactly 4 (`0x0062B400`): `G` =
   100 + `R`'s stat 79 (`item_goldbonus`) + its minion owner's; gold =
   gold × `G` / 100 (i32, toward zero).

Setting gold (`0x00530EA0`): a negative value stores 0.

### 9. Quest drop helper (`0x00559A30`)

The drop of the quest and object code (`world/objects.md`,
`world/quests-*.md`; 25 call sites). Fastcall: ECX game, EDX source unit
`U`; stack: quality `q`, `&level` (output), `&request` (output, may be
none), then two values `p6`, `p7` passed on to the class pick of step 3
(the quest specs give −1 and 0, or −1 and a value they call
"droppable", `world/quests-act2.md`); `ret 0x14`. Returns the
created item, or none.

1. Game none → fatal 0x9C7.
2. Item level `L`: `U` none → 1; monster → total stat 12; player → base
   stat 12; else the area level of `U`'s room level (`0x0061DCA0` with
   game +0x6D, +0x70, as §7 rule 3); `L` < 2 → 1. Written to `*&level`
   (`0x00559AF8`) before any read (`world/quests-act3-2.md` §11.3).
3. Item class `c`:
   - `U` +0xB8 (the unit's drop item code, `world/objects.md` Inputs)
     ≠ 0 → `c` := the items index of that code (`0x00633680`); no such
     code → fatal 0x9EA. No draw.
   - Else `c` := the random class pick `0x00556240(L, U's unit seed +0x20,
     p6, p7, U is a monster)`; when `q` = 4 (magic), while the record of
     `c` is missing or its items `bitfield1` (+0xDC) bit 0 is clear: the
     first 11 retries call `0x00556240` again, every later one calls
     `0x00555FB0(L, unit seed, p6, p7)`; there is no retry limit.
     `0x00556240`: `L` > 65 → fatal 0x180; else one `roll(100)` r on
     `U`'s unit seed (`0x00472280`): r < 65 − `L` → gold (`gld `, its index
     cached once, `0x008846EC`); r < (65 − `L`) + `L`/2 + 5 →
     `0x00555E70`; r < that + `L`/2 + 10 (+ 1 when `L` is odd) →
     `0x00555FB0`; r < 100 → `0x005560F0`; else fatal 0x1BA. The three
     sub-pickers (Open question 10, answered) are §9.1.
4. Position: `U`'s unit coordinates (`0x00620870`) and room
   (`0x00620BB0`) into the floor drop `0x00555DA0`(room, position, size 1,
   fallback 1), as §7 rule 2; no spot → return none (no request is
   written out, nothing created).
5. Request (0x84 bytes, zeroed; `items/generation.md` Inputs): unit `U`,
   +0x04 0, game, ilvl `L`, item `c`, spawn mode 3, the found x, y and
   room, init flags 1, format game +0x78, quality `q`; every other field
   0 (not forced, no index, no flags). Create through `0x00558D90` with
   "use seed" 0 (`items/generation.md` §3). When `&request` ≠ none the
   whole request is copied there (after the pipeline, so with the
   written-back ilvl). Return the pipeline's item.

#### 9.1 Class sub-pickers (`0x00555E70`, `0x00555FB0`, `0x005560F0`)

Fastcall ECX game, EDX the unit seed; stack `L`, `p6`, `p7` (and, for
`0x005560F0` only, the "`U` is a monster" flag `m`). The combined items
array header `0x0096CA58` (`0x00633590`) holds count +0x00, records
+0x04, then (start, count) per part: weapons +0x08/+0x0C, armor
+0x10/+0x14, misc +0x18/+0x1C (written at load, `0x006333E8`–
`0x0063343B`). So `0x00555FB0` = weapons, `0x00555E70` = **armor**,
`0x005560F0` = misc (the range from the misc start to the array end).
In the quest drop, `0x00555E70` (armor) has the band (65 − `L`) …
(65 − `L`) + `L`/2 + 5, `0x00555FB0` (weapons) the next one, misc the
rest; the magic retries after the eleventh call weapons.

Per record of the part, in index order:

1. Misc only: type (+0x11E) = 40 and `m` = 0 → skip (body parts only
   from monsters).
2. Filter `0x00555E00` (EAX `L`, ESI record, EDI `p6`; stack seed, `p7`):
   `L` < 1 counts as 1; `spawnable` (+0x133) ≠ 0, `quest` (+0x12A) = 0
   and `level` (+0xFD) ≤ `L`, else skip. If `p7` = 0: a = `0x006427F0(L)`
   (the act of **level id** `L`: the item level is passed where a level
   id is expected, so `L` < 40 → 0, < 75 → 1, < 103 → 2, < 109 → 3, else
   4); d = `rarity` (+0xFC, u8) − a; d > 0 → `roll(seed, d)`
   (`0x0045C3E0`), result ≠ 0 → skip. Then `p6` = −1, or the record's
   `type` (+0x11E, i16) = `p6`; else skip.
3. Expansion game (game +0x70 ≠ 0) or `version` (+0xF6) < 100, and fewer
   than 1,023 candidates held → append the record's combined index.

Pick: count n > 0 → one step of the seed; n a power of two → `lo'` &
(n − 1), else `lo'` mod n (unsigned); return that candidate. A part
start of 0 returns −1 at once. n = 0 → the routine returns the
**uninitialised** first slot of its candidate array (a stack value),
reachable whenever every record is filtered out (for example all rarity
rolls reject at a low `L`) — d2rs: Open question 12. `p6` = item type filter (−1 = any), `p7` = skip
the rarity roll; the quest specs pass `p6` = −1. Draws: one `roll(d)`
per record that reaches the rarity test with d > 0, in index order, then
the pick step. `bitfield1` (+0xDC) bit 0, tested by the §9 magic retry
loop, is the same "may be magic" bit the stores test
(`world/vendors.md` §3 step 5).

## Constants & data dependencies

| Item | Value | Where |
|---|---|---|
| automatic TC levels | 3…96 step 3 (32 per type) | `0x006541C0` |
| group offset `A` | number of itemtypes with `treasureclass` ≠ 0 (5) | `0x0096D4B0` |
| max slots | 64 | `0x0055AB8F` |
| default `max` | 6 | `0x0055A76B` |
| classic throwable limit | 10, then `lsd` | `0x0055AE9C`, `0x0055AEB5` |
| chest names | `Act %d%s Chest %s`, `%s%d` | `0x0065A2C0`, `0x006EC7D0` |
| chest act ranges | `treasure-chest-acts.tsv` | `0x006E1988` |
| quality ladder | `treasure-quality.tsv` | `0x00558640` |
| MF factors | 250 unique, 500 set, 600 rare | `0x00558746`, `0x005587BF`, `0x00558847` |
| stats | 12 `level`, 14 `gold`, 79 `item_goldbonus`, 80 `item_magicbonus`, 100 `monster_playercount` | itemstatcost |

Columns: treasureclassex all; itemtypes `equiv1/2` (via
`runtime-maps.md`), `throwable`, `magic`, `rare`, `normal`,
`treasureclass`, `rarity`, `class`; items `code`, `ubercode`,
`ultracode`, `version`, `level`, `type`, `type2`, `unique`, `quest`,
`spawnable`; itemratio all; monstats `TreasureClass*`, `TCQuestId`,
`TCQuestCP`, `noRatio`, `boss`; superuniques `TC*`; levels `MonLvl*`;
uniqueitems/setitems `index`, `code`/`item`.

`treasure-quality.tsv`: one row per ladder step; `result` quality code,
`gate` (itemtypes flag or `always`), itemratio columns, `mf_factor`
(number, `linear` = `M` + 100, `none` = MF ignored), `tc_slot` (slot mod
subtracted), `draw_when` (`chance>0` or `never`). Step 8 is the
fall-through. `treasure-chest-acts.tsv`: per act, the level ids whose
area levels bound the tiers.

## Randomness

All draws below are on `U`'s unit seed (unit +0x20; `rng.md` §7), in
this order per pick:

| # | When | Draw | Decides |
|---|---|---|---|
| 1 | positive picks | `roll(T + N)` (`F` = 1: `roll(T)`), inline `0x0055A9FC`–`0x0055AA3D`; none if the range < 1 | NoDrop / entry |
| 2–7 | item entry, `Q` = 0, not unique/set | `roll(chance)` `0x0045C3E0`, one per reached ladder step with chance > 0 (unique, set, rare, magic, superior, normal: at most 6) | quality |
| 8 | slot mod 5 ≠ 0 | step, `lo'` & 0x3FF | drop flag 0x04 |
| 9 | slot mod 6 ≠ 0 | step, `lo'` & 0x3FF | drop flag 0x10 |

Then item creation (items spec: item seeds; the gold base of §8 on the new
item unit's seed). Draws 2–9 happen before the classic throwable check
(§5.7 step 5), so a re-picked throwable still spends them. Negative picks
draw only 2–9. Nothing in §1–§4 draws.

## Edge cases & original bugs

1. Chests pass the tier (0–2) as `L`, so `D` = tier − item level is
   negative and ratios rise (§4 step 6). Reproduced.
2. uniqueitems record 0 cannot be named in a TC (§1.5 step 4.3).
3. The 6th item of a monster walk gets no gold find (§5.7 step 8).
4. Classic per-entry mods raise the slot for its remaining picks (§5.7
   step 2).
5. `R` absent still counts players: `n` = 1 + (`P` − 1) / 2 (§5.4).
6. Classic games use the `Version` 1 itemratio rows (§6 step 3).
7. A forward TC reference is silently dropped at load (§1.2); d2rs: same
   result, plus a load note.
8. D2MOO 1.10f differences: on an exact hit the expansion search returns
   `max(lo − 1, 0)` instead of `m` (1.14d: `m`, §5.5); the act 5 chest
   range ends at level 132 (1.14d: 136); player count lacks `S` (§5.4).
9. `M` ≤ −100 jumps past the itemtypes `magic` gate (§6 step 5:
   `0x0055871F` branches to the superior step at `0x00558921`; the gate
   is at `0x005588B4`), so a `magic` itemtype (rings, amulets, jewels,
   charms) can get drop quality 3, 2 or 1. Creation does not keep it:
   the quality dispatch overrides any quality outside 4–9 to 4 for a
   `magic` itemtype (`items/quality.md` §4 step 3.1, `0x005574E6`–
   `0x00557513`). The drop quality only changes the draws: steps 1–5
   draw nothing and the item comes out magic. (Handoff triage Q3.)

## Test vectors

Synthetic (from the rules; CI-safe). RNG steps per `rng.md` §2.

| Input | Expected | Source |
|---|---|---|
| Walk, expansion. TC: picks 2, nodrop 100, items `a` 21, `b` 16, `c` 21, `d` 2 (starts 0, 21, 37, 58; `T` 60). `Q` = 4, mods 0, `R` none, `P` = 1, `U` an object with seed {12345, 666} | pick 1: `lo'` 22752887, `roll(160)` = 87 < 100 → NoDrop; pick 2: `lo'` 2337785264, 144 − 100 = 44 → `c`. Seed after: {2337785264, 9490055}; 2 draws | §5.3, §5.5 |
| Same, `P` = 3 | `n` = 2, `N` = 38; `roll(98)`: 31 → NoDrop; 66 − 38 = 28 → `b` | §5.4 |
| Search, starts [0, 21, 37, 58], `r` = 37 / 0 / 59 | `c` / `a` / `d` | §5.5 |
| NoDrop `n0` 100, `C` 60, `n` = 2…8 | 38, 19, 10, 6, 3, 2, 1 | §5.4 |
| NoDrop `n0` 19, `C` 81, `n` = 2…8 | 3, 0, 0, 0, 0, 0, 0 | §5.4 |
| Quality: ratio row Unique 400/1/6400, Set 160/2/5600, Rare 100/2/3200, Magic 34/3/192, HiQuality 12/8, Normal 2/2; type rare, not magic/normal; `L` 50, item level 30, `M` 0, mods 0; seed {12345, 666} | rolls `roll(48640)` 38007, `roll(19200)` 12464, `roll(11520)` 2551, `roll(3584)` 1693, `roll(1280)` 1048; normal chance −1024 → quality 2, 5 draws | §6 |
| Same with `M` = 100 | unique `f` 171, `b` 28444; set `f` 183, `b` 10491; rare `f` 185; magic `f` 200, `b` 1792 | §6 step 6 |
| Slot mod magic 1024 (e.g. `Andariel`), magic step reached | chance 0 → 4 without a draw | §6 |
| Negative picks −2, entries prob 1, 2 | picks give `r` 0 → entry 0, `r` 1 → entry 1; no draw | §5.3 |
| Quality, `M` = −100, an item of a `magic` itemtype (`amul`; not `normal`, not `unique`, not `quest`), ratio row as above (HiQuality 12/8), `L` 99, item `level` 1 | `D` 98, `h` = (12 − 12) × 128 = 0 → 3, no draw; creation sets quality 4 (`items/quality.md` §4 step 3.1) | §6 step 5, edge case 9 |

Real 1.14d (game-file tests, `#[ignore]`, from the live `.bin` set):

| Input | Expected | Source |
|---|---|---|
| TC count, kinds | 1,013 TCs; 160 automatic; 2,742 TC / 660 item / 2 unique / 0 set entries; 0 misses; 81 `mul` | §1, measurement |
| `Act 1 H2H A` | TC 430, group 12 (7 + `A`), picks 1, nodrop 100, entries `gld` (item 523) 21, TC 218 (`Act 1 Equip A`) 16, TC 203 (`Act 1 Junk`) 21, TC 370 (`Act 1 Good`) 2; starts 0, 21, 37, 58; totals 60 / 60 | §1.4 |
| `ROP (N)` | entries TC 855 (`Diablo (N)`) prob 4, `Annihilus` (flags 0x11, row 381) prob 1; totals classic 4, expansion 5 | §1.5 |
| `Act 1 Champ A` | picks −2; entries `Act 1 Citem A` 1, `Act 1 Cpot A` 2 | §1.4 |
| `get(430, 40)` | 445 `Act 1 (N) H2H B` (levels in group: …, 38, 40, 41, …) | §2 |
| `get(430, 0)`, `get(430, 85)`, `get(0, 40)` | 430; 471 `Act 5 (H) H2H C` (last of the group); none | §2 |
| Chest tier, expansion normal, act 0 | `lo` 1, `hi` 12, `s` 4: level 8 (area 1) → tier 0, `Act 1 Chest A` (385); level 37 (area 12) → tier 2 | §4 |
| Chest, expansion Hell, act 4 | `lo` 0 (level 109), `hi` 83 (level 136), `s` 28 | §4 |

## Provenance

- 1.14d `Game.exe` disassembly (`re/exports/all.asm`), register arguments
  read from it: loader `0x0065A390` → `0x006541C0` (automatic TCs),
  `0x006547D0` (rows), `0x00654440` (item strings), `0x00654080` /
  `0x006540C0` (entries), `0x0065A2C0` (chest table); lookups
  `0x00654E00`, `0x00654E80`; monster drop `0x005A6830`, `0x005A6600`,
  `0x0055AFA0`; Find Item `0x005D8780` → `0x005A8000`; chest
  `0x00585B90`; walk `0x0055A6D0`; quality `0x00558640` (helpers
  `0x005585D0`, `0x00558610`, `0x00637910`, `0x00629F70`, `0x0062B4D0`);
  creation `0x0055A550`, `0x00555DA0`, `0x00558200`; gold `0x00557AB0`,
  `0x005589A0`, `0x00530EA0`; player and party counts `0x00535790`,
  `0x005405A0`. Data read from `Game.exe`: bit masks `0x006CE264` (noRatio
  4 at `0x006CE270`, boss 0x40 at `0x006CE280`), act ranges `0x006E1988`,
  chest name parts `0x00745A10`–`0x00745A24`.
- Measurements: a script rebuilt §1 from `game/extracted/patch_d2` `.bin`
  files (TC count equals `loading.md` §10.6 and `data-tool links`);
  NoDrop precision comparison over all live pairs; levels area values.
- D2MOO 1.10f (`Items.cpp` `D2GAME_DropTC_6FC51360`, `sub_6FC52110`;
  `MonsterMode.cpp` `sub_6FC631B0`; `ObjMode.cpp`
  `OBJMODE_DropFromChestTCWithQuality`) as a hint; every rule above was
  read from 1.14d; differences in Edge cases 8. D2MOO's quality code is
  inline in its walk; 1.14d calls `0x00558640` with the same ladder and
  MF factors.

## Open questions

1. No recording of a drop: record RNG draws while killing monsters and
   opening chests (sites in `0x0055A6D0`–`0x0055AF80` and calls from
   `0x00558640`) and check §Randomness order and counts.
2. Walk arguments and results not observed: log TC index, `Q`, `L`, `F`
   at `0x0055A6D0` entry and id/quality/index/flags at the creation call
   `0x0055AEE7`.
3. NoDrop `N` not observed: log `n0`, `C`, `n` and the result at
   `0x0055A9B9` for a game with `players` > 1.
4. The runtime TC array (`0x0096C5EC`, 1,013 × 0x2C plus entries) and the
   chest table (`0x0096C5F4`) are not dumped; §1 is a model until a dump
   matches (entry counts, starts, flags, rows). Recording list IT-9.
5. x87 precision control on the server thread during §5.4 (irrelevant
   for 1.14d data, §5.4; matters for mods with other nodrop/total pairs).
   Partly answered (2026-10-07, from the binary); the rest **Needs
   recording**.
   - CRT startup: `___tmainCRTStartup` calls `__cinit(1)` (`0x006828EA`,
     ebx = 1), which calls `__fpmath(1)` through `0x006F2874`; with a
     non-zero argument it runs `__setdefaultprecision` (`0x00682FC4` →
     `0x0068E70A`): `_controlfp_s(NULL, 0x10000 = _PC_53, 0x30000 =
     _MCW_PC)`, then `fnclex`. Rounding control and exception masks are
     not touched, so the main thread starts with the Windows default
     word plus PC = 53 bits: round to nearest even, 53-bit significand,
     all exceptions masked. `__set_controlfp` has no callers;
     `__controlfp_s` is called only from there and from
     `__setdefaultprecision`.
   - Game code: the only `fldcw` sites outside the CRT are 14 local
     pairs in 8 functions (`0x0047CC90`, `0x0047CD00`, `0x00605080` ×4,
     `0x00605F00`, `0x0061BCE0`, `0x00679190` ×2, `0x00679250` ×2,
     `0x0067A140` ×2): each saves the word (`fnstcw`), sets RC = chop
     (`or 0xC00`) for one `fistp`, and restores it. None is on the §5.4
     path, and none changes the precision. The CRT's own math helpers
     (`0x006879DD`–`0x00688718`, `__ctrlfp` `0x0069D097`) also save and
     restore.
   - §5.4 itself (`0x0055A935`–`0x0055A9B9`): `fild` of the i32 values,
     `fdivp`, the `fmul` chain, `fld1` / `fsubrp`, the `fucomp` zero
     test, `fsub`, `fimul` by `C`, `fdivrp`, all on the x87 stack with
     no store between them, then `0x00682FD0` (SSE2: `fstp` to a double,
     `cvttsd2si`, truncation regardless of RC). Under the startup word
     (PC = 53, RC = nearest) every one of these operations rounds once
     to a 53-bit significand; the exponent range is wider than
     binary64's, which matters only outside its normal range (here, for
     n0 ≥ 1 and n0 + C in 1..2³¹−1, x > 2⁻³¹ and p' = x^n with n ≤ 8,
     so every intermediate stays above 2⁻²⁴⁸, far inside binary64's
     normal range), and the `fstp`
     to a double is exact. So with the startup word §5.4 equals the
     IEEE binary64 evaluation d2rs uses, bit for bit.
   - Not settled by the binary: single player runs the server inline
     on the client frame thread (`sim/tick.md` §1 rule 4), the same
     thread as the renderer. Game.exe passes neither `DDSCL_FPUSETUP`
     (0x800) nor `DDSCL_FPUPRESERVE` (0x1000) at its SetCooperativeLevel
     calls found by their error strings (`0x0051129F`, `0x005115E6`,
     `0x006B40E2`, `0x006B4541`: 0x11; `0x006B505A`: 0x411), so whether
     the DirectDraw / Direct3D runtime, a Glide wrapper or a driver
     leaves the thread at another precision (24-bit) is decided outside
     Game.exe. Recording: in a single-player game under each video mode
     (DirectDraw, Direct3D, Glide), read the x87 control word (`fnstcw`)
     at `0x0055A935` on a monster kill with a nodrop TC and `n` ≥ 2;
     expect 0x027F (PC = 53, RC = nearest, all masked); any other PC
     value means the evaluation is not binary64 there.
6. Meaning of drop flags 0x04/0x10 (D2MOO: superior / normal) and their
   effect in creation: items spec.
   Answered (2026-10-07, `0x0055A550` disassembled): §7 stores `d`
   (its fourth stack argument) OR-ed into request flags2 (+0x80,
   `0x0055A69B`–`0x0055A69E`; 0x01 first for `hellbovine`), so they
   are `items/generation.md` §1.5 bits: 0x04 = always ethereal (§8.1
   there: the ethereal roll is drawn, then applied regardless, on
   eligible items), 0x10 = always sockets (§7.1 step 6: p := 0, so the
   socket count is applied). Not superior / normal quality. In 1.14d
   slot mods 5 and 6 come from record +0x30 / +0x32, which no column
   fills (§1.4, OQ12 item 10: 0 in all 853 rows), so a TC drop never
   sets either bit.
7. Answered (handoff `impl-treasure` 13): "living" = not dead by
   `0x005541B0` (`sim/units.md` §2: unit flag 0x10000, a player in mode
   0 or 17, a monster in mode 0 or 12; any other unit type counts as
   dead). The player count `0x00535790` counts every player the game's
   player walk `0x005538D0` visits with that test (callback
   `0x00535760`). The party count `0x005408E0` → `0x005405A0`: `O`
   without a room or without a party (`0x00554630` = 0xFFFF) → `O` alone
   (counted when living); else each member of the party list
   (`0x00540290`; none → 0) found as a player by GUID whose room's level
   equals `O`'s, counted when living (callback `0x005404F0`).
8. Answered: `sim/path-placement.md` §7 (search, no RNG draw) and §9 (floor drop, §7 step 2 here).
9. Answered (handoff `triage-game-findings` Q3): edge case 9; the d2rs
   sweep's "`magic` ⇒ q ≥ 4" holds after creation, not for the drop
   quality when `M` ≤ −100.
10. Answered: §9.1 (disassembly of the three functions and of
    `0x00555E00`, `0x00556240`). Original text: §9 rule 3 (quest drop
    with no drop code): the sub-pickers
    `0x00555E70`, `0x00555FB0` (a candidate list over the items records
    between table +0x08 and +0x10, filtered by `0x00555E00`, expansion
    or `version` < 100, then one unit-seed step: mask for a power-of-two
    count, else mod) and `0x005560F0` are not specified, nor the meaning
    of items `bitfield1` bit 0 and of `p6` / `p7` inside them. The quest
    specs set the drop code before their calls (e.g. Wirt's body `gld `,
    `world/quests-act1.md`), so the path matters only for a caller that
    leaves +0xB8 at 0; settle with `disasm.py fn` on the three
    functions.
11. Answered (handoff `impl-treasure` questions 2, 5, 6, 8): §6 step 2's
    last test is one conjunction (itemtypes +0x14 `magic` and items +0x12A
    `quest`, `0x00558640`); §5.7 step 7's gold test is `0x00629BB0(item,
    4)` on the created item unit (`0x0055AEF5`: the `type` row or, when
    `type2` > 0, the `type2` row equivalent to 4); the class-index test
    `0x00629A90` differs only for a negative `type2` (taken as a row
    there), which 1.14d data has not. §6 step 3 compares the itemratio
    bytes for equality with the two 0 / 1 results (`0x00637910`: record
    +0x43 `Class Specific` = `0x00629F70`, +0x42 `Uber` = `0x0062B4D0`), so
    a stored 2 matches neither; the uber test's type 38 is the item's own
    `type` field (items +0x11E), its weapon / armor test is the class-index
    test with `type2`. §3.1's order is given there.
12. Answered (handoff `impl-treasure` 1, 3, 4, 7, 9–12, 14), from the
    1.14d code:
    - 1 (`atol` beyond i32): `0x00681EBB` → `0x00681E95` is
      `strtol(s, NULL, 10)` (`0x0068676E` → `strtoxl` `0x00686543`), which
      saturates: an overflowing positive value gives 0x7FFFFFFF, a
      negative one 0x80000000 (errno ERANGE), then §1.5 step 5 cuts to
      u16 (0xFFFF, 0x0000). d2rs's strtol saturation is exact.
    - 3 (NoDrop range): the conversion `0x00682FD0` takes the SSE2 path
      when `0x00994C88` is set (`__get_sse2_info` `0x0069A8C0`, CPUID
      SSE2 and OS support): `fstp` to a double, then `cvttsd2si`, so a
      NaN, an infinity or a value outside i32 gives 0x80000000; the x87
      fallback (`0x00683006`) is used only without SSE2. `n0` + `C` is an
      i32 sum before `fild` (`0x0055A947`), and the q = 0 test
      (`0x0055A9A1`–`0x0055A9A8`) treats an unordered (NaN) q as nonzero.
      Default FPU exceptions are masked, so a division by zero gives an
      infinity, not a fault. Then `T` + `N` (i32, wrapping) < 1 → `r` = 0
      without a draw, and the NoDrop test `r` < `N` is signed
      (`0x0055AA45`). The precision control (Open question 5) stays open.
    - 4 (§5.6 with `get` = none): the new slot's TC pointer is stored and
      its picks are read through it at once (`0x0055ABAE`), so none
      faults; unreachable, as §1.5 step 4.2 builds TC entries only for an
      index ≥ 1 below the count.
    - 7 (itemratio divisor 0): the divisions of §6 are plain `idiv` by
      the record fields (`0x00558733`, `0x005587AE`, `0x00558836`,
      `0x005588CC`, `0x00558925`, `0x0055895A`), each only when its step
      runs: a 0 divisor is an integer-divide fault (process crash). Every
      divisor of the 6 live `itemratio` rows is ≥ 1.
    - 9 (chest act): `act` = `0x006427F0(level id)` (`drlg/levels.md` §6
      rule 3; table `0x006EB2F4` = 40, 75, 103, 109, 1024): always 0–4
      (≥ 1024 → 0), never from `levels.txt`, and `0x00654E80` clamps
      again.
    - 10 (`treasureclassex` +0x30/+0x32): measured: the live
      `treasureclassex.bin` (853 rows of 736 bytes) holds 0 at +0x30 and
      +0x32 in every row.
    - 11 (item string quotes): `0x00654440` drops one leading `"` and
      then replaces **every** later `"` by a 0 byte, i.e. cuts at the next
      quote whether or not a leading one was dropped.
    - 12 (expansion search over zero entries): `lo` = `hi` = 0 → index
      max(−1, 0) = 0, then the index-below-count test (`0x0055AB35`)
      fails → no entry; unreachable (`T` = 0 ends the slot first).
    - 14: the fatal paths (0xF3A, 0xF44, 0xFEA, no ratio row, bone wall,
      > 65,534 TCs) are the original's asserts (process exit); how d2rs
      reports them is a Ruleset choice, not a fidelity fact.
    ~~Still open: §9.1's n = 0 result.~~ Struck (2026-10-07): the value
    is whatever earlier calls left in that stack slot, so no reading of
    the binary fixes it; d2rs picks a Ruleset value. Recording list
    `docs/handoff/pc2-rec-pc2-items.md` IT-11 (what 1.14d does in the
    one reproducible case).
