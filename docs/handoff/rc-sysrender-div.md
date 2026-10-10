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
