# Spec: Tools — Soak (seeded random input) and save/load round trip

- **Status:** implemented (d2rs-own tool, q-tool-soak, 2026-10-09): `d2-client soak`
  (`crates/d2-client/src/app/soak/`), `tools/soak/soak.py`, and the
  `#[ignore]` round-trip tests `crates/d2-client/tests/save_roundtrip.rs`.
  d2rs only: no 1.14d side, so a finding is a d2rs bug or gap, never a
  fidelity claim (CLAUDE.md rule 10).
- **Target version:** none (robustness of d2rs); the format is d2rs-own.
- **Crate/module:** `d2-client::app::soak` (`log`, `gen`, `checks`), `tools/soak/soak.py`.
- **Related specs:** `tools/poke.md` (the start pokes), `tools/state-snapshot.md`,
  `client/bridge.md` §6 (the receive log), §8 r5 (the pause),
  `seams/movement-prediction.md`, `items/inventory.md`, `formats/d2s.md`.

## Summary

The soak runs the play client headless (the app `d2-client play` builds,
on `MinimalPlugins`, as `tests/play_smoke.rs` runs it) on the user's
install, faster than real time on a stepping clock, and feeds it seeded
random input: clicks anywhere (UI panels, the ground) and on units,
right clicks, key actions (potions, skill hotkeys, the panels, Esc, Alt,
run, swap), item moves (pick up, drop, insert, remove, equip, belt, use),
town portal scrolls and books, waypoint use. After every frame it checks
for panics, hangs, broken server invariants and client/server desync.
Every input is written to an input log; a log replays the run exactly,
and `soak.py reduce` shrinks a finding's log to the least input that
still finds it.

## Inputs

| Name | Type | Source |
|---|---|---|
| start | `--save FILE.d2s` or `--new CLASS`, `--warp LEVEL`, `--game-seed`, `--difficulty`, `--no-kit` | command line, or a log's header |
| generator seed | u64 `--seed` | command line |
| input log | `soak-log 1` (§2) | `--replay LOG` |

## Outputs / state changes

The report: one JSON line per finding `{kind, sig, step, frame, detail}`
and a summary line `{summary, steps, server_frame, actions, refused,
findings, wall_s}`; the input log (`--log-out`, flushed per line). Exit 0
no finding, 1 findings. The soak changes no game rule; the start pokes
are the poke tool's.

## Rules

### 1. The run

1. One step is one app frame (`App::update`, the clock +40 ms). Steps are
   counted from the first frame after the start (rule 3); a log's step
   numbers are these.
2. Starts (`soak.py`): a new sorceress and barbarian (Rogue Encampment);
   a level-30 sorceress save per act (`d2s-tool new ... --act N
   --waypoints all --quests acts=N`) in its town; the same saves with the
   start poke `warp L` for Blood Moor (2), Cold Plains (3), Rocky Waste
   (41), Spider Forest (76), Outer Steppes (104), Bloody Foothills (110).
3. Start sequence: join (until the server and the model both have the
   local player, at most 2000 frames), 10 frames, the `warp` poke and 60
   frames, then the kit: `item <code> @x+dx @y+dy` pokes for `tsc tsc tbk
   isc hp1 hp1 mp1 mp1 rvs lbl hax buc cap rin box` around the player (a
   refused placement is skipped), 10 frames.
4. Each step without a pending `wait` draws one action with probability
   `--rate` % (default 30) from the generator (`gen.rs`, splitmix64; the
   tool's RNG, never the game's): 22 % left click (half anywhere, half
   near the centre), 8 % right click, 2 % cursor move, 10 % a click on a
   unit in view (its sub-tile projected 32 × 16 around the frame centre),
   18 % a key action, 6 % `interact` with a unit within 25 sub-tiles, 21 %
   an item move (cursor item: drop, insert into page 0/1/3/4, equip to a
   body location 1–12, belt slot 0–15; no cursor item: pick a ground
   item within 30 sub-tiles, remove, use from the belt, use from the
   grid), 4 % a portal scroll / book from the grid, 5 % a waypoint
   (interact, or C→S 0x49 to a waypoint level, 1 in 5 to any level
   0–136), else `wait 1–20`. Unit and item choices come from the client
   model only.
5. Every input goes through the client: UI events queued as the window
   queues them, the bridge's `interact`, or the C→S intents the client's
   panels send (`bridge::items`). A client refusal (bridge error) is
   counted (`refused`), not a finding.

### 2. The input log `soak-log 1`

Line 1: `soak-log 1 seed=S` then the start as `key=value` words
(`save=FILE` or `new=CLASS`, `game-seed=N`, `difficulty=D`, `warp=L`,
`kit=0|1`). Then one action per line, `STEP ACTION`, steps never going
back; `#` starts a comment. Actions: `click L|R X Y`, `move X Y`, `key
NAME` (a `controls` action name), `interact UT GUID`, `pick GUID 0|1`,
`drop GUID`, `insert GUID X Y PAGE`, `remove GUID`, `equip GUID BODY`,
`belt GUID SLOT`, `usegrid GUID`, `usebelt GUID`, `wp GUID LEVEL`, `wait
N`. A replay runs each line's action before its step's frame; an action
naming a unit the model does not hold is refused (rule 1.5).

### 3. The checks (after every frame)

1. **GUIDs:** two listed units of one type with one GUID; a GUID past its
   type's counter; a (type, GUID) that left the lists and came back as
   another class (reuse).
2. **Vitals:** life, mana or stamina below 0 on a player or monster; a
   negative maximum on a player.
3. **Rooms:** a unit whose path room is not its list room; a unit outside
   its path room's sub-tile rectangle (closed containment).
4. **Items:** an item in two inventories, twice in one (two grids, or
   the cursor and a grid), in an inventory and a room, on the ground
   (mode 3) and in an inventory; a grid cell whose item is not in the
   grid's list; the item data's inventory not the one listing it; a mode
   that does not match the place (body 1, belt 2, page 0, cursor 4;
   socketed 6 allowed anywhere).
5. **Model:** a new unhandled, rejected or discarded S→C message in the
   bridge's receive log (`bridge.md` §6). A dropped one (a unit message
   for a unit the model lacks at receive) is not a finding: 1.14d drops
   it too (`client/model.md` §4 r6).
6. **Hang:** 250 frames without a server tick while the game is not
   paused (UI state 9 or 11 open, `bridge.md` §8 r5). A run that does not
   exit in its wall-time limit is a `timeout` (`soak.py`), a run that dies
   a `crash`.
7. **Panic:** a panic in any frame or input (the run stops), or the
   server thread stopped.
8. **Desync:** a difference between the model and the server that holds
   for 50 checks in a row (the model trails by the round trip and the
   walk prediction leads): the local player's GUID; its position more
   than 10 sub-tiles apart; its life (whole points, not while dead); one
   of its items missing in the model, placed differently (mode; page and
   cell for mode 0; body location for mode 1), or in the model only.
9. Each signature is reported once per run. A signature carries no step,
   frame, GUID or coordinate, so a replay or a reduced log gives the same
   one.

### 4. Reduction (`soak.py reduce`)

1. Replay the log; it must give the signature.
2. Drop every action after the finding's step, if the signature stays.
3. Delta debugging (ddmin) over the remaining action lines, each test a
   replay with a wall-time limit, at most 400 replays.
4. The reduced log goes to `OUT/repro/<sig>.log`; the repro command is
   `d2-client soak --replay LOG --steps N --keep-going`.

### 5. Save/load round trip

At each milestone of a scripted run (the join, after moving items, after
a waypoint, after a level warp, after spending points) and from each act
save: Save and Exit through the server's leave, read the `.d2s`, join
again from it, compare the full state (header, stats, skills, waypoints,
quests, mercenary, items with their bytes and places: inventory, stash,
cube, belt, body, corpse) and the `.d2s` bytes of a second save made
right after the reload (save time and checksum excluded; `d2s.md` §8.2
rule 7's load-cleared 0x2000 flag is the spec's, not a difference).

## Constants & data dependencies

The kit codes, the waypoint level list of `gen.rs` (`levels` rows with a
waypoint) and the `objects` rows with `operatefn` 23 (waypoint classes).

## Randomness

The generator's splitmix64 only; the game's RNG is untouched by the tool
(the kit pokes are game-side item creations, like any poke).

## Edge cases & original bugs

None (d2rs-own).

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| a generated log, replayed twice | identical reports | `soak` replay, 2026-10-09 |
| `ddmin` on 0..40 with "7 and 31" | `[7, 31]` | `soak.py selftest` |

## Provenance

d2rs-own tool (q-tool-soak).

## Open questions

1. Unit clicks project with a fixed 32 × 16 sub-tile around (400, 290),
   not the play camera: a click lands near, not always on, the unit.
