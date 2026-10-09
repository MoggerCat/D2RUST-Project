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
