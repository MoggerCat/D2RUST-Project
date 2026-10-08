# Spec: Seams — d2-sim ↔ d2-server (intents in, messages out, tick order)

- **Status:** draft: a contract between existing owner specs, read on
  both sides of the code by the 2026-10-08 seam audit; partly checked by
  `crates/d2-server/tests/seam_sim_server.rs` (no game files); §2.2 has
  a known disagreement (finding F1 of the audit) with no test yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-server::{host, dispatch, seams, adapters::sim,
  adapters::session, adapters::handlers}`, `d2-sim::{game, tick,
  wiring::action, wiring::path}`
- **Related specs:** `sim/tick.md` §1, §3, `sim/intents-events.md` §1–§3,
  §7.8, §8, `sim/unit-order.md` §1, §7, `combat/vitals.md` §5.1,
  `client/model.md` §11, `seams/drlg-coords.md`

## Summary

`d2-server` owns transport, queues, buffers and the host schedule;
`d2-sim` owns every game rule. They meet in the `seams` traits
(`Intents`, `Tick`), implemented by `adapters::SimGame` over a
`d2_sim::game::Game` plus a world host whose seams collect the sim's
outgoing messages in outboxes (`Pending::send`, `take_sent`). This spec
states what crosses and in which order. It adds no behavior.

## Inputs

| Name | Type | Source |
|---|---|---|
| C→S message | bytes, size | client queues, drained per frame |
| host clock | u32 ms | `Clock` (never reaches the sim except as the object host tick) |
| transport client id | u32 | host |

## Outputs / state changes

S→C messages queued to client buffers, flushed after a tick.

## Rules

### 1. What crosses

| Value | Sim side | Server side | Space / unit |
|---|---|---|---|
| client | `units::ClientId` (record slot, `unit-order.md` §7) | `seams::ClientId` u32 | mapped by `SimGame::join`; the client list order is the sim's |
| unit | `UnitId` (sim handle) | (type u8/u32, GUID u32) on the wire | `find_unit(type, guid)`; GUIDs per type (`unit-order.md` §1) |
| message receiver | player `UnitId` (`Pending::send(player, …)`) | the client whose record holds that player | none: dropped (`intents-events.md` §3.2 rule 1) |
| position | path record, act sub-tiles (i32) | `seams::Pos` (i32), u16 on the wire | act sub-tiles (`seams/drlg-coords.md` §1) |
| frame | `Game::frame` (game +0xA8) | `Intents::frame` | ticks |
| level, act | u32 level id, u8 act | u8 level, u8 act on the wire | `levels.txt` id; act = `act_of_level` |

## 2. Contract

### 2.1 Frame order

One host frame: read the clock once, drain every queue in arrival order
(handlers run outside any tick, with the previous tick's frame number),
run at most one tick, and flush every client in client-list order only
if a tick ran (`tick.md` §1, `intents-events.md` §1 rules 1–3).
`Host::frame` is the only driver.

### 2.2 Per-client message order is production order

The messages one client receives in a flush are in the order the game
produced them (`intents-events.md` §1 rule 3, §3.2 rule 1): the drain's
handler messages first, in drain order, then the tick's, then the
vitals sync's, which end the batch (`combat/vitals.md` §5.1 rule 1). An
outbox is emptied into the client buffers after every handler and after
every tick; messages of different outboxes produced in one tick must
keep their production order, not be grouped by outbox.

### 2.3 Hand-built messages equal the TSV layouts

Every S→C message the sim wiring builds by hand (`map_reveal`,
`map_hide`, `assign_player`, `load_act`, …) has the bytes of its
`sim/server-messages.tsv` layout, the one the client decodes with
`d2-proto`.

### 2.4 One 0x03 for both callers

Game entry (`adapters::session::load_act`) and the act change
(`wiring::path::act_change::load_act`) build the same S→C 0x03 for the
same act, seed and object seed: u16@6 is the act's town level
(`client/model.md` §11 rule 1).

### 2.5 Facts the dispatcher reads are current

The point and unit parsers (`intents-events.md` §2.4 rules 3–4) read the
player's and target's act and position through `SimGame`'s staged facts;
`Intents::refresh_targets` brings live facts up to date before each
parse, and the tick copies path positions back after each tick. A
staged fact that is never refreshed is a stale copy of sim state.

### 2.6 Ownership

| State | Owner | Copy |
|---|---|---|
| Units, rooms, client records, DRLG | `d2-sim` `Game` + wiring hooks | none |
| Gate fields, player data +0x168, unit act / position for the parser | `d2-sim` | `SimGame` staged facts (§2.5) |
| Queues, buffers, receive lists, duplicate filter, tick driver | `d2-server` host | none |
| Hot-key slots (client +0x3DC) | `d2-server` `SimGame` | none |

## Constants & data dependencies

`TICK_MS` = 40, `FLUSH_MS` = 40 (`tick.md` §1, `intents-events.md`
§3.2 rule 3); layouts `sim/server-messages.tsv`.

## Randomness

None.

## Edge cases & original bugs

1. A message to a player without a client is dropped, never an error
   (`intents-events.md` §3.2 rule 1).

## Test vectors

| Input / seed | Expected output | Source (trace id) |
|---|---|---|
| 0x07 / 0x08 for (0x3A0, 0x388, 1), (0xFFFF, 0x7FFF, 136) | `d2-proto` decodes the same fields | `seam_sim_server.rs` |
| 0x03 acts 0–4 | both builders equal; u16@6 ∈ `TOWN_LEVELS` of the act | `seam_sim_server.rs` |

## Provenance

Read from the owner specs and both sides of the code by the 2026-10-08
seam audit (implementation session; no `re/`).

## Open questions

1. §2.2 inside a tick: the wired host groups the tick's messages by
   outbox (action, inventory, rest); settled by the fix queued as
   `q-fix-seam-tick-order` and a recording of a tick whose systems
   interleave.
