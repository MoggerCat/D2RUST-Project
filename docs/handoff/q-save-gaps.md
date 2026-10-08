# q-save-gaps: mouse skills, act byte, merc and golem items, runeword refresh

Branch `claude/q-save-gaps`. d2rs-own, unverified (rule 10); PROVISIONAL points: REC-241.

## Path traced and links connected

| # | Link | Where |
|---|---|---|
| 1 | Save: left / right mouse skill of the live skill list → header +0x78 / +0x7C, item index = 1-based position of the owner item in the inventory list (`d2s.md` §2.4 r1–r3) | `app/save_gaps.rs` `read_gaps` / `mouse_slots` / `apply_gaps`; `WiredWorld::item_guids` (`d2-server` `world/gap_items.rs`) |
| 2 | Load: header mouse pairs → the player's skill list (`left` / `right`), item index → GUID (§2.4 r4–r6) | `save_gaps::join_gaps` / `select_mouse`, called after `save_full::join_items` in the join |
| 3 | Swap pair (both weapon sets) | passes through as loaded (no sim state, REC-241 1) |
| 4 | Save: act / difficulty byte `towns[d] = act \| 0x80`, the others 0 (§2.1) | `apply_gaps` (act from `unit_act`, difficulty from `ai_info`) |
| 5 | Save: hireling items (`jf`, §8.4) from the living hireling unit | `WiredWorld::hireling_unit` + existing `save_items` |
| 6 | Load: hireling items onto the restored hireling | `join_hireling_items` runs the queued restore (`hireling_calls`), then `load_items` on the merc; `load_items` now makes a monster inventory for a non-player owner |
| 7 | Save: golem item (`kf`, §8.5) from the Iron Golem unit (pet type 3, class 0x123) | `WiredWorld::golem_unit` |
| 8 | Load: golem item | kept as loaded (re-summon not wired, REC-241 5) |
| 9 | Runeword refresh on load (§8.2 r5): stale runeword item freed (stored / cursor) or unequipped | `d2-sim` `InvDesk::load_entry` (`LoadFault::StaleRuneword`), `runeword_matches` shares its inputs with `socket_runeword` |

## Tests (synthetic)

- `d2-client` `tests/app_save_gaps.rs` (6): all four values written and read back field by field (mouse pair, swap pair kept, town byte, hireling list, golem item); no gaps keep the loaded values; golem without item; mouse encode / select with an item index, absent left, unknown item; hireling list needs a hireling block.
- `d2-sim` `wiring::inventory::tests::load` (2 new): a stale runeword item is freed on load; an item without the flag loads.
- Not covered (no wired-host fixture with a hireling and an inventory model on the cloud side): `join_hireling_items`, `golem_unit`, `hireling_unit` on a running game.

## What is left

Swap pair and weapon switch state; item-granted skill entries; golem re-summon; live hireling block; progression bits; hotkeys (`resolve_item_indices`).

## The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
cargo run -p d2-client --release -- play --new sorceress Test
```
Pick a different right skill, take the waypoint to act 2 (or use the Lut Gholein door), close the window. Then `play --save "%USERPROFILE%\Documents\d2rs\saves\Test.d2s"`: the right skill is as left, the character starts in the saved act, and the log has no `join: save load: hireling items` lines. With a hired mercenary carrying items (only if the hireling block was already in the loaded save): the items are on the merc after the reload. Report any `join: save load` line.
