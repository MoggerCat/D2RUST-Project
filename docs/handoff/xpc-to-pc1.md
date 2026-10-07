# Cross-PC requests: PC 2 → PC 1

When a PC 2 answer needs a change in a PC 1 file (messages, client, AI,
skills, missiles, monster spawn/init, units, DRLG, render, `ui/text.md`,
combat, tools, data/formats other than wav and d2s), PC 2 appends a line
here instead of editing it: file, rule or question id, the 1.14d fact with
its address, the change needed. PC 1 handles each line and strikes it with
the commit that did it. The reverse direction is `xpc-to-pc2.md`.

## Open

- `specs/combat/vitals.md` §4.3 / OQ2 (from PC 2 spec-hirelings): `0x0057E480` applies the `ExpRatio` step `0x0057E390` after the level factor (alvl pushed at `0x0057E4CE`, gain in EAX): r = `0x00613E60(alvl)`, s = `0x00613E60(0)` (10); s − 1 ≥ 31 → unchanged; e > 0x7FFFFFFF >> (((r >> s) + s) & 31) → (e >> s)·r, else (r·e) >> s; then + pct(gain, stat 85, 100). Change: state it for players (same text as `world/hirelings.md` §7.2 rule 3).
- `specs/combat/damage.md` §7.2 (from PC 2 spec-hirelings): "pet kill credit to a player owner" is `0x005751A0` (pet death bookkeeping: node dead bit, 0x9B, 0x7A remove), run only when the 4th argument is non-zero (0 only from `0x00574450` at `0x005744D3`) and the owner is a player; the `killable` test is `0x00457490(class, 15)` at `0x0057CCF4`. Change: describe the step that way and link `world/hirelings.md` §8 rule 1.
- `specs/sim/pets.md` §5 r2 / SK-2a / OQ2 (from PC 2 spec-hirelings): the full-list eviction in `0x00575C70` calls `0x00574850(head GUID, list, 1)` at `0x00575CE5` (one 0x7A broadcast, then `0x00574450`), asserts 0x312 / 0x316; max recompute `0x00575900` (per-skill `petmax`, else `basemax`; `0x00575850` evicts with kill for types ≠ 7 while count > max). Change: state the eviction path and close OQ2 for type 7 (text in `world/hirelings.md` §5 rule 3).
- `specs/sim/server-messages.tsv` 0x58 row and `specs/sim/intents-events.md` §6 masking (from PC 2 spec-quests-act2): 0x58 is u8 0x58, u32 object GUID @1, u8 result @5 (0 open, 1 cancelled, 4 refused, 5 accepted), u8 @6 (1 = accepted with effect), built by `0x0053D8D0`; byte 6 is never written for result 0 (`0x0059DD6A`), 1 (`0x00585479`) and 4 (`0x0058538C`). Change: add the field layout and a mask on byte 6 for results 0, 1, 4 (also requested by `world/objects.md` OQ12).
- `specs/drlg/rooms.md` §8 rule 1 (from PC 2 spec-quests-core): the setter of DRLG room flag 0x400000 is `0x0061AED0(room, clear)` → `0x0061BAC0` (clear = 0 sets, ≠ 0 clears; quest code passes 0, missile bodies 1). Change: own the setter there (it is currently stated in `world/quests-act1-rest.md` §9 item 12, which will link to it).
- `specs/sim/units.md` §3.1 (from cloud impl-pc2-fixes, `docs/handoff/impl-pc2-fixes.md` item 5): to move d2rs's `SUNIT_Add` after the per-kind init, the spec must say where 1.14d's monster / object per-kind inits get their room and level before the unit is linked (d2rs reads them from the list entry `SUNIT_Add` fills). Code unchanged until then.
- `specs/monsters/ai.md` (from impl-pc2-fixes): nothing in the AI specs sets the monster re-path budget at path +0x94 to 20 (`pathing.md` §9.10); name the setter(s).
- `specs/drlg/*` §9.6 wall remap (from impl-pc2-fixes): corner handling, R+0x20 vs R's own half, is not stated precisely enough to implement.
- Staging-5 merge contradictions (commit 666e2f2 message): S→C 0x50 / 0x58 / 0x63 layouts and C→S 0x3A field name; `combat/vitals.md` §4.4 rule 2 (stat-holder vs `exp` state-group gate); `render/camera.md` §9 vs OQ5 (client path step once per update vs not; `[0x007A04C4]`), roof block count 13,432 vs 15,432, wall block count 104,780 / 251 DT1s vs 104,767 / 250. Each is marked "to reconcile" in place.
