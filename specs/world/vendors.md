# Spec: World — Vendors (store inventories, refresh, gambling, prices)

- **Status:** draft: store and gamble generation, refresh, the
  transaction-cost function and the buy/sell flow are read from the 1.14d
  `Game.exe` (addresses below); per-vendor data is measured on the live
  `npc.txt`, `weapons/armor/misc.txt`, `gamble.txt`; the 0x2A layout and
  three recorded transactions (one sell, two buys) match
  `traces/raw/20261006-015956-packets.jsonl`. No store contents recorded
  yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::vendors`
- **Related specs:** `world/npc.md` (NPC records, interaction, menu
  actions, heal, identify, hire; opens the trade and gamble screens that
  call §3 and §5); `world/quests.md` §1 (quest flag records read by the
  price multipliers); `sim/rng.md` §3, §5.2 (NPC-control seed); `sim/tick.md`
  §5.6 (event 13); `data/fields.tsv` (`npc`, item vendor columns,
  `gamble`, `difficultylevels`); item creation, item duplication and
  inventory placement belong to the items and inventory specs (Phase 3,
  not written). Machine table: `world/vendors.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 46–58 |
| Inputs | 59–70 |
| Outputs / state changes | 71–76 |
| Rules | 77–78 |
|   1. Vendor lists (built once per process) | 79–113 |
|   2. Store item level | 114–120 |
|   3. Filling a store (`0x00576980`) | 121–180 |
|   4. Refresh (when a store is regenerated) | 181–209 |
|   5. Gambling | 210–252 |
|   6. Transaction cost (`0x0062EFB0`, wrapper `0x0062FDC0`) | 253–328 |
|   7. Buying (C→S 0x32, `0x00577F30` → `0x00577830`) | 329–355 |
|   8. Selling (C→S 0x33, `0x00579510`) | 356–374 |
|   9. Repair (C→S 0x35) | 375–381 |
|   10. S→C 0x2A NpcTransaction | 382–399 |
| Constants & data dependencies | 400–417 |
| Randomness | 418–429 |
| Edge cases & original bugs | 430–441 |
| Test vectors | 442–455 |
| Provenance | 456–474 |
| Open questions | 475–488 |
<!-- /index -->

## Summary

Every interactable town NPC has a record in the game's NPC control
(`world/npc.md` §1). A trader's record owns one store inventory shared by
all players. It is filled the first time a player opens the trade screen
(or after a refresh) from a per-vendor item list built from the 17
vendor column groups of the item tables, at an item level derived from
the opening player's level. Gamblers build one 14-item gamble inventory
per player. All store and gamble randomness comes from the NPC-control
seed. Prices for buying, selling, repairing and gambling come from one
transaction-cost function using the item's cost, its affixes, `npc.txt`
multipliers and quest-flag discounts.

## Inputs

| Name | Type | Source |
|---|---|---|
| item tables | `weapons`/`armor`/`misc` combined (stride 0x1A8): `cost` +0xE0, `level` +0xFD, `spawnable` +0x133, vendor columns +0x146/+0x157/+0x168/+0x179/+0x18A (+ vendor index), `PermStoreItem` +0x1A4, `ubercode` +0x88, `ultracode` +0x8C, `NightmareUpgrade` +0x19C, `HellUpgrade` +0x1A0, `gamble cost` +0xD4, `version` +0xF6 | `data/fields.tsv` |
| `npc.txt` | 76-byte records | §6.2 |
| `gamble.txt` | codes | §5.1 |
| `difficultylevels.txt` | `GambleRare` +0x44, `GambleSet` +0x48, `GambleUnique` +0x4C, `GambleUber` +0x50, `GambleUltra` +0x54 | §5.2 |
| player | level (stat 12), gold (14), stash gold (15), reduced prices (stat 87) | stats spec |
| NPC-control seed | game +0x1D24 → control +0x18 | `world/npc.md` §1 |
| host millisecond counter | `GetTickCount` in 1.14d | §4 (a d2rs host input, like `sim/rng.md` §5.1) |

## Outputs / state changes

Store and gamble inventories (item units created through item creation),
gold changes, S→C 0x2A NpcTransaction, item messages (items/inventory
specs).

## Rules

### 1. Vendor lists (built once per process)

`0x00536F80` (from `0x00530690`, game setup) walks monstats rows with the
interact flag (monstats +0xD bit mask 2) and, for each vendor NPC, builds
its list once (`0x00536D50(index)`, guarded by byte `0x00883E88[index]`):

| NPC (monstats id) | List index | NPC | Index |
|---|---|---|---|
| akara 148 | 0 | ormus 255 | 8 |
| gheed 147 | 1 | elzix 199 | 9 |
| charsi 154 | 2 | asheara 252 | 10 |
| fara 178 | 3 | halbu 257 | 12 |
| lysander 202 | 4 | jamella 405 | 13 |
| drognan 177 | 5 | malah 513 | 14 |
| hratli 253 | 6 | larzuk 511 | 15 |
| alkor 254 | 7 | drehya 512, nihlathak 514 | 16 |

Index i selects the item-table vendor columns at offset +i (Akara 0,
Gheed 1, Charsi 2, Fara 3, Lysander 4, Drognan 5, Hratli 6, Alkor 7,
Ormus 8, Elzix 9, Asheara 10, Cain 11 (no vendor uses it), Halbu 12,
Jamella 13, Malah 14, Larzuk 15, Drehya 16).

For list i, every item row r in item-table order with `spawnable` ≠ 0
and (`<V>Max` ≠ 0 or `<V>MagicMax` ≠ 0):

- `PermStoreItem` ≠ 0 → append r's code to the permanent list;
- else append a 12-byte entry {min, max, magic min, magic max (u8 each),
  code (u32 @4), magic level (u8 @8)} from r's `<V>Min`, `<V>Max`,
  `<V>MagicMin`, `<V>MagicMax`, code, `<V>MagicLvl`.

The game copies its NPC's list into the NPC record at NPC-control
creation (`0x00535FB0`, index from `vendors.tsv` `cache`). Nihlathak's
record copies list **15** (Larzuk's), although list 16 is also built for
him (original quirk; D2MOO 1.10f has the same 15).

### 2. Store item level

At fill time (§3), with L = the opening player's character level (stat
12): ilvl = L + 5; in Normal (game +0x6D = 0) ilvl = min(ilvl,
cap[act]) with cap = 12, 20, 28, 36, 45 for acts I–V (`0x00576890`; act =
NPC record +0x22). Nightmare and Hell: no cap.

### 3. Filling a store (`0x00576980`)

Called when the store is empty (`world/npc.md` §4.1) or flagged for
refresh. seed = NPC-control seed. Record +0x28 (ticks) = host ms counter.
fails = 0.

1. For each list entry e (list order):
   1. Skip unless items[e.code].level ≤ ilvl.
   2. n = 0 if ilvl ≥ 25; else n = roll_range(seed, e.min, e.max + 1)
      (`0x004BC500`: no draw and n = min when max + 1 ≤ min; else
      min + roll(max + 1 − min)).
   3. Skip the rest unless the item's `version` < 100 or the game's item
      format (game +0x78) ≥ 100.
   4. Repeat n times: quality = §3.1 (one draw); create (§3.2); failure
      → fails += 1; if fails > 32 stop the whole fill.
   5. If the item row is magic-capable (`0x00629C80` = 1) and
      e.magic_level ≤ ilvl: extra = 1, or if ilvl ≥ 25 extra =
      roll_range(seed, 1, 3) + 1 (one draw: 2 or 3); m =
      roll_range(seed, e.magic_min, e.magic_max + extra); repeat m times:
      create with quality 4 (magic); failure → fails += 1 (not tested
      against 32 inside this loop).
2. For each permanent code (list order): create with quality 2 (normal);
   on success, if the code is `cqv` (bolts) or `aqv` (arrows) set its
   quantity (stat 70) to its total max stack (`0x006295B0`); failure →
   fails += 1; stop if fails > 32.

#### 3.1 Normal-slot quality (`0x00576900`)

One step of the seed, r = lo' mod 100: ilvl < 5: r > 90 → 1 (inferior),
else 2; 5 ≤ ilvl < 10: r > 85 → 3 (superior), else 2; ilvl ≥ 10: r > 74 →
3, else 2.

#### 3.2 Creating a store item (`0x00576330`)

(npc, code, quality q, ilvl, player level L):

1. Code upgrade, only if difficulty ≠ Normal and L > 25:
   - Nightmare: r = roll(seed, 100000); if r < ilvl·64 + 4000 and the
     row's `ubercode` is set (≠ 0 and ≠ four spaces) → ubercode; else if
     `NightmareUpgrade` ≠ `xxx` → that code.
   - Hell: r = roll(seed, 100000); if expansion (game +0x70), r <
     ilvl·16 + 1000 and `ultracode` is set → ultracode; else if r <
     ilvl·128 + 5000 and `ubercode` set → ubercode; then, regardless, if
     `HellUpgrade` ≠ `xxx` → that code.
2. Up to two attempts: create (item creation `0x00559CE0`: count 1,
   quality q, level ilvl); if the result is inferior with low-quality name
   "Cracked", free it and create again (at most 5 creations; 5 Cracked →
   fail). If the item's base code ≠ the chosen code (item creation
   substituted it), free it, q = 2, next attempt; after two mismatches →
   fail.
3. Store page = item type `StorePage` (`0x0062E880`); 0xFF → free, fail.
   Set the page; restore durability/quantity (`0x005761C0`, `world/npc.md`
   §6); place in the NPC inventory (`0x00560200`); if that fails and page
   ≠ 1 → hand the item to the NPC's pending list (`0x00535F30`), fail; if
   page = 1, retry on page 2, same failure handling.
4. Mark it a vendor item (`0x005762C0`: identified, update flags, add to
   the trade inventory). Return it.

All draws inside item creation belong to the items spec.

### 4. Refresh (when a store is regenerated)

NPC record fields (`world/npc.md` §1): +0x1C trading opened, +0x20
filled, +0x27 refresh pending, +0x28 fill time (ms), +0x40 NPC GUID,
+0x10/+0x11 refresh-on-level flags (`vendors.tsv` `refresh_flag`).

- `0x00537230(game, act, nobody_left)` runs for each trader record of
  that act with trading opened and filled:
  - nobody_left = 0: if fill time + 240000 < now (ms, unsigned), set
    refresh pending and fill time = now.
  - nobody_left = 1: if the NPC unit is gone (GUID lookup fails or
    class differs): free the inventory, filled = 0, fill time = 0, new
    empty inventory. Else if the NPC has no interacting player
    (`0x00572DA0` = 0): `0x00536D10` (free and reallocate the inventory,
    filled = 0); else set refresh pending.
- Callers: `0x00537340` (a player leaving town level 1, 40, 75, 103, 109
  for act 0–4; nobody_left = no other player in that level), and
  `0x00537580` (player leaving the game).
- When a player opens trade (`world/npc.md` §4.1, `0x00579430`) with only
  this player interacting and refresh pending: clear it, free and
  reallocate the inventory (`0x00536D10`), fill (§3), filled = 1.
- Timer event 13 (UPDATETRADE / REFRESHVENDOR, player handler
  `0x005689D0`, `sim/tick.md` §5.6): its effect on stores is not yet read
  (Open question 1).

In single player the 240000 ms rule makes a store refresh after 4
minutes of wall time once the player leaves and re-enters town; d2rs
takes the millisecond counter as a host input.

### 5. Gambling

#### 5.1 Gamble table (`0x00638AE0`, at table load)

Rows of `gamble.bin` (12 bytes: code, level, item index) get level =
items[code].level and the item index; the rows are sorted by level with
the C runtime `qsort` (comparator `0x00638AC0`: unsigned level, ascending;
order of equal levels is whatever MSVC `qsort` produces, Open question
3). selection[k] = item index of sorted row k. limit[0] = 2; for L = 1
.. 99: limit[L] = index of the first sorted row with level > L, or the
row count. 1.14d `gamble.txt` (d2exp) has 126 rows.

#### 5.2 Filling a player's gamble inventory (`0x00578790`)

(game, npc, player, record): needs the gamble table and the
difficulty's `difficultylevels` row. high = GambleRare + GambleSet +
GambleUnique. A new per-player node {inventory, player GUID, next} is
prepended to the record's gamble list. L = player level; seed =
NPC-control seed. count = 0; repeat:

1. Step the seed; ilvl = L + (lo' mod 10) − 5; ilvl < 6 → 5; ilvl > 98
   → 99.
2. n = limit[ilvl]; idx = 0 if n < 1 (no draw), else roll(seed, n) (one
   step); item = selection[idx].
3. Classic game: item `version` ≥ 100 → discard this pick (count not
   incremented, loop again).
4. count 0 → item = `rin `; count 1 → item = `amu `.
5. Expansion upgrade (`0x005786A0`): if `ubercode` set and chance_u =
   (ilvl − uber.level)·GambleUber + 1 > 0: if roll(seed, 10000) < chance_u
   → uber item; else if `ultracode` set and chance_x = (ilvl −
   ultra.level)·GambleUltra + 1 > 0 and roll(seed, 10000) < chance_x →
   ultra item.
6. Quality 4 (magic); if high > 0: step, r = lo' mod 100000; if r < high:
   r < GambleUnique → 7 (unique); else r ≥ GambleSet + GambleUnique → 6
   (rare); else 5 (set).
7. count += 1; create (item creation: count 1, the quality, level ilvl);
   page 0, durability restored, **not identified**; place it in the
   node's inventory; placement failure frees the item and ends the fill.
8. Stop when count > 13 (14 items).

The 0x37 IdentifyGamble intent and the purchase are in §7 and
`world/npc.md` §5.

### 6. Transaction cost (`0x0062EFB0`, wrapper `0x0062FDC0`)

(player, item, difficulty d, player quest record, NPC class, type):
type 0 = buy (vendor sells), 1 = sell (vendor buys), 2 = gamble, 3 =
repair. All divisions are signed and truncate toward zero; "×m/1024"
means (v·m)/1024, except when v > 65535 and m ≠ 0, where it is
(v/1024)·m.

1. No player or not an item → 0x7FFFFFFF. Type 3 and the item is not
   repairable (`0x0062E660`) → 0. Item flag 0x20000 (start item) → 1.
2. q = max(1, quantity stat 70); red = min(99, player stat 87).
3. **Gamble (type 2):** if item data +0x30 = 0 → return `gamble cost` of
   the row of the item's normcode (or code). Else c = §6.1(L); return c −
   ratio(c, red, 100).
4. Base values (A vendor-sell, B vendor-buy, C repair), divisor k = 1:
   - ear (item flag 0x10000): A = B = ear level byte × `cost`;
   - item type 40 (body part): A = B = `cost` + 8 × monstats level[d] of
     the part's monster (row +0xAA + 2d);
   - item type 18 (book): A = B = `cost` + q × books `cost per charge`
     (+0x14);
   - quiver-class type (itemtypes +0xE ≠ 0): A = B = q·`cost`/1024;
     C = total max stack·`cost`/1024;
   - else A = B = C = `cost`; if stackable, k = total max stack when ≥ 2.
   - armor (type 50): if `maxac` − `minac` ≠ −1 and `maxac` ≠ 0: A = B = C
     = `cost` × base AC (stat 31) / `maxac`.
5. Item skills (`0x0062EDD0`, stat 107 entries: skills.txt cost mult
   +0x234 / add +0x238) are added before step 6 when the quality is not in
   magic..tempered, else after step 7.
6. If identified (flag 0x10): sums s = automagic affix (×mult/1024 + add,
   magicprefix/suffix rows +0x88 mult, +0x8C add); then by quality: 1
   inferior → s = −X/2 (replaces); 3, 9 → bonus-stat costs
   (`0x00628E70`); 4 magic → prefix 1 + suffix 1, then bonus stats; 5 set
   → setitems mult +0x38, add +0x3C; 6, 8 → 3 prefixes + 3 suffixes, then
   bonus stats; 7 unique → uniqueitems mult +0x7C, add +0x80 (no row →
   treated as magic). Then A += sA/k, B += sB/k, C += sC/k.
7. Socketed items: + half `cost` of each socketed item to A, B, C
   (`0x006292F0`).
8. Ethereal (flag 0x400000): B /= 4. Class-specific type (itemtypes +0x21
   < 7): B /= 4.
9. Type 1: if ethereal with max durability ≠ 0 and durability (72) < 1 →
   B = 0. Type 3: if not quiver-class, not indestructible
   (`0x0062BA80`) and has durability: if max dur ≠ 0 and dur < max: if
   stat 252 (replenish durability) ≠ 0: C = (max − 1 > dur) ? C·(max −
   1)/max : 0; else C = C·(max − dur)/max; else C = 0.
10. `npc.txt` row of the NPC (`0x00656900`; none → fatal): A ×= `sell
    mult` (+4)/1024; B ×= `buy mult` (+8)/1024, with the overflow form
    chosen by **`sell mult` ≠ 0** (original bug, D2MOO same); C ×= `rep
    mult` (+0xC)/1024.
11. For each of the 3 quest flags (+0x10, +0x14, +0x18) that is ≠ 0 and
    whose slot has bit 0 or bit 1 set in the player record: A ×= quest
    sell mult (+0x1C + 4i), B ×= quest buy mult (+0x28 + 4i), C ×= quest
    rep mult (+0x34 + 4i), each /1024.
12. Unless type 18 (book) or quiver-class: A ×= q; then if stackable and
    repairable: m = total max stack; if q < m and stat 253 (replenish
    quantity) = 0: C = (m − q)·C, B = m·B − C; else B = m·B, C = 0; else
    B ×= q.
13. Type 3 and not ethereal: C += charged-skill recharge cost
    (`0x00628D30`, stat 204 entries, factor 10000).
14. B = min(B, `max buy` for d (+0x40 + 4d)).
15. Result: type 1 → B, or 1 if B < 1; type 3 → C − ratio(C, red,
    100), min 1; type 0 → A − ratio(A, red, 100), min 1.

ratio(a, b, c) (`0x00483360`) = 0 if c = 0; a·b/c computed in 32 bits
when a ≤ 0x100000 and b ≤ 0x10000; else (b/c)·a or (a/c)·b when the
divisor is ≤ 1/16 of the big operand; else a 64-bit a·b/c.

#### 6.1 Gamble price (`0x00629370`, L = player level)

r = row of the item's normcode (or code). `rin ` / `amu ` → r.`gamble
cost`. Else: avg = max(1, (r.`maxstack` + r.`minstack`)/2); pu = 0, cu =
0: if r.`ubercode` ≠ 0 and ≠ `0   `: u = its row; pu = max(0, (L −
u.level)·100/2 + 1), cu = u.`cost`; px, cx likewise for `ultracode` with
/4. L' = 5 if L < 6 else L; t = max(0, r.level − 45) − r.level/2 + L'.
price = ((t·250/3 + ((10000 − px − pu)·r.cost·avg + cx·px +
cu·pu)/10000) · ((2L' + 1)/3 + 20)) / 15.

### 7. Buying (C→S 0x32, `0x00577F30` → `0x00577830`)

0x32 (17 bytes): npc u32@1, item u32@5, u32@9 (bit 31 = buy as many as
possible; bits 16–30 and 0–15 passed on, low u16 = transaction type: 0
buy, 2 gamble), u32@13 cost (sent by the client, not used by the
server's price check).

1. The NPC must be the player's interact unit (`0x00554D00`), else 0x2A
   error 9.
2. Item missing → error 7. Type 0: item must be in the NPC's trade
   inventory, else 7. Type 2: the player's gamble node at this NPC must
   exist and hold the item, else 7.
3. price = cost(type); gold + stash gold < price → error 12. Item on the
   player's cursor → error 7.
4. Multi-buy only for the NPC's permanent items (`0x00576ED0`: in the
   permanent list; in Nightmare/Hell also `hp4 hp5 mp4 mp5`).
5. Tomes (type 22) refill a matching tome in the inventory
   (`0x0055F640`), price × charges, capped by gold; stackables with
   multi-buy: count = min(gold / unit price, room) (`0x00577700`).
6. Else: copy the vendor item (`0x0055A2A0`), take the gold
   (`0x00576D90`: from gold first, the rest from stash gold), remember the
   copy at player data +0x6C (for 0x37), place it (`0x00560200` / a
   matching stack `0x0055E9B0`), remove the original from the vendor if
   it is not permanent (`0x00576F50`: gamble items are replaced through
   `0x00576650`), send 0x2A kind 4 (success). Failure to place refunds
   the gold and sends error 10.

### 8. Selling (C→S 0x33, `0x00579510`)

0x33 (17 bytes): npc u32@1, item u32@5, u16@9 mode (0 inventory, 4 cursor
or other), u32@13 (client cost, unused).

1. NPC = interact unit, else error 9; item must exist; it must belong
   to the player (`0x00557FF0`), else 11; its mode must equal u16@9, else
   9; sellable (`0x0062A130`), else 9.
2. price = cost(type 1).
3. The vendor keeps a copy unless the item is Cracked, broken (flag
   0x100), quality 7, a runeword (0x1000000), ethereal (0x400000),
   socketed-with-content (`0x0055F590`), a quest item (`0x00575FA0`), the
   player is gambling there, or the item is permanent stock
   (`0x00576ED0`); the copy is placed in the vendor inventory and its
   price for the same transaction is computed; price = min(price, copy
   price).
4. Remove the player's item (mode-specific), add price gold
   (`0x0055B060`), send 0x2A kind 3, flag 1.

### 9. Repair (C→S 0x35)

`0x00578050(npc, item, mode, cost)`: price per item = cost(type 3);
repairing restores durability, quantity and charges (`world/npc.md`
§6). A u32 item of 0 repairs all (D2MOO). The remaining steps are not
yet read (Open question 2).

### 10. S→C 0x2A NpcTransaction

15 bytes (`0x0053D740`): u8 0x2A, u8 kind, u8 code, 4 bytes **not
written** (stack contents), u32 item GUID or −1, u32 player gold after
the transaction.

| kind / code | Meaning |
|---|---|
| 4 / 0 | bought |
| 5 / 0 | bought into a stack or tome; hired; resurrected (`world/npc.md`) |
| 3 / 1 | sold |
| 0 / 7, 9, 10, 11, 12, 15 | refused: 7 item not available, 9 invalid NPC/item/mode, 10 no room, 11 not owner / quest lock, 12 not enough gold, 15 spawn failed |
| 0 / 3 | identify-all done (`world/npc.md`) |
| 0 / 6, 14 | heal bought / nothing to heal (D2MOO; no 1.14d heal purchase found) |

d2rs writes the 4 unwritten bytes as recorded once a policy exists (Open
question 4).

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| refresh delay | 240000 ms | `0x0053730A` |
| failure limit | > 32 | `0x00576980` |
| Normal ilvl caps | 12, 20, 28, 36, 45 | `0x00576890` |
| gamble items | 14 | `0x00578790` |
| upgrade odds | NM ilvl·64+4000; Hell ultra ilvl·16+1000, uber ilvl·128+5000 (of 100000) | `0x00576330` |

`npc.txt` columns: `npc` (+0), `sell mult` (+4), `buy mult` (+8), `rep
mult` (+0xC), `questflag A–C`, `questsellmult`, `questbuymult`,
`questrepmult`, `max buy` ×3. Live values: Gheed sell 1088, Charsi 960,
Akara and Acts II–IV 1024, Act V 2048; buy 512, rep 128 everywhere; quest
flag 4 (Act I), 9 (II), 17 (III), 41 (IV, V: slot 41 is the respec slot
in 1.14d, edge case 3) with sell mult 922; max buy 5000/10000/15000/20000/
25000 by act in Normal, 30000 Nightmare, 35000 Hell.

## Randomness

All from the NPC-control seed, in this order:

| Step | Draws |
|---|---|
| §3 per list entry with level ≤ ilvl | count roll (if ilvl < 25 and max + 1 > min); then per normal item: 1 quality step, then §3.2 (1 roll(100000) if NM/Hell and L > 25) and item creation; magic: 1 extra roll if ilvl ≥ 25, 1 count roll (if range > 0), then per item §3.2 |
| §3 permanent items | per item §3.2 |
| §5.2 per pick | 1 level step, 1 pick roll (if limit ≥ 1), 0–2 roll(10000) (expansion), 1 quality step if high > 0, then item creation |

Item creation, duplication and placement draws: items spec.

## Edge cases & original bugs

1. Nihlathak sells from Larzuk's list (§1).
2. B's overflow form tests `sell mult` (§6 step 10).
3. Acts IV–V `npc.txt` rows use quest flag 41; in 1.14d slot 41 is the
   Den of Evil respec slot (`world/quests.md` §10.3), so their 922
   discount applies once a respec is pending or used.
4. The 0x2A message carries 4 uninitialized bytes (§10).
5. The magic loop of §3 does not test the failure limit.
6. Gamble table order for equal levels depends on MSVC `qsort`.
7. Refresh uses wall-clock milliseconds (§4).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| buy `hp1` (cost 30) at Akara, Normal, no reduction, slot 4 bits 0, 1 clear | A = 30, price 30 | §6, live npc.txt / misc.txt |
| same with slot 4 bit 0 set | 30·922/1024 = 27 | §6 step 11 |
| sell `hp1` to Akara | B = 30·512/1024 = 15 | §6 |
| ilvl for L = 30 at an Act I vendor, Normal | min(35, 12) = 12 | §2 |
| quality roll lo' mod 100 = 91, ilvl 3 | 1 (inferior) | §3.1 |
| gamble ilvl: L = 10, lo' mod 10 = 0 | 5 | §5.2 |
| recorded sell `33 06000000 07000000 0400 0000 f4010000` | `2a 03 01 ?? ?? ?? ?? 07000000 f4010000` (gold 500) | `015956` frame 1157 |
| recorded buy `32 06000000 12000000 00000000 38000000` | `2a 04 00 ???????? 36000000 bc010000` (444 = 500 − 56) | frame 1279 |
| recorded buy `32 10000000 59000000 00000000 28000000` | `2a 04 00 ???????? 62000000 94010000` (404 = 444 − 40) | frame 1853 |

## Provenance

- 1.14d `Game.exe` exports: vendor lists `0x00536F80`, `0x00536D50`,
  `0x00535FB0`; NPC control `0x00536070`; store fill `0x00576980`,
  `0x00576900`, `0x00576330`, `0x00576890`; refresh `0x00537230`,
  `0x00537340`, `0x00579430`; gamble `0x00638AE0`, `0x00578790`,
  `0x005786A0`; cost `0x0062EFB0`, `0x00629370`, `0x00483360`; buy
  `0x00577830`; sell `0x00579510`; message `0x0053D740` (assembly read
  for the byte layout). Assert strings `.\UNIT\SUNITNPC.CPP`,
  `.\UNIT\sunitproxy.cpp`, `.\DATATBLS\ItemTbls.cpp`.
- D2MOO 1.10f (`SUnitNpc.cpp`, `SUnitProxy.cpp`, D2Common `Items.cpp`
  `ITEMS_CalculateTransactionCost`) gave the structure; each rule above
  was matched to the 1.14d function cited. Differences: 1.14d stack sell
  value (§6 step 12: B = m·B − (m − q)·C, D2MOO's form is garbled), gamble
  `0   ` sentinel, `hp4/hp5/mp4/mp5` permanent in NM/Hell.
- Live data: `npc.txt` (17 rows), `misc.txt`, `gamble.txt` (126 rows),
  `fields.tsv` offsets.
- Recording `015956-packets` (frames 1157, 1279, 1853).

## Open questions

1. What timer event 13 (`0x005689D0`) does to stores or trades.
2. Repair (`0x00578050`) and repair-all details and messages.
3. Equal-level order of the sorted gamble table: read the runtime array
   at `0x0096CAB4` with `dump_tables.py`.
4. Policy for the 4 unwritten bytes of 0x2A (compare as don't-care?).
5. Bonus-stat cost `0x00628E70` (itemstatcost fields) and item-skill cost
   `0x0062EDD0` exact rules (only the entry points are given here).
6. Gamble item data +0x30 = 0 path: which items take it.
7. The `force_vendor` byte of the NPC table (record +0x26): no reader
   found.
8. What happens to store items handed to the NPC pending list.
