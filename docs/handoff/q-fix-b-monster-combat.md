# q-fix-b-monster-combat — hand-back (2026-10-09)

Branch `claude/q-fix-b-monster-combat`, from `claude/specs-staging-7`
(rows from PC1-B's `claude/local-pc1-day3-b`, gate b65), staging merged
in before each push. Four rows of `docs/handoff/build-queue.tsv`: rows 1,
2 and 4 done (marked `DONE (q-fix-b-monster-combat)`), row 3 waits on PC 1.

Checks were run on the d2rs side only (`scenario_diff.py … --d2rs-only`,
cloud, 1.14d install from the private repo). The 1.14d state files are on
PC 1 (`traces/raw/check-*`, gitignored), so the comparison is against the
values PC1-B wrote into the rows.

## 1. q-fix-b-monster-melee-no-damage (done)

Cause: a monster's attack rating was 0, so every swing had 5% to hit and
did 1–2 damage. Two links of the spec were not wired:

- The monster mode set `0x005A7C20` never ran the mode damage rewrite
  `0x005A4F50` (`skills/bodies-2.md` §2.1, `umod-callbacks.md` §2 rule
  1: after the bookkeeping, before umod mode 0, every mode but GH). New
  `UnitHooks::monster_mode_damage` called from `units/modes.rs`
  `monster_set_mode`; `ActionHooks` routes it to
  `Pending::monster_mode_damage` (default nothing, as
  `monster_attack_strike`), which `LocalSeams` sends to
  `wiring/interaction/skill_events.rs` `monster_mode_damage` (the existing
  `mode_damage` body on a `UseView`).
- The monster base list (flag 1, `monsters/init.md` §6 step 12) that
  `mode_damage` and the umod callbacks write into was never created
  (`InitHost::post_extra_list` was empty). `WorldHost` now allocates it
  (flags 1) and attaches it to the unit. PROVISIONAL REC-891: owner and
  attach `reset` are not stated (owner = the monster, reset 1).

Check (`traces/checks/combat-fallen-hits-player.check`, d2rs): player hp
12800 → 12321 at 77, 11866 at 98, 11397 at 124, 11039 at 137, 10560 at
173: the five 1.14d values, frame for frame (the last two need row 2).
Fallen A1 at 70 / 91 / 130 / 151 as in 1.14d.

Tests: `units/gap_tests.rs`
`the_mode_set_rewrites_mode_damage_after_the_bookkeeping_before_umod_0`
(order, none for GH); real data `d2-client/tests/app_state_dump.rs`
`a_fallen_party_hits_the_player_on_the_recorded_frames` (`#[ignore]`,
`D2_GAME_DIR`; failed before: hp stayed 12800).

## 2. q-fix-b-fallen-leader-mode-41 (done)

Cause: population's owner data `0x0058F030` and minion list
`0x0058F100` (`population.md` §10.2) never reached the AI control
record: `WorldHost::set_owner_data` only forwarded to
`WorldPending::set_owner_data` (default nothing), and `add_minion`
filled only the world's minion map. So the Fallen leader was not its own
minion owner, skipped `ai-bodies.md` §9.4 step 5.2 (the S2 shout and the
command to the pack) and fell to 5.4 (A2).

Fix (`wiring/worldgen/population_init.rs`): `set_owner_data` writes the
control's minion owner (type `a`, the key unit's GUID: both keys are the
GUID, `0x00451F50`); `add_minion` also pushes the GUID into the leader's
control `minions` (read by `command_minions` `0x0058F730`). PROVISIONAL
REC-892: the f1 / f2 restart `0x005DD230` is not stated; nothing is done
for it.

Check: the leader (GUID 17) takes S2 at 41, NU 65, S2 90, NU 114, A2 129
— the 1.14d sequence of the row. Test: real data
`the_fallen_leader_shouts_on_the_recorded_frames` (failed before: A2 at
41).

## 3. q-fix-b-quillrat-shoot (not done: needs PC 1)

d2rs reproduces the row (walk at 59). Findings: the rat sits exactly on
the poke point (4,4 from the player), D = 6 at both thinks; at 31 P(aip2
35) passes, at 59 it fails (lo' 89) and the escape walks. The same run
without the arrows is identical. No number of extra draws on the rat's
seed between 31 and 59 makes P(35) pass (0 extra → 64, 2 → 39), so 1.14d
took A2 by another branch or from another position (PC1-B: the 1.14d rat
"is placed elsewhere"; the Blood Moor population already differs at the
warp, `q-fix-b-bloodmoor-warp-population`). Not guessed. Question for
PC 1 in `pc1-data.md` Step 4 ("Quill Rat at frame 59"): the 1.14d rat
lines (x, y, m, s) for frames 30–64 of the check, or the §9.7 step it
takes.

## 4. q-fix-b665-zero-walk (done)

`wiring/action/ai.rs` `walk_in_radius`: no radius point (k ≤ 0 or t on
U) → the mode-2 request is still made at the unit's own cell
(`ai.md` §7.5 rule 8). The existing set-up then gives type 13 → 0 points
→ type 15, 0 points, the velocity request consumed, neutral, think at
f + `aidel`, no draws. `radius_point`'s geometry is unchanged (its own
row `q-fix-p3-walk-in-radius`).

Test: `wiring/action/unit_update/tests.rs`
`a_zero_length_walk_goes_neutral_and_sends_code_7`. One difference from
the row's test text: the message. `ai.md` §7.5 r8 says S→C 0x67 code 7;
the message builder's owner spec `sim/intents-events.md` §7.4 r5 sends
mode 1 as 0x6D (and d2rs does). The test pins one mode message, 0x6D at
the cell, with PROVISIONAL REC-890 and a PC 1 question.

## PROVISIONAL points and PC 1 questions

- REC-890 zero-walk message (0x67 code 7 vs 0x6D), REC-891 base-list
  arguments, REC-892 owner-data f1 / f2: `docs/HANDOFF.md` §7 and
  `docs/handoff/pc1-data.md` Step 4 (`[q-fix-b-monster-combat]`).
- Quill Rat frame 59: `pc1-data.md` Step 4.

## Not touched

The death path (q-diff-combat-a1). `radius_point` geometry
(q-fix-p3-walk-in-radius). The Blood Moor warp population.

## Local check

```sh
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-fallen-hits-player.check
```
Expect the player's hp (stat 6) and the three Fallens' modes to match
1.14d through frame 200 (other differences: the warp population,
q-fix-b-bloodmoor-warp-population).
