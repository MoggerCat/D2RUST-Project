# Play preview: HUD and panel details (`claude/stitch-hud`)

> Stitching session, 2026-10-07. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Nothing here is verified against 1.14d (rule 10). Every
> preview fill is marked `// d2rs-own, unverified`. Audio is not wired.

## Links connected

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Control panel overlays → draw list | `ui/panels/control/*` held every rule (globes, bars, buttons, mini panel, input), but only the base art was drawn (`ui/original.rs` `BorderUi`, `border.rs` doc: "overlays … not drawn here") | New `ui/hud.rs` (`HudUi`, a child module of `ui::original` that shares its `Shared`), installed after the border and open for good (`HUD_PANEL` 0x101). It draws every frame from the model: life globe (stats 6/7, potion state 100, stat 74, poison state 2), mana globe (8/9, state 106, stat 26), experience bar (12/13 plus `experience` rows), run button (the walk's run lock), stamina bar (10/11), menu button, left / right skill icons (`SkillList::left_entry` / `right_entry` → `Spells\<CC>Skillicon`, frame = skilldesc `IconCel`), and the mini panel when state 0x15 is open |
| 2 | HUD files → art loader | the HUD files were not in `UiFiles` | `UiFiles::extend` (`ui/panels/mod.rs`); `OriginalUi::new` registers `hud::hud_files()` |
| 3 | Control panel input (§10) | `CtrlInput` / `MiniPanel` not called | `HudUi::event`: the run button gives `run_toggles`; the menu button opens or closes 0x15; the mini panel buttons open C / I / skill tree (`function_actions`); a skill button toggles state 3 |
| 4 | Run button → walk | — | `world_view/present.rs` (one block, in the existing `PreviewWalk` branch): `OriginalUi::sync_run(run_lock)` returns the toggles, which are applied to `RunMods` |
| 5 | Skill icon / experience tables | not loaded | New `app/hud.rs` `hud_tables` (`skills` `charclass` + `skilldesc` `iconcel`; `experience`); `app/play.rs` calls `install_hud_tables` after `add_original_ui` (one line) |
| 6 | Skill select → 0x3C | the skill select panel (`0x004AA7E0`) has no layout spec | d2rs-own: with state 3 open, the local player's skills (level > 0) are drawn as icons in rows of 10 above the button that opened it. A click on one sends C→S 0x3C `SelectSkill{skill, left, item −1}` and closes state 3. A click elsewhere only closes it. The button changes when S→C 0x23 arrives (the client's existing handler) |
| 7 | Inventory gold (subagent) | not drawn | New `ui/panels/inv_gold.rs`: stat 14 total in Font16 at (W − sx − 212, H + sy − 72) and the `goldcoinbtn` art (`panels-2.md` §21 r1). There are 4 new ui-1 rows in `specs/ui/panel-layout.tsv` |
| 8 | Character stat-points block (subagent) | gated on the total of stat 4 | gated on the base (`panels-2.md` §17 r1), as the click handler already was |

Preview fills (D1), all marked in the code:

- **Bars.** The UI draw list has no line or rectangle, so the bars are cels of a synthetic file `d2rs\hudfill`, clipped to the bar. Its frames are red, gold, blue and index 0xFF, made in `PanelArtLoader::ensure`; each colour is the palette's nearest index. Stamina mode 2 (translucent) is drawn opaque.
- **Globe windows** are the cel clipped to its rows. Draw mode 0 vs 5 is not represented.
- **Smoothing counter C** is the UI frame tick.
- **Missing HUD art.** A HUD DC6 that no archive holds is drawn empty and logged once (`hud (d2rs-own, unverified): … in no archive, drawn empty`), so one missing file no longer blacks out the frame.
- **`skilldesc` link.** It is read as the `skilldesc` row index.

## Tests

- `crates/d2-client/tests/app_hud_e2e.rs` (new, synthetic): the globes are half and fully filled from S→C 0x1F stats; the stamina bar is 51 px gold; the run button frame is 0. Clicking the run button sets the run lock, and the button frame becomes 2. Clicking the left skill button opens state 3 and lists skill 1. Clicking skill 1 sends exactly `3C 01 00 00 80 FF FF FF FF` and closes state 3. A synthetic 0x23 then turns the left icon into skill 1's.
- Updated tests:
  - `ui/original_tests.rs`: the HUD panel is open for good.
  - `tests/app_original_ui.rs`: the fixture gains `goldcoinbtn`, and the item count is 17.
- New subagent unit tests: `inv_gold::tests::*`, `character::tests::the_base_statpts_gates_the_points_block`, and `original::tests::inventory_draws_the_gold_value_and_button_from_the_model`.

Gate results: `cargo fmt --all --check`, `cargo clippy -p d2-client --all-targets -- -D warnings`, `cargo test -p d2-client --tests` (lib 1519 passed, every integration test passed) and `python3 tools/coverage.py --check` (0 errors) are all clean.

## What is left

- **Server confirmation of 0x3C.** The e2e test injects the 0x23. The server-side player skill list is play-fix2's work (S→C 0x94; `adapters/character.rs` "no provider"). Once that lands, check that the server answers 0x3C with 0x23 on live data.
- **Text and labels.** Tool tips (run, menu, experience, stamina, mini panel), the globe numbers (strings 4165 / 4166), the character labels and the class line all need the string table by id; play still passes `NoStrings`.
- **Not drawn yet:**
  - the new-stats / new-skills buttons (§8);
  - the level-change timer;
  - the belt (stitch-items' area);
  - the stamina state group 24 (blue) and state 136, which are not in the model.
- **Quest log and game menu.** The quest log and game menu mini panel functions do nothing; there is no panel for them in play.
- **Esc game menu and automap (Tab).** Not done in this session. Esc and Tab have no binding in `Preset::Dev` handled by the original UI.
- **Character panel checks.** `panels-3.md` §24 r3 (defense colour comes only from states) and `character.rs`'s total-vs-base colour disagree. The `panels.md` §8.8 "max width" and `panels-3.md` §24 r5 "width A" also disagree. Both are left as they are and need a spec decision.
- **Other.** Stash gold (stat 15), and the gold button press and dialog (§21 r3–r9, in `PENDING`).

## Local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/stitch-hud
git checkout claude/stitch-hud
cargo run -p d2-client --release -- play --new sorceress Test
```

What you should see:

1. **The bottom panel.**
   - The red life globe on the left and the blue mana globe on the right are full, each with its overlap rim.
   - The stamina bar between the run button and the menu button is gold and full width.
   - The experience bar above it is empty at 0 experience, so no line is drawn.
   - The run button shows "walk".
   - The left and right skill buttons show the Attack icon (left), and the right skill's icon if the join selected one.
2. **Run button.** Click it: the button changes to "run" and the next click-to-move runs. Click it again to go back to walk. The R key still works and the button follows it.
3. **Menu button** (the small button in the centre). Click it: the mini panel opens above it. Its first three buttons open C, I and the skill tree. Click the menu button again to close it.
4. **Skill select.** Click the left skill button: a row of the Sorceress's skill icons appears above it (at level 1, only the skills she has). Click one: the row closes, and once the server answers (after play-fix2's skill list) the button shows the new skill. Copy any `hud (d2rs-own, unverified): … in no archive` warn line into `docs/HANDOFF.md`; it names an icon file the spec's name rule gets wrong (for example `Spells\Skillicon` for Attack).
5. **Panels.** Press I: the inventory shows the gold amount (0 for a new character) and the gold coin button at its lower left. Press C: the values are as before; the stat points box shows only with unspent points.
6. **Copy any of these into `docs/HANDOFF.md`:**
   - `frame not drawn: …` lines;
   - the globe fill looking upside-down (it should grow from the bottom);
   - a bar colour that looks wrong.
