# Handoff: coverage session ui/messages, ui/menus, ui/control-panel (2026-10-07)

Cloud COVERAGE sub-session, branch `worktree-agent-a3914df9044d70d71` (off
`claude/cov-ui` `9d076a1b`), three code commits plus this file. Not pushed.
Only `d2-client` was built or tested (`cargo test -p d2-client`: 1054 lib
tests plus the integration tests pass; `cargo clippy -p d2-client
--all-targets -- -D warnings` and `cargo fmt` clean;
`py tools/coverage.py --check`: 0 errors).

## 1. Rules covered per spec (unit tier, `py tools/coverage.py --summary`)

| Spec | Units | Before | After | Left |
|---|---|---|---|---|
| `specs/ui/messages.md` | 52 | 0 | 52 | none |
| `specs/ui/menus.md` | 26 | 0 | 26 | none |
| `specs/ui/control-panel.md` | 51 | 0 | 50 | `§edge-cases-original-bugs` |

All claims are `unit` tier: implemented and unverified (M02). The three
specs have no recording yet (spec status "draft, no capture").

## 2. New code (plain Rust, no Bevy; the Bevy layer calls it)

- `ui/messages.rs` and `ui/messages/`: `chat` (§2 screen message list, §3
  chat formats, §4 recipe scroll), `overhead` (§5 bubbles, placement, §8
  timed box), `npc_text` (§6 text list, caption lookup, talk topic box),
  `dialog` (§7 dialog panel: open, text, scroll, draw, pass, skip, close),
  `hire` (§9 popup, §10 Inifuss scroll and stones), `socket` (§11 item
  dialog), `intro` (§13 intro table, §6 r4-r5 gossip, §14 interact NPC).
  Text measuring is the `Metrics` trait the caller gives.
- `ui/panels/menu_box.rs`: the menu object of `menus.md` §2.1, §2.4-§2.7.
- `ui/panels/npc_menu.rs`: §2.2-§2.3 NPC menu build and captions, §3 hire
  list (box, rows, row text, choose).
- `ui/panels/waypoint.rs`: §1 mouse down / up, key close, hit tests, tab
  setter, latched close, self close.
- `ui/panels/shop.rs`: §4 click, kind and price, send (0x32 / 0x33 /
  0x35 bytes), confirm dialog, callers.
- `ui/panels/control.rs` and `control/`: `globes` (§3, §4), `belt` (§5),
  `buttons` (§6, §7, §8), `minipanel` (§9), `input` (§10), root (§1, §2).
- `OriginalUi::free_npc_text` (`msg_ui.rs`, `messages.md` §6 r2).
- Files placed beside the ones the task named (`menu_box.rs`,
  `npc_menu.rs`, `control*`) so `npc.rs` / `border.rs` stay small; `border.rs`
  only gained two `Covers` lines (control-panel §1 r1, r2 on its existing
  tests).

## 3. Code fixes found (a test of the spec failed against the code)

1. **Waypoint latch** (`menus.md` §1.5, §1.7; `panels/waypoint.rs`).
   `WaypointPanel::close`, `close_hook` and `choose_row` sent a level-0
   0x49 every time, so a row choice followed by the close hook sent two
   messages. Per §1.5 every level-0 send needs the latch clear and a
   player with its room, and sets the latch; the row choice sets it too.
   Fixed (`latched_close_msg`, `no_player_room`). The old test
   `messages_choose_and_close` asserted the unlatched sequence; its
   assertions were changed to the latched behavior (the test vector
   "row chosen, then the close hook runs: one 0x49"), and it still checks
   the level-0 send, the close and the hook. `self_close` keeps its
   signature; `self_close_checked(has_player_room, player_ok)` adds the
   "needs a player and its room" and "the draw continues" parts.
2. Nothing else in existing code contradicted the three specs; the
   `panels.md` tests of the waypoint, NPC menu, shop and border still pass
   unchanged.

## 4. PROVISIONAL points (grep `PROVISIONAL crates`; ids are suggestions)

| Where | Choice | Settled by |
|---|---|---|
| `chat.rs` `to_8bit`, `RecipeScroll::set` | wide ↔ 8-bit conversion by unit (code page not stated) | REC-ui-chat-filter |
| `overhead.rs` `place` | the slot holds the 400 × 280 candidate, the bubble keeps its size at the candidate origin | REC-ui-bubble-move |
| `dialog.rs` `parse_text` | no trailing empty line; empty text speed 8 | REC-ui-dialog-text |
| `dialog.rs` `pass` | the dead-player / mode-12 step runs inside the running step | REC-ui-dialog-end |
| `intro.rs` `menu_open`, `roll_gossip` | plain greeting mode; list `0x00725CB0` read as 0xFF-terminated text ids | REC-ui-npc-greeting, REC-ui-gossip-list |
| `hire.rs` stones | a stone starts at f ≥ 1; the counter reset stores `now` | REC-ui-stones |
| `socket.rs` `left_down` | area click with an item placed takes it back (no cursor item), refuses with one | REC-ui-socket |
| `shop.rs` `send` | a refused repair-all clears the pending transaction | REC-ui-shop-refuse |
| `globes.rs` `Smoother::shown` | integer `(e + 1) · Δ / d` instead of x87 | REC-ui-globe-x87 (control-panel OQ1) |
| `minipanel.rs` press / release | layout read from the sign of the region offset | REC-ui-mini-layout |
| `belt.rs` `hover_text` | name and stat lines joined by LF | REC-ui-belt-hover |

## 5. Rules left, with reason

- `control-panel.md §edge-cases-original-bugs` (one unit). Its last bullet
  is "globe smoothing uses x87 floating point": the code is provisional
  integer arithmetic (§3 r1, OQ1 needs a frame-by-frame recording of a
  globe refilling), so a claim would overstate. The other three bullets
  are tested (belt font, `[0x007BC968]` vs the drawn side, skill
  replacement flag) in the unit tests of their rules but are not claimed.

## 6. Seams not wired (Bucket C, no rule left uncovered by them)

The new modules take plain inputs; these callers do not exist yet:

- `OriginalUi::apply_output` still skips the consumers it names in
  `msg_ui::skip` (screen message, NPC text show, NPC dialog UI, quest-log
  table, ...): wiring needs a string table and `Metrics` in `OriginalUi`.
- Belt records (`belts.bin`), the client world's items, unit screen
  positions (overhead bubbles, NPC anchor), the player's stats and states
  (globes, bars), key binding names, registry values (`Show HP Text`,
  `PopupHireling`) and the skill overlays (control-panel OQ3).
- The intro table data (46 classes, acts, 15-byte text records,
  `§13 r1`) and the caption table `0x00722678` (527 pairs) come from the
  install: `IntroEntry::new` applies the static flags, the data is a
  parameter. The claim on §13 r1 covers the structure, count and static
  values only.
- Drawing: the modules return draw requests (`MenuDraw`, `BeltDraw`,
  `GlobeDraw`, `DialogDraw`, ...); nothing renders them yet.
- Handler tables of the dialog (`SKIP_TABLE`) and menu box window
  handlers are registered by the host.

## 7. Spec observations

- `messages.md` §5 r5 "becomes the bubble's rectangle" does not say if the
  400 × 280 candidate is also the bubble's size (provisional: only the
  origin).
- `messages.md` §7 r6 "in the same pass" (dead player step) and §7 r7 (which
  handlers call the skip) are read as above.
- `control-panel.md` §9 r7 gives the press region offset but not which
  layout's button x the strict x test uses.
