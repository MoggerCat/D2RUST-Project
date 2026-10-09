# q-fix-d4-placement — hand-back (2026-10-09)

Session q-fix-d4-placement (branch `claude/q-fix-d4-placement`), ledger
first divergence D4 (placement). REC ids used: REC-1390, REC-1391.

## Done

Check results after the fixes (1.14d under Wine vs d2rs, `scenario_diff.py`):

| Check | Before | After |
|---|---|---|
| merc-rogue-cow | DIVERGED 3/120 (f4 `tx`) | PARTIAL 120/120 |
| merc-desert-cow | DIVERGED 3/120 (f4 `tx`) | PARTIAL 120/120 |
| merc-sorc-cow | DIVERGED 3/120 (f4 `tx`) | PARTIAL 120/120 |
| merc-barb-cow | DIVERGED 3/120 (f4 `tx`) | PARTIAL 120/120 |
| dru-tornado | DIVERGED 29/70 (f30 `xf`) | PARTIAL 70/70 |
| warp-cold-plains-ama | DIVERGED 9/60 (f10 `x` 5168 vs 5183) | unchanged (open, below) |
| combat-pop-cold-plains | DIVERGED 3/400 (f4 `x`) | not re-run: same arrival point as warp-cold-plains |

Regression runs with the changes: ass-lightning-sentry and dru-fissure
stay PARTIAL; ass-fire-blast (f28 game seed) and dru-volcano (f34 missile
seed) keep their earlier first divergences (both were checked with and
without REC-1391: identical).

Fixes, in the order the checks showed them:

1. **Pet follow of a poke `warp` ran a tick late** (hireling `tx` at f4).
   `0x0053AEC0` follows the pets inside the placement; d2rs queued the
   follow and drained it at the end of the next tick, after the units
   moved, so the hireling's walk ended one frame late. The app's poke
   entry (`crates/d2-client/src/app/poke.rs` `apply_now`, `goto_now`)
   now runs `WiredWorld::handler_work` (made `pub`) when the directive
   returns, as the C→S handlers already do.
2. **Scan 6 took units without unit flag 0x4** (f17: the hireling chose
   the cow). `ai.md` §5.3 scan 6 rule 1 skips a candidate without
   `0x00451F30(C, 4)`; the cow (class 179) has no monstats2 `isAtt`, so
   1.14d never targets it. `LocalSeams::secondary_target` now skips
   units without `IS_ATT` (`not_att`, filled by `sync_seams`).
3. **Mode-change result** (f33: wander then escape). PROVISIONAL
   REC-1390 (`sim/units.md` §4.6): `0x005A7C20` returns 0 when the
   requested mode's start failed and neutral ran instead, so the AI's
   failure branches run. `units::modes::monster_set_mode_started`;
   `ActionHooks::monster_set_mode` returns it.
4. **Missile snap to the current path point** (tornado f30). PROVISIONAL
   REC-1391 (`sim/pathing.md` §9.6 rule 3): the snap runs for missiles
   too; the tornado's charged-bolt path starts with its own cell, so the
   first step is Δ = 0, index 1, aim at point 1, as 1.14d shows.

Tests: `motion_tests::a_walk_without_a_path_point_reports_a_failed_mode_change`,
`a_charged_bolt_path_snaps_to_its_start_point_on_the_first_step` (fails
without the change), `target_search_tests::the_secondary_search_skips_a_unit_without_flag_4`;
`without_the_provider_the_monster_does_not_move` now expects the failed
result.

Ledger part: `docs/handoff/ledger/q-fix-d4-placement.tsv` (9 rows: the
hireling rows, the tornado rows). `ledger.py --check` reports them as
contradictions only until `checks-status.md` is regenerated from a suite
run that includes this branch.

## Open

- **warp-cold-plains-ama / combat-pop-cold-plains**: the arrival point is
  the waypoint preset's (`drlg/levels.md` §10 rule 4); d2rs places the
  Cold Plains waypoint (+15, +5) sub-tiles off 1.14d's, so the player
  lands at (5183, 4663) instead of (5168, 4658). That is task 2 of
  q-fix-a1-den-wp (session_011nm8wnAZ8dXwKuehwW9a8D), messaged at the
  start of this session (no answer yet); not touched here, per the brief.
- **The merc-*-cow checks never exercise hireling skills**: in 1.14d the
  hireling never attacks the cow (no `isAtt`), it only follows and
  wanders. A hireling skill check needs an attackable target (e.g.
  `poke spawn 19` fallen) — new checks for the skills rows.
- **ass-fire-blast**: d2rs's Fire Blast explodes on its first tick at
  (5144, 4263) (f28) while 1.14d's flies to (5151, 4262) and explodes at
  f41. Pre-existing (same with and without REC-1391); the checks-status
  row names q-fix-real-unit-seed-order (game seed at f28), but the first
  visible cause is the missile's early hit (missiles: q-diff-skills-2);
  reported to the coordinator for routing.
- PC 1 reads for REC-1390 and REC-1391 queued in `docs/handoff/pc1-data.md`
  Step 4.

## Repro

```
sh tools/coord/session-setup.sh            # from claude/coord-resume-3
export D2_GAME_DIR=$HOME/game CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
for c in merc-rogue-cow merc-desert-cow merc-sorc-cow merc-barb-cow dru-tornado warp-cold-plains-ama; do
  python3 tools/scenario-diff/scenario_diff.py traces/checks/$c.check
done
```
