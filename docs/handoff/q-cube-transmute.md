# q-cube-transmute: transmute animation and open latch

Branch `claude/q-cube-transmute`. d2rs-own, unverified (rule 10); REC-267 (HANDOFF §7).

| # | Gap (q-cube-gaps "left") | Link made |
|---|---|---|
| 1 | `StashCubeInput::cube_opened` had no non-test caller | S→C 0x77 0x15 (`msg_ui.rs`) sets `Shared::cube_opened`; `CubeUi::event` takes it and calls `cube_opened()` before the press/release, so every open/close sends one 0x17 |
| 2 | Transmute animation (§12.4) not drawn | `CubeUi::draw` steps `HoradricAnim` on the frame tick (40 ms/frame), draws `menu\horadric` frame n at `horadric_pos`, hides the grid while n < 14; started at the transmute release, cleared on close/open |

## Tests (synthetic, `ui/cube_ui_tests.rs`)

- `opening_and_closing_the_cube_twice_sends_0x17_each_time` (fails without the latch call).
- `the_transmute_animation_plays_frames_0_to_29_on_the_70ms_steps`.

## PROVISIONAL / left (REC-267)

- Animation start trigger (caller of `0x0048A540` unknown), 40 ms frame clock, draw mode 3 not in `ImageRequest`; the cel is in the panel files list as `menu\horadric`, but the play host must load it from `d2data` (not checked here).
- The grid-hidden rule (n < 14) is wired but not asserted by a test.
- Cube-gone close does not clear the animation (the panel stops drawing anyway).

## The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-cube-transmute; git checkout claude/q-cube-transmute
cargo run -p d2-client --release -- play --new sorceress Test
```

Open the cube (right-click it in the inventory), close, reopen, close: the server sees 0x17 each time. Press Transmute: a 30-frame animation plays at screen centre (about 2.4 s) if the sink loads `menu\horadric`.
