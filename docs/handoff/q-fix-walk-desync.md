# Handoff: q-fix-walk-desync (`claude/q-fix-walk-desync`)

Queue row: the player position desync / walk re-target on a click while
walking. Base: staging `claude/specs-staging-7` plus `claude/integ-r3`
(the input-lock fix). REC block 1250–1259 (1250, 1251 used).

## Cause

`soak.py run --start town1 --seed 7` gave `desync:player-position` at
step 261. Traced per step (client prediction vs server player): the walk
itself was exact; the desync came from four separate seams, each
reduced and fixed in turn.

1. **Soak input the client never sends.** The generator's `pick`
   (C→S 0x16) and `interact` (the interact sender, 0x13 / 0x16) went out
   for any unit within 30 / 25 sub-tiles. The 1.14d client sends them at
   once only within its interact distance (`ui/controls.md` §6 r9.2:
   item / player d ≤ 4, NPC reach 2) and otherwise walks to the unit
   first (code 2 / 4 and a pending interaction). From farther the server
   walks the player (`items/inventory-moves.md` §7.1, `world/objects.md`
   §7.3 r4, `world/npc.md` §2 r3) and the own client is told nothing, so
   the client stood while the server walked (the task's seed-7 finding:
   `pick 6 1`, reduced to two lines).
2. **Wrong distance in the client's decision.** `bridge/click.rs`
   `distance` (the trait says `0x00641530`) used the size-reduced
   `0x006416D0`. Δ(1, 5) from a player to an item read 4 (pick at once)
   where the unit distance reads more, so the server walked
   (`blood-moor` seed 1).
3. **The prediction ignored the player's mode requests.** A hit (S→C
   0x0D code 6, mode 4) or death (code 8 / 9) while walking stopped the
   server player; the client walked on 27 sub-tiles (`town1` seed 3).
4. **No object footprints on the client path.** The server player
   stopped at a colliding object (class 370 at (5128, 5150)); the client
   path walked through it (`cold-plains` seed 2).

## Fixed

- `app/soak/gen.rs`: `pick` and `interact` only where the client sends
  at once (unit distance ≤ 4 for an item, ≤ 2 otherwise); farther units
  are reached by the unit clicks, through the client's own decision.
  `specs/tools/soak.md` §1 r4 updated.
- `app/soak/log.rs`: a click point may be negative (a unit click's
  jitter left the frame; such logs could not be replayed or reduced).
- `bridge/click.rs` `distance`: `0x00641530` (`sim/pathing.md` §9.5,
  `geom::unit_distance`) with the §3 sizes (monster: `monstats2`
  `SizeX`); helper `bridge::objects::unit_distance_at`.
- `bridge/predict.rs` `server_walk`: a mode request other than the walks
  0x00, 0x01, 0x17, 0x18 and the interact sender 0x02 ends the walk under
  way. PROVISIONAL REC-1250 (`client/model.md` §8 r4).
- `bridge/client_path.rs` `stamp_others` + `world_view/walk.rs`
  `other_objects`: model objects with `HasCollision[mode]` ≠ 0 stamped
  as the server stamps them (box `SizeX` × `SizeY`, §3 mask);
  `ObjClientRow::shape` from `d2_sim::wiring::action::objects::object_shape`.
  PROVISIONAL REC-1251 (`client/msg-units.md` §1.3 r2).
- `specs/seams/movement-prediction.md` §2.9 r4–r6 (the three contract
  points); HANDOFF REC-1250, REC-1251; provisional indexes regenerated.

## Checks

- The task's command: `python3 tools/soak/soak.py run --start town1
  --seed 7 --steps 1500 --out OUT` → 0 findings (before: position desync
  at step 261).
- 54 soak runs (towns 1–5, Blood Moor, Cold Plains, Rocky Waste, Spider
  Forest × seeds 1, 2, 3, 7, 11, 13; 1500 steps): no
  `desync:player-position`. One `desync:player-life` (rocky-waste seed 1,
  step 1016, client 46 server 45) — not movement, not fixed here.
- 1.14d under Wine: `python3 tools/scenario-diff/scenario_diff.py
  traces/checks/walk-click-walk-sor.check` (new: four clicks 14 frames
  apart, each while the previous walk is under way) → PARTIAL, "no
  difference in what was compared": 110 frames, 2,725 unit records, x y
  xf yf tx ty d m equal (gaps: `own`, `q`, as every state check).
- Tests: `bridge::predict::tests::a_hit_or_death_request_ends_the_walk_and_a_walk_code_keeps_it`,
  `bridge::click::tests::the_interact_distance_is_the_unit_distance`,
  `soak::log` round trip with a negative click; real data (ignored, the
  real-data gate): `tests/soak_walk.rs`
  `the_client_position_follows_the_server_in_town_and_outdoors`,
  `tests/play_smoke.rs` `the_client_path_stops_short_of_a_colliding_object`.

## Open / routed

- The server's item / interact distance (`d2-server`
  `adapters/handlers/world.rs:1078`) is Chebyshev ("0x00641530 not
  specified", D1); it is specified now (`sim/pathing.md` §9.5). Sent to
  the items owner (`q-fix-items-shop`).
- `desync:player-life` (rocky-waste seed 1; also town1 seed 7 before the
  generator change, step 1441, client 43 server 40): vitals, not mine.
- REC-1250 / REC-1251 need client-side recordings (the client unit's
  position under a hit while walking; a walk into a colliding object) and
  a read of `0x004BC720`.

## Repro

```sh
export D2_GAME_DIR=/home/user/game     # tools/cloud-game/README.md
python3 tools/soak/soak.py run --start town1 --seed 7 --steps 1500 --out OUT
python3 tools/soak/soak.py run --start cold-plains --seed 2 --steps 1500 --out OUT
python3 tools/scenario-diff/scenario_diff.py traces/checks/walk-click-walk-sor.check
```
