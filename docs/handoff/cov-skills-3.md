# cov-skills-3 (coverage session)

Branch `claude/cov-skills-3`, base `claude/specs-staging-7` @ 1633f67.
Specs: `skills/bodies-3.md`, `skills/levels.md`, `skills/use.md`. All claims are `unit` tier (unverified against 1.14d).

## Counts (`python3 tools/coverage.py`, covered / units)

| spec | before | after |
|---|---|---|
| skills/bodies-3.md | 24 / 271 (the report's covered column) | 268 / 271 |
| skills/levels.md | 43 / 72 | 72 / 72 |
| skills/use.md | 64 / 65 | 65 / 65 |

(Before-figures are the "covered" column of the first report; uncovered lists were 247, 29 and 1.)

## New code (bucket B/C)

- `skills::stat_cb` (levels.md §7, §6.5): oskill entries, class/tab bonuses, item states, pet maximum (with `pettype_skill_lists`), item auras, charged skills, skill reset, over a `StatCbWorld` seam on `BodyWorld`. Not yet wired into `stats::lists::server_callback` (the host hook): a wiring session must call `skill_stat_callback` from the `StatHost` and implement `StatCbWorld` for the wired world.
- `levels::add_skill_point` (§6.4 handler) with two new `LearnUnits` methods (`send_attack_reset`, `point_client_updates`). TODO: spec does not say whether the step-5 tail runs after a failed spend; it runs only after success here.

## Code fixes found by tests

None. (The bodies-3 §3.3 r6 throw-mastery gate was already on the base.)

## Left

- bodies-3 §1 (conventions text): not testable.
- bodies-3 §3.9 (dead-body footprint): the body only emits `BodyEffect::DeadFootprint`; the host side is a Pending seam in `wiring::body_effect`. Needs the wired host.
- bodies-3 §5.31 text: generic pattern spawn `0x005B3270` and prison placement probe `0x005B34C0` need new seams (`MonsterSpawn::Leader` arg, creation-request variant, owner-data seam). The four-piece case is tested inline via `diab_prison`. Opus session.
- Not run: nothing is verified against recordings; all stay "unverified".

## Gate

`sh tools/gate.sh --no-client`: all PASS except `test d2-sim`: `world::objects::tests::routes_match_function_table` and `route_check_catches_perturbations` fail (objects init/operate routes 35–40, 60); this branch changes nothing under `specs/` or `world/`, so it is on the base (not mine). The d2-client steps could not run: apt cannot install the Wayland/ALSA/udev dev libs in this container (signature error), so the full gate's client steps fail at build.
`levels::LearnUnits::{send_attack_reset, point_client_updates}` default to no-ops because `d2-server/src/adapters/handlers/skills/wired.rs` has its own `LearnUnits` impl and does not call `add_skill_point` yet.
