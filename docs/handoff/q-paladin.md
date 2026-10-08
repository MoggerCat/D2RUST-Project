# q-paladin: Paladin skills in the play preview (`claude/q-paladin`)

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Synthetic fixtures only. Nothing here is verified against
> 1.14d (rule 10); fills are marked `d2rs-own, unverified`, spec gaps
> PROVISIONAL (REC-153). Sound not wired.

## 1. The path and what is connected

The cast path (q-skills-cast), state messages (q-states-auras) and client
missiles (q-missiles-draw) are reused unchanged.

| # | Skill | Link | State |
|---|---|---|---|
| 1 | Holy Bolt (srvmissile) | cast → mana → server missile → 0x4D | already worked; test `holy_bolt_costs_mana_and_creates_the_missile` |
| 2 | Blessed Hammer (srvdo 73) | do step → missile | the do step ran, but its path calls (`Type(14)`, `Compute`) went to `Pending::body_path_op`, a no-op: **now real** (`wiring/interaction/body_path.rs`) |
| 3 | Might etc. (srvdo 65 aura) | state set, 0xA8 to the client | worked with a states table, `BodyTables` and the skill's `aura` flag (periodic refresh); test `might_sets_its_state_and_the_client_hears_of_it` |
| 4 | Charge (srvst 31 / srvdo 67) | `has_path` was the host's default `false` (start refused), path ops no-ops, and the point target never reached the path | **connected**: `UseView::has_path` reads the path provider, `path_op` runs Velocity / TargetPoint / TargetUnit / Type / Steps / masks / Compute on it, and the server's `start_mode` sets the path target point for a run-mode skill used at a point (`body_path::run_to_point`). Test `charge_at_a_point_moves_the_player` fails before |

All tests: `crates/d2-client/tests/app_paladin.rs` (test-local skill rows).

## 2. PROVISIONAL points (REC-153)

- Where a skill mode's point target reaches the player's path is not
  specified (`use.md` §4); only the run mode (3) takes it.
- Path operations other than the ones listed (`Clear14`, `Face`, `Reset`,
  `TurnToward`, `SnapCenter`, `Op649070`, `Op648E40`) still fall to the
  host seam's no-op.
- Not exercised: Sacrifice, Smite, Zeal (they need a monster with
  `monstats` / `monstats2`, a weapon or shield on the server player, and
  the hit rolls; the synthetic game has none). Their bodies are the
  d2-sim ones (`b3_lvl01`, `starts*`), covered by unit tests only.
- Blessed Hammer's client missile flies straight (q-missiles-draw); the
  real hammer spirals.
- Aura skills: the preview row needs `aura` for the periodic refresh;
  with real `skills.txt` data it is set by the table.

## 3. What is left

Melee skills end to end on a server monster (hit, damage, get-hit on the
client), Charge's strike on arrival, the spiral hammer, aura tints and
Holy Shield / Holy Fire damage-aura pulses on monsters.

## 4. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/q-paladin
git checkout claude/q-paladin
cargo run -p d2-client --release -- play --new paladin Test
```

What to see: bind Might on the right skill: right-click once, the aura
overlay plays and stays; Holy Bolt / Blessed Hammer: walk out of the
camp, right-click the ground, a missile flies; Charge: right-click a far
ground point, the Paladin runs there (not a walk). Copy any `Anim(NoRecord)`
or `effect art` line into `docs/HANDOFF.md` (REC-153).

Headless: `cargo test -p d2-client --test app_paladin`.
