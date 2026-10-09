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
