# Gap tests: monsters/ai, missiles, combat/damage

Branch `claude/gaps-combat-ai`, from `claude/bold-ptolemy-jvyvxy` at
`ac01471` (PR #15). Cloud session, tests from specs (M14 medium, M08).
Test code only in `crates/d2-sim/src/{monsters/ai,missiles,combat}/`.

## Coverage (unit tier, `py tools/coverage.py --summary`)

| Spec | Units | Before | After | Uncovered after |
|---|---|---|---|---|
| `specs/monsters/ai.md` | 119 | 39 (32.8%) | 108 (90.8%) | 11 |
| `specs/missiles/missiles.md` | 117 | 55 (47.0%) | 104 (88.9%) | 13 |
| `specs/combat/damage.md` | 126 | 74 (58.7%) | 115 (91.3%) | 11 |
| total (all specs, any tier) | 2704 | 1815 (67.1%) | 1974 (73.0%) | |

All new claims are unit tier. Nothing is verified against 1.14d by this
work, so verified coverage is unchanged.

New tests:
- `monsters/ai/tests/rules.rs`, a child module of `tests.rs`. The fake
  world gains alignment, doors, line of sight, vision, a teleport spot,
  corpses, forced, good, nearest and special-walk targets, busy players,
  failing modes, and the neutral-start `aidel` think on a failed mode
  start (§1.3). Its collision now answers for mask 0x40 only.
- `missiles/tests.rs`: 35 tests. The fake records creation calls and can
  fail allocation or the room lookup.
- `combat/damage_tests.rs`: 26 tests. A `Spy` wraps `skills::fake::Fake`
  to record the dual-wield switch, the melee-range argument, the reaction
  record, alignment, dead units and monster modes.

Perturbation (M08): the agents broke rules on purpose and the matching
tests failed each time, then the breaks were reverted.
- ai: Brute's second chance read from aip2; Navi's roll(3) moved after
  the "close" test; the teleport test changed from D < 10 to D ≤ 10.
- missiles: the remainder made unsigned; AlwaysExplode ignored.

## Code fixes

None. Every new test passed against the existing code.

## Not claimed

### monsters/ai.md (11)
- §1.3 text: "sets combat mode 1" is the caller's job and is not modelled
  in monsters/ai. Its other sentences are covered by §1.7 and §9.3 text.
- §2.1 r4: the fatal assert on a bad code pointer has no d2rs
  counterpart: an unknown AI address logs a stub call. The rest of r4 is
  checked by `dispatch_order_and_stops`, which claims r1–r3 only.
- §3.3 text: prose listing who calls install (creation code and 32
  skill/state sites), all in other specs.
- §5.1, §5.3, §5.4: forced targets, the secondary helpers and the room
  scan table come from outside monsters/ai through the `AiTargets` seam.
  §5.1 is also open question 6. §5.2 r3 (a forced target is used first)
  is tested.
- §9 text: naming conventions only.
- §9.1: values from the live `monstats.txt`. Needs a game-file check.
- §9.7 text, §9.12 text: data and provenance lines only.
- §9.9: Npc is a stub by design.

### missiles.md (13)
- R1 r1: "684 records × 420 bytes" needs `missiles.bin`. The class range
  check is tested.
- R2.2: a fact about call sites in the binary.
- R2.3 r9: seeding and the GUID counter happen inside `alloc_missile`
  (units spec).
- R2.3 r11: setting unit +0x44 (start frame << 8) is not implemented; a
  TODO in `create.rs` defers it to the units spec. The `frames -= start
  frame` part works.
- **R2.4: message 0x73 (`0x0059FEE0`) is not implemented.** It needs a
  message/protocol seam outside `missiles/`. This is a real gap and is
  left for a session that owns the protocol side.
- R4 text, R9.3 text: they say which server-do functions call the default
  flight last or use the helper (8, 10, 17, 25). Those functions are
  stubs by design.
- R5 text: the signature and local variables only.
- R6.1 r3: the spec is ambiguous (question 1).
- R9.4: only the R9.3 re-seed exists. Server-hit 58 and server-do 28,
  34, 35 are stubs; the init callbacks belong to the skills spec.
- R10 text: the table of recorded row values needs `missiles.bin`. R10
  r1–r4 are tested on synthetic rows with the recorded values.
- R11: a column-usage table found by a heuristic search of the binary.
- Edge r10: the original crashes on a null server-do, server-hit or
  server-damage entry; d2rs logs `Unhandled`. That is a deliberate
  difference, so the test `null_table_entries_are_flagged` stays but
  claims nothing.

### combat/damage.md (11)
- §1: a 0x70-byte layout. `DamageRecord` is a Rust struct with no byte
  layout to compare.
- §2 text: the missile path calls the reaction from code outside combat.
  `apply` with `missile = true` is covered under §5.2 r5.
- §2 r1: the skill do-functions store `melee_result` (`skills/use.md`).
- §3 text, §5.2 text: calling-convention prose.
- §5.1 r2: "copy its damage record" can't be observed, because step 8
  frees the list entry.
- §5.4: event dispatch goes through the `unit_event` seam
  (`sim/units.md`, open question 4).
- §7.1, §7.2: reaction and kill are the `CombatWorld::reaction` seam
  (open question 3). There is no code for them in combat.
- §8: the 32-entry function table is not in d2-sim. The existing
  `crushing_blow_vectors` and `open_wounds_vectors` check its two
  specified functions but do not check the table.
- Edge r7: where the hit-class counter lives is up to the server's
  provider. `element_hit_class` only gets a `&mut u8`.

## Stubs by design (not gaps)
- AI: Npc (32, `summarized`); the D2MOO-only Act I AIs 4, 5, 10, 37, 43
  and 59 (§9.14 checks their catalogue rows and the stub log); every init
  and alternate function; the special-state thinks.
- Missiles: server-do 2, 3 and 5–37, and server-hit 1–59, log
  `Unhandled`. In the TSVs their bodies are `summarized` or `D2MOO-only`
  (open question 8).

## Spec questions
1. missiles.md R6.1 r3: "armor −= missile stat 120 (`item_damagetargetac`,
   clamped at ≥ 0 after adding)". "−=" and "after adding" disagree on the
   sign. The code passes `+stat120` to `add_target_ac`. Which is right?
2. missiles.md edge case 5: a roomless missile "is removed by R4 step 5 on
   its first collision-active run". In R4, step 5 (no room → 2) comes
   before step 7 (the Activate check). Does "collision-active" mean
   `CollideType` ≠ 0, or run k ≥ `Activate`? Tested only with
   `Activate` 0, where both readings agree.

The tests also stay out of cases that existing `TODO(spec gap)` notes in
the code leave open:
- damage.md §4.5 step 3: do leech rows take the difficulty resist
  penalty?
- damage.md §3.1 step 6: the getter for stats 103, 104 and 106 isn't
  named.
- damage.md §5.1 step 6: the "attacker is an object…" condition is
  tested on its literal reading.
- ai.md: Navi's param 1 clamp is tested only from 5 to 4.
- ai.md: the distance reported for an alternative target from slot 9.
- ai.md: combat and flags for a forced target (open question 6).
- ai.md: a mode or move request toward target 0 is only checked as
  passed through.
