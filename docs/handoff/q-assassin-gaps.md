# q-assassin-gaps: Assassin traps, dual claws, finisher (`claude/q-assassin-gaps`)

Nothing here is verified against 1.14d (rule 10). PROVISIONAL: REC-241
(`docs/HANDOFF.md` §7). Synthetic fixtures only; sound not wired.

## Links connected

| # | Gap | Fix |
|---|---|---|
| 1 | A laid trap (srvdo 45) was only a pet: it followed the owner and meleed | `BodyEffect::SentryLaid` → `ActionHooks::sentries` (owner, skill, level, shots = `calc4`); `d2-server` `sentry_drive.rs` runs the §14 steps each frame (owner left/dead/town or no shots → death; hostile in range → shot, shots − 1, the class's `Skill1` through the existing monster skill start/do); the followers' think skips traps and shares `nearest_hostile` |
| 2 | A pet's skill never hit (pets were not hostile to monsters) | `sync_seams`: listed pets are on the player side |
| 3 | Dual claws: srvdo 35's two-claw test never ran (`has_inventory` / `item_usable` were defaults) | `LocalSeams` answers both from the weapon copy; two claws strike on two frame events, one claw once |
| 4 | Finisher after a charge | already worked; now tested (extra strike, charge state gone) |

## Tests (`crates/d2-client/tests/app_assassin_gaps.rs`, 4, all pass)

`a_trap_without_a_target_holds_its_shots`, `a_trap_shoots_in_range_then_dies`
(3 shots, hurt, trap gone), `dual_claws_strike_twice` (fails without
link 3), `a_finisher_spends_the_charge`. The rig of `app_skill_gaps` now
takes the PGSV group flags from the state records when `pgsv` is given
(it had none, so no state was progressive); the 12 `app_skill_gaps`
tests still pass.

## PROVISIONAL (REC-241)

- Shots = the laying skill's `calc4`; range 25 sub-tiles, 15-frame gap;
  no `roll(100)` draw; the shot skill is the class's monstats `Skill1`.
- The second claw uses the same weapon damage as the first.
- Pets are on the player side in `sync_seams` (alignment effect unwired).

## What's left

Trap missiles on live data (the test shoots a melee skill), per-trap AI
params, Dragon Talon / Flight (q-skill-moves), real `skills.txt` rows.

## The user's local check (Windows, PowerShell)

```powershell
git fetch origin claude/q-assassin-gaps; git checkout claude/q-assassin-gaps
cargo test -p d2-client --test app_assassin_gaps --test app_skill_gaps
```
Expect 4 and 12 passed. In `play --new assassin Test`: leave the camp,
cast Lightning Sentry / Wake of Fire at the ground, let a monster come
near: the trap should shoot it a few times and disappear.
