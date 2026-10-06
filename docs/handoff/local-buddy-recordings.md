# local-buddy: recordings of the original 1.14d (task `rec`)

Branch `claude/local-buddy-recordings-2026-10-07` (from
`origin/claude/specs-staging` + `spec-spawn-hooks-buddy`,
`local-buddy-saves-2026-10-07`, `local-buddy-scenarios`; the only merge
conflict was one README table row each side added: both kept). Local PC,
user's own 1.14d install (`Game.exe` reference hash, checked by the tools
on every run). One game at a time under `C:\d2slots\game.lock`. Raw files
live only in `C:\Users\zffit\Desktop\D2test\traces-raw-buddy\` (never
committed); no normalized trace was committed because no spec asks for one
of these recordings in a fixed place.

## Tool changes (committed)

- `spawn.py` 0.2.0: `--level`, `--trigger`, `--status`, `--packets`
  (README "spawn.py"). Reason: a kill needs a field level (towns forbid
  attacks), spawns must wait until the player has walked out, several
  spawns per game are needed to level a character, and the kill and drop
  are visible only in the messages.
- `d2ui.py`: a one-action CLI to drive the window from a shell.

Driving: the game window takes `PostMessage` mouse / keys; target clicks
are computed from the `--status` snapshot (screen offset of a monster =
600 + 24 (dx − dy), 445 + 12 (dx + dy) in the 1200 × 900 client at 150 %
scaling, subtile deltas). The status snapshot also gave the act's level
rects (town of `bdBar`: level 1 at tiles (1200, 960) 56 × 40, Blood Moor
(1160, 1000) 96 × 56: the exit is on the town's +y side).

## Queue

| # | Item | Status | Raw file (local only) | Check | Key numbers |
|---|---|---|---|---|---|
| 1 | monster kill with item drop | **done** | `kill1-spawn.jsonl` (spawn-raw-1), `kill1-spawn-packets.jsonl` (packets-raw-1, 40,284 events) | `check_rng.py`: OK (46 inline draws, 0 unresolved, 7 `chain_unexplained`, 10 draw sites); `check_packets.py`: **6 failures**, all `s2c id 0x3e size 34 != rule 7 (R5)` (finding F1) | see "Item 1" |
| 7 | Kick / Bash on a monster | **Bash done; Kick blocked** | same files | as item 1 | see "Item 7" |

## Item 1: kill with drop

`bdBar` (Barbarian, level 1 at start), `spawn.py --level 2 --trigger …
--status … --packets -- -w -ns -name bdBar -bar` (save kept: the
character reached level 2 and Save And Exit wrote `bdBar.d2s`, 987 bytes).
Walked out of the Rogue Encampment through the south gate into Blood Moor
(level 2) and killed with left clicks (skill 0, C→S 0x06 on the monster):

| Monster | Spawned (frame) | Killed (0x69 state 8 / 9, frame) | Drop (S→C 0x9C action 0, frame: GUID code) |
|---|---|---|---|
| 2 natural quill rats (class 63, GUIDs 12, 13) | — | 4251 / 4265; 4351 / 4365 | none |
| champion zombie (class 5, GUID 14, umod 16; 1 unit, 8 draws in the call) | 4686 | 5250 / 5274 | 5250: 9 `tkf`, 10 `hp1`, 11 `hp2` |
| champion fallen pack (class 19, GUIDs 15–17; 3 units, 23 draws) | 6261 | last 7338 | 6479: 12 `gld`, 13 `hp1`, 14 `mp2` |
| champion fallen pack (GUIDs 18–21; 4 units, 34 draws) | 6899 | last 7562 | 7338: 15 `ssd`, 16 `hp1`, 17 `mp2` |

The champion zombie's last hit was C→S 0x06 at frame 5243; its three
drops, the 0x69 state-8 message and the death are all in frame 5250, the
state-9 (dead) message in frame 5274. Items 11 and 10 were picked up (C→S
0x16 at frames 5615, 5938; then 0x9C action 14 / 4). Item codes were
read from the 0x9C bit stream (code at bit 141 for action 0). Per-spawn
seeds (game seed before / after the call, room seed) are in the
`spawn` records; the first: game `[329514667, 1578085098]` →
`[2160383105, 137438053]`.

## Item 7: Kick / Bash

- **Bash: done.** After the kills the Barbarian was level 2 (xp 699); the
  skill point went to Bash (C→S 0x3B `7e00`, frame 7878), Bash was set as
  the right skill (C→S 0x3C `7e000000ffffffff`, frame 8421) and used on
  two spawned normal zombies (class 5): C→S 0x0D (right skill on unit) on
  GUID 22 at frame 8813, killed at frame 8820 (one Bash; drop GUID 18
  `aqv`), and on GUID 23 at frame 9106 (state 8 at frame 9113).
- **Kick: blocked.** Kick (skill 1) is innate for every class
  (`charstats` Skill 2) but has no `leftskill` / `rightskill` flag, and
  the right-skill menu of the Barbarian shows no Kick icon (screenshot
  `out-rec\k24.png`): it cannot be selected through the UI. Recording it
  would need an injected C→S 0x3C select of skill 1 (`run_scenario.py`
  can inject, but cannot spawn yet) — a message no unmodified client
  sends, so it was not done.

## Findings

- **F1 (S→C 0x3E size).** Every `UpdateItemStats` (0x3E) message the
  server queued (`0x0053B280`, caller `0x53D200` in the sender
  `0x0053D130`) has queue size 34 while its length byte (u8@1, the rule
  in `specs/sim/server-messages.tsv`) is 7, e.g. frame 5085
  `3e071022b100…` (34 bytes, 27 trailing zeros). Six times in the run
  (frames 5085, 5116, 7091 …, while fighting). Either the spec's size
  rule describes the message as the client reads it and the queue size is
  a fixed 34-byte buffer, or the rule is wrong; owner
  `specs/sim/intents-events.md` / `server-messages.tsv`. Not loosened in
  the checker.
- Rain in the Rogue Encampment at game start for `bdBar` in both runs
  (screenshots `k01`, `k11`), stopped within a few minutes (Blood Moor
  screenshot `k19` dry, `k17` raining).
- `check_rng.py` reports 7 `chain_unexplained` draws on an OK verdict
  (five spawn calls in one game; the spawn-only runs of
  `local-buddy-spawn.md` had none listed): worth a look by the RNG owner.
