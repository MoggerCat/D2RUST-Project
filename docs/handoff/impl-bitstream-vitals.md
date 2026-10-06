# Handoff: item bit stream and client vitals sync — `claude/impl-bitstream-vitals`

Cloud implementation session, 2026-10-06, medium (METHODS M14). Base:
`claude/tender-meitner-mphas3` at `7f684ad` (M09: repo only, synthetic
tables, no game files, no recordings). Specs: `items/bitstream.md`
(draft), `sim/server-messages.tsv` rows 0x18 / 0x95 (`bits:` layouts),
`combat/vitals.md` §5. Fold into `docs/HANDOFF.md` / `docs/PLAN.md`:
this session did not edit them.

## 1. State

**(1) Item bit stream (`bitstream.md` §1–§4; settles `inventory.md` OQ1
in code).** Implemented, **unverified against a live game** (M02), but
the ten recorded streams B1–B10 are reproduced byte for byte by the
writer and decoded to the spec's exact bit counts by the reader.

- Writer `d2_sim::items::bitstream` (`write`, `write_into`, `BitWriter`,
  `StreamItem`, `IscTable`): header (forced / cleared bits), compact and
  full records, alt-code record, clamps, ISC values, affix ids, every
  quality branch, runeword / ear / personalized names, type values,
  sockets, set mask, property lists with the grouped stats and the
  terminator rule. The save format (§5) is not written (never on the
  wire).
- Wiring `d2_sim::wiring::inventory::bits`: `InvDesk::stream_item` fills
  the writer's view from the item store, unit record, inventory state,
  item tables and stat lists; `InvDesk::item_stream` writes it.
  `MovePending::item_bits` on the desk now answers with it (it no longer
  forwards to the rest): every deferred 0x9C / 0x9D and the direct sends
  of §6.4 carry the real stream on the wired host.
- `ItemTables` gained `isc` (itemstatcost `ValShift`, `Save Bits`, `Save
  Add`, `Save Param Bits`) and `ItemRec` gained `compactsave`, `normcode`
  (`from_fixed` fills them; fixtures that build `ItemTables` by hand get
  an empty `isc`, i.e. every stat `Save Bits` 0).
- Reader `d2_proto::item_bits` (`decode`, `decode_record`, `BitReader`,
  `ItemBits`, `ItemLookup`): strict (unknown code / stat, stat on the
  wire with `Save Bits` 0, short stream, more than the padding, a set
  padding bit are errors). Tables adapter for the reader:
  `d2_server::adapters::item_bits::TablesLookup` (`ItemTables` →
  `ItemLookup`; d2-proto holds no tables, d2-sim may not depend on
  d2-proto).

**(2) 0x18 / 0x95 layouts.** Builders `combat::vitals::sync::
{life_mana_update, life_mana_update2}` (0x96 reuses
`path::walk::messages::walk_verify`); checked field by field against the
`bits:` rows of `server-messages.tsv` (with a perturbation test) and,
on the host, decoded with `d2_proto::server::{LifeManaUpdate2,
WalkVerify}`.

**(3) Client vitals sync (`vitals.md` §5).** Implemented, unverified
(OQ8: no recording holds these bytes with their ticks).

- Pure rules `d2_sim::combat::vitals::sync`: force rule (§5.1 r2), life /
  mana predictions (§5.2, 32-bit wrapping, low byte, 100 cap), steps 1–6
  (§5.3), experience message, cache record.
- Wiring `d2_sim::wiring::action::vitals_sync`: per-client caches in
  `ActionHooks::sync` (**opt-in**, `None` by default,
  `ActionHooks::enable_vitals_sync()`), current values from the stat
  lists (totals, max life / mana, state 100 / 106 lists), the path record
  (position, target offset) or the host's staged position, the charstats
  row (`ActionHooks::vitals`), the game frame; `run` resets client
  +0x1B0 when the routine returns 1.
- Host: `SimGame::tick` runs it after the deferred item pass, per client
  in client-list order (`WorldHost::vitals_sync`, implemented by
  `ActionWorld` and `WiredWorld`); `MessageSink::has_queued` (default
  false; `ClientBuffers`: the client's buffer list is not empty) gives
  the "queued buffer" of §5.1 rule 2.
- Why opt-in: on by default it would add 0x95 / 0x96 / gold / experience
  bytes to every exact-byte host and e2e test. Turning it on for the
  app's single-player game (`d2-client` `app::single_player`, owned by
  the client session) is the follow-up; expect the e2e byte lists to gain
  the sync's messages at the end of each tick's batch.

## 2. Values the tests use that the spec does not list

The vectors need three itemstatcost rows the spec's Constants omit. They
are the only widths under which B5, B6 and B8 end exactly in their last
byte (found by decoding the vectors, not from game files):

| Stat | Save Bits | Save Add |
|---|---|---|
| 22 `maxdamage` | 7 | 0 |
| 60 `lifedrainmindam` | 7 | 0 |
| 75 `item_maxdurability_percent` | 7 | 20 |

Confirm with the local check LB1 below; add them to the spec's
Constants.

## 3. Readings and open points (TODO(spec) at the sites)

- BV1 `bitstream.md` §3 rule 2 says "Location (§4.1 rules 2–3)" for the
  compact record; B1 / B2 need the 3 mode bits of §4.1 rule 1 first (92
  bits). Written as mode + location; reword the rule.
- BV2 §4.3 rule 7: "property lists skipped" read as only for a quality
  that was overwritten (0, > 9); quality 2 keeps its lists (B10 has stat
  107 on a quality-2 item).
- BV3 §4.6 rule 4.3: the recorded partner values are kept per list; the
  spec does not say whether the record spans the lists of one item.
- BV4 §4.1 rule 1 names item data +0x30 as the version; the item store's
  `format` (`generation.md` §1.2, +0x2A in the request) is used (101 in
  expansion games, as the vectors).
- BV5 §4.4 rule 1: the runeword record of an item (`0x0062BED0`) has no
  provider in `d2-sim`; sent as 0xFFFF ("no record").
- BV6 §4.1 rule 8 / §4.3 rule 6: the writer's changes to the item (level
  < 1 → 1, quality outside 1–9 → 2) are returned (`WriteBack`) but not
  written back: `MovePending::item_bits` is `&self`.
- BV7 §3 rule 3: a name filling all 16 bytes of +0x4A has no terminator;
  written as 16 characters then a 0 (the original reads on).
- BV8 rare names `0x00627FE0` / `0x00628040` read as the item store's
  `rare_prefix` / `rare_suffix` as stored; set lists found with
  `find_list` (state, flags 0x2040, else 0x40), the main list with
  `ListKey::ITEM`, entries in key order.
- BV9 the reader cannot apply §3 rule 5 to a compact ear (no code on the
  wire); read as "no quest difficulty" (no ear record is a quest item).
- VS1 `vitals.md` §5.2: the cache's values when a client record is
  created are not written; read as all zero (so the first sync after
  joining sends 0x95 once life ≥ 10 % of max).
- VS2 §5.1 rule 3: the fatal assert for a client whose unit is not a
  player is read as "nothing runs"; §5.1 rule 4 (`0x0052DA00`, OQ7) is
  not run.
- VS3 §5.1 rule 1: single player flushes once per tick, so the sync runs
  at the end of `SimGame::tick`; a host with several flushes per tick
  (multiplayer) would call it from its flush instead.

## 4. Tests (`// Covers:` claims)

- `d2-sim items::bitstream::tests` 18: B1–B10 re-encoded byte-exact with
  bit counts, synthetic gold / clamp vectors, overflow, header bits, alt
  code, quality rewrite, rare / set / unique / tempered branches, set
  mask and runeword terminators, ear / personalized / quest difficulty,
  unidentified end.
- `d2-proto item_bits::tests` 6: B1–B10 decoded field by field; M08:
  longer, shorter, padding bit, changed code are refused.
- `d2-server tests/prop_item_bits.rs`: `writer_and_reader_agree`
  (writer → reader, every field after clamps; 512 cases default, 20,000
  clean), `grouped_partners_round_trip`.
- `d2-sim combat::vitals::sync::tests` 9 (layouts vs `bits:` rows + M08,
  force, gates, priority, quiet resync, gold / experience, predictions);
  `wiring::action::vitals_sync::tests` 4 (off by default / not in game,
  0x95 then 0x96 and the counter reset, gold and experience, the
  healthpot list); `d2-server walk::tests::vitals_sync_on_the_host_tick`
  (0x95 then the forced 0x96 at the 20th update, typed `d2-proto`
  builders).
- Changed: the d2-server item-move tests now decode every 0x9C / 0x9D
  stream with the reader (exact length, code, and the mode in the item's
  last message of a batch) before comparing the headers;
  `click_button_transmutes` checks the ring's stream (code, page 3);
  `wiring::inventory` `pickup_position_follows_the_grid_record` (the
  stream carries the grid x: one bit differs) and
  `send_item_page_queues_0x9d_now` (the stream with page 3); the
  forward test of `mutants_wiring_inventory` (`item_bits` is no longer
  forwarded).

## 5. Gate

See §7.

## 6. Local run queue (add to `docs/HANDOFF.md` §5)

- LB1 (game files): the itemstatcost rows of §2 — `data-tool tables`
  output or `itemstatcost.txt` of 1.14d `patch_d2`: stats 22, 60, 75
  `Save Bits` 7, 7, 7 and `Save Add` 0, 0, 20.
- LB2 (recordings `20261006-015956-packets.jsonl`,
  `20261006-022633-packets.jsonl`): decode all 144 0x9C / 0x9D streams
  with `d2_proto::item_bits::decode` and `TablesLookup` over the 1.14d
  tables (`ItemTables::from_fixed`); expect every stream `Ok` (exact
  length, zero padding). Then, for each decoded item, rebuild a
  `StreamItem` from the decoded fields and `d2_sim::items::bitstream::
  write` it: expect the recorded bytes. (A conformance test reading the
  recordings under `D2_GAME_DIR` / `traces/` would hold this; the
  recordings are not in the repo.)
- LB3 (recording, `vitals.md` OQ8): any recording with damage, potions
  and running: replay through `SimGame` with `enable_vitals_sync` and
  compare every 0x18 / 0x95 / 0x96 / 0x19–0x1F / 0x1A–0x1C byte and its
  tick.

## 7. Gate result

`sh tools/gate.sh` (after `tools/cloud-setup.sh`): **FAIL, on failures
the base already has.** PASS: spec_index, methods, coverage (4,524
claims, 0 errors), trace checkers, hook selftest, fmt, depcheck, clippy
workspace, test rest (no client: 886), both doc-test steps. FAIL: test
d2-sim + conformance, test d2-client. Run with `--no-fail-fast` on this
branch and on the base `7f684ad` (worktree, same toolchain): the same
55 tests fail on both (`diff` of the two lists: identical), none in the
files this session touched:

- d2-sim (9): skill / missile / AI catalogue checks
  (`skills::use_::tests::function_tables_match_tsv` and its
  perturbation / mutant tests, `missiles::tests_bodies::*`,
  `monsters::ai::tests::implemented_matches_catalogue`,
  `d2moo_only_act1_ais_are_stubs`): specs merged ahead of their code
  (`spec-skill-bodies`, missile and AI bodies); owners: the
  implementation sessions of those specs.
- d2-client (46): the bridge's dispatch table does not match its spec
  table (`bridge::tests::dispatch_table_matches_spec`: `NoHandler` for
  51 S→C ids), so every test that builds the bridge fails (bridge, UI,
  app frame loop, every e2e, the bridge properties); owner:
  `impl-client-model`.

This branch's own tests all pass (d2-sim 2,754 of 2,809 in the three
crates' run; d2-server and d2-proto clean).
