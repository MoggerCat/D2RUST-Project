# q-tool-replay-diff — hand-back (2026-10-09)

Session q-tool-replay-diff, branch `claude/q-tool-replay-diff`. Task:
`docs/handoff/q-tool-replay-diff-task.md` (fidelity-gaps.md §4 `replay-diff`).
REC ids used: REC-1370, REC-1371.

## Done

- **Tool** `tools/replay-diff/` (spec `specs/tools/replay-diff.md`):
  - `record_replay.py` = `record_state.py` unchanged + `c2s_tap.py` (hooks
    `0x0053F3D0` / `0x0053F100`): **one** 1.14d run gives the state after
    every tick and every C→S message with the frame whose drain took it
    (`{"k":"c2s","f":F,...}` lines in the state-1 file).
  - `replay_diff.py run SESSION` builds the save, records 1.14d (input script
    played by autostart), replays the recorded C→S on d2rs (`state-dump
    --send "F hex ..."` per message + the session's pokes, no input), records
    d2rs' packets, compares. `compare` streams both files (constant memory,
    10⁴+ frames), reports the first divergence, the next N, the **divergence
    rate after it** (diverged / compared frames from it on), equal stretches,
    longest diverged run, per unit type, per field, per 1,000-frame bucket,
    and the **input channel** (C→S drained per frame on both sides, masked
    as packets_diff); says when the input diverged first. `--types`,
    `--ignore`, `--from/--to`, `--json` (replay-summary-1).
  - Sessions `tools/replay-diff/sessions/`: `smoke-town-ama` (300 frames),
    `town-ama-10k`, `bloodmoor-bar-10k` (10,000 frames each; `input+`
    continuation lines; input generated once by a fixed LCG, committed).
  - `replay_diff.py --selftest` green (12,000-frame synthetic run, perturb at
    exact places, drift / rate / stretches / buckets, input channel cases,
    session parsing, the tap).
- **Runs** (1.14d under Wine in the cloud; ~20 min per 10k-frame recording):

| Session | Frames | First divergence | Rate after it | Player first | Input channel |
|---|---|---|---|---|---|
| smoke-town-ama | 139 (1.14d paused, see below) | 86 player m 5 vs 6 | 54/54 | 86 | 1/5 windows: the bridge's own 0x5F at 86 |
| town-ama-10k | 10,000 | 4 item flags `if` 0x80010 vs 0x80000 (poked hp1, gld, r05) | 9997/9997 | 494 (6203/9507, 17 equal stretches) | 90/398 (87 bridge 0x5F, first 374) |
| bloodmoor-bar-10k | 10,000 | 5 game seed + Blood Moor population after `warp 2` | 9996/9996 | 61 | 52/272 (all bridge 0x5F, first 145) |

  Findings (each is a first divergence of something, owners below):
  1. **Replay fidelity (state-dump, blocks an exact replay)**: under `--send`
     the bridge has no walk prediction for the local player, so its position
     check (`bridge/check.rs` `correct()`) sends C→S 0x5F with the walk's
     start point; 1.14d never sends it. In smoke it is the first state
     divergence (the server walks the player back). Needs a state-dump switch
     that silences the bridge's own C→S after 0x67. Routed to the
     coordinator (the state-dump owner session is gone).
  2. **0x16 PickItem (items)**: town, input identical through frame 373.
     1.14d removes the picked gold 4:3 (class 523) at frame 21 and moves the
     leather armor 4:4 into the inventory (m 0) at 71; d2rs dispatches the
     same bytes and sends S→C `0a 04 03000000` but both items stay on the
     ground (m 3, same x/y) in its server unit list all run.
  3. **Poked item flags (items / poke)**: `poke item hp1|gld|r05` gives flags
     0x80010 on 1.14d, 0x80000 on d2rs (bit 0x10) from frame 4; cap (magic)
     and lea (rare) equal.
  4. **Blood Moor population / GUIDs (population, unit order)**: after
     `warp 2` the populations differ (frame 5); the poked fallen party is
     GUID 19–21 on 1.14d, 17–19 on d2rs, so the replayed attack `06 01000000
     14000000` (frame 61) hits nothing on d2rs. Same cause as
     combat-melee-fallen's known GUID offset.
  5. Positive: the player matches on every compared field through the
     replayed 0x01 (smoke, 10–85) and through 5 pick-ups and 3 runs (town,
     to frame 493), each time until the bridge's own 0x5F.
- **Ledger** part `docs/handoff/ledger/q-tool-replay-diff.tsv` (6 rows,
  validated with q-fidelity-ledger's `ledger.py`: 0 format errors):
  `net.c2s.0x16` → DIVERGED (note: items.tsv's row wins the name-order merge;
  the integrator should take this one's state/note), `net.c2s.0x01`,
  `net.c2s.0x03` (notes: replay evidence, still NO-CHECK),
  `system.replay.town-ama-10k`, `system.replay.bloodmoor-bar-10k`,
  `system.replay.input-fidelity` (DIVERGED).

## Update 19:55 UTC: exact replay (state-dump `--no-own-c2s`)

q-fix-replay-hooks added `state-dump --no-own-c2s` (merged here, 369a81f8).
replay_diff.py now replays every recorded message but 0x67 / 0x6B and drops
the bridge's own 0x2F, 0x31, 0x5F (REC-1370 settled). d2rs side re-run on the
same 1.14d recordings:

| Session | Input channel | State | Player |
|---|---|---|---|
| smoke-town-ama | 0/4 windows differ | **no difference, frames 1–139** (PARTIAL only: `own`/`q` one-sided, header gaps) | equal throughout (two 0x01 walks) |
| town-ama-10k | 0/312 | first frame 4 (item flags, finding 3), rate 9997/9997 | equal to frame 622; then 1384/9378 frames, 5 equal stretches (longest 1561) |
| bloodmoor-bar-10k | 0/226 | first frame 5 (population, finding 4), rate 9996/9996 | first 61 (GUID offset, finding 4) |

New finding 6 (town NPC wander, act1-town / NPC tracks owner): NPC 1:7
(Warriv, class 155) has the same unit seed on both sides but its wander path
target differs at frame 287 (ty 4228 on 1.14d, 4229 on d2rs, mode 2). The
player's first divergence (frame 623) follows from it: the replayed 0x04
(run to unit 1:7, repeated every ~6 frames from 600) re-targets to the NPC,
which is one sub-tile apart by then.

Finding 1 is fixed (by q-fix-replay-hooks); 2, 3, 4 stand unchanged.

## Open

- REC-1370: settled (update above).
- REC-1371: the host clocks are not aligned (state-dump starts at 1 s,
  steps 40 ms; 1.14d is wall time under the debugger). Not yet seen as a
  cause.
- autostart.py: `clickunit` posted clicks outside the 800×600 window
  (`clickunit 2 119` → (928, 316)); fixed by q-fix-replay-hooks (refuses
  off-screen units). The recordings here predate the fix. In the smoke session that click
  became a walk (0x01). In town-ama-10k the waypoint was on screen later:
  1.14d opened it 7 times (0x13 on object 2:10, frames 1615–7117) and
  closed it by walking away (0x49 `0a000000 00000000`, no travel: the
  player stays in level 1), and talked to NPCs 3, 7 and 12 (0x13 → 0x2F,
  0x38, 0x31, 0x30; first at frame 632). C→S ids recorded: town 01 02 03 04
  13 16 2f 30 31 38 49 59 6b 6d; Blood Moor 03 04 06 13 16 6b 6d.
- `key ESC` with no panel open opens the single-player menu and **pauses**
  1.14d (smoke: no tick after frame 139). Sessions close panels by walking
  away instead.
- Not covered by these sessions: waypoint travel to another level, vendor panels.
- 0x6D (ping): d2rs drains it as a system message with no reply; whether
  1.14d answers (S→C 0x8F) is not in this recording (no S→C hooks).

## Repro

```sh
sh tools/coord/session-setup.sh            # from the repo (the script finds the root from its own path)
cargo build --release -p d2-client -p d2s-tool
export D2_GAME_DIR=$HOME/game D2RS_BIN_DIR=$PWD/target/release
python3 tools/replay-diff/replay_diff.py --selftest
python3 tools/replay-diff/replay_diff.py run tools/replay-diff/sessions/town-ama-10k.replay
# a second session in parallel: its own prefix and display, and its own lock file
cp -al ~/.wine-d2 ~/.wine-d2b && rm ~/.wine-d2b/.run.lock
WINEPREFIX=~/.wine-d2b D2_DISPLAY=:97 python3 tools/replay-diff/replay_diff.py run tools/replay-diff/sessions/bloodmoor-bar-10k.replay
# re-compare / narrow (work dir traces/raw/replay-<name>/):
W=traces/raw/replay-town-ama-10k
python3 tools/replay-diff/replay_diff.py compare $W/orig.state.jsonl $W/d2rs.state.jsonl --packets $W/d2rs.packets.jsonl --types 0
# d2rs side only after a d2rs change (keeps the 1.14d recording):
rm $W/d2rs.*.jsonl; python3 tools/replay-diff/replay_diff.py run tools/replay-diff/sessions/town-ama-10k.replay --reuse
```

`cp -al` hard-links `.run.lock` too: remove it in the copy, or the two runs
serialize.
