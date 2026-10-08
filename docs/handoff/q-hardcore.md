# q-hardcore: hardcore flag, permanent death, save at death (`claude/q-hardcore`)

> Stitching session, 2026-10-08. Synthetic fixtures only; nothing verified against 1.14d (rule 10). Provisional: REC-120. Sound not wired.

## 1. The path, and what was missing

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Hardcore flag of the character | `LiveData::read_save` passed `hardcore: false`, so a hardcore `.d2s` was refused (check 11) | `hardcore::save_is_hardcore` reads status bit 0x4; `play --new NAME --hardcore` makes a new one; `PlayConfig.hardcore` |
| 2 | Client flag 4 on the server | `Pending::client_hardcore` default softcore | `LocalSeams.hardcore` answers it; `play::run` sets it after the start |
| 3 | C→S 0x41 on a hardcore death | handler existed (drop reason 3) but the flag was never on; `drop_client` did nothing | `LocalSeams::drop_client` records `(player, reason)`; the app closes the game on Esc (`hardcore::leave_dead`) |
| 4 | Save at death | only on window close | `hardcore::save_on_death` saves when the death screen comes up: the gold / experience loss (penalties §4.6, per difficulty from `difficultylevels` `DeathExpPenalty`, already in the sim) is on disk whichever way the window is closed |
| 5 | Permanent death | no dead bit written | `Live.hardcore_dead` (hardcore and player mode 0 / 17) → `mark_dead` sets status 0x8; `d2s::read` already refuses a dead hardcore save (error 10) |

Penalty rules (gold, experience by difficulty) and the corpse experience already ran from q-death; they are unchanged.

## 2. Tests

- `tests/app_play_hardcore.rs`: hardcore death → Esc drops the client (reason 3), the game closes, the saved status has 0x4 | 0x8; softcore → no drop, respawn, no dead bit.
- `hardcore.rs` unit tests: the status bit read, `mark_dead`.

## 3. What is left

1. Corpse retrieval: the client does not send the corpse pickup (click on a mode-17 player unit); items do not move to the corpse (the inventory model has no corpse grid); no gold piles at the death spot (`death_drop_gold` default); no ear drop.
2. The save is written at the DT start, not the DD start; a character screen to return to (the game just closes).
3. The death penalties on real difficulty need a play past Normal (`GAME_SETUP.difficulty` is Normal).

## 4. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-hardcore; git checkout claude/q-hardcore
cargo test -p d2-client --test app_play_hardcore
cargo run -p d2-client --release -- play --new barbarian HcTest --hardcore
```

In `play`, a hardcore character that dies shows the death screen; Esc closes the game; `HcTest.d2s` is then marked dead and `play --save HcTest.d2s` refuses it ("dead hardcore character"). A `--new` without `--hardcore` respawns in town.
