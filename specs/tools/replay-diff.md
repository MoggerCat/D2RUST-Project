# Spec: Tools — Replay a recorded 1.14d input stream on d2rs (`replay-diff`)

- **Status:** draft: `tools/replay-diff/` implements §1–§5
  (`record_replay.py` + `c2s_tap.py` on 1.14d, `replay_diff.py` run /
  compare / sends); selftest green; first runs on the two sessions of
  `tools/replay-diff/sessions/` in `docs/handoff/q-tool-replay-diff.md`.
- **Target version:** 1.14d (the original side); the formats are d2rs-own.
- **Crate/module:** `tools/replay-diff/` (Python stdlib); the d2rs side is
  `d2-client state-dump` unchanged (`--send`, `--poke`, `--packets`).
- **Related specs:** `tools/state-snapshot.md` (the state-1 files and the
  per-frame comparison order), `tools/packets-trace.md` (the C→S records,
  windows, masks), `tools/scenario-diff.md` (check syntax §2, the shared
  input script §2 r4, `at … send` and the injection point §3 r12),
  `sim/intents-events.md` §6 r1 (the per-frame unit of input).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 36–51 |
| Inputs | 52–60 |
| Outputs / state changes | 61–70 |
| Rules | 71–72 |
|   1. Sessions | 73–89 |
|   2. The 1.14d recording | 90–107 |
|   3. The d2rs replay | 108–137 |
|   4. Comparison | 138–178 |
|   5. Verdict | 179–186 |
| Constants & data dependencies | 187–192 |
| Randomness | 193–197 |
| Edge cases & original bugs | 198–216 |
| Test vectors | 217–223 |
| Provenance | 224–229 |
| Open questions | 230–242 |
<!-- /index -->

## Summary

A long-horizon check. One 1.14d session is played by a frame-anchored
input script (minutes of walking, fighting, picking up, town, waypoint)
under the state recorder with a C→S tap: one run gives the state after
every server tick **and** every client → server message with the frame
whose drain took it. d2rs then gets exactly those messages, frame by
frame, injected after the bridge's duplicate filter (no input script, no
client decisions of its own beyond the bridge's fixed answers), with the
same save, seed and pokes. The two state files are compared frame by
frame over the whole run (10⁴+ frames, streamed): the first divergence,
then how much of the run diverges after it (the drift: rate, equal
stretches, per unit type, per field, per bucket of frames). The input
channel checks the replay itself: the C→S messages d2rs' server drained
per frame against 1.14d's.

## Inputs

| Name | Type | Source |
|---|---|---|
| session | `.replay` file (§1) | `tools/replay-diff/sessions/` |
| 1.14d game | under the recorder's debugger | `game/Game.exe` (private data repo), Wine in the cloud |
| d2rs game | `d2-client state-dump` | the real install (`D2_GAME_DIR`) |
| C→S masks | TSV | `specs/tools/scenario-masks-c2s.tsv` (`scenario.md` §6 r4) |

## Outputs / state changes

- Work dir `traces/raw/replay-<name>/` (gitignored): `orig.state.jsonl`
  (state-1 with `c2s` lines, §2), `d2rs.state.jsonl`,
  `d2rs.packets.jsonl` (packets-raw-1), `replay.summary.json` (§4 r7),
  the save, the 1.14d run directory.
- The report on stdout and the exit code (§5).
- Neither game changes because it is recorded: the tap only reads
  (packets-trace.md §2 r1 for the d2rs recorder).

## Rules

### 1. Sessions

1. A session file is a check file of `tools/scenario-diff.md` §2
   (`check 1`, `name`, `save`, `seed`, `ticks`, `seconds`, `difficulty`,
   `at … poke`, `ignore`), named `<name>.replay`, plus continuation
   lines `input+ <steps>`: each is appended to the `input` line with
   `; ` (a long script stays one script, one step group per line).
2. The input is the shared frame-anchored script (scenario-diff.md §2
   r4). Only 1.14d plays it; d2rs never gets it (§3). `input orig` /
   `input d2rs` are refused (no shared timing), and so are `at … send`
   lines: 1.14d records a sent message as C→S, so d2rs would get it
   twice.
3. Pokes run on both sides at their frame (`poke.md` §2 r6), as in a
   check: a session may fix the game seed or spawn monsters so that a
   divergence known from a short check does not hide the long run.
   `channels` and `draws-at` are ignored.

### 2. The 1.14d recording

1. `record_replay.py` is `record_state.py` with every option unchanged
   (`--auto`, `--seed`, `--ticks`, `--seconds`, `--poke`, `--input`,
   `--snap-every 1`), plus the C→S tap of `c2s_tap.py` armed on the
   game entry `0x0053F3D0` and the system entry `0x0053F100` (the hooks
   of `record_packets.py`, packets-trace.md §1 r3: ECX = client id then
   message, EDX = size).
2. Each tapped message is a line `{"k":"c2s","f":F,"q":"game"|"sys",
   "client":C,"size":N,"bytes":"<hex>"}` in the state file, written
   when it is drained. F = the game +0xA8 read at the last tick-return
   stop `0x0052FD1E` plus one: the frame whose drain takes it (the
   drain before tick F, intents-events.md §6 r1); `null` before the
   first tick return. Bytes are the drain copy (at most 0x1FC).
3. The file stays state-1: readers skip the `c2s` kind (state-snapshot
   §1 r5). The header's `tool` names both tools; the footer notes the
   tapped count.

### 3. The d2rs replay

1. `d2-client state-dump --save S --seed N --difficulty D --no-own-c2s
   0x2F,0x31,0x5F [--poke …] --send "F hex <bytes>"… --ticks T --out d2rs.state.jsonl --packets
   d2rs.packets.jsonl`, the session's save, seed, difficulty, pokes and
   ticks; one `--send` per recorded message in recorded order. state-dump
   injects the sends of frame F after the snapshot of frame F − 1 and its
   pokes, before frame F's drain, in command-line order, after the
   duplicate filter (scenario-diff.md §2 `at … send`, `scenario.md` §4 r2
   (a)): the drain before tick F takes them, as on 1.14d.
2. Not replayed: messages with no frame (before the first tick: the
   create / join sequence the bridge runs itself), empty ones, and the
   two ids the d2rs bridge keeps sending itself: 0x67 (create) and 0x6B
   (the answer to S→C 0x02; measured 2026-10-09 in the same frame window
   as 1.14d's on all three sessions). Every other recorded message is
   replayed, and the bridge's own sends of 0x5F (position resync,
   `bridge/check.rs`), 0x2F and 0x31 (talk and dialog reply on the
   server's NPC interaction) are dropped by `--no-own-c2s`
   (`tools/scenario-diff.md` §3 r13; each drop is a footer note). Settled
   REC-1370 (2026-10-09): with this split the input channel (§4 r6) shows
   0 differing windows on smoke-town-ama (4), town-ama-10k (312) and
   bloodmoor-bar-10k (226). Before the switch the bridge's own 0x5F
   (its model stays at a walk's start under `--send`) was 87 of 398
   town windows and the first state divergence of the smoke run, and its
   0x2F was sent twice when also replayed (town frame 633).
3. Nothing else is decided on the d2rs client side: no input script, no
   clicks, no hover pick; the messages carry 1.14d's GUIDs and
   coordinates as recorded, so a GUID that differs between the games
   shows as the state divergence it causes.

### 4. Comparison

1. **Frames.** Both state files are read once, front to back (frames
   ascend in each; a frame not above the previous one is an error); the
   frames present in both are compared, in order (`--from`, `--to`
   narrow it). Memory does not grow with the run length.
2. **Within a frame**: exactly state-snapshot.md §4 r2–r3: the game seed,
   then units in (`ut`, `g`) order over both sides (`missing` / `extra`),
   then the fields both headers list and `--ignore` / the session's
   `ignore` do not name, in the §2 table order; `--types` limits the
   unit types.
3. **First divergence**: its frame, unit, field, both values and both
   unit records; then the next `--next N` (default 20).
4. **Drift after it.** Over the frames compared from the first
   divergence's frame to the last: how many have any difference (the
   divergence rate, as n / m frames), the equal stretches among them
   (count, the first and the longest), the longest run of diverged
   frames. Per unit type (and the game seed): its first frame and the
   frames with a difference of that type over the frames from its first
   on. Per field: the first place and the number of differing (frame,
   unit) pairs. Per bucket of `--bucket B` frames (default 1000): frames
   with a difference over frames compared.
5. A frame is "diverged" when any compared value of it differs; a frame
   with no difference is "equal" even when it follows a divergence
   (re-convergence is reported, never assumed).
6. **Input channel.** 1.14d's `c2s` lines and d2rs' `c2s` / `c2s_sys`
   records (window = `frame` + 1, or `frame` when `phase` is `tick`,
   packets-trace.md §3 r1), per frame window inside the compared frame
   range, index by index: the queue (game / system), the id, the size,
   then the bytes from offset 1 outside the C→S mask table's masked
   bytes (packets-trace.md §3 r5). Report: windows, windows diverged,
   messages per side, the first five divergences with both windows' id
   lists. No d2rs packets file: not compared.
7. `--json FILE` (and `run`'s `replay.summary.json`): format
   `replay-summary-1`: code, verdict, frames compared, frame range,
   frames equal, differences, the first divergence, `after_first`
   (frames, diverged, equal stretches, longest diverged run), `by_type`,
   `by_field`, `buckets`, `input` (counts and the first divergence).
8. `--perturb F:T:G:FIELD` (self-test aid, state-snapshot.md §4 r6)
   changes that d2rs value before comparing.

### 5. Verdict

Exit 0 `MATCH`: no state or input divergence, every field of either side
compared, no header gaps, the input channel compared. 1 `DIVERGED`: any
state or input divergence. 2 `PARTIAL`: no divergence, one of the MATCH
conditions not met. 3: error (unreadable or non-state-1 file, frames out
of order, no common frame, a refused session line, a failed run).

## Constants & data dependencies

Hook addresses `0x0053F3D0`, `0x0053F100`, `0x0052FD1E`, game +0xA8
(owned by `tools/packets-trace.md`, `tools/original-hooks.md` §3,
`sim/tick.md` §2); the drain copy limit 0x1FC; the bridge-own ids (§3 r2).

## Randomness

None in the tool. Both games run from the session's seed; pokes may set
seeds on both sides.

## Edge cases & original bugs

1. The 1.14d client sends messages the input did not cause (keep-alive
   0x6D, the walk resync): they are replayed like any other; their bytes
   (tick counts) are d2rs' input as recorded.
2. A message 1.14d's server drained but did not dispatch (no player
   yet, a refused id) is replayed all the same: the d2rs server decides
   it again from the same bytes.
3. If the 1.14d tick driver runs a catch-up of several ticks between
   two drains (the debugger stops the game), the messages still carry
   the frame of their drain; d2rs, stepping one tick per pump, injects
   them before the same tick.
4. A session longer than the 1.14d `--seconds` limit ends early on
   1.14d; the comparison covers the common frames only and says so in
   its frame range.
5. Thousands of `--send` values go on one command line (Linux: well
   inside `ARG_MAX` for 10⁴ frames of play; the run prints their count,
   not the line).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| `replay_diff.py --selftest` | a 12,000-frame synthetic file against itself with an equal replayed input: MATCH; the `--send` values of a tapped stream (frame, hex, the pre-tick and bridge-own messages left out); a perturbed field at frames 1, 777, 5000 (seed), 11999 is the first divergence at exactly its place with rate 1 / (frames from it); a drift on one unit over frames 3000–8999 with a missing unit at 6000–6009: first divergence frame 3000, rate 6000 / 9001, one equal stretch, longest diverged run 6000, per type, per field and per bucket counts; `--types` / `--ignore` narrow it; a changed byte, a missing, an extra, a reordered message and a queue change in the input channel are reported at their window and index; no packets file or a one-sided field: PARTIAL; descending frames and a missing file: error; a session's `input+` lines join, a `send` line is refused | this tool |
| `c2s_tap.py` (selftest) | the record of a message before and after the first tick return; another game's tick return ignored; a message longer than the drain copy truncated to 0x1FC bytes | this tool |

## Provenance

d2rs-own tool. The hook points and the frame rule restate
`tools/packets-trace.md` §1 r2–r3 and `sim/intents-events.md` §6 r1;
nothing here is a new 1.14d fact.

## Open questions

1. ~~REC-1370 (§3 r2): the bridge-own id set.~~ Settled 2026-10-09:
   §3 r2 (`--no-own-c2s`, q-fix-replay-hooks).
2. REC-1371: the d2rs `state-dump` clock starts at 1 s and steps 40 ms
   per tick; 1.14d's server clock is wall time under the debugger. A
   handler that reads the host clock (the waypoint's hostile delay,
   `world/waypoints.md` §6.1) can take a different branch on the two
   sides from the same bytes. PROVISIONAL: the replay does not align the
   clocks; settled when a replay's first divergence is traced to a
   clock read (then the session pokes or state-dump would need a clock
   option, owned by the state-dump session).
