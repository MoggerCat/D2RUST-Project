# Handoff: end-to-end single player, remaining spec-owned steps — `claude/e2e-next`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Base: `main` at `edad871`. Repo only,
synthetic tables, fixed seeds, no game files (M09). For the coordinator
to fold into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited
here). Continues `e2e-single-player.md`, `e2e-combat-path.md` and
`e2e-vendor-host.md`.

## 1. State

**Wired, unverified** (M02): every spec on the path is a draft; this
session adds no rule and changes no `d2-sim` / `d2-server` file. The
single-player e2e (`crates/d2-client/tests/e2e_single_player.rs`, 3
tests, all pass) now runs on `SimGame<WorldSim<_>, TradeWorld<Rest>>`
(the drop-in of `e2e-vendor-host.md` §6) with Akara allocated beside the
player in the generated room, and the server's `ItemWorld` attached for
the cube. 26 recorded bridge frames (was 17), 26 server ticks.

| Frame | Step | C→S | Result | S→C (asserted) | Stops at |
|---|---|---|---|---|---|
| 2–15 | 1–5a: creation, level, join, cast, kill, drop | 0x0C | as before | none | as before (`e2e-combat-path.md`) |
| 15 | level-up from the kill's 100 experience | — | — | none | — runs: `vitals.md` §4.3 → §3: level 2, `nextexp` 1500, 5 stat points, 1 skill point, mana refilled to its max (4000); `level_up_event` seam logged last in the kill |
| 16 | 7: stat point | 0x3A strength ×1 | 0 | none | — runs: `vitals.md` §2 (`spend`: points 5 → 4, strength 0 → 1) |
| 17 | 5b: pick-up of the dropped gold (its GUID) | 0x16 | 0 | none | **stub** (no inventory / item-use spec, `server-items.md` §2) |
| 18 | 8: talk to Akara | 0x13 | 0 | 0x27 (40), 0x29 (97), 0x28 (103), in order | — (0x27 bytes 6–39 are the staged `encode_text_list`, 0x27 `partial`) |
| 19 | 9: chat | 0x2F | 0 | none | — |
| 20 | 10: trade → store | 0x38 action 1 | 0 | none | — store generated (1–3 bucklers, then the permanent cap; game seed stepped 2 per item; identified). 0x9C action 11 is the item spec's (`add_trade_inventory` stub) |
| 21 | 11: buy the store's cap | 0x32 | 1 | 0x2A code 9, GUID −1 | **`VendorRest::copy_item`** (`0x0055A2A0`, §7.1 rule 9.2) |
| 22 | 12: sell the player's buckler | 0x33 | 3 | 0x2A code 9, GUID −1 | **`VendorRest::copy_item`** (§7.2 rule 8) |
| 23 | 13: sell the player's cap | 0x33 | 0 | 0x2A kind 3 code 1, GUID, gold + price | runs (permanent code: no copy); removal is the `remove_stored` stub (inventory spec) |
| — | quests 0x31 / 0x40 / 0x58 | — | — | — | **marked step** (`TODO(after the quest-host merge)` in the run): the quest-host session adds `QuestCall` to `TradeWorld` |
| 24 | 14: cube put-in, cursor ring | 0x2A | 0 | none | — runs `cube.md` §2 (targeting reset, page 3, placement = staged inventory stub) |
| 25 | 15: cube transmute | 0x4F button 0x18 | 0 | none | — runs `cube.md` §1, §3, §8 (ring → amulet, sound 4, identified, page 3); the cube's *opening* is staged (item use, `cube.md` §10, no spec) |
| 26 | 6: waypoint travel | 0x49 | 0 | none | as before: same-act placement `0x00554EA0` (path spec) → no 0x0D |

The client counts the received 0x27, 0x28, 0x29 (1 each) and 0x2A (3)
as unowned (`bridge-dispatch.tsv`: every id TBD); no unit enters its
model. `same_seed_same_run` compares the whole transcript (now also the
player's level / stat points / strength / gold, the store rows with item
seeds and ACs, the NPC-control seed, the vendor and cube stub logs).
`other_seed_other_run` (M08): another game seed changes the monster
seeds, the NPC-control seed and the store; the wire is identical up to
the trade (frames 1–19) and every frame's result codes agree.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What |
|---|---|
| `crates/d2-client/tests/e2e_single_player.rs` | the e2e above: `TradeWorld` host, Akara, store / buy / sell, level-up and 0x3A, the cube on `ItemWorld` (`item_world`, `CubeRest`), quest marker |
| `crates/d2-client/tests/e2e_support/mod.rs` (new) | fixtures shared by the two `TradeWorld` e2e tests, moved out of `e2e_vendor.rs` unchanged: `Rest` (the NPC / vendor / quest rests: staged answers and a log), `item_tables`, `vendor_tables`, `monstats`, `equiv`, `tx`, `blank` |
| `crates/d2-client/tests/e2e_vendor.rs` | uses `e2e_support`; behaviour unchanged (4 tests pass) |

## 3. Fixes and signature changes

None. No file outside the test files above was changed.

## 4. Seams reached and fixture answers (never behaviour)

| Seam | Owner (unwritten) | Fixture answer |
|---|---|---|
| Akara's kind init / placement | monster spec (`ActionHooks::init_kind` → `init::type_init` not routed, `HANDOFF.md` §2 step 7b) | allocated with `allocate` in the player's room at (40022, 40022), mode 1, `InteractionState::add_npc` |
| NPC / vendor / quest rests (`Rest`) | unit / path / player / inventory specs | as `e2e-vendor-host.md` §2–§3 (distance 3, text list zeros, `copy_item` null, staged inventory, gold caps) |
| Order of the world systems' creation seeds | game creation (only `NpcControl::new` → `QuestControl::new` is written, §2 step 7b) | after `create_regions`, before the player's items |
| Player's items (buckler, cap) | inventory spec | made by `TradeWorld::with_economy` (game seed), held in the staged inventory |
| Pick-up 0x16 | inventory / item-use spec | stub |
| Cube opening (interaction type 4, cube GUID) | item-use spec (`cube.md` §10) | staged before 0x4F |
| Cube inventory calls (`ItemPending`) | inventory spec | staged list: place appends, remove drops; logged |
| Level-up fixture | — | `experience.txt` thresholds 0, 100, 1500 (synthetic), sorceress `StatPerLevel` 5, player max mana 4000 |

Blocked steps that stay stops (stub asserted): pick-up 0x16 (inventory),
the waypoint's same-act placement `0x00554EA0` (path / placement), the
item copy `0x0055A2A0` (items), inventory placement / removal, session
flow (`PendingSession`, `SimGame::join` before the link).

## 5. Findings

1. **The cube runs on a second unit world** (`HANDOFF.md` §7 J1).
   `SimGame::items` is the server's `ItemWorld` with its own `Units`,
   `StatLists`, `GameFields` (a second game seed) and `ItemStore`; the
   cube's box, ring and amulet are not in `ActionSim`'s unit records and
   their creation steps the item world's seed, not
   `ActionHooks::game_seed`. They share only the game's unit lists
   (GUIDs). Making `ServerCube` / `handlers::items::handle` generic over
   the economy's hooks and sourcing the economy from the wired host (as
   `TradeWorld::with_economy` does) would fix it; the cube parts
   (`CubeData`, `Staged`, `creation`, `ItemPending`) then need a home on
   the wired world host (`trade.rs`, quest-host's file this round).
2. **Three item stores in one game.** `DeathDrops::items` (drops),
   `TradeWorld::items` (store and the player's items) and
   `ItemWorld::items` (cube), each with its own `ItemTables`; an item
   made by one is unknown to the others' item reads (e.g. the dropped
   gold cannot be sold or cubed). One `ItemStore` + `ItemTables` per
   game is the fix (with finding 1).
3. **Interaction owners** (as `e2e-vendor-host.md` §4.1, now visible in
   one run): after the talk, `Rest::interact` holds Akara; the waypoint
   step's interaction is `TestPending::interact`, and the cube's is
   `ItemWorld::staged.interactions` — three copies of player +0x64/+0x68.
4. Level-up `level_up_event` comes after the kill's last step
   (`BarricadeDoors`), as C2 of `e2e-combat-path.md` places the
   experience.

## 6. Questions

- N1: where in game creation the NPC-control and quest seeds are drawn
  relative to the regions and the object-control seed (§4 row 3; settles
  the monster seeds of this run against a recording).

## 7. Local checks to queue

None new runnable now. When the trade recording replay
(`wire-interaction.md` §8 item 1) and the sorceress kill recording
(`e2e-combat-path.md` §6 item 2) exist: replay both through this e2e
(`cargo test -p d2-client --test e2e_single_player` with recording-backed
seams) and compare S→C bytes per frame (0x2A bytes 3–6 masked) and the
level-up stats after the kill (expected: equal; the 100-experience
level-up threshold here is synthetic, the real `experience.txt` needs
500).

## 8. Gate

`cargo fmt --all -- --check`: ok. `cargo clippy --workspace
--all-targets -- -D warnings`: ok. `cargo test -p d2-client`: 205 + 3 +
3 + 4 passed, 5 ignored (no public item changed, so no workspace run is
required). `cargo run -p depcheck`: OK (8 crates). `python3
tools/spec_index.py --check`: ok. `python3 tools/methods.py check`: 21
methods OK. `python3 tools/coverage.py --check`: 3195 claims, 0 errors;
`--selftest`: ok.
