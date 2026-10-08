# q-fe-act-videos: act videos in the loading flow

Branch `claude/q-fe-act-videos`. PROVISIONAL: REC-251 (HANDOFF §7).

## Links connected

- `ClientWorld::session_log` / `session_total` (`bridge/world.rs`, `SessionMark`): 0x03 (`load_act`), 0x04, 0x05 (`msg/session.rs`) and 0x61 (`msg/ui_text.rs` `act_video`) append a mark in arrival order. The model's edges alone lost a repeated same-act 0x03 and a 0x05/0x04 pair inside one frame.
- `LoadFeed::observe` (`app/front_start.rs`) turns new marks into `LoadEvent`s in order (S05, S03, S61, S04); with no marks it keeps the old edge detection (state set directly).
- `LoadingScreen` (`screens/loading.rs`): a table video (ids 2–5) records the id (stub) and presents black, no loading redraw (L10 r2, REC-223). The overlay then shows black until the L7 frames.

## Tests

`cargo test -p d2-client --test front_start`: `act_change_sequence_drives_the_states_in_spec_order` (join, then 0x05/0x03/0x61/0x04: loading frame 0, black, black, world of the new act), `repeated_same_act_load_redraws_loading`.

## Left

- Real video playback (`StubVideo` only records the id); 0x04 timing after a warp (REC-221) and the post-video screen (REC-223) are unverified.

## Local check

```
D2_GAME_DIR=... cargo run -p d2-client --release -- play
```
Walk to a waypoint and take it to another act: the loading art shows (frame 0), then black, then the new act's world. First Warriv travel: black (no video yet) instead of the loading art after the load.
