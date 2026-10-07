# Stitch: NPC interaction (`claude/stitch-npc`)

> Stitching session, 2026-10-07. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Glue for the `play` preview under decisions D1–D3
> (`docs/PLAN.md`). Nothing here is verified against 1.14d (rule 10);
> every preview fill is `// d2rs-own, unverified`.

## Path traced (input → bridge → server → client → UI)

| # | Link | State before | Where |
|---|---|---|---|
| 1 | A click picks a unit (hover target) | **missing**: `ModelClick::hover` always `None`, every click is a ground click | `bridge/click.rs` `hover` |
| 2 | Click on an NPC takes the NPC path (`npc` / `interact` bits) | **missing**: `monster_npc_interact` hard-coded `(false, false)` | `bridge/click.rs` |
| 3 | Out of reach: walk + pending interaction, then C→S 0x13 on arrival | **missing**: the `Pend` output was only logged; no client path arrival | `world_view/present.rs` world clicks |
| 4 | Server 0x13 object case → operate 23 → S→C 0x63 | **blocked by a spec gap**: the operate needs the player's interact info (unit record `interact`, type + GUID, active) staged; no written spec opens it (`e2e_single_player.rs` `stage_interact` sets it by hand). The play server answers the 0x13 with nothing | `d2-server/.../handlers/world.rs` `WaypointOperate`; owner: `world/objects.md` §7.1 / `sim/intents-events.md` (0x13) |
| 5 | S→C 0x63 → `OriginalUi::waypoint_menu` → UI 0x14 on | wired | `ui/msg_ui.rs` |
| 6 | Waypoint panel drawn, a row click sends C→S 0x49 | **not installed**: `ui/panels/waypoint.rs` (`WaypointPanel::choose_row`) is used by no app code | `ui/original.rs` `install` |
| 7 | Server 0x49 → warp (`waypoints.md` §7) | wired (`e2e_single_player.rs` step 6) | `handlers/world.rs` |
| 8 | Synthetic game has a town NPC | **missing**: `GameParts::synthetic` has no `monstats`, `build_with` allocates only the waypoint | `app/single_player.rs` ~1102–1148, ~1339–1358 |
| 9 | NPC interaction lists | **missing** outside tests: `WorldState::add_npc` is never called by monster init, so `NpcControl::start` returns 0 | `d2-sim/src/wiring/interaction/mod.rs:152`, `world/npc.rs:654` |
| 10 | Play host's NPC seam | **refuses**: `AppRest` `distance` = `i32::MAX`, `player_busy` = 1, `start_allowed` = false | `app/rest.rs:73, 90, 93` |
| 11 | S→C 0x27 / 0x28 → NPC menu box (Talk / Trade / Hire / Leave) | **not installed**: `npc_dialog` / `npc_text_record` skip the shown parts; `ui/panels/{npc_menu,npc,menu_box,shop}.rs`, `ui/messages/{dialog,npc_text}.rs` used only by tests | `ui/msg_ui.rs:292–322`, `ui/original.rs` `install` |
| 12 | Trade: store items to the client | **missing**: after 0x38 action 1 the server sends nothing (`add_trade_inventory` log-only stub); the store item message is in an unwritten item spec | `app/rest.rs:307`; `e2e_vendor.rs:589–605` |
| 13 | Buy / sell senders (C→S 0x32 / 0x33) | **missing**: "not built" (open in `ui/panels.md` §14.5) | `ui/panels/shop.rs:17–23` |

## Connected here

- **1 Hover pick** (`bridge/hover.rs`, new): the unit whose feet are
  nearest the mouse inside a 48 × 112 box standing on the feet;
  monsters, objects, items only; dead monsters skipped. On only in the
  preview (`ClickView::pick` = `WorldViewState::preview`); the press reads
  the event position. d2rs-own, unverified (no hover spec, `0x00467A10`).
- **2 NPC bits**: `ModelClick::monster_npc_interact` answers the class's
  `MonsterClass { npc, interact }` from the client tables.
- **Skill row fill**: `ModelClick::skill_row` answers a row with no
  flags and range 0 in the preview (the client `skills` rows have no
  flag columns), so a click on a picked unit takes §6 r8.3 → 3.6
  interact instead of the point path. d2rs-own, unverified.
- **3 Pending interaction** (`world_view/interact.rs`, new;
  `Bridge::interact`): the click's `Pend` is kept; when the predicted
  walk was seen and ended (or no walk showed within 8 frames) the
  interact sender runs (C→S 0x13). A new press without a pend drops it.
  d2rs-own, unverified (the original's arrival is the client path's,
  REC-51).

Test: `crates/d2-client/tests/app_play_npc.rs` — clicking the synthetic
town waypoint picks it, walks with the interaction pending and sends
C→S 0x13 {2, GUID} on arrival (it failed before: no pending record, the
click was a ground walk). It also pins the seam: no S→C 0x63 comes back
yet (link 4), so the menu stays closed.

## What is left (in order)

0. Spec the server's interact-info staging for C→S 0x13 (link 4,
   spec session: who sets the unit record's `interact` before
   `objects.md` §7.1's operate); then the 0x63 comes back and UI 0x14
   opens (`ui/msg_ui.rs` is wired).
1. Install the waypoint panel (link 6): a `WaypointUi` wrapper in
   `OriginalUi::install` over `WaypointPanel` with a `WaypointView` from
   `msg.waypoint` + `waypoint_rows.rs`; its 0x49 leaves as a root intent.
2. NPC in the synthetic game (8) + `add_npc` at monster init (9) +
   `AppRest` NPC seam (10), then an e2e that talks to it.
3. NPC menu panel (11): `NpcMenuUi` built from `NpcMenus::menu(class)`;
   Talk → 0x2F, Trade → 0x38 action 1, Leave → 0x30 (`ui/panels/npc.rs`
   `option_intent`, `msg_chat_end`).
4. Trade (12, 13): needs the store item message spec (stitch-items) and
   the 0x32 / 0x33 builders (`ui/panels.md` §14.5).
5. `selectable` reads `flag_4` (from `isAtt`): town NPCs may never be
   selectable for skill clicks; the pick and §6 r3.6 interact do not
   need it.

Seam for stitch-combat: `ClickView::pick` / `bridge::hover::pick` is the
hover target for monster clicks too.

## Local check (Windows, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=debug"
git fetch origin claude/stitch-npc; git checkout claude/stitch-npc
cargo run -p d2-client --release -- play --new sorceress Test
```

What to see: left-click the town waypoint. The log shows `world click:
… Pend(Some(Pending { code: 19, target: UnitKey { unit_type: 2 … } }))`,
the sorceress walks to it and stops; no menu opens yet (left items 0
and 1). With the live server that may differ: if a `ui: …` effect for
UI 20 (0x14) appears, the live 0x13 path stages the interaction; note
which in `docs/HANDOFF.md`. Clicking an NPC (once play-fix2 draws them): a `Pend` on unit type 1
and a C→S 0x13; nothing opens yet (items 2–3). Copy any `frame not
drawn` or `interact:` error line into `docs/HANDOFF.md`.
