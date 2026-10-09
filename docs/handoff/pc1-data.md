# PC 1 data loop: upload the game, extract it, measure rendering

Started 2026-10-08 (coordinator session_01QeN5r8PoLZsUhwAH2iDzLJ), after
the user's decision on CLAUDE.md rule 1: the private repository
`MoggerCat/D2RUST-private-repo` holds the 1.14d install and its extracted
data so cloud sessions have the whole game; the public repository holds
our measurements (`facts/`, `traces/`). This loop **replaces**
`pc1-autotest.md` and `pc1-loop.md` while it runs: stop any session
running those.

**One `Game.exe` open at a time (user rule, 2026-10-09):** at most one
1.14d `Game.exe` process may be open on PC 1 at any moment. Any PC 1
session may run it (directly, through `poke.py`, `scenario_diff.py`'s
1.14d side or a recorder), but only when none is open: check first
(`tasklist | findstr /i game.exe`), hold the lock file
`%TEMP%\d2-game.lock` (create it before starting, delete it once
`Game.exe` has exited), and wait if the lock exists. Reading the binary
(Ghidra, `re/`) is not limited.

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

## Set up any state for a check (q-tool-poke, final 2026-10-09)

Never walk through the game to reach a state. Three layers, the same on
1.14d and d2rs, so a comparison starts from identical state:

| Need | Tool | Spec |
|---|---|---|
| the character (class, level, stats, skills, waypoints, quests, items) | `d2s-tool new` / `set` → a `.d2s` | `formats/d2s.md` |
| units and world state at a frame | a poke (directives below) | `tools/poke.md` |
| table data (a level with one monster class, a fixed damage range, ...) | a test variant: `traces/variants/<name>/<name>.d2stack` → `data-tool variant build` | `tools/test-variants.md` |
| the front-end screens | `record_frames.py --front-end --front-end-script "..."` | `render/capture.md` §2a |

Directives (`tools/poke.md` §1; refs `@player`, `@x±N`, `@y±N`,
`@<type>[:<class>][#n]`, `<type>/<guid>`):

| Directive | 1.14d (`poke.py`) | d2rs |
|---|---|---|
| `spawn <class> <x> <y> normal\|champion umod <u>\|random-boss\|unique umod <u>...` | runs (all four kinds checked on 1.14d) | runs |
| `superunique <row> <x> <y>` | runs | runs |
| `object <class> <x> <y> [mode <m>]` (the object init runs at the point: a chest operates and drops as the seed rolls) | runs | runs |
| `missile <class> <x> <y> <tx> <ty> [skill <id> <lvl>] [owner <ref>]` | runs | runs |
| `seed-game <lo> <hi>`, `seed-unit <ref> <lo> <hi>` | runs | runs |
| `time <period 0..5> <ticks>` | runs | runs |
| `freeze <seconds>` | holds the game | no-op |
| `pos <ref> <x> <y>` | **item 22 (a)**: fill `CALL_FORMS["teleport"]` or `["place"]` | runs |
| `warp <level> [tile <n>]` | **item 22 (b)**: `CALL_FORMS["warp"]` | runs (any act) |
| `item <code> <x> <y> [quality <q>] [ilvl <n>]` | **item 22 (c)**: `CALL_FORMS["item_create"]` | runs |
| `stat <ref> <stat> <layer> <value>`, `state <ref> <state> on\|off` | **item 22 (d)**: `CALL_FORMS["stat_set"]`, `["state_set"]` | runs |

A directive whose form is missing reports `gap`, and the comparison is
`partial`, never silently skipped. Filling a form (item 22): answer into
the owning spec, try it with `poke.py --forms FILE` (format
`poke-forms-1`, `tools/trace-recorder/README.md` "Call forms"), copy it
into `CALL_FORMS`, `py tools\trace-recorder\poke.py --selftest`, run once
on the game.

Commands (Windows, PC 1; in the cloud prefix each 1.14d command with
`tools/cloud-game/run.sh --python --seconds 150 --`):

```
:: 1.14d: a poke file (relative ticks) or absolute frames
py tools\trace-recorder\poke.py --poke-file traces\pokes\spawn-town.poke --auto ScnAma --seed 1234 --seconds 60
py tools\trace-recorder\poke.py --poke "4 spawn 19 @x+3 @y+3 normal" --poke "8 missile 58 @x @y @x+3 @y+3 skill 36 1" --auto ScnAma --seed 1234
py tools\trace-recorder\poke.py --forms my-forms.json --poke "4 warp 2" --auto ScnAma --seed 1234
:: any recorder takes the same --poke / --poke-file (record_state, record_frames)
:: one-command 1.14d vs d2rs comparison of a .check file (at <frame> poke ... lines)
py tools\scenario-diff\scenario_diff.py traces\checks\poke-fallen-town.check
:: d2rs
cargo run -p scenario-run -- run traces\scenarios\poke-spawn-town.scenario --game-dir %D2_GAME_DIR%
cargo run -p d2-client -- play --save ScnAma.d2s --seed 1234 --poke-file traces\pokes\spawn-town.poke
cargo run -p d2-client -- state-dump --save ScnAma.d2s --seed 1234 --ticks 54 --poke "4 spawn 19 @x+3 @y+3 normal" --out traces\raw\x.jsonl
:: test variant (output outside the repo; never commit it)
cargo run --release -p data-tool -- variant build traces\variants\only-fallen\only-fallen.d2stack --out ..\variants\only-fallen
py tools\trace-recorder\record_tick.py --game ..\variants\only-fallen\Game.exe --auto ScnAma --seed 1234
:: front end (main menu, character select, create, loading)
py tools\trace-recorder\record_frames.py --seconds 140 --front-end --front-end-script "wait 8; shot main-menu; wait 5; click 400 307; wait 10; shot char-select; wait 5; click 117 498; wait 12; shot char-create; wait 5; click 97 555; wait 10; shot loading change; key ENTER; wait 25; end"
py tools\trace-recorder\facts_render.py traces\raw\<time>-frames.jsonl --shot main-menu
```

Rules: pokes run between two server frames (after frame f − 1, before
frame f's drain) and draw no RNG of their own. Checked on 1.14d vs d2rs
(REC-590): `traces/checks/poke-fallen-town-unpinned.check` equal in
every compared field for 54 frames. The front-end script's waits were
tuned under Wine (menu up after ~20 s); on Windows shorten the first
wait. A variant is a patch stack (rule 9): commit the `.d2stack` /
`.d2patch`, never the built install.

## Step 4 — binary reads and one recording only PC 1 can do (queued by the coordinator, 2026-10-08)

Cloud sessions now record 1.14d themselves under Wine (REC-290 RNG half
equal to PC 1's traces; `tools/cloud-game/`), so PC 1 keeps only what
needs `re/` or a real Windows run. Each answer goes into its owner spec
(prose / authored pseudocode, addresses); a code disagreement becomes a
`q-fix-*` row.

- [q-fix-act4-play] De Seis seal on a WingN1 game: open the Chaos Sanctuary's seal 394 (object 394, De Seis) on a game whose arm file is WingN1 (seal at DS1 x 33; boss spot (-39,+33) lies 6 sub-tiles outside the DS1). Does De Seis spawn, and where? d2rs ends `free_spot` on a spot with no room and leaves the seal shut (`docs/handoff/q-fix-act4-play.md` Open 1).
- [q-fix-npc-menus] What closes ui 0x11 (UI_QUESTLOG, opened by the S→C quest-log tail `msg-ui.md` §1 r6) in 1.14d, and what does Esc do with it open? The flag table says Esc-closable 0, so d2rs's Esc closes it through the game-menu open and the next Esc restores it (keep = 1). Check on the Windows game: talk to Atma with a pending quest log, press Esc once.
- [q-fix-npc-interact] A2Q4 palace hooks, three reads for `world/quests-act2.md` §10: (1) which chain's record does `0x0059D7E0` ("not-intro with state 1") test (d2rs reads chain 13, REC-1631)? (2) `0x0059F580`'s placement beside the blocker: is "free test sizes 1, 2, 3 in turn" the first size whose `0x0064E7B0` search succeeds, and with which mask / fallback arguments (d2rs: first size, mask 0x3C01, REC-1632)? (3) `0x0059B8F0(game, &(x, y))`: when does it return nonzero, and is the point written only then (d2rs keeps (0, unchanged), REC-1633; `ai-bodies-7.md` §7 step 2.1)?
- [q-fix-npc-interact] Text list order of S→C 0x27 (`world/npc.md` §2 step 5): does `0x006612F0` (add an entry) prepend or append, and does `0x00661480` write the entries from the list head? d2rs sends newest first (REC-1401), matching the one recording with two entries (Akara (64, 11)); a table state with two entries for one NPC (e.g. `quest-messages.tsv`) would settle it.
59. **[q-fix-act4-play] De Seis seal on a WingN1 game** answered → `world/quests-act4.md` §5.4 (recorded: spawns; dummy at (7770, 5226), De Seis at (7773, 5207)). open the Chaos Sanctuary's seal 394 (object 394, De Seis) on a game whose arm file is WingN1 (seal at DS1 x 33; boss spot (-39,+33) lies 6 sub-tiles outside the DS1). Does De Seis spawn, and where? d2rs ends `free_spot` on a spot with no room and leaves the seal shut (`docs/handoff/q-fix-act4-play.md` Open 1).
59. **[q-fix-act4-play] De Seis seal on a WingN1 game** answered → `world/quests-act4.md` §5.4 (recorded: spawns; dummy at (7770, 5226), De Seis at (7773, 5207)). open the Chaos Sanctuary's seal 394 (object 394, De Seis) on a game whose arm file is WingN1 (seal at DS1 x 33; boss spot (-39,+33) lies 6 sub-tiles outside the DS1). Does De Seis spawn, and where? d2rs ends `free_spot` on a spot with no room and leaves the seal shut (`docs/handoff/q-fix-act4-play.md` Open 1). *Cloud, Wine (q-fix-a4-deseis, 2026-10-09)*: the same numbers when the seal is operated right after the walk (`traces/checks/a4-deseis-seal-early.check`, d2rs identical); operated after 200 idle frames at the seal, with the arm room south of it unloaded, 1.14d keeps the seal in mode 0 and d2rs too (`a4-deseis-seal-unloaded.check`): the boss-spot lookups see active rooms only. No d2rs change needed (`docs/handoff/q-fix-a4-deseis.md`).

1. **Vitals dx/dy sign (top suspect for the remaining rubber-banding).** answered → see "Hand-back — PC 1 day run" below.
   S→C 0x18 / 0x95 / 0x96: the server side `0x00548760` vs the client
   side `0x0045DC50` / `0x0045DB20`. `combat/vitals.md` §5.2/§5.4 and
   `client/msg-units.md` §5 r3 contradict each other; the ignored test
   of row `q-fix-proto-vitals-dx-sign` / `q-fix-seam-vitals-delta` is
   ready.
2. **Esc menu pause in single player** answered → see "Hand-back — PC 1 day run" below. (`client/bridge.md` §8 r5; rows
   `q-fix-ui-pause`, `q-fix-seam-pause`): does 1.14d stop the game loop
   under the Esc menu, and from which call.
3. **Hireling target search range** answered → see "Hand-back — PC 1 day run" below.: 20 sub-tiles (REC-100) vs 35
   (REC-279).
4. **x87 precision at start-up (REC-21)** answered → see "Hand-back — PC 1 day run" below.: the C runtime start-up's
   control word; settles five provisional points.
5. **REC-290 tick half** answered → see "Hand-back — PC 1 day run" below.: `record_tick.py --auto ScnAma --seed 1234
   --ticks 600` on PC 1, committed as a trace, so the cloud can compare
   its Wine run (equal except ms between two Wine runs).

6. (answered → see "Hand-back — PC 1 day run" below) UI spec gaps from `docs/handoff/q-ui-audit.md`: menu-box window
   handlers 0x0E/1, drop cell `0x00486BD0`, gamble flag (panels-2 §14
   r11 vs menus §4.2), waypoint level names.
7. **Callers of three S→C senders (q-fix-proto-rest, REC-415)** answered → see "Hand-back — PC 1 day run" below.: the static
   callers of `0x0053C1D0` (0x20 StatUpdate), `0x0053C6F0` (0x93 skill
   bonus by element and page) and `0x0053E1C0` (0xA6; the spec found none)
   and `0x0053B3D0`'s 0x92 call site: when each runs and with which
   values. The d2-sim builders and the client handlers exist and are
   contract-tested; only the call sites are missing. Answer into
   `client/msg-stats-items.md` §1 r4 / §5 r5 / r7 and `client/msg-skills.md`
   §9.
8. (answered → see "Hand-back — PC 1 day run" below) **Argument form of `0x00554200(unit)` at the 0xAB case of
   `0x00571CD0`** (REC-412) and the base-or-total read of stat 178 in
   `0x00625A50` (REC-410): `sim/intents-events.md` §7.9 r2 and §7.3 r2
   step 9.
9. **Field sources of S→C 0x73 in `0x0059FEE0`** answered → see "Hand-back — PC 1 day run" below. (REC-414): which missile
   fields fill the two u32 positions, the first path point and the level
   byte; `missiles/missiles.md` R2.4.
10. **Node order and asserts of the client 0x92 handler `0x004C23E0`** answered → see "Hand-back — PC 1 day run" below.
    (REC-416): the order of the inventory nodes it walks and what `0x0063E0B0`
    does at its end; `client/msg-stats-items.md` §5 r5.
11. **Command 1 slot order in the controls table** answered → see "Hand-back — PC 1 day run" below. (`q-fix-real-controls-default-order`):
    Game.exe at 0x312220 holds (cmd 1, 'B', slot 0) before (cmd 1, 'I', slot 1);
    `specs/ui/key-commands.tsv` with the §3.4 / §B4 r1 rule gives them the other
    way round. Spec decision: a slot-order column in the TSV or an exception
    for command 1, and whether first-match lookup makes the order matter; then
    `BindingTable::defaults` and the test builder change together.
12. (answered → see "Hand-back — PC 1 day run" below) **Format-0 property wrapper `0x0065FE10` (REC-289 (5),
    q-fix-items-play)**: (a) the sixth argument of `0x0065FEC0` (the
    apply type, `items/properties.md` §2) at each static caller other
    than the affix roller (`0x005C12F0`), §11 and §12: which value each
    passes (d2rs passes 0); (b) `0x0065DD80`: is the `param` > 3 test
    signed, and is a negative `param` clamped or used as is; (c) does
    the craft list (`0x00660240`, mode 7) call `0x0065FE10` or
    `0x0065FD70` directly (§2 says directly, §14 lists §12 among the
    wrapper's callers). Answer into `items/properties.md` §2 / §14.
13. **State param sign in S→C 0xA8 / 0xAA** answered → see "Hand-back — PC 1 day run" below. (`q-fix-proto-state-param-sign`):
    the sim writes the param unsigned (`units/messages.rs`), the client
    reads it signed (`client/stat-lists.md` §3 r1); settle the read sign
    with `0x0045EE20` / `0x00470E30` (`stat-lists.md` OQ5).
14. **Stamina scale on the client** answered → see "Hand-back — PC 1 day run" below. (`q-fix-seam-stamina-scale`): the wire
    carries stamina >> 8, the client's exhaustion test reads raw 1..255;
    does 1.14d's client drain stamina locally (`client/model.md` OQ2,
    REC-51)? Spec decision first, then `bridge/predict.rs`.
15. **The other four progression call sites** answered → see "Hand-back — PC 1 day run" below. (`q-fix-save-gaps`, REC-265
    (2)): `0x00538680(client, step, difficulty)` at `0x0058DCE2`,
    `0x0058DD65`, `0x0058E4F1` (Act II) and `0x0059C848` (Act III): which
    quest event and which `step` each passes. Act I (`0x00596210`), Act IV
    (`0x005B4D77`, step 4, classic only) and Act III's Mephisto credit
    (`0x005BC182`) are wired.
16. **Loader messages 0x22 / 0x21 at the join** answered → see "Hand-back — PC 1 day run" below. (`q-fix-flow-join-load`):
    the per-item conditions of the loader's S→C 0x22 (`0x0055C216`) and
    0x21 (`0x0057017B`) (`intents-events.md` §8.2 r3.1 (c)); the quest
    entry itself is wired (`world/quests.md` §3 names the caller).
17. **Think restart gate `0x00553160`** answered → see "Hand-back — PC 1 day run" below. (REC-442, q-cloud-game): in
    SUNIT_Add's monster branch (`init.md` §4.1), when does 1.14d restart
    the think at f + 2? d2rs always restarts it. Answer into `init.md`
    §4.1 / `ai.md`.
18. **Missile damage setup `0x0059F900` → `0x0064B860`** answered → see "Hand-back — PC 1 day run" below.
    (`q-fix-real-missile-damage`, from q-fix-real-skills): how a missile's
    damage record is filled from its owner and skill; no spec exists.
    Write `missiles/damage.md` (or a section of `missiles/missiles.md`).
19. **Arcane Sanctuary star tick** answered → see "Hand-back — PC 1 day run" below. (`draw-order-2.md` OQ 12, HANDOFF §5
    entry 103; REC-420): the initial `last` of §12 r3 and the time / seed
    argument form.
20. **Windows recordings Wine cannot make** partly answered (front-end, night and earlier scenes done; frontend-character-select open, `docs/handoff/pc1-day4.md`). (`facts/requests.tsv`, open
    rows from q-facts-scenes): stash panel; NPC dialog, shop and gamble
    screens (clicks do not open them under Wine); the 4 front-end scenes;
    Blood Moor monsters at night; Den of Evil. Record with
    `tools/trace-recorder/record_frames.py` and `facts_render.py` as in
    Step 3, each twice, and commit the facts.
21. **Owner of a unit, for the state snapshot** answered → see "Hand-back — PC 1 day 2" below. (`specs/tools/state-snapshot.md`
    §2 `own`, OQ 1; q-tool-state-diff): where 1.14d keeps the owner GUID
    of a pet / summon / hireling, a missile and an item (its container or
    holder), as offsets read from a server unit, so `record_state.py` can
    fill `own`. Write the answer into `sim/units.md` (or the owner spec)
    and the §2 row of `state-snapshot.md`.
28. **0x8E flag byte at the join** answered → see "Hand-back — PC 1 day 2" below. (q-fix-pc1-proto-items) The join's 0x8E CorpseAssign per corpse of another client's player (`0x0053DFB0` from `0x0052C410`, `sim/intents-events.md` §8.3) is sent with flag byte 1 (assign); the flag source at that call is not read. Needed: the byte `0x0052C410` passes to `0x0053DFB0`.
34. **Town critters are a client-side spawn from `Levels.txt` C1 / CA1** answered → see "Hand-back — PC 1 day 2" below. (q-fix-real-critters-drops; same topic as item 30) The Rogue Encampment chickens (ck, class 149) are not server units: the server's unit lists have none (state snapshots to frame 90, both sides equal at 25 units). 1.14d's client creates them with `0x00466730(class 149, x, y, 1, ..)` (type 1, client set C, GUID from the client counter `[0x00711F30]`), three per group, from return address `0x0046C316`; the caller chain is `0x0046C54D` <- `0x0044C774` (the client room function `0x0044C750`, which also runs the preset pass `0x00466820`). Levels.txt row 1 has `C1` = 149, `CA1` = 30 (the level record dump at `[arg5]` shows 0x95 and 0x1E). Per group the draws are: 3 steps at `0x0046C4AC` on three different `{lo, 666}` seeds, a `roll(0)` via `0x0045C3E0` at `0x0046C516` on the room's client seed, then per chicken one step at `0x0046C257` and one at `0x0046C29C` (x and y). First groups in frame 2 of `a1-town-arrival-ama`: (4827, 4195) (4820, 4198) (4832, 4191) and (4927, 4198) (4939, 4198) (4956, 4195); later rooms (frames 89-95): (4814, 4256) (4827, 4269) (4834, 4275) and (4871, 4242) (4877, 4254) (4849, 4267). Needed (a spec for a new `world/` or `client/` section): the body of the function holding `0x0046C257`-`0x0046C54D`: what makes a group (CA chance? MinGrp / MaxGrp 3 / 3 of the chicken row), the centre point and offset formula from the draws, the seed each step uses, and when the room function runs it (client room entry and exit; the GUIDs 93-95 of the scene need the 190 river-object creations at `0x00466862` before them). Probe: a scratch subclass of `record_state.py` hooking `0x00466730` / `0x00466360` (not committed); the facts are in `docs/handoff/q-fix-real-critters-drops.md`. `population.md` §11.3 (critters are not placed by the server) is confirmed.
26. **Re-record sim-0009 without input** answered → see "Hand-back — PC 1 day 2" below. (q-prov-recording, REC-290): the Wine
    run of the same command equals `traces/sim/tick/sim-0009.json` for ticks
    0–60, then the Windows trace has a client message at tick 61 (drain: a
    timer on unit (1, 7), player queue) and the player changes rooms at 121,
    241, 351: the window got input. Run `record_tick.py --auto ScnAma --seed
    1234 --ticks 600` again with the mouse outside the game window and replace
    sim-0009. (Item 20 note: under Wine the front end does take X input,
    `tools/cloud-game/xinput.sh`; in-game NPC menus were not tried.)

29. **Monster path target at the death message** answered → see "Hand-back — PC 1 day 2" below. (q-tool-poke, REC-594): 1.14d state
    snapshots read an idle spawned monster's path target (+0x10/+0x12) as
    its spawn point (`monsters/init.md` §4.1 step 1.1; check
    `traces/checks/poke-fallen-town.check`), so d2rs now writes it there;
    but `sim/intents-events.md` §7.4 rule 7 says the 0x69 code-8 target
    is (0, 0) "when the path never had a target", from the recorded
    `69 1b000000 08 0000 0000 38 06` (frame 2882). Read which field 0x69
    code 8 copies and whether that monster's target was cleared (an AI
    request) before its death; answer into §7.4 rule 7. d2rs tests
    `monster_death.rs` / `e2e_night_world.rs` now expect the spawn point.
57. **[q-fix-real-item-replay-belt-use] Potion use: what entry 3 does to the state (`items/use.md` §3, `0x005BE3F0`)** answered → see `items/use.md` §3.1 (REC-102 / REC-730).
    The spec is silent (open question 1); d2rs now does what the
    recording shows (`facts/items/a1-town-potions-low.tsv` n 9–31,
    `a1-town-item-moves.tsv` n 40–51): a potion used from the grid (0x20)
    is drunk by U and consumed (0x9D action 5, flag 0x20), not refused; the
    state (100 / 106) goes on with an **empty** stat stream in 0xA8
    (`ff 01`: either its per-tick stats are not on the state list, or they
    have no send bits); at full life / mana the state ends in the same
    frame (0xA9, no 0xA8). Read entry 3 and answer into `items/use.md`:
    which list it attaches (stats, length: 170 frames at 10 of 50 life,
    51 frames for an mp1 at 1 mana), what ends it when the vital is full,
    and whether the unit is queued for update by the toggle
    (`0x00639DB0`) or by the entry itself. d2rs: PROVISIONAL REC-730 in
    `d2-sim/src/wiring/inventory/potion.rs`.
47. **[q-fix-pc1-day3-a-r2] Who gives a player its alignment (state 105, stat 172 = 2), and the frame-2 resend** answered → see `docs/handoff/pc1-day4.md`.
    The Wine join of a new sorceress (`facts/join/a1-new-sor.tsv`) shows
    the player already carrying state 105 with stat 172 = 2 in the
    loader's 0xAA (n 8, `aa00010000000c6959f9ff1f`), then at frame 2 a
    0xA8 resend of it (n 86, `a800010000000b69acfc0f`, the "resend s"
    writer `0x0055448A` of `sim/intents-events.md` §3.5 rule 6) followed by
    the player update's stat sends 12, 0, 2 (n 87–89, §7.3 rule 1 step
    7). No spec names the `0x005543B0` caller for a player (allocation?
    the loader?), nor which call sets the changed bit and queues the
    player at frame 2. d2rs gives players no alignment state, so its join
    has an 8-byte 0xAA and no frame-2 0xA8 / 0x1D: the last four of
    `app_frame_loop`'s missing join messages (122 of 126). Read the
    caller, its place in the loader / allocation, and the resend's
    caller; answer into `sim/units.md` (allocation) or
    `formats/d2s-load.md`, and `intents-events.md` §8.2.
45. **Melee on a fallen from the shared script** answered → see docs/handoff/pc1-day3-b.md. (q-fix-b-headless-unit-click-keys;
    `specs/tools/scenario-diff.md` §2 r4.5, §3 r8.5):
    `py tools\scenario-diff\scenario_diff.py traces\checks\combat-melee-fallen.check --orig-only`.
    Look for: the footer notes `autostart: frame 39: clickunit 1 19 368 300
    posted at the stop of frame 38` and `frame 40: click 368 300 posted at
    the stop of frame 39` (the hover frame); in the state the player in
    mode 7 from frame 40 to 54 and fallen GUID 21's `hp` 256 -> 0 at frame
    46 (Wine and d2rs both do this); in `orig.packets.jsonl` a C->S
    `06 01000000 15000000` (left skill on the fallen), not a walk (0x01 /
    0x03) — only once `record_packets.py` applies `frame` input
    (`AutoStart.attach`; today it notes "they never run here"). Then run
    without `--orig-only` (both sides) and record the first difference.
46. **A belt potion mid-fight from the shared script** answered → see docs/handoff/pc1-day3-b.md. (same row):
    `py tools\scenario-diff\scenario_diff.py traces\checks\combat-potion-midfight.check --orig-only`.
    Look for: the footer note `autostart: frame 70: key 49 posted at the
    stop of frame 69`; in the state the player's `hp` 2560 -> 2640 at
    frame 70, +80 a frame, and item GUID 2 (slot 0) gone (Wine: so); in
    `orig.packets.jsonl` (once it takes `frame` input) the join's S->C
    0x9C action 0x0E for the two hp1 (d2rs sends none: its load leaves
    belt items in mode 4) and a C->S `26 02000000 00000000 00000000`.
48. **[q-fix-pc1-proto-items] Where the join sets the player's alignment (state 105, stat 172 = 2)** answered → see `docs/handoff/pc1-day4.md`.
    `combat/hit.md` §7.1 says players carry it, and the recording shows
    it in the player's first 0xAA (`packets-town-arrival-ama.check` seq
    39) with a 0xA8 of state 105 one frame later, but no spec names the
    call site. Read the character load of the join (`0x005344B0` /
    `0x00534520`, the allocation's per-kind init `0x005348C0`) for the
    `0x005543B0(player, 2, v)` call: where it runs relative to the unit
    seed and the stats, its v argument, and whether a corpse (player
    unit in mode 17) gets it too. Answer into `combat/hit.md` §7.1.
    d2rs: PROVISIONAL REC-732, set right after the unit seed
    (`d2-client` `app/single_player.rs` loader, `View::set_alignment`).
49. **[q-fix-pc1-proto-items] `0x0059F570` (ACT2Q4 "Jerhyn palace activated") and the Jerhyn AI case** answered → see `docs/handoff/pc1-day4.md`.
    `monsters/ai-bodies.md` §9.9 step 2 row 201 calls `0x0059F570`; no
    spec gives its body. Recorded (`act-travel-lut-ama.check`,
    `join-act2-quests-ama.check`): start Jerhyn's first think (frame 24)
    walks to his own position + (2, 2) with no unit-seed draw, so the
    "= 0 → idle 40" branch is not what runs for a fresh or act-1-done
    character. Read `0x0059F570` (and confirm `0x0059F580`'s outputs a,
    b for these states) into `world/quests-act2.md` §10. d2rs: REC-734.
50. **[q-fix-pc1-proto-items] `0x00625870`: the mod-array test and a key absent from the base array** answered → see `docs/handoff/pc1-day4.md`.
    `sim/stat-lists.md` §11 rule 4 was corrected from the recording
    (`packets-town-arrival-ama.check` frame 2: the player update sends
    12, 0, 2, keys in the mod array, and not 67 / 68, base 100 outside
    it): the single-stat send runs when the key **is** in the mod array.
    Read `0x00625870` to confirm the test's sense and say what it sends
    for a key in the array but absent from the base array (0, as the
    flush `0x006258D0` does, or nothing; d2rs: nothing). Answer into
    §11 rule 4.
- **[q-fix-b-monster-combat] Zero-length walk: which mode message (REC-890)** answered → see docs/handoff/pc1-day3-b.md.
    `monsters/ai.md` §7.5 rule 8 says the neutral start after a
    zero-length walk sends S→C 0x67 code 7 at U's cell; the builder's
    owner spec `sim/intents-events.md` §7.4 rule 5 sends mode 1 as 0x6D
    (GUID, cell, life byte; stat 328 += 1) and stops. d2rs sends 0x6D (test
    `a_zero_length_walk_goes_neutral_and_sends_code_7`). Read which one
    `0x00597E20` sends for that update and fix the other rule.
- **[q-fix-b-monster-combat] Monster base list: owner and attach reset (REC-891)** answered → see docs/handoff/pc1-day3-b.md.
    `monsters/init.md` §6 step 12 (`0x006251F0` + `0x00626E10`, the
    flag-1 list at `0x0057407D`) gives no owner type / GUID, expire or
    attach `reset`. d2rs: flags 1, expire 0, owner = the monster, reset = 1
    (a DYNAMIC list would keep `mindamage` / `maxdamage` / `tohit` out of
    the totals). Read the arguments; answer into init §6 step 12.
- **[q-fix-b-monster-combat] Owner data f1 / f2 (REC-892)** answered → see docs/handoff/pc1-day3-b.md.
    `0x0058F030(game, u, GUID, type, f1, f2)`: `umod-callbacks.md` §1
    rule 5 says f1 / f2 ≠ 0 "restart the AI" (`0x005DD230`);
    `sim/units.md` (`0x0058F530`) reads as control flags |= 0x2, |= 0x1.
    d2rs writes the minion owner (+0x2C / +0x30) and nothing for f1 / f2
    (the Fallen leader `SetBoss`, `BossXfer` call is (GUID, 1, 1, 1)).
    State what `0x005DD230` does with each flag.
- **[q-fix-b-monster-combat] Quill Rat at frame 59 (q-fix-b-quillrat-shoot)** answered → see docs/handoff/pc1-day3-b.md.
    d2rs (`traces/checks/combat-arrow-quillrat.check`, staging + this
    branch): the rat is placed exactly at the poke point (5147, 4267),
    4,4 from the player at (5143, 4263); its think at 31 has D = 6, no
    command, C = 0, P(aip2 35) passes (lo' 8) → A2; one more draw on its
    seed before 59 (the quill); at 59 the draw gives 89 → escape (walk).
    The same run without the arrows is identical, so the arrows do not
    cause it. No number of extra draws between 31 and 59 makes P(35)
    pass (0 → 64, 2 → 39), so 1.14d took A2 by another branch (D ≤ 3
    after a failed escape, AI state 3/19, a command) or from another
    position: PC1-B notes the 1.14d rat "is placed elsewhere". Please
    commit (or paste here) the 1.14d rat lines of
    `traces/raw/check-combat-arrow-quillrat/orig.state.jsonl` for frames
    30–64 (x, y, m, s), or answer which §9.7 step the 1.14d think at 59
    takes and the rat's position.


42. **Control-panel help button `0x004A64C0`** answered → see `docs/handoff/pc1-day3-c.md`. (q-scenes-compare) Step 8 of the UI pass (`ui/panels.md` §5) calls it before the new-stats button; `a4-town-pandemonium-fortress` rows 258–267 draw the text "Help (H)" (CelDrawColor at (714, 403)), `Panel\Levelsocket` frame 0 at (725, 440) and `Panel\Level` frame 0 at (728, 436). Specify when it draws (character level? first game?), its positions and its press / release, for `ui/control-panel.md`.
43. **Mini panel open at game start (REC-519)** answered → see `docs/handoff/pc1-day3-c.md`. (q-scenes-compare) Every recorded scene has state 0x15 open with no input; name the call that opens it at game entry (and whether a saved setting decides it), for `ui/control-panel.md` §9.
44. **Shadow pre-test arguments `0x00471620` (REC-511, REC-518)** answered → see `docs/handoff/pc1-day3-c.md`. (q-scenes-compare) The measured shadows fit the §4 box test on the sheared shadow box, and objects need their mode's `BlocksLight`; read the arguments `0x00471620` passes to `0x004709A0` and the object branch, for `render/blend-modes.md` §5 r3.
51. **[q-scenes-compare] Client footprints of walking monsters (REC-706)** answered → see `docs/handoff/pc1-day4.md`. The 1.14d client stamps mask 0x100 for each living monster (`client/msg-units.md` §3 r2); d2rs re-stamps it at the model position before each client path step. Record the client collision grid (mask 0x100 cells) around Warriv in the Rogue Encampment for 30 ticks while he walks, with the unit's client path position each tick, for `client/model.md` open question 2.
58. **[q-scenes-compare] Hover state after a use press (REC-707)** answered → see `docs/handoff/pc1-day4.md`. `a1-panel-cube` (right click on the cube, no move after) draws the cube with tint 2 and no tip, so `0x007BCBF4` / `0x007BCBE4` are 0 after the press. Name the callers of the hover handler `0x00487000` (move, press, release?) and which code clears the two globals after a right-click use (C→S 0x20), for `ui/inventory.md` §5 r4.

- [prov-data] **Hratli's unit seed two steps at creation** answered → see `docs/handoff/pc1-day3-c.md`. (q-prov-data,
  `world/quests-act3-2.md` §3.3, `monsters/init.md` §4): in the Act III
  town (`a3-start-noquest-sor`, level-1 sorceress, town byte act III,
  seed 1234) Hratli (class 253), spawned by his dummy's init through
  `0x005B2F20(game, room, x, y, 253, mode 1, -1, 0)`, has the unit seed
  [3975998680, 1369742535] at frame 2: the fresh draw (4040195123, 666)
  stepped **twice**. d2rs has it unstepped, and so do the NPCs made by
  presets (294, 264), which match 1.14d. Find the two draws on Hratli's
  own seed between the allocation and frame 2: a spawn-wrapper step
  (flags 0, spread -1), the dummy init `0x005B...` (init 49 / 50) or a
  think; `monsters/init.md` §4 lists none.

60. **[q-fix-room-links]** answered → `drlg/rooms.md` §8 (recorded: 1.14d also leaves far town rooms unpopulated; old level freed 122 frames after the return). After a waypoint return to a town, does 1.14d populate only the rooms near the player (far NPCs/objects appear on approach), as d2rs does? Drive town -> waypoint -> back, snapshot units at +50 and +600 frames (`docs/handoff/q-fix-room-links.md`).
61. **[q-fix-npc-menus] What closes ui 0x11 (UI_QUESTLOG), and Esc with it open** answered → `ui/panels.md` §2 r10, `ui/frontend-options.md` §O1 r2 (from the binary: ui 17 is the quest-log alert button; Esc remembers, closes and restores it with the latch 0; d2rs never clears the latch: `q-fix-pc1eve-questlog-alert`).

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

22. **Poke call forms** (q-tool-poke): (a)–(d) *answered* 2026-10-09
    from the asm into the owning specs and `poke.py` `CALL_FORMS`; first
    1.14d run of the five directives is REC-655 (HANDOFF §7). Was
    (`specs/tools/poke.md` Open questions 1–4):
    register / stack form and `ret` of (a) `0x00554EA0(game, unit, room,
    x, y, exact, alt)` or the teleport path `0x00650BE0` (`pos`,
    `path-placement.md` §10 / §6 r4); (b) the level warp
    `0x0053AEC0(game, player, level, tile)` (`warp`, `waypoints.md` §7
    r5); (c) a ground-item entry at a point: `0x00558D90(game, request,
    use seed)` with request +0x18 = 3, +0x1C/+0x20 x, y, +0x24 room, and
    the code → index lookup `0x00633640` (`item`, `items/generation.md`
    §3); (d) `0x00627260(unit, s, value, layer)` and `0x00639DB0(unit,
    s, on)` (`stat`, `state`, `stat-lists.md` §5 r2, §9.2). Answer into
    the owning specs; then delete the gap rows in `poke.md` §1. (e) *answered* by
    `monsters/init.md` §25.1 / §25.3 (boss spawn, minions, umod init,
    umod list): `poke.py` runs `spawn champion`, `random-boss`, `unique`
    (not yet run on 1.14d). Each form can be tried first with
    `poke.py --forms FILE` (README "Call forms") before it goes into
    `CALL_FORMS`.

23. **Poke runs on Windows** (REC-590) — answered → see `docs/handoff/pc1-day3-a.md`. The cloud ran every runnable
23. **Poke runs on Windows** (REC-590; answered → see `docs/handoff/pc1-day3-c.md`): the cloud ran every runnable
    directive on 1.14d under Wine (`specs/tools/poke.md` Status) and
    settled the variant load (REC-591, `tools/test-variants.md` Status).
    Left: run `traces/pokes/spawn-town.poke` once on PC 1 with a
    screenshot (Wine screenshots are blank). Commands in "Set up any
    state for a check" below.
24. **Client missile motion and body gaps** answered → see "Hand-back — PC 1 day 2" below. (`q-fix-client-missiles-rest`,
    REC-540–549): `missiles/client.md` Open questions 9 onward: the
    timed arc `0x004DA5B0` (flag store vs set, the vz division), the
    motion getters `0x004DA110`–`0x004DA150` (shifted or stored) and the
    other points that session lists there. Answer into
    `render/unit-composite.md` §8 and the client missile specs.
25. **Potion state length and the end-when-full rule** answered → see "Hand-back — PC 1 day 2" below. (`q-fix-real-potion-effect`, REC-102): item-use entry 3 body (`0x005BE3F0`, `items/use.md` §3 OQ 1). Recording: hp1 at 10/50 life, state 100 from 0xA8 to 0xA9 = 170 frames, mp1 at 1 mana 51 frames (mana full), hp1 at full life ends the next frame; `misc.txt` says `len` 192 / `calc1` 30 (hp1) and `len` 128 / `calc1` 20 (mp1), so the rule that gives 170 and 51 is not `len`. Answer into `items/use.md` §3; then `wiring/inventory/potion.rs` loses its PROVISIONAL.
26. **Re-record sim-0009 without input** answered → see "Hand-back — PC 1 day 2" below. (q-prov-recording, REC-290): the Wine
    run of the same command equals `traces/sim/tick/sim-0009.json` for ticks
    0–60, then the Windows trace has a client message at tick 61 (drain: a
    timer on unit (1, 7), player queue) and the player changes rooms at 121,
    241, 351: the window got input. Run `record_tick.py --auto ScnAma --seed
    1234 --ticks 600` again with the mouse outside the game window and replace
    sim-0009. (Item 20 note: under Wine the front end does take X input,
    `tools/cloud-game/xinput.sh`; in-game NPC menus were not tried.)
27. **NPC nearest player and walk in radius** answered → see "Hand-back — PC 1 day 2" below. (q-fix-real-unit-seed-order)
  (REC-500, REC-501, `specs/monsters/ai.md` §5.3, §7.2): read the scan-2
  callback of `0x005DDF20` (distance function, `<` or `≤ 15`, ties) and
  `0x005DE4E0` (target point geometry, rounding, path step count,
  failure when the point is the unit's own). d2rs reads: no-size
  distance ≤ 15; point = own + Δ·min(a, dist − b) / dist rounded to
  nearest; it matches Warriv's three recorded arrival walks.
28. **0x8E flag byte at the join** answered → see "Hand-back — PC 1 day 2" below. (q-fix-pc1-proto-items) The join's 0x8E CorpseAssign per corpse of another client's player (`0x0053DFB0` from `0x0052C410`, `sim/intents-events.md` §8.3) is sent with flag byte 1 (assign); the flag source at that call is not read. Needed: the byte `0x0052C410` passes to `0x0053DFB0`.
29. **Monster path target at the death message** answered → see "Hand-back — PC 1 day 2" below. (q-tool-poke, REC-594): 1.14d state
    snapshots read an idle spawned monster's path target (+0x10/+0x12) as
    its spawn point (`monsters/init.md` §4.1 step 1.1; check
    `traces/checks/poke-fallen-town.check`), so d2rs now writes it there;
    but `sim/intents-events.md` §7.4 rule 7 says the 0x69 code-8 target
    is (0, 0) "when the path never had a target", from the recorded
    `69 1b000000 08 0000 0000 38 06` (frame 2882). Read which field 0x69
    code 8 copies and whether that monster's target was cleared (an AI
    request) before its death; answer into §7.4 rule 7. d2rs tests
    `monster_death.rs` / `e2e_night_world.rs` now expect the spawn point.
28. **Interact range test `0x00623660(P, O)`** answered → see `docs/handoff/pc1-day4.md`. (REC-94): its formula
    (object size, which positions). Measured under Wine
    (`facts/objects/objanim-a1-town.tsv` run r3): returns 1 for the
    waypoint 119 with the player 4 sub-tiles off in x and 3 in y, 0 at 5;
    for the stash 267 only at 3 / 1. Answer into `world/objects.md` §7.1
    r3 (d2rs reads it as always in range for a player: true for every
    0x13 the client sends).

30. **Client-made critters (set C monsters)** answered → see "Hand-back — PC 1 day 2" below. (q-fix-real-unit-seed-order)
  (`q-fix-real-town-critters`, `client/model.md` §5 r3 "C monsters",
  `monsters/population.md` §11.3 r2): the Rogue Encampment arrival has
  three chickens (ck, class 149) with GUIDs 93–95 that the server never
  allocates (25 server unit seeds in 90 s under Wine, none for them;
  critter presets are not placed by the server). Read the client path
  that makes them: which client pass reads the DS1 critter presets (or
  another source), the GUID counter (why 93), the client set-up
  (`0x004AE8D0`-like: stats, seed, first frame), and their client-side
  AI / motion (the recorded ck frames walk: WL at tick 8 and on).
34. **Town critters are a client-side spawn from `Levels.txt` C1 / CA1** answered → see "Hand-back — PC 1 day 2" below. (q-fix-real-critters-drops; same topic as item 30) The Rogue Encampment chickens (ck, class 149) are not server units: the server's unit lists have none (state snapshots to frame 90, both sides equal at 25 units). 1.14d's client creates them with `0x00466730(class 149, x, y, 1, ..)` (type 1, client set C, GUID from the client counter `[0x00711F30]`), three per group, from return address `0x0046C316`; the caller chain is `0x0046C54D` <- `0x0044C774` (the client room function `0x0044C750`, which also runs the preset pass `0x00466820`). Levels.txt row 1 has `C1` = 149, `CA1` = 30 (the level record dump at `[arg5]` shows 0x95 and 0x1E). Per group the draws are: 3 steps at `0x0046C4AC` on three different `{lo, 666}` seeds, a `roll(0)` via `0x0045C3E0` at `0x0046C516` on the room's client seed, then per chicken one step at `0x0046C257` and one at `0x0046C29C` (x and y). First groups in frame 2 of `a1-town-arrival-ama`: (4827, 4195) (4820, 4198) (4832, 4191) and (4927, 4198) (4939, 4198) (4956, 4195); later rooms (frames 89-95): (4814, 4256) (4827, 4269) (4834, 4275) and (4871, 4242) (4877, 4254) (4849, 4267). Needed (a spec for a new `world/` or `client/` section): the body of the function holding `0x0046C257`-`0x0046C54D`: what makes a group (CA chance? MinGrp / MaxGrp 3 / 3 of the chicken row), the centre point and offset formula from the draws, the seed each step uses, and when the room function runs it (client room entry and exit; the GUIDs 93-95 of the scene need the 190 river-object creations at `0x00466862` before them). Probe: a scratch subclass of `record_state.py` hooking `0x00466730` / `0x00466360` (not committed); the facts are in `docs/handoff/q-fix-real-critters-drops.md`. `population.md` §11.3 (critters are not placed by the server) is confirmed.
35. **`0x0063E6B0(unit, 0)`: are the action-frame tests skipped?** answered → see "Hand-back — PC 1 day 2" below. (REC-700) `0x005A6D50` passes the moving flag r as `0x0063E6B0`'s second argument (`skills/use.md` §5.2 "Monsters"); `skills/bodies-3.md` §5.18 step 3 gives the tests (+0x4E = 0, or no action event in the frames ((cur − speed) >> 8, cur >> 8]) for the argument 1 only. Read: what the function does with 0 (d2rs: no tests, the column by mode). Write the answer into `bodies-3.md` §5.18 step 3.
36. **Who writes unit +0x4E from a type-0 timer's frame code?** answered → see "Hand-back — PC 1 day 2" below. (REC-701) trigger(U) of `0x005A7670` reads +0x4E = 1 (`skills/use.md` §5.2); `sim/units.md` §4.2 schedules event 0 with args (E[i], k) and the field table names only `0x005533D0` (0 at mode start). Read: the writer of +0x4E on the event-0 path of a monster (the unit-type dispatcher or the timer run), and whether a code-0 event writes it. Write the answer into `sim/units.md` §4.2 / §4.6.
37. **The client's mode-18 leap / whirl: hold, path end, code 0x16** answered → see "Hand-back — PC 1 day 2" below. (REC-702..704, `d2-client` `world_view/skill_motion.rs`; measured leap / whirl in `facts/client/anim/a1-cold-plains-*-bar.tsv`) Read: (1) which client code holds the Leap sequence at frame 11 while the motion record lives, and whether Leap Attack (`seqnum` 14) holds at its frame 11 the same way (REC-704); (2) the client whirl path: its step per update (d2rs: class `WalkVelocity` << 12, 0x6000 measured for the barbarian) and the update mode 18 ends on (d2rs: the one whose step reaches the end, `((d << 16) − 1) / step` after the do, `d` by `0x006417F0`; REC-703); (3) the skill mode request's first mode-18 update and code 0x16's record 2 / 3 as unit type / GUID (REC-702). Write the answers into `render/unit-composite.md` §8 or `skills/sequences.md` §3.
38. **Range state mask 0x26 = `meleeonly`?** answered → see "Hand-back — PC 1 day 2" below. `range(P,
  skill)` `0x00645460` (`skills/use.md` §3 r6) tests "state mask 0x26".
  d2rs (`d2-client` `bridge/combat.rs` `in_melee_only_state`) reads it as
  `0x0063A130` with the per-flag mask at data +0xCC + 4·0x26, i.e. the
  `states.txt` flag bit 38 `meleeonly` (`data/fields.tsv`; the
  `ui/panels-3.md` §24 r1 scheme). Confirm the argument `0x00645460`
  passes is that flag index (not a data offset or a precomputed group),
  and write it into `use.md` §3 r6.
39. (answered → see "Hand-back — PC 1 day 2" below) **Evade's reaction (`combat/damage.md` §7.1 step
  5.2)**: "state 68 list; s, E as above" — does the evade branch also
  set E flags |= 4 and make the unit form request (E, mode 13, tA, gA,
  0) like 5.1, or only the `stsound` sound event 12? d2rs
  (`wiring/action/reaction.rs`) does the full 5.1 skill form plus the
  sound. Answer into §7.1 step 5.2.
40. (answered → see "Hand-back — PC 1 day 2" below) **Missile damage setup weapon of a monster
  (`missiles/damage.md` §1 step 6)**: d2rs serves `0x00622830` (a type-1
  owner with an inventory) with the same seam as the player's attack
  weapon `0x00623990(owner, 1)` (`Pending::attack_weapon`). Is
  `0x00622830` the same pick (`sim/units.md` §4.7 "Attack weapon") or
  plain `0x0063C9B0`? Answer into §1 step 6.

- **[q-play-act5] join act byte** — answered → see `docs/handoff/pc1-day3-a.md`. Which 1.14d function writes the
  player unit's act (+0x18) on a join into a save's act? The recording
  `traces/checks/a5-town-arrival-bar.check` shows 4 from frame 2 (a
  barbarian saved in Act V); `sim/units.md` §2 names only the allocation
  (`0x005552ED`, the allocation room's act; the player is allocated in no
  room) and the act change (`0x0053AE4E`). d2rs writes it at the join's
  act step (rule 4, REC-797). Answer into `sim/units.md` §2 and
  `sim/intents-events.md` §8.2.

## How to check a behaviour in one command

A check file `traces/checks/<name>.check` (`specs/tools/scenario-diff.md`)
names the save, seed, ticks, input and channels. One command builds the
save with `d2s-tool`, runs 1.14d with one recorder per channel and d2rs
with the same save, seed and input, and prints the first difference per
channel. Read the state report first: a state divergence is found at the
tick it happens, a draw difference only where it reaches a pixel.

Windows (PC 1; recorders run with `py` directly, no Wine; `D2_GAME_DIR`
defaults to `game\` in the repo; the save goes to `%USERPROFILE%\Saved
Games\Diablo II`, `D2_SAVE_DIR` overrides):

```
py tools\scenario-diff\scenario_diff.py traces\checks\a1-town-arrival-ama.check
```

Linux / cloud (1.14d under Wine; setup once: `tools/cloud-game/README.md`):

```
python3 tools/scenario-diff/scenario_diff.py traces/checks/a1-town-arrival-ama.check
```

| Channel | Worked example | 1.14d recorder | d2rs side | Comparator | First line of the report |
|---|---|---|---|---|---|
| `state` | `a1-town-arrival-ama.check` | `record_state.py` | `d2-client state-dump` | `state_diff.py` | `FIRST DIVERGENCE: frame N <unit> field K: 1.14d A vs d2rs B` |
| `state` + `input` | `walk-town-ama.check` (`input frame 10; click 600 300`) | same, with `--input` | `state-dump --input` | `state_diff.py` | as above |
| `rng` | `rng-town-arrival-ama.check` | `record_rng.py --frames` + `rng_owners.py` | `state-dump --rng` (feature `rng-trace`, built by the tool) | `rng_diff.py` | frame, owner (`game`, `unit T:G`), draw index, both sides' sites |
| `packets` | `packets-town-arrival-ama.check` | `record_packets.py --ticks` | `state-dump --packets` | `packets_diff.py` (masks: `specs/tools/scenario-masks.tsv`) | frame, stream (`c2s`, `s2c`), message, byte offset |
| `draws` | `draws-town-arrival-ama.check` (`draws-at 73`) | `record_frames.py` + `facts_render.py` | `play --dump-draws --at-tick` | `d2-client facts-compare --ignore tick` | `draws.tsv` row and column |
| pokes | `poke-fallen-town.check` (`at <frame> poke ...`) | `poke.py` in the recorders | `state-dump --poke` | per channel | — |

Options: `--channels state,rng` (override the file), `--orig-only` /
`--d2rs-only` (one side), `--reuse` (keep outputs in
`traces/raw/check-<name>/`, re-compare), `--dry-run` (print the
commands), `--next N`. Exit code: the worst channel's (0 match, 1
diverged, 2 partial, 3 error). Each comparator also runs alone on two
files (`py tools/trace-recorder/state_diff.py ORIG D2RS`, `rng_diff.py`,
`packets_diff.py`).

Every check at once (parallel, 1.14d recordings reused, match % per area, playthrough per act): `python3 tools/scenario-diff/suite.py [--filter GLOB] [--area A] [--md F]` (`scenario-diff.md` §4).

Known limits: the d2rs side has no hover model (a click on a unit is a
ground click) and no `key` steps headless; the click target can differ
by a sub-tile (`scenario-diff.md` OQ 3); 1.14d draws a timing-dependent
set of ticks, so `draws-at` falls back to its last drawn tick at or
before it; `rng` and `packets` with pokes or frame input are partial on
1.14d. For a new behaviour, add a `.check` file (copy the example of the
channel) rather than a hand-run recipe, and paste the report's first
lines into the `q-fix-*` row.
26. **Item flag 0x2000 and the default file index of a poked item** answered → see `docs/handoff/pc1-day3-c.md`. (area G, `traces/checks/items-ground-many.check`, 2026-10-09): 36 items made with the poke `item` (`poke.py` request: `0x00558D90`, spawn mode 3, force 0, use seed 0) have, on 1.14d, item flags 0x80000 only (0x2000 clear) and item data +0x28 = 0 for items with no unique / set / superior index; d2rs sets 0x2000 (`generation.md` §3 step 5, "not forced") and file index −1 (`quality.md` §1 "Clear"). The recorded kill drops carry 0x2000 in their 0x9C flags (`facts/items/a1-cold-plains-poke-kills.tsv`: 0x00A02010), so the poke request differs from the treasure request in something that clears it. Read in the creation function `0x00558D90`: what clears 0x2000 (and with which request field or caller state), and what writes the file index of a normal / magic / rare item (0 or −1, and when). Answer into `items/generation.md` §3 and `items/quality.md` §1; then drop `ignore if fi` from the check.

- [prov-data] **Monster think in a room with no clients** — answered → see `docs/handoff/pc1-day3-a.md` and `docs/handoff/pc1-day3-c.md` (same answer; fix row `q-fix-p3-room-empty-think`, `q-fix-p3-leave-cancels-thinks` is its duplicate) (q-prov-data,
27. **Order of the property rolls of a magic item** answered → see `docs/handoff/pc1-day4.md`. (area G, `traces/checks/items-vendor-akara-buy.check`, 2026-10-09): the store's 7th item (guid 8, quality 4, prefix 1156, suffix 552, ilvl 6) has a charged-skill property (stat 204, skill 193, level 4, max 67). 1.14d current charges 67, d2rs 65: `properties.md` §5 rule 9 gives cur = (r + c/8 + 1) & 0xFF with r = roll(c − c/8) on the item seed, so r is 58 on 1.14d and 56 on d2rs, while the item seeds after creation are equal (`ik` equal). Same draws, different order. Read which property of the item rolls before the charged one (prefix and suffix properties, the 107 single-skill rows, the 45 row) and in which order the affix properties are applied (`affixes.md` §7: P0 S0 P1 S1 P2 S2?); answer into `items/affixes.md` §7 and `items/properties.md` §5.

28. **Identified flag 0x10 of normal items** answered → see `docs/handoff/pc1-day4.md`. (area G, `traces/checks/items-ground-many.check`, 2026-10-09): the ten poked items with no quality (normal, misc: `tbk`, `rvl`, `gsw`, `gzv`, `r10`, `r30`, `key`, `ibk`, `hp5`, `mp4`, `sbk`) have item flags 0x80010 on 1.14d (identified set), d2rs 0x80000; the magic / rare / set / unique poked items are equal (0x10 clear). `generation.md` §1.4 row 0x10 says "set for quest-difficulty items and by callers", `quality.md` has no set for the normal routine. Read in `0x00558D90` and the quality dispatch `0x00557450` / normal routine `0x00556E80` where flag 0x10 is set for a normal item (and whether weapons and armor of quality 2 get it too); answer into `items/generation.md` §1.4 and §6.1.

- [prov-data] **Monster think in a room with no clients** (duplicate entry, answered above) (q-prov-data,
- [prov-data] **Monster think in a room with no clients** — answered → see `docs/handoff/pc1-day3-a.md` and `docs/handoff/pc1-day3-c.md` (same answer; fix row `q-fix-p3-room-empty-think`, `q-fix-p3-leave-cancels-thinks` is its duplicate) (q-prov-data,

- [prov-data] **Monster think in a room with no clients** — answered → see `docs/handoff/pc1-day3-a.md` and `docs/handoff/pc1-day3-c.md` (same answer; fix row `q-fix-p3-room-empty-think`, `q-fix-p3-leave-cancels-thinks` is its duplicate) (q-prov-data,
27. **Order of the property rolls of a magic item** answered → see `docs/handoff/pc1-day4.md` (duplicate entry). (area G, `traces/checks/items-vendor-akara-buy.check`, 2026-10-09): the store's 7th item (guid 8, quality 4, prefix 1156, suffix 552, ilvl 6) has a charged-skill property (stat 204, skill 193, level 4, max 67). 1.14d current charges 67, d2rs 65: `properties.md` §5 rule 9 gives cur = (r + c/8 + 1) & 0xFF with r = roll(c − c/8) on the item seed, so r is 58 on 1.14d and 56 on d2rs, while the item seeds after creation are equal (`ik` equal). Same draws, different order. Read which property of the item rolls before the charged one (prefix and suffix properties, the 107 single-skill rows, the 45 row) and in which order the affix properties are applied (`affixes.md` §7: P0 S0 P1 S1 P2 S2?); answer into `items/affixes.md` §7 and `items/properties.md` §5.

- [prov-data] **Monster think in a room with no clients** (duplicate entry, answered above) (q-prov-data,
  `monsters/ai.md` §1.5, §2, `ai-bodies.md` §9.9 Map AI): after the
  player warps away (same act, `a4-warp-plains-ama`, warp 105 at frame
  6), the Fortress NPCs (classes 405, 257, 246; Npc AI `0x005E7130`)
  think at frame 24 (idle 20 from the frame-4 home think). d2rs runs
  the map AI (`lo' % 100` and `roll(count)`, 2 draws each) and their
  seeds change; 1.14d's seeds never change from frame 6 to frame 80.
  Read `0x005A7F80` / `0x005B1740` / `0x005E7130` for a room-client test
  (active room +0x78 = 0) that skips the think or the map AI, and what
  it schedules instead. The same think with a client in the level
  matches (`a4-fortress-arrival-ama`). Evidence: `traces/checks/a4-warp-plains-ama.check`.

52. **[q-fix-real-unit-seed-order] Client NPC stops a walk the server makes** answered in part (REC-1110 open) → see `docs/handoff/pc1-day4.md`.
  (`client/model.md` §19 r4 "NPC busy", monster data +0x28 bit 0):
  scenes-compare's panel run (SceSor, `-seed 1234`, inventory open at
  tick 22, an item taken to the cursor at 52, put down at 62): the 1.14d
  server walks Warriv (1:7) from tick 56 to 67 (state recording), but
  the 1.14d client draws him in NU from tick 56 (frame 0.5·(t − 56)), so
  the client ignored the 0x67 at 56. Without the panel input the client
  walks it. Read who sets the client's NPC-busy bit (+0x28 bit 0; only
  its clear, §17 r1.7, is specified) or what else makes the client's
  walk dispatch fall back to neutral here. Blocks a1-panel-character /
  skilltree / automap / esc-menu-wine (Warriv NU frame) in d2rs.

53. **[q-diff-combat-a1] preparation probe at the start (REC-753)** answered → see `docs/handoff/pc1-day4.md` (`sim/pathing.md` §4 r3).
54. **[q-diff-combat-a1] town NPC thinks while the player runs (REC-754)** answered → see `docs/handoff/pc1-day4.md` (same mechanism as `q-fix-p3-room-empty-think`).
55. **[pc1-day4] Local player attack / cast mode end and the next click (coordinator blocker 1)** answered → see `docs/handoff/pc1-day4.md` (`client/model.md` §20).
56. **[pc1-day4] Andariel never dies, Radament stays at 256 hp (playthrough act1 #14, act2 #6)** answered (setup, not damage) → see `docs/handoff/pc1-day4.md`.

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
31. **Door step 0x004BCB20** (q-fix-pc1-client-ui) Specify the object door step (REC-725): what it does per update for an `IsDoor` non-cycling mode (frame advance, mode change, collision / sound calls), for `world/objects-client.md` §25 r9.2.1.
32. **NPC introduction handler 0x004B41E0** (q-fix-pc1-client-ui) Read which text record the "introduction" topic plays (REC-727; d2rs plays record 0 of the intro entry) and whether it sets +0x11 or the talk flag like "gossip" (`messages.md` §6 r3 / OQ4).
33. **Gamble buy 0x32 u32@9** (q-fix-pc1-client-ui) menus.md §4.2 r2 says `[0x007C0DB0]` is only ever written with 0, so a buy in a Gamble window would send transaction 0; the server read (vendors.md §7.1 rule 2) accepts a gamble item only with transaction 2. Record or read what a real Gamble buy sends (C→S 0x32 u32@9) so q-fix-shop-gamble-flag-dead can be settled either way (REC-729; the click env keeps the OR 2 meanwhile).
41. **Skill button state 0x004A8D30** answered → see docs/handoff/pc1-day3-b.md. (q-fix-pc1-client-ui) Specify the icon state (0, 1 or 4) the control panel's skill buttons pass as the colored cel draw's `k` (REC-720; `control-panel.md` §7 r2: which conditions give 4, and the town / flag bit `[0x006CE268]` rule). d2rs draws the button icons with k 0.

## Hand-back — PC 1 day 2, 2026-10-09 (branch `claude/local-pc1-day2`)

**Done** (items 21–40; answers in the owner specs with addresses):
- 22: poke call forms for pos / warp / item / stat / state are in
  `poke.py` `CALL_FORMS` and the specs. Run once on 1.14d (PC 1): all
  five returned `ok` with no fault (REC-655; the level after `warp 2` was
  not logged).
- 30 / 34: town critters are client-made from the Levels.txt `cmon` /
  `cpct` columns, rolled on the client room seed, in a room pass before
  the unit update. Original bug: all four `camt` fields parse into one
  field. Client GUID counter: pre-increment from a .data 1, never reset.
  The critter AI walks 30% of the time. Specs: `client/model.md` §5 r6,
  `monsters/population.md` §11.7.
- 27: the nearest-player scan uses the full-size distance ≤ 15, the first
  qualifying player wins, and the `interact` quest gate applies. Walk in
  radius: own geometry, no early exit (REC-500 / 501 settled; d2rs
  differs).
- 21: owner links for a pet / hireling (AI control +0x2C / +0x30), a
  missile (+0x94 / +0x98 while +0xC8 bit 0x400 is set) and an item
  (inventory +0x08). Written to `sim/units.md` and `state-snapshot.md`
  §2.
- 29: 0x69 code 8 sends the path **end** (last path point, or (0, 0)),
  not the path target (REC-594 settled; d2rs and its tests differ).
- 35–40:
  - action-frame tests skipped with argument 0 (REC-700, matches);
  - only the frame advance writes +0x4E (REC-701, d2rs differs);
  - client leap / whirl machine (REC-702 / 704 match; REC-703 step
    matches, end rule open as REC-671);
  - meleeonly mask applies only to `both` → rng;
  - Evade only plays a sound;
  - a monster's missile weapon is the plain first weapon.
- 24: client timed arc ORs flag 2, the vz division form; motion getters
  return value >> 11 (REC-540 / 541 / 545 / 548 settled).
- 25: potion entry 3 body. 170 / 51 frames are the fill time at
  calc1·256·class factor / len; the regen tick frees the list when full
  (REC-102 settled).
- 28: the 0x8E join flag is the constant 1 (d2rs matches).
- 31: door step for opening / closing. 32: introduction record by class
  (Malah / Nihlathak / Qual-Kehk special cases), sets +0x11 like gossip.
- 33: a gamble buy sends t = 2. The handlers at `0x004B3D40` /
  `0x004B42B0` write the flag 1 / 0 (the earlier static export missed
  them). **`q-fix-shop-gamble-flag-dead` is withdrawn**; d2rs's OR 2 is
  right.
- 26: `traces/sim/tick/sim-0009.json` re-recorded with the mouse outside
  the window. The player stays in its arrival room (the old run changed
  rooms at 121 / 241 / 351); `convert_tick --check` reports 0 errors.

**Left**
- 23: run `traces/pokes/spawn-town.poke` with a screenshot (not asked in
  this run).
- Still PROVISIONAL:
  - REC-660: client GUIDs 2–92 (needs a runtime count);
  - REC-661: critter think-timer start;
  - REC-665: mode request to own position; answered → see docs/handoff/pc1-day3-b.md
  - REC-670: local input path for mode 18; answered → see docs/handoff/pc1-day3-b.md
  - REC-671: whirl end rule; answered → see docs/handoff/pc1-day3-b.md
  - REC-680: Blood Golem life share. answered → see docs/handoff/pc1-day3-b.md
  - REC-660: client GUIDs 2–92 (needs a runtime count); answered → see `docs/handoff/pc1-day3-a.md`;
  - REC-661: critter think-timer start; answered → see `docs/handoff/pc1-day3-a.md`;
  - REC-665: mode request to own position;
  - REC-670: local input path for mode 18;
  - REC-671: whirl end rule;
  - REC-680: Blood Golem life share.
- Not traced: the other critter handlers (rat, bat).

**New q-fix rows (build-queue.tsv)**
- **Combat:** `q-fix-p4-mon-frame-code`, `q-fix-p4-mon-missile-weapon`,
  `q-fix-p4-evade-sound-only`, `q-fix-p4-range-meleeonly`,
  `q-fix-p4-whirl-end`.
- **AI and world:** `q-fix-p3-npc-nearest-player`,
  `q-fix-p3-walk-in-radius`, `q-fix-p3-death-path-end`,
  `q-fix-real-town-critters` (now concrete), `q-fix-real-client-guid-start`,
  `q-fix-real-critter-ai`.
- **Items, missiles, UI:** `q-fix-p5-potion-entry3`, `q-fix-p5-timed-arc-or`,
  `q-fix-p5-motion-getters-shift`, `q-fix-p6-object-door-step`,
  `q-fix-p6-npc-intro-record`.
- **Tools:** `q-fix-p3-state-own`.
- **Withdrawn:** `q-fix-shop-gamble-flag-dead`.

- [q-fix-d9-arcane] Quest A2Q4 event 3 (`0x0059F0C0`): is the old-level-40 handling (start-Jerhyn removal, quick remove) skipped when the new level is 74? Recording `a2-warp-arcane-ama` keeps Jerhyn (class 201, 1:1) alive to frame 143 on 1.14d, every other warp from Lut Gholein removes him at the warp frame. d2rs skips the whole `a == 40` block for `b == 74` (REC-1405, PROVISIONAL); read the branch structure and fix `specs/world/quests-act2.md` §6.6 / `quests-act2-2.md` §2 item 3.
