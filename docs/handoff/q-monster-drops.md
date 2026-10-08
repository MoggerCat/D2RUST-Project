# Handoff: monster drops, `claude/q-monster-drops`

## Links connected
- Missing link: `LocalSeams` (the play host's `Pending`) kept the default
  `monster_death_start` (no mode DT set, no drop). Now it calls
  `app::monster_drop::death_start`: sets mode DT, then runs
  `monster_death_drop` (`treasure.md` §3.1–§3.5, §7) on the game's drop state
  (`ActionHooks::object_drops`, the same state the chest drop uses). Items go
  to the one item store; the existing item pass announces them and
  stitch-items draws and picks them up.
- Test: `tests/e2e_full_loop.rs` now runs the real `death_start` (drop state
  moved to `object_drops`): kill → gold on the ground → picked up. 3 pass.

## PROVISIONAL (REC-108, d2rs-own, unverified)
- Free spot is the start spot as is (`StartSpot`); with the path walk-back
  field loaded the sim's own floor drop is used instead of this seam.

## Left
- Not checked against real tables in a play run (synthetic only).
- Gold is drawn with item art only (no gold class, stitch-items).

## Local check
`cargo run -p d2-client --release -- play --new amazon Test`, kill a monster:
it dies and gold/items appear where it stood; click one to pick it up.
