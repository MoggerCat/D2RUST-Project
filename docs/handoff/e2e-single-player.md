# Handoff: end-to-end single-player test — `claude/e2e-single-player`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud integration-test session, 2026-10-06, task class: integration
from clear specs, medium (METHODS M14). Base: `main` at `fd37fba`. Repo
only, synthetic tables, no game files (M09). For the coordinator to fold
into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here).

## 1. State

`crates/d2-client/tests/e2e_single_player.rs` (3 tests, all pass) drives
the whole stack: `Bridge` → `bridge::local::LocalLink` → `d2_server::host::Host`
(`ProtoSizes`, tick driver, flush) → `SimGame<WorldSim<_>, ActionWorld>`
with `WiredSkills` → the wired `d2-sim` (`wiring::worldgen::WorldSim`
around `wiring::action::ActionSim`). Act 0's DRLG is created through the
level-type dispatcher `WorldTypes` on the recorded Act I init seed
(644409375), game seed 1234. The S→C side runs through the spec dispatch
table (`bridge-dispatch.tsv`, every id `TBD`).

- `single_player_end_to_end`: every step below, with its result codes,
  the exact C→S bytes, the S→C bytes per frame (all empty, see §2), the
  sim state after each step and the `ClientWorld` state (5 frames, 4
  server ticks, no unit, nothing unowned / rejected / discarded).
- `same_seed_same_run`: two runs, byte-identical transcript (every C→S
  byte, result code, S→C chunk, game seed, monster GUID / seed /
  position / mode, active rooms, player mode and mana, seam logs,
  stubbed intents, client state, errors).
- `other_seed_other_run` (M08): game seed 1235 changes the game seed and
  the monster's unit seed; the comparison sees it.

Gate: see §5.

## 2. Steps: what runs, where each stops

| # | Step | Result | Stops at (why) |
|---|---|---|---|
| 1 | Game creation, fixed seed | **runs** | — (`Drlg::create` act 0 through `WorldTypes`: Act I outdoor placer, preset inits; `WorldSim::create_regions`) |
| 2 | Small level, real generator | **runs** | — (level 30, DrlgType 2, `drlg::preset`: 15 rooms of the 40 × 18 synthetic DS1; the player's room streamed) |
| 3 | Room activation and population, join | **runs** | — (client joined without a room; tick 1 runs `client_level_change` → `rooms.md` §4.1: 4 active rooms; the room pass `population.md` §11.1 creates the DS1 preset monster at (40012, 40010)) |
| 4 | Skill cast → missile → hit → damage / death / experience | **runs** (`claude/e2e-combat-path`) | — C→S 0x0C → result 0, mana 4000 − 3328, srvst 4, mode SC; the animation schedule reads the AnimData record of the composed COF name (`formats/animdata.md` §5; the composer itself is animdata OQ2, a fixture seam): event 0 at frame 5, end at 9; the action frame runs `use.md` §5.2/§5.4: srvdo 8 (seam, body catalogued only) and the real missile creation; flight (fixture path) → hit on frame 15 → damage (fixture damage setup `0x0059F900`) → result 3 → reaction `damage.md` §7.1 → kill §7.2 (seam steps in order, death mode DT with the attacker as target) → 100 experience (`vitals.md` §4.2–§4.3). Details: `e2e-combat-path.md` |
| 5a | Drop from the kill (treasure → item) | **runs** (`claude/e2e-combat-path`) | — the death start `0x005A6FF0` (body unwritten: fixture seam sets mode DT and calls the drop) runs `wiring::economy::monster_death_drop`: gate §3.1, TC §3.2, walk on the monster's seed, gold created through `ItemDrops` on the game seed, placed at (x + 2, y + 3) by the start-offset room search and a fixture free-spot seam, added to the room's units in mode 3, gold amount in stat 14 |
| 5b | Pick-up | **stub** | C→S 0x16 → result 0, recorded in `SimGame::unhandled`: no owner spec (inventory / item-use specs not written, `server-items.md` §2) |
| 5c | Buy / sell at a vendor | **stub** | C→S 0x32 / 0x33 → result 0, recorded: the handlers exist (`handlers::world`, `vendors.md` §7) but the server's `ActionWorld` implements only `WorldHost::waypoints`. `d2_sim::wiring::interaction` now provides `VendorDesk` (`VendorWorld`), so the next step is a `WorldHost::vendors` on a host that also holds `Economy` parts (`GameFields`, `ItemTables`, `ItemStore`), `QuestControl`, `VendorTables`, `InteractionState` and a `VendorRest`; even then buy / sell stop at `VendorRest` inventory calls (no inventory spec) and sell needs an owned item (step 5b) |
| 6 | Waypoint travel | **stops at the warp** | C→S 0x49 → result 0; interaction closed (§7 rule 2), destination bit tested, tile code 0, `Pending::warp(player, 31, 0)` called, spawn search `0x00619E50` streamed the destination room (5 active rooms), arrival node prepended (rule 8). The same-act placement `0x00554EA0` (rule 5) belongs to the unwritten path / placement spec, so the player stays where it was and rule 7's room test (player room = spawn room) is false: **no S→C 0x0D** (correct for that state). The bridge's earlier waypoint test gets 0x0D only because its player already stands in the destination's only room |

Fixture choices (no behaviour invented): positions, interaction record,
warp and arrival mode are logged `Pending` answers as in the existing
local-link and worldgen tests; the skill list / right skill / srvst are a
`SkillSeams` book as in the server's skill tests. The travel destination
is level 31, a preset level outside the Act I chain: travelling to a
chain level (Monastery Gate, 26) generates its outdoor neighbours, which
fail on the fixture's empty `lvlsub` rows (`Outdoor(NoSubRows(0))`,
`Drlg(LevelType(7))`), a fixture limit, not a seam fault.

## 3. Fixes made (outside the test file)

1. **`SimGame::tick` ignored the dispatch's tick hooks**
   (`crates/d2-server/src/adapters/sim.rs`). It ran `tick::tick` with a
   private `Steps` wrapper whose `TickHooks` were all defaults, so with
   `ActionSim` / `WorldSim` as the dispatch no room activation (client
   room change, `rooms.md` §4.1), room removal (step 9), level freeing
   (step 10) or room pass (`tick.md` §4, population) ever ran through the
   host (flagged in `server-world.md` §6). Now `Tick` needs `D:
   TickHooks` and passes the dispatch itself (`tick.md` §3: the step
   bodies are the hooks). `Unspecified` gets the default `TickHooks`
   (unchanged behaviour); the two test dispatches `RunLog`
   (`tests/adapters.rs`) and `NoEvents` (`handlers/world/tests/fake.rs`)
   get an empty `impl TickHooks`. All 99 d2-server tests unchanged.
2. **The server's action-seam hosts accepted only a bare `ActionSim`**
   (`handlers/world/action.rs`, `handlers/skills/wired.rs`,
   `handlers/world.rs` re-export). worldgen's dispatch `WorldSim` wraps
   `ActionSim` (field `action`), so a game with population could not
   have waypoint or skill handlers. New trait
   `handlers::world::ActionEvents` (`type X: Pending; fn action(&mut
   self) -> &mut ActionSim<X>`), implemented for `ActionSim<X>` and
   `WorldSim<X>`; `ActionWorld: WorldHost<D>` for `D: ActionEvents` with
   `D::X: Outbox`, `WiredSkills<S>: SkillHost<D>` for `D: ActionEvents`.
   Pure widening: the call bodies are the same.

No `d2-sim` file touched. Files touched outside the test file:
`crates/d2-server/src/adapters/sim.rs`,
`crates/d2-server/src/adapters/handlers/world.rs`,
`crates/d2-server/src/adapters/handlers/world/action.rs`,
`crates/d2-server/src/adapters/handlers/skills/wired.rs`,
`crates/d2-server/src/tests/adapters.rs`,
`crates/d2-server/src/adapters/handlers/world/tests/fake.rs`.

## 4. For the next sessions (in the order that unblocks the most steps)

1. **AnimData record routing** — done on `claude/e2e-combat-path`
   (`e2e-combat-path.md`); the original note:
   `UnitHooks::anim_record` on `ActionHooks` from an AnimData table
   (`formats/animdata.md` §5, by COF name). The player's COF name needs
   the weapon class, which needs equipment (inventory). Unblocks step 4
   up to srvdo; the missile then needs `SkillSeams::create_skill_missile`
   wired to `UseView` (`wiring::interaction::skill_use`) instead of a
   seam, and the kill needs `Pending::reaction` → death →
   `kill_experience` / `ItemDrops`.
2. **Path / placement spec** (`units.md` path spec, `0x00554EA0`):
   unblocks the warp (step 6's 0x0D) and positions everywhere
   (`Pending::position` / `place`).
3. **Vendor world host** on the server (§2 row 5c) and the inventory
   spec (rows 5b, 5c).
4. When the session spec exists, replace `SimGame::join` before the link
   with the 0x67..0x70 flow through `PendingSession`.

## 5. Gate

`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --
-D warnings`; `cargo test --workspace`; `cargo run -p depcheck`;
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`;
`python3 tools/coverage.py --check`. Results in the commit message of
this branch.

## 6. Checks to queue

None new (synthetic run). When steps 4–6 run, the e2e transcript is the
place to replay a recorded single-player session (`record_packets.py`)
through the bridge and compare S→C bytes per frame.
