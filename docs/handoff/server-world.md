# Handoff: world intent handlers in `d2-server` — `claude/server-world`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: implementation from
clear specs, medium (METHODS M14). Branched from
`claude/bold-ptolemy-jvyvxy` at `1470723`. Repo only. Specs:
`world/npc.md`, `world/vendors.md`, `world/waypoints.md`,
`world/quests.md`, `sim/intents-events.md` §2.4, §3.2. For the
coordinator to fold into `docs/HANDOFF.md` / `docs/PLAN.md` (not edited
here).

## 1. State

**Implemented, unverified** (every spec involved is a draft; the module
behaviour was already unverified, this layer only routes it).

- The world C→S ids now reach `d2_sim::world` through
  `crates/d2-server/src/adapters/handlers/world.rs`. Each handler finds
  the client's player unit, hands the message to the module through the
  game's `WorldHost`, queues what the module sent to the receivers'
  clients (§3.2 rule 1: a player without a client gets nothing) and maps
  the module result (0–3) to `ResultCode`. No field check, refusal code
  or message layout is written here: all are the modules'.
- **Wired on real providers: 0x49 only.** The waypoint seam has a
  provider (`d2_sim::wiring::action::WaypointView` on `ActionSim`);
  `ActionWorld` hosts it with `WaypointData` and the arrival list. The
  NPC, vendor and quest seams (`NpcWorld` + `NpcVendors`, `VendorWorld`
  + `NpcLink`, the quests' `QuestRest`) have **no provider in `d2-sim`**,
  so on `ActionWorld` (and on the default `NoWorld`) their ids stay
  stubs (recorded in `SimGame::unhandled`, result 0, as before). Their
  handlers are tested on a seam fake with the real `NpcControl`,
  vendor trade functions and `QuestControl`.
- Tests: 25 new in `adapters/handlers/world/tests/` (`cargo test -p
  d2-server`: 68 pass). Every test runs one message through the real host
  frame (send → drain → tick → flush → receive) with `ProtoSizes`, so
  every S→C message is also split by `d2-proto`'s sizes.
- Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p
  d2-server -p d2-sim --all-targets -- -D warnings`, `cargo test -p
  d2-server -p d2-sim` (68 + 828), `cargo run -p depcheck`, `python3
  tools/spec_index.py --check`, `python3 tools/methods.py check`,
  `python3 tools/coverage.py --check` (0 errors).

## 2. World C→S ids and owners

Machine-readable in `handlers/world.rs` `WORLD_IDS` (checked against
`client-messages.tsv` by `world_ids_are_sim_handlers`, with a
perturbation).

| Id | Name | Owner spec | Status |
|---|---|---|---|
| 0x13 | InteractWithEntity | `npc.md` §2 (unit type 1) | implemented (NPC); type 2 → object-interaction spec, not written (`waypoints.md` §5.2 names it): stub; other types: stub |
| 0x2A | ItemToCube | `cube.md` | items session (`handlers/items.rs`) |
| 0x2F | InitEntityChat | `npc.md` §3 | implemented (NPC) |
| 0x30 | TerminateEntityChat | `npc.md` §3 | implemented (NPC) |
| 0x31 | QuestMessage | `quests.md` §7.3 | implemented (quests) |
| 0x32 | BuyItem | `vendors.md` §7.1 | implemented (vendors) |
| 0x33 | SellItem | `vendors.md` §7.2 | implemented (vendors) |
| 0x34 | IdentifyWithNpc | `npc.md` §6 | implemented (NPC) |
| 0x35 | Repair | `vendors.md` §8.1 | implemented (vendors) |
| 0x36 | HireMerc | `npc.md` §7.3 | implemented (NPC) |
| 0x37 | IdentifyGamble | `vendors.md` §5.5 | implemented (vendors) |
| 0x38 | EntityAction | `npc.md` §4 | implemented (NPC) |
| 0x3E | ActivateInifussScroll | `quests.md` §9.4 (partial) | **stub**: the handler's item checks (exists, the player's, same act) have no result codes and no `QuestWorld` item seam; `quests::read_clue` exists for after them |
| 0x3F | PlayAudio | none | stub |
| 0x40 | RequestQuestData | `quests.md` §6.2 | implemented (quests) |
| 0x44 | StaffInOrifice | none (Act II quest) | stub |
| 0x46 | MercInteract | none | stub |
| 0x47 | MoveMerc | none | stub |
| 0x49 | TakeOrCloseWp | `waypoints.md` §6 | implemented and **wired** (`ActionWorld`) |
| 0x4C | Transmogrify | `cube.md` | items session |
| 0x4D | PlayNpcMessage | none | stub |
| 0x4F | ClickButton | `cube.md` (0x17/0x18) | items session |
| 0x58 | QuestCompleted | `quests.md` §1.7 | implemented (quests) |
| 0x59 | MakeEntityMove | none | stub |
| 0x62 | ResurrectMerc | `npc.md` §7.4 | implemented (NPC) |

## 3. Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-server/src/adapters/handlers/mod.rs` | handler modules (one `mod` line per system; the items and skills sessions add theirs) | `sim/intents-events.md` |
| `crates/d2-server/src/adapters/handlers/world.rs` | `WORLD_IDS`, `system`; seam `WorldHost<D>` with the visitor traits `NpcCall`, `VendorCall`, `WaypointCall`, `QuestCall`; `NoWorld`; `WorldError` / `WorldFault`; `handle` (player lookup, module call, send routing, result mapping) | `world/{npc,vendors,waypoints,quests}.md`, `intents-events.md` §2.4, §3.2 |
| `crates/d2-server/src/adapters/handlers/world/action.rs` | `ActionWorld` (`WorldHost<ActionSim<X>>`: waypoints), seam `Outbox` (the sends of `Pending::send`) | `world/waypoints.md` §6, §7.1 |
| `crates/d2-server/src/adapters/handlers/world/tests/` | host-frame tests: `waypoints` (wired `ActionSim`, two-act synthetic DRLG), `npc`, `vendors`, `quests` (seam fake `fake.rs`), `ids` | Test vectors |

## 4. Edits outside the new files (registration)

- `adapters/mod.rs`: `pub mod handlers;`.
- `adapters/sim.rs`: `SimGame<D = Unspecified, W = NoWorld>` with a
  public field `world: W`; `with_events` needs `W: Default` (existing
  call sites infer `NoWorld`); `Intents` is implemented for
  `W: WorldHost<D>` and `handle` first calls `world::handle` (the stub
  path runs when it returns `None`); `Tick` for any `W`. No existing test
  changed.
- `crates/d2-server/Cargo.toml`: dev-dependency `d2-data` (typed records
  for the test fixtures).
- **No `d2-sim` change**: no accessor was needed.

## 5. Design points

1. **Why a second type parameter.** The waypoint provider lives in the
   event dispatch (`ActionSim` owns the units, stats and DRLG), while
   the world tables and lists (`WaypointData`, `ArrivalList`, later
   `NpcControl`, vendor records, `QuestControl`) are game state outside
   it. `WorldHost<D>` gets both, `&mut Game` and `&mut D`, per call.
   Its system calls are generic visitors (`fn npc<C: NpcCall>`) because
   the `d2-sim` functions take a generic `W: NpcWorld + …` (sized),
   not a `dyn` seam. The default methods return `None` (no system → stub).
2. **Sends.** Seams send by player unit; `WorldHost::take_sent` hands
   them back after each call in order and `handle` queues them to the
   client whose player is that unit. A sink error is a `WorldFault`.
3. **Fatal paths** (`NpcError`, `WaypointError`, `QuestError`,
   `PriceFatal`, sink errors) end the game in 1.14d; here they are
   recorded with `WorldHost::fault` and the handler returns
   `ResultCode::Malformed` (1.14d ignores the code, §2.2 rule 5).
4. **Vendor records.** `buy` / `sell` need the record of the message
   NPC's class: `handle` finds it among the host's records by class
   (one per `npc.md` §1 record). When the NPC is missing the trade
   functions refuse before reading the record, so an empty scratch
   record stands in; a class without a record reads as an empty record
   (`TODO(vendors.md design point 1)` at the site).
5. **Readings** (TODO-free, recorded here): 0x40's handler result is 0
   (the spec names no other; `0x00546040` returns nothing).

## 6. For the next sessions

- **Providers needed** before NPC / vendor / quest ids run in a real
  game: `NpcWorld` + `NpcVendors` (impl-npc.md §3 lists the owners),
  `VendorWorld` + `NpcLink` (impl-vendors.md §4), `QuestRest`
  (wire-economy). Once a wired host has them, implement its
  `WorldHost::npc` / `vendors` / `quests` like `ActionWorld::waypoints`;
  the handlers need no change. The fake in `tests/fake.rs` shows the
  quest wiring through the NPC seam (`quest_text_list`,
  `send_game_quests`, `quest_chat_end` on the game's `QuestControl`).
- `SimGame::tick` runs `Steps` (default `TickHooks`) rather than
  `ActionSim`'s `TickHooks` (rooms): unchanged here, worth a look when
  the wired host is assembled.
- The coordinator merges `handlers/mod.rs` with the items and skills
  sessions' `mod` lines; if they also add a type parameter or bound to
  `SimGame`, fold them into one host trait.

## 7. Checks to queue (local run queue, `docs/HANDOFF.md` §5)

None new: the recordings already queued by the module handoffs
(`impl-npc.md` §5 item 2, `impl-vendors.md` §7 item 3, `impl-world.md`
"Checks" item 3) should now be replayed through `Host` with these
handlers once the providers exist (compare S→C bytes, 0x2A bytes 3–6
masked, and order per frame).
