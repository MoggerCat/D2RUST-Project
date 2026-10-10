# rc-sysrender-div hand-back (REC-2981 used, PROVISIONAL)

Base: integ-r23. Scope: ledger `system.render.*` DIVERGED (78) and `render.*` DIVERGED (90). EQUAL 0 -> 0. No ledger rows changed (all stay DIVERGED).
Coordinator ruling applied: the skip-weather / skip-cursor directives, `--skip-cursor` and the pointer pin were removed (rule 10); check files, scenario-diff and facts-compare equal integ-r23.

## Fixed (client, d2-client --lib 2322 pass)
- Warp out of town: the 0x15 placement ends in `0x00463B80` (mode request 7 = neutral mode of the new room); the player drew town-neutral, 1.14d NU (`bridge/msg/units.rs`).
- A 0x15 placement turns no unit (the view took the jump as a turn: dir 57 vs 0) (`ClientUnit::placements`, `unit_assets.rs`).
- Neutral-mode player frame loops from the model and restarts at the mode set (was a global tick phase) (`player_anim.rs`, `unit_rules.rs`). Measured firebolt: frames 1, 2, 1 at ticks 5, 8, 22 = 1.14d.
- Server-created monster: initial path direction (msg-units §1.2 r6.9), after the frame draw (the other order gave 40/29 where 1.14d draws 57/46) (`monster_anim::first_direction`). Base-class rules (96, 301) not run.
Measured effect (with the earlier skip directives, now removed): firebolt 89.2 -> 98.9% rows, blood-moor 90.2 -> 100% (only rain/cursor left).

## Open (current run, strict compare)
- town-dawn/night 95.9/95.5%, blood-moor 72.9%: first difference is the rain DrawLines (x 593 vs 79): waits on the footstep fix (rc-audio-fmt-div), then the cursor frame.
- firebolt (94.5%): cow dir 56 vs 57 at row 106; frozen: cow frame 7 vs 0. The poke-spawned cow's unit-seed order. S.
- den-of-evil 44.4%: row 70 shadow frame 3 vs 1 (monster frame). S-M. kurast-rain 35.6%: row 97 frame 8 vs 3. M.
- 28 rows (camera/sprite-placement/unit-composite, `local-pc1-today`) need Windows pixel compare; render.effect/scene rows (90) need the scene re-runs under Wine; torch: rc-draw-row173.
- Cloud: disk hit 0 once (deleted target/debug, ~/.wine-d2-suite-*).

## Round 2 (REC-3570..3589 unused)
- C009 (player cast shadow): a client skill start faces its cast point/unit (modes 0x15/0x16, whole turn, PROVISIONAL) and an ending-mode player draws the model's frame. draws-fire-bolt-sor 60.6 -> 69.4%, draws-frost-nova-sor 35.7 -> 38.4%; first diff moved from player shadow row 98 to row 106, the poke-spawned cow's frame (rc-player-mode's cow cause).
- den-of-evil row 70: torch B at (510,324) frame 7/19/10 (1.14d) vs 7/18/9 (d2rs) at ticks 13/27/41, torch A equal: B's object animation speed (`roll` on its client seed) differs, so the room's unit-seed stream order (creation order in the room, incl. client-made units) differs. Needs the 1.14d seed per object; not found. M.
- kurast-rain row 97 is now the rain DrawLines (x 4 vs 47): not mine.
- render.* scenes: fxfireball re-run on integ-r23 (1.14d under Wine ~15 min): all 4 scenes 15-19% pixels, first diff tick 3 `tile_origin_x` at the scene frame: both sides in Cold Plains but the player stands 31/51 subtiles apart (tile_origin 7729,78285 vs 7760,78336) after the click path: walk/click position, not an effect. The other 19 groups share the click path, not re-run (same cause expected).
- Windows rows: queued in pc1-data.md Step 4 ([rc-sysrender-div]).

## Round 3 (REC-3570..3589 unused)
- Object footprints: `bridge/client_missiles::stamp_object_footprints` stamps objects whose init stamped a footprint (`HasCollision[mode]`) as the objects.txt SizeX x SizeY box with `ObjectShape::foot_mask` on the grids the critter placement and missiles read (before: players and monsters only). No gen-render check moved (torches are 1x1).
- den-of-evil torch B (guid 22, room 132): 1.14d speed 212 (= roll 24 on its unit seed), d2rs 207; the seed is the room-132 seed after the critter pass + one step, so 1.14d's room-132 seed stream is 4 steps ahead of d2rs's at B's creation (derived from frames 7/19/10 at ticks 13/27/41: 20-frame loop, created tick 4, speed 212). Checked and ruled out: the placement draws (`0x0046C1A0` matches `critters.rs` draw for draw, incl. the pow-2 branch), the pass order (fresh room 132 seed gives 195), the 4 critters x 1 extra creation step is untested. d2rs also places a rat at (7541,5151) that 1.14d does not show (1.14d has one at +5,-5 of it): the placement test `0x0064D9B0` (size 0, mask 0x3F11) differs at that cell, or the critter walks. Needs the 1.14d client collision flags of room 132 or a recording of the room seed: open (M).
- `d2-client --lib`: 2321 pass, 1 FAIL `app::state_dump::tests::options_parse` ("frame 2; key i" must be rejected as a shared input): comes from integ-r23 (files not touched by this branch), not from these changes.
