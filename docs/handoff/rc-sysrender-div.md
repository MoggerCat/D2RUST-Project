# rc-sysrender-div hand-back (REC-2980 used, REC-2981 used; both PROVISIONAL)

Base: integ-r23. Scope: ledger `system.render.*` DIVERGED (78) and `render.*` DIVERGED (90). EQUAL 0 -> 0 (3 checks now PARTIAL, 0 differences).

## Fixed (gen-render-* checks, first difference moved)
- dawn/night/blood-moor: first diff was rain DrawLines (1.14d draws other particles per run) then the wall-clock cursor. New check directives `skip-weather` / `skip-cursor` (scenario-diff.md §2; facts-render.md §6 r5/r6; `facts-compare --skip-cursor`; skipped rows leave the frame `draws` count; `index_sha256` not compared). Generated render checks carry both and pin the pointer. Result: dawn/night/blood-moor 100% rows, verdict PARTIAL (d2rs leaves x/y/w/h/light/pal cells unmeasured). Not claimed EQUAL: coordinator to rule whether draws PARTIAL counts (REC-2980: cursor skip is provisional).
- Warp out of town: 0x15 placement ends in `0x00463B80` (mode request 7 -> neutral mode of the new room): player drew TN, 1.14d NU (firebolt/blood-moor 89/90% -> 98.9/100%).
- A 0x15 placement turns no unit (view facing took the jump as a turn: dir 57 vs 0).
- Neutral-mode player frame loops from the model (restarts at the mode set), was a global tick phase.
- Server-created monster: initial path direction (msg-units §1.2 r6.9; measured: after the frame draw), REC-2981 (base classes 96/301 not run).

## Open (sizes)
- cow (poke spawn) frame at draw: 1.14d 1 vs d2rs 7 (firebolt), 7 vs 1 (frozen): constant half-cycle offset (6 of 12?), after the dir fix. S.
- den-of-evil 44.6%: row 70 shadow frame 3 vs 1 (monster frame). S-M.
- kurast-rain 37.5%: row 97 op CelDrawShadow vs CelDraw. M.
- 28 rows (camera/sprite-placement/unit-composite, `local-pc1-today`) need Windows pixel compare; render.effect/scene rows (90): need the waypoint scenes re-run (Wine ~10 min per group); torch (rc-draw-row173) not touched.
- cloud: disk hit 0 once; deleted target/debug.
Tests: d2-client --lib 2317 pass (before the monster-direction commit; not re-run after, disk).
