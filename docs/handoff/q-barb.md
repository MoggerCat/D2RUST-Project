# q-barb: Barbarian skills in play (`claude/q-barb`) — UNFINISHED

> Wrapped up early at the coordinator's request. Nothing here was run:
> the d2-client build was still compiling (a first apt install of the
> wayland/alsa/udev dev packages had silently failed; fixed later).
> No REC id was taken (the PROVISIONAL points below get one when the
> work continues).

## Done
- `crates/d2-client/tests/app_barbarian.rs` + `app_barbarian/rig.rs`:
  a Barbarian (class 4) test rig on the synthetic game (skill rows for
  Bash 126, Howl 130, Leap 132, Double Swing 133, Shout 138, Battle
  Orders 149, Whirlwind 151; AnimData for BAA1HTH; leave town through
  the Den cave warp; spawn a still monster; C→S 0x0C / 0x0D helpers) and
  one test, `bash_on_a_monster_costs_mana_and_hurts_it`.
  **State: compiles and runs; fails at the first assertion**
  (`Bash spent mana`: mana unchanged, server `errors` empty, no panic).
  Fixed on the way: `Send`/`move` closures, `skilldesc` 0 on the rows
  (levels.md Edge case 4 panic). Next: check the 0x0D handler reached
  the skill use (is the right skill Bash? is a player in the Den
  reaching melee range? `LocalSeams::use_state` / `in_melee_range`).
- Merged staging (q-amazon: `app/weapons.rs`, `bodies/passive.rs`).

## Findings (read from code, not run)
- d2-sim already has the bodies: srvst 32/38/40, srvdo 2/22/68/70/76/77.
- `LocalSeams` (app/single_player.rs, skill_rest.rs) leaves `body_effect`,
  `body_path_op`, `line_clear` (blocked), `buff_refresh` as no-ops/default:
  Leap and Whirlwind movement (path ops), warcry buff refresh need them.
- Masteries: reuse q-amazon's passive refresh; learning via 0x3B is
  refused in `LearnRest::is_class_skill` (always false) unless q-levelup/
  q-amazon changed it — check.
- Warcries: ring of 64 missiles + `shout_state` (needs `state_ok`, states
  table in the synthetic tables).

## Left
Everything beyond the Bash rig: run/fix Bash; Howl/Shout/Battle Orders
(state on caster, via q-states-auras messages); Double Swing; Leap and
Whirlwind movement seams; masteries; REC id; handoff local check.

## Local check
`cargo nextest run -p d2-client --test app_barbarian` (expect failures to
fix first), then `play --new barbarian Test`.
