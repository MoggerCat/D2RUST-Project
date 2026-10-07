none — the coordinator folds it

# Handoff: items rest (bit stream, sockets / InvRest, copy, save indices, HM3 provider) — `claude/impl-items-rest`

Cloud implementation session, 2026-10-07, implementation from specs.
Base: `claude/specs-staging-5` at `666e2f2`. Repo only, no game files
(M09): every claim below holds on this branch, on synthetic data.
Task: the items part of HANDOFF §1 row "3 not implemented" (ninth fold).

## 1. Landed (one commit per item, in push order)

| Commit | What | Public API / seams |
|---|---|---|
| `Item bit stream: runeword name id, …` | `bitstream.md` §4.4 rule 1: the runeword record's name id (`properties.md` §10.1 on the item's own inventory, stat 194 as u8); the writer's changes to the item (§4.1 rule 8, §4.3 rule 7) reach the item store; names bounded at the setter (edge case 7) | `RuneRec::name_id` (+0x82 from the fixed-up raw record, `RuneRec::from_record`); `props::runeword_row(t, record, quality, sockets, fillers)` (exact form; `runeword_match` wraps it); `InvDesk::runeword_name`; `InvDesk::apply_write_backs` (queued by `item_stream`, applied at desk creation, `sync_out`, `update_done`); `Item::set_name` / `NameTooLong` |
| `Inventory seams: item queries and socket effects on the desk` | `wiring::inventory::queries`: two-handed (items `2handed`), ammo type (primary type's `shoots`), the §4.8 level requirement's inputs, the spell (item data +0x3E), the filler's properties (`properties.md` §9) and the runeword step of `inventory-moves.md` §7.19 step 3 (§10.1, §10.2, replenish timer) | `InvDesk::{is_two_handed, ammo_of, spell_of, level_req_unit, level_req_item, item_level_requirement, apply_filler_properties, activate_runeword_on}`. `InvRest::{two_handed, ammo_type, level_requirement}` now have defaults and are **not asked**; `MovePending::runeword` is the desk's; `filler_linked` runs §9 then still forwards to the rest (owner link `0x006276C0`). New columns: `InvItemRec::{twohanded, levelreq}`, `AffixRec::{levelreq, class, classlevelreq}`, `UniqueRec::lvl_req`, `SetItemRec::lvl_req`, `SkillRec::charclass` |
| `Item copy and items from save records` | the save-format record reader (inverse of the writer, `vendors.md` §7.3 step 3 / OQ8), an item unit from a record (`0x00558CB0`), the item copy `0x0055A2A0` (`vendors.md` §7.3) | `items::bitstream::read::{read_save_record, read_save_entry, BitReader, ReadItem, ReadEntry, ReadError, record_of, kind_of}`; `Economy::item_from_record(&ReadItem, room)`; `InvDesk::{save_view, copy_of(src, fillers)}`; `InvError::Unwritten(&str)` |
| `Interact info on the unit record (HM3 provider)` | +0x64 GUID / +0x68 type / +0x6C active with get `0x00554100`, set `0x00554120` (ignored while active), reset `0x00554190` (GUID −1, type 6, inactive) | `UnitRecord::interact: InteractInfo`, `InteractInfo::{get, set, reset}`, `INTERACT_RESET_TYPE`. **Not wired yet** (§3) |
| `Save item indices; the vendors' copy on the inventory model` | `d2s.md` §2.4 rules 2, 4, 6; the server's vendor world copies through the model | `InvState::{save_item_index, item_at_save_index}`, `IndexTooLarge`; `d2-server` `InvVendors::copy_item` → `InvDesk::copy_of(item, true)` when the host has inventory parts |
| `Items from records: the save's 32 bits seed the item` | `sim/rng.md` §5.3 "item seed from a save": the full record's 32 bits are the item seed (`init_low`; compact 0) | — |

The writer's TODO on the partner record (§4.6 rule 4.3) is gone: the spec
now says per list.

## 2. Tests

New: `items::bitstream::read_tests` (4: round trips of every record class
byte for byte, children, flag rule, errors), `items::bitstream::tests`
(+3: stat 326, compact ear, name setter; edge-case claims 1–8 added to the
existing vectors), `wiring::inventory::tests::{bits (2), queries (5),
copy (3), save_index (1)}`, `units::record::interact_tests` (1).

Changed to the new providers (old answer came from a fake rest; listed per
the rule): d2-sim `tests::equip` (2h from the column), `items::tests::mutants::skill_record_projection` (charclass),
`mutants_wiring_inventory` (`forward_inv_world`, `forward_pending`,
`forward_inventory_ops`, `hand_result_7…`, `socket_getters`: not
forwarded any more); d2-server `items::moves::tests` (`swap_two_handed_item`,
`scroll_to_book_and_its_fatal`: column / suffix slot instead of the rest),
`items::tests::mutant_tests::inv_vendors_pass_other_calls_through` (the
model's copy, source flag 0x8000000), `tests/prop_unified_items.rs`
(`take_from_store` keeps the item: `vendors.md` §7.1 rule 12, reachable now
that a buy copies). Fixtures: empty itemtypes `shoots` = 0xFFFF (the link
miss) in the d2-sim and d2-server inventory fixtures.

Gate on `9be97a0`: `cargo fmt --all --check`, `cargo clippy --workspace
--exclude d2-client --all-targets -D warnings`, `coverage.py --check`
(8931 claims, 0 errors), `spec_index.py --check` clean; `cargo nextest run
--workspace --exclude d2-client`: 4613 passed, **5 failed, all red on the
base `666e2f2` too** (`d2-sim monsters::ai::tests::specd_here_matches_tsv`,
`…specd_here_check_catches_perturbations`: index 41 status; three
`scenario-run::scenarios` tests: `vendor-buy-sell.scenario` line 29
`BuyItem` has no field `mode`).

**d2-client not built** (broken on the base). Its e2e fixture
(`tests/e2e_support`) is affected when it compiles again: the desk no
longer asks `InvFx` for `two_handed`, `ammo_type`, `level_requirement`,
`spell`, `runeword`; they come from the tables (its `inv_tables` leaves
`twohanded` / `levelreq` 0 and its itemtypes `shoots` 0 — set `shoots`
to 0xFFFF as the other fixtures now do, or ammo type 0 matches every type
through the "any" row).

## 3. Left (not done here)

- **HM3 wiring.** The provider exists (`UnitRecord::interact`); the
  readers still ask their rests: `NpcRest::{interact_unit, set_interact,
  reset_interact}` (`wiring/interaction/npc_world.rs`, `npc_vendors.rs`,
  `economy/quest_host.rs`), `CubeRest::{interaction, set_interaction,
  reset_interaction}` (`economy/cube_items.rs`), `Pending::{set_interact,
  reset_interact, interact_guid}` (`action/waypoints.rs`,
  `action/objects.rs`), `InvRest::{interaction, clear_interaction}`
  (`inventory/inv_world.rs`), and the server's `HostWaypoints` /
  `RestInteract` (`d2-server …/world/wired.rs`). Move them together in
  one change (a partial move makes two homes); it touches the action
  wiring and `world/wired.rs`, held by other sessions tonight. Player
  data +0x4C / +0x50 have no home yet either.
- **Cube duplicate on the model.** `ServerCube::duplicate` can call
  `InvDesk::copy_of(item, fillers)` like the vendors; the five
  `mutants_handlers_items` tests that script copies by unit id
  (`placement_outcomes`, `mod_copy_takes_the_output_class`,
  `rem_drops_the_runeword_stats`, `rep_and_rch`, `useitem_tempered`)
  must be rewritten with it. `MovePending::copy_item` (hireling take,
  §7.23) still goes to the rest: its fillers argument is not written.
- **`.d2s` load items.** `ActionCharacter::create_items` / corpse /
  hireling items / golem stay `Unapplied`: the reader and
  `Economy::item_from_record` exist, but the placement routines are not
  written (gap 6). `resolve_item_indices` can use
  `InvState::item_at_save_index` once the action character has the
  inventory state.
- **Item-use bodies.** Blocked: the dispatcher `0x005BF240` has no spec
  (gap 5); the cube's entry (`0x005BF0C0`) is the only written one and
  exists (`world/cube.rs`).
- Sockets: `socket_link`, `link_into_item`, `socket_filled`,
  `socket_filler`, `one_or_two_handed`, `has_allowed_location`,
  `quiver_kind`, `item_active_on`, `own_contribution` stay on the rest
  (gap 4).

## 4. Spec gaps (questions for PC 2)

1. `items/inventory-moves.md` §7.19 step 3 vs `items/properties.md` §10.2:
   a runes row with `server` ≠ 0 is allowed "in an expansion game" (record
   +0x81 = 0 or expansion) in the first, "unless game +0x74 (ladder) ≠ 0"
   in the second. Implemented: §10.2 (owner). Which is right?
2. `items/inventory.md` §2.4 step 6 calls `0x0055C2C0(owner, 0)` a stat
   refresh; `items/properties.md` §9 and `inventory-moves.md` §7.19 call
   `0x0055C2C0` the socket-filler properties. One address, two meanings.
3. `world/vendors.md` §7.3 step 5: what the flag arguments `(0, 1, 0, 0)`
   of `0x00562660(child, copy, &out, …)` switch off of
   `inventory-moves.md` §7.19 (the handler passes `1, 1, 1, 1`). Until
   written, `copy_of` with fillers and children stops
   (`InvError::Unwritten`, result none).
4. No written body for: `0x0063B210` on an item's inventory (the socket
   link; edge case 11 says it uses +0x28 as the filler's x), `0x0055F590`
   (socketed with fillers), `0x0062BEB0` (socket-filler test),
   `0x0062A1E0` (one-or-two-handed for a unit), `0x0062FDF0` (allowed
   location), `0x00628480` (quiver kind), `0x00625820` / `0x0062B450`
   (`inventory.md` OQ6).
5. The item-use dispatcher `0x005BF240` (`inventory-moves.md` §7.11,
   §7.17, §7.18; `world/cube.md` OQ7): no spec.
6. `formats/d2s.md` §8.2 rule 3: the placement routines `0x00531210` /
   `0x00531520` (and the child insert `0x00531210(child, parent)`); rule 5
   `0x00563470` (OQ13).
7. `items/bitstream.md` §4.2 / §4.1 rule 11: the writer sends a prefix id
   p ≤ P (an auto affix a ≤ A) unchanged, so the decoder's inverse is not
   unique; read as p′ + P (a′ + A) — every prefix / automagic id of the
   combined array is above its offset. Confirm `0x0062CBE0`.
8. `items/bitstream.md` §4.6 rule 1: a set list's flags (0x2040 or 0x40)
   are not on the wire; the reader rebuilds 0x2040. Which does
   `0x0062CBE0` use?
9. `sim/rng.md` §5.3 "item seed from a save": whether `0x0062CBE0` writes
   the whole seed (`init_low`, high word 666) or the low word only; read
   as `init_low`. And the start seed (+0x10) of a loaded / copied item.
10. `sim/units.md` §2: the allocation values of +0x64 / +0x68 (the
    interact info starts inactive here; no getter reads them).
11. `items/inventory-moves.md` §7.23: the fillers argument of the
    hireling take's copy (`0x0055A2A0`).
12. `items/inventory.md` §4.4 rule 2 / §4.7 rule 2: "has an ammo type"
    (`0x0062E6F0`): implemented as the `shoots` row tested through the
    equivalence (an empty cell is the link miss −1, which matches
    nothing). Confirm there is no separate "≠ −1" test.
