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
- **REC-660 — client GUIDs 2–92** (runtime count, settled). New tool
  `tools/trace-recorder/record_client_creations.py` (hooks the creator
  `0x00466730` and the critter AI after `0x004AE110`); trace
  `traces/client/a1-arrival-creations-ama.jsonl` (ScnAma, seed 1234,
  40 s; two runs gave the same 220 records). The counter does not move
  in the front end; the arrival's room pass at server frame 2 makes 205
  units, per room critters before presets: chickens 2–7, river presets
  8–89, chickens 90–95, presets 96–121, chickens 122–124, presets
  125–206 (objects 40/41/42 River1–3, 65 invisible river sound1). So
  2–92 = 9 chickens + 82 river presets; no client missile. Written
  into `specs/client/model.md` §5 r6.3. d2rs differs (counter start,
  no preset pass yet): row `q-fix-p6-client-arrival-guids`.
- **REC-661 — critter think-timer start** (settled). Every critter reads
  T = 0 at its first AI call, in the client update that created it
  (same trace, `think0` records). `specs/client/model.md` §5 r6.4.
  d2rs: the timer's default 0 matches; the AI itself is
  `q-fix-real-critter-ai`.

## Round 2 (2026-10-09; no 1.14d runs in this session, PC1-C runs them)

- **Item 1 — C→S 0x67 trailing bytes.** The builder `0x00477CA0` keeps
  the message in an uncleared 0x30-byte stack local; `0x004135D0` copies
  each name through its NUL only. Bytes 2–16 (after the empty game name)
  and the bytes after the character name's NUL through 36 are what
  earlier callees of `0x0044F360` left there. Three Windows recordings
  show a callback address `0x0044C520`, a stack address `0x0019F9F0`, the
  constant `0x0000020C` and a pointer that differs every run, so the
  bytes are process memory, not state. The server reads both names as
  cstr16 and ignores the rest. Answer in `specs/client/model.md` §7 r9
  "Unwritten bytes". New C→S mask table
  `specs/tools/scenario-masks-c2s.tsv` (`tools/scenario.md` §6 rule 4,
  `packets-trace.md` rule 5). d2rs's zeros stay; the comparator has to
  learn the c2s masks: row `q-fix-tool-c2s-masks`.
