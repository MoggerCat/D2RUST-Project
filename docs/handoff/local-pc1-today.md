# local-pc1-today — PC 1, 2026-10-10 (branch `claude/local-pc1-today`)

Started 12:25 local from `claude/specs-staging-7` f61de2413; hard stop 17:40.
REC block 2400–2449 (used: 2415, 2416, 2417, 2432).

## Push 1 (13:00) — Step 4 binary reads and two recordings

Answered (each line in `pc1-data.md` Step 4 is marked "answered (pc1-today)"):

| Item | Answer | Row (`q-fix-pc1today-…`) |
|---|---|---|
| Scan 5 callback `0x005DCA70` (REC-1642) | `monsters/ai.md` §5.4: flag-4 + hostile filter, C-size distance ≤ 35, threat main / alt, earliest wins ties; the item's "picks 1:17" was a misread (1:19) | `scan5` |
| LOS-draw test `0x0061AA40` | `ai.md` §5.2: DRLG room type 1 → true, preset → lvlprest `Outdoors`; true in levels 108 and 110 | `los-draw` |
| Mode set result `0x005A7C20` (REC-1390) | `sim/units.md` §4.6: the start's own value; d2rs right | — |
| Snap `0x00650660` (REC-1391, REC-1643) | `sim/pathing.md` §9.6 r3: only test is path type ≠ 4 | `snap-path-type` |
| Animation schedule +0x44 | `sim/units.md` §4.2: f·256, then the Attack skill start overwrites it with bonus·256 | `attack-start-frame` |
| Ancients' gate `0x0058CF90` (REC-1561) | `world/quests-act5-2.md` §7.9: pure test, no store; d2rs right | — |
| Inactive restore `0x005424F0` (REC-1560) | `sim/units.md` §3 r4.1: creation runs the whole boss-mod case | `restore-bossmods` |
| Per-class mode records | `sim/units.md` §4.6 (rc-mon-modes' text confirmed); d2rs right | — |
| cltstfunc 5, 20–24 (REC-1641) | `client/model.md` §8 r7 | `cltstfunc` |
| Client use state tests (REC-1640) | `ui/controls.md` §6 r9.1 | `use-state-tests` |
| Hit while walking (REC-1250) | `client/model.md` §8 r4 | `walk-hit` |
| Client path mover (REC-1385) | `client/model.md` §5 r7: single-player copy `0x00465070` each client update | `sp-path-copy` |
| A2Q4 palace hooks (REC-1631..1633) | `world/quests-act2.md` §10 | `palace-guard-point` |
| 0x27 text list order (REC-1401) | `world/npc.md` §2: prepends, newest first; cap 8 | `textlist-cap` |
| SpecialState06 `0x005E7C10` | already spec'd (`ai-bodies.md` §9.33) and implemented; recorded vector added | — |
| Shadow Master first think | `ai-bodies-7.md` §27 | `shadow-master-skill-list` |
| Druid vines f67 (REC-1111) | `ai-bodies-7.md` §20, `ai.md` §7.5 r4.6, `path-placement.md` §10 r6 | `pet-history-wiring` |
| Paper dolls (REC-1907, 2181..2184) | `ui/frontend-menus.md` §F2.10; facts `facts/ui/doll-*.tsv` | `doll-dy`, `doll-monster-root`, `doll-fixups` |
| Level 106 coordinate lists | recorded: `traces/pc1/gen-lvl-106-coordlists.tsv`; 8 rooms, 1260 tries = 1260 density draws | — (cloud diffs d2rs against the list) |
| Poke `@wp` gap | not a gap on the current recorder; `--send` fixed too | — |

Duplicates of items answered on 2026-10-09 (59–64, pc1-late A2, F1) are marked.

Running: 1.14d sides of gen-wp-1..8, 17, 28..38 into `traces/orig-cache`.
Open in Step 4: client seed at the first sound tick (two items), critter walk
(REC-742 recording), frost-nova audio check, equipped-doll captures (REC-2181).
Ledger: no verdict rows yet (`docs/handoff/ledger/local-pc1-today.tsv` comes with the suite runs).
