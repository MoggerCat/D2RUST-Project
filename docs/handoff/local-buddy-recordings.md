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
| 6 | Might aura in a party | **partly: Might solo done; party blocked** | `pal1-spawn.jsonl`, `pal1-spawn-packets.jsonl` (25,922 events) | `check_rng.py` OK (10 draw sites); `check_packets.py` 3 failures, all F1 (0x3E) | see "Item 6" |
| 8 | Druid summon | **done (Raven)** | `dru1-spawn.jsonl`, `dru1-spawn-packets.jsonl` (30,133 events) | `check_rng.py` OK (4 `chain_unexplained`, 9 sites); `check_packets.py` 3 failures, all F1 | see "Item 8" |
| 2 | Countess kill | **blocked** | — | — | needs a character that reaches the Forgotten Tower (Black Marsh, Act I, area level ~ 8–10 Normal) and survives five tower levels: a level ~10+ character; our characters are level 1–2 |
| 3 | Cain rescue | **blocked** | — | — | needs the Scroll of Inifuss (Dark Wood tree), Akara's decoding, the Cairn Stones order in Stony Field, then Tristram: quest chain plus a level ~8+ character |
| 4 | act change | **blocked** | — | — | needs Andariel killed (Catacombs level 4, Act I end) or a quest flag; level ~15+ |
| 5 | hireling game | **done (pass 2)** | `merc1-spawn.jsonl`, `merc1-spawn-packets.jsonl`; leveling `merc-lvl1-`, `merc-lvl2-`, `merc-gold-spawn.jsonl` | `check_rng.py` OK (all four); `check_packets.py` **OK** (no 0x3E in the run, so no F1) | see "Pass 2, B" |
| 9 | Clay Golem | **done (pass 2)** | `golem1-spawn.jsonl`, `golem1-spawn-packets.jsonl` (13,552 events); leveling `golem-lvl1-spawn.jsonl` | `check_rng.py` OK (both); `check_packets.py` **OK** (no 0x3E in this run, so no F1) | see "Pass 2, A" |
| 10 | Act III entry | **blocked** | — | — | Acts I–II completed (Andariel, Duriel) |
| 11 | levels 74 and 120 | **blocked** | — | — | Arcane Sanctuary (Act II, via the Palace) and Arreat Summit (Act V, Ancients): far story progression |
| 12 | weather-0001 (Rogue Encampment in rain) | **done (recorded; it rained)** | `wx1-frames.jsonl` (frames-raw-2 + `weather_pools`), PNGs `captures\20261007-084753\` (1,851) | `record_frames.py --selftest` ok; recorder verdict `stability: 0 state keys seen twice or more … NOT ENOUGH` (expected: rain, cursor and the player seed change every frame); no `weather-0001` case file exists to verify against | see "Item 12" |
| 13 | draw-order OQ7 (`TownE1` tile (950, 933) of run 2) | **blocked (cells not reachable on screen)** | `oq7-frames.jsonl`, PNGs `captures\20261007-085541\` (856; draws every 20th) | recorder verdict `NOT ENOUGH` (walking) | see "Item 13" |

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

## Item 6: Might (solo)

`bdPal` (west town exit, gate guard in the passage), `spawn.py --level 2
--trigger --status --packets -- -w -ns -name bdPal -pal`. Natural Blood
Moor monsters (2 quill rats, zombies) and one spawned champion fallen pack
(spawn frame ~3,100) took it to level 2 (xp 501). C→S 0x3B `6200`
(point into Might, skill 98) at frame 4418; C→S 0x3C `62000000ffffffff`
(Might as right skill, which turns the aura on) at frame 4843; S→C 0xA8
(state set on the player, type 0 GUID 1) `a8000100000011211950c0ab98af01ff01`
in frame 4844, then 0xAA at frame 4979. A spawned normal zombie was killed
with left attacks under Might (frame ~5110). Save And Exit kept the
level-2 Paladin with Might. **Party part blocked:** single player has no
other player; the only party member could be a hireling (item 5,
blocked).

## Item 8: Druid summon (Raven)

`bdDru` (`TownE1`, east over the bridge), same options. Natural fallen
packs (xp 459) plus one spawned normal fallen pack (frame 4372, 3 units)
→ level 2 (xp 513). C→S 0x3B `dd00` (point into Raven, skill 221) at
frame 5027; 0x3C `dd000000ffffffff` at 5446; first cast C→S 0x0C (right
skill at location (5690, 4654)) at frame 5485 → S→C 0x7A `7a010aa301…32`
and 0xAC (new monster GUID 50, class 419 Raven) in frame 5495; second cast
at frame 5816 → frame 5826: 0x7A removes GUID 50 (twice), 0x7A adds and
0xAC creates GUID 52; the raven attacked a spawned zombie (class 5, spawn
frame 5761), killed by left clicks; 0x7A removes GUID 52 at frame 6271.
Saved (level 2 Druid with Raven).

## Town variants of the saves (spawn.py `--status`, `-nosave`)

Level 1 / level 2 rects (tiles) give the exit side: `bdAma`, `bdSor`,
`bdDru` town (1064, 928) 56 × 40, Blood Moor (1120, 920) 56 × 96 (east
exit; bridge seen: **`TownE1`**, start (5473, 4708)); `bdBar` (1200, 960),
Blood Moor (1160, 1000) (south gate); `bdNec` (768, 1032) / (824, 1024)
(east); `bdPal` (1024, 1208) / (968, 1160) (west); `bdAss`, `bdDead`
(1104, 1080) / (1064, 1120) (south); `bdMerc`, `ScnSor`, `ScnAma`
(864, 912) / (920, 904) (east). (`-name` loads the save's own map.)

## Item 12: weather-0001

`bdAma` (`TownE1`), `record_frames.py --seconds 420 --every 5
--draws-every 40 --weather` (new option, draw-order-2.md §11.1): 1,851
captured frames (every 5th in-game frame, server frames 2–9342), 232 with
the full draw list. **It rained**: rain flag on in every frame; target /
particle count rose to 255 (particles live in frames 6–4279), then the
rain cycle ended (target 0, particles 0 from about frame 4279); rain
phases 0–3 seen; lightning never on. Splashes were live in 159 frames
(frames 2980–3792, walking along the river bank and the bridge); bubbles
never. **W5 answered by observation:** every one of those 159 frames had
int(intensity) = 0 (intensity is at most 0.99609375 = 255/256), so
splashes do appear while int(intensity) is 0. Player seed per frame
(`seed_start`, `seed_end`): every one of the 1,851 transitions is reached
by stepping the D2 RNG (0 unresolved); steps per frame mostly 12 (426
frames; e.g. dry town), 0 (109), 25, 21, 4, and ~100–131 during full rain.
Example pair (frames 6 and 9, `frame-0000002.png`, `frame-0000003.png`):
seeds `[2268541034, 812154327]` → `[2669174633, 946191154]` and
`[658306235, 1655926658]` → `[1583635139, 1518167455]`. Not done: the
pass 4 / 9 pixel compare (no scene source is wired, `render-capture.md`)
and the water-floor count per frame (the draw records carry no floor
material).

## Item 13: draw-order OQ7

`bdAma`'s town is `TownE1` (bridge east of the start, Blood Moor east) at
origin (1064, 928), so the three cells local (48, 36…38) are level tiles
(1112, 964…966) (run 2's (952, 932…934) moved by the origin).
`record_frames.py --seconds 400 --every 10 --draws-every 20`, walked to
both sides: inside the town the south palisade (local y 28) stops the
player at subtile y 4780 (closest point tile (1111, 956)); the cells are
then ~10 tiles down-screen (dx + dy ≈ 10, the 800 × 600 view shows about
± 6.5); from the east bank in Blood Moor (tile (1122.4, 962.6), via the
bridge and south) they are ~10 tiles left (dx − dy ≈ −12, the view shows
± 5). The strip between the palisade and the river that holds the cells
was not reachable. Blocked: needs another seed / variant whose cells are
on a walkable spot (the survey counts such cells in every Act 1 town
variant; the other variants' cell positions are not in the specs), or a
camera-only capture.

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

## Pass 2 (task `rec-2`, branch `claude/local-buddy-recordings-2026-10-07`)

Decisions: new characters `bdGolem` (Necromancer) and **`bdMercTwo`**
(Barbarian) instead of `bdMerc2`: the character-name field takes no
digits (screenshot: the typed `bdMerc2` stays `bdMerc`, an existing name,
so nothing was created). Created by `make_saves.py`'s `Game.create` from
a scratch script, Save And Exit at once (958 / 980 bytes). Driving: a
scratch driver (`out-rec2\drive.py`, not committed) reads the `--status`
snapshot, writes `--trigger` files and holds left clicks on the target's
screen position (the pass-1 formula). All leveling was real kills of
monsters placed by `spawn.py` 0.2.0 in Blood Moor (`--level 2`); no
memory writes other than spawn.py's own call.

### Leveling method (finding)

In Normal a monster's level is the monstats `Level` column wherever it is
spawned (`monsters/init.md` §7), so the experience of a spawned row does
not depend on the area. `window1` (row 392, Act V barricade window, AI
`Idle`, no attack) has `Level` 1 and `Exp` 491: 147 experience as a normal
unit, **441 as a champion** (level 3, 9–18 life), and no level penalty up
to character level 8 (`combat/vitals.md` §4.2). It has 90 % physical
resistance, `DamageRegen` 2 and `deathDmg` 1 (a few life points to the
killer when it breaks), so the Necromancer took Amplify Damage (skill 66,
level-2 point) and cursed each window before hitting it. Measured: a
normal window ~18 s per 147, a champion ~45 s per 441 (8.3–80 s). A
**unique** window pack (boss umods `[30]` aura enchanted plus four
minions, each level 4, 735 experience) killed the level-1 Necromancer in
90 s (corpse in Blood Moor; the save's status now has the died bit, d2s
`status` 0x28): avoided afterwards.

### A. Clay Golem (queue item 9): done

- `skills.txt` (patch_d2): Clay Golem id 75, `reqlevel` 6, **no
  `reqskill`** (Raise Skeleton is not a prerequisite).
- Leveling run (`spawn.py --level 2 --trigger --status --seconds 3000
  -- -w -ns -name bdGolem -nec`, raw `golem-lvl1-spawn.jsonl`): 35 spawns
  (2 normal and 1 unique window at the start, then 32 champion windows),
  experience 0 → 14,406, level 1 → 6 in about 40 min of game time (frame
  43,920). Save And Exit. `check_rng.py`: OK (268 inline draws, 0
  unresolved, 11 `chain_unexplained`, 13 draw sites).
- Recording run (`spawn.py ... --packets <golem1-spawn-packets.jsonl>
  --seconds 1200 -- -w -ns -name bdGolem -nec`): walked to Blood Moor,
  C→S 0x3B `4b00` (point into skill 75) at frame 1271; 0x3C
  `4b000000ffffffff` (right skill) at 1572; cast C→S 0x0C `0ca1138610`
  (right skill at location (5025, 4230)) at 1613. Frame 1621: S→C 0x7A
  `7a01032101010000001b000000` (pet list add, GUID 27), 0x7F
  `7f0064001b0000000200` (pet life 100 %, GUID 27) and 0xAC
  `ac1b0000002101a1138610800e08` (new monster GUID 27, class 289
  `claygolem`, at the cast point). The golem killed a natural fallen pack
  (4) and a spawned normal zombie (class 5, spawn frame 2397; one spawn
  record, 7 draws in the call); the kills credited the Necromancer (xp
  14,406 → 14,511). Save And Exit with the golem alive (life 100): S→C
  0x7A `7a00000000000000001b000000` (remove GUID 27) at frame 3087.
  `check_rng.py` OK (2 `chain_unexplained`, 7 sites); `check_packets.py`
  **OK** (13,552 events; the run had no 0x3E, so F1 did not show).
- Save `bdGolem.d2s` 967 bytes, `d2s_check.py` (from
  `origin/claude/spec-d2s-buddy`): **23 pass, 0 fail**; level 6, skills
  {66: 1, 75: 1}, 3 unspent skill points; hireling block zero
  (present = False); `jf` present (no corpse, count 0); **`kf` g = 0**: a
  Clay Golem alive at Save And Exit writes the same empty `kf` section as
  no golem (`d2s.md`: only an Iron Golem's item is kept; this confirms a
  Clay Golem is not saved). Also seen: the starting wand has durability 0
  of 15 after the window fights.

### B. Hireling game (queue item 5) and the mercenary save: done

Character `bdMercTwo` (Barbarian, see the name decision above), Kashya's
level-8 path (`npc.md` §7.3 step 2: at level ≥ 8 no quest is needed).

- Leveling (raw `merc-lvl1-spawn.jsonl`, 1 spawn; `merc-lvl2-spawn.jsonl`,
  235 spawns): the first spawn, a champion `fallen2` pack (class 20,
  level 7 + champion), killed the level-1 Barbarian (corpse; Save And Exit
  put it next to the player in town on reload, picked up). Then normal
  `window1` units: the Barbarian breaks one in 1–8 s (147 experience;
  118 from level 7, factor 207/256), ~230 kills, level 1 → 8
  (experience 32,951 at frame 52,890), about 55 min of game time. Belt
  potions 1–4 used; each window's death damage costs ~2 life. The hand
  axe ended at durability 0 (attack damage 1–2), like the Necromancer's
  wand.
- Gold (raw `merc-gold-spawn.jsonl`, 14 spawns): 35 stat points spent
  (25 vitality, 10 strength; life 69 → 169), then 11 champion `fallen1`
  packs (class 19) plus natural fallen in Blood Moor; their gold piles
  were clicked from screenshots: 13 → 296 gold.
- Recording (`spawn.py --packets <merc1-spawn-packets.jsonl> --seconds
  1500 -- -w -ns -name bdMercTwo -bar`; 3,312 frames):
  - frame 969: C→S 0x13 `130100000003000000` (interact, NPC GUID 3,
    Kashya) → S→C 0x4F and **ten 0x4E** (7 bytes: name id, slot seed;
    name ids 0x0D56, 0x0D5A, 0x0D5B, 0x0D60, 0x0D66, 0x0D67, 0x0D68,
    0x0D6F, 0x0D70, 0x0D76), then 0x27 (40 bytes, menu); C→S 0x2F at 970.
  - frame 1236: C→S 0x38 `38030000000300000001000000` (action 3, the
    hire list) → 0x4F and the same ten 0x4E. The client showed 7 rows
    (scroll bar), e.g. Diane level 7, life 81, defense 47, cost 160, fire.
  - frame 1730: C→S 0x36 `3603000000680d0000` (hire, NPC 3, name 0x0D68
    Diane) → S→C **0x81** `81070f01010000000d000000f83287d1680d0000`
    (type 7, class 0x10F = 271 `roguehire`, owner GUID 1, mercenary GUID
    13, seed = the 0x4E slot seed of 0x0D68, name 0x0D68), 0x27 (40
    bytes), 0x4F + **nine** 0x4E (0x0D68 gone), 0x2A
    `2a0005049cf61a0d00000088000000` (code 5, GUID 13), and in frame 1731
    S→C 0x1D `1d0e88` (gold 296 → 136: price 160). The client then sent
    0x38 action 3 again (frame 1731, list resent). Frame 1751: S→C 0x7F
    `7f0064000d0000000100` (pet life 100 %, GUID 13). **No 0x7A** for the
    hire (the Raven / golem summons add themselves with 0x7A; the
    mercenary is announced by 0x81 only). No separate mercenary-stats
    message was seen besides the 0x27 of frame 1730.
  - C→S 0x30 (chat close) at 2011; Blood Moor: the mercenary (class 271,
    life 81) killed a spawned champion zombie (frame 2480) and a normal
    fallen pack (3 units) with arrows: S→C 0x4C (unit skill, GUID 13)
    from frame 2479, e.g. `4c010d00000050010101150000000000`.
  - Save And Exit with the mercenary alive (life 81): S→C 0x7A
    `7a00000000000000000d000000` (remove GUID 13) at frame 3311.
  - `check_rng.py` OK (3 `chain_unexplained`, 10 sites; 2 spawns);
    `check_packets.py` **OK** (694 S→C, 43 C→S; no 0x3E, so F1 did not
    show).
- Save `bdMercTwo.d2s` 1,079 bytes, `d2s_check.py`: **24 pass, 0 fail**.
  For OQ4 (merc): header hireling block **present = True** (flags 0,
  seed = the 0x81 / 0x4E seed above, name index 21 (would match 0x0D68 if the row's first name
  id is 0x0D53, not checked), hireling type id 0 (Fire, Normal), experience 39,482), the
  `+0xBF..+0xCE` rest zero (PASS); `jf` present, hireling item list count
  0 (PASS, she wears nothing); `kf` g = 0; corpse count 0.

### C. Skipped

Might in a party (no second player single-player) and Kick (no
`leftskill` / `rightskill` flag) stay blocked as in pass 1; not retried.

### Leveling cost (for later passes)

Level 6 Necromancer ~40 min, level 8 Barbarian ~55 min of game time with
normal / champion `window1` spawns; a Barbarian (no physical-resist
answer needed) is about 3× faster per window than a Necromancer with
Amplify Damage. Unique windows (aura) and level-7 champions kill a
level-1 character.
