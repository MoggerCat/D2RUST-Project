# Handoff: every missile server-do / server-hit body — `claude/impl-missile-bodies-2`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the ninth fold (`claude/fold-handoff-night`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Ninth set" (PC 1 / PC 2).

Cloud implementation session, 2026-10-06, task class: implementation
from clear specs, medium (METHODS M14). Base: `claude/specs-staging` at
`f294bbf`. Repo only, synthetic data, no game files (M09). Branch
`claude/impl-missile-bodies-2`, no PR. `docs/PLAN.md`'s monsters line is
updated here; `docs/HANDOFF.md` is left to the fold.

## 1. Result

- **Every non-null catalogue entry has a body.** `srvdo.tsv` (36
  non-null: 1–3, 5–37) and `srvhit.tsv` (53 non-null) are all
  `spec'd-here`; `SRV_DO_IMPLEMENTED` / `SRV_HIT_IMPLEMENTED`
  (`missiles/catalogue.rs`) list exactly them, so
  `missiles::tests_bodies::bodies_match_catalogue_status` is green.
  Only null entries reach the `Unhandled` log now.
- `missiles/bodies_ext.rs` (Spec: `missiles/bodies.md` §1–§30) and
  `missiles/bodies_ext2.rs` (Spec: `missiles/bodies-2.md` §31–§62), with
  their helpers: plague ring `0x005A9370`, `ring8` `0x005AB700`, meteor
  scatter `0x005AAA90`, `nova` `0x0056D4E0` (`nova_offsets` exposes the
  offsets), `zigzag` `0x005AC040`, `fire_disc` `0x005A9530`, bone spirit
  `retarget` / `aim`, Cairn portal `0x005A9930`, `missile_at`
  `0x0056EDE0` (`skills/bodies.md` §6.13, small enough to live here),
  Frozen Orb circle (C64 / S64 as constants), `corpse_effect`,
  `spawn_for_level`, `scatter_at_target`, the owner follow `0x005A99E0`,
  Tyrael's helper.
- `create.rs`: the `zigzag` init callback is the missile spec's, so
  creation step 21 runs it itself (`ZIGZAG_CALLBACK` = `0x005AC040`);
  every other callback still goes to `MissileHooks::init_callback`.
  `MissileParams::gfx` (+0x24) added (server-do 28 writes it; creation
  ignores it).
- `hit.rs`: `damage_tail` (`0x005ADCD0`) split out of the damage stage
  for server-hit 22.
- `Unhandled::Fatal { addr, missile }` logs where the original asserts,
  hangs or reads through a null record (plague ring with a negative
  step, `nova` n ≥ 41, server-do 9 without a record, `spawn_for_level`).
- `full_record` maps the full roll `0x005A89A0` (`hit::Damage`) onto the
  0x70-byte `combat::DamageRecord` the area helpers take.

### Seams

New `MissileBodies` methods (`missiles/seams.rs`), every one defaulted
to the narrowest reading (nothing happens / none / 0): skill columns
(`skill_field` with `SkillField::{Param(n), AuraFilter,
AuraTargetState, PetType}`), `SkillCalc::Calc4`, counts (states,
overlays, pet types, monstats), `skill_phys`, `skill_elem_len`,
`skill_entry_param1`, max life / mana, overlay, large monster, pet /
ally / `accepts` tests, target position, path target point / type /
distance / teleport, room refresh, line test, room seed, `scan_units`
(with `noaura`), skill server-do, `shout_state`, state lists
(`state_list_expiry`, `new_state_list`, `aura_fill`,
`mark_state_changed`, `set_state_list_expiry`, `apply_state`),
`terror`, summons (`summon_class`, `summon_spawn`,
`bind_bone_wall_piece`), portal, sound, chest drop, floor drop spot,
gold, unit mode / `FrameCnt1`, corpse finder, Redemption effect, level
`mon` list, `isSpawn`, monster create / spawn, rabies poison, quest
test, Tyrael.

The wired `View` (`wiring/action/missiles.rs`) answers `skill_field`
and `Calc4`; **every other new seam is unwired**. Providers exist for
several (`skills::use_::bodies::helpers` `scan_unit`, `aura_fill`,
`apply_state`, `accepts`; `monsters::population::preset` `0x0054E600`;
`world::quests` `0x0056D130`; `path::footprint` `0x00650BE0`;
`path::place` `0x00555DA0`) — a wiring task (§3 step 2).

## 2. Tests

- `missiles/tests/ext.rs`: 64 tests on the missile fake, one or more
  per body, from the specs' test vectors (chaos ice turns, plague and
  panther rings, meteor scatter L = 3, bone spirit re-aim, `nova` counts
  and n = 3 offsets, trailing javelin sides, `fire_disc` order and
  r = 2 with a wall, finger mage step, howl level test, blade fury
  class 0, frozen orb (27, −11) and the −70 index, orb nova curl,
  `scatter_at_target` n = 8 / n = 5, healing vortex, Baal inferno,
  recycler, plague vines trail, tower chest, …). Covers claims on
  `bodies.md` / `bodies-2.md` rules.
- `tests_bodies::bodies_check_catches_perturbations`: no `D2MOO-only`
  row is left, so the fixture is (a) a synthetic `spec'd-here` row
  appended at index 53 (past the 53-entry table, so no body can exist),
  reported exactly; the same row with another status is not counted;
  (b) a real row (server-hit 2) demoted to `summarized` while its body
  stays, reported exactly; (c) a body with no row (30) and the null-row
  rule. Green.
- Three dispatch tests used stub entries (server-hit 2, server-do 6) as
  observable no-ops; they now observe real bodies instead (server-hit
  36's allocation, server-hit 18's `shout`, server-do 36's room
  refresh), same assertions otherwise.
- `cargo test -p d2-sim -p d2-server -p d2-data --no-fail-fast`: all
  green except the 8 tests red on the base that belong to parallel
  sessions (`monsters::ai::tests::specd_here_*`, `skills::use_::tests::*`
  tables / bodies, `skills::mutant_tests::table_check_mutants::*`).
  Missile tests 177. clippy `-D warnings`, fmt, `coverage.py --check`
  (5,589 claims, 0 errors) and `spec_index.py --check` clean.

## 3. The small fixes from PC 1's answers

1. **Monster death 0x69 code 8 then 9** (`sim/intents-events.md` §7.4
   rule 7): **not done — nothing emits 0x69.** The server has no §7.3
   client-pass unit messages at all (0x67–0x6D: `d2-proto` audit
   `Partial`, no builder; no sender in `d2-server` / `d2-sim`). Adding
   code 8 / 9 needs that pass first; it is not a small fix. Next step
   below.
2. **A1Q6 Catacombs entry keeps states 4 and 5** (`quests-act1.md` §10.8
   event 3 step 1): fixed in `world/quests/act1/q6.rs` (state := 3 only
   below 3; otherwise unchanged and not "changed"); the slaughter test
   enters Catacombs 1 at states 4 and 5.
3. **`HratliMagicLvl` never binds** (`vendors.md` §1 r1): no code
   change needed — the compiler binds by header name, so the
   weapons / armor header `HratliMagicLvl` leaves `HraltiMagicLvl`
   (offset 400) at its missing-column value 0, and `world/vendors.rs`
   reads offset 400. New check `compile::tests::
   hratli_magic_lvl_header_does_not_bind` pins it with the real field
   definitions (weapons, armor, misc).

## 4. Questions (spec gaps; implemented as stated, marked `TODO(spec…)`)

1. `bodies.md` §6 step 2: the plague ring's flags are 0x17 (and
   `ring8`'s 0x1F), which hold 0x10 "velocity already fixed point"
   (`missiles.md` §R2.1), yet step 3 and Open question 3 say creation
   shifts the velocity << 8 (49,152 for `Param1` 2). Implemented with
   the flag value as stated (no shift: 2 << 7 = 256 → 192 after 75 %).
   Which holds?
2. `bodies-2.md` §41 step 2 (fire head): v = `elem_roll(…)`, whose
   return `missiles.md` §R9.6 gives as `EType` (fire = 1), but edge case
   5 reads v as a rolled amount. Implemented as the return (`EType`).
3. `missiles.md` §R6.2 for an area record: where the full roll's crit
   flag and the 103 / 104 / 106 bypass flags land in the 0x70-byte
   record. `full_record` sets result 0x2000 for the crit (as
   `elem_roll` does) and carries no bypass flags.
4. `bodies-2.md` §60 `tyrael`: is the room refresh inside the quest-test
   branch or after it? Implemented after it (always).
5. `bodies-2.md` §47 `spawn_for_level`: step 4's "c = −1 → return 0"
   versus step 3's fatal read for an invalid class; and what a missile
   with no room passes (also §43 step 4.2's floor drop). Implemented:
   an invalid class is fatal, −1 checked after the loop, no room → no
   spawn / no gold.
6. `bodies-2.md` §44 / §47: the corpse finder `0x0065A950` (Open
   question 3) and the room seed have no provider; without them nothing
   is found / spawned.

## 5. Next steps

1. A task for the §7.3 client pass (0x67–0x6D unit mode messages), then
   0x69 code 8 at death and code 9 at the end of the death animation
   (`intents-events.md` §7.4 rule 7, recorded bytes there).
2. Wire the new `MissileBodies` seams to the existing providers (§1
   Seams); until then the wired game runs these bodies with no-op
   effects for everything another spec owns.
3. Recordings (`bodies.md` / `bodies-2.md` Open question 1; HANDOFF §5
   S9-A4): Cairn
   Stones portal, Baal's taunt, Royal Strike, Plague Javelin, Blade
   Fury, a lightning trailing javelin explosion, Lightning Fury, Bone
   Wall, Battle Cry, Fist of the Heavens, the panther potions.

## 6. Lesson (M21)

One commit (`badcd32`) went out with a clippy error and a malformed
Covers line: the pre-push checks ran as `check | tail -1 && …`, and the
pipe's exit code is `tail`'s, so the failure did not stop the chain.
Caught by re-running the checks; fixed in the next commit (`2125506`).
Check: run the gate commands without a pipe (or with `set -o
pipefail`) before every push.
