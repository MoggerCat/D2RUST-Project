# Handoff: triage rows, items / inventory / bitstream and vendors / hirelings / session — `claude/impl-triage-items`

Base `claude/specs-staging-6` @ 740449b. Rows from
`docs/handoff/todo-spec-triage.md` part 1 ("sim: items / inventory /
bitstream", "sim: vendors / hirelings / session (PC 2)") and the part 2
NOT SPECIFIED rows of those areas (provisional per M22), plus PC 2's
final implementation list (vendors §7.3, cube, inventory-moves §8.5 /
§12) and three stale game-file test expectations.

## What landed (ANSWERED rows: code follows the spec, marker removed)

Items / inventory (`specs/items/inventory-moves.md`, `world/cube.md`):

- §7.8: E's unlink failing is fatal (`MoveFatal::Unlink`).
- §7.17: no hireling → the potion is used on the player (was: nothing).
- §7.23: a failed copy on the take from the hireling is fatal (new
  `MoveFatal::Create`), after the original left the hireling.
- §8.1 r5 (comment only, code agreed), §8.1 r7: the page-0 link failing
  is fatal.
- §9.3: the cube spill's unlink failing is fatal.
- §10.2: a failed gold-pile creation skips that pile and goes on (was:
  stop).
- §7.12 / cube.md §8 "Exact" 1: `0x00557FD0` unlinks the item from any
  player inventory list or cursor still holding it, then frees it
  (`InvError::FreedWhileLinked` removed).
- §7.19 step 3: the recharge `0x0055FE80` (`generation.md` §12.2) is now
  implemented (`items::recharge`, `InvDesk::recharge_item`,
  `InvDesk::socket_runeword`); the socketing announces each recharged
  charged skill with the S→C 0x3E seam (stat 204).
- vendors-2.md §7.3.1 r5 / rng.md §5.1: the decoded item seed is
  `init_low` {x, 666} (comment; code agreed).
- bitstream.md §3 r3: the 16-character name (three comments; code
  agreed).
- **Corpse take-back** (§8.5 / §12): `impl-items-wiring` only routed the
  pickup to the unit hooks; the action wiring's
  `Pending::corpse_take_back` was a no-op. Now: the hook
  `UnitHooks::player_corpse_pickup` returns whether §12.1 steps 1–2
  passed (`ActionHooks::corpse_pickup` returns `Option<i32>`, no longer
  calls a take-back; `Pending::corpse_take_back` removed); the 0x16
  type-0 handler then runs `items::moves::ground::corpse_pickup_rest`:
  §12.2 phases 1–2 (`corpse_take_back`), §12.3 slot fit
  (`items::inventory::corpse_slot_fit`, `pair_location`), then
  `MovePending::corpse_taken` (§12.1 step 4: corpse list, room, 0x8E,
  free — the desk forwards it to the rest, which models those) and sound
  93, or sound 23. New seams: `corpse_slot_fit`, `corpse_taken`,
  `replenish_timers` (desk: `InvDesk::schedule_replenish`). New fatal
  `MoveFatal::SlotHeld` (line 0x1775).
- `grid_put` (`0x005600A0`) factored out of §8.1 step 7 (with or without
  the room step).

Vendors / hirelings / session (`specs/world/vendors.md`, `vendors-2.md`,
`hirelings-2.md`, `formats/d2s-load.md`, `audio/sound-table.md`):

- §7.1 r2 (code differed): the 0x2A of "not offered" carries the
  requested GUID.
- §7.2 r8, §8.1, §9.2 (four), §5.1 (two), §3 (two): code agreed;
  markers replaced by spec statements.
- §8.1 r7 (V12): the repair routine returns its own results (1 for rules
  1, 2, rule 4's two refusals and repair-all's failed payment, 3, 0) and
  both handlers (`npc_vendors.rs`, `d2-server` world 0x35) drop it: 0 for
  every 17-byte message.
- §9.4 (code differed): a missing normal-code record is fatal
  (`PriceFatal::NoNormalRecord`); the normal code is `normcode` when ≠ 0,
  else `code`. `gamble_price` returns a `Result`.
- §5.1 step 4 (code differed): missing `rin` / `amu` → item 0 (`hax`).
- §3.1 r1 (code differed): an upgrade code missing from the map → class
  index 0; §3.1 r2 (code differed): a null creation is fatal
  (`PriceFatal::NullStoreItem`; `make_store_item`, `generate`,
  `store::open` return `Result`, mapped to `NpcError::Vendor`); V7: a
  code mismatch in round 2 also destroys the item and gives null.
- §7.2 r7 (code differed): the no-sell mask is 4 (`UNIQUE_NOSELL_MASK`,
  the `carry1` bit).
- hirelings-2.md §17 r2: `HirelingItems::duplicate` returns an `Option`;
  a failed duplicate skips the mode set, equips GUID −1, still consumes
  C and clears the cursor; a null old copy leaves the cursor none.
- vendors.md §7 "Message order" (code differed): the inventory messages
  of a vendor call now precede its 0x2A (`wired.rs` `take_sent`).
- §7.2 r9: the sell of a stored item sets stored page := page and sends
  S→C 0x9D action 5 (flags 0x20) before the removal
  (`vendor_inv.rs` `remove_stored`; new `InvError::Move`).
- d2s-load.md §8 r1–r3: `PlayerRecord::new_character(start_skill)`
  (+0x2C = 1, hand 0 = StartSkill or 0, items 0). Wiring it into the
  client's new-character join is the client's (single_player.rs:211,
  impl-triage-client's list).
- sound-table.md §2: the 13 `EAX …` columns (0x24–0x54) are bound.

PC 2's final list:

- **Two game-seed steps per copy** (§7.3): already true (each item
  allocation steps twice); now also for every filler read in step 5,
  checked by `children_and_the_fillers_argument` (2 · (1 + 1) steps).
- **Ladder cube recipes in single player (V17b)**: no code change
  needed. The cube's record gate (`world/cube.rs:738`, §4 test 3) passes
  a `ladder` row when game type ≠ 0, and the single-player app builds
  `GameFields` with `GAME_TYPE` 3 (`d2-client/src/app/single_player.rs`
  `GAME_TYPE`, used for the create message and the fields). Not
  re-checked against live record 104 (local run).
- **Corpse take-back**: see above.

## Changed test expectations (every one, with the rule)

| Test | Was | Now | Rule |
|---|---|---|---|
| `wiring::inventory::tests::host::remove_then_free` | freeing a linked item logs `FreedWhileLinked` | no error, the item is unlinked and freed | cube.md §8 "Exact" 1 |
| `wiring::inventory::tests::copy::children_and_the_fillers_argument` | fillers 1 → no copy, `Unwritten` logged | copy with its filler linked in mode 6; 4 game-seed steps | vendors-2.md §7.3 step 5 (provisional) |
| `world::vendors::tests::trade::buy_refusals` | "not offered" 0x2A GUID −1 (two cases) | the requested GUID | vendors.md §7.1 r2 (V10) |
| `world::vendors::tests::store::store_item_tries` | round-2 mismatch kept | destroyed, null; plus a null creation → `NullStoreItem` | vendors.md §3.1 r2 (V7) |
| `world::vendors::tests::trade::repair_one` | "nothing to repair" result 0 | 1 | vendors.md §8.1 r7 (V12) |
| `wiring::action::tests::player_death` (2 tests) | `corpse_pickup` returns 0 when refused; logs "take back" | `None` when refused, `Some(75)`; no take-back call (now the inventory's) | inventory-moves.md §12.1 |
| `d2-server …world::tests::vendors::buy_refusals_before_the_price` | GUID −1 | GUID 8 (requested) | vendors.md §7.1 r2 |
| `d2-server …world::tests::vendors::repair_refusals` | comment only (result 0 now by rule) | — | §8.1 r7 |
| `d2-server tests/mutants_handlers_world::sale_uses_the_npcs_own_record_and_client` | only the 0x2A | 0x9D action 5, then the 0x2A (one send) | vendors.md §7.2 r9, §7 "Message order" |
| `d2-server …player::tests::wired` (hot keys) | untouched slots `UNBOUND` | `NEW_RECORD` (skill 0) | intents-events.md §8.2 r3.6 (provisional) |
| `d2-sim/tests/game_monsters.rs::real_levels_rows` (ignored) | every Act 0 WarpDist 2025 | 2025 except (15, 3800), (20, 100), (21, 100), (23, 100), (25, 100) | population.md §8 / Real (live) |
| `d2-sim/tests/game_drlg_tables.rs::maze_defs_exist_in_live_lvlprest` (ignored) | every def Files ≥ 1 | every def exists; the Files-0 set is exactly {1, 2, 3, 167, 512, 514–516, 518–524} | live C12 run; preset.md Files 0 → file 0 without a draw |
| `d2-sim/tests/game_skills.rs::every_skill_function_in_table` (ignored) | referenced = `mapped` rows | referenced = `spec'd-here` ∪ `mapped` rows | use.md §8: no row is `mapped` any more; **the test was stale, not the table** |

New tests: `items::recharge` (2), `wiring::inventory::tests::corpse`
(3: take-back, permission, §12.3 rows), `world::hirelings::tests::items::failed_duplicate_loses_the_item`,
`d2-server adapters::session::record_tests::new_character_record`.

## PROVISIONAL (M22), wire / saved-layout ones first

Each is in code (`// PROVISIONAL (…)`) and in its spec (`PROVISIONAL:`).

1. **Wire** — bitstream.md §4.2, §4.1 r11 (`items/bitstream/read.rs`):
   prefix inverse p′ + P, auto affix a′ + A. Settled by: item-record
   capture with a prefix / auto affix near P / A.
2. **Saved bytes** — bitstream.md §4.6 r1 (`wiring/economy/item_records.rs`):
   decoded set list flags 0x2040. Settled by: save / copy round-trip of a
   set item.
3. **Wire** — intents-events.md §8.2 r3.6 (`d2-server …/player.rs`
   `HotKey::NEW_RECORD`, `adapters/sim.rs`): a new client record's slots
   are zero (skill 0). Settled by: new-character join capture
   (d2s-load.md OQ4). Note: `session::Entry::new` still uses its own
   `NO_HOT_KEY` (skill −1) for the join's 0x7B; the two hot-key models
   are not merged here.
4. vendors-2.md §7.3 step 5 (`wiring/inventory/copy.rs`):
   `0x00562660(…, 0, 1, 0, 0)` = link + mode 6 only. Settled by: bin read
   or a socketed-item copy recording.
5. sim/units.md §2 (`units/record.rs`): +0x64/+0x68/+0x6C allocated as
   the reset (−1, 6, inactive). Settled by: bin read of `0x00555230`.
6. combat/vitals.md §4.7 (`wiring/action/death.rs`): no corpse → client
   +0x508 unchanged. Settled by: bin read.
7. intents-events.md §9 r6 (`d2-server …/player.rs`): `0x0053FDF0` has no
   side effect. Settled by: bin read.
8. intents-events.md §9 r8: merc command params 3, 4 = 0, 0. Settled by:
   bin read.
9. generation.md §12.2 (`items/recharge.rs`, new): the set-charges list
   order is item list, then runeword list. Settled by: recharge capture
   of a runeword whose base has the same charged skill.

## Rows left, with reason

- `wiring/action/death.rs:17` (units.md §4.5, the rest of `0x00580EC0`
  and the character save of `0x0057FCA0`): triage gives no provisional
  choice ("none obvious"); needs the bin read / save capture.
- `d2-server …/player/action.rs:226` (pathing.md §1.2): triage says "d2rs
  wiring choice, no spec needed"; the marker became a plain note.
- `world/vendors.md` edge case 10 (`wired.rs` "TODO(vendors.md edge case
  10)", not a `TODO(spec` row): left.
- The recharge seams of the cube and vendor repair (`CubeRest::recharge`,
  `VendorRest::recharge`) still go to their rests; only the socketing
  recharge runs on the desk.
- §12.1 step 4 (corpse list, room, 0x8E, unit free) is the rest's
  (`MovePending::corpse_taken`); the desk does not model the corpse list.

## Gate

See the coordinator message / commit for the gate result.
