# q-skill-gaps: Paladin, Necromancer and Assassin gaps in `play` (`claude/q-skill-gaps`)

Nothing here is verified against 1.14d (rule 10). Synthetic fixtures only;
sound not wired. PROVISIONAL: REC-176 (`docs/HANDOFF.md` §7).

## Links connected

| # | Gap | Fix |
|---|---|---|
| 1 | Smite on a player needs a shield: `Pending::shield` / `shield_damage` were the defaults (none), so Smite returned 0 | the weapon copy (`app/weapons.rs`) carries the left-hand shield (`ItemFacts::shield`, `dam` from the items row `mindam`/`maxdam`); `LocalSeams` answers `shield`, `shield_damage`, `has_shield` |
| 2 | Revive's mode request (`mode_request`) was a no-op, so the corpse stayed in mode 12 and the pet sweep dropped it | `UseView::mode_request` runs the monster mode set when the host gives no answer (`wiring/interaction/skill_use.rs`) |
| 3 | The weapon-type test of `use_state` (`itypea1`, claws) was never run | `UseView::use_state` checks the skill's `itypea1..3` / `etypea1..2` against the hand items |

## Tests (`crates/d2-client/tests/app_skill_gaps.rs`, 12, all pass)

Smite with / without a shield, Sacrifice (monster and caster lose life),
Zeal, a curse (state on the monster), Corpse Explosion (damage beside the
corpse), Raise Skeleton and Revive (a pet, the corpse stands), Fire Blast
(generic `srvmissile`), Tiger Strike and a finisher on a claw (damage +
charge), Tiger Strike refused without a claw. `app_skill_gaps/rig.rs` is a
class-neutral copy of the Barbarian rig (the player leaves the camp: a
town room cuts damage).

## PROVISIONAL points (REC-176)

- Shield = left-hand item of type `shld`; the weapon-type test combines
  `itypea1..3` / `etypea1..2` over both hands (the spec names `itypea1` /
  `etypea1` only).
- Monster mode requests run the plain monster mode set.
- Test-local rows; fixture needs in REC-176.

## What's left

Lightning Sentry's own missiles (the sentry AI), dual claws (no
inventory in the preview), Revive's life with real `monlvl`, Dragon Talon /
Dragon Flight (q-skill-moves), real `skills.txt` rows.

## The user's local check (Windows, PowerShell)

```powershell
git fetch origin claude/q-skill-gaps; git checkout claude/q-skill-gaps
cargo test -p d2-client --test app_skill_gaps
```
Expect 12 passed. In `play --new paladin Test`: equip a shield, leave the
camp, bind Smite and right-click a monster (it takes damage); a Paladin
without a shield does nothing. With an Assassin, Tiger Strike needs a claw.
