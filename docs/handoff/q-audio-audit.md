# q-audio-audit — rule-by-rule audio audit (2026-10-08)

Branch `claude/q-audio-audit`. Scope: `specs/audio/*`, `specs/client/audio.md`,
the audio parts of `specs/client/*` against `crates/d2-client` (read-only
audit by five readers, one per spec; no game files). Rules read: triggers
137/137, triggers-2 46/46, sound-table 82/91 (+A1–A5 of client/audio.md),
sound-table-2 33/33, environment 71/71.

## Headline (M23)

The rule functions match their specs almost everywhere (two exceptions
below). The failure is wiring: in play, only UI ids, S→C 0x2C events
(not 12, 16 on monsters, 17, 18), player events 19/23, object mode sounds
and `UnitFreed` reach the sound system. **No music, ambience, rain, quest
stingers, footsteps, swings, hits, deaths, monster voices, skill/missile
sounds, item sounds, NPC speech or greetings are ever requested.** Tests
use stub worlds, so none of this showed. `driver.rs` PENDING says part of it.

## Fixed in this branch (each with a test)

| Spec rule | Fix | Heard before |
|---|---|---|
| sound-table §4 r3 (+ OQ 3) | `pick_variant` loop is bounded (64 draws) | With no client seed `roll` is stuck at 0: the 2nd request of any id with Group Size ≥ 3 spun forever (freeze). Interim only; real fix is q-fix-audio-client-seed |
| sound-table §8.3 r3, audio.md §A4 | `device_occluded` product in f64, trunc | device volume one too high in 110 of 23×256 cells (occ 0.05f, v 200 → 190, spec 189) |
| triggers §1 r6 | `Globals::sound_init()` (idle gap 90), used by the driver | first monster idle voice could fire at once (not live yet) |
| sound-table-2 §14 / app seam | `audio_frame` no longer `?`-returns on `DriverError::Pending` before `engine.pump`/`present` | every frame with a unit-attached sound dropped its cues and returned an Err (Bevy default handler panics) |

## Disagreements queued (rows `q-fix-audio-*` in build-queue.tsv)

1. **Environment/music never run** (environment.md §1–§9, all 71 rules): `Environment::tick`, `sound_init`, `Music`, `Jukebox`, stingers, `EnvHooks` have no caller; no `EnvHooks` impl; no `EnvRow` table, period index, weather, level count, quest check, `PlayerState`. Silent game and front end. → `q-fix-audio-environment-wire`, `q-fix-audio-frontend-music`.
2. **Client seed** (sound-table §4 r5/r6, sound-table-2 §14): `client_seed()` is always None → variants never vary; every draw is a pending question. → `q-fix-audio-client-seed`.
3. **Positions in subtile cells, spec says pixel points** (sound-table §8.1 r1): falloff/range/pan scales wrong (700 px treated as 40). Small projection `((x−y)·16, (x+y)·8)` (model.md §6 r6), exactness on moving units needs the 16.16 position. → `q-fix-audio-pixel-positions`.
4. **Settings never reach the sound system** (sound-table §9, sound-table-2 §15 r6): `set_settings`/`from_store` have no caller; Master/Music sliders do nothing, Music rows play at 50. → `q-fix-audio-settings`.
5. **Options/ESC menu silent** (sound-table-2 §15 r5, triggers-2 §17): `MenuEvent::CursorPass => {}` drops id 1; actions play id 0 (silent) instead of id 2. Panel clicks use `CLICK_SOUND_ID = 0` (ui/original.rs:77) → buttons silent (triggers §11 ids 1–6,15,16). → `q-fix-audio-ui-ids`.
6. **Request tick stamping** (triggers-2 §14 r2, triggers §1 r5): all requests of a frame get C = last server tick and run before the sound ticks; 75/15/25-tick gaps and delays collapse when a frame runs several ticks. → `q-fix-audio-request-ticks`.
7. **Trigger rule groups with no caller** (need model inputs: frame, speed, mode machine, +0x4E, monsounds, floor material, hit class): triggers §4 mode sounds, §5 footsteps, §6 idle/init/flee, §8 skills/missiles/states, §9 items, §10 dialog/greeting, §12 overlays/quake/waypoint/Inifuss/4640/4638/thunder, triggers-2 §13 hooks, §15, §16, §18 identity. Events 12/16/17/18 skipped by the driver; 11 quest stingers and event 92 re-arm dropped. → `q-fix-audio-unit-sounds`, `q-fix-audio-skills-items`, `q-fix-audio-npc-speech`, `q-fix-audio-ui-overlays`.
8. **Server never queues events 15–18, 25–32** (triggers-2 §14 r3): `Pending::play_sound` is a no-op in `LocalSeams`; flag 0x400 guard unverified. → `q-fix-audio-server-events`.
9. **UnitFreed** (bridge.md §10 r3.1): done for set S (the existing row's "no caller" is stale: driver.rs applies `detach_all(..,false)`). Open: set-C frees (`bridge/objects/mod.rs:587 remove_client_unit`) emit nothing → loops on client objects never stop; capture map is per-frame and only for `ServerSound` (other unit requests of a unit freed before delivery play centred at full volume); lock release not applied (no `lock` callers → Async Only sounds start up to 25 ticks late). → `q-fix-audio-unit-freed-c`.
10. **State duck / pause** (sound-table §6.5 r2, §6.1): `state_duck()` always false; no sound ticks while ESC open. Blocked line test `0x00622AA0` always false (walls never muffle). → `q-fix-audio-pause-duck` (with q-fix-sp-pause).
11. **Weather/thunder** (triggers §12): `rules/draw_order/weather.rs:1080` draws x before y and stores 25+roll(50) as volume, spec: y first, then x, and it is a delay; `source.rs:119` hard-codes `thunder_sound_starts: false`. → in `q-fix-audio-ui-overlays`.
12. **Float steps** (sound-table §8.2 r8, r10, r12): volume.rs:142/159 round to f32 per op; spec keeps f64 intermediates (4 of 726,121 gain grid points differ). → `q-fix-audio-float-steps`.
13. **Voice log CLI** (audio.md §A5): `VoiceLog::to_jsonl` has no caller, no `--audio-log` flag, so exactness check 2 cannot run. → `q-fix-audio-log-flag`.
14. **drain_cues re-ids** (driver.rs:426): stops/params match only because both counters start at 0; any second cue producer breaks them. Fragile, correct today. → in `q-fix-audio-environment-wire`.
15. **Spec issues, no code change**: sound-table-2 §15 r4 slider x0 (h−133) vs ui/frontend-options.md r8 (h−48) conflict, three §15 drag vectors assume h−133 (`sliders.rs` unused); sound-table §7 r3 victim ordering ambiguous; triggers-2 §20 r1 (ClientFn ≤ 3 one call) vs r2 (ClientFn 3 twice). → spec session.
16. **Dead duplicate**: `triggers/ui.rs:45 ui_action` (live path is `ui/msg_ui.rs:372`, same table).

## Verified wired and matching

0x2C events 10, 13, 14, 15, 84–87, 90, 91, 93 and the player fall-through
(§3); object mode sounds (§7, triggers-2 §20); 0x5D UI table (§11); trade
event 23, object event 19; sound-table requests/fades/tick/channels/steal,
§16 cache, §17 streams; §19 handles; `UnitFreed` for set S. Thunder
position order and Globals above are the only rule-function mismatches.
