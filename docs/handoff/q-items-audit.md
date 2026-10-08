# q-items-audit: rule-by-rule audit of the item code (`claude/q-items-audit`)

> Audit session, 2026-10-08, from `staging` at `aefd2b82`. Read only
> `specs/`, `docs/`, `crates/`, `tools/`; no game files. Nothing here is
> verified against 1.14d (rule 10): the audit compares the code with the
> specs, not with the game. PROVISIONAL points marked in code (M22) were
> accepted unless the code contradicts the spec's chosen behaviour.

## Scope

Every numbered rule of the item specs, against d2-sim (logic), d2-server
(handlers, play host) and d2-client (item model, tool tips, grids), one
auditor per spec group:

| Group | Specs |
|---|---|
| Generation, drops | `items/generation.md`, `items/treasure.md` |
| Affixes, quality | `items/affixes.md`, `items/quality.md` |
| Bit stream | `items/bitstream.md`, `items/bitstream-legacy.md` |
| Inventory, moves, gold | `items/inventory.md`, `items/inventory-moves.md`, `ui/inventory.md` |
| Properties, use, tool tips | `items/properties.md`, `items/use.md`, `ui/item-tips.md` |
| Vendors, gamble, cube | `world/vendors.md`, `world/vendors-2.md`, `world/cube.md` |

Overall: the pure sim modules match their specs closely, including every
RNG draw site compared (affix roller, rare / crafted loops, quality chain,
sockets, ethereal, TC walk, vendor stock, gamble, transmute) and the item
bit layout (writer and both readers agree field by field). The
disagreements sit in three places: the client tool tip (built before
`ui/item-tips.md` existed), the client's click-to-intent mapping, and the
server play host's seams that are still stubs.

## Findings, most visible first

`Fixed` rows carry their test; `Queued` rows are build-queue rows
`q-fix-items-<topic>` (`docs/handoff/build-queue.tsv`).

| # | Sev | Spec rule | Code | Disagreement → effect | State |
|---|---|---|---|---|---|
| 1 | high | `world/vendors.md` §3.1 r2, §5.1 step 7; `items/generation.md` §10.2 | `d2-sim/src/wiring/interaction/vendor_world.rs` `create_item` | Store and gamble items were created with flags2 0 (ethereal roll allowed) and without item flag 0x10. In expansion ~5 % of store weapons / armour came out ethereal, the extra draw shifted every later roll on the item, and the store repair (needs identified) never ran, so throwing weapons kept random quantities | Fixed: `wiring::interaction::tests::vendors::store_item_creation_is_never_ethereal_and_identified` |
| 2 | high | `items/bitstream.md` §4.1 r4, edge case 9; `inventory-moves.md` §6.2 store check `0x0053EF30` | `d2-sim/src/wiring/inventory/bits.rs:74` (`alt: false`), `items/moves/seams.rs:545` (`store_messages` empty), `d2-server/.../world/wired.rs:385` | Gamble / store items are sent as full records (real code incl. exceptional / elite upgrade, quality, ilvl, rare affixes) instead of alt-code records; the tip colours the name by quality → the gamble window shows which items are unique / set / rare | Queued `gamble-alt-code` |
| 3 | high | `ui/item-tips.md` §7.2 f15 / f24 (+ f6–f14, f17–f28) | `d2-client/src/ui/item_tip_desc.rs:135`, `:147` | Layer split reversed (level = param>>6, skill = param&0x3F; spec: skill = layer>>6, level = layer&0x3F): every chance-to-cast and charged-skill line names the wrong skill and level; per-level stats show the raw value; several descfuncs differ. `each_descfunc_gives_its_line` asserts the swapped form | Queued `tip-descfuncs` |
| 4 | high | `ui/item-tips.md` §8, §7.1 r3, §6 r5 | `d2-client/src/ui/item_tip.rs:494-505` | No damage groups / dgrp: "Adds 10-16 fire damage", "+N% Enhanced Damage", all resistances / attributes show as separate raw lines | Queued `tip-groups` |
| 5 | high | `ui/item-tips.md` §2 A…R, §3, §4–§6, §10–§11 | `d2-client/src/ui/item_tip.rs:288-425` (`lines_of`), `:526` (`shop_marks`); `ui/panels/inv_items_socket.rs:81` | Line order and content differ from the spec (Unidentified under the name, Req Str above Req Dex, no Ethereal line, filler properties as separate blocks, runeword list green, no damage / speed / block lines, no failed-requirement red, store price at the bottom instead of the top `Cost:` line). Existing tests assert the d2rs-own layout | Queued `tip-builder` |
| 6 | high | `ui/panels-3.md` §29 r1–r5; `items/inventory.md` §4.3, §5.6 | `d2-client/src/ui/panels/inv_items.rs:532` (`equip_press`) | Body-slot click sends only 0x1A / 0x1D / 0x1C, never runs §4.3: no 0x1B, 0x1E, 0x21; a two-hander onto a free hand with a shield, or onto sword + shield, does nothing; arrows cannot stack onto a worn quiver; no "can't use this yet" speech | Fixed (q-fix-items-play): `ui::panels::inv_items::equip::tests` (E1–E4, Q4, Q5, §29 r2 / r5) |
| 7 | high | `inventory-moves.md` §10.1, §7.11 step 4, §7.12, §7.20, §8.1 | `d2-server/src/adapters/handlers/world.rs:979-1160` (`PreviewMoveRest`) | `rest_pile`, sounds, `quest_item_used`, `quest_tr2_used`, `reset_skills_stats`, `send_item_stat`, `book_count_changed` are empty in the play host: gold above the level × 10000 cap is destroyed, Token of Absolution does nothing, no 0x5D / 0x3E, no pickup sounds, tome counts do not update | Partly fixed (q-fix-items-play, `InvState::move_effects`): rest pile (G2), 0x3E, 0x5D with the chain, tome counts (`0x0055C070` as §5.5 add n). Still open: `tr2` resist list and `toa` reset (no play-host body of `add_resist_list`, `0x00570360`, `0x00570C80`), pickup / requirement sounds (event ids not written) |
| 8 | med-high | `ui/item-tips.md` §9; CLAUDE.md rule 7 | `d2-client/src/ui/item_tip_set.rs:59-127`, `item_tip.rs:411-423` | The client runs `d2_sim::props::set_bonuses` itself on seed {0,0} and lists every step whatever is worn; spec shows only switched-on partial lists and the server-sent state-165–170 lists | Queued `tip-set` |
| 9 | med | `items/affixes.md` §1 r1 with `bitstream.md` §4.2 | `d2-client/src/ui/item_tip.rs:307-309`, `:247` | Magic prefix / suffix names looked up by the wire id without the −1: every identified magic item shows the next row's name | Queued `tip-affix-ids` |
| 10 | med | `world/vendors.md` §7.1 r7–r9, §7.1.1, §7.2 r7 / r9, §9.2 r6 (B) | `d2-server/src/adapters/handlers/items/vendor_inv.rs:285-342` → `d2-client/src/app/rest.rs:276ff` | Buy / sell inventory seams are stubs: potions not to the belt, scrolls not into tomes, ammo not topped up, no auto-equip, equipped items unsellable, socket fillers missing from the price, bonus costs 0 | Fixed (q-fix-items-play): `mutant_tests::inv_vendors_*` (§7.1 r7, r8, r9.6, §7.1.1); bonuses from the stat lists (`VendorDesk::bonuses`) |
| 11 | med | `items/treasure.md` §3.6 | `d2-sim/src/skills/use_/bodies/b3_lvl12.rs:378-408` → `wiring/action/pending.rs:1286` (no-op) | Find Item uses up the corpse and its draws but never drops an item | Queued `find-item-drop` |
| 12 | med | `items/use.md` §1 r4–r5, §2, §4 | `d2-sim/src/wiring/inventory/town_portal.rs:28-50`; `d2-server/.../items/moves.rs:350` | A TP scroll / tome charge is consumed before the cast; a refused cast (town) still costs the scroll. No `pSpell` dispatcher, no failure reset. The REC-117 PROVISIONAL marker predates `use.md` | Fixed (q-fix-items-play): `InvDesk::dispatch_use` (§1–§4; entry 2 refuses in a town / level 136 before any cost), `wiring::inventory::tests::town_portal`, `use_town_portal_scroll_and_tome`. Entries 1, 3–11 keep their earlier answers (use.md OQ1) |
| 13 | med | `inventory-moves.md` §7.12 (vs §8.1) | `d2-sim/src/items/moves/handlers.rs:970` | The whole 0x21 merge runs only when the source has durability; quivers / keys never merge. §7.12 wording is ambiguous: re-read the branch, clarify the spec, then fix | Queued `stack-merge-durability-gate` |
| 14 | med | `inventory.md` §4.7 r2, §4.4 r4 | `d2-server/src/adapters/handlers/world.rs:1136-1147` | `quiver_kind` and `one_or_two_handed` always false: auto-pickup equips arrows without a bow; a barbarian cannot hold a 1-or-2-handed sword with a shield | Queued `preview-quiver-1or2` |
| 15 | med | `ui/inventory.md` §10 r4.3–r4.4 | `d2-client/src/ui/panels/inv_items.rs:384-390`, `:440-455` | Grid click facts stubbed (`stackable_onto`, `book_kind`, `cursor_scroll_kind`, `cube_has_room`, `ready`): no 0x21, no 0x29, dropping onto the cube always says "cannot" | Queued `client-grid-facts` |
| 16 | med | `ui/inventory.md` §10 r3.4 | `d2-client/src/ui/panels/inv_items.rs:149` (`fits_belt`) | Shift-click to belt uses a fixed potion list, not itemtypes `beltable`; scrolls are lifted instead of belted | Queued `client-beltable` |
| 17 | med | `world/cube.md` §8 step 3; `ui/panels-2.md` §20 r7 | `d2-client/src/ui/cube_ui.rs:161-165` | The Horadric animation plays on every transmute button release; spec: only when `hst ` / `qf2 ` lands on page 3 by S→C 0x9C | Queued `cube-horadric-anim` |
| 18 | med | `items/properties.md` §10.2 | `d2-sim/src/items/props.rs:686-712`, caller `wiring/inventory/queries.rs:247` | Runeword properties roll on the socketed item's seed and reset against its row; spec: the filler just inserted is I (its seed and row), the socketed item is the owner. Ranged runeword values differ | Queued `runeword-filler-seed` |
| 19 | low-med | `items/treasure.md` §7 r4, §9 r5; `generation.md` §6.1 r2–r3 | `d2-sim/src/wiring/economy/treasure_items.rs:67-78`; `treasure/walk.rs:90` | Drop requests carry no source unit: a `body` item gets file index 0 instead of the monster's class; the quest misc picker's monster test fails | Queued `drop-request-unit` |
| 20 | low | `items/treasure.md` §7 r2 | `d2-sim/src/wiring/economy/death.rs` ~263-290; `chest_drop.rs:158` | The free-spot search gets the start lookup's room, not the monster's (a fix needs `StartSpot` to look the room up itself; tried and reverted, TODO expanded) | Queued `drop-search-room` |
| 21 | low | `items/treasure.md` §5.4 step 2 | `d2-sim/src/wiring/economy/death.rs:81-104`; stale `TODO(treasure OQ7)` `treasure/walk.rs:37` | NoDrop never sees more than one player (no `players` command, living count fixed) | Queued `players-setting` |
| 22 | low | `bitstream.md` §5 r2; legacy §2 r2 | `d2-sim/src/items/bitstream/read.rs:137-148`; `wiring/economy/item_records.rs` | Save trailer u32s read and dropped; a non-zero third u32 fails the item | Queued `save-records` |
| 23 | low | `bitstream.md` Outputs, §2 r5, §4.1 r8, §4.3 r7 | `d2-server/src/adapters/character/save.rs:124`; `d2-sim/src/wiring/inventory/copy.rs:131`; `items/bitstream.rs:408-415` | Writer write-backs dropped on save and copy paths (and children's always) | Queued `save-records` |
| 24 | low | legacy §3 r6.4, 6.7, 8, 9.5; §4 r1; edge cases 1, 2, 6 | `d2-sim/src/items/bitstream/read.rs:349`, `:323-332`, `:172`; `item_records.rs:227` | Save reader accepts records the original fails (quality 0 / > 9, unknown set index, two stat ids 0) and fails some it ends quietly (no row / save bits 0); stat 194 set without the socket clamp | Queued `save-records` |
| 25 | low | `inventory-moves.md` §7.23 (MV4) | `d2-sim/src/items/moves/handlers.rs:1471-1482` | 0x61 take from the hireling: bad location → 2 (spec 3), no "no inventory → 3", failed unlink fatal (spec 2). `merc_item_gates` asserts the contrary value for location 11; left unchanged | Queued `merc-take-codes` |
| 26 | low | `inventory-moves.md` §7.9 step 3, step 5 | `d2-sim/src/items/moves/handlers.rs` ~641 | Missing other-hand item → refused (spec fatal; unreachable after §4.3 = 7). `0x0055FB10` is named a sound in §7.9 but does more in §7.23: spec clarification | Queued `merc-take-codes` (same file, same session) |
| 27 | low | `items/properties.md` §5 r4 / r9 vs §14 | `d2-sim/src/items/props.rs:350-358`, `:421-428` | Functions 11 / 19 with a skill outside the table write nothing; §14's same wording means "skill := 0, continue". Ambiguous, bad data only | Queued `prop-skill-oob` |
| 28 | low | `world/vendors.md` §3.1 r1–r2 | `d2-sim/src/world/vendors/store.rs:85`, `:99`; `vendors.rs:627` | Store upgrade tests the ubercode with `valid_code` (must be found); spec only ≠ 0 / spaces, unfound → class 0 → null item. No effect on 1.14d tables | Queued `store-upgrade-unfound-code` |
| 29 | low | `affixes.md` §12; `quality.md` §10; `properties.md` §14, §2 mode 4; `generation.md` §11 | `d2-sim/src/items/affixes.rs:133`, `quality.rs:46-47`, `:383`, `props.rs`, `create.rs` | Format-0 (version-0x47 save) branches not implemented, three TODOs say "unspecified" though the specs now answer them. Unreachable until the 0x47 import exists | Queued `format0` |
| 30 | low | `items/bitstream.md` §4.1 r4 reader; legacy §3 r2 | `d2-sim/src/items/bitstream/read.rs` alt branch | Alt-code record read back with ilvl 0, quality 0 (spec 1, 1) | Fixed: `items::bitstream::read_tests::an_alt_code_record_reads_level_1_quality_1` |
| 31 | med | `ui/item-tips.md` §4 r1, r3, r4 | `d2-client/src/ui/item_tip.rs` (new `name_color`) | Name colour: low quality always grey, ethereal ignored, tempered missing, special codes / runes not orange, broken not red. Rule 2 (unidentified store item) needs store state; noted in code | Fixed: `ui::item_tip::tests::the_name_colour_follows_quality_flags_codes_and_broken` |
| 32 | low | `inventory-moves.md` §7.10 r5 | `d2-sim/src/items/moves/handlers.rs` `swap_cursor_buffer` | 0x1F never cleared item flag 0x4000 on T or C, nor ran C's socket-filled 0x1 test: an item swapped in kept its red "switched off" tint | Fixed: `swap_cursor_buffer_clears_4000_on_both` |
| 33 | low | `inventory.md` §4.2 r3–r4, vector Q1 | `d2-sim/src/items/inventory/equip.rs` `stat_ok` | Dexterity also ran the stat-list link test; spec: strength only (asymmetry reproduced) | Fixed: `requirements_q1_dexterity_has_no_link_test` |
| 34 | low-med | `properties.md` §5 r8 / r9, OQ 5 | `d2-sim/src/items/props.rs` func 18 / 19 | Written through §4.2 (valshift, 0 skipped, range test); spec: plain set | Fixed: `items::tests::props::func18_func19_set_without_valshift` |
| 35 | low | `properties.md` §5 r3, §4.2 | `d2-sim/src/items/props.rs:330` | Function 7 took "owner present" (set bonuses) as a non-item target and skipped the base reset; I is always an item | Fixed: `items::tests::props::func7_set_bonus_on_item` |
| 36 | low | `treasure.md` §3.1 | `d2-sim/src/treasure/drop.rs:27` | Gate tested collision before the bone-wall fatal | Fixed: `treasure::tests::monster_gate_bonewall_before_collision` |
| 37 | low | `treasure.md` §3.1 collision word | `d2-sim/src/wiring/economy/death.rs` | A monster outside every room grid read collision 0 and dropped; with the path provider now reads 0x27 (no drop, seed unchanged). Without the provider the old read stays (TODO) | Fixed: `wiring::action::tests::death::a_monster_outside_every_room_grid_drops_nothing` |
| 38 | low | `generation.md` §12.2 "List order of the walk" | `d2-sim/src/items/recharge.rs` `LISTS` | Recharge searched the main list before the runeword list (outdated PROVISIONAL) | Fixed: `items::recharge::tests::set_charges_walks_the_runeword_list_first` |

Notes, not findings:

- `d2-client/src/app/items.rs:170-178` (`TableDecoder`) shifts grouped
  partner stats (18, 49, 51, 53, 55, 56, 58, 59) by `ValShift`; the writer
  and the server reader keep them unshifted (`bitstream.md` §4.6 r4.3).
  Equal only while those rows have `ValShift` 0: fold into
  `tip-builder`, or a game-file check.
- `quality.rs:21-39` duplicates `treasure/quality.rs:165` `ratio_row`
  (same result on 0/1 data); optional cleanup.
- `quality.md` edge case 5 (tried-array overflow) is not reproduced; it
  needs more than 10 `qualityitems` rows (1.14d has 8).
- `d2-client/src/app/synthetic_items.rs:390` comment says gold 1× for
  `row: 1024` (4×, `treasure.md` §5.7 step 7); smoke content only.
- Rule 7: no client code moves items or resolves recipes; the client's
  0x32 / 0x33 price field is never read by the server. The one client
  outcome computation is the set tip (#8).

## Rules checked and matching

- `generation.md`: §1.3–§1.5, §2, §3 steps 1–10, §4 r1–5, §5.2–§5.3,
  §6.1–§6.2, §7.1–§7.3 (vectors 3×4 → 6, 2×2 → 4), §8, §9 steps 1–6,
  §10.2–§10.3; draw order ethereal → sockets → automagic.
- `treasure.md`: §1.1–§1.6, §2, §3.2–§3.5, §4, §5.1–§5.7, §6 (matches
  `treasure-quality.tsv`), §7 r1 / r3, §8, §9 r2–5, §9.1.
- `affixes.md` §1–§11 and edge cases 1–7; `quality.md` §1, §3–§9,
  §7.1, §8.1, edge cases 1–4, 6, 7 (format ≥ 1, draw order and count).
- `bitstream.md` §1–§5 and edge cases 1–8, 10 (B1–B10 byte for byte);
  legacy at v = 0x60: §1 r1, r3, r4; §3 except #24; §4 r3.3, r3.4,
  r6–7; §5.
- `inventory.md` §1–§5 and edge cases 1–14; `inventory-moves.md` §6,
  §7.1–§7.24 except #13, #25, #26, §8–§12; `ui/inventory.md` §10 r1–r4
  decision logic, §5 r3, §1 r2.
- `properties.md` §1–§4, §5 (TSV functions 1–4, 8–10, 12–17, 20–24,
  36), §8.1, §9, §10.1, §11–§13, edge cases 1–5; `item-tips.md` §7.2
  f2–f5, f7–f10 value parts, f19, property sort.
- `vendors.md` §1–§9 except #10, #28; `vendors-2.md` §7.3, §10;
  `cube.md` §1–§9 and edge cases 1–15.

## No code found

- `bitstream-legacy.md` every version gate below 0x60 (IT-2, REC-44;
  `d2-formats` `VERSION_MIN = 0x5C` still accepts 0x5C–0x5F files, read
  with the 0x60 layout).
- `use.md` §1–§3 (#12); `properties.md` §14 (#29).
- `item-tips.md` §1 variant dispatch, §3.2, §3.4, §3.6, §3.7 r2,
  §3.8–§3.12, §5 name forms, §6 r1 / r3 / r6 / r7, §8–§11, edge cases
  1–9 (#3–#5, #8).
- `vendors.md` §3.4 level-up event list (no gameplay effect); store grid
  dimensions (no spec; preview 10×10 is REC-162).
- `inventory.md` §4.2 r6 out flags (client description, no spec yet);
  §5.6 client pre-check (#6).

## Environment

The properties auditor installed the client build packages of
`tools/cloud-setup.sh` (pkg-config, libasound2-dev, libudev-dev,
libwayland-dev, libxkbcommon-dev) through apt so `d2-client` builds in
this container; `cargo-nextest` was installed with `cargo install`.

## q-fix-items-play notes

- `d2-client --test smoke_combat` `*_fights_levels_dies_and_respawns`
  fails about one run in three on staging without this branch's
  changes too (a random class: the hire gate answers 0x2A code 9, the
  test wants 11). Not caused here; needs its own row.
