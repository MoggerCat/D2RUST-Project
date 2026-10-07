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
  messages). Machine table: `items/item-actions.tsv` (`inventory-moves.md` §6).
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
| Summary | 48–67 |
| Inputs | 68–78 |
| Outputs / state changes | 79–86 |
| Rules | 87–88 |
|   1. Inventory model | 89–181 |
|   2. Grid placement | 182–277 |
|   3. Belt | 278–335 |
|   4. Equipping | 336–520 |
|   5. Shared checks | 521–645 |
| Constants & data dependencies | 646–668 |
| Randomness | 669–682 |
| Edge cases & original bugs | 683–727 |
| Test vectors | 728–776 |
| Provenance | 777–833 |
| Open questions | 834–923 |
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
update walks the update list (`inventory-moves.md` §6). This spec owns the model, the
placement and search rules, the belt and equip rules, the requirement
check, the item-move handlers and the deferred-message dispatcher.

Split: §6–§11 (deferred item messages, intents, pickup, drop, gold,
message layouts) moved unchanged to `items/inventory-moves.md` (same
section numbers and rule ids; the two files share no id, so a bare §
reference is unambiguous). Constants, Randomness, Edge cases, Test
vectors, Provenance and Open questions for both files stay here.

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
0x48 (`inventory-moves.md` §6, §11); result codes (`intents-events.md` §2.3).

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
| item data | +0x14 | command flags (`inventory-moves.md` §6) |
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
7. Cursor := none (unless step 3 said not), mode := 0 (stored). The
   clear (`0x0063C180(inventory, none)`, §1.4 rule 3) does not test what
   the cursor holds: only step 3's page-1 flag gates it (`0x00560200`;
   the call at `0x0056035C` follows only the flag test at `0x00560353`).
   A caller placing a mode-4 item that is not the cursor item while the
   player holds one orphans the held item (edge case 12).
8. "Send" set: command flag 0x2; item flag 0x1 when the item is socketed
   with fillers (`0x0055F590`); item flag 0x4000 cleared; owner refresh
   (`0x00621000`, `inventory-moves.md` §6.1); update list += item.
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

#### 4.9 Equip without the cursor (`0x00562E00`, ECX game, EDX unit U, item I, skip)

Callers, all with skip 0: auto pickup `inventory-moves.md` §8.1 step 5 (`0x005636DE`), the
vendor buy §7.1 rule 9.7 of `world/vendors.md` (`0x00577D90`), and
`0x00562F30` (`0x005631F6`; called from `0x0057FB70`).

1. §4.7(U, I, &L, skip) fails → result 0, nothing changed.
2. Put I at L on grid 0 (`0x0063BDB0`); failure → fatal assert
   (`ItemMode.cpp` line 0x1674). L = 11 or 12 → fatal assert (0x1656;
   §4.7 never returns them). Link check `0x0063B210(kind 3)` fails →
   location L emptied again (`0x0063BE30`), then fatal assert (0x1671).
3. Body location := L (`0x00627D70`); stat link `0x0063D1D0`; cursor :=
   none (§1.4 rule 3); stat refresh `0x0055C2C0(U, 0)`; unit flag 0x2
   cleared; mode 1; command flag 0x200 (`item-actions.tsv` row 5: 0x9D
   action 6 to every client); unit flag 0x2000000 cleared; update list +=
   I; owner refresh; page := 0xFF; weapon bookkeeping `0x0055C5C0`.
   Result 1.

Differences from §4.6: no mode test and no §4.3 (the caller's), no
weapon-in-use update `0x006233A0`, command flag 0x200 instead of 0x8, no
item flag 0x1, item flag 0x4000 not cleared, no inventory pass, no out
argument.

Re-read in full (handoff A6, `prop-unified-items` Q3): the steps above
are the whole routine. Neither game nor U's inventory (unit +0x60) is
tested for none; L is written into the skip argument's slot after step
1. When each caller tries it: `inventory-moves.md` §8.1 step 5 (after §4.3), `world/vendors.md`
§7.1.1 (buy), and `0x00562F30` (the corpse take-back of `0x0057FB70`,
`inventory-moves.md` §7.1 step 2; corpse spec, not specified here).

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
item's GUID, 0xFFFF; `inventory-moves.md` §11). Runs first in most item routines (cited as
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
   (`0x0053C520`; `inventory-moves.md` §11).
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
7. Owner refresh (`0x00621000(U, 1)`, `inventory-moves.md` §6.1).
8. send ≠ 0: S→C 0x48 (type U, arg 0, U's GUID; `0x0053D3C0`) to U's
   client, or for a non-player to its owner's (`0x0058F0D0`) client when
   that owner is a player.

## Constants & data dependencies

| Constant | Value | Use |
|---|---|---|
| inventory signature | 0x01020304 | §1.1 |
| body-location grid / belt grid | 13 × 1 / 16 × 1 (`0x0074479C` / `0x007447B4`) | §1.2 |
| class → record table | `0x00744544`, 7 pairs | §1.3 |
| similar-potion groups | `0x00744684` (5), `0x0074466C` (5), `0x00744660` (2) | §3.4 |
| default belt record | 2 (4 boxes) | §3.1 |
| ground pickup / walk ranges | ≤ 50 / < 5 | `inventory-moves.md` §7.1 |
| ground range for 0x29 / 0x2A | 10 per axis | §5 |
| use range (0x20) | 50 per axis | `inventory-moves.md` §7.11 |
| gold limit | level × 10000 | `inventory-moves.md` §7.22, §10 |
| pile cap / piles per drop | 2,000,000,000 / 32 | `inventory-moves.md` §7.22 |
| ground expiry | 15000 / 30000 / 45000 frames | `inventory-moves.md` §9.2 |
| town levels | 1, 40, 75, 103, 109 (`0x006426A0`) | `inventory-moves.md` §7.3 |
| quest-pickup table | `0x00731FEC`, 4 pairs | `inventory-moves.md` §8.4 |

Tables read: `inventory` (gridX, gridY), `belts` (numboxes), `itemtypes`
(body, bodyloc1/2, beltable, class, equiv chain), items (invwidth,
invheight, reqstr, reqdex, component, belt, autobelt, quest, useable,
maxstack), `levels` (via town test). Layout owners: `data/fields.tsv`.

## Randomness

No draw in the grid, belt, equip, requirement or message code. Draws
happen only inside the systems these paths call, in handler order:

1. 0x50 drop gold and `inventory-moves.md` §10.1 rest piles: item creation of each `gld` pile
   (`items/generation.md` §3), pile by pile.
2. 0x61 take from hireling: the duplicate `0x0055A2A0`
   (`world/cube.md` OQ4): two game-seed steps per item unit it allocates,
   2 · (1 + k) with k fillers (`world/vendors.md` §7.3).
3. Item use (0x20, 0x26, potions on the hireling): item-use spec.
4. Free-spot searches (`inventory-moves.md` §9.1, §10.2): `sim/path-placement.md` (whether
   `0x0064E810` draws is that spec's to state).

## Edge cases & original bugs

1. 0x22 always returns 3 for an owned item (`inventory-moves.md` §7.13).
2. The auto-belt gate is always true (§3.6).
3. 0x18's "not busy" page check is unreachable (`inventory-moves.md` §7.3 step 3).
4. 0x63 with a failed slot placement leaves the item in limbo (`inventory-moves.md` §7.24).
5. `0x005491B0` / `0x005492F0` pass a missing item (§5).
6. 0x25 ignores `numboxes` and similarity (`inventory-moves.md` §7.16); 0x23 ignores
   `numboxes` (§3.7).
7. The 0x18 cube page needs no open cube (`inventory-moves.md` §7.3).
8. 0x16 picks up items to the cursor only when the client asks
   (cursor flag); the server never decides it.
9. Weighted searches fail when every fitting spot has weight 0 (cannot
   happen in a non-empty search space: a free region always touches an
   edge or an item).
10. 0x1F takes the target before testing the cursor item's fit and
    restores nothing (`inventory-moves.md` §7.10 rule 4): with a failed placement (e.g. a
    2 × 2 cursor item at x = 9 of a 10-wide page) the old cursor item is
    left in mode 4, in no grid and not the cursor; the target is the new
    cursor item; result 3.
11. Each successful 0x1F lowers inventory +0x28 by one below the number
    of linked items (`inventory-moves.md` §7.10 rule 6). Readers of +0x28 (getter
    `0x0063CD60`, callers `0x004843E0`, `0x0048C060`, `0x0055F590`,
    `0x00562660`, `0x005697F0`; the socket link `0x0063B210` uses it as
    the filler's x): the ones read (`0x004843E0`, `0x0055F590`,
    `0x00562660`, `0x0063B210`) take an item's own (socket) inventory,
    where 0x1F never runs. `0x0048C060` (client item text) also reads
    the item's own socket inventory (`item +0x60`, after the socketed
    flag 0x800). `0x005697F0` (save writer `0x00569AD0`, "JM" corpse
    section) reads the inventories of the player's corpses (+0x34 list,
    player-type units) and only tests the count for zero; a corpse
    inventory is new and filled by links (`0x0057F700`: `0x0063B210`
    per moved item), so it carries no 0x1F drift. No reader of +0x28
    sees a player's own inventory: the drift changes no outcome.
12. §2.4 step 7 clears the cursor whatever it holds. A transmute
    (`world/cube.md` §8 step 3: outputs are created in mode 4) while the
    player holds a cursor item H unlinks H (§1.4 rule 3) and leaves it in
    mode 4, in no grid and not the cursor. Nothing on the server's
    transmute path tests for a cursor item (`0x00568060` → `0x00566AE0` →
    `0x005665F0`); whether the client sends it while holding an item is
    open question 20. A ground item offered by 0x2A never reaches step 7
    (step 2 refuses mode 3; `world/cube.md` edge case 15).
13. A cube spill item whose drop search finds no room stays in mode 4,
    in no grid and not the cursor (`inventory-moves.md` §9.3).

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
| G1 | 0x50 amount 0 | result 0, no pile, no draw | `inventory-moves.md` §7.22 |
| G2 | level 1 (limit 10000), gold 9500, pile 1000 | gold 10000, new pile 500 | `inventory-moves.md` §10.1 |
| G3 | 0x19 new 120, old 100 / new 400, old 100 / new 70000 | `19 14` / `1E 0E 90 01` / `1F 0E 70 11 01 00` | `inventory-moves.md` §10.3 |
| X1 | 0x22 on an owned item | result 3 | `inventory-moves.md` §7.13 |

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
| R7 | Cube open with a valid recipe (e.g. 3 `gcv` chipped gems): hold another item on the cursor and press Transmute; then swap (0x1F) a 2 × 2 item onto a 1 × 1 item at the inventory's last column | whether the client sends 0x4F 0x18 / the 0x1F at all; the held item's later 0x9C messages (edge cases 10, 12) |

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
re-enters mode 3; 0x22 is a stub; hireling item rules (`inventory-moves.md` §7.23) and the
dual-wield monster classes are 1.14d constants.

Second pass (open questions 3–19, HANDOFF IV/WN/MV/PN/GX items): read
from the 1.14d decompile and `tools/ghidra/disasm.py` (register
arguments) at the addresses cited in §2.2, §3 rules 5 and 7–10, §4.2,
§4.4, §4.5, §4.7, §4.8, §5.5–§5.7, `inventory-moves.md` §6.1 rule 4, §6.2, §6.3, §7.1, §7.6,
`inventory-moves.md` §7.7, §7.9, §7.11, §7.18, §7.23, §8.1, §8.4, §9.2, §11 (0x22); constant
tables read from the image (`0x00738C70`, `0x00738C4C`, `0x006CE270`);
itemtypes, itemstatcost, states and inventory row numbers measured on
the 1.14d `patch_d2` `.txt` files (the `Expansion` row skipped). The
"charm re-link" of the first pass is the scroll/tome item-skill link
(§5.5).

Third pass (property-test questions, MV4 sites): `0x00561B00`,
`0x00560200`, `0x0063AAF0` with `0x0063A810` (disassembled: the list
removal is a no-op for an item that is neither head, tail nor linked),
`0x0063CD60` and its callers, `0x00562E00` (disassembled: ECX / EDX
arguments, the three callers and their skip 0), `0x00563560`,
`0x005600A0`, `0x0055D0D0`, `0x00562390` (disassembled:
`0x00562494`–`0x005624A0`), `0x0054D130` (disassembled: return values),
`0x0055FB10`, `0x00563840`, `0x0055A090`, `0x00555DA0` (disassembled:
size and fallback pushes at `0x00563B9C` / `0x00563C83`), `0x005628C0`.

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
9. Answered: `inventory-moves.md` §6.1 rule 4 (`0x00597B00`, per-item reset `0x005979B0`).
10. Answered: `inventory-moves.md` §7.1 step 2 (types 0, 3, 5).
11. Answered: `inventory-moves.md` §7.4 step 2 (`0x00549A60` = "can't do that" 0x5A).
12. Answered by the code (`inventory-moves.md` §7.4, `0x00560420`): a busy player has no page
    restriction there; R2 (stash and cube use while the panels are
    open) confirms on live data.
13. Answered: §3 rules 9–10 (from the binary; a recording removing a belt with potions in rows 2–4 would confirm the 0x9C order: R3).
14. Answered: `inventory-moves.md` §7.9 (0x1E), `inventory-moves.md` §7.11 (0x20), `inventory-moves.md` §7.18 (0x27), `inventory-moves.md` §8.1 step 4 (pickup specials). The use effects behind `0x005BF240` stay with the item-use spec (`world/cube.md` OQ7).
15. Answered: `inventory-moves.md` §7.12 (stat 72 = `durability`).
16. Answered: `inventory-moves.md` §7.23 step 2 (used skill).
17. Answered: `inventory-moves.md` §9.2 (reader `0x00558B90`).
18. Answered: `combat/vitals.md` §5 owns the sync (when it runs, the
    10 % life gate, force every 20 ticks or 10 with queued messages);
    R5 still confirms the gold bytes and timing.
19. Answered: `inventory-moves.md` §8.4 step 6 (full pair list; second list = corpses).
20. Does the 1.14d client send C→S 0x4F button 0x18 (transmute) while
    an item is on the cursor (edge case 12)? Settle: recording R7.
21. ~~Readers of inventory +0x28 at `0x005697F0` and `0x0048C060`~~:
    answered in edge case 11 (socket and corpse inventories only).

Answered handoff questions (`docs/HANDOFF.md` §7):

- WN1: both statements hold; unit flag 0x10 is the "not yet announced"
  flag, cleared by the room clean-up (`0x0055325A`); a new ground item is
  announced by the unit-add path, later changes by `0x0055BED0` (`inventory-moves.md` §6.3,
  from `0x0053A500`, `0x00571F90`, `0x00553220`).
- GX1: record 29 (Hireling2) is 255 × 255 because its `.txt` grid is −1;
  records 16–31 are not copies (§1.3 table).
- WN2: answered on `origin/claude/spec-answers-inventory` (`inventory-moves.md` §7.6: no
  cursor clear in `0x00563D20`; X stays the cursor item).
- IS1: `inventory-moves.md` §6.1 rule 4 (the clean-up clears +0xC8 bit 0 only; bit 1 is
  cleared by the character save `0x00532400`).
- IS2: no spec change: `sim/tick.md` §6 rule 5 runs the unit updates
  (`0x0053A620`) inside the per-client update, before the room switch
  `0x00537B50`; running them after the tick is a wiring difference.
- GX2: §1.3 (pages 0 and 5–255 take the class record). GX3: §3 rule 1
  (no type test; only `belt` items reach location 8 in 1.14d).
- MV2: `inventory-moves.md` §11 (flag argument 0 from the dispatcher; fillers: owner = the
  parent item, flag | 0x8; `0x0053D330` has no caller). MV3: `inventory-moves.md` §6.2 (0x7D
  state = item flags & flag), `inventory-moves.md` §9.2 (absolute 0).
- MV4 (failure results, read per site): `inventory-moves.md` §7.7 no item from `0x0063E490`
  → out 1; `inventory-moves.md` §7.8 E not in mode 1, N's placement or link failing → out 1,
  N missing or not in mode 4 → out 0; `inventory-moves.md` §7.10 target not in mode 0 →
  nothing (out 0), C's link failing → out 1; `inventory-moves.md` §7.16 C's link failing →
  fatal assert (line 0x12D4); `inventory-moves.md` §7.19 only "filler missing / not in mode
  4 / target missing" set out, every other check → 0 with out 0. Not
  re-read here: `inventory-moves.md` §7.17, §7.23 copy, `inventory-moves.md` §8.1 rules 5 and 7, §9.3 unlink,
  `inventory-moves.md` §10.2 pile creation (Ghidra on `0x00562390`, `0x0054D130`,
  `0x0055D0D0`, `0x00563840`, `0x0055A090`).
- MV5: type tests through `0x00629BB0` use itemtypes equivalence; those
  through `0x0062B400` compare the primary type only (§4.7 type 38;
  `inventory-moves.md` §7.6–§7.8 type 19; `inventory-moves.md` §7.12 and §7.20 the book type 18); `inventory-moves.md` §7.12 reads the
  max stack of the item it fills first and announces it first
  (`0x0055E590`); `inventory-moves.md` §7.16 both items join the update list.
- MV6: `inventory-moves.md` §8.2 a refused pickup returns at once (no pickup sound,
  `0x0055D01C`); 0x26 reads bytes 1–8 only (`0x0054B560`: +1, +5),
  bytes 9–12 are unread.
- MV4, the rest (re-read): `inventory-moves.md` §7.17 no hireling → the player is the
  target; `inventory-moves.md` §7.23 `0x0054D130` returns 3 (not 2) for classic / no hireling
  inventory / location ∉ 1..10, and a failed copy is fatal; `inventory-moves.md` §8.1 rule 5
  `0x00562E00` is §4.9 (result 0 unreachable there), rules 6–7 link
  failures fatal; `inventory-moves.md` §8.1 rule 4 auto-stack failures fatal; `inventory-moves.md` §9.3 unlink
  failure fatal, a roomless drop leaves the item in mode 4; `inventory-moves.md` §10.2 a
  failed pile creation skips the pile, the loop goes on.
- `docs/handoff/prop-unified-items.md` Q1: `inventory-moves.md` §7.10 rules 4–6, edge cases
  10–11 (T is taken first; nothing restored). Q2: §2.4 step 7, edge
  case 12 (unconditional clear; 0x2A ground items never reach it; the
  client side is OQ20). Q3: §4.9 (`0x00562E00` written; failure →
  nothing changed, result 0; not reachable from `inventory-moves.md` §8.1).
- IV1: §1.4 rule 3 (the cursor item is not in the item list). IV2:
  §2.4 step 5. IV3: §2.4 step 3 (after step 8, before step 9). IV4: §3
  rule 5 (next column). IV5: §3 rule 8 (item flags). IV6: §4.7 steps
  1–2. IV7: §1.3 (no record for item owners). IV8: §4.4 rule 6 (no).
  MV1: `inventory-moves.md` §6.2. MV7: OQ1, 6, 9–17, 19 above (answered); the remaining
  `MovePending` seams are code. PN1: §2.2 (32-bit wrap: the item is
  placed without cells). WN3: `inventory-moves.md` §7.7 (empty location → 0 before §4.3).
