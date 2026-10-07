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
- `specs/sim/intents-events.md` §8.2 rule 3.1 "Static call order of the save path" (from PC 2 items lane): it names loader `0x00534020` and sites `0x005341D5`, `0x005341FD`, `0x0053423F`, `0x0053427C` inside it, but `0x00534330` sends versions ≥ 0x5C (every 1.14d save, 0x60) to `0x0056B180`; `0x00534020` is the pre-1.09 legacy loader (`formats/d2s.md` §1 rule 6). Change: redo the static order on `0x0056B180` (header `0x0056A090`, items `0x0056A7E0` → `0x005337F0`, post-load `0x0056AF80`) and check the recorded order still matches.
