# play-server (S-C2 Server): first playable build, server half

> Implementation session, 2026-10-07, branch `claude/play-server`.
> Read only `specs/`, `docs/`, `crates/`, `tools/`. Plan:
> `docs/handoff/first-playable-scope.md` (gaps G14, G15 send half, G17,
> G18). Nothing here is verified against 1.14d (CLAUDE.md rule 10).

## 1. What changed

| Gap | Change | Where | Spec |
|---|---|---|---|
| G15 (send half) | The join sends the player's stat messages twice: after 0x5F (rule 3.4) and after the two 0x23 (rule 3.8). Each is the mod-array flush `0x006258D0` (`StatLists::mod_values`, key order, layer dropped) through `0x0053BE40`'s size choice: unsigned v < 0xFF → 0x1D, < 0xFFFF → 0x1E, else 0x1F; stat > 0xFE sends nothing (the fatal assert). | `d2-server/src/adapters/session.rs` `stat_message`, `stat_messages`, `enter_game` | `sim/intents-events.md` §3.5 r7, §8.2 r3.4 / r3.8; `sim/stat-lists.md` §11 r2 |
| G14 | Start items `0x00534F10`: the §10.3 algorithm (`character::start_items` over the `StartItemWorld` seam: slot / count order, a code not found ends its slot, flag-0x40 list dropped, `StartSkill` stat 107 on slot 0, stack fill, start flag 0x20000 + body location, step 2.6 belt / equip / inventory placement with the quiver 250 / 100 and durability fills) and its wired-host provider `WiredWorld::start_items` (creation via the quest reward's `create_reward`: ilvl the player's base level, quality 2, mode 4; placement on an `InvDesk` per call). | `adapters/character.rs` (`StartSlot`, `start_slots`, `StartItemWorld`, `start_items`, `StartPlace`); `adapters/handlers/world.rs` (`StartItems`, `WiredWorld::start_items`, `WiredStart`) | `items/generation.md` §10.3 (§10.1, §10.2 through `create_reward`) |
| G14 | `session::load_new_character_with_items`: the stub load, then the start items on the wired host; the report's "start items" unapplied entry goes away when they ran. Start stats and items are the only player steps before the start-skill selection and nothing between them draws, so the game-seed order of §10.3 holds. | `adapters/session.rs` | `formats/d2s-load.md` §1 r1 |
| G14 seam | `PreviewMoveRest` + `preview_inv_parts(InvTables)`: an inventory model for the play host. The play app builds `WiredWorld` with `inventory: None` and the repo had no non-test `MoveRest`; this one answers the `InvRest` open points with preview fills (D1), every one `d2rs-own, unverified`. | `adapters/handlers/world.rs` | D1 (`docs/PLAN.md`) |
| G17, G18 (server half) | `act1_stream.rs`: on the Act I-shaped synthetic set, a town NPC preset (DS1 type 1) spawns and its S→C 0xAC reaches the client; walking out of the town into the Blood Moor streams rooms of both levels (0x07 / 0x08) and the Blood Moor's monsters' 0xAC; two runs give the same transcript. | `crates/test-fixtures/tests/act1_stream.rs` (new) | `sim/intents-events.md` §7.8, `drlg/rooms.md` §3.3 / §4, `monsters/population.md` §2.3 / §3 |

Tests added: `session.rs` (stat message sizes, mod-array order), `character.rs`
`start_items_tests` (slot order, fallbacks, quiver quantities on a fake),
`test-fixtures/tests/start_items.rs` (wired host on the synthetic install:
`sb1` equipped at `rarm`, two `pt1` in the inventory, flags, durability,
the missing-inventory and no-model cases, determinism, the session entry
on `preview_inv_parts`), `act1_stream.rs` (above). Updated:
`synthetic_game.rs` `a_stub_starts_a_new_character_before_the_join_sequence`
now expects the stat messages between 0x5F and the two 0x23 and again
after them (same bytes both times; strength among them).

Fixture fix: the synthetic `bodylocs` row 0 had code `none`; 1.14d
registers the empty key there (`data/txt-format.md` §8 "Empty keys"), so
an empty `itemNloc` must link to 0. It linked to −1 (255) and the start
items "equipped" potions at location 255. Row 0 is now the empty code
(`test-fixtures/src/content.rs`). `test-fixtures`, `d2-server`, `d2-data`,
`conformance` tests pass with it (except the pre-existing failure, §5).

## 2. Gap ids closed (server side; nothing verified live)

- **G15 send half**: done from the spec (the 0x1D / 0x1E / 0x1F choice was
  answered 2026-10-08 in `intents-events.md` §3.5 r7; no `d2rs-own` fill
  needed).
- **G14**: done on the server, **not yet called by the app** (seam §3.1).
- **G17 / G18**: server half checked on synthetic data (CI). The live
  check needs the user's files (§4) and G1 (live DRLG room panic, owned by
  play-drlg).

## 3. Seams other sessions must connect

1. **S-D (`crates/d2-client/src/app/single_player.rs`)**, new character:
   - when building the `World` (`WiredWorld<AppRest>`), set
     `world.inventory = Some(d2_server::adapters::handlers::world::preview_inv_parts(InvTables::from_fixed(&fixed)?))`
     (`d2_sim::items::inventory::InvTables`; the same `FixedSet` the
     item tables come from);
   - at `single_player.rs:1294` replace
     `load_new_character(s, player, r.char_name)` with
     `d2_server::adapters::session::load_new_character_with_items(s, player, r.char_name)`;
     it returns `(entry, report, items)`: log `items.faults` like the
     report's unapplied steps. `items.sent` holds the inventory
     placements' queued messages (0x9C / 0x9D); the join does not send
     item messages yet (rule 3.5 unwired, item stream decode is G16),
     so dropping them is fine for the preview.
   - Without the inventory model the start items stay unapplied (one
     fault, no draw), exactly as before.
2. **S-D (`ui/original.rs`)**: the character panel's base values arrive
   as S→C 0x1D / 0x1E / 0x1F (stat u8@1, value @2) at the join; for a new
   character: the start stats' `Saved` base values (str, dex, vit, ene,
   level, …; 0–15 except 6, 8, 10, 13, 14). Life / mana / stamina come
   with 0x95; gold and experience with their own messages (rule 3.9).
3. **Shared synthetic set (`test-fixtures/src/content.rs` monstats)**:
   `isSpawn` is empty on every row, so `population.md` §2.3 draws no
   region entry and no monster ever spawns in any synthetic game.
   `act1_stream.rs` sets it on its own copy. Setting it in the shared set
   would change other tests' transcripts (not done here).
4. **play-drlg (G1)**: nothing in these changes touches `d2-sim::drlg`.

## 4. Local checks for the user (game files)

Run after S-D connects §3.1, on the 1.14d install
(`D2_GAME_DIR` set, `docs/LOCAL-RUN.md`):

1. `RUST_LOG=info cargo run -p d2-client --release -- play --new amazon Test --frames 3000`
   - look for: no `join: new character: Unapplied { step: "start items"`
     line; no start-item fault line;
   - the log's server message trace (if on) shows, after `5F`, a run of
     `1D` / `1E` messages, then `23 23`, then the same run again, then
     `95`.
   - With the inventory panel (I) still empty (G16 deferred) the items
     are only visible server side; a debug dump of the player's
     inventory should list the class's charstats `item1`…`item10` rows
     (beltable ones in the belt, rows with a location equipped, the rest
     in the inventory), each with item flag 0x20000.
2. Same run with `--save X.d2s`: the join sends the stat messages only
   for the stats the load's base writes put in the mod array; count them
   against a recording (REC join: `intents-events.md` §8.2 "Recorded seq
   113–141: … 8 stat messages …").
3. G17 in town: `RUST_LOG=info … play --save X.d2s --frames 3000`:
   model units in town > 20; Akara, Kashya, Charsi, Gheed, Warriv, Cain
   and the other NPCs present (0xAC for each), as the scope doc's
   "S-C alone" row.
4. G18: walk east out of the camp: new rooms stream (0x07), Blood Moor
   monsters (Fallen, Quill Rats, Zombies) arrive as 0xAC; no server
   panic (needs G1).

Nothing above counts as done until compared with 1.14d (rule 10); queue
the compare in `docs/HANDOFF.md` §5 when the run passes: a new-character
join recording (REC-02 already lists the new-character runs) for the
stat-message and start-item bytes.

## 5. What's left / open

- `0x0055E9B0` belt placement is read as free-slot search + slot place
  (`d2rs-own, unverified` in `WiredStart::place_belt`); the item's x is
  not read.
- A creation refusal (code found, `create_reward` returns none) ends the
  slot like a missing code; §10.3 states only the not-found case.
- The player's inventory is added on first use by `start_items` (the
  play host never adds it; `0x0063ABD0` at unit allocation has no caller
  there), `d2rs-own, unverified`.
- The per-tick mod-array flush (`tick.md` §6.5) and its clear
  (`stat-lists.md` §11 r3) have no caller yet; only the join sends stat
  messages.
- Pre-existing, not from this branch: `cargo test -p d2-server --test
  world_data_tables` fails 3 tests on the base too (`string.tbl` /
  `monstats.bin`: "failed to fill whole buffer" while loading the
  synthetic install; looks like tests sharing one install directory).
- `d2-client` could not be built in this container (`wayland-sys` build
  script panics: no system Wayland library), so its tests did not run
  here. My changes to its inputs: the additive `d2-server` APIs and the
  synthetic `bodylocs` row 0 (the client's synthetic-data tests,
  `bridge/msg/tests_units.rs` and `app/single_player.rs` tests, should be
  checked in the full gate).
