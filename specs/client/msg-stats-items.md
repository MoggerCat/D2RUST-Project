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
| Summary | 41–53 |
| Inputs | 54–60 |
| Outputs / state changes | 61–65 |
| Rules | 66–67 |
|   1. Local player stats: 0x19–0x1F (`0x0045D780`) | 68–97 |
|   2. Item actions: 0x9C ItemActionWorld (`0x0045EB10`), 0x9D ItemActionOwned (`0x0045EC70`) | 98–168 |
|   3. Other item messages | 169–198 |
|   4. Hireling stats: 0x9E–0xA2 (`0x0045D540`) | 199–214 |
| Constants & data dependencies | 215–223 |
| Randomness | 224–227 |
| Edge cases & original bugs | 228–237 |
| Test vectors | 238–269 |
| Provenance | 270–287 |
| Open questions | 288–309 |
<!-- /index -->

Owned ids: 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x3F, 0x42,
0x47, 0x48, 0x9C, 0x9D, 0x9E, 0x9F, 0xA0, 0xA1, 0xA2.

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
of the cursor item (0x42), item flag 4 (0x3F).

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
