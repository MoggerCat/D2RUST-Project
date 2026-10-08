# q-proto-audit: one byte layout per client↔server message

Session q-proto-audit, 2026-10-08, branch `claude/q-proto-audit` (from staging `aefd2b82`).
Method: METHODS M22 (wire byte layouts spread silently), M23 (contract checks at seams),
HANDOFF §8 lesson of 2026-10-08. No game files.

- Index: `docs/handoff/proto-index.tsv`, one row per (id, direction): 181 S→C (0x00–0xB4) and
  113 C→S rows, with spec §, every producer (d2-sim / d2-server builder), the d2-proto type,
  the server path, the client handler or encoder (file:line), per-side layout notes and the
  contract test.
- Contract tests: `crates/d2-client/src/bridge/msg/tests_proto_{a,b,c,d,e}.rs` (S→C
  0x00–0x2C, 0x2D–0x66, 0x67–0x9B, variable + 0x9C–0xB4, all C→S). 112 tests. Each takes the
  d2-sim / d2-server builder's bytes (where one exists) over edge values plus a fixed xorshift
  sweep, asserts them equal to the `d2_proto` encode, decodes them, and drives the real client
  handler through the spec dispatch (`receive_chunk` / update pass), checking the
  `ClientWorld` or output effect. C→S: client encoder bytes == `d2_proto::client` encode ==
  what `d2_server::dispatch` and the d2-sim parsers read. 2 are `#[ignore]`d with a queue id
  (P1, P6); both fail when run.

## Headline

**No producer/consumer pair disagrees on the wire.** Wherever a sim or server builder and
a client handler both exist, their offsets, widths, endianness and bit order agree with
each other and with the TSV layout. That includes the bit-packed 0x18, 0x95 and 0x96
(LSB-first, widths 8/15/15/15/7/7/16/16/8/8), the 0x9C/0x9D item-stream header, and the
0xA8/0xAA/0xAC tails. The visible problems sit one level up: what a field *means* (P1),
messages nobody sends (P2–P4), and one C→S byte (P5, fixed).

## Findings, most visible first

| # | Ids | Sev | Finding | State |
|---|---|---|---|---|
| P1 | 0x18, 0x95, 0x96 | high (suspected) | **dx/dy sign is mirrored between server and client specs.** Server `combat/vitals.md` §5.2 l.564: dx = (X − path target) & 0xFF (`d2-sim wiring/action/vitals_sync.rs:66-70`). Client `client/msg-units.md` §5 r3: tx = x + sdx, used as the target by `client/model.md` §6 r5 (`d2-client bridge/msg/units.rs:914-925`). The client rebuilds 2X − target, so the rule-5 "closer to the target" acceptance fails and rule 8 snaps to the server point: rubber-banding. Example (ignored test): client at (100,100) on its target, server point (104,100): client sends correction `5F 64 00 64 00`. Recordings B 15933 / 32694 lack the path target, so they cannot settle it. | QUEUE `q-fix-proto-vitals-dx-sign`; test `tests_proto_a::life_mana_update_0x18_dx_is_the_path_target_offset` (ignored) |
| P2 | 0x3E | medium | **UpdateItemStats is never sent.** Every `send_item_stat` seam is a no-op or log (`d2-sim items/moves/seams.rs:537`, `wiring/action/pending.rs:1256`, `d2-client app/rest.rs:324`); no spec gives the server-side width choice of `0x0053D130`. Stack quantity, durability and charges after buy/stack/repair/recharge never reach the client: stale shop and inventory values. | **FIXED** (q-fix-proto: `update_item_stat`, PROVISIONAL REC-336) |
| P3 | 0x22 (also 0x11, 0x20) | medium | **TSV `produced_by=sim`, no producer.** 0x22 seam `send_skill_quantity` (`items/inventory/bookkeeping.rs:118`) defaults to no-op (`wiring/inventory/mod.rs:288`): tome / scroll quantities never update on the client. 0x11 (`intents-events.md` §7.9 r9) and 0x20 (`0x0053C1D0`) have no builder. | 0x22 **FIXED** (q-fix-proto); 0x11, 0x20 still QUEUE `q-fix-proto-missing-producers` |
| P4 | 0x5B, 0x5C, 0x65, join 0x5A | medium | **Join/leave roster messages not sent** (`intents-events.md` §8.3, §2.5 r2): only the leave 0x5A code 3 is sent (`d2-server adapters/session_flow.rs:408`). The client roster stays empty in single player, so roster life updates from 0x0D / 0x82 / 0x8E find no record. | **FIXED** (q-fix-proto: join sequence 0x5B / 0x65 / 0x5A, leave 0x5C; PROVISIONAL REC-337) |
| P5 | C→S 0x30 | medium | **Shop close sent 0x30 with u32@1 = 0**; every other interaction end sends `[u32 1][u32 G]` (`client/model.md` §17 r1 step 5, `ui/panels-2.md` §14 r5). Server reads only u32@5, so no mis-parse, but the bytes differed from 1.14d. | **FIXED**: `d2-client ui/panels/shop.rs` `close_intent` and `ui/shop_ui.rs` now send `npc::msg_chat_end`; `shop_close_sends_0x30` expects byte 1 = 1; `tests_proto_e::shop_close_0x30_one_layout` un-ignored |
| P6 | 0x50 | medium (latent) | `d2_proto::s2c::parse` refuses every QuestSpecial code but 1 (`s2c/parse.rs:88-91`), though d2-sim sends codes 2, 4, 13, 23 whose layouts the TSV gives. Client reads raw bytes, so play is unaffected; trace comparison through `parse` would refuse them. | **FIXED** (q-fix-proto: `QuestSpecialForm`; test un-ignored) |
| P7 | 0xA3, 0xA4, 0xA5, 0xA6, 0xAB | medium | Never sent: d2rs keeps no pending event records (`0x00571CD0`, §7.9 r2); seams are no-ops (`wiring/action/pending.rs:629, 1275`). Client handlers match the d2-proto encodes. | 0xA3, 0xA4 **FIXED** (q-fix-proto: `event_records`); 0xA5, 0xA6, 0xAB still QUEUE |
| P8 | 0x57 (0x40) | medium | NpcEnchants not sent (`wiring/action/unit_update.rs:97`, step 10); 0x40 has no spec of its sender `0x0053D270`. | 0x57 **FIXED** (q-fix-proto); 0x40 still needs a spec |
| P9 | 0x92 | medium (latent) | Client handler only checks the size (`bridge/msg/items.rs:169`); `msg-stats-items.md` §5 r5 specifies a full refresh. No producer yet. | QUEUE `q-fix-proto-0x92-client` |
| P10 | 0x23 | medium | **`q-fix-set-skill-fatal` is not a layout problem**: layout agrees on all sides. The client's skill table lacks rows: `app_client_drlg.rs:242/283` binds only `levels`; synthetic play binds one row (`app/synthetic_client.rs:24`); `app/play.rs:151` `add_client_data` replaces `ClientTables` and wipes rows set earlier. Test `tests_proto_a::set_skill_0x23_fatal_0x668_is_the_missing_skill_rows`. | folded into existing row `q-fix-set-skill-fatal` |
| P11 | 0xA8, 0xAA | low | State-stat param written unsigned (`d2-sim units/messages.rs:262`), read signed by the client per `client/stat-lists.md` §3 r1 (`bridge/msg/states.rs:147-148`); 0xAC reads the same kind of field unsigned. Both follow their specs; needs RE confirmation. | QUEUE `q-fix-proto-state-param-sign` |
| P12 | 0x95, 0x96 | low | Sim cuts over-wide values to field width (§5.4); `d2_proto::schema::packed_put` (`schema.rs:224`) panics instead. No sender uses the typed encoder today. | **FIXED** (q-fix-proto: `packed_put` cuts) |
| P13 | many | low | Second layouts that can drift: two d2-proto types per id (0x0B, 0x0D, 0x28, 0x29, 0x2A, 0x4E, 0x50, 0x52, 0x58, 0x59, 0x5D, 0x63, 0x77, 0x89, 0x8A, 0x91, 0x9B; hand-written 0x28 pins byte 6 = 0 where the TSV and client leave it free); duplicate byte makers (0x1D–0x1F in sim and server, 0x21 inline with bonus 0, 0x0B / 0x05 / 0x06 / 0x28 / 0x29 / 0x27 inline); no typed codec for 0x26 and the variable ids (0x3E, 0x94, 0x9C, 0x9D, 0xA6, 0xA8, 0xAA, 0xAC, 0xB3); two client item-header readers. All byte-identical today (tested). | QUEUE `q-fix-proto-one-type` |
| P14 | C→S 0x3E, 0x4C, 0x59, 0x44, 0x13 (types 0, 4) | low | Client sends, server stubs (returns 0). 0x4C is documented as routed to items (`handlers/world.rs:181`) but `handlers/items.rs:234` takes only 0x2A and 0x4F. | 0x4C marked NoOwner (q-fix-proto); the others stay stubs |
| P15 | C→S 0x15 | low | §2.4 r6 string check not implemented (`d2-server dispatch.rs:257`); nobody sends 0x15. | **FIXED** (q-fix-proto; PROVISIONAL REC-338) |
| P16 | docs / TSV | low | 0x96 "never sent" in `d2-server adapters/handlers/walk.rs:22-23` and HANDOFF §1 3ag (the vitals sync sends it); 0x5A TSV lacks `account:cstr16@24` (§8.3); 0x50 client reads a sixth word (`msg-ui.md` §7) the TSV does not list; 0x27 "partial" comment in `world/npc.rs:450`; 0x18 TSV names `life_pred`/`mana_pred` vs `a`/`b`; 0x26 u32@5 `on_merc` vs `shift`; 0x7F and 0x90 share TSV sender `0x0053CDF0`. | QUEUE `q-fix-proto-docs` |
| P17 | notes | — | Faithful, no action: dx = −128 reads as +128 (`units.rs:860`); narrower client reads of 0x9A skill and 0x9B cost (specced); 0x5D/0x65/0x53/0x57 signed or narrower reads per spec; 0x32 mode ≥ 0x8000 sets the fill bit; 0xB3 min size 8 vs a 7-byte len-0 message; 0xAC size byte would wrap past 242 stream bytes (max today ~186); Pong 0x8F never sent, so the client's ping tolerance uses L = 0. | — |

### Rubber-banding and shop, from the protocol side
- Rubber-banding: no C→S layout mismatch (0x01–0x04, 0x5F agree byte for byte). Suspects: P1
  (sign), and the staged player position the point parser checks against
  (`d2-server adapters/sim.rs` `point_state`, refreshed by `refresh_targets` →
  `WorldHost::live_facts`, default `None`): if stale, in-range walks are refused and after 25
  frames S→C 0x15 is queued. Check: log `PointState.player` against the path position on each
  refused 0x01 / 0x03 in `play_smoke`.
- Shop: no layout mismatch in 0x13, 0x2F–0x38, 0x62; P2 (stale item stats) and P5 (fixed).

## Queue ids

The part details below proposed some ids that the table merges: `q-fix-proto-walkverify-dx-sign`
→ `q-fix-proto-vitals-dx-sign`; `q-fix-proto-missing-builders` → `q-fix-proto-missing-producers`;
`q-fix-proto-dup-types`, `q-fix-proto-dedupe-builders`, `q-fix-proto-chat26-codec`,
`q-fix-proto-variable-types` → `q-fix-proto-one-type`; `q-fix-proto-doc-a`,
`q-fix-proto-quest-special-word6`, `q-fix-proto-event-account` → `q-fix-proto-docs`;
`q-fix-proto-chat-end-flag` is done (P5); `q-fix-proto-ac-size-assert` is not queued (P17).
The rows are in `docs/handoff/build-queue.tsv`.

## Detail, part a

All 45 ids covered; nothing disagrees on the wire. 0x18 bit order and widths match on all three sides: sim `put_bits` (`crates/d2-sim/src/combat/vitals/sync.rs:154`), d2-proto `packed_put`/`packed_get` (`crates/d2-proto/src/schema.rs:204`), client `BitReader` (`crates/d2-client/src/bridge/bits.rs:31`): LSB-first from bit 0, widths 8, 15, 15, 15, 7, 7, 16, 16, 8, 8 (115 bits; bits 115–119 never written). 28 tests pass, 1 ignored (A1).

#### A1 0x18 (also 0x95/0x96) dx/dy sign (QUEUE q-fix-proto-vitals-dx-sign)
- Server spec `specs/combat/vitals.md` §5.2 (l.564–565): dx = (X − path target x) & 0xFF; producer `crates/d2-sim/src/wiring/action/vitals_sync.rs:66-67` `x.wrapping_sub(p.target_x) as u8`, on the wire via `combat/vitals/sync.rs:154` `life_mana_update`.
- Client spec `specs/client/msg-units.md` §5 r3: tx := (x + sdx) & 0xFFFF, the target of `client/model.md` §6 r5; consumer `crates/d2-client/src/bridge/msg/units.rs:923-925`.
- The client rebuilds the mirrored target 2X − target. Ignored test: client (100,100), server point (104,100), path target (100,100): the client sends `5F 64 00 64 00` while standing on the target.
- Recording B 15933 (dx 0x03, dy 0xFC) lacks the path target.
- Row: a spec-writing read of how `0x00548760` builds dx against how `0x0045D9B0` / `0x0045DB20` / `0x0045DC50` use it; fix one side (vitals.md §5.2 + `vitals_sync.rs:66-67`, or msg-units.md §5 r3 + `units.rs:923-924`); un-ignore the test with its sign adjusted; add a REC recording 0x96 / 0x18 while walking to a known target.

#### A2 0x23 fatal 0x668 (q-fix-set-skill-fatal) is not a layout disagreement
- Layout agrees: sim `crates/d2-sim/src/units/messages.rs:169`, server `crates/d2-server/src/adapters/session.rs:399,430`, generated `SetSkill`, client `crates/d2-client/src/bridge/msg/skills.rs:131`.
- Cause: `crates/d2-client/src/bridge/skills.rs:228` `select` refuses skill ≥ `tables.skills.len()` (msg-skills.md §2 r3). `crates/d2-client/tests/app_client_drlg.rs:242/283` binds only `levels`, so all three join 0x23s are refused; synthetic play binds one row (`app/synthetic_client.rs:24`), so the two carrying StartSkill are refused ("twice").
- Hazard: `crates/d2-client/src/app/play.rs:151` `add_client_data` calls `set_tables(ClientTables{levels, ..default})`, wiping rows set earlier.
- Fix: bind the real rows (`single_player::client_skill_rows`, as `play.rs:449`) in the test; give the synthetic install rows covering StartSkill; make `add_client_data` set only `levels`.

#### A3 TSV `produced_by=sim`, no producer: 0x11, 0x20, 0x22 (QUEUE q-fix-proto-missing-producers)
- 0x11: `intents-events.md` §7.9 r9 (l.897) has no builder; `wiring/action/unit_update.rs:269-310` does only the overlay removal.
- 0x20 (`0x0053C1D0`): no builder.
- 0x22: seam `send_skill_quantity` (`items/inventory/bookkeeping.rs:118`) defaults to a no-op (`wiring/inventory/mod.rs:288`); no host implements it (`preview_skills.rs:13`).
- Client handlers of all three match the TSV.
- Fix: `units::messages::update_item_skill` (12 bytes, bytes 2 and 10 = 0, msg-skills.md §5 r1) plus the seam; `report_kill` for §7.9 r9; decide whether 0x20 has a 1.14d caller, else fix its `produced_by`.

#### A4 Two d2-proto types for 0x0B, 0x0D, 0x28, 0x29, 0x2A (QUEUE q-fix-proto-one-type)
Generated `crates/d2-proto/src/generated.rs` l.3787, 3869, 4750, 4804, 4843; hand-written `crates/d2-proto/src/s2c/messages.rs` l.22, 36, 60, 75, 86 (used by `parse`). Identical bytes today (tested), names differ. Hand-written QuestInfo has `consts [6 => 0]` so `parse` refuses R ≠ 0, while the TSV has a free `u8@6` and the client (`ui_npc.rs:31`) reads it as R and echoes it in C→S 0x30. Hand-written NpcTransaction lists unwritten bytes 3–6; the generated one does not.

#### A5 Duplicate byte makers (QUEUE q-fix-proto-one-type)
0x1D–0x1F (`0x0053BE40`) in `d2-sim/src/wiring/action/vitals_sync.rs:204` (private) and `d2-server/src/adapters/session.rs:279`; 0x21 inline in `d2-server/src/adapters/handlers/skills/world.rs:506` with bonus always 0 (msg-skills.md §4 r1 says bonus_level; PROVISIONAL REC-96; client does not read byte 10); 0x0B inline at `wiring/path/act_change.rs:135`; 0x05/0x06 literals at `act_change.rs:26,93`, `session_flow.rs:392-393`; 0x28/0x29 inline in `world/quests.rs:2118` / `:1555` (test checks a replica); 0x27 inline at `world/npc.rs:674` plus `app/npc_seams.rs:138` `encode_text_list`.

#### A6 0x26 has no typed d2-proto codec (QUEUE q-fix-proto-one-type)
TSV gives the full layout but `s2c/audit.rs:80` says "size rule only" and `parse` returns Unbuilt. Size rule (`schema.rs:128`), sim `overhead_chat` (`units/messages.rs:48`) and client `ui_text.rs:40` agree. u8@2 is `lang` in the client, `byte8` in the sim.

#### A7 Docs (QUEUE q-fix-proto-docs)
`d2-sim/src/world/npc.rs:450-451` still calls 0x27 partial; `encode_text_list` caps at 7 entries while TSV / msg-ui.md §5 r1 allow k < 8 and the client asserts at ≥ 8 (`ui/msg_ui.rs:108`) without a spec line; 0x18 TSV `life_pred`/`mana_pred` vs msg-units.md §5 r2 "a"/"b" (stats 74/26).

#### Not fully traced
0x12, 0x13, 0x14, 0x16, 0x24, 0x25 (no layout, client no-op); 0x17, 0x2B size 0; 0x28/0x29 sim bytes by reading and replica; 0x27 npc.rs path via the client-crate `encode_text_list` with a rebuilt head; caller values (0x0C b0, 0x0D a/b, 0x0F/0x10 code) not traced (values, not layout).

## Detail, part b

No wire disagreement between a producer and a consumer in this range: where a sim/server builder and a client handler both exist, they agree byte for byte with each other and with the TSV layout. 25 contract tests pass; 1 is ignored on purpose (F1).

#### F1 0x50 QuestSpecial: `s2c::parse` refuses every code but 1 (medium, QUEUE q-fix-proto-quest-special)
- `crates/d2-proto/src/s2c/parse.rs:88-91` returns `Unbuilt{Partial}` unless bytes 1–2 are `01 00`; its comment and `docs/handoff/s2c-builders.md` §3 say only the mercenary form is missing.
- d2-sim sends forms whose layout is given: code 4 `world/quests/act1/q4.rs:818` (`quests-act1-rest.md` §7), code 13 `world/quests.rs:2308` (`quests.md` §9.4), code 23 `world/quests/act4/q2.rs:757`, code 2 `world/npc/hire.rs:473`.
- The TSV row (`server-messages.tsv:82`) and `gen::QuestSpecial` (`generated.rs:5286`) cover every code; the built `s2c::QuestSpecial` (`messages.rs:117`) pins code 1 and bytes 9–14 = 0.
- Impact: latent (client `ui_quest.rs:50` reads raw bytes); a trace comparison through `parse` would refuse valid messages.
- Row: `s2c::parse` decodes every 0x50 code with `gen::QuestSpecial`; keep `s2c::QuestSpecial` as the code-1 view; fix the 0x50 notes in `s2c/audit.rs` and `s2c-builders.md` §3; un-ignore `tests_proto_b::quest_special_0x50_parse_every_quest_form`.

#### F2 0x50: client reads a sixth word never written (low, QUEUE q-fix-proto-quest-special-word6)
- `ui_quest.rs:56-59` reads 6 words (@3…@13) per `msg-ui.md` §7; TSV has 5 (@3…@11); `act1/q4.rs:809` says the original never writes bytes 13–14 (d2rs sends 0).
- Row: spec session reconciles `msg-ui.md` §7 with the TSV (add masked `u16@13` and regenerate, or cut the client output to 5 words).

#### F3 0x5A EventMessage: TSV omits the account name @0x18 (low, QUEUE q-fix-proto-event-account)
- `intents-events.md` §8.3 and `d2-server` `session_flow.rs:257` put the account name at @0x18–0x27; TSV (`server-messages.tsv:92`) stops at `name:cstr16@8`. Client copies all 40 bytes (`ui_text.rs:100`): unaffected.
- Row: add `account:cstr16@24` to the TSV row, regenerate, extend `event_message_0x5a_one_layout`.

#### F4 0x5B, 0x5C, 0x65 and the join 0x5A (code 2) have no producer (medium, QUEUE q-fix-proto-roster-join)
- `intents-events.md` §8.3 join sequence (0x5B, 0x65, 0x8D, join 0x5A) and the leave 0x5C (§2.5 r2). Nothing in d2-sim/d2-server builds 0x5B, 0x5C, 0x65; only the leave 0x5A (code 3, `session_flow.rs:408`) is sent; no `EVENT_JOIN`.
- Impact: client roster empty in single player, so roster life updates from 0x0D, 0x82, 0x8E find no record.
- Row: implement them in `crates/d2-server/src/adapters/session_flow.rs` with builder == d2-proto checks. Blocker: a spec must say where 0x5B's @0x18–0x21 fields and its two strings come from.

#### F5 0x40 ItemFlags and 0x57 NpcEnchants have no producer (medium, QUEUE q-fix-proto-npc-enchants)
- 0x57: `wiring/action/unit_update.rs:97` notes step 10's 0x57 is not implemented; layout specified (`intents-events.md` ~l.901, `umod-callbacks.md` §28.4). Row: sim builder `npc_enchants(..) -> [u8; 14]`, send from step 10, compare with `d2_proto::server::NpcEnchants`.
- 0x40: no spec or code names its sender `0x0053D270`; needs a spec session first.

#### F6 Signedness (low, no action: as the specs say)
0x5D `extra` u16 in TSV, client reads i16 (`ui.rs:117`, `msg-ui.md` §1 r1); 0x65 `count` sign-extended (`roster.rs:121`, `msg-units.md` §8 r5); 0x53 period/ticks read as i32; 0x57 `umod2` (TSV u16@10) read as u8@0xA (`msg-units.md` §7 r3). The TSV has no signed types.

#### F7 Two typed layouts per id in d2-proto (low)
0x4E, 0x50, 0x52, 0x58, 0x59, 0x5D, 0x63 each have a generated and a hand-written type (field names differ; 0x63 `magic/bits0..3` vs `record[16]`). Bytes identical (tested per id), but it is a second place a layout can drift; a cleanup could keep only the generated types.

#### F8 Unnamed TSV bytes (low)
0x4C `u8@8`/`u16@14`, 0x4D `u8@10`/`u16@15` are the sim's `b` (skill level) and `w` (d2-proto `f8`/`f14`, `f10`/`f15`). The 0x4D shrine form carries the operator GUID in `skill:u32@6` and the shrine code in `u8@10`, per `objects.md` §14.

#### Not fully traced
0x45, 0x54, 0x66 (no layout anywhere, client no_op); 0x40 (when sent unknown); 0x5B field sources; 0x50 code 2 (only `u16@3 = name` specified; bytes 5–14 = 0 unverified).

## Detail, part c

Bytes agree on every producer/consumer pair (offsets, widths, endianness, bit order, including the 0x67–0x6D records and the bit-packed 0x95/0x96). 29 contract tests pass, none ignored. The one real problem is the meaning of dx/dy.

#### F1 (high, suspected) 0x95/0x96 (and 0x18): server spec and client spec use opposite signs for dx/dy — QUEUE q-fix-proto-walkverify-dx-sign
- Server: `specs/combat/vitals.md` §5.2 (l.564) dx = (X − path target) & 0xFF; sim `crates/d2-sim/src/wiring/action/vitals_sync.rs:66-70` (`x.wrapping_sub(p.target_x)`), on the wire via `combat/vitals/sync.rs:186`, `path/walk/messages.rs:96`.
- Client: `specs/client/msg-units.md` §5 r3 tx = x + sdx (`crates/d2-client/src/bridge/msg/units.rs:914-916`); `client/model.md` §6 r5 treats tx as the target. With the server's sign tx = 2X − target (the target mirrored through the server point).
- Effect: rule-5 "already closer to the target" acceptance fails when it should pass, so rule 8 (snap to server point, REC-277 (c)) runs more often: rubber-banding.
- Unsettled: recorded vectors (B 15933 dx +3 dy −4; B 32694 dx −2 dy +5) lack the path target.
- Row: local RE reads the subtraction order in `0x00548760` (server, path +0x10/+0x12) and the tx computation in `0x0045DC50`/`0x0045DB20` (client); fix `vitals.md` §5.2 or `msg-units.md` §5 r3, then `vitals_sync.rs::current` or the client; then a test path-with-target → `sync::sync` → client `vitals`, assert tx == target. No test asserts either sign yet.

#### F2 (low) 0x96 stale "never sent" doc — QUEUE q-fix-proto-docs
`crates/d2-server/src/adapters/handlers/walk.rs:22-23` and HANDOFF §1 3ag say 0x96 is never sent; the vitals sync sends it (enabled at `crates/d2-client/src/app/single_player.rs:2711`). Update the `pathing.md` §10 r5 / OQ6 cross-reference to `combat/vitals.md` §5.

#### F3 (low) 0x95/0x96: sim cuts over-wide values, d2-proto panics — QUEUE q-fix-proto-packed-cut
Sim cuts to field width (§5.4); `d2_proto::schema::packed_put` (`schema.rs:224`) panics (e.g. life > 0x7FFF). No effect today (nothing sends through the typed encoder). Mask in the generated `write`, or document that callers cut first.

#### F4 (low, no action) dx = −128 arrives as +128
Sim sends 0x80; client reads +128 (`units.rs:860` `sdelta`, per spec = 1.14d). Keep in the F1 fix.

#### F5 (medium, latent) 0x92 client handler does nothing — QUEUE q-fix-proto-0x92-client
`crates/d2-client/src/bridge/msg/items.rs:169` checks only the size; `specs/client/msg-stats-items.md` §5 r5 specifies a full inventory and stat refresh. No sim producer yet.

#### F6 (low) 0x73, 0x92, 0x93, 0x99, 0x9A: TSV `produced_by=sim`, no sim builder — QUEUE q-fix-proto-missing-builders
0x99/0x9A only have the queue record `ItemCastMsg` (`combat/events.rs:71`), never sent. Client reads TSV offsets; tested against `d2_proto` encode only.

#### F7 (low) duplicate d2-proto types with different field names — fold into q-fix-proto-docs
0x77, 0x89, 0x8A, 0x91, 0x9B: generated + hand-written (`s2c/messages.rs` l.211, 222, 233, 245, 258); bytes identical, names differ (`code`/`action`, `name/cost`/`f1/f3`, `npc0..11`/`slots`). Same as HANDOFF SC4/J13.

#### F8 (low, spec) 0x7F and 0x90 share TSV sender `0x0053CDF0`
Probably a copy error; RE session re-reads both (party, out of scope).

#### F9 (low) 0x8F Pong never sent
Client ping rtt stays 0 so its position tolerance uses L = 0: slightly tighter than on a real server, feeding the same check as F1. C→S 0x6D side in part e.

#### F10 (low, no action) 0x7A argument order
`pets::pet_action(action, pet_type, class, pet, owner)` writes owner@5, pet@9; callers pass the function's order, so the wire matches TSV and client (tested with distinct values and the recorded remove `7a 00 00 0000 00000000 0d000000`). Optional parameter reorder.

#### F11 (as specced, no action) narrower client reads
0x9B client reads u16@3 of the sender's u32@3 (`msg-ui.md` §15 r1); 0x9A client reads the low u16 of skill u32@6 (`msg-skills.md` §7 r2). Tests assert the truncation.

#### Not fully traced
0x67–0x6D send timing (`wiring/action/unit_update.rs:157`, not driven); 0x73, 0x92, 0x93, 0x99, 0x9A no producer; 0x75, 0x78, 0x79, 0x7F, 0x8B, 0x8C, 0x8D, 0x90 multiplayer (`produced_by=out`); 0x89, 0x8A, 0x8E, 0x91 built inline (`world/quests.rs:1967/1952/2200`, `wiring/inventory/pending.rs:251`), tests rebuild bytes by hand, 0x89 checked by reading; 0x8F no server producer; 0x6E–0x72, 0x80, 0x83–0x88 short rows.

## Detail, part d

Scope: 27 ids (0x3E, 0x94, 0x9C-0xB4). There is no high-severity disagreement. Wherever both a
producer and a consumer exist (0x94, 0x9C, 0x9D, 0x9E-0xA2, 0xA7-0xAA, 0xAC, 0xB0, 0xB2), the sim
or server builder, the TSV / d2-proto layout and the client handler agree field by field. That
includes the bit order (LSB-first on every side) and the bit-packed tails of 0x9C/0x9D (item
stream header), 0xA8, 0xAA and 0xAC. The size rules come from one table: the client
(`bridge/receive.rs:73` → `d2_proto::transport::split_server_buffer`) and the server
(`d2-server/src/adapters/sizes.rs:28`) both evaluate the generated `SERVER_MESSAGES`.

No code was fixed in this part: no plain one-liner with an unambiguous spec turned up.

Tests: `crates/d2-client/src/bridge/msg/tests_proto_d.rs`, 12 tests, all pass, none ignored.

### Findings

#### D1. 0x3E UpdateItemStats: nothing ever sends it (medium, QUEUE)
- TSV `specs/sim/server-messages.tsv:64` says `produced_by=sim`, sender `0x0053D130`.
- The client reader exists and follows `client/msg-stats-items.md` §5 r1:
  `crates/d2-client/src/bridge/msg/items.rs:60`.
- d2rs has no byte builder. Every `send_item_stat` seam is a no-op or a log line:
  - `crates/d2-sim/src/items/moves/seams.rs:537`, whose doc comment says "layout not written".
  - `crates/d2-sim/src/wiring/action/pending.rs:1256` (skill bodies: durability 72, quantity 70).
  - `crates/d2-client/src/app/rest.rs:324`: the play rest only logs the call, so the vendor
    paths (`world/vendors.md` §459, stack quantity 70; repair, recharge 204) reach nothing.
- No spec gives the server side of the bit layout. In particular nothing says how
  `0x0053D130(client, item, 1, s, v, 0)` picks the 8, 16 or 32-bit width for the GUID and the
  value, or the 8 / 16-bit param width.
- Effect in play: item stat changes never reach the client. This covers stack quantity after a
  buy or stack, durability after a repair or hit, and charges after a recharge. The shop and
  inventory then show stale values (likely part of the "shop problems in play").
- Proposed row **q-fix-proto-item-stat-0x3e**: spec the builder `0x0053D130` in
  `specs/client/msg-stats-items.md` §5 r1 (or in `specs/sim/intents-events.md` §4) from a
  Ghidra read: width choice for GUID, value and param, and the set flag. Then:
  - add a d2-sim builder, e.g. `units/messages.rs::update_item_stat`;
  - route every `send_item_stat` seam through it: `items/moves/seams.rs`, `wiring/inventory/pending.rs:555`, `wiring/action/pending.rs`,
    the d2-server vendor rest `adapters/handlers/items/vendor_inv.rs:235` and `d2-client/src/app/rest.rs:324`;
  - extend `tests_proto_d::update_item_stats_0x3e_one_layout` to compare the builder's bytes
    with the client read.

#### D2. 0xA3, 0xA4, 0xA5, 0xAB, 0xA6: nothing ever sends them (medium, QUEUE)
- The TSV marks all five `produced_by=sim`:
  `server-messages.tsv:165,166,167,173` (0xA3, 0xA4, 0xA5, 0xAB) and `:168` (0xA6).
- Their original producers are the pending event records of `0x00571CD0`
  (`sim/intents-events.md` §7.9 r2). d2rs keeps no such records: see the module doc of
  `crates/d2-sim/src/wiring/action/monster_add.rs:6`.
  - 0xA3: the `ProgressiveMsg` record (`crates/d2-sim/src/skills/use_/bodies/mod.rs:390`) is
    handed to the seam `queue_progressive`, a no-op at `wiring/action/pending.rs:1275`.
  - 0xA4: the seam `ai_preload_class` is a no-op at `wiring/action/pending.rs:629`.
  - 0xA5: exists only as a `BodyEffect` (`skills/use_/bodies/effects.rs:146`).
  - 0xAB: no record at all.
  - 0xA6: no producer.
- The client handlers exist and agree with the d2-proto encodes (tested):
  `skills.rs:336` (0xA3), `unit_misc.rs:217` (0xA4), `skills.rs:361` (0xA5),
  `unit_misc.rs:229` (0xAB), `items.rs:193` (0xA6).
- A small spec wording difference, not a wire problem:
  - `skills/bodies.md` §2.14 step 5 lists the 0xA3 record as "{n, k, lvl, unit, T, r, 0}".
  - `skills/bodies-2.md` §2.21 calls the same fields x and y.
  - So x = the roll r and y = 0. The builder must take `ProgressiveMsg::roll` as x@0x10 and
    write 0 at y@0x14.
- Proposed row **q-fix-proto-event-records**: implement the per-unit event-record list and
  its flush `0x00571CD0` (§7.9 r2: 0x9E is already done through `hirelings::level::flush_stats`).
  Add builders for 0xA3, 0xA4, 0xA5 and 0xAB that equal `d2_proto::server::{UnknownA3, BaalWave,
  UnknownA5, NpcHeal}::encode`. Wire them to the seams named above. The tests in
  `tests_proto_d::skill_npc_baal_0xa3_0xa4_0xa5_0xab_one_layout` should then compare sim bytes
  to proto.

#### D3. 0xA8 / 0xAA state stat param: written unsigned, read signed (low, QUEUE spec-confirm)
- Writer: `crates/d2-sim/src/units/messages.rs:262` writes the low `send param bits` of
  the param, unsigned.
- Client: `crates/d2-client/src/bridge/msg/states.rs:147-148` reads the param with
  `read_signed(...) as u16`, as `client/stat-lists.md` §3 r1 says ("read signed").
- Result: when param ≥ 2^(pb−1) the client keys the state-list entry by the sign-extended u16.
  Example: pb 4, param 15 is stored under param 0xFFFF.
- 0xAC reads the same kind of field unsigned (`units.rs:263-266`, `msg-units.md` §1.2 r4).
- Both sides follow their specs, so this is the original's behaviour if the spec is right.
  `tests_proto_d::set_state_0xa8_one_layout` and `add_unit_states_0xaa_one_layout` assert the
  behaviour as specced.
- Proposed row **q-fix-proto-state-param-sign**: confirm the signed param read at
  `0x0045EE20` / `0x00470E30`, which ties into `stat-lists.md` open question 5. If the read is
  unsigned, fix `states.rs:148` (one line).

#### D4. 0xB3 size rule needs 8 bytes but the smallest message is 7 (low, no producer; note)
- TSV `u8@1+7;min=8` (`server-messages.tsv:181`, `intents-events.md` §3.1 r1 table line
  "0xB3 | 8 | u8 at +1 + 7").
- A 0xB3 with len 0 is 7 bytes. When it ends a buffer, `server_size` returns `Incomplete`:
  - `d2-proto/src/transport.rs:142` breaks and the bytes are discarded;
  - `d2-server/src/buffers.rs:131` breaks the same way.
- Followed by more bytes it splits correctly. The test documents both cases.
- Probably faithful to the 1.14d min check. d2rs never sends 0xB3: game types 1 and 2 only,
  `d2-server/src/host.rs:284`.
- No action unless a Ghidra read shows the min applies only to the length byte.

#### D5. 0xAC size byte can wrap in theory (low, QUEUE optional)
- `crates/d2-sim/src/wiring/action/monster_add.rs:168` writes `(13 + stream.len()) as u8`.
  The stream may be up to `STREAM_MAX` = 0xF4 bytes (`:30`), so lengths ≥ 243 would wrap.
- `monsters/init.md` §24 r1 says size = 0xD + bytes with a 0xF4 buffer, so the original has the
  same arithmetic.
- The largest stream the writer can produce is about 186 bytes (16 stats × 73 bits plus the
  header blocks), so this cannot happen today.
- 0xAA cannot wrap: 7 + 0xF4 = 251.
- Optional row **q-fix-proto-ac-size-assert**: add `debug_assert!(13 + stream.len() <= 0xFF)`.

#### D6. Variable-size ids have no typed d2-proto encode or decode (low, QUEUE)
- `d2_proto::s2c::parse` returns `Unbuilt` for 0x3E, 0x94, 0x9C, 0x9D, 0xA6, 0xA8, 0xAA, 0xAC and
  0xB3 (`crates/d2-proto/src/s2c/parse.rs:95`). Only their size rule and TSV offsets exist.
- The client handlers read raw offsets themselves (`Bytes` in `msg/mod.rs`). They match the TSV
  today, and `tests_proto_d` checks this through the TSV field table, but nothing ties the
  handlers to the generated layout.
- There are also two hand-written readers of the item-stream header:
  - `bridge/msg/stats_items.rs:86` `ItemHeader::peek`;
  - `bridge/items.rs:85` `peek`.
  They agree with each other and with `d2_proto::item_bits` (tested), but they are duplicates.
- Proposed row **q-fix-proto-variable-types**:
  - add typed messages for these ids in `crates/d2-proto/src/s2c/messages.rs` (header fields
    plus the tail as `Vec<u8>`, the size byte written by `encode`);
  - have the client handlers parse through them;
  - make `ItemHeader::peek` and `items::peek` call `d2_proto::item_bits` for the header.

#### D7. `items::peek` returns no code for an ear-flagged full record (low, doc)
- `crates/d2-client/src/bridge/items.rs:98` returns `code: None` whenever the EAR flag is set.
- In a full record (`bitstream.md` §4.1 r5, §4.4) the 32-bit code comes before the ear fields.
  Only compact ears exist in 1.14d, so this is latent. No action.

#### D8. 0xB4 and 0xAF are not sent by d2rs (low, note)
- The client handles both (`session.rs:156`, `:235`).
- d2-server drops the refusal's 0xB4 on purpose (`adapters/session_flow.rs:131`, module doc
  `:36`) and never sends 0xAF.
- Layout checks pass: the d2-proto encode and size rule against the client.
- No action in Phases 0–6.

### Agreements checked (no finding)
- **0x9C / 0x9D** (`items/moves/layouts.rs:9`, `:22`): header bytes match the TSV.
  - The size byte is the total length, and the builder refuses ≥ 0xFD.
  - The stream is LSB-first on all three sides: the sim writer `items/bitstream.rs:239`, d2-proto
    `item_bits.rs:110` and the client `bridge/bits.rs`.
  - The header is 32 flags, 10 version and 3 mode, then either 16 + 16 bits (ground) or
    4 + 4 + 4 + 3 bits (slot, page + 1). Grid x / y are capped at 15.
  - The full and compact records were cross-read rule by rule. `crates/d2-server/tests/prop_item_bits.rs`
    already covers them.
  - Which actions go in 0x9C and which in 0x9D agrees across `deferred.rs` ITEM_ACTIONS,
    `items/item-actions.tsv` and the client's `world_action` (`stats_items.rs:221`).
- **0x9E-0xA2** (`world/hirelings/level.rs:163`, `:190`): the width thresholds < 0xFF and < 0xFFFF
  and the "0xA0 carries old" edge case match `hirelings.md` §13 and the client add / set
  (`items.rs:18`).
- **0xA7 / 0xA9** (`units/messages.rs:272`): equal the generated encode.
- **0xA8** (`:280`; size = 8 + stream) and **0xAA** (`:310`; size = 7 + stream): equal the TSV.
  The client reads every entry back, with the clamp of §7.9 r1.3, and value 0 removes the entry.
- **0xAC** (`wiring/action/monster_add.rs:81`) against the client (`msg/units.rs:142`), matched in
  this order:
  - mode, 4 bits;
  - components, widths taken from the `monstats2` counts;
  - the type block: 5 flags, hcIdx when superunique, umods ending in 0, name seed, and
    "1 + 32 hireling owner";
  - the source link, 31 bits;
  - the stat list, ended by 0x1FF, or 2 zero bits when no stat is sent.
- **0x94** (`units/messages.rs:186`): equals the TSV, and the client assigns every entry.
- **0xB2** (`d2-server adapters/session_flow.rs:272`): equals `GameList` encode (u16@49, u16@51).
- **0xB0** (`session_flow.rs:394`): equals `ConnectionTerminated` encode.

### Ids not fully traced
- **0x3E**: the server-side encoding (widths) has no spec and no code (D1). Only the client
  reader was checked, against the spec layout.
- **0xA6**: the producer's meaning (code, index, the 0x120-byte record) comes from the client
  spec only; there is no server spec or code.
- **0xAF and 0xB3**: transport semantics (flag meaning, the save-download chunks of game types
  1 and 2) are not built. Only the size rules were checked.
- **Item stream fields past the header** (quality blocks, property lists): checked by reading
  writer against reader, plus the existing proptest. `tests_proto_d` decodes only the header,
  the code, gold and one property.

## Detail, part e

(F1 below was fixed in this session: P5.)


No high-severity wire disagreement. The server's exact-size check equals the TSV `transport_size` / `handler_size` for every `handler` row. Walk, run and resync (0x01–0x04, 0x5F) and the shop/NPC ids (0x13, 0x2F–0x38, 0x62) agree byte for byte: the rubber-banding and shop problems in play are not a byte-layout problem on the C→S side. 15 contract tests pass, 1 ignored (F1).

#### F1 0x30 TerminateEntityChat: shop close writes u32@1 = 0 (medium, QUEUE q-fix-proto-chat-end-flag)
- Spec: `specs/client/model.md` §17 r1 step 5, the interaction end `E(G)` (`0x004B3C20`) sends `[u32 1][u32 G]`; `specs/ui/panels-2.md` §14 r5, the shop close goes through `0x004B3C20`.
- Right: `crates/d2-client/src/ui/panels/npc.rs:302` `msg_chat_end` (used by `ui/messages/dialog.rs:471,561`, `ui/messages/intro.rs:359`, `bridge/chat_end.rs:21`).
- Wrong: `crates/d2-client/src/ui/panels/shop.rs:269-272` `ShopPanel::close_intent` and `crates/d2-client/src/ui/shop_ui.rs:657` build `TerminateEntityChat{id}` with u32@1 = 0.
- Server: `d2-sim world/npc.rs:720` `chat_close` reads only u32@5, so nothing mis-parses; the bytes still differ from 1.14d (rule 10).
- Existing tests asserting 0: `ui/panels/shop.rs:863-870` `shop_close_sends_0x30`, `bridge/tests.rs:151`, `tests/e2e_vendor.rs:957`, `tests/e2e_single_player.rs:1399`.
- Row: send `npc::msg_chat_end(guid)` from both; update the four tests to byte 1 = 1; un-ignore `tests_proto_e::shop_close_0x30_one_layout`.

#### F2 Client sends, server stubs (low; behaviour gaps, not layout)
Bytes right; the server falls to the `unhandled` stub (returns 0).
- 0x3E (`ui/messages/hire.rs:187`): WORLD_IDS `NoOwner`.
- 0x4C `[-1]` (`controls/click.rs:556`, `ui/panels/control/input.rs:143`): `handlers/world.rs:181` says `OtherModule("items")` but `handlers/items.rs:234` takes only 0x2A and 0x4F. QUEUE q-fix-proto-4c-route: route 0x4C to an item-use owner or mark it `NoOwner`.
- 0x59 (`controls/click.rs:972`): `NoOwner`.
- 0x44 (`ui/messages/socket.rs:285,554,581`): parsed at u32@5 / u32@9 / u16@13 (`handlers/player.rs:541`), body `None`.
- 0x13 for unit types 0 and 4 (`controls/click.rs:1083`): `NpcControl::interact` returns `None`.

#### F3 0x15 string check of §2.4 r6 not implemented (low, QUEUE q-fix-proto-chat15-check)
Spec: strlen < 256 and strlen + 4 < size, else rejected. `dispatch.rs:257` `check_size` returns true for 0x15 and no handler implements it (stub 0). Neither 1.14d single player nor d2-client sends chat. Add the check in `handlers/player.rs` or `dispatch.rs` after the spec names the refusal code.

#### F4 0x26 u32@5 naming (low, doc)
TSV `on_merc`; `ui/controls.md` §7 r3 `shift` (0 or 0x8000). Client `bridge/belt.rs:57-59` encodes `on_merc: 0` then patches bytes 5..9 with `shift`; server `d2-sim items/moves/handlers.rs:105` reads on_merc (≠ 0 → hireling). Same meaning; the client could write `UseBeltItem{item, on_merc: shift}`.

#### F5 0x32 note (matches spec)
`ui/panels/shop.rs:546` writes `m << 16` into u32@9; a mode ≥ 0x8000 sets bit 31, read as fill by `BuyMsg::parse` (`d2-sim world/vendors/trade.rs:181`), as in 1.14d (store items are mode 0). Asserted.

#### Checked and agreeing
- Point ids 0x01, 0x03, 0x05, 0x08, 0x0C, 0x0F: `controls/click.rs:313` `code_bytes` = proto; `dispatch.rs:277` reads u16@1/u16@3 (50 in range, 51 not; resync S→C 0x15 after > 25 frames); `walk.rs:128`, `skills/use_ validate_point` same offsets; `predict.rs:155` `walk_of` reads the client bytes back.
- Unit ids 0x02, 0x04, 0x06, 0x07, 0x09, 0x0A, 0x0D, 0x0E, 0x10, 0x11: `code_bytes` = proto; dispatcher looks up (type u32@1, id u32@5); type ≥ 6 → Invalid.
- 0x0B, 0x12, 0x13: `code_bytes` sends nothing (§2.1 r8); 0x13 only from `objects/interact.rs:26` = proto.
- Item moves: `d2_sim::items::moves::HANDLED` sizes = TSV for all 23 ids; offsets incl. bodyloc u8@5 match.
- Shop 0x32/0x33/0x35 `ShopTx` = proto = `BuyMsg`/`SellMsg`/`RepairMsg::parse` (incl. repair-all); 0x36 u16@5; 0x31 npc u32@1, msg u16@5; 0x2F id u32@5; 0x3A `[stat][n-1]`; 0x3C/0x51 bit fields = `dispatch::select_skill`/`bind_hotkey`; gates 0x3C none, 0x41 dead; 0x44, 0x49, 0x4C, 0x5F agree (0x5F not point-parsed); exact-size check passes exactly the TSV size; `handlers/player.rs:397` `size_ok` agrees (by eye); 0x67 session = proto both sides.

#### Not fully traced
- Rubber-banding (behavioural suspects, not layout): the point parser checks against the staged player position (`adapters/sim.rs` `point_state`), refreshed by `refresh_targets` → `WorldHost::live_facts` (default `None`) and the per-tick `unit_positions` copy; if stale, in-range walks are refused and after 25 frames S→C 0x15 is queued (a rubber band). The client's walk target is the client path end (`controls/click.rs:593`), a straight line in the preview (REC-277). Check: log `PointState.player` vs the path position on each refused 0x01/0x03 in `play_smoke`.
- Shop: only F1 differs at the byte level and the server ignores that byte.
- Inline private encoders checked by reading: `click.rs` `send_u32` (0x17, 0x27, 0x4C), 0x59 at l.972; `bridge/update.rs:63` (0x4B); `ui/messages/intro.rs:251` (0x4D); `bridge/objects/interact.rs:68` (inline 0x16).
- World-needing handlers (NPC, quest, waypoint, cube, player §9, skill handlers, item moves): parse offsets by reading only.
- Not encoded by the client: 0x14, 0x15, 0x66, 0x6C (variable-size rules covered by transport tests); 0x5D, 0x5E, 0x68, 0x6D out of scope.
