# Spec: World — Horadric Cube (put in, transmute)

- **Status:** draft: every rule read from the 1.14d `Game.exe` routines
  listed in Provenance; recipe facts measured on the live `cubemain.txt` /
  `cubemain.bin` (151 records); no trace of a transmute recorded yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::cube`
- **Related specs:** `data/callbacks.md` §2, §3, §9 (cube input/output slot
  bytes, flag and kind values: owned there), `data/fields.tsv` (`cubemain`
  record, 328 bytes), `sim/intents-events.md` (dispatch, gate, result
  codes, message transport), `sim/client-messages.tsv`,
  `sim/server-messages.tsv`, `sim/rng.md` (§3 helpers, §5.3 unit/item
  seeds, §7), `world/quests.md` (quest items, quest flags),
  `world/npc.md`, `world/vendors.md`, `world/waypoints.md`; machine table
  `world/cube-ops.tsv` (§5).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–55 |
| Inputs | 56–68 |
| Outputs / state changes | 69–79 |
| Rules | 80–81 |
|   1. Routing | 82–98 |
|   2. Put an item into the cube (C→S 0x2A) | 99–145 |
|   3. Transmute entry (`0x005665F0`) | 146–158 |
|   4. Recipe eligibility | 159–173 |
|   5. Ops | 174–193 |
|   6. Input matching | 194–262 |
|   7. Outputs | 263–397 |
|   8. Commit | 398–466 |
|   9. Portals | 467–486 |
|   10. C→S 0x4C is not the cube | 487–501 |
| Constants & data dependencies | 502–525 |
| Randomness | 526–544 |
| Edge cases & original bugs | 545–579 |
| Test vectors | 580–624 |
| Provenance | 625–663 |
| Open questions | 664–772 |
<!-- /index -->

## Summary

The cube is an item (code `box `) that holds items on inventory page 3.
C→S 0x2A puts the cursor or ground item into it. C→S 0x4F with button 0x18
transmutes: the server counts the page-3 items, walks `cubemain` in record
order, takes the first enabled, eligible record whose seven input slots all
match, and runs its three output slots. If at least one output succeeds,
every page-3 item is removed, the transmute sound is attached to the
player and the outputs are placed in the cube. Item creation itself
(affixes, quality, unique/set picks) is specified by `items/generation.md`
(the request of §7.4 enters its pipeline, §3 there), `items/quality.md`
and `items/affixes.md`; this spec fills the request.
C→S 0x4C is **not** the cube (§10).

## Inputs

| Name | Type | Source |
|---|---|---|
| C→S 0x2A `item`, `cube` | u32 ids | `client-messages.tsv` |
| C→S 0x4F `button` | u16 (0x17 close cube, 0x18 transmute) | `client-messages.tsv`; `p1`, `p2` unused for these buttons |
| player interaction state | player unit +0x64 GUID, +0x68 unit type, +0x6C active byte | set to (cube GUID, 4, 1) by the cube's use function (§1) |
| player inventory items, page byte | item data +0x45 | page 3 = cube |
| `cubemain` records | 328 bytes each | `data/fields.tsv`, `data/callbacks.md` §2–§3 |
| game | expansion u32 +0x70, game type u8 +0x6A, ladder u32 +0x74, difficulty u8 +0x6D, item format u16 +0x78, unique bitset +0x1B24 (4096 bits) | written at game creation `0x00530BF0` (+0x70 = flags bit 20, +0x74 = flags bit 21, +0x78 = 101 if expansion else 2) |
| local date | day of month, day of week | `GetLocalTime` (import `0x006CC1D4`); d2rs: host-supplied input, read only by ops 1–2 |
| game seed, output item unit seeds | RNG | `sim/rng.md` §5.2, §5.3 |

## Outputs / state changes

- Cube contents replaced by the outputs (and freed socket fillers, `rem`).
- Unit sound event on the player (unit +0x6E = event, +0x70 = target,
  update flag 0x400 at +0xC4; `0x00553380`): 4 transmute, 19 refused put,
  20 Cow-portal refused.
- Game unique bitset bits cleared (`usetype` + `reg` on a unique, §7.4).
- Quest hooks for `hst ` and `qf2 ` outputs (§8); Cow portal object and
  game quest flag (§9).
- S→C messages: §8 and Rules §2; order rules there.

## Rules

### 1. Routing

| Event | 1.14d | Behaviour |
|---|---|---|
| open cube (use the cube item) | `0x005BF0C0` (`SkillItem.cpp`, item-use table entry `0x007417CC`; `misc.txt` `box` `pSpell` = 7) | If the player is interacting with the stash object (type 2, class 0x10B): clear the interaction (`0x00554190`), run `0x0055FA40`, queue 0x77 with 0x11. Then set interaction (type 4, cube GUID) through `0x00554120` (only if no interaction is active), queue 0x77 with 0x15, run `0x0055FA40`. |
| C→S 0x4F, any button | `0x00568060` | No active interaction (player +0x6C = 0): queue 0x77 with 0x0C, result 0. |
| C→S 0x4F button 0x17 | `0x00568060` → `0x00566AE0` | Interaction type ≠ 4 → result 3. Else reset it (GUID −1, type 6, active 0, `0x00554190`), then `0x0055FA40`; result 0. |
| C→S 0x4F button 0x18 | `0x00568060` → `0x00566AE0` → `0x005665F0` | Interaction type ≠ 4 → result 3. Type 4: transmute (§3); result 0. The GUID is not checked: any interaction of type 4 transmutes page 3. |
| C→S 0x2A | `0x0054B790` → `0x005628C0` | §2 |
| C→S 0x4C | `0x0054C760` | body-part transmogrify, not the cube (§10) |

`0x0055FA40` (inventory pass over stored items) and the 0x77 byte values
belong to the UI/inventory owner; listed here for message order only.
`server-messages.tsv` marks 0x77 `sim` (since PC 1 `e2fa8c2`: single
player sends it too, `sim/intents-events.md` §4 rule 4); these paths send
it in single player.

### 2. Put an item into the cube (C→S 0x2A)

Handler `0x0054B790`, size == 9 else 3.

1. Item check `0x00549350(item)` (shared with one other handler): item
   missing → 1; item mode > 4 → 1; mode 3 (on the ground): another act →
   2, distance test `0x00548EF0` (range argument 10) fails → non-zero;
   modes 0–2, 4: the item must be in the player's inventory or be its
   cursor item, else 1. Non-zero ends the handler with that result.
   The distance test's result is exactly 1 (handoff `server-items` SI1):
   `0x00548EF0` returns 0 when |unit x − item x| ≤ 10 and |unit y − item
   y| ≤ 10 (the unit's static-path position for types 2, 4, 5, else its
   path position; a unit without a path counts as (0, 0)), else 1.
2. Cube check `0x00549150(cube)`: the cube must exist, be in mode 0
   (stored) and be in the player's inventory, else result 1.
3. `0x005628C0`, with a "refused" flag cleared first:
   1. Targeting reset `0x0055BF50` (shared, 26 callers): every inventory
      item with item flag 0x4 gets it cleared; when `0x0044BE50` (unit
      type of its argument, 6 for none) returns 0, 0x3F is queued
      (`0x0053D220`, arguments 0xFF, 1, 0, 0xFFFF).
      Exact (handoff `server-items` SI3): the argument of `0x0044BE50` is
      the player whose inventory is walked (ECX at `0x0055BF98`), so the
      0x3F is sent for every reset item when the unit is a player (type
      0), to that player's client (`0x005531C0`), at once. Bytes (8,
      `sim/server-messages.tsv` 0x3F; builder `0x0053D220`): `3F FF`,
      the item GUID u32 @2, `FF FF` @6 (with the second argument 1 the
      code byte is 0xFF whatever the first; the third argument is not
      read). Only the inventory item list is walked (`0x0063B2C0`); the
      cursor item is not in it (`items/inventory.md` §1.4 rule 3) and is
      never reset here.
   2. Cube = item unit `cube`; it must exist, be mode 0 and have code
      `box ` (items.txt code, `0x00628590`), else refused.
   3. If the player is trading (interaction type 0 with a live unit,
      `0x005678A0`) and the cube's page is not 0 → sound event 19 on the
      player, return (result 0, nothing moved).
   4. Item = item unit `item`; missing → refused. Item mode must be 3
      (ground) or 4 (cursor), else refused. Cube mode re-checked (0), else
      refused.
   5. Item page := 3; place it with `0x00560200(game, player, item id, 0,
      0, 1, 1, 0)` (inventory placement; owner: inventory spec). The
      placement result is ignored; the routine returns success.
4. Handler result: refused → 3; otherwise 0.

Nothing in this path is cube-recipe specific; no item kind is forbidden
(a second cube can be offered; placement decides). Messages: 0x3F per
reset item (step 3.1), then whatever placement queues.

### 3. Transmute entry (`0x005665F0`)

1. n = number of player inventory items whose page is 3 (`0x00564FD0`).
   n ≤ 0 → nothing (no message).
2. Read the local date once (day of month, day of week + 1).
3. For record r = 0 … count−1 (`0x0066A200` count, `0x0066A1B0` record):
   if r passes §4 and its op (§5), run input matching (§6) with fresh
   capture slots (§6.4) and fresh "used" marks. On a full match run the
   outputs (§7–§8) and **stop**, whatever the outputs do. Otherwise try
   r + 1.
4. No record matched → nothing happens: no message, no sound, contents
   kept.

### 4. Recipe eligibility

Tested in this order on record bytes (`data/fields.tsv` offsets); the
first failure skips the record.

| # | Test | Field |
|---|---|---|
| 1 | `enabled` ≠ 0, or the global `0x0088CAC0` ≠ 0 (BSS, no writer in 1.14d: always 0) | +0 u8 |
| 2 | game is expansion, or `version` < 100 | +18 u16 |
| 3 | game type (+0x6A) ≠ 0, or game ladder (+0x74) ≠ 0, or `ladder` = 0 | +1 u8 |
| 4 | `min diff` ≤ game difficulty (unsigned bytes) | +2 u8 |
| 5 | `class` = 0xFF, or = player class id (unit +4, `0x00451F60`) | +3 u8 |
| 6 | `numinputs` = n (exact) | +16 u8 |
| 7 | recipe-scope op passes (§5) | +4 op, +8 param i32, +12 value i32 |

### 5. Ops

`world/cube-ops.tsv` lists every op: scope, subject, source, pass rule
and 1.14d site. Columns: `op` (u8 value), `scope` (`recipe` = tested in
§4 on the player; `input0` = a filter on candidate items of input slot 0;
`inputs` = a filter on candidates of every slot), `subject`, `source`,
`pass_if`, `address`. Ops 0, ≥ 29 and the input-scope ops pass at recipe
scope; recipe-scope ops are not tested on items.

Stat ops 3–26 share one guard: s = `param`. If s < 0 or s > the
`itemstatcost` record count (data +0xBD4) the op **passes** without a
test. Otherwise the record (`0x0045C4F0`, stride 0x144) is fetched; none
(s = count, an off-by-one in the range test) → the op **fails**. Else
t = `value` arithmetically shifted right by the record's `ValShift` byte
(+0x18), compared signed with the stat read by `0x00625480` (stat value),
`0x006253B0` (base stat) or `0x00625560` (stat bonus) with layer 0 (D2MOO
names `STATLIST_UnitGetStatValue`, `GetUnitBaseStat`, `GetUnitStatBonus`;
stat reading is owned by the stats spec). Op 1 compares signed: `param` ≤
day ≤ `value`. The live data uses op 28 only (records 0, 1, 2, 148, 149).

### 6. Input matching

`0x005658C0` calls `0x00565010` for slots k = 0 … 6 in order; any slot
returning false fails the record. Slot bytes: `data/callbacks.md` §2
(flags +0, item/type +2, unique/set + 1 at +4, quality +6, quantity +7).

#### 6.1 Per slot

1. Flags & 0x0003 = 0 (empty slot) → true.
2. need = quantity, or 1 if 0. found = 0; stack = false.
3. If flags & 0x0001 and item ≠ 0xFFFF: R = items record of the slot item.
4. Walk the player inventory in list order (first `0x0063B2C0`, next
   `0x0063DFA0`). The position counter p (1-based) counts **every** page-3
   item seen. For each page-3 item whose items record exists, apply §6.2;
   a failing test skips the item.
5. On a pass: if slot 0, apply §6.4 (capture); mark used[p−1] if p < 48;
   found += 1; if the item's items.txt `stackable` (+0x132) ≠ 0, stack =
   true.
6. Result: stack → found ≥ need; else found = need.

#### 6.2 Item tests, in order

| # | Test (skip the item if false) | 1.14d helper |
|---|---|---|
| 1 | flags & 0x0001: item 0xFFFF passes; with 0x0080 (`upg`) see §6.3; else item class = slot item. Else (0x0002): the item is of the slot's item type (type or type 2 through the itemtypes equivalence matrix) | `0x00629BB0` (owner: items) |
| 2 | slot quality ≠ 0 → item quality (item data +0, default 2) = slot quality. A mismatch with slot quality 9 calls the getter `0x00625290` (no effect) | `0x00627E70` |
| 3 | flags & 0x0040 → item file index (item data +0x28) = slot +4 − 1 | `0x00629DA0` |
| 4 | 0x0004 (`nos`): stat 194 (sockets) = 0; else 0x0008 (`sock`): stat 194 ≠ 0 | `0x006299B0` |
| 5 | 0x0020 (`noe`): not ethereal; else 0x0010 (`eth`): ethereal (item data flags +0x18 bit 0x400000) | `0x0062A8D0` |
| 6 | 0x0100 (`bas`): item code = its items record normcode (+0x84); else 0x0200 (`exc`): = ubercode (+0x88); else 0x0400 (`eli`): = ultracode (+0x8C) | code `0x00628590` (+0x80) |
| 7 | 0x0800 (`nru`): item flags bit 0x4000000 (runeword) clear | `0x00628110` |
| 8 | op 28 and the items record has `quest` (+0x12A) ≠ 0 and `questdiffcheck` (+0x12B) ≠ 0 → item stat 356 ≥ game difficulty | `0x00625480` |
| 9 | slot 0 only: output a kind is 0xFF or 0xFE and output a flags has 0x0002 (`sock`) → item max sockets ≠ 0 | `0x0062BC20` (owner: items) |
| 10 | p ≥ 48, or used[p−1] = 0 | — |
| 11 | slot 0 only: input-scope op 15–27 passes on this item (`cube-ops.tsv`) | — |

Only the first word of each pair (`nos`/`sock`, `noe`/`eth`,
`bas`/`exc`/`eli`) is tested when several are set.

#### 6.3 `upg` (flag 0x0080 with a slot item)

Let c = the cube item's code and N, U, L its own items record's normcode,
ubercode, ultracode; S = the slot item's code. c = N → pass iff S = N;
c = U → pass iff S ∈ {N, U}; c = L → pass iff S ∈ {N, U, L}; c is none of
them → fail. So `fhl,upg` accepts `fhl`, `xhl`, `uhl`; `xhl,upg` accepts
`xhl` and `uhl`.

#### 6.4 Capture for the outputs (slot 0 matches only)

Three capture entries j = 0, 1, 2 (item, class, level; 12 bytes each,
zeroed per record). For every item that passes slot 0, for each j:
level = item level (item data +0x2C; values < 1 are first stored back as
1, `0x006281E0`); then, all keyed on **output a** (slot 0) regardless of
j:

- output a flags & 0x0001 (`mod`) → item = this item;
- output a kind 0xFF (`usetype`) → item = this item, class = its class
  id; kind 0xFE (`useitem`) → item = this item, class = −1. For both:
  output a flags & 0x0080 (`exc`) → if the items record ubercode ≠ `    `
  and its index i is found (code linker, `0x006BD130`) and (game is
  expansion or record i `version` < 100) → class = i; else if flags &
  0x0100 (`eli`) → the same with ultracode.
  Exact (handoff `impl-world` C1, `0x0056572E`–`0x005657C4`): the `eli`
  test runs only when `exc` is clear; when `exc` is set and the uber
  upgrade fails (ubercode 4 spaces, not found, or the version test),
  the class stays as set above and `eli` is not tried.

With quantity > 1 the last passing item in list order wins.

### 7. Outputs

`0x00565AB0(game, player, record, capture, 1)`. Locals: success = 0,
craft = 1, out[3] = none, remove[3], fillers list (max 18). For slot j =
0, 1, 2 (`output`, `output b`, `output c`; slot bytes `data/callbacks.md`
§3), in order:

#### 7.1 Remove flag and level

1. remove[j] = flags & 0x0030 (`uns` or `rem`) ≠ 0.
2. Level L: `lvl` (+9) ≠ 0 → L = `lvl`. Else L = ratio(player stat 12
   (level), `plvl` (+10)) if `plvl` ≠ 0, plus ratio(capture[j].level,
   `ilvl` (+11)) if `ilvl` ≠ 0 (an empty sum is 0).
3. ratio(a, b), b = the u8 field: a ≤ 0x100000 (signed, negatives
   included) → (a·b)/100 in signed 32-bit, truncating toward zero;
   a > 0x100000 → (a/100)·b. The routine's other branches (64-bit
   division, (b/100)·a) need b > 0x10000 and cannot run here.
4. Clamp: L = max(1, min(L, M)) with M = max level of class 0 from
   `experience.txt` (`0x00611830(0)`, 99 in 1.14d).

#### 7.2 Portal kinds

kind 1–3 → call table `0x006E11C8` [kind] (§9) with (game, player);
**success := its result** (overwrites earlier slots' success). No item is
made for kinds 0–3.

#### 7.3 Item paths

| Case | Steps |
|---|---|
| flags & 0x0001 (`mod`, copy) | it = capture[j].item; page 0xFF, mode 4 (cursor); copy = duplicate(it, fillers = not remove[j]) (`0x0055A2A0`, `world/vendors.md` §7.3; D2MOO `ITEMS_Duplicate`); class by kind: 0xFC slot item; 0xFD type pick (§7.5) with L; 0xFE capture class if ≥ 0 else 0; 0xFF capture class; other 0. If the copy exists its class := that value. Item init `0x00557AB0(game, &copy, 0, 0)` (owner: items) — out[j] = the result; mode 4; it page := 3 |
| kind 0xFE (`useitem`) | it = capture[j].item; page 0xFF, mode 4; out[j] = duplicate(it, fillers = not remove[j]); mode 4; it page := 3. Capture class (`exc`/`eli`) is **not** used. Quality byte 9: prefix = `0x005C1BC0(out, 1)`, suffix = `0x005C1BC0(out, 0)` (tempered affix rolls, owner: items; `0x005C1BC0` takes ECX = item, EDX = 1 prefix / 0 suffix and is the rare-name pick by item format: format ≥ 1 → `items/affixes.md` §5 `0x005C1AB0`, format 0 → §12.2 `0x005C19A0`; both picks always run, prefix first (`0x00565EC7`, `0x00565ED3`), the same pair as the tempered case `items/affixes.md` §9, which owns the routine); both ≠ 0 → quality 9, rare prefix and suffix set (`0x00627EA0`, `0x00628010`, `0x00628070`); else craft := 0 |
| kind 0xFF, 0xFC, 0xFD | create through an item request (§7.4) |
| any other kind | nothing (out[j] none) |

Null cases (handoff `impl-world` C2, C3, `0x00565D61`–`0x00565ED3`):
`mod` or `useitem` with capture[j].item = none → the page and mode
setters skip none, then the duplicate `0x0055A2A0` asks the room of none
(`0x00620BB0`) → fatal assert (line 0x896): the original exits. It
needs output b or c with `mod` / `useitem` while output a has none of
`mod`, `usetype`, `useitem` (capture is keyed on output a, §6.4); no
1.14d record does that (checked over the 151 live records). A
`useitem` duplicate that returns none with quality byte 9 → the
tempered roll `0x005C1BC0(none, 1)` reads the format of none
(`0x0062A670`) → fatal assert (line 0x1504).

#### 7.4 Item request (`0x00558D90(request, 0)`, owner: item creation)

Request fields (D2MOO `D2ItemDropStrc` names; zeroed first, 0x84 bytes):

| Field | Value |
|---|---|
| pUnit (+0) | player |
| pGame (+8) | game |
| nItemLvl (+0x0C) | L |
| nId (+0x14) | class: 0xFF → capture[j].class; 0xFC → slot item (+2); 0xFD → type pick (§7.5) with L |
| nSpawnType (+0x18) | 4 |
| nX, nY, pRoom | 0 |
| wUnitInitFlags (+0x28) | 1 |
| wItemFormat (+0x2A) | game +0x78 |
| nQuality (+0x30) | slot quality byte (0 when not given) |
| nItemIndex (+0x40) | slot +4 (unique/set number + 1, else 0) |
| nPrefix[3] (+0x68), nSuffix[3] (+0x74) | slot `pre`, `suf` u16s, zero-extended |
| dwFlags2 (+0x80) | (flags & 0x0002 and quantity = 0) ? 0x10 (always sockets) : 0x08 (no sockets); OR (flags & 0x0004) ? 0x04 (always ethereal) : 0x02 (never ethereal) |

`usetype` with `reg` (0x0040) and a captured item: quality := the item's
quality, item index := its file index + 1, L := its item level (no
clamp); if quality = 7 and the game's unique bit for that index is clear,
remember the index and clear the bit again after creation. A unique that
already counts as found stays set; creating one that did not count does
not mark it.

#### 7.5 Item-type pick (`0x00565930`)

1. N = items record count. start = roll(game seed, N) (`sim/rng.md` §3,
   helper `0x0045C3E0`). stop = start − 1, or N − 1 if that is < 0.
   start = stop → 0.
2. i = start; while i ≠ stop: if fewer than 256 candidates and item i is
   of the type (`0x00629A90`) and `spawnable` (+0x133) ≠ 0 and (`version`
   (+0xF6) < 100 or game item format ≥ 100) and `level` (+0xFD) ≤ L →
   append i. i += 1, wrapping to 0 at N.
3. No candidate → 0. Else return candidate[roll(game seed, count)].

Item `stop` is never examined (original bug), and "no candidate" yields
item 0.

Exact (`0x005659B7`, handoff `impl-world` C5): the candidate test of
step 2 is not "append if fewer than 256": when 256 candidates are held
the scan **ends** (jump to the pick), so the later items are never
examined; the 1.14d type output (record 19, `pole` up to level 50)
never reaches 256. N = 0 (no items table) would loop for ever (i stays
0, stop is −1); the item tables always load, so this is unreachable.
No candidate → 0 without the second roll (one draw spent).

#### 7.6 After an item exists (out[j] ≠ none)

In this order:

1. success := 1.
2. remove[j]: drop the copy's runeword stat list (`0x00558C50`,
   `items/generation.md` §12.3); if
   flags & 0x0020 (`rem`) and the source item has an inventory: for each
   item in it, duplicate it (fillers on), page 0xFF, mode 4, append to
   fillers. (`uns` alone loses the fillers: the copy was made without
   them.) The fillers list is an 18-entry stack array with no bound test
   (`0x005661AC`–`0x005661BC`, handoff `impl-world` C4): a 19th entry
   would overwrite the frame's other locals (the craft-property buffer
   at ebp−0xE8, then remove[] and out[]). 18 is exactly three `rem`
   outputs of one captured item with 6 sockets, the 1.14d maximum, so
   the overflow needs a modded socket count.
3. craft ≠ 0: for mods m = 1 … 5 (slot +24 + 12(m−1): property i32, param
   +4, min +6, max +8, chance u8 +10): property < 0 → skip. If 0 < chance
   < 100: step the **output item's unit seed** (unit +0x20, inline draw)
   and skip when lo′ mod 100 > chance. Then add the property {property,
   param, min, max} (param, min, max sign-extended from 16 bits) with
   `0x00660240(out, &prop, game expansion)` (D2MOO
   `ITEMMODS_AddCraftPropertyList`; owner: `items/properties.md` §12).
   The third argument (game +0x70, pushed at `0x0056626B`) is never
   read: `0x00660240` uses only the item and the record (it calls the
   wrapper `0x0065FE10` with ESI = item, EDI = record, mode 7, flags
   0x40) and pops it (`ret 0xC`). So the craft mods do not depend on
   the game's expansion flag.
4. flags & 0x0200 (`rep`): if stackable and quantity ≠ 0 → stat 70
   (quantity) := min(quantity, max stack) where max stack =
   `maxstack` + stat 254, capped at 511 (`0x006295B0`). Then broken (item
   flag 0x100) → repair `0x0055F900` (`items/generation.md` §12.1); else if base stat 72 (durability) <
   stat 73 (max durability) → stat 72 := stat 73.
5. flags & 0x0400 (`rch`): recharge `0x0055FE80` (owner: items;
   `items/generation.md` §12.2, with `0x0065C940`).
6. flags & 0x0002 (`sock`): if quantity ≠ 0 and stat 194 = 0 and item
   flag 0x800 clear: s = min(max sockets `0x0062BC20`, quantity); quality
   4 or 9 → s ≤ 3; quality 5, 6, 7, 8 → s ≤ 1; s > 0 → set item flag
   0x800 and add s sockets (`0x0062BCB0`). Else (no `sock`): quantity ≠ 0
   and stackable → stat 70 := min(quantity, max stack).

### 8. Commit

After slot c: success = 0 → return. Nothing is removed and no message
is sent; outputs already made in out[] are neither placed nor freed
(leak, reproduced as "discarded"). success ≠ 0:

1. Remove contents (`0x00564F30`): for every inventory item in list order
   whose page is 3: queue 0x9D for it (`0x0053D010` → `0x0053CEF0`, action
   5, flags 0x20, page shown as the item's stored page, set to 3 first by
   `0x00628320`); then remove it from the inventory and free it
   (`0x0055DF10(game, player, item, 0)` → `0x00557FD0`). All page-3 items
   go, matched or not. `0x0055DF10` (`world/vendors.md` §7.2 rule 9):
   item unit flag 0x2 cleared, unlink `0x0063AD90`, grid cells cleared
   `0x0063BCF0`, page := 0xFF, freed; a null game or player, or an
   unlink that does not return this item, is a fatal assert (no return).
   The walk passes items of the player's own list, so the unlink never
   fails here.
2. Sound event 4 on the player.
3. For out[0], out[1], out[2] that exist: page := 3; place with
   `0x00560200(game, player, id, 0, 0, 1, 1, 0)`. Failed placement → free
   the unit (`0x00555600`), the output is lost. Placed → item flag 0x10
   (identified) set; if its items record `quest` ≠ 0: code `hst ` →
   `0x0059E5C0` (Act 2 Horadric Staff hook, `world/quests-act2.md`
   §4.9), `qf2 ` → `0x005B86E0` (Act 3 Khalim's Will hook,
   `world/quests-act3.md` §4.8); quest state changes: `world/quests.md`.
4. Fillers in order: page 3, place; failure frees, success sets
   identified.

Message order of a successful transmute: (1) 0x9D action 5 per removed
item, each followed by what removal queues; (2) per placed output, what
placement queues; (3) per placed filler, the same. The sound and the
quest hooks travel through their owners' messages. Exact bytes of (1)'s
removal part, (2) and (3) are open (question 1).

Exact (Open questions 1 and 2 answered from the code; a recording of V1
still confirms):

1. Removal: `0x0053D010` sends the 0x9D action 5 at once, while the
   0x4F is handled (page shown = the stored page, then page 3 restored;
   layout `items/inventory-moves.md` §11). `0x0055DF10(game, player,
   item, 0)` then sends nothing: unit flag 0x2 cleared, unlink
   `0x0063AD90`, the notice `0x0063BCF0` is an empty function, page :=
   0xFF, the fourth argument 0 skips `0x00571600`; `0x00557FD0` unlinks
   the item from any player inventory list or cursor still holding it
   (callback `0x00557FA0` → `0x00557F50` over the players) and frees it
   (`0x00555600`, which queues nothing for an item outside a room;
   handoff `server-items` SI2: nothing else the sim sees).
2. Placement: every `0x00560200(…, send 1, …)` first runs the targeting
   reset (0x3F per flagged item, at once), then sets command flag 0x2
   (and item flag 0x1 for a socket-filled item) and appends the item to
   the player's update list (`items/inventory.md` §2.4 step 8). Nothing
   is sent at the call. In the player's unit update of the same tick
   (`0x00580860`, per client, room update queue walk;
   `items/inventory-moves.md` §6.1 rule 2) the dispatcher sends one
   0x9C action 4 (`items/item-actions.tsv` row 3, owner's client only)
   per placed item, in update-list order: out[0], out[1], out[2], then
   the fillers; then the 0x47 / 0x48 of that rule.
3. Sound (question 2): `0x00553380(player, event, player)` stores the
   event at unit +0x6E, the target at +0x70 and sets unit flag 0x400. In
   the same `0x00580860` call, after the item messages and 0x47 / 0x48,
   flag 0x400 → `0x00571740`: S→C 0x2C (8 bytes, `2C`, unit type u8 0,
   player GUID u32 @2, event u16 @6; `audio/triggers.md` §2) to that
   client, only when the target is none or is the client's own player
   (`0x00537860`). The cube passes the player as target (`0x0056296C`
   event 19, `0x00566439` event 4, `0x00594253` event 20), so only the
   acting player's client hears it. The room clean-up clears flag 0x400
   (`items/inventory-moves.md` §6.1 rule 4): one sound per unit per
   tick, a later event in the same tick overwriting +0x6E.

### 9. Portals

The output kind indexes a 4-entry table at `0x006E11C8` (read from the
image): [0] none, [1] `0x00565A80`, [2] `0x00565A90`, [3] `0x00565AA0`.
Each entry is a 5-byte `jmp` thunk: to `0x00594140` (Cow portal), to
`0x00594270` and to `0x00594280` (Pandemonium, Pandemonium Finale). This
table is the only route to `0x00594140` (`world/quests.md` open question
3). Called with (game, player); the result becomes `success` (§7.2).

- **Cow portal** (kind 1, record 2: `leg` + `tbk`): conditions, the
  free-spot search, the portal object (class 60 → level 39) and the game
  quest flag are owned by `world/quests.md` §8.4. On failure it attaches
  sound event 20 to the player and returns 0 (`0x0059424B`), so the
  contents stay; on success the transmute commits (§8): `leg` and `tbk`
  are removed and sound event 4 follows.
- **Pandemonium portals** (kinds 2, 3; records 148, 149: enabled,
  version 100, op 28): both 1.14d targets return 0 at once. The transmute
  does nothing: no item, no sound, contents kept. D2MOO builds them behind
  `D2_VERSION_HAS_UBERS`; 1.14d has them stubbed.

### 10. C→S 0x4C is not the cube

`client-messages.tsv` describes 0x4C as "transmute the cube contents";
1.14d does not. Handler `0x0054C760`: size == 5; item = item unit
`u32@1`; missing and id ≠ −1 → 1. Else `0x0056C6A0`: clear player unit
+0xC8 bit 0x40; if an item was given and `0x00563FE0` succeeds → sound
event 4; always queue 0x3F (0xFF, 1, 0, 0xFFFF). `0x00563FE0` is the
misc.txt `Transmogrify` feature (`hrt`, `brz`, `jaw`, `eyz`, `hrn`,
`tal`, `fng`, `qll`, `sol`, `scz`, `spe` in 1.14d): the item must be
stored (mode 0) and its record `Transmogrify` (+0x139) ≠ 0; it builds a
request from the item, removes the item (`0x0055E000`), creates
`TMogType` (+0xC8) and sets its quantity to `TMogMin` + roll(player unit
seed, `TMogMax` − `TMogMin`) when both are > 0 (else 0). Owner: the
item-use spec; listed here only to fix the routing.

## Constants & data dependencies

| Item | Value / field | Source |
|---|---|---|
| cube code | `box ` (0x20786F62) | `0x0056293B` |
| cube page | 3 (item data +0x45) | `0x00564FD0` |
| interaction type "cube" | 4 | `0x005BF0C0`, `0x005680E7` |
| transmute / close buttons | 0x18 / 0x17 | `0x00566AE0` |
| used-mark array | 48 entries | `0x005658C0` (0xC0 bytes) |
| capture entries | 3 × 12 bytes | `0x005665F0` |
| filler list | 18 | `0x00565AB0` stack |
| type-pick candidates | 256 | `0x00565930` |
| item-type pick count | items record count (stride 0x1A8) | `0x00633590` |
| sound events | 4 transmute, 19 refused put, 20 cow refused | `0x00566439`, `0x00562965`, `0x0059424C` |
| quest-item difficulty stat | 356 | `0x005652E9` |
| stats written | 70 quantity, 72 durability (from 73), 194 sockets (read), 254 extra stack (read) | `0x00565AB0` |
| item flags | 0x10 identified, 0x100 broken, 0x800 socketed, 0x400000 ethereal, 0x4000000 runeword | as cited |
| max level | experience.txt `MaxLvl` of class 0 = 99 | `0x00611830`, live table |

`items.txt` fields read (all three item tables): code +0x80, normcode
+0x84, ubercode +0x88, ultracode +0x8C, version +0xF6, level +0xFD,
quest +0x12A, questdiffcheck +0x12B, stackable +0x132, spawnable +0x133.
`itemstatcost` ValShift +0x18. Cubemain fields: `data/fields.tsv`.

## Randomness

Draws the cube code makes, in order, per output slot a → b → c:

| Step | Seed | Draw | Decides |
|---|---|---|---|
| §7.3 copy path, kind 0xFD | game seed (game +0xD0) | roll(N), then roll(count) if count > 0 | class |
| §7.4 create path, kind 0xFD | game seed | roll(N), then roll(count) if count > 0 | class (before the request) |
| §7.6 step 3, per mod with 0 < chance < 100 | output item unit seed (+0x20) | one step, lo′ mod 100 | mod applied or not |

Order inside a slot: copy path = duplicate (its draws) → type pick →
item init (its draws); create path = type pick → item request (its
draws); then mod chances m = 1 … 5. Draws inside duplicate, item init,
item creation, tempered rolls, recharge, unit allocation (`sim/rng.md`
§5.3: game-seed steps per unit and item) and the Cow portal path belong
to their owners (question 4). The 1.14d data has no mod chance (all 133
mods have chance 0) and one item-type output (record 19), so in practice
the cube's own draws are record 19's two game-seed rolls.

## Edge cases & original bugs

All reproduced by default.

1. First matching record wins even if all its outputs fail; later records
  are not tried (§3).
2. A portal slot overwrites `success` (§7.2): an item made in an earlier
  slot followed by a failing portal consumes nothing and is discarded.
3. Failed transmutes discard already-made outputs without freeing them
  (§8).
4. Outputs that cannot be placed are destroyed after the inputs are gone
  (§8 step 3).
5. Capture uses output a's kind and flags for all three entries (§6.4): a
  `usetype` in output b with another kind in a gets class 0.
6. `useitem` without `mod` ignores the `exc`/`eli` upgrade (§7.3);
  `useitem,mod` without a valid upgrade gets class 0 (`hax`).
7. Quantity counts items, not stack sizes; with any stackable match the
  test is "at least" (§6.1). A slot marks **all** matching items used,
  so a later slot asking for the same item finds none (vector V4).
8. Type pick never considers the item just before its random start and
  returns item 0 when nothing qualifies (§7.5).
9. Mod chance passes when lo′ mod 100 ≤ chance: chance c gives (c+1)%
  (§7.6).
10. Stat ops: `param` = record count fails, `param` > count passes (§5).
11. Input quality 9 mismatch calls a getter with no effect (§6.2 #2).
12. `reg` keeps the source item's level unclamped (§7.4).
13. `rep` with `qty` refills stackables; rows 137–140 use `qty=255`, so a
  repaired throwing weapon gets its full stack.
14. 0x2A ignores the placement result (§2 step 3.5).
15. 0x2A with a ground item (mode 3, accepted by §2 steps 1 and 3.4)
   sets its page to 3 and then `0x00560200` refuses it (it places only
   mode-4 items, `items/inventory.md` §2.4 step 2): the item stays on
   the ground with page byte 3, handler result 0 (`0x005628C0`). In
   1.14d a ground item never enters the cube through 0x2A.

## Test vectors

Synthetic and live-data cases. "Live" = 1.14d `cubemain` record numbers
(0-based, = data line − 2); game = expansion, single player, Normal, game
type 0, ladder 0 unless stated.

| # | Cube contents / state | Expected | Source |
|---|---|---|---|
| V1 | 3 × `gcv` | records 0–22 fail (numinputs or inputs); record 23 matches → output `gfv`, L = 1 (no lvl fields), request quality 0, item index 0, flags2 0x0A; contents removed; sound 4 | live rec 23 |
| V2 | 3 × `gcv` + `gcr` | n = 4; no record matches; no message | live |
| V3 | 2 × `aqv` | record 21 → `cqv`. 3 × `aqv`: n = 3, no match | live rec 21 |
| V4 | recipe {numinputs 3; `"aqv,qty=2"`; `aqv`}, cube 3 × `aqv` | slot 0 matches all 3 (stackable, 3 ≥ 2) and marks them used; slot 1 finds 0 ≠ 1 → no match | synthetic |
| V5 | 2 magic `rin` + 1 rare `rin` | record 13 slot 0 found 2 ≠ 3 → fail; no record → nothing | live rec 13 |
| V6 | magic `xhl` + `jew` + `r06` + `gpb` | record 64 (`fhl,mag,upg`) matches; magic `uhl` also; magic `hlm` does not; classic game: record 64 skipped (version 100) | live rec 64 |
| V7 | `lvl` 30, `ilvl` 60 | L = 30 | live rec 15 |
| V8 | `plvl` 75, character level 40 | L = 30 | live rec 13 |
| V9 | `plvl` 40, `ilvl` 40, char 50, item level 80 | L = 20 + 32 = 52 | live rec 61 |
| V10 | `plvl` 66, `ilvl` 66, char 99, item 99 | 65 + 65 = 130 → 99 | live rec 62 |
| V11 | `ilvl` 100, item level 0 (stored back as 1) | L = 1 | live rec 60 |
| V12 | mod chance 0 or ≥ 100 | applied, no draw | rule |
| V13 | chance 50, lo′ mod 100 = 50 / 51 | applied / skipped | rule |
| V14 | `sock` quantity 6: quality 4 max 4 → 3; quality 7 max 6 → 1; quality 2 max 6 → 6; stat 194 = 2 → nothing | rule |
| V15 | type pick, N = 5, qualifying {1, 2, 4}, first roll 3 | stop 2; scan 3, 4, 0, 1 → [4, 1]; second roll(2) = 1 → item 1 (item 2 never seen) | synthetic |
| V16 | type pick, first roll 0, qualifying {4} | stop 4; scan 0–3 → none → item 0 | synthetic |
| V17 | 3 × `r14` + `gcg`, game type 0, ladder 0 | record 104 skipped (ladder); nothing | live rec 104 |
| V18 | disabled record 142 (`armo,hiq,nos` + `jew` + `r08`) | skipped | live rec 142 |
| V19 | `msf` (stat 356 = 0) + `vip` (356 = 0), Nightmare | op 28: 0 < 1 → slot fails; nothing. Normal → `hst`, quest hook `0x0059E5C0` | live rec 0 |
| V20 | `pk1` + `pk2` + `pk3` | record 148 matches; portal stub → success 0; nothing changes | live rec 148 |
| V21 | `leg` + `tbk` in Lut Gholein, quests done | record 2; Cow portal step 4 fails → sound 20, contents kept | live rec 2 |
| V22 | `leg` + `tbk` in Rogue Encampment, expansion, quest 40 flag 0 set, no cow flags | portal object 60 → level 39; game flag (4, 11) set; contents removed; sound 4 | live rec 2 |
| V23 | weapon dur 10 / max 50, not broken, + `r09` | record 137: dur := 50 | live rec 137 |
| V24 | 0x4F 0x18 with no active interaction | 0x77 0x0C; result 0 | rule |
| V25 | 0x2A with the item stored in the backpack (mode 0) | step 1 passes, step 3.4 refuses → result 3 | rule |
| V26 | stat op 3, param = itemstatcost count | fails; param = count + 1 passes | rule |

Live-data facts (check `data-tool`-style, Python over the 1.14d files):
151 records; `enabled` 146 (off: 142–146); `ladder` 21 (104–122, 131,
132); `version` 100 on 96; `op` 28 on 5, other ops 0; `min diff`,
`class`, `param`, `value` unused (0, 0xFF, 0, 0); `numinputs` = Σ max(qty,
1) over non-empty slots for all 151; only `output` (slot a) is used:
kinds 0xFC 78, 0xFF 48, 0xFE 21, 0xFD 1, portals 1 + 1 + 1; output flags
`mod` 8, `exc` 4, `eli` 4, `rep` 4, `rch` 2, `sock` 1, `uns` 1; 133 mods,
all chance 0; level modes: none 99, plvl+ilvl 38, lvl+ilvl 6, lvl 5,
plvl 2, ilvl 1.

## Provenance

1.14d `Game.exe` (1.14.3.71) exports in `re/exports/` (Ghidra C and
disassembly); data tables read from the executable image where the
exports have none (`0x006E11C8` portal table, `0x00566594` socket-cap
jump table, `0x00566AA8` op jump table, the two Pandemonium stubs at
`0x00594270`/`0x00594280`, the cube-use table `0x007417CC`).

- Routing: `0x00568060` (button dispatch, interaction read `0x00554100`),
  `0x00566AE0`, `0x005BF0C0`, `0x00554120`, `0x00554190`, `0x0053CAB0`
  (0x77 builder).
- Put: `0x0054B790`, `0x00549350`, `0x00549150`, `0x005628C0`,
  `0x0055BF50`, `0x005678A0`.
- Transmute: `0x005665F0` (eligibility, ops 1–14), `0x00564FD0`,
  `0x005658C0`, `0x00565010` (inputs, ops 15–28), `0x00565AB0` (outputs
  and commit), `0x00565930` (type pick), `0x00564F30` (removal),
  `0x00594140` (Cow portal).
- Helper identities checked in their 1.14d bodies: page `0x00628250`
  (+0x45), quality `0x00627E70`, file index `0x00629DA0`, level
  `0x006281E0`, sockets `0x006299B0` (stat 194), ethereal `0x0062A8D0`,
  stackable `0x006289F0`, max stack `0x006295B0`, code `0x00628590`, max
  level `0x00611830`, sound `0x00553380`, itemstatcost record
  `0x0045C4F0`, class `0x00451F60`.
- Game fields: `0x00530BF0`/`0x00530930` (creation writes +0x6A, +0x6D,
  +0x70, +0x74, +0x78, zeroes +0x1B24).
- **D2MOO 1.10f** (`PlrTrade.cpp`: `PLRTRADE_HandleCubeInteraction`,
  `CheckCubeInput`, `CreateCubeOutputs`, `RollRandomItemClassOfSameType`;
  `A1Q4.cpp` `ACT1Q4_CreateCowPortal`; `ItemMode.cpp`
  `D2GAME_Transmogrify`) was used as a map; each rule above was re-read
  in the 1.14d routine. Agreements: test order, flag semantics, level
  formula, request flags, mod-chance test, commit order. 1.14d
  differences: the Pandemonium portal functions are
  stubs returning 0; the overflow-safe ratio in §7.1 (D2MOO plain
  multiply-divide; same results for real values); D2MOO's 0x4C name
  matches 1.14d (transmogrify), the repo TSV text does not.
- Live data: `game/extracted/patch_d2/.../cubemain.txt` and `.bin`,
  `misc.txt`, `weapons.txt`, `armor.txt`, `experience.txt` (scratch
  Python, 2026-10-06).

## Open questions

1. Answered from the code (§8 "Exact", items 1–2; the V1 recording
   still confirms). Original text: Exact S→C bytes and order of a
   successful transmute: what placement
   (`0x00560200`), removal (`0x0055DF10` → `0x00557FD0`) and freeing
   (`0x00555600`) queue. Settle: packet recording of V1 (record 23) and
   V22.
2. Answered from the code (§8 "Exact", item 3: S→C 0x2C in the
   player's unit update, to the acting player's client). Original text:
   Which message carries the attached sound events 4/19/20 and in which
   tick pass. Settle: same recording (expect it with the player's unit
   update).
3. Single-player values of game +0x6A and +0x74 (ladder records usable or
   not). Settle: V17 in a recorded SP game, or read the game struct after
   creation.
   Answered (2026-10-07, `0x00530BF0` disassembled, plus the recorded
   single-player games of `sim/intents-events.md` §8.1 and its
   recording notes): game creation writes +0x6A := its game-type
   argument (`0x00530CFF`, the byte of C→S 0x67) and +0x74 := bit 21
   (0x200000) of its game-flags argument (`0x00530D4C`–`0x00530D59`;
   +0x70 = bit 20). The recorded single-player games show game type 3
   on every tick and S→C 0x01 u8@7 = 0 (u8@7 = (+0x74 ≠ 0)), so in
   single player +0x6A = 3 and +0x74 = 0: §4's test 3 passes through
   its first term, so `ladder` cube records are usable in single
   player.
4. RNG draws inside duplicate `0x0055A2A0`, item init `0x00557AB0`, item
   request `0x00558D90`, free-spot `0x00545340` and portal creation
   `0x0056D130`. Settle: rng trace (`check_rng.py`) of V1, record 19 and
   V22; the owners' specs.
   Partly answered (duplicate only): `0x0055A2A0` makes exactly two
   game-seed steps per item unit it allocates (unit seed `0x00552DF0`,
   item seed `0x00552E90`, inside `0x00555230`), i.e. 2 · (1 + k) for a
   copy with k fillers read, and no other draw (`world/vendors.md` §7.3).
   The other four functions stay open.
   Further answered: item init `0x00557AB0` and the item request
   `0x00558D90` draw as `items/generation.md` Randomness states (rows 1–2
   game seed for the unit and item seeds of each new unit, then base
   stats on the unit seed, then the item seed; the duplicate path's
   `0x00557AB0(game, &copy, 0, 0)` is the "no request" branch, §4 there);
   the free-spot search `0x00545340` draws nothing
   (`world/quests-act1-rest.md`, town Cain rule 2). Still open: the
   portal creation `0x0056D130` (free spot `0x0064E810`, object
   allocation `0x00555230`, mode set `0x00624690(obj, 1)` and the
   destination half `0x0056CF40`); owner: objects / quests specs, settle
   with `disasm.py fn 0x0056CF40` and an rng trace of record 2.
   Answered (2026-10-07; disassembly of both functions, plus a
   reachability scan of every callee for the step constant 0x6AC690C5
   and the draw helpers, `rng.md` §3/§6): portal creation draws, in
   this order, on these seeds:
   1. Free spot `0x0064E810` (only when the last argument is 0),
      cell lookup `0x00463740`: no draw.
   2. Object 1 (`0x0056D249`: `0x00555230`, type 2, the class
      argument, mode 1, flags 1, GUID 0): **one game-seed step** (unit
      seed `0x00552DF0`, `0x0055530E`; the item seed `0x00552E90` only
      for type 4; the GUID `0x00552EE0` is a counter). The init dispatch
      `0x0054F5D0` draws only for `PreOperate` ≠ 0 (rule 8; 0 for
      classes 59 and 60 in the live table); inits 11 (`0x00550140`) and
      12 (`0x0054FE70`) draw nothing (their `0x005417D0` scheduling
      draws only for a monster, `sim/tick.md` §5.2 rule 4; init 12's
      mode set runs only for mode 0); the object branch of
      `0x00554850` draws nothing.
   3. Mode set `0x00624690(obj, 1)` (`0x0056D25C`): the object is
      already in mode 1, so no animation setup and no draw
      (`sim/units.md` §4.1; the `roll` at `0x00624563` runs only on a
      real mode change).
   4. `0x0056CF40`: the spawn point `0x0061B060` may build the
      destination level (`0x0066B2B0`: level / DRLG room seed draws,
      `drlg/*`), then the room population `0x0052D0F0` (draws of
      `monsters/population.md`, on the game seed and the room seeds),
      both before the null test; for level 73 the A2Q6 arrival
      `0x00545830` → `0x0059DFD0` does the same for level 40; the free
      point `0x0064E7E0` draws nothing; the fallback `0x0061B060`
      (`0x0056D033`) runs only when that point is not found.
   5. Object 2 (`0x0056D092`: same class as object 1 (+0x04), mode 2,
      flags 1): **one game-seed step** (its unit seed), init as in 2
      (no draw: init 11 acts only in mode 1, init 12 only in mode 0).
   6. `0x00624690(obj2, 2)` (`0x0056D107`): same mode, no draw;
      `0x00553590`, `0x00621CE0` (twice), `0x00622300`, `0x0061AED0`:
      no draw.
   So outside the level build and the population the cube's portal
   costs exactly two game-seed steps, one before and one after
   `0x0052D0F0`. The failure paths free object 1 (`0x00555600`), whose
   draws are that function's. All five functions of this question are
   now answered.
5. Answered by the inventory spec: link order (`items/inventory.md`
   §1.4 rule 1: `0x0063AF20` appends at the tail). Original text:
   Inventory list order of `0x0063B2C0`/`0x0063DFA0` (decides capture
   with quantity > 1 and removal order). Owner: inventory spec.
6. Answered: `0x00560200` changes nothing on a failed placement
   (`items/inventory.md` §2.4 step 4), so after 0x2A the item stays
   where it was (the cursor, mode 4) with its page byte set to 3; a
   ground item is refused the same way (edge case 15).
7. Cube-use table base and index (`0x007417CC`, `pSpell` 7?). Owner:
   item-use spec.
   Answered (2026-10-07, `0x005BF240` disassembled, table read from the
   binary): the item-use table is at `0x00741790`, 31 entries
   (`0x0074178C`) of two words (first use, second use); the index is
   items `pSpell` (+0x94; for `book` / `scro` items the +0x04 word of
   the `books` row named by suffix slot 0 (`0x006374B0`) when it is >
   0), used only when 0 < index < 31. A first word is called while item
   flag 0x4 is clear; otherwise, or when it is 0, the second word is
   called after setting flag 0x4. Entry 7
   = (0, `0x005BF0C0`): the first word is 0, so the dispatcher takes the
   second word (`0x007417CC`). `box`
   is the only live `misc.txt` row with `pSpell` 7, so the cube-open
   routine is exactly entry 7, word 2.
8. Answered: `0x0055FA40` recounts the scroll/tome skill quantities (stored items on page 0 only); `items/inventory.md` §5.5.
