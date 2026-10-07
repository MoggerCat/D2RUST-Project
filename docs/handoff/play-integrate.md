# Play preview: integration (`claude/play-integrate`)

> Integration session, 2026-10-07. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Connects the seams listed in `play-map.md`, `play-units.md`,
> `play-walk.md` and `play-char.md` (context: `play-server.md`) under
> decisions D1–D3 (`docs/PLAN.md`). Nothing here is verified against
> 1.14d (rule 10): every preview fill stays `// d2rs-own, unverified`.
> No file under `crates/d2-sim/src/drlg/` was touched.

## What was connected

| Seam | Where | What |
|---|---|---|
| play-walk 1 (link) | `app/play.rs` `predict_link` | The play link is wrapped in `PredictLink`. The link is boxed (`DynLink`), so the walks are read through a shared `WalkTap` (`PredictLink::tap`, `bridge/predict.rs`) instead of `bridge.link_mut().take_walks()`. |
| play-walk 2 (bridge) | `bridge/mod.rs` | `Bridge::world_click_at`, `Bridge::click_repeat_at` (mirror `world_click` / `click_repeat`, take `local_at`). |
| play-walk 3 (per frame) | `world_view/walk.rs` (new) | `PreviewWalk` resource (`Predict`, `RunMods`, `Speeds`, `WalkTap`) and the `preview_walk_frame` system (`PreUpdate`, after `bridge_frame`, before `mirror_units`): `Predict::frame` with the tap's walks; "ticked" = the model's server tick count changed. |
| play-walk 4 (view) | `world_view/feed.rs`, `model_feed.rs`, `unit_assets.rs`, `unit_rules.rs` | New `ViewFeed::set_local_prediction` (default: ignored). With a preview, `ModelFeed` answers `player()` (camera) and `unit_position` of the local player from the prediction. The predicted mode (2 / 3) is `UnitArt::pose_mode`; the unit rules and the art loader draw the local player in it (`UnitArt::posed`). |
| play-walk 5 (input) | `world_view/ui_bind.rs` `world_clicks`, `present.rs` | `world_clicks` takes `mods` and `local_at` and calls the seam-2 methods. With a `PreviewWalk`: `mods = run.word()`, `local_at = predict.position()`; an unhandled `Action::ToggleRun` (R) toggles the run lock; `Action::StandStill` (LeftShift) is held while its key is down (`ui_input`). Without one: `0` / `None`, as before. |
| play-walk 6 (speeds) | `app/single_player.rs` `walk_speeds` | The class's `charstats` `WalkVelocity` / `RunVelocity` from the live tables (`GameTables::rows::<Charstats>`); `None` on synthetic data (the prediction then only follows the model's placements). |
| play-units 1 (edge clip) | `world_view/feed.rs` | New `ViewFeed::edge_clip()` (default `false`); `ModelFeed` answers `true` with a preview. `build_frame` passes it to every `OriginalView` (both `build_lit` arms and `NoWorld`). Strict path unchanged. |
| play-units 2 (G8) | `model_feed.rs` | Already answered by the preview (`unit_offset` (0, 0), `unit_position` from the model / prediction). Checked by the new e2e test (the player's unit draws). |
| play-units 3 | `app/play.rs` | Play does not replace `state.rules` after `add_original_ui`, so the unit rules stay. `add_original_ui_with` / `install_unit_rules_with` take the unit tables (for tests); `add_original_ui` reads them from the archives as before. |
| play-char 1 (fonts, penalties) | `app/ui.rs` | `UiParts` has `fonts` and `resist_penalties`. `UiParts::live` loads `FontMeasure::load(&archives, &CHARACTER_FONTS)` and `client_resist_penalties(&archives)` (a load error is an error). `add_original_ui` calls `set_fonts` / `set_resist_penalties`. |
| play-char 2 (text assets) | `world_view/present.rs` | `WorldViewUi::text: Option<TextAssetLoader>`, set by `add_original_ui`; `text.ensure(...)` runs right after `art.ensure(...)`. |
| play-char 3 (text colours) | `world_view/panel_art.rs`, `app/ui.rs` | `PanelArtRules::text: Option<SharedTextColors>`: `ui_text` answers through `text_sprites(&OriginalTextHooks { colors }, …)`. The `push_text_colors` system pushes the palette act's PL2 maps (`TextColors::push`) into `ViewAssets.maps` once per act (runs when `ActPalettes` exists, i.e. with game files). |
| play-char finding | `ui/panels/character.rs` `resist_value` | Resist colour now follows `panels.md` §8 r9 exactly: 3 raised, then 1 lowered; at the cap 4 unless 3; below the cap a shown value < 0 is 1. The old no-effect `cmp` colour is gone. The existing test's last assertion (lowered at the cap) expected 1; §8 r9 says 4, so it now asserts 4. New test `resist_colour_order` covers every branch. |
| play-server 1 | — | Already done on `play-char` (start items through `load_new_character_with_items`). |

Also: `WorldViewState::last_tags` / `last_ui` (the last drawn frame's item
tags and UI draws), for logs and the e2e test.

## Test

`crates/d2-client/tests/app_play_e2e.rs` (synthetic fixtures only: a
one-tile DT1, an invented player token `OY` whose torso file
`OYTRlitTNhth` is the one player name read as a DC6, panel and glyph
DC6s, font tables, zero act palettes). It wires the app as `play --new
sorceress Test` does and checks:

1. the frame has tile items and the player's unit item;
2. a left click on the ground sends a walk (the prediction has a walk) and
   the predicted position moves; frames keep drawing;
3. C opens the character panel and the frame draws the text "Test".

Synthetic-only fill in the test: the synthetic join sends no skill list,
so the test injects S→C 0x94 (skill 0) + 0x23 (left skill 0) before the
click (without a left skill the click does nothing, `ui/controls.md` §6
r8.1); and it gives `ResistPenalty` values (the synthetic game is an
expansion game). Live data sends both itself.

Gate on the pushed head: `cargo fmt --all`, `cargo clippy -p d2-client
--all-targets -- -D warnings`, `cargo nextest run -p d2-client` (1645
passed), `python3 tools/coverage.py --check` (0 errors).

## Local run steps (Windows, PowerShell, 1.14d files)

Set the install once per shell:

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
cd <your clone of D2RUST-Project>
git fetch origin claude/play-integrate
git checkout claude/play-integrate
```

The DRLG room-free fix (play-drlg, G1) is merged into this branch, so the
tick-104 panic (`drlg/room.rs` `expect("live DRLG room")`) should be gone.
If it still appears, copy the panic line into `docs/HANDOFF.md`.

### 1. New character

```powershell
cargo run -p d2-client --release -- play --new sorceress Test
```

What to see:

- Console: `play: new character Test (sorceress), not saved`, then the
  join log; every 250 frames a `frame N:` line with `last view
  Some(FrameStats { items: <hundreds>, units_drawn: <several> .. })`.
- Window: the Rogue Encampment's floor and walls, full bright, in the act
  1 palette, centred on the Sorceress (light armour, bare hands, facing
  direction 0). Town NPCs and the waypoint cycle their idle animation.
  The control panel is at the bottom.
- Left-click the ground a few tiles away: the view and the player move
  smoothly toward the click (about 9 sub-tiles a second) and stop there.
  A second click walks on from where the player is drawn, with no jump
  back. While moving, the player's walk animation shows if its COF loads
  (a `unit art: … in no archive` warn for `…WL…` / `…TW…` means the
  walking pose is not drawn; the standing pose returns when the walk ends).
- Hold Left Shift and click: no walk.
- Press R, then click: still a walk, not a run, until the join sends
  stamina (S→C 0x1D–0x1F, play-walk caveat). That is expected.
- Press C: the character panel opens on the left. "Test" in Font16 is
  centred on its top line; level 1, the Sorceress's start attributes
  (str 10, dex 25, vit 10, ene 35 from your `charstats`), life / mana /
  stamina, defense and the four resists (0 on Normal) show if the join
  sent the stats; otherwise 0s (play-char seam 4). Resists at the cap are
  colour 4, negative ones red. Press C again: it closes.
- Press I: the inventory panel opens on the right (art and close button,
  no items). Press I again to close.
- Expected `warn` lines: a few `preview (d2rs-own, unverified): skipped:`
  tiles and `unit art: … in no archive` for player component files of the
  D1 fills (for example `RH` / `LH`). Copy any `frame not drawn: …` line
  or a `.COF` warn for a town NPC into `docs/HANDOFF.md`.

### 2. Existing save

```powershell
cargo run -p d2-client --release -- play --save "C:\Users\<you>\Saved Games\Diablo II\<Name>.d2s"
```

What to see: the same as step 1, with the save's class drawn (no items:
D1), and the save's name on the character panel's top line. The walk
speed is the save's class's `charstats` speed.

### 3. Smoke run (no window interaction)

```powershell
cargo run -p d2-client --release -- play --new sorceress Test --frames 600
```

What to see: the window opens, the log has `frame 250:` and `frame 500:`
lines, then `play: exiting after 600 frames`, exit code 0. No panic
(the DRLG fix is merged; a panic is a bug to report). `git status` shows no new file.

## What is left

- Stamina at the join (G15 send half) so R runs; the command-34 run-held
  binding and C→S 0x53 / 0x54.
- The synthetic join sends no skill list (fine on live data).
- Character labels / class line (need a string table in play), resist
  effect tests, the damage block.
- Predicted facing; REC-51 to replace `bridge/predict.rs`.
- play-drlg's seam request (a synthetic level-2 room next to the town in
  `app/single_player.rs` for `tests/app_level_border.rs`) is not done here.
- Queue in `docs/HANDOFF.md` §5 (not done here): the capture compares for
  the map, units and the character panel once a `SceneSource` exists, and
  REC-51.
