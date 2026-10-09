# PC 1 data loop: upload the game, extract it, measure rendering

Started 2026-10-08 (coordinator session_01QeN5r8PoLZsUhwAH2iDzLJ), after
the user's decision on CLAUDE.md rule 1: the private repository
`MoggerCat/D2RUST-private-repo` holds the 1.14d install and its extracted
data so cloud sessions have the whole game; the public repository holds
our measurements (`facts/`, `traces/`). This loop **replaces**
`pc1-autotest.md` and `pc1-loop.md` while it runs: stop any session
running those.

Effort: speed first, no token limit; stay within PC 1's hardware (one
`Game.exe` at a time, one cargo build at a time, keep 15 GB disk free).
Subagents (up to 6) for analysis and writing only; the main session runs
the game and builds.

## Step 1 — upload the install (first, the cloud is waiting on it)

1. Clone the private repo next to the project:
   `git clone https://github.com/MoggerCat/D2RUST-private-repo ..\D2RUST-private-repo`.
2. From it: `py tools\split.py "<the 1.14d install folder>" --out install`
   (the folder with `Game.exe` and the MPQs; the same one `game/` is).
   Do not include saves, screenshots, or anything outside that folder.
3. Commit and push in batches **under 2 GB per push** (GitHub's limit):
   e.g. the small files first, then one or two MPQs per commit and push.
4. Check: in a scratch folder, `py tools\assemble.py --src install <scratch>`
   prints `0 mismatches`; then delete the scratch folder.
5. Push a marker commit `install: complete` and write in
   `docs/handoff/pc1-data-log.md` (public repo, branch below) the file
   count and the total size. The cloud coordinator starts the Wine test
   when it sees that commit.

## Step 2 — extract the data (private repo, `extracted/`)

With the project's own tools (release build once:
`cargo build --release -p mpq-tool -p data-tool`):

- Every MPQ: `mpq-tool extract <mpq> "*" extracted\<mpq name>\` (archive
  paths kept). Files over 95 MiB: split with `tools\split.py` rules or
  leave them out (the install already has them).
- `extracted/README.md`: per folder, the exact command, the tool name and
  version (`git rev-parse HEAD` of the project), and the file count.
- Push in batches under 2 GB. Marker commit `extracted: complete`.

## Step 3 — rendering facts (public repo, branch `claude/local-pc1-facts`)

Create the branch from `origin/claude/specs-staging-7`. Facts go in
`facts/render/`. Every fact file starts with a header line
`# facts v1; tool: <name version>; command: <exact command>; game: 1.14d`
and is plain text (TSV / JSON). Rule 1: measurements and digests only;
frame PNGs and raw captures stay local or go to the private repo
(`captures/`), never to the public repo.

**Format: `specs/tools/facts-render.md` §1–§4 (exact columns, units,
`-` / `?` markers, row order, header line). Write exactly that format:
the cloud's `d2-client facts-compare` reads it and refuses anything else
(exit 3).** Check a converted scene with
`cargo run -p d2-client -- facts-compare facts/render/scenes/<scene> facts/render/scenes/<scene>`
(must print MATCH or PARTIAL, never an error).

Tool: `tools/trace-recorder/record_frames.py` (it already reads the 8-bit
framebuffer, the palette and, with `--draws-every N`, every draw call),
under `autostart.py --auto CHAR --seed N [--input SCRIPT]`. Add a small
converter `tools/trace-recorder/facts_render.py` (a tool, not `crates/`)
that turns one capture into (summary; the spec is the format):

- `facts/render/scenes/<scene>/draws.tsv`: every draw call of the chosen
  frame in order (kind, cell file / frame / direction, screen x, y, size,
  offsets, palette / blend / transform, light);
- `facts/render/scenes/<scene>/frame.tsv`: frame number, server tick,
  camera, player position, level, light quality, weather, and the sha256
  of the index framebuffer and of the palette;
- `facts/render/sprites.tsv`: per distinct (file, direction, frame) seen
  in any draw list: width, height, x / y offset as 1.14d decoded them
  (merged over all scenes, sorted, deduplicated).

Scenes, in this order (fixed characters from `d2s-tool new`, fixed seeds,
fixed camera; one frame each after the scene is stable):

1. Rogue Encampment, idle, by the waypoint (each class once: the player
   sprite and its composite layers).
2. The player walking and running in each of the 8 directions (one class).
3. Blood Moor with monsters on screen; a corpse; a gold and item drop.
4. Panels: inventory, character, skill tree, belt open, stash, cube, the
   Esc menu, NPC dialog and a shop, the automap overlay.
5. Front end: main menu, character select, character create, the loading
   screen.
6. One town per act (Lut Gholein, Kurast Docks, Pandemonium Fortress,
   Harrogath), the Den of Evil (dungeon lighting), Blood Moor at night,
   rain in Act I.

After each scene: push. Then process `facts/requests.tsv` (format:
`facts-render.md` §7; cloud sessions add rows there for cases the existing
facts do not answer: scene, what to capture, who asked); run only those,
newest-first, and set each row's status to `done <commit>`.

## Hand-back

- Public repo: merge (never rebase) `origin/claude/specs-staging-7` before
  each push; `py tools/spec_index.py --check`, `py tools/coverage.py
  --check`, `py tools/methods.py check` must pass. No code in `crates/`.
- Log every step in `docs/handoff/pc1-data-log.md` (newest first): what
  ran, counts, sizes, failures. A failure is written down, never hidden.
- REC ids for any provisional note: REC-300..REC-349.
- The coordinator polls `claude/local-pc1-facts` and the private repo's
  `main`; no message is needed.

## Set up any state for a check (q-tool-poke, 2026-10-09)

Never walk through the game to reach a state. Three layers, the same on
1.14d and d2rs, so a comparison starts from identical state:

| Need | Tool | Spec |
|---|---|---|
| the character (class, level, stats, skills, waypoints, quests, items) | `d2s-tool new` / `set` → a `.d2s` | `formats/d2s.md` |
| units and world state at a tick (monster, superunique, object, missile, game / unit seed, time of day; d2rs also pos, warp, item, stat, state) | a poke: `traces/pokes/<name>.poke`, `at <t> poke ...` scenario steps, or `--poke "<frame> <directive> ..."` | `tools/poke.md` |
| table data (a level with one monster class, a fixed damage range, ...) | a test variant: `traces/variants/<name>/<name>.d2stack` → `data-tool variant build` | `tools/test-variants.md` |

```
# 1.14d (Windows; in the cloud: tools/cloud-game/run.sh --python --seconds 150 -- <the same command>)
py tools/trace-recorder/poke.py --poke-file traces/pokes/spawn-town.poke --auto ScnAma --seed 1234 --seconds 60
py tools/trace-recorder/poke.py --poke "4 spawn 19 4876 4231 normal" --auto ScnAma --seed 1234
#   other recorders: import poke; poke.add_options(ap); poke.PokeLayer.from_args(a).attach(rec)
# d2rs
cargo run -p scenario-run -- run traces/scenarios/poke-spawn-town.scenario --game-dir %D2_GAME_DIR%
cargo run -p d2-client -- play --save ScnAma.d2s --seed 1234 --poke-file traces/pokes/spawn-town.poke
# test variant (output outside the repo; never commit it)
cargo run --release -p data-tool -- variant build traces/variants/only-fallen/only-fallen.d2stack --out ..\variants\only-fallen
py tools/trace-recorder/record_tick.py --game ..\variants\only-fallen\Game.exe --auto ScnAma --seed 1234
```

Rules: pokes run between two server ticks (after frame t, before the next
drain) and draw no RNG of their own; a directive with no spec'd 1.14d
call form reports `gap` on 1.14d (item 22 below lists them) and the
comparison is `partial`, never silently skipped. A variant is a patch
stack (rule 9): commit the `.d2stack` / `.d2patch`, never the built
install.

## Step 4 — binary reads and one recording only PC 1 can do (queued by the coordinator, 2026-10-08)

Cloud sessions now record 1.14d themselves under Wine (REC-290 RNG half
equal to PC 1's traces; `tools/cloud-game/`), so PC 1 keeps only what
needs `re/` or a real Windows run. Each answer goes into its owner spec
(prose / authored pseudocode, addresses); a code disagreement becomes a
`q-fix-*` row.

1. **Vitals dx/dy sign (top suspect for the remaining rubber-banding).**
   S→C 0x18 / 0x95 / 0x96: the server side `0x00548760` vs the client
   side `0x0045DC50` / `0x0045DB20`. `combat/vitals.md` §5.2/§5.4 and
   `client/msg-units.md` §5 r3 contradict each other; the ignored test
   of row `q-fix-proto-vitals-dx-sign` / `q-fix-seam-vitals-delta` is
   ready.
2. **Esc menu pause in single player** (`client/bridge.md` §8 r5; rows
   `q-fix-ui-pause`, `q-fix-seam-pause`): does 1.14d stop the game loop
   under the Esc menu, and from which call.
3. **Hireling target search range**: 20 sub-tiles (REC-100) vs 35
   (REC-279).
4. **x87 precision at start-up (REC-21)**: the C runtime start-up's
   control word; settles five provisional points.
5. **REC-290 tick half**: `record_tick.py --auto ScnAma --seed 1234
   --ticks 600` on PC 1, committed as a trace, so the cloud can compare
   its Wine run (equal except ms between two Wine runs).
6. UI spec gaps from `docs/handoff/q-ui-audit.md`: menu-box window
   handlers 0x0E/1, drop cell `0x00486BD0`, gamble flag (panels-2 §14
   r11 vs menus §4.2), waypoint level names.
7. **Callers of three S→C senders (q-fix-proto-rest, REC-415)**: the static
   callers of `0x0053C1D0` (0x20 StatUpdate), `0x0053C6F0` (0x93 skill
   bonus by element and page) and `0x0053E1C0` (0xA6; the spec found none)
   and `0x0053B3D0`'s 0x92 call site: when each runs and with which
   values. The d2-sim builders and the client handlers exist and are
   contract-tested; only the call sites are missing. Answer into
   `client/msg-stats-items.md` §1 r4 / §5 r5 / r7 and `client/msg-skills.md`
   §9.
8. **Argument form of `0x00554200(unit)` at the 0xAB case of
   `0x00571CD0`** (REC-412) and the base-or-total read of stat 178 in
   `0x00625A50` (REC-410): `sim/intents-events.md` §7.9 r2 and §7.3 r2
   step 9.
9. **Field sources of S→C 0x73 in `0x0059FEE0`** (REC-414): which missile
   fields fill the two u32 positions, the first path point and the level
   byte; `missiles/missiles.md` R2.4.
10. **Node order and asserts of the client 0x92 handler `0x004C23E0`**
    (REC-416): the order of the inventory nodes it walks and what `0x0063E0B0`
    does at its end; `client/msg-stats-items.md` §5 r5.
11. **Command 1 slot order in the controls table** (`q-fix-real-controls-default-order`):
    Game.exe at 0x312220 holds (cmd 1, 'B', slot 0) before (cmd 1, 'I', slot 1);
    `specs/ui/key-commands.tsv` with the §3.4 / §B4 r1 rule gives them the other
    way round. Spec decision: a slot-order column in the TSV or an exception
    for command 1, and whether first-match lookup makes the order matter; then
    `BindingTable::defaults` and the test builder change together.
12. **Format-0 property wrapper `0x0065FE10` (REC-289 (5),
    q-fix-items-play)**: (a) the sixth argument of `0x0065FEC0` (the
    apply type, `items/properties.md` §2) at each static caller other
    than the affix roller (`0x005C12F0`), §11 and §12: which value each
    passes (d2rs passes 0); (b) `0x0065DD80`: is the `param` > 3 test
    signed, and is a negative `param` clamped or used as is; (c) does
    the craft list (`0x00660240`, mode 7) call `0x0065FE10` or
    `0x0065FD70` directly (§2 says directly, §14 lists §12 among the
    wrapper's callers). Answer into `items/properties.md` §2 / §14.
13. **State param sign in S→C 0xA8 / 0xAA** (`q-fix-proto-state-param-sign`):
    the sim writes the param unsigned (`units/messages.rs`), the client
    reads it signed (`client/stat-lists.md` §3 r1); settle the read sign
    with `0x0045EE20` / `0x00470E30` (`stat-lists.md` OQ5).
14. **Stamina scale on the client** (`q-fix-seam-stamina-scale`): the wire
    carries stamina >> 8, the client's exhaustion test reads raw 1..255;
    does 1.14d's client drain stamina locally (`client/model.md` OQ2,
    REC-51)? Spec decision first, then `bridge/predict.rs`.
15. **The other four progression call sites** (`q-fix-save-gaps`, REC-265
    (2)): `0x00538680(client, step, difficulty)` at `0x0058DCE2`,
    `0x0058DD65`, `0x0058E4F1` (Act II) and `0x0059C848` (Act III): which
    quest event and which `step` each passes. Act I (`0x00596210`), Act IV
    (`0x005B4D77`, step 4, classic only) and Act III's Mephisto credit
    (`0x005BC182`) are wired.
16. **Loader messages 0x22 / 0x21 at the join** (`q-fix-flow-join-load`):
    the per-item conditions of the loader's S→C 0x22 (`0x0055C216`) and
    0x21 (`0x0057017B`) (`intents-events.md` §8.2 r3.1 (c)); the quest
    entry itself is wired (`world/quests.md` §3 names the caller).
17. **Think restart gate `0x00553160`** (REC-442, q-cloud-game): in
    SUNIT_Add's monster branch (`init.md` §4.1), when does 1.14d restart
    the think at f + 2? d2rs always restarts it. Answer into `init.md`
    §4.1 / `ai.md`.
18. **Missile damage setup `0x0059F900` → `0x0064B860`**
    (`q-fix-real-missile-damage`, from q-fix-real-skills): how a missile's
    damage record is filled from its owner and skill; no spec exists.
    Write `missiles/damage.md` (or a section of `missiles/missiles.md`).
19. **Arcane Sanctuary star tick** (`draw-order-2.md` OQ 12, HANDOFF §5
    entry 103; REC-420): the initial `last` of §12 r3 and the time / seed
    argument form.
20. **Windows recordings Wine cannot make** (`facts/requests.tsv`, open
    rows from q-facts-scenes): stash panel; NPC dialog, shop and gamble
    screens (clicks do not open them under Wine); the 4 front-end scenes;
    Blood Moor monsters at night; Den of Evil. Record with
    `tools/trace-recorder/record_frames.py` and `facts_render.py` as in
    Step 3, each twice, and commit the facts.
21. **Owner of a unit, for the state snapshot** (`specs/tools/state-snapshot.md`
    §2 `own`, OQ 1; q-tool-state-diff): where 1.14d keeps the owner GUID
    of a pet / summon / hireling, a missile and an item (its container or
    holder), as offsets read from a server unit, so `record_state.py` can
    fill `own`. Write the answer into `sim/units.md` (or the owner spec)
    and the §2 row of `state-snapshot.md`.

## How to check a behaviour in one command

A check file `traces/checks/<name>.check` (`specs/tools/scenario-diff.md`)
names the save, seed, ticks and input. One command builds the save with
`d2s-tool`, runs 1.14d with the recorders and d2rs with the same save and
seed, and prints the first difference per channel (game state first):

```
py tools/scenario-diff/scenario_diff.py traces/checks/a1-town-arrival-ama.check
py tools/scenario-diff/scenario_diff.py <check> --orig-only        # only record 1.14d
py tools/scenario-diff/scenario_diff.py <check> --reuse            # re-compare what is in the work dir
```

On PC 1 the recorders run with `py` directly (no Wine); the 1.14d save is
copied to `%USERPROFILE%\Saved Games\Diablo II` (`D2_SAVE_DIR` overrides).
The state channel's report starts with `FIRST DIVERGENCE: frame N <unit>
field K: 1.14d A vs d2rs B`, then the next 20, then the first frame per
field; read the state report before any draw list. Two snapshot files
compare alone with `py tools/trace-recorder/state_diff.py ORIG D2RS`.
For a new behaviour, add a `.check` file (copy `a1-town-arrival-ama.check`)
rather than a hand-run recipe.
22. **Poke call forms** (q-tool-poke, `specs/tools/poke.md` Open
    questions 1–4; until answered these directives are gaps on 1.14d):
    register / stack form and `ret` of (a) `0x00554EA0(game, unit, room,
    x, y, exact, alt)` or the teleport path `0x00650BE0` (`pos`,
    `path-placement.md` §10 / §6 r4); (b) the level warp
    `0x0053AEC0(game, player, level, tile)` (`warp`, `waypoints.md` §7
    r5); (c) a ground-item entry at a point: `0x00558D90(game, request,
    use seed)` with request +0x18 = 3, +0x1C/+0x20 x, y, +0x24 room, and
    the code → index lookup `0x00633640` (`item`, `items/generation.md`
    §3); (d) `0x00627260(unit, s, value, layer)` and `0x00639DB0(unit,
    s, on)` (`stat`, `state`, `stat-lists.md` §5 r2, §9.2). Answer into
    the owning specs; then delete the gap rows in `poke.md` §1. (e) For
    scenario `spawn` kinds `champion` / `random-boss` on 1.14d: the
    register form of the champion / boss minions call `0x0054E1E0`
    (`scenario.md` §3.1, `population.md` §6.4); `poke.py` writes them as
    gaps until then (`normal` runs).
23. **Poke runs on Windows** (REC-590): the cloud ran every runnable
    directive on 1.14d under Wine (`specs/tools/poke.md` Status) and
    settled the variant load (REC-591, `tools/test-variants.md` Status).
    Left: run `traces/pokes/spawn-town.poke` once on PC 1 with a
    screenshot (Wine screenshots are blank). Commands in "Set up any
    state for a check" below.

24. **Client missile motion and body gaps** (`q-fix-client-missiles-rest`,
    REC-540–549): `missiles/client.md` Open questions 9 onward: the
    timed arc `0x004DA5B0` (flag store vs set, the vz division), the
    motion getters `0x004DA110`–`0x004DA150` (shifted or stored) and the
    other points that session lists there. Answer into
    `render/unit-composite.md` §8 and the client missile specs.

## Step 5 — spec gaps (107 provisional points no spec states)

`docs/handoff/provisional-index.tsv` rows with `settle_kind` = `unstated`
are behaviours d2rs guessed because no spec covers them. Work them with
`re/` per `specs/README.md`, highest impact first (crashes and wrong game
outcomes before cosmetics), and write the answer into the owner spec
(prose / authored pseudocode, addresses). Each answered point either
confirms d2rs (mark the REC settled) or becomes a `q-fix-*` row. Re-run
`py tools/provisional_index.py` after each batch.

## Hand-back — PC 1 day run 2026-10-09 (branch `claude/local-pc1-day`)

**Done**
- Step 4 binary reads, all of items 1–4 and 6–19, into their owner specs
  with addresses. Headlines:
  - 1: the vitals dx/dy "sign" is 1.14d's mirror rule; d2rs already
    matches, so rubber-banding is not from here. Next suspect:
    `q-fix-seam-check-own-position`.
  - 17: the think restart is gated on the room's client count; d2rs
    always restarts.
  - 18: new spec `missiles/damage.md`; d2rs has no missile damage setup,
    so every missile does 0 damage.
  - 3: hireling search is 49 scan / 25 engage, not 20 or 35.
  - 4: the C runtime sets x87 precision PC = 53; the globe smoothing
    differs.
  - 2: Esc pauses the client loop in single player (`0x0044EFA0`).
  - 19: the Arcane star `last` starts at 0; star and cloud seeds settled.
  - 7–10, 12, 15, 16: S→C senders, 0xAB, 0x73, 0x92, the format-0
    wrapper, progression sites, join loader.
  - 11 needed nothing new; 6 covers the UI gaps (gamble flag is dead
    code, drop cell, waypoint names, menu box).
- Step 4 item 5: REC-290 tick half recorded on Windows, trace
  `traces/sim/tick/sim-0009.json` (`record_tick.py --auto ScnAma --seed
  1234 --ticks 600`; `convert_tick.py --check`, 0 errors).
- Step 5: about 85 of the 107 `unstated` points read and written into
  their owner specs. Many confirm d2rs; the settled lists are in the
  HANDOFF §7 REC rows (REC-21, 100, 111, 115, 265, 289, 400, 401, 406,
  410, 412, 414, 415, 416, 420, 442, 460). The code comments in `crates/`
  still say PROVISIONAL (spec sessions do not edit `crates/`), so
  `provisional_index.py` still counts them.
- HANDOFF §5 local run queue:
  - q-fix-cold-plains: `outdoor.md` / `outdoor-tilesub.md` seq labels
    were one too high.
  - q-fix-ui-npc-talk facts: `tools/facts/npc_talk.py` →
    `facts/ui/npc-talk-*.tsv`; class 210 is not a no-intro NPC.
  - q-fix-flow-save `real_saves`: 2 passed on game-written saves.

**Left**
- Step 4 item 20, recorded later the same day with the user at the game
  (recorder running with no `--input`: `record_frames.py --every 5
  --draws-every 1 --auto SceSor --seed 1234`, one frame per scene picked
  by `facts_render.py --frame N`). The new scenes in `facts/render/scenes/`
  are a1-npc-intro-akara, a1-npc-dialog, a1-npc-shop,
  a1-npc-shop-tooltip, a1-npc-shop-gheed, a1-npc-gamble,
  a1-npc-gamble-tooltip, a1-panel-stash, a1-panel-esc-menu,
  a1-blood-moor-monsters (live monsters, 4 corpses, 1 ground item),
  a1-blood-moor-automap and a1-den-of-evil. Each was recorded once, not
  twice: the scenes are user-driven and so not repeatable frame for frame.
  The open rows in q-facts-scenes' `facts/requests.tsv` are now answered
  except Blood Moor at night and the 4 front-end scenes; set them done
  when that branch merges.
- Old note: scripted
  `goto` to NPCs is unreliable on PC 1 (Akara wanders; one run captured
  no menu). The user will drive these by hand with the recorder running
  (no `--input`). `facts/requests.tsv` rows are still open.
- Step 5 rows not read:
  - item tooltip totals (REC-242);
  - char damage block (REC-269);
  - DCC frames > 256 (needs the gargoyle-trap capture);
  - wall light for direction 0 / > 9;
  - REC-02 hand items;
  - automap `.ma` hex dump;
  - the tooling-choice rows (REC-295 / 296, native-assets).
- New provisional points: REC-602 (`0x00463260` branch at stamina
  exhaustion; needs a recorded run), REC-610 (video DLL precision under
  `-d3d`), REC-643 (dialog reader without a NUL test), REC-644 (recipe
  scroll ACP bytes ≥ 0x80).
- The rest of the HANDOFF §5 queue needs the player at the game
  (front-end screenshots, idle rooms, `play` checks).
- REC ids used today: REC-600..654 were taken as PC 1's day block; only
  602, 610, 643 and 644 are used.

**New q-fix rows (build-queue.tsv; 59 ids, first seven by impact)**
- `q-fix-missile-damage-setup`
- `q-fix-think-restart-gate`
- `q-fix-monster-hit-reaction`
- `q-fix-player-hit-reaction`
- `q-fix-monster-attack-event0`
- `q-fix-client-path-compute`
- `q-fix-hireling-range`

Then the rest:
- **Protocol:** `q-fix-join-multiclient`, `q-fix-join-load-item-msgs`,
  `q-fix-proto-0x20-sender`, `q-fix-proto-0x73-fields`,
  `q-fix-proto-0x92-all-items`, `q-fix-proto-0xab-gate`,
  `q-fix-player-mode-rows`, `q-fix-game-id-counter`.
- **Items / quests:** `q-fix-load-taken-cell`, `q-fix-quest-reward-no-spot`,
  `q-fix-quest-item-delete-modes`, `q-fix-code-drop-20-7`,
  `q-fix-progression-sites`, `q-fix-trade-lock-test`,
  `q-fix-legacy-bytime-param`.
- **Hirelings / pets:** `q-fix-mercitem-room-order`,
  `q-fix-merc-swap-update-list`, `q-fix-pet-resync`, `q-fix-pet-palette`,
  `q-fix-dead-body-path-settings`.
- **Client:** `q-fix-click-seq-mode`, `q-fix-screen-to-world-y`
  (= `q-fix-click-no-minus-8`), `q-fix-preview-use-state`,
  `q-fix-preview-class-skill`, `q-fix-ai-plain-attack-skill`,
  `q-fix-monster-mode-machine`, `q-fix-range-statemask`,
  `q-fix-object-generic-step`, `q-fix-hire-list-stats`,
  `q-fix-anim-key-weapon-class`.
- **UI:** `q-fix-ui-globe-x87`, `q-fix-ui-drop-cell`,
  `q-fix-shop-gamble-flag-dead`, `q-fix-shop-repair-all-refuse`,
  `q-fix-socket-left-down`, `q-fix-automap-ma-link-first`,
  `q-fix-automap-fade3-mini`, `q-fix-automap-name-colours`,
  `q-fix-quest-tab-open-order`, `q-fix-quest-icon-anim-order`,
  `q-fix-dialog-pass-abort`, `q-fix-dialog-text-100-lf`,
  `q-fix-npc-greeting-open`, `q-fix-gossip-725cb0`,
  `q-fix-minipanel-last-drawn-layout`, `q-fix-chat-filter-utf8`,
  `q-fix-ui-npc-talk-facts`, `q-fix-render-bg-seed`.
- **Closures (no code change, tests only):** `q-fix-proto-vitals-dx-sign`,
  `q-fix-proto-state-param-sign`, `q-fix-seam-stamina-scale`.
- **[q-fix-pc1-client-ui] door step 0x004BCB20** Specify the object door step (REC-725): what it does per update for an `IsDoor` non-cycling mode (frame advance, mode change, collision / sound calls), for `world/objects-client.md` §25 r9.2.1.
- **[q-fix-pc1-client-ui] NPC introduction handler 0x004B41E0** Read which text record the "introduction" topic plays (REC-727; d2rs plays record 0 of the intro entry) and whether it sets +0x11 or the talk flag like "gossip" (`messages.md` §6 r3 / OQ4).
