# Spec: Seam — server ↔ proto ↔ client messages

- **Status:** draft — contract written from both sides' code and owner
  specs (2026-10-08 seam audit, `docs/METHODS.md` M23); checked by
  `crates/d2-client/tests/seam_messages.rs` on synthetic values only, no
  recording yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-proto` (`generated`, `transport`, `s2c`),
  `d2-sim` / `d2-server` (S→C builders, C→S handlers), `d2-client::bridge`
  (`receive`, `dispatch`, `msg/*`, `intent`, hand-built C→S bytes)
- **Related specs:** `sim/intents-events.md` (§2, §3, §5, §8),
  `sim/server-messages.tsv`, `sim/client-messages.tsv`,
  `client/bridge.md`, `client/bridge-dispatch.tsv`, `client/model.md`,
  `client/msg-units.md`, `client/msg-stats-items.md`,
  `client/msg-skills.md`, `client/msg-ui.md`, `combat/vitals.md` §5

## Summary

Every game message crosses three pieces of code that are written
separately: the sender's byte builder (`d2-sim` or `d2-server` for S→C;
`d2-client::bridge` or `d2-client::ui` for C→S), the shared size rules and
generated layouts of `d2-proto` (from the two TSVs), and the receiver's
reader (`d2-client::bridge::msg` handlers, which read offsets by hand, or
the `d2-sim` handlers behind `d2-server::dispatch`). This spec names what
each side may assume about the other. It restates no layout: the TSVs and
the owner specs cited are the source; this spec only says which of them
both sides must agree on, and how the agreement is checked.

## Inputs

| Name | Type | Source |
|---|---|---|
| S→C message bytes | `&[u8]`, id at byte 0 | `d2-sim` / `d2-server` builders |
| C→S message bytes | `&[u8]`, id at byte 0 | `d2-client::bridge` (`intent::encode`, hand-built bytes) |
| Layouts and size rules | generated tables | `sim/server-messages.tsv`, `sim/client-messages.tsv` via `d2-proto` |

## Outputs / state changes

None of its own: the contract constrains the outputs of the owner specs.

## Rules

### 1. Contract table (per message family)

| Family | Ids (S→C unless noted) | Builder side | Reader side | Units crossing | State owner |
|---|---|---|---|---|---|
| Session | 0x00–0x06, 0x0B, 0xAF, 0xB0, 0xB2, 0xB4 | `d2_sim::units::messages`, `d2_server::adapters::session{,_flow}` | `bridge/msg/session.rs` (via `d2_proto::s2c::parse`) | flags, act index, seeds | server: game; client: `ClientWorld` session fields |
| Rooms in sight | 0x07, 0x08 | `wiring::path::place::map_reveal`, `units::messages::map_hide` | `session.rs` `room_sight` | room **tile** x, y; level id | server: room lists; client: client DRLG |
| Unit add / remove / place | 0x09, 0x0A, 0x15, 0x51, 0x59, 0xAC, 0x74, 0x8E | `units::messages`, `wiring::action::{switch,monster_add}`, `path::walk::messages` | `msg/units.rs`, `roster.rs` | unit type u8, GUID u32, **sub-tile** x, y | server |
| Unit modes (queued) | 0x0C–0x10, 0x4C, 0x4D, 0x67–0x6D | `path::walk::messages`, `monsters::mode_message`, `unit_update::skill_message` | `units.rs` `queued` (row table) | sub-tile x, y; velocity i16 | server; client predicts |
| Local vitals | 0x18, 0x95, 0x96 (bit-packed, LSB first) | `combat::vitals::sync`, `walk::messages::walk_verify` | `units.rs` `vitals` (`bits::BitReader`) | life / mana / stamina in **whole points** (stat >> 8); x, y sub-tile; dx, dy one byte each | server |
| Stats / experience / gold | 0x19–0x1F, 0x20, 0x9E–0xA2 | `d2_server::adapters::session::stat_message`, `vitals::sync::exp_message`, `hirelings::level::exp_delta_message` | `stats_items.rs` `local_stat`, `items.rs` `merc_stat` | raw stat values (no shift); the id chosen by value | server |
| Skills | 0x21–0x23, 0x7B, 0x93, 0x94, 0x99, 0x9A, 0xA3, 0xA5 | `units::messages` | `msg/skills.rs`, `ui_text.rs` `hotkey` | skill id u16 (0x7B: 12 bits + left bit 15) | server |
| Items | 0x3E–0x48, 0x7C, 0x7D, 0x9C, 0x9D | `items::moves::layouts`, item streams | `stats_items.rs`, `items.rs` | GUIDs; bit stream (`items/bitstream.md`, other seam) | server |
| UI / NPC / quests | 0x26–0x2A, 0x4E–0x50, 0x52, 0x58, 0x5A, 0x5D, 0x5E, 0x61–0x63, 0x77, 0x78, 0x89, 0x8A, 0x91, 0x9B | `world::{npc,quests,waypoints,cube}`, `hirelings::pets` | `ui*.rs` (outputs) | strings (cstr), string ids | server |
| Pets | 0x7A, 0x81 | `hirelings::pets` | `msg/pets.rs` | owner / pet GUIDs, class | server |
| C→S intents | 0x01–0x66 | `intent::encode` (generated) and hand-built bytes (`objects::interact`, `ui::panels::npc`, `check.rs` 0x5F, `update.rs` 0x4B, `belt.rs` 0x26) | `d2-server::dispatch` → `d2-sim` handlers | sub-tile points u16; unit type **u32** on the wire | client asks, server decides |
| C→S system | 0x67–0x70 | client front end, `session.rs` (0x6B), `ui_quest.rs` (0x69) | `d2_server::adapters::session_flow` | — | server |

## 2. Contract

### 2.1 One layout per id

Both sides read and write every fixed field at the offset, width and
signedness of the id's `layout` column (`sim/server-messages.tsv`,
`sim/client-messages.tsv`); all multi-byte fields are little-endian. A
builder whose argument order differs from the wire order (for example
`hirelings::pets::pet_action(…, pet, owner)` writes owner @5, pet @9)
must still put each value in its own field. A reader may read a narrower
part of a field only where its owner spec says so (0x9B u16@3 of u32@3,
`client/msg-ui.md` §15; 0x57 umod2 u8 of u16@10).

### 2.2 Units on the wire

Unit positions are sub-tile u16 (0x09, 0x0D, 0x0F, 0x10, 0x15, 0x51,
0x59, 0x67–0x6D, 0x95, 0x96, C→S 0x01–0x0F points, 0x5F); room positions
in 0x07 / 0x08 are room tile coordinates; unit types are u8 in S→C and
u32 in C→S; 0x0B and 0x15 at a join carry type 0 (player).

### 2.3 Sizes

Every byte string a sender emits has the length the id's size rule gives
on it (`d2_proto::transport::server_size` / `client_size`), including the
value-chosen ids (0x1D/0x1E/0x1F, 0x1A/0x1B/0x1C, 0xA0/0xA1/0xA2) and the
variable ones (0x26, 0x94, 0xA8, 0xAA, 0xAC, 0x9C, 0x9D). A chunk of
several such messages splits back into the same messages with nothing
discarded (`intents-events.md` §3.3).

### 2.4 Vitals units

0x18 / 0x95 carry life, mana, stamina as stat total >> 8 and the client
stores them << 8 (`combat/vitals.md` §5.4, `client/msg-units.md` §5 rule
2). dx, dy must mean the same direction on both sides: the server's
(X − path target) & 0xFF (`combat/vitals.md` §5.2) and the client's
(X + sdx, Y + sdy) (`client/msg-units.md` §5 rule 3) are both the 1.14d
reads (`0x00548760`, `0x0045DC50` / `0x0045DB20`): the client's point is
the path target reflected through (X, Y), not the target. A round trip
keeps dx, dy byte-exact; the client never recovers the target.

### 2.5 Join order

The server sends the §8.1 and §8.2 messages of `sim/intents-events.md`
in that order. The client's handlers need: 0x59 (player unit) before
0x0B (local player); 0x0B before any 0x19–0x1F (else fatal 0x9AA); 0x03
before 0x07 / 0x08 / 0x53 (client act); 0x15 placing the player before
0x04 (else fatal 0x527, `client/model.md` §7 rule 5). The client answers
0x02 with C→S 0x6B (`client/model.md` §7 rule 3), which
`d2_server::adapters::session_flow` takes as the join.

### 2.6 No refusals on the server's own messages

A message the server builds for a client is never unowned, dropped
(when its unit was announced), discarded or rejected by the client's
receive path (`client/bridge.md` §2, §6), for the model the server's
earlier messages built.

### 2.7 Client-built C→S bytes

Hand-built C→S bytes (not through `intent::encode`) have the size of the
id's `transport_size` and `handler_size` and the field offsets of the
`layout` column; the client routes ids below 0x67 to the game queue and
0x67–0x70 to the system queue (`client/bridge.md` §4).

## Constants & data dependencies

None beyond the two message TSVs and `client/bridge-dispatch.tsv`.

## Randomness

None.

## Edge cases & original bugs

- 0x7E: 1.14d leaves bytes 1–4 unwritten; d2rs sends zeros and the
  client reads none (`sim/intents-events.md` Edge cases).
- 0xA0 carries the old value, not the new one (`world/hirelings`
  edge case 7); the client sets it.
- 0x96 dx = 0x80 reads as +128 on the client (`client/msg-units.md` §5).

## Test vectors

| Input / seed | Expected output | Source (trace id) |
|---|---|---|
| join sequence built by the server builders (`seam_messages.rs`) | no refusal; local player placed at the 0x15 point; stat 6 = life << 8 | synthetic |

## Provenance

Read from both sides' code at `aefd2b82` and the owner specs cited; no
Ghidra, no recording. Every layout is the TSVs'.

## Open questions

1. (Settled, §2.4.) The sign of dx / dy in 0x18 / 0x95 / 0x96: both
   owner specs match 1.14d; the client point is 2·X − target.
