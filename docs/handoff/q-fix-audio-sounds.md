# q-fix-audio-sounds — unit, state, missile, item and NPC sounds on the play path (2026-10-09)

Branch `claude/q-fix-audio-sounds` (from staging). Real install (the
private data repo's `install/`, assembled into `$HOME/game`) for every
test below (M23); the cue asserted is id (group base), unit, start tick
and point.

## Done

| Sound | Where | Test (real install) |
|---|---|---|
| `monsounds`, `monstats`, `monstats2`, `superuniques`, `skills`, `states`, `missiles`, item tables, AnimData as `UnitSoundRows` (the typed `Monsounds` table already existed in d2-data) | `audio/unit_feed.rs` | all below |
| Events 16 (Taunt), 17 (Flee voice, 4-update gap, delay 6 + roll 3), 12 (`stsound` of the state-68 stat-350 skill) | `driver.rs` `request`, `UnitFeed::event_*` | `audio_unit_sounds.rs` `event_16_…`, `event_17_…`, `event_12_…` |
| Monster mode sounds (attack voice, hit, death) and the player's class hit voice | unit pass, `mode_set` | `a_zombie_swinging_…`, `a_zombie_hit_and_dying_…`, `the_local_player_hit_…` |
| `Init` voice, `Neutral` idle voice | unit pass | `a_new_swarm_…`, `an_idle_zombie_groans_…` |
| Footsteps (n, offset, material default, run +24) | unit pass | `a_walking_zombie_steps_…` (step ticks equal an oracle of §5 r3–r4 over the install's AnimData F and s); play path `play_smoke::the_play_path_steps_and_speaks` (the sorceress's own steps while running to Akara) |
| State on / off sounds | unit pass | `a_state_turning_on_…` |
| Missile `TravelSound` | unit pass | `a_new_missile_…` |
| Item cursor (235) and drop (216 + row drop sound, frame delay, volume 180) | unit pass | none yet (below) |
| NPC dialog line (B2) and interaction greeting (B3 / B6) | `ui/msg_ui.rs` → `SoundRequest::NpcDialogLine` / `NpcGreeting` → driver | `msg_ui_tests::npc_dialog_branches_ask_for_speech`; `a_dialog_line_plays_…`, `interacting_with_akara_…`; play path (Akara's text line spoken on the player, delay 5) |
| `NPC Speech` option: the first value seen is the stored one and is not applied (OQ 7); a change is the setter | `SoundDriver::set_settings` | covered by the dialog tests' default (enabled) |

Play path finding (M23): the model's local-player mode stays 1 while the
preview walks it, so no footstep played; the driver now reads the drawn
mode (`set_local_mode`, from `PreviewWalk`).

## Provisional (REC-430 … REC-436, `docs/HANDOFF.md` §5 and the REC list)

407 animation f / F / s, 408 floor material, 409 unit order, 410 weapon
hit class, 411 first-sight sounds, 412 skill start / missile hit /
ProgSound not requested, 413 greeting mode.

## Not done

- Item sounds have no real-install test: an item unit's code needs a
  built item bit stream; queue: add an item to the play-smoke run (kill
  a monster, hear 216 + the drop sound) and assert it.
- Skill start sounds (§8 r1), missile hit / prog sounds, `dosound` /
  `tgtsound`, item place / use / gold sounds, unique / set drop sounds
  (quality is in the stream), overhead text, level-entry lines: see
  `driver::PENDING`.
- Request tick stamps (q-fix-audio-request-ticks): a frame with several
  server ticks still requests at its first update's C.
- Wussie / guard / Nihlathak lines (§10 r4, OQ 8).

## Local run

`D2_GAME_DIR=$HOME/game cargo test -p d2-client --test audio_unit_sounds
--test play_smoke -- --ignored`

Gate note: `play_smoke::the_live_run` fails identically on the parent commit 56796e3 ('monster 3 died: life Some(256)', play_smoke.rs:778); not from this branch. No remote `staging` branch exists; the branch is based on the staging tip 56796e3, so there was nothing to merge.
