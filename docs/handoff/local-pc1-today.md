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
Rows so far: 22 `q-fix-pc1today-*`.

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
Rows so far: 23 `q-fix-pc1today-*`.

## Push 6 (15:31) — stable UI recordings

- The run-to-run instability of the UI checks is explained and fixed: 1.14d
  reads the real pointer, so an unpinned cursor hovered Warriv in some runs
  (six glyph rows). All key-only draws checks now pin the cursor (`frame 38;
  move 790 10`), the skill-pick checks use `hold X Y 2`; 1.14d sides
  re-recorded twice: equal UI rows (`specs/tools/scenario-diff.md` Edge case 2).
  (`ui-draws-character-ama` re-checked later: three runs, 282 UI rows each.)
- With the pin, the belt key and the party key add no UI row on 1.14d (empty
  belt; single player).
- `pc1-data.md` Step 4: the audit item is marked (214 of 217).

## Push 7 (15:47) — cinematics, camera while moving

- **Cinematics menu**: opens with sound on (`-w`); with `-ns` (the recorders'
  default) the click enters `0x00431600` but the menu does not stay
  (`ui/frontend-credits.md` §C8 recorded, REC-2444). Scene
  `frontend-cinematics`, two runs, stable. Left needing PC 1 of the audit's
  217: one row (playing a cinematic: full-screen, owner's OK).
- **Camera rows (10) now have a verdict**: camera values of both sides over
  34 draws checks (`traces/pc1/camera-compare.tsv`): equal standing, DIVERGED
  while walking / running (new checks `draws-walk-ama`, `draws-run-ama`); row
  `camera-moving`.
- Row `frontend-draw-dump`: d2rs needs a draw-list dump for front-end screens
  and paused menus before the 13 front-end / menu scenes can be compared.
Rows so far: 25 `q-fix-pc1today-*`.

## Push 8 (15:57, before the 14:30Z ledger freeze)

Merged `claude/integ-r23` and `claude/specs-staging-7`; `ledger.py --check`
0 errors (the part's `last_verdict` cells were re-fixed with `--fix` after
each merge, since the coordinator's fix pass and this part touch the same
lines).
- Three **action scenes** on Windows, draws + pixels: `draws-melee-bar`,
  `draws-frost-nova-sor`, `draws-fire-bolt-sor`: about 84.5 % of pixels equal
  (light ring around the player, the Help button / mini panel shown only by
  d2rs, the player's composite); row `action-scenes`.
- Private repo 0592b709: the frame pairs of all 35 compared scenes.
- Totals of this session: Step 4 empty; audit list 216 of 217 rows recorded
  (left: playing a cinematic); 26 `q-fix-pc1today-*` rows; 36 new checks
  (`ui-draws-*` 25, `draws-*` 11); facts scenes: 5 option menus, trademark,
  credits, cinematics; 17 client-state recordings.

## Push 9 (16:08)

- **Delete Character prompt** captured twice (`frontend-character-delete`,
  442 UI rows, 0 differing; answered NO, no save deleted). The earlier misses
  were timing: character select needs about 8 s here before a click lands.
- Front-end capture recipe on Windows (`record_frames.py --front-end`):
  `wait 14` before the first click on the main menu; plain `click X Y`;
  sound on (`-- -w`) for the Cinematics menu; `wait 9` after opening character
  select.
- Every `ui-draws-*` check is stable on the 1.14d side now (cursor pinned).

## Push 10 (16:16)

Four more front-end scenes, each recorded twice (0 UI rows differing but the
heroes' animation phases): `frontend-character-select-classic` (a classic
character selected), `frontend-create-name` (name typed, check boxes),
`frontend-create-hardcore`, `frontend-hardcore-warning`. Not captured: Other
Multiplayer and the Convert prompt (their buttons did not react to posted or
held clicks in three tries). Test saves left in the save folder: `Doll*`,
`FeClassic`.

## Push 11 (16:59) — new goal (99 %); rain lines

Merged `claude/integ-r23` 174dbce3d (ledger: EQUAL 2866 of 4479, check 0
errors). `pc1-data.md` Step 4: no new open item at this merge. REC used:
2445.

**Rain (item f): 1.14d rain is reproducible.** 26 runs on Windows, 70 drawn
frames each: `traces/pc1/rain-lines-a1-town.tsv` (cursor never moved),
`rain-lines-a1-town-pin.tsv` (cursor moved), `rain-lines-a3-town.tsv`; tool
`tools/trace-recorder/rain_lines.py`. Each file gives, per drawn frame, the
client seed before and after, the steps, and every pass-9 line; the header
names the groups of equal runs.

- The lines depend only on the client seed stream, and the stream is
  stepped per **drawn frame** (key the comparison by frames-raw `seq`, not
  by tick or client update).
- Exactly two things vary between runs:
  1. the client update count at the first drawn frame (1 or 2, load time).
     Rogue Encampment: no effect. Kurast Docks: the splash arming of
     `draw-order-2.md` §11.5 is keyed on the update count, so a 3-step
     splash spawn lands in drawn frame 11 (first frame = update 2) or
     later (update 1); two groups, each exactly repeatable.
  2. the cursor's idle change after 5,000 ms of wall-clock time (state
     1 → 2, one seed step fewer per frame afterwards): frame 40 or 41
     with no pointer move. This is what made REC-510 look random.
- With the pointer moved every 18 frames: ten of ten runs equal in each
  town (within one first-update value).
- New check `draws-town-rain-ama` (no weather skip, cursor moved every 18
  frames), both sides run here: DIVERGED. d2rs draws 36 rain lines against
  35, five start points equal, no colour equal (another colour table), and
  the list first differs at row 191 (a `unit` row for object 2:5 that d2rs
  lacks; `draws-town-arrival-ama` shows the same on this build).
- Rows: `q-fix-pc1today-rain-phase`, `-rain-colors`, `-cursor-idle-checks`
  (the last one matters to every draws check: a check pinned once at frame
  38 can idle at frame 74 here, earlier on a slower host).

Open: re-run of the 35 scenes and the draws checks on this merge (next
round); music-stream pairing; the 117 rows still marked needs_pc1.

## Push 12 (17:27) — music pairing solved; draws checks re-run

REC used: 2446.

- **Music-stream pairing (`tools/audio-diff.md` OQ1): solved, it was a
  setting.** PC 1's registry had `Music Volume` = 0, and at 0 1.14d starts no
  song, so the Windows captures never held a music voice. The two stereo
  streams they do hold are `wilderness day 2.wav` (T 0) and `rain2.wav` (T 3),
  equal to d2rs's samples byte for byte through every refill. With Music
  Volume 100 for one run (set back to 0 after): 23 voices as under Wine,
  `town1.wav` pairs at T 0, 9 voices paired against 6
  (`traces/audio/win/audio-town-ambience-ama.summary-win-music100.json`).
  Left for the song: device volume 1.14d −730, d2rs −1348. Row
  `q-fix-pc1today-audio-settings` (checks must pin the volume settings on
  both sides).
- **All 38 draws checks re-run on this merge** (both sides, Windows): all
  still DIVERGED. `traces/pc1/draws-first-diff.tsv` is the baseline of
  verdict and first difference per check, so later rounds report what moved.
  Where they stop now:
  - 14 checks at one row: 1.14d's `unit` row for object 2:5 (site
    `0x004DC952`, no cel), which d2rs does not emit (row `unit-row-object`).
  - 5 at the NPC speech balloon's frame (row 173, 1.14d 2 against 3). Over
    23 recordings of the same start that frame is 2..6 at the same tick, so
    it follows neither the tick nor the client update count (row
    `balloon-frame`).
  - `draws-a3-town-ama` at a splash cel (row 97, site `0x00473BD0`): the
    update-count phase of the rain finding.
  - the rest at the player's composite (shadow / file / dir, row 98, as
    before) and camera origin while moving (walk, run).
- The 1.14d side draws 60–72 frames by tick 73 under the draw-logging
  recorder, depending on the open panel; whatever is keyed by drawn frames
  moves with it.

Running: pixel comparison of the 35 scenes on this merge. Open: the other
three audio checks at Music Volume 100; the 117 needs_pc1 rows.

## Push 13 (17:59) — merge 02516cee1; pixels round 2; rain is now the blocker

Merged `claude/integ-r23` 02516cee1 (ledger: EQUAL 3116 of 4479, check 0
errors). Step 4: no new item. Recorders unchanged, so every 1.14d side in
`traces/orig-cache` stays valid.

- **Draws checks on this merge** (`traces/pc1/draws-first-diff.tsv`, column
  `prev` = the round before): all 38 still DIVERGED, but 16 moved forward:
  the `unit` row for object 2:5 is fixed, and those 16 (town arrival, town
  rain, belt, the hovers, inventory, quest log, skill tree, skill bar, both
  skill picks, mini panel, party) now stop at the **first rain line** (row
  195 / 198, column x). Rain is what stands between them and the UI rows.
  d2rs's rain on `draws-town-rain-ama`: 36 lines against 35, 5 start points
  equal, no colour equal (1.14d indices 22, 25, 27, 115, 185, 188, 192, 198,
  200, 206, 231; d2rs 19, 114, 179, 180, 183, 226, 236, 237, 242).
- **Pixels, round 2** (`traces/pc1/pixel-compare-r2.tsv`, measured on
  174dbce3d; frame pairs in the private repo 64a62800,
  `recordings/pc1-2026-10-10/pixel-compare-r2`): 32 of 35 scenes moved, most
  up by 0.1–0.8 points (town arrival 97.56 → 97.78 %, character 97.88 →
  98.44, party 97.44 → 98.28, skill bar 96.03 → 96.86); down: Act IV town
  98.46 → 97.80, hover-mana 98.13 → 97.67; help overlay (12 %) and message
  log (25 %) unchanged; action scenes unchanged (84.4 %).

Running: a sample of cloud (Wine) 1.14d recordings re-recorded on Windows
and compared line by line. Next: the rain colour tables read from the live
game; the other three audio checks at Music Volume 100.

## Push 14 (18:28) — 9 waypoint rows EQUAL; rain colours; Wine = Windows

REC used: 2447. Ledger: EQUAL 3125 of 4479 (+9), check 0 errors.

- **Waypoints 18–26 → EQUAL (9 rows).** 21 cached 1.14d sides
  (`gen-wp-0`, `9`–`16`, `18`–`27` and two more) were recorded before the
  `@wp` fix of `send.py`: their waypoint message was never sent (poke
  result `gap`) and the player never left town. Re-recorded on Windows into
  `traces/orig-cache`; all 21 now give state PARTIAL with the client gap
  only (460 frames, about 26,000 unit records each). Rows in
  `rc-00-local-pc1-today.tsv`.
- **Rain colours: d2rs's tables are right, its day period is wrong.** Live
  tables read from 1.14d (`traces/pc1/rain-color-tables.tsv`,
  `draw-order-2.md` §11.4): every rain line of a fresh town start uses the
  day-period-0 table; d2rs's lines use the period-1/3 table. Row
  `q-fix-pc1today-rain-colors` rewritten with the values. This and
  `rain-phase` are what 16 draws checks now wait on.
- **Wine recordings equal Windows recordings.** Ten cloud-recorded cache
  entries re-recorded here and compared record by record
  (`traces/pc1/wine-vs-windows.tsv`): every snapshot equal; the one
  difference is a pointer in a poke result (`eax`). So the cloud's 1.14d
  sides can be trusted as ground truth for state.
- **Cache staleness (item d):** 2,045 of 2,157 cached checks carry an older
  recorder hash (recorder edits of 2026-10-10: `send.py`, `record_state.py`),
  339 of them behind DIVERGED / NO-CHECK rows. The sample says their content
  is unchanged but for the player's `q` field encoding and the waypoint
  message above, so I re-recorded only the 21 whose content was wrong. A
  full refresh is about 50 hours of game runs on this PC: say if you want it.
- **Audio at Music Volume 100** (all four checks,
  `traces/audio/win/*.summary-win-music100.json`): the song pairs in each;
  frost nova is down to 2 differences (song device volume −730 against
  −1348; one mono 1.14d voice at T 75 d2rs does not start).

Open: nothing in Step 4. Still DIVERGED on Windows: all 38 draws checks
(16 at the first rain line), the four audio checks.

## Push 15 (19:00) — merge c3e9ddc24; Step 4 [rc-link-2] answered

Merged `claude/integ-r23` c3e9ddc24. Ledger: EQUAL 3217 of 4479 (13 of
them from this push), check 0 errors. Messages recorded for this item:
`traces/pc1/net-provoked.tsv`. (Push times above were corrected to the
commit times.)

- **13 client-message ids without a handler → EQUAL**: 13 injected checks
  `net-c2s-unused-XX-ama`, packets MATCH on both sides (1.14d neither
  dispatches nor answers; the game runs on).
- **Overhead chat (c2s 0x14) → DIVERGED with the cause.** The real client's
  message for `!hi` is 8 bytes; the cloud's injected one had 7, and 1.14d
  drops that before dispatch. With 8 bytes 1.14d answers S→C 0x26 (type 5)
  one frame later; d2rs sends none. Typed chat works with posted input on
  Windows (`net-chat-typed-ama`: 0x14, 0x15, both 0x26 forms). Row
  `q-fix-pc1today-overhead-chat`.
- **Pong (s2c 0x8F)**: 1.14d sends it, at the join (answer to the client's
  own ping, before tick 1) and again for the injected ping at frame 9.
  d2rs has no join ping, and handles the injected ping twice. The packets
  comparison still says MATCH: it leaves system messages and the pre-tick
  join records out. Row `join-ping-pong`.
- **Hotkey (s2c 0x7B)**: sent only at the join, one per hotkey in the save
  (recorded: `7b 00 02 00 ff ff ff ff` for F1 = Throw on a save written by
  1.14d; the save is in the private repo). A bind in game sends C→S 0x51
  and no 0x7B (`net-hotkey-bind-ama`; d2rs has no headless skill hotkey).
  Row `hotkey-join`.
- The 37 s2c ids on the stub handler `0x0045C900` cannot be provoked by any
  session; party / trade / PvP ids need two clients (Other Multiplayer is
  out of scope); item creation R1 not done.

Open in Step 4: nothing else.

## Push 16 (19:32) — merge d4f79226f; Step 4 [rc-audio-fmt-div] answered

Merged `claude/integ-r23` d4f79226f. Ledger: EQUAL 3283 of 4479, check 0
errors.

- **Windows voice lists** for all 14 audio checks:
  `traces/audio/win/<name>.orig-win.voices.jsonl` (digests only), with the
  d2rs comparison of each in `<name>.summary-win-music100.json`. Captured at
  Music Volume 100 (set for the batch, back to 0 after).
- **1.14d audio does not repeat run to run on Windows either**: each check
  captured twice, 1 of 14 equal (`traces/audio/win/voices-two-runs.tsv`).
  What moves: stream digests, fade ramps, stop ticks, a start tick by one,
  variant picks. So one capture, Wine or Windows, cannot be the reference;
  row `q-fix-pc1today-audio-repeat` (compare streams by prefix, pin the
  cursor, log the client seed and drawn frames per sound tick).
- **Draws checks on c3e9ddc24** (`traces/pc1/draws-first-diff.tsv`): all 38
  DIVERGED; 5 moved forward a long way (char-skill row 9 → 375, inv-char-l5
  22 → 324, quest-skill 28 → 148, inv-char 243 → 286, inv-char-tip 249 →
  292). Not yet re-run on d4f79226f.

Open in Step 4: nothing. Waiting on the owner / coordinator: whether to
refresh the 2,045 stale cache entries (about 50 hours).

## Push 17 (20:45) — merge 5567801d3; Step 4 [rc-sysrender-div] answered; player light regression

New brief from the owner (99 % EQUAL, no deadline; REC ids 3200..3249).
Ledger after the merge: EQUAL 3465 of 4479, DIVERGED 653, NO-CHECK 278,
check 0 errors. No ledger row is flagged `needs_pc1` now.

- **Draws re-run** (38 checks, build = integ-r23 4e573a7c5,
  `traces/pc1/draws-first-diff.tsv`): all still DIVERGED. First difference
  moved in 6: blood-moor 98:file → 137:x (rain line), cave 65:file →
  108:frame (menu button frame 0 against 2), fire-bolt and frost-nova
  98:dir → 106:frame (shadow frame of the cast), inventory and skilltree
  195:x → 172:frame (the NPC balloon frame, not a function of the tick).
- **Camera** (`traces/pc1/camera-compare.tsv`, now 38 checks): no value
  changed; only walk / run differ, as before.
- **Pixels round 3** (`traces/pc1/pixel-compare-r3.tsv`; pairs in the
  private repo, `recordings/pc1-2026-10-10/pixel-compare-r3`, 4b292c4b+1):
  the 25 scenes that show the world fell 13–27 points (town arrival
  97.78 → 75.83). d2rs no longer draws the lit area around the local
  player. Read on PC 1: `remove_unit_light` removes the unit's own light,
  where 1.14d's `0x00461250` removes only the cast light of the skill list
  (`0x00643A00(U, 0)` → `0x004743D0`, `client/model.md` §8 r4); commit
  1bc2ef0c2 put that call on the join placement. Row
  `q-fix-pc1today-cast-light`. Panels that cover the world rose a little
  (char-skill 99.01 → 99.87, inv-char-tip 97.06 → 99.85).
- Also seen in the d2rs frame of town arrival: the mini panel and the Help
  button are drawn (1.14d on PC 1 has both off through the registry) and
  the cursor sits elsewhere. The recorder on integ does not yet write the
  registry values (Mini Panel, Help Menu) into its header; the per-frame
  clock (`cursor.last_step`) and `-ns` are in every capture of this round.
- **Draws recorder gap** (`?` cells): they come from `facts_render.py`, not
  from the capture: mode / light / pal of every cel wrapper but `CelDraw`
  ("argument positions not specified"), the file of text glyphs
  (`CelDrawColor` from 0x501BC0) and of the balloon (`CelDraw` from
  0x46E539), and the box of `CelDrawClipped` (automap, 747 rows) and
  `CelDrawEx`. The cloud holds the claim (C-ui-draws-q); re-recording
  changes nothing until that lands.
- **Audio**: all 14 audio checks have Windows voice lists
  (`traces/audio/win/`, push 16); no new audio check came with this merge.
- Merge conflicts: the cloud re-recorded 14 `ui-draws-*` cache entries
  under Wine at the same time; the Windows entries of this round were kept.

Open in Step 4: nothing new for PC 1.

## Push 18 (21:15) — draws re-run on 5567801d3

- 38 draws checks on integ-r23 5567801d3 (`traces/pc1/draws-first-diff.tsv`):
  all DIVERGED. First difference moved in 2, both from the NPC balloon
  frame, which is not a function of the tick (automap 173:frame → 198:x,
  inventory 172:frame → 195:x, the rain line). Nothing else moved.
- Pixel table not re-run: every world scene would show the missing player
  light again (`q-fix-pc1today-cast-light`); it runs once that fix is on
  integ.
- For the `?` cells (claim C-ui-draws-q): the argument orders are already
  in the specs: `CelDrawEx` (x, y, skip, lines, mode) in
  `ui/control-panel.md` §(line 141), `CelDrawColor` (x, y, light, mode,
  colour) same file line 507, `CelDrawClipped` (x, y, rect, mode) in
  `ui/automap.md` §10 r4; `CelDrawShadow` takes (x, y) only, so its mode /
  light / pal cells are `-`, not `?`.
