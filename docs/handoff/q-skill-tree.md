# q-skill-tree: skill tree icons, levels and point spend in play

## Links connected
- `ui/skill_tree_ui.rs` (new): `SkillTreeTables` (class skill rows from `skills` + `skilldesc`: page, row, column, `IconCel`, max level, InGame, required level/skills/stats) and `entries()`, which joins them with the local player's stats and `SkillList` into the panel's `SkillEntry`s (level, hard points, bonus, learnable, req-level, below-max, flag byte).
- `OriginalUi::set_skill_tree_tables`; `ModelSkillTree` now returns the icon file (`spells\<cc>skillicon`, `CC` = Am So Ne Pa Ba Dr As, `panels.md` §10.3), free points (stat 5), flag mask 4 and the entries. The seven icon files are registered in `UiFiles` in `OriginalUi::new`, so the art loader reads them.
- `app/hud.rs` `skill_tree_tables` / `install_skill_tree_tables`, called in `app/play.rs` next to the HUD tables.
- The existing panel then draws icons at the row/column positions, level numbers, and a click with a free point sends C→S 0x3B (`AddSkillPoint`), which the server's `LearnRest` handler already serves.
- Test (`ui/original_tests.rs`, `skill_tree_draws_icons_and_levels_and_spends_a_point`): icons at the row positions, the level number, a 0x3B on click, nothing for a skill below its required level.

## PROVISIONAL: REC-270 (see `docs/HANDOFF.md` §7).

## What's left
Hover description, tab tool tips, free-points box and the no-points message (`panels-2.md` §19). The shown level is simplified (no full `skill_level` formula); the point cost is 1.

## The user's local check
`cargo run -p d2-client --release -- play --new amazon Test`, press **T** (skill tree): the class icons appear in their grid on each tab, a learned skill shows its level number; level up (or use a character with free points) and click an icon: the number goes up by one and the free points drop.
