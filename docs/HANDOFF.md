# Handoff (updated 2026-10-06, branch `claude/phase3-specs-index`)

Start here in a fresh session, after `CLAUDE.md`. This file holds state,
the next steps, the code and command map, and the local run queue. Rules
and facts live in specs (`specs/README.md`); this file points to them
rather than restating them.

## 1. State

| Phase | Status | Proof |
|---|---|---|
| 0 Setup | done | CI green on GitHub (`MoggerCat/MXL-ULTIMATE`) |
| 1 Formats | done | `mpq-tool check`, `mpq-tool formats` |
| 1b First pixels | done | `d2-client verify` (GPU = CPU reference, byte-exact) |
| 2 Data | done | `data-tool tables` (2026-10-05, with callbacks): 73 live tables, 72 byte-identical, 1 explained (`monstats` `NameStr`); 4/4 code buffers identical; `data-tool links`: 0 broken; `data-tool dump-compare traces/raw/20261006-021210-tables`: 70/70 tables and every map identical to 1.14d memory; `d2-data` game-file tests all pass (incl. `fixups_on_live_set`, `typed_tables_decode`, patch G1–G8) |
| 3 Simulation | in progress: RNG done; tick, unit-ordering and intents/events specs written and confirmed on recordings; units, stats, stat-lists specs written (branch `claude/phase3-units`; units confirmed on recordings, stats unverified until the queued recording); tick traces `traces/sim/tick/sim-0006`–`0008` committed; specs for items, treasure, skills/combat, DRLG, monsters, world in progress on `claude/phase3-<topic>` | `cargo test -p d2-sim -p conformance`: spec vectors pass; all 256 draws of `traces/sim/rng/*.json` replay exactly. `check_tick.py` / `check_packets.py` on the 2026-10-06 recordings: 0 mismatches; `check_units.py`: 0 errors on all three recordings (14,034 schedules checked exactly); `convert_tick.py --check traces/sim/tick/*.json`: 0 errors (CI) |
| 4 Conformance | recording proven feasible | `tools/trace-recorder`: 32,543 recorded RNG draws match the spec exactly |
| 5–6 | not started | |
| 7–9 | deferred (out of current scope) | |

## 2. Next steps (in order)

1. **Phase 3 specs, remaining topics** (spec writing / RE, local, high):
   writers run in parallel, one branch each: `claude/phase3-items`,
   `-treasure`, `-skills`, `-drlg`, `-monsters`, `-world`. The local
   coordinator batches their Ghidra and recording requests (§5). Units and
   stats are done on `claude/phase3-units`: `specs/sim/units.md` (+
   `unit-events.tsv`, `unit-handlers.tsv`), `sim/stats.md` (+
   `stat-ops.tsv`), `sim/stat-lists.md`.
2. **Implement the tick core** (implementation, medium; cloud): `d2-sim::tick`
   timer queue and step order (`tick.md`), `d2-sim::units::lists`
   (`unit-order.md`), `d2-proto` ids and sizes (`intents-events.md`).
   Replay `traces/sim/tick/sim-0006`–`0008` through them as the
   conformance test (`tick.md`, Test vectors, "Trace sim/tick": the
   replay rule); `crates/conformance/tests/tick_traces.rs` already checks
   their shape.
3. **Implement units, stats, stat lists** (implementation, medium; cloud,
   after step 2): from `sim/units.md`, `sim/stats.md`, `sim/stat-lists.md`
   (+ TSVs). Stats stay unverified until the stats recording (§5) passes.

## 3. Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-formats/src/mpq/` | MPQ archive (`mod.rs`), hash/block tables (`tables.rs`), crypto, PKWARE explode, Huffman, ADPCM, `ArchiveSet` (`set.rs`) | `formats/mpq.md`, `mpq-tables.md`, `data/loading.md` §2 |
| `crates/d2-formats/src/{palette,dc6,dcc,dt1,ds1,cof,tbl,font,animdata}.rs` | file formats | `specs/formats/*.md` |
| `crates/d2-data/src/txt.rs` | strict `.txt` reader, column binding | `data/txt-format.md` |
| `crates/d2-data/src/schema.rs` | embedded `fields.tsv` / `tables.tsv` | `data/schema.md` |
| `crates/d2-data/src/compile.rs` (+ `compile/tests.rs`) | cell → bytes, linkers, `strkey`/`calc`/`param` | `data/field-types.md` |
| `crates/d2-data/src/compile/callbacks.rs` (+ `callbacks/tests.rs`) | cube, skill-mode, composit, place callbacks; `@uniques`/`@sets` | `data/callbacks.md` |
| `crates/d2-data/src/codegen.rs`, `tables/` | typed record structs: generator, generated `tables/generated.rs` (don't edit; `data-tool gen-tables`), `Record`, `decode_all` | `data/schema.md` |
| `crates/d2-data/src/links.rs` (+ `links/tests.rs`) | cross-reference validation: linker sizes, broken links by table/row/column | `data/field-types.md` §6.7 |
| `crates/d2-data/src/fixup.rs` (+ `fixup/{records,maps,text,qsort,tests}.rs`) | post-load fix-ups (`records`), runtime maps (`maps`), wide text and tile paths (`text`), the CRT `qsort`; on a copy of the loaded set; `apply(&BinSet, &AnimData)` | `data/fixups.md`, `data/runtime-maps.md` (summary `loading.md` §7.4) |
| `crates/d2-data/src/calc.rs` (+ `calc/tests.rs`) | formula compiler, code buffers | `data/calc-expressions.md` |
| `crates/d2-data/src/compile_set.rs` | all tables in load order | `data/loading.md` §6–7 |
| `crates/d2-data/src/bin.rs` | `.bin` container, live-file resolution, load checks | `data/loading.md` §3–4, §8 |
| `crates/d2-data/src/crosscheck.rs` | txt → bin byte comparison | `data/loading.md` §11 |
| `crates/d2-data/src/patch.rs` (+ `patch/{syntax,apply,diff,check,tests}.rs`) | mod patch layers: parse, apply, render/digests, diff, patched compile (branch `claude/patch-layers`) | `data/patch-layers.md` |
| `crates/d2-data/src/strings.rs` | string tables, `strkey` | `field-types.md` §7 |
| `crates/d2-client/src/map/` | DS1+DT1 map assembly, CPU reference renderer | `render/map-preview.md` |
| `crates/d2-client/src/{app,assets,render}` | Bevy app, `mpq://` assets, palette shader | `render/map-preview.md` |
| `crates/d2-sim/src/rng.rs` | seeded RNG: `Seed`, draw helpers, `derive`, `time_value` | `sim/rng.md` |
| `crates/conformance/src/{trace,rng}.rs` (+ `tests/rng_traces.rs`) | trace loading and top-level checks; RNG trace replay | `traces/FORMAT.md`, `sim/rng.md` |
| `d2-proto`, `d2-net`, `d2-server`, `d2-verify` | stubs | |
| `tools/mpq-tool` | info, list, extract, check, formats, render | |
| `tools/data-tool` | `tables`: the Phase 2 cross-check; `links`: broken links in the live set; `gen-tables`: regenerate typed structs; `dump-compare`: fix-ups vs a 1.14d memory dump; `patch check/render/diff`: mod stacks | `data/field-types.md` §6.7, `data/fixups.md`, `data/runtime-maps.md`, `data/patch-layers.md` §10 |
| `tools/trace-recorder` | Python debugger recording RNG draws from `Game.exe` (Windows); `dump_tables.py`: the excel tables and runtime maps in 1.14d memory after the load; `record_tick.py` + `check_tick.py`: server tick, timer events, unit lists; `record_packets.py` + `check_packets.py`: client↔server messages | `sim/rng.md`, `traces/FORMAT.md`, `data/runtime-maps.md`, `sim/tick.md`, `sim/unit-order.md`, `sim/intents-events.md` |
| `tools/trace-recorder/convert_tick.py` | tick recording → format-1 trace `traces/sim/tick/sim-NNNN.json`; `--check` replays traces through the `check_tick.py` model (CI) | `sim/tick.md` Test vectors |
| `tools/trace-recorder/{check_units,record_stats,check_stats}.py` | per-kind event rules U1–U11 on tick recordings; stat-list recorder (`stats-raw-1`) and checker | `sim/units.md`, `sim/stat-lists.md` |
| `tools/depcheck` | dependency rules (no Bevy outside `d2-client`) | |
| `tools/methods.py` | methods collection `docs/METHODS.md`: `check` (CI), `list`, `new`, `export` | `docs/METHODS.md` |
| `tools/spec_index.py` | section indexes in specs (`--check` in CI) | `specs/README.md` |
| `tools/cloud-setup.sh` | cloud session setup (Linux libs, pinned Rust) | |
| `tools/ghidra/` | Ghidra scripts (label import, export); `disasm.py`: disassembly, xrefs, whole-binary dump (the decompile drops register arguments) | |

## 4. Command map (what proves what)

| Command | Proves | Needs |
|---|---|---|
| `cargo test -p <crate>` | unit tests from spec vectors | repo |
| `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo fmt --all` | lint/format gate | repo |
| `cargo run -p depcheck` | crate dependency rules | repo |
| `py tools/spec_index.py --check` | spec indexes current | repo |
| `cargo test -p d2-data -p d2-formats -- --ignored` | game-file tests | `game/` |
| `cargo run --release -p data-tool -- tables` | every live table and code buffer reproduced from `.txt` | `game/` |
| `cargo run --release -p data-tool -- links` | no broken link in the live `.bin` set | `game/` |
| `cargo test -p d2-data --test patch_game -- --ignored` | patch layers on the live tables (G1–G8) | `game/` |
| `cargo run --release -p mpq-tool -- check` / `formats` | every archive block / every format file decodes | `game/` |
| `cargo run --release -p d2-client -- verify` | GPU render byte-identical to CPU reference | `game/`, GPU |
| `py tools/trace-recorder/record_rng.py --seconds N` then `check_rng.py` | RNG spec matches the real game | `game/`, Windows |
| `py tools/trace-recorder/record_tick.py --seconds 200` (play by hand) then `check_tick.py <file>` | tick steps, timer queue and unit lists match `sim/tick.md`, `sim/unit-order.md` | `game/`, Windows, a player |
| `py tools/trace-recorder/record_packets.py --seconds 180` (play by hand) then `check_packets.py <file>` | message transport rules R1–R7 of `sim/intents-events.md` | `game/`, Windows, a player |
| `convert_tick.py --check traces/sim/tick/*.json` | committed tick traces replay through the spec model, round trip exact | repo |
| `check_units.py <tick file>`, `check_stats.py <stats file>` | unit event rules / stat-list rules on a recording | a recording |
| `check_tick.py --selftest`, `check_packets.py --selftest`, `convert_tick.py --selftest`, `check_units.py --selftest`, `check_stats.py --selftest`, `tools/ghidra/disasm.py selftest` | the checkers catch perturbations (M08) | repo (disasm: `game/`) |
| `py tools/trace-recorder/dump_tables.py` (~7 s) then `cargo run --release -p data-tool -- dump-compare traces/raw/<time>-tables` | d2rs fix-ups and maps equal 1.14d memory after the load | `game/`, Windows |

Tools read `D2_GAME_DIR` (= `<repo>/game`). If a shell doesn't have it:
`export D2_GAME_DIR="$PWD/game"`.

## 5. Local run queue

Cloud sessions add game-file checks here (command + what to look for);
a local session runs them, records the result, and removes the entry.

Done 2026-10-06 (local): fix-ups (`dump-compare
traces/raw/20261006-021210-tables`: 70/70 tables, every map with a d2rs
counterpart identical, nothing pending; `fixups_on_live_set` ok) and
patch layers (`patch_game` 5/5 incl. G1–G8; `data-tool patch check
game/patch-example/overhaul.d2stack`: exit 0, one N01 note, data digest
`66010ecda7c8df5b7135579888c536fd2a30287fb877848719a31b6c8f97a625`).

Phase 3 recordings: done 2026-10-06 (first skipped at the user's
request, then recorded when the user asked). Combat with missiles
`traces/raw/20261006-022304-tick.jsonl` (4,632 ticks, 24,894 timer runs
incl. 1,580 missile, 186 snapshots) and melee `...-021854-tick.jsonl`:
`check_tick.py` 0 errors. Messages `...-022633-packets.jsonl`:
`check_packets.py` OK, perturbation reported, new client ids 0x0C,
0x3A, 0x3B. Still open: a hosted (multi-client) game.

**Phase 3 units/stats recordings** (local, needs the user at the game;
one session, batched with the topic writers' requests):
1. Units: `py tools/trace-recorder/record_tick.py --seconds 240` (record_tick
   0.2.0) on Normal: melee and skill combat, a shrine, a well, a vendor or
   trade, a cooldown skill. Then `check_units.py` and `check_tick.py` on
   the file: 0 errors each, "U4 anim exact" and "U11 site rule" counts
   above 0. Repeat on Nightmare: AI delays use the Normal `aidel` column
   unless game +0x6A / +0x74 is set (`units.md` OQ7). One recording near
   trap objects (with `record_rng.py`) for the seed of object event 0/8
   delays.
2. Stats: `py tools/trace-recorder/record_stats.py --seconds 240`: equip and
   unequip a +vitality/+life item and a weapon, drink a health and a mana
   potion, spend a stat point, get poisoned or use a timed buff, fight,
   walk and run. `check_stats.py <file>`: 0 errors, callbacks, expiry
   frees and regen checks all above 0; `--perturb-snap 5` and
   `--perturb-cb 0` must fail at the changed record. The max-life
   rescale sets must match too (`stat-lists.md` §7.2).

**Treasure** (`specs/items/treasure.md`, branch `claude/phase3-treasure`):
1. Memory dump (no player needed; extend `dump_tables.py` per M10): the TC
   array `[0x96C5EC]` (count `[0x96C5F0]`, 0x2C bytes each, entry list at
   +0x28 with count × 0x1C) and the chest table `0x96C5F4` (45 dwords);
   compare with `treasure.md` §1.
2. Recording (player): `record_rng.py` extended with hooks at `0x0055A6D0`
   entry (TC index = (ptr − [0x96C5EC]) / 0x2C, quality, level, mode),
   `0x0055AEE7` (item id ECX, quality, index, flags) and `0x0055A9B9`
   (nodrop, total, n, result EAX, FPU control word); kill monsters incl.
   champions and uniques with `players 8`, open chests. Draws in
   `0x0055A6D0`–`0x0055AF80` and from `0x00558640` must match
   `treasure.md` §Randomness in order and count.

**Phase 3 topic recordings** (player; one session, batched; recorder
extensions per M10 first; exact hooks in each spec's open questions):
items R1 (item creation: seed sets `0x00552DF0`/`0x00552E90`, every draw
on the item's unit and item seed until `0x00558D90` returns, dump of the
result); skills (hit/block `0x0057DB61`, `0x0057E04B`; damage
`0x0057DBF0`, `0x0057C6C0`; leech `0x0057C420`; mana `0x0056BFE0`;
start/do on an ally `0x0056FAF0`, `0x0056F7F0`; Strafe/Zeal frame codes);
monsters (new Normal game, Blood Moor and Cold Plains incl. a champion
pack, seed-step hooks listed in `population.md`/`init.md`, monster x/y
and type flags; Nightmare melee, freeze, knockback, Fallen Shaman
resurrect; missile exits `0x005AE1F0`, `0x005ADF10`); world (Flavie chat
RNG + packets; game start mode at `0x00546270`; act transitions via
Warriv/Meshif, 0x61; Inifuss scroll 0x50; waypoint to another act and an
undiscovered waypoint twice); vitals (hook `0x00570880` entry/exit and
`0x00570D60`, stats 4–13 before/after, one level-up and stat spending);
DRLG (full RNG hook: Den of Evil, Cave 1, Acts 2–4 with drlg +0x94,
+0x484, +0x474; entry returned by `0x0066D820`; `record_tick` extended
with each active room's DRLG rect, near list, status/counts, client
count and counter, `rooms.md` Test vectors).

**Cross-spec fixes reported by writers** (apply on the owning spec with
evidence): `rng.md` §5.3 (item seed re-init after a failed quality
routine; four missile seed sites are missile init callbacks), §7 (item
base stats and low-quality durability use the item's unit seed; monster
population draws mostly from the active room seed, only density/sparse
rolls from the game seed); `calc-expressions.md` §3.5 (`skill(s,c)`
without an entry returns the level-0 special value; OQ5/OQ8 answered by
`skills/levels.md`); `server-messages.tsv` (0x63, 0x28, 0x5D, 0x50, 0x91
per `world/quests.md`; 0xAC per `monsters/init.md` §24); `tick.md` §4
(`0x0052D0F0`), §5.6 (thinks dropped by the freeze gate are never
rescheduled); `units.md` (pointers from the skills and monsters specs);
`client-messages.tsv` 0x3A (byte +1 stat id ≤ 15, byte +2 count − 1 ≤ 99,
not `stat:u16@1`, `combat/vitals.md`); `rng.md` §7 DRLG rows (level seed
for `0x0066F690`/`0x0066F990`, room-seed sites `0x0066D820`, `0x00670170`,
`0x006706D7`, maze draws `0x006711A0`–`0x00673EC9`, `drlg/rooms.md`);
`unit-order.md` §9/OQ2 → `drlg/rooms.md` §6, OQ3 → units/monsters;
`tick.md` OQ4 → `drlg/rooms.md` §7; `ds1.md` OQ1/OQ4 closed (bytes never
read), OQ3 (truncated `trees.ds1` group: keep 0). Carried-over question 1
below: the server tile build does not hide those tiles; the cause is in
the client draw path.

Next RNG capture when convenient (local, needs the user at the game):
start `py tools/trace-recorder/record_rng.py --seconds 120`, enter a
single-player game, kill a few monsters and pick up a drop. Recording
slows the game a lot (all 846 inline RNG sites are hooked).

## 6. Environment

- Developer PC: Windows 10, Git, rustup (default and pinned 1.99.0),
  VS 2022 Build Tools, Temurin JDK 21, Ghidra 12.1.4 at
  `%LOCALAPPDATA%\Programs\ghidra_12.1.4_PUBLIC`, Python 3.10 (`py`),
  GitHub CLI (`gh`, logged in as MoggerCat). Disk is tight: build only the
  crates you need.
- `game/`: 1.14d install (`Game.exe` 1.14.3.71), gitignored. Extracted
  tables in `game/extracted/{patch_d2,d2exp,d2data}/` (scratch).
- `re/`: Ghidra project `re/ghidra/D2_114d.gpr` and exports
  (`re/exports/functions.tsv`, `re/exports/funcs/`), gitignored. How to
  regenerate: `tools/ghidra/README.md`.
- `../refs/`: D2MOO (MIT), Riiablo (Apache-2.0), CE_Database,
  1.14d-notes. Spec sessions only.
- Git: `main` on `git@github.com:MoggerCat/MXL-ULTIMATE.git`. CI runs on
  pushes to `main` and on pull requests. A cold CI run takes ~40 min
  (Bevy), cached ~3 min.

## 7. Where facts live

Simulation core: `specs/sim/tick.md` (tick rate, steps, timer queue; the
sim/tick trace format), `sim/unit-order.md` (GUIDs, unit/room/client
lists), `sim/intents-events.md` (+ `client-messages.tsv`,
`server-messages.tsv`). Units: `sim/units.md` (kinds, lifecycle, modes,
what each timer event does per kind; `unit-events.tsv`,
`unit-handlers.tsv`). Stats: `sim/stats.md` (ids, values, ops;
`stat-ops.tsv`), `sim/stat-lists.md` (lists, modifiers, states, regen).
These are on `claude/phase3-units` until merged. Treasure classes and
drops: `specs/items/treasure.md` (+ `treasure-quality.tsv`,
`treasure-chest-acts.tsv`; branch `claude/phase3-treasure`). Items:
`specs/items/generation.md`, `quality.md`, `affixes.md`, `properties.md`
(+ `property-functions.tsv`; `claude/phase3-items`). Combat and skills:
`specs/combat/hit.md`, `damage.md`, `specs/skills/levels.md` (+
`skillcalc.tsv`, `misscalc.tsv`), `use.md` (+ `functions.tsv`;
`claude/phase3-skills`; `combat/vitals.md`: creation, stat points,
level-up, experience table). Monsters and
missiles: `specs/monsters/population.md` (+ `preset-monsters.tsv`),
`init.md` (+ `umods.tsv`), `ai.md` (+ `ai-functions.tsv`),
`specs/missiles/missiles.md` (+ `srvdo.tsv`, `srvhit.tsv`;
`claude/phase3-monsters`). World: `specs/world/quests.md` (+
`quests.tsv`, `quest-messages.tsv`), `waypoints.md` (+ `waypoints.tsv`;
`claude/phase3-world`; cube, NPC, vendors in progress). Level generation:
`specs/drlg/levels.md`, `rooms.md` (rooms-near order, adjacency arrays,
the deactivation counter = `tick.md` OQ4), `preset.md` (+
`preset-tables.tsv`), `maze.md` (+ `maze-specials.tsv`), `outdoor.md`,
`outdoor-tilesub.md` (`claude/phase3-drlg`). All draft: rules
from the 1.14d disassembly, RNG draw order not yet checked on a trace. Format facts: `specs/formats/*`. Map rendering: `specs/render/map-preview.md`.
Data loading and tables: `specs/data/*` (start at `loading.md`). RNG:
`specs/sim/rng.md`. Each spec's "Open questions" holds its unknowns.
Carried-over open questions not yet in a spec's list:

1. 8 unflagged invisible collision tiles in `townN1.ds1` draw as blue
   patches (`map-preview.md` OQ3; needs RE of the client tile draw path).
2. DS1 v12/13 trailing bytes (possibly an early NPC-path section).
3. DC6/DCC vertical placement (one-row disagreement between sources).
4. Meaning of the PL2 rendering tables (Phase 6).

## 8. Lessons (problems met, fixes)

| Problem | Fix |
|---|---|
| Ghidra 12.1 crashes on JDK 25 | use JDK 21 (`JAVA_HOME_OVERRIDE` in Ghidra's `launch.properties`) |
| Cargo can't extend workspace lints | `d2-sim` repeats them in its own `[lints]` |
| Running `cargo` outside the repo reinstalled `stable` (1.3 GB) | default toolchain set to 1.99.0 |
| `.txt` tables in the MPQs aren't what 1.14d uses | `.bin` is truth; the txt compiler is proven against it (`loading.md`) |
| 1.14d inlines its RNG step (846 sites) | recorder hooks every site, not one function (`sim/rng.md`) |
| A 24-agent review workflow cost ~6M tokens for little gain | one writer per spec + executable checks (`CLAUDE.md` working rules) |
| Two parallel cloud branches (fix-ups, patch layers) compiled alone but not together: `fixup::apply` gained an `AnimData` argument while `patch_game.rs` was written against the old one (caught by the coordinator's merge gate, 2026-10-06) | the coordinator merges parallel branches together and runs the full gate before opening the PR; a session changing a public signature names it in its handoff (METHODS M21 escape log) |
| The Ghidra decompile drops register arguments (fastcall ECX/EDX, custom conventions) | read register use from the disassembly: `tools/ghidra/disasm.py` |
| Recording a game needs a player: the game never enters a game by itself | ask the user to play during the recording (~3 min) or queue it |
| A local session reported a push as done; the push had been rejected (local and remote branch names differ) and the command's last output line hid the error (2026-10-06, caught by the coordinator reading the remote) | push with an explicit remote branch (`git push origin HEAD:claude/<name>`) and verify by reading the remote (`git status` not ahead, or `git ls-remote`), never by the command's output (METHODS M09, M21) |
| GPU render exactness | R8Uint indices, sRGB palette via `textureLoad`, `Msaa::Off`, `Tonemapping::None`, pixel-aligned quads |
