# q-menu-loading: loading and act-change screen

Branch `claude/q-menu-loading`. File `crates/d2-client/src/ui/front_end/screens/loading.rs` (spec `ui/frontend-loading.md` L3–L10). Plain Rust, no Bevy.

## Connected
- `LoadingScreen` state machine fed with `LoadEvent` (GameStart, S→C 0x01/0x03/0x04/0x05/0x61): one loading draw per GameStart / 0x03, frame counter clamped to 9 (L4, L5), no timer.
- End: 0x04 plus a placed player → one `Black` frame, then `World{act}` (L7); a 0x05 stops game frames (L9).
- `placement(w,h)` = (W/2−128, H/2+128) (800×600 → 272,428; 640×480 → 192,368); `art_path(lang)` (L3); `video_name(id)` + `videos` log for 0x61 ids 2–5 (stub, L10).
- Input ignored while loading (`wants_input()`), the registered screen has no controls.
- Tests: `cargo test -p d2-client --test front_end loading` (game start: frame 0, 1, black, world; act change: frame 0, black, world; clamp, placement, path).

## PROVISIONAL (REC-183, d2rs-own, unverified)
Keys ignored while loading (spec REC-222); no loading redraw after an act video (REC-223); 0x04 timing (REC-221). Nothing here is checked against the original.

## Left
Hook into the host: feed `LoadEvent`s from the bridge (0x03/0x04/0x05/0x61 and game start), draw `Presented::Loading` as `loadingscreen.dc6` frame at `placement` with the Loading palette (needs game files), the act palette at the world frame.

## Local check
`cargo test -p d2-client --test front_end loading` — 3 pass. No visible change in `play` yet.

Local check done 2026-10-08 (PC 1 round 2, branch claude/local-pc1-s8): headless part passes (the whole `front_end` binary, 12 pass).
