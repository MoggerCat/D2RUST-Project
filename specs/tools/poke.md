# Spec: Tools — Poke (set up game state for a check, on 1.14d and d2rs)

- **Status:** draft: format `poke 1` and the directive table are ours;
  every 1.14d address and argument form below is cited from the owning
  spec (the forms of `pos`, `warp`, `item`, `stat`, `state` read from
  the asm, not yet run: REC-655). Run on 1.14d under Wine
  (2026-10-09, `poke.py` 0.1.0, `--auto ScnAma --seed 1234`, F0 = 2):
  `spawn` normal (GUID 8), `seed-unit`, `object` (GUID 18), `time`,
  `seed-game`, `superunique` (GUID 8), `missile` (GUID 1) all returned
  `ok` with no fault (`traces/pokes/spawn-town.poke`,
  `missile-superunique.poke`); scenario `spawn` kinds `champion`
  (GUID 8), `random-boss` (GUID 13), `unique` with umods 5, 7 (GUID 17)
  returned `ok` with minions (`boss-kinds.poke`, monsters/init.md §25
  forms). The comparison with d2rs is REC-590. The d2rs side (§3, §5) is implemented
  (`d2-sim::poke`, scenario `poke` steps, `scenario-run`, `d2-client
  play --poke`): every directive runs on the synthetic install; `warp`
  to another act runs the act change (§1 table); `item` without item
  tables is a d2rs gap.
- **Target version:** 1.14d (the original side); the format is d2rs-own.
- **Crate/module:** `d2-sim::poke` (directives, parser, d2rs apply);
  `conformance::scenario` (`poke` steps); `tools/scenario-run`;
  `d2-client` `play --poke`; `tools/trace-recorder/poke.py` (1.14d side,
  Python; spec-role tool).
- **Related specs:** `tools/scenario.md` (steps, run model, references,
  comparison), `traces/FORMAT.md` §Scenario traces (the `poke` record),
  `tools/original-hooks-spawn.md` (the call procedure §5 and monster
  entries §1), `tools/original-hooks.md` §3 (tick boundary), §4 (unit
  hash lists), `sim/units.md` §3.1 (allocator), `missiles/missiles.md`
  §R2 (missile record), `sim/rng.md` §4–§5 (seeds),
  `render/lighting.md` §9.1 (environment record),
  `world/waypoints.md` §7, §11 (warps), `tools/test-variants.md`
  (patched installs: what a poke cannot set).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 54–66 |
| Inputs | 67–73 |
| Outputs / state changes | 74–82 |
| Rules | 83–84 |
|   1. Directives | 85–119 |
|   2. Poke files | 120–142 |
|   3. In scenarios | 143–157 |
|   4. The 1.14d side (`poke.py`) | 158–206 |
|   5. The d2rs side (`d2-sim::poke`) | 207–221 |
| Constants & data dependencies | 222–234 |
| Randomness | 235–241 |
| Edge cases & original bugs | 242–251 |
| Test vectors | 252–263 |
| Provenance | 264–269 |
| Open questions | 270–277 |
<!-- /index -->

## Summary

A check that needs a monster next to the player, a missile in flight, a
unit seed or a time of day should not walk through the game to get
there. A **poke** is one directive that writes that state directly at
a chosen tick, through the game's own creation functions or by writing
the field the game itself keeps the state in. The same directive runs
on 1.14d (a debugger calls the function or writes the field between two
server ticks) and on d2rs (a debug entry point of `d2-sim` calls its
own creation path), so the two sides start the comparison from the same
state. Saves (`d2s-tool`) set the character; test variants
(`tools/test-variants.md`) set the tables; pokes set everything else.

## Inputs

| Name | Type | Source |
|---|---|---|
| poke file | text, §2 | `traces/pokes/<name>.poke` (committed), or the `poke` steps of a scenario (§3) |
| game | 1.14d under the debugger, or a d2rs game | `poke.py`, `scenario-run`, `d2-client play` |

## Outputs / state changes

The directive's state change, made between ticks t − 1 and t (§2 rule
5). Each directive yields a **result**: `ok` (with the GUID of a unit it
created), `failed` (the game's own function refused: placement, class
check), `unresolved` (a reference matched no unit) or `gap` (this side
cannot run the directive, §4). Results are written as `poke` records
(§3 rule 3).

## Rules

### 1. Directives

1. One directive per line: a keyword and its arguments, separated by
   runs of spaces and tabs. Numbers as `scenario.md` §2 rule 4 (decimal
   or `0x` hex; signed fields take `-`). Coordinates are absolute
   sub-tiles. `<ref>` is a unit reference (`scenario.md` §3 rule 3:
   `@player`, `@<type>[:<class>][#n]`, `@wp[#n]`) or a decimal GUID with
   type, `<type>/<guid>`; positions may be `@x±N` / `@y±N`.
2. The directives (`d2rs` column: the `d2-sim` path `d2-sim::poke` calls;
   1.14d column: the call or field write `poke.py` makes, §4):

<!-- rows -->
| Directive | Arguments | Effect | 1.14d | d2rs |
|---|---|---|---|---|
| `object` | `<class> <x> <y> [mode <m>]` | create an object (objects row), mode default 0 | allocator `0x00555230` with type 2, flags 1 (`sim/units.md` §3.1 steps 1–8; `world/objects-2.md` §22 r4: mode is the sixth stack argument) | `View::create_object` (the creation the game's own objects take: population, quests): the allocator with the per-kind init (`world/objects.md` §3: control record, InitFn on the control seed, at the allocation's room and (x, y)) before the add (`sim/units.md` §3.1 steps 7–8); without object state, or a class past the `objects` rows, `View::allocate` |
| `superunique` | `<row> <x> <y>` | create a superunique (superuniques row) and its minions | entry 4 `0x005A49B0` (`original-hooks-spawn.md` §1, §3) | `monsters::population::preset::superunique` (`population.md` §11.4) |
| `missile` | `<class> <x> <y> <tx> <ty> [skill <id> <level>] [owner <ref>]` | create a missile at (x, y) aimed at (tx, ty); owner default `@player` | `0x0059FA30` (ECX game, EDX record; `original-hooks.md` §7.1): record +0x00 flags = 0x21 (position given, target absolute), +0x04 owner, +0x08 origin = owner, +0x10 class, +0x14/+0x18 x, y, +0x1C/+0x20 tx, ty, +0x2C skill, +0x30 level (`missiles.md` §R2.1); other fields 0 | `missiles::create::create_missile` with the same flags and fields |
| `seed-game` | `<lo> <hi>` | set the game seed | write u32 lo, hi at game +0xD0 (`rng.md` §5.2; `original-hooks-spawn.md` Constants) | `ActionHooks.game_seed.set(lo, hi)` |
| `seed-unit` | `<ref> <lo> <hi>` | set a unit's seed | write u32 lo, hi at unit +0x20, +0x24 (`rng.md` §5.3; `original-hooks.md` §4) | the unit record's `seed.set(lo, hi)` |
| `time` | `<period 0..5> <ticks>` | set the time of day of the player's act | write the environment record (act +0x04; acts at game +0xBC + 4·act): +0x00 period, +0x08 ticks (`render/lighting.md` §9.1) | `ActEntry.environment` `period`, `ticks` |
| `pos` | `<ref> <x> <y>` | teleport a unit | `0x00650BE0` (stack: unit path, unit, room, x, y; `ret 0x14`; 1/0; `path-placement.md` §6 r4), else `0x00554EA0` (ECX game, EDX unit; room, x, y, exact 1, alt 0; `ret 0x14`; 1/0; §10); room by entry 6 from the unit's room | `WalkCtx::teleport` (`path-placement.md` §6 r4 teleport path) |
| `warp` | `<level> [tile <n>]` | move the player to a level (tile index default 0) | `0x0053AEC0` (ECX game, EDX player; level, tile; `ret 8`; EAX not a status: `ok`; `world/waypoints.md` §7 r5) | `wiring::path::place::level_warp` (`path-placement.md` §11); another act: the act change `wiring::path::act_change::run` (`world/waypoints.md` §11 steps 1–19, `flows/act-change.md` §1; step 3 builds the act's DRLG when missing), as waypoint travel to another act; the client's 0x04 follows on the next tick's client pass |
| `item` | `<code> <x> <y> [quality <q>] [ilvl <n>]` | create an item on the ground | `0x00558D90` (ECX game, EDX request; use seed 0; `ret 4`; EAX the item; `items/generation.md` §3) with request spawn mode 3, x, y, room (entry 6), item = the combined index of the code (§4 rule 8) | `ItemUnits::create_item` (spawn mode ground) then `items::moves::ground::ground_place` |
| `stat` | `<ref> <stat> <layer> <i32>` | set a base stat | `0x00627260` (stack: unit, s, value, layer; `ret 0x10`; `sim/stat-lists.md` §5 r2) | `StatLists::unit_set` (`stat-lists.md` §5 r2) |
| `state` | `<ref> <state> on\|off` | set or clear a state | `0x00639DB0` (stack: unit, s, 1/0; `ret 0xC`; `stat-lists.md` §9.2) | `toggle_state` + `set_state_changed` (`stat-lists.md` §9.2 toggle with update-queue insert) |
| `freeze` | `<seconds>` | hold the game between ticks for wall time (screenshots, a human look) | the debugger keeps the thread stopped at the hook (§4 rule 2) | no-op, `ok` (the runner owns the clock) |

3. Monsters keep the scenario `spawn` step (`scenario.md` §3.1); a poke
   file writes it as `spawn <class> <x> <y> <kind> [umod <id>...]` with
   the same meaning.
4. A directive creates or changes only what its row says. It draws no
   RNG of its own; the game function it calls draws as it would in play
   (`original-hooks-spawn.md` §4 for monsters). A field write draws
   nothing.

### 2. Poke files

1. `traces/pokes/<name>.poke`: UTF-8, LF, committed; `<name>` matches
   `[a-z0-9-]+`. Comments and blank lines as `scenario.md` §2 rule 1.
2. First line that is not blank or a comment: `poke 1` (the format
   version; a reader rejects any other).
3. Every other line: `at <tick> <directive> <args>...` (§1) or
   `at <tick> spawn ...` (§1 rule 3). Ticks are non-decreasing; lines
   with the same tick run in file order.
4. Ticks are relative: tick 0 is the first server tick after client 0
   reaches state 4 (`scenario.md` §4 rule 2). A poke at tick t runs
   after frame F0 + t returned and before frame F0 + t + 1 runs, i.e.
   at (a) of `scenario.md` §4 rule 2, before that tick's messages are
   drained.
5. Unknown keywords, missing or extra arguments and out-of-range values
   are errors naming the line; nothing is defaulted beyond §1.
6. **Absolute form** (command lines and `.check` files of
   `tools/scenario-diff.md` §2): `--poke "<f> <directive> <args>..."`,
   repeatable, where f is the absolute server frame (game +0xA8). It is
   applied after frame f − 1 returned and before frame f's drain: on
   1.14d at the `0x0052FD1E` hit whose game +0xA8 = f − 1. Directives
   and results are those of §1; the record carries `f` instead of `t`.

### 3. In scenarios

1. A scenario step `at <tick> poke <directive> <args>...` runs one
   directive (§1) at (a) of `scenario.md` §4 rule 2, in script order
   with `msg`, `hex` and `spawn` steps. It counts in the step index `i`.
2. The canonical form writes the directive and its arguments as parsed,
   numbers decimal, optional arguments in the table order.
3. Each poke step writes a `poke` record (`traces/FORMAT.md`
   §Scenario traces): `t`, `i`, `d` (the directive keyword) and `r`
   (`ok`, `failed`, `unresolved`, `gap`), plus `guid` when `ok` created a
   unit. It belongs to the `c2s` stream (compared there, after `spawn`
   in the order of `scenario.md` §5 rule 3).
4. A `gap` result on either side is also listed in that side's header
   `gaps` (`poke <d> at <t>`), so the verdict is at best `partial`.

### 4. The 1.14d side (`poke.py`)

1. **Base:** `record_tick.py`'s `TickRecorder` (debugger, autostart,
   tick hooks), plus the tick-return hook `0x0052FD1E` (bytes
   `8B 76 18`, ESI = game; `original-hooks-spawn.md` §5 rule 2).
2. **When:** at the hit of `0x0052FD1E` whose game +0xA8 equals
   F0 + t (the frame just run), run every directive of tick t in order.
   F0 is the frame of the tick in which client 0 first reached state 4
   (`original-hooks.md` §1 rule 5).
3. **Calls:** the procedure of `original-hooks-spawn.md` §5 rules 3–8
   (scratch page with an INT3 return trap, saved context, arguments on
   the stack above the return address, registers as the function's
   call form says, rule 8). After
   the return trap the saved context is restored before the next call,
   so a callee's `ret N` need not be known.
4. **Field writes:** `WriteProcessMemory` at the hook, on the
   addresses of §1.
5. **Pointers:** game (ESI at the hook); player (`original-hooks-spawn.md`
   §2 rule 2); room of a point (entry 6 `0x00463740` from the player's
   room, `original-hooks-spawn.md` §2 rules 3–4; 0 → `failed`); a unit
   by reference (hash lists, `original-hooks.md` §4 rule 1).
6. **Results:** EAX of the call (0 → `failed`), or `ok` for a field
   write. Created units: the GUID at unit +0x0C.
7. A directive whose 1.14d column says gap is not run: result `gap`.
8. **Call forms.** Every function a directive calls is one entry of a
   table in `poke.py` (`CALL_FORMS`): its address and the arguments
   `poke.py` supplies by name, both cited from the owning spec, and its
   form: which argument goes in which register (any of EAX, EBX, ECX,
   EDX, ESI, EDI) and which on the stack, in order from [ESP+4], each
   placed exactly once (literals allowed), plus the callee's `ret N` and
   what EAX means (created unit, 1/0, or ignored) where the entry leaves
   it open. An entry without a form is a gap: every directive that calls
   it returns `gap` naming the function, its address and the
   `docs/handoff/pc1-data.md` item that asks for it, and nothing is
   called. Record offsets and table addresses the directives use
   (item request format source, combined items array, umod list) are a
   second table (`FIELDS`), with the same rule for a missing value.
   Filling a form once its owning spec states it is the whole change:
   the directive then runs as rule 3 says. A run may override both
   tables from a JSON file (`--forms`, format `poke-forms-1`) to try a
   form before it is committed; the run's output records the forms in
   force. `pos` calls the teleport `0x00650BE0` (d2rs' path) when it
   has a form, else the placement `0x00554EA0`; the spawn kinds' `0x005A09E0`, `0x0054E1E0`, `0x005A2120`
   and the umod list (monster data +0x1C) are stated in
   `monsters/init.md` §25.1, §25.3. `item` finds the item index in the
   combined items array (`items/treasure.md` §9.1: header `0x0096CA58`,
   count, records; code at record +0x80, `data/loading.md` §6–§9), not
   through `0x00633640`.

### 5. The d2rs side (`d2-sim::poke`)

1. `d2-sim::poke` parses directives and applies one to a running
   `WorldSim` game through the paths of §1's d2rs column. It is a
   debug and test entry point: nothing in normal play calls it, it has
   no I/O, and it keeps `d2-sim`'s determinism rules (`CLAUDE.md` rule
   6): it draws only what the creation path it calls draws.
2. Callers: `scenario-run` (scenario `poke` steps, §3), `d2-client
   play --poke <file>` (applied on the server thread between ticks, at
   §2 rule 4's point) and `d2-client state-dump --poke "<f> ..."` (§2
   rule 6, through the bridge after the snapshot of frame f − 1; its
   1.14d counterpart is `record_state.py --poke`, whose pokes run after
   that stop's snapshot; `tools/scenario-diff.md` §3 rule 5). Both resolve references on the state after tick
   t − 1 (`scenario.md` §3 rule 3).

## Constants & data dependencies

| Address / field | Use | Owner |
|---|---|---|
| `0x0052FD1E` | between-tick hook (ESI = game) | `original-hooks-spawn.md` §5 |
| `0x00555230` | allocator (ECX type, EDX class; stack x, y, game, room, flags, mode, GUID) | `sim/units.md` §3.1 |
| `0x005A49B0` | superunique (ECX game, EDX room; x, y, row) | `original-hooks-spawn.md` §1 |
| `0x0059FA30` | missile creator (ECX game, EDX record) | `original-hooks.md` §7.1 |
| `0x00463740` | room of a point | `original-hooks-spawn.md` §1 |
| game +0xD0, unit +0x20/+0x24 | game seed, unit seed | `sim/rng.md` §5 |
| game +0xBC, act +0x04, env +0x00/+0x08 | environment record period, ticks | `render/lighting.md` §9.1 |
| game +0xA8, +0x1120 | frame, hash lists | `original-hooks.md` §3–§4 |

## Randomness

None of its own (§1 rule 4). A `seed-game` or `seed-unit` changes every
later draw of that seed on purpose; a creation shifts every later draw
of the seeds it touches (`original-hooks-spawn.md` Edge cases 3), the
same on both sides.

## Edge cases & original bugs

1. A field write is not followed by the message the game would send for
   that change in play (e.g. `time` sends no S→C 0x53 until the
   environment advances on its own); both sides behave so.
2. `object` with an objects row whose InitFn spawns monsters creates
   them too (`sim/units.md` §3.1 step 7.5).
3. `superunique` refuses a second spawn of a row without `Stacks`
   (`original-hooks-spawn.md` §1 rule 2): `failed`.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| each malformed line (unknown directive, missing / extra argument, value out of range, tick out of order, wrong version) | an error naming the line | synthetic (`d2-sim::poke` tests) |
| every directive parsed and written | the canonical text; parsing it again gives the same directive | synthetic |
| `traces/scenarios/poke-spawn-town.scenario` twice on the synthetic install | byte-identical traces | synthetic |
| `traces/checks/poke-fallen-town.check` (ScnAma, seed 1234; frame 4: `spawn 19 @x+3 @y+3 normal`, `seed-unit @1:19 0x12345678 666`), 1.14d against d2rs, 54 frames | both pokes `ok`; the same party (GUIDs 8–11, same class, positions, mode 1 for all 50 ticks) and the poked seed equal | REC-590, run 2026-10-09 (cloud, Wine): equal as stated; differs: minion seeds and every creation hp, because d2rs's game seed is one step behind 1.14d from frame 2 (the joining player's unit seed: 1.14d {lo of one game-seed step, 666}, d2rs {1, 666}), a join finding outside this spec |
| same check with the seeds pinned first (`seed-game 0x1234 666`, `seed-unit @player 0x55 666`, then the spawn) | the party and the game seed equal for 50 ticks | REC-590, 2026-10-09: equal (the party's every compared field and the game seed, frames 4–54); left: fields d2rs's snapshot does not fill (monster `tx`/`ty`, player `fc`/`sp`), outside this spec |
| `traces/checks/poke-fallen-town-unpinned.check` (no seed pins) on b0850e52 (join seed fix, staging 9aa0b329; path target, fc/sp, walk speed) | every unit and the game seed equal for 54 frames | REC-590, 2026-10-09 (cloud, Wine): no difference in any compared field of any unit or the game seed, frames 1–54; verdict PARTIAL only for the snapshot's own gaps (owner, headless client) |
| `traces/checks/poke-firebolt.check`: as above, then before frame 8 `missile 58 @x @y @x+3 @y+3 skill 36 1` (Fire Bolt, owner the player) | missile created on both sides, game seed after it equal, the target's hp equal | REC-590, 2026-10-09: missile `ok` (GUID 1) on both, game seed equal every frame after it, leader hp 1024 on both (no damage in town on either side); missiles are not in `state-snapshot.md` records. Damage outside town needs `warp` on 1.14d (form stated, REC-655) |

## Provenance

- d2rs-own format (§2–§3).
- 1.14d column: every address and argument form is quoted from the spec
  named in its row; none is new here.

## Open questions

None. (Questions 1–4, the call forms of `pos`, `warp`, `item`, `stat`
and `state`, are answered in `path-placement.md` §6 r4 / §10,
`world/waypoints.md` §7 r5, `items/generation.md` §3 / §10.1 and
`sim/stat-lists.md` §5 r2 / §9.2, read from the asm; their first 1.14d
run is REC-655.)
