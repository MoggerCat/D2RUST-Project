# q-fix-proto-rest: the protocol rows q-fix-proto left

Session q-fix-proto-rest, 2026-10-09, branch `claude/q-fix-proto-rest` (from staging
`specs-staging-7`). Methods: M22 (provisional over blocked; REC-410..416), M23 (contract checks at
seams). Predecessor: `docs/handoff/q-fix-proto.md` ("Not done"). Index: `docs/handoff/proto-index.tsv`
(rows updated for every id below).

Every sender has a contract test in the `tests_proto_*` pattern: the d2-sim builder's bytes ==
the d2-proto encode == what the client handler reads.

## Done

| Row | What changed | Tests |
|---|---|---|
| 0x11 step 9 (missing producers) | `ActionTables.overlay_count` (`overlay.bin` records; every constructor sets it, the data loaders read the real count); `StatLists::overlay_to_send`; `units::messages::report_kill`; `View::monster_update` step 9 (list flag 0x100, overlay list flag 0x80, stat 178, `0 ≤ v ≤ count` inclusive). **PROVISIONAL** base read, **REC-410**. | `sound::monster_update_sends_the_overlay_0x11`; `tests_proto_a::report_kill_0x11_one_layout` now asserts sim bytes |
| 0x99 / 0x9A (P7 rest) | `EventRecord::CastOnUnit` / `CastOnPoint`; written by `queue_item_cast` (item cast, `combat/events.md` §3); sent by `send_event_records` with the receiver-dependent 16/17-byte choice of `0x0053D530` flag 1 (`skill_on_unit_message` now takes w and flag). **PROVISIONAL** level byte and w = aim, **REC-413**. | `sound::pending_skill_event_records_0x99_0x9a_0xa5_0xab`; `tests_proto_c::skill_events_0x99_0x9a_one_layout` |
| 0xA5 | `EventRecord::Landing`, written by the `MsgA5` body effect (`landing_msg`, Leap). **PROVISIONAL** unit identity, **REC-411**. | same sim test; `tests_proto_d::skill_npc_baal_0xa3_0xa4_0xa5_0xab_one_layout` asserts sim bytes |
| 0xAB | `EventRecord::NpcHeal`, written by `ActionHooks::send_life_fraction` (the regeneration fraction update, `stat-lists.md` §10.1, was a no-op hook before). Not sent for a revived unit. **PROVISIONAL** hostility test not applied, **REC-412**. | same sim test and contract test |
| 0x73 | `units::messages::client_missile`; `View::missile_add` (switch.rs part A for a missile: `ClientSend` row, existing owner, non-zero path velocity). **PROVISIONAL** field sources, **REC-414**. | `tests_proto_d::client_missile_0x73_one_layout` (new) |
| 0x20, 0x93 | Builders `stat_update`, `skill_bonus` only: no spec names a caller. **PROVISIONAL**, **REC-415**; call sites on the PC 1 list. | `tests_proto_a::stat_update_0x20_one_layout` (asserts sim bytes); `tests_proto_d::skill_bonus_0x93_one_layout` (new) |
| 0x92 client handler (P9) | `ItemData.unlinked`; `remove_items_display` marks the unit's body items (and active charms) unlinked; `item_lists::attached_to` skips them; the next record re-adds (`refresh` clears it). The model has no inventory nodes, so the nodes are derived from the items' last record (as the 0x74 handler does). **PROVISIONAL**, **REC-416**. | `tests_proto_d::remove_items_display_0x92_unlinks_the_owners_body_items` (new) |
| `prop_transport::dispatch_any` | The oracle `expected` did not know q-fix-proto's 0x15 string check (strlen < 256 and strlen + 4 < size, else 2, REC-402), so the nextest gate failed on `[0x15]`. The oracle now states that rule (a rule update, no assertion removed). | `prop_transport::dispatch_any` |
| 0xA6 | No writer: the spec found no caller of `0x0053E1C0` (§5 r7.2), so none exists to write. Re-check on the PC 1 list. | `item_table_entry_0xa6_one_layout` (unchanged) |

## Not done

- **q-fix-proto-one-type (P13)**: not small. The five ids with two `d2_proto` types (0x0B, 0x0D, 0x28,
  0x29, 0x2A) have hand-written types in `s2c/messages.rs` (named fields, `UNWRITTEN`, `consts` from the
  `s2c_message!` macro) used by `parse`, the `Message` enum, `s2c` tests and `seam_messages.rs`, and
  generated types in `generated.rs` (`type_`, `f6`…) used by the client handlers' tests. One type means
  choosing a field naming, giving the generated structs the macro's `UNWRITTEN` / `consts`, and touching
  `parse.rs`, `audit.rs` and about 30 call sites in two crates. The bytes are identical and tested
  (`*_one_layout`), so it stays a refactor row for a session with no behavior work.
- **Binary rows** (PC 1, `docs/handoff/pc1-data.md` Step 4 items 7–10): callers of `0x0053C1D0` /
  `0x0053C6F0` / `0x0053E1C0` / the 0x92 sender; the argument form of `0x00554200(unit)` in the 0xAB case;
  `0x00625A50` base or total; the 0x73 field sources; the 0x92 node order. Each has a REC id (410, 412,
  414–416) and a recording entry in `docs/HANDOFF.md`.
- **0x23 records** (writer `0x00571C60`, 2 callers): no spec names the callers; unchanged.
- **Not mine**: `vitals-dx-sign`, `state-param-sign` (binary).

## Environment notes

- `d2-client` builds need `pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`
  (`tools/cloud-setup.sh`); `cargo-nextest` is not preinstalled (`cargo install cargo-nextest --locked`).
