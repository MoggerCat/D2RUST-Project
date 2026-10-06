# Handoff: Rust replay harnesses for the coming recordings — `claude/conformance-harness`

> Not folded into `docs/HANDOFF.md` / `docs/PLAN.md` yet; for the coordinator to fold (neither file is edited here).

Cloud implementation session, 2026-10-06. Task class: conformance
harnesses from the specs, the recorder README and the Python checkers,
medium effort (METHODS M14). Base: `claude/tender-meitner-mphas3` at
`4b5b0bf`. Repo only, no `game/`, no `traces/raw/` (M09). Owned files:
`crates/conformance/` only.

## 1. State

Goal: when the units, stats and packets recordings of `docs/HANDOFF.md`
§5 A arrive, one `cargo test … -- --ignored` turns each into a pass or a
first mismatch the same day, numbered as the Python checker numbers it.

| Kind | Harness | Input (format, version-checked) | Through | CI fixture | Recording test |
|---|---|---|---|---|---|
| units / schedules | `conformance::units::replay_anim` | `traces/raw/*-tick.jsonl`, header `format` = `tick-raw-1`; needs `record_tick.py` 0.2.0 (`anim` records) | `d2_sim::units::anim::schedule` | `units-anim.jsonl`: pass, 14 sets × 4 fields perturbed, missing / extra set, cancels | `units_replay::recorded_mode_schedules_replay_exactly` (ignored) |
| stats | `conformance::stats::replay_stats` | `traces/raw/*-stats.jsonl`, `stats-raw-1` | `d2_sim::stats::StatLists` (+ `d2_data` `stat_ops` for the tables) | `stats-lists.jsonl`: pass, snapshot / callback / missing / extra / flag / expiry / table perturbations, seed rebuild | `stats_replay::recorded_stat_lists_replay_exactly` (ignored) |
| packets (S→C bytes) | `conformance::packets::replay_packets` | `traces/raw/*-packets.jsonl`, `packets-raw-1` | a `PacketServer`; provider `DispatchServer` on `d2_server::dispatch::process_game_message` + `Tick` | `packets-dispatch.jsonl` (real `d2-server`, result codes 0 / 1 / dropped), `packets-scripted.jsonl` (every byte flip, missing / extra / wrong client) | `packets_replay::recorded_messages_replay_exactly` (ignored; expected to fail today, §4) |
| rooms (adjacency) | `conformance::rooms::replay_rooms` | `traces/sim/tick/*.json` (format 1, `convert_tick.py` output, the file `check_rooms.py` reads) | a `RoomModel`; no d2-sim provider yet (§4) | `rooms-adjacency.json` (rooms.md §6 vector): pass, all 38 adjacent swaps, missing room | `rooms_replay::committed_tick_traces_read_and_match_the_active_set` (runs in CI: 446 snapshots, 5,808 arrays read, rooms = replayed active set; arrays not compared) |

Every harness compares exactly and stops at the first difference
(METHODS M01) with a `raw::Mismatch { source, at, field, detail }`: `at`
is the line index of raw recordings (header = 0, as `check_units.py` /
`check_stats.py` report it) or the `seq` (packets, format-1 traces).
Each fixture passes its Python checker (`check_units.py` 0 errors,
`check_stats.py` 0 errors, `check_packets.py` R1–R7 clean,
`check_rooms.py` C1–C4 clean); units and stats fixtures were generated
from the checkers' own `synthetic()` recordings, so CI compares d2-sim
against the Python spec models, not against itself. Results on them:

- units: d2-sim's `schedule` equals `check_units.py`'s `anim_schedule`
  for all five forms in the fixture (main, percent, frames p = 0, start
  frame reading byte −1, main on a monster); one cancel compared.
- stats: the whole hand-built recording replays through `StatLists`
  (16 operations, 5 callbacks with their values, the max-life rescale
  inside the callback, one expiry free, 3 snapshots); a dumped
  player+item tree is rebuilt by d2-sim from base values and the chain
  alone (full and mod arrays equal); the recorded load-time columns of
  itemstatcost equal `d2_data::fixup::records::stat_ops`.
- packets: `d2-server` dispatch returns the recorded result codes for an
  accepted and a refused point message and drops the no-game client.

**Unverified** (M02): no harness has run on a 1.14d recording. No
`Covers:` claim was added: a claim in `crates/conformance/` counts as
the `trace` tier, i.e. verified (`docs/COVERAGE.md` §2), and only a
passing recording earns that. Add claims to the ignored tests once they
pass locally.

## 2. Code map

| File | What |
|---|---|
| `crates/conformance/src/raw.rs` | `RawRecording::{load, parse, from_records}` (header `format` check, M20), `TICK_RAW` / `STATS_RAW` / `PACKETS_RAW`, `raw_dir()` (`traces/raw/` or `$D2_TRACES_RAW`), `raw_files(suffix)`, `Mismatch`, `HarnessError` |
| `crates/conformance/src/units.rs` | `replay_anim`, `UnitStats` |
| `crates/conformance/src/stats.rs` | `replay_stats`, `StatsStats` |
| `crates/conformance/src/packets.rs` | `PacketServer` trait, `replay_packets`, `PacketStats`, `DispatchServer`, `Capture` |
| `crates/conformance/src/rooms.rs` | `RoomModel` trait, `replay_rooms`, `RoomStats` |
| `crates/conformance/tests/{units,stats,packets,rooms}_replay.rs` | fixture tests + ignored recording tests |
| `crates/conformance/tests/fixtures/` | `units-anim.jsonl`, `stats-lists.jsonl`, `packets-dispatch.jsonl`, `packets-scripted.jsonl`, `rooms-adjacency.json` (hand-built, our own numbers) |
| `crates/conformance/Cargo.toml` | + `d2-server` (no Bevy; `depcheck` OK) |

## 3. Formats used, and how each record is read

No format-1 converter exists for units, stats or packets, so those
readers read the recorders' raw JSON lines as documented in
`tools/trace-recorder/README.md` and as the `check_*.py` scripts read
them. When converters to `traces/FORMAT.md` are written, add a reader
for their output beside these (the replay cores take records, not
files).

- **units** (`tick-raw-1`): per `anim` record: form from `fn`
  (`0x5539b0` main with `b`, `0x553b10` percent, `0x553c70` frames,
  `0x553dc0` start frame, each with `arg`), `f`, `sp`, `fc`, `cur`, event
  bytes from `ev` pairs, byte −1 = `ad_speed >> 24`. The group is the
  records right after it: `cancel`s, then the unit's `set`s of the
  delayed list with type 0 / 1 up to the ENDANIM; compared: type,
  requested expire (`req`, else `x`), a1, a2; cancels against
  `Schedule::cancels` and the unit's live type-0 / 1 timers (tracked from
  `set` / `cancel` / `ex`). Skipped and counted: `seq: true` (sequence
  bytes are not recorded), event index outside the record. `site` is not
  used (no row table in Rust); `check_units.py` U1 / U11 cover it.
- **stats** (`stats-raw-1`): `stab` → `StatData` (itemstatcost rows
  rebuilt into 0x144-byte records, `stat_ops` derivation compared;
  charstats from `cs`; states table of `nstates` rows with the `life`
  group from `life_states`). `ssd` / unseen `ssn` lists are seeded
  through d2-sim operations with callbacks muted, then base / flags
  fixed, state bits toggled, mods rebuilt with `unit_set`, and the
  dump compared. Operations map to `set` (with the `u` unit when
  present), `add`, `remove_all`, `attach(U, L, r)`, `detach`, `free`,
  `make_dynamic` / `make_static` (`swap` false; skipped when the item
  list is not on that unit, as `check_stats.py` does), `by_time_refresh`,
  `toggle_state`, `expire_lists(U, f)`. Callbacks: d2-sim's
  `on_callback` events in order against `scb` (L, key, old, new, and `u`
  when the field exists). Records inside a callback: `ss` of stat
  max − 1 on the callback's list is d2-sim's own rescale (not replayed);
  any other is host work, applied after the operation with its own
  callbacks compared. Every list carries a marker remove callback so the
  harness sees d2-sim's detach order (compared with `sxf` in expiry).
  `act_time` comes from `senv` and the units' `act`.
- **packets** (`packets-raw-1`): `c2s` → `game_message(client, bytes,
  size, ms)`, `c2s_sys` → `system_message`, `tick` → `tick`; `dispatch`
  / `result` (`dispatched`, `code`) compared with the returned code; the
  `s2c` records between two inputs compared, in order, with
  `take_sent()` (client, size, every byte). `net` / `flush` are not
  replayed.
- **rooms** (format 1, `sim/tick`): `room_activate`,
  `room_deactivate`, the player's `room_add` / `room_remove`, `lists`
  (`acts[a][r].room`, `.adj`) by `data.seq`.

## 4. Gaps and findings (not fixed: outside the owned files)

1. **Rooms have no d2-sim provider.** `Drlg`'s arrays come from the
   rooms-near order, i.e. tile rects the tick recordings lack. Next step
   once the `rooms.md` Test vectors "Full check" recorder extension
   exists (per active room: DRLG room, rect, level, rooms-near array;
   format to be defined by its spec session): a `RoomModel` that builds a
   `Drlg` level from the recorded rects (a `LevelTypes` returning the
   rects and a floor grid, `stream_room` on activation,
   `remove_active_room` on removal) and returns
   `Drlg::adjacent_rooms`.
2. **`convert_tick.py` rejects `record_tick.py` 0.2.0 recordings**: its
   `convert()` raises "unknown record kind" for `anim` (handled kinds:
   header, game, footer, tick, step, set, cancel, ex, hin, hout, rin,
   rout, qin, qout, qclear, ract, rdeact, snap). The units harness reads
   the raw file and is not affected; converting the A1 recording to a
   `sim/tick` trace will fail until `anim` is skipped there (tools
   owner).
3. **Stats replay limits** (counted or documented in `stats.rs`): the
   regeneration entry points are not replayed (d2-sim's
   `player_regen` / `monster_regen` need a whole `Sim`); their writes are
   applied as recorded. Host work inside callbacks is applied after the
   operation, not at its place inside it. monstats `DamageRegen` and
   itemstatcost `itemevent1` are not in the recording (the monster
   HPREGEN write is applied as recorded; `item_event` is a no-op).
4. **Packets through `d2-server` will fail on today's recordings**: the
   ignored test replays with an empty `SimGame` (no session code builds
   the recorded game, Phase 5), so it reports the first message the
   original dispatched as dropped. That is the measured state, not a
   harness fault; it passes when session + world reproduce the recorded
   game and every S→C sender exists.

## 5. Local checks to queue (`docs/HANDOFF.md` §5)

Run from the repo root on the local PC; `D2_TRACES_RAW=<dir>` points the
tests at another folder.

1. After A1 (`record_tick.py` 0.2.0 recording in `traces/raw/`):
   `cargo test -p conformance --test units_replay -- --ignored --nocapture`.
   Expect: pass, printed `schedules` > 0. A failure names the record
   index; cross-check with `py tools/trace-recorder/check_units.py <file>`
   (U4 "anim exact" at the same index).
2. After A2 (`record_stats.py`):
   `cargo test -p conformance --test stats_replay -- --ignored --nocapture`.
   Expect: pass with `operations`, `callbacks`, `snapshots` > 0. A
   mismatch names the record (same numbering as
   `check_stats.py`); a `seed …` field means d2-sim does not rebuild a
   dumped tree (`stat-lists.md` §6 / §11), `isc[s].…` a load-time column
   (`fixups.md` §2), `callback.u` the unit argument of §7.
3. Existing packets recordings:
   `cargo test -p conformance --test packets_replay -- --ignored --nocapture`.
   Expect today: FAIL at the first dispatched message (§4 item 4). Record
   the seq and the message id as the baseline.

## 6. Gate (this branch)

`cargo fmt --check`; `sh tools/cloud-setup.sh` then
`cargo clippy --workspace --all-targets -- -D warnings`;
`cargo test -p conformance` (36 passed, 3 ignored: the recording tests);
`cargo run -p depcheck` (OK, 8 crates);
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`
(21 OK); `python3 tools/coverage.py --check` (0 errors) and
`--selftest` (ok). All pass.
