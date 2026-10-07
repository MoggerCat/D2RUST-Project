# Handoff: items wiring (gate on the merged base, HM3, cube copy, corpse pickup) — `claude/impl-items-wiring`

Cloud implementation session, 2026-10-07, implementation from specs.
Base: `claude/specs-staging-6` at `2e1a003` (main + impl-session-flow +
impl-path-motion + impl-pc1-wiring + impl-items-rest). Repo only, no game
files (M09): every claim below holds on this branch, on synthetic data.
Task: `docs/handoff/impl-items-rest.md` §3 "Left" (a)–(c), after making
the gate pass on the merged base.

## 1. Landed (one commit per item, in push order)

| Commit | What |
|---|---|
| `Gate on the merged base: d2-client tests to the written specs` | The base gate failed only in `d2-client` (not built by impl-items-rest). Fixtures: itemtypes `shoots` = 0xFFFF (the link miss, as the other inventory fixtures) in both e2e item-table builders; the itemstatcost save columns of stats 31 / 72 / 73 in `e2e_support::item_tables` (the copy's save-format round trip otherwise drops defense and durability). The new `InvItemRec` / `AffixRec` / `UniqueRec` / `SetItemRec` / `SkillRec` columns needed nothing: their defaults (0) are the fixtures' answers. |
| `HM3: every interaction reader on UnitRecord::interact` | One home for +0x64 / +0x68 / +0x6C. Removed: `NpcRest::{interact_unit, set_interact, reset_interact}`, `CubeRest::{interaction, set_interaction, reset_interaction, interacting_with_stash, trading}`, `Pending::{set_interact, reset_interact, interact_guid}`, `InvRest::{interaction, clear_interaction}`; d2-server `Interact`, `RestInteract`, `HostFacts::interacting`, `HostWaypoints::rest`. Readers: NPC desk, `EconomyQuests` (`HostQuests` forwards), `EconomyCube` (stash = type 2 + object class 0x10B, `cube.md` §1; trading = type 0 + a live player, `inventory.md` §5.2), waypoint and object views, the AI's and the waypoints' busy (record active, then `Pending::busy`, whose doc now says "without the interaction part"), the inventory desk (`InteractionTarget`: type + GUID lookup `0x00552F60`, a missing unit → `Missing`), `ServerCube` (asks the economy cube), the player handler's busy. Every fake's interaction map is gone (d2-sim, d2-server, d2-client incl. the production `LocalSeams`, test-fixtures `Seams`, scenario-run). |
| `Server cube duplicate on the inventory model` | `ServerCube::duplicate` → `InvDesk::copy_of(item, fillers)` when the host has inventory parts (as `InvVendors::copy_item`); `ItemPending::duplicate` only without them. |
| `Corpse pickup through the unit hooks` | New `UnitHooks::player_corpse_pickup(sim, player, corpse)` (default nothing); `ActionHooks` answers with `corpse_pickup` (`0x0057FB70`, impl-pc1-wiring); the inventory desk's `MovePending::corpse_pickup` resolves both owners and calls it (no longer the rest). |

Player data +0x4C / +0x50 still have no home (`InvRest::player_data_4c`,
`player_data_50`, `Pending::busy`'s cursor / +0x4C part): no spec gives
their storage.

## 2. Changed test expectations (old answer → spec)

- `d2-client app_frame_loop::frame_loop_ticks_the_server_and_feeds_the_world_view`:
  handled 19 → 20: the session sequence carries S→C 0x53 after 0x03
  (`intents-events.md` §8 step 4, recorded `53 02000000 00000000 00`).
- `d2-client e2e_vendor` (steps 5–7, `vendor_end_to_end`): the buy no
  longer stops at the copy (0x2A code 9 → code 0 kind 4, the copy in the
  backpack, gold − 100·AC/5); the buckler sale completes (code 9 → 0x2A
  code 1 kind 3, a restored copy in the store, gold + (80·AC/6)·512/1024);
  `vendor_end_to_end`: no copy reaches the rest.
- `d2-client e2e_single_player`: step 20 buy and step 21 buckler sale as
  above (the buy's 0x9C action 4, 0x47, 0x48 in the update pass); later
  inventories follow; the client model counts +1 unit, handled 40 → 46.
  With one interaction home, the Akara talk makes the player busy, so a
  cursor pickup during the trade does nothing (`inventory-moves.md` §8.2,
  `inventory.md` §5.2 — the old pass relied on `InvFx` answering "no
  interaction"): the scenario sends C→S 0x30 (`npc.md` §3) in the frames
  of steps 11 and 23 and talks again (0x13) in step 19's frame; step 20
  gets the client's own 0x2F first.
- `d2-client e2e_full_loop`: step 13 buy as above; handled 17 → 20;
  inventory 2 → 3; final gold `> PLAYER_GOLD` → `!= PLAYER_GOLD` (the buy
  costs more than the sale paid; each step's gold is exact in the run).
- `d2-server mutants_handlers_items` (`mod_copy_takes_the_output_class`,
  `useitem_tempered`, `rem_drops_the_runeword_stats`, `rep_and_rch`,
  `placement_outcomes`): the copy is the model's copy of the slot-0 input
  (the ring, a new unit), not a scripted amulet from `ItemPending`;
  `rep_and_rch` breaks the ring (the copy carries the flag); the
  placement failure widens the ring's record. No "duplicate" reaches the
  rest.
- `d2-sim mutants_wiring_inventory`: `forward_pending` — `interaction` /
  `clear_interaction` are answered by the desk, not forwarded;
  `trading_is_an_interaction_with_a_player` stages the record.
- Tests that staged the interaction in a fake map now stage the record
  (reset, then set: the fixtures' stand-in for openings no spec writes —
  the cube's item use, the waypoint's operate).

## 3. Gaps

1. `0x00554D00` (`waypoints.md` §6.3 rule 1, `quests-act2-2.md` §3.1):
   no written body; `interact_guid` reads the record's GUID while active
   (TODO in `wiring/action/waypoints.rs`).
2. **Flaky property, pre-existing on `2e1a003`:**
   `d2-server prop_unified_items::item_moves_keep_one_place` fails on
   about 1 case in a few hundred (`PROPTEST_CASES=2000` finds it; the
   gate's 64 usually miss it). Minimal input: seed 271983823, ops
   `[OpenTrade, Spawn(0), Msg { id: 0x32, a: 204, b: 0, x: 91, y: 0 }]`:
   a buy with t = 91 (`vendors.md` edge case 3) of a **ground** item;
   §7.3 step 3 allocates the copy in the source's room R, §7.1 rule 9.5
   sets mode 4 and §2.4 places it without leaving the room, so the copy
   is in the backpack and on the ground. Spec question for PC 2: does
   1.14d leave the copy in R's unit list (then the property needs a
   quirk), or does something unwritten take it out?
3. Still on the rest (unchanged): the NPC socketing's copy
   (`NpcRest::duplicate`), `EconomyCube`'s own `CubeRest::duplicate`
   (d2-sim hosts without the server cube), `MovePending::copy_item`
   (hireling take, fillers argument unwritten), the socket routines
   (impl-items-rest §3), `.d2s` item placement and item-use bodies
   (spec-blocked, queued for PC 2).
4. The client-side 0x9C of a bought copy in `e2e_vendor` (recorded next
   frame, `vendors.md` §7.1 rule 10) is not produced in that fixture (no
   item update pass there), as its step-3 0x9C action 11.

## 4. Gate

`CARGO_INCREMENTAL=0 sh tools/gate.sh`: see the coordinator message for
the head's result.
