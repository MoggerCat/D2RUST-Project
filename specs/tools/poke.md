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
  forms). Run on 1.14d on Windows (PC 1, 2026-10-09, same command,
  `--after 125`): `spawn-town.poke` all five `ok` (spawn GUID 8, object
  GUID 18); screenshots at 40 and 100 ticks show the fallen party of 4
  beside the player, the lit brazier 3 sub-tiles left, Warriv walking;
  the PNGs stay off the public repo (rule 1; `docs/handoff/pc1-day3-a.md`).
  The comparison with d2rs is REC-590. The d2rs side (§3, §5) is implemented
  (`d2-sim::poke`, scenario `poke` steps, `scenario-run`, `d2-client
  play --poke`): every directive runs on the synthetic install; `warp`
  to another act runs the act change (§1 table); `item` without item
  tables is a d2rs gap. `goto` (§6, 2026-10-09) is ours: a walk built
  from `warp` and the placement; first runs on both sides are in the
  Test vectors.
  tables is a d2rs gap. `msg` (one C→S message through the local
  client's sender, 2026-10-09) is implemented on both sides
  (`poke.py` selftest; d2-sim unit tests); its 1.14d run and the
  d2rs `state-dump` run on the real install are unverified (Test
  vectors). `operate` and `talk` (2026-10-09): the server's
  C→S dispatcher called directly on both sides (§4 rule 11, §5 rule 4);
  `poke.py` selftest and d2-sim / d2-client unit tests; live runs in
  the Test vectors.
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
  `sim/intents-events.md` §2.1 (client sender, duplicate filter),
  `sim/client-messages.tsv` (`msg` layouts),
  `world/waypoints.md` §7, §11 (warps), `tools/test-variants.md`
  (patched installs: what a poke cannot set).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 72–84 |
| Inputs | 85–91 |
| Outputs / state changes | 92–100 |
| Rules | 101–102 |
|   1. Directives | 103–142 |
|   2. Poke files | 143–165 |
|   3. In scenarios | 166–180 |
|   4. The 1.14d side (`poke.py`) | 181–354 |
|   5. The d2rs side (`d2-sim::poke`) | 355–418 |
|   6. `goto`: walking to a target | 419–498 |
| Constants & data dependencies | 499–514 |
| Randomness | 515–521 |
| Edge cases & original bugs | 522–549 |
| Test vectors | 550–577 |
| Provenance | 578–588 |
| Open questions | 589–596 |
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
cannot run the directive, §4); `goto` also has `pending` between its
steps (§6). Results are written as `poke` records (§3 rule 3).

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
| `pos` | `<ref> <x> <y> [free]` | teleport a unit; `free` (REC-1080): land on the nearest free, missile-passable cell instead of the raw point: the cell of the room holding (x, y) whose collision mask has none of the bits 0x1C0D (the player's move bits 0x1C09 and the missile-blocking bit 0x4), nearest (x, y) by squared distance, row by row from the top-left of the room's sub-tile rectangle, the first found on a tie; none → `failed`. Plain `pos` stays a raw teleport | `0x00650BE0` (stack: unit path, unit, room, x, y; `ret 0x14`; 1/0; `path-placement.md` §6 r4), else `0x00554EA0` (ECX game, EDX unit; room, x, y, exact 1, alt 0; `ret 0x14`; 1/0; §10); room by entry 6 from the unit's room; with `free` the cell is read from the room's collision record (§6 data: `poke.py` `free_cell`) before the call | `WalkCtx::teleport` (`path-placement.md` §6 r4 teleport path); with `free` the cell from the active room's grid (`poke::nearest_free`) |
| `hop` | `<ref> <x> <y>` | d2rs-own test aid (playthrough sweeps): move the unit at most 16 sub-tiles per axis toward (x, y) from where it stands, to the first spot of a fixed candidate list where a `pos` moves it: rings of radius 0, 2, 4, 7 (8 directions) around the full step, then around the half step, then a sidestep of 8 to either side across the step; a candidate whose cell has any bit of 0x1C0D in its room's collision mask (or no room or grid) is skipped without a `pos` (REC-1080: the unit never ends on a cell that stops missiles); `ok` when the unit moved (or is within 1 of the point), `failed` when no candidate did | each candidate as `pos`; moved = the path's sub-tile position changed (`poke.py` `hop_candidates`) | `poke::hop_candidates` and `WalkCtx::teleport` per candidate; a refused candidate's path error is dropped |
| `warp` | `<level> [tile <n>]` | move the player to a level (tile index default 0) | `0x0053AEC0` (ECX game, EDX player; level, tile; `ret 8`; EAX not a status: `ok`; `world/waypoints.md` §7 r5) | `wiring::path::place::level_warp` (`path-placement.md` §11); another act: the act change `wiring::path::act_change::run` (`world/waypoints.md` §11 steps 1–19, `flows/act-change.md` §1; step 3 builds the act's DRLG when missing), as waypoint travel to another act; the client's 0x04 follows on the next tick's client pass |
| `item` | `<code> <x> <y> [quality <q>] [ilvl <n>]` | create an item on the ground | `0x00558D90` (ECX game, EDX request; use seed 0; `ret 4`; EAX the item; `items/generation.md` §3) with request spawn mode 3, x, y, room (entry 6), item = the combined index of the code (§4 rule 8) | `ItemUnits::create_item` (spawn mode ground) then `items::moves::ground::ground_place` |
| `stat` | `<ref> <stat> <layer> <i32>` | set a base stat | `0x00627260` (stack: unit, s, value, layer; `ret 0x10`; `sim/stat-lists.md` §5 r2) | `StatLists::unit_set` (`stat-lists.md` §5 r2) |
| `state` | `<ref> <state> on\|off` | set or clear a state | `0x00639DB0` (stack: unit, s, 1/0; `ret 0xC`; `stat-lists.md` §9.2) | `toggle_state` + `set_state_changed` (`stat-lists.md` §9.2 toggle with update-queue insert) |
| `freeze` | `<seconds>` | hold the game between ticks for wall time (screenshots, a human look) | the debugger keeps the thread stopped at the hook (§4 rule 2) | no-op, `ok` (the runner owns the clock) |
| `goto` | `unit [<type>:]<class>` \| `preset <level> [<type>:]<class>` | move the player onto a free cell next to a unit of that type (default 1, monster) and class; `preset`: the level's preset of that type and class, found as the unit its room creates, after a warp to the level when the player is elsewhere (§6) | a multi-tick walk (§6) of `warp` (`0x0053AEC0`) and placement `0x00554EA0` with exact 0 (`path-placement.md` §10: free point §7, mask 0x1C09) | the same walk: `level_warp` / `act_change::run`, then `wiring::path::place::place_unit` with exact `false` |
| `msg` | `<id> <value>...` | one C→S game message (id 0x01..0x70) sent as the local client's, drained in the next frame: the values fill the id's fields in the order of the `layout` column of `sim/client-messages.tsv`; each is a number, `@x±N` / `@y±N` (the player's position) or a `<ref>` (its GUID). Bytes: the id, then each field little-endian at its offset (`uN` / `bitN`: into the u32 at the offset), `transport_size` bytes. Only ids with a fixed size whose layout has integer fields only (`u8`, `u16`, `u32`, `uN`, `bitN`); bytes the layout does not list (e.g. 0x49 bytes 7–8, which the server ignores) are written 0 on both sides; another id, a wrong count or a number too big for its field is an error (§2 rule 5); a resolved reference that does not fit its field is `unresolved` | the client sender `0x00478350` with the bytes (§4 rule 9); `ok` | `Host::send_game` for client 0 (§5 rule 3): `ok`, or `failed` with the reason |
| `operate` | `<ref>` | interact with the unit (an object's operate, a portal, an NPC's interaction start) as the player: one C→S 0x13 {type, GUID} of the unit, run by the server's dispatcher at the poke point (§4 rule 11, §5 rule 4), not through the client's sender; out of operate range the handler starts the walk and operates on arrival (`world/objects.md` §7.3, `world/npc.md` §2 rule 3) | the dispatcher `0x0054D750` (ECX game, EDX player, [ESP+4] message, [ESP+8] size; EAX the result code 0–3; `sim/intents-events.md` §2.3) with 0x13 (handler `0x0054AA90`, `sim/client-messages.tsv`) | `Host::dispatch_now` (the model of `0x0054D750`) for client 0 with the same bytes |
| `talk` | `<ref> [<choice>...]` | talk to an NPC: 0x13 {1, GUID} then 0x2F {GUID} (chat open, `world/npc.md` §2–§3), then one message per choice, in order: `trade` 0x38 {1, GUID, 0}, `gamble` 0x38 {2, GUID, 0}, `hire` 0x38 {3, GUID, the player's GUID} (the client's own form, `world/npc.md` §4), `action:<n>` 0x38 {n, GUID, 0}, `quest:<n>` 0x31 {GUID, n}, `close` 0x30 {GUID}; each run by the dispatcher now; the first call whose result is not 0 ends the directive | as `operate`, one dispatcher call per message (handlers `0x0054AA90`, `0x0054B930`, `0x0054BCA0`, `0x0054BA90`, `0x0054B9F0`) | as `operate`, one `Host::dispatch_now` per message |

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
   unit (`goto`: the target's GUID, and `steps`, §6 rule 4). It belongs to the `c2s` stream (compared there, after `spawn`
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
9. **`msg`:** `CALL_FORMS` entry `send`: the client game-message
   sender `0x00478350`, EDI = size, [ESP+4] = message
   (`sim/intents-events.md` §2.1 rule 1; EAX ignored). The bytes are
   written at scratch +0x300 (after the item request at +0x200,
   0x84 bytes; a message is < 0x200 bytes). Unit references give the
   GUID at unit +0x0C, positions the player's path x / y ± N. The
   result is `ok`: whether the sender's duplicate filter dropped the
   message is not visible here (Edge cases 4).
10. **Interaction calls** (2026-10-09, static asm): the entry points
    of C→S 0x13 / 0x2F / 0x30 / 0x38 and their inner functions. §1
    `operate` and `talk` use the first row, the dispatcher (rule 11);
    the other rows are alternatives no directive calls yet, kept as the
    reference. Synchronous alternatives to `msg` for
    C→S 0x13 / 0x2F / 0x30 / 0x38: called at the hook, they act in the
    same stop and return the handler's code. Conventions and proofs
    live in the owning specs (`world/objects.md` §7.4,
    `world/npc.md` §2–§4 "Call forms"); this is the poke-side digest.
    The message bytes are built exactly as `msg` builds them (rule 9,
    scratch +0x300), size = `transport_size`; the player is P (§4
    rule 5); the NPC's interaction block is the u32 at
    (u32 at NPC +0x14) + 0x30.

    | Use | Function | Registers | Stack from [ESP+4] | `ret` | EAX | Preconditions |
    |---|---|---|---|---|---|---|
    | any C→S message, with the alive gate | dispatcher `0x0054D750` (`sim/intents-events.md` §2.3) | ECX game, EDX player | message, size | 8 | handler code 0 done, 1 refused, 2 invalid, 3 malformed; 0 also when the gate refuses | P alive and not uninterruptable (gate) |
    | 0x13 interact (type 1 NPC, 2 object, 4 item, 5 tile, 0 player) | handler `0x0054AA90` | ECX game, EDX player | message `13 <type u32> <GUID u32>`, 9 | 8 | code | type 1: unit distance P → NPC ≤ 6 starts the talk (7–8: walk then talk on arrival; 9–50: nothing; > 50: 1), P free (no interact unit, no cursor item, player data +0x4C = 0), NPC mode ∉ {0, 12} with monstats `interact`; type 2: object mode < 8, distance ≤ 50, in the interact box and line test clear, else a walk (result 0 either way) |
    | the same, without the size / type parse | `0x00548B00` | ECX player, EDX type | GUID, flag 0, game | 0xC | code | as the row above |
    | operate an object at once (no range, no walk) | `0x00584420` | ECX game, EDX player | type 2, GUID, pointer to a u32 out | 0xC | 0 refused / no operate function, else the operate function's result (not a status; check the object's mode) | object exists; P's interact info inactive, +0x4C = 0, no cursor item unless stash 267; P's path not stopped (0x13 stops it first) |
    | start talking without range (no NPC stop) | `0x00573020` | ECX game, EDX player | NPC unit, block, 0 | 0xC | not set | block non-null; P free; NPC alive with `interact`; P not already in its list. The NPC's AI is not paused (0x13 rule 2 is skipped) |
    | 0x2F open chat (node 0 → 1, heal) | handler `0x0054B930` | ECX game, EDX player | message `2F 00000000 <NPC GUID>`, 9 | 8 | code (1 when \|dx\| or \|dy\| > 10) | node from 0x13 in state 0; same act; within 10 per axis |
    | the same, inner | `0x00572E60` | ECX game, EDX player | NPC unit, block | 8 | not set | node in state 0 (else nothing) |
    | 0x38 menu action (1 trade, 2 gamble, 3 hire list, services) | handler `0x0054BCA0` | ECX game, EDX player | message `38 <action> <NPC GUID> <item GUID>`, 13 | 8 | code: 1 missing / > 50 per axis, 2 other act, 3 service item check | NPC with `interact` and a block; for actions 1 / 2 a node in state 1 (0x2F first) as the client does it |
    | the same, inner | `0x00579D60` | ECX game, EDX player | action, NPC GUID, item GUID | 0xC | 0, or 3 | as above, no act / range test |
    | 0x30 end chat (unlink node, reset interact unit) | handler `0x0054B9F0` | ECX game, EDX player | message `30 00000000 <NPC GUID>`, 9 | 8 | code | same act; no range test |
    | the same, inner | `0x00572F20` | ECX game, EDX player | NPC unit, block | 8 | not set | block non-null |

    The order the client produces is 0x13 (type 1) → 0x2F → 0x38 → 0x30;
    calling the handlers in that order (one stop or several) leaves the
    NPC state as play does, the NPC paused by 0x13 rule 2. The game
    message entry `0x0053F3D0` (ECX = buffer with the client id first,
    EDX = size) is not proposed: it takes the game's lock itself and
    drops the handler's code (`sim/intents-events.md` §2.2).
    A handler called at the tick-return stop has the same effect as the
    same message injected for that frame (REC-1150, settled 2026-10-09 by
    recording): ScnHi1, `-seed 1234`, Act I town, chest class 5 spawned
    next to the player (`object 5 @x+2 @y`, GUID 23) at frame 6, then at
    frame 12 either `send InteractWithEntity type=2` (C→S 0x13, drained
    after the frame-11 stop) or the direct operate `0x00584420(type 2,
    GUID 23)` at the frame-11 stop. Every field of every unit and the game
    seed are equal in all 40 snapshots of the two runs; the chest is in
    mode 2 from snapshot 12 in both, so the events are scheduled from the
    same frame (no off-by-one from game +0xA8). Probe: `probe_operate2.py`
    (scratch; a `stat @player 65535 0 <GUID>` poke rerouted to the call)
    until `operate` is in `CALL_FORMS`.

    Proposed `CALL_FORMS` entries (same `Fn` / `Form` shapes as the
    table in `poke.py`; `code` is a proposed fourth `RESULTS` kind:
    EAX 0 → `ok`, else `failed` with the code in the record's `eax`),
    plus `FIELDS["mon_interact"] = 0x30` (monster data +0x30, the
    interaction block pointer):

    ```
    "dispatch":      Fn(0x0054D750, ("game", "player", "message", "size"), "code",
                        "sim/intents-events.md §2.3", None,
                        Form({"ecx": "game", "edx": "player"}, ["message", "size"], 8)),
    "h_interact":    Fn(0x0054AA90, ("game", "player", "message", "size"), "code",
                        "world/npc.md §2; world/objects.md §7.4", None,
                        Form({"ecx": "game", "edx": "player"}, ["message", "size"], 8)),
    "interact":      Fn(0x00548B00, ("player", "type", "guid", "flag", "game"), "code",
                        "world/npc.md §2 call forms; world/objects.md §7.4", None,
                        Form({"ecx": "player", "edx": "type"}, ["guid", "flag", "game"], 0xC)),
    "operate":       Fn(0x00584420, ("game", "player", "type", "guid", "out"), "none",
                        "world/objects.md §7.2, §7.4", None,
                        Form({"ecx": "game", "edx": "player"}, ["type", "guid", "out"], 0xC)),
    "npc_start":     Fn(0x00573020, ("game", "player", "npc", "block"), "none",
                        "world/npc.md §2 call forms", None,
                        Form({"ecx": "game", "edx": "player"}, ["npc", "block", 0], 0xC)),
    "h_chat_open":   Fn(0x0054B930, ("game", "player", "message", "size"), "code",
                        "world/npc.md §3", None,
                        Form({"ecx": "game", "edx": "player"}, ["message", "size"], 8)),
    "chat_open":     Fn(0x00572E60, ("game", "player", "npc", "block"), "none",
                        "world/npc.md §3 call forms", None,
                        Form({"ecx": "game", "edx": "player"}, ["npc", "block"], 8)),
    "h_npc_action":  Fn(0x0054BCA0, ("game", "player", "message", "size"), "code",
                        "world/npc.md §4", None,
                        Form({"ecx": "game", "edx": "player"}, ["message", "size"], 8)),
    "npc_action":    Fn(0x00579D60, ("game", "player", "action", "npc_guid", "item_guid"), "code",
                        "world/npc.md §4 call forms", None,
                        Form({"ecx": "game", "edx": "player"}, ["action", "npc_guid", "item_guid"], 0xC)),
    "h_chat_close":  Fn(0x0054B9F0, ("game", "player", "message", "size"), "code",
                        "world/npc.md §3", None,
                        Form({"ecx": "game", "edx": "player"}, ["message", "size"], 8)),
    "chat_close":    Fn(0x00572F20, ("game", "player", "npc", "block"), "none",
                        "world/npc.md §3 call forms", None,
                        Form({"ecx": "game", "edx": "player"}, ["npc", "block"], 8)),
    ```
    *Run on 1.14d (2026-10-09, PC 1, Windows):* the `operate` form was
    called at the tick-return stop of frame 39 (scratch wrapper rerouting a
    sentinel `stat` poke into the call; `record_packets.py --auto CkAndariel
    --seed 1`): stack `2, 11, out` (the Rogue Encampment waypoint, GUID 11,
    at (5680, 5793); the player at (5663, 5813), far outside the §7.1 range
    box) → EAX 1, out word 1, and the server's next send to the client is
    S→C 0x63 `63 0b000000 02 01 ff01 …` (the waypoint menu of GUID 11) in the
    same frame: the form is right, and the operate skips range and walk as
    stated.

    `out` is a scratch u32 (e.g. scratch +0x2F0). Every form keeps
    `ret` = 4 × stack length (rule 8).

11. **`operate`, `talk`:** `CALL_FORMS` entry `dispatch`: the C→S
    dispatcher `0x0054D750`, ECX = game, EDX = player, [ESP+4] =
    message, [ESP+8] = size, `ret 8` (rule 10), EAX = the handler's result code
    (`sim/intents-events.md` §2.3: the gate, then the handler; handlers
    and their sizes `sim/client-messages.tsv`). Each message of the
    directive (§1 rows) is written at scratch +0x300 followed by four
    zero bytes (`world/objects.md` §7.3 rule 4 reads a u32 at +9 of the
    9-byte 0x13) and the dispatcher is called once per message, in
    order, at the hook (§4 rule 2; the server thread, between ticks).
    The bytes are the `msg` encoding of the id's layout (rule 9; bytes
    the layout does not list are 0). Result: `ok` with the target's
    GUID when every call returned 0; else `failed` with note `<id>
    returned <code>` for the first call that did not, and no later call
    is made. The record carries `codes` (each call's EAX, in order).
    The packet recorder's dispatcher hook (`record_packets.py`) sees
    each call as a `dispatch` record; the queue entry `0x0053F3D0` is
    not run, so there is no `c2s` or `result` record.
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
3. `msg` is not applied to the game. `d2-sim::poke` parses its
   values by their form and resolves them on the state after tick
   t − 1 (`msg_values`); the layouts are transport knowledge, which
   `d2-sim` does not hold (`tools/depcheck`: no `d2-proto`), so the
   `d2-client` poke module checks the line against the id's layout at
   parse time (`--poke`, `--poke-file`; §2 rule 5) and builds the bytes
   from the `d2-proto` tables. The caller passes them to the host's
   client sender for client 0 (`Host::send_game`, the model of `0x00478350` with its
   duplicate filter, `sim/intents-events.md` §2.1 rule 1), so the
   next frame's drain handles them as the client's own. Result `ok`
   when the sender queued them; `failed` with note `duplicate filter`
   when the filter dropped them; `failed` with the sender's error when
   it refused them. The `d2-client` callers (`play --poke`,
   `state-dump --poke`) have the host; a runner without one
   (`d2-sim::poke::apply`, scenario `poke` steps, `scenario-run`)
   returns `gap` with note `msg needs the host's client queue`
   without the layout check (scenarios write messages as their own
   `msg` steps, `scenario.md`).
4. `operate` and `talk` are not applied to the game by `d2-sim::poke`
   either: `interact_calls` resolves the target on the state after
   tick t − 1 and gives the handler calls of §1 (id and values in
   layout order). The `d2-client` poke module encodes each with the
   `msg` encoder and runs it through `Host::dispatch_now` for client 0:
   the dispatcher (`d2-server::dispatch`, the model of `0x0054D750`)
   called at once, outside the queues (no sender, no duplicate filter),
   its S→C replies going to the client's buffers as in a drain. Results
   as §4 rule 11. A runner without a host returns `gap` with note
   `operate / talk need the host's dispatcher`.
5. **Tick end.** The 1.14d hook (§4 rule 1) stops inside the frame,
   after the tick and before that frame's flush, so a handler's replies
   leave in the same frame's buffer (first run: `interact-talk-akara`,
   0x27 / 0x29 / 0x28 flushed in frame 15 on 1.14d, 16 on d2rs between
   frames). `state-dump` therefore runs every poke of a frame that
   holds an `operate`, a `talk`, a `goto` or a `warp` at the same point: a
   tick-end hook of the link (`Host::frame_with`, after the tick, before
   the flush), in file order, after taking that frame's snapshot there
   (as `record_state.py` snapshots before its pokes, §5 rule 2). A
   `goto`'s later steps (§6) run there too, one per frame, before the
   pokes due in that frame. What the directives send through the sim
   (a placement's 0x07, `path-placement.md` §10 rule 6) is queued into
   the client's buffer right after them (`Host::queue_now`), so it
   leaves in that frame's flush, as on 1.14d (first run:
   `a2-npc-atma-talk`, the walk's 0x07 in frames 3–12 on 1.14d, one
   frame later on d2rs before; rc-net-s2c; `warp`: the landing room's
   0x07 in the poke's frame window, the neighbours' in the next tick,
   448 generated checks with `4 warp 2`, rc-packets-join-order). Other
   frames keep the between-frames point: at the tick end a `pos` /
   `hop` (d2rs-own reassign request, §1) sends an S→C 0x15 in the next
   frame that 1.14d does not send (`interact-operate-waypoint`,
   `interact-talk-akara`, `items-vendor-akara-buy` frame 4, rc-packets-join-order; open: why
   the between-frames point sends none is not traced).
   `play --poke` runs every poke between frames: its
   `operate` / `talk` replies reach the client one frame later than on
   1.14d (a play aid, not a comparison).

### 6. `goto`: walking to a target

A target outside the player's active rooms does not exist yet as a unit
(rooms are populated when they become active), and a far room cannot be
entered by a placement (it needs an active room). So `goto` is a walk
that runs one **step** per tick, at the poke point (§2 rule 4), until
it lands, using only the warp and the placement of §1 and the room
records both sides keep.

1. **Target.** `<type>:<class>`: unit type (1 monster, 2 object; other
   types are refused by the parser) and class (the `monstats` /
   `objects` row of the created unit; a superunique preset is the
   `monstats` row its unit has). `<class>` alone is type 1. The goal
   level G: `preset` names it; `unit` takes the player's level at the
   first step.
2. **State** kept by the runner between steps: the target, G, the set
   of DRLG rooms **seen** and the set of rooms **blocked** (both by
   level id and tile origin), the step count.
   A new `goto` starts with an empty set; one `goto` runs at a time.
3. **A step:**
   1. `preset` and the player's level is not G: at the first step,
      `warp G` (§1, tile 0); refused → `failed`. Later steps: no warp
      (the walk left G through a free-point search; it goes on from
      there).
   2. **Found:** the first unit, in ascending GUID order over the
      server's unit lists, with the target's type and class whose room's
      level is G. Then place the player (placement with exact 0, alt 0)
      in that unit's room at the unit's position: the free-point search
      (§7 of `path-placement.md`, the player's size and mask 0x1C09)
      picks the cell, so a blocked cell (an object, lava) moves the
      player to the nearest free cell around it. Placed → `ok`, with
      the target's GUID as the result's GUID (a later `g @pI` names it);
      not placed → `failed`. Then (REC-1080), when the cell the player
      stands on has any bit of 0x1C0D (the free-point search excludes
      only 0x1C09, so bit 0x4 can remain), place the player again in the
      same room with exact 1 on the cell the `pos … free` rule (§1)
      picks around that cell; no such cell → the first placement stands.
   3. **Not found:** add to the seen set the player's DRLG room and
      every room of its near array that has an active room (their
      units exist now). Then a breadth-first search over near arrays
      (each room's array in its stored order, never entering a blocked
      room) from the player's DRLG room finds the first room of level G
      not seen. None → `failed`
      ("explored, n rooms seen"). Else the first room H on the way to
      it (a member of the player's room's near array): H without an
      active room → `failed`. The hop's cell: among H's cells (its
      collision grid, row by row from the top-left) whose mask has none
      of the bits 0x1C0D (the move bits 0x1C09 and the missile-blocking
      0x4, REC-1080), the one nearest the centre of
      H's sub-tile rectangle (cx = x + w / 2, cy = y + h / 2, integer
      division; squared distance; the first found on a tie). Such a
      cell, not the centre, is the target because a centre on lava
      would send the free-point search back to the island the player
      stands on (River of Flame, level 107). H without a free cell is
      marked seen and blocked (no placement). Else place the player in
      H's active room at that cell, exact 0. A refused placement, or one
      that leaves the player outside H, marks H seen and blocked.
      The step's result is `pending`.
   4. After 400 steps the walk ends `failed` ("step limit").
4. **Records.** A pending step writes nothing; the last step writes the
   directive's one `poke` record, with `f` / `t` of that last step and
   `steps` = the number of steps. A run that ends while a `goto` is
   pending writes it as `failed` ("not finished").
5. **Not a comparison point.** The walk enters rooms, so it activates
   and populates them, and each side does that in its own order of
   seeds; the state after a `goto` is a start for a session, not a
   state the two sides are expected to agree on. Checks that compare
   start from saves and the other directives.
6. **Data.** 1.14d: the player's room (dynamic path +0x1C) → +0x10 DRLG
   room; DRLG room +0x08 near array of DRLG rooms, +0x2C its count,
   +0x30 active room (0: none), +0x34 / +0x38 tile x, y, +0x58 level →
   level +0x1D0 id (`drlg/rooms.md` §1, `drlg/levels.md`); active room
   +0x4C sub-tile x, y, w, h, +0x20 collision record (+0x00 x0, +0x04
   y0, +0x08 width, +0x0C height, +0x20 the u16 masks, index
   (y − y0) · width + (x − x0); `drlg/rooms.md` §10.3 and Open question
   11, `items/treasure.md` §8); a unit's room: dynamic path +0x1C, static
   path +0x00 (`tools/state-snapshot.md` §2). d2rs: `Drlg::room`,
   `DrlgRoom::near` / `active`, `ActiveRoom::{id, subtiles}`,
   `Drlg::drlg_room_of`.

## Constants & data dependencies

| Address / field | Use | Owner |
|---|---|---|
| `0x0052FD1E` | between-tick hook (ESI = game) | `original-hooks-spawn.md` §5 |
| `0x00555230` | allocator (ECX type, EDX class; stack x, y, game, room, flags, mode, GUID) | `sim/units.md` §3.1 |
| `0x005A49B0` | superunique (ECX game, EDX room; x, y, row) | `original-hooks-spawn.md` §1 |
| `0x0059FA30` | missile creator (ECX game, EDX record) | `original-hooks.md` §7.1 |
| `0x00463740` | room of a point | `original-hooks-spawn.md` §1 |
| `0x00478350` | client game-message sender (EDI size, [ESP+4] message; duplicate filter) | `sim/intents-events.md` §2.1 rule 1 |
| `0x0054D750` | C→S dispatcher (ECX game, EDX player; [ESP+4] message, [ESP+8] size; EAX result 0–3) | `sim/intents-events.md` §2.3 |
| `0x0054AA90`, `0x0054B930`, `0x0054B9F0`, `0x0054BA90`, `0x0054BCA0` | handlers of 0x13, 0x2F, 0x30, 0x31, 0x38 (reached through the dispatcher) | `sim/client-messages.tsv`, `world/npc.md` §2–§4, `world/objects.md` §7.3 |
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
4. `msg` passes the client sender's duplicate filter
   (`sim/intents-events.md` §2.1 rule 1): a message byte-identical to
   the last one sent, less than its window after it (50 ms for ids
   0x05–0x0A and 0x0C–0x11, never for 0x3A, 200 ms for the others), is
   dropped. The filter's clock is wall time on 1.14d and the step clock
   on d2rs `state-dump` (40 ms per frame); 1.14d under Wine runs slower
   than real time, so d2rs is the stricter side. A check therefore never
   repeats a byte-identical `msg` within 5 frames (two in one frame
   included). The client's own sends share the filter's stored copy on
   both sides. Only d2rs reports a dropped message (`failed`,
   `duplicate filter`); 1.14d reports `ok`.

5. `operate` / `talk` result `ok` means only that every handler
   returned 0: the 0x13 handler returns 0 for a walk (out of operate
   range) and for an NPC 9..50 sub-tiles away (no interaction,
   `world/npc.md` §2 rule 3), and 0x2F returns 0 when the player is not
   in the NPC's list. The state and packet channels show what happened;
   a check places the player first (`goto`).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| each malformed line (unknown directive, missing / extra argument, value out of range, tick out of order, wrong version) | an error naming the line | synthetic (`d2-sim::poke` tests) |
| every directive parsed and written | the canonical text; parsing it again gives the same directive | synthetic |
| `goto unit 5` / `goto preset 2 2:119` / `goto preset 107 376` parsed and written | canonical `goto unit 1:5`, `goto preset 2 2:119`, `goto preset 107 1:376`; `goto unit 3:5`, `goto preset 2`, `goto here 5` errors | synthetic (`d2-sim::poke` tests, `poke.py --selftest`) |
| `goto` on a synthetic walk: target in the player's near rooms; target two rooms away; no target | `ok` with the target's GUID in 1 step; `pending`, `pending`, `ok`; `failed` "explored" | synthetic (`d2-sim::poke` tests; `poke.py --selftest` on a fake game) |
| checkpoint `a4-hellforge` (`traces/checkpoints/`), seed 1: `5 warp 107`, `20 goto preset 107 2:376` | lands next to the Hellforge | 2026-10-09: d2rs (`state-dump`) and 1.14d under Wine (`record_state.py`): `ok` after 50 steps at f69 on both, the player at (7779, 6133), the forge at (7781, 6135), Hephasto at (7810, 6143) on both (GUIDs differ: forge 94 / 96) |
| `pos @player X Y free` and `hop @player X Y` with bit 0x4 set on the target cell (full step); `goto unit` whose landing cell has 0x4 | the unit ends on a cell with none of 0x1C0D, within 3 sub-tiles of the point; `goto` places again with exact 1 | REC-1080: `test-fixtures` `poke_goto.rs` (d2rs); `poke.py --selftest` (1.14d side, fake DRLG); the 1.14d run unverified |
| `traces/scenarios/poke-spawn-town.scenario` twice on the synthetic install | byte-identical traces | synthetic |
| `traces/checks/poke-fallen-town.check` (ScnAma, seed 1234; frame 4: `spawn 19 @x+3 @y+3 normal`, `seed-unit @1:19 0x12345678 666`), 1.14d against d2rs, 54 frames | both pokes `ok`; the same party (GUIDs 8–11, same class, positions, mode 1 for all 50 ticks) and the poked seed equal | REC-590, run 2026-10-09 (cloud, Wine): equal as stated; differs: minion seeds and every creation hp, because d2rs's game seed is one step behind 1.14d from frame 2 (the joining player's unit seed: 1.14d {lo of one game-seed step, 666}, d2rs {1, 666}), a join finding outside this spec |
| same check with the seeds pinned first (`seed-game 0x1234 666`, `seed-unit @player 0x55 666`, then the spawn) | the party and the game seed equal for 50 ticks | REC-590, 2026-10-09: equal (the party's every compared field and the game seed, frames 4–54); left: fields d2rs's snapshot does not fill (monster `tx`/`ty`, player `fc`/`sp`), outside this spec |
| `traces/checks/poke-fallen-town-unpinned.check` (no seed pins) on b0850e52 (join seed fix, staging 9aa0b329; path target, fc/sp, walk speed) | every unit and the game seed equal for 54 frames | REC-590, 2026-10-09 (cloud, Wine): no difference in any compared field of any unit or the game seed, frames 1–54; verdict PARTIAL only for the snapshot's own gaps (owner, headless client) |
| `msg 0x01 @x+2 @y`, player at (5000, 4000) | bytes `01 8A 13 A0 0F` | synthetic (`d2-sim::poke` tests with a fake resolver, `d2-client` poke tests, `poke.py --selftest` on a fake process) |
| `msg 0x06 1 @1`, the first monster's GUID 5 | bytes `06 01 00 00 00 05 00 00 00`; 1.14d: `0x00478350` with EDI = 9, [ESP+4] = scratch +0x300 | synthetic (as above) |
| `msg 0x3C 36 1 0xFFFFFFFF`; `msg 0x51 5 1 3 7`; `msg 0x60` | `3C 24 00 00 80 FF FF FF FF`; `51 05 80 03 00 07 00 00 00`; `60` | synthetic (as above) |
| `talk @1:148 trade hire quest:92 close`, Akara GUID 12, player GUID 1 | dispatcher calls with `13 01000000 0c000000`, `2f 00000000 0c000000`, `38 01000000 0c000000 00000000`, `38 03000000 0c000000 01000000`, `31 0c000000 5c00 0000`, `30 00000000 0c000000`, in order; `operate @wp` on waypoint GUID 18: `13 02000000 12000000` | synthetic (`d2-sim::poke` and `d2-client` poke tests, `poke.py --selftest` on a fake process: `0x0054D750` with ECX game, EDX player, stack scratch +0x300, size) |
| `operate` / `talk` whose second call returns 1 | `failed` "0x2f returned 1", no third call | synthetic (`poke.py --selftest`) |
| `operate`, `talk` malformed: no ref, an extra argument, an unknown choice, `quest:65536` | an error naming the line | synthetic |
| `traces/checks/interact-operate-waypoint.check` (ScnAma, seed 1234; `pos` next to the town waypoint, then before frame 10 `operate @2:119`), 30 frames | `ok` on both (GUID 10, codes [0]); state and packets equal | 2026-10-09 (cloud, Wine; d2rs tick-end point, §5 rule 5): `ok` GUID 10 on both; state no difference in any compared field (PARTIAL only for the snapshot's own gaps); packets MATCH (S→C 0x63 queued and flushed in frame 9 on both) |
| `traces/checks/interact-talk-akara.check` (ScnAma, seed 1234; poked next to Akara, before frame 16 `talk @1:148 trade`), 30 frames | `ok` on both (GUID 12, codes [0, 0, 0]); state and packets equal | 2026-10-09: `ok` GUID 12 on both, codes [0, 0, 0]; state no difference; packets: first difference frame 15, S→C 0x27 NpcInfo bytes 10 and 14 (the text list, `world/quests.md` §7.1: 1.14d `40 … 0b`, d2rs `0b … 40`), a d2rs NPC text-list divergence outside this spec (the id already diverges in `items-vendor-akara-buy`); the 1.14d client then sends 0x2F and 0x31 in frame 16 (its own reaction to 0x27) |
| `traces/checks/interact-operate-stash.check` (ScnAma, seed 1234; before frame 4 `operate @2:267` from the start, 7 sub-tiles from the stash) | `ok` on both (GUID 17); the player runs to the stash, which opens (S→C 0x77 at frame 13) | 2026-10-09: `ok` GUID 17 on both; 1.14d runs the player to (4868, 4229) and opens the stash at frame 13; d2rs does not move (frame 4, player mode 3 vs 5): its 0x13 object case uses the play preview's reach and never starts the walk of `world/objects.md` §7.3 rule 4 (the same with the 0x13 sent through the client queue), a d2rs gap outside this spec |
| the ids `msg` takes | 01–13, 16–2A, 2D–49, 4B–4D, 4F–54, 58, 59, 5D–63, 69–6B, 6D, 6E, 70 (from `client-messages.tsv`; both sides check the same list) | synthetic (`MSG_IDS` in both tests) |
| `msg` with no id, id 0 or 0x71, 0x14 (no fixed size), a wrong count, 65536 in a `u16`, 2 in a `bitN` | an error naming the line | synthetic |
| `state-dump --poke "4 msg 0x01 @x+5 @y"` twice (ScnAma, seed 1234) | first `ok`, second `failed` `duplicate filter`; the player has walked east by frame 12 | `d2-client` `app_state_dump` (real data, `#[ignore]`): unverified until the real-data gate runs it |
| `traces/checks/poke-firebolt.check`: as above, then before frame 8 `missile 58 @x @y @x+3 @y+3 skill 36 1` (Fire Bolt, owner the player) | missile created on both sides, game seed after it equal, the target's hp equal | REC-590, 2026-10-09: missile `ok` (GUID 1) on both, game seed equal every frame after it, leader hp 1024 on both (no damage in town on either side); missiles are not in `state-snapshot.md` records. Damage outside town needs `warp` on 1.14d (form stated, REC-655) |

## Provenance

- d2rs-own format (§2–§3).
- 1.14d column: every address and argument form is quoted from the spec
  named in its row; none is new here.
- §4 rule 10 (2026-10-09): quoted from `world/objects.md` §7.4 and the
  "Call forms" paragraphs of `world/npc.md` §2–§4 (read from `all.asm`
  there), and the dispatcher form of `sim/intents-events.md` §2.3
  (`0x0054D750`: `0x0054D75C` ESI := EDX, `0x0054D75E` EDI := ECX,
  `ret 8` at `0x0054D785`, `0x0054D81E`).

## Open questions

None. (Questions 1–4, the call forms of `pos`, `warp`, `item`, `stat`
and `state`, are answered in `path-placement.md` §6 r4 / §10,
`world/waypoints.md` §7 r5, `items/generation.md` §3 / §10.1 and
`sim/stat-lists.md` §5 r2 / §9.2, read from the asm; their first 1.14d
run is REC-655.)
