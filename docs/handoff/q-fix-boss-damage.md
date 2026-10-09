# q-fix-boss-damage — bosses that never die, monsters that stand back up (2026-10-09)

Branch `claude/q-fix-boss-damage` (from `claude/specs-staging-7`). Real
install (private data repo `install/`, assembled into `/home/user/game`)
for every run below (M23). No REC ids used (REC-1020..1029 free): the fix
follows a 1.14d-read spec rule, nothing provisional.

## Findings

1. **"Monsters stand back up, hp 0" (the real defect, d2-sim).** The DT
   start (`units.md` §4.6 r1.2, the death clean-up's `0x005738D0`)
   cancels the monster's type-2 (AI think) and type-3 (stat regen)
   events. d2rs's death start (`Pending::monster_death_start`, the host's
   `monster_drop::death_start`) did not, so a think queued before the
   kill fired during the death animation and set NU / attack / SQ modes
   before DT's event function could set mode 12. Seen on the Blood Moor
   zombie (`kill-zombie`: DT at f49, mode 5 at f62, then 1/5 forever,
   hp 0) and on Andariel (0 → 14 → 1 → 4 …, hp 0).
   Fix: the same fix reached staging from `q-diff-combat-a1` while this
   session made it (`monsters::ai::cancel_think_and_regen`, run by
   `ActionHooks::monster_mode_function` right after the host's DT
   start; synthetic test `the_death_start_cancels_the_pending_think_and_regeneration`).
   On merge this branch took staging's version and kept only the
   real-data test below.
2. **Andariel "never dies" was the milestone, not the damage.** At the
   `warp 37` spot every cell from `@x+1` east is wall + missile barrier
   (collision 0x5). `pos @1:156 @x+3 @y` put her inside the wall; the
   Fire Bolt's first path step was blocked by the 0x4 bit (`missiles.md`
   §R4 step 2 → expiry, removed) and never reached her. Placed reachable
   (via `goto unit 156`, then `@x+2`), one bolt kills her: fire 1382
   → total 2073 (resist −50%), life 0, DT f41, DD f81 (with fix 1).
3. **Radament "stays at 256".** Three things, none in the damage code:
   - the `find` probe ran only to `deadline` (1000) while the sweep it
     searches runs to its last hop (spec §1 r4); Radament is first seen at
     f1697. Fixed in `tools/playthrough/playthrough.py` `find_probe`
     (its docstring already said "the whole sweep");
   - the sweep's last hop leaves the player in a wall block (every cell
     in a 25×13 window around (7687, 8120) in the Sewers has 0x5; the
     player's own cell reads 0x1085), so `@x+3` was wall again. Not
     analysed further (hop / DRLG area): see Open;
   - next to him (after `goto unit 229`), a Returned skeleton (class 3,
     98 hp, fire-resistant) stands on his sub-tile (7774, 8127) and
     takes the bolt first (`missiles.md` §R4 step 9 finds it first). At
     `@x+2` the bolt hits Radament: DT f1746, DD f1771, stays dead.
4. **Oracle (M25).** `traces/checks/combat-arrow-kill.check`'s pokes
   on d2rs (state-dump, ScnAma, seed 1234): the Quill Rat enters mode 0
   with hp 0 at f38 and mode 12 at f52, the frames 1.14d recorded
   (`docs/handoff/pc1-day3-b.md` Item 4). Same check, not mine: 1.14d's
   rat quill hits the player at f46 (12800 → 12415); d2rs's player
   stays at 12800 (monster missile combat, owner `q-fix-b-monster-combat`).

## Done

| Change | Where | Check |
|---|---|---|
| Real-data check of the DT start's think + regen cancel (fix itself from staging, finding 1) | `crates/d2-client/tests/app_boss_death.rs` | real install; fails without the cancel (Andariel 0 → 14 → 1 → 4 …, hp 0), passes with it |
| `find` probe runs the whole sweep | `tools/playthrough/playthrough.py` | `radament-killed` finds him at f1697 |
| `andariel-killed`, `radament-killed`: `goto` first, the boss at `@x+2`, and `need … dead` at the deadline (stays dead) | `traces/playthrough/act1.play`, `act2.play` | both reached |

Playthrough (`--build`, real install): act1 **14/17** (was 12/17:
`kill-zombie` and `andariel-killed` now reach), act2 **15/15**. Act 1's
three left (`den-of-evil-done`, `andariel-done`, `act2-open`) are quest
/ NPC-click rows, not combat.

## Open

- The Sewers player-in-wall after the sweep's hops (finding 3): does
  `hop` place on a cell with 0x1 set, or is the room's collision wrong?
  Owner: path / DRLG (`specs/sim/path-placement.md`, `specs/drlg/`).
  Repro: `python3 tools/playthrough/playthrough.py traces/playthrough/act2.play --only radament-present`,
  then the state at f1696 (player (7687, 8120), level 49).
- A 1.14d run of the boss kill itself (scenario-diff state channel) was
  not made: no Wine set up in this session. The death timing is
  compared through `combat-arrow-kill` (finding 4).

## Repro

```sh
export D2_GAME_DIR=/home/user/game
python3 tools/playthrough/playthrough.py traces/playthrough/act1.play --build --only andariel-killed
python3 tools/playthrough/playthrough.py traces/playthrough/act2.play --only radament-killed
python3 tools/playthrough/playthrough.py traces/playthrough/act1.play --only kill-zombie
cargo test --release -p d2-client --test app_boss_death -- --ignored
cargo test -p d2-sim --lib the_death_start_cancels
```
