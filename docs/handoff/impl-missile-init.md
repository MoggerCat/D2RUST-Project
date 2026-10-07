# Handoff: missile init callbacks at creation — `claude/impl-missile-init`

Cloud implementation session, 2026-10-07, medium (METHODS M14). Base:
`claude/specs-staging-6` at `65fad0b`. Repo only, synthetic tables and
DRLG (the action fixture), no game files (M09). Task: the missile init
callbacks of `docs/handoff/impl-path-motion.md` §5 (`missiles.md` §R2.3
step 21, §R9.4; `skills/bodies-2.md` §2.3, `bodies-3.md` §5.28, §3.8,
`bodies-4.md` §2.4–§2.5, `bodies.md` §8.19). Nothing is checked against
1.14d: everything here is **wired, unverified**.

## 1. What landed

- **Step 21 has the store.** `missiles::init_cb::run` (new,
  `crates/d2-sim/src/missiles/init_cb.rs`) is called by
  `create_missile` step 21 with the creation `Ctx` (store + world), after
  the stat list (step 20) and before the damage setup (step 23). It runs
  every init callback the specs describe; an id no spec names still goes
  to `MissileHooks::init_callback` (`Pending::missile_init_callback`).
  The missile spec's own `zigzag` `0x005AC040` keeps its dispatch.
- **Callbacks** (bodies already in `skills::use_::bodies`, now run on a
  real missile through an adapter implementing `JitterMissile` /
  `PathMissile` over the store and `MissileWorld`):
  - jitter `0x005C9290`: frames capped at 77 (total and left), seed :=
    `init_low(P target x + a)`, type 10, step counts n, compute;
  - DiabWall `0x005CD110`: as jitter's re-seed, one draw `lo' mod 100`,
    r ≥ 20 → type 10 + compute, else the straight path stays;
  - lightning fan `0x005D4680`: seed := `init_low(a)`, two draws (s, k),
    n points written one by one along `dir8(d += k·s)`, a draw every
    15th point, point count n;
  - lightning ring `0x005D40F0`: seed := `init_low(a)`, type 10, compute;
  - damage percent `0x005DB6A0`: a ≠ 0 → stat 25 += a.
  Seed re-inits and draws are in the callbacks' order (§R9.4); the
  charged-bolt compute draws its own (`pathing.md` §11.2).
- **New `MissileBodies` seams** (default: 0 / nothing), answered by the
  path provider in `wiring/path/missiles.rs`: `dir64` (`0x00621DC0`,
  `pathing.md` §8.3 rules 1–3 from the missile's sub-tile),
  `set_path_point` (+0x9C array), `set_path_point_count` (+0x28,
  `0x00648790`).
- **`e2e_single_player.rs`: not changed.** With the provider on, the
  test no longer stays green: the fixture's arrow flew one sub-tile a
  frame on the fake path; on the provider it flies its real path, so the
  hit on frame 15 and everything after it (death, drops, the transcript)
  move. Turning it on means reworking steps 4 onward of that test
  (`fx.pending().pos` reads also become `path_position`); that belongs to
  a d2-client session. The WP1 comments are still stale (WP1 itself is
  gone, `impl-path-motion.md` §1).

## 2. Tests

- `cargo nextest run -p d2-sim init_cb_tests` (7, `wiring/path/init_cb_tests.rs`,
  provider on unless stated): jitter cap 100 → 77, seed `init_low(82)`
  + 38 charged-bolt draws, 39 points; under the cap 50 frames, 26 points
  (M08: another argument, another path); DiabWall r < 20 straight (one
  point, one draw) vs r ≥ 20 type 10; fan points equal to a model of the
  rule text, 6 draws for 50 frames (M08: other argument); dir64 values
  on the 8 neighbours and scale-free; ring seed `init_low(a)`; damage
  percent 37 / 0; provider off: cap and re-seed still run.
- `missiles::tests::creation_runs_the_skills_init_callbacks_after_the_stat_list`
  (fake seams: order statlist → build, the host hook not called; M08: an
  unknown id still reaches the host hook) and
  `creation_start_frame_shortens_the_frames` (step 11, previously
  uncovered).
- Coverage: newly covered `missiles.md` §R2.3 r11, §R9.4;
  `bodies-2.md` §2.3; `bodies-3.md` §5.28, §3.8; `bodies-4.md` §2.4 l2,
  §2.5; `bodies.md` §8.19 r5.

Gate at the pushed head (`CARGO_INCREMENTAL=0 sh tools/gate.sh`): every
step PASS except `test d2-client`, whose only failure is
`app_frame_loop::frame_loop_ticks_the_server_and_feeds_the_world_view`
(`handled` 20 vs 19 expected at line 170). It fails the same way on the
base `65fad0b` with this branch's changes stashed: it is not from this
branch. It sits in the game-start area (impl-app-gamestart).

## 3. Spec gaps (for PC 1)

- **MI1** `skills/bodies-3.md` §3.8: `0x00621DC0` → `0x0064FDC0` does
  not say which coordinates it hands §8.3 (the unit's sub-tile or its
  16.16 path position, and the fraction given to (x, y)), nor whether
  §8.3 rule 4 (flag 0x200 flip, at `0x0064FED5`) is part of it. Sub-tiles
  are used, rule 4 is not applied (TODO in `wiring/path/missiles.rs`).
- **MI2** DiabWall seed source: `missiles.md` §R2.3 step 21 says
  `0x005CD110` re-seeds from "path first point x + a caller value";
  `bodies-3.md` §5.28 (the owning spec) says P target x + a. The
  latter is implemented; one of the two specs needs correcting.
- **MI3** `bodies.md` §8.19 / `bodies-2.md` §7.2: `0x005DB6A0` "its value
  + a": whether the read is the full stat or the base, and that the write
  is the base, are not stated. Equal at step 21 (fresh stat list).
- **MI4** `bodies-4.md` §2.4: the fan writes points and the count but
  says nothing of the current point index (+0x24) or the flags a built
  path sets; left as the creation build left them.

## 4. Left

- `e2e_single_player.rs` with the provider on (§1), d2-client session.
- The pre-existing `app_frame_loop` failure (§2).
- Uncovered `missiles/*.md` rules still open: `missiles.md` §R1 r1,
  §R2.2, §R2.3 r9, §R2.4 (0x73, not implemented), Edge case 10;
  `missiles/bodies.md` / `bodies-2.md` section texts (`coverage.py`).
