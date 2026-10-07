# Spec: Client — Stat and item messages

- **Status:** draft: handlers read in the 1.14d `Game.exe` (addresses
  below); stat values checked against the single-player recordings
  `traces/raw/20261006-015956-packets.jsonl` and
  `-022633-packets.jsonl`; the item actions are dispatched only (their
  placement rules wait for the item bit-stream spec); unverified: no
  executable check runs it yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge` handlers for the ids below, over
  the model of `client/model.md`.
- **Related specs:** `client/model.md` (model §1, unit table §2, local
  player §3, bit reader §10); `sim/stat-lists.md` §5 (set and add on a
  unit); `sim/stats.md` (stat ids, total and base getters);
  `items/inventory.md` §6, §11 (the server side of 0x9C / 0x9D / 0x3F /
  0x42 / 0x47 / 0x48; open question 1: the item bit stream);
  `items/item-actions.tsv` (action labels).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–55 |
| Inputs | 56–62 |
| Outputs / state changes | 63–68 |
| Rules | 69–70 |
|   1. Local player stats: 0x19–0x1F (`0x0045D780`) | 71–100 |
|   2. Item actions: 0x9C ItemActionWorld (`0x0045EB10`), 0x9D ItemActionOwned (`0x0045EC70`) | 101–171 |
|   3. Other item messages | 172–201 |
|   4. Hireling stats: 0x9E–0xA2 (`0x0045D540`) | 202–217 |
|   5. Item state messages: 0x3E, 0x40, 0x7C, 0x7D, 0x92, 0x97, 0xA6 | 218–303 |
| Constants & data dependencies | 304–312 |
| Randomness | 313–316 |
| Edge cases & original bugs | 317–332 |
| Test vectors | 333–371 |
| Provenance | 372–395 |
| Open questions | 396–421 |
<!-- /index -->

Owned ids: 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x3F, 0x42,
0x47, 0x48, 0x9C, 0x9D, 0x9E, 0x9F, 0xA0, 0xA1, 0xA2; §5: 0x3E, 0x40,
0x7C, 0x7D, 0x92, 0x97, 0xA6.

## Summary

Stat messages write the local player's stat list (0x19–0x1F) or a
player's (0x20) and run a small post-write hook. Item messages carry an
action byte and an item bit stream: the client checks the action
against a fixed table (some actions are fatal, high ones are ignored)
and calls one action handler, which parses the stream, creates or
updates the item unit and places it. This spec owns the stat rules, the
action tables and what the model records for each item message; where
an item lands (ground, cursor, grid, belt, body) is decided by the
stream's header, which `items/inventory.md` open question 1 has to
specify first.

## Inputs

| Name | Type | Source |
|---|---|---|
| message | id + bytes | `client/model.md` §4 |
| local player | `client/model.md` §3 | model |

## Outputs / state changes

Unit stats (layer 0 base values), item records on item units, removal
of the cursor item (0x42), item flag 4 (0x3F); item stats and flags,
stat-list links, `weapon_set`, the runtime item table (§5). No outputs.

## Rules

### 1. Local player stats: 0x19–0x1F (`0x0045D780`)

1. A local player is required: none → fatal assert 0x9AA (bridge:
   handler error).
2. Per id (jump table `0x0045D85C`):

   | Id | Stat s | New value |
   |---|---|---|
   | 0x19 SmallGoldPickup | 14 (gold) | total(14) + u8@1 |
   | 0x1A AddExpByte | 13 (experience) | total(13) + u8@1 |
   | 0x1B AddExpWord | 13 | total(13) + u16@1 |
   | 0x1C AddExpDword | 13 | u32@1 (absolute, not added) |
   | 0x1D SetStatByte | u8@1 | u8@2 |
   | 0x1E SetStatWord | u8@1 | u16@2 |
   | 0x1F SetStatDword | u8@1 | u32@2 |

   total(s) is the unit total getter `0x00625480(unit, s, 0)`
   (`sim/stats.md`); the write is a base set, layer 0
   (`0x00627260`, `sim/stat-lists.md` §5 rule 2).
3. Then the hook (rule 5) with (s, new value).
4. **0x20** StatUpdate (`0x0045D880`): GUID u32@1, stat u8@5, value
   u32@6. Look up (0, GUID) (players only); none → nothing; else set
   (stat, value) and run the hook on that unit.
5. **Hook** (`0x0045D4B0(stat, unit, value)`): stat 6 (life) with value
   ≠ 0 on a unit in mode 0x11 (dead) → `0x00480E70`, `0x004647D0`
   (leave the dead mode; `client/model.md` open question 1). Stat 12
   (level) → `0x0045D3B0`; if the unit is the local player also
   `0x0045D3E0` and `0x004C1C10`. Stats 0 and 2 → `0x004C1C10`. Others:
   nothing. (`0x0045D3B0`, `0x0045D3E0`, `0x004C1C10`: open question 2.)

### 2. Item actions: 0x9C ItemActionWorld (`0x0045EB10`), 0x9D ItemActionOwned (`0x0045EC70`)

1. Layouts (`items/inventory.md` §11): 0x9C action u8@1, size u8@2,
   category u8@3, item GUID u32@4, stream @8 (size − 8 bytes); 0x9D the
   same plus owner type u8@8, owner GUID u32@9, stream @13 (size − 13
   bytes).
2. The handler copies the message and dispatches on the action byte:
   action > 0x17 → nothing; a fatal action → fatal assert (0x9C: 0xF6D,
   0x9D: 0xFB7); else the action handler:

   | Action | 0x9C handler | 0x9D handler | Server label (`item-actions.tsv`) |
   |---|---|---|---|
   | 0x00 | `0x004C25B0` | fatal | (new on ground) |
   | 0x01 | `0x004C2650` | fatal | GroundToCursor |
   | 0x02 | `0x004C26F0` | fatal | dropped (`inventory.md` §6.3) |
   | 0x03 | `0x004C2810` | fatal | on ground (§6.3) |
   | 0x04 | `0x004C2AD0` | fatal | PutInContainer |
   | 0x05 | fatal | `0x004C2C80` | RemoveFromContainer |
   | 0x06 | fatal | `0x004C2E90` | Equip |
   | 0x07 | fatal | `0x004C3070` | IndirectlySwapBodyItem |
   | 0x08 | fatal | `0x004C3380` | Unequip |
   | 0x09 | fatal | `0x004C3920` | SwapBodyItem |
   | 0x0A | `0x004C3B30` | fatal | AddQuantity |
   | 0x0B, 0x0C | `0x004C3C00` | fatal | – |
   | 0x0D | `0x004C40D0` | fatal | SwapInContainer |
   | 0x0E | `0x004C4130` | fatal | PutInBelt |
   | 0x0F | `0x004C42A0` | fatal | RemoveFromBelt |
   | 0x10 | `0x004C45C0` | fatal | SwapInBelt |
   | 0x11 | fatal | `0x004C4740` | AutoUnequip |
   | 0x12 | `0x004C20B0` | fatal | ToCursor |
   | 0x13 | fatal | `0x004C4990` | (socket filler, `inventory.md` §11) |
   | 0x14 | fatal | `0x004C4AA0` | Unknown0x14 |
   | 0x15 | fatal | `0x004C4C70` | UpdateStats |
   | 0x16 | fatal | `0x004C2340` | Unknown0x16 |
   | 0x17 | fatal | `0x004C3B00` | WeaponSwitch |

3. Every action handler first parses the stream header
   (`0x0062E410(stream, bytes, 0, header)`), then finds the item unit
   (4, GUID) in set S: known → updated from the header, or removed and
   re-created when the header says so (`0x006287A0`); unknown → created
   from the stream (`0x004C0F20` / `0x004C0FF0`, the item unit's
   creation through `client/model.md` §2 rule 4). 0x9D handlers also
   need the local player and the owner unit (owner type, owner GUID);
   missing → nothing. The handler then places the item (ground point,
   cursor, container grid, belt, body location) and refreshes the
   owner's requirement flags (`0x004C1350`, rule 3.3) and UI.
4. Model, until the stream is specified (open question 3): the item
   unit (4, GUID) is created on its first 0x9C / 0x9D (class unknown,
   `position` none) and its kind data records the last {message id,
   action, category, owner key (0x9D), stream bytes}. A fatal action is
   a handler error; an action > 0x17 changes nothing.
5. **Cursor item** (the local player's `cursor_item`, cleared by 0x42,
   §3 rule 1). Two 0x9C actions set it; both use the **local player's**
   inventory (`0x00463DD0` +0x60), whoever the stream names:
   1. 0x12 ToCursor (`0x004C20B0`): no local player → fatal 0xB0C. An
      existing (4, GUID) is removed (`client/model.md` §2 rule 5); the
      item is created from the stream (`0x004C0F20`); no inventory →
      fatal 0xB13; cursor item := the new item (`0x0063C180(inventory,
      item)`, `0x004C2137`), UI refresh (`0x00468070`).
   2. 0x01 GroundToCursor (`0x004C2650`): only when the stream header's
      byte +8 is 4 (else nothing); (4, GUID) already in S → fatal
      0x3A0; created from the stream (none → fatal 0x3A2); cursor item
      := it (`0x004C26CC`); UI refresh; `0x004C2180(item)`.
   Model: `cursor_item` := (4, GUID) of the created item. The other
   action handlers that call `0x0063C180` (`0x004C2340`, `0x004C26F0`,
   `0x004C2970`, `0x004C2C80`, `0x004C2E90`, `0x004C3070`,
   `0x004C3380`, `0x004C3760`, `0x004C3B30`, `0x004C3F60`,
   `0x004C4130`, `0x004C42A0`, `0x004C44D0`, `0x004C4740`,
   `0x004C4990`, `0x004C4C70`) set or clear it as part of their
   placement rule: open question 6.

### 3. Other item messages

1. **0x42** ClearCursor (`0x0045E270` → `0x004C2050`): type u8@1, GUID
   u32@2. If that unit is the local player and its inventory (+0x60)
   has a cursor item (`0x0063C1E0`): cursor item := none
   (`0x0063C180`), UI cursor refresh (`0x00468070`), the former cursor
   item is removed (`client/model.md` §2 rule 5), then `0x004B29E0`.
   Otherwise nothing. Model: the local player's `cursor_item` (kind
   data) := none and the item unit removed.
2. **0x3F** UseStackableItem (`0x0045E220` → `0x004C4620`): code u8@1,
   item GUID u32@2, argument u16@6.
   1. argument = 0xFFFF and code = 0xFF: item (4, GUID) in S → item
      flag 4 := 0 (`0x006280D0(item, 4, 0)`) and `0x004C2180`; then
      `0x00468070(0)` (cursor cleared).
   2. Otherwise: for each entry of the code table `0x00727A40` (2
      entries: 0, 2) equal to code, if `0x00453A90(1)` is 0 →
      `0x00455F20(1, 0)` (with argument ≠ 0xFFFF the scan stops there).
      Then item (4, GUID) in S → argument = 0xFFFF: item flag 4 := 1;
      both: `0x00468010(item, code)` (use-item cursor). Item not in S →
      nothing more.
   3. Model: item flag 4 per rule 2.1 / 2.2 on the item's kind data
      (`flags4: bool`), and `use_cursor` := {item, code} or none.
3. **0x47** Relator1 (`0x0045E2A0` → `0x004C1BC0`) and **0x48** Relator2
   (`0x0045E2D0` → `0x004C1BF0`): type u8@1, GUID u32@3 (byte 2: 0 or
   an argument, unused here). Unit in S → `0x004C1350(unit)`: if the
   unit is a player, or has unit flag 0x200, and has an inventory, the
   requirement flag 0x4000 of its items is recomputed (item flags,
   `0x006280D0(item, 0x4000, …)`); the rule body is open question 4.
   Model: no field until then.

### 4. Hireling stats: 0x9E–0xA2 (`0x0045D540`)

1. Layout (`sim/server-messages.tsv`; senders `0x0053BEE0`,
   `0x0053BFD0`): stat u8@1, GUID u32@2, value @6: 0x9E u8 (set), 0x9F
   u16 (set), 0xA0 u32 (set), 0xA1 u8 (add), 0xA2 u16 (add) (jump
   table `0x0045D5C4`, read from the image).
2. Look up (1, GUID) (monsters only, whatever the sender's unit;
   `client/model.md` §2 rule 2); none → nothing.
3. Stat 12 (level) → first the requirement refresh of 0x47 on that
   unit (`0x0045D3B0`: it builds `47 <type> 00 <GUID>` and calls
   `0x004C1BC0`, §3 rule 3).
4. Then set (`0x00627260(unit, stat, value, 0)`, base layer 0) for
   0x9E–0xA0, or add (`0x006272B0`, same arguments) for 0xA1, 0xA2
   (`sim/stat-lists.md` §5 rules 2, 3). No hook (contrast §1 rule 5).
5. Model: the unit's `stats` entry for the stat.

### 5. Item state messages: 0x3E, 0x40, 0x7C, 0x7D, 0x92, 0x97, 0xA6

1. **0x3E** UpdateItemStats (`0x0045E130` → `0x004C1F30`; size u8@1,
   min 2). The bit reader (`client/model.md` §10) runs over bytes
   2 … size − 1, fields in order:

   | Field | Bits |
   |---|---|
   | item GUID | 1 bit a; a = 0 → 8 bits; else 1 bit b: 16 (b = 0) or 32 |
   | set flag | 1 |
   | stat | 9 |
   | value | as the GUID: 8, 16 or 32 |
   | param | 1 bit c: 8 (c = 0) or 16 |

   Item (4, GUID) absent → nothing.
   1. stat 204 (`item_charged_skill`): only with a local player:
      `0x004C1E40(item, skill = param >> [data +0xC6C], level = param &
      [data +0xC70], charges = value & 0xFF)`: key := (skill <<
      [+0xC6C]) + level; in the item's stat lists with flag 0x40
      (`0x00625760` / `0x00625730`, list order) the first stat-204
      entry with layer key: max := entry >> 8; max in 1 … 255 → entry
      := max << 8 | min(charges, max) (charges < 1 → 0)
      (`0x00627220(list, 204, v, key, item)`) and stop; max 0 or > 255
      → stop with nothing written. No entry → the same search on each
      item of the item's own inventory (+0x60), in node order, until one
      writes.
   2. Any other stat: set flag = 1 → base stat := value
      (`0x00627260(item, stat, value, 0)`; param unused). Then, with a
      local player: stat ≠ 70 (`quantity`) → the requirement refresh
      `0x004C1350` on the **local player** (§3 r3); stat 70 and value > 0
      → item flags 4 and 0x4000 := 0 (`0x006280D0`).
   Model: the item's stats, item flags 4 / 0x4000, the refresh's item
   flags.
2. **0x40** ItemFlags (`0x0045E240` → `0x004C2020`, 13 bytes): GUID
   u32@1, mask u32@5, value u32@9. Item (4, GUID) present → item flags
   (item data +0x18): value ≠ 0 → |= mask, else &= ~mask (`0x006280D0`).
   Model: item flags.
3. **0x7C** UseScroll (`0x0045E910` → `0x004C51B0`, 6 bytes): type
   u8@1, GUID u32@2. Unit present → `0x004C2180(unit)` (also run by
   0x3F's rule 2.1): its state-54 stat list (`0x006256B0(unit, 54)`)
   present → state 54 off (`0x00639DB0(unit, 54, 0)`), the list
   unlinked (`0x006277E0`) and freed (`0x00626CD0`), then the unit's
   stats refreshed (`0x00623F50`). Model: the unit's states and stat
   lists (`client/stat-lists.md`).
4. **0x7D** SetItemState (`0x0045E930` → `0x004C2270`, 18 bytes):
   owner type u8@1, owner GUID u32@2, item GUID u32@6, code u32@10,
   value u32@14. Item (4, u32@6) or owner absent → nothing. loc := the
   item's body location on the owner (`0x00623D60`).
   - code 0x100: item flag 0x100 := value (`0x006280D0`); the owner's
     gfx and body slot refreshed (`0x0046F950`, `0x004C1290(loc)`,
     `0x00470610(owner, 0)`); the item's stats linked to the owner
     (`0x00663CC0(owner, item, 1, 1)`); requirement refresh
     `0x004C1350(owner)`.
   - code 0x200: item flag 0x100 := 0; `0x004C12F0(owner, item, the
     item's mode, loc)`; stats unlinked (`0x00663CC0(owner, item, 0,
     0)`); `0x004C1350(owner)`.
   - other codes: nothing.
   Model: item flag 0x100, the owner's stat-list links
   (`sim/stat-lists.md`), item flags of the refresh. The gfx calls are
   render (`render/unit-composite.md` reads the model).
5. **0x92** RemoveItemsDisplay (`0x0045E5B0` → `0x004C23E0`, 6 bytes):
   type u8@1, GUID u32@2. Unit U with an inventory: for each node in
   order whose kind is 3 (body), or 1 when `0x0062FF70(item, U)` holds:
   gfx refresh (`0x0046F950`), the item unlinked (`0x0063D2B0`) and
   re-added (`0x0063AD90`; none → fatal 0xD5A, another item → fatal
   0xD5B); kind 3 → its body slot cleared (`0x0063C110`,
   `0x0063BE30`); stats linked (`0x00663CC0(U, item, 1, 1)`); a player
   (type 0) → `0x0063BEF0(inventory)`; item flag 0x100 clear →
   `0x006277F0(U, item)`; `0x004C1350`. A node without an item is
   fatal 0xD4F. Finally `0x0063E0B0(inventory)`. Model: U's inventory
   and stat links (helpers: `items/inventory.md`).
6. **0x97** WeaponSwitch (`0x0045EAD0`, 1 byte; id byte ≠ 0x97 is fatal
   0xF46, unreachable): `0x0048A700`: when the `d2exp.mpq` check
   (`0x00408F20`) and the expansion flag `[0x007A04F4]` (`0x0044DCC0`)
   are both non-zero, the active weapon set `[0x007BCC4C]` := 1 − itself.
   Model: `weapon_set` (0 / 1) of the local player.
7. **0xA6** (`0x0045EDC0`; size u16@2, min 4; sender `0x0053E1C0`): code
   u8@1, index u16@4, record @6. Code ≠ 0 → nothing. Code 0 →
   `0x00639CC0(index, record)`: 0x120 bytes are copied from @6 into
   entry `index` of the runtime item table `[0x0096CA9C]` (count
   `[0x0096CA98]`; index ≥ count → the table is reallocated to index + 1
   entries, the new ones zeroed, count := index + 1). The copy is always
   0x120 bytes, whatever the message size. Model: that table
   (`item_table_ext`), read through `0x00639D60(i)` (0 < i < count,
   else fatal 0x972); meaning open question 7.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| stat ids | 6 life, 12 level, 13 experience, 14 gold; 0 strength, 2 dexterity | `0x0045D780`, `0x0045D4B0` |
| fatal asserts | 0x9AA (no local player), 0xF6D / 0xFB7 (fatal action) | §1, §2 |
| action range | 0x00–0x17; higher ignored | `0x0045EB36`, `0x0045EC97` |
| 0x3F code table | `0x00727A40` = {0, 2}, count `[0x00727A48]` = 2 | §3 rule 2 |

## Randomness

None.

## Edge cases & original bugs

- 0x19, 0x1A, 0x1B add to the **total** (with item and state bonuses)
  and store the sum as the **base**: with a non-zero bonus on gold or
  experience the base drifts by the bonus on every message.
- 0x1C sets experience absolutely while 0x1A / 0x1B add.
- 0x20 looks up players only (type 0), whatever unit it was meant for.
- An item action > 0x17 is silently ignored; an unlisted action in range
  is fatal.
- 0x3E's set flag 0 changes nothing for stats other than 204 except the
  refresh; stat 204 ignores the flag.
- 0xA6 copies 0x120 bytes from the message whatever its size (a short
  message reads past its end in 1.14d; d2rs: a message shorter than
  0x126 bytes is a handler error).
- 0x97 toggles; two 0x97 in one frame cancel out.

## Test vectors

From `traces/raw/20261006-022633-packets.jsonl` ("B") and
`-015956-packets.jsonl` ("A"); synthetic where marked.

| Input | Expected | Source |
|---|---|---|
| `1d 00 0a` | stat 0 := 10 | B 115 |
| `1e 07 00 28` | stat 7 := 0x2800 | B 119 |
| `1d 0c 01` | stat 12 := 1; hook: level path (local player) | B 122 |
| `1b ad 01` with experience total 0 | stat 13 := 0x1AD | B 141 |
| `1a 42` with experience 0x1AD (no bonus) | stat 13 := 0x1EF | synthetic |
| `1c 00 00 01 00` | stat 13 := 0x10000 | synthetic |
| `19 05` with gold total 100, base 90 (bonus 10) | gold base := 105 | synthetic, edge case 1 |
| 0x1D with no local player | handler error (fatal 0x9AA) | synthetic |
| `9c 0e 14 10 01 00 00 00 10 00 a2 00 65 08 00 80 06 17 03 02` | action 0x0E → `0x004C4130`; item (4, 1) created; record {0x9C, 0x0E, category 0x10, stream 12 bytes} | B 123 |
| `9d 06 21 05 07 00 00 00 00 01 00 00 00 11 00 …` | action 6 → `0x004C2E90`; owner (0, 1); item (4, 7) | B 129 |
| 0x9C action 0x05 | handler error (fatal 0xF6D) | synthetic |
| 0x9C action 0x20 | no change | synthetic |
| `42 00 01 00 00 00` with a cursor item (4, 9) | cursor none, (4, 9) removed | A 77949 (cursor state synthetic) |
| `42 00 01 00 00 00` without a cursor item | no change | synthetic |
| 0x9C action 0x12 for item (4, 9), local player placed | (4, 9) re-created; `cursor_item` = (4, 9) | synthetic, §2 rule 5.1 |
| 0x9C action 0x12 with no local player | fatal 0xB0C → handler error | synthetic |
| 0x9C action 0x01 for (4, 9) already in S, header byte +8 = 4 | fatal 0x3A0 → handler error | synthetic, §2 rule 5.2 |
| `3f ff 05 00 00 00 ff ff` with item (4, 5) | item flag 4 := 0; use cursor none | synthetic |
| `3f 04 05 00 00 00 ff ff` with item (4, 5) | item flag 4 := 1; use cursor {(4, 5), 4} | synthetic |
| `47 00 00 01 00 00 00 00 00 00 00` | (0, 1) found → requirement refresh | A 76132 |
| `9e 07 0a000000 05`, monster (1, 10) | stat 7 := 5 | synthetic, §4 |
| `a2 0d 0a000000 6400`, (1, 10) stat 13 = 1000 | stat 13 = 1100 | synthetic, §4 |
| `a0 0c 0a000000 0b000000`, (1, 10) | requirement refresh, then stat 12 := 11 | §4 rule 3 |
| `9e 07 0a000000 05`, only player (0, 10) | nothing | §4 rule 2 |
| 0x40 `40 07000000 00010000 01000000`, item (4, 7) | item flag 0x100 set | synthetic, §5 r2 |
| 0x40 same with value 0 | item flag 0x100 cleared | synthetic |
| 0x3E bits: a 0, GUID (8 bits) 7, flag 1, stat 70, value: a 0, 8 bits = 3; param: c 0, 8 bits = 0; item (4, 7), local player present | quantity := 3; item flags 4, 0x4000 cleared | synthetic, §5 r1.2 |
| 0x3E stat 204, key matches an entry with max 10, charges 12 | entry := 0x0A0A | synthetic, §5 r1.1 |
| 0x97 twice, expansion game | `weapon_set` back to its start value | synthetic, §5 r6 |
| 0x7D code 0x300 | nothing | synthetic, §5 r4 |
| 0xA6 code 0, index 3, count 2 | table count 4, entry 2 zero, entry 3 = bytes 6…0x125 | synthetic, §5 r7 |

## Provenance

§4 (2026-10-07): `0x0045D540` (table `0x0045D5C4`), `0x0045D3B0`,
`0x004C1BC0`, `0x00627260`, `0x006272B0`.

1.14d `Game.exe`: stats `0x0045D780` (jump table `0x0045D85C`, 7
entries), `0x0045D880`, hook `0x0045D4B0`; items `0x0045EB10` (byte
table `0x0045EC54`, pointer table `0x0045EC20`), `0x0045EC70` (byte
table `0x0045EDA0`, pointer table `0x0045ED70`), action handlers listed
in §2, `0x0062E410`, `0x004C0F20`, `0x004C0FF0`; `0x004C2050`,
`0x004C4620`, `0x004C1BC0`, `0x004C1BF0`, `0x004C1350`. Tables read
from the image with `tools/ghidra/disasm.py`'s loader. Action labels
from `items/item-actions.tsv` (server side). Recorded stat bytes from
both recordings (join: 0x1D / 0x1E for stats 0, 1, 2, 3, 7, 9, 11, 12;
0x1B experience). Cursor (join-update session, 2026-10-06): `0x004C20B0`,
`0x004C2650`, `0x0063C180` (callers listed in §2 rule 5, found by a
scan for calls to it).

Area 4 session (2026-10-07): §5 from `0x0045E130`, `0x004C1F30`
(tail `0x004C1F95`–`0x004C2012`), `0x004C1E40`, `0x0045E240` →
`0x004C2020`, `0x004C51B0`, `0x004C2180`, `0x004C2270`, `0x004C23E0`,
`0x0045EAD0` → `0x0048A700`, `0x0045EDC0` → `0x00639CC0`, `0x00639D60`;
stat names from `itemstatcost`.

## Open questions

1. Which stat each recorded 0x1D / 0x1E sets is fixed by its byte; the
   meaning of the player-creation values (`client/msg-units.md` §1.1)
   versus these: check by replaying the join and comparing the client's
   stat list (recorder: dump the local player's stat list after frame
   2).
2. `0x0045D3B0`, `0x0045D3E0`, `0x004C1C10` (level and attribute
   hooks): UI and requirement effects; Phase 6 UI spec.
3. The item stream header (`0x0062E410`) and each action handler's
   placement rule: after `items/inventory.md` open question 1, a client
   item spec takes §2 rule 3 to the bar.
4. `0x004C1350` requirement refresh: the full rule (it walks the body
   locations and the grid, tests requirements `0x004C10E0`, sets item
   flag 0x4000).
5. Answered: 0x21, 0x22, 0x23, 0x94 are owned by `client/msg-skills.md`
   (skill list, §3–§6 there).
6. `cursor_item` in the other item actions (§2 rule 5's list): per
   handler, whether it passes the action's item, a swapped-out item or
   0 to `0x0063C180`; with the header byte +8 of the GroundToCursor
   test named (open question 3's header spec).
7. The runtime item table of 0xA6 (`[0x0096CA9C]`, 0x120-byte entries,
   built at load by `0x006394A0`, read by `0x00639D60` and
   `0x0062BED0`): which table it is and whether a single-player server
   ever sends 0xA6 (no static caller of `0x0053E1C0` found).
