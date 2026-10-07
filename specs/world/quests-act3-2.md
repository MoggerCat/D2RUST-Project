# Spec: World — Quests, Act III part 2 (implementation clarifications)

- **Status:** draft: every rule below is read from the 1.14d `Game.exe`
  (disassembly; addresses inline). No Act III recording exists yet;
  nothing is verified against a trace.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests::act3`
- **Related specs:** `world/quests-act3.md` (part 1: every Act III rule
  §1–§10; its notation is used here unchanged); `world/quests-act1-rest.md`
  §5 (character progression), §8 (the drop helper's return);
  `world/objects.md` §3 (init record); `monsters/init.md` §2, §16–§17
  (type flags, random boss); `items/inventory.md` §1.1 (weapon GUID);
  `data/fields.tsv` (monstats flag bits); `formats/d2s.md` (progression).

## Summary

Answers to the questions the Act III implementation raised
(`docs/handoff/impl-quests-act3.md`, HANDOFF §7 QC-1 … QC-7). Each item
pins down a function the part-1 rules call but did not describe fully.

## Inputs

As `world/quests-act3.md`.

## Outputs / state changes

As `world/quests-act3.md`; no new outputs.

## Rules

### 11. Clarifications (implementation questions QC-1 … QC-7, 2026-10-07)

Each item settles a gap the Act III implementation raised
(`docs/handoff/impl-quests-act3.md` open questions 1–5, 7). Read from the
1.14d disassembly at the addresses given.

#### 11.1 Bit 17.3 is never set (QC-1)

No 1.14d code sets 17.3. Checked over a linear disassembly of every code
section: the player-record bit set `0x0065C360(record, slot, bit)` has
332 call sites; none passes slot 0x11 with bit 3, the variable-bit sites
use slots 7, 0x0A, 0x12, 0x14, 0x15, 0x1C, and the variable-slot sites
use bits 0, 1, 5, 9, 0x0A, 0x0C, 0x0D, 0x0F only. The only other writer
of the player record is the save load `0x0065C4D0` (copies the 0x60
saved bytes; then, per slot, clears bits 13 and 14 and sets bit 15 when
bit 0 is set). So the §3.7 event-13 branch "17.3 → state 3, status 1" is
reached only by a save whose slot-17 word already carries bit 3; a
character saved by 1.14d never does. D2MOO 1.10f reads the same bit in
the same place and never sets it either.

#### 11.2 Character progression on Mephisto's credit (QC-2, open question 5)

The credit `0x005BC140` sets 22.13, 22.0, 22.11, then calls `0x00538680`
with the client of the player (`0x005531C0`), step 3 (EDX) and the game
difficulty (game +0x6D, stack) at `0x005BC182`. The rule is
`quests-act1-rest.md` §5: n = 4·difficulty + 3 (classic character) or
5·difficulty + 3 (expansion character, client +0x0A bit 5); the
progression bits 8–12 of client +0x0A become n unless they are already
greater. It is the save header's progression field (`formats/d2s.md`).

#### 11.3 The `&level` argument of `0x00559A30` (QC-3a, open question 2)

`0x00559A30` is fastcall (ECX game, EDX source unit) with five stack
arguments (quality, `&level`, `&request out`, then two passed on to the
item-creation path; `ret 0x14`). `&level` is an output: the function
writes it (`0x00559AF8`) before any read, so the caller's incoming value
(an operate-context slot or uninitialised stack in the Act III callers)
is never used. Value written: source unit type 1 (monster) → its total
stat 12 (`0x00625480`); type 0 (player) → its base stat 12
(`0x006253B0`); any other type (objects) → the area level of the unit's
room level (`0x0061DCA0`, `MonLvlEx` in an expansion game (game +0x70),
else `MonLvl`, by difficulty game +0x6D); a result ≤ 1, or no source
unit, gives 1. That level is then the item level of the drop
(read back at `0x00559B69`, `0x00559BAD`, `0x00559C5E`). So the Gidbinn,
the figurine, the council flails / cubes and the soulstones take the
victim monster's level; the tome and Khalim's parts take the area level
of the object's level. The return value is the created item (null when
none), as `quests-act1-rest.md` §8 rule 1.

#### 11.4 The monster tests (QC-3b, QC-3c, open questions 3, 4)

- §6.2 `0x00544E80`: the byte at `0x006CE280` is 0x40 (the bit-mask
  table entry 6), tested against monstats byte +0x0D, i.e. bit 14 of the
  monstats flag word at +0x0C: the `flying` column (`data/fields.tsv`
  monstats seq 46). Flying monsters never carry the Golden Bird. The
  test runs only when the monstats row exists (class 0 … count − 1) and
  chain 18 is present with not-intro = 1. Order (`0x00544E80`): the unit's room's
  level must be in Act III (act 2), the unit present, class ≥ 0 and <
  the monstats row count (`0x00544ED3`), chain 18 present with not-intro
  = 1, class ≠ 407 (fetish11), not flying → `0x005BAC70`. A class with no
  row fails the whole test: nothing is chosen or linked.
- §5.6 kill test: `0x005A0180(victim, 0x0E)` = monster data type flags
  (+0x16) & 0x0E ≠ 0, i.e. superunique (2), champion (4) or unique (8)
  (`monsters/init.md` §2 type flags); else `0x0063E9F0(0, victim)` = the
  victim's monstats `boss` column (flag word +0x0C bit 6). Either one
  passes.
- §5.6 boss spawn `0x005B9930`: `0x005A43E0(game, room, cl 0, class 407,
  champion allowed 1, x 0, y 0, warp check 1)`, the random boss of
  `monsters/init.md` §16.1 (searched point, room box). So the Gidbinn
  boss is a unique fetish11, or a champion when the §17 champion roll
  succeeds; both carry type flags 1 | 8 and pass the kill test.

#### 11.5 The Compelling Orb's weapon (QC-3d, open question 6)

`0x0063BEF0(inventory)` returns the weapon in use: inventory +0x1C (the
weapon GUID, `items/inventory.md` §1.1) must be ≠ −1; then the item at
body location 5 (left hand) is returned when it is of item type 45
`weap` (`0x00629BB0`) and its GUID equals +0x1C; else the same test on
body location 4 (right hand); else none. Locations 11 / 12 (swap) are
not consulted. The orb operate then compares that item's code with
`qf2 ` (`0x00628590`, `0x005BB9B3`).

#### 11.6 Natalya's and Hratli's map AI (QC-4)

Confirmed: `0x005BD040` (stores chain 20 +0x24) and `0x005B7230`
(stores chain 14 +0x18) have no caller: no rel32 call or jump, no 4-byte
pointer anywhere in the image (`disasm.py xref`) and no immediate in the
linear disassembly. Those fields are written only by the init functions
(zeroed) and by these two functions, so they stay 0, the "map AI
stored" test of init 52 (§8.7) and init 50 (§9.1) always fails, and
chain 20 +0x30 and chain 14 +0x10 stay 0. `0x0058F000` / `0x00666120`
are never reached from Act III code. `0x005B7120` (chain 14: returns
the end position +0x04 / +0x08 when +0x02 = 1 and +0x03 set) has no
caller either.

#### 11.7 Positions and rooms (QC-5)

Object init functions receive the init record {game +0x00, object
+0x04, room +0x08, control +0x0C, objects row +0x10, x +0x14, y +0x18}
(`world/objects.md` §3 step 6). "Its position" in §5.6, §5.7, §9.1 and
§9.2 is that record's (x, y), the object's creation point.

1. Decoy init 25: +0x08 / +0x0C := (x, y); the boss spawn there uses the
   init record's room. The timer `0x005B9A30` looks the point up in the
   Act III record (game +0xC4 = game +0xBC + 4·2) with `0x00619DA0`: the
   first room of the act's active room list (head act +0x10, next room
   +0x7C) whose rectangle (+0x4C / +0x50 origin, +0x54 / +0x58 size)
   contains it. It scans that room and its adjacent rooms (`0x00619790`
   order) for a unit of type 0 in each room's unit list (+0x74, next
   unit +0xE8) and spawns the boss in the **covering room**, not the
   player's room.
2. Altar init 39: +0x10 / +0x14 := (x, y), +0x28 := object GUID. The
   activation `0x005B9CD0` clears +0x06 first; when the altar unit
   (+0x28, unit type 2) is not found nothing else happens (+0x2C keeps
   its value).
3. Hratli dummies (inits 49, 50): Hratli is spawned at the dummy's
   (room, x, y) with `0x005B2F20(game, room, x, y, 253, mode 1, −1, 0)`.
   Init 50 stores +0x04 / +0x08 := (x, y) before the game-flag test.
4. Wanderer init 43: target := (x + 7, y) (stored only while +0x01);
   room := `0x00463740(init room, x + 7, y)`: the init room when its
   rectangle contains the point, else the first adjacent room
   (`0x00619790` order) that does, else none. With no room the spawn
   `0x005B2F20` fails at `0x005B2B50` before any draw (no coordinate
   list and no room → 0); +0x01 stays 1, so a later init 43 of the
   object tries again; +0x00 := 1 in every case.
5. Walk target `0x005BD0D0`: the blocked test `0x006229F0(wanderer, x,
   y, 0x3C01)` uses the wanderer's own room and size; a unit without a
   room tests free (returns 0), so the first candidate (X, Y − 20)
   becomes the target.
6. Minion timer: the free-spot search starts in the wanderer's room
   (`0x00620BB0`) from wanderer position + offset; dummy 131 is created
   in the room the search returns (`0x00555230(type 2, class 131, x, y,
   game, room, 1, 0, 0)`). A wanderer without a room finds no spot (every
   probe's `0x00463740` returns none), so no dummies; the bits, +0x0C,
   +0x0D and the quest-seed draw still happen.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| boss-choice mask | 0x40 on monstats byte +0x0D (`flying`) | `0x006CE280`, §11.4 |
| Gidbinn kill type-flag mask | 0x0E | `0x005B99B4`, §11.4 |
| weapon item type | 45 `weap`; body locations 5, then 4 | `0x0063BEF0`, §11.5 |
| progression step | 3 | `0x005BC17B`, §11.2 |

## Randomness

No draws beyond `world/quests-act3.md` §Randomness. The roomless
wanderer spawn fails before any draw (§11.7 rule 4); the minion timer's
quest-seed draw happens even when no dummy is placed (rule 6).

## Edge cases & original bugs

1. A roomless Dark Wanderer always walks to (X, Y − 20) (§11.7 rule 5).
2. The orb ignores a Will held in the swap slots (§11.5).
3. The Gidbinn boss's drop takes the boss's level, the tome's and
   Khalim's parts take the area level (§11.3).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Mephisto credit, expansion character, hell (difficulty 2), progression 12 | progression 13 (5·2 + 3) | §11.2 |
| Mephisto credit, classic character, normal, progression 5 | unchanged (n = 3 < 5) | §11.2 |
| boss choice for a flying monster (monstats `flying` = 1), chain 18 ready | not linked; +0x01 stays 1 | §11.4 |
| Gidbinn boss kill, victim champion (type flags 1, 4 and 8) | `g33 ` dropped, item level = victim stat 12 | §11.3, §11.4 |
| orb operate, Will at body location 4, inventory +0x1C = its GUID | accepted (counts as a hit) | §11.5 |
| orb operate, Will at body location 11 (swap) | sound 19 | §11.5 |
| wanderer walk target, wanderer has no room | (X, Y − 20) | §11.7 |

## Provenance

- §11 (2026-10-07, QC-1 … QC-7): a capstone linear sweep of every code
  section of `Game.exe` (scratch script outside the repo) for the
  `0x0065C360` call sites and their pushed arguments (17.3);
  `disasm.py fn` / `at` for `0x00559A30`, `0x00544E80`, `0x005B9930`,
  `0x005B9980`, `0x0063BEF0`, `0x005BC140`, `0x005B9A30`, `0x005B9AE0`,
  `0x005B9D40`, `0x005B9CD0`, `0x005B70B0`, `0x005B7160`, `0x005BD1F0`,
  `0x005BD0D0`, `0x005BD390`, `0x005B2A00`, `0x00545340`; `disasm.py
  xref` for `0x005BD040`, `0x005B7230`, `0x005B7120`; image byte at
  `0x006CE280` (0x40); `data/fields.tsv` for the monstats flag bits.
- D2MOO 1.10f (`A3Q1.cpp`, `QUESTS.cpp`) agrees on 17.3 (read, never
  set) and on the flying test of the boss choice; confirmed in 1.14d as
  above.

## Open questions

None of its own; `world/quests-act3.md` open questions 1 and 8 stay open.
