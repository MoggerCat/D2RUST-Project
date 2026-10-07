# Spec: World — Vendors part 2 (the item copy `0x0055A2A0`)

- **Status:** draft: read from the 1.14d `Game.exe` (addresses inline);
  the two recorded copies (buy GUID 0x36, sell GUID 0x35) fit rule 3's
  GUID numbering. Moved out of `world/vendors.md` (size), text and
  numbers unchanged.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::vendors` (copy provider)
- **Related specs:** `world/vendors.md` (part 1; its Constants,
  Randomness, Edge cases, Provenance and Open questions (8, 9) also cover
  this part); `items/bitstream.md`, `items/generation.md` §1.4, §9,
  `items/inventory-moves.md` §6.1, §7.19, `sim/units.md` §2, §6.5,
  `sim/rng.md` §5.3. A § number without a file name (§7.1, §7.2, §8.2)
  is `world/vendors.md`'s, except §7.3 and §7.3.1.

## Summary

`0x0055A2A0` makes a second item equal to an existing one by writing
the source as a save-format stream and decoding it into a new item unit
(§7.3); §7.3.1 lists the fields the decoder rebuilds instead of reading,
so the copy can differ from its source there.

## Rules

### 7.3 Item copy (`0x0055A2A0`, ECX game, EDX source S, owner, fillers)

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

### 7.3.1 Fields the decoder rebuilds (Open question 8)

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
