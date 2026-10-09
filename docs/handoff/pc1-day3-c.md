# Hand-back — PC 1 day 3, session C (branch `claude/local-pc1-day3-c`)

REC ids reserved for this session: 830–849.

## Recordings (HANDOFF §5 run queue)

- **Item 23 / "q-tool-poke on 1.14d" (REC-590), done 2026-10-09 (PC 1, Windows):**
  `py tools/trace-recorder/poke.py --poke-file traces/pokes/spawn-town.poke
  --auto ScnAma --seed 1234 --seconds 60 --after 400 --input "wait 3; shot
  spawn-town; wait 3" --shots <scratch>`: F0 2, results `{'ok': 5}`, not
  reached 0. Spawn at frame 4 (x 4876, y 4231) returned GUID 8, the brazier
  (object 39) at frame 5 (4870, 4231) GUID 18. The screenshot about 3 s
  after arrival shows the fallen party (four fallen, GUIDs 8–11 as in
  `poke.md` Test vectors) right-below the player, and the brazier lit
  left-below. The night palette of period 2 is visible. The Windows screenshot is not blank
  (the Wine screenshots were). It is not committed (game pixels).
  Note for the recipe: `--input "waitticks 40; shot …"` with the default
  `--after 50` took no screenshot, because the game stopped first. Use a
  time-based `wait` and `--after` ≥ 400.

## One 1.14d at a time (the user's rule, 2026-10-09)

Several PC 1 sessions launched `Game.exe` at once, and runs failed
(`player level at end: None`, `not reached 1`). Every recorder launch now
goes through `record_rng.CreateProcessW`, which takes the named mutex
`Local\d2rs-original-game-1.14d` and then waits until no `Game.exe` is
running (`tools/trace-recorder/README.md` "One game at a time";
`D2_GAME_LOCK=0` turns it off). Sessions A and B cherry-picked it. The
failed runs were repeated under the lock, and every one passed.

## Milestones (Step 3 of the brief: targets for the cloud playthrough harness)

New queue items: none were tagged `[play-act3]`, `[prov-data]`, `[play-act5]`,
`[prov-recording]` or `[store-fill]` on staging at 15:40 or 16:00, so this part ran.

Measured on 1.14d (PC 1, Windows, `Game.exe` sha256 631066c1…, `-seed
1234`, saves from `d2s-tool new --class sor --expansion --map-seed 1`):

| Milestone | Level | Arrival (x, y) | How it is reached in the runs |
|---|---|---|---|
| Act III entry | 75 Kurast Docks | (5118, 5168) | town byte act 2 at load, or `warp 75` |
| Act IV entry | 103 Pandemonium Fortress | (5048, 5043) | town byte act 3, or `warp 103` |
| Act V entry | 109 Harrogath | (5098, 5023) | town byte act 4, or `warp 109` |
| Baal's chamber | 132 The Worldstone Chamber | (15173, 5888) | `warp 132` (from Act I or Act V town) |

State findings:

1. **No quest gate at load.** A save whose town byte names act A starts in
   that act's town with `--quests none` exactly as with `--quests acts=A`
   (MilA3none / MilA3q → 75; MilA5none / MilA5q → 109; MilA4none → 103).
   For a direct start, the harness needs only the town byte (`d2s-tool
   --act`). The quest and waypoint state matters only for the in-game ways
   across: Meshif, the Durance portal, Tyrael's portal, and the Throne
   portal.
2. **The warp has no gate either.** `poke warp <level>` before frame 4
   (`poke.py` `CALL_FORMS["warp"]`, `0x0053AEC0`) moves the player in that
   frame: `record_state.py --poke "4 warp N" --auto ScnAma --seed 1234
   --ticks 40` shows act / level / position changing between the frame-3
   and frame-4 snapshots. The positions are the same as the load's, the
   mode is 5 throughout, and the 40 frames show no movement.
3. **Baal's chamber.** After `warp 132` the server holds Baal (monstats
   544) as monster GUID 8 at (15135, 5920), mode 1, from frame 4, from
   either start. From Harrogath, the Harrogath units stay in the list
   (classes 511, 513, 567–569); from the Rogue Encampment, its units stay.
   Objects in 132: classes 536 ×11, 537 ×6, 523 ×5–6, 267 ×1, plus the
   origin town's.
4. **Unit counts after the arrival frame** (all server units, origin town
   included): III 45, IV 62, V 46; 132 is 48 from Act I and 26 → 45
   (frames 4 → 5) from Act V. In the check files' runs (start from the
   previous act's town), the counts at frame 4 are III 33 (from Lut Gholein, level 40,
   (5153, 5203)), IV 58, V 59, and 132 26 → 45. The arrival positions match the table.

Check files (1.14d side, `scenario_diff.py <check> --orig-only`; the d2rs
side needs a release `d2-client` build, which was not run on PC 1):
`traces/checks/milestone-act3-entry.check`, `milestone-act4-entry.check`,
`milestone-act5-entry.check`, `milestone-baal-chamber.check`. Each one
warps from the previous act's town before frame 4 and records 40 frames
of state.

The same targets are in the harness format as
`traces/playthrough/milestones-a3-baal.play`: four milestones, `need`
on player lv / act / x / y (and m 5), plus exactly one Baal (ut 1, cl 544)
in level 132. It parses, and the harness selftest passes. It has not been
run on d2rs here (it needs the release `d2-client`). A failing milestone
there is a d2rs finding for the cloud: queue a q-fix with the
harness's evidence. Baal on 1.14d: monster level 60, hp 6779904 (1/256
points), path target = its position.

## Pending / not done

- No q-fix rows: the d2rs side of the four checks was not run on PC 1, so
  there is no d2rs difference to row yet.
- No `[play-act3]` / `[play-act5]` / `[prov-*]` / `[store-fill]` items had
  arrived by the last staging pull.

## Round 2 — items 42–44 (rendering session's items, numbered 41–43 on `claude/q-scenes-compare`)

- **42 Help button.** `0x004A64C0` is not the help button. It is the
  new-stats / new-skills sync: it opens and closes states 6 and 7 from
  stats 4 and 5 while panels 2 and 4 are closed, and does nothing while
  0x0B is open (`ui/control-panel.md` §8 r6).
  - The help button is state 0x22, drawn by `0x00495180`, with press
    `0x004952B0` and release `0x00495320` (new §11).
  - Opened at every game entry by `0x00456970`. On its first pass it
    closes itself when the registry value `Help Menu` ≠ 0. There is no
    level or first-game test.
  - The positions reproduce the scene's (714, 403), (725, 440) and
    (728, 436).
  - d2rs differs: `q-fix-p6-levelup-sync`, `q-fix-p6-help-button`.
- **43 Mini panel at entry (REC-519 settled).** Opened by `0x00456970` →
  `0x004567F0` unless the registry value `Mini Panel` ≠ 0. That value is
  written at game exit from the Esc-menu "was open" record
  (`ui/control-panel.md` §9 r9). d2rs differs: `q-fix-p6-minipanel-entry`.
- **44 Shadow pre-test (REC-511, REC-518 settled).** `0x00471620` calls
  `0x004709A0(U, X, Y, 0, 1)`. The last argument moves only the left
  bound, and the y bounds are the whole COF box, not sheared rows
  (`render/blend-modes.md` §5 r3, r3a). The object branch adds only the
  offsets; the `BlocksLight` gate is unit flag +0xC4 0x20.
  - REC-518: d2rs matches.
  - REC-511: d2rs differs, `q-fix-p6-shadow-pretest`.
  - Side note: `render/unit-composite.md` §4 and `client/model.md` §13 r2
    call the fourth argument "centering off". It is never read.

## Round 2 — placement of the Act IV / V bosses and quest objects (item 1)

Measured on 1.14d (PC 1, Windows, `-seed 1234`, saves `d2s-tool new --class
sor --expansion --act 2` (Act IV targets, from the Pandemonium Fortress) or
`--act 4` (Act V, from Harrogath)). Method: `poke warp <level>`, then
`pos @player` steps toward the target, with `record_state.py` snapshots every
frame. Rooms and presets come from `autostart.py` `dumpdrlg rooms<id>` (new:
the level's DRLG rooms with tile rect, type and lvlprest index). The DS1
positions come from the extracted DS1 files. Level origin = level rect × 5.

| Target | Level | Room (tile x, y, 8×8) / preset | x, y | Class, GUID, mode | Created |
|---|---|---|---|---|---|
| Izual | 105 Plains of Despair (rect 1064, 912, 64×80) | (1112, 960), lvlprest 822 `Act4/Mesa/Mid08X08Izual.ds1`, type-1 preset id 13 at (18, 18) | (5578, 4818) = room origin + (18, 18) | monster 256, GUID 91, mode 1 | on approach (room activation); first seen at Chebyshev 57 in a 30-step sweep |
| Hellforge | 107 River of Flame (maze, rect 1500, 1120, 144×192) | (1532, 1248) in the Forge E block (1524..1548, 1240..1264), lvlprest 854 `Act4/Lava/ForgeE.ds1` | (7661, 6255) | object 376, GUID 106, mode 0 | on approach: frame 89, the player in room (1540, 1248) next to it, Chebyshev 77 |
| Hephasto | 107 | same room; ForgeE type-1 preset id 27 at (65, 60) → (7685, 6260) | (7672, 6270) | monster 409 (superunique 41), GUID 81, mode 1 | on approach, the same frame as the forge, Chebyshev 66 |
| Anya (frozen) | 114 Frozen River (rect 2000, 1300, 64×64) | (2008, 1308), lvlprest 1038 (RiverIce01..04 by seed; this seed's matches RiverIce03, whose type-1 id 46 is at (50, 58)) | (10056, 6551) | object 558 `fana`, GUID 72, mode 0; **no** monster 512 / 527 | on approach, Chebyshev 30 (10-step path) |
| Nihlathak | 124 Halls of Vaught (rect 2500, 1000, 84×84) | (2540, 1072), lvlprest 864, `NihlN.ds1` (type-1 id 49 at (207, 393)) | (12706, 5391) | monster 526, GUID 66, mode 1 | on approach; first seen at Chebyshev 51 in a 30-step sweep |
| Baal (throne) | 131 Throne of Destruction (rect 3000, 1000, 40×52) | (3016, 1000), lvlprest 1086 `wthrone.ds1`, type-1 id 28 at (90, 11) | (15090, 5011) = level origin + (90, 11) | monster 543, GUID 53, mode 1 | on approach, Chebyshev 60 (from the south entrance at (15103, 5213)) |
| Worldstone Chamber portal | 131 | same room | (15090, 5005) | object 563, GUID 71, mode 1 | on approach with Baal, Chebyshev 66; present with no quest progress |

Arrival points of the warps (tile 0): 105 (5456, 4731), 107 (7803, 5918),
114 (10303, 6603), 124 (12727, 5223), 131 (15103, 5213).

Approach rule: none of the targets exists when the level loads. Each is
created when its room is populated, which happens when the player's room
becomes one of its near rooms (Hellforge and Hephasto: the player stepped
into the neighbouring room (1540, 1248)). The distances above therefore
depend on the room grid and the approach path, not on a fixed radius.

Recording notes:
- `pos` reaches only rooms near the player's room (the room comes from
  `0x00463740`), so a blind sweep stalls in mazes and voids. Use the room
  dump to plan a path through rooms.
- A frozen-Anya monster is never created at this point. Only the object
  exists.

Check files (1.14d side recorded with `scenario_diff.py <check> --orig-only`):
`traces/checks/milestone-{izual,hellforge,hephasto,anya,nihlathak,baal-throne,worldstone-portal}.check`.

The check runs reproduce every position, class and mode above. The GUIDs
depend on the order in which rooms are populated, so they follow the path
taken. The values for the check files' own paths:

| Check | GUID | First seen | Player then | Chebyshev |
|---|---|---|---|---|
| milestone-izual | 24 | f 28 | (5520, 4780) | 58 |
| milestone-hellforge | 89 | f 133 | (7730, 6260) | 69 |
| milestone-hephasto | 80 | f 133 | (7730, 6260) | 58 |
| milestone-anya | 68 | f 92 | (10070, 6560) | 14 |
| milestone-nihlathak | 38 | f 40 | (12721, 5329) | 62 |
| milestone-baal-throne | 53 | f 52 | (15100, 5070) | 59 |
| milestone-worldstone-portal | 71 | f 52 | (15100, 5070) | 65 |

The harness should test class and position, not GUID.
