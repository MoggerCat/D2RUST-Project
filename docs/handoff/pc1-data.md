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
