# q-menu-cinematics: the cinematics menu

Branch `claude/q-menu-cinematics`. File: `ui/front_end/screens/cinematics.rs` (spec `frontend-credits.md` C5, C6, C8). PROVISIONAL points: REC-186 (HANDOFF §7).

## Connected
- 7 (expansion) / 5 (classic) entry buttons + CANCEL (Esc hotkey), coordinates and string ids from C8.
- Enabled entries from N via `level(N)` (C6 r2): expansion 1…L, classic 1…min(L,5); disabled expansion labels empty. Missing N: level 1, default 0x22 written.
- Click → `VideoHook::play(DATA\LOCAL\video\ENG\<name>640x292.bik)`; stays on the menu; N unchanged (C8 r7).
- `note_video_request(store, id)`: the in-game N writer (C6 r3).
- CANCEL / Esc → main menu via the existing flow table.

## Left
- Host: pass the settings-file store and real video into `register_with`; perform the input flushes (screen counts them).
- Nothing calls `note_video_request` yet (no in-game video path).

## Local check
```
cargo test -p d2-client --test front_end_cinematics --test front_end
```
All pass (6 + 9). No visible change in `play` (front-end host not written).
