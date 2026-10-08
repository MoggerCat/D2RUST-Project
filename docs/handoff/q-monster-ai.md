# q-monster-ai: monsters attack the player (`claude/q-monster-ai`)

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Synthetic fixtures only. Nothing here is verified against
> 1.14d (rule 10); the open points are REC-110 in `docs/HANDOFF.md` §7.
> Sound not wired.

## 1. The path, and the links that were missing

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Monster think → target (`ai.md` §5.2 step 5) | worked from `LocalSeams::target_nodes` (q-hire/stitch) | unchanged. Note: players in a town room are skipped (§5.2 step 5.1), so the monster only acquires a player outside the Rogue Encampment |
| 2 | `class_has_mode` (`0x0046C140`) | `Pending` default `false`: every mode request fell back to neutral | `app/monster_ai.rs` `MonsterAi::class_has_mode`, from monstats2 `mDT`…`mRN` |
| 3 | Mode request target | `Pending::set_mode_target` no-op | `MonsterAi::set_target`; `UseRest::target` reads it (`skill_rest.rs`); `clear_target` is a no-op (the start clears it before the per-frame do reads it) |
| 4 | AnimData name / rate for a monster | `anim_name` `None`, `anim_rate` 0: no event schedule, an A1 never fired or ended | q-skills-cast's `app/anim_names.rs` (the client art's name rules; the test supplies `UnitLooks` on synthetic data) |
| 5 | Attack start `0x005A75C0` → skill start | no used skill → no start | `used_skill` for a monster = the request's current skill, else Attack (skill 0); the monster skill start / per-frame routes are q-skills-cast's `app/skill_events.rs` |
| 6 | Attack-family event 0 `0x005A7670` | not handled (fell to the death functions): the swing never did anything | `wiring/path/monsters.rs` `monster_attack_event0` → `Pending::monster_sequence_frame`; `units/dispatch.rs` stores the type-0 timer's code in unit +0x4E first |
| 7 | Hit → damage → vitals | existed (`apply_melee`, `combat/*`) | unchanged: the Attack do (`skills/bodies.md` §4.1) takes it; the 0x95 / 0x18 vitals sync and q-death's DT follow |

## 2. Test

`crates/d2-client/tests/app_play_monster_ai.rs`
`a_monster_next_to_the_player_attacks_until_the_player_dies`: the
synthetic game with a Zombie-AI monster class (fixtures installed on the
server before the join), the player walked out of town by the cave warp
(q-warps), a monster one sub-tile away; the monster attacks in A1, the
server's damage path takes the player's life to 0, the client model goes
to DEATH and the death screen comes up. It failed before each link above
(checked in order: no mode change; no event; no skill; no target).
`d2-sim` `attack_event0_runs_the_skill_frame_and_keeps_the_frame_code`
covers link 6.

## 3. PROVISIONAL (REC-110)

- Which skill an attack mode uses (`monsters/ai.md` §7.1: the AI's plain
  attack request sets none): Attack, level 1 (`MonsterAi::used_skill`).
- The attack-family per-frame `0x005A7670` (`skills/use.md` §5.2, OQ6):
  runs the sequence frame's skill part with unit +0x4E := the timer code.
- The monster's request target is kept past the skill start's clear (`UseRest::target` falls back to `MonsterAi`).

## 4. What is left

- Approach: a monster further than the melee range walks to the player
  through the path provider; not run by this test (it starts adjacent).
- Ranged and spell attackers (`skill_usable`, `ai_skill_*`, missile
  creation for monsters) and the get-hit reaction of the player.
- The S→C attack animation messages for the player's client (0x4C / 0x4D,
  `stitch-combat.md` row 9): the monster is drawn attacking only through
  the mode messages (0x6x) the tick already sends.
- `ai.md` §5.2 step 5.1: the preview player is safe in town rooms.

## 5. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-monster-ai
git checkout claude/q-monster-ai
cargo test -p d2-client --test app_play_monster_ai
cargo run -p d2-client --release -- play --new barbarian Test
```

The test passes. In `play`, leave the Rogue Encampment (east, the Blood
Moor) and let a monster reach you: it should swing (attack animation),
your life should drop, and at 0 life the death screen appears (Esc
respawns in town). If a monster just stands next to you, copy the console
lines into `docs/HANDOFF.md` §7 REC-110 (the usual suspect: the monster's
class lacks the mode in monstats2, or AnimData has no `<code>A1HTH` row).
