# q-fix-real-skills: Leap, Leap Attack, Whirlwind and the traps on the install (2026-10-09)

Branch `claude/q-fix-real-skills` (from staging `76d36bc`, merged with
`claude/q-fixture-migrate` and `claude/specs-staging-7`). Rows
q-fix-real-leap, q-fix-real-whirlwind, q-fix-real-leap-attack,
q-fix-real-sentry of `docs/handoff/q-fixture-migrate.md` §4. Methods
M22, M23, M25.

## Causes and changes

| Row | Cause (found on the install) | Change |
|---|---|---|
| leap, leap-attack | The skill world's `anim_from` / `anim_rewind` / `anim_restart` (`sim/units.md` §4.2 variants `0x00553DC0`, `0x00553B10`, `0x00553C70`) were `Pending` no-ops, so Leap's Land never re-armed its frame-11 event; and `attack_frame_event` skipped the rule-3 do while a moving skill's step was unfinished (`skills/use.md` §5.2 r2–3, answered REC-232), so Land ran only on frames 11 and 14: two path steps, 1 sub-tile | `d2-sim` `wiring/interaction/skill_use.rs` runs the variants on the unit (`units::anim::run`); `skills/use_/mod.rs` falls through to rule 3 |
| whirlwind | The click's point never reached the path target (`0x00648AD0`; d2rs wrote it for the run mode only), so Whirlwind's start computed its path to the last walk's target and was refused; the 0x4D of every point skill carried that stale point too (`pathing.md` §10 r2) | `body_path::point_target` writes it for every skill mode: **PROVISIONAL REC-460** (the point-form start `0x0057FE90` is not specified; every reader, `0x0056D2C0`, needs it there). R-SKPT-1 settles it |
| leap arc, spin (client) | `SkillMotion` measured the leap / spin distance from the model's position, which never moves for the local walker (the server sends it nothing), so a 5-sub-tile leap drew a ~300-tick arc | `d2-client` `world_view/skill_motion.rs` uses the client's own path cell (the walk prediction) when it has one; the real rig records its walks in a `WalkTap` as `PredictLink` does (`tests/real_rig`) |
| sentry | AI 101 (`ai-bodies-6.md` §14) killed every trap: `BodyEffect::OwnerData` was never applied (no minion owner → charges step 1), `SetSkill` reached no store (no `Skill1` entry → step 2), the AI host's `skill_calc` was the default 0 (→ step 4); and the d2rs-own sentry driver (REC-233) killed the Lightning Sentry at once (it counted the laying skill's `calc4`, 0) | `wiring/interaction/summon.rs` writes the AI owner link and keeps a summon's entries (`ActionHooks::monster_skills`); `wiring/action/ai.rs` answers `skill_entry` / `skill_level` from them and evaluates `skill_calc` (`AiSummons::skill_calc` takes the game now). The driver (`d2-server` `sentry_drive.rs`), `ActionHooks::sentries` and `SentryLaid` are gone: REC-233 (1) retired |
| sentry (shot) | The trap's shot is a monster SQ (`monanim` 14): no `monseq` sequence was loaded (`load_sequence` returned none for monsters), `prepare_animation` overwrote +0x48 with the AnimData count in the sequence branch (`sequences.md` §2: count · 256), the frame advance `0x00623E00` was a seam no-op, and the every-tick type-0 event (a1 0) cleared +0x4E | `skills/sequences.rs` `MonsterSequences` (monstats slot sequences, `callbacks.md` §4, and `monseq` rows: **PROVISIONAL REC-461** for `0x00659E30`); `units/modes.rs` keeps the sequence count; `units/anim.rs` `advance_sequence` (§3 sequence branch) runs in the SQ event 0; `units/dispatch.rs` stores a1 in +0x4E only for frame events |

The hireling think (`hireling_drive.rs`) leaves traps to their AI (monstats `AI` 101) instead of
the old sentry list.

## Tests

- Real install (`#[ignore = "real data: ..."]`, `tests/real_rig`): `app_barbarian` 10/10,
  `app_move_anims` 2/2, `app_assassin_gaps` 4/5. The traps' helpers read the 1.14d state now:
  the living AssassinSentry monsters and their AI charge count (param 1); `lay` waits for the
  trap's first think (F + 25), where §14 counts the shots.
- Unit: `attack_frame_events` (rule-3 fall-through of an unfinished move), `sequences.rs`
  (monster lookup, table build), `units/tests.rs` (sequence advance), `gap_tests.rs`
  `setting_a_mode` (+0x48 of the sequence branch).

## Left

| Row | What | Owner |
|---|---|---|
| q-fix-real-missile-damage | `a_lightning_sentry_fires_its_missile` still fails on "the missile hit": the bolt flies, its hit test sees the zombie (collided mask 0x100 in its cell), but its damage stats are 0. Missile damage setup `0x0059F900` → `0x0064B860` / `0x0064AC60` (`missiles.md` §R2.3 step 23) has no spec ("the skills spec computes the damage data"), so `Pending::missile_damage_setup` stays a no-op and every missile hits for 0 | spec (local): the damage data of `0x0064B860`, then implement |
| REC-460 / REC-461 | the point-form start's write and the `monseq` record reading | R-SKPT-1; R-SENTRY-1 + a read of `0x00659E30` (`docs/HANDOFF.md` §7) |
| drawn frames | `advance_sequence` keeps only the event bytes (+0x4E); the drawn mode / frame (+0x40 / +0x44) of a sequence are not kept on the server | when a reader needs them |
