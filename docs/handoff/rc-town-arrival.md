# rc-town-arrival hand-back (REC-2130..2134 unused)

Base: claude/specs-staging-7 + origin/claude/integ-r16. Checks: the 7
arrival checks (`suite.py --dir traces/checks --filter '*arrival*'`).

## Checks before → after (equal frames / verdict)

| check | before (r16) | after |
|---|---|---|
| rng-town-arrival-ama | 33/33 MATCH | AFTER_RNG |
| packets-town-arrival-ama | 40/40 MATCH | AFTER_PACKETS |
| a1-town-arrival-ama (state) | 40/40 PARTIAL | AFTER_A1 |
| a4-fortress-arrival-ama (state) | 60/60 PARTIAL | AFTER_A4 |
| a5-harrogath-arrival-ama (state) | 60/60 PARTIAL | AFTER_A5H |
| a5-town-arrival-bar (state) | 40/40 PARTIAL | AFTER_A5B |
| draws-town-arrival-ama | DIVERGED row 111 | AFTER_DRAWS |

The state PARTIALs are every frame equal; PARTIAL is only the declared
headless-client gap. The ledger's older "DIVERGED@2" verdicts for the
arrival coverage rows are stale (they also name warp checks not run here).

## Root cause fixed (draws row 111)

The two client-made chickens (set C) never walked: d2rs skipped the
critter's 0xAC set-up and had no client path. Now (`bridge/critter_path.rs`,
spec `client/model.md` §5 r6.4 "Client path of a C monster"):
set-up `0x004AE8D0` on a critter (stats, path + footprint, mode 1, first
frame and direction draws on the unit seed, stop distance); the code-1
walk (target, compute `0x00649970` with town access, mode set 2 with the
velocity half); per update the monster update's path step
(`0x00650840(U, 0x400)`), anim advance, turn `0x00648640`, and the
client-only mode end. Set-C monsters now stand on the footprint grids.
Result: chickens equal at tick 73 (93 WL dir 40 f1 at (80,416), 94
stopped one cell short, NU dir 32 f1 at (48,516)) and their turns match
the 1.14d per-update unit records.

## Open

- draws-town-arrival-ama new first difference: row 172, Warriv's CelDraw
  x/y 303/291 vs 1.14d 304/292 (an S monster on the provisional
  `bridge/motion.rs` track, not set C), then the unit-pass rows go out
  of alignment (1.14d 261 rows, d2rs 268). Size M. Owner (route.py):
  rendering / client path, coordinator.
- REC-742 left: flee (code 0x0C) face `0x00621C00` and `S1mv` path; the
  other modes' mode records (anim kinds 1–3, end kinds). Size S.
- S-monster set-up direction draw (r6.9) is still not run for set S
  (the model has no S-monster path direction). Size S.
- `tools/coverage.py --check` reports 1 error in
  `crates/d2-sim/src/debug/state/tests.rs:409` that predates this branch.
- Ownership: the router gives "client critters" to session
  01JhQc4n (claude/q-fix-seed-order); this branch touched
  `bridge/critters.rs`, `client_missiles.rs` (footprints),
  `monster_anim.rs`, `world.rs`, `world_view/unit_assets.rs`.
