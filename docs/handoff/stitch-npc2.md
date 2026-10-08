# Stitch: NPC interaction, part 2 (`claude/stitch-npc2`)

> Stitching session, 2026-10-07, continuing `docs/handoff/stitch-npc.md`.
> Read only `specs/`, `docs/`, `crates/`, `tools/`. Preview glue under
> decisions D1–D3 (`docs/PLAN.md`); nothing here is verified against
> 1.14d (rule 10).

## Links connected

| # (stitch-npc) | Link | Now | Where |
|---|---|---|---|
| 4 | Server 0x13 object case → operate 23 → S→C 0x63 | **wired, PROVISIONAL** (below) | `app/single_player.rs` `LocalSeams::object_in_range` |
| 5 | S→C 0x63 → UI 0x14 on | wired; the root now mirrors the flags a delivered output set before it routes the next events | `world_view/ui_bind.rs` (`OriginalUi::sync_root`) |
| 6 | Waypoint panel drawn, rows, row click → C→S 0x49 | **installed** (`WaypointUi`) | `ui/waypoint_ui.rs`, `ui/original.rs` `install`; map: `app/ui.rs` `set_waypoint_map`, `single_player::client_waypoint_map` |
| 7 | Server 0x49 → travel | accepted by the server; the warp itself is `LocalSeams::warp` (logged: same-act placement is `path-placement.md` §11, the path provider's) | unchanged |

Test: `crates/d2-client/tests/app_play_npc.rs` — click the synthetic
town waypoint → walk → C→S 0x13 → S→C 0x63 → menu open (UI 0x14) with
rows town (current) and Cold Plains (known) → a click on row 1 sends
C→S 0x49 [GUID][3] and closes the menu. Failed before (no 0x63: the
play host's `object_in_range` default "out of range" stopped the
operate at `objects.md` §7.1 rule 3).

## PROVISIONAL points (M22)

- `world/objects.md` §7.1 r3, REC-94: the interact range `0x00623660`
  is read as "in range" for a player operator
  (`// PROVISIONAL (world/objects.md §7.1 r3; REC-94)`).
- Finding on stitch-npc's "spec gap": the spec does **not** need the
  interact info staged before the operate. §7.2 rule 2 refuses an
  active one, and operate 23 sets it itself after sending 0x63
  (`waypoints.md` §5.2 step 3). The blocker was the range seam above;
  the spec line says so.

Preview fills (d2rs-own, unverified): the menu keeps tab 0 (no client
quest flags in the model) and draws no row text (no string table by id
in play).

## What is left (in order)

1. **NPCs** (stitch-npc items 2–3; not started here): synthetic NPC in
   `GameParts::synthetic` / `build_with` (`app/single_player.rs`
   ~1102–1148, ~1339–1358); `WorldState::add_npc` at monster init
   (`d2-sim/src/wiring/interaction/mod.rs:152`); `AppRest` seam
   (`app/rest.rs:73` distance, `:90` player_busy, `:93` start_allowed);
   then `NpcMenuUi` over `ui/panels/npc_menu.rs` from
   `NpcMenus::menu(class)` (Talk → 0x2F, Trade → 0x38 action 1, Leave →
   0x30), and the `msg_ui.rs` `npc_dialog` / `npc_text_record` shown
   parts. Install it like `ui/waypoint_ui.rs` (a submodule of
   `original.rs`, one `root.add`).
2. **Trade** (item 4): store item message (stitch-items) and the
   0x32 / 0x33 builders (`ui/panels.md` §14.5).
3. Waypoint travel in play: `LocalSeams::warp` only logs; the warp
   needs the path provider's same-act placement (`path-placement.md`
   §11).
4. The waypoint menu's tab switch needs the client quest flags
   (`msg-ui.md` OQ 4); row names need `StringLookup::get_id`.

## Local check (Windows, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=debug"
git fetch origin claude/stitch-npc2; git checkout claude/stitch-npc2
cargo run -p d2-client --release -- play --new sorceress Test
```

What to see: left-click the town waypoint. The sorceress walks to it,
the waypoint menu opens (art, act tab 0, rows without names). The
current row is not clickable; a known destination row sends 0x49 (log
`world click` / intent) and the menu closes; no warp happens yet
(left item 3). Note in `docs/HANDOFF.md` whether the menu opens at the
distance the walk stopped (REC-94), and copy any `ui:` error line.
