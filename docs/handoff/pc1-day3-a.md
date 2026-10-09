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
- **Item 2 — town NPC path choice.** Re-read the map AI `0x005E7080`, the
  roll `0x0045C3E0` and the walk handler `0x005E6DE0`; the rule in
  `specs/monsters/ai-bodies.md` §9.9 was already exact. A summary is now
  written there ("Path node choice"): no order, no wrap or reverse;
  per think `lo' % 100` < 66, then i = `roll(count)` (count 1 still
  draws); the walk target is the node itself; command 4 (12 tries, idle
  10, walks again past 3) holds the NPC before the next pick; the
  interaction step runs first. d2rs's `npc_map_ai` and `roll` follow it,
  so the Warriv difference at r110 of the Act I scenes comes from the
  seed state or think timing, or from the interaction gate (item 3).
  No q-fix row for the node choice. Which cause it is: live run 1 below
  (`traces/checks/rng-town-idle-sor.check`).
- **Item 3 — quest active-test seam** (the interact gate of
  `q-fix-p3-npc-nearest-player`). Read `0x00544590`, its caller
  `0x005DDE80` and `0x005DDF20`. Written into `specs/world/quests.md`
  §6.4 "The active test as a seam":
  - inputs: game, player, NPC; output: a bool, plus the 0x8A send;
  - the `interact` precheck (false, no walk);
  - the act comes from the player's client;
  - records are walked newest → oldest and filtered by act;
  - each active fn gets (record, NPC class, player, the player's
    quest-flag record of the game difficulty, NPC);
  - the first true sends `8A 01 GUID` and stops;
  - the fatal asserts are listed.
  The scan callback calls the test before `d < best` and stops at the
  first taker, so 0x8A goes out at the think rate. A scan of the 39
  active fns found no RNG and no stores. d2rs differs: the test returns
  nothing, walks the records oldest first, and the AI never calls it.
  Row `q-fix-p3-npc-interact-gate`.
