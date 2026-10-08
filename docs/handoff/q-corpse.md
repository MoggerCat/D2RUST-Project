# q-corpse: corpse and death losses in the play preview (`claude/q-corpse`)

> Stitching session, 2026-10-08. Synthetic fixtures only; nothing verified against 1.14d (rule 10). Provisional: REC-141. Sound not wired. Starts from `q-death.md` and `q-hardcore.md`.

## 1. The path, and what was missing

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Items to the corpse at DD (`vitals.md` §4.7 r1.7) | the corpse unit existed with no inventory; the player kept everything | `ActionHooks::allocate_corpse` queues `(player, corpse)` in `DeathState.loot`; `WiredWorld::corpse_fill` (run from `after_tick`, `d2-server/.../world/wired.rs`) gives the corpse an inventory and calls `items::moves::ground::corpse_fill`: the cursor item into the corpse grid, body items onto the same locations; grid and belt stay |
| 2 | Gold lost at the death | `death_drop_gold` was a no-op default | the penalty (`death_penalties`) queues `DeathState.gold_drops`; the same host call makes the piles (`gold_piles`) near the death spot |
| 3 | Corpse click | the client never sent the pick-up for a dead player unit | `world_view/corpse_click.rs`: a left press on a mode-17 player unit (not the local player) sends C→S 0x16 type 0, walks, asks again (as the ground items) |
| 4 | Server take-back | the sim had `corpse_take_back` (§12.2); `corpse_pickup` refused a corpse in a bare fixture (no state row) and `corpse_taken` did nothing | a wiring-allocated corpse counts as state 7; `corpse_taken` sends 0x8E + 0x0A, drops the corpse's room entry, inventory and unit |
| 5 | Client item view | the player's items stayed on screen after they moved | S→C 0x0A per item that leaves the player (REC-141); the take-back re-adds them with the usual 0x9C |
| 6 | Save at DD | only at DT (q-hardcore) | `hardcore.rs` `save_at_dd` also saves when the local unit reaches mode 17; `save_full` writes the corpse's items in the d2s corpse section (`Extra.corpses`) |

## 2. Tests

- `d2-server` `items/moves/tests.rs` `die_respawn_click_the_corpse_items_come_back`: the e2e. A cap on the head, a key in the grid and a sword on the cursor; 1000 gold; DT (penalty: gold 0, 990 in piles), DD (corpse holds cap and sword, key stays); the respawn; 0x16 type 0 on the corpse; cap back on the head, sword and key in the grid, corpse unit freed. Runs on the item-move fixture (the synthetic play game has no inventory model, `GameParts.inventory` is `None`, so a client-level e2e cannot hold items).
- `d2-client` `world_view/corpse_click.rs` tests: the press sends the exact 13-byte 0x16 type 0, the walk and the second ask, the record ends with the corpse; a living player or the local body is no corpse.
- `tests/app_save.rs`: `apply_extra` writes the corpse section.

## 3. What is left

1. Ear drop; the corpse's own gold piles (rule 1.8); an item the corpse cannot take is not dropped near the player.
2. Loading a save with a corpse section (`create_corpse` stays unapplied): after a restart the corpse is gone and its items are lost.
3. Softcore status bit 0x8 on a death (`d2s.md` §8.3 r6 note).
4. Other players' clients are not told of the corpse removal; party pick-up (`corpse_loot_allowed`).
5. The 0x41 respawn leaves the corpse where it died; in `play` nothing kills the player yet (`q-death.md` §3 item 1), so by hand the check needs a `--hardcore`-less debug kill.

## 4. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-corpse; git checkout claude/q-corpse
cargo test -p d2-server --lib die_respawn_click
cargo test -p d2-client --lib corpse_click
cargo test -p d2-client --test app_save
```

All pass. In `play` the click path is reached only after a death (item 5 of §3).
