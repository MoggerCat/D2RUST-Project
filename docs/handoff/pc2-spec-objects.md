# Handoff: PC 2 spec worker — objects (`claude/spec-objects`)

Spec session, 2026-10-07. Base: `origin/claude/local-pc2-integration`
plus a merge of `origin/claude/spec-objects-buddy` (it brought in
`world/object-population.md` and `world/objects.md` §16–§18, which had
not reached the integration branch). Evidence: the 1.14d `Game.exe`
(Ghidra exports + `tools/ghidra/disasm.py`), live `patch_d2` tables, and
recording `obj1` (`origin/claude/local-buddy-q9-rec-2026-10-07:docs/handoff/local-buddy-q9-rec.md`).

`world/objects.md` passed 60 KB, so §16–§18 moved unchanged to
`world/objects-2.md` (numbers kept; new §18.6 there).

## Answered

Questions from `docs/handoff/impl-objects.md` §4 = HANDOFF §7 "Ninth
set" OB-1 … OB-24, and `objects.md` open questions.

| Id | Spec § | Answer |
|---|---|---|
| OB-1 | objects.md Test vectors | the fifth class-4 pick is index 4 = id 9 (live `LevelMin` 26); vector fixed, result id 13 unchanged |
| OB-2 | §4 r4 | the speed sum is 32-bit (sign-extended d, d >> 4), ≤ 0 → 0, ≥ 0x7FFF → 0x7FFF, low 16 bits stored (`0x00624558`) |
| OB-3 | objects-2.md §18.6 | event 1: mode written directly to 2; the footprint free is inside the mode-1 / `Mode2` branch (`0x00581490`) |
| OB-4 | §14 | 0x4D @6 = operator field − 1 = the operator's GUID; builder `0x0053D4D0` sends 0x9A when its last argument ≠ 0 |
| OB-5 | §14 | S→C 0x60: 0x60, portal flags (data +0x05), destination level, object GUID (`0x0053D900`) |
| OB-6, OQ6 | §6 | 580 always runs 581 (371 replaces the pick at level 25); 581 draws one control step: act tables listed; 581 and 582 allocate in mode 0 |
| OB-7 | §8.1 r8 | locked chest with no operator → fatal in the key test (`0x0055F173`); the class-6 bypass tests the class id only |
| OB-8 | §8.1 r9 | confirmed: a null drop counts as neither, small bands go to the tail at once |
| OB-9 | §8.2 | wrong for barrels: 5 and 7 return 0, the others 1 |
| OB-10 | §8.2 | players/monsters: dx² + dy² ≤ 9 inclusive (`0x00641890`); barrels: `0x006416D0` ≤ 2 |
| OB-11, OQ3 | §8.3 | step first, then `0x005474C0` (no draws): family-base lookup over the level's monster region, cached; default 234 |
| OB-12 | §8.3 | fire objects 162 / 160 are allocated in mode 1; 160 tests only x against the room |
| OB-13 | §9.1 | 1.14d has no guards; record read unchecked after the mode change; no live case reaches either |
| OB-14 | §9.2 | codes 4 / 5 read unit totals and write base-stat adds (`0x006272B0`), not set-stat |
| OB-15 | §9.2 | code 16 reverses P's name in player data; code 17 with no free spot creates nothing (portal in P's room) |
| OB-16 | §9.3 | storm: base add of −trunc((life>>8)·Arg0/100)·256; i outer; missile flags 3, relative offsets |
| OB-17 | §9.3 | potion drop inline: `0x00633680`, `0x00555DA0`, `0x00558D90`, quantity via `0x00627260` |
| OB-18 | §10 | confirmed: doors have no assassin exemption |
| OB-19 | §10 | confirmed: unsigned wrapping compares (doors, gate, portal) |
| OB-20 | §11 | "used" = any heal write / state removal / `0x00578C20` / pet callback; set-stat writes; `Parm2` 0 → 0 charges, never heals |
| OB-21, OQ7 | §12 r4–r14 | full portal order; no operator or non-player → fatal (`0x0058494F`), not a refusal; quest gate via leveldefs `QuestFlag`/`QuestFlagEx` |
| OB-22 | §5.5 | the init footprint stamp takes room / x / y arguments and the class size and mask; it needs no path record |
| OB-23 | §7.3 | 0x13 object case: operate or walk → 0; no object / > 50 → 1; mode ≥ 8 or object gone → 3 |
| OB-24 | `object-population.md` Outputs | population allocates in mode 0; the init and PreOperate decide the rest (d2rs reading is right) |
| OQ2 | §4 r5 | Answered (confirmed by recording `obj1`), binary agrees (`0x005553E5`, `0x00623520`, `0x00624690` same-mode no-op) |
| OQ11 | `object-functions.tsv` | init 13 → §17, init 51 and operate 48 → §18.4 |
| relay (quests-core) | `object-functions.tsv` init 37 | already `world/quests-act2.md` on this branch; no change needed |

## Still open

- objects.md OQ1 (no operate recording), OQ4 (0x0E @6 and 0x4D zero
  fields: client handlers), OQ5 (warping-shrine callback: monsters spec),
  OQ8 (door mode 6 source: DS1 survey), OQ9 (obelisk completion), OQ10
  (needs recording), OQ13 (items drops), OQ14–16 (new).
- WW-6 rows owned by quest specs with no stating spec: init 7
  `0x00544990`, init 9 `0x00593FC0`, init 46 `0x005506D0`, init 59
  `0x0054FE10`, init 61 `0x00594290`, operate 33 `0x00583E70`, operate 43
  `0x00584D00` (cross-file requests below).

## CODE-TABLE CHANGE commits

- `d44907e` specs/world/object-functions.tsv: init 13 → §17, init 51 and operate 48 → §18.4

## Cross-file requests

- to PC 1: `sim/units.md` §6.4 type 1 — `0x00581490`: the mode test is
  16-bit, mode 2 is a direct field write (no mode set), and the
  `HasCollision2` footprint free (`0x00623830`) is inside the "mode 1 and
  `Mode2` ≠ 0" branch; reword the row (objects-2.md §18.6).
- to PC 1: `sim/server-messages.tsv` 0x60 — layout
  `flags:u8@1 level:u8@2 guid:u32@3` (`0x0053D900`); objects.md §14.
- to PC 1: `sim/server-messages.tsv` 0x4D — sender `0x0053D4D0`, layout
  `type:u8@1 guid:u32@2 a:u32@6 b:u8@10 c:u16@11 d:u16@13 e:u16@15`
  (the same builder sends 0x9A when its 7th argument ≠ 0); objects.md §14.
- to PC 1: `sim/server-messages.tsv` 0x58 — sender `0x0053D8D0`, layout
  `guid:u32@1 a:u8@5 b:u8@6` (b never written by the obelisk);
  objects-2.md §16.3.
- to PC 2 quests: state the quest-owned object functions no spec covers
  (rows stay `world/quests.md`): init 7 `0x00544990` (gibbet), init 9
  `0x00593FC0` (Inifuss tree), init 46 `0x005506D0` (trapped-soul
  placeholder: room-area monster spawner of classes 403 / 404 with
  control-seed draws), init 59 `0x0054FE10` (mode 0 → mode 1, event 7 at
  f + 27, ENDANIM), init 61 `0x00594290` (event 7 at f + 25), operate 33
  `0x00583E70` (Wirt's body; events 1 and 7), operate 43 `0x00584D00`
  (Duriel portal: asserts a player operator, portal flags |= 5, free
  point `0x0064E7B0`).

## Recording list

- objects.md OQ1: with `record_objects.py`, operate one chest, shrine,
  door, well and portal (packets + RNG): draw order of §8–§12, the 0x0E /
  0x4D / 0x60 bytes of §14.
- objects.md OQ10: a fire object (class 160–162) for a few seconds:
  no 0x0E 1 → 2 update expected.
- objects.md OQ14: in `obj1-objects.jsonl`, the mode argument of the two
  class-37 allocations with speed 0 (expected 2).
