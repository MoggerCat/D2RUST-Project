# Spec: World — Vendors (store inventories, gambling, buy, sell, repair, prices)

- **Status:** draft: every rule is read from the 1.14d `Game.exe`
  (addresses below); two recorded stores (Charsi 43 items, Akara 43
  items at character level 1, Normal) fit the generation rules item for
  item in order, counts and quality bands, and three recorded
  transactions match the price rules exactly (Charsi buy 56, Charsi sell
  500, Akara buy 40; `traces/raw/20261006-015956-packets.jsonl`).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::vendors`
- **Related specs:** `world/npc.md` (NPC records §1, talk and menu
  actions §2–§4, the 0x2A message §9: this spec does not restate them);
  `world/quests.md` §1.3 (quest slots read by the npc.txt multipliers);
  `sim/rng.md` §3, §5.2, §7; `sim/intents-events.md` §2 +
  `client-messages.tsv` (0x32, 0x33, 0x35, 0x37) / `server-messages.tsv`;
  `data/fields.tsv` (`npc`, `weapons`/`armor`/`misc`, `itemtypes`,
  `difficultylevels`, `magicprefix`, `uniqueitems`, `setitems`, `books`,
  `skills`, `itemstatcost`); `data/runtime-maps.md` §7 (gamble index and
  level thresholds); `data/loading.md` §9 (combined item array order);
  item spec (Phase 3, not written: item creation, item messages 0x9C /
  0x9D, inventory placement, stat lists). Machine table:
  `world/vendors.tsv` (per-NPC vendor column, act, menus).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–62 |
| Inputs | 63–73 |
| Outputs / state changes | 74–82 |
| Rules | 83–84 |
|   1. Vendor columns and per-NPC store lists | 85–125 |
|   2. Store item level | 126–131 |
|   3. Store generation (`0x00576980(npc, player, record)`) | 132–242 |
|   4. Opening trade or gamble (`0x00579430(npc, single, gamble)`) | 243–272 |
|   5. Gambling | 273–339 |
|   6. Refresh | 340–371 |
|   7. Buying and selling | 372–622 |
|   8. Repair | 623–669 |
|   9. Prices | 670–846 |
| Constants & data dependencies | 847–866 |
| Randomness | 867–882 |
| Edge cases & original bugs | 883–912 |
| Test vectors | 913–935 |
| Provenance | 936–974 |
| Open questions | 975–1038 |
<!-- /index -->

## Summary

Each trading NPC sells from one of 17 vendor columns of the item tables
(`<Vendor>Min/Max/MagicMin/MagicMax/MagicLvl`) plus its permanent items
(`PermStoreItem`). The store is generated from the NPC-control seed when
a player first opens trade (§3) and kept until no player is left in that
town (§6). Gamblers make a separate 14-item list per player (§5),
dropped when the chat closes. Buying (0x32) copies the store item into
the player's inventory (§7.1), selling (0x33) pays the player and puts a
restored copy into the store (§7.2), repairing (0x35) restores
durability, quantity and charges (§8). All prices come from one function
(§9): an item value from the item tables, scaled by `npc.txt`
multipliers, quest discounts, quantity and the player's reduced-prices
stat, capped by `max buy` when the NPC pays.

## Inputs

| Name | Type | Source |
|---|---|---|
| C→S 0x32 BuyItem, 0x33 SellItem, 0x35 Repair, 0x37 IdentifyGamble | intents | `client-messages.tsv` |
| trade / gamble open | call from `npc.md` §4 | `0x00579430` |
| player leaves a town level, client leaves the game | calls | §6 |
| item tables, `npc.txt`, `itemtypes`, `difficultylevels`, gamble index | tables | Constants |
| NPC-control seed | game +0x1D24 → +8 | `npc.md` §1.1 |
| wall-clock milliseconds (`GetTickCount`) | host input | §6 rule 2 only |

## Outputs / state changes

NPC inventories (store grid), per-player gamble inventories, items
created / copied / removed, player gold (stat 14) and stash gold (stat
15), item durability (72), quantity (70) and charges, S→C 0x2A
(`npc.md` §9), 0x3E (stat of an item), item messages 0x9C / 0x9D (item
spec; recorded action 11 = shown in a store, 12 = taken out of a store,
4 = given to the player).

## Rules

### 1. Vendor columns and per-NPC store lists

1. Vendor column index i (0–16) = byte offset of `<Vendor>Min` − 326 in
   the item record: Akara 0, Gheed 1, Charsi 2, Fara 3, Lysander 4,
   Drognan 5, Hratli 6, Alkor 7, Ormus 8, Elzix 9, Asheara 10, Cain 11,
   Halbu 12, Jamella 13, Malah 14, Larzuk 15, Drehya 16. Fields at
   326+i (Min), 343+i (Max), 360+i (MagicMin), 377+i (MagicMax), 394+i
   (MagicLvl), u8 each (`fields.tsv`). Column names: Hratli's (index 6)
   are spelled `HraltiMin`, `HraltiMax`, `HraltiMagicMin`,
   `HraltiMagicMax`, `HraltiMagicLvl` in the `Game.exe` field tables of
   weapons, armor and misc (Blizzard's typo, offsets 332 / 349 / 366 /
   383 / 400); look columns up by these names (or by offset), never by
   the NPC's name. The 1.14d `weapons.txt` and `armor.txt` headers spell
   the last one `HratliMagicLvl`, so it does not bind (`data/schema.md`
   absent list) and offset 400 holds the missing-column value 0 in every
   row of the live `weapons.bin` (306 rows) and `armor.bin` (202 rows),
   although the `.txt` cells hold 1, 20 or 255; `misc.txt` spells it
   `HraltiMagicLvl` and its 151 rows load 255.
2. **Global lists** (`0x00536F80` → `0x00536D50(i)`, once per server
   start from `0x00530690`, freed by `0x00537120`): for every
   `interact` monstats row whose class has a column (switch at
   `0x00536FF6`; table in `vendors.tsv`), column i is built once: walk
   the combined item array in index order (`loading.md` §9); an item
   qualifies if `spawnable` (u8 +307) ≠ 0 and (Max ≠ 0 or MagicMax ≠ 0).
   `PermStoreItem` (u8 +420) ≠ 0 → its code (u32 +128) goes to the
   permanent list; else a 12-byte entry {Min, Max, MagicMin, MagicMax,
   code u32 @4, MagicLvl @8} goes to the item list. Both keep item order.
3. **Per-game copy** (`0x00535FB0(i)`, from `npc.md` §1.1 via the switch
   at `0x00536278`): the trader's record gets copies of column i's two
   lists (+0x2C/+0x30, +0x34/+0x38). i > 16 → fatal assert.
4. Class → column (1.14d, both switches): gheed 1, akara 0, charsi 2,
   drognan 5, fara 3, elzix 9, lysander 4, asheara 10, hratli 6, alkor 7,
   ormus 8, halbu 12, jamella 13, larzuk 15, drehya 16, malah 14,
   **nihlathak 15** (Larzuk's column; the global switch builds 16 for
   him, but 15 is the one copied). Column 11 (Cain) is never used.
5. Record flags set at the same time: has gamble list (+0x0C := 1) for
   gheed, elzix, alkor, jamella, drehya, nihlathak; +0x24 and +0x25 := 1
   for gheed, charsi, fara, hratli, asheara, halbu, jamella, larzuk,
   drehya (no reader found, `npc.md` Open question 3). A record whose
   NPC table byte "trader" is 0 gets empty lists.

### 2. Store item level

L_p = player level (stat 12). ilvl = L_p + 5; in Normal (game +0x6D =
0) capped by the NPC's act: 12, 20, 28, 36, 45 for acts 0–4
(`0x00576890`). Nightmare and Hell: no cap.

### 3. Store generation (`0x00576980(npc, player, record)`)

Runs when trade opens and the record's +0x20 (store generated) is 0
(§4), or on a pending refresh (§6). Seed: the NPC-control seed. Record
+0x28 := `GetTickCount()` (host value, §6). fails := 0.

For each item-list entry e, in list order (every list code and
permanent code is the `code` of an item record, §1 rule 2, so the code
map always finds it; a missing one would be read through a null record
at `0x005769F7` — handoff `impl-vendors` V8, unreachable):

1. rec = item record of e.code. rec `level` (u8 +253) > ilvl → next
   entry, no draw.
2. n_norm = ilvl < 25 ? range(e.Min, e.Max + 1) : 0, where
   range(min, max) (`0x004BC500`) = min with no draw if max ≤ min (signed),
   else roll(max − min) + min (`rng.md` §3 roll).
3. If rec `version` (u16 +246) ≥ 100 and the game's item format (game
   +0x78) < 100: next entry (the n_norm draw already happened).
4. Repeat n_norm times: q = quality draw (`0x00576900`: one step,
   r = lo' mod 100; ilvl < 5: r > 90 → 1 inferior else 2 normal; ilvl
   5–9: r > 85 → 3 superior else 2; ilvl ≥ 10: r > 74 → 3 else 2); make
   the item (§3.1) with quality q; a null result counts fails += 1; if
   fails > 32 → **stop the whole generation**.
5. If rec `bitfield1` (u32 +220) bit 0 is set and e.MagicLvl ≤ ilvl:
   k = 1, or for ilvl ≥ 25 k = range(1, 3) + 1 (one draw: 2 or 3);
   n_mag = range(e.MagicMin, e.MagicMax + k); make n_mag items of
   quality 4 (magic); nulls count in fails but are **not** tested
   against 32 here.

Then for each permanent code, in list order: make one item of quality 2
(§3.1); null → fails += 1; else if its code is `cqv` or `aqv`, quantity
(stat 70) := max stack (§9.2 rule 10). After each permanent item: fails
> 32 → stop.

Recorded check (frame 899, Charsi, character level 1, ilvl 6): 43 items
in exactly this order — per entry the n_norm items (quality 2 or 3)
then the n_mag items (quality 4), entries with level > 6 skipped, `ktr`
(version 100) present in an expansion game, then `aqv`, `cqv`. Akara
(frame 1779): 5 + 8 `wnd`, 3 + 3 `scp`, 6 + 8 `sst` (inside the Min–Max
and MagicMin–MagicMax + 1 bands), then one each of `vps yps wms tbk ibk
tsc isc key hp1 mp1`. Every recorded store item has item level 6.

#### 3.1 Making one store item (`0x00576330`)

Arguments: code, quality q, ilvl, L_p.

1. Upgrade (Nightmare and Hell only, and only if L_p > 25), on the base
   record rec of the code; one draw r = roll(NPC-control seed, 100000):
   - Nightmare: r < ilvl·64 + 4000 and `ubercode` (+136) ≠ 0 and ≠
     4 spaces → ubercode; else if `NightmareUpgrade` (+412) ≠ `xxx ` →
     that code.
   - Hell: expansion (game +0x70) and r < ilvl·16 + 1000 and
     `ultracode` (+140) valid → ultracode; else if r < ilvl·128 + 5000
     and ubercode valid → ubercode. Then, independently, if
     `HellUpgrade` (+416) ≠ `xxx ` → that code (overrides the above).
2. Up to 2 rounds: up to 5 tries of item creation (`0x00559CE0`: owner
   NPC, mode 4, quality q, item level ilvl; item spec); a try whose
   result is inferior with the low-quality name `Cracked` is destroyed
   (5 such tries → return null). If the created item's base code ≠ the
   chosen code: destroy it, set q := 2 and run the second round; equal
   → done. Exact (handoff `impl-vendors` V7, `0x0057652B`–`0x0057655B`):
   a mismatch in round 2 also destroys the item, and the result is null
   (the earlier sentence "after round 2 the item is kept whatever its
   code" was wrong). The chosen code is the upgrade code even when the
   code map lacks it: the lookup `0x00633640` then writes class index 0
   (`hax`), so both rounds create class 0, mismatch, and return null. A
   null result of the creation call itself is not a failed try: the
   quality test `0x00627E70` reads quality 2 for none, the try loop ends,
   and the code read `0x00628590(none)` is a fatal assert (line 0x61A).
3. Store page = `itemtypes.storepage` of the item's type (u8 +34;
   `storepage.txt` rows: 0 armo, 1 weap, 2 mag, 3 misc); 0xFF → destroy,
   null.
4. Inventory page := store page; repair it (§8.2, no player); place in
   the NPC grid (`0x00560200`). Failure on page 1 → page 2 and retry.
   Still no room → the item is parked on the record's event list
   (`0x00535F30`, §3.4) and null is returned.
5. Mark it (`0x005762C0`): flag 0x10 (identified); vendor item (unit
   +0xC8 |= 4); flag 1 when it has filled sockets (`0x0055F590`); add to
   the NPC's trade inventory (`0x0063CC70`; the client sees 0x9C action
   11). "Flag 0x10", "flag 1" and "flag 2" in this spec are item flags
   (item data +0x18, `0x006280D0`): values 0x10, 0x1, 0x2 (handoff
   `impl-vendors` V13). Between the socket flag and the trade-inventory
   add, `0x005762C0` refreshes the NPC unit (`0x00621000(npc, 1)`).

#### 3.2 What item creation is asked for

| Caller | Code | Quality | Item level | Owner / mode |
|---|---|---|---|---|
| store normal (§3 step 4) | e.code or upgrade | 1, 2 or 3 | ilvl | NPC, 4 |
| store magic (§3 step 5) | e.code or upgrade | 4 (→ 2 on code mismatch) | ilvl | NPC, 4 |
| store permanent | code or upgrade | 2 | ilvl | NPC, 4 |
| gamble (§5) | §5 step 3 | 4, 5, 6 or 7 | L_g | NPC, 4 |

Item seeds come from the game seed per `rng.md` §5.3, never from the
NPC-control seed.

#### 3.3 Failures

Normal and permanent items stop the generation after 33 null results;
magic items never stop it. A null is: 5 cracked tries, a code mismatch
in both rounds, no store page, or no room (§3.1).

#### 3.4 NPC event list (record +0x14)

Nodes {unit, arg, kind, deferred, next}. Two writers: §3.1 step 4
(deferred = 1, the unplaced item) and level-up (`0x00536850`: for every
record with trader = 1, store generated = 1 and has traded = 1, a node
{player, 4, 1}). The only consumer `0x005368F0` is never called in
1.14d (`npc.md` §10); nodes are freed when the record's data is cleared
(§6), deferred items destroyed with them. No gameplay effect.

### 4. Opening trade or gamble (`0x00579430(npc, single, gamble)`)

From `npc.md` §4 (actions 1, 2):

1. Record +0x1C := 1 (has traded), +0x40 := NPC GUID.
2. `0x00578B30`: vendor-chain node of this player (record +0x18,
   created on first use, `0x00536CB0`).
   - Trade: only if trader: node gamble-mode := 0; store generated = 0 →
     generate (§3), set 1; hire list made = 0 → for kashya, qual-kehk,
     greiz, asheara make it (`npc.md` §7.1); set it to 1.
   - Gamble: only if the record has gamble lists: node gamble-mode := 1;
     if the player has no gamble list here → make one (§5).
3. For NPC classes other than kashya, qual-kehk, greiz:
   - trade, single, and refresh pending (+0x27): clear it, clear the
     record's items (§6 rule 4), new NPC inventory, generate, +0x20 := 1;
   - refresh the NPC inventory (`0x00621000`);
   - every item of the store (trade) or of the player's gamble list
     (gamble): vendor item flag (unit +0xC8 |= 4), flag 1 if it has
     filled sockets, add to the NPC's trade inventory
     (`0x00576C30`); the client receives one 0x9C action 11 per item
     (recorded, frame 899). Order (handoff `impl-vendors` V14): the
     walk is the NPC's (or the gamble node's) inventory item list, first
     `0x0063B2C0`, next `0x0063DFA0`, which is link order
     (`items/inventory.md` §1.4 rule 1): items in the order they were
     placed, a sold copy appended when §7.2 rule 8 places it, a bought
     item unlinked by §7.1 rule 12.

D2MOO 1.10f inverts the class test of step 3 (only kashya / greiz /
qual-kehk continue); 1.14d excludes them.

### 5. Gambling

#### 5.1 Making a player's list (`0x00578790`)

Requires the gamble index (`runtime-maps.md` §7: count, index list,
thresholds T[0..99]) and the `difficultylevels` row of the game's
difficulty: rare R (+68), set S (+72), unique U (+76), uber (+80),
ultra (+84). H = R + S + U. Item ids of `rin` and `amu` are cached once.
A new node {inventory, player GUID, next} is prepended to the record's
list (+0x08). L_p = player level. c := 0. Loop:

1. One step of the NPC-control seed: L_g = (lo' mod 10) − 5 + L_p;
   L_g ≤ 5 → 5; L_g ≥ 99 → 99.
2. idx = T[L_g] < 1 ? 0 (no draw) : roll(T[L_g]) (inlined, `rng.md`
   §3); id = index list[idx].
3. Classic game: item record missing → stop; `version` ≥ 100 → skip
   this pass (c unchanged, back to step 1).
4. c = 0 → id := `rin`; c = 1 → id := `amu` (the step 1–2 draws are
   still made).
5. Upgrade (`0x005786A0`, expansion only): rec = record of id; if
   `ubercode` ≠ 4 spaces, ≠ 0 and found: w_u = (L_g − uber.level)·uber
   + 1 (uber = `GambleUber`); if w_u > 0: roll(10000) < w_u → uber
   item; else if `ultracode` valid: w_x = (L_g − ultra.level)·ultra + 1;
   w_x > 0 and roll(10000) < w_x → ultra item. (No ultra test when the
   uber test is skipped.)
6. Quality: 4 (magic); if H > 0: one step, r = lo' mod 100000; r < U →
   7 (unique); else r < U + S → 5 (set); else r < H → 6 (rare).
7. c += 1. Create the item (owner NPC, mode 4, quality, item level
   L_g). If created: inventory page 0, repair (§8.2), flag 0x10 cleared
   (unidentified), place in the node's inventory; no room → destroy and
   stop.
8. Stop after c reaches 14.

Exact (handoff `impl-vendors` V9, `0x00578790`): the gamble index
`0x00638CC0` is the fixed block `0x0096CAB0` and is never none; with a
gamble count of 0 every threshold is 0, idx = 0 and the read of index
list[0] goes through the absent (null) list (a fault; 1.14d has 125
rows). idx < T[L_g] ≤ count always (`runtime-maps.md` §7), so no index
runs past the list. The `rin` / `amu` ids are cached by `0x00633640`,
which stores 0 when the code is missing: the first two passes then use
item 0 (`hax`), and the lookup is retried on the next list. An expansion
game never tests the record (step 3 is classic only); the upgrade
`0x005786A0` returns the drawn id unchanged when its record is missing.

#### 5.2 Price

`§9.4`, with the player's level at purchase time.

#### 5.3 Buying a gambled item

C→S 0x32 with transaction 2 (§7.1). The bought copy stays unidentified;
the list item is taken out of the list (§7.1 step 12).

#### 5.4 Dropping a player's list

- `0x00537190(player)`: when the player's 0x30 empties the NPC's
  interaction list (`npc.md` §3): unlink the player's node, remove its
  items, free it. The next gamble open makes a new list (§5.1 draws).
- Clearing the record's data (§6 rule 4) drops every player's list.

#### 5.5 C→S 0x37 IdentifyGamble

Handler `0x0054BC30`: size 5 else 3; item (u32 @1) missing → 2; player
data +0x6C (GUID of the last bought item, §7.1 step 9) ≠ item → 3; then
`0x00578640`: if the item lacks flag 0x10, identify it (`0x00562590`).
No 0x2A.

### 6. Refresh

1. **Leaving a town level** (`0x00537340(from, to)`), called whenever a
   player's room level changes (`0x005380D0`) and on act warps
   (`0x0053AEC0`): if `from` is 1, 40, 75, 103 or 109 (act 0–4 town),
   count living players whose room is in `from` (`0x005538D0`; the
   leaving player is already in `to`) and run rule 3 for that act with
   "empty" = (count = 0). Then, for `to` 1, 40, 75, 109, the quest
   intro hook (`quests.md` §6.7).
2. **Client leaving the game** (`0x00537580`, from `0x00539DA0`; skipped
   for game type 3): if the player's room level is 1, 40, 75 or 103 (not
   109), count as in rule 1 minus 1 and run rule 3.
3. Rule 3 (`0x00537230(act, empty)`), for every record with act = act,
   trader = 1, has traded = 1, store generated = 1:
   - not empty: if record +0x28 + 240000 < `GetTickCount()` (unsigned),
     set refresh pending (+0x27) and +0x28 := now;
   - empty: NPC unit (GUID +0x40) missing or of another class → clear
     the record's data (rule 4), store generated := 0, +0x28 := 0, new
     empty inventory; else if the NPC's interaction list is empty →
     same (`0x00536D10`, also assigned to the NPC unit); else refresh
     pending := 1.
4. Clearing a record's data (`0x00536580`): every gamble list (items
   removed, inventories freed), every store item removed (`0x00536510`),
   every event node freed (deferred items destroyed).
5. A pending refresh is applied at the next trade open with a single
   interacting player (§4 step 3). Otherwise the store stays as it is:
   re-talking does not regenerate it.

Single player: walking or warping out of town empties the town, so the
next trade there generates a new store (new draws); the 240 000 ms
timer only matters while another player stays in town.

### 7. Buying and selling

#### 7.1 Buy (C→S 0x32, 17 bytes)

Layout: NPC GUID u32 @1, item GUID u32 @5, u32 @9 = transaction t (bits
0–15) | unused (bits 16–30) | fill (bit 31), u32 @13 = client price
(**never read**). What the 1.14d client writes in bits 16–31: the
item's mode (unit +0x10, u16) shifted left 16 (`0x004B2713`,
`0x004B2760`–`0x004B2763`), then bit 31 OR-ed for fill; store items are
in mode 0, so bits 16–30 arrive 0 (the server never reads them either
way). Handler `0x0054BAC0` → `0x00577F30`: NPC missing or
not the player's interact unit → 0x2A code 9, result 1. Then
`0x00577830`:

1. Item missing → code 7, GUID = requested, result 1.
2. t = 0: item must be in the NPC's inventory; t = 2: the player's
   gamble list here must exist and hold the item; else code 7, result 1,
   GUID = requested. Other t values skip this test (edge case 3).

   GUIDs and kinds of every 0x2A of buy, sell and repair (handoff
   `impl-vendors` V10, pushes before each `0x0053D740` call in
   `0x00577830`, `0x00577F30`, `0x00579510`, `0x00578050`): rules 1–2 of
   §7.1 carry the requested item GUID; the successes carry the GUIDs
   named in §7.1 (kind 4 / 5), §7.2 rule 10 (kind 3) and the repair code
   2 (kind 1, GUID −1); every other 0x2A (codes 7 cursor, 9, 10, 11, 12
   in buy, sell and repair) has kind 0 and GUID −1.
3. price = cost(t) (§9) with the NPC's class.
4. gold + stash < price → code 12, GUID −1, result 0.
5. Player has a cursor item → code 7, GUID −1, result 1.
6. fill := 0 unless the item is a permanent item of this NPC, or in
   Nightmare / Hell `hp4`, `hp5`, `mp4`, `mp5` (`0x00576ED0`).
7. **Scroll into a tome**: item type 22 (scroll) and a backpack tome
   (type 18, page 0, matching, quantity < max; `0x0055F640`) with free
   space f: k = 1, or with fill k = min((gold + stash) / price, f)
   (unsigned division); pay k·price (§9.1; fail → code 12); on-buy hook
   (step 12); tome quantity += k (`0x0055F6E0`); 0x2A code 0, kind 5,
   GUID = tome. Done.
8. **Fill a stack**: fill and `itemtypes.autostack` (u8 +19): unit price
   u = cost(0) with the item's quantity temporarily 1; a = (gold +
   stash) / u (unsigned); find a partial stack of the same item on the
   player (`0x00577700`: equipped first, then backpack; free space f),
   else f := max stack. k = min(f, a). Stack found: k < 1 → code 12;
   pay k·u (fail → code 12); hook; stack quantity += k; 0x3E (stat 70);
   0x2A code 0, kind 5, GUID = stack. Done. No stack found: n :=
   max(k, 1); price := n·u; continue.
9. Purchase loop. fill := 0 unless the item can go to the belt
   (`0x00628BA0`). Each pass:
   1. after one purchase without fill → done (result 0);
   2. copy the store item (§7.3, `0x0055A2A0`, fillers 1); null → code
      9, result 1;
   3. step 8's n is set as the copy's quantity (fill, stackable);
   4. pay price (§9.1); fail → code 12, result 0 (copy not freed);
   5. player data +0x6C := copy GUID; copy mode := 4;
   6. belt-able → put in the belt (`0x0055E9B0`); fail → fill := 0;
   7. not placed: the equip try (§7.1.1) when it applies; success →
      step 8. Else after a first purchase → undo this pass's gold
      change, destroy the copy, done silently; else page := 0 and
      auto-place (`0x00560200`, find a free position, send); fail →
      undo gold, destroy, code 10, result 0;
   8. on-buy hook (step 12); copy flag 2 := 1; 0x2A code 0, kind 4,
      GUID = copy.
10. Recorded: `32 06000000 12000000 00000000 38000000` (Charsi, `dgr`
    GUID 0x12, t 0) → 0x2A code 0, kind 4, GUID 0x36, gold 444; next
    frame 0x9C action 12 for GUID 0x12 and action 4 for GUID 0x36.
11. The client price at @13 (0x38 = 56 above) is not compared.
12. On-buy hook (`0x00576F50(item, t)`): t = 2 and the record has gamble
    lists and the item is in the player's list → take it out of that
    list (`0x00576650`), done. Else permanent items (step 6 set) stay;
    any other store item is taken out of the NPC grid (`0x005766D0`:
    removed, unit +0xC8 |= 0x10, re-added to the trade inventory so the
    client drops it). "Store item" is the item the 0x32 named (the hook
    gets the source, not the copy), whatever list holds it; "permanent"
    is exactly `0x00576ED0` (handoff `impl-vendors` V13).

##### 7.1.1 Equip try at buy (`0x00577D18`–`0x00577D9A`)

W := the weapon in use (`0x0063BEF0`: the right-hand item, else the
left-hand item, that is type 45 `weap` and whose GUID is inventory
+0x1C; else none). h := the player's hand class (`0x00623C60`,
`skills/bodies.md` §2 `bow_missile`: weapon class index from items `wclass` through
table `0x007446A0`, 1 = `bow`, 7 = `xbw`, 0 = none). The class ids of
`cqv` and `aqv` are the globals of `world/npc.md` §1.1 step 3
(`0x00883E9C` = `cqv`, `0x00883EA0` = `aqv`, read at `0x00536102`).

| W | Copy | Try |
|---|---|---|
| none | `cqv` or `aqv` | no |
| none | anything else (weapons too) | yes |
| present | type 45 (`weap`, equivalence) | no |
| present | `cqv` and h ≠ 7, or `aqv` and h ≠ 1 | no |
| present | anything else (quivers matching h, armor, jewelry) | yes |

Try = `items/inventory.md` §4.9 (`0x00562E00(copy, skip 0)`): the
auto-equip test §4.7 picks the location; a free matching slot equips
the copy (command flag 0x200, 0x9D action 6). Result 0 (no slot, a
requirement failing) → the "not placed" path of step 7. Quivers are not
weapons (`bowq` / `xboq` → `misl` → `misc`, live `itemtypes.txt`).

#### 7.2 Sell (C→S 0x33, 17 bytes)

Layout: NPC GUID u32 @1, item GUID u32 @5, item mode u16 @9, u32 @13
client price (not read). Handler `0x0054BB20` → `0x00579510`:

1. Item missing → result 1, **no message**.
2. NPC missing or not the interact unit → code 9, result 1.
3. Item not the player's (`0x00557FF0`) → code 11, result 3.
4. Item mode (unit +0x10) ≠ u16 @9 → code 9, result 3.
5. Flag 0x1000 set, or a quest item (u8 +298 ≠ 0) or of type 39
   (`0x0062A130`) → code 9, result 3.
6. price = cost(1) (§9) with the NPC's class (no `npc.txt` row → fatal,
   edge case 2).
7. Re-sellable unless: inferior `Cracked`; broken (0x100); type 7
   (ear); personalized (0x1000000); ethereal (0x400000); filled
   sockets; a unique whose `uniqueitems` flag byte +0x2C has the bit
   of mask `0x006CE270` (`0x00575FA0`); the player's vendor-chain node
   at this NPC is in gamble mode. The mask is entry 2 of the bit table
   `0x006CE268` (1, 2, 4, 8, …; read from the image): value 4, the
   `carry1` bit of the flag byte (`data/fields.tsv` uniqueitems
   `carry1`, bit 2 of +0x2C), so a carry-one unique (in 1.14d `uniqueitems.txt`: Gheed's Fortune,
   Annihilus, Hellfire Torch) is not re-stocked (handoff `impl-vendors` V1).
8. Re-sellable and not a permanent item / NM-Hell hp4-5 mp4-5
   (`0x00576ED0`): copy into the NPC (§7.3, `0x0055A2A0`, fillers 1; null → code 9,
   result 3); mode 4; store page (§3.1 step 3; 0xFF → destroy, no
   copy); place, page 1 → 2 retry (fail → destroy); placed: mark
   (§3.1 step 5), durability := max, quantity := max stack, and price
   := min(price, cost(1) of the restored copy).
   Exact (handoff `impl-vendors` V11, `0x00579510` after the placement
   `0x00560200`): stat 72 := the max durability (`0x00625E00`) and stat
   70 := `0x006295B0` = items `maxstack` (+0xE8) + total stat 254, capped
   at 511, for **every** placed copy, stackable or not (a non-stack item
   with `maxstack` 0 gets stat 70 = its stat 254, usually 0).
9. Take the item from the player by mode: 4 (cursor) → `0x0055EEA0`
   (fail → code 9, result 1); 0 (stored): scroll (type 22) lowers the
   matching scroll skill quantity by 1, a book (type 18) by its
   quantity when ≥ 0 (`0x00576E40`: skill quantity floor 0, right skill
   cleared when it was that skill, S→C 0x22), then item cell := page,
   item update message, removed (`0x0055DF10`); other modes → unequip
   (`0x00560CD0`; fail → destroy the copy, result 1, no message).
10. Receive price (§9.1); 0x2A code 1, kind 3, GUID = the sold item.

Recorded: `33 06000000 07000000 0400 0000 f4010000` (Charsi, `skc`, on
cursor) → S→C 0x42 then 0x2A code 1, kind 3, GUID 7, gold 500; the
copy appears next frame as 0x9C action 11, GUID 0x35.

#### 7.3 Item copy (`0x0055A2A0`, ECX game, EDX source S, owner, fillers)

The copy routine of every caller that needs a second item equal to an
existing one: buy (§7.1 rule 9.2), sell (§7.2 rule 8), cube outputs
(`world/cube.md` §7.3), hireling take (`items/inventory-moves.md` §7.23), NPC
socketing (`world/npc.md` §8.1); 17 call sites. The owner argument (stack
1) is not read in 1.14d. Result: the copy, or none.

1. R := S's room (`0x00620BB0`; none when S is not on the ground).
2. Write S as a **save-format** stream with children
   (`items/bitstream.md`, `0x006313E0(S, buffer, 0x400, save 1,
   children 1, alt 0)`) into a 1,024-byte buffer. A stream that would
   not fit gives length 0 and the read in step 3 fails.
3. Read the first record (`0x00558CB0`): peek its header (`0x0062E410`:
   flags, version, mode, location, item code → class; filled-socket
   count N; the `ear` flag 0x10000 maps to code `ear `); class outside
   the items table → none. Allocate a new item unit of that class in R
   at the stream's position and mode (`0x00555230`, `sim/units.md`; a
   new GUID). Decode the record into it (`0x0062E430`); a decode error
   or a missing record frees the unit (`0x00555600`) and the result is
   none. Then item flag 0x80000 (init) set, 0x2000 (in store) cleared
   (`items/generation.md` §1.4), replenish timers
   (`items/generation.md` §9 step 6: `0x00558530`, `0x00558580`).
4. Item flag 0x80000 set, 0x2000 cleared on the copy again.
5. fillers ≠ 0 and N ≠ 0: for each of the N child records in stream
   order: read it as in step 3 with no room (failure → result none;
   the copy and the children read so far are not freed); child mode
   := 4; socket it into the copy through `0x00562660(child GUID, copy
   GUID, &out, 0, 1, 0, 0)` (`items/inventory-moves.md` §7.19; result 0 →
   fatal assert, line 0xDD4); child item flags 0x80000 set, 0x2000
   cleared; child command flag 0x1 cleared (`0x00628170`).
   fillers = 0: the children are not read; the copy keeps the stream's
   socket flags and its stat lists but has no fillers.
6. S item flag 0x8000000 set.
7. Replenish: for stat 252 (`item_replenish_durability`) and then 253
   (`item_replenish_quantity`), total r ≠ 0 and no type-3 timer on the
   copy (`0x005415A0`) → a type-3 timer at game frame (+0xA8) + 2500 / r
   + 1 (`0x005417D0`; `sim/unit-events.tsv` rows `0x0055a4be`,
   `0x0055a500`; the handler is `sim/units.md` §6.5).
8. Per-item reset of the deferred-message bits (`0x005979B0`,
   `items/inventory-moves.md` §6.1 rule 4); command flag 0x1 cleared. Result:
   the copy.

What carries over is exactly what the save stream carries
(`items/bitstream.md` §2–§5): stats with `Save Bits` 0, values the clamp
changes (§1 rule 3) and unit state outside the item record (timers
other than step 7, owner links, unit flags) are not copied. No draw
decides any property of the copy (the stream holds the seeds, §4.1 rule
7), but the **game seed does advance**: each unit allocation of steps 3
and 5 (`0x00555230`, type 4) makes one game-seed step for the unit seed
(`0x00552DF0` at `0x0055530E`) and one for the item seed (`0x00552E90`
at `0x0055532A`), both on game +0xD0 (`sim/units.md` §2 rule 4,
`sim/rng.md` §5.3); the decode then overwrites the item's seeds from the
stream. So a copy with k fillers read costs 2 · (1 + k) game-seed steps,
copy first, children in stream order; the GUID (`0x00552EE0`) and the
rest of the routine draw nothing. The decode rules of
`0x0062E430` (`0x0062CBE0` full record, `0x0062A970` compact) are the inverse
of `items/bitstream.md` except for the fields of §7.3.1.

#### 7.3.1 Fields the decoder rebuilds (Open question 8)

The copy passes save version 0x60 (`0x0055A2FE`), so none of the
decoder's old-version conversions apply. The decoder entry (`0x0062E430`)
stores the stream's flags without 0x2000000 and 0x80000 and zeroes the
affix and rare-name slots before the record is read. Not read from the
stream but rebuilt:

1. Full record (`0x0062CBE0`), weapon (type `weap`): stat 68 := −items
   `speed`; stats 22, 21, 24, 23 :=
   `maxdam`, `mindam`, `2handmaxdam`, `2handmindam`, and, when
   `maxmisdam` ≠ 0, 159 := `minmisdam`, 160 := `maxmisdam` (unit base
   set `0x00627260`). Quality 1: each is ⌊3v / 4⌋ instead, then 22, 24,
   160 at least 2 and 21, 23, 159 raised from 0 to 1. Item flag
   0x400000 (`0x0062A8D0`): each of the six := base × 3 / 2 (signed).
   Durability (73, then 72 when 73 ≠ 0) is read.
2. Full record, armor (type `armo`): stat 20 := items `block`, 67 :=
   −items `speed`; defense (31) and durability are read.
3. While the stat lists are read: an entry for stat 17 (read with its
   pair 18) first raises base 22, 24 and, for a throwable item
   (`0x0062BA80`), 160 to the items column when the base is below it,
   and before 18 raises 21, 23, 159 the same way (`0x0062C9F0`, a
   "raise when below" form of `items/properties.md` §4.3); an entry for
   stat 57 (read with 58 and 59) sets stat 326 (`poison_count`) := 1 in
   that list.
4. Item level: a read value < 1 → 1. Unique (quality 7): a file index ≥
   the uniqueitems count → −1.
5. Compact record (`0x0062A970`): item level := 1, quality := 2, unit
   seed field +0x28 := 0 and the item seed initialised from it
   (`0x00650E40`), suffix slot 0 := 0 for `tsc ` and 1 for `isc `.

So a copy differs from S (whose base values come from
`items/generation.md` §6 and `items/quality.md` §6) in: the item level
and item seed of a compact item; stat 326 when S's list holds a value ≥
2 (two poison properties added to one list, `items/properties.md` §4.2);
the base damage of a low-quality weapon whose runeword list holds stat
17 or 18 (the runeword's own reset acts on the filler, §10.2 there, so S
keeps ⌊3v / 4⌋ while the copy gets the full column); any weapon base
damage S holds other than the values of rule 1 (e.g. the craft list
re-applying × 3 / 2 to an ethereal weapon, `items/properties.md` §12).
The low-quality missile floors differ from creation (creation 159 ≥ 2,
160 ≥ 1; decode 159 ≥ 1, 160 ≥ 2), but no live `weapons.txt` row with
`maxmisdam` ≠ 0 reaches a floor.

Recorded: the buy of rule 10 creates the copy GUID 0x36 from store item
0x12; the sell of §7.2 creates GUID 0x35 from GUID 7 (each the next
item GUID; `sim/units.md` numbering).

### 8. Repair

#### 8.1 C→S 0x35 (17 bytes)

Layout: NPC GUID u32 @1, item GUID u32 @5, u16 @9 (not read), u32 @13:
bit 31 = repair all. For a one-item repair the 1.14d client writes the
item's mode at @9 and the item's total stat 72 (durability, `0x00625480`
at `0x004B27F1`) as the u32 @13, so bit 31 is clear unless the
durability is ≥ 2^31. Handler `0x0054BB60` → `0x00578050`:

1. NPC missing or not the interact unit → code 9.
2. Class not charsi 154, fara 178, hratli 253, halbu 257, larzuk 511 →
   code 9.
3. **Repair all**: total = sum over the 13 body locations
   (`0x0062FE60`) of cost(3) for each equipped item that needs it:
   repairable and durability-applicable with 0 < durability... no:
   max ≠ 0 and durability ≠ max; or a replenishable stack below max;
   or has charges not all full. total = 0 → code 2, kind 1. Else pay
   (§9.1; fail → code 12, kind 0); repair each such item (§8.2, with
   the player); code 2, kind 1.
4. **One item**: item missing or not in the player's inventory → code
   9, result 3. Not repairable (§9.2 rule 0) → code 9. If the item is a
   non-ethereal throwable stack below max, go on; else it must have
   charges not all full, or max durability ≠ 0 and durability < max;
   otherwise code 9.
5. c = cost(3). Pay c → success: repair (§8.2), code 2, kind 1.
6. Not enough gold: g = gold (stat 14). If durability < max: per =
   (c·1024) / (max − durability) (unsigned); if g > 0, 0 < per < g·1024
   and the item is not broken: pay g (inventory gold only), durability
   := min(max, durability + g·1024 / per), S→C 0x3E (stat 72), code 2,
   kind 1. Otherwise code 12, kind 0.
7. Results (handoff `impl-vendors` V12): the handler `0x0054BB60`
   returns 3 for a size ≠ 17 and **0 for every 17-byte message**: it
   drops the result of `0x00578050`. That routine's own results (not
   seen by the host): 1 for rules 1, 2, 4's "not repairable" and
   "nothing to repair" refusals and repair-all's failed payment; 3 for
   rule 4's "missing or not in the inventory"; 0 for every repair and
   every rule 6 outcome. Every 0x2A of this section has GUID −1.

#### 8.2 Repairing an item (`0x005761C0(item, player)`)

Only if repairable (§9.2 rule 0): throwable and stackable → quantity
:= max stack (0x3E stat 70 to the player when given); recharge
(`0x0055FE80`); broken (0x100) → `0x0055F900` (item spec), done; else
durability := max durability when > 0 (0x3E stat 72 when a player is
given).

### 9. Prices

#### 9.1 Paying and receiving gold

Pay (`0x00576D90(player, c)`, signed): g = stat 14, s = stat 15. g + s <
c → fail. c ≤ g → g −= c. Else g := 0 and s += g − c; if that is < 0, or
the player's stash cap (`0x00623460`) is below it, s := 0.

Receive (`0x0055B060(player, a)`): cap = carried-gold cap
(`0x00622E70`, player spec). g = cap → the whole amount is dropped as a
gold pile at the player (`0x0055A090`); g + a > cap → g := cap, the rest
dropped; else g += a (unsigned compares).

#### 9.2 Item value (`0x0062EFB0`, wrapper `0x0062FDC0`)

cost(t) for player P, item I, difficulty d, P's quest record of d, NPC
class, transaction t (0 buy, 1 sell, 2 gamble, 3 repair). Values S
(what the NPC sells for), B (what it pays), R (repair). "x/1024" is
signed division truncating toward zero; "guard(x, m)" means: if x ≥
0x10000 and m ≠ 0 then (x/1024)·m else x·m/1024.

0. I null or not an item → 0x7FFFFFFF. t = 3 and not **repairable**
   (`0x0062E660`: flag 0x10 set, not ethereal, and either charges not
   all full, or `itemtypes.repair` (u8 +8) set and (replenishable stack
   — `repair`, `throwable`, `stackable`, not ethereal — or
   durability-applicable)) → 0. Flag 0x20000 → 1.
   Durability-applicable (`0x00629930`): `nodurability` = 0,
   `durability` ≠ 0, max durability ≠ 0, stat 152 < 1.
1. qty = max(stat 70, 1). rp = player stat 87 (reduced prices), ≥ 99 →
   99. t = 2 → §9.4.
2. Base (cost = u32 +224; div := 1):
   - flag 0x10000 (ear): S = B = (ear level & 0xFF)·cost, R = 0;
   - type 40: S = B = cost + 8·`monstats` level of difficulty d (u16
     +170 + 2d) of the item's file index; cost alone without a row; R = 0;
   - type 18 (book): S = B = cost + qty·`books.costpercharge` of the
     item's suffix 0 (missing → fatal); R = 0;
   - ammunition (`itemtypes.quiver` ≠ 0): S = B = cost·qty/1024, R = max
     stack·cost/1024;
   - else S = B = R = cost; stackable (u8 +306): div := max stack if ≥ 2.
   - Then any type 50 (armor): AC = base stat 31; if max AC − min AC ≠
     −1 and max AC ≠ 0: S = B = R = cost·AC / max AC.
3. mt = quality 4–9. Not mt → item-skill costs (A).
4. Identified (flag 0x10) only, else go to rule 6: deltas from the
   automagic affix (`magicprefix` table row of the item's auto affix;
   multiply +136, add +140): dX = add + guard'(X, mult) for X = S, B, R,
   where guard' tests S for all three. Then by quality:
   - 1 inferior: dS, dB, dR := −(S/2), −(B/2), −(R/2) (replacing);
   - 3 superior, 9 tempered: bonus-stat costs (B), then apply;
   - 4 magic: + prefix 0 and suffix 0 affixes (same form); (B); apply;
   - 5 set: + `setitems` costadd (+60) / costmult (+56) of the file
     index; apply;
   - 6 rare, 8 crafted: + prefixes 0–2 and suffixes 0–2; (B); apply;
   - 7 unique: file index ≥ 0 with a `uniqueitems` row: + costadd
     (+128) / costmult (+124); apply; no row → as magic;
   - other: apply.
   "apply": S += dS/div, B += dB/div, R += dR/div (signed); (B) adds to
   S, B, R before the deltas are applied. mt → item-skill costs (A)
   after.
   Empty slots (handoff `impl-vendors` V2): every affix slot (auto,
   prefix i, suffix i) is looked up with `0x00633EE0`, which returns none
   for id 0 or an id above the affix count; a slot without a record adds
   nothing (no `add` either), slot by slot, and the other slots still
   count.
5. —
6. Sockets: for each item in I's inventory, cost/2 is added to S, B, R
   (`0x006292F0`). "cost" is the filler's items record `cost` (+0xE0 of
   the record of its class, unit +4), halved with signed truncation; no
   price of the filler is computed (handoff `impl-vendors` V5).
7. Ethereal: B /= 4. `itemtypes.class` (u8 +33) < 7: B /= 4.
8. t = 1: ethereal, durability-applicable, durability < 1 → B := 0.
   t = 3, not ammunition, not throwable, durability-applicable: max = 0
   or max ≤ durability → R := 0; else stat 252 = 0 → R := (max −
   durability)·R / max; else durability < max − 1 → R := (max − 1)·R /
   max; else R := 0.
9. `npc.txt` row of the NPC class (`0x00656900`; none → fatal):
   S := guard(S, sell mult); R := guard(R, rep mult); B: if B ≥ 0x10000
   and **sell mult** ≠ 0 then (B/1024)·buy mult else B·buy mult/1024.
   For each quest A, B, C with flag f ≠ 0 and quest slot f bit 0 or
   bit 1 set (`quests.md` §1): S := guard(S, questsellmult), B :=
   guard(B, questbuymult), R := guard(R, questrepmult).
10. Not book and not ammunition: S ×= qty. Not stackable or not
    repairable: B ×= qty. Else M = max stack (`0x006295B0`: `maxstack` +
    stat 254, at most 511): qty < M and stat 253 = 0 → R := R·(M − qty),
    B := M·B − R; else B := M·B, R := 0.
11. t = 3 and not ethereal: R += charged-skill costs (C) with base 10000.
12. B := min(B, `max buy` of d: +64 + 4d).
13. Result: t = 1 → B, or 1 if B ≤ 0. t = 3 → R − ratio(R, rp, 100)
    when rp ≠ 0; 1 if ≤ 0. Other t → S − ratio(S, rp, 100) when rp ≠ 0;
    at least 1.

ratio(v, p, d) (`0x00483360`): d = 0 → 0; v ≤ 0x100000: p ≤ 0x10000 →
v·p/d, else d ≤ p >> 4 → (p/d)·v; v > 0x100000 and d ≤ v >> 4 →
(v/d)·p; otherwise the 64-bit v·p / d.

**(A) Item-skill costs** (`0x0062EDD0`, only if `itemtypes.staffmods`
(u8 +31) ≠ 7): for each stat 107 entry (up to 64; skill = layer, v =
value), with `skills` cost mult m (+564) and cost add a (+568), k = 2v −
1: S < 0x10000 or m = 0: dS += (m·S/1024 + a)·k, dB += (B·m/4096 + a)·k,
dR += (R·m/1024 + a)·k; else dS += ((m/1024)·S + a)·k, dB += ((m/4096)·B
+ a)·k, dR += (R·(m/1024) + a)·k. Then X += dX / div, **unsigned**. An entry whose skill id (the layer,
u16) is ≥ the `skills` count is skipped: it adds nothing, not even `a`
(handoff `impl-vendors` V3).

**(B) Bonus-stat costs** (`0x00628E70`): for each stat entry (up to 511)
with bonus b = `0x00625560(I, stat, layer)` ≠ 0 and a valid stat: b >>=
`valshift`; by `encode` (u8 +48):
- 1: skill = layer; m, a from `skills`; guard on S·b: small → dS +=
  m·S·b/1024 + a, dB += B·m·b/4096 + a, dR += R·m·b/1024 + a; large →
  (S·b/1024)·m + a, ((B·b)/4096)·m + a, ((R·b)/1024)·m + a.
- 2, 3: skill = layer >> shift, u = layer & mask (data +0xC6C / +0xC70);
  same formulas with u in place of b, guard S·u > 0xFFFF.
- 4: (min, max) of the by-time value (`0x0065CA30`), u = (min + max)/2;
  `itemstatcost` multiply m (+16), add a (+20); all three /1024; guard
  on S·u. Exact (handoff `impl-vendors` V4): with v = b after the
  valshift, min = ((v >> 2) & 0x3FF) − 256 and max = ((v >> 12) & 0x3FF)
  − 256 (arithmetic shifts); u = (min + max) / 2, signed, toward zero
  (`0x0062914B`–`0x0062915E`).
- other: m, a from `itemstatcost`, all three /1024, guard on S·b.
Then X += dX / div, unsigned.
Missing `skills` row (handoff `impl-vendors` V3): encode 1 with skill
id ≥ the `skills` count → the entry is skipped (`0x00628F48`); encode 2
/ 3 with skill = layer >> shift ≥ the count → the record pointer is 0
and the next read (+0x234) faults (`0x00629070`, `0x00629084`): the
original crashes; unreachable with the 1.14d tables (the skill ids come
from the item's own stat list).

**(C) Charged skills** (`0x00628D30(I, base)`): for each stat 204 entry
with current = value & 0xFF < max = value >> 8: skill = layer >> shift,
lvl = layer & mask; t = lvl + 2 + (reqlevel/6)·2 (`skills.reqlevel` i16
+372); x = t·base; c = x < 0x10000 or m = 0 ? m·t·base/1024 :
(x/1024)·m; total += (max − current)·(c + a) / max.

Products are 32-bit signed and wrap.

#### 9.3 Vendor multipliers (live `npc.txt`, 1.14d)

| NPCs | sell mult | buy mult | rep mult | quest A (slot: sell / buy / rep) | quest B | max buy N / NM / H |
|---|---|---|---|---|---|---|
| gheed | 1088 | 512 | 128 | 4: 922 / 1024 / 1024 | — | 5000 / 30000 / 35000 |
| charsi | 960 | 512 | 128 | 4 | — | 5000 / … |
| akara | 1024 | 512 | 128 | 4 | — | 5000 |
| lysander, drognan, elzix, fara | 1024 | 512 | 128 | 9 | — | 10000 |
| hratli, alkor, ormus, asheara | 1024 | 512 | 128 | 17 | — | 15000 |
| jamella, halbu | 1024 | 512 | 128 | 41 | — | 20000 |
| malah, drehya, larzuk, nihlathak | 2048 | 512 | 128 | 41 | 35: 512 / 1024 / 1024 | 25000 |

Column note: the txt order is `buy mult, sell mult, rep mult`; the
values land at +8, +4, +12 (`fields.tsv`), so "sell mult" (+4) is the
NPC's selling price factor. Slot 41 is the respec slot (`quests.md`
§1.3): Den of Evil sets 41.1, so Act IV and V vendors give the 922
discount from then on in that difficulty (edge case 6).

#### 9.4 Gamble price

t = 2 (rule 1): if the item's format field (item data +0x30) is 0 →
`gamble cost` (u32 +212) of the item's normal-code record. Else the
record of the item's normal code (`0x00629370`), L = player level:
("normal code" = `0x006287D0`: items `normcode` (+0x84) when ≠ 0, else
`code`. Handoff `impl-vendors` V6: a normal code missing from the code
map gives no record; the format-0 branch then reads `gamble cost`
through a null record (fault) and `0x00629370` exits with the fatal
assert 0xB1A; the 1.14d normal codes are all present. The uber and
ultra tests are literally "≠ 0, ≠ `0   ` (0x20202030) and found"; the
store upgrade of §3.1 tests "≠ 0 and ≠ 4 spaces" instead.)

- `rin` or `amu` → `gamble cost`.
- st = max((minstack + maxstack)/2, 1) (u32 +228, +232).
- uber (ubercode ≠ 0 and ≠ `0   `, found): w_u = max((L − level)·100/2
  + 1, 0), c_u = its cost; ultra: w_x = max((L − level)·100/4 + 1, 0),
  c_x = its cost; else 0.
- L' = 5 if L < 6, else L.
- price = ((max(level − 45, 0) − level/2 + L')·250/3 + ((10000 − w_x −
  w_u)·cost·st + c_x·w_x + c_u·w_u)/10000) · ((2L' + 1)/3 + 20) / 15,
  with level and cost of the normal-code record.

Then − ratio(price, rp, 100) when rp ≠ 0 (no minimum).

## Constants & data dependencies

- Item record: code +128, cost +224, gamble cost +212, minstack +228,
  maxstack +232, bitfield1 +220, version +246, level +253, type +286,
  durability +274, nodurability +275, stackable +306, spawnable +307,
  quest +298, ubercode +136, ultracode +140, NightmareUpgrade +412,
  HellUpgrade +416, PermStoreItem +420, vendor columns 326–410
  (`fields.tsv`). `multibuy` (+421) is **not read** by the 1.14d vendor
  code; fill buying uses §7.1 step 6.
- `itemtypes`: repair +8, quiver +14, throwable +16, autostack +19,
  staffmods +31, class +33, storepage +34.
- `npc.txt` +4 sell, +8 buy, +12 rep, +16/20/24 quest flags, +28/32/36
  quest sell, +40/44/48 quest buy, +52/56/60 quest rep, +64/68/72 max buy.
- `difficultylevels` 1.14d: GambleRare 10000, GambleSet 100,
  GambleUnique 50, GambleUber 90, GambleUltra 33 (all three rows).
- Constants: store level caps 12/20/28/36/45; quality bands 90/85/74;
  failure limit 32; 14 gamble items; refresh 240 000 ms; repair-all over
  13 body locations; charged-skill base 10000; fixed codes `cqv`, `aqv`,
  `rin`, `amu`, `hp4`, `hp5`, `mp4`, `mp5`, `leg`, `Cracked`.

## Randomness

NPC-control seed (`npc.md` §1.1), shared by every NPC and player of the
game, in this order:

| Where | Draws |
|---|---|
| §3 per item entry (level ≤ ilvl) | range(Min, Max+1) if ilvl < 25 (0 or 1 draw); then per normal item: 1 step (quality), then §3.1 |
| §3 magic part | k: 1 draw if ilvl ≥ 25; range(MagicMin, MagicMax + k) (0 or 1); per item §3.1 |
| §3.1 per item (incl. permanent) | 1 roll(100000) only in NM/Hell with L_p > 25 |
| §5.1 per pass | 1 step (L_g); roll(T[L_g]) unless T < 1; uber roll(10000) and maybe ultra roll(10000) (expansion, positive weights); 1 step (quality) if H > 0 |

range() and roll() never draw for an empty range (`rng.md` §3 rule 1).
Item creation draws from item seeds (game-seed derived, `rng.md` §5.3).
Buying, selling and repairing draw nothing.

## Edge cases & original bugs

Reproduced by default.

1. Nihlathak's store list is Larzuk's column (§1 rule 4); he has no
   trade action, so it is only visible to tools.
2. Selling to an interacting NPC without an `npc.txt` row (e.g. Kashya,
   Warriv) reaches the fatal assert of §9.2 rule 9 (0x2A is never sent;
   the original process exits). d2rs: Open question 5.
3. Buy with t ∉ {0, 2} skips the "item is offered" test: any existing
   item GUID is copied and priced with cost(t) (t = 1 gives the sell
   price).
4. Buy: when payment fails inside the loop the copy is not destroyed;
   the client price field is ignored.
5. B uses the **sell** multiplier for its overflow guard (§9.2 rule 9).
6. `npc.txt` quest flag 41 of Act IV / V vendors is the 1.14d respec
   slot: the discount follows Den of Evil, not an Act IV / V quest.
7. Item-skill and bonus-stat deltas are divided unsigned: negative
   deltas with div ≥ 2 (stackables) become huge.
8. §3 step 3 makes the n_norm draw before skipping expansion items in a
   classic game; §5.1 step 4 overrides the first two picks after their
   draws.
9. Magic store items never count toward the 33-failure stop.
10. Refresh timer and store time use `GetTickCount` (wall clock): the
    only non-deterministic input; d2rs takes it as a host input in ms.
11. Selling an item that resells for less once restored (durability,
    quantity) pays the lower price (1.14d; D2MOO 1.10f has no such step).
12. Repair partial: only inventory gold is used, never the stash.
13. Unidentified items are not repairable (rule 0 requires flag 0x10).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| buy `dgr` (cost 60, normal, full durability, identified) from Charsi, Normal, slot 4 clear, rp 0 | 60·960/1024 = 56; gold 500 → 444 | recording frame 1279 |
| sell `skc` (cost 1000) to Charsi | B = 1000·512/1024 = 500 ≤ 5000 → 500; gold 0 → 500 | frame 1157 |
| buy `yps` (cost 40, permanent) from Akara | 40; gold 444 → 404; store copy stays | frame 1853 |
| Charsi store, L_p 1, Normal, expansion | codes in order `hax hax lax lax spc ssd scm scm dgr dgr tkf tkf jav jav spr spr bar bar sbw sbw hbw hbw ktr ktr cap cap skp qui qui lea hla buc buc sml sml lgl lgl lbt lbt lbl lbl aqv cqv`, all item level 6 | frame 899 |
| Gheed, item S = 1000 | 1062; with slot 4 bit 0 or 1: 956 | synthetic |
| Malah, S = 100 | 200; slot 41 set: 180; slots 41 and 35: 90 | synthetic |
| B = 100000, buy mult 512 | (100000/1024)·512 = 49664 (not 50000) | synthetic (guard) |
| B = 20000, Charsi, Normal | min(10000, 5000) = 5000 | synthetic (max buy) |
| `aqv`, 350 arrows, Charsi | S = 256·350/1024 = 87 → 81 (no ×qty) | synthetic |
| gamble `hax` (cost 170, level 3; 9ha 31/810, 7ha 54/14033), L = 1, 5, 10, 30, 60, 99 | 771, 771, 1656, 6896, 21552, 57986 | synthetic §9.4 |
| repair value R = 100, durability 5 of 20, no replenish | 75 | synthetic §9.2 rule 8 |
| quality draws, seed {1, 666}, ilvl 6, 5 items | 2, 2, 2, 3, 2; seed after {3217527747, 1278238622} | synthetic §3 step 4 |
| gamble level draws, seed {1, 666}, L_p 10 | lo' mod 10 = 1, 1, 2 → L_g 6, 6, 7 (other draws omitted) | synthetic §5.1 step 1 |

Game-file test (`#[ignore]`): build the 17 column lists from live
`weapons/armor/misc.txt`; Charsi's list holds exactly the entries listed
by its `Charsi*` item columns (§1; e.g. `aqv`, `cqv` permanent; `axe`
Min 1 Max 1 MagicMin 1 MagicMax 1 MagicLvl 1).

## Provenance

- 1.14d code: global lists `0x00536F80`, `0x00536D50`; per-game copy
  `0x00535FB0` from `0x00536070`; store `0x00576980`, `0x00576330`,
  `0x00576900`, `0x00576890`, `0x004BC500` (range helper, read from its
  instructions); trade open `0x00579430`, `0x00578B30`; gamble
  `0x00578790`, `0x005786A0`, `0x00578AE0`, `0x00537190`, `0x00578640`;
  refresh `0x00537340`, `0x00537580`, `0x00537230`, `0x00536580`,
  `0x00536D10`; buy `0x0054BAC0`, `0x00577F30`, `0x00577830`,
  `0x00577700`, `0x0055F640`, `0x00576F50`, `0x00576650`, `0x005766D0`,
  `0x00576ED0`; equip try `0x00577D18`–`0x00577D9A`, `0x0063BEF0`,
  `0x00623C60` → `0x00623990` / `0x00629FE0` (table `0x007446A0` read
  from the image: `bow` 1 … `xbw` 7, `ht1` 12), `cqv` / `aqv` lookups
  `0x00536102`; item copy `0x0055A2A0` (disassembled; 17 call sites),
  `0x00558CB0`, `0x0062E410` → `0x0062AE20`, `0x0062E430`,
  `0x00451F50`, `0x00620BB0`; sell `0x0054BB20`, `0x00579510`, `0x00576E40`,
  `0x0055B060`; repair `0x0054BB60`, `0x00578050`, `0x0062FE60`,
  `0x005761C0`; pay `0x00576D90`; prices `0x0062EFB0`, `0x0062EDD0`,
  `0x00628E70`, `0x00628D30`, `0x006292F0`, `0x00629370`, `0x00483360`,
  predicates `0x0062E660`, `0x00629930`, `0x0062E5D0`, `0x0062A840`,
  `0x006295B0`. Switch tables read from the `Game.exe` image
  (`0x00537064`/`0x00537098`, `0x00536454`/`0x00536488`).
- Draw arguments checked in the instructions: roll(100000) at
  `0x0057639E`/`0x0057640E`, roll(10000) at `0x0057871D`/`0x00578771`,
  range(1, 3) at `0x00576AF9`, quality `lo' mod 100000` at `0x00578980`.
- Recording `20261006-015956-packets.jsonl`: store contents decoded from
  S→C 0x9C (action 11, item code, item level, quality bits); prices from
  0x2A gold.
- Live tables: `patch_d2` `npc.txt`, `weapons/armor/misc.txt`,
  `difficultylevels.txt`; `d2exp` `gamble.txt`, `StorePage.txt`.
- D2MOO 1.10f (`SUnitNpc.cpp`, `SUnitProxy.cpp`, `Items.cpp`) as a map
  only. Differences found in 1.14d: buy core (empty in D2MOO) written
  from the 1.14d code; trade-open class test inverted in D2MOO; sell
  takes the restored copy's lower price; repair cost multiplies before
  dividing (D2MOO divides first); stack buy value M·B − R (D2MOO
  transcribes M − R); charged-skill term (reqlevel/6)·2 (D2MOO
  2·reqlevel/6); gamble level clamp ≤ 5 / ≥ 99 (same results);
  B's guard uses the sell multiplier in both.

## Open questions

1. `0x00625560` (stat "bonus" used by §9.2 (B)): exact definition
   belongs to the stat-list spec; confirm with one recorded magic-item
   price.
   Answered (definition; 2026-10-07): `sim/stats.md` §4.2 "unit
   bonus": `0x00625560(unit, s, layer)` = 0 when unit +0x5C (the stat
   list) is null, else the list total `0x00625420` minus the list base
   `0x00625350`, both with layer and each with its own minimum rule
   (disassembled). The recorded price check stays under OQ6.
2. Item format field (item data +0x30) that selects the gamble-cost
   column (§9.4): when it is 0 in an expansion game; check items created
   by §5.1.
   Answered (2026-10-07): never for a gamble-list item. §5.1 creates
   each item through `0x00559CE0` (`0x005789F9`, game = the list
   builder's ECX), which zeroes its request and stores the game's
   format (u16 game +0x78) at request +0x2A (`0x00559D28`–`0x00559D2E`);
   the creation pipeline copies +0x2A to item data +0x30
   (`items/bitstream.md` §3 rule 1), i.e. 101 in an expansion game, 2
   in a classic game (`items/generation.md` §1.2). So §9.4's format-0
   branch (`gamble cost`) is taken only for a format-0 item (one decoded
   from an old save), which no store list holds.
3. Answered from the code: §7.1.1 (`0x00577D18`). A recording still
   confirms it: buy `aqv` with a bow, then with a crossbow equipped, and
   a helm with the head slot empty (expect 0x9D action 6, no 0x9C
   action 4).
4. Order of §6 rule 1 versus the player's room change: the code passes
   the current room as `to`; confirm with a recording that leaving town
   and returning gives a new Charsi store.
5. Fatal path of edge case 2: decide d2rs behaviour (end the game like
   the original, or reject); a Ruleset decision, not a fidelity fact.
6. Price of a magic / rare / unique item: record a buy and a sell of
   one to confirm §9.2 rules 4–5 end to end.
7. Gamble list: record one gamble open (14 items, ring then amulet
   first) and one gamble purchase + 0x37.
8. §7.3 step 3: the decoder `0x0062E430` (full record `0x0062CBE0`, compact
   record `0x0062A970`) is read only as "the inverse of
   `items/bitstream.md`"; a field it rebuilds instead of reading (base
   stats from the item record, list values after the clamp) would make
   the copy differ from S. Settle: Ghidra on `0x0062CBE0`, or a buy of
   a socketed magic item compared stat by stat with the store item.
   Answered (2026-10-07, the exported decompile of `0x0062E430`,
   `0x0062CBE0`, `0x0062A970`, `0x0062C9F0`, register uses checked in
   the disassembly; the copy's version 0x60 at `0x0055A2FE`): §7.3.1.
   Rebuilt instead of read: weapon base damage / speed and armor block
   / speed from the items row (× 3 / 4 low quality, × 3 / 2 ethereal),
   the stat-17/18 base raise, stat 326 := 1 with stat 57, item level ≥
   1, a unique index past the table → −1, and for compact items item
   level 1, quality 2 and a seed from 0. The differences from S that
   follow are listed there; a recorded buy still confirms them.
9. §7.3 step 6: S's item flag 0x8000000 (set on every copied source)
   has no name in `items/generation.md` §1.4. Settle: the readers of
   item flag 0x8000000.
   Answered (2026-10-07): 1.14d has no reader. The only use of the
   immediate 0x8000000 on an item is this setter (`0x0055A476`,
   `0x006280D0(S, 0x8000000, 1)`); a scan of `all.asm` finds every other
   0x8000000 operand in MPQ / file-open flags (`0x00412E97`,
   `CreateFile` pushes), DRLG room flags (`0x0066D290`–`0x0066FC7B`)
   and the CRT, and no `test` / `and` / `push` immediate with bit 27
   set in the server range `0x00530000`–`0x0063FFFF` other than
   `0x0055A476` and masks that keep every high bit (`0x0053E66D` and
   0x8FFFFFFF). The bit is only carried in the item flags word (saved
   and streamed with it). d2rs keeps it as an opaque flag bit.
