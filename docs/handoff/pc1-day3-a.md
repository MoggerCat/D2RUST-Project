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
- **Item 4 — act quest flags for the playthrough harness.** New
  `specs/world/quests.md` §1.9 "Checkpoints per act". It has one table
  of the 25 asked quests: slot, and what sets bit 0 with its
  precondition bit. A second table lists the act gates, with what each
  needs and what each sets:
  - I → II: warriv1, needs 6.0, sets 7.0;
  - II → III: meshif1, needs 14.0, sets 15.0;
  - III → IV: the Hellgate opened by Mephisto's death; its Durance warp
    sets 23.0;
  - IV → V: tyrael2, expansion and 26.0, sets 28.0;
  - Baal: 40.0.
  "Done" = bit 0 (§1.8). The table is collected from the act files: the
  Acts II–V bit-0 writers were extracted by an Explore agent and
  spot-checked, and I removed the goal-bit cells I couldn't verify. The
  harness can't read quest flags yet: row `q-tool-playthrough-quests`
  adds the state field and the `quest <slot> <bit>` predicate.

## Live runs for PC1-C

This session doesn't run 1.14d. Each run below is one command.

1. **Warriv's node pick at the Act I arrival** (item 2; `ai-bodies.md`
   §9.9 "Path node choice", `quests.md` §6.4):
   ```
   py tools\scenario-diff\scenario_diff.py traces\checks\rng-town-idle-sor.check
   ```
   Look for:
   - (a) in the rng report, the first divergence owned by `unit 1:7`
     (Warriv): its frame, and whether the 1.14d site is the map AI
     (`0x005E7080` seed step / `0x0045C3E0` roll) or something earlier
     (a different seed value at the same site means the seed state;
     the same value in another frame means the think timing);
   - (b) in the packets channel, whether 1.14d sends `8a 01 07000000`
     (the active test passed for Warriv) and from which frame, and
     whether d2rs does.
   If 1.14d sends no 0x8A but walks to a node while d2rs walks in radius
   toward the player, the cause is the interact gate
   (`q-fix-p3-npc-interact-gate`). If both send it, the cause is the seed
   state (`q-fix-real-unit-seed-order`).
2. **C→S 0x67 mask** (item 1), after `q-fix-tool-c2s-masks` lands:
   ```
   py tools\scenario-diff\scenario_diff.py traces\checks\packets-town-arrival-ama.check
   ```
   Look for: the c2s stream no longer diverges at C→S 0x67 (frame 1).
   The report's "masked bytes skipped" count includes 0x67's bytes 2–16
   and the character-name tail.

## Item 5 (round 2)

No new Step 4 items on staging with the tags `[seed-order]`,
`[recording-2]` or `[proto-items]`, and none about towns, NPCs, quests
or waypoints, as of the last pull (2026-10-09). Staging is polled every
30 minutes.

## Round 3 (2026-10-09; 1.14d runs under the lock)

- **Item 1 — act and game seed for a save outside Act I.** Written into
  `specs/sim/intents-events.md` §8.2 rule 8.
  - Act: the save header read `0x0056A090` writes client +0x1AC from
    town byte +0xA8 + difficulty (& 0x7F, ≥ 5 → 0), the last write
    before the join builds the act (`0x0052C210`) and enters the town.
  - Game seed: act creation draws nothing from it. Before the first
    town unit there are only the four creation derivations and the
    player's unit seed (plus two steps per loaded item); then frame 2's
    room population in the client act's town.
  - Two 1.14d recordings under the lock confirm it (`record_rng.py
    --frames --ticks 4 --auto SceAct2|ScnAma --seed 1234`): SceAct2
    enters level 40; both runs have the same five draws, and the first
    frame-2 unit draws 108806926 in both; Act I has 24 frame-2 unit
    draws, Act II 12.
  - Trap: with `-seed N` the DRLG seed and the game seed start equal,
    so match draws by owner, not by value.
  - d2rs has the act rule (`d2-server` `adapters/character.rs`), so the
    act-0 start is a wiring gap; q-prov-data implements it (no new row
    from here).
- **Item 2 — what keeps a dead monster dead.** Written into
  `specs/sim/units.md` §4.6 "What keeps a dead monster dead", with a
  pointer in `monsters/ai.md` §1.1.
  - Sequence: request 0 → DT start (clean-up) → DT event 0 / 1 sets 12
    with the plain set → DD schedules nothing.
  - The think has no dead test. The DT start's clean-up cancels the
    unit's type-2 / 3 events (`0x005738D0`), and no scheduler runs for
    a dead unit afterwards.
  - The mode set `0x005A7C20` does not refuse a dead unit. A start that
    fails leaves the mode unchanged (dead → return 1); the GH start
    keeps mode 0/12; but the NU start sets mode 1 and an attack / skill
    start sets its mode.
  - d2rs's `monster_death_start` has no clean-up, so the think pending
    at death fires `aidel` (15) frames later, idles, requests neutral,
    and gets mode 1 with hp 0: exactly the symptom. Row
    `q-fix-p4-death-cleanup`.
- **Item 3 — right click: cast or walk.** `specs/ui/controls.md` §6
  rules 4–11 already held the full decision. The new rule 12 sums it
  up for the right button as a truth table: skill = the right skill,
  rng = `range(P, skill)`, U after the re-pick, hostility.
  - Ground with rng ≠ 1 → use check → C→S 0x0C. A failed check sends
    nothing; Attack with no mana falls through to a walk.
  - Ground with h2h → walk.
  - Hostile monster with rng 0 / 2 → 0x0D. With rng 1 / 4 → 0x0D in
    melee range, else walk to the unit.
  - Stand Still forces 0x0C.
  - A dead monster, or an object without `TargetItem`, counts as ground.
  `d2-client` `controls/click.rs` follows this order, so a build where
  every right click walks has rng resolving to 1 (range lookup / `both`
  resolution) or a right skill of Attack (id 0). In `Patch_D2` Fire
  Bolt and Frost Nova are range `none`, so on the ground they must send
  0x0C. No q-fix row: the skills-2 session is bisecting the regression,
  and this rule is the check for its fix.
- **Item 4 — my live runs** (under the lock, Windows, SceSor /
  `-seed 1234`).
  1. *Warriv's node pick.* 1.14d (`record_rng.py --frames --ticks 120`,
     `record_packets.py --ticks 120`, `record_state.py --ticks 50`):
     - Warriv takes no node pick: one unit-seed draw only (frame 2,
       `0x00573F8F` `roll(1)`).
     - 0x8A `8a 01 07000000` at every think (24, 45, 56, 67, 87, 107):
       the interact gate passes for this save.
     - The walks are step 7's walk in radius: to (4868, 4233) at 24,
       (4869, 4232) at 45, (4870, 4231) at 56, stop at 67.
     
     d2rs: `d2-client state-dump` (the release binary built 2026-10-09
     16:26, no rebuild) equals 1.14d for Warriv on every frame 22–47
     (mode, position, fraction, target) and has the same walks to 120.
     So the scene note (tick 42) is stale; there is no Warriv
     difference in the current build. Written into `ai-bodies.md` §9.9
     (replacing my round-2 guess). `q-fix-p3-npc-interact-gate` is
     still right for saves where the test fails, but it doesn't change
     this arrival.
  2. *C→S 0x67 mask.* Not run: `q-fix-tool-c2s-masks` hasn't landed on
     staging (`packets_diff.py` still masks only s2c), so the check
     would show the known 0x67 divergence. Run 2 of "Live runs for
     PC1-C" stays queued for after that row.
- **Item 5 (round 3).** No Step 4 items with `[seed-order]`,
  `[recording-2]`, `[proto-items]`, `[q-fix-pc1-day3-a-r2]` or
  `[prov-data]` on staging yet; polling every 30 minutes.

## Later (2026-10-09 evening)

- **[prov-data] Monster think in a room with no clients.** The gate is
  not in the think path: `0x005A7F80` and `0x005B1740` have no room
  test. It is the room leave `0x0053A9B0`: when the leaving client was
  the room's last (room +0x78 = 0), every monster in the room gets
  `0x005738D0`, which cancels its thinks (type 2) and type-3 events and
  schedules nothing. The next think comes only when a client joins the
  room again (`0x00573780`, f + 2). So after the frame-6 warp, the
  Fortress NPCs' frame-24 think is gone and their seeds stay put, as
  1.14d shows. Written into `specs/sim/intents-events.md` §7.8 rule 3.2.
  d2rs's `wiring/action/switch.rs` leaves the cancel out (its doc lists
  it as unspecified). Row `q-fix-p3-leave-cancels-thinks`.
- **Live runs, done** (Windows, release build of staging at 17:38, the
  recorders' own lock):
  - `packets-town-arrival-ama.check`: the c2s stream is now equal (28
    masked bytes of C→S 0x67 skipped), so `q-fix-tool-c2s-masks` works.
    New first divergence: frame 2, s2c #66, 1.14d 0xA8 (state 105
    `alignment`, player 1) where d2rs sends the next 0xAC. Read from the
    asm: the setter `0x005543B0(P, 2, 1)` runs in the player-unit init
    `0x005348C0` at the allocation, and its resend marks state 105
    changed until frame 2's sends. That settles REC-732 (call site and
    v = 2) in `combat/hit.md` §7.1. d2rs doesn't send it: row
    `q-fix-p5-alignment-resend`.
  - `a1-town-arrival-ama.check` (state, 40 frames): no difference in
    any compared field. PARTIAL only for d2rs's known gaps (`own`, `q`).
    Warriv still matches after `q-fix-p3-npc-interact-gate`.
