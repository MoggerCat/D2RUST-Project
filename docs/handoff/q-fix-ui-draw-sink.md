# q-fix-ui-draw-sink: UI rectangles, blend modes, remaps; play at 640 × 480

Branch `claude/q-fix-ui-draw-sink` (from `claude/q-ui-audit`, finding
"UI draw sink" and the 640 × 480 note of `q-ui-audit.md`). Only the draw
sink and the resolution changed: no UI module emits a rectangle, mode or
remap yet (the callers switch in their own rows, list below).

## What the sink supports now

| Request | Spec | Where |
|---|---|---|
| `ImageRequest.look: CelLook { mode, remap }` (`CelLook::PLAIN` = mode 5, no remap) | `ui/panels.md` §1.4, §1.6; `render/blend-modes.md` §1, §2 | `ui/draw.rs`; `world_view/panel_art.rs::panel_sprite`, `world_view/ui_bind.rs::{ui_cel_ops, ui_remap}` |
| `Remap::Palette(k)`: 0 none, 1–12 PL2 text-colour map `k`, −18 … −48 light maps 31 … 1; −1, −2 … −17 and the rest are errors (no map in d2rs) | `ui/text.md` §4.3–§4.5 | `ui_remap` |
| `Remap::ItemColor { t, c }` (map `c` of item palette file `t`, `rules::shading::item_color`), the 8 `items\palette\*.dat` files loaded once | `render/shading.md` §6 r4 | `PanelArtLoader::push_item_palettes`, `ViewAssets::item_palettes` |
| `UiDraw::Rect(RectRequest { x0, y0, x1, y1, color, mode })` = `D2GFX_DrawRectangle`; `RectRequest::sized` = the UI primitive `0x0046EFD0(x, y, w, h, color, mode)` | `render/blend-modes.md` §8 r2; `ui/inventory.md` §2 r2–r3; `ui/control-panel.md` §5 r4; `ui/text.md` §8 r4 | `ui_bind.rs::{ensure_rects, rect_sprite}` over `rules::blend::{gdi_rectangle_box, gdi_rectangle_ops}` (split out of `gdi_rectangle`, same behaviour) |
| text drawn with a mode (§9 "draw with mode"): glyph remap `k`, then `T` of the mode | `ui/text.md` §4.3, §9 | `OriginalTextHooks.shades` |

A blended mode (0–4, 6, 7) without the act's tables (`ViewAssets.shades`)
is an error naming them (M07), never an opaque fallback.

## 640 × 480

`d2-client play --res 640x480` (default 800x600; decision in
`docs/PLAN.md`). `FrameSize::set_play` once per process; `FrameSize::play()`
replaces `FrameSize::D2RS` on the play path (camera feed, present,
click view, automap, ground items, missiles, corpse click, visibility,
weather), `world_view::play_view()` replaces `VIEW` for the composed
frame, `Presentation::for_frame` maps the window, `Screen::play()` is the
original UI's screen, the window opens at 640 × 480.

## Tests

| Test | Data | State |
|---|---|---|
| `world_view/ui_draw_sink_tests.rs` (13 tests: rect blend kinds 0/1/2, clamp, reversed edges, the 29 × 29 primitive, `k` remaps, light-map `k`, mode 3 cel, other modes as 5, remap-then-table, text with a mode, framed box under text, item colours, the play frame default and refusals, the 640 window mapping) | synthetic asymmetric tables | pass |
| `tests/game_ui_draw_sink.rs` (ignored, 4 tests): act 1 rectangles 31 / 190 / 29 / 207 (the spec's measured vectors) and ADD row 0; the real `SoSkillicon` with `k` = 5 equals PL2 text map 5; the real `horadric.dc6` frame 1 in mode 3 equals ADD[d][s]; `invgrey.dat` map 3 | **real 1.14d install** (private repo, cloud, 2026-10-08) | pass |
| `tests/app_play_640.rs`: the play app headless at 640 × 480: 640 × 480 cycle, camera `ViewRect::new(LOW, mode)`, character panel quads at (0, 256), (256, 256), (0, 432), (256, 432), camera shift ±160 | synthetic play fixtures | pass |

## Callers still to switch (their rows; not done here)

- `ui/inventory.md` §2 / §4 / §6 r5 placement tints and §8 r4 ethereal
  (mode 1) / item colour: `q-fix-ui-grid-hover` (+ item art).
- `ui/control-panel.md` §5 r4 / r5 belt boxes, §4 r2, §5 r14, §7 r2.
- `ui/panels.md` §10 r3 skill icon `k`, §12 r4 transmute mode 3.
- `ui/text.md` §8 framed text (hover boxes).
- `ui/frontend-options.md` §O4 r1–r2 disabled rows, slider rectangles.

## Findings for other rows

- Several UI modules still place themselves with the 800 × 600 constants
  `ui::FRAME_W` / `FRAME_H` (`ui/hud.rs`, `hud_tips.rs`, `gold_dialog.rs`,
  `npc_menu_ui.rs`, `esc_menu.rs`, `esc_art.rs`, `item_tip.rs`,
  `imbue_ui.rs`, `hire_list.rs`, `game_messages.rs`, `overhead_ui.rs`,
  `controls_host.rs`, `widget.rs`): at 640 × 480 they need the `Screen`
  they are given (UI-module rows; the control panel already follows the
  screen in `app_play_640`).
- `Settings::window_size` sizes the window by the Resolution row while the
  frame stays 800 × 600 (§O8); with Resolution = 640 × 480 and the
  default frame the window is smaller than the frame and the cursor
  mapping fails (`FrameError::TooSmall`). Pre-existing; not changed.
