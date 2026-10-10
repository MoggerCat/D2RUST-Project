# rc-town-arrival hand-back (REC-2130..2134 unused)

Base: specs-staging-7 + integ-r16. `suite.py --dir traces/checks --filter '*arrival*'`.

| check | before (r16) | after |
|---|---|---|
| rng / packets-town-arrival-ama | 33/33, 40/40 MATCH | same |
| a1 / a4 / a5-harrogath / a5-town-bar arrival (state) | all frames equal, PARTIAL | same |
| draws-town-arrival-ama | DIVERGED row 111 (64.9%) | DIVERGED row 172 (66.3%) |

Equal ticks 273/274 → 273/274. State PARTIAL = declared headless-client gap
only; the ledger's "DIVERGED@2" arrival coverage rows are stale (they also
name warp checks not run here). Tests: d2-client 2560/2563, the 3
`e2e_full_loop` failures are identical on the r16 base; clippy clean.

## Fixed: client-made chickens never walked (row 111)

d2rs skipped a critter's 0xAC set-up and had no client path. New
`bridge/critter_path.rs` (spec `client/model.md` §5 r6.4 "Client path of a
C monster"): set-up `0x004AE8D0` on critters (stats, path + footprint,
mode 1, first-frame and direction draws on the unit seed, stop distance);
code-1 walk (target, compute `0x00649970` with town access, mode set 2
with velocity); per update the monster update's movement
`0x00650840(U, 0x400)`, anim advance, turn `0x00648640`, client-only mode
end. Set-C monsters now stand on the footprint grids. Chickens 93/94 equal
at tick 73 (dir, frame, mode, screen position) and their turns match the
1.14d per-update unit records.

## Open

- draws row 172: Warriv CelDraw x/y 303/291 vs 304/292 (S monster on the
  provisional `bridge/motion.rs` track), then unit-pass rows misalign
  (1.14d 261 rows, d2rs 268). Size M; route.py owner: coordinator.
- REC-742 rest: flee 0x0C face `0x00621C00` + `S1mv` path; anim kinds
  1–3 / end kinds for set C. Size S.
- Set-up direction draw (r6.9) still not run for set S. Size S.
- `coverage.py --check`: 1 pre-existing error, d2-sim debug/state/tests.rs:409.
- Ownership: "client critters" is q-fix-seed-order's (01JhQc4n); touched
  bridge/critters.rs, client_missiles.rs, monster_anim.rs, world.rs,
  world_view/unit_assets.rs.
