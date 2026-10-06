# Spec: Items — Inventory model and item-move intents

- **Status:** draft: grids, placement search, belt, equip checks, the
  deferred item-message dispatcher and every handler's validation order
  read from the 1.14d `Game.exe` (addresses below); grid sizes measured
  on the 1.14d `inventory.bin` / `belts.bin`; no recording replayed yet
  (test vectors T1–T9 are synthetic or table facts; R1–R6 need
  recordings).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::items::inventory` (grids, placement, belt,
  equip checks), `d2-sim::items::moves` (intent handlers, deferred item
  messages). Machine table: `items/item-actions.tsv` (§6).
- **Related specs:** `sim/intents-events.md` (transport, gate, size check,
  result codes, the two message TSVs); `sim/client-messages.tsv`
  (handler addresses, layouts); `sim/units.md` (unit record, modes);
  `sim/unit-order.md` §6 (room update queues: when deferred messages go
  out); `sim/tick.md` §6 (per-client update); `items/generation.md` §1.3–1.4
  (item getters, item flags); `items/properties.md` §9–10 (socket
  fillers, runewords); `items/treasure.md` §7 (drop start point);
  `sim/path-placement.md` (free-spot search, written in parallel);
  `world/cube.md` (C→S 0x2A, 0x4F, 0x4C, transmute); `world/vendors.md`
  (stores, sell/buy placement); `world/npc.md` §2 and
  `world/waypoints.md` (C→S 0x13 / 0x16 for non-item units);
  `world/quests.md` (item quest hooks); data layouts
  `data/fields.tsv` (`inventory`, `belts`, `itemtypes`, `weapons`/`armor`/
  `misc`).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 54–67 |
| Inputs | 68–78 |
| Outputs / state changes | 79–86 |
| Rules | 87–88 |
|   1. Inventory model | 89–181 |
|   2. Grid placement | 182–272 |
|   3. Belt | 273–330 |
|   4. Equipping | 331–485 |
|   5. Shared checks | 486–610 |
|   6. Deferred item messages | 611–704 |
|   7. Intents | 705–1090 |
|   8. Pickup from the ground | 1091–1191 |
|   9. Drop to the ground | 1192–1234 |
|   10. Gold | 1235–1265 |
|   11. Message layouts | 1266–1295 |
| Constants & data dependencies | 1296–1318 |
| Randomness | 1319–1331 |
| Edge cases & original bugs | 1332–1347 |
| Test vectors | 1348–1395 |
| Provenance | 1396–1442 |
| Open questions | 1443–1509 |
<!-- /index -->

## Summary

Every player, NPC, mercenary and item-with-sockets has an inventory: a
record holding a list of items, a cursor slot, a set of grids (body
locations, belt, one grid per page) and a list of items whose state the
clients must hear about. The item-move intents (C→S 0x16–0x29, 0x50, 0x61,
0x63) validate the request, move the item between ground, cursor, grids,
belt and body, set per-item "command flags" and put the item on the
update list. The messages (S→C 0x9C, 0x9D, 0x7D) are not sent by the
handlers: they go out in the next client pass, when the player's unit
update walks the update list (§6). This spec owns the model, the
placement and search rules, the belt and equip rules, the requirement
check, the item-move handlers and the deferred-message dispatcher.

## Inputs

| Name | Type | Source |
|---|---|---|
| intent bytes | C→S 0x16–0x29, 0x50, 0x61, 0x63 | `client-messages.tsv` layouts |
| inventory grids | `inventory.bin` records (gridX, gridY) | `data/fields.tsv` `inventory` |
| belt sizes | `belts.bin` `numboxes` | `data/fields.tsv` `belts` |
| item record | items `invwidth`/`invheight` (+0x10F/+0x110), `reqstr` (+0x10A), `reqdex` (+0x10C), `component` (+0x115), `belt` (+0x130), `autobelt` (+0x131), `quest` (+0x12A), `useable` (+0x11D) | `data/fields.tsv` |
| item type record | itemtypes `body` (+9), `bodyloc1`/`bodyloc2` (+10/+11), `beltable` (+25), `class` (+33) | `data/fields.tsv` |
| unit stats | strength 0, dexterity 2, level 12, gold 14, quantity 70, `item_req_percent` 91 | `sim/stats.md` |

## Outputs / state changes

Item unit: mode (`sim/units.md`: 0 stored, 1 equipped, 2 in belt, 3 on
ground, 4 on cursor, 6 socketed), item data page, body location, grid
node, command flags, item flags; inventory grids, cursor, item list,
update list; unit flags; S→C 0x9C / 0x9D / 0x7D / 0x3F / 0x42 / 0x47 /
0x48 (§6, §11); result codes (`intents-events.md` §2.3).

## Rules

### 1. Inventory model

#### 1.1 Records and fields

| Record | Offset | Meaning |
|---|---|---|
| inventory | +0x00 | signature 0x01020304 (every accessor checks it; mismatch → "none") |
| inventory | +0x08 | owner unit |
| inventory | +0x0C / +0x10 | item list first / last |
| inventory | +0x14 / +0x18 | grid array (16 bytes per grid) / grid count |
| inventory | +0x1C | GUID of the weapon in use (−1 none; `0x0063BEF0`) |
| inventory | +0x20 | cursor item |
| inventory | +0x28 | count of linked items |
| inventory | +0x2C / +0x30 | update list head / tail (nodes {GUID, next}) |
| grid | +0x00 / +0x04 | grid's own item list first / last |
| grid | +0x08 / +0x09 | width / height (u8) |
| grid | +0x0C | cells: width × height item pointers, row-major (cell (x, y) = y × width + x) |
| item data | +0x14 | command flags (§6) |
| item data | +0x18 | item flags (`items/generation.md` §1.4; this spec also uses 0x1 "changed", 0x4 targeting, 0x20, 0x40, 0x80, 0x200 repaired, 0x4000, 0x40000) |
| item data | +0x44 | body location (`0x00627D40` / `0x00627D70`) |
| item data | +0x45 | page (`0x00628250` / `0x00628280`; 0xFF none) |
| item data | +0x47 | stored page (`0x00628320`; the page shown in 0x9D action 5) |
| item data | +0x5C | owning inventory |
| item data | +0x60 / +0x64 | item list prev / next |
| item data | +0x68 | grid + 1 (0 = not in a grid) |
| item data | +0x69 | node kind: 1 page grid, 2 belt, 3 body location 1–10, 4 body location 11–12 |
| item data | +0x6C / +0x70 | grid list prev / next |

#### 1.2 Grids

| Grid | Size | Layout source | Placement treats items as |
|---|---|---|---|
| 0 body locations | 13 × 1 | constant `0x0074479C` | 1 × 1, x = body location |
| 1 belt | 16 × 1 | constant `0x007447B4` | 1 × 1, x = slot |
| 2 + page | gridX × gridY of the record (§1.3) | `inventory.bin` via `0x0065C1F0` | invwidth × invheight |

A grid is created on first use with the size given (`0x0063ADD0`, cells
zeroed); a later use with a different size fails (returns none). Pages:
0 inventory, 1 and 2 trade pages, 3 cube, 4 stash. Body locations: 1 head,
2 neck, 3 torso, 4 right hand, 5 left hand, 6 right ring, 7 left ring, 8
belt, 9 feet, 10 gloves, 11 / 12 weapon-swap right / left (`bodylocs`
codes; the intents accept 1–10 only).

#### 1.3 Grid record by page (`0x00621050`)

| Owner | Page | `inventory.bin` record |
|---|---|---|
| player | 1 / 2 / 3 | 6 / 7 / 9 |
| player | 4 | 8 in a classic game, 12 in an expansion game (game +0x70) |
| player | any other page (0 and 5–255: `0x00621050` tests page − 1 ≤ 3 unsigned) | class table `0x00744544` (7 pairs): class 0–4 → 0–4, 5 → 14, 6 → 15; no match → −1 |
| monster | any | 5 |
| object | any | class 0x152 → 10, 0x153 → 11, else −1 |
| missile, item, tile (types 3–5) | any | −1 (no record; socket fillers join an item's inventory through the link `0x0063B210`, not a grid) |

Measured on the 1.14d `inventory.bin` (32 records; the `.txt`
`Expansion` row is not compiled, so `.txt` rows after it shift by one):

| Record | Name | gridX × gridY |
|---|---|---|
| 0–4, 14, 15 | player classes | 10 × 4 |
| 5 | Monster | 10 × 10 |
| 6 / 7 | Trade Page 1 / 2 | 10 × 4 |
| 8 | Bank Page 1 (classic stash) | 6 × 4 |
| 9 | Transmogrify Box | 3 × 4 |
| 10 / 11 | Guild Vault / Trophy Case | 10 × 4 |
| 12 | Big Bank Page 1 (expansion stash) | 6 × 8 |
| 13 | Hireling | 0 × 0 |
| 16–31 | 800 × 600 layouts of records 0–15 (record r + 16): same grid sizes, different screen coordinates, except record 29 (`Hireling2`, `.txt` gridX/gridY −1, stored 255 × 255; record 13 is 0 × 0) | client only |

The server uses records 0–15 only (`0x00621050` returns no other value). Screen coordinates in the records are
the client's (`client/ui.md` B5).

#### 1.4 Item list and update list

1. Linking an item (`0x0063AF20`) **appends** it at the tail of the item
   list; its grid also appends it to the grid's list (`0x0063AF70`).
   Unlinking (`0x0063AAF0`) clears its cells (if it has a grid), clears
   the cursor if it was the cursor item (else the count drops by 1),
   clears the weapon GUID if it matches, and zeroes the node fields. So
   the item list is in **link order** (`world/cube.md` OQ5: removal and
   capture walk the list from the first linked item).
2. Update list (`0x0063CC70`): appends the item's GUID unless it is
   already listed (`0x0063CC30`).
3. Cursor (`0x0063C1E0` get, `0x0063C180` set): one item, not in any grid
   and **not** in the item list. Set with an item (an item unit with item
   data): inventory +0x20 := item, item data +0x5C := inventory; nothing
   else (no list link, no count change). Set with none: the current
   cursor item, if any, is unlinked (`0x0063AAF0`: since it is the
   cursor, only the cursor field is cleared, not the list or the count;
   then as for any unlink: +0x5C, +0x68, +0x69 zeroed, item data +0x0C
   := −1, `0x006277F0(owner, item)` when the inventory has an owner,
   weapon GUID cleared if it matches).

### 2. Grid placement

#### 2.1 Fit test (`0x0063A8A0`)

An item of w × h fits at (x, y) when every cell x..x+w−1 × y..y+h−1 is
empty. Callers check the bounds first: x ≥ 0, y ≥ 0, x + w ≤ width,
y + h ≤ height.

#### 2.2 Place at a position (`0x0063BCC0` → `0x0063AFD0`)

Grid = page + 2. Items with invwidth or invheight 0 never place. Bounds
and fit as §2.1 (negative x or y become 0 in `0x00560200` before this).
All arithmetic is signed 32-bit with wrap (`0x0063B05D`–`0x0063B08C`:
x < 0 or x + w > width fails, the same for y): when x + w (or y + h)
wraps past 2^31 the bound test passes, and the fit test (`0x0063A8A0`)
and the cell marking (`0x0063A910`) loop from x to x + w with a signed
`<`, so they run zero times: the item fits and is placed (linked,
counted, x and y stored) without occupying any cell. Reproduce (e.g.
0x18 with x = 0x7FFFFFFF on a 1-wide item). Grids 0 and 1 skip this
bound test (1 × 1, callers check the slot).
On success: an item still in a room (mode 3) is removed from the room
(room delete notice `0x0061A270`, collision freed `0x00623830`, room list
`0x0064C370`); unlinked from its old inventory; linked (§1.4); cells
marked (`0x0063A910`); node grid := grid + 1; count + 1; item x, y :=
(x, y); item owner := owning player's GUID (−1 when the owner is not a
player); grids ≥ 2 set page := grid − 2; node kind := 1 (page), 2
(belt), 3 or 4 (body location ≤ 10 or 11–12).

#### 2.3 Free-position search (`0x0063B850`)

Item w × h from items `invwidth`/`invheight`; either 0 → fail. Grid from
the page record. Four strategies; "player" means the inventory's owner is
a player unit:

| Item | Owner | Order | Choice |
|---|---|---|---|
| h = 1 | player | x from width−1 down to 0, then y from height−1 down to 0 (`0x0063B490`) | best weight |
| h = 1 | other | x from width−1 down to 0, then y from 0 up (`0x0063B540`) | first fit |
| h ≥ 2 | player | y from 0 up, then x from 0 up (`0x0063B620`) | best weight |
| h ≥ 2 | other | x from 0 up, then y from 0 up (`0x0063B6E0`) | first fit |

(`0x0063B850` routes h = 3 with w ≤ 2, h = 2 with w = 2 and all other
h ≥ 2 through three wrappers that call the same two searches; no
difference results.) A candidate must have its top-left cell empty, be
in bounds and fit (§2.1).

**Weight** (`0x0063B340`), u8, of a fitting candidate: count, along each
of the four sides, the occupied cells just outside the item, where a side
on the grid edge counts as fully occupied:

- left: x > 0 → occupied cells (x−1, y..y+h−1); else h;
- right: x+w < width → occupied cells (x+w, y..y+h−1); else h;
- top: y > 0 → occupied cells (x..x+w−1, y−1); else w;
- bottom: y+h < height → occupied cells (x..x+w−1, y+h); else w;
- total ≥ 2 × (w + h) → 255.

Weighted searches keep the first candidate whose weight is strictly
greater than the best so far, stop at 255, and succeed only if the best
weight > 0.

#### 2.4 Item placement into a page (`0x00560200`)

Arguments: game, owner, item GUID, x, y, "find a free position", "send",
optional inventory (default: the owner's). Used by 0x18, vendors, cube,
quests.

1. Game or owner missing → 0. Targeting reset (§5.3).
2. The item must exist, be an item and be on the cursor (mode 4), else 0.
3. Record from the item's page (§1.3). For a player owner: page 1 → the
   cursor is **not** cleared afterwards; page 2 → trade hook
   `0x00568770` after step 8 and before step 9 (multiplayer trade, out of
   scope).
4. Find-free → §2.3 then §2.2; else §2.2 at (max(x, 0), max(y, 0)).
   Failure → 0, nothing changed.
5. Link check `0x0063B210(inv, item, 1)` (sockets an item when the
   inventory belongs to an item; else succeeds). Failure → result 0 with
   nothing undone: the item stays placed in the grid by step 4 (cells,
   item list, count; the placement's unlink already cleared the cursor)
   but stays in mode 4 (original bug).
6. Page ≠ 4 → item-skill link (§5.5; `0x0055C270`: for a player owner
   `0x0055C110(1)`). Unit flag 0x2
   (targetable, unit +0xC4) cleared. If the item counts as an active
   inventory item for its owner (§5.6, `0x0062FF70`) → stat refresh
   `0x0055C2C0(owner, 0)`.
7. Cursor := none (unless step 3 said not), mode := 0 (stored).
8. "Send" set: command flag 0x2; item flag 0x1 when the item is socketed
   with fillers (`0x0055F590`); item flag 0x4000 cleared; owner refresh
   (`0x00621000`, §6.1); update list += item.
9. Step 6's active-item test again → inventory pass (§5.7) `0x0055DBC0(0)` and
   owner refresh. Result 1.

### 3. Belt

1. **Belt type** = items `belt` (+0x130) of the item at body location 8
   (`0x00621ED0`, no item-type test: whatever item is at location 8);
   no belt → record 2. In 1.14d only itemtype `belt` has body location
   `belt` (`ItemTypes.txt`, measured), so location 8 holds belts only. `numboxes` from `belts.bin`
   (14 records, `.txt` `Expansion` row not compiled): records 0–6 =
   belt 12, sash 8, default 4, girdle 16, light belt 8, heavy belt 12,
   uber belt 16; records 7–13 repeat them (800 × 600 copies).
2. **Slots**: grid 1, slot s = column (s & 3) + 4 × row; row 0 is the
   bottom row. A belt of n boxes uses slots 0..n−1.
3. **Beltable** (`0x0062BAD0`): itemtypes `beltable` of the item's type.
4. **Similar** (`0x00628A40`): same item class, or both codes in one
   group: {`hp1`…`hp5`}, {`mp1`…`mp5`}, {`rvl`, `rvs`} (tables
   `0x00744684`, `0x0074466C`, `0x00744660`).
5. **Free slot for an item** (`0x0063C600`): beltable, 1 × 1. For column
   c = 0..3: if slot c holds a similar item and c < n, the first empty
   slot among c, c+4, c+8, … < n is the answer; a column with no empty slot falls through to the next column (`0x0063C600`). After the four columns, if items `autobelt`
   (+0x131) ≠ 0: the first empty slot among 0..3. Else none.
6. **Auto-belt gate** (`0x00628BA0`): returns true for every item. It
   calls the "similar item in the bottom row" test `0x0063C560` (which
   skips codes `isc`, `tsc`) but only rejects on −1, which that test never
   returns (original bug; reproduce).
7. **Place in a slot** (`0x0063C4F0`): beltable, 1 × 1, slot ≤ 15
   (unsigned, `0x0063C4D0`) → §2.2 on grid 1 at (slot, 0). No `numboxes`
   check here, and none in 0x23's `0x0055E9B0` (with find 0) or 0x25's
   `0x0055EB30` (neither calls the belt-type lookup `0x00621ED0`): a
   crafted 0x23 / 0x25 fills any empty slot 0–15 whatever the belt.
8. **Compaction** after a slot s is emptied (`0x0055EDC0`): column c =
   min(s & 3, 3); walking rows 0..3, each item found in c moves down to
   the lowest free row of the column (re-placed with §3.7), gets item
   flags 0x400 and 0x1 (item flags +0x18 via `0x006280D0`, not command
   flags; only items whose row changes), item flag 0x4000 cleared, owner refresh, update
   list += item. The "remove from slot" helper `0x0063C550` is empty.
9. **Belt change** (`0x005608C0(game, unit U, new belt N or none)`, run
   when a belt leaves location 8 or a new one replaces it): n :=
   `numboxes` of N's belt type (`0x00621ED0`), or of record 2 (4 boxes)
   when N is none (`0x00660CB0(type, 0)`: the 640 × 480 set). For slots
   s = 0..15 in order, an item P in slot s (`0x0063C7F0`) with s ≥ n:
   1. S→C 0x9C action 0xF for P with bit-stream flag 0x20, direct to U's
      client (`0x0053EED0`).
   2. P leaves grid 1 (`0x0063AD90`; not found → fatal), mode 4,
      item-skill unlink (§5.5), page := 0.
   3. With a game: P (found by GUID, mode 4) goes to page 0 as §2.4 with
      find-free (record by page, `0x0063B950`), item-skill link, unit
      flag 0x2 cleared, cursor := none, mode 0, command flag 0x2, item
      flag 0x1 when socket-filled, 0x4000 cleared, owner refresh, update
      list, inventory pass when active; success → next slot.
   4. Else (no game, or no free position): free-spot search at U's
      position in U's room (`0x00555DA0`, size 1, flag 1); found →
      `0x0055C730(U, 0, 1)` and ground placement `0x00558AA0(P, room, x,
      y)`. Not found → P stays detached in mode 4 (neither cursor nor
      grid; original bug, reproduce).
10. **Belt removal gate** (`0x00567840(U)`): refused (0) only when U has
   an interaction (`0x00554100`) of unit type 0 (another player: trade)
   and any item has node kind 2 (in the belt); else allowed. Used by
   0x1C and 0x1D for location 8.

### 4. Equipping

#### 4.1 Body-location compatibility (`0x0062ED50`)

The item's allowed locations are itemtypes `bodyloc1`/`bodyloc2`
(`0x0062EA80`). Location L is allowed when it equals either, or when L is
11 or 12 and either allowed location is 4 or 5.

#### 4.2 Requirements (`0x0062EAF0`, item, unit, equipping)

1. Item missing or no items record → fail.
2. p = item stat 91 (`item_req_percent`, item or skill stat
   `0x00625500`). p ≠ 0 → bonus_str = reqstr × p / 100, bonus_dex =
   reqdex × p / 100, each `pct(req, p, 100)` (`0x00483360`, signed and
   truncating, `combat/damage.md` §0; ECX = req, EDX = p at `0x0062EB86`).
   Ethereal (item flag 0x400000) → both bonuses −10.
3. Strength: unit stat 0 < 1 → fail; < reqstr + bonus_str → fail;
   "equipping" and the item is active on the unit (`0x00625820`) →
   subtract the item's own strength contribution (`0x0062B450(0)`) and
   test again (< 1 or < requirement → fail).
4. Dexterity: the same with stat 2, reqdex, `0x0062B450(2)`.
5. Level: requirement R from §4.8 (never negative, so the caller's "R =
   −1 → skip" test at `0x0062EC8A` never fires); unit stat 12 < R → fail.
6. Pass needs 3–5 and **identified** (item flag 0x10). Then: type 18
   (book) needs item stat 70 > 0. Class: itemtypes `class` 7 (none) →
   pass; a player whose class equals it → pass; a monster of class 0x230
   or 0x231 (act 5 hirelings) with item class 4 (barbarian) → pass; else
   fail.

#### 4.3 Equip check (`0x0063DE60`, unit, location L, item N, skip requirements)

Result codes; N absent means "may L be emptied":

1. N present: §4.1 fails → 0; skip = 0 and §4.2 (equipping) fails → 0.
2. L = 0 → 0.
3. L ∉ {4, 5, 11, 12}: N present → 5 if L is occupied else 1; N absent →
   3 if occupied else 0.
4. L ∈ {4, 5} (other hand O = the other of 4/5) and L ∈ {11, 12} (O =
   the other of 11/12): T = item at L, X = item at O (`0x0063DD90`):

   | N | T | X | Result |
   |---|---|---|---|
   | absent | present | — | 3 |
   | absent | absent | two-handed (`0x006289C0`) | 4 |
   | absent | absent | other | 0 |
   | present | present | — | 6 if T and N stack (§4.5); else 5 if N and X are compatible (§4.4); else 7 if X fits a free position of page 0 (`0x0063CB00`); else 0 |
   | present | absent | present | 1 if compatible (§4.4), else 2 |
   | present | absent | absent | 1 |

#### 4.4 Hands compatible (`0x0063DBC0`, unit, A, B)

1. A or B absent → yes.
2. A has an ammo type (`0x0062E6F0`) and B is of that type, or the
   reverse → yes.
3. A or B has a quiver type (`0x0062E740`) → no.
4. A two-handed and not one-or-two-handed for the unit (`0x0062A1E0`) →
   no; the same for B.
5. Not both of type 45 (`weap`, with equivalence): A or B a weapon →
   yes; neither → no.
6. Both weapons: unit missing → no. Player class 4 (barbarian) → yes;
   player class 6 (assassin) → yes when both are type 67 (`h2h`); other
   players → no. Monster class 0x1A1, 0x1A2 → yes when both are `h2h`;
   0x21C–0x21E → yes; other monsters → no. Any other unit type (object,
   missile, item, tile) → no (`0x0063DCBD`).

#### 4.5 Stack test (`0x0062C850`, A, B)

Same item class, same quality (item data +0, `0x00627E70`), same
unique/set file index (item data +0x28, `0x00629DA0`), stackable
(`0x006289F0`), equal ethereal bits (item flag 0x400000, `0x0062A8D0`),
both qualities in 1–3 (low, normal, superior; `0x0062A2F0` passes q ≠ 0
and q ∉ 4–9), equal stats 21, 22, 23, 24, 159, 160 (damage ranges), and
neither has sockets (`0x006299B0` = 0). Used by 0x21 and hand result 6.

#### 4.6 Equip from the cursor (`0x005606B0`, game, player, item, L, skip, out)

1. out := 0; targeting reset. Item missing → out := 1, 0. Not an item →
   0. Mode ≠ 4 → out := 1, 0.
2. §4.3 ≠ 1 → 0 (out 0).
3. skip = 0 and §4.2 (not equipping) fails → out := 1, 0.
4. L ∉ {11, 12} and a weapon is in use → `0x006233A0` (weapon-in-use
   update).
5. Put at L (`0x0063BDB0` = §2.2 on grid 0), link (`0x0063B210`, kind 3,
   or 4 for 11/12; failure → out := 1, 0). Body location := L. L ∉
   {11, 12} → stat link `0x0063D1D0` and stat refresh. Cursor := none,
   unit flag 0x2 cleared, mode 1, page 0xFF, command flag 0x8, item flag
   0x1, item flag 0x4000 cleared, update list += item, refresh, weapon
   bookkeeping `0x0055C5C0`, inventory pass (§5.7) `0x0055DBC0(0)`. Result 1.

#### 4.7 Auto-equip on pickup (`0x0055D710`, unit, item, &L, skip)

1. skip = 0: §4.2 (not equipping) must pass. Then, for either skip:
   itemtypes `body` ≠ 0 with an allowed location (`0x0062FDF0`),
   identified, not broken and no item flag 0x4000 (`0x0062A4E0`),
   **primary** type (`0x0062B400`, not the equivalence test) ≠ 38
   (`tpot`).
2. Quiver-type items (`0x00628480` ≠ 0) need a hand weapon whose primary
   type's itemtypes `shoots` (`0x0062E6F0`) the item is (equivalence
   test): the right hand (location 4) first, then the left hand (5);
   no match → no.
3. bodyloc1 = bodyloc2: L := it if empty, else no.
4. Else: loc1 and loc2 empty → loc1; loc1 empty, loc2 used → loc1 if the
   new item N and the loc2 item are compatible; loc1 used, loc2 empty →
   loc2 if N and the loc1 item are compatible; both used → no.

Compatibility (`0x0055D670`, calls at `0x0055D8CC` / `0x0055D8FE`) reads a
ten-flag profile of each item X for the unit U (`0x0055D560`; ECX = U,
EBX = X, ESI = out; "type T" is the equivalence test):

| Flag | Set when |
|---|---|
| bow / xbow | X is type 27 (`bow`) / 35 (`xbow`) |
| bowq / xboq | X is type 5 (`bowq`) / 6 (`xboq`) |
| shield | X is type 51 (`shld`) |
| weapon | X is type 45 (`weap`) |
| 2h | items `2handed` ≠ 0 (`0x006289C0`) |
| dual | U is a player of class 4 (barbarian), or of class 6 (assassin) and X is type 67 (`h2h`) |
| throw | itemtype `throwable` ≠ 0 or item stat 125 ≠ 0 (computed; the comparison never reads it) |
| ring | X is type 10 (`ring`) |

N and E are compatible iff any of: N bow and E bowq; N bowq and E bow;
N xbow and E xboq; N xboq and E xbow; N a one-hand weapon (weapon and
not 2h) and E shield; E a one-hand weapon and N shield; both one-hand
weapons and both dual; both rings.

#### 4.8 Level requirement (`0x0062B5B0`, item, unit or none)

The unit only selects class-specific values; "affix value" of a magic
affix row A is A `classlevelreq` when A `class` ≠ 0xFF, a unit is given
and its class (unit +4) equals A `class`, else A `levelreq`. Affix ids
are item data +0x36 (automagic), +0x38 + 2i (prefix i), +0x3E + 2i
(suffix i), i = 0–2; id 0 or an unknown id is skipped (`0x00633EE0`).
R starts at 0; quality (item data +0; no item data → 2):

| Quality | R |
|---|---|
| 4 magic | max affix value of prefix 0, suffix 0 and the automagic affix |
| 5 set | setitems `lvl req` (i16) of the file index (`0x00483440`); negative → 0 |
| 6 rare | max affix value of the six prefixes/suffixes and the automagic affix |
| 7 unique | uniqueitems `lvl req` (i16) of the file index (`0x00483470`; file index < 0 or no row → 0); with a unit given that lacks unit flag 0x2000000 (expansion, `0x00463720`) on an item whose version (item data +0x30, `0x0062A670`) is 0 → 0; negative → 0 |
| 8 crafted | min(max affix value of the six prefixes/suffixes + 10 + 3 × (present affixes among the six), max level(0) − 1 = 98) (`0x00611830`); the automagic affix is ignored |
| other | 0 |

Then, in order, R := max(R, x) for: items `levelreq` of the item class
(`0x006335F0`); §4.8 of every item in the item's own inventory (socket
fillers, recursive, same unit; `0x0062B901`); for each entry of stat 107
(`item_singleskill`) in the item's stat list, skills `reqlevel` of the
entry's layer (skill id); for each entry of stat 97
(`item_nonclassskill`), skills `reqlevel` + 6, or `reqlevel` alone when a
unit is given, it is a player (`0x0044BE50` = 0) and the skill's
`charclass` (0–6) equals its class (`0x00451F60`). Entries are read only
from an extended stat list (flag bit 31; `0x006261D0`, at most 64 per
stat; skill ids ≥ the skills count are skipped). Last, R := R + item
stat 92 (`item_levelreq`); a sum < 1 returns 0.

### 5. Shared checks

#### 5.1 Item checks

All take the game and player; "item" = unit lookup type 4 by GUID
(`unit-order.md` §2). A null game is a fatal assert.

| Check | Address | Passes (result 0) when | Else |
|---|---|---|---|
| cursor item | `0x005490E0` | item exists, mode 4, and is the player's cursor item | 1 |
| stored item | `0x00549150` | item exists, mode 0, in the player's inventory | 1 |
| stored or equipped | `0x005491B0` | item missing, or mode not 0/1, or mode 0/1 and in the player's inventory | 1 when mode 0/1 and not the player's |
| owned item | `0x00549220` | item exists and (in the player's inventory or the cursor item) | 1 (missing → 1) |
| belt item | `0x005492F0` | item missing, or mode ≠ 2, or mode 2 and in the player's inventory | 1 when mode 2 and not the player's |
| ground or owned | `0x00549350` | modes 0, 1, 2, 4: owned item; mode 3: as `world/cube.md` §2 step 1 (same act, else 2; within 10 subtiles per axis, `0x00548EF0`) | 1; mode > 4 → 1 |

Note the inverted sense of `0x005491B0` and `0x005492F0`: a missing item
passes them (handlers then fail later on the lookup).

#### 5.2 Busy and trading

**Busy** (`0x00535060`): player with an interaction (`0x00554100`), or a
cursor item, or player data +0x4C ≠ 0. **Trading** (`0x005678A0`): the
interaction is with a player unit that exists (multiplayer only).

#### 5.3 Targeting reset

`0x0055BF50`: for every item in the player's item list with item flag
0x4: clear it; if `0x0044BE50` returns 0 queue S→C 0x3F (code 0xFF, the
item's GUID, 0xFFFF; §11). Runs first in most item routines (cited as
"targeting reset").

#### 5.4 Item-move gate

`0x00535610` (game, player, item): no
interaction → refused if player data +0x4C ≠ 0, else
`0x00567620`; interaction with a missing unit → interaction cleared
(`0x00554190`), refused; with a player (type 0) → `0x00567620`; with a
monster (type 1, NPC) → allowed unless the player is in the NPC's
interaction list with state 0 (talking; `world/npc.md` §2: allowed when
absent or state > 0); other types → `0x00567620`. `0x00567620` without a
player trade allows when player data +0x50 < 5; the player-trade part is
out of scope.

#### 5.5 Item-skill link (`0x0055C110`, unit U, item I, add)

Scrolls and tomes add their charges to a skill of the player (the
town-portal / identify counts). Called as "item-skill link"
(`0x0055C270`, ECX = owner, EDX = item: add = 1) and "item-skill unlink"
(`0x0055C6E0`: add = 0); both act only for a player owner (unit type 0)
and do nothing for others. Not related to charms.

1. Skill S (`0x0055BFF0`): I not of type 22 (`scro`) or 18 (`book`) →
   none, return 0. Books row = item suffix 0 (item data +0x3E, the spell
   index; `0x006374B0`, 0x20 bytes per row): scroll → books
   `scrollskill`, book → books `bookskill`; no row → none.
2. q = item stat 70 (`quantity`); q = 0: a book → return 0, a scroll →
   q := 1.
3. U's skill S (`0x006439B0`, owner −1). add = 1: present → new :=
   current quantity (skill +0x30, `0x006450F0`) + q; absent → stat 5
   (`newskills`) += q (`0x006272B0`), learn S (`0x00570080`, skills
   owner), fetch again (absent → fatal), new := q. add = 0: absent →
   fatal; new := max(current − q, 0).
4. Skill quantity := new (`0x00645120`); S→C 0x22 to U's client
   (`0x0053C520`; §11).
5. new = 0: if S is U's left skill (`0x00620190`) → select skill 0 with
   owner −1 on the left (`0x005701B0`, EDX = 1); if S is U's right skill
   (`0x006201D0`) → the same on the right (EDX = 0). Return 1.

**Recount on cube open/close** (`0x0055FA40`, unit U; callers
`world/cube.md` §3): for every item of U's item list in mode 0 (stored,
any page): skill S as step 1 (none → skip), S's quantity := 0 (absent →
fatal) and S→C 0x22 with quantity 0 (`0x0055F9C0`); count those on page 0.
Then the page-0 stored items, in list order, get the item-skill link
(add = 1) until the count is used up. Stash (page 4) and cube (page 3)
scrolls and tomes therefore stop counting after the first cube use.

#### 5.6 Active inventory item and usable equipment

- **Active inventory item** (`0x0062FF70`, item, unit): not broken (item
  flag 0x100), item flag 0x4000 clear, type 13 (`char`, equivalence
  test), page 0, and §4.2 (not equipping) passes. These are the charms
  whose stats count.
- **Usable** (`0x0055DB00`, ECX = unit, EDX = item): §4.2 (not
  equipping) passes, and an item whose itemtype `quiver` (`0x0062E740`)
  is set also needs the other hand's item (location 4, or 5 when the
  item itself is at 4) to be of that type.
- **Item flag 0x4000** marks an equipped item or charm whose stats are
  switched off because it is not usable.

#### 5.7 Inventory pass (`0x0055DBC0`, ECX = game, EDX = unit U, send)

Runs only for a player or a unit with unit flag 0x200 (+0xC4 bit 9;
hirelings). U without an inventory → nothing (after step 1).

1. Save U's left and right skill as (skill id, owner GUID at skill
   +0x34; id 0 when none) and stats 6, 8, 10 (`0x0055D9D0`; the stats are
   not read again by this pass).
2. Charms: each item of the item list with node kind 1 (+0x69,
   `0x0063E020`) that is an active inventory item (§5.6), whose stat list
   is not linked to U (`0x00625820` = 0) and is usable → item flag 0x4000
   cleared, then `0x0055D970`: stat link (`0x0063D1D0`, skipped for
   body locations 11 / 12), and when the item is in mode 1, or in mode 0
   and an active inventory item, stat refresh `0x0055C2C0(item, U, 1)`.
3. Switch off: body locations 1–10 in order, each item X that is (not
   broken or linked) and (flag 0x4000 clear or linked) and **not**
   usable → flag 0x4000 set, stat unlink (`0x0063D2B0`), X in mode 1 →
   deactivation `0x0055C730(X, U, 1, 1)`.
4. Switch on, repeated until a sweep changes nothing: locations 1–10,
   each X not broken with (flag 0x4000 set or not linked) and usable →
   flag 0x4000 cleared, stat link (not for 11 / 12), and when X is in
   mode 1, or in mode 0 and an active inventory item, stat refresh
   `0x0055C2C0(X, U, 1)`.
5. Set items: locations 1–10, each X of quality 5, not broken, flag
   0x4000 clear → `0x0055C730(X, U, 1, 1)` then `0x0055C2C0(X, U, 1)`
   (set bonuses re-applied).
6. Restore the step-1 skills (`0x0055DA70`), left then right: saved id
   ≠ 0, U still has the skill (id, owner) (`0x006439B0`), it is not the
   one now selected, and `0x00647960` returns neither 2 nor 7 → select it
   again (`0x005701B0`, EDX = 1 left / 0 right, id, owner; skills owner).
7. Owner refresh (`0x00621000(U, 1)`, §6.1).
8. send ≠ 0: S→C 0x48 (type U, arg 0, U's GUID; `0x0053D3C0`) to U's
   client, or for a non-player to its owner's (`0x0058F0D0`) client when
   that owner is a player.

### 6. Deferred item messages

#### 6.1 Marking

1. Handlers set command flags (item data +0x14, `0x00628170`) and item
   flags, append the item to the owner's update list (§1.4) and call the
   owner refresh `0x00621000(unit, 1)`: queue the unit for update
   (`unit-order.md` §6), set unit +0xC8 bit 0 and, for players, bit 1.
2. Player unit update (`0x00580860`, per client, from the room update
   queue walk): if the player has +0xC8 bit 0 → `0x00597890(client)`,
   then S→C 0x47 and 0x48 (§11) for the player.
3. `0x00597890`: for each node of the player's update list, in order:
   look the item up by GUID (missing → skip), run the dispatcher (§6.2);
   if that item has its own inventory (unit +0xC8 bit 0), run the
   dispatcher for each node of the item's update list. Then for a
   hireling owner (`0x0063EE90`) an inventory pass and `0x0055F4F0(1)`.
4. Clearing: the room update clean-up (`tick.md` §3 step 6,
   `0x00553220(game, unit)`) clears unit +0xC4 bits 0x1, 0x10, 0x400,
   0x8000 and +0xC8 bits 0x800, 0x1000, 0x10000, 0x200000, then calls
   the update-list reset `0x00597B00(game, unit)` (D2MOO
   `D2GAME_INVMODE_Last`). Unit without an inventory → nothing. Else:
   owner refresh with 0 (`0x00621000(unit, 0)`: clears +0xC8 bit 0 only;
   bit 1 stays set: only the character save `0x00532400` clears it, so
   bit 1 is a "save pending" mark and no message depends on it). For each node of the update list, in order, the
   item looked up by GUID (type 4; missing → skip):
   1. command flag 0x10 or 0x4000 → body location := 0 (`0x00627D70`);
      command flag 0x20 with item flag 0x80 → body location := 0.
   2. Per-item reset (`0x005979B0`): item +0xC8 bits 0x4 and 0x10
      cleared; command flags 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80,
      0x100, 0x40000, 0x400, 0x800, 0x1000, 0x2000, 0x200, 0x4000, 0x8000,
      0x10000, 0x20000, 0x80000, 0x100000, 0x200000 cleared (table
      `0x00738C70`, 21 entries; command flag 0x1 is **not** cleared);
      item flags 0x20, 0x2, 0x8, 0x80, 0x40, 0x1, 0x200, 0x40000 cleared
      (table `0x00738C4C`, 8 entries).
   3. The item has +0xC8 bit 0 (own inventory changed): its refresh with
      0, the per-item reset on every item of its update list (found by
      GUID), its update list freed (`0x0063CBD0`).
   4. Command flag 0x1 still set → unit removal `0x00557FD0(game,
      item)`.
   Then the unit's update list is freed (`0x0063CBD0`).

#### 6.2 Dispatcher (`0x005973F0`)

"Owner" = the client's own player is the inventory's owner. First the
store checks (the item's +0xC8 bits 2 and 4 → `0x0053EF30` with 0x38 /
0x39 to the client trading with the owner: `world/vendors.md`). Then the **first** matching row of
`items/item-actions.tsv` sends one message; later rows are skipped. A row matches only when its
flag test **and** its `to` test pass: a row whose flags
match but whose `to` excludes this client does not end the walk; later
rows are tried (`0x005973F0`: each test is "flag and client is owner"). Exception, the
item-flag rows 18 and 19: flag set but the client neither the owner
nor the item in mode 1 → the walk **ends** with nothing sent
(`0x0059775B`–`0x00597767`, `0x005977B4`–`0x005977C1`), so row 20 is
not tried; row 20 itself sends nothing in the same case. Every send of
the walk passes the bit-stream flag argument 0 (§11), except the 0x7D
rows, whose state = item flags & the row's flag (`0x006280A0`).
Only the store checks end the walk for every client (item +0xC8 bit 2
or 4 set: 0x38 / 0x39 to the trading client, nothing to others, done).
Columns: `order`; `test` (`cmd` = command flags, `item` = item flags);
`flags` (any of); `to` (`owner` = only the owner's client, `all` = every
client that processes this player; `owner|mode1` = owner, or item mode
1); `condition`; `message`; `action` (0x9C/0x9D action byte; for 0x7D the
flag value); `sender`; `d2moo` (D2MOO 1.10f name, a label only).

#### 6.3 Ground items

Unit flag 0x10 (unit +0xC4) means "not yet announced": the allocator
sets it on every unit (`sim/units.md` §3.1 step 5) and the room clean-up
`0x00553220` clears it together with 0x1. The per-unit update
`0x0053A500` (`tick.md` §6 step 5) handles an item in two parts:

1. Flag 0x10 set (a new item, announced once): the unit-add messages
   `0x00571F90`; for an item: mode 3 with unit flag 0x1000 → 0x9C
   action 2 (dropped, `0x0053EC90`); otherwise → 0x9C action 0 (new,
   `0x0053EC00`).
2. Item unit update (`0x0055BF30` when unit +0xC4 bit 0; `0x0055BED0`):
   items in mode 3 **without** flag 0x10 (already announced) send 0x9C
   action 2 when unit flag 0x1000 is set, else action 3 (on ground,
   `0x0053ECF0`).

So a ground item gets one message per tick in which it changed: part 1
in its first tick, part 2 later. Flag 0x1000 is set by a drop
(`0x00558AA0`) and by a refused pickup (`0x0055C9A0`) and cleared by the
room clean-up (`0x00553220`, item case, which also clears item flags
0x20 and 0x2000).

#### 6.4 Direct sends

In this spec's paths only 0x63 (§7.24) and the cube spill (§9.3) queue
item messages inside the handler (other callers: `world/cube.md` §8,
`world/vendors.md`): 0x9D action 5 via `0x0053D010` (asserts the client;
temporarily shows page := stored page) and 0x9C action 0xE via
`0x0053EE70`.

### 7. Intents

Every handler checks its exact size first (→ 3, `intents-events.md`
§2.4). "out" is the helper's refusal flag: helper result 0 with out ≠ 0
→ handler 3; helper result 0 with out = 0 → handler 0. Layouts:
`client-messages.tsv`.

#### 7.1 0x16 PickItem (`0x0054AAD0`)

1. Type (u32 @1) > 5 → 2. Type 0 and GUID = the player's own → 3.
2. `0x00548B00(GUID, cursor flag u32 @9)` by type: 1 `world/npc.md` §2,
   2 `world/waypoints.md` (object). Type 0 (player P): P missing or
   distance > 50 → 1; distance > 8 → walk to P (`0x00548A50`, as below)
   → 0; P in mode 17 (dead) and the player passes the busy test
   `0x005678A0(1)` = 0 → corpse pickup `0x0057FB70(game, player, P)`
   (needs P's state 7 `playerbody`; corpse spec, not specified here) →
   0; else `0x00566E60` (player-to-player interaction, wall-clock
   throttled with `GetTickCount`; multiplayer, out of scope) → 0. Type 5
   (tile): missing or distance > 50 → 1; distance < 5 → warp
   `0x005550B0` (`sim/path-placement.md` §12.2) → 0; else walk to it →
   0. Type 3 → 1. Type 4 (item):
   1. Item missing or mode ≠ 3, or distance (`0x00641530`, unit to unit;
      `sim/path-placement.md`) > 50 → 1.
   2. Distance ≥ 5, or a collision between player and item
      (`0x00622B50`, mask 0x804) → walk to the item (`0x00548A50`:
      player mode 3 toward (type 4, GUID) and interaction data −1, or
      −2 when the cursor flag is set; arrival is the movement spec's) →
      0.
   3. Else cursor flag ≠ 0 → §8.2 (to cursor); 0 → §8.1 (auto). Out ≠ 0
      → 3, else 0.

#### 7.2 0x17 DropItem (`0x0054AB40`)

1. Cursor item check (§5) ≠ 0 → that result.
2. Busy and trading → 3 (single player: never).
3. §9.1 drop; result 0 (also when no ground spot was found).

#### 7.3 0x18 InsertItemInBuffer (`0x0054ABB0`)

Fields: item u32 @1, x u32 @5, y u32 @9, page u32 @13.

1. Cursor item check ≠ 0 → that result.
2. Player missing or not a player: page > 4 → 2, then step 4.
3. Player: (busy = 0 and page ≠ 0 → 2: unreachable, the cursor item makes
   the player busy). Page 4 → the player's room must be in a town level
   (`0x0061AB00`: levels 1, 40, 75, 103, 109), else 3. Page 2 → trading,
   else 3. Page 1 or ≥ 5 → 2. Pages 0, 3 → on (the cube needs no open
   cube).
4. Item lookup missing → 2. Page := page; §2.4 at (x, y), send. Success →
   0, else 3.

#### 7.4 0x19 RemoveItemFromBuffer (`0x0054ACD0`)

1. Stored item check ≠ 0 → that result.
2. A cursor item exists → `0x00549A60` ("can't do that": S→C 0x5A, 40 bytes `5A 0E 01` then 37 zero bytes, to the player's client; same message as `skills/use.md` skill-start state 1), 2.
3. Item missing → 2. Page 1 → 3. Item-move gate refuses → 0.
4. `0x00560420` (to cursor, below); refused with out → 3; else 0.

`0x00560420(item, &out, "send" 1, 0, 0, 0)`: cursor present → 0. Item
missing → out 1. Player, not busy, page ≠ 0 → out 1 (players may lift
only page-0 items when idle; a busy player — open stash or cube is an
interaction — passes this test on any page, the page-1 check of the
handler aside; R2 confirms). Mode ≠ 0
→ out 1. Targeting reset; unit flag 0x2 cleared; unlink (missing or
mismatch → fatal); stat refresh `0x0055C730(player, 0, 1)`; inventory
pass if active; item-skill unlink `0x0055C6E0`; room-change notice
`0x0063BCF0` with the item's old cell; stored page := page; page := 0xFF;
cursor := item (except page 1 for players); mode 4; command flag 0x4,
item flag 0x1 when socket-filled, item flag 0x4000 cleared; update list
+= item; refresh.

#### 7.5 0x1A EquipItem (`0x0054AD90`)

Cursor item check; body location (u8 @5) ∉ 1..10 → 2; §4.6 with skip 0;
refused with out → 3 (the item lookup is repeated with no effect); else 0.

#### 7.6 0x1B Swap2HandedItem (`0x0054AE30` → `0x00563D20`)

Cursor item check; location ∉ 1..10 → 2. `0x00563D20`: location must be
4 or 5 (else out 1); the other hand must hold an item X (else out 1); §4.3
must give 2 (else out 1); requirements (not equipping) fail → stat
refresh, sound, 0 (out 0). Then X leaves the body (`0x0062A360`,
`0x0063D2B0`, unlink, slot cleared `0x0063BE30`; X of type 19 (belt) →
`0x005608C0`, §3 rule 9, with no new belt) and becomes the cursor item (mode 4, unit
flag 0x2 cleared; no command flag and no update-list entry for X). N then
goes to the location (`0x0063BDB0`) and is linked (kind 3; link failure →
out 1, 0): body location, stat link, stat refresh, unit flag 0x2
cleared, mode 1, page 0xFF, command flag 0x10000 (0x9D action 7), item
flag 0x1, 0x4000 cleared, update list, weapon bookkeeping, inventory
pass. The cursor is **not** cleared afterwards: X stays the cursor item
(read at `0x00563D20`; §4.6 step 5's "cursor := none" does not apply). If
the put at the location fails, N is left detached, X is the cursor, and
the result is still 1 (original bug). Owner refresh; result 0 or 3.

#### 7.7 0x1C RemoveBodyItem (`0x0054AEC0` → `0x00560CD0`)

Location u16 @1 ∉ 1..10 → 2. Item-move gate for the item at the location
refuses → 0. Location 8 (belt) needs `0x00567840` (§3 rule 10),
else 0. `0x00560CD0`: cursor present → 0; empty location → 0 (checked before §4.3, so §4.3 result 4 never occurs here: 0x1C on the empty hand opposite a two-handed weapon does nothing, `0x00560CD0` read in full); §4.3 (N
absent) must give 3 or 4, else out 1; `0x0063E490` picks the item to
remove (for 4: the two-handed item in the other hand); remove from body
as in §7.6; type 19 → `0x005608C0` (§3 rule 9, no new belt); cursor := item; stat refresh; unit
flag 0x2 cleared; mode 4; command flag 0x10 (0x9D action 8); item flag
0x1 when socket-filled; 0x4000 cleared; update list; refresh; weapon
bookkeeping; inventory pass. Result 0 or 3.

#### 7.8 0x1D SwapCursorWithBody (`0x0054AF50` → `0x00560F00`)

Cursor item check; location ∉ 1..10 → 2; empty location → 1; location 8
needs `0x00567840`; item-move gate on the equipped item. `0x00560F00`:
§4.3 must give 5 (else out 0, result 0); weapon-in-use update; the
equipped item E (via `0x0063E490`) must be in mode 1; stat refresh;
requirements of N (not equipping) fail → stat refresh, sound, 0. N of type
19 → `0x005608C0(N)` (§3 rule 9). E: removed, cursor := E, mode 4, item flag 0x80,
command flag 0x20, item flag 0x1, update list. N: placed at the location,
body location set, stat link, mode 1, page 0xFF, item flags 0x40 and 0x1,
command flag 0x20; update list; weapon bookkeeping; inventory pass. Both
send 0x9D action 9.

#### 7.9 0x1E Swap1HWith2H (`0x0054B030` → `0x00561220`)

Cursor item check; location ∉ 1..10 → 2; location ∉ {4, 5} → 3; empty
location → 1; item-move gate; `0x00561220(game, player, N's GUID, L,
&out)` (the cursor item N goes to hand L; the item T at L to the cursor;
the item X in the other hand to page 0):

1. out := 0. N must be an item in mode 4, else 0.
2. §4.3 (L, N, skip 0) ≠ 7 → 0. §4.2 (not equipping) fails → out 1, 0.
3. r := page-0 grid record (§1.3); O := 4 when L = 5, else 5; X := item
   at O. `0x0063CB00(inv, N, O, r)` = 0 → out 1, 0. X missing → fatal.
4. X in mode 1: X leaves the body (`0x0062A360`, stat unlink
   `0x0063D2B0`, removed from grid 0 (`0x0063AD90`; not found or another
   item → fatal), slot cleared `0x0063BE30`, deactivation
   `0x0055C730`); free-position placement on page 0 (`0x0063B950`):
   placed → link (failure → out 1, 0), page 0, unit flag 0x2 cleared,
   cursor := none, mode 0, item flag 0x1 when socket-filled, 0x4000
   cleared, command flag 0x4000, item flag 0x1, update list. Not placed
   → X stays detached in mode 1 off the grid (original bug).
5. T := item at L (missing → fatal); T not in mode 1 → out 1, 0.
   Deactivation of T; §4.2 for N (not equipping) fails → stat refresh,
   cursor := N, refused-pickup sound `0x0055FB10(N)`, `0x00553380`,
   owner refresh, 0 (X stays moved).
6. T leaves the body as X did, cursor := T, unit flag 0x2 cleared, mode
   4, command flag 0x10, item flag 0x1, 0x4000 cleared, update list.
7. N put at L (`0x0063BDB0`) and linked (kind 3; either failing → out
   1, 0): body location L, stat link, stat refresh, unit flag 0x2
   cleared, mode 1, page 0xFF, item flag 0x8, command flag 0x8, item
   flag 0x1 (and when socket-filled), 0x4000 cleared, update list,
   weapon bookkeeping `0x0055C5C0`, inventory pass (§5.7). Result 1.

#### 7.10 0x1F SwapCursorBufferItem (`0x0054B0F0` → `0x00561B00`)

Fields: cursor u32 @1, target u32 @5, x u32 @9, y u32 @13.

1. Cursor item check, then stored item check on the target.
2. Target missing → 2; target page 1 or 2 → 3; item-move gate → 0.
3. `0x00561B00`: target page 3 and the cursor item is the cube (`box `,
   class cached at `0x0088C6FC`) → 0 (no cube in a cube); target page 2
   → out 1. Target mode must be 0. Target T: unlink, stat refresh,
   room-change notice, page := 0xFF, cursor := T, item-skill unlink, stored
   page := page, unit flag 0x2 cleared, mode 4, command flag 0x40000,
   item flag 0x1 if socket-filled, update list. Cursor item C: §2.2 at
   (x, y) on the same page (fail → out 1); stored page 0, page := page,
   x, y set; link; item-skill link; stat refresh if active; mode 0; command
   flag 0x40000; update list; refresh; inventory pass if T was active.
   Both send 0x9C action 0xD.

#### 7.11 0x20 UseGridItem (`0x0054B1E0` → `0x0055E170`)

Stored item check; (x u32 @5, y u32 @9) within 50 subtiles of the player
per axis (`0x00548EF0`), else 1; `0x0055E170(game, player, I, x, y,
&out)` (item use; the effects behind `0x005BF240` are owned by the
unwritten item-use spec, `world/cube.md` OQ7); refused with out → 3:

1. out := 0; targeting reset. I missing → out 1, 0. I not an item or a
   cursor item exists → 0. I not in mode 0 or items `useable` = 0 (`0x00628C20`)
   → out 1, 0.
2. I of primary type 18 with stat 70 < 1 → 0. Busy test
   `0x005678A0(1)` ≠ 0 → 0.
3. `0x005BF240(I, I, x, y)` ≠ 0 (used): primary type 18 with skill S
   (§7.18 step 6) ≠ −1 and stat 70 > 0 → stat 70 −= 1 (S→C 0x3E), S→C
   0x7C (I), skill decrement, 1. Type 18 otherwise → 1 (nothing more).
   Other types: S ≠ −1 and the player has S → S's quantity − 1 (< 1 →
   0, and S as the right skill → skill 0 on the right), S→C 0x22;
   targeting reset; consume I (`0x0055E000`); 1.
4. Not used: by items code, with the player's quest record for the
   difficulty (`0x00543520`; flag test `0x0065C310`, clear
   `0x0065C3A0`, set `0x0065C360`; meanings `world/quests.md`):
   - `ass`: targeting reset; flag (9, 5) set → clear it, stat 5
     (`newskills`) += 1, `0x005458E0`, consume I, 1.
   - `xyz`: targeting reset; flag (20, 5) set → clear it, stat 7
     (`maxhp`) += 0x1400 (20 life, 8.8 fixed), `0x005458E0`, consume I,
     1.
   - `tr2`: targeting reset; (37, 8) set and (37, 7) clear → set (37,
     7), `0x0058A0A0`, `0x005458E0`, consume I, 1.
   - `toa`: targeting reset, `0x00570360`, `0x00570C80` (skills and
     stats reset; skills / character owner), consume I, sound
     `0x00553380`, 1.
   - Other codes → 0. A failed flag test above → sound `0x00553380`,
     1.

#### 7.12 0x21 StackItems (`0x0054B300` → `0x0055E7C0`)

Owned item check on src (u32 @1) and dst (u32 @5); src = dst → 3.
`0x0055E7C0`: either missing → out 1; dst page 2 → out 1; §4.5 fails → 0
(handler 0). Same class → merge (`0x0055E590`): with q_s, q_d the stat-70
quantities and m the max stack (`0x006295B0`, `items/generation.md`
§1.3): q_s + q_d > m → dst := m, src := q_s + q_d − m (each announced by
S→C 0x3E, `0x0053D130`), both books (type 18) → `0x0055C070(m − q_d)`,
dst item flag 0x8; else (when `0x00629930(src)`) dst stat 72 (`durability`, live `itemstatcost` row 72) lowered
to src's if src's is lower (0x3E; throwing weapons keep the worse
durability), dst := q_s + q_d
(0x3E), both books → `0x0055C070(q_s)`, cursor := none, S→C 0x42 for
src, src freed (`0x00557FD0`). Then dst command flag 0x100 (0x9C action
0xA), update list, refresh. (Different classes never pass §4.5.)

#### 7.13 0x22 UnstackItems (`0x0054B380`)

Owned item check; then `0x0055E9A0` returns 0 without touching the
refusal flag, which is the message size (5): the handler returns **3**
for every owned item (original quirk; reproduce).

#### 7.14 0x23 ItemToBelt (`0x0054B3E0` → `0x0055E9B0`)

Cursor item check; `0x0055E9B0(item, slot u32 @5, find 0)`: targeting
reset; item missing or mode ≠ 4 → out 1; not beltable → 0; §3.7 into the
given slot (fail → 0, page := 0xFF); link kind 2 (fail → out 1); cursor
:= none; item-skill link; unit flag 0x2 cleared; stat refresh if active;
mode 2; page 0xFF; command flag 0x400 (0x9C action 0xE); refresh; update
list. Result 0 or 3. (With find ≠ 0, used by other callers, §3.5
chooses the slot.)

#### 7.15 0x24 ItemFromBelt (`0x0054B450` → `0x00562250`)

Belt item check; item-move gate (with no item) refuses → 0; a cursor
item → 2. `0x00562250`: item missing → out 1; mode ≠ 2 → out 1;
targeting reset; slot := item x; unlink; cursor := item; item-skill unlink;
unit flag 0x2 cleared; stat refresh; mode 4; command flag 0x800 (0x9C
action 0xF); refresh; update list; compaction §3.8 of the slot.

#### 7.16 0x25 SwitchBeltItem (`0x0054B4E0` → `0x0055EB30`)

Cursor item check on the cursor (u32 @1), belt item check on the belt
item (u32 @5). Cursor item must be beltable (else 0) and in mode 4;
belt item mode ≠ 2 → out 1. Belt item B: unlink (mismatch → fatal),
cursor := B, mode 4, command flag 0x1000. Cursor item C: §3.7 into B's
slot (fail → fatal assert, line 0x12D8), x := slot, y := 0, link kind 2,
mode 2, page 0xFF, command flag 0x1000. Both send 0x9C action 0x10.
Inventory pass. No `numboxes` or similar-item check.

#### 7.17 0x26 UseBeltItem (`0x0054B560` → `0x00562390`)

Belt item check; the player's own position passes the 50-subtile test
(always). `0x00562390(item, x, y, &out, on_merc u32 @5)`: item missing →
out 1; a cursor item → 0; mode ≠ 2 or items `useable` = 0 → out 1;
trading → 0. on_merc ≠ 0 with a player in an expansion game: the item
must be of type 76, 80 or 81 (`hpot`, `apot`, `wpot`, with itemtypes
equivalence `0x00629BB0`), else 0; the target becomes the hireling
(`0x00574EC0(7, 0)`). Use: `0x005BF240` (item-use spec). Used → tome /
skill charge update (`0x0055E050`, `0x006439B0`, S→C 0x22 via
`0x0053C520`), targeting reset, removal `0x00561E70`, compaction §3.8.

#### 7.18 0x27 UseItemAction (`0x0054B280` → `0x00561ED0`)

Owned item check on target (u32 @1) and used item (u32 @5);
`0x00561ED0(game, player, T, U, &out)` (U = scroll or tome used on item
T; the effect itself is the item-use dispatcher `0x005BF240`, owned by
the item-use spec):

1. out := 0. U missing → out 1, 0. T missing or T = U → targeting reset
   (§5.3), 0.
2. U in mode 2 (belt) and not type 22 (`scro`) → out 1, 0. Busy test
   `0x005678A0(1)` ≠ 0 → 0. T or U not an item, or a cursor item exists
   (`0x0063C1E0`) → 0.
3. T not in mode 0 or 1: U in mode 2 → targeting reset, consume U from
   the belt (`0x00561E70`: S→C 0x9C action 0xF with bit-stream flag
   0x20, then `0x0055ED30(U)`), 0; U in mode 0 of primary type 18 → U's
   stat 70 := max(stat 70 − 1, 0) with S→C 0x3E, 0; U in mode 0 of
   another type → 0; else out 1, 0.
4. U of primary type 18 with stat 70 < 1 → targeting reset, S→C 0x7C
   (U's type, U's GUID; 6 bytes `0x0053B3D0`), 0.
5. `0x005BF240(U, T, 0, 0)` = 0 (not used) → result 1.
6. Item skill S of U (`0x0055E050`: book → books `bookskill`, scroll →
   `scrollskill`, else −1; no books row → fatal). "Skill decrement"
   (`0x0055E0D0(S)`): S's quantity − 1; < 1 → 0, and S as the right
   skill → select skill 0 (owner −1) on the right; S→C 0x22 (§11); U's
   skill missing → fatal.
7. U in mode 2: S = −1 → S→C 0x7C (U); else skill decrement and consume
   U from the belt (`0x00561E70`). Targeting reset, 1.
8. U not in mode 0 → out 1, 0.
9. U of primary type 18: S ≠ −1 and stat 70 ≥ 1 → stat 70 −= 1
   (S→C 0x3E), skill decrement, S→C 0x7C (U); else consume U
   (`0x0055E000`: S→C via `0x0053D010` with flag 0x20, then
   `0x0055DF10(U, 0)`). Other U: S ≠ −1 → skill decrement; consume U
   (`0x0055E000`). Targeting reset, 1.

#### 7.19 0x28 SocketItem (`0x0054B650` → `0x00562660`)

1. Cursor item check on the filler (u32 @1); stored-or-equipped check on
   the target (u32 @5).
2. If the player has an interaction whose unit exists and is a player →
   0 (trade). Else `0x00562660(filler, target, &out, 1, 1, 1, 1)`:
   targeting reset; filler missing → out 1; filler mode ≠ 4 or target
   missing → out 1; target not identified (0x10) → 0; mode check on
   (target mode 0 or 1); filler must be a socket filler (`0x0062BEB0`)
   and identified; target must be socketed (item flag 0x800) and have
   free sockets (filler count `0x0063CD60` < sockets `0x006299B0`); the
   target's inventory is created if needed (`0x0063ABD0`).
3. Link the filler into the target's inventory (failure → fatal); cursor
   := none; unit flag 0x2 cleared; filler properties
   (`items/properties.md` §9, `0x0055C2C0`); owner link `0x006276C0`;
   filler mode 6. Runeword (`items/properties.md` §10): a match whose
   runes record allows it (record +0x81 = 0, or an expansion game) →
   `0x006600A0` (runeword stats), `0x00558530`, `0x00558580`,
   `0x0055FE80`. Then target item flag 0x1, 0x4000 cleared, refresh,
   update list += target (0x9D action 0x15 per §6.2 row 20). Result 0 or
   3.

#### 7.20 0x29 ScrollToBook (`0x0054B710` → `0x0055EF20`)

Ground-or-owned check (§5) on the scroll (u32 @1); stored item check on
the book (u32 @5). `0x0055EF20`: scroll missing → out 1; scroll mode not
3/4 or not type 22 (`scro`) → out 1; book missing → out 1; book mode ≠ 0
or type ≠ 18 → out 1; book and scroll spell (`0x00627F80`) differ →
fatal assert (line 0x149C); book quantity (stat 70) ≥ max stack → 0.
Scroll consumed: unless `0x0055EEA0` (scroll is a stack it decrements),
it is removed from its room and freed, cursor := none. Book stat 70 += 1;
S→C 0x3E for stat 70; `0x0055C070(1)`. Result 0.

#### 7.21 0x4C — `world/cube.md` §10 (not the cube).

#### 7.22 0x50 DropGold (`0x0054C800` → `0x00535510`)

Fields: unit u32 @1, amount u32 @5. Busy and trading → 3. `0x00535510`:
the unit looked up by the GUID must be the player itself, else 3; amount
must be 0 ≤ amount ≤ gold (stat 14) and ≤ the gold limit (level × 10000,
`0x00622E70`), else 3. Amount 0 → 0. Else cap 2,000,000,000; create up to
32 gold piles (`0x0055A090`, §10.2); for each pile, in creation order:
owner := player (`0x00621CE0`, only when `0x0044BE50` is 0); gold :=
gold − pile gold (`0x00530EA0`). Result 0.

#### 7.23 0x61 MercItem (`0x0054D430`), expansion only

1. Classic game → 3. Busy and trading → 3.
2. Ends with 0 (no effect) unless: the player has no used skill (`0x00620250`(player): skill list +0x10,
   `skills/levels.md`; register argument pushed at `0x0054D47A`), the player is alive (`0x005541B0` = 0), a hireling
   exists (`0x00574EC0(7, 0)`), it is alive, and it belongs to the player
   (`0x0065A590`).
3. No player inventory → 3. Cursor item C present: C's items `quest` = 0
   → `0x0054D230` (give to the hireling; result ignored), 0. No cursor:
   location (u16 @1) ≠ 0 → `0x0054D130` (take from the hireling) and its
   result; location 0 → 0.

`0x0054D130` (take): location ∉ 1..10 → 2; the hireling's item at the
location must exist in mode 1, else 2; unlink, slot cleared, hireling
stat refresh `0x0055C730(merc, 0, 0)`, command flag 0x10 on it, update
list of the hireling, hireling refresh; a **copy** (`0x0055A2A0`,
`items/generation.md` duplicate) becomes the player's cursor item
(`0x0055FB10`), the original gets item flag 0x20; hireling inventory pass,
`0x0055F500`, `0x0055F4F0(0)`. Result 0.

`0x0054D230` (give): classic → 3; C not identified → 0; C broken (0x100)
→ 0; C of type 76/81/80 (potions) → used on the hireling
(`0x005BF240`), consumed (`0x0055EEA0`), cursor := none, 1. Else allowed
when: C is of type 3 (`tors`) or 37 (`helm`); or by hireling class:
0x10F (271, Act 1) type 27 (`bow`); 0x152 (338, Act 2) type 33 or 34
(`spea`, `pole`); 0x167 (359, Act 3) type 2 (`shie`), or type 30
(`swor`) one-handed; 0x230 (560) type 28 (`axe`) one-handed or type 71
(`phlm`); 0x231 (561) type 30 or type 71. No hireling unit → only types
3 and 37. Allowed and §4.2 (hireling, not equipping) passes →
`0x0054CED0` (equip on the hireling); sound on the player either way.

#### 7.24 0x63 ItemToBeltShift (`0x0054D520`)

1. Stored item check ≠ 0 → that result. A cursor item → `0x00549A60` (§7.4), 2.
2. Item missing or not beltable → 2; page ≠ 0 → 3. §3.5 slot (none, or
   < 0) → 0; item-move gate refuses → 0.
3. No player inventory → fatal; item page ≠ 0 → 2; item mode ≠ 0 → 2;
   unlink (missing or mismatch → fatal assert).
4. Room-change notice with the old cell; stored page := page; page :=
   0xFF; queue 0x9D action 5 (`0x0053D010`, now); mode 4; unit flag 0x2
   cleared; §3.7 into the slot: success → page := 0xFF, link kind 2, mode
   2, queue 0x9C action 0xE (`0x0053EE70`, now). Owner refresh (no
   update list). Result 0. A failed slot placement leaves the item in
   mode 4, not linked and not the cursor item (original quirk).

### 8. Pickup from the ground

#### 8.1 Auto (`0x00563560`, cursor flag 0)

1. Cursor item or busy → 0. Item missing → out 1; mode ≠ 3 → out 1.
2. Targeting reset. Can-pick (§8.4) fails → refused pickup (§8.3, sound
   0x13), 0.
3. Sound event on the player (`0x00553380`). Gold (type 4) → §10.1.
4. Special items (`0x00560020`): type 22 (scroll) → into a tome
   (`0x0055FFA0`); type 18 (book) → `0x0055D370`; stackable with
   auto-stack (`0x006289F0`, itemtypes `autostack` `0x0062E790`) → onto
   existing stacks (`0x0055D0D0`). Handled → 0. "Tome for P"
   (`0x0063C3B0`) = the first item of page 0's grid item list (grid list
   order, item data +0x70 next) of primary type 18 whose spell (item
   suffix 0, +0x3E) equals P's and whose stat 70 < total max stack.
   - Scroll (`0x0055FFA0`): a tome T for P → the 0x29 routine
     `0x0055EF20(P, T, &out)` (§7.20) and its result; none → not handled.
   - Book (`0x0055D370`): a tome T for P (none → not handled); q_p, q_t
     = stat 70, m = T's total max stack (any negative → fatal). q_t + q_p
     > m: T := m, P := q_t + q_p − m (each S→C 0x3E), item-skill add
     m − q_t (`0x0055C070`: §5.5 skill quantity += n, S→C 0x22), handled
     (P stays on the ground). Else T := q_t + q_p (0x3E), item-skill add
     q_p, P leaves its room (`0x0061A270`, `0x00623830`, `0x0064C370`),
     unit flag 0x2 cleared, P freed (`0x00557FD0`), cursor := none,
     handled.
   - Auto-stack (`0x0055D0D0`), while P's stat 70 > 0: candidate D :=
     the next item, starting at the previous candidate, that passes §4.5
     with P and has stat 70 < total max stack: from the body-location
     grid (`0x0063C2F0`) when P's itemtype `quiver` ≠ 0, then (none
     there, or `quiver` = 0) from page 0's grid (`0x0063C200`). No D →
     not handled (earlier partial merges stay). q_d + q ≤ m: D's stat
     72 lowered to P's when P has durability and P's is lower (0x3E), D
     := q_d + q (0x3E), P := 0, both books → item-skill add q, P leaves
     its room and is freed as above, cursor := none, handled. Else D :=
     m (0x3E only for D), P := q + q_d − m, both books → add m − q_d;
     next candidate.
5. Auto-equip §4.7 (skip 0) gives L → §4.3(L, item, 0) must be 1, else
   out 1; leave the room; `0x00562E00(item, 0)` equips; success → quest
   hook ITEMPICKEDUP (`0x00543D80`, `world/quests.md`).
6. Else beltable, §3.6 (always) and §3.5 + §3.7 succeed → leave the room,
   link kind 2 (failure fatal), cursor := none, item-skill link, stat
   refresh if active, unit flag 0x2 cleared, mode 2, page 0xFF, command
   flag 0x2000 (0x9C action 0xE), unit flag 0x2000000 cleared, update
   list, refresh, inventory pass if active.
7. Else page-0 free position (`0x005600A0(game, 1, page 0)`: §2.3 + §2.2,
   leave the room, link, cursor := none, item-skill link, stat refresh,
   unit flag 0x2 cleared, mode 0, command flag 0x80 (0x9C action 4),
   update list, refresh, unit flag 0x2000000 cleared, page := 0, quest
   hook ITEMPICKEDUP, inventory pass if active). No room → refused
   pickup with sound 0x17, 0.

#### 8.2 To cursor (`0x0055CF50`, cursor flag ≠ 0)

Cursor item or busy → nothing. Targeting reset. Item not an item →
nothing; mode ≠ 3 → out 1. Can-pick fails → refused pickup (sound 0x13).
Room delete notice; gold → §10.1; else collision freed, room left, unit
flag 0x2 cleared, cursor := item, mode 4, command flag 0x40 (0x9C action
1, which first sets the item's x, y to 0), page 0xFF, refresh, unit flag
0x2000000 cleared, update list, quest hook ITEMPICKEDUP. Then the
pickup sound on the player (also for gold).

#### 8.3 Refused pickup (`0x0055C9A0`, sound s)

Room delete notice for the item, page := 0xFF, unit flag 0x1000, mode :=
3 again (re-announced as 0x9C action 2, §6.3), sound event s on the
player.

#### 8.4 Can pick (`0x0055CC90`)

1. No player inventory → no.
2. Unique (quality 7) with a file index ≥ 0 whose `uniqueitems` record
   flag at +0x2C has the bit of `0x006CE270` (D2MOO `carry1`) and the
   player already holds one (step 6 test) → no.
3. Items `quest` (+0x12A) = 0 → yes.
4. By code, with quest flags of the player's quest record for the
   current difficulty (`0x00543520` / `0x0065C310`; flag meanings:
   `world/quests.md`): `ass` (book of skill) needs record (9, 5) clear;
   `j34` needs (20, 0) clear; `xyz` needs (20, 5) clear; `g33` needs
   (19, 7) and (19, 8) clear; `tr2` needs (37, 8) set and (37, 7) clear.
5. By `quest` value: 10 → (10, 0) clear unless the code is `box`; 5 →
   (4, 0) clear unless the code is `leg`; table `0x00731FEC` pairs
   (quest 3, value 4), (0x12, 0x11), (0x13, 0x12), (0x1B, 0x19): the
   quest's flag 0 must be clear.
6. Held test (`0x0055CA40`; EAX = list start, EBX = the picked item's
   items row, stack = the picked item P), run on the player's item list,
   then on the item list of every unit in the inventory's +0x34 list
   (`0x0063D570`; nodes linked at +0xC, `0x0063D610`; each node's GUID
   `0x0063D630` looked up as a player unit, type 0: the player's corpses,
   D2MOO corpse list). The walk stops at P itself (returns "not held"
   for the rest of that list). An item X (unit type 4) before P means
   held → no, when:
   - P is a unique (quality 7) whose uniqueitems row has `carry1`
     (flag bit 0x4 at +0x2C, mask `0x006CE270` = 4) and X is not on page
     1, of quality 7, with the same file index (the row's id at +0); or
   - X's items `quest` ≠ 0, X not on page 1, X's `quest` equals P's, and
     the codes are equal or one of the pairs, both orders: `j34`/`g34`,
     `bks`/`bkd`, `d33`/`g33`, `hst`/`msf`, `hst`/`vip`, and `qf2` with
     each of `qf1`, `qhr`, `qey`, `qbr` (`0x0055CA00`). This is the full
     list.
   Else yes.

### 9. Drop to the ground

#### 9.1 Drop the cursor item (`0x00563C00`)

1. Game or player null → fatal. Targeting reset. Item must exist and be
   in mode 4, else nothing.
2. Start point and search: player position; start (x + 2, y + 3) if a
   room exists there, else (x, y); free-spot search
   (`0x00555DA0` → `0x0064E810`, size 1, masks 0x3E01 / 0x801;
   `items/treasure.md` §7 rule 2, `sim/path-placement.md`). No spot →
   nothing (the item stays on the cursor).
3. Ground placement (`0x00558AA0`): position set, room added, unit flags
   0x1002 (0x1000 = "dropped"), mode 3, page 0xFF, unit flag 0x2000000,
   item data +0x24 := expiry (§9.2), quest hook ITEMDROPPED
   (`0x00543DB0`) when in a room.
4. Cursor := none; item flag 0x1 if socket-filled; item flag 0x4000
   cleared; cube spill (§9.3). Clients see 0x9C action 2 (§6.3).

#### 9.2 Ground expiry (`0x00558A10`)

Quest items (`0x00628CD0` ≠ 0) → the absolute value 0 (never expires; not frame + 0). Others: frame (game +0xA8) plus:
quality (item data +0) 4 (magic) → 30000; quality 5–9 → 45000; gold
(type 4) with more than 10000 → 45000; other items for which
`0x0062BEB0` is true (the socket-filler test of §7.19) → 30000; else
15000. Stored in item data +0x24 by the drop paths (`0x00554C04`,
`0x00558B19`). Reader: `0x00558B90(game, act)`, run every 1,500 frames
for acts 0–4 (`sim/tick.md` §3 step 11): for each room of the act
(`0x0061A180` first, room +0x7C next), each unit of the room's unit list
(+0x74, unit +0xE8 next, saved before the body): an item with expiry ≠ 0
and expiry ≤ game frame (read once at entry) is removed: if it has a room
(`0x00620BB0`), room unit removal `0x0061A270`, `0x00623830`,
`0x0064C370`; then unit flag 0x2 cleared, `0x005538D0`, unit free
`0x00555600`. Quest items (expiry 0) never expire.

#### 9.3 Cube spill (`0x00563840`)

When the dropped item is the cube (`box `): for each item of the
player's item list on page 3, in list order: queue 0x9D action 5 now
(item flag 0x20 set in the message's flags argument; stored page shown
as 3), unlink, room-change notice, mode 4, page := 0, and place with
§2.4 steps 2–9 into page 0 (find free, send). Placement fails → page :=
0xFF and the item is dropped next to the player (§9.1 steps 2–3).

### 10. Gold

#### 10.1 Gold pickup (`0x0055C850`)

limit = level × 10000 (`0x00622E70`); g = player gold (stat 14), p =
pile gold. take = p, rest = 0; g + p > limit → take = limit − g, rest =
p − take. Pile owner (`0x00552FD0`) missing or not a player: party share
(`0x00554630` ≠ −1 → `0x00540900`, multiplayer) else add take to stat 14
(stat set to 0 when the sum is negative, or for a player when it would
exceed the limit); pile owner a player → `0x0053FF00(take)`. Then the
pile leaves the room, unit flag 0x2 cleared, freed (`0x00557FD0`); rest
> 0 → a new pile of rest at the player (`0x0055B030`) and sound on the
player.

#### 10.2 Gold piles (`0x0055A090`, unit, amount, out list, max)

Item class `gld` (`0x00633640`); per pile while placed < amount and
count < max: pile = min(rest, 2,000,000,000); start and search as §9.1
step 2 (last argument 0); none → stop; create the item (`0x00559CE0`,
code `gld`, `items/generation.md`); pile gold := pile (0 if negative);
ground placement (§9.1 step 3); append to the list.

#### 10.3 Gold messages

Inventory gold reaches the client through the per-client vitals sync
`0x00548760` (`combat/vitals.md` §5, which owns when it runs): when stat 14 differs
from the client's cached value (client data +0x14), `0x0053E9B0(new,
old)` sends: new − old in 1..254 → 0x19 [delta u8]; else new < 0xFF →
0x1D [0x0E][u8]; new < 0xFFFF → 0x1E [0x0E][u16]; else 0x1F [0x0E][u32];
then the cache := new.

### 11. Message layouts

Machine copy: `server-messages.tsv` `layout` column (rows 0x19, 0x22,
0x3F, 0x42, 0x47, 0x48, 0x7D, 0x9C, 0x9D). Little-endian.

| Id | Bytes |
|---|---|
| 0x9C | [0]=0x9C, [1] action, [2] total size, [3] category, [4..7] item GUID (−1 none), [8..] item bit stream |
| 0x9D | [0]=0x9D, [1] action, [2] total size, [3] category, [4..7] item GUID, [8] owner unit type (6 none), [9..12] owner GUID (−1 none), [13..] item bit stream |
| 0x7D | [1] owner type (6 none), [2..5] owner GUID, [6..9] item GUID (−1 none), [10..13] flag (0x100 or 0x200), [14..17] state (the flag test's value) |
| 0x47 / 0x48 | [1] unit type (6 none), [2] 0 (0x47) or the argument (0x48; 0 from `0x00580860`), [3..6] unit GUID, [7..10] 0 |
| 0x42 | [1] unit type (6 none), [2..5] GUID |
| 0x3F | [1] code (0xFF when the "reset" argument ≠ 0), [2..5] item GUID (0 when no item), [6..7] u16 argument |
| 0x22 | 12 bytes (`0x0053C520`; ECX = client, DL = unit type, stack GUID, skill, quantity): [1] unit type (6 none), [2] unwritten, [3..6] unit GUID (−1 none), [7..8] skill id, [9] quantity (low byte), [10] unwritten, [11] 1 when the unit found by (type, GUID) in the client's game has state 7 (`playerbody`, `0x00639DF0`), else 0 |

Bit stream: written by `0x006313E0(item, buffer, 0xF4, 0, 0, arg)`
(format: `items/bitstream.md`): the item's item flags are temporarily
OR-ed with the sender's flag argument, serialized, then restored
(`0x006280D0`), then `0x0053EA50(flags)`: when the item is socketed
(`0x00629900`) and `flags` lacks 0x20, for each filler of its
inventory in list order one 0x9D action 0x13 (`0x0053CEF0`: owner =
the parent item, type 4 and its GUID; flag argument = `flags` | 0x8;
arg 0), after its parent. The dispatcher (§6.2) passes flags 0 and arg
0; the cube spill passes 0x20 (§9.3: no fillers). The
wrapper `0x0053D330` (action 0x13) has no caller. Total size ≥ 0xFD → fatal.
Category ([3], `0x00623D60`): items `component` (+0x115); except an item
of body location 4 or 5 when both hands hold non-broken items, neither
two-handed, the owner is a player of class 4 or 6 or a monster of class
0x1A1/0x1A2, and the item is not the weapon in use → 6.

## Constants & data dependencies

| Constant | Value | Use |
|---|---|---|
| inventory signature | 0x01020304 | §1.1 |
| body-location grid / belt grid | 13 × 1 / 16 × 1 (`0x0074479C` / `0x007447B4`) | §1.2 |
| class → record table | `0x00744544`, 7 pairs | §1.3 |
| similar-potion groups | `0x00744684` (5), `0x0074466C` (5), `0x00744660` (2) | §3.4 |
| default belt record | 2 (4 boxes) | §3.1 |
| ground pickup / walk ranges | ≤ 50 / < 5 | §7.1 |
| ground range for 0x29 / 0x2A | 10 per axis | §5 |
| use range (0x20) | 50 per axis | §7.11 |
| gold limit | level × 10000 | §7.22, §10 |
| pile cap / piles per drop | 2,000,000,000 / 32 | §7.22 |
| ground expiry | 15000 / 30000 / 45000 frames | §9.2 |
| town levels | 1, 40, 75, 103, 109 (`0x006426A0`) | §7.3 |
| quest-pickup table | `0x00731FEC`, 4 pairs | §8.4 |

Tables read: `inventory` (gridX, gridY), `belts` (numboxes), `itemtypes`
(body, bodyloc1/2, beltable, class, equiv chain), items (invwidth,
invheight, reqstr, reqdex, component, belt, autobelt, quest, useable,
maxstack), `levels` (via town test). Layout owners: `data/fields.tsv`.

## Randomness

No draw in the grid, belt, equip, requirement or message code. Draws
happen only inside the systems these paths call, in handler order:

1. 0x50 drop gold and §10.1 rest piles: item creation of each `gld` pile
   (`items/generation.md` §3), pile by pile.
2. 0x61 take from hireling: the duplicate `0x0055A2A0`
   (`world/cube.md` OQ4).
3. Item use (0x20, 0x26, potions on the hireling): item-use spec.
4. Free-spot searches (§9.1, §10.2): `sim/path-placement.md` (whether
   `0x0064E810` draws is that spec's to state).

## Edge cases & original bugs

1. 0x22 always returns 3 for an owned item (§7.13).
2. The auto-belt gate is always true (§3.6).
3. 0x18's "not busy" page check is unreachable (§7.3 step 3).
4. 0x63 with a failed slot placement leaves the item in limbo (§7.24).
5. `0x005491B0` / `0x005492F0` pass a missing item (§5).
6. 0x25 ignores `numboxes` and similarity (§7.16); 0x23 ignores
   `numboxes` (§3.7).
7. The 0x18 cube page needs no open cube (§7.3).
8. 0x16 picks up items to the cursor only when the client asks
   (cursor flag); the server never decides it.
9. Weighted searches fail when every fitting spot has weight 0 (cannot
   happen in a non-empty search space: a free region always touches an
   edge or an item).

## Test vectors

Synthetic (CI): grid cells as (x, y, w, h) occupied rectangles.

| # | Input | Expected | Source |
|---|---|---|---|
| T1 | empty 10 × 4, item 1 × 1, player / other | (9, 3) / (9, 0) | §2.3 |
| T2 | empty 10 × 4, item 2 × 3 or 2 × 2, player / other | (0, 0) / (0, 0) | §2.3 |
| T3 | 10 × 4 with (0, 0, 2, 3), item 1 × 1 | player (0, 3) (weight 3), other (9, 0) | §2.3 |
| T4 | 10 × 4 with (0, 0, 2, 3), item 2 × 2 | (2, 0) both | §2.3 |
| T5 | 10 × 4 with (9, 3, 1, 1), item 1 × 1 / 1 × 2 | player (9, 2), other (9, 0) / (0, 0) both | §2.3 |
| T6 | cube 3 × 4 empty, item 1 × 1 / 2 × 4 | player (2, 3), other (2, 0) / (0, 0) | §2.3 |
| T7 | stash 6 × 8 with (0, 0, 2, 4), item 2 × 4 | (0, 4) both (player weight 8) | §2.3 |
| T8 | 10 × 4 full except (4, 1), item 1 × 1; 10 × 4 full | (4, 1); none | §2.3 |
| T9 | weight of 1 × 1 at (5, 1) in an empty 10 × 4 / at (0, 0) | 0 / 2 | §2.3 |
| B1 | no belt (4 boxes), slot 0 = `hp1`, item `hp2` with autobelt 0 / ≠ 0 (synthetic records) | none / slot 1 | §3.5 |
| B2 | sash (8), slot 1 = `mp3`, item `mp1` | slot 5 | §3.5 |
| B3 | girdle (16), slots 0, 4, 8 = `hp1`, item `hp4` | slot 12 | §3.5 |
| B4 | default belt, slots 0–3 empty, item `rvs` with autobelt ≠ 0 | slot 0 | §3.5 |
| B5 | compaction of column 1 after slot 5 emptied, items at 1, 9, 13 | 9 → 5, 13 → 9 | §3.8 |
| E1 | §4.3 location 1 with N, head empty / occupied | 1 / 5 | §4.3 |
| E2 | §4.3 location 4, N = two-handed sword (not 1-or-2 for a sorceress), left hand holds a shield | 2 | §4.3, §4.4 |
| E3 | §4.3 location 4, barbarian, N = one-handed sword, left holds an axe, right empty | 1 | §4.4 |
| E4 | §4.3 location 4 without N, right empty, left holds a bow (two-handed) | 4 | §4.3 |
| G1 | 0x50 amount 0 | result 0, no pile, no draw | §7.22 |
| G2 | level 1 (limit 10000), gold 9500, pile 1000 | gold 10000, new pile 500 | §10.1 |
| G3 | 0x19 new 120, old 100 / new 400, old 100 / new 70000 | `19 14` / `1E 0E 90 01` / `1F 0E 70 11 01 00` | §10.3 |
| X1 | 0x22 on an owned item | result 3 | §7.13 |

Real 1.14d (game files, `#[ignore]`):

| # | Check | Expected | Source |
|---|---|---|---|
| D1 | `inventory.bin` count and records 0–15 gridX × gridY | 32; the §1.3 table | measured 2026-10-06 |
| D2 | `belts.bin` count, numboxes 0–13 | 14; 12, 8, 4, 16, 8, 12, 16, 12, 8, 4, 16, 8, 12, 16 | measured |
| D3 | itemtypes ids used here | 2 `shie`, 3 `tors`, 4 `gold`, 18 `book`, 22 `scro`, 27 `bow`, 28 `axe`, 30 `swor`, 33 `spea`, 34 `pole`, 37 `helm`, 45 `weap`, 67 `h2h`, 71 `phlm`, 76 `hpot`, 80 `apot`, 81 `wpot` | `itemtypes.bin` |

Recordings (conformance; none recorded yet):

| # | Player does (single player, Normal, Act 1 town) | Compare |
|---|---|---|
| R1 | Drop a ring from the cursor; pick it up to the cursor; drop again; click it (auto pickup) | C→S 0x17/0x16 and every S→C 0x9C/0x9D/0x3F (bytes, frame) |
| R2 | Move an item inside the inventory: lift (0x19), place (0x18) at a chosen cell, swap with another item (0x1F); stash one item (page 4); open the cube and put one item in (0x18 page 3) | grid positions in 0x9C bit streams, action order |
| R3 | Equip, swap and unequip: helm (0x1A/0x1C), weapon over a shield (0x1D), a two-handed weapon over sword + shield (0x1B), weapon swap key | 0x9D actions 6/7/8/9/0x17, 0x47/0x48 placement |
| R4 | Belt: pick up three `hp1` with an empty belt column and a sash equipped; shift-click a potion from the inventory (0x63); take one from the belt (0x24); swap (0x25); drink one from slot 4 (0x26) | slots, compaction, direct 0x9D 5 + 0x9C 0xE |
| R5 | Gold: drop 1, 255, 70000 gold (0x50); pick each up | piles, 0x19/0x1E/0x1F bytes, RNG trace of pile creation |
| R6 | Pick up an item with a full inventory; equip an item whose strength requirement is not met | refused-pickup bytes (sound 0x17), result codes |

## Provenance

Read from the 1.14d `Game.exe` (Ghidra exports and `tools/ghidra/disasm.py`
for register arguments): handlers `0x0054AAD0`–`0x0054B710`, `0x0054C800`,
`0x0054D430`, `0x0054D520`; checks `0x005490E0`–`0x00549350`; grid code
`0x0063A8A0`, `0x0063AFD0`, `0x0063B340`–`0x0063B950` (disassembled: loop
orders, weight sides, the four-way split), `0x0063ADD0`, `0x0063AF20`,
`0x0063AF70`, `0x0063AAF0`; belt `0x0063C600`, `0x0063C790`, `0x0063C4F0`,
`0x00628A40`, `0x00628BA0` (disassembled: the −1 compare), `0x0055EDC0`;
equip `0x0063DE60` and `0x0063DD90` (disassembled: register operands),
`0x0063DBC0`, `0x0062ED50`, `0x0062EAF0`, `0x005606B0`, `0x0055D710`;
item routines `0x00560200`, `0x00560420`, `0x00560CD0`, `0x00560F00`,
`0x00561B00`, `0x00563D20`, `0x0055E7C0`, `0x0055E590`, `0x0055E9B0`
(and the `xor eax, eax; ret 8` of `0x0055E9A0`), `0x00562250`,
`0x0055EB30`, `0x00562390`, `0x00562660`, `0x0055EF20`, `0x00563560`,
`0x0055CF50`, `0x0055C9A0`, `0x0055CC90`, `0x00563C00`, `0x00563840`,
`0x00558AA0`, `0x00558A10`, `0x00535510`, `0x0055A090`, `0x0055C850`,
`0x0054D130`, `0x0054D230` (disassembled: hireling class constants);
messages `0x005973F0`, `0x00597890`, `0x00580860`, `0x0055BED0`,
`0x0053CEF0`, `0x0053EAE0` and their wrappers `0x0053D010`–`0x0053D4B0`,
`0x0053EC00`–`0x0053EF10` (disassembled: action bytes), `0x0053D2E0`,
`0x0053D220`, `0x0053D370`, `0x0053D3C0`, `0x0053D440`, `0x0053E9B0`.
Data tables read from `Game.exe` with `pefile` (class table, potion
groups, quest-pickup pairs). `inventory.bin`, `belts.bin`, `itemtypes.bin`
measured on the 1.14d install.

D2MOO 1.10f (`D2Common/src/D2Inventory.cpp`, `D2Game/src/ITEMS/ItemMode.cpp`,
`INVENTORY/InvMode.cpp`, `PLAYER/PlrMsg.cpp`) was used as a map of names
and structure only; every rule above was re-read in 1.14d. Differences
found: 1.14d routes the free-position search through four wrappers (same
result); grid 0 is the body-location grid and grid 1 the belt (D2MOO
labels them the other way); 1.14d's refused pickup does not re-mark the
item for a refresh message the D2MOO way but sets unit flag 0x1000 and
re-enters mode 3; 0x22 is a stub; hireling item rules (§7.23) and the
dual-wield monster classes are 1.14d constants.

Second pass (open questions 3–19, HANDOFF IV/WN/MV/PN/GX items): read
from the 1.14d decompile and `tools/ghidra/disasm.py` (register
arguments) at the addresses cited in §2.2, §3 rules 5 and 7–10, §4.2,
§4.4, §4.5, §4.7, §4.8, §5.5–§5.7, §6.1 rule 4, §6.2, §6.3, §7.1, §7.6,
§7.7, §7.9, §7.11, §7.18, §7.23, §8.1, §8.4, §9.2, §11 (0x22); constant
tables read from the image (`0x00738C70`, `0x00738C4C`, `0x006CE270`);
itemtypes, itemstatcost, states and inventory row numbers measured on
the 1.14d `patch_d2` `.txt` files (the `Expansion` row skipped). The
"charm re-link" of the first pass is the scroll/tome item-skill link
(§5.5).

## Open questions

1. Answered: `items/bitstream.md` (owner; checked on all 144 recorded
   0x9C / 0x9D streams).
2. Order of 0x9C/0x9D relative to other per-player update messages in
   one client pass (life, stats, 0x47/0x48). Settle: R1–R3 packet order.
3. Answered: §3 rule 7 (no `numboxes` check server-side; slot ≤ 15).
4. Answered: §4.2 step 2 (`pct`, `combat/damage.md` §0).
5. Answered: §4.8.
6. Answered: §5.5 (item-skill quantity of scrolls/tomes, not charms; cube
   recount `0x0055FA40`), §5.6, §5.7 (inventory pass).
7. Answered: §4.5.
8. Answered: §4.7.
9. Answered: §6.1 rule 4 (`0x00597B00`, per-item reset `0x005979B0`).
10. Answered: §7.1 step 2 (types 0, 3, 5).
11. Answered: §7.4 step 2 (`0x00549A60` = "can't do that" 0x5A).
12. Answered by the code (§7.4, `0x00560420`): a busy player has no page
    restriction there; R2 (stash and cube use while the panels are
    open) confirms on live data.
13. Answered: §3 rules 9–10 (from the binary; a recording removing a belt with potions in rows 2–4 would confirm the 0x9C order: R3).
14. Answered: §7.9 (0x1E), §7.11 (0x20), §7.18 (0x27), §8.1 step 4 (pickup specials). The use effects behind `0x005BF240` stay with the item-use spec (`world/cube.md` OQ7).
15. Answered: §7.12 (stat 72 = `durability`).
16. Answered: §7.23 step 2 (used skill).
17. Answered: §9.2 (reader `0x00558B90`).
18. Answered: `combat/vitals.md` §5 owns the sync (when it runs, the
    10 % life gate, force every 20 ticks or 10 with queued messages);
    R5 still confirms the gold bytes and timing.
19. Answered: §8.4 step 6 (full pair list; second list = corpses).

Answered handoff questions (`docs/HANDOFF.md` §7):

- WN1: both statements hold; unit flag 0x10 is the "not yet announced"
  flag, cleared by the room clean-up (`0x0055325A`); a new ground item is
  announced by the unit-add path, later changes by `0x0055BED0` (§6.3,
  from `0x0053A500`, `0x00571F90`, `0x00553220`).
- GX1: record 29 (Hireling2) is 255 × 255 because its `.txt` grid is −1;
  records 16–31 are not copies (§1.3 table).
- WN2: answered on `origin/claude/spec-answers-inventory` (§7.6: no
  cursor clear in `0x00563D20`; X stays the cursor item).
- IS1: §6.1 rule 4 (the clean-up clears +0xC8 bit 0 only; bit 1 is
  cleared by the character save `0x00532400`).
- IS2: no spec change: `sim/tick.md` §6 rule 5 runs the unit updates
  (`0x0053A620`) inside the per-client update, before the room switch
  `0x00537B50`; running them after the tick is a wiring difference.
- GX2: §1.3 (pages 0 and 5–255 take the class record). GX3: §3 rule 1
  (no type test; only `belt` items reach location 8 in 1.14d).
- MV2: §11 (flag argument 0 from the dispatcher; fillers: owner = the
  parent item, flag | 0x8; `0x0053D330` has no caller). MV3: §6.2 (0x7D
  state = item flags & flag), §9.2 (absolute 0).
- MV4 (failure results, read per site): §7.7 no item from `0x0063E490`
  → out 1; §7.8 E not in mode 1, N's placement or link failing → out 1,
  N missing or not in mode 4 → out 0; §7.10 target not in mode 0 →
  nothing (out 0), C's link failing → out 1; §7.16 C's link failing →
  fatal assert (line 0x12D4); §7.19 only "filler missing / not in mode
  4 / target missing" set out, every other check → 0 with out 0. Not
  re-read here: §7.17, §7.23 copy, §8.1 rules 5 and 7, §9.3 unlink,
  §10.2 pile creation (Ghidra on `0x00562390`, `0x0054D130`,
  `0x0055D0D0`, `0x00563840`, `0x0055A090`).
- MV5: type tests through `0x00629BB0` use itemtypes equivalence; those
  through `0x0062B400` compare the primary type only (§4.7 type 38;
  §7.6–§7.8 type 19; §7.12 and §7.20 the book type 18); §7.12 reads the
  max stack of the item it fills first and announces it first
  (`0x0055E590`); §7.16 both items join the update list.
- MV6: §8.2 a refused pickup returns at once (no pickup sound,
  `0x0055D01C`); 0x26 reads bytes 1–8 only (`0x0054B560`: +1, +5),
  bytes 9–12 are unread.
