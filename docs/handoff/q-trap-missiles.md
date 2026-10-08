# q-trap-missiles: a laid trap fires its missile (`claude/q-trap-missiles`)

Nothing here is verified against 1.14d (rule 10). PROVISIONAL: REC-276
(`docs/HANDOFF.md` §7). Synthetic fixtures only; sound not wired.

## Links connected

| # | Gap | Fix |
|---|---|---|
| 1 | A trap shot a melee skill; with a missile skill (`srvmissile`) nothing flew: the monster had no target position at the do, so `skill_missile_unit` returned none (`helpers.rs:248`) | `UseView::target_position` (`d2-sim` `wiring/interaction/skill_use.rs`) falls back to the unit's path target (target unit's position, else the path's target point) when the host answers none |

The rest of the path already worked (sentry driver → monster skill
start/do → `do_core` → `create_skill_missile` → flight).

## Test (`crates/d2-client/tests/app_assassin_gaps.rs`)

`a_lightning_sentry_fires_its_missile`: a laid trap (3 shots) shoots a
missile skill at a monster in range: a missile flies (fails before the
fix), shots drop, the missile ends at the monster (collide-kill) and the
monster loses life. The 4 earlier tests of the file still pass.

## PROVISIONAL (REC-276)

- Missile damage setup (`0x0059F900`) is not wired in the host: the test
  writes the missile's damage stats (21 / 22) each frame.
- Spawned monsters have no collision footprint: the test stamps 0x100 on
  the target's sub-tile so the missile's hit test runs.
- Made-up rows: shot skill 4 (`srvmissile` 0), missile row 0 with vel 16,
  range 200.

## What's left

Real Fire Blast / Lightning Sentry / Wake of Fire rows (needs `game/`),
per-trap AI params, missile damage setup in the host, footprints for
spawned monsters.

## The user's local check (Windows, PowerShell)

```powershell
git fetch origin claude/q-trap-missiles; git checkout claude/q-trap-missiles
cargo test -p d2-client --test app_assassin_gaps
```
Expect 5 passed. Missiles in `play` only hurt once the damage setup is
wired; a laid Lightning Sentry should now show bolts leaving the trap.
