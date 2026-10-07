# Spec: World — Act II quest clarifications (implementation questions QB-1–QB-20, Jerhyn's spawns, the orifice insert)

- **Status:** draft: every rule read from the 1.14d `Game.exe` disassembly
  (`tools/ghidra/disasm.py`, addresses inline) on 2026-10-07; no Act II
  recording exists yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests::act2`
- **Related specs:** `world/quests-act2.md` (owner of the Act II quest
  rules; this file settles the readings the Act II implementation left
  open, `docs/handoff/impl-quests-act2.md`, HANDOFF ids QB-1–QB-20, and
  replaces `quests-act2.md` §6.10); `world/quests.md` (shared machinery:
  §4.1 event arguments, §6.3 0x5D, §9.3 GUID lists); `world/quests-act1.md`
  §10 (chain 4's +0xB4 list); `world/quests-act1-rest.md` §5 (character
  progression `0x00538680`), §6 (party iteration); `monsters/init.md`
  (spawn wrappers `0x005B2F20`, `0x005B3090`); `formats/d2s.md` §6
  (the two NPC intro bit fields); `sim/server-messages.tsv` (0x58).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 38–47 |
| Inputs | 48–53 |
| Outputs / state changes | 54–58 |
| Rules | 59–60 |
|   1. Answers (QB-1–QB-20) | 61–239 |
|   2. Jerhyn's objects and spawns (replaces `quests-act2.md` §6.10) | 240–300 |
|   3. The staff in the orifice (C→S 0x44, S→C 0x58) | 301–346 |
|   4. `quests.tsv` addresses not named in part 1 | 347–363 |
|   5. Helpers the Act II–III quest code calls (QuestWorld seams) | 364–448 |
| Constants & data dependencies | 449–458 |
| Randomness | 459–463 |
| Edge cases & original bugs | 464–482 |
| Test vectors | 483–501 |
| Provenance | 502–514 |
| Open questions | 515–522 |
<!-- /index -->

## Summary

The Act II implementation (`claude/impl-quests-act2`) recorded twenty
readings of `quests-act2.md` it could not settle from the text. Each is
answered here from the 1.14d binary, numbered by its HANDOFF id. Section 2
gives the full Jerhyn spawn logic (the start and palace object inits and
the shared palace spawn), section 3 the C→S 0x44 / S→C 0x58 orifice
exchange. Where an answer corrects `quests-act2.md`, the rule there was
edited in place and points here.

## Inputs

As `quests-act2.md`; in addition the object-init parameter block of
inits 18 / 19 (game, object, room, …, x at +0x14, y at +0x18) and C→S
0x44 (`client-messages.tsv`: object GUID @5, item GUID @9, action u16 @13).

## Outputs / state changes

As `quests-act2.md`; in addition the Kaelan (331) spawn of init 19 and
S→C 0x58 (7 bytes, §3.3).

## Rules

### 1. Answers (QB-1–QB-20)

1. **QB-1 Test vector 1 messages.** The vector's message range was a
   typo. Record state 3 maps through index `0x007398FC` (−1, 0, 1, 2, 3,
   4, 0) to table state 2, which holds messages 315–324 (Atma 317,
   `quest-messages.tsv` table `0x00739218`). The implementation's reading
   (follow §1.2) is right; the vector now says 315–324.
2. **QB-2 Event-10 bodies of chains 7 and 8.**
   - Chain 8 `0x00598980` is a jump to the shared record-list removal
     `0x00545530`: if the leaving player's record has both s.1 and s.0
     (s = the record's filter slot, +0xE0) and the record's GUID count
     (+0x9C) is not 0, remove the player's GUID (−1 when the event has no
     player) from the record list (`0x00545240`, swap with the last).
     Otherwise nothing. (Same function as Act I's event 10 removals,
     `quests-act1.md` §10.)
   - Chain 7 `0x005987B0`: remove the player's GUID (−1 when none) from
     the extra GUID list (extra +0x00, the list chain 7's event 11 fills
     with message 125), with no bit test and no count test.
3. **QB-3 NPC class in the event-11 handlers of chains 26 / 27.** Both
   test it.
   - Chain 26 `0x0059E0E0`: only NPC class 377 (act2guard4) (args +0x14).
     Message 59 or 60 → set 30.13 and set callback 2 (record +0xA8) to
     `0x0059E0B0`. Any other message from 377 → set 30.0 (not only 61–63).
     Callback `0x0059E0B0` (chat end): NPC class 377 and extra byte +0x00
     = 1 → extra +0x00 := 0 and callback 2 := 0. Extra +0x00 (1 byte,
     init `0x0059E330` writes 0) has no writer of 1 in 1.14d (the chain-26
     code is `0x0059E0B0`–`0x0059E3B8`, and no code looks chain 26 up by
     id), so the installed callback never fires and stays installed.
   - Chain 27 `0x0059E3C0`: NPC class 378 (act2guard5) and message 303 →
     set 31.0. Nothing else.
4. **QB-4 Altar and the quest-chest gate.** Confirmed: the altar operate
   `0x0059A7E0` never calls `0x00545850`; only the three chests do
   (`0x00599C17`, `0x00599CF7`, `0x00599DF7`). The altar's own guards are
   `quests-act2.md` §5.7 steps 1–2. §1.3 there now says so.
5. **QB-5 Altar init "status 0 → status 2 to all".** The branch exists
   but never fires in 1.14d. Layout of `0x0059A3F0`: +0x05 := 1, +0x0C :=
   GUID (−1 without an object); if not-intro = 1: (if state ≤ 1: darken,
   success → +0x02 := 1, failure → +0x03 := 1; state := 3 by a direct
   byte store `0x0059A434`; flags := 0); then, still inside not-intro, if
   status = 0: status 2 to all (`0x0059A455`). So the status test also
   runs when state ≥ 2. It cannot pass: chain 10's status byte starts at
   0 and is only ever written with 1, 2, 3 or 13 (`0x0059A364` darken,
   `0x0059A455`, the timers, `0x0059EC56`, `0x0059ECD4`, event 13
   `0x0059A632`–`0x0059A658`), and every path that moves the state off 0
   (darken, event 13) writes status ≥ 1 first. Keep the test (no
   observable effect).
6. **QB-6 "status n, state m" in game-start callbacks.** Confirmed for
   every Act II event 13 (`0x00599230`, `0x0059A5D0`, `0x0059B530`,
   `0x0059C220`, `0x0059D4F0`): direct byte stores to +0x0B / +0x0C; the
   flags byte is not reset and nothing is sent by the store. The only
   sends in these callbacks are the ones the rules name (chain 10's
   darken `0x0059A570` with its status 1 to all, and its failure
   `5D 0A 01 00 0000`).
7. **QB-7 Scope in §5.6 and §6.7.**
   - §5.6 messages 362–372 (`0x0059EBE0`): the order is (state ≠ 5,
     not-intro and 11.13 → status 13 silent, state := 5, own seq fn);
     refresh; then if 11.1: set 11.0, clear 11.1, add GUID, and only
     inside that block, if intro: game 11.13 (`0x0059ED63`). No NPC test
     for 362–372. Message 348 needs NPC 177 (a dword compare of args
     +0x14 with 177) and not-intro; state ≠ 1 after the status step →
     return with no flag iterate.
   - §6.7 tome (`0x0059B970`): the three player iterates (`0x0059B3F0`,
     `0x0059B320`, `0x0059B940`) run only inside "not-intro and state ≠
     5" (`0x0059B9E4`–`0x0059BA34`). An intro game grants nothing; a
     second reading after state 5 grants nothing. The 396 text and +0x08
     := tome room happen whenever chain 11 exists.
8. **QB-8 Summoner kill (`0x0059C150`).** Confirmed: with not-intro,
   +0x09 := 0 on every kill (`0x0059C1A9`, after the "no timer yet"
   block, not inside it). In the Summoner AI hook `0x0059C330` the 13.2
   iterate (`0x0059BD10`) runs only inside "status < 2", after the status
   2 to all.
9. **QB-9 Tyrael's chat (`0x0059C3C0`).** Confirmed: for tyrael1 (251)
   the handler adds table state 2 when extra +0x18 = 2 and returns; when
   +0x18 ≠ 2 it returns at once (`0x0059C448` → `0x0059C645`). The later
   rules (14.13 NPCs, 14.3, 14.4, GUID, index) never apply to Tyrael.
10. **QB-10 Messages 442 and 430 (`0x0059CB20`).**
    - 442 (jerhyn 201, needs 14.3): only "state := 5" depends on 14.13
      (`0x0059CCFA`); +0x0A := 1, callback 2 := `0x0059C760`, set 14.4,
      clear 14.3 and the refresh run whenever 14.3 was set.
    - 430 (jerhyn 201): refresh first (`0x0059CC54`), then state := 2,
      +0x08 := 1, callback 2 := `0x0059C760`, then chain 10's sequence
      function on chain 10's record when chain 10 exists (the code checks
      chain 13's own +0xF0 pointer with `0x006CC34C` but calls chain 10's,
      `0x0059CCCF`).
    - Dispatch: tyrael1 falls through to the 444–452 switch after the 302
      block (whether or not 302 matched); jerhyn and meshif1 return for
      any message other than theirs (430 / 442; 450), so 444–452 count
      for every other NPC class. Jump table `0x0059CEB4` (9 entries, msg
      − 444, read raw): 444 → 14.9, 445 → 14.6, 446 → 14.7, 447 → 14.11,
      449 → 14.8, 452 → 14.10; 448, 450, 451 → nothing (as §8.11 of
      `quests-act2.md`). `switches.tsv` lists only 7 rows for this switch
      and pairs 450 with the 14.10 target: use the raw table.
11. **QB-11 C→S 0x44 with a non-orifice object; the 0x58 layout.** See
    §3. The handler is the generic "item into object" path
    (`0x005852E0`): the orifice (class 152) is one case. For any other
    class the cursor item must leave the cursor (`0x0055EEA0`, failure →
    result 4), then result 5 is sent with byte 6 from `0x00585240` (a
    per-item-type table, object spec). The literal "else returns 5" was
    wrong for the orifice: the orifice with `hst ` skips `0x0055EEA0`.
12. **QB-12 Duriel kill party credit (`0x0059D050`).** Party members are
    tested by `0x0059CF20`, not with the killer's test: chain 13 exists;
    the member lacks 14.0, 14.3, 14.4 and 14.5; the member has a room
    whose level is in Act II (`0x006427F0` = 1) → set 14.5. No level-73
    test, no 0x28. The same member function serves the killer's party and
    the party of every player credited in level 73 (`0x0059CFB0`). The
    stub `0x00545990` is called only when the killer qualified, after his
    party.
13. **QB-13 Pick-up of `tr1 ` with 10.3 set (`0x00599A30`).** No: such a
    pick-up only clears the flags byte; no status change and no 0x5D. The
    0x5D (test `0x00599510`, send `0x00544190` chain 9) follows only the
    branches that write the status: `tr1 ` with 10.3 clear, `vip `, `box `,
    `msf `. An earlier call of the test at `0x00599A96` (when the player's
    act equals the record's) discards its result: no effect.
14. **QB-14 Counts at events 13 / 14.** Confirmed: one per code at most.
    `0x00558110` returns the first matching item or null and the callers
    add 1 per non-null result (`0x0059E8B7`–`0x0059E8DC`). A yes/no seam is
    exact.
15. **QB-15 Storage of chain 38's intro record (`quests-act2.md` open
    question 5).** Answered there. The player's NPC record per difficulty
    (player data +0x60 + 4·d) points to two 8-byte bit fields
    (`formats/d2s.md` §6): A at record +0 and B at record +4.
    `0x005723C0` (test) / `0x00572360` (set) act on A; `0x00572470` /
    `0x00572420` (`quests.md` §6.7, 0x91) act on B. Same class → bit table
    `0x00732738` (35 pairs); a class not in the table uses bit 0. 1.14d
    set: the first matching pair's bit only (D2MOO 1.10f also sets bit 0
    every time). So chain 38's intro bits are field A, separate from the
    0x91 bits.
16. **QB-16 Jerhyn's palace spawn.** §2.
17. **QB-17 Tyrael's party tail (`0x0059C9A0` → `0x0059C920`).** For each
    player (from Tyrael) with 14.13: if the player's party id
    (`0x00554630`) ≠ −1, every member (`0x00540510`) gets `0x0059C920`:
    chain 13 exists; the member lacks 14.0, 14.3 and 14.4; the member has
    a room whose level is in Act II → the same grant as the level-73
    players (`0x0059C810`): set 14.13, 14.3 and character progression
    `0x00538680(client of the member, 2, difficulty)` (`quests-act1-rest.md`
    §5). The member's own 14.13 and level are not tested; no 0x28 is
    sent. The level-73 test (`0x0059C860`, asserts chain 13) is: room
    level 73, lacks 14.13, 14.3 and 14.4.
18. **QB-18 Bodies.**
    - `0x005940A0(game, player)` (chain 7's Cain hook, called after table
      state 1 for cain2): remove the player's GUID (−1 when none) from
      chain 4's +0xB4 list (`quests-act1.md` §10, the "credited when Cain
      reached Act II" list) via `0x00545240`. It does not test that chain
      4 exists.
    - Chain 38's active fn `0x005985C0`: returns false (chain 38 never
      raises 0x8A).
    - Status fns: chain 7 `0x00598770`, chain 26 `0x0059E2B0`, chain 27
      `0x0059E4A0`: return false, write nothing. Chain 38 `0x005985B0`:
      writes 0 to the out byte and returns false. With a status fn the
      0x5D builder (`0x00544190`) presets the status to 0 and keeps it
      unless the fn returns 1, so all four report status 0 (no code sends
      0x5D for them); the 0x40 handler skips them.
    - `0x00538680` (`quests-act2.md` open question 7): specified in
      `quests-act1-rest.md` §5; Tyrael's grant uses step 2.
19. **QB-19 Radament hook with status ≥ 2 and state < 3; orifice with a
    busy player.**
    - `0x00599420`: after the guards (chain 8, not-intro, (state < 3 or
      status < 2), unit in level 49): callback 2 := 0; state < 3 → state :=
      3. Status < 2 → status 2 to all (iterate per +0x09, §3.6) then the
      flag iterate `0x005989E0`. Status ≥ 2: the flag iterate runs only
      if the state was just raised (then 9.4, since status ≠ 1); no 0x5D.
    - Orifice operate in mode 0 with a busy player (`0x00535060` = 1):
      return 1 at once: no sound, no mode change, no 0x58 (`0x0059DCFC`).
20. **QB-20 Chest drop code and item level.**
    - The chests store the drop code (object +0xB8) once, right after the
      gate and before the count (`0x00599C28`, `0x00599D08`,
      `0x00599E08`); `0x00559A30` reads it on every call and never clears
      it. When chain 9 is absent the chests skip count and items but still
      drop treasure and gold.
    - Item level: the `&level` argument of `0x00559A30` is an out
      parameter. The helper computes the level itself and writes it before
      any read (`0x00559AF8`): no unit → 1; a player → `0x006253B0`
      (stat 12, level); a monster → `0x00625480` (stat 12); any other
      unit (chests, the altar) → `0x0061DCA0(level id of the unit's room,
      difficulty, game +0x70)` = `levels.txt` MonLvl (MonLvlEx when game
      +0x70 ≠ 0) for the difficulty, 0 / out of range → 1; then at least
      1. So the chest's level is its area level, and the altar's stored
      level id (`quests-act2.md` §5.7, edge case 8) is overwritten unread.

### 2. Jerhyn's objects and spawns (replaces `quests-act2.md` §6.10)

Fields of chain 11's extra: +0x0C start Jerhyn present, +0x0D palace
Jerhyn spawned, +0x11 harem blocker created, +0x15 palace position
stored, +0x28 / +0x2C that position, +0x30 / +0x34 guard target, +0x38
blocker GUID, +0x3C start Jerhyn's GUID.

1. **Init 18, start Jerhyn object 121 (`0x005448B0` → `0x0059F380`).**
   Chain 11 absent → nothing. If +0x0D = 0, game 8.13 clear, game 12.13
   clear, and chain 13 exists, not-intro, state < 2: free spot from the
   object's (x, y) in the object's room (`0x00545340`, size 2, mask
   0x100, sixth argument 10, limit 100); spawn jerhyn (201) there
   (`0x005B2F20`: the free spot's room, x, y, class 201, mode 1, spread
   −1, flags 0); created → +0x0C := 1, +0x3C := its GUID (`0x0059F42F`).
   No free spot: the spawn is still made, with room 0 and the object's
   own (x, y) (the search leaves the point as passed and returns room 0,
   `quests-helpers.md` §1 rule 3); the null-room outcome is
   `monsters/init.md`'s (as Edge case 5).
   This is the only writer of +0x3C.
2. **Init 19, palace Jerhyn object 122 (`0x005448E0` → `0x0059F440`).**
   Chain 11 absent → nothing. Else:
   1. Game 14.13 clear: +0x30 := x + 1, +0x34 := y; spawn act2guard2
      (Kaelan, 331) at (x + 1, y) in the object's room (`0x005B3090`: mode
      1, flags 0, spread −1).
   2. Always: object event 7 on the object at the next frame
      (`0x005417D0(game, object, 7, frame + 1)`); it creates the harem
      blocker at (x − 2, y − 1) (`quests-act2.md` §6.8, `0x005449E0` class
      122 → `0x0059B710`).
   3. If +0x0D = 0 and (game 8.13 or game 9.13 or chain 13 absent, intro
      or state ≥ 2): palace spawn (item 4) with point = the object's (x,
      y) and room = the object's room.
3. **Event 3, old level 40 (`0x0059F0C0`, before the quick remove).**
   1. +0x0C = 1: unit +0x3C (monster) missing → +0x0C := 0. Present: if
      it has an interact unit (`0x00572DC0` on its monster data +0x30) →
      `0x00573180(game, unit, 0, 1)` (AI spec); else remove it
      (`0x0052E050`) and +0x0C := 0.
   2. If +0x0D = 0, +0x11 = 1 and the blocker object (+0x38) exists:
      palace spawn (item 4) with point = the blocker's position
      (`0x00620870`) and room = the blocker's room.
4. **Palace spawn `0x0059EF70(record, &point, room)`** (record in eax,
   point pointer in esi, room on the stack; the point is changed in
   place):
   1. The +0x0C handling of item 3.1 again (so when the start Jerhyn is
      talking, `0x00573180` runs and the function returns without a
      spawn).
   2. +0x0D = 1 → return.
   3. x += 15 when chain 13 is absent, intro or state ≥ 2; x −= 10 when
      chain 13 is not-intro with state < 2. y −= 3.
   4. Free spot from the point, search starting in `room`
      (`0x00545340`, size 3, mask 0x100, sixth argument 9, limit 100). It
      returns nothing: found → the point and the out room are the spot;
      not found → point unchanged, out room null. (A room lookup
      `0x00463740(room, x, y)` before it is dead: its result is
      overwritten.)
   5. Spawn jerhyn (201) at (point, out room) (`0x005B2F20`, mode 1,
      spread −1, flags 0); none → again with spread 2 (mode 1, flags 0).
      Both failed → return.
   6. Unit flags (+0xC4) |= 0x3000000; +0x0D := 1; if +0x15 = 0: +0x15
      := 1, +0x28 / +0x2C := the point. The palace Jerhyn's GUID is not
      stored.

### 3. The staff in the orifice (C→S 0x44, S→C 0x58)

#### 3.1 Orifice operate (operate 25, `0x0059DC70`)

Chain 13 absent → return 0. Object mode (object +0x10; no object → 0):

- Mode 0: busy player (`0x00535060` = 1) → return 1 (nothing else).
  No `hst ` held → sound 19, return 1. Else interact unit := (type 2,
  orifice GUID) (`0x00554120`), mode := 1, send 0x58 with result 0
  (§3.3), return 0.
- Mode 1 and the player's interact unit (`0x00554D00`) is this orifice:
  reset it (`0x00554190`), mode := 2, return 0.
- Otherwise return 0.

#### 3.2 C→S 0x44 (`0x0054C380` → `0x005852E0`)

1. Size ≠ 17 → 3. Player busy (`0x00535060` ≠ 0) and trading
   (`0x005678A0(…, 1)` ≠ 0) → 3. Then the check `0x00549520` (object
   spec); non-zero → that result. Else `0x005852E0(game, player GUID,
   object GUID, item GUID, action)`, result 0.
2. `0x005852E0`: object = unit (type 2, GUID), player = unit (type 0),
   item = unit (type 4). No player: with no object either → fatal assert,
   else return. No object → return.
3. Action 2 (cancel): send 0x58 result 1; object mode := 0; reset the
   interact unit. Action 3 (insert): below. Other actions: nothing.
4. Insert, orifice (class 152): item record (`0x006335F0` of the item's
   class, −1 when no item) code ≠ `hst ` → 0x58 result 4, stop. `hst ` →
   no cursor removal; reset the interact unit; 0x58 result 5, byte 6 = 1;
   mode := 1, then mode := 2; `0x0059DD80` (`quests-act2.md` §8.7, which
   deletes the staff).
5. Insert, any other class: `0x0055EEA0(game, player, item)` (item off
   the cursor) fails → 0x58 result 4. Else reset the interact unit; byte
   6 := `0x00585240(game, item record byte +0x122)` (table `0x00732EB0`,
   draws on the player's seed; object spec); 0x58 result 5; byte 6 = 1 →
   mode 1 and an end-animation event at frame + (`FrameCnt1` >> 8) + 1;
   else mode 0.

#### 3.3 S→C 0x58 (7 bytes, `0x0053D8D0`)

| Byte | Field |
|---|---|
| 0 | 0x58 |
| 1–4 | u32 object GUID (the orifice; for 0x44 the GUID the client sent) |
| 5 | result: 0 open the insert dialog, 1 cancelled, 4 refused, 5 accepted |
| 6 | accepted with effect (1 for the orifice); not written for results 0, 1 and 4 (open question 1) |

### 4. `quests.tsv` addresses not named in part 1

Read 2026-10-07 (quests-fixups) so that every address of the Act II rows
of `quests.tsv` (rows 8–16, 38) is accounted for; with these the rows are
`specified`.

| Address | Row (chain) | What it is |
|---|---|---|
| `0x00598810`, `0x0059E330`, `0x0059E530`, `0x005985D0` | 8 (7), 15 (26), 16 (27), 38 (38) | init functions: zero the 15 callbacks, store the callbacks / table / filter that the row lists, active := 1, state and status := 0 (`quests.md` §2.3), and allocate the extra block (+0x18) that `quests-act2.md` §2 lists; 38 stores +0x18 := 0 and filter 42 |
| `0x00599A10`, `0x0059AD40`, `0x0059BF70`, `0x0059C6B0` | 10 (9), 12 (11), 13 (12), 14 (13) | event 10: a jump to `0x00545530` (remove the leaving player, §1 item 2) |
| `0x00599FB0` | 11 (10) | event 10: `0x00545530`, then remove the player's GUID (−1 when no player) from the extra +0x14 list (`0x00545240`); `quests-act2.md` §5.6 "Event 10" |
| `0x0059E2A0` | 15 (26) | event 8: a bare `ret` |
| `0x00598770`, `0x0059E2B0`, `0x0059E4A0` | 8, 15, 16 | status fn: return false, writes nothing |
| `0x005985B0` | 38 | status fn: writes 0 to its third argument, returns false (§1 item 18) |
| `0x005987D0`, `0x00598780`, `0x0059E2C0`, `0x0059E4B0` | 8, 8, 15, 16 | chain 7's event 13 and the three active fns ("wants to talk") of `quests-act2.md` §9 |
| `0x00738FC8`, `0x0073B8C0`, `0x0073BD58`, `0x00738D60` | 8, 15, 16, 38 | NPC message tables (`quest-messages.tsv`) |

### 5. Helpers the Act II–III quest code calls (QuestWorld seams)

#### 5.1 Quest-chest gate (`0x00545850(op)`)

Callers: the Act II chests (operates 39–41, `0x00599DF0`, `0x00599C10`,
`0x00599CF0`) and the Act III chests (operates 57, 58, `0x005B8860`,
`0x005B8940`); the altar and every other quest object do not call it.
`op` is the operate record (+0x00 game, +0x04 object, +0x10 object
class id). In order:

1. Object present and its mode (+0x10) ≠ 0 → return 0 (already open;
   the chest does nothing more).
2. The object's position is read (`0x00620870`; the result is unused).
3. R := the `objects.txt` record of the class (`0x00640E90(op +0x10)`).
4. `Mode1` (R +0x140) ≠ 0: object mode := 1 (`0x00624690`, a mode set
   of `world/objects.md` §4) and one ENDANIM event (event 1) on the
   object at game frame + (`FrameCnt1` (R +0xDC) >> 8)
   (`0x005417D0(game, object, 1, frame, 0, 0)` at `0x005458AB`;
   `sim/unit-events.tsv` row `0x005458ab`). Note: no "+ 1", unlike the
   chest open of `world/objects.md` §8.1 rule 7. `Mode1` = 0: mode := 2
   (no event).
5. Object flags (+0xC4) &= ~0x2.
6. Return 1. No RNG draw, no message of its own (the mode set's
   update is the objects spec's).

A null object (never passed by the callers) skips step 1 and reaches
the position read with no unit (`0x00620870(null)`); not reproduced.

#### 5.2 Tainted Sun start / end (`0x0061C450(act)`, `0x0061C4D0(act)`)

Both act on the act's environment record E (act +0x04, `0x0061AA60`;
a null act is fatal 0x547; layout `render/lighting.md` §9.1). They
write the server's record only; clients learn of it from the 0x53 /
0x5D the quest code sends (`quests-act2.md` §5.2, §5.7). "Intensity" and
"color" are `render/lighting.md` §9.3 r4 and §9.4, run here with L = 0
and A = 0 (`0x006427F0(0)`).

Start (`0x0061C450`, called by Darken with game +0xC0):
1. E+0x2C := 1; speed E+0x28 := `[0x007443E8]` = 4 (ticks per degree).
2. Period index E+0x00 := 0; ticks E+0x08 := 0; type E+0x04 := the
   eclipse table's entry-0 type (`[0x00744484]` = 3).
3. Intensity (eclipse flag still 0: the sine branch with ticks 0, so
   I := 128).
4. Eclipse flag E+0x30 := 1.
5. Period reset (`0x0061BDF0`): eclipse entry 0 → type 3, ticks := 300 ×
   4 = 1,200.
6. Intensity again (eclipse branch: 128 → 120).
7. Color (eclipse table, index 0 → next index 1; t = 0): R, G, B := 0,
   30, 243.

End (`0x0061C4D0`, called by the altar operate on Act II):
1. E+0x2C := 0; speed := `[0x007443E4]` = 128.
2. Index := 2; ticks := 0; type := the normal table's entry-2 type
   (`[0x0074440C]` = 0).
3. Intensity with the eclipse flag **still set** (I −= 8 when > 32,
   then at least 32).
4. Eclipse flag := 0. No period reset and no color: the next advance
   (`sim/tick.md` §3 step 1, `render/lighting.md` §9.2 r1) recomputes
   them.

From then on the record runs the ordinary advance; with speed 4 and
the eclipse table the eclipse cycle is 360 × 4 = 1,440 ticks long. The
two functions draw nothing.

#### 5.3 Remove a unit for everyone (`0x0052E050(game, unit)`)

1. For each client of the game (list game +0x88, next client +0x4A8)
   in state 4 (client +0x04, in game; `0x0052DED0`, callback
   `0x0052DFB0`): the client's room (client +0x1B4; null → fatal
   0x15BE) gives its adjacent-room list (`0x00619790`, the room itself
   included); when the unit's room (`0x00620BB0`) is in it (first
   match), S→C 0x0A RemoveUnit (type, GUID; `0x0053BDA0`, none for a
   missile, type 3) to that client. A null unit sends nothing.
2. Then the unit is freed (`0x00555600`, `sim/units.md`).

Used by chain 7's event 3 (§2 item 3.1).

#### 5.4 Scroll text (`0x005456A0(player, object, string)`)

S→C 0x27 to the player's client (`0x005531C0`, then `0x0053C8D0`):
unit type 2, the object's GUID, count 1, entry 0 kind 0 with the string
id; bytes 7, 9 and 12–39 are not written (layout and masking:
`sim/intents-events.md` §3.5 "0x27"). Callers: the Act II tome (string
396), Act I (`quests-act1.md` §10, 127) and Act V (`quests-act5.md`).

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| intro class → bit table | 35 pairs at `0x00732738`, count `0x00732734` | image |
| Jerhyn palace offsets | x + 15 / x − 10, y − 3 | `0x0059F010`–`0x0059F018` |
| Kaelan spawn offset | (x + 1, y) | `0x0059F46C` |
| free-spot arguments | init 18: 2, 0x100, 10, 100; palace: 3, 0x100, 9, 100 | §2 |
| 0x58 results | 0, 1, 4, 5 | §3 |

## Randomness

No new quest-seed draws. The non-orifice insert path (§3.2 step 5)
draws once on the player's seed in `0x00585240` (object spec).

## Edge cases & original bugs

1. Chain 26 installs a chat-end callback (`0x0059E0B0`) that never runs
   its body: extra +0x00 is never 1 (§1 item 3).
2. The 0x58 sent by the orifice operate, and the 0x44 results 1 and 4,
   carry a stale stack byte in byte 6 (§3.3).
3. A non-meshif NPC sending 450 sets nothing (case 6 is the exit);
   meshif's 450 is handled before the switch.
4. The free-spot helper `0x00545340` never reads its sixth argument
   (the "radius"): `[ebp + 0x14]` has no reader; the search grows ring
   by ring up to the seventh argument (limit). Every "radius r" of the
   quest specs is unused (cross-file request in
   `docs/handoff/pc2-spec-quests-act2.md`).
5. When the palace free spot fails, the spawn is tried with a null room
   (§2 item 4.5); the monster spec decides the outcome.
6. Start Jerhyn talking when the player leaves Lut Gholein: `0x00573180`
   runs twice (event 3 and again inside the palace spawn) and the palace
   Jerhyn is not spawned that time.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| quest chest (class 355 scroll, `Mode1` 1, `FrameCnt1` 0x1100) at mode 0, frame 1000 | mode 1, event 1 at frame 1017, flag 0x2 cleared, return 1 | §5.1 |
| quest chest at mode 1 | return 0, nothing changed | §5.1 |
| Tainted Sun start on a record at index 3 | index 0, type 3, ticks 1,200, speed 4, I 120, eclipse 1, R G B 0 30 243 | §5.2 |
| Tainted Sun end (I 120) | index 2, type 0, ticks 0, speed 128, I 112, eclipse 0 | §5.2 |
| chain 8 chat, record state 3, 9.0/9.1/9.13 clear, not listed | table state 2 (msgs 315–324) | §1 item 1 |
| pick-up `tr1 `, 10.3 set | flags := 0; status and 0x5D unchanged / none | §1 item 13 |
| chain 26: msg 61 from NPC 377 | 30.0 set; callback 2 unchanged | §1 item 3 |
| chain 26: msg 60 from NPC 377 | 30.13 set; callback 2 = `0x0059E0B0`; chat end leaves it set | §1 item 3 |
| palace init at (100, 200), chain 13 not-intro state 0, game 8.13 set, free spot found at once | Kaelan at (101, 200); Jerhyn at (90, 197); +0x28/+0x2C = 90/197 | §2 |
| same, chain 13 state 2 | Jerhyn at (115, 197) | §2 |
| Duriel killed by P (no 14.x), P's party member M in Lut Gholein lacking 14.0/14.3–14.5 | P and M get 14.5; no 0x28 | §1 item 12 |
| orifice mode 0, player busy | return 1; no 0x58, no sound | §3.1 |
| 0x44 action 3, orifice, cursor item `hst ` | 0x58 `58 <guid> 05 01`; mode 1 → 2; staff handed in | §3.2 |
| 0x44 action 3, orifice, cursor item `msf ` | 0x58 result 4 | §3.2 |

## Provenance

- 1.14d `Game.exe` disassembly (`tools/ghidra/disasm.py fn / at /
  xref`) at the addresses inline; `re/exports/index/switches.tsv` for
  the 0x59CC2F jump table; raw image bytes for `0x007398FC` and
  `0x00732738`. A scratch script (outside the repo) listed the
  `0x00543640` lookups by constant chain id: none for chains 7, 26, 27,
  38.
- D2MOO 1.10f `PlrIntro.cpp` gave the names of the two intro fields
  (`pQuestIntroFlags` A, `pNpcIntroFlags` B); confirmed by the 1.14d
  record offsets (+0 / +4) of `0x00572360` / `0x00572420`. Difference:
  1.10f's quest-intro set also sets bit 0.

## Open questions

1. Byte 6 of S→C 0x58 for results 0, 1, 4 is stale stack data: a
   recording of an orifice operate (result 0), a cancel (1) and a wrong
   item (4) shows what the client receives; until then d2rs sends 0 and
   the conformance comparison masks byte 6 for these results.
   **Needs recording.**
