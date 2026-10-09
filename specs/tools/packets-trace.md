# Spec: Tools — Packet traces and their comparison (1.14d vs d2rs)

- **Status:** draft: the `packets-raw-1` format, the d2rs recorder
  (`d2_server::packets`, `d2-client state-dump --packets`) and the
  comparator (`tools/trace-recorder/packets_diff.py`) implement it; the
  1.14d side is `record_packets.py`. First real run:
  `traces/checks/packets-town-arrival-ama.check` (Open questions 1).
- **Target version:** 1.14d (the original side); the format is d2rs-own.
- **Crate/module:** `d2-server::packets` (observer and line writer),
  `d2-server::host` (the hook points), `d2-client` `app::packet_dump`;
  `tools/trace-recorder/record_packets.py`, `packets_diff.py`;
  `tools/scenario-diff/packets_channel.py` (the `packets` channel).
- **Related specs:** `sim/intents-events.md` (§1 loop order, §2–§3 the
  transport, §6 the exact-match comparison this tool runs),
  `tools/scenario.md` §6 (masks), `tools/scenario-diff.md` (one-command
  run), `tools/state-snapshot.md` (the state channel, same run).

## Summary

Both games record every message on the single-player transport: each
client → server message as the server takes it from its queues, each
dispatch and result, the tick markers, each server → client message as
it is queued, each direct send and each buffer the flush hands to
delivery, with the frame and the loop phase. `packets_diff.py` cuts both
recordings into the per-frame units of `sim/intents-events.md` §6 rule 1
and compares them message by message, byte by byte, skipping only the
masked bytes, and prints the first divergence.

## Inputs

| Name | Type | Source |
|---|---|---|
| 1.14d recording | `packets-raw-1` | `record_packets.py --auto C --seed N --ticks T` |
| d2rs recording | `packets-raw-1` | `d2-client state-dump --save --seed --ticks T --out S --packets FILE` |
| masks | TSV | `specs/tools/scenario-masks.tsv` |
| message names | TSV | `specs/sim/client-messages.tsv`, `server-messages.tsv` (`name`) |

## Outputs / state changes

The two recordings (gitignored, `traces/raw/`); the comparator's report
on stdout and its exit code (§4). Nothing in either game changes.

## Rules

### 1. Format `packets-raw-1`

1. JSON lines. The first line is the header `{"type": "header",
   "format": "packets-raw-1", ...}`: 1.14d `tool`, `date`,
   `game_exe_sha256`, `args`; d2rs `"side": "d2rs"`, `tool`, `date`,
   `command`, `save`, `seed`. The last line is the footer `{"type":
   "footer", "events", "counts", "notes"}`. Every line between is a
   record.
2. Every record has `type` (table below), `frame` (the frame of the last
   tick: 1.14d game +0xA8 read at the tick's entry, + 1; d2rs
   `Intents::frame` + 1 before the tick; `null` before the first tick),
   `phase` (the loop phase after the record's event: `start`, then
   `input` after a `drain`, `tick` after a `tick`, `post` after a
   `tick_end`, `flush` after a `flush`) and `seq` (line number among the
   records, from 0). 1.14d adds `tid` and `ms` (wall clock).
3. Record types (bytes are lower-case hex):

<!-- rows -->
| type | Fields | 1.14d (`record_packets.py`) | d2rs (`Host`) |
|---|---|---|---|
| `client_send` | client, size, bytes | game-message sender `0x00478350`, before its duplicate filter | `send_game`, before the filter |
| `client_out` | client, size, bytes | net send `0x0052AE50` | `send_game` after the filter, `send_system` |
| `drain` | — | drain `0x0052CFE0` | start of `Host::frame`'s drain |
| `c2s` / `c2s_sys` | client, size, bytes | game entry `0x0053F3D0` / system entry `0x0053F100`; bytes = the drain copy (≤ 0x1FC) | each drained game / system message (queue 2 is not recorded: no 1.14d hook) |
| `dispatch` | game_frame, size, id (1.14d also unit) | dispatcher `0x0054D750` | before `process_game_message` when the player lookup finds a player |
| `result` | dispatched, code | `0x0053F45E` | after a dispatched message's handler |
| `tick` / `tick_end` | (frame) | `0x0052D870` / `0x0052FD1E`, `0x00564608` | around `Tick::tick` |
| `flush` | — | flush `0x0052FD90` | `Host::flush` entry |
| `s2c` | client, size, bytes (1.14d also caller) | queue `0x0053B280` | `ClientBuffers::queue` for a known client |
| `net` | kind, client, size, bytes, caller or via | net send `0x0052B330` | a buffer popped by a flush, or a direct send |

4. A `net` record of a flushed buffer has `"caller": "0x52e3b5"` (1.14d:
   the flush's call of the net send, `sim/intents-events.md` §3.2 rule 4;
   d2rs writes the same label). Any other `net` record is a direct send
   (§3.3 rule 5): 1.14d gives the sender's call site as `caller`, d2rs
   writes `"via": "direct"` and no `caller`.
5. `--ticks T` on either side ends the recording after tick T's flush:
   1.14d at the next queue drain (that `drain` is not written), d2rs when
   `state-dump` has run T ticks.

### 2. The d2rs recorder

1. `Host::set_packet_observer(Some(observer))` installs the recorder;
   `None` (the default) removes it. Recording only reads: copies of the
   messages, `Intents::frame`, `Intents::player`; with or without it
   the game, the host's reports and every message delivered are the
   same (`d2-server` test `recording_does_not_change_the_game`).
2. Events reach the observer in the order the host does them: client
   sends as they are made; per frame `drain`, then per drained message
   `c2s`/`c2s_sys`, `dispatch` (when dispatched), the messages its
   handling queued (`s2c`) or sent directly (`net`), `result`; then, if
   a tick runs, `tick`, the tick's `s2c`, `tick_end`, `flush` and one
   `net` per buffer. Queued messages, direct sends and popped buffers are
   taken from the buffers' tap (`ClientBuffers::set_tap`) in the order
   they happened.
3. `d2_server::packets::PacketLog` writes the records of §1 rules 2–4;
   the frame and the phase follow §1 rule 2 exactly as
   `record_packets.py` sets them.
4. `d2-client state-dump ... --packets FILE` installs a `PacketLog` on
   the dump's host (on the server thread, through the link, before the
   bridge sends C→S 0x67), writes the header, the records after each
   bridge frame and the footer; the state file is unchanged.

### 3. Comparison (`packets_diff.py`)

1. **Window.** A record belongs to window F = its `frame` when its
   `phase` is `tick`, else `frame` + 1; a record with `frame` null to the
   file's first tick frame. Window F holds I(F) and O(F) of
   `sim/intents-events.md` §6 rule 1 (from the end of tick F − 1 to the
   end of tick F).
2. **Range.** Windows up to the smaller last `tick_end` frame of the two
   sides are compared (`--from`, `--to` narrow it). A side with no
   complete tick, or two different last ticks, makes the result partial
   (§4); different first tick frames are a divergence.
3. **Streams**, in this order per window: `c2s` (the `c2s` and
   `c2s_sys` records in record order), `s2c` (the `s2c` records and the
   direct-send `net` records in record order), `buf` (the flushed `net`
   records).
4. **Records**, index by index: the client (each side's client ids
   numbered by first appearance: 1.14d and d2rs use different ids),
   then for `c2s` / `s2c` the kind (game / system; queued / direct), the
   id (byte 0), the size, then the bytes from offset 1 skipping masked
   bytes; for `buf` the size only (its bytes are the window's `s2c`
   messages packed by §3.2 rules 1–2, which `check_packets.py` R6 checks
   per side).
5. **Masks** (`tools/scenario.md` §6): the rows of
   `scenario-masks.tsv` apply to `s2c` records of their id; keys and NUL
   positions are read from the 1.14d record; key bytes and the size are
   still compared. The table is read strictly (`scenario.md` §6 rule 3).
6. A record on one side only is a divergence (`missing in d2rs` /
   `extra (d2rs only)`).
7. **Report.** Per divergence: frame, stream, index in the window's
   stream, where (`client`, `kind`, `id`, `size`, `bytes[k]`, missing /
   extra), both values, and per side the record's `seq`, kind, client,
   id, name (from the two message tables), size and the bytes ±8 around
   the offset (masked bytes shown `--`), the number of records of that
   window's stream on each side, and the previous record. Only the first
   divergence of each window and stream is reported (the rest of that
   stream is shifted); the first, then `--next N` more, then the
   summary: windows compared, records per stream, masked bytes skipped,
   the first divergent window per stream, partial reasons.
8. Not compared: `client_send`, `client_out` (the client side), `drain`,
   `dispatch`, `result`, `tick` / `tick_end` / `flush` beyond rule 2,
   `tid`, `ms`, `caller` of `s2c`, header and footer fields.

### 4. Verdict

1. Exit 0 `MATCH`: no divergence, at least one window compared, the
   same last tick on both sides, no `gaps` in either header. 1
   `DIVERGED`: a divergence. 2 `PARTIAL`: no divergence, but one of the
   `MATCH` conditions does not hold. 3: error (unreadable file, wrong
   format, bad mask table, bad option).

## Constants & data dependencies

Flush call site label `0x52e3b5`; drain copy 0x1FC bytes; context 8
bytes. The two message tables and the mask table (Inputs).

## Randomness

None.

## Edge cases & original bugs

1. A 1.14d `s2c` for a null client is not possible to record (the hook
   reads the client record); d2rs ignores unknown clients and records
   only known ones.
2. A filtered duplicate leaves a `client_send` with no `client_out` on
   both sides.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| `packets_diff.py --selftest` | a synthetic 1.14d recording against its d2rs form matches; every byte of every compared message perturbed is reported at its window, stream, index and offset, except masked bytes (ignored); a size change, a dropped and an extra record, a buffer size and a keyed mask's key byte are reported; fewer ticks on one side is partial; bad mask rows are rejected | this tool |
| `cargo nextest run -p d2-server packets` | the game is unchanged with the recorder on; record order and fields of §1–§2 on a fixed host session | this tool |
| `check_packets.py` on both recordings of `packets-town-arrival-ama` | R1–R7 hold on each side (2026-10-09: OK, OK); d2rs `state-dump` state lines identical with and without `--packets` | this tool |
| `scenario_diff.py --selftest` | the packets channel's dry run issues `record_packets.py --ticks`, `state-dump --packets` and `packets_diff.py` with the check's save and seed | this tool |

## Provenance

d2rs-own tool. The 1.14d hook points and record fields are
`record_packets.py`'s (addresses: `sim/intents-events.md` §2–§3, §6).

## Open questions

1. First real comparison (2026-10-09,
   `traces/checks/packets-town-arrival-ama.check`): see
   `tools/scenario-diff.md` §3 rule 9 for the result.
2. `record_packets.py` has no poke layer: a check with `at … poke` lines
   reports the packets channel as not compared.
