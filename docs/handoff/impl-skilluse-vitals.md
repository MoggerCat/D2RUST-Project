# Handoff: skill use pipeline and vitals (branch `claude/impl-skilluse-vitals`, 2026-10-06)

Scope: implementation of `specs/skills/use.md` (+ `specs/skills/functions.tsv`)
and `specs/combat/vitals.md` in `d2-sim`, started from
`claude/bold-ptolemy-jvyvxy` at `a5b323a` (cloud session, repo only). Files
changed: the new `crates/d2-sim/src/skills/use_/{mod,table,tests}.rs` and
`crates/d2-sim/src/combat/vitals/{mod,tests}.rs`, one `pub mod` line each
in `skills/mod.rs` and `combat/mod.rs`, and this note. No spec, HANDOFF,
PLAN, `d2-server` or other module file was edited.

## 1. State

**Implemented, unverified.** Both specs are drafts with no trace check;
the unit tests prove the spec's synthetic vectors only (METHODS M02).

Gate on this branch: `cargo fmt --all -- --check`, `cargo clippy -p d2-sim
--all-targets -- -D warnings`, `cargo test -p d2-sim` (551 passed, 3
ignored: the existing combat game-file tests), `cargo run -p depcheck`,
`python3 tools/spec_index.py --check`, `python3 tools/methods.py check`,
`python3 tools/coverage.py --check` (0 errors): all pass.

Coverage (`coverage.py --summary`): `skills/use.md` 64/64 rule units and
`combat/vitals.md` 19/19 claimed by unit tests; 0 verified.

Tests: 28 in `skills::use_::tests`, 12 in `combat::vitals::tests`. Every
test vector of both specs is a test (1.14d records rebuilt as synthetic
records from the values the specs give: Fire Bolt, Fire Ball, Teleport,
Blade Fury, Multiple Shot, Meteor, Might, the charstats Constants table, a
synthetic experience table with level 1 → 500 and level 99 →
3,837,739,017), plus every edge case. The one RNG draw (§4 interrupt
gate) is checked by seed state after each case, including "no draw".

## 2. Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/skills/use_/mod.rs` | seams `UseWorld`, `UseMissiles`, `SkillFunctions`; §1 validators and handlers (`validate_point`, `validate_unit`, `handle_message`, `handle_hold`, `MsgResult`); §2 `dual_wield`, `use_at_point`; §3 `range`, `use_on_unit`; §4 `can_change_mode`, `interrupt_gate`, `set_mode_with_skill`; §5.1 `skill_mode`; §5.2 `frame_events`, `attack_frame_event`; §5.3 `start`, `mana_check`, `LOS_MASKS`; §5.4 `do_skill`, `do_core`; §6 `set_delay`, `cooldown_blocks`; §7 `period`, `schedule_periodic`, `periodic_event` (type 8), `item_aura_event` (type 9), `active_state_event` (type 5), `select_skill` (0x3C) | `skills/use.md` |
| `crates/d2-sim/src/skills/use_/table.rs` | `FUNCS` (64 srvst + 152 srvdo filled slots: index, 1.14d address, D2MOO name, status), `lookup`, `check_tsv` (row-by-row check against `functions.tsv`, M05; perturbation test, M08) | `use.md` §8, `functions.tsv` |
| `crates/d2-sim/src/combat/vitals/mod.rs` | seam `VitalsUnits`; `VitalsTables` (`charstats`, `experience` typed records; `max_level`, `threshold`, `level_from_exp`); §1 `init_player_stats`, `set_experience_for_target_level`; §2 `handle_add_stat_point` (0x3A), `spend`, `gain_energy`, `gain_vitality`, `reset_stats`; §3 `level_up`; §4.2 `level_factor`; §4.3 `add_experience` | `combat/vitals.md` |

## 3. Seams (traits other sessions provide)

| Trait | Methods | Expected provider |
|---|---|---|
| `skills::use_::UseWorld` (extends `UseMissiles + SkillFunctions`, which extend `skills::ManaUnits`) | frame, `send` (0x15 resync, 0x5A); player data +0x168; position, `find_unit`, own inventory, same act, reach test `0x00548EF0`, owner; left / right skill get/set, `find_entry`, `find_entry_owned` (0x3C), `owns_skill` (`0x006439F0`), used skill set (`0x00620210`) and flags, entry mode (+8), Attack Param4, `use_state` (`0x00647960`), `dec_quantity` (`0x0056C3F0`); dual wield (`0x006235A0`), equippable, bow equipped, state mask, melee range (`0x00622C40`); mode, cursor item, `endanim_expire` (`0x005415A0`), `set_mode`, `start_mode` (`0x0057FE90` / `0x0057FEF0`), `run_to` (`0x00548A50`), target, unit flags +0xC4, event arg (`0x006212C0`), `step_path`, alive; hostile / pet / ally, room kind, target position (`0x0056D2C0`), line test (`0x00645950`); `schedule` / `delete_timers`; stat lists: delay list (§6), expiry, aura state on/off | units/stats (most), tick (timers), `d2-server` (`send`), items (inventory, equippable), drlg (line test) |
| `skills::use_::UseMissiles` | `create_skill_missile(u, skill, lvl, missile, lob, aim)` (`0x0056EE90` / `0x0056ECB0`) | missiles, through the action wiring (`claude/wire-action`, in progress) |
| `skills::use_::SkillFunctions` | `srvst(index, …)`, `srvdo(index, …)`: per-skill bodies, called only for filled slots of `table::FUNCS` | future per-skill spec sessions (`use.md` OQ10) |
| `combat::vitals::VitalsUnits` | type, class, base / unit getters, base set / add (`0x00627260`, `0x006272B0`), max life / mana / stamina (`0x00625D10/60/B0`), refresh (`0x0064C040`), level-up notifications (§3 step 7), unit event 12 | units/stats; `d2-server` for the notifications |

`UseWorld::use_state` is a seam because `use.md` §2 lists its parts but not
their order; `cooldown_blocks` is the §6 part the provider calls.
Monster mode starts (`0x005A75C0`, `0x005A7670`) are left to the monsters
branch (`use.md` OQ6).

## 4. Open questions (each has a `TODO` in code naming it)

1. `use.md` §1 rule 2: result codes of the unit validator for a bad type
   (≥ 6) and a failed distance test are not given; `MsgResult::Unspecified`
   carries them.
2. `use.md` §1 rule 1: resync when the unit has no player data (no
   last-success frame): d2rs sends none.
3. `use.md` §2: the order of the `use_state` parts; the entry lookup
   function (D2MOO `SKILLS_GetSkillById`): both behind the seam.
4. `use.md` §4: `can_change_mode` for current modes past KB: d2rs says no.
   State 42 on without a stat list: stat 164 read as 0.
5. `use.md` §5.1: skill mode for unit types other than player / monster
   (d2rs uses `anim`).
6. `use.md` §5.2 / OQ7: one running index shared by frame codes 1, 2, 4.
7. `use.md` §5.3 step 1 ("stop" = return 0 without the neutral reset);
   step 2: the corpse rules apply to monster targets only; step 6.4: no
   target position fails the line-of-sight check.
8. `use.md` §5.4 step 2: "level > 0" of the highest entry read as its
   skill level with bonuses; step 7: "valid missile" read as
   `srvmissile ≠ 0xFFFF` and a `missiles` row exists; step 5: an
   `ItemEffect` index past the table is an empty slot.
9. `use.md` §7: type-8 with arg1 0 or < −1; how type 9 reads skill and
   level from stat 151 (the caller passes them); the type-5 do arguments
   (d2rs passes charge 1, item 0, aim 0).
10. `vitals.md` §1: the table write order; `threshold(class, target)` is
    the experience of level `target + 1` under §4.1 (literal reading).
11. `vitals.md` §2: `gain_energy` reads "if `n > 0`" as covering the
    current mana only (maximum always changes), matching the closing
    paragraph and Edge case 2.
12. `vitals.md` OQ2: §4.3 implemented only as far as the creation path
    needs it (cap, level-up, event 12); `lastexp` (stat 29) not written;
    the kill gain (`ExpRatio`, stat 85, hireling, party share float) not
    implemented. OQ3: operand roles of the `pct` branch in §4.2.

## 5. Checks to queue (local run queue, `docs/HANDOFF.md` §5)

1. Group A (player): the recordings both specs already request —
   `use.md` OQ3–5, OQ7 (hooks `0x0056FAF0`, `0x0056F7F0`, `0x0056BFE0`,
   frame codes for Strafe / Zeal), and `vitals.md` OQ1 (hook `0x00570880`
   entry/exit and `0x00570D60`, stats 4–13 before/after, one level-up and
   stat spending). Replay them through `start` / `do_core` /
   `level_up` / `spend` with a recording-backed fake; expect equal stats
   and the same draw count (one `roll(100)` per interruptible request
   with state 42, none otherwise).
2. Group C (game files): a game-file test that builds `VitalsTables` and
   `SkillTables` from the live `.bin` set and reruns the vitals vectors
   (Sorceress / Barbarian created and level 1 → 10, `threshold(0, 1)` =
   500, `level_from_exp(0, 499 / 500)` = 1 / 2) and the `use.md` mana
   vectors (Fire Bolt 640, Fire Ball L10 2,432, Teleport 6,144 / 1,280 /
   0 / −1,280, Multiple Shot L10 3,328). Not written yet (needs a
   `D2_GAME_DIR` loader for `charstats` / `experience`); expect the
   numbers in the specs' Test vectors.
3. Group B (Ghidra): `use.md` OQ1, OQ2 and the questions in §4 above that
   need the disassembly (1, 3, 7, 8, 9); `vitals.md` OQ2, OQ3.
