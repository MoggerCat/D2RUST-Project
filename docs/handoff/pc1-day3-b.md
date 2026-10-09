# PC 1 day 3, session B — hand-back (branch `claude/local-pc1-day3-b`)

REC block REC-815..829. Items from the PC1-B prompt (2026-10-09).

## REC-680 — Blood Golem life share (answered)

- `0x005C6870` (ECX game, EDX U, stack v; `ret 4`): U's first type-3 pet
  (`0x00574EC0(game, U, 3, 0)`), if a monster of class 290, takes
  x = p·v / 100 of a life amount, where p = `Param6` of monstats 290's
  `Skill1` (BloodGolem, 25). Then v −= Heal(golem, x) (`0x005C5F10`:
  capped at the golem's max, returns what was applied). It applies to
  the whole potion amount before the stat-74 list spreads it; callers
  are use entries 3 / 4 / 5.
- Specs: `items/use.md` §3.1, `combat/events.md` §1 "Heal".
- d2rs differs: new row `q-fix-b680-golem-potion-share`.

## REC-665 — mode request to the unit's own position (answered)

- A mode-2 request at U's own cell is still made. The path compute
  (`0x00649970`) hits start = target for type 13 and computes no path,
  so the path has 0 points. It retries as type 15 (flags |= 1, unit
  queued) with the same result.
- The walk start `0x005A7520` returns 0, so the unit goes to neutral
  (`0x005A73E0`, mode 1) and never enters walk.
- Next think at f + aidel (+45 with state 21). One S→C 0x67 code 7 is
  sent; no 0x68 and no RNG draws.
- Spec: `monsters/ai.md` §7.5 rule 8 ("Zero-length walk"), §7.2 row
  `0x005DE6D0`.
- d2rs differs (`walk_in_radius` makes no request): new row
  `q-fix-b665-zero-walk`, which depends on `q-fix-p3-walk-in-radius`.

## REC-670 — local input path for mode 18 (answered)

- The local client starts mode 18 itself at the click.
  - `0x00481030`: gate `0x00480BA0`; point codes → mode request 0x15,
    unit codes → 0x16.
  - `0x00480C10` → `0x004C6EB0` / `0x004C6F40` → `0x004C6140`: mode 18,
    frame 0.
  - Only then does stat 0x148 go up by 1 and the C→S send happen.
- The server sends its own client no 0x4C / 0x4D (`0x00548090` skips the
  own client unless E flags bit 0x4, dodge / avoid).
- First machine step: the first client update after the click. Other
  units are unchanged (the queued request loads frame 0, then the next
  update steps).
- Spec: `skills/sequences.md` §3 "Local player".
- d2rs differs: it takes the local mode 18 from the server echo, one or
  more updates late. New row `q-fix-b670-local-skill-start`.

## REC-671 — whirl end rule (answered)

- Count client updates from the request update (update 0). The last
  partial path step is at a = 2 + ceil((d << 16) / (walk << 12)). Mode 18
  lasts N = max(8, 4·ceil(a/4)) updates.
- Arrival (`0x004C9320`) only clears the E flags and stops the sound; it
  does not end the mode. The only end is machine step 5 of `0x00463390`
  (`0x006217C0`: +0x48 < 1 after the advance `0x00623E00`), which falls
  on every 4th update.
- Fits both recorded whirls: d = 8 → 24 and d ≈ 14 → 40. Both arrivals
  fall on a multiple of 4, so the current d2rs fit also gives 24 / 40.
- Spec: `skills/sequences.md` §3 Whirlwind r1–r3.
- d2rs: `whirl_updates` must return N − 3 (not a − 3); 5 when walk ≤ 0.
  New expected values: (9, 6) → 25, (1, 6) → 5, (6, 6) → 17. Row
  `q-fix-b-whirl-end-rule`, which replaces q-fix-p4-whirl-end's "check
  first".
- Still PROVISIONAL:
  - REC-815: why the path first steps on update 3 (updates 1–2 do not
    step).
  - REC-816: whether update N's mode change is machine step 5 or a
    queued server mode message. N is the same either way; a whirl whose
    arrival is not on a multiple of 4 would decide it.

## Item 41 — skill button state `0x004A8D30` (answered, round 2)

- `0x004A8D30(P, skill)` runs the use check `0x004D9FC0(P, skill)` → u.
  The state is 0 when u = 0 (usable), 4 when u = 6 (aura), and 1 for any
  other u.
- The caller `0x00496BE0` (`0x00496C24`–`0x00496C42`) forces state 1
  when the skills.txt row lacks `InTown` (byte +5 & `[0x006CE268]`, mask
  1) and P's room is a town.
- Spec: `ui/control-panel.md` §7 r2 (REC-720 settled).
- d2rs differs (it draws the plain cel): new row
  `q-fix-b41-skill-button-state`.

## Left / next

- Pending: any new Step 4 items tagged `[combat-a1]`, `[skills-1]` or
  `[skills-2]`, or about damage, monster AI, missiles or skills (none
  existed at the start of this session).

## Round 2 — combat basics on 1.14d (scenario-diff)

No tagged items existed, so the combat checks ran. Both are under
`traces/checks/`. They run in the Blood Moor (`poke warp 2` at frame 4)
because 1.14d's Rogue Encampment allows no combat: a first town run
showed idle Fallens and no damage. Seeds are fixed at frame 30, then the
spawn.

- **combat-fallen-hits-player** (Fallen party 2 sub-tiles away, 200
  ticks).
  - 1.14d: Fallens attack (mode 4) at 70 / 91 / 130 / 151; player hp
    12800 → 12321 at frame 77, then 11866, 11397, 11039, 10560.
  - d2rs: the same attack modes at 70 and 91, but **the player never
    loses life** → `q-fix-b-monster-melee-no-damage`.
  - The leader's think at frame 41: 1.14d S2 (mode 9), d2rs A2 (mode 5)
    → `q-fix-b-fallen-leader-mode-41`.
- **combat-arrow-quillrat** (Quill Rat 4,4 away, five arrows from frame
  38).
  - Both sides: the rat shoots a quill at 31 / 36. The arrows miss on
    both sides; the aim point is the spawn offset, and the rat is placed
    elsewhere.
  - Frame 59: 1.14d shoots again (A2, quill hits the player at 102, hp
    12800 → 12352); d2rs walks → `q-fix-b-quillrat-shoot`. No missile
    kill was reached.
- **First state difference in both:** the Blood Moor population right
  after the warp (frames 4–5): a monster class, a missing Quill Rat,
  positions, an object seed, the game seed →
  `q-fix-b-bloodmoor-warp-population`. Because of it the full diff
  reports thousands of differences; the comparisons above follow the
  poked units and the player only.
- **Not run:** the player's melee hit on a Fallen and a potion drunk
  mid-fight. The d2rs headless side has no unit click (no hover pick)
  and no key steps (`scenario-diff.md` §3 r8.2), so neither can be
  compared → tool row `q-fix-b-headless-unit-click-keys`.
- The 1.14d and d2rs state files are in `traces/raw/check-*/`
  (gitignored) on PC 1.

## Round 3

### Item 1: monster melee damage to the player (answered)
- **Cause in d2rs:** the monster mode set never runs the mode damage. In
  1.14d, `0x005A7C20` calls `0x005A4F50(unit, mode)` at `0x005A7D39` for
  every mode except GH. It runs after the path set-up and before umod
  mode 0, and writes base tohit (19), mindamage (21) and maxdamage (22)
  from the A1 / A2 / S1 row by mode.
- d2rs `monster_set_mode` skips it, so a Fallen has tohit 0, its hit
  chance clamps to 5 %, and it always misses. In the d2rs trace one seed
  step at f77 is the hit draw; no roll follows.
- **Full path:** `combat/damage.md` §10 ("Monster melee on a player, end
  to end"):
  - mode damage with MonLvl level scaling and the difficulty and
    player-count columns;
  - event 0 `0x005A7670` → `0x005A5490` → melee apply;
  - hit chance, then a physical roll of 256 per point;
  - flat damage reduction, damage resist (capped at 50 %);
  - the life change in 1/256 units;
  - a soft hit: flag 0x8000, S→C 0x0D; life sync 0x95 at a 10 % drop.

  Also `monsters/ai.md` §7.5 r9.
- **Draw order per strike:** hit (mod 100), physical roll(256), burn
  roll(1), crit (mod 100 vs `Crit` 5). A hit is 4 seed steps, a miss is 1.
- **Reproduced with `d2rng.py` on the recorded seeds:**

  | Fallen GUID | Mode, start → hit | Damage | Hit draw r |
  |---|---|---|---|
  | 21 | A1, 70 → 77 | 479 | 25 |
  | 20 | A1, 91 → 98 | 455 | 29 |
  | 20 | A2, 116 → 124 | 469 | 25 |
  | 19 | A2, 129 → 137 | 358 | 45 |
  | 21 | A2, 165 → 173 | 479 | 20 |

  Misses: 21 at f137 (r 92), 20 at f158 (r 65), 19 at f172 (r 96).
  Damage is 1–2 points (256–512); there is no reduction. The player's
  seed and mode never change: no block draw, no get-hit.
- **Row:** `q-fix-c1-monster-melee-rule`.
  - Change: a `monster_mode_damage` hook in `monster_set_mode`, between
    the bookkeeping and the umods, using `helpers3::mode_damage`.
  - Test: rerun the check; hp must read 12321 / 11866 / 11397 / 11039 /
    10560 at f77 / 98 / 124 / 137 / 173.
- **PROVISIONAL REC-817:** the exact MonLvl `L-TH` at level 1. MonLvl.txt
  is not in the extracted data (1.14d ships the compiled table only). Any
  value giving a to-hit of 6–11 reproduces the recording.

### Item 2: Fallen leader A2 vs S2 at f41 (answered)
- The Fallen body matches. The leader is its own minion owner
  (`0x005B28E9` SetBoss → `0x0058F030` owner data), so step 5.2 makes
  the aip1 draw (`0x005F047F`–`0x005F04A8`: D < 15, then
  `0x0058F0D0(unit)` == unit, then roll(100) < aip1).
- 1.14d: 3 draws at f30 (idle 10), then S2 at f41.
- d2rs never sets the owner (`set_owner_data` / `unique_minion_owner_data`
  are empty trait defaults), so it skips that draw and picks A2.
- Both sides' seeds reproduce exactly with `d2rng.py`.
- Spec: `monsters/ai-bodies.md` §9.4 r6–r7, `monsters/ai.md` §8.
- Row: `q-fix-c2-fallen-s2-choice`.

### Item 3: Quill Rat shoot or walk (answered)
- The QuillRat body (`0x005F1140`) matches. The order is:
  1. command → A2;
  2. C → A1;
  3. AI state 3 / 19 → A2 with no draw;
  4. D ≥ 10 → wander;
  5. P(35) → A2;
  6. escape 2;
  7. D > 3 → wander, else A2.

  The next think comes at mode end + aidel 15.
- d2rs never stores the monster AI state (data +0x54):
  `Pending::ai_state` returns 0 and the setter does nothing. So the hit
  that sets 19 (→ 3 on mode change, `0x005A68E0`) is lost, and at f59 d2rs
  draws (89 ≥ 35) and escapes instead of firing again with no draw.
- Spec: `monsters/ai-bodies.md` §9.7 r8–r9. Row: `q-fix-c3-quillrat-choice`.
- PROVISIONAL REC-826: the 1.14d rat-seed step at f46 when its quill
  reaches the player without damage.

### Item 4: a missile kill (recorded, 1.14d side)
- New check `traces/checks/combat-arrow-kill.check`: Fire Arrow
  (missiles row 12, skill 7 level 20) aimed at the Quill Rat's recorded
  cell (player + 4, 4) every 8 frames from f34.
- The earlier plain arrows did 0 because ScnAma has no bow, and arrow
  damage comes from the owner's weapon (`missiles/damage.md` §1).
- 1.14d: the first Fire Arrow (f34) kills the rat. It enters mode 0
  (death) with hp 0 at f38, and mode 12 (dead) at f52. The rat's first
  quill hits the player at f46 (12800 → 12415).
- The d2rs side is not run here; the check runs both sides with
  `scenario_diff.py`.

### REC-815 / REC-816 (whirl details)
- **REC-816 settled:** the mode change at update N is machine step 5
  (`0x006217C0`: +0x30 ≠ 0 and +0x48 < 1 → `0x004611F0`), not a server
  message.
  - The 213 in the facts is the speed +0x4C. It is written by the
    velocity branch of the rate function `0x00623F50` (selected in
    `0x006214A0`: a V-skill mode with E-flags bit 0).
  - +0x3C stays 256. The facts columns `f` / `F` / `s` are +0x44 / +0x48
    / +0x4C.
  - Spec: `skills/sequences.md` §3 Whirlwind r3.
- **REC-815 narrowed, still PROVISIONAL as REC-900:** the 2-update
  delay before the first path step is not in `0x00463390`, `0x004C9120`
  or `0x00650840`. Every gate passes from update 1, and a failing gate
  would end the path. So either the start runs after the click (the
  target not ready yet), or the recorder's `px` is not path +0x00. The
  anim recorder tools are not in this tree, so the second is unchecked.
  N still uses the measured a.
- No new rows. d2rs needs only `q-fix-b-whirl-end-rule`.

### REC-826: the Quill Rat seed step when its quill reaches the player (settled)
- That step is the quill's to-hit roll drawn from the owner's seed (the
  rat): hit handler `0x005ADF10` → hit test `0x0057D9B0` at `0x005AE06F`
  (ECX owner, EDX player).
  - A miss is 1 owner step: the missile is removed, no damage.
  - A hit is 2 owner steps: the to-hit roll, then the monster crit
    `0x005A5560` from `0x005AD730`.
  - Damage is rolled on the missile's own seed.
- Recorded:
  - quillrat run: f46 a miss (roll 89).
  - kill run: f46 a hit (roll 39, crit roll 76), 12800 → 12415.
- Specs: `missiles/missiles.md` §R5 step 5 and §R6.1 (full hit order),
  `combat/damage.md` §10 (monster missiles), `sim/units.md` §1 (the
  player gets unit flags |= 0x0E at `0x005348EC`), `monsters/ai-bodies.md`
  §9.7.
- **d2rs: every monster missile passes through players.** The player
  never gets unit flags 0x0E, so `missiles/flight.rs` `accepts()` rejects
  it. Also, d2rs's missile damage skips block / dodge and the monster
  crit. Row: `q-fix-c6-player-flags`.

### Item 5: combat-melee-fallen and combat-potion-midfight (1.14d side recorded)
- Run from `claude/q-tool-state-diff` at `06306f17` (now on staging) with
  `scenario_diff.py <check> --orig-only` on PC 1. The 1.14d state and
  packet files are in `traces/raw/check-*/` (gitignored, PC 1).
  - Note for any PC 1 runner: the new recorders take
    `%TEMP%\d2-game.lock` themselves. Do not hold it around
    `scenario_diff.py`, or the recorder waits forever (two runs timed
    out this way before the fix).
- **combat-melee-fallen** (CmbBar, level 3, short sword):
  - `clickunit 1 19` clicked (368, 300) at frame 39 and the click went
    in at frame 40.
  - The player enters mode 7 (A1) at f40. The nearest Fallen (GUID 21,
    hp 256) dies on that one swing: mode 0 with hp 0 at f46, then mode 12
    at f66. The player is back to mode 1 at f55.
  - The leader (19) goes S2 at f41, as in combat-fallen-hits-player.
- **combat-potion-midfight** (CmbPot, hp1 in belt slots 0 and 4):
  - The player's life is poked to 2560 (10 points) at f60, and `key 1`
    is posted at frame 70.
  - From f70 life rises 80 per frame (hp1: 30·256 × 2 for the Barbarian / len 192 = 80,
    `items/use.md` §3.1).
  - A Fallen hit lands at f77 (3120 + 80 − 479 = 2721); the regen goes on
    (4801 at f103, …).
- The d2rs side runs with the same command without `--orig-only`; it
  needs the d2-client from that branch (the cloud runs it).

### Items 45 and 46: both sides (pc1-data Step 4)
- Run with `scenario_diff.py <check> --reuse`: the 1.14d side is item 5's
  recording; d2rs is the staging d2-client built on PC 1.
- **combat-melee-fallen: the melee itself matches.**
  - Both sides: player mode 7 at f40; the nearest Fallen (1.14d 21,
    d2rs 19) is killed by the one swing (mode 0, hp 0 at f46, mode 12 at
    f66); the player is back to mode 1 at f55.
  - The leader's f41 choice still differs (A2 vs S2, the known
    `q-fix-c2-fallen-s2-choice`), and the pack's modes follow from it.
- **combat-potion-midfight: d2rs never drinks.**
  - 1.14d sends C→S `26 02000000 …` in frame 69's input phase, and the
    player's hp rises 80 a frame from f70.
  - d2rs sends no C→S message after the join and its hp stays 2560 →
    `q-fix-b45-belt-key-no-send`.
  - d2rs also takes no Fallen damage (known: `q-fix-c1-monster-melee-rule`).
- **First difference in both checks:**
  - state, f2: save-loaded items have seed [1, 666] in d2rs, the real
    seed in 1.14d → `q-fix-b45-save-item-seed`;
  - packets, f1: C→S 0x67 byte 18 is 0 in 1.14d and 4 (the class) in
    d2rs → `q-fix-b45-createxgame-byte18`.
  - Then the Blood Moor warp population (known:
    `q-fix-b-bloodmoor-warp-population`).

### [q-fix-b-monster-combat] Quill Rat at frame 59 (answered)
- **Correction:** my earlier note ("the rat is placed elsewhere") was
  wrong. In 1.14d the rat is at the poke point (5147, 4267), exactly as in
  d2rs. The arrows (missile class 0) reach it at f41–42 and f53–54 and do
  0 damage, because ScnAma has no bow.
- **1.14d rat lines, f30–64** (from
  `traces/raw/check-combat-arrow-quillrat/orig.state.jsonl`, the
  `record_state.py` output; every row x 5147, y 4267, hp 1280):

  | Frames | Mode | Seed s |
  |---|---|---|
  | 30 | 1 | [21370634, 838424606] |
  | 31–35 | 5 | [2409280208, 8913528] (think, 1 draw) |
  | 36–43 | 5 | [4094205064, 1004892389] (quill 1 created at f36) |
  | 44–45 | 1 | same |
  | 46–58 | 1 | [1069704589, 1707661690] (quill 1's to-hit at the player at f46: a miss, 1 owner step, REC-826) |
  | 59–63 | **5** | **[1069704589, 1707661690], unchanged** |
  | 64 | 5 | [3163442939, 446165621] (quill 2 created) |

- **Answer:** the f59 think takes A2 with no draw on the rat's seed (the
  same seed from f46 to f63). That is `monsters/ai-bodies.md` §9.7 r8 step
  3: AI state 3 / 19 → A2, no draw. The arrow that reached the rat at
  f54 set state 19 even with 0 damage (a hit with no get-hit mode), and
  the 0x005A68E0 rule turns 19 into 3 when the next non-neutral mode is
  left.
- So the cloud's draw count is right: no extra draws make P(35) pass,
  because 1.14d takes no draw there. d2rs walks because it never stores
  the AI state (`q-fix-c3-quillrat-choice`). The arrows **do** cause it:
  they set the state. A run without arrows would differ in 1.14d too.

### [q-fix-b-monster-combat] REC-890, REC-891, REC-892 (answered)
- **REC-890:** the neutral start after a zero-length walk sends **0x6D**
  (`0x00597E20` mode-1 case `0x00598067`–`0x005980B0` → `0x0053BB70`:
  GUID, cell, life byte; stat 328 += 1), not 0x67 code 7.
  `monsters/ai.md` §7.5 r8 is corrected (my REC-665 text was wrong there).
  d2rs already matches. Correction row: `q-fix-c7-b665-correction`.
- **REC-891:** the monster base list (`0x00574072`–`0x00574086`) is
  `0x006251F0` with flags 1, expire 0, owner type 1 / owner GUID = the
  unit, then `0x00626E10(unit, list, reset 1)`. It is not DYNAMIC, so
  mindamage / maxdamage / tohit go into the totals. Spec:
  `monsters/init.md` §6 step 12. d2rs matches.
- **REC-892:** `0x0058F030` (ECX game, EDX unit; stack GUID, type, f1,
  f2): f2 ≠ 0 sets control flags bit 0x2, f1 ≠ 0 sets 0x1
  (`0x005DD230` only sets bits; it does not restart the AI). Then
  +0x2C / +0x30 / +0x28.
  - The death handover `0x0058F6C0` uses the bits: 0x1 alone releases
    the pack; 0x1 + 0x2 makes the first minion the leader (`0x0058F530`).
  - Spec: `monsters/umod-callbacks.md` §1 r5.
  - d2rs differs: row `q-fix-c7-owner-flags`.
