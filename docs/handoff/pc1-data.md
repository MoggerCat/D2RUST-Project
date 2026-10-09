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
17. **Interact range test `0x00623660(P, O)`** (REC-94): its formula
    (object size, which positions). Measured under Wine
    (`facts/objects/objanim-a1-town.tsv` run r3): returns 1 for the
    waypoint 119 with the player 4 sub-tiles off in x and 3 in y, 0 at 5;
    for the stash 267 only at 3 / 1. Answer into `world/objects.md` §7.1
    r3 (d2rs reads it as always in range for a player: true for every
    0x13 the client sends).
