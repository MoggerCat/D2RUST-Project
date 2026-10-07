# impl-c-sim: server/sim bucket-C wiring

Branch `claude/impl-c-sim` (from `claude/specs-staging-7` at 5bb11ba).
Task: the server/sim rows of the "C list for Opus" in
`docs/handoff/uncovered-rules-sort.md`. Two subagents did items 4 and
5 + 7 in worktrees; their branches are merged here.

## What landed

| # | Item | Where | Status |
|---|---|---|---|
| 1 | Inactive-unit store (`sim/units.md` §3.3–§3.4) | `d2-sim` `units::inactive` (rules, records, area nodes, restore order/filters); `wiring::action::inactive` (tick step 9 compress, the restore of `0x00542B40`) | rules done; wiring opt-in (`ActionHooks::enable_inactive_store`), facts and re-creation are `Pending` seams |
| 2 | Equipment bookkeeping + set-item state (`items/inventory.md` §5.5, §5.7, §5.8; `items/properties.md` §9 r3, §13) | `items::inventory::bookkeeping` (EquipWorld rules), `items::set_state` (SetWorld rules); `wiring::inventory::equip_rules` (both on `InvDesk`: inventories, item store, item tables, stat lists park/unpark and the stat-71 owner lists) | rules done; wiring opt-in (`InvState::equip_rules`); skill list, books skill columns, mouse slots, stat link/unlink, 0x22/0x48 sends, §11 set bonuses are new `InvRest` seams with defaults |
| 3 | Quest drop helper `0x00559A30` + sub-pickers (`items/treasure.md` §9, §9.1) | `treasure::quest_drop` (rules); `wiring::economy::unit_quest_drop` (floor drop, real item); wired into `HostQuests::drop_item_at` / `quest_drop` and the umod quest drop (`WorldHost`, new `WorldPending::umod_recharge`) when the game holds the drop state | done |
| 4 | Save appearance fill + hireling items in the d2s writer (`formats/d2s-appearance.md`, `formats/d2s.md` §2.8, `world/hirelings.md` §10 r8) | `d2-formats` `d2s::appearance`, `Header::rebuild_appearance`, `Body::set_hireling_items`; `d2-server` `adapters::character::save` | done; no save trigger in d2-server calls it yet; weapon class `0x0064F380` still a stub |
| 5 | Sound-event slot `0x00553380` + S→C 0x2C (`audio/triggers-2.md` §14, `world/cube.md` §8 l2 r3, `world/objects.md` §14 r2) | `d2-sim` `units::sound` (slot on `Game`), flushed for objects (`send_unit_update`), monsters (`monster_update` step 6), players (server `update_pass`); cube and object sounds fill it | done (`Pending::object_sound` removed: no implementor) |
| 6 | Collision line `0x00622AA0` + line test `0x0064E260` (`render/draw-order-2.md` §15.1, §16) | `d2-sim` `path::line`; with the path provider the AI `line_blocked` / `line_blocked_mask` and the skill bodies' `line_blocked` use it | done for the server. **Visibility predicate (client/model §13) not done** (d2-client render, other sessions' area) |
| 7 | C→S 0x4F stash buttons (`world/vendors-2.md` §10.1, §10.2, §10.4) | `d2-sim` `world::stash`; `d2-server` 0x4F handler (`StashWorld` on the cube world, new `TownRooms` trait) | done; §10.3 trade stays the stub |

Also: `ItemRec` gains `spawnable`, `rarity`, `bitfield1`; `ItemTables`
gains `parts` (weapons/armor/misc ranges).

## Coverage (any tier, `py tools/coverage.py --summary`)

| spec | before | after |
|---|---|---|
| sim/units.md | 51 / 87 | 65 / 87 |
| items/inventory.md | 72 / 105 | 94 / 105 |
| items/properties.md | 41 / 49 | 49 / 49 |
| items/treasure.md | 83 / 96 | 91 / 96 |
| formats/d2s-appearance.md | 0 / 21 | 21 / 21 |
| formats/d2s.md | 92 / 133 | 98 / 133 |
| render/draw-order-2.md | 43 / 53 | 49 / 53 |
| audio/triggers-2.md | 10 / 63 | 12 / 63 |
| world/vendors-2.md | 7 / 26 | 18 / 26 |
| world/cube.md, objects.md, hirelings.md | 59, 96, 106 | 60, 97, 107 |
| **total** | 7152 / 10166 (70.4 %) | 7253 / 10166 (71.3 %) |

All new claims are `unit` tier (one `game`-tier ignored test for
d2s-appearance §1 r3). Claims on exempt heading-text rules were removed
(the exemption list landed on staging-7 after the subagents' base).

## PROVISIONAL points

- `formats/d2s-appearance.md` §1 r2 / OQ3: the reference table
  `0x00744CA8` reconstructed as "slots 57–124 are weapon slots"
  (IT-6; HANDOFF local run queue item 99 reads it from the image).
- `formats/d2s-appearance.md` §2 r1 / OQ4: an empty `alternategfx` is
  compared like any code (IT-6).
- `audio/triggers-2.md` §14 r2: the chest's at-once key sound leaves the
  slot set, so the player's update sends it again in the same tick
  (recording: a locked chest opened with a key; not yet on the HANDOFF
  §7 list).

d2rs choices (not provisional): `treasure.md` §9.1 n = 0 → −1 (the
spec's Ruleset choice, IT-11); the §9 magic loop stops after 2²⁰
retries with `TreasureError::QuestDropHang` (1.14d hangs); the
equipment rules queued while an inventory is lent run at the end of the
inventory call (`wiring::inventory::equip_rules::run_equip_queue`); the
inactive node key is the room's sub-tile origin (same x order as the
tile origin).

## Gate

Pre-existing failures on the base (not from this branch; CODE-TABLE
CHANGE commits on staging-7 whose code is other sessions'):

- `d2-sim world::objects::tests::routes_match_function_table` and
  `route_check_catches_perturbations`: `specs/world/object-functions.tsv`
  (e44dcaa) gives null slots owners (`§7.2`, `§3`) the route table does
  not have yet (impl-final-world).
- `d2-client`: every test that builds the bridge fails with
  `NoHandler` for ids 117, 121, 127, 139–141, 143, 144, 174–176, 178,
  179: `specs/client/bridge-dispatch.tsv` (0df31c8) owns 13 rows the
  client has no handler for (impl-pc1-breadth).

Full gate (`CARGO_INCREMENTAL=0 sh tools/gate.sh`) on the head: every step
passes except the two test steps, which fail only on these. Every other
suite passes (d2-sim, d2-server, d2-formats, d2-proto,
d2-data, d2-net, d2-verify, conformance).

## Left

- Client: the visibility predicate provider (client/model §13); the
  client 0x2C handler; `crates/d2-client/tests/e2e_single_player.rs`
  step 25 (frame 34) will need a 0x2C after the update pass once the
  dispatch table is fixed (the transmute's sound now reaches the wire).
  The client's own §16 copy (`rules/draw_order/sight.rs`) can switch to
  `d2_sim::path::line::line_test`.
- Hosts turning on the opt-in providers: `enable_inactive_store` with a
  `Pending` that answers the §3.3 facts and re-creates records (monster
  spawn with a GUID, item record via the bit stream); `equip_rules` with
  an `InvRest` that holds the skill list and mouse slots.
- `properties.md` §11 set bonuses on the owner list (`InvRest::set_bonuses`
  seam; `items::props::set_bonuses` needs an `ItemStats` adapter).
- `ChestWorld::drop_item_code` (objects.md §8.1 r7) still a default:
  objects drops are impl-final-world's; it can call
  `wiring::economy::unit_quest_drop`.
- A d2-server save trigger calling the appearance / hireling writer.
- Corpse take-back (inventory-moves.md §8.5, §12) was in the C list but
  not in this task.
