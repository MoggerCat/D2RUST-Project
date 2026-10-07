# Handoff: S-D2 character and panels (`claude/play-char`)

> Implementation session, 2026-10-07. Read only `specs/`, `docs/`,
> `crates/`, `tools/`. Scope: `docs/handoff/first-playable-scope.md` §5,
> gaps **G13** (with decision D3) and the **binding half of G15**. Owned
> files: `crates/d2-client/src/{main.rs, app/single_player.rs,
> world_view/ui_bind.rs, ui/original.rs}`, plus one new test file
> `crates/d2-client/tests/app_new_character.rs`.

## What was done

| Gap / item | Change | Where |
|---|---|---|
| G13 (D3) | `play --new <class> <name>`: class by name (`amazon` … `assassin`, any case) or `0`–`6`; name 1–15 bytes of `A–Z a–z 0–9 - _`. `--new` with `--save` is an error. Prints `play: new character <name> (<class>), not saved`. Nothing is written to disk. | `main.rs` (`Options::new`, `play`), `app/single_player.rs` (`Character::Named(NewCharacter)`, `new_character`, `parse_class`, `CLASS_NAMES`, `MAX_NAME_LEN`) |
| G13 | `Character::Named` builds its C→S 0x67 with the chosen class and name (`create_request_for`). At the join it takes the same stub load as `Character::New` (`load_new_character`, `intents-events.md` §8.2 r7). | `app/single_player.rs` |
| G15 binding | Character panel (ui 2) bound to the client model. `ModelCharacter` implements `CharacterView`: `stat` is `ClientWorld::total` and `base` is `ClientWorld::base` (layer 0, `stat-lists.md` §1 r3); `alive` comes from the unit; the popup width is the Font16 max width (`panels.md` §8 r8). The panel draws the stat-points block, values (level, experience, next level, attributes, life / mana / stamina, defense, resists) and the name line (`panels-2.md` §17 r4: font by code-point count, centred). Add buttons spend through the root (C→S 0x3A). | `ui/original.rs` (`CharacterUi`, `ModelCharacter`, `name_line`, `FontMeasure`, `CHARACTER_FONTS`, `OriginalUi::set_fonts`, `OriginalUi::set_resist_penalties`) |
| G15 binding | Expansion resist penalty: `client_resist_penalties(archives)` reads `difficultylevels.ResistPenalty`. Without it, an expansion game draws no values (classic needs no table, §8 r9). | `app/single_player.rs`, `ui/original.rs` |
| Risk row (`ui_bind.rs:161`) | `TextColors::push`: the `try_into().expect("256-byte row")` is now a `ViewError::Unresolved` error. | `world_view/ui_bind.rs` |
| Text assets | `TextAssetLoader::ensure(draws, assets)` loads each text draw's font `.tbl` into `ViewAssets.fonts` and its glyph DC6 (direction 0) into `ViewAssets.frames`, once per font. It is the text half of `PanelArtLoader::ensure`. | `world_view/ui_bind.rs` |
| I key | Already wired: the `dev` preset maps I to `ToggleInventory` and C to `ToggleCharacter` (`controls/mod.rs:71-72`). The inventory panel opens with its art and close button and no contents (G16 deferred). No change. | — |

Fallback without the seams below: with no fonts set, the character panel
draws only its art and close button, exactly as before. Nothing new can
fail a frame until the seams are connected.

Tests (all synthetic fixtures, no game files):
- `app::single_player::new_character_tests` (3): class parsing, name rules, the 0x67 bytes.
- `tests/app_new_character.rs`: the join loads a `--new` barbarian "Conan" (class 4, name stored, 0x5F after 0x0B). This test lives in `tests/` because `bridge::gaps_numbered_tests` forbids link calls in `src/` outside the bridge.
- `ui::original::character_bind_tests` (5): values and name from the model, with positions, fonts and colours; the long-name font switch; expansion penalties; the stat-point box plus an add-button spend through the root (0x3A); without fonts, only the art.
- `world_view::ui_bind::text_tests::text_assets_load_once_per_font`.
- `main.rs` `new_takes_a_class_and_a_name`.

Gate on the pushed head: `cargo fmt --all --check`,
`cargo clippy -p d2-client --all-targets -- -D warnings`,
`cargo test -p d2-client` (all pass, 1491 lib tests) and
`python3 tools/coverage.py --check` (0 errors).

## Gap ids closed

- **G13**: closed, through the D3 CLI stand-in. The select / create menus are still unspecified.
- **G15, binding half**: done on the code side. Two things are still missing before values show on screen: the seams below, and S-C's 0x1D–0x1F at the join (the send half).
- Risk row "`ui_bind.rs:161` panic": closed.

## Seams other sessions must connect

1. **S-B, `app/ui.rs` `add_original_ui` / `UiParts::live`**. Make `original` mutable, then before `install`:
   ```rust
   original.set_fonts(FontMeasure::load(&*parts.source, &CHARACTER_FONTS)?);      // crate::ui::original
   original.set_resist_penalties(single_player::client_resist_penalties(&archives)?);
   ```
   `UiParts::live` already holds the archives. A load error is an error, not a fallback.
2. **S-A, `world_view/present.rs`**. Add a `text: Option<TextAssetLoader>` field to `WorldViewUi`, set by S-B to `TextAssetLoader { source: parts.source.clone() }`. Right after `art.ensure(&frame.draws, &mut state.assets)?` (about `present.rs:528`), call `text.ensure(&frame.draws, &mut state.assets)?`. Without this, the first text draw fails with `ViewError::FontMissing`. Seams 1 and 2 must land together.
3. **S-B, text colours**. `PanelArtRules { rules: Unspecified }` answers `ui_text` with `Unspecified`, which has no text-colour maps. A glyph in colour 1, 3 or 4 is then an error (strength above base is blue, for example). The rules type should answer `ui_text` through `text_sprites(&OriginalTextHooks { colors: Some(TextColors::push(&mut assets.maps, act_pl2)?) }, …)`, where `act_pl2` is the act's `pal.pl2` bytes (`ui/text.md` §4.4). Until then, S-A's per-item skip (G19) keeps such a value from blacking the frame. A fresh character mostly draws colour 0.
4. **S-C**. Send S→C 0x1D–0x1F at the join (G15 send half). Until then the model's player stats are empty, and the panel shows 0s (life shows 1 while alive, §8 r7).
5. `ClientWorld::difficulty` and `expansion` are read from the model; S→C 0x01 already fills them (`bridge/msg/session.rs:32-33`). No action needed.

## Findings

- `ui/panels/character.rs` `resist_value` (not owned here) still colours a resist with no active effect by the §8 r7 compare. `panels.md` §8 r9 now says: shown < 0 → red (1), shown at the cap → colour 4 unless blue. The owner of `ui/panels/character.rs` should fix it. My test only asserts the value (`-90`), not the colour.
- The labels (§8 r6) and the class line (`panels-2.md` §17 r3) need the string table by id. Play installs `NoStrings`, so they stay undrawn; see `PENDING` in `ui/original.rs`.
- Shift is not part of `UiEvent`, so an add-button release always spends 1 point.

## Local checks for the user (need `D2_GAME_DIR`, not run here)

1. `cargo run -p d2-client --release -- play --new amazon Test --frames 600`
   - Expected: `play: new character Test (amazon), not saved`, then the usual join log.
   - After S-B lands, the player drawn is an Amazon.
   - Afterwards `git status` shows no new file, and no `Test.d2s` exists anywhere (nothing is written).
2. `cargo run -p d2-client --release -- play --new 7 Test`
   - Expected: exits with `unknown class "7": use 0-6 or one of amazon, sorceress, …`.
   - `--new druid "two words"` exits with the name error.
3. `cargo run -p d2-client --release -- play --new paladin Uther`, press **C** (needs seams 1–2 and S-C's 0x1D–0x1F):
   - The character panel's art and close button appear on the left.
   - "Uther" is in Font16 on the top line, centred in x 93–240 at y 85.
   - Level `1` appears at the level box (y 119).
   - Strength 25, dexterity 20, vitality 25, energy 15: the user's `charstats` start values for the Paladin.
   - Life, mana and stamina show their current / max values.
   - Resists show 0 on Normal.
   - Without the seams, only the art and close button show, as before.
4. Same run, press **I**: the inventory panel art and close button appear on the right, with no items. Press **I** again and it closes.
5. `cargo run -p d2-client --release -- play --save <your.d2s>` still loads the save; the panel shows its name.

All of the above is unverified against 1.14d (rule 10). The panel
positions come from `panel-layout.tsv`. The capture compare for the
character panel is not queued yet: add it to HANDOFF §5 once a
`SceneSource` exists.

## What's left

- Seams 1–4 above.
- Labels and class line: they need a string table (`StringLookup` by id) in play.
- Resist effects (the `0x0063A570` state tests), the damage block and the popups (`panels-2.md` §17 r5–r9).
- Shift-spend.
- The colour fix in `character.rs` (findings).
- The original select / create screens (a menu spec), replacing D3.
