# q-ui-audit

Rule-by-rule audit of the UI and input code against `specs/ui/*` and the
input / hit-test parts of `specs/client/ui.md`, `msg-ui.md`, `bridge.md`
(2026-10-08, cloud, no game files; M23 and the HANDOFF §8 lesson of
2026-10-08). Six read-only passes, one per spec area, then fixes of the
small plain-spec findings on this branch. World rendering (q-render-audit)
and system seams (q-seam-audit) are out of scope.

## Headline

- **Much of the spec-correct code is not what play runs.** The play app
  installs d2rs-own preview adapters (`npc_menu_ui`, `shop_ui`,
  `hire_list`, `imbue_ui`, `item_tip`, the dev key preset), while the
  spec-tested modules (`panels/menu_box.rs`, `panels/npc_menu.rs::build_npc_menu`,
  `panels/shop.rs::confirm_box`, `messages/dialog.rs`, `ui/cursor.rs`,
  `controls/original.rs` pointer / wheel / key modes, `widget.rs::CellGrid::cursor_cell`,
  `inv_grid::HoverState` / `placement_tint`, `inventory::equip_backgrounds`,
  `text::framed_text`) are reached only by their unit tests. A green
  unit test of those modules says nothing about play.
- **Play is 800×600 only** (`app/ui.rs`, `Screen::R800`): every 640×480
  rule is untested in play.
- **The UI draw sink has no rectangle, blend mode or colour remap**
  (`ui/draw.rs` `ImageRequest`, `world_view/panel_art.rs` draws every UI
  image `Opaque`): every translucent box, tint, disabled look, additive
  animation and remapped icon of the specs is drawn wrong or not at all.
- **UI-state close hooks are logged, not applied**
  (`world_view/present.rs:646`): a close that is not the panel's own
  button (Esc, a gate, a hotkey) sends no 0x4F 0x12 / 0x4F 0x17 / 0x49.

## Fixed on this branch (each with a test from the spec)

| # | Spec rule | Was | Fix |
|---|---|---|---|
| F1 | `panels-2.md` §14.9 (`0x004B3C20`: 30 [1 u32][GUID]) | shop close sent `30 00 …` (`TerminateEntityChat` encodes @1 = 0) | `shop_ui.rs`, `panels/shop.rs` send `npc::msg_chat_end`; the expectation of `shop_close_sends_0x30` changed from byte 1 = 0 to 1, following §14.9 |
| F2 | `panels.md` §2 r9; `frontend-options.md` §O1 r2–r3 | Esc closed a d2rs list (included 6, 7, 8, 0x11; missed 5, 0x0B, 0x0D, 0x12, 0x17–0x21); the menu's open closed nothing and its close restored nothing | `original.rs` `ESC_CLOSABLE` (flag-1 set, id order), `open_game_menu` (closes all but 0 / 9, remembers keep = 6, 7, 10, 17, 21, 35), `restore_game_menu_states` on Esc and Return to Game; the mini-panel menu button (`SetUi(9, on)`) goes through `open_game_menu` too (no own test) |
| F3 | `panels-2.md` §18 r1 | inventory close button never cleared its pressed flag on release | `panels/inventory.rs` `release` clears it first |
| F4 | `panels-2.md` §18 r2 | right-panel area bottom exclusive | `InvArea::rect` bottom inclusive |
| F5 | `panels.md` §8 r7 | level (stat 12) never compare-coloured | `CMP_STATS` gains 12 |
| F6 | `menus.md` §1 r3 | waypoint tab hit used the classic 4-tab geometry in an expansion game | `WpEnv::new(.., env().exp)`; test: (300, 80) → tab 3 |
| F7 | `panels.md` §13 r6 | waypoint title never drawn (`NoMeasure`) | the bound fonts measure it (no own test: needs font tables; the panel's title math is tested with a fake measure) |
| F8 | `panels.md` §13 r5 | pressed-row colour 3 even with the close button pressed | `(sel && !close_pressed) || current` |
| F9 | `item-tips.md` §3.3 r4 | "Required Level: 1" shown | level line only when > 1 |
| F10 | `item-tips.md` §7.2 f15 / f24 | skill and level swapped in "chance to cast" / charges lines | skill = layer >> 6, level = layer & 0x3F; the test inputs of `each_descfunc_gives_its_line` changed to the spec layout (same expected text) |
| F11 | `panels.md` §12 r2, `panels-2.md` §20 r4 | the cube panel closed (2× 0x4F 0x17) when the cube item left the model, never on death | closes on no player / mode 0x11 only (`cube_player_ok`); `cube_ui_tests` expectation changed to the spec (a missing cube keeps it open) |
| F12 | `menus.md` §2 r3 | Resurrect row captioned "Identify Items: n" | `hireresurrect2` (22696) with the cost; the merc name id `[0x00725494]` is not in the client model, so `%s` is empty until q-fix-ui-npc-menu |
| F13 | `client/ui.md` §A6 (location) | play read `<save parent>/controls.toml`, Configure Controls wrote `<config_dir>/d2rs/controls.toml` | play loads from the Configure Controls folder (`config::controls_dir`); fallback the old folder when the platform has no config dir. A local `~/Documents/d2rs/controls.toml` is no longer read when a config dir exists |
| F14 | `control-panel.md` §10 r1–r2 | a press in the world released over the belt picked a potion | the belt click needs the recorded press; flags cleared on every release |
| F15 | `controls.md` §3 row 44 | W swapped weapons with the shop, trade or stash open | `swap_key::swap_allowed` gate |
| F16 | `frontend-options.md` §O2 r3 | Party Names always enabled | enabled only while Show Party is on |

Gate on the branch: `cargo fmt --check`, `cargo clippy -p d2-client --all-targets -- -D warnings`, `cargo nextest run -p d2-client` (2205 run, all pass), `py tools/coverage.py --check`, `py tools/spec_index.py --check`. None of these ran on the real install (cloud, no game files fetched): M23 real-data checks are queued below.

## Fixed on `claude/q-fix-ui-input` (follow-up session)

| Build-queue row | What changed | Play-path test |
|---|---|---|
| q-fix-ui-grid-hover | `ItemsUi.hover` keeps the §5 hover state on every mouse event over an open inventory / stash / cube grid; an overhanging footprint keeps the last cell; the grid click places at it (`cursor_cell_for` gone). Placement tint (§4) not drawn: draw-sink work | `original_tests::grid_hover` (spec vector; stash last column) |
| q-fix-ui-input-pass | input and `world_clicks` every loop pass (draw stays per tick); input reset `0x0044DA40` applied (`take_input_reset`, `ClickState::input_reset`); release outside the frame; focus-loss release | `tests/app_input_pass.rs` (3 tests, each red on the old code) |
| q-fix-ui-close-hooks | `OriginalUi::set_ui` runs the close hook of every state it closes (stash 0x4F 0x12 in mode 0x0C / 0x0D, cube 0x4F 0x17, waypoint 0x49 level 0); a panel's own close keeps sending its own | `original_tests::close_hooks` (Esc) |
| q-fix-ui-keys | `Preset::Original` from `COMMANDS` (§B4 r1 checked against `key-commands.tsv`), play default; 17 new actions; handlers M, O, Space, F9–F12, V, NumPad 0–7, Run / Stand Still / Show Items held, middle / X buttons, wheel | `controls::tests`, `edge` tests, `original_tests::key_commands`, `app_input_pass::the_play_keys_are_the_original_preset` |

Still open from these rows: skill up / down (`0x004AA740`), clear text
(`0x004A01E0`) and the F1–F8 skill hot keys (`0x004AA030`) have no spec'd
callee in the client; the Run down handler's walk → run switch (C→S
0x53 / 0x54); the key modes of §4.1 r4–r5; the short / long key names
(`control-panel.md` §5 r4, r13); the `client/ui.md` §A6 naming conflict
(spec task). None of this ran on the real install (cloud, no game files).

## Findings, most visible first

Size: S = few lines, value in the spec; M = one module; L = needs a spec,
a draw-sink feature or a new wiring. "→ q-fix-ui-*" names the build-queue
row that carries it.

### 1. Controls and input

| Spec rule | Code | Player sees | Size |
|---|---|---|---|
| `controls.md` §B4, `client/ui.md` §A6 (`"original"` preset) | `app/config.rs:301-310`, `app/ui.rs:149` default to `Preset::Dev`; `controls/mod.rs:52` refuses `Original` | A, B, M, N, V, O, numpad, F9–F12, Ctrl run, middle-button automap, wheel skill cycle all missing or wrong | L → q-fix-ui-keys |
| `controls.md` §3 commands | `ui/original.rs` `hotkey_state` handles I, C, T, Q only; S, F1–F8, Space, Enter, H, `, Z, Alt, PrintScreen do nothing | most keys dead | L → q-fix-ui-keys |
| `controls.md` §4.2 r1–r3, §7 r5 | `present.rs:561-571`: middle / X buttons / wheel never reach a command; `controls::original::{pointer_meaning, WheelAccumulator, key_mode_for, …}` test-only | no wheel skill cycle, no middle-button automap | M → q-fix-ui-keys |
| `controls.md` §6 r6, r2 (per client loop pass) | `present.rs:612` runs `world_clicks` once per server tick | held repeats at tick rate; press+release within one tick loses the walk end; a 2nd click within 40 ms dropped | L → q-fix-ui-input-pass |
| `msg-ui.md` §2 r2.2, §8 r4 (input reset `0x0044DA40`); `controls.md` §4.3 r3 (focus loss) | reset only logged (`msg_ui.rs:405`); `present.rs:549` drops the release outside the frame; no focus-loss release | releasing the button in a black bar / outside the window / on alt-tab leaves the character walking | M → q-fix-ui-input-pass |
| `client/ui.md` §A6 (file location) | play reads `<save parent>/controls.toml` (`app/play.rs:403`), Configure Controls writes `config_path()` (`screens/controls.rs:745`) | rebinds lost on restart | **fixed F13** |
| `client/ui.md` §A6 (no silent defaults) | `app/ui.rs:148` `saved_bindings().or_else(dev)`, parse error dropped | bad file silently ignored | S |
| `controls.md` §3 cmd 44 | `world_view/swap_key.rs` sends 0x60 with ui 0x0C / 0x17 / 0x19 open | W swaps weapons in the shop / stash | **fixed F15** |
| `controls.md` §7 r2–r3 | `bridge/belt.rs:46` `shift: false`, `ui9_open: false`, no dead gate | Shift+1 never feeds the merc; 1–4 drink with the Esc menu open | S/M |
| `controls.md` §4.1 r4–r5 (key modes) | none | I toggles the inventory in a shop / chat | L → q-fix-ui-keys |
| VK 0x10–0x12 cover both sides (§3, §4.3 r2) | `vk_to_key` maps to left keys only | Right Shift does not stand still | S |
| `control-panel.md` §5 r4, r13 key names | `Key::name()` (`Numpad1`, `LeftCtrl`) | belt labels / Controls screen names differ | S |

### 2. Cursor and drawing order

| Spec rule | Code | Player sees | Size |
|---|---|---|---|
| `client/ui.md` §B6, `panels-3.md` §23 | `ui/cursor.rs` never instantiated; OS cursor not hidden | system arrow, no gauntlet / animations | L → q-fix-ui-cursor |
| `panels.md` §5 step 10, `panels-3.md` §23 r9 | `BorderUi` draws the cursor item and tooltips, but `HudUi`, gold dialog, messages, Esc menu are installed after it (`original.rs:428-444`) | held item and tooltip covered by globes / belt / buttons | S/M → q-fix-ui-cursor |
| `panels.md` §5 order | Esc menu installed last (step 1), quest log after the inventory family, NPC menu before HudUi | minor | S |

### 3. Inventory, stash, cube grids (the stash misclick)

| Spec rule | Code | Player sees | Size |
|---|---|---|---|
| `ui/inventory.md` §5 r3 (cursor cell kept when `w + c > gridX`), §10 r4 | running path uses `inv_items.rs:485-517 cursor_cell_for` (computed at click, `None` → no send); spec `CellGrid::cursor_cell` / `HoverState` unused | a 2-wide item clicked over the right half of the last column (or lower half of the last row) is not placed — **likely the stash misclick** | M → q-fix-ui-grid-hover |
| `ui/inventory.md` §10 r4.2 drop cell `0x00486BD0` | no spec: the code uses the cursor cell (d2rs-own) | the remaining unverified part of the misclick | spec task |
| §10 r4.3 (0x21 stack, 0x29 scroll → tome) | `inv_items.rs:384-391` `stackable_onto: false`, `book_kind: None`; `:476` drops `Stack` / `ScrollBook` / `ToCube` | scroll onto tome, arrows onto quiver swap instead of merging | S/M → q-fix-ui-grid-msgs |
| §10 r4.4 (0x2A to cube) | `inv_items.rs:450` `cube_has_room: false` | dropping an item on the cube does nothing | S/M → q-fix-ui-grid-msgs |
| `panels-3.md` §29 r1.1 / r2 (identify on equipped) | `equip_press` ignores the identify state | identify scroll on worn item unequips it | S |
| `panels-3.md` §29 r1 (box both ends inclusive) | `inv_items.rs:521-529`, `inv_items_tip.rs:63` exclusive | 1-px dead edge on equipment boxes | S |
| `panels-3.md` §29 r3 (e codes 0–7) | only 0x1A / 0x1D / 0x1C | empty hand opposite a two-hander dead; wrong swap messages | L |
| §10 r3.3 (Ctrl) | `ctrl: false, store_open: false` | Ctrl-click lifts; no Ctrl-sell | S |
| §4 r1–r3, §6 r5 (placement tint) | none drawn | no green / red / yellow tint while carrying | M (after grid hover) |
| §3, §5 r1 (stash / cube hover, tips) | `stash_items.rs`, `cube_items.rs` draw only; tip gated on flag 1 (`original.rs:1340`) | no tooltips with stash or cube open | M |
| §8 r2 (inventory file per quality / variant) | base `invfile` always | uniques, sets, rings, amulets, charms, jewels show the base picture | M/L |
| §8 r4 (measured frame height) | `set_item_frame_sizes` never called (`app/items.rs`) | items drawn up to a few px low | S |
| `panels.md` §9 r8 → `panels-2.md` §18 r2 (bottom inclusive) | `original.rs:149` `InvArea::rect` `h = bottom − top` | bottom row of the right panel falls to the world | **fixed F4** |
| `panels.md` §12 r2 (missing cube does not close) | `cube_ui.rs:198` closes and sends 2× 0x4F 0x17 when the cube is absent; never on death | cube closes when the cube item moves | **fixed F11** |
| `panels-2.md` §20 r7 | transmute animation on the button release | animation without a server answer | M |
| `panels.md` §11 r7, `panels-2.md` §20 r2–r3 | `stash_ui.rs:76`, `cube_ui.rs:149` `in_inv_close: false`; click reaches InventoryUi → `SetUi(1, off)` | inventory close button leaves stash / cube open and shifts the camera | M → q-fix-ui-stash-close |
| `panels.md` §2 r6 (close hooks) | `present.rs:646` only logs `UiEffect`s | Esc / gate closes send no 0x4F 0x12, 0x4F 0x17, 0x49 | L → q-fix-ui-close-hooks |
| `panels-2.md` §21 r4, §11 r4, §20 (sounds 4, 0xDD) | everything maps to `ClickSound` = id 0 | wrong UI sounds (also menus.md §1.2/§1.6, control-panel §9 r7, §10 r1) | S/M → q-fix-ui-sounds |

### 4. NPC menu, shop, hire, dialogs

| Spec rule | Code | Player sees | Size |
|---|---|---|---|
| `menus.md` §2.2–§2.6 | `npc_menu_ui.rs` fixed 200-wide list at (W−200)/2, H/4, 20-px rows, no frame / name / selected colour; spec `build_npc_menu` + `MenuBox` unused | bare text list mid-screen, not the framed box above the NPC | L → q-fix-ui-npc-menu (needs the menu-box handler spec) |
| `menus.md` §4.2, §4.4–§4.5 | `shop_ui.rs:330` quick always; Left = Right; confirm dropped; repair auto-confirmed | buys instantly on a misclick, no confirm box | L → q-fix-ui-shop |
| `panels.md` §4.1, §5 step 5, `panels-2.md` §14 r13 | shop is a left panel + ui 1 (REC-278) | wrong open mode / borders; closing the inventory leaves half a shop | L → q-fix-ui-shop |
| `panels-2.md` §14 r13 (NPC `inventory.bin` store grid) | `shop_ui.rs:137-212` 10×10 invented grid, re-packed items | store items in other cells | L → q-fix-ui-shop |
| `panels-2.md` §14.9, `messages.md` §6–§7 | Talk prints kind-0 strings as plain text; dialog panel skipped | no gossip choices, no scrolling speech | L → q-fix-ui-npc-talk |
| `menus.md` §2.3 (`hireresurrect2` 22696) | `npc_menu_ui.rs:274` captions every cost row with 4021 | Resurrect row reads "Identify Items: n" | **fixed F12** (name gap) |
| `menus.md` §3.2 (Back rebuilds the NPC menu) | `hire_list.rs:241` clears only | Back leaves nothing up, chat not ended | S |
| `menus.md` §3.4 (confirm when a merc exists) | `hire_list.rs:253` `no_hireling: true` | current merc replaced without warning | M |
| `menus.md` §2.2 (C→S 0x38 action 3 on open) | not sent by `open_npc_menu_with` | — (trace differs) | S |
| `panels-2.md` §14.10 (S→C 0x2A result) | no UI consumer | menu vanishes after Identify / Resurrect / Hire | M |
| `panels-2.md` §14.7 (record 0 fallback) | `npc_menu_ui.rs:151` no box | NPC without a record has no menu | S |
| `panels-2.md` §14.8–§14.9, `messages.md` §13 r4 (ui 8) | NPC menu is panel 0x103, not ui 8 | Esc opens the game menu over it; I / C / T open panels during chat | L → q-fix-ui-npc-menu |
| `menus.md` §4.2–§4.3 sell / repair / shift | sell price 0, repair durability 0, `a4: 0` | sell shows no price; repair message fields wrong | M → q-fix-ui-shop |
| `panels-2.md` §14 r13 button toggle; `menus.md` §4.5 repair-all on up | buttons never stay down; Close / repair-all on press | repair button pops while repair mode stays | S/M |
| `messages.md` §11 | `imbue_ui.rs` local preview | imbue dialog wrong art and flow | L |
| spec conflict: `panels-2.md` §14 r11 vs `menus.md` §4.2 (gamble flag) | — | — | spec task |

### 5. Waypoint, skill tree, character

| Spec rule | Code | Player sees | Size |
|---|---|---|---|
| `panels.md` §13 r5–r6 | `waypoint_ui.rs:76` empty names; `:160` draws with `NoMeasure` | no level names, no title | title **fixed F7**, names L → q-fix-ui-waypoint |
| `menus.md` §1.3, `panels.md` §13 r3 | `waypoint_ui.rs:184` `WpEnv::new(.., false)` (classic 4 tabs) | tab clicks land on the wrong act; Act V tab unreachable | **fixed F6** |
| `msg-ui.md` §2 r2.3 (tab = act, gated) | skipped (`msg_ui.rs:427`); `WaypointUi::sync` resets to tab 0 | menu opens on Act I in Acts II–V | S/M |
| `menus.md` §1.2 (outside click closes) | rect limited to W/2 (`waypoint_ui.rs:146`) | menu stays up while walking away | S/M |
| `panels.md` §13 r5 (pressed close button) | `sel || current` | row colour while close pressed | **fixed F8** |
| `panels.md` §7 r3 (`0x00486EF0` clears pressed) | `panels/inventory.rs:278` never clears | close button drawn pressed forever after one click | **fixed F3** |
| `panels.md` §8 r7 (stat 12 compare colour) | `character.rs:61` `CMP_STATS` lacks 12 | level never blue / red | **fixed F5** |
| `panels.md` §10 r3 (icon remap) | remap dropped | unlearnable skills not grey | L (draw sink) |
| `panels.md` §10 r2 (tab-1 band to H−49) | tree rect = inventory `inv` rect | presses at y 502–550 fall to the world | S/M |
| `panels.md` §9 r4 (empty slot pictures) | `equip_backgrounds` test-only | no empty-slot outlines | M |
| close / tab tool tips, free-points number (§8 r2, §11 r4, §10 r7 / r9) | implemented, not called | no button tips; no unspent-points number | M |

### 6. Control panel

| Spec rule | Code | Player sees | Size |
|---|---|---|---|
| `control-panel.md` §9 r5 (f 4 automap, f 6 quest log) | automap view follows `AutomapSession.open` only; QuestLog dropped (`hud.rs` ~555) | two mini-panel buttons do nothing | S |
| §10 r2 (no press recorded → no belt click) | `hud.rs:592` clicks on any release | press in the world, release on belt → potion picked | **fixed F14** |
| §5 r14, §1 (tips in a pop-up box at step 10) | plain text, no box, under the HUD | tips without a box | L (draw sink) |
| §6 r1 (run tip key names) | `[None, None]` | "Run" without "(R)" | S |
| §9 r6 (mini-panel tips) | `MiniPanel::tip` test-only | no tips | S |
| §5 r5 (cursor-item belt highlight) | `cursor_highlight` unused | no green box | S |
| §9 r2, r7 (belt blocks the right side, −118) | `hud.rs:520` passes `false, 1` | mini panel overlaps a popped belt | S |
| §4 r2 (`stambarblue`) | `hud.rs:410` `false` | stamina never blue | S |
| §7 r1–r2 (skill icon state colour) | plain image | town-unusable skills not dark | L |
| §3 r5 (Show HP/MP text stored) | `StoreRegistry` ignored | setting lost on restart | S |
| §5 r8 (belt short text) | full item tooltip; `hover_text` built wrong | belt hover shows the full tip | S |

### 7. Text, item tips, Esc / options

| Spec rule | Code | Player sees | Size |
|---|---|---|---|
| `item-tips.md` §2 (block order, labels by string id) | `item_tip.rs:285-420` d2rs-own order | Str above Dex, Quantity below Durability, no speed / damage / block / class lines | L → q-fix-ui-item-tips |
| `item-tips.md` §3.3 r4 (Required Level only when > 1) | `item_tip.rs:380` `> 0` | "Required Level: 1" shown | **fixed F9** |
| `item-tips.md` §7.2 f15 / f24 (skill = layer >> 6, level = layer & 0x3F) | `item_tip_desc.rs:135,147` swapped | every "chance to cast" / charges line names the wrong skill and level | **fixed F10** |
| §7.2 f11, f12, f13, f20, f21, f22, f23, f17 / 18 | shapes wrong or missing | stat lines read differently | S–M → q-fix-ui-item-tips |
| §4 (name colours), §3.3, §3.11 (red requirements, blue defense) | partial | wrong colours | S/M |
| §2 r4, `control-panel.md` §5 r14 (anchor at the item, box W = max + 8) | anchored at the mouse, opaque tiles | box follows the mouse, wrong size | L (draw sink) |
| §9 (set tip), §11 (store lines) | d2rs-own | set and price lines differ | M/L |
| `frontend-options.md` §O1 r6 (single-player pause) | none | world runs under the Esc menu | L → q-fix-ui-pause (bridge.md must state it) |
| §O8 (no Window Mode row) | `options_menu.rs:141` adds it | Video rows 22 px off | S |
| §O8 (settings applied) | only window size / mode | Light Quality, Shadows, Automap, NPC Speech do nothing | L |
| §O2 r3 (Party Names enabled when party shown) | always enabled | — | **fixed F16** |
| §O4 r1–r2 (disabled look, slider rectangles) | not drawn | disabled rows full bright | L (draw sink) |
| §O3, `frontend-menus.md` §F1.3 (exit to character select, REC-200) | `AppExit` | Save and Exit quits the program | M |
| §O4 r3 (pentagram clock 50 ms) | `(tick/2) % 8` | spin rate tied to ticks | S |
| `text.md` §7, §4 r5 (any colour k) | `ui_bind.rs:291` errors the frame for k ∉ 0..13 | a `ÿc!` code fails the whole UI frame | M |
| `text.md` §15 r6 (edit-box scroll) | `edit_box.rs:331,359` | not reached in play | S |

## Spec gaps found (spec tasks, local)

- NPC menu-box window handlers (0x0E, 1): hit edges, hover, outside click, keys (`menus.md` §2.1).
- Drop cell `0x00486BD0` (`ui/inventory.md` §10 r4.2) — the open half of the stash misclick.
- `0x004B1F80` repairable, `0x00489870` repair-all on, shop Close release, Buy / Sell inventory modes, walking away from a shop.
- Gamble flag conflict `panels-2.md` §14 r11 vs `menus.md` §4.2.
- `client/ui.md` §A6 TOML example names (`toggle_inventory`) vs the `string_key` rule.
- `client/bridge.md`: the single-player pause (`frontend-options.md` §O1 r6).
- Waypoint level names (`0x00453E70`).

## Local checks queued (need the real install)

- Stash misclick: at 800×600, carry a 2×2 item to the stash, click the
  right half of column 6 and the lower half of row 8: the original
  places it at the last valid cell; record the C→S 0x18 x,y.
- Esc order: open inventory + automap, press Esc twice and once more:
  inventory closes, menu opens with the automap gone, menu closes and
  the automap is back.
