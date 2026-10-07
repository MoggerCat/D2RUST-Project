# Spec: Simulation — Player pet lists

- **Status:** draft: every function below read from the 1.14d `Game.exe`
  disassembly (`py tools/ghidra/disasm.py fn|at`); D2MOO 1.10f
  `PlayerPets.cpp` used for names only. No recording yet (Open
  questions).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::player::pets`
- **Related specs:** `skills/bodies.md` (§3.3 Unsummon, §6.2 summon
  spawn), `skills/bodies-2.md` (§8.7 Revive); `sim/server-messages.tsv`
  (0x7A PetAction); `sim/unit-order.md` §2 (player iteration);
  `combat/damage.md` §7.2 (kill); `data/fields.tsv` (`pettype`).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 38–46 |
| Inputs | 47–56 |
| Outputs / state changes | 57–62 |
| Rules | 63–64 |
|   1. Data | 65–78 |
|   2. Add `0x00575D90(game, player, pet, t, max)` | 79–94 |
|   3. Group eviction `0x00575720(t)` (player in EDI) | 95–106 |
|   4. Set the maximum `0x00575850(game, player, t, max)` | 107–117 |
|   5. Append `0x00575C70(pet, extra)` (E in ESI, game in EAX) | 118–127 |
|   6. Remove `0x005750E0(game, player, GUID, kill)` and unlink `0x00574850` | 128–156 |
|   7. Dismiss `0x00574450` (GUID in EBX, game in EDI) | 157–165 |
|   8. Broadcast and message 0x7A | 166–179 |
|   9. Lookup `0x00574A20(player, GUID)` | 180–186 |
| Constants & data dependencies | 187–196 |
| Randomness | 197–200 |
| Edge cases & original bugs | 201–212 |
| Test vectors | 213–221 |
| Provenance | 222–261 |
| Open questions | 262–272 |
<!-- /index -->

## Summary

Each player keeps one list of its pets per `pettype.txt` row (summons,
golems, revives, hirelings, traps, …). Adding a pet evicts pets of other
types in the same `group`, trims the type's list to its maximum (oldest
first) and tells every client (message 0x7A). Removing a pet unlinks it,
optionally dismisses the monster, and tells every client again. The
unsummon test and monster bookkeeping look a pet's type up by GUID.

## Inputs

| Name | Type | Source |
|---|---|---|
| game, player | game, player unit | callers |
| pet | monster unit, or its GUID | callers |
| pet type t | i32 | `pettype` (+0xBE) of the summoning skill, or the hireling code |
| max | i32 | `eval(petmax)` of the skill (at least 1 from `bodies.md` §6.2) |
| `pettype.txt` | 0xE0-byte rows, count at data tables +0xBF0 | `group` +0x08 (u16), `basemax` +0x0A |

## Outputs / state changes

The player's pet lists; pet monster flags (unit +0xC4 bit 0x80000000
cleared, 0x4000000 set when dismissed); monster death or a death mode
request; S→C 0x7A to every player's client.

## Rules

### 1. Data

1. The player data (`0x006221A0(player)`; none → fatal assertion in every
   function that needs it) holds at +0x44 a pointer P to the pet lists,
   or null (no lists: every operation below does nothing).
2. P +0x00 points to an array of one 12-byte entry per pet type,
   index = `pettype` row: +0x00 head node, +0x04 count, +0x08 max. Entry
   t exists for 0 ≤ t < pettype count (`0x00574540`).
3. Node, 0x18 bytes, allocated from the game pool (`PlayerPets.cpp`):
   +0x00 flags (bit 0: skipped by "first pet" with `any` = 0), +0x04
   pet GUID (−1 for none), +0x08, +0x0C, +0x10 three caller values (0
   from §2), +0x14 next. New nodes go at the **tail**; the head is the
   oldest pet.

### 2. Add `0x00575D90(game, player, pet, t, max)`

ECX game, EDX player; stack pet, t, max; `ret 0xC`. Called by the summon
spawn (`skills/bodies.md` §6.2 step 4) and Revive (`skills/bodies-2.md`
§8.7 step 9).

1. Player none or not type 0 → nothing. t not in 1…pettype count − 1 →
   nothing.
2. Group eviction (§3) for t.
3. Set the maximum (§4) of t to max.
4. Player data none or P null → nothing. E = entry t (§1 rule 2); none →
   nothing.
5. Append (§5) the pet to E; failure → nothing.
6. Broadcast (§8) "add": {pet GUID (−1 none), the player's GUID, pet class
   (u16; 0xFFFF none), t}.

### 3. Group eviction `0x00575720(t)` (player in EDI)

g = `group` of row t (i16). g ≤ 0 → nothing. For every other pet type u
(u ≠ t, in row order) with `group` = g: K = the first pet of u
(`0x00574EC0(game, player, u, any 1)`: the unit of the head node,
`0x00552F60`). While K exists: Remove (§6) K's GUID with kill 1; then K
:= the unit of u's new head (stop when the player data, P, the head or
its unit is missing).

1.14d data: `spiritwolf`, `fenris` and `grizzly` share group 1, so
summoning one of them removes the others.

### 4. Set the maximum `0x00575850(game, player, t, max)`

1. Player data none → fatal assertion.
2. t = 1 (`single`) and max ≠ 1 → nothing. P null or t out of range →
   nothing.
3. E.max := max. While max < E.count, E.count > 0 and t ≠ 7
   (`hireable`): Remove (§6) the head's GUID (`0x005747B0`; −1 without a
   head) with kill 1.

The hireling list is never trimmed here.

### 5. Append `0x00575C70(pet, extra)` (E in ESI, game in EAX)

1. E.max = 0: resync `0x00575900(game, player)` (`skills/bodies.md` Open
   question 5); still 0 → dismiss (§7) the pet's GUID (−1 without one);
   return 0.
2. E.count = E.max: head none → fatal assertion; unlink (§6 step 4) the
   head's GUID with kill 1; still full → fatal assertion.
3. A new node {flags 0, pet GUID, extra (3 values)} is linked at the
   tail; E.count += 1; E.count > E.max → fatal assertion. Return 1.

### 6. Remove `0x005750E0(game, player, GUID, kill)` and unlink `0x00574850`

Remove (ECX game, EDX player; stack GUID, kill):

1. Player data none → fatal assertion. P null → nothing.
2. t = Lookup (§9) of GUID. t = 0 and kill ≠ 0 → dismiss (§7) GUID.
3. t in range → unlink (step 4) GUID from entry t (for t = 0 the list is
   normally empty: nothing happens).
4. Broadcast (§8) "remove": {GUID, 0, 0, 0}.

Unlink `0x00574850` (ECX GUID, EAX game; stack E, kill): find the first
node with this GUID (none → return); unlink it; broadcast "remove"; the
unit of GUID exists: kill = 0 → its flags (+0xC4) &= ~0x80000000; else
dismiss (§7); free the node; E.count −= 1 (< 0 → fatal assertion).

Broadcast count of one Remove (each broadcast = one 0x7A remove per
player, §8):

| Case | Broadcasts | From |
|---|---|---|
| GUID in list t ≥ 1, kill 1, its unit exists | 3 | unlink (`0x005748B3`), dismiss inside unlink (`0x005748D5` → `0x0057452B`), Remove (`0x0057518F`) |
| GUID in list t ≥ 1, kill 0, or its unit gone | 2 | unlink, Remove |
| t = 0 (in no list), kill 1, unit exists | 2 | dismiss (`0x00575137`), Remove |
| t = 0, kill 0, or unit gone | 1 | Remove |

The dismiss kill `0x0057CCB0(game, unit, 0, 0)` passes 0 as its second
stack argument, so the kill path's own pet removal (`0x0057CD0D` →
`0x005751A0`, which calls Remove again) does not run inside a remove.

### 7. Dismiss `0x00574450` (GUID in EBX, game in EDI)

Unit = the monster of GUID (`0x00552F60(game, 1, GUID)`); none → nothing.
Unit +0xC8 bit 8 set → fatal assertion. Flags (+0xC4) |= 0x4000000 (no
experience). Its monstats has `killable` (+0x0D bit 7, i.e. flags bit
15) → kill `0x0057CCB0(game, unit, 0, 0)` (`combat/damage.md` §7.2); else
mode request 0 (death) with target the unit's owner (`0x00552FD0`)
(`0x005A7E60`, `0x005A7C20(game, &req, 1)`). Broadcast "remove" {GUID}.

### 8. Broadcast and message 0x7A

`0x005538D0(game, fn, record)` (`sim/unit-order.md` §2 rule 5): fn runs
for every player of the game without state 7, in player-list order.

- Add `0x00574930`: record +0x10 and +0x14 both 0 → S→C 0x7A to that
  player's client (`0x005531C0`, `0x0053CB30` with action 1); else S→C
  0x81 (`0x0053CB80`; hirelings, owner of 0x81: hireling spec).
- Remove `0x00574410`: S→C 0x7A with action 0.

0x7A layout (13 bytes): +0 0x7A, +1 action (1 add, 0 remove), +2 pet type
(u8), +3 pet class (u16), +5 owner GUID (u32), +9 pet GUID (u32). A
remove sends type 0, class 0, owner 0.

### 9. Lookup `0x00574A20(player, GUID)`

ECX player, EDX GUID. Player data none → fatal assertion. P null → 0.
For t = 1…pettype count − 1 in order, each list from its head: a node
with this GUID → return t. Not found → 0. Used by Unsummon
(`skills/bodies.md` §3.3) and Remove (§6).

## Constants & data dependencies

| Item | Value |
|---|---|
| list entry | 12 bytes: head, count, max |
| node | 0x18 bytes; GUID +0x04, next +0x14 |
| pet types with special rules | 1 `single` (max only 1), 7 `hireable` (never trimmed by §4) |
| `pettype` columns read | `group` +0x08 (i16) |
| messages | S→C 0x7A PetAction (13 bytes), 0x81 AssignMerc |

## Randomness

None.

## Edge cases & original bugs

1. A remove broadcasts 0x7A up to three times: three for a listed pet
   removed with kill 1 whose unit exists (§6 table).
2. Remove of a GUID that is in no list still dismisses the monster when
   kill ≠ 0 and still broadcasts (§6 step 2).
3. Group eviction stops at the first list head whose unit no longer
   exists, leaving later nodes of that type (§3).
4. Setting a maximum below the current count removes the oldest pets one
   by one (kill 1), each with three broadcasts when the unit exists
   (§4, §6 table).

## Test vectors

| Case | Expected |
|---|---|
| Summon Grizzly with 3 spirit wolves alive (1.14d `group` 1) | the 3 wolves removed (kill 1) before the grizzly is appended |
| Raise a 4th skeleton with max 3 (synthetic) | the oldest skeleton removed first; list count 3 |
| Lookup of a hireling's GUID | 7 |
| 0x7A add, pet GUID 5, owner 1, class 363, type 4 (synthetic) | bytes 7A 01 04 6B 01 01 00 00 00 05 00 00 00 |

## Provenance

- 1.14d disassembly of `0x00575D90`, `0x00575720`, `0x00575850`,
  `0x00575C70`, `0x005750E0`, `0x00574850`, `0x00574450`, `0x00574A20`,
  `0x00574EC0`, `0x005747B0`, `0x00574540`, `0x00574930`, `0x00574410`,
  `0x0053CB30`; `pettype.txt` values from the 1.14d `patch_d2` tables.
- §8 layout re-checked 2026-10-07 (spec-client-msgs-3) against the
  conflicting `world/hirelings.md` §13 r2 and `sim/server-messages.tsv`
  (pet @5, owner @9; both corrected): record built by `0x00575D90` at
  `0x00575E42`–`0x00575E73` (+0x00 pet GUID, +0x04 owner GUID, +0x08
  class, +0x0C type); `0x00574930` / `0x00574410` push +0x08, +0x04,
  +0x00, +0x0C; `0x0053CB30` stores the third argument (+0x04) at @5 and
  the second (+0x00) at @9. The client handler `0x0045E860` passes u32@9
  as the pet GUID (record +0x08, the field the hireling lookup
  `0x00478F20` returns) and u32@5 as the owner (`client/model.md` §14).
  No 0x7A occurs in the two recordings (`traces/raw/20261006-015956`,
  `-022633`).
- Re-checked again 2026-10-07 (spec-senders-area-2) because
  `skills/bodies-3.md` §2 (branch `spec-skill-area`) read pet @5 / owner
  @9: **owner @5, pet @9 stands**. `0x0053CB30` stores ECX = client, DL
  @1, stack arg 1 ([EBP+8]) @2, arg 2 ([EBP+0xC]) @9, arg 3 ([EBP+0x10])
  @5, arg 4 ([EBP+0x14], u16) @3 (`0x0053CB44`–`0x0053CB6B`). `0x00575D90`
  (ECX game, EDX = the player: its type is checked to be 0 at
  `0x00575DA7`; stack arg 1 = the pet unit, EBX) builds the record +0x00 =
  pet +0x0C (GUID, `0x00575E4F`), +0x04 = player +0x0C (`0x00575E61`),
  +0x08 = pet +0x04 (class, `0x00575E5E`), +0x0C = pet type
  (`0x00575E42`). `0x00574930` / `0x00574410` push +0x08, +0x04, +0x00,
  +0x0C, so arg 2 = pet, arg 3 = owner. The third caller, `0x00574F80`
  (call `0x0053CA6E`, every pet of a player to one client, action 1),
  pushes the player's GUID as arg 3 and the node GUID as arg 2
  (`0x0057501E`–`0x00575026`): the same order. `sim/server-messages.tsv`
  0x7A (`owner:u32@5 pet:u32@9`) is right; bodies-3.md §2 is the one
  to fix.
- Same session, broadcast count: `0x005750E0`, `0x00574850`,
  `0x00574450`, `0x005751A0`, `0x0057CCB0` (`0x0057CD0D`–`0x0057CD21`);
  the remove-with-kill count is 3, not 2 (§6 table): bodies-3.md §2 was
  right on this point.
- D2MOO 1.10f `PlayerPets.cpp` names (`PLAYERPET_*`) used as hints only;
  every step above is from 1.14d.

## Open questions

1. Who allocates and frees the pet lists (player data +0x44) and sets
   each entry's initial max from `basemax`; owner: player init spec.
2. `0x00575900` (resync after a zero maximum): messages and effect
   (`skills/bodies.md` Open question 5).
3. Recording: summon pets past their maximum and across a group; compare
   the 0x7A messages and which units die.
4. The three extra node values (+0x08…+0x10): every caller seen passes 0;
   find a caller that does not (hirelings).
