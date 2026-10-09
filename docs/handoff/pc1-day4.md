# Hand-back — PC 1 day 4, 2026-10-09 (branch `claude/local-pc1-day4`)

One local session (no parallel PC1 sessions were running), with three
analysis subagents for the binary reads. REC block 1100–1149 (A 1100–1109,
B1 1110–1119, B2 1120–1129); used: REC-1110 only. Step 4 items are numbered
47–58 in `pc1-data.md`.

Several items in the day-4 brief were already answered by the day-3
sessions: Hratli's unit seed (`pc1-day3-c.md`, row
`q-fix-p3-quest-spawn-creation`), the Quill Rat lines (`pc1-day3-b.md`,
row `q-fix-c3-quillrat-choice`), items 45–46 combat-melee-fallen /
combat-potion-midfight (both sides, `pc1-day3-b.md`), REC-816 (settled) and
REC-815 (narrowed to REC-900, needs the anim recorder tools, not in this
tree). They were not redone.

## Items answered

| # | Item | Answer (owner spec) | Rows |
|---|---|---|---|
| 47, 48 | Player alignment at the join and the frame-2 resend (0xA8 state 105 + 0x1D 12, 0, 2) | Allocation `0x00555230` → player init `0x005348C0` → `0x005543B0(P, 2, 1)` at `0x0053495A`; the changed bit (`0x00639DB0`, resend `0x00639E30` at `0x0055448A`) survives to frame 2 because the queue insert `0x0064C040` is a no-op without a room; the game-entry placement `0x00554850` queues P at `0x005549F3`; frame 2's update `0x00580860` sends 0xA8 then single stats 12, 0, 2. Corpses get the alignment too (`sim/intents-events.md` §8.2 r9, `combat/hit.md` §7.1) | `q-fix-pc1d4-alignment-queue`, `q-fix-pc1d4-alignment-in-player-init` |
| 50 | `0x00625870` mod-array test | Sends only when the key **is** in the mod array and present in the base array; absent from base → nothing (d2rs matches) (`sim/stat-lists.md` §11 r4) | — |
| 49 | REC-734 Jerhyn | `0x0059F570` = `0x0059F510`; no palace Jerhyn → (a, b) = (1, 0); the think reaches the interaction step, whose walk in radius (3, 2) lands on own + (2, 2), no draw (`world/quests-act2.md` §10, `monsters/ai-bodies.md` §9.9 r201) | `q-fix-pc1d4-jerhyn-seams` |
| 53 | REC-753 preparation probe | `0x00648120` writes the compute record's local target copy (`0x00649A5D`), never path +0x10; only p0 is start-tested; equal → result 0 (`sim/pathing.md` §4 r3). **Merge note:** `claude/q-diff-combat-a1` rewrote §4 r3 with PROVISIONAL REC-753 text; on merge keep this branch's text | `q-fix-pc1d4-prep-target-local` |
| 54 | REC-754 Gheed / Charsi stop thinking | Room +0x78 counts clients that **see** the room (adjacency set, `0x0061A660` from `0x0053A8E0`); the run crosses x = 4880 and drops the x-960 rooms, so `0x0053A9B0` cancels their thinks (`drlg/rooms.md` §7 r1, `intents-events.md` §7.8 r3.2). Same fix as `q-fix-p3-room-empty-think` | — |
| 55 | **Local player attack / cast mode end (blocker 1)** | Player update `0x00463390` with mode table `0x00711E00`: A1/A2/SC/TH/KK/S2–S4/SQ and GH/BL/S1 advance until complete (`0x006217C0`), then the end at `0x0046362D` (light off, `0x004611F0`, used skill none, mode 5 in town else 1); the server sends its own client nothing. The next click or held repeat acts in the loop pass after the ending update (`client/model.md` §20 r1–r8) | `q-fix-pc1d4-player-mode-end`, `q-fix-pc1d4-local-click-mode` |
| 52 | Client NPC drops a server walk | The NPC-busy bit (+0x28 bit 0) has one setter, the interact sender's NPC tail `0x00461DC0` (`0x0046210B`) (`client/model.md` §17 r7). It does not explain the panel scene: PROVISIONAL REC-1110 | `q-fix-pc1d4-npc-hold` |
| 56 | Andariel never dies / Radament at 256 hp | Not a damage bug: both milestones fire the Fire Bolt from or into a wall cell (`pos` / hop are exact teleports with no collision test), so the bolt is removed before it hits. Death test `0x0057C8E7` (life ≤ 0), sub-point floor `0x0057C865`; `DamageRegen` 0 for both; Andariel fire res −50. Fired on open floor, both die at f42 in d2rs. Radament's "mode 4" is A1 (`combat/damage.md` §5.2, `tools/playthrough.md` §1 r7) | `q-fix-pc1d4-andariel-kill-setup`, `q-fix-pc1d4-radament-kill-setup`, `q-fix-pc1d4-playthrough-windows` |

Notes from item 55: `modes.rs` `player_mode` constant names are wrong (9 = BL,
0xD = S1, 0x13 = KB); the values are right. Item 56: after the hit lands,
the boss stands up again at hp 0 in this branch's build: the known
`q-fix-p4-death-cleanup` (fixed on `claude/q-diff-combat-a1`, not yet in
staging). Whether the two cells are walls in 1.14d too was not checked
(that depends on the DRLG matching).

## RECs still provisional

- REC-1110 (`client/model.md` §17 r7): what makes the 1.14d client drop
  Warriv's walk in the panel scene. Settled by recording Warriv's +0x28,
  mode, frame and client path at ticks 50–60 of that scene, and whether the
  0x67 at tick 56 reaches his queue.
- REC-900 (from day 3): the whirl path's first step at update 3.

## Live runs

### Checkpoint quest log and waypoints (HANDOFF §5 q-tool-checkpoints)

Saves built with `d2s-tool` from staging (`tools/checkpoints/make.py`'s
build step; its `tools()` misses `.exe` on Windows, in row
`q-fix-pc1d4-playthrough-windows`), copied as `CkAndariel`, `CkMephisto`,
`CkHellforge`, `CkBaal`, loaded with `autostart.py --try` and a script
(Q, a click on each act tab and each quest icon, then the town
waypoint). Screenshots are local only:
`C:\Users\pc\Documents\Claude code folder\shots\pc1-day4\ck\`.

| Checkpoint | Quest log | Waypoints |
|---|---|---|
| CkAndariel (Act I) | Den of Evil, Sisters' Burial Grounds, Search for Cain done; the other three not open | all 9 Act I active |
| CkMephisto (Act III) | Act I as the definition (+ Sisters to the Slaughter), Act II all 6 done, Act III 5 done, The Guardian not open | not shot: the docks waypoint is out of `goto`'s sight at the start; its list is a subset of CkHellforge's and CkBaal's, both all active |
| CkHellforge (Act IV) | Acts I–III done, The Fallen Angel done, Hell's Forge in progress ("Take Mephisto's Soulstone to the Hellforge"), Terror's End not open | Act I 9, Act III 9, Act IV 3 all active (second run: the town waypoint took him to City of the Damned, whose menu was shot; the Act II tab click missed) |
| CkBaal (Act V) | Acts I–IV done, Act V five done, Eve of Destruction open ("Find Baal's Throne Room") | all 39 active (5 tabs) |

Every step the definitions set shows done; no finding for the
definitions or `d2s-tool`.

### REC-706 client footprints (item 51)

Recorded with a scratch poll probe (`..\d2rs-probes\probe_footprint.py`,
not in git; no breakpoints): Warriv's 0x100 footprint is the plus pattern
on the sub-tile holding his client path position, moving as soon as that
position crosses a sub-tile. Written into `client/model.md` (the REC-706
PROVISIONAL now has the recorded answer; d2rs's choice matches when its
model position is the client path position).

### REC-1110 (item 52) at normal speed

The same probe, SceSor `-seed 1234` with the panel scene's inputs in
seconds: the client walks Warriv normally and his +0x28 stays 0, so the
scene's NU does not reproduce without the breakpoint recorder. Noted in
`client/model.md` §17 r7; REC-1110 stays open for a frame-anchored rerun.

### Positions (Hephasto, Hellforge, Izual, Anya, Nihlathak)

Already recorded by PC1-C on day 3 (`pc1-day3-c.md` Round 2, the seven
`traces/checks/milestone-*.check`). Nothing left.

### HANDOFF §5 queue

Only the checkpoint quest-log entry was run (above). The rest of §5 is
mostly front-end screenshot work that needs a person at the menus;
combat-melee-fallen / combat-potion-midfight were done on day 3.

## Playtest of `d2rs-windows-018587d2` (staging HEAD)

Computer use is not on in this session, so there was no hands-on 10–15
minute play. Instead the artifact (`gh run download 37959911940`) ran
from the D2 folder with its own scripted input (`play --input`, image
dumps per tick), and the same script ran on 1.14d (`poke.py --input
--shots`, frame-anchored). New Barbarian (hand axe, buckler, 4 hp1),
`-seed 1234`, `warp 2` at frame 4, a Fallen party at player + (3, 3) at
frame 30, attack the nearest Fallen at 40 and again at 80, walk click
(600, 300) at 120, I at 160 / 190, right click at 220.

Screenshots and saves stay local:
`C:\Users\pc\Documents\Claude code folder\shots\pc1-day4\play-d2rs\tick-*\frame.png`,
`...\play-orig\t*.png`, the side-by-side montages `play-d2rs.png`,
`play-orig.png`, `play-*-zoom.png`; the 1.14d save is `PtBar.d2s` in
`%USERPROFILE%\Saved Games\Diablo II`.

| What | 1.14d | d2rs `018587d2` |
|---|---|---|
| First swing (f40) | kills a Fallen (corpse and blood at f60); the rest of the pack scatters | swing animates; no Fallen visibly dies, the pack stays packed round the player to f230 |
| Second attack (f80), walk click (f120) | both act; the player has walked away by f125 (camera moved) | neither acts: the player stands on the same spot to f230 (blocker 1, `q-fix-pc1d4-player-mode-end`) |
| Player life | 66 → 63 by f90 (a Fallen hit) | no visible loss |
| Level entry text | "Entering The Blood Moor" drawn f39–45 | none |
| Life text over the orb | "Life: 66 / 66" | none |
| Hover label | "Fallen / Demon" name box at the top at f45 | none (script clicks don't hover) |
| "Help (H)" button over the right of the control panel | not drawn | drawn every frame |
| Inventory open / close (I) | opens and closes | the same (panel looks alike) |
| Exit | — | `play: the server never answered the leave`; **no `.d2s` written** (only `PtBar.map`): the new character was lost |
| Log | — | dozens of `audio: cue at tick N queued after tick N was presented` warnings from f218 on |

The kill, the scatter and the life loss need the state channel to say
whether d2rs's server missed or only the client doesn't show it; the
cleaner check is `traces/checks/combat-melee-fallen.check`, where day 3
found the kill matching. Here the attack went to the nearest Fallen by
the client's own pick, so a different target is possible.
