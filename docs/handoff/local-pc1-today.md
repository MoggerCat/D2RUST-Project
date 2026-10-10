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

## Push 2 (13:10) — Step 4 is empty; Windows recordings

Merged `claude/specs-staging-7` (bab315e49 and later). No open item is left in
Step 4 (the audit's regrouped list had not landed at this merge).

- **Client seed** (two items): one seed, {1, 666} at start, `S[16]` =
  {0xE4CA4C4E, 0x3A4FDE2B} at the first sound tick: derived from the binary
  (`client/model.md` Randomness r4, `audio/sound-table-2.md` §14.5,
  `render/draw-order-2.md` §11.3) and **confirmed on the live game**
  (`traces/pc1/client-seed-town-ama.tsv`). Row `cursor-shared-seed`.
- **Equipped paper dolls (REC-2181)**: five saves, saved once by 1.14d, then
  character select captured: appearance bytes and component file names in
  `traces/pc1/charselect-dolls.tsv`, `ui/frontend-menus.md` §F2.10 r8. A
  d2s-tool save alone has all-0xFF appearance bytes: row `d2s-tool-appearance`.
  Pixels vs d2rs still open (saves and frames go to the private repo).
- **Audio on Windows**: `audio_diff.py run` now works on Windows; all four
  `traces/audio` checks ran, all DIVERGED; effects pair by samples, the music
  streams do not (`tools/audio-diff.md` OQ1, summaries in `traces/audio/win/`).
  Frost nova item answered (no second `coldcast.wav`).
- **gen-wp 1..8, 17, 28..38**: 1.14d sides re-recorded into `traces/orig-cache`
  (every 0x49 accepted).
- **New checks**: 20 `ui-draws-*-ama` draw-list checks (panels, hover texts,
  skill pick lists, item tooltip), for the UI rows that had no check.
- Variant installs on PC 1 live outside the repo (`..\variants`, linked at
  `d2rs\variants`), so `variant` checks run here now.

Running: every check behind a needs_pc1 ledger row (143) plus the UI checks,
both sides on Windows, about 2 h. Ledger part
`docs/handoff/ledger/local-pc1-today.tsv` follows with that batch.
Rows so far: 21 `q-fix-pc1today-*`.

## Push 3 (14:30) — the audit's Windows list (217 rows), UI family first

Merged `claude/specs-staging-7` and `claude/integ-r23`. The batch over the
other needs_pc1 checks was stopped when the audit moved them off this list
(102 checks had run; their 1.14d sides are in `traces/orig-cache`).

**Ledger part: `docs/handoff/ledger/rc-00-local-pc1-today.tsv`** (177 rows). It
is named `rc-00-…` and not `local-pc1-today.tsv` because `ledger.py` keeps the
first `rc-*` part by name for an area, and `rc-gen-ui` / `-render` / `-audio`
own these areas: under the plain name only 3 rows took effect.
needs_pc1 = y and not EQUAL: 383 → 235. NO-CHECK 1480 → 1444.

| Family | Rows | Done | State now |
|---|---|---|---|
| ui: inventory, control-panel, panels-2, panels-3, controls, text, messages | 73 | 25 `ui-draws-*-ama` checks, both sides on Windows, 1.14d side recorded twice (five of them a third time) | DIVERGED, needs_pc1 n; row `ui-draws` |
| ui: frontend-options | 9 | facts scenes `a1-menu-options / -sound / -video / -automap` (two runs, stable) and `-controls` (one run) | NO-CHECK (no d2rs comparison yet), needs_pc1 n |
| ui: frontend-menus | 23 | `frontend-trademark` scene (two runs); dolls trace; day-4 scenes | NO-CHECK, needs_pc1 n |
| ui: frontend-loading | 11 | day-4 scene stands | NO-CHECK, needs_pc1 n |
| ui: frontend-credits | 11 | trademark only | **still needs PC 1**: Credits / Cinematics (and Delete Character) do not react to posted clicks on Windows |
| audio: sound-table, triggers-2 | 22 | four audio checks on Windows, client seed recorded | DIVERGED, needs_pc1 n |
| render: camera | 10 | `camera-0001`, `placement-0001` recorded; values in `traces/pc1/*.tsv` | NO-CHECK, needs_pc1 n; row `capture-case-format` |
| render: unit-composite, sprite-placement | 18 | real-GPU `verify`: 11 pass (Intel HD 630, Vulkan); frames in the private repo | **still open** (no pixel comparison run) |
| client: model, msg-units, msg-stats-items, stat-lists; seams | 38 | not started | still needs PC 1 |

Not stable on the 1.14d side (re-record before trusting): `ui-draws-questlog`,
`-left-skill-pick`; `-belt`, `-party`, `-automap` differ by a 6-glyph transient
text in one of three runs.
Private repo: `recordings/pc1-2026-10-10/frames` (d313e3ac).
Rows so far: 23 `q-fix-pc1today-*`.

## Push 4 (14:46) — client state, pixels

- **system.client (36 rows) and seams (2)**: new 1.14d-side channel:
  `record_state.py --client-out` writes the client's own unit sets S and C per
  frame (`tools/state-snapshot.md` §3 r5). 17 checks recorded in
  `traces/pc1/client-state/` (30 MB); two runs equal but the local player's
  client seed. needs_pc1 → n; still NO-CHECK until d2rs dumps its
  `ClientWorld` (row `client-state-dump`).
- **system.render unit-composite, sprite-placement (18 rows)**: pixel
  comparison on real Windows for 26 scenes (`traces/pc1/pixel-compare.tsv`):
  town arrival 97.56 % equal; UI-only scenes 98.9–99.7 %; help overlay 12 %,
  message log 25 % (d2rs draws neither). DIVERGED, needs_pc1 → n. Row
  `pixel-diffs` (Defense 6 vs 0 on the character panel, the missing globe
  label, missing overlays).
- needs_pc1 = y and not EQUAL: 235 → 179 (of the audit's 217 only
  `ui/frontend-credits`, 11 rows, is left: Credits / Cinematics do not take
  posted clicks on Windows).
Rows so far: 25 `q-fix-pc1today-*`.

## Push 5 (15:02) — credits, more world scenes

- **frontend-credits**: the Credits button does open (wait 14 s for the menu,
  then one plain click); scene `facts/render/scenes/frontend-credits` recorded
  twice (122 UI rows, 0 differing). 8 rows → needs_pc1 n. The **Cinematics
  button and Delete Character** still do not open by posted clicks in this
  windowed game: 3 cinematics rows stay needs_pc1 y.
- **Six more world draw-list checks** (`draws-a2-town`, `-a3-town`, `-a4-town`,
  `-a5-town`, `-blood-moor`, `-cave`), both sides plus pixels: 97.3–98.5 %
  equal, all DIVERGED; row `world-draws` (after a warp d2rs draws the player
  in town-neutral mode; mini-panel button frame 0 vs 2).
- Private repo f47bdb6f: `recordings/pc1-2026-10-10/pixel-compare` (the 32
  1.14d / d2rs frame pairs), credits frame.
- needs_pc1 = y and not EQUAL: 171. Of the audit's 217 rows, 3 are left.
Rows so far: 26 `q-fix-pc1today-*`.
