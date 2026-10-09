# Hand-back — PC 1 day 3 (A), 2026-10-09 (branch `claude/local-pc1-day3-a`)

Queue: `docs/handoff/pc1-data.md` Step 4 item 23, REC-660, REC-661, then
new Step 4 items tagged `[seed-order]`, `[recording-2]`, `[proto-items]`
or about towns, NPCs, quests, waypoints. REC ids for new provisional
points: 800–814.

## Done

- **Item 23 — `spawn-town.poke` on 1.14d, Windows** (REC-590).
  `py tools\trace-recorder\poke.py --poke-file traces\pokes\spawn-town.poke
  --auto ScnAma --seed 1234 --seconds 90 --after 125 --input "waitticks 40;
  shot spawn-town-t40; waitticks 60; shot spawn-town-t100" --shots <dir>`.
  F0 = 2 (client 0 in state 4); results `ok` ×5: spawn 19 normal GUID 8,
  seed-unit, object 39 GUID 18, time 2 0, seed-game. The screenshots show
  the fallen party of 4 next to the player, the lit brazier 3 sub-tiles
  left of the player (screen −96 px, same row), Warriv walking, night
  light. Written into `specs/tools/poke.md` Status.
  Screenshots are game pixels, so they are kept locally, outside the
  public repo (CLAUDE.md rule 1): `C:\Users\pc\Documents\Claude code
  folder\shots\pc1-day3-a\` with the poke recording:
  - `spawn-town-t40.png` sha256 `43864c55855271954017b283300b25332dfd696ef37dd00b29d8bda29412376f`
  - `spawn-town-t100.png` sha256 `0a4656e9a065412e5027c0690de2e9f936685c09c864f15f2333c0a79c361cf5`
  - `spawn-town-poke.jsonl` (140 records)
