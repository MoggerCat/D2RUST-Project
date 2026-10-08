# Handoff: combat smoke test — `claude/q-smoke-combat`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md`. The coordinator folds it.

Cloud session, 2026-10-08. A readiness check before the user's local testing: synthetic end-to-end combat flows through the real play path (the app's bridge, the in-process server on its thread, and the sim; no Bevy window). Base: `claude/specs-staging-7` (merged in at `6b9c00fa`). Nothing here is verified against 1.14d (rule 10). PROVISIONAL: **REC-279** (two points, below).

## What runs

`crates/d2-client/tests/smoke_combat.rs`, 8 tests, all passing and none ignored:

- **`the_synthetic_play_join_is_clean`**: the join on the `play --synthetic` wiring. Nothing is dropped or rejected, and the town's four NPCs reach the client model.
- **`<class>_fights_levels_dies_and_respawns`** × 7, one per class 0–6. Each runs these steps in order:
  1. join;
  2. hire from Kashya (C→S 0x36). Below level 8 she refuses with S→C 0x2A code 11. With quest 2 (Sisters' Burial Grounds) set done, the hire succeeds: the mercenary (class 271) reaches the client model, the hire costs gold, and the server unit carries monster init's flags 0x0A. The chat then ends (C→S 0x30);
  3. leave town (cave entrance → Den of Evil); the mercenary follows;
  4. kill a pack of three: one with the left skill (Attack, C→S 0x06), two with the class's start skill (C→S 0x0D), which costs mana;
  5. experience reaches level 2;
  6. spend a stat point (0x3A) and a skill point (0x3B) on the level-2 skill, then kill a fresh monster with that skill;
  7. a Zombie-AI monster hurts the player (the client sees life drop) and kills them;
  8. the death screen; Esc (C→S 0x41) respawns the player in town at the join spot;
  9. back to the Den (the mercenary follows again); C→S 0x16 type 0 on the corpse takes it back: the client sees it leave and the server frees it;
  10. a healing potion on the ground: picked up into the belt (0x16 type 4, re-clicked while a unit stands in the way), then drunk at a quarter life (0x26). The server's life regeneration (stat 74) goes from 0 to positive, the client's life rises, and the potion leaves the belt.

  Every frame asserts three things: every C→S message the server drained was dispatched with result 0; the client log has nothing unowned, dropped, rejected or discarded; and `ActionHooks::errors` is empty. A link tap keeps every S→C message.
- **Test-local fills** (made up, not original values), because the synthetic game has no combat tables: the stat table; 15 skill rows; `charstats` / `experience`; `monstats` row 0 (a killable Zombie) and row 271 (the mercenary, Hireable AI 61), with the synthetic game's other rows kept; AnimData A1 and DT records for the seven player tokens, the Zombie and the mercenary; the full player mode codes (DT … SQ); an `hp1` row added to the synthetic item tables. Test monsters are allocated through `WorldSim::with`, so the monster type init runs (unit flags, AI control). The attacking Zombie gets its first think as a spawner gives it (install state 0, a think next frame; as `start_host_ai`, REC-254).

## Breaks found

| # | Break | Cause | State |
|---|---|---|---|
| 1 | `play --synthetic`: the join drops 4 S→C 0x6D, rejects both join 0x23, and the town NPCs never reach the model | `app::play::run` set client unit and skill rows only for live data | **Fixed** `4ffa6ea9` (`app::synthetic_client::install`) |
| 2 | In the play window, S→C 0x96 is rejected (`model.md` open question 7, the visibility predicate) | Nothing in `src/` calls `Bridge::set_visibility` (`model.md` §13 r6) | **Being fixed by q-fix-visibility** (coordinator). The rig supplies `\|_, _, _\| true` as the headless render input. |
| 3 | The left-skill kill never finished | Fixture gap: raw-allocated monsters lacked monster init's unit flag 0x02 (`use.md` §5.3 step 2 cleared the target), the same as q-play-smoke's F3 | **Fixed** (test): the monsters go through the world's allocation, so `type_init` runs. The kill's experience (monster init's stat 13) is set by the test, as before. |
| 4 | Back in the Den, the corpse shows as a live player (mode 5), not mode 17 | The room switch's add sends 0x59 (mode 5) but not the corpse 0x74 of `intents-events.md` §7.2 part B. No spec gives that call's fields (`wiring/action/switch.rs` module docs). The client's 0x74 sets P to mode 0 when P is not dead (`msg-units.md` §7 r7), so the death-time fields would kill the live local player. | **Open, spec gap** (RE): the arguments of `0x0053DA40` from part B (`0x00571F90` for a unit with state 7). The corpse take-back itself works. The rig finds the corpse as "the other player unit". |
| 5 | After the respawn the client keeps the player at the death spot (20, 60) in the Den, and never sees the town's units around (103, 23) | The server's level warp is right (spawn point, `path-placement.md` §11; server room 4). Its 0x15 (`pathing.md` §10 r3) reaches a client whose player is still dead, and `msg-units.md` §3 rule 4.3 leaves a dead unit where it is. The next message, 0x0D code 7 with record (103, 23), was read as "animation only" (`0x00480EF0`, `model.md` open question 1). | **Fixed, PROVISIONAL REC-279 (1)**: `bridge/modes.rs` code 7 places a unit that was dead at (r0, r1), as 0x15's place (room of the point, position, room-list recache). Test `bridge::modes_tests::code_7_places_a_player_that_was_dead` (fails with the placement removed). |
| 6 | The hired mercenary got no monster init: no unit flags 0x0A, no AI control (its thinks logged `NoControl`) | `WiredWorld::with_economy` ran the handlers on `events.action()`, the bare `ActionSim`, without lending the world state to the action hooks, so `ActionHooks::init_kind` had no monster world for `type_init` (`npc.md` §7.3 step 7 → `0x005B23C0`) | **Fixed**: new `ActionEvents::lend_world` (default: as is; `WorldSim`: `lend`), and `with_economy` runs inside it. Every economy-handler allocation of a monster now gets its type init. The smoke test asserts the mercenary's 0x0A (fails with the old path). |
| 7 | The hired mercenary never moves or fights (mode 1 the whole fight) | `hirelings.md` §3.2 rule 8 (owner link `0x0058F030`) and rule 10 (AI hook 19, `0x005A4850`) go to `AppRest::set_owner` / `hireling_ai` (`d2-client/src/app/rest.rs:241`, `:250`). These keep the owner in the rest's own map and only log a note, so the AI control's `minion_owner` stays empty and Hireable (61, `ai/bodies6.rs:774`) idles. | **Open** (hireling / AI wiring owners): write the owner link into the AI control (`AiStore` control `minion_owner`) and run the AI hook. Then the mercenary leg can assert attack modes (the rig already has the mercenary row, AI 61 and its A1 record). |
| 8 | (synthetic only) After the town's rooms are freed, the town NPCs lose their room (Kashya: unit 3, room none) and do not come back when the player returns | `single_player::build_with_town` places the synthetic town NPCs once (d2rs-own). The room free detaches them, and nothing repopulates them (the live town's NPCs come from presets). | **Open, preview-only**. The scenario hires before leaving town. In `play --synthetic`, the NPCs are gone after a trip out and back. |

## REC-279 (PROVISIONAL; d2rs-own, unverified)

1. **Client mode request code 7 places a dead unit** (`d2-client` `bridge/modes.rs`): `0x00480EF0(U, r0, r1, was-dead)`, with was-dead true and (r0, r1) ≠ (0, 0), places U at (r0, r1) like 0x15's place (`msg-units.md` §3 rule 4, without the free-point fallback). Needed: the body of `0x00480EF0` (`model.md` open question 1), or a 1.14d trace of a respawn showing the client's position after the 0x0D.
2. (No new rule; for the record.) The smoke test's attacking monster gets its first think the way REC-254's host monsters do.

## Other findings

- **The live loading path cannot run on the fixture install.** `single_player::build` on live data needs every act's levels (`DRLG: UnknownLevel(17)` on the base set, `UnknownLevel(40)` on `act1()`). A merged five-act fixture install would let this smoke test run on the exact path `play` uses on the user's files.
- **Gate disk use.** The full `cargo nextest` gate links about 120 Bevy test binaries of about 200 MB each and runs out of the session's disk allowance. With `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_STRIP=debuginfo`, running the gate in chunks and deleting the test binaries between chunks, it fits.

## The user's local check

```
cargo run -p d2-client --release -- play --synthetic --new sorceress Test
```

Expected:

- Akara, Kashya, Gheed and Charsi stand in the synthetic town.
- After a death and Esc, the player stands in town by the NPCs. Before the fix the view stayed at the death spot.
- A hired mercenary appears next to Kashya and follows through the cave entrance, but stands still (break 7).
- After going out and back, the synthetic town has no NPCs (break 8, synthetic only).
- On live data (`D2_GAME_DIR`), until q-fix-visibility lands, walking diagonally near monsters may log `S→C 0x96 refused: … visibility predicate` (break 2).
