# q-save-swap: swap set, switch state, golem re-summon, hireling block, progression

Branch `claude/q-save-swap`. d2rs-own, unverified (rule 10); PROVISIONAL points: REC-265.

## Links connected

| # | Link | Where |
|---|---|---|
| 1 | Swap pair and switch bit in the live list; C→S 0x60 trades the pairs | `d2-sim` `SkillList::switch_weapons`; `WorldHost::weapon_switched` (`items/moves.rs` `swap_weapons`, `world/wired.rs`) |
| 2 | Save: header +0x80 / +0x84 / +0x10 from the list | `save_gaps::read_gaps` / `swap_slots` / `apply_gaps` |
| 3 | Load: swap pair and switch bit onto the list | `save_gaps::select_swap_skills` (in `join_gaps`) |
| 4 | Iron Golem re-summon on join (golem item + skill 90) | `b3_lvl24::golem_summon`, `skill_events::golem_resummon`, `Pending::golem_resummon`, `ActionSim::golem_resummon`, `save_gaps::join_golem`; item bytes kept in `AppRest::golem_items` |
| 5 | Live hireling block | `WiredWorld::hireling_block`, `apply_gaps` |
| 6 | Progression bits | `AppRest::save_flags` (`QuestRest::client_save_flags`), seeded at the join, never lowered in `apply_gaps` |
| 7 | Hotkeys | pass through as loaded (no live source), REC-265 6 |

## Tests

- `d2-client` `tests/app_save_gaps.rs` (8): every field set and read back from a written file; progression never lowered; weapon switch trades the pairs.
- `d2-sim` `golem_summon_without_an_item_unit`: the golem is spawned and added as a pet with no item moves.
- Not covered: the join-time re-summon on a running game (needs the user's tables and a saved golem item), and the 0x60 handler on the wired host.

## What is left

The golem item as a unit worn by the golem (REC-265 3); live hotkeys and `resolve_item_indices`; the skills' behavior on a switch from a trace.

## The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
cargo run -p d2-client --release -- play --new necromancer Test
```
Press W (switch weapons), pick different mouse skills, close the window; `play --save "%USERPROFILE%\Documents\d2rs\saves\Test.d2s"`: the switch state and both skill pairs are as left. With an Iron Golem item saved and skill 90 learned, the golem stands next to the character after the load (log: no `join: save load: golem` line). A hired mercenary shows in the saved header block.
