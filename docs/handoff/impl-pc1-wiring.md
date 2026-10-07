# Handoff: PC 1 wiring — `claude/impl-pc1-wiring`

Cloud implementation session, 2026-10-07. Base `claude/specs-staging-5`
@ 2675253. Task: the "Open" items 1–2 of `impl-pc1-final.md` and the UI
side of `client/msg-ui.md` OQ10. One subagent (the client part).
Everything is **implemented, unverified** (no trace or game-file run).

## What landed

### 1. Unit event registry (`combat/events.md`, `skills/bodies.md` §2.18)

- `ActionHooks::unit_events: Option<UnitEventFn<X>>` (the
  `0x005C0C30` iteration). `ActionHooks::enable_unit_events()` (hosts
  with `UseRest`) installs `wiring::interaction::skill_use::
  run_unit_event`, which runs `combat::events::run` on `UseView` over
  `ActionHooks::handlers`. Without it every event stays on
  `Pending::unit_event` / `Pending::level_up_event` (no behavior change
  for hosts without `UseRest`, e.g. the client's `LocalSeams`).
- `EventWorld` on `UseView`: handler lists, GUIDs, list owner (type /
  GUID resolved by `0x00552F60`) are real; the rest are new `Pending`
  seams (`event_layer_split`, `event_terror`, `event_point_free`,
  `event_corpse_near`, `queue_item_cast`, `raise_test`, `clear_pattern`,
  `raise_step`).
- Callers routed through `CombatView::fire_unit_event`: combat's events
  (`damage.md` §5.4, `CombatWorld::unit_event`), missile event 0
  (`hit_by_missile_event`), DT end event 13 (`death_event1`), level-up
  event 12 of the kill's distribution (`vitals.md` §4.5; `VitalsUnits` +
  `ExpShare` on `CombatView`, the kill now distributes on it).
- Tests: `wiring::interaction::tests::unit_events::*` (3).

### 2. Player death (`vitals.md` §4.6–§4.7)

- `wiring::action::death`: `ActionHooks::death_penalties` from the DT
  start (`UnitHooks::player_death`, `0x00580EC0` → `0x00580F59`),
  `corpse_creation` from the DD start (`UnitHooks::player_corpse`,
  `0x0057FCA0` → `0x0057F700`), `corpse_pickup` (`0x0057FB70`). Client
  +0x508 is `ActionHooks::death.exp_lost` (per player).
- Reading taken: K (killer) = the DT start's unit target, which the host
  puts in `ActionHooks::mode_target` (`units.md` §4.5 unit form,
  `damage.md` §7.1 r5.4 requests DT with tA/gA).
- Gold limit = level × 10000 (`inventory-moves.md` §7.22); game type =
  `ai_info.game_type`; `DeathExpPenalty` from the game's difficulty.
- New `Pending` seams: `stash_cap`, `death_drop_gold`, `create_corpse`,
  `corpse_owner_guid`, `corpse_loot_allowed`, `corpse_take_back`.
- Tests: `wiring::action::tests::player_death::*` (2).

### 3. Client: 0x28 NPC-dialog branch → C→S 0x31 (subagent)

- `ui/msg_ui.rs`: `NpcTextList` (0x27, `msg-ui.md` §5 r2) and
  `dialog_case` (B0 when `[0x007C0C68]` ≠ 0, ends the branch; no list →
  fatal 0x1060 `NoNpcText`; B1 cursor item; B2 m ≠ 0xFFFF; else Rest).
  The UI writes no model.
- `world_view/present.rs`: delivery is the plain fn `deliver`; after a
  UI output with a chosen case it calls `Bridge::npc_dialog_branch` at
  once (same frame as the 0x28, before the slot drop). The Bevy system
  is a thin wrapper, scheduled before `mirror_units`.
- Tests: `bridge::tests::ui_answers_npc_dialog_with_0x31_in_order`,
  `msg_ui::tests::{npc_text_list, npc_dialog_case,
  npc_dialog_answer_is_kept_for_the_bridge}`.

## Spec gaps (seams left)

- `[0x007C0C68]` has no specified writer (`msg-ui.md` OQ10): stays 0
  (`MsgUiState::ui_7c0c68`, setter `set_ui_7c0c68`).
- `0x00661400` / `0x00661440` (the NPC text list walk giving m) are not
  written: only the A 37351 shape (one entry, kind 0) is answered; any
  other shape sends no 0x31 (`NpcTextList::first_m`).
- PC 1: record decision A in `msg-ui.md` OQ10 / §16 r5 and the two
  seams above.
- The item-event layer split values (data +0xC6C / +0xC70) are not
  written (`sim/stats.md`): `Pending::event_layer_split` default (0, 0).
- The rest of `0x00580EC0` and `0x0057FCA0` (character save), corpse
  creation itself (`0x0057F700` without the experience), and the
  no-corpse path of the +0x508 clear are not written.
- Stash limit `0x00623460` not written (unused in game type 3).
- Not wired here (other areas / layers): the inventory desk's
  `MovePending::corpse_pickup` (items area) should call
  `ActionHooks::corpse_pickup`; the interaction layer's
  `VitalsRest::level_up_event` and the hireling `level_events` (event
  12 on the merc) stay seams; nothing in the action wiring starts the
  player DT yet (`Pending::reaction` holds `damage.md` §7.1's player
  branch).
- Skipped (spec-blocked, as instructed): monster 0xAC server fields past
  `init.md` §24; 0x67 refusal name checks; Cold Plains 97 vs 98 (C92).

## Gate

See the coordinator message for the `CARGO_INCREMENTAL=0 sh
tools/gate.sh` result on the pushed head.
