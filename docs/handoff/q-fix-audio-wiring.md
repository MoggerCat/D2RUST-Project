# q-fix-audio-wiring — audio rules on the play path (2026-10-08)

Branch `claude/q-fix-audio-wiring` (from `claude/q-audio-audit`, merges of
`claude/specs-staging-7`). Goal: the audio rule code reaches the play path,
each part with a test. No game files in this session: every check runs on
synthetic tables (M23: the real-install check is the play smoke path, see
"Local run" below).

## Done (each with a test)

| Row | What changed | Test |
|---|---|---|
| q-fix-seam-audio-pending | `DriverError::Pending` removed: a model question the sound layer cannot answer is answered neutrally, reported by `SoundDriver::take_pending`, logged once (`AudioStats::pending`). It no longer reaches Bevy's error handler (panic on the first unit sound). | `tests/app_audio_play.rs` `a_pending_sound_input_does_not_stop_play` |
| q-fix-seam-sound-listener | The local player is heard at its drawn (predicted) point (`PreviewWalk::local_at`). | `driver::tests::the_listener_is_the_drawn_local_player` |
| q-fix-audio-pixel-positions | Positions are client pixel points (`render/camera.md` §2, the view's projection), not subtile cells; 0x2C captures are projected the same way. | `positions_are_client_pixel_points` (40 subtiles → (640, 640), out of Falloff 0) |
| q-fix-audio-ui-ids | `PanelOutput::Sound(id)` with the `client/ui.md` §B8.1 id per site (ESC/options 1/2, skill tree 6/4/5, shop tab 6, cube/stash 4, new-stats/skills 4, gold dialog 0xDD; waypoint, shop, mini panel and control panel pass their own ids). | `original_tests` (menu 2 then 1, skill tree tab 6), `skilltree`, `buttons` tests |
| q-fix-audio-settings | App settings (Master, Music, Positional Bias, NPC Speech; mixer 0) → `SoundSystem::set_settings` each audio frame. | `the_options_master_volume_reaches_the_sound_layer` (master 0 silences) |
| q-fix-audio-ui-overlays (thunder part) | Pass 9 draws the delay 25 + roll(50) (was stored as a volume), requests 202 through `ThunderSound` (`SoundLink`, the app's driver), then y before x only on a handle, and sets (x, y, 640). | `weather_tests::thunder_requests_202_with_the_delay_and_places_it_y_first`, `driver::tests::the_thunder_step_requests_through_the_link` |
| q-fix-audio-client-seed | `ClientSeed`: the local player's model seed, with the model's own steps (0x59, client object draws) replayed each frame before the sound draws (`sound-table-2.md` §14.2 r1, §14.3). Variants vary. | `variants_draw_on_the_local_players_client_seed` (12 picks = a §4 r3 replay, 3 model steps mid-way), `a_new_local_player_or_seed_is_taken_as_is` |
| q-fix-audio-environment-wire | `Environment` runs each sound tick before the request update: level, `soundenviron` row, day phase (act environment, 2 before any 0x53), weather (active, ⌊255 n / 256⌋ from the weather view through `SoundLink`), settings, P. Sound init on the first frame with a local player. Quest stingers (0x2C / player events 33…83) and the event-92 re-arm reach the music machine. | `the_levels_song_and_bed_start_on_the_sound_tick`, `a_quest_stinger_event_starts_the_stinger`, `app_audio_play::the_town_levels_song_starts_on_the_play_path` |
| event 18 (q-fix-audio-npc-speech, part) | 0x2C event 18 greets with `npc-greetings.tsv` (`GreetingRecords::spec`). | `event_18_greets_with_the_npcs_record` |
| q-fix-audio-unit-freed-c | Set-C frees (`remove_client_unit`) emit `UnitFreed { client_only: true }`; the driver detaches and drops the unit's sound fields. The last point of every unit with requests is kept across frames for a key that no longer resolves. | `a_client_only_free_appends_a_unit_freed_output`, `a_client_only_free_detaches_and_a_gone_unit_keeps_its_last_point` |

## Not done, and why (inputs the model does not hold)

- **Unit sounds** (q-fix-audio-unit-sounds: mode sounds, footsteps,
  swings, hits, idle/init/flee voices; events 16, 17): the client model
  has no animation frame / speed, +0x4E, path flags, hit class, floor
  material or `monsounds` rows (no `monsounds` table in d2-data either).
- **Event 12** (state 68 `stsound`): needs U's state-68 stat list and the
  skills row in the client.
- **Skills / missiles / states / items** (q-fix-audio-skills-items): no
  client handler starts them yet (`bridge/modes.rs item()`,
  `bridge/msg/states.rs`).
- **NPC speech / interact greetings** (rest of q-fix-audio-npc-speech):
  the interact NPC is UI state (`client/bridge.md` §10 r9); `DialogState`
  needs the NPC dialog wiring.
- **Level-entry lines** (environment §4 r2): the client quest check needs
  the 0x5E bytes and quest records; no client owner holds them for the
  sound layer. Asked as pending (answered no), so no entry line plays.
- **Request tick stamps / per-tick positions** (q-fix-audio-request-ticks):
  the bridge receives all of a frame's messages after its server ticks,
  so every output of a frame has the same receive point; stamping needs a
  per-tick receive in the bridge first. The driver runs the env machine
  with C per tick, requests at the frame's last tick.
- **Lock release** `0x004CC160(U, −1)`: no lock is ever taken
  (`SoundSystem::lock` has no caller), so there is nothing to release.
- Not started: q-fix-audio-frontend-music, q-fix-audio-pause-duck,
  q-fix-audio-float-steps, q-fix-audio-log-flag, q-fix-audio-server-events,
  `drain_cues` re-ids (audit item 14).

## Spec disagreements found

1. `specs/seams/bridge-app.md` §2.7 r2 says sound positions are model
   sub-tile cells; the owner spec `audio/sound-table.md` §8.1 r1 says
   client pixel points. The code follows the owner spec. §2.7 r2 needs
   the edit (spec session).
2. `ui/panels.md` §10.2 / `ui/control-panel.md` §8 r4 call the skill
   tree / new-stats button sound "click sound 0"; `client/ui.md` §B8.1
   (newer, from the disassembly scan) gives ids 6/4/5 and 4. The code
   follows §B8.1; the older rules need the edit.

## Local run

Play on the real install (the play smoke path): the town level's song
(its `soundenviron` row's `Song`) starts on entry, then the bed of the
next level by day or night; the ESC menu sounds 2 then 1 on choices; the
log shows each `sound layer pending:` question once and no audio panic.
A rain level plays 202 on a lightning strike after its delay. Record the
request log (`client/audio.md` §A5) once `q-fix-audio-log-flag` lands.
