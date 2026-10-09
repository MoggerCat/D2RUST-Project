# Handoff: q-fix-real-unit-seed-order — `claude/q-fix-real-unit-seed-order`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md`. The coordinator folds it.

Cloud session, 2026-10-09 (day run, REC block 500–509; none used).
Base: `claude/specs-staging-7` at `87736d7`. Oracle: 1.14d `Game.exe`
under Wine (`tools/cloud-game/`), install from the private repo at
`9710830`.

## Cause (measured)

The join's game-seed order differed by one step. 1.14d's character load
draws a unit seed for the **player** (`0x00552DF0`) right after the
allocation: the first game-seed step after the four of game creation,
before any item the load makes and before the act's DRLG. d2rs drew none
for the player (`units.md` §3.1 r4 says the allocation skips players,
which is right; nothing said the load draws). Every later unit therefore
took the step 1.14d gives the next one.

The row's reading ("1.14d hands the in-between steps to town objects,
d2rs gives them to the starting items") was a misreading: `ScnAma`
(`d2s-tool new`, 848 bytes) has **no items**, and the d2rs run drew no
item seeds either. The "every other value" pattern was the one-step shift
seen against an interleaved list.

Recordings (raw files stay in the container; the values are in the test
and the spec):

```
tools/cloud-game/run.sh --python --seconds 900 -- tools/trace-recorder/record_rng.py \
    --game "$D2_GAME_DIR/Game.exe" --seconds 800 --auto ScnAma --seed 1234 --input "wait 3; end" --out scnama-rng.jsonl
# the same with --auto StubAma (d2s-tool new-stub --name StubAma --class ama --expansion)
# and with --input "wait 90; end" for the town NPCs' first thinks
```

| Game seed step (`-seed 1234`) | 1.14d site | ScnAma (full save, no items) | StubAma (stub) |
|---|---|---|---|
| 2972047412, 1542758918, 1961566614, 2016663226 | `0x547D38`, `0x546CB9`, `0x5360D8`, `0x545F27` | creation | creation |
| 4048349444 (seq 2349) | `0x552E31` | **player** | **player** |
| 108806926, 4040195123, … | `0x552E31` / `0x552E9F` | town units from seq 7270, after the act DRLG | 8 start items (unit + item seed), seq 2352–2402, then the DRLG, then town units from 1222022467 |

The `0x676xxx` draws that seem to sit in the game-seed chain are the Act I
placer's DRLG-seed copy: with `-seed N` the DRLG seed is also `{N, 666}`,
so its states repeat the game seed's. Follow chains by seed pointer, not
by state alone.

## Fix

| Where | Change |
|---|---|
| `specs/sim/units.md` §3.1 r4.1 (revision, Provenance note) | the load draws the player's unit seed; recorded values and order |
| `specs/sim/rng.md` §5.2, §5.3 | the first post-creation step is the player's; the table row says "but a player's" |
| `specs/sim/intents-events.md` §8.2 r2 | the load's first game-seed step |
| `d2-sim` `units::lifecycle::{draw_unit_seed, init_player_seed}`, `View::init_player_seed` | `0x00552DF0` on an allocated player |
| `d2-client` `app/single_player.rs` loader | calls it right after allocating the player (both load paths) |
| `d2-sim` `wiring/action/dying.rs` `new_corpse` | the death corpse draws its seed too (`combat/vitals.md` §4.7 r1.4 already said so; d2rs drew none) |

Tests: `d2-sim` `units::tests::the_player_load_draws_the_recorded_unit_seed_before_the_town`
(CI); `d2-client/tests/app_unit_seed_order.rs` (real data, two tests:
full-save join = player + 24 town units with the recorded seeds; stub
join = player + 8 start items with unit and item seeds). Both real-data
tests fail with the player draw removed (M08 checked).

After the fix every town unit of the arrival takes 1.14d's seed, and the
first thinks of Kashya (rc, 150), Gheed (147) and Charsi (154) draw the
same values on the same frames as 1.14d (Kashya: frame 24 `lo'`
880455687, 87 % → idle; frame 32 3150991505, 5 % → `roll(4)` = 1, the
second node).

## Found, not fixed here

1. **Start cube (REC-244)**: the play host gives a new character a
   Horadric Cube after the charstats items (`WiredWorld::start_extra`,
   d2rs-own). 1.14d gives 8 items (StubAma recording), d2rs 9; the cube's
   two game-seed steps shift every town unit of a new character by two.
   Removing it changes `app_frame_loop` (CI, synthetic), `app_save`,
   `smoke_save` expectations; a decision for the coordinator
   (proposed row `q-fix-real-start-cube`).
2. A stub given as `--save` (`Character::Save` with no body) takes the
   new-character branch without start items (`single_player.rs`
   `load_save` path); 1.14d's stub load makes them (StubAma recording).

## Round 2 — town NPC thinks and the client walk frame

| Fix | Where | Check |
|---|---|---|
| Npc interaction step: every monster has an interaction block; `nearest_player` = scan 2 (client players within 15, PROVISIONAL REC-500) | `d2-sim` `wiring/action/ai.rs` | `the_nearest_client_player_within_15_is_found` |
| Walk in radius `0x005DE4E0`: own + Δ·min(a, dist − b)/dist rounded (PROVISIONAL REC-501), with the staged velocity request | `monsters/ai/tactics.rs`, `ai.rs` | `walk_in_radius_points_follow_the_recorded_walks` (Warriv's 3 walks) |
| Think restart gate `0x00553160` (REC-442 settled by PC 1) | `wiring/action/units.rs` | `a_monster_created_where_no_client_is_gets_no_think` |
| Client monster anim frame: per unit, mode set → 0, §4.7 rate, first frame rolled (r6.5), stat-67 tail; PROVISIONAL REC-502, REC-503 | `d2-client` `bridge/monster_anim.rs` | `a_walk_restarts_at_0_and_steps_three_quarters_a_tick` (recorded ticks 33–89) |

State: `python3 tools/scenario-diff/scenario_diff.py <check with ticks 90>`
(Wine 1.14d vs d2rs) → no difference in m, x, y, xf, yf, seed, stats
over 90 frames; left: monster path target at rest (`tx`, `ty`: 1.14d =
own position, d2rs 0) and the player's `fc`, `sp`.

1.14d per-frame draws for this: `record_frames.py --every 1
--draws-every 1 --ticks 90 --auto ScnAma --seed 1234` (48 drawn frames;
raw file in `traces/raw/`, not committed), split with `facts_render.py
--frame N`.

## Round 3 — start cube, walk message point, walk end

| Fix | Where | Check |
|---|---|---|
| q-fix-real-start-cube: no extra start cube (REC-244 settled by the StubAma recording) | `d2-client` play host | `app_unit_seed_order` stub test pins player + 8 items + 19 town units |
| 0x67 (a, b) = path target for WL / RN (`intents-events.md` §7.4 r4; recorded 0x67 of Kashya at tick 32 carries (4898, 4233)) | `d2-sim` `monsters/mode_message.rs` | `a_walk_to_a_point_sends_the_path_target`; `builder_vectors` input corrected |
| Walk / run end writes NU through `0x00624690` (`ai.md` §1.4): unit queued, flag 0x1 → 0x6D (recorded at tick 67 for Warriv) | `units::modes::write_mode`, `wiring/action/ai.rs` | `the_inner_mode_write_queues_and_flags_the_unit` |

Packets: `record_packets.py --auto ScnAma --seed 1234 --input "wait 40;
end"` under Wine; d2rs's walk messages are now byte-equal at ticks 24,
32, 45, 56.

## Next difference (a1-town-arrival-ama)

Equal through `draws.tsv` row 112. Row 113: the first chicken (ck,
1.14d monster GUID 93). The chickens are client-made (set C) monsters:
the server allocates none (25 unit seeds in 90 s). No spec covers their
creation: `pc1-data.md` Step 4 "[q-fix-real-unit-seed-order]
Client-made critters". Stopped there.

Still open on the state side: the monster path target at rest (`tx`,
`ty`: 1.14d = own position from the add, d2rs 0) and the player's `fc`,
`sp` (path fields).
