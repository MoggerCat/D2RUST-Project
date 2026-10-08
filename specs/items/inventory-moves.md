# Spec: Items — Deferred item messages and item-move intents (§6–§11 of the inventory spec)

- **Status:** draft: grids, placement search, belt, equip checks, the
  deferred item-message dispatcher and every handler's validation order
  read from the 1.14d `Game.exe` (addresses below); grid sizes measured
  on the 1.14d `inventory.bin` / `belts.bin`; no recording replayed yet
  (test vectors T1–T9 are synthetic or table facts; R1–R6 need
  recordings).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::items::moves` (intent handlers, deferred item messages). Machine table: `items/item-actions.tsv` (§6).
- **Related specs:** `items/inventory.md` (owner of the inventory model §1, grid placement §2, belt §3, equipping §4, shared checks §5, and of its Constants, Randomness, Edge cases, Test vectors, Provenance and Open questions; this file holds §6–§11 moved out of it unchanged, rule ids kept); `sim/intents-events.md`; `sim/client-messages.tsv`; `sim/server-messages.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 27–33 |
| Rules | 34–35 |
|   6. Deferred item messages | 36–129 |
|   7. Intents | 130–681 |
|   8. Pickup from the ground | 682–863 |
|   9. Drop to the ground | 864–911 |
|   10. Gold | 912–951 |
|   11. Message layouts | 952–981 |
|   12. Corpse take-back (`0x0057FB70` → `0x00562F30`) | 982–1109 |
<!-- /index -->

## Summary

§6–§11 of `items/inventory.md`, split out of that file to keep it readable
(section numbers and rule ids are unchanged, so a reference names
this file and the same §). Status, evidence and open questions are
those of `items/inventory.md`.

## Rules

### 6. Deferred item messages

#### 6.1 Marking

1. Handlers set command flags (item data +0x14, `0x00628170`) and item
   flags, append the item to the owner's update list (`inventory.md` §1.4) and call the
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
   (needs P's state 7 `playerbody`; §12; `combat/vitals.md` §4.7 rule 2;
   item take-back §8.5) →
   0; else `0x00566E60` (player-to-player interaction, wall-clock
   throttled with `GetTickCount`; multiplayer, out of scope) → 0. Type 5
   (tile): missing or distance > 50 → 1; distance < 5 → warp
   `0x005550B0` (`sim/path-placement.md` §12.2) → 0; else walk to it →
   0. Type 3 → 1. Type 4 (item):
   1. Item missing or mode ≠ 3, or distance (`0x00641530`, unit to unit;
      formula: `sim/pathing.md` §9.5 "Unit distance")
      > 50 → 1.
   2. Distance ≥ 5, or a collision between player and item
      (`0x00622B50`, mask 0x804) → walk to the item (`0x00548A50`:
      player mode 3 toward (type 4, GUID) and interaction data −1, or
      −2 when the cursor flag is set; arrival is the movement spec's) →
      0.
   3. Else cursor flag ≠ 0 → §8.2 (to cursor); 0 → §8.1 (auto). Out ≠ 0
      → 3, else 0.

#### 7.2 0x17 DropItem (`0x0054AB40`)

1. Cursor item check (`inventory.md` §5) ≠ 0 → that result.
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
4. Item lookup missing → 2. Page := page; `inventory.md` §2.4 at (x, y), send. Success →
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

Cursor item check; body location (u8 @5) ∉ 1..10 → 2; `inventory.md` §4.6 with skip 0;
refused with out → 3 (the item lookup is repeated with no effect); else 0.

#### 7.6 0x1B Swap2HandedItem (`0x0054AE30` → `0x00563D20`)

Cursor item check; location ∉ 1..10 → 2. `0x00563D20`: location must be
4 or 5 (else out 1); the other hand must hold an item X (else out 1); `inventory.md` §4.3
must give 2 (else out 1); requirements (not equipping) fail → stat
refresh, sound, 0 (out 0). Then X leaves the body (`0x0062A360`,
`0x0063D2B0`, unlink, slot cleared `0x0063BE30`; X of type 19 (belt) →
`0x005608C0`, `inventory.md` §3 rule 9, with no new belt) and becomes the cursor item (mode 4, unit
flag 0x2 cleared; no command flag and no update-list entry for X). N then
goes to the location (`0x0063BDB0`) and is linked (kind 3; link failure →
out 1, 0): body location, stat link, stat refresh, unit flag 0x2
cleared, mode 1, page 0xFF, command flag 0x10000 (0x9D action 7), item
flag 0x1, 0x4000 cleared, update list, weapon bookkeeping, inventory
pass. The cursor is **not** cleared afterwards: X stays the cursor item
(read at `0x00563D20`; `inventory.md` §4.6 step 5's "cursor := none" does not apply). If
the put at the location fails, N is left detached, X is the cursor, and
the result is still 1 (original bug). Owner refresh; result 0 or 3.

#### 7.7 0x1C RemoveBodyItem (`0x0054AEC0` → `0x00560CD0`)

Location u16 @1 ∉ 1..10 → 2. Item-move gate for the item at the location
refuses → 0. Location 8 (belt) needs `0x00567840` (`inventory.md` §3 rule 10),
else 0. `0x00560CD0`: cursor present → 0; empty location → 0 (checked before `inventory.md` §4.3, so `inventory.md` §4.3 result 4 never occurs here: 0x1C on the empty hand opposite a two-handed weapon does nothing, `0x00560CD0` read in full); `inventory.md` §4.3 (N
absent) must give 3 or 4, else out 1; `0x0063E490` picks the item to
remove (for 4: the two-handed item in the other hand); remove from body
as in §7.6; type 19 → `0x005608C0` (`inventory.md` §3 rule 9, no new belt); cursor := item; stat refresh; unit
flag 0x2 cleared; mode 4; command flag 0x10 (0x9D action 8); item flag
0x1 when socket-filled; 0x4000 cleared; update list; refresh; weapon
bookkeeping; inventory pass. Result 0 or 3.

#### 7.8 0x1D SwapCursorWithBody (`0x0054AF50` → `0x00560F00`)

Cursor item check; location ∉ 1..10 → 2; empty location → 1; location 8
needs `0x00567840`; item-move gate on the equipped item. `0x00560F00`:
`inventory.md` §4.3 must give 5 (else out 0, result 0); weapon-in-use update; the
equipped item E (via `0x0063E490`) must be in mode 1; stat refresh;
requirements of N (not equipping) fail → stat refresh, sound, 0. N of type
19 → `0x005608C0(N)` (`inventory.md` §3 rule 9). E: removed, cursor := E, mode 4, item flag 0x80,
command flag 0x20, item flag 0x1, update list. N: placed at the location,
body location set, stat link, mode 1, page 0xFF, item flags 0x40 and 0x1,
command flag 0x20; update list; weapon bookkeeping; inventory pass. Both
send 0x9D action 9.

Failure outcomes (`0x00560F00` re-read 2026-10-07): game or player none
→ fatal assert; location > 10 → 0 with out untouched (set only after
this test); cursor item missing, not an item, or not in mode 4 → out 0,
0; E none or E's mode ≠ 1 → out 1, 0 (nothing changed). The unlink of E
(`0x0063AD90`) not returning E → fatal assert (`0x00560F18`; the
handler never sees it). N's put at the location (`0x0063BDB0`) failing
→ out 1, 0 after E is already the cursor item (N is then neither
cursor nor placed); unreachable, since §4.3's 5 means the location
fits N once E has left. The link check (kind 3) failing → out 1, 0;
unreachable for a player (`0x0063B210` passes when the owner is not
an item, §12.2 phase 1 step 5).

#### 7.9 0x1E Swap1HWith2H (`0x0054B030` → `0x00561220`)

Cursor item check; location ∉ 1..10 → 2; location ∉ {4, 5} → 3; empty
location → 1; item-move gate; `0x00561220(game, player, N's GUID, L,
&out)` (the cursor item N goes to hand L; the item T at L to the cursor;
the item X in the other hand to page 0):

1. out := 0. N must be an item in mode 4, else 0.
2. `inventory.md` §4.3 (L, N, skip 0) ≠ 7 → 0. `inventory.md` §4.2 (not equipping) fails → out 1, 0.
3. r := page-0 grid record (`inventory.md` §1.3); O := 4 when L = 5, else 5; X := item
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
   Deactivation of T; `inventory.md` §4.2 for N (not equipping) fails → stat refresh,
   cursor := N, refused-pickup sound `0x0055FB10(N)`, `0x00553380`,
   owner refresh, 0 (X stays moved).
6. T leaves the body as X did, cursor := T, unit flag 0x2 cleared, mode
   4, command flag 0x10, item flag 0x1, 0x4000 cleared, update list.
7. N put at L (`0x0063BDB0`) and linked (kind 3; either failing → out
   1, 0): body location L, stat link, stat refresh, unit flag 0x2
   cleared, mode 1, page 0xFF, item flag 0x8, command flag 0x8, item
   flag 0x1 (and when socket-filled), 0x4000 cleared, update list,
   weapon bookkeeping `0x0055C5C0`, inventory pass (`inventory.md` §5.7). Result 1.

#### 7.10 0x1F SwapCursorBufferItem (`0x0054B0F0` → `0x00561B00`)

Fields: cursor u32 @1, target u32 @5, x u32 @9, y u32 @13.

1. Cursor item check, then stored item check on the target.
2. Target missing → 2; target page 1 or 2 → 3; item-move gate → 0.
3. `0x00561B00`: target page 3 and the cursor item is the cube (`box `,
   class cached at `0x0088C6FC`) → 0 (no cube in a cube); target page 2
   → out 1. Target mode must be 0. Target T: unlink, stat refresh,
   room-change notice, page := 0xFF, cursor := T, item-skill unlink, stored
   page := page, unit flag 0x2 cleared, mode 4, command flag 0x40000,
   item flag 0x1 if socket-filled, update list. Cursor item C: `inventory.md` §2.2 at
   (x, y) on the same page (fail → out 1); stored page 0, page := page,
   x, y set; link; item-skill link; stat refresh if active; mode 0; command
   flag 0x40000; update list; refresh; inventory pass if T was active.
   Both send 0x9C action 0xD.
4. Order and failures (`0x00561B00` read in full): T is taken (rule 3,
   up to its update-list entry) **before** C's fit test, and nothing is
   restored on a later failure. C's `inventory.md` §2.2 placement failing → out 1,
   result 0: T stays the cursor item in mode 4 (its update-list entry,
   command flag 0x40000, goes out at the next owner refresh, wherever T
   is by then); C stays in mode 4, in no grid and not the cursor (its
   item data +0x5C still names the inventory); no owner refresh runs.
   C's link check (`0x0063B210`, kind 1) failing → out 1, result 0 with
   C placed in the grid but in mode 4 (as `inventory.md` §2.4 step 5). Edge case 10.
5. Item flag 0x4000 is cleared on both items: on T before C's placement,
   on C after its item flag 0x1 test. The page-2 trade hook
   `0x00568770` after C's link is unreachable (page 2 returned in rule 3).
6. C's placement unlinks C as a listed item (it is no longer the cursor,
   `inventory.md` §1.4 rule 1): the list is unchanged but the count drops by 1, and the
   link adds 1. A successful 0x1F leaves inventory +0x28 one below the
   number of linked items (edge case 11).

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
`0x0055E7C0`: either missing → out 1; dst page 2 → out 1; `inventory.md` §4.5 fails → 0
(handler 0). Same class → merge (`0x0055E590`): with q_s, q_d the stat-70
quantities and m the max stack (`0x006295B0`, `items/generation.md`
§1.3): q_s + q_d > m → dst := m, src := q_s + q_d − m (each announced by
S→C 0x3E, `0x0053D130`), both books (type 18) → `0x0055C070(m − q_d)`,
dst item flag 0x8; else (when `0x00629930(src)`) dst stat 72 (`durability`, live `itemstatcost` row 72) lowered
to src's if src's is lower (0x3E; throwing weapons keep the worse
durability), dst := q_s + q_d
(0x3E), both books → `0x0055C070(q_s)`, cursor := none, S→C 0x42 for
src, src freed (`0x00557FD0`: the unit is taken off any player's
inventory list or cursor still holding it, then freed by `0x00555600`
(`sim/units.md` §3.2); nothing else, `world/cube.md` §8 "Exact" rule 1). Then dst command flag 0x100 (0x9C action
0xA), update list, refresh. (Different classes never pass `inventory.md` §4.5.)

#### 7.13 0x22 UnstackItems (`0x0054B380`)

Owned item check; then `0x0055E9A0` returns 0 without touching the
refusal flag, which is the message size (5): the handler returns **3**
for every owned item (original quirk; reproduce).

#### 7.14 0x23 ItemToBelt (`0x0054B3E0` → `0x0055E9B0`)

Cursor item check; `0x0055E9B0(item, slot u32 @5, find 0)`: targeting
reset; item missing or mode ≠ 4 → out 1; not beltable → 0; `inventory.md` §3.7 into the
given slot (fail → 0, page := 0xFF); link kind 2 (fail → out 1); cursor
:= none; item-skill link; unit flag 0x2 cleared; stat refresh if active;
mode 2; page 0xFF; command flag 0x400 (0x9C action 0xE); refresh; update
list. Result 0 or 3. (With find ≠ 0, used by other callers, `inventory.md` §3.5
chooses the slot.)

#### 7.15 0x24 ItemFromBelt (`0x0054B450` → `0x00562250`)

Belt item check; item-move gate (with no item) refuses → 0; a cursor
item → 2. `0x00562250`: item missing → out 1; mode ≠ 2 → out 1;
targeting reset; slot := item x; unlink; cursor := item; item-skill unlink;
unit flag 0x2 cleared; stat refresh; mode 4; command flag 0x800 (0x9C
action 0xF); refresh; update list; compaction `inventory.md` §3.8 of the slot.

#### 7.16 0x25 SwitchBeltItem (`0x0054B4E0` → `0x0055EB30`)

Cursor item check on the cursor (u32 @1), belt item check on the belt
item (u32 @5). Cursor item must be beltable (else 0) and in mode 4;
belt item mode ≠ 2 → out 1. Belt item B: unlink (mismatch → fatal),
cursor := B, mode 4, command flag 0x1000. Cursor item C: `inventory.md` §3.7 into B's
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
`0x0053C520`), targeting reset, removal `0x00561E70`, compaction `inventory.md` §3.8.
No hireling (`0x00574EC0` returns none): the target stays the player,
so the potion is used on the player (`0x00562494`–`0x005624A0`). Use
failing (`0x005BF240` = 0) → 0 with out 0, nothing removed.

#### 7.18 0x27 UseItemAction (`0x0054B280` → `0x00561ED0`)

Owned item check on target (u32 @1) and used item (u32 @5);
`0x00561ED0(game, player, T, U, &out)` (U = scroll or tome used on item
T; the effect itself is the item-use dispatcher `0x005BF240`, owned by
the item-use spec):

1. out := 0. U missing → out 1, 0. T missing or T = U → targeting reset
   (`inventory.md` §5.3), 0.
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
4. The four flags of `0x00562660(…, f1, f2, f3, f4)` (EDX = the unit
   whose inventory and refresh it uses, here the player; none skips
   every step that names it), as read in 1.14d:
   - f3 ≠ 0: the target's mode must be 0 or 1 (the mode check of rule 2);
     f3 = 0 skips it.
   - f2 ≠ 0 and EDX unit ≠ none: its inventory cursor is cleared
     (`0x0063C180(inventory, 0)`) after the link (the "cursor := none" of
     rule 3).
   - Runeword match (rule 3): `0x006600A0`, `0x00558530`, `0x00558580`,
     and `0x0055FE80(target)` only when f4 ≠ 0.
   - No match (or the runes record forbids it): f1 = 0 → return 1 right
     after filler mode 6 (no target flag 0x1, no 0x4000 clear, no
     refresh, no update list); f1 ≠ 0 → the tail of rule 3 as for a match.
   - The tail's refresh (`0x00621000(EDX unit, 1)`) and update list
     (`0x0063CC70`) run only when EDX unit ≠ none.
   The item copy passes (0, 1, 0, 0) with EDX = the copy (`0x0055A41A`;
   `world/vendors-2.md` §7.3 step 5).

#### 7.20 0x29 ScrollToBook (`0x0054B710` → `0x0055EF20`)

Ground-or-owned check (`inventory.md` §5) on the scroll (u32 @1); stored item check on
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
owner := player (`0x00621CE0`, only when `0x0044BE50(player)`, the
player's unit type, is 0); gold :=
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

`0x0054D130` (take; EBX game, ESI hireling): classic game, a hireling
without an inventory, or location ∉ 1..10 → 3 (`0x0054D141`–`0x0054D15E`);
the hireling's item at the location must exist in mode 1 and its unlink
must return it, else 2; unlink, slot cleared, hireling
stat refresh `0x0055C730(merc, 0, 0)`, command flag 0x10 on it, update
list of the hireling, hireling refresh; a **copy** (`world/vendors.md` §7.3, `0x0055A2A0`,
`items/generation.md` duplicate) becomes the player's cursor item
(`0x0063C180`, then `0x0055FB10`: mode 4, command flag 0x100000 = 0x9C
action 0x12, update list, owner refresh), the original gets item flag
0x20; hireling inventory pass (`0x0055DF00`), `0x0055F500`,
`0x0055F4F0(0)`. Result 0. A failed copy (none) is fatal: the cursor is
set to none, then `0x0055FB10` asserts (line 0x19A1), after the original
already left the hireling.

`0x0054D230` (give): classic → 3; C not identified → 0; C broken (0x100)
→ 0; C of type 76/81/80 (potions) → used on the hireling
(`0x005BF240`), consumed (`0x0055EEA0`), cursor := none, 1. Else allowed
when: C is of type 3 (`tors`) or 37 (`helm`); or by hireling class:
0x10F (271, Act 1) type 27 (`bow`); 0x152 (338, Act 2) type 33 or 34
(`spea`, `pole`); 0x167 (359, Act 3) type 2 (`shie`), or type 30
(`swor`) one-handed; 0x230 (560) type 28 (`axe`) one-handed or type 71
(`phlm`); 0x231 (561) type 30 or type 71. No hireling unit → only types
3 and 37. Allowed and `inventory.md` §4.2 (hireling, not equipping) passes →
`0x0054CED0` (equip on the hireling); sound on the player either way.

#### 7.24 0x63 ItemToBeltShift (`0x0054D520`)

1. Stored item check ≠ 0 → that result. A cursor item → `0x00549A60` (§7.4), 2.
2. Item missing or not beltable → 2; page ≠ 0 → 3. `inventory.md` §3.5 slot (none, or
   < 0) → 0; item-move gate refuses → 0.
3. No player inventory → fatal; item page ≠ 0 → 2; item mode ≠ 0 → 2;
   unlink (missing or mismatch → fatal assert).
4. Room-change notice with the old cell; stored page := page; page :=
   0xFF; queue 0x9D action 5 (`0x0053D010`, now); mode 4; unit flag 0x2
   cleared; `inventory.md` §3.7 into the slot: success → page := 0xFF, link kind 2, mode
   2, queue 0x9C action 0xE (`0x0053EE70`, now). Owner refresh (no
   update list). Result 0. A failed slot placement leaves the item in
   mode 4, not linked and not the cursor item (original quirk).

#### 7.25 0x60 SwapWeapons (`0x0054CE70` → `0x005616A0`)

Handler gates (size 1, expansion, used skill, dead; result 3 when the
body fails): `sim/intents-events.md` §9 rule 14. Body `0x005616A0(ECX
game, EDX player P, out)`, read in full from the disassembly
(`0x005616A0`–`0x00561AF9`, 2026-10-08). Game or P none → fatal assert
0xE1E / 0xE1F. No item-move gate, cursor, busy, trade, requirement or
durability test: the switch is never refused for the items' sake.

Move table (16-byte entries on the stack, `0x005616A7`–`0x005616FB`),
walked in this order:

| Entry | From | To | Link kind |
|---|---|---|---|
| 0 | 4 (right hand) | 11 | 4 |
| 1 | 5 (left hand) | 12 | 4 |
| 2 | 11 | 4 | 3 |
| 3 | 12 | 5 | 3 |

1. out := 0; targeting reset (`inventory.md` §5.3, `0x0055BF50`).
2. Mouse-skill sets (player data = P +0x14; slots as `formats/d2s.md`
   §2.4 rule 5): L := the saved swap-left pair (id +0x84, owner GUID
   +0x8C) and R := the saved swap-right pair (+0x80, +0x88)
   (`0x006231A0`, `0x00623120`; P none or not a player → (0, −1));
   both also reset the throw-restore slots (`inventory.md` §5.8): +0x74
   := 0, +0x7C := −1, +0x70 := 0, +0x78 := −1, and +0x90 := −1, +0x94
   := 0. Then the current right skill → +0x80 / +0x88 (`0x00622F80`)
   and the current left → +0x84 / +0x8C (`0x00622FF0`; id `0x00643CE0`,
   owner `0x00643AD0`; non-players skipped).
3. Look-up, entries 0–3: item := the item at From (`0x0063BDE0`);
   present → `0x0063E490` (a no-op here: it only fills an empty
   result) and the item must be in mode 1 (item +0x10), else out := 1,
   result 0. This test runs after step 2, so a failure leaves the
   skill slots exchanged (unreachable for game-placed body items, which
   are always mode 1).
4. Deactivation: hand-4 item present → `0x0055C730(item, P, 0, 1)`;
   then the same for the hand-5 item (ECX game, EDX item).
5. Removal, entries 0–3 with an item: entries 0–1 (hand items) → stat
   unlink (`0x0063D2B0`); unlink from the inventory (`0x0063AD90`;
   none or another item → out 1, result 0; this unlink also clears the
   weapon in use when it was that item, `inventory.md` §1.4 rule 1);
   From slot cleared (`0x0063BE30`); page := 0xFF (`0x00628280`);
   command flag 0x200000 (`0x00628170`); item flag 0x1; item flag
   0x4000 cleared when set (`0x006280A0` / `0x006280D0`); update list
   += item (`0x0063CC70`). All four items are off the body before any
   is placed.
6. Placement, entries 0–3 with an item: put at To (`0x0063BDB0`) and
   link with the entry's kind (`0x0063B210`); either failing → out 1,
   result 0 (unreachable: To was emptied in step 5; items already
   handled stay moved, later ones stay detached). Body location := To
   (`0x00627D70`). To 4 or 5 → stat link (`0x0063D1D0`, which also sets
   the weapon in use) and stat refresh `0x0055C2C0(item, P, 0)`, then
   item flag 0x40; To 11 or 12 → item flag 0x80 (no stat link: the
   swap set's stats never count). Then unit flag 0x2 cleared (+0xC4),
   mode 1 (`0x00624690`), item flag 0x1, command flag 0x200000.
   Entries 0–1 then run weapon bookkeeping (`inventory.md` §5.8,
   `0x0055C5C0`); both hands are empty at that point and the weapon in
   use is none, so it does nothing (`inventory.md` edge case 14).
7. Inventory pass `0x0055DBC0(game, P, send 0)` (`inventory.md` §5.7):
   this is where requirements act. A new hand item that is not usable
   (§5.6: §4.2 not equipping, the quiver rule) gets item flag 0x4000
   and its stat list unlinked, but stays in the hand. Broken items are
   moved and linked like any other; what counts for them is §5.7's.
   Step 7 of the pass is the owner refresh (`0x00621000(P, 1)`) that
   gets the update list sent; send 0 means no 0x48 from the pass.
8. Look up the step-2 pairs: Ls := P's skill (L id, L owner), Rs := P's
   skill (R) (`0x006439B0`), after the pass, so item-granted skills of
   the new hands exist.
9. P's client (`0x005531C0`; none → fatal 0xE94): client weapon switch
   +0x45C := (+0x45C = 0) (`0x00539220` / `0x00539230`); **S→C 0x97**
   (1 byte, `0x0053E110`, queued now) to P's client only.
10. Left: Ls exists and use_state(P, Ls) (`0x00647960`) ∉ {2, 7} →
    select L on the left (`0x005701B0`, EDX 1; that call sends its own
    S→C 0x23 to P's client at once, `0x0057026C`) and queue the event
    record 0x23 (`0x00571C60`: skill u16 +0x0C = L id, item u32 +0x08 =
    L owner, hand u8 +0x0E = 1). Else the record carries the current
    left skill (`0x00620190`: id, owner; none → 0, −1) and nothing is
    selected.
11. Right: the same with Rs, R, the right skill (`0x006201D0`), EDX 0,
    hand 0. Result 1 (handler result 0).

Messages, in order. In the handler: any 0x23 that selections inside
the pass send (`inventory.md` §5.7 step 6), S→C 0x97, then the direct
0x23 of step 10 and of step 11 (each only when that side selects the
saved skill). In the next per-unit update, to every client that
processes P (`sim/intents-events.md` §7.3 rule 1, `0x00580860`): one
**0x9D action 0x17** (`0x0053D110`, `item-actions.tsv` row 15, `to`
all) per moved item in update-list order (hand 4, hand 5, swap 11,
swap 12, those present; an item already listed this tick keeps its
place), then 0x47 and 0x48, then the two queued **0x23** (left, then
right; `0x00571CD0` at `0x005808FC`, layout `sim/server-messages.tsv`
0x23: type u8@1, GUID u32@2, hand u8@6, skill u16@7, item u32@9). The
stat changes of the link / unlink go by the stat-sync path, not by
this handler.

Example (synthetic). P holds sword A at 4, shield S at 5, axe X at 11,
nothing at 12; left Attack (0, −1), right Bash; saved swap pairs (0,
−1), (0, −1). After: X at 4, A at 11, S at 12, 5 empty; +0x84 / +0x8C
= Attack, +0x80 / +0x88 = Bash; both saved pairs resolve to Attack
(usable) → left and right select Attack (`0x005701B0` sends its 0x23
even when the skill is already selected). Sent in the handler: 0x97,
0x23 hand 1 skill 0, 0x23 hand 0 skill 0; next update: 0x9D
0x17 for A, S, X, then 0x47, 0x48, 0x23 hand 1 skill 0, 0x23 hand 0
skill 0. With nothing in 4, 5, 11 or 12: no item moves, no 0x9D, but
the skill sets still trade, 0x97 is sent and two 0x23 records are
queued; result 0.

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
     m − q_t (`0x0055C070`: `inventory.md` §5.5 skill quantity += n, S→C 0x22), handled
     (P stays on the ground). Else T := q_t + q_p (0x3E), item-skill add
     q_p, P leaves its room (`0x0061A270`, `0x00623830`, `0x0064C370`),
     unit flag 0x2 cleared, P freed (`0x00557FD0`), cursor := none,
     handled.
   - Auto-stack (`0x0055D0D0`), while P's stat 70 > 0: candidate D :=
     the next item, starting at the previous candidate, that passes `inventory.md` §4.5
     with P and has stat 70 < total max stack: from the body-location
     grid (`0x0063C2F0`) when P's itemtype `quiver` ≠ 0, then (none
     there, or `quiver` = 0) from page 0's grid (`0x0063C200`). No D →
     not handled (earlier partial merges stay). q_d + q ≤ m: D's stat
     72 lowered to P's when P has durability and P's is lower (0x3E), D
     := q_d + q (0x3E), P := 0, both books → item-skill add q, P leaves
     its room and is freed as above, cursor := none, handled. Else D :=
     m (0x3E only for D), P := q + q_d − m, both books → add m − q_d;
     next candidate. Every other exit of `0x0055D0D0` is a fatal assert
     (negative max stack, a negative sum, the loop ending after a pass
     without a full merge): none is reachable with valid stats.
5. Auto-equip `inventory.md` §4.7 (skip 0) gives L → `inventory.md` §4.3(L, item, 0) must be 1, else
   out 1; leave the room; `0x00562E00(item, 0)` equips (`inventory.md` §4.9); success →
   quest hook ITEMPICKEDUP (`0x00543D80`, `world/quests.md`). §4.9's
   result 0 cannot occur here (it repeats the `inventory.md` §4.7 test that just passed,
   on an unchanged inventory); its other failures are fatal. Were it 0:
   result 0, out 0, the item out of its room in mode 3 and sent nothing.
6. Else beltable, `inventory.md` §3.6 (always) and `inventory.md` §3.5 + §3.7 succeed → leave the room,
   link kind 2 (failure fatal), cursor := none, item-skill link, stat
   refresh if active, unit flag 0x2 cleared, mode 2, page 0xFF, command
   flag 0x2000 (0x9C action 0xE), unit flag 0x2000000 cleared, update
   list, refresh, inventory pass if active.
7. Else page-0 free position (`0x005600A0(game, 1, page 0)`: `inventory.md` §2.3 + §2.2,
   leave the room, link, cursor := none, item-skill link, stat refresh,
   unit flag 0x2 cleared, mode 0, command flag 0x80 (0x9C action 4),
   update list, refresh, unit flag 0x2000000 cleared, page := 0, quest
   hook ITEMPICKEDUP, inventory pass if active). No room → refused
   pickup with sound 0x17, 0. The link (`0x0063B210`, kind 1) failing
   in `0x005600A0` is a fatal assert.

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

#### 8.5 Corpse take-back (`0x00562F30`, ECX game, player P, corpse C)

The same routine is also written up in §12 (a parallel PC 2 session,
staging-6 merge; the two read the same 1.14d code and agree).

Called from the corpse pickup `0x0057FB70` (`combat/vitals.md` §4.7 rule
2: state 7 and the permission test, then the experience return) after
those steps. P's inventory I_P, C's inventory I_C; either missing →
result 0 and nothing else.

1. **Body pass**, repeated while the last pass found a body item on C
   and moved one: for location l = 1 … 12 in order, X = C's item at l
   (`0x0063BDE0`); none → next. X fails the requirements for P
   (`inventory.md` §4.2, equipping) → next (X stays on C). Else A = P's
   item at l, o = the other hand of l (`0x0055F240`: 4↔5, 6↔7,
   11↔12, else 0), B = P's item at o, or B = A when o = 0; then the
   slot test (rule 3) picks a location L:
   1. Slot test fails: X goes to a free position of P's inventory page 0
      (`0x005600A0`, §8.1 step 7); that failing → next location (X
      stays).
   2. Slot test passes: P already has an item at L → fatal assert
      0x1775. X put at L (`0x0063BDB0`) failing → next location. Link
      (`0x0063B210`, kind 3, or 4 when L is 11 or 12) failing → P's slot
      L cleared (`0x0063BE30`), next location. Linked: P's cursor :=
      none, X's body location := L, unit flag 0x2 cleared, mode 1,
      command flag 0x8, P's update list += X, P refreshed
      (`0x00621000(P, 1)`), unit flag 0x2000000 cleared, page := 0xFF;
      kind 3 only: stat link `0x0063D1D0`, stat refresh `0x0055C2C0(P,
      0)`, item-skill link `0x0055C270`, weapon bookkeeping
      `0x0055C5C0`; quest event ITEMPICKEDUP (`0x00543D80`).
   3. Either success (1 or 2): replenish timers (`0x00558530`,
      `0x00558580`, `items/generation.md` §9), item flag 0x1 set, X
      unlinked from I_C (`0x0063AD90`) and C's slot l cleared.
2. **Stored items**, in passes over I_C's item list (first item, each
   item's successor taken before it is handled). A pass starts with
   r := 1 and m := 0; the grid flag g starts at 0. For each item Y:
   1. Can-pick (§8.4) fails → r := 0; Y stays.
   2. Equip without the cursor (`inventory.md` §4.9, skip 0) succeeds
      → moved.
   3. Else Y is beltable (`inventory.md` §3 rule 3) and P's belt has a
      free box for it (`0x0063C790`) and the link (kind 2) succeeds:
      P's cursor := none; P a player → item-skill link
      (`0x0055C110`, add 1); unit flag 0x2 cleared; Y an active
      inventory item of P (`inventory.md` §5.6) → stat refresh; mode 2;
      unit flag 0x2000000 cleared; command flag 0x2000; page := 0xFF;
      P's update list += Y; P refreshed → moved. (A failed link leaves
      Y and r as they are.)
   4. Else g = 1: a free position of P's page 0 (`0x005600A0`) →
      moved; none → r := 0. g = 0: Y stays.
   Moved: m += 1 and item flag 0x1 set. After a pass with m = 0: g = 1
   → stop; else g := 1. Then the next pass from the first item; an
   empty list stops at once (r = 1).
3. **Slot test** (`0x0055F2D0`, P, X, &A, &B, &L): A and B both
   present → fail; both absent → pass. A present (B absent): B := A, A
   := none, L := the other hand o (X tries the free opposite slot).
   L not a hand (4, 5, 11, 12) → pass when A is absent. Hands (B is
   present here), with ammo type a(·) (`0x0062E6F0`) and quiver type
   q(·) (`0x0062E740`): a(X) > 0 → pass when B is-a a(X); else q(X) ≠ 0
   → pass when a(B) > 0 and X is-a a(B); else a(B) > 0 → pass when X
   is-a a(B); else q(B) > 0 → pass when X is-a q(B); else X or B
   two-handed (`0x006289C0`) and not one-or-two-handed for P
   (`0x0062A1E0`) → fail; exactly one of X, B is-a `weap` (45) → pass;
   neither → fail; both: P a monster (unit type 1), or class 4, or
   class 6 with both is-a `h2h` (67) → pass, else fail. (It differs
   from `inventory.md` §4.4 in the monster case and the ammo order.)
4. Last, the inventory pass `0x0055DBC0(0)` (`inventory.md` §5.7);
   result r of the last pass.
5. In `0x0057FB70`: r ≠ 0 → C's node is removed from I_P's corpse list
   (`0x0063D4E0(I_P, C's GUID, 1)`), C leaves its room (`0x0061A270`),
   a removal notice for C's GUID goes out (`0x0053DF80(GUID, 0)`),
   `0x00623830(C)`, C is freed (`0x00555600`), and event 0x5D
   (`object_corpse_loot`, `audio/triggers-2.md`) is queued on P
   (`0x00553380`). r = 0 → event 0x17 (`cantcarry`) on P; C keeps what
   it still holds.

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
`inventory.md` §2.4 steps 2–9 into page 0 (find free, send). Placement fails → page :=
0xFF and the item is dropped next to the player (§9.1 steps 2–3).
The unlink not returning the item is a fatal assert. The drop search
is `0x00555DA0` with size 1 and fallback 1 (`0x00563B9C`), as §9.1's
(`0x00563C83`); it returns none only when the start cell has no room:
then the item stays in mode 4 with page 0xFF, in no grid and not the
cursor, and the spill goes on with the next item (edge case 13).

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
The search (`0x0064E810` called directly, size 1, masks 0x3E01 /
0x801, fallback 0, field origin = the unit's position) takes as its
room the previous pile's result room, the unit's room for the first
pile. A failed creation (`0x00559CE0` returns none) skips that pile and
does **not** stop: its amount already counts as placed, the loop goes
on with the rest, and the caller (§7.22) subtracts only the piles in
the list, so that gold stays with the player. (A per-pile cap through
`0x00622E70` applies only when the new unit's type is 0, never for an
item: dead code.)

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

### 12. Corpse take-back (`0x0057FB70` → `0x00562F30`)

Reached from 0x16 PickItem type 0 (§7.1 rule 2) on a dead player's
corpse C by player U (`0x0057FB70`, ECX game, EDX U, stack C; read
from the 1.14d disassembly, 2026-10-07). The experience part is
`combat/vitals.md` §4.7 rule 2 (owner).

#### 12.1 Outer routine (`0x0057FB70`)

1. C lacks state 7 (`playerbody`, `0x00639DF0`) → nothing (no sound,
   no message). The take permission `0x0057FAF0` (`combat/vitals.md`
   §4.7 rule 2) fails → nothing.
2. Experience return (owner GUID = U's only, `combat/vitals.md`).
3. r := §12.2(game, U, C).
4. r ≠ 0: C is taken off U's corpse list (`0x0063D4E0`(U's inventory,
   C's GUID, 1): the node with that GUID and kind 1 of the list at
   inventory +0x34, §8.4 rule 6; none → nothing), C leaves its room
   (`0x0061A270`), S→C 0x8E `CorpseAssign` [1] 0, [2..5] U's GUID,
   [6..9] C's GUID to every player (`0x0053DF80` → `0x005538D0`),
   `0x00623830`(C), C is freed (`0x00555600`), then sound event 93
   (`object_corpse_loot`) on U (`0x00553380`).
5. r = 0: sound event 23 (`cantcarry`) on U; C stays with whatever
   items §12.2 could not move.

#### 12.2 Items (`0x00562F30`, ECX game, EDX U, stack C)

U, C, U's inventory or C's inventory none → result 0, nothing moved.
"Grid put" below is `0x005600A0`(EDI U, ESI X; game, leave-room 0,
page 0): §8.1 step 7's free-position placement on page 0 without the
room step (result 0 when §2.3 finds no position; its link failure is
fatal), i.e. item-skill link, stat refresh, cursor := none, unit flag
0x2 cleared, mode 0, command flag 0x80 (0x9C action 4), update list,
owner refresh, unit flag 0x2000000 cleared, page := 0, quest hook
ITEMPICKEDUP, inventory pass when `0x0062FF70` holds. Every put of an
item into U (`0x0063AFD0`) first unlinks it from C's inventory
(`inventory.md` §1.4).

**Phase 1, body locations** (repeated):

1. present := 0, moved := 0. For b = 1 … 12: X := C's item at body
   location b (`0x0063BDE0`); none → next b. present += 1.
2. `inventory.md` §4.2(X, U, equipping 0) fails → next b.
3. D := U's item at b. c := pair(b) (`0x0055F240`): 4 ↔ 5, 6 ↔ 7,
   11 ↔ 12, every other location 0. A := U's item at c when c ≠ 0,
   else A := D. L := b. fit := §12.3(U, X, D, A, L) (which can change
   D, A, L).
4. fit = 0: grid put; result 0 → next b. Result 1 → replenish timers
   (`0x00558530`, `0x00558580`, `items/generation.md` §9 step 6), step 7.
5. fit ≠ 0: U's item at L present → fatal assert (`ItemMode.cpp` line
   0x1775; unreachable, §12.3 only returns 1 for an empty L). Put X at
   L on grid 0 (`0x0063BDB0`) fails → next b. Link check
   `0x0063B210`(kind k) with k = 4 for L = 11 or 12, else 3; failure →
   U's location L emptied (`0x0063BE30`), next b. (`0x0063B210` returns
   1 for any item when the inventory's owner is not an item
   (`0x0063B24A`–`0x0063B24F`), so for U this failure is unreachable.)
6. Success: cursor := none (`0x0063C180`(U's inventory, none):
   **a cursor item U holds is detached**, `inventory.md` §1.4 rule 3;
   reproduce), body location := L, unit flag 0x2 cleared, mode 1,
   command flag 0x8, update list += X, owner refresh
   (`0x00621000`(U, 1)), unit flag 0x2000000 cleared, page := 0xFF;
   k = 3 only: stat link `0x0063D1D0`, stat refresh `0x0055C2C0`(X;
   U, 0), item-skill link `0x0055C270`(U, X), weapon bookkeeping
   `0x0055C5C0`(U). Then quest hook ITEMPICKEDUP (`0x00543D80`(game,
   U, X)) and the replenish timers.
7. Tail (steps 4 and 6): item flag 0x1 on X; `0x0063AD90`(C's
   inventory, X) (X is no longer C's: returns none, ignored); C's body
   location b cleared (`0x0063BE30`); present −= 1; moved := 1.
8. After b = 12: present ≠ 0 and moved ≠ 0 → step 1 again; else phase 2.

**Phase 2, every item left in C** (C's item list in list order, the
next node read before the item is handled; body items that phase 1
did not move are in it too): pass := 0.

1. placed := 0, r := 1. For each item X:
   1. Can pick §8.4(game, U, X) fails → r := 0; next.
   2. `inventory.md` §4.9(game, U, X, skip 0) = 1 → placed += 1, item
      flag 0x1; next.
   3. X beltable (§3 rule 3 of `inventory.md`) and the belt free-slot
      place `0x0063C790` succeeds (`inventory.md` §3 rules 5 and 7 on U:
      the slot of rule 5, then put at (slot, 0) of grid 1) → link check
      `0x0063B210`(kind 2) (always passes for U, phase 1 step 5; were
      it 0: next, X left in U's belt cells with no further step). Then: cursor := none
      (as phase 1 step 6), U a player (type 0) → item-skill link
      (`inventory.md` §5.5, add 1), unit flag 0x2 cleared, stat refresh
      `0x0055C2C0`(X; U, 0) when `0x0062FF70`(X, U) holds, mode 2, unit
      flag 0x2000000 cleared, command flag 0x2000 (0x9C action 0xE),
      page := 0xFF, update list += X, owner refresh; placed += 1, item
      flag 0x1; next. (No ITEMPICKEDUP, no replenish timers here.)
   4. Else (not beltable, or no slot): pass ≠ 0 → grid put; 1 →
      placed += 1, item flag 0x1; 0 → r := 0. pass = 0 → nothing.
2. After the walk: placed ≠ 0 → step 1 again (same pass); placed = 0
   and pass = 0 → pass := 1, step 1 again; placed = 0 and pass ≠ 0 →
   end. A walk that starts on an empty list ends at once with r = 1.
3. Inventory pass `0x0055DBC0`(game, U, 0) (`inventory.md` §5.7);
   result r.

So equipment goes back to its own slots first (repeated while slots
free up), then auto-equip, belt, and only in the second sweep the
inventory grid; the corpse is removed only when the last sweep met no
item that can-pick refused and no grid put that failed.

#### 12.3 Corpse slot fit (`0x0055F2D0`, ECX U, EDX X, &D, &A, &L)

1. U none, X none or not an item → 0.
2. D none and A none → 1 (L unchanged). D and A both present → 0.
3. D present (A none): D := none, A := the old D, L := pair(L) (the
   free partner slot). (D none, A present: unchanged.)
4. L ∉ {4, 5, 11, 12} → 1 when D is none (always here, rings move to
   the free finger), else 0.
5. Hands, with O := A (the item in the other hand, present here); "of
   type t" = itemtypes equivalence `0x00629BB0`; s(·) = the ammo type
   (`0x0062E6F0`, itemtypes `shoots`), q(·) = the quiver type
   (`0x0062E740`):
   1. s(X) > 0 → 1 if O is none or of type s(X), else 0.
   2. Else q(X) ≠ 0 → O none → 1; s(O) ≤ 0 → 0; else 1 if X is of
      type s(O), else 0.
   3. Else s(O) > 0 → 1 if X is of type s(O), else 0.
   4. Else q(O) > 0 → 1 if X is of type q(O), else 0.
   5. X two-handed (`0x006289C0`) and not usable one-or-two-handed by
      U (`0x0062A1E0`) → 0; the same for O.
   6. wX, wO := X, O of type 45 (`weap`). Neither → 0. Exactly one →
      1. Both: U a monster (type 1) → 1; U's class (unit +4) 4 → 1;
      class 6 and both of type 67 (`h2h`) → 1; else 0.

Differences from `inventory.md` §4.4: a quiver next to a bow passes
(rule 5.2), two non-weapons (two shields) fail, any monster passes
two weapons.
