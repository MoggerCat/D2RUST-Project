# Spec: Items — item use dispatcher and use table

- **Status:** draft: the dispatcher, its failure reset and the table
  were read from the 1.14d `Game.exe` disassembly and the table words
  from the file image (2026-10-08, REC-117); entry 2 (Town Portal) is
  `world/objects-2.md` §27, entry 7 (cube) `world/cube.md` §1. The
  other entries' bodies are not covered (open question 1). No recording.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::items::moves` (item use seam)
- **Related specs:** `items/inventory-moves.md` §7.11 (0x20), §7.17
  (0x26), §7.18 (0x27) (the callers and what a "used" result costs:
  quantity, charges, consumption); `skills/bodies-3.md` §4.4 (srvdo 113,
  the scroll / book skill); `items/inventory.md` §5.3 (targeting
  reset), §5.5 (books rows); `world/cube.md` §1, open question 7;
  `world/objects-2.md` §27.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 36–44 |
| Inputs | 45–64 |
| Outputs / state changes | 65–70 |
| Rules | 71–72 |
|   1. Dispatcher `0x005BF240(ECX game, EDX U; I, T, x, y)` (`ret 0x10`) | 73–95 |
|   2. Failure reset `0x005BE1C0` (U in EBX) | 96–104 |
|   3. Use table (`0x00741790`, read from the image) | 105–125 |
|   4. Town Portal (entry 2) | 126–137 |
| Constants & data dependencies | 138–144 |
| Randomness | 145–148 |
| Edge cases & original bugs | 149–161 |
| Test vectors | 162–171 |
| Provenance | 172–185 |
| Open questions | 186–192 |
<!-- /index -->

## Summary

Using an item on the server ends in one dispatcher, `0x005BF240`. It
picks an entry of a 31-entry table of {first use, second use} function
pairs by the item's `pSpell`, marks the item "targeting" (item flag
0x4) and runs the entry. A zero result undoes the targeting marks and
tells the client (S→C 0x7C). The caller decides from the result whether
the item was used (charges, consumption).

## Inputs

| Name | Type | Source |
|---|---|---|
| game, unit U | ECX, EDX | U: player or monster (type 0 or 1) |
| item I | stack 1 | unit type 4 |
| target T | stack 2 | what the use is aimed at: 0x20 passes I itself (`inventory-moves.md` §7.11 rule 3), 0x27 the target item (§7.18 rule 5), 0x26 see §7.17 |
| x, y | stacks 3, 4 | the request's point (0x20: the point the client sent) |
| table | `0x00741790`, 31 × 8 bytes; count `[0x0074178C]` = 31 | `Game.exe` image |
| `books.txt` `pSpell` (+0x04), `BookSkill` (+0x0C) | row of I's suffix slot 0 | `items/inventory.md` §5 |
| `misc.txt` `pSpell` (items record +0x94) | I's class | live 1.14d data |

The client's request for a backpack item: the grid right-click handler
`0x00487740` sends C→S 0x20 (`0x004786D0`) with I's GUID and U's path
position (types 0, 1, 3: `0x006488C0` / `0x00648900`; types 2, 4, 5:
static path +0x0C / +0x10), when no cursor item is held and items
`useable` holds (`0x00628C20`); for the `box ` code it also sets the
cube-open flag `[0x007BCC50]` := 1 (`0x004878DC`–`0x004878F4`). The
panel owner is `ui/panels.md`.

## Outputs / state changes

Result 0 or the entry's nonzero result; item flag 0x4 of I (item data
+0x18); on a zero result S→C 0x3F per reset item and S→C 0x7C (I); the
entry's own effects.

## Rules

### 1. Dispatcher `0x005BF240(ECX game, EDX U; I, T, x, y)` (`ret 0x10`)

1. U null, or U's type not 0 / 1 → 0. I null or not an item → 0.
2. Index n and extra e (e := −1):
   1. I of item type 18 (`book`) or 22 (`scro`) by `itemtypes`
      equivalence (`0x00629BB0`) and its books row (`0x006374B0` of
      `0x00627F80(I, 0)`) exists: e := `BookSkill` (+0x0C), n := `pSpell`
      (+0x04). n > 0 → step 3.
   2. Else n := the items record's `pSpell` (+0x94, `0x006335F0(I's
      class)`) when the record exists. n < 1 → 0.
3. n ≥ 31 (`[0x0074178C]`) → 0.
4. **First use**: word 0 of entry n ≠ 0 and I's flag 0x4 clear
   (`0x006280A0(I, 4)`): r := word 0 (ECX game, EDX U; I, **0**, x, y,
   e). r ≠ 0 → return 0. r = 0 → failure reset (§2), S→C 0x7C (I's
   type, I's GUID; 6 bytes, `0x0053B3D0`) to U's client, return 0.
5. **Second use** (word 0 is 0, or I's flag 0x4 set): word 1 = 0 → 0.
   Else I's flag 0x4 := set (`0x006280D0(I, 4, 1)`); r := word 1 (ECX
   game, EDX U; I, T, x, y, e). r = 0 → failure reset (§2), S→C 0x7C
   (I) to U's client. Return r.

A first use never reports "used": an entry with a first word (only
entry 1) arms the item and is used by a second request.

### 2. Failure reset `0x005BE1C0` (U in EBX)

U's inventory (+0x60) none → nothing. Else every item of U's inventory
list in list order (`0x0063B2C0`, next `0x0063DFA0`) with flag 0x4:
flag cleared (`0x006280D0(item, 4, 0)`), S→C 0x3F to U's client
(`0x0053D220(item, 0xFF, 1, 0, 0xFFFF)`: `3F FF`, the item GUID, `FF
FF`; `world/cube.md` §1 bytes). Unlike the targeting reset
(`items/inventory.md` §5.3) there is no unit-type test.

### 3. Use table (`0x00741790`, read from the image)

| n | word 0 | word 1 | live `pSpell` rows (`misc.txt`, Patch_D2) | owner |
|---|---|---|---|---|
| 0 | 0 | 0 | — | never reached (n ≥ 1) |
| 1 | `0x005BE130` | `0x005BE230` | `isc`, `ibk` | open question 1 (identify, REC-113) |
| 2 | 0 | `0x005BE290` | `tsc`, `tbk` | `world/objects-2.md` §27 |
| 3 | 0 | `0x005BE3F0` | `hp1`–`hp5`, `mp1`–`mp5` | open question 1 |
| 4 | 0 | `0x005BE7B0` | none | open question 1 |
| 5 | 0 | `0x005BEAC0` | `rvs`, `rvl` | open question 1 |
| 6 | 0 | `0x005BEF90` | `yps`, `wms` | open question 1 |
| 7 | 0 | `0x005BF0C0` | `box` | `world/cube.md` §1 |
| 8 | 0 | `0x005BF060` | `elx` | open question 1 |
| 9 | 0 | `0x005BEDA0` | `vps`, `hrb` | open question 1 |
| 10 | 0 | `0x005BF170` | none | open question 1 |
| 11 | 0 | `0x005BF1F0` | none | open question 1 |
| 12–30 | 0 | 0 | — | nothing (result 0, no reset) |

`books.txt` rows: "of Town Portal" `pSpell` 2, "of Identify" 1, "of
Ressurect" (unused, no codes) 0.

### 4. Town Portal (entry 2)

`tsc` / `tbk` reach word 1 directly: flag 0x4 is set on the scroll or
tome, then the cast `0x005BE290` (`world/objects-2.md` §27.1) runs with
I; T, x, y and e are not read. Its result is the use result: 1 when
the pair was made, 0 otherwise (refused in a town or in level 136, or
the creation failed). What a 1 costs is the caller's (0x20:
`inventory-moves.md` §7.11 rule 3: a scroll loses its skill count
(S→C 0x22), the targeting reset, consumption; a tome loses one
`quantity` (stat 70, S→C 0x3E), S→C 0x7C, the skill count; 0x26:
§7.17). A 0 costs nothing: the scroll stays, the tome keeps its charge.

## Constants & data dependencies

- Table `0x00741790` (31 entries, 8 bytes), count `0x0074178C` = 31.
- `books.txt` `pSpell`, `BookSkill`; `misc.txt` `pSpell`; `itemtypes`
  equivalence for 18 / 22.
- Item flag 0x4 (item data +0x18): "targeting".

## Randomness

The dispatcher draws nothing. Entries: their owners.

## Edge cases & original bugs

1. **A tome stays flagged.** After a used tome the 0x20 path does not
   run the targeting reset (`inventory-moves.md` §7.11 rule 3, type
   18), so flag 0x4 stays on the tome until the next targeting reset,
   which then sends its S→C 0x3F. A scroll's flag goes with the scroll.
2. **Two 0x7C on a failed Town Portal outside a town**: the cast sends
   one itself (`world/objects-2.md` §27.1 rule 9), the dispatcher a
   second (§1 rule 5). The town refusal returns before the cast's 0x7C:
   one 0x7C.
3. A first word's nonzero result returns 0 to the caller (§1 rule 4):
   the item is not counted as used on the arming request.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| U player, I `tsc` (books row 0: `pSpell` 2, `BookSkill` Book of Townportal) | n = 2, e = book skill id; flag 0x4 set; `0x005BE290` called | §1, §3; live `books.txt` |
| I `hp1` (not book / scroll) | n = 3 from `misc.txt` | §1 r2.2 |
| I with `pSpell` 0 (e.g. a weapon) | 0, no message | §1 r2.2 |
| I `tsc`, the cast returns 0 | flag 0x4 cleared on every flagged inventory item with `3F FF <GUID> FF FF` each; `7C 04 <GUID>`; result 0 | §1 r5, §2 |
| U a hireling (type 7) | 0 | §1 r1 |

## Provenance

- 1.14d `Game.exe` disassembly (`re/exports/all.asm`, 2026-10-08):
  `0x005BF240`–`0x005BF3C4` (dispatcher), `0x005BE1C0` (failure
  reset), `0x006280A0` / `0x006280D0` (item flag test / set: item data
  +0x18), `0x0053B3D0` (6-byte sender, DL = 0x7C), `0x0053D220`.
- Table words read from the file image with `re/scripts/rd.py 00741790
  62` and `0074178C 1`.
- Client request: `0x00487740` (`0x004877F2`–`0x004878F4`), sender
  `0x004786D0`.
- Live `books.txt` and `misc.txt` (`game/extracted/patch_d2`), column
  `pSpell`.
- D2MOO 1.10f `SKILLITEM` names were not used; no rule here rests on it.

## Open questions

1. The bodies of entries 1 and 3–11 (`0x005BE130`, `0x005BE230`,
   `0x005BE3F0`, `0x005BE7B0`, `0x005BEAC0`, `0x005BEF90`, `0x005BF060`,
   `0x005BEDA0`, `0x005BF170`, `0x005BF1F0`): settled by reading each
   (static); potions are partly in `data/calc-expressions.md`.
