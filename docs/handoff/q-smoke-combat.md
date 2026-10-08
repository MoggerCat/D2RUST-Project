# Handoff: combat smoke test — `claude/q-smoke-combat`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md`. The coordinator folds it.

Cloud session, 2026-10-08. This was a readiness check before the user's local testing: synthetic end-to-end combat flows through the real play path (the app's bridge, the in-process server on its thread, and the sim; no Bevy window). Base: `claude/specs-staging-7`. REC-279 was assigned but not used: no PROVISIONAL point was added.

## What exists now

`crates/d2-client/tests/smoke_combat.rs`:

- **`the_synthetic_play_join_is_clean`** (runs, passes): the join on the `play --synthetic` wiring. Nothing is dropped or rejected, and the town's four NPCs are in the client model.
- **`<class>_fights_levels_dies_and_respawns`** × 7 (`#[ignore]`, they stop at open break 3). One scenario runs per class 0–6:
  1. join;
  2. leave town (cave entrance → Den of Evil);
  3. kill a pack of three: one with the left skill (Attack, C→S 0x06) and two with the class's start skill (C→S 0x0D, costs mana);
  4. check experience and level 2;
  5. spend a stat point (0x3A) and a skill point (0x3B) on the level-2 skill;
  6. kill a fresh monster with the new skill;
  7. a Zombie-AI monster hurts the player (the client sees life drop) and kills them;
  8. the death screen; Esc respawns the player in town.

  Every frame asserts three things: every C→S message the server drained was dispatched with result 0; the client log has nothing unowned, dropped, rejected or discarded; and `ActionHooks::errors` is empty. The scenario reads the client model for life, mana, experience, level, stat points, skills and monster deaths.
- **Test-local fills**, because the synthetic game has no combat tables (as in `app_play_monster_ai.rs`): the stat table; 15 skill rows (Attack, plus a start skill and a level-2 skill per class, all on do function 1); `charstats` / `experience`; one killable Zombie monster class; AnimData A1 records for the seven player tokens and the monster. These are made up, not original values.

## Breaks found

| # | Break | Cause | State |
|---|---|---|---|
| 1 | On `play --synthetic`, the join drops 4 S→C 0x6D, rejects both join S→C 0x23 (`Fatal(1640)`), and the town NPCs never reach the model | `app::play::run` set client unit and skill rows only for live data. Without a monster row, 0xAC creates no unit (`msg-units.md` §1.2 r2), so the following 0x6D is dropped. Without skill row 0, the 0x23 select fails `msg-skills.md` §2 r3. Every headless app test filled these rows by hand, so no test saw it. | **Fixed** `4ffa6ea9`: new `app::synthetic_client::install` (synthetic unit rows plus skill row 0), called from `play::run`'s synthetic branch. Test: `the_synthetic_play_join_is_clean`. |
| 2 | During combat movement the client rejects S→C 0x96 with `Unspecified("model.md open question 7: the visibility predicate 0x004DBF20")` | `model.md` §13 r6: the render side must give the bridge the predicate (`Bridge::set_visibility`). Nothing in `src/` calls `set_visibility`, so in the **play window** every position check whose server point differs from the client cell on both axes is rejected: no correction and no C→S 0x5F. The window logs it as `S→C 0x96 refused: …`. | **Open.** `VisibleFn` is a plain `fn(&ClientUnit, i32, i32) -> bool`, so rules 1–5 (`rules::unit_visibility::unit_visible`, already written) cannot reach the camera, COF or cel state. Proposed fix (bridge + world_view owners): make the input a boxed `Fn + Send + Sync` holding the world view's unit-composite / cel store and camera origin, and install it in `add_preview*` once the view exists. The smoke rig supplies `|_, _, _| true` as the render input (headless has no render state, §13 r6), so the run can go further. |
| 3 | The left skill (C→S 0x06 Attack, `srvdofunc` 1) on a monster one sub-tile east of the player does not kill it: after 40 × (send + 8 frames) the client model still shows it alive | Not localized yet. Candidates: the fill's monster or player stats (to-hit, damage: player damage comes from stats the synthetic new character may not have), the 0x06 skill-on-unit path in the wired skill slot, or the client's death message. | **Open; probable cause found elsewhere.** The coordinator relayed that q-play-smoke found C→S 0x06 dispatched with result 0 while the attack never starts (mode stays 1, empty skill log). They put this down to a silent exit in d2-sim `use_on_unit` and are fixing it on `claude/q-play-smoke`; no second fix is made here. Once that lands, merge it and remove the `#[ignore]`s to run the rest of the scenario. If it still stops, Next step: per attack, log the server player mode, the monster's mode / life and `ActionHooks::errors` (the snippet was in the session; remove the `#[ignore]` and add it to `Rig::kill`). |

Not reached, because of break 3: right-skill mana, level-up and point spending, the new skill, monster damage to the player, death and respawn after combat, corpse take-back (C→S 0x16 type 0 on the corpse, `inventory-moves.md` §7.1 r2, §12), belt potions (C→S 0x26: the server handler and `use_potion` exist, but the synthetic item tables have no potion rows and the new character gets no start items) and the mercenary run. Death and respawn alone are covered by `app_play_death.rs`, and a monster killing the player by `app_play_monster_ai.rs`.

## Other findings

- **The live loading path cannot run on the fixture install.** `GameData::select` on `test_fixtures::install` (base set or `act1()`) loads, but `single_player::build` on live data needs every act's levels (`DRLG: UnknownLevel(17)` on the base set, `UnknownLevel(40)` on `act1()`). A merged five-act fixture install (`act1` … `act5` together) would let this smoke test run on the exact path `play` uses on the user's files. That is the strongest readiness check still missing.
- **Gate disk use.** The full `cargo nextest` gate links about 120 Bevy test binaries of about 200 MB each (24 GB) and ran out of the session's disk allowance. `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_STRIP=debuginfo` brings them to about 140 MB and fit.

## The user's local check

```
cargo run -p d2-client --release -- play --synthetic --new sorceress Test
```

Expected:

- Akara, Kashya, Gheed and Charsi stand in the synthetic town (before the fix they were not created).
- The log has no `S→C 0x23 refused` lines and no `dropped (unit not in the model) {109: 4}`.
- On live data (`D2_GAME_DIR`), walking diagonally while monsters are near may still log `S→C 0x96 refused: … visibility predicate`. That is break 2: please note how often it appears.
