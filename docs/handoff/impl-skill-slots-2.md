# impl-skill-slots-2 — the batch 2 and 3 skill bodies, player pet lists

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the ninth fold (`claude/fold-handoff-night`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Ninth set" (PC 1 / PC 2).

Branch `claude/impl-skill-slots-2`, on `claude/specs-staging` at `f294bbf`.
Specs: `specs/skills/bodies.md` §6–§8 (batch 2), `specs/skills/bodies-2.md`
(batch 3), `specs/sim/pets.md`, with `specs/skills/functions.tsv`.
Implementation only; no spec edited.

## What landed

- **Bodies** (`crates/d2-sim/src/skills/use_/bodies/`): every
  `functions.tsv` row of status `spec'd-here` now has a body: 45 start
  and 85 do slots (16 before, 114 new: the 29 shared slots of
  `bodies.md` §7–§8 and the 85 single-skill slots of `bodies-2.md`).
  - `helpers2.rs`: `bodies.md` §6 (summon class and spawn, target-node
    insert, base / skill stats, progressive missile, missile ring, shout
    state, sentry, charge counter, golem stats and summon resistance,
    free target point, missile at the point, raise penalty, skeleton
    components, Inferno, missile fan, shadow stats, source link) and
    shared pieces (target position, distance, the batch 3 conversion).
  - `helpers3.rs`: `bodies-2.md` §2 (mode damage, minion damage, the
    jitter callback over a `JitterMissile` trait, potion codes, kick
    damage / hit, knockback column, uninterruptable, nearest, scatter,
    progressive count, claw hit, Leap and Leap Attack helpers, wear,
    Charge helpers, burst, skill result / element length, Plague, base
    roll, `next_unit` / `count_units` / `area_damage` over `BodyWorld`,
    pack and alignment helpers, conversion and its remove callbacks,
    Frenzy, Whirlwind, Blade Shield pulse).
  - `starts2.rs` (§7), `dos2.rs` (§8), `b3_lvl01.rs` … `b3_lvl30.rs`
    (`bodies-2.md` §3–§5, `bodies-2b.md` §6–§8). `run_start` / `run_do` dispatch them;
    `START_BODIES` / `DO_BODIES` list them.
  - Remove callbacks of batch 2 / 3 (`callback::CHARGE`, `INFERNO`,
    `BLADE_FURY`, `SHAPE`, `WHIRLWIND`, `HOLY_FREEZE`, `CONFUSE`,
    `ATTRACT`, `CONVERSION`, `MIND_BLAST`); `remove_callback` now also
    receives the detached list (the shape callback reads its 350).
  - `dos.rs`: the srvdo 30 curse callback computes k = 12 for state 27
    (Taunt, `bodies-2.md` Edge case 10; srvdo 30 never has it); its
    context, `aura_unit` and `spend_aura_mana` are shared with batch 2
    / 3.
- **Seam** (`BodyWorld`): a `Room` type, `KickItems` as a supertrait,
  value seams (rooms, collision, paths, skill-entry params / flags /
  mode, animation, unit fields +0x38 / +0x44 / +0x48 / +0xC8 / +0xD0,
  items, monster creation and mode requests, monlvl rows, pettype count,
  L-flag) and two command seams for calls whose result the bodies do not
  read: `effect(BodyEffect)` (owner data, umods, AI params / commands,
  alignment, packs, equipment, pet add, target lists, kills, removals,
  messages 0xA3 / 0xA5 / 0x7F, item drops, missile data, patterns …) and
  `path_op(PathOp)`. `MissileRequest` carries the full 0x5C record
  (target, velocity, loops, activate, range, init callback);
  `spawn_missile` returns the missile.
- **Wiring** (`wiring/interaction/skill_use.rs`): `UseView` implements
  the new seams on real data where the wired host has it (unit +0xC8,
  +0x44, +0x48, +0xD0, the stat holder, rooms through the DRLG provider,
  list add / clear, unit stat add, handler lookup, monlvl / pettype from
  `BodyTables`, the full missile record on the real missile store) and
  forwards the rest to new `Pending` defaults (narrowest reading:
  nothing done, no monster, no path, collisions true, refusals).
  `BodyTables` gains `monlvl` and `pettype_count` (`from_bin` reads
  `monlvl` and `pettype`).
- **Pets** (`crates/d2-sim/src/player/pets.rs`): `sim/pets.md` §1–§9 over
  a `PetWorld` seam (add with group eviction and maximum trim, append,
  remove / unlink, dismiss, broadcast, lookup, first pet). Not wired to
  a host yet: the bodies' `BodyEffect::PetAdd` goes to `Pending`.
- **Table mirror** (`table.rs`): 130 rows `SpecdHere`.
- **Fixtures**: tests that used slots 6, 8 and 66 as stand-ins for
  unspecified bodies now use still-`mapped` slots 42, 3 and 111
  (d2-sim `fight.rs`, `skill_events.rs`; d2-server skills tests,
  `prop_handle`, `mutants_handlers_skills`; d2-client e2e fixtures).
  `srvdofunc` 116 stays in `mutants_handlers_skills` (the pipeline's
  shape-shift mana rule keys on it; its do never runs there).

## Tests and coverage

- The six target tests pass: `function_tables_match_tsv`,
  `function_table_check_reports_perturbations`, `bodies_match_tsv_notes`,
  `bodies_check_reports_perturbations` (new perturbations: a bodies-2
  note removed, a bodies-2 row demoted, a helper note on an unreferenced
  slot), `table_check_mutants::{repeated_slot_names_kind, index_bounds}`.
  The body-note check accepts an `unreferenced` row whose note names a
  helper body (srvdo 142, the Blade Shield pulse) only when listed in
  `HELPER_SLOTS`.
- `bodies/fake.rs`: `BodyFake`, a `BodyWorld` over `skills::fake::Fake`.
  `bodies/tests2.rs` (23 tests, `bodies.md` §6–§8: ring offsets,
  base stats, charge counter, summon class / spawn, components, Inferno,
  link source, Multiple Shot, dual claws, Inner Sight, shape shift,
  srvdo 66 Edge case 9, Meteor, Fend, Guided Arrow, Armageddon, Raise
  Skeleton, Twister) and `bodies/tests3.rs` (29 tests, `bodies-2.md`:
  player multiplier, potion codes, jitter, scatter, Leap clamp, hit
  frame, burst, conversion, Whirlwind pacing, Sacrifice, Smite, Find
  Potion, Dragon Talon, Corpse Explosion, Shock Field, Bone Wall, Charge
  velocity, Find Item bands, Charged Strike, Fire Wall, Vengeance, Blade
  Fury, Leap Attack aim, Strafe / Fend counts, Bone Prison, Volcano,
  Hydra, Redemption, Whirlwind start). `player/pets_tests.rs`: 27 tests.
- Not every rule unit of `bodies-2.md` has a test; `py tools/coverage.py`
  lists the uncovered ones. No recording covers any body (both specs'
  Open questions).

## Gate (this container)

- `cargo test -p d2-sim`: 2401 passed, 4 failed — only the reds the task
  names as other sessions' (`missiles::tests_bodies::*` ×2,
  `monsters::ai::tests::specd_here_*` ×2).
- `cargo test -p d2-server`: all pass.
- `cargo clippy -p d2-sim -p d2-server --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, `py tools/coverage.py --check`,
  `py tools/spec_index.py --check`: clean.
- d2-client: builds after installing CI's system packages, but its lib
  `bridge` / `ui` tests and the e2e tests fail on the base already:
  `Dispatch::from_spec` → `Mismatch([NoHandler { id: 122 },
  NoHandler { id: 129 }])` (S→C ids without a client handler, from
  another session's TSV change). So the edited d2-client e2e fixtures
  (srvst 42, srvdo 3) are unverified here: queue
  `cargo nextest run -p d2-client --test e2e_single_player --test
  e2e_full_loop` once the dispatch mismatch is fixed; expect the logs
  `"srvst 42 1 10"` and `"srvdo 3 1 10 true false false"`.

## Open questions

1. pets.md §8 vs `sim/server-messages.tsv`: §8 and its test vector put
   the owner GUID at +5 and the pet at +9; the confirmed TSV row (and
   `hirelings.md` §13) give pet@5, owner@9. `PetMsg::bytes` follows the
   TSV (coordinator decision); the test vector is adapted.
2. pets.md: a remove broadcasts 0x7A three times when the monster still
   exists (unlink → dismiss broadcasts too) against the "twice" of Edge
   cases 1 and 4; a GUID in two lists can make §3 / §4 loop forever;
   the add record's +0x10 / +0x14 are never set (read 0 → always 0x7A).
3. bodies.md §6.1: with an invalid skill `summon_class` does not write
   `mode` (d2rs reports 0; every caller tests R first).
4. bodies.md §6.5 step 2: whether `itemevent2` shares the
   `itemevent1` / handler-lookup condition (read as the same branch);
   step 9: whether the ilvl clamps apply to a given ilvl (read as part of
   the ilvl = 0 branch).
5. bodies.md §6.15: a negative skeleton-mastery level reads before the
   component table (nothing written).
6. monsters/init.md §8.1 flag 8: "TH from TH / L-TH" read as pct(monlvl
   TH, the monstats to-hit, 100) in `mode_damage`.
7. Clause scope where the spec separates steps with ";" after "Hit:" or
   "EType ≠ 0:": read as inside the clause (`claw_hit`, Fend, Poison
   Dagger), as `bodies-2.md` §4.3 states for the same wording;
   `leap_strike` (§2.19) and `blade_pulse` (§2.26) apply the combat only
   on a hit.
8. bodies-2.md §2.16: a negative `step2` (read as 1); §6.8: a stored
   hit-class index outside 0…2 sets no class; §7.2 step 8: the Strafe
   rewind is made whether or not K2 exists; §7.10: Bone Prison's "pet type
   ≥ count → 0" read as pt := 0.
9. bodies.md §6.2: `0x0057CCB0(it, 1)` on a replaced linked unit has no
   full argument list (`BodyEffect::KillReplaced`).
10. bodies.md §6.5 step 6 names `0x005701B0(m, 0, k, −1)` "right skill"
    while §2.16 calls side 0 left; passed raw (`SelectSkill { side: 0 }`).
11. Pending on the wired host: monster creation, paths of skill moves,
    entry params / flags, collision and free points, room levels, item
    reads, the pet lists. Each is a `Pending` default with its address;
    the summon, leap, charge, whirlwind and teleport bodies do nothing
    observable on the wired host until those providers exist.
12. The missile init callbacks the bodies pass (`init_cb::JITTER`
    `0x005C9290`, `init_cb::DAMAGE_PERCENT` `0x005DB6A0`) are not routed
    by the missile wiring yet (`Pending::missile_init_callback`); the
    jitter body exists as `helpers3::jitter` over `JitterMissile`.
