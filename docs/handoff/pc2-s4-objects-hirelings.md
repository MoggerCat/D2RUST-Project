# Handoff: PC 2 spec worker, session 4 — objects and hirelings (`claude/spec-objects-hirelings-s4`)

Spec session, 2026-10-07. Base `origin/claude/pc2-spec-gaps` (`dd480ad`).
Evidence: the 1.14d `Game.exe` (Ghidra exports, `tools/ghidra/disasm.py`).
No recordings, no Ghidra runs, no cargo.

## Written

| Spec § | Behaviour | 1.14d addresses |
|---|---|---|
| `world/objects-2.md` §22 rule 4 | allocation mode of every object `0x00555230` call site: population, evilhut and barricade-door objects mode 0; traps' fire objects, town portal, quest objects mode 1; `0x0056CF40` portal mode 2; presets / restore by argument | 53 call sites, e.g. `0x0054E03B`, `0x0054E5AA`, `0x0054E5EA`, `0x00550526`, `0x005823B9`, `0x0056D092` |
| `world/objects-2.md` §24 | preset 580's mode-0 set queues + flag 0x1; barrel guard and order; storm kills nothing; shrine event 6 with no hover; well "used" = a write; locked door with no operator fatal; refill queue | `0x0054F370`, `0x005868A0`, `0x00582DA0`, `0x00581620`, `0x00585720`, `0x00581510` |
| `world/hirelings-2.md` §15 (new file) | player death (mode 17) kills the hireling in every game type: 0x7A, room removal notice, node dead, death mode request, 0x9B | `0x0057FCA0` → `0x00575BC0` → `0x00574570` |
| `world/hirelings-2.md` §16 | restore: class argument, `Id` 0xFFFF only from version-0x47 saves (class 0 unit, new-hire branch), roomless allocation and the join follow `0x005773D0`, row checks per loader, rule 7 order per loader, `0x005738D0` = cancel timer types 2 and 3 | `0x005774F0`, `0x0056AA50`, `0x00533C70`, `0x005738D0`, `0x005394A0` |
| `world/hirelings-2.md` §17 | item swap: the "item-removal notices" are cancels of type-9 timers with C's GUID (merc, then player); regen timer reset; failed duplicate loses the item; socket / replenish details of copies | `0x0054CED0`, `0x00540E60`, `0x0055A2A0`, `0x005606B0` |
| `world/hirelings-2.md` §18 | range pets: squared distance > 1600 to the player unit's position; free branch; list head in `0x00574570`; flags 2 bit 0x100 readers (OQ10) | `0x00575380`, `0x0057554A`, `0x00574570`, `0x0056D840`, `0x00574450` |
| `world/hirelings-2.md` §19 | entry points (callers and phase) and the tables a game needs | `index/calls.tsv` |
| `world/hirelings.md` §11 rules 3–4 | wording fixed (timer cancels), pointers §6 r8, §8 r6, §10 r9, §11 r8; §12 moved to part 2 (not cited by code); OQ10 answered | |

### Code TODOs → spec answer (implementers can resolve these)

| Code site | Answer |
|---|---|
| `world/objects.rs:388` (§4 r3) | `objects.md` §4 rule 4: 32-bit sum, clamps, low 16 bits |
| `world/objects.rs:908` (§6) | `objects-2.md` §24 rule 1: the §4 mode set (`0x00624690`) |
| `world/objects.rs:1176` (§14) | `objects.md` §14 builder details: field − 1 |
| `objects/chests.rs:205` (§8.1 r2) | `objects.md` §8.1 rule 8: fatal (`0x0055F173`) |
| `objects/chests.rs:330` (§8.1 r4) | `objects.md` §8.1 rule 9 |
| `objects/chests.rs:409` (§8.2) | `objects.md` §8.2 return values: barrels 0, others 1 |
| `objects/chests.rs:470` (§8.2 fn 5) | `objects-2.md` §24 rule 2: always cleared |
| `objects/chests.rs:635` (§8.3) | `objects.md` §8.3 trap monster id: step first, no draws |
| `objects/chests.rs:677` (§8.3 h5, h7) | `objects.md` §8.3 fire objects: mode 1 |
| `objects/misc.rs:106`, `:215` | `objects.md` §10 last paragraph: unsigned wrapping |
| `objects/misc.rs:117` | `objects.md` §10 (no assassin exemption); `objects-2.md` §24 rule 6 (no operator fatal, unreachable) |
| `objects/misc.rs:194` | `objects.md` §12 rule 5: non-player or none fatal; monsters stop at §7.1 |
| `objects/misc.rs:221` (OQ7) | `objects.md` §12 rules 4–14 |
| `objects/misc.rs:254` | `objects-2.md` §24 rule 5: "used" = a write, not a change |
| `objects/misc.rs:322` | `objects-2.md` §24 rule 7 |
| `objects/shrines.rs:228`, `:249` | `objects.md` §9.1 "No guards": unreachable with live data |
| `objects/shrines.rs:359` | `objects.md` §9.2: base-stat adds (`0x006272B0`) |
| `objects/shrines.rs:433` | `objects.md` §9.2 code 16: name reversal |
| `objects/shrines.rs:450` | `objects.md` §9.2 code 17: none → nothing |
| `objects/shrines.rs:479`, `:487` | `objects.md` §9.3 storm (read in 1.14d); `objects-2.md` §24 rule 3 (no kill) |
| `objects/shrines.rs:555` | `objects-2.md` §24 rule 4: nothing, no reschedule |
| `objects/tests.rs:24` | `objects.md` Test vectors: id 9 already fixed |
| `wiring/worldgen/population_init.rs:190` | `objects-2.md` §22 rule 4: mode 0 |
| `wiring/action/objects.rs:322`, `wiring/economy/quest_objects.rs:13` | `objects.md` §3, §5.5: order is specified; the d2rs order is an implementation divergence |
| `world/objects.rs` routes `NotCovered` (inits 8, 10, 13, 14, 22, 24, 26, 27, 28, 34, 58; operates 13, 16–20, 26, 27, 29, 30, 32, 47, 50, 51, 61; events 0, 3, 8, 9, 10) | all in `objects-2.md` §16–§18; init 37 `quests-act2.md` §8.8. Implementation gap only |
| `hirelings/items.rs:64` (§11) | `hirelings-2.md` §17 rule 2 |
| `hirelings/life.rs:230` (§6 r1) | `hirelings-2.md` §18 rule 1 |
| `hirelings/life.rs:257` (§6 r4) | `hirelings-2.md` §18 rule 3 |
| `hirelings/life.rs:311` (§10 r3) | `hirelings-2.md` §16 rules 1–2 |
| `hirelings/life.rs:384` (§10 r7) | `hirelings-2.md` §16 rules 5–6 |
| `d2-server …/world/wired.rs:373` (§8 r1) | `hirelings.md` §8 rule 1 (inside the kill); implementation order divergence |
| seams `HirelingTables` / death / 0x61 / restore / act change callers | `hirelings-2.md` §19 |

## Pending

- `world/hirelings-2.md` OQ1 (needs recording): a player death with a
  living hireling, to confirm 0x7A / 0x9B and what the owner's client
  shows (the server posts a room removal notice but keeps the unit).
- Not written on purpose: `world/object-population.md` OQ5 (theme bodies
  `0x00552000`, `0x00552140`, `0x00552200`): no theme runs with 1.14d
  data (§4), so no code needs them in Phases 0–6.
- Unchanged recording items: `objects.md` OQ1, OQ10, OQ14;
  `hirelings.md` OQ9; `object-population.md` OQ1.

## CODE-TABLE CHANGE commits

None (`object-functions.tsv` unchanged: every row's owner is already
specified).

## Cross-file requests

- `sim/pets.md` (§ on the player free / list walk): 1.14d `0x0057FCA0`
  (player mode-17 start) calls `0x00575BC0` at `0x0057FD25` in every
  game type, so on a player's death every pet of a type other than 7 is
  killed (`0x00574450`) and its node freed with count and max
  decremented, then `0x00575900` recomputes the maxima; add the player
  death as a caller (hireling part: `world/hirelings-2.md` §15).
- `monsters/population.md` §9 rule 6 and OQ3: the objects created at
  the evilhut leader (562, `0x0054E03B`) and at barricade-door monsters
  (571 / 572, `0x0054E5AA` / `0x0054E5EA`) are allocated in mode 0 with
  flag 1 and GUID 0 (`world/objects-2.md` §22 rule 4); state the mode
  there (code `wiring/worldgen/population_init.rs:190` waits on it).
- `world/objects.md` OQ5 (mine, no edit needed now): the eligibility
  test of `0x00582750` is specified in `monsters/init.md` ("Nearest
  eligible monster"); `ShrineWorld::make_nearest_unique` can implement
  it from there.
