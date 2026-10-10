# rc-wp-walk-tx: hand-back

Ledger part: `docs/handoff/ledger/rc-wp-walk-tx.tsv` (2 rows: level.a3.100, level.a5.132).

## Checks (suite `--checks-dir traces/checks/gen`, base staging-7 53049f11)
Before (measured on staging-7): gen-lvl-100 rng MATCH / state PARTIAL; gen-lvl-132 DIVERGED@97
(rng site 0x552e31 missing); gen-wp-9 PARTIAL. Only the gen-lvl-100/132 and gen-wp-9..17
subset was run; the full `gen-wp-*,gen-lvl-*` run (175 checks, ~2 h) was not.
After: gen-lvl-100 rng MATCH / PARTIAL, gen-wp-9 PARTIAL, gen-lvl-132 still DIVERGED@97 (new first
cause below), gen-wp-10..17 DIVERGED@400 (cause 2 below). No new EQUAL.

## Findings
1. **gen-wp 9-17 player tx 0 vs 5153 at frame 68: gone on staging-7** (wp-9 PARTIAL, wp-10..17 reach
   frame 400). Nothing to fix. They now stop at frame 400 `game.seed` after C->S 0x49 (level change),
   owner `claude/q-fix-join-items` (unchanged from the q-run-gen-wp-shrine hand-back).
2. **gen-lvl-100 site 0x552e31: already MATCH on staging-7.**
3. **gen-lvl-132 site 0x552e31 (frame 97): fixed.** Cause: Baal Tentacle (srvdo 140,
   specs/skills/bodies-4.md 3.22) spawns 3 class-562 monsters; `UseView::spawn_monster` had no
   provider (`body_spawn_monster` default None), so no creation and no game-seed step.
   `crates/d2-sim/src/wiring/interaction/skill_use.rs`: `MonsterSpawn::At` now goes through the lent
   monster world `spawn_at` (fallback: the host seam). Also `BodyEffect::WaitThink` (wait N, 0x005DE0F0)
   was a no-op; now deletes thinks and schedules one at frame + N. Game draw at 97 now equals 1.14d.
   d2-sim nextest 4737 pass, clippy clean.

## Open
- gen-lvl-132 rng frame 112: tentacle 1:9 think draws site 0x5ef875 (roll 10) missing: spawned
  tentacles have mode 1 and a think but no AI control (ai::install runs only in worldgen
  init_units). Needs AI install on `spawn_at` monsters from a skill (small).
- gen-lvl-132 state frame 97: Baal `fr` 1184 vs 18688 (animation frame at the cast); not analysed.
- Other `Near`/`NearLevel`/`Leader`/`Minion` MonsterSpawn variants still have no provider.
- The q-run-gen-wp-shrine orig-cache was fetched into the working tree to run; not committed.
