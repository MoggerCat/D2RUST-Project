# Coverage: missiles and monsters (branch claude/cov-missiles-monsters)

Details: `cov-missiles.md` and `cov-monsters.md` (this directory).

- Missiles: 79 of 83 rules covered (bodies 33/34, bodies-2 42/43, missiles 4/6, plus the rest). 4 left; no code fixes.
- Monsters: about 202 of 245 covered; 43 left (reasons in `cov-monsters.md`).
- Code fixes found by tests: unit-type guard on elemental umods and teleport, zero-add stat skip, teleport AI flag seam (`set_ai_flag`), negative-radius unit find. New AI modules `skill_check.rs`, `forced.rs`, `scans.rs` are tested against fakes and not yet wired.
- Whole spec set after: 7372/10166 rules covered (72.5%); `coverage.py --check`: 0 errors.
- Gate: tools/gate.sh skipped per coordinator. clippy -p d2-sim clean; `cargo test -p d2-sim`: 3707 pass, 2 fail (`world::objects::tests::routes_match_function_table`, `route_check_catches_perturbations`). The first fails on the base 5bb11ba too, so it is not from this branch; the second is the same test family and was not rechecked on base.
