# Handoff: the parallel client pieces wired into the app (`claude/p6-integrate`, 2026-10-06)

> Not folded into `docs/HANDOFF.md` and `docs/PLAN.md` yet (a docs session folds it); this file stays as the detailed record.

Cloud implementation session, task class: integration from clear specs,
medium (METHODS M14). Base: `claude/tender-meitner-mphas3` at `2f8c7e3`
(p6-window, verify-map, client-own-gaps, drlg-data, wire-routing merged).
Repo only: no `game/`, no real GPU. Finishes the hand-offs those branches
left for each other (`p6-window.md` "Frame store", `verify-map.md` "For
p6-window", `client-own-gaps.md` §4, `drlg-data.md` §1). **No original
rule is added**: every §B point stays a `TODO(spec: …)` hook.

## State

**Wired, checked in CI by headless tests on synthetic data; the live-file
and real-GPU halves are queued below (unverified, M02).**

1. **Frame store in the app.** `world_view::ViewAssets::sets` (a map of
   frame sets) became `ViewAssets::frames: frames::FrameStore`. Units go
   through `composite::build_with(…, &assets.frames)` (the
   `ComponentResolver::frame_id` hook is no longer implemented by the
   world view); tiles and UI sprites take their ids from the store too.
   `world_view::FrameTable` / `BoundFrames` are gone: ids are the store's
   (insertion order), not first use per frame. `GpuAtlas` packs the
   store's frames in id order (`slots[n]` = `FrameId(n)`, as
   `FrameStore::atlas`), incrementally: frames the store gained since the
   last call are packed on top; an atlas ahead of its store is an error
   (`ViewError::AtlasAhead`; C2 eviction would rebuild both, not wired).
   The CPU reference reads the store directly (`FrameSource`).
   **`verify`** runs every case, map and synthetic, on the one shared
   `verify::gpu::Wgpu` (`verify::map::run_with(…, &mut gpu)`), opened at
   the first case; the single-map form (`--ds1` …) uses `run_with` too
   and now also prints the report lines.
2. **Text and sound.** UI text: `world_view::text_sprites` runs
   `ui::text::layout_text` for a `TextRequest` (font table from the new
   `ViewAssets::fonts`, glyph frames from the font's DC6 set in the
   store). What the request does not carry is a new hook trait
   `world_view::TextHooks` (`text_font`, `text_rules`, `glyph_look`, all
   `TODO(spec: ui/text.md)` §B3); `Unspecified` implements it with
   refusals and `NoTextRules`, and its `ui_text` now goes through
   `text_sprites`. Sound: new `app::sound` — `GameAudio` (the
   `AudioEngine` whose `SoundBank` is `PoolBank`: sound table hook →
   `audio::pool::SoundPool::load`), an `Update` system after the bridge
   frame (pool `begin_frame` with the bridge frame number, cue source
   drained, the frame's server tick presented, errors logged and counted
   in `AudioStats`), and `add_output` (the `MixerStream` on Bevy's audio
   device, window only). Hooks with placeholders: `SoundTable` /
   `NoSoundTable` (`audio/sound-table.md`), `WavDecoder` /
   `NoWavDecoder` (`formats/wav.md` §B1), `CueSource` / `NoCues`
   (`audio/triggers.md`). So the window plays nothing yet, by design.
   With `D2_GAME_DIR` the pool reads the user's `ArchiveSet`.
3. **Live levels.** `single_player::GameData::Live` now holds
   `Arc<LiveData>`: the waypoint tables plus drlg-data's `LevelTables`
   and `WorldFiles` (`d2_server::world_data::archive::load`) and the
   `ArchiveSet`. `GameData::select(game_dir, synthetic)` picks: no
   directory or `--synthetic` → synthetic; a directory → live, and a
   directory that does not load is an error (no fallback, M07). The build
   goes through one `LevelSource`: synthetic = the bridge test's DRLG
   (unchanged behavior; its tile library is now drlg-data's `Dt1Files`
   type); live = `WorldTypes` (Maze / Presets / Outdoor dispatcher, built
   as drlg-data's game tests build it) behind `SharedTypes`, the parsed
   DT1s as tile source, acts 0 and 1 created with the server's town level
   ids 1 and 40 (`levels.md` §2 step 2) and init seed = the app's
   `--seed` (game +0x7C; what sets it is not specified, `TODO`). A town
   is generated at act creation (`levels.md` §3 step 8); the app
   generates Cold Plains and streams the first room of Cold Plains and of
   Lut Gholein. The staged units (waypoint object, sorceress) stand at
   fixed sub-tile offsets from that room's origin (synthetic origin is
   (0, 0): positions unchanged). Where the original places a joining
   player (spawn room, `levels.md` §10, then a position) is not wired.
4. **Tests.** `app_frame_loop.rs`: the existing loop test unchanged and
   passing; the GPU node test now runs on a store-backed asset set; new
   `frame_loop_uses_the_frame_store_text_layout_and_sound_pool` (headless
   app on the synthetic game: three store tiles plus "Hii" from a panel,
   glyph items carry the store ids of their DC6 frames at the rules'
   placements, presented image = CPU reference; two cues for one sound →
   two voices, two `Start` log records with the file, one decode);
   `placeholder_hooks_refuse_text_and_sounds` (Unspecified text names
   `ui/text.md`; default audio parts start no voice, decode nothing);
   ignored `frame_loop_runs_on_the_users_levels`. `app_single_player.rs`:
   new `data_selection_falls_back_only_without_game_files`, ignored
   `live_data_generates_the_levels_from_the_users_files`.
   `world_view/tests.rs`: updated to the store; new
   `frame_ids_are_the_frame_stores` (reversed insertion order → other
   ids, same rows and image) and the atlas growth / `AtlasAhead` checks
   in `gpu_packing_emulates_to_the_cpu_image`. M08 by hand: mapping a
   glyph to its record index instead of its DC6 frame fails the new loop
   test (it passed until the fixture's record order was made to differ
   from frame order; fixed); not presenting the tick fails it too.

## Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/world_view/mod.rs` | `ViewAssets { cofs, fonts, frames: FrameStore, maps, palette }`, `ViewAssets::id`; `build` via `composite::build_with`; `GpuAtlas` (store frames in id order, incremental, `frames`, `slots`); `ViewError::{Frame, AtlasAhead, FontMissing, Text}` | `render-pipeline.md` §A1, §A7, §A9 |
| `crates/d2-client/src/world_view/ui_bind.rs` | `TextFont`, `TextHooks`, `text_sprites` (`layout_text` → sprites); `Unspecified` text through it | `ui.md` §A3 |
| `crates/d2-client/src/app/sound.rs` (new) | `SoundTable`, `NoSoundTable`, `NoWavDecoder`, `NoCues`, `PoolBank`, `AudioParts`, `GameAudio`, `AudioStats`, `add_audio`, `add_output` | `audio.md` §A1, §A3; `assets.md` §A5 |
| `crates/d2-client/src/app/single_player.rs` | `LiveData::load`, `GameData::{Synthetic, Live(Arc<LiveData>)}`, `GameData::select`, private `LevelSource::{synthetic, live}`; `UNIT_Y` | `levels.md` §2, §3; `bridge.md` §3 |
| `crates/d2-client/src/app/play.rs` | `add_game` also adds the audio frame; `run` uses the user's archives for sounds and adds the output | `bridge.md` §8 |
| `crates/d2-client/src/main.rs` | `play` via `GameData::select` (prints table and file counts); `verify` on one shared `Wgpu` | `render-pipeline.md` §A10 |

## Signature changes (outside nothing; inside my files)

- `ViewAssets::sets` removed → `ViewAssets::frames` (`FrameStore`); new
  field `fonts`. `WorldFrame::frames`, `FrameTable`, `BoundFrames`,
  `FrameSlots` removed. `GpuAtlas::ensure(&FrameStore)` (was
  `(&WorldFrame, &ViewAssets)`), `GpuAtlas::sets()` → `frames()`, new
  `slots()`. `ViewError::{SetMissing, FrameIndex}` → `Frame(StoreError)`.
- `GameData::Live(WaypointTables)` → `GameData::Live(Arc<LiveData>)`.
- `SimGame` construction stays in one place (`single_player::build`:
  `SimGame::with_events(game, sim)`, then `join`, `set_player`); the
  host-merge change there is a one-line fix.
- No `Cargo.toml` change (d2-server, d2-sim, d2-data, d2-formats were
  already dependencies). No file outside `crates/d2-client/{src/main.rs,
  src/app.rs, src/app/, src/world_view/, tests/app_*.rs}` and this note.

## Seams reached (stopped, not invented)

- Every §B rendering rule is still `Unspecified`; the window stays black
  (palette all zeros, §B3); nothing is loaded into the frame store
  because no rule names a frame set.
- Text: font per style, layout rules, glyph color → shade (`ui/text.md`
  §B3); layout options (`TextOpts`) stay the defaults.
- Sound: sound table, WAV decode, cue source, gain curve, voice policy.
- Live levels: the original's init seed source (game +0x7C), the
  join-time player placement (`levels.md` §10 spawn room + position),
  the waypoint object from a level's presets (the app still allocates
  one; the objects come from the presets only on `WorldSim`, which the
  app does not run: it runs `ActionSim` + `ActionWorld` as before).
- Material2d path (`render/`, `app::Mode::Verify`, `VerifyConfig`,
  `compare_rgba`) is still compiled: `view` uses `PaletteMaterial`, and
  removing `Mode::Verify` / `render/` is a separate cleanup
  (`verify-map.md` "then is removed"); not done here.

## Questions

- Q1. The live acts take the app's `--seed` as the DRLG init seed
  (game +0x7C). Which spec owns game creation's seeds (where +0x7C comes
  from)? Until then a live run is reproducible but not the original's
  seed for a given game.
- Q2. `GpuAtlas` packs every resident frame, not only the frame's: fine
  while residency is small; with C2 wired, should the atlas follow the
  store's eviction (rebuild) or pack per frame?

## Local checks to queue (`docs/HANDOFF.md` §5)

1. Window with real files: `D2_GAME_DIR=<game> cargo run -p d2-client
   --release -- play --frames 1500`. Expect `play: game data from
   D2_GAME_DIR (<n> levels, <m> objects, waypoint object class <k>; level
   files: <a> DS1, <b> lvlsub DS1, <c> DT1)` (record n, m, k, a, b, c;
   drlg-data measured 2,043 lvlprest DS1s), then `single player: seed
   1234, …`, a black 800×600 window, log lines every 250 frames with ≈ 25
   server ticks per second, `gpu: true`, node frames = frames − 1 or − 2,
   `audio Some(AudioStats { … load_errors: 0, engine_errors: 0 })`, no
   error, exit 0. A DRLG error names the level; record it (it would be a
   live-data finding for the level types, like drlg-data DL1).
2. `D2_GAME_DIR=<game> cargo test -p d2-client --test app_single_player
   --test app_frame_loop -- --ignored --nocapture`. Expect
   `live_tables_give_a_waypoint_object`,
   `live_data_generates_the_levels_from_the_users_files` (prints rooms
   and rect of act 0 levels 1 and 3, act 1 level 40; record them: Cold
   Plains should be rect (920, 984, 80, 80) only if the init seed were
   the recorded one, which it is not, Q1) and
   `frame_loop_runs_on_the_users_levels` (101 frames, 100 ticks) to pass.
3. `verify` on real cases, one device: `cargo run --release -p d2-client
   -- verify`. Expect one `GPU compositor: adapter: <real GPU>` line
   before the first case, then as `verify-map.md` checks 1 and 3: the
   map case lines (`<C> chunks of at most 1024x1024`, `CPU binned: 0 of
   …`, `GPU indices: 0 of …`, `GPU: 0 of …`, `PASS map (map)`) and
   `summary: 11 pass, 0 fail, 0 error, 0 GPU not wired, 0 no adapter`,
   exit 0. With `--perturb 7`: every case FAILs with exactly 7 on each
   comparison, exit 1.
4. Single-map form: `cargo run --release -p d2-client -- verify --ds1
   'data\global\tiles\ACT1\TOWN\townN1.ds1'`. Expect the same map lines
   (now with the report lines printed) and exit 0.
5. GPU node on a real GPU (store-backed assets now): `cargo test -p
   d2-client --test app_frame_loop -- --nocapture`. Expect `adapter:
   <real GPU>` and all non-ignored tests `ok` (0 differing pixels).

## Gate results

`sh tools/gate.sh` on this branch after merging the base at `93b37c8`
(which carries the d2-proto regeneration `e909c15`; before that merge
the gate failed only on d2-proto's stale `generated.rs`, not this
branch's change): all 13 steps PASS, `GATE: PASS`.

Measured here (cloud, Mesa llvmpipe 25.2.8 / LLVM 20.1.2 from `apt-get
install mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0`, not part of
`tools/cloud-setup.sh`): `cargo test -p d2-client --test app_frame_loop
-- --nocapture` printed `adapter: llvmpipe (LLVM 20.1.2, 256 bits)
(Vulkan, llvmpipe)`, 4 pass, 1 ignored (the GPU node test ran on the
store-backed assets, 0 differing pixels). `xvfb-run cargo run -p
d2-client -- play --synthetic --frames 600`: `play: synthetic tables and
levels`, log at frames 250 / 500 with 118 / 232 server ticks, `gpu:
true`, node frames 249 / 499, `audio Some(AudioStats { …, decodes: 0,
load_errors: 0, engine_errors: 0 })`; Bevy warned `No audio device
found` (no sound device in the container, so the output edge did not
play); exit 0. Without a device the mixer stream is never pulled; the
audio core still runs each frame.
