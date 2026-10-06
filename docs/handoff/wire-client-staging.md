# Handoff: client wiring of panels, feeds and audio — `claude/wire-client-staging`

> Not yet folded into `docs/HANDOFF.md` §1–§4 (only the §5 queue entries
> C76–C77 and `docs/PLAN.md` are edited here); the coordinator folds it.

Cloud implementation (wiring) session, 2026-10-06, task class:
implementation from specs, medium. Base: `claude/specs-staging` at
`2f860d1` (the merges of `impl-ui-panels`, `impl-lighting-blend`,
`impl-draw-order-2`, `impl-audio`, `impl-client-staging`, …). Repo only:
no `game/`, no `re/`, no recordings (M09). Read: the five handoff notes
named in the task, `specs/ui/panels.md`, `specs/client/model.md` §9, §12,
`specs/audio/{sound-table,triggers}.md` (sections cited in code).

## 1. What changed

| Path | What | Spec |
|---|---|---|
| `ui/original.rs` (+ `original_tests.rs`) | `OriginalUi`: `UiStates` (authority), `PanelTables`, panel state; `install` adds the adapters to a `UiRoot` in the §5 order (inventory 1, skill tree 4, character 2, border `PanelId(0x100)` kept open); per event `before_event` (model facts: class, P alive / dead, expansion game; mouse) and `after_event` (panel outputs in order: intent → `UiRoot::queue_intent`, `SetUi` → `UiStates::set` with the model's `GateEnv`, `ClickSound` → sound id 0; then a d2rs hotkey action no panel took → `SetUIState(ui, toggle, jump 0)`; root mirrors the flags); effects and sounds via `take_outcome`; `PENDING` table; `inventory_record` (class → `inventory.bin` record, + 16 at 800 × 600); `InvArea` | `ui/panels.md` §2–§10, §15; `items/inventory.md` §1.3 |
| `ui/root.rs` | `UiRoot::queue_intent` (intents still leave only through `forward`) | `client/ui.md` §A2 |
| `ui/edge.rs` | `KEY_CODES` (Bevy `KeyCode` ↔ controls `Key`, every keyboard key once), `key_of`, `key_actions` (bound world-context keys → `UiEvent::Action`) | `client/ui.md` §A4, §A6 |
| `world_view/ui_bind.rs` | `run_ui_with(…, Option<&mut OriginalUi>)`: dispatch event by event, the original UI applies its outputs between events; `run_ui` unchanged in behavior | `client/ui.md` §A2 |
| `world_view/present.rs` | `WorldViewUi.{original, bindings, art}`; key input (in `KEY_CODES` order, deterministic); after the UI frame: effects logged, sounds → `UiSounds` resource, `feed.set_ui_open_mode(original.open_mode())`, panel art made resident before the build | — |
| `world_view/feed.rs`, `model_feed.rs` | `ViewFeed::set_ui_open_mode` (default ignores); `ModelFeed.ui_open_mode` answers `open_mode` (camera §1) once set; `model_feed::PENDING` (see §2) | `ui/panels.md` §4.2, `render/camera.md` §1 |
| `world_view/panel_art.rs` | `PanelArtRules<R>`: `ui_image` = DC6 `data\global\ui\<name>.dc6` direction 0, frame, `draw_position` (cel draw position, bottom anchor), empty shade chain (light 0xFF), `Opaque` (mode 5); `ui_pass` = pass 11 (`draw-order.md` §10, as `OriginalView`); `PanelArtLoader::ensure` reads each named file once (missing / bad file → error, never skipped) | `ui/panels.md` §1.3, §1.4, §1.6, §7.1; `shading.md` §3 r1; `blend-modes.md` |
| `audio/driver.rs` | `SoundDriver` (a `CueSource`): UI sound requests (`triggers::ui::ui_sound`), then one `SoundSystem::run_tick` per server tick since the last frame (none before the first tick), cues handed to the core once; `ModelSoundWorld` (local player, unit cells; `blocked`, `indoors`, `client_seed` are recorded as pending questions and fail the frame with `DriverError::Pending`); `PENDING` table | `audio/sound-table.md` §4 r5, §6.1, §6.4 r2, §6.5 r2, §8.1; `audio/triggers.md` §1 r5, §11 |
| `app/sound.rs` | `AudioParts::original(source, table)` (sound paths, `D2Wav`, `DeviceGain`, `Unlimited`, the driver); `GameAudio.driver`; the audio frame drains `UiSounds`, runs the driver, presents the sound tick; `sound_table_live` (`sounds.txt` + `soundenviron.txt` from the archives) | `client/audio.md` §B1–§B3 |
| `app/ui.rs` | `UiParts` (`live`: `inventory.bin` `inv` rectangles, `d2exp.mpq` present), `add_original_ui` (root + adapters, `dev` bindings, art loader, `UiSounds`, `PanelArtRules` over `Unspecified`) | `ui/panels.md` §4.4, §9.2 |
| `app/play.rs` | `run` on live data adds the original UI and the original audio | — |
| `tests/app_original_ui.rs` | headless app: hotkey → inventory open → 16 UI items (art 4, close 1, right border 5, control panel 6), files read once, feed open mode 1 / 0, sound tick = server tick; `#[ignore]` live-install test (C76) | — |

Tests added: 11 in `ui::original::tests`, 2 `ui::edge::key_tests`, 2
`world_view::panel_art::tests`, 4 `audio::driver::tests`, 2
`world_view::model_feed::tests`, 1 + 1 ignored in `app_original_ui`. Each
carries its `// Covers:` claim; spec vectors used: §Test vectors rows
"800 × 600, inventory open", "char panel open", "`SetUIState(1, on)`",
"inventory open, `SetUIState(2/4, on)`", "amazon skill tree tab 1",
`inventory.bin` records 0 / 16.

## 2. Pending (input the client model or app does not hold; nothing guessed)

The three `PENDING` tables in code are the record; in short:

- **`ClientWorld::active_rooms`**: no client DRLG on the base. `model.md`
  §12 r1 (the act built from the 0x03 seed with the client flag, rooms
  activated by 0x07) is not implemented; the bridge still only appends
  `rooms_in_sight`. Everything downstream waits on it: `near_rooms`,
  `map_tiles`, `tile_art`, `tile_blocks`, the player's level (so
  BlankScreen, the sound environment, weather level presets).
- **`ViewFeed::light`**: room ambients and collision (no client rooms),
  act environment (S→C 0x53 has no handler), light records, per-unit
  look inputs.
- **`ViewFeed::weather_frame`**: level presets (no level), no water floor
  without tiles; passes 4 / 9 have no art path (a live pool would fail
  the frame).
- **Panels**: character values / labels / stat box / add buttons (total
  stats `0x00625480` vs the model's layer-0 base stats, resist effects,
  language, strings by id); inventory equipment backgrounds (item stream);
  skill icons (no skill list: S→C 0x94 unhandled; `CC` prefix open);
  waypoint (S→C 0x63 unhandled), stash / cube / NPC / shop (openers
  unhandled); cursor jump (no warp edge; only waypoint / 0x0F pass jump
  1); other hotkeys (panels unspecified).
- **Audio triggers**: 0x2C, 0x5D (no handlers), mode / footstep / idle /
  object / skill / item / NPC sounds (unit animation state, hit class,
  states, monsounds rows, floor material, events not in the model),
  environment (no level, day phase, weather). The local player's client
  seed is read-only in the model (shared with the camera shake,
  `camera.md` OQ 6): a request that rolls a variant fails the audio frame.

## 3. Readings chosen (for the spec owners)

1. `ui/panels.md` §4.4: the `inv` rectangle's bottom is read exclusive
   like its right (spec silent; `impl-ui-panels` §3 lists it).
2. §2.5 "P alive" uses `ClientUnit::is_dead` (mode only): unit flag
   0x10000 is not in the model (`model.md` OQ 4 area).
3. `GateEnv`: `chat_blocked` and `input_hold` are false, but only ui 5 /
   9 read them and nothing requests those; `modal_text` and `npc_active`
   are false because d2rs has no modal text screen and no NPC interaction
   yet (facts of the client, not of the original).
4. Hotkeys: the d2rs `dev` actions `ToggleInventory` / `ToggleCharacter`
   / `ToggleSkillTree` toggle states 1 / 2 / 4 with jump 0 (§4.3: "hot
   keys pass 0"); the original key table is `ui/controls.md` (OQ 2).
5. Character adapter: the `draw` rows whose conditions need no panel
   word are emitted in file order (art, close); stat points are passed as
   0 to `press` / `release`, which keeps the add buttons inactive, as
   their rows are not drawn.
6. Skill tree view: class from the model; no skills, no icon file,
   free points 0 and flag mask 0 — read only for icons, so inert.
7. `sound-table.md` §6.5 r2 (OQ 6): the state-duck condition is taken as
   never holding: d2rs has neither a game menu nor a pause, the states
   the spec names as likely. Revisit when the condition is read.
8. `SoundWorld::position` returns the model's cell (sub-tile) of the
   unit; units per type are `sound-table.md` OQ 2.
9. Sound tick = one per server tick since the previous frame (`triggers.md`
   §1 r5: C and T advance once per client update, one per server tick in
   single player); the first frame with a tick runs one.

## 4. Gate (this branch, head in the commit)

- `cargo test -p d2-client`: lib 822 passed, 0 failed, 8 ignored; every
  integration target passes (`app_original_ui` 1 + 1 ignored). The 47
  bridge failures the earlier notes report are gone on this base (the
  0x7A / 0x81 handlers landed with `impl-client-staging`).
- `cargo clippy -p d2-client --all-targets -- -D warnings`: clean.
  `cargo fmt --all --check`: clean. `cargo run -p depcheck`: OK.
- `python3 tools/coverage.py --check`: 7,115 claims, 0 errors.
  `python3 tools/spec_index.py --check`: ok. `tools/methods.py check`: ok.
- Not run here (other sessions' known reds on the base, untouched):
  `monsters::ai::tests::specd_here_*`, `skills::use_::tests::*`,
  `skills::mutant_tests::table_check_mutants::*` (`d2-sim`).
- Cloud setup: `apt-get install pkg-config libasound2-dev libudev-dev
  libwayland-dev libxkbcommon-dev` (as CI).

## 5. Local run queue (added to `docs/HANDOFF.md` §5)

- C76 `D2_GAME_DIR=<install> cargo test -p d2-client --test
  app_original_ui -- --ignored` (live `inventory.bin` areas, sound
  table, every wired panel's DC6 per class).
- C77 `play --frames 500` smoke with I / C / T (log only: the palette is
  still all zeros), then `ui-0001` / `ui-0002` captures once a palette is
  presented.

## 6. Next steps

1. The client DRLG (`model.md` §12 r1–r2): a bridge-owned `d2_sim::drlg::Drlg`
   built from 0x03 with the client flag, 0x07 / 0x08 setting rooms in
   sight (§9 r1–r2), `active_rooms` from its active rooms. It unblocks the
   map, tile blocks, the level (BlankScreen, sound environment, weather),
   the light map's ambients and the sight test.
2. The act palette in the app (`ViewAssets::from_pl2` from
   `ClientWorld::palette_act`, `play.rs` TODO), so panels and the world
   are visible.
3. S→C handlers that open panels / make sounds (0x63 waypoint menu, 0x77,
   0x2C, 0x5D, 0x94 skills) per their `client/msg-*` owners.
4. The local player's client seed as a mutable model field shared by the
   camera shake and the sound layer (`camera.md` OQ 6, `sound-table.md`
   §4 r5).
