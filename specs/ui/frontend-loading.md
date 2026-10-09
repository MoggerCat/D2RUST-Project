# Spec: UI — Game load and act-change loading screen (difficulty enable rules, loading screen, act transitions)

- **Status:** draft (2026-10-08, RE on the 1.14d `Game.exe` client; DC6 header read from `d2data.mpq`; no
  capture yet). Points that need a capture are PROVISIONAL with a REC id (METHODS M22).
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui::front_end::loading`
- **Related specs:** `ui/frontend-menus.md` (§F1.3 flow, §F2.6 OK, §F2.8 difficulty box: this spec continues
  from its "game load"), `formats/d2s.md` (§2.1 fields, §2.2 loader checks, §2.3 status word, §2.7
  character-select reads), `client/model.md` (§7 rules 2–6 session messages 0x01–0x05, rule 9 C→S 0x67,
  §11 current act), `sim/intents-events.md` §8 (single-player session order), `sim/tick.md` §6 rules 4–6
  (client states 3 / 5, when S→C 0x04 is sent), `world/waypoints.md` §7 rule 5 and Open question 1 (the
  recorded cross-act message order), `world/npc.md` §8.3 and `world/quests.md` §8.1 (NPC act travel, S→C
  0x61), `render/composition.md` §3 (frame clears, the post-load black frame), §4 (act palette after the
  load), `render/sprite-placement.md` (cel placement), `render/blend-modes.md` §1 (draw mode 5),
  `ui/controls.md` §4 (input dispatch), `ui/ui-states.tsv` (ui 9, 11), `client/audio.md` (sound, deferred).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–56 |
| Inputs | 57–68 |
| Outputs / state changes | 69–78 |
| Rules | 79–82 |
|   L1 Save fields the front end reads (`0x00439840`, `0x00439780`) | 83–98 |
|   L2 Enable rules | 99–124 |
|   L3 Art and palette (`0x00456550`) | 125–141 |
|   L4 One loading draw (`0x004565E0(step)`) | 142–165 |
|   L5 When loading draws happen (single player) | 166–179 |
|   L6 Between draws | 180–189 |
|   L7 End: the first game frame (`0x0044C990` → `0x004547B0`) | 190–208 |
|   L8 Input during loading | 209–235 |
|   L9 Act change (S→C 0x05, 0x03, …, 0x04) | 236–259 |
|   L10 Act-start cinematics (hooks only) | 260–286 |
|   L11 Sounds (deferred) | 287–293 |
| Constants & data dependencies | 294–310 |
| Randomness | 311–314 |
| Edge cases & original bugs | 315–330 |
| Test vectors | 331–350 |
| Provenance | 351–375 |
| Open questions | 376–388 |
<!-- /index -->

## Summary

What the player sees from pressing OK on character select (or create) until the first game frame, and
again whenever the local player changes act. The front end ends (`ui/frontend-menus.md` §F2.6 rule 4); the
game state `0x0044F360` queues C→S 0x67 and then draws the **loading screen**: one 256 × 256 frame of
`data\global\ui\Loading\loadingscreen.dc6`, centred, on black, with the `Loading` palette. It is not
timed: the frame index steps once per loading draw, and in single player there are exactly two draws at a
game start (frames 0 and 1) and one per act change (frame 0). The screen stays until the first in-game
frame, which needs S→C 0x04 (in game) and a placed local player; that frame frees the art, loads the act
palette and is presented black. An act change (waypoint, Warriv, Meshif, the Durance red portal, Tyrael to
Act V) sends S→C 0x05 and 0x03, which show the same screen; there is no act-specific art. The act-start
videos (S→C 0x61) play from the message handler while it shows. The part 1 below adds what
`ui/frontend-menus.md` §F2.8 lacks about the difficulty box: the save fields read and the enable rules.

## Inputs

| Input | Source |
|---|---|
| Character status word (+0x24) of the selected entry | `formats/d2s.md` §2.3, read by character select (§2.7 rule 1) |
| Expansion installed | `0x00408F20` (`client/model.md` Inputs `expansion_installed`) |
| Language id | `0x00525150` (string-table language, 0 = ENG; `client/model.md` §7 rule 9 @0x2D) |
| Loading art and palette | `data\global\ui\Loading\loadingscreen.dc6`, `data\global\palette\Loading\pal.dat` / `pal.pl2` (d2data) |
| Session messages | S→C 0x01, 0x03, 0x04, 0x05, 0x61 through the bridge (`client/model.md` §7) |
| Display size | 800 × 600 or 640 × 480 (`client/ui.md`) |
| Pointer and keys | the client input edge (`ui/controls.md` §4) |

## Outputs / state changes

| Output | Where |
|---|---|
| Chosen difficulty (session +0x210) and the C→S 0x67 flags | `ui/frontend-menus.md` §F2.8 rule 3, `client/model.md` §7 rule 9 |
| Loading frame presented (index frame + palette) | rule L4 |
| Loading-screen state: art held, frame counter | rules L3–L6 (d2rs `LoadingScreen` resource; client only) |
| Hand-off to the in-game draw (act palette, black first frame) | rule L7, `render/composition.md` §3 step 4, §4 |
| Video request (act-start cinematic id) | rule L10 (playback is a stub) |

## Rules

**Part 1 — Difficulty choice and the save**

### L1 Save fields the front end reads (`0x00439840`, `0x00439780`)

Character select reads only the header fields of `formats/d2s.md` §2.7 rule 1. For the OK / difficulty
decision it uses the u16 status word at +0x24 alone (`formats/d2s.md` §2.3):

| Field | Bits of u16 +0x24 (byte) | Use here |
|---|---|---|
| expansion | 0x0020 (+0x24 bit 5) | E = 1 → expansion thresholds; E = 1 on a classic install → OK does nothing (returns 0, `0x00439840`) |
| hardcore, dead | 0x0004, 0x0008 (+0x24 bits 2, 3) | both set → message 5304, no box (`ui/frontend-menus.md` §F2.6 rule 2) |
| progression p | bits 8–12 (+0x25 bits 0–4) | the box and Hell rule (L2) |

Not read for the decision: level +0x2B, the three "town per difficulty" bytes +0xA8..+0xAA (read only by
the loader after the choice, rule L2.4), the quest and waypoint sections. The title prefix drawn in the
slot (`ui/frontend-menus.md` §F2.4 rule 4) uses the same p, hardcore and E; its owner is
`world/quests-act1-rest.md` "Character title".

### L2 Enable rules

With p and E from L1 and connection mode 0 (single player, `[0x007795EC]`):

1. **Box or direct start.** The box (`0x00439780`) opens when (E = 0 and p ≥ 4) or p ≥ 5
   (`0x00439A79`–`0x00439A97`: `p > 3` for a classic character, `p > 4` for any). Otherwise the game
   starts at Normal (difficulty 0) with no box. This confirms `ui/frontend-menus.md` §F2.6 rule 3 statically
   (its REC-203 capture stays the check).
2. **Buttons.** Normal and Nightmare are created enabled. Hell is disabled by `0x004F96F0` unless
   (E = 0 and p ≥ 8) or p ≥ 10 (`0x00439780`, ECX = p, EDX = E). The thresholds equal the server
   loader's refusals (`formats/d2s.md` §2.2 rule 5.4: Nightmare 13, Hell 14), so an enabled button never
   produces a load refusal.
3. **Hardcore.** No difference in the box: a live hardcore character has the same thresholds; only the
   dead case of L1 blocks.
4. **After the choice.** The loader picks the start act from byte `+0xA8 + difficulty`: act = byte & 0x7F,
   0 when ≥ 5 (`formats/d2s.md` §2.2 rule 8). The writer sets only the byte of the difficulty being played
   (act | 0x80) and zeroes the other two (`formats/d2s.md` §2.1), so a difficulty other than the one last
   saved starts in Act I (edge case 3).
5. **What OK sends.** `0x00434A00` (single player): session +0x35E := 1, `[0x007795E8]` := 1, session +0x19
   := 0, flags session +0x209 := 4, | 0x800 when status & 0x04 (hardcore), | 0x100000 when status & 0x20,
   then `0x004F9190` ends the front-end loop. These are the C→S 0x67 u32@0x27 flags (`client/model.md` §7
   rule 9): a classic softcore character sends 0x00000004 (this settles that rule's PROVISIONAL classic
   value statically; its capture still confirms).

**Part 2 — The loading screen**

### L3 Art and palette (`0x00456550`)

1. Path: language id 0 (ENG) → `DATA\GLOBAL\UI\Loading\Loadingscreen` (format `0x006D6528`); any other
   language → `DATA\LOCAL\UI\LoadingScreen` (`0x006D6544`). Both resolve to DC6 files in `d2data.mpq`
   (`data\global\ui\Loading\loadingscreen.dc6`, `data\local\UI\loadingscreen.dc6`, 666,014 bytes each,
   same headers, different pixels). **Settled** (2026-10-09, q-prov-data, REC-220): the d2data copy is the one used;
   `Patch_D2.mpq` probed by name has neither path, so no override exists, and the two d2data files read
   with identical headers (`mpq-tool extract` of both names).
2. DC6 header (d2data): version 6, 1 direction, **10 frames**, every frame 256 × 256, offset (0, 0),
   bottom-up rows (flip 0).
3. Before the load, `0x00454740` loads and sets the palette `DATA\GLOBAL\Palette\Loading\pal.dat` /
   `pal.pl2` (`0x004FB500`); it stays the presented palette until rule L7 replaces it.
4. State set by the load: art handle `[0x007A2888]`, frame counter `[0x007A2A6C]` := 0, loading flag
   `[0x007A2868]` := 1 (read by `0x00454840`), progress `[0x007BB3B4]` := 0 (`0x00478130(0, 0)`),
   `[0x007A27B8]` := `unloaded` (`client/model.md` §7 rule 6). The load first runs the release of L7
   (`0x004547B0`), which with no art held only clears the three globals.

### L4 One loading draw (`0x004565E0(step)`)

1. Whole-frame clear to index 0 (`0x004F63B0`), then `StartDraw(1, 0, 0, 0)` (`render/composition.md` §3
   table). If no art is held, rule L3 runs first; a failed load is fatal (string 0x19C2).
2. Clamp: n = the art's frames per direction (`0x006019F0`, 10); if counter ≥ n, counter := n − 1.
3. Draw frame `counter`, direction 0, at **x = W/2 − 128, y = H/2 + 128** (W, H = display size; integer
   halves), draw mode 5 (`render/blend-modes.md` §1), no light / palette table. With offset (0, 0) and
   bottom-up rows the cel covers columns x … x + 255 and rows y − 255 … y (`render/sprite-placement.md`):

   | Display | x, y | Columns | Rows |
   |---|---|---|---|
   | 800 × 600 | 272, 428 | 272–527 | 173–428 |
   | 640 × 480 | 192, 368 | 192–447 | 113–368 |

   Everything else is index 0 (black in every palette, `render/composition.md` §4).
4. No text and no progress bar in single player: the bar (two rectangles at (W/2 ± 130, H/2 + 158 …
   H/2 + 178), fill `[0x007BB3B4]`, and a text through `0x0046EFD0`) is drawn only for client game types
   6–9 with `[0x007A27B8]` = 0 (multiplayer; Phase 7+, named only).
5. `EndScene` (present), then `0x0044DA40` and `0x0044DA70`: both mouse buttons reset to "up" (states
   `[0x0070F234]`, `[0x0070F2BC]` := 0x10; held / repeat flags `[0x007A0650]`, `[0x007A0654]`,
   `[0x007A066C]`, `[0x007A0670]` := 0; `0x00466FE0`).
6. After the draw (also when the draw was skipped by `0x004F6070`, minimised window): step ≠ 0 →
   counter += 1.

### L5 When loading draws happen (single player)

| Event | Code | Step | Frame shown | Counter after |
|---|---|---|---|---|
| game start, right after C→S 0x67 is queued (`0x0044F45E`) | `0x0044F360` → `0x0044E200` (`0x0044F4E0`): L3 then L4 | 1 | 0 | 1 |
| S→C 0x03 at the join (art still held) | `0x0045C8E0` → `0x0044E100` → L4 (`0x0044E185`) | 1 | 1 | 2 |
| S→C 0x03 of an act change (art released by the last game frame) | same; L4 runs L3 first | 1 | 0 | 1 |

There is no timer: the picture changes only at these calls, so a single-player game start shows frame 0
then frame 1, and an act change shows frame 0; frames 2–9 appear only through the multiplayer callers
(`0x00478160`: 8 draws with step 0 and `Sleep(250)`, from `0x004781D0` for game types 6–9, named only).
The third caller, the room-graphics preload `0x00470070`, draws only when `[0x007A8920]` ≠ 0; nothing in
1.14d writes it (image value 0), so it never draws.

### L6 Between draws

1. The game draw (`[0x007A0484]` = `0x0044C990`) runs from the client loop `0x0044EFA0` only when (a)
   `in_game` (`[0x007A061C]`, set by S→C 0x04) and a local player with a room (`0x004646A0`), or (b) in
   single player, ui 9 (Esc menu) or ui 11 (config) open and a local player with a room (the paused path,
   `0x0044EFE3`–`0x0044F017`; the server is not run that loop). Neither holds during a normal load, so
   nothing is drawn and the loading frame stays presented.
2. d2rs: while the loading state is active the client presents the last loading frame every render frame
   (same pixels; presentation rate is not a fidelity value).

### L7 End: the first game frame (`0x0044C990` → `0x004547B0`)

1. S→C 0x04 sets `in_game` (`client/model.md` §7 rule 5); the next client loop with a placed player draws
   the game frame. Its first call `0x004547B0`, art held: free the art (`0x00478A00`), load the act
   palette `act<[0x007A288C]+1>` (`0x004FB480`, `render/composition.md` §4), then the act set-ups
   (`0x00600C00(act)`, `0x0045A620`, `0x00483960`, `0x00499E10`, `0x00499E30`, `0x004AB2F0`,
   `0x00497250`–`0x004972B0`, `0x00472890`, `0x0044DB40`; Phase 6 owners); finally the art handle,
   `[0x007A2884]` and the loading flag `[0x007A2868]` := 0.
2. That frame is drawn and then cleared to black, because `0x0044E100` set the post-draw clear counter
   `[0x0070F2C0]` := 1 (`render/composition.md` §3 step 4). So the sequence presented is: loading
   frame(s), one black frame, then the world.
3. `0x0044E100` also raises `[0x007A04BC]` to at least now + 10,000 ms: for 10 s after any act load the
   client frame clock does not run catch-up frames (`0x0044EFA0`; wall-clock pacing, not modelled in d2rs,
   which steps by tick).
4. **Ticks, single player game start.** C→S 0x67 and 0x6B are drained by the server, the join sends 0x03
   (frame 1), and S→C 0x04 goes in the next tick's step 5 once the town rooms are populated: recorded at
   server frame 2 (`sim/tick.md` §6 rule 6, `sim/intents-events.md` §8.3). The loading screen therefore
   covers server ticks 1–2 (one tick = 40 ms) plus the wall-clock build time.

### L8 Input during loading

1. Mouse: every button, wheel and move handler tests `in_game` first (`0x0044BF40`, `0x0044C060`,
   `0x0044C180`, `0x0044C370`, `0x0044C400`, `0x0044C470`–`0x0044C520`, `0x00499BB0`;
   `ui/controls.md` §4.2, §4.3, §6 rule 1): ignored, no C→S message. L4 rule 5 also releases any held
   button at each loading draw.
2. Keys: the key-down handler `0x0046A840` has no `in_game` test; it runs whenever the key mode is ≠ 0
   (`ui/controls.md` §4.1). The mode becomes 1 at the in-game UI set-up `0x00456970` (call
   `0x00456D42`), run by S→C 0x01 (`client/model.md` §7 rule 2), so during the join (after 0x01) and
   during an act change (mode stays 1) bound commands are dispatched. Before 0x01 (the first loading
   draw) the mode is 0 and keys do nothing.
3. Esc (command 56, `0x004690B0`) is the one key with a visible effect. During an act change it opens
   ui 9 (`[0x007A27E4]` = 1), and the server stops ticking while the menu is open (L6 rule 1 (b)), so
   0x04 does not come and the load waits for the menu to close. Measured (REC-222, settled 2026-10-09,
   PC 1 Windows, ScnAma `-seed 1234`, `poke 50 warp 40`, Esc posted at the first loading draw after frame
   10): the key-down handler `0x0046A840` and `0x004690B0` ran at server frame 49 with ui 9 = 0. Ui 9 read
   1 from the next loading draw on. The server ran one more tick (frame 50) and then none in the
   following 50 s, and the loading draws stopped with it. A skill hotkey (F1) posted at the same point
   reached `0x0046A840`, and the load went on (ticks continued). At game start, Esc posted at the first
   loading draw (before 0x01, key mode 0) reached neither handler, and the game started normally. So
   d2rs must dispatch bound keys during an act-change load (key mode 1), with Esc opening the menu and
   pausing the load, and ignore keys only before 0x01.
4. No cancel: nothing returns to the front end from the loading screen; a refused load ends through S→C
   0xB4 (`client/model.md` §7 rule 8).

**Part 3 — Act transitions**

### L9 Act change (S→C 0x05, 0x03, …, 0x04)

1. Trigger: the server warp `0x0053AEC0` to a level of another act calls `0x0053ACC0`, which sets the
   client state 5, queues **0x05** then **0x03** (new act, same map seed, the act's town level) and 0x53,
   then the room adds and 0x0D (`client/model.md` §7 open question 13; recorded order:
   `world/waypoints.md` Open question 1). Callers: waypoint travel (`world/waypoints.md` §7 rule 5), NPC
   travel Warriv / Meshif / Tyrael and the town NPCs of `world/npc.md` §8.3 (`0x0054B830`), the Durance
   red portal to Act IV (`0x00546AC0` path, `world/quests.md` §8.1). The Act V entry is Tyrael's travel
   (level 109) and uses the same path. Town portals stay in one act and never reach it.
2. Client: 0x05 clears `in_game` (game draws stop, the last world frame stays presented); 0x03 rebuilds the
   client act and draws the loading screen (L5 row 3: frame 0, Loading palette). **Same art for every
   act**: the path in L3 takes no act argument; the act byte only chooses the palette at L7.
   **Every 0x03 does this, same act or not**: the handler (`0x0045C8E0` → `0x0044E100`) has no act
   comparison; it frees the current client act when one exists (`[0x007A0634]` → `0x0061AFD0`), builds
   the new one (`0x006194A0`) and runs the loading draw (`0x004565E0(1)`, L4), so a repeated 0x03 of
   the same act shows the next loading frame by the L5 table (art held: frame = counter; released:
   frame 0). Messages are handled one at a time in arrival order (`client/model.md` §7), so 0x05,
   0x03, 0x61 and 0x04 act in the order sent.
3. End: the server sends 0x04 and client state 4 when the new room is ready (`sim/tick.md` §6 rule 4,
   state 5); the first game frame follows (L7: art freed, new act palette, black frame). PROVISIONAL:
   0x04 comes in the tick after the warp, as at the join (because the warp activates the arrival rooms in
   the drain and the next tick populates them, and the recorded new-act units arrive in that tick); settled
   by REC-221.

### L10 Act-start cinematics (hooks only)

1. S→C **0x61** (2 bytes, `61 <id>`) → `0x0045E660` → `0x004B9320` → `0x00482EF0(id)`: plays video `id`
   synchronously inside the message handler (then `0x0047F1D0`). Ids sent by act completion
   (`world/quests.md` §8.1), only when player data +0x4C ≠ 1:

   | Transition | 0x61 id | Video table entry (`0x00721D9C`, 8 entries of 12 bytes) |
   |---|---|---|
   | Warriv, Act I → II | 2 | `ACT02START%s.BIK` |
   | Meshif, Act II → III | 3 | `ACT03START%s.BIK` |
   | Durance red portal, Act III → IV | 4 | `ACT04START%s.BIK` |
   | Tyrael, Act IV → V | 5 | `ACT04END%s.BIK` |

   (`%s` = a 640 × 292 or 640 × 146 variant; entries 0 `d2_promo.bik`, 1 `d2intro%s.bik`, 6
   `D2X_INTRO_%s.BIK`, 7 `D2X_OUT_%s.BIK` are not sent here.) Playback, skip and the file choice belong to
   the cinematics spec (sibling worker); d2rs: a stub that records the id and returns.
2. Order: the act change runs before act completion at every travel site, so the client gets 0x05, 0x03
   (loading screen) and only then 0x61: the video plays over the loading screen, before 0x04.
   `0x00482EF0` ends by loading Act V's palette (`render/composition.md` §4); L7 then loads the arrival
   act's palette anyway. PROVISIONAL: after the video the loading frame is not redrawn and the screen
   shows black until L7 (because no caller of `0x004565E0` follows the video and the player cannot see a
   redraw); settled by REC-223. After the video the handler calls `0x0047F1D0`, which re-reads the
   registry `Resolution` (missing → 1) and re-applies the resolution mode (`0x0044BA20(2 or 0)`,
   `ui/frontend-options.md` §O6 r7); d2rs: no effect (one logical size).
3. End-of-game videos (`[0x007A0604]` → entry 5, `[0x007A0628]` → entry 7, after the game loop in
   `0x0044F360`) are not act transitions: named only.

### L11 Sounds (deferred)

The loading code (`0x00456550`, `0x004565E0`, `0x004547B0`) starts no sound. The OK click is the
front-end button sound (`ui/frontend-menus.md` §F1.6 rule 2); the act's music and ambience start from the
act-load and first-frame set-ups (`client/model.md` §7 rule 4, L7 rule 1); video audio belongs to the
cinematics spec. Owner: `client/audio.md`.

## Constants & data dependencies

| Constant | Value | Source |
|---|---|---|
| Loading art | `loadingscreen.dc6`, 1 × 10 frames, 256 × 256, offset (0, 0) | d2data DC6 header |
| Path formats | `%s\UI\Loading\Loadingscreen` + `DATA\GLOBAL` (ENG), `%s\UI\LoadingScreen` + `DATA\LOCAL` | `0x006D6528`, `0x006D6544` |
| Loading palette | `DATA\GLOBAL\Palette\Loading\pal.dat` / `pal.pl2` | `0x006D64CC`, `0x006D64B0`, `0x00454740` |
| Draw origin | (W/2 − 128, H/2 + 128), mode 5 | `0x004565E0` |
| Box threshold | p ≥ 4 classic, p ≥ 5 expansion | `0x00439A79`–`0x00439A97` |
| Hell threshold | p ≥ 8 classic, p ≥ 10 expansion | `0x00439780` |
| 0x67 flags | 4, \| 0x800 hardcore, \| 0x100000 expansion | `0x00434A00` |
| Post-load black frames | 1 | `[0x0070F2C0]`, `0x0044E100` |
| Catch-up hold after act load | 10,000 ms (wall clock) | `[0x007A04BC]`, `0x0044E100` |
| Multiplayer progress steps | 8 × `Sleep(250)` (named only) | `0x00478160` |
| Video table | 8 entries, count `[0x00721D94]` | `0x00721D9C` |
| Strings | 5304 (dead hardcore); box strings in `ui/frontend-menus.md` | `string.tbl` |

## Randomness

None. The loading screen and the difficulty rules draw no game-seed values.

## Edge cases & original bugs

1. The loading "animation" is not one: single player shows at most frames 0 and 1; frames 2–9 exist in
   the file but only multiplayer callers reach them (L5).
2. An expansion character on a classic install: OK silently does nothing (L1, `0x00439840` returns 0).
3. Difficulty and start act: the save keeps only the act of the difficulty last played (`+0xA8..+0xAA`,
   one byte non-zero). Choosing another difficulty starts in that difficulty's Act I town, even when the
   character reached a later act there before (d2s writer zeroes the others; reproduce).
4. A byte `+0xA8 + d` with act ≥ 5 (hand-edited save) starts in Act I (`formats/d2s.md` §2.2 rule 8).
5. The first world frame after every act load is black (L7 rule 2); reproduce.
6. A mouse button held through the load is released by the loading draw (L4 rule 5): no click carries
   into the game.
7. With the Esc menu open during a load (single player), the paused path draws the world as soon as the
   player has a room, before 0x04, and the server does not tick while it is open (L6 rule 1 (b); REC-222).
8. `[0x007A8920]` is never written, so the preload's loading draws (`0x00470070`) are dead code in 1.14d.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| status 0x0420 (E = 1, p = 4), OK | no box; game at Normal; 0x67 flags 0x00100004 | L2.1, L2.5 |
| status 0x0520 (p = 5), OK | box; Normal, Nightmare on; Hell off | L2.1, L2.2 |
| status 0x0920 (p = 9) / 0x0A20 (p = 10) | Hell off / on | L2.2 |
| status 0x0400 (classic, p = 4), expansion install | box; Hell off; 0x67 flags 0x00000004 | L2 |
| status 0x0804 (classic hardcore, p = 8) | box; Hell on; flags 0x00000804 | L2.2, L2.5 |
| status 0x0020 on a classic install | OK does nothing | L1 |
| +0xA8..+0xAA = `00 83 00`, Nightmare / Normal chosen | start act 3 (Act IV) / act 0 | L2.4 |
| 800 × 600, counter 0 | frame 0 at columns 272–527, rows 173–428; rest index 0; Loading palette | L4 |
| 640 × 480, counter 1 | frame 1 at columns 192–447, rows 113–368 | L4 |
| single-player start: 0x0044E200, then S→C 0x01, 0x00, 0x02, 0x03, …, 0x04 | presented: frame 0, frame 1, one black frame, world | L5, L7 |
| act change: 0x05, 0x03 (act 1), 0x53, 0x07 × n, 0x0D, next tick 0x04 | presented: last world frame, frame 0, black, Act II world with `act2` palette | L9, L7 |
| counter 12, 10 frames, step 1 | frame 9 drawn; counter 10 | L4.2, L4.6 |
| language id 3 | art path `DATA\LOCAL\UI\LoadingScreen` | L3.1 |
| left click at (400, 300) between 0x03 and 0x04 | no C→S message | L8.1 |
| Warriv travel, first time | 0x05, 0x03 (frame 0), …, 0x61 `02` → video hook id 2 (`ACT02START`) before 0x04 | L10 |

## Provenance

1.14d `Game.exe` (Ghidra exports `re/exports`, `re/scripts/da.py`, image reads, 2026-10-08): character
select OK `0x00439840` (compares `0x00439A79`–`0x00439A97`), box `0x00439780`, game start `0x00434A00`,
front-end loop end `0x004F9190`; game state `0x0044F360` (0x67 builder call `0x0044F45E`, `0x0044E200`
call `0x0044F4E0`); game init `0x0044E200` (`0x0044E27D`, `0x0044E287`, multiplayer `0x004781D0` at
`0x0044E2AA`, draw callback `0x0044E300`); act load `0x0044E100` (`0x0044E185`), handler `0x0045C8E0`;
loading art `0x00456550`, palette `0x00454740`, draw `0x004565E0` (clamp `0x0045676F`–`0x00456788`,
progress-bar branch `0x004566A9`–`0x0045676A`), release `0x004547B0`, flag reader `0x00454840`, progress
`0x00478130`, `0x00478150`, `0x00478160`, `0x004781D0`, preload `0x00470070` / `0x00470350`
(`[0x007A8920]` read only; image value 0), frame-count accessor `0x006019F0`, mouse resets `0x0044DA40`,
`0x0044DA70`; client loop `0x0044EFA0` (paused path `0x0044EFE3`–`0x0044F017`, in-game draw
`0x0044F28B`); first frame `0x0044C990`; mouse handlers `0x0044BF40`, `0x0044C000`, `0x0044C060`,
`0x0044C180`, `0x0044C2C0`, `0x0044C370`, `0x0044C400`, `0x0044C470`, `0x0044C4A0`, `0x0044C4D0`,
`0x0044C520`, `0x00499BB0`; key down `0x0046A840`; key mode `0x0046AC70` (from `0x00456D42` in
`0x00456970`), `0x0046ACB0`; act change `0x0053ACC0` (state 5 `0x0053AD31`, 0x05 `0x0053AE39`, 0x03
`0x0053AE5B`), warp `0x0053AEC0`; 0x61 `0x0045E660` → `0x004B9320` → `0x00482EF0`, video table
`0x00721D9C` (count `0x00721D94` = 8) read from the image; strings `0x006D6528`, `0x006D6544`,
`0x006D64B0`, `0x006D64CC`. DC6 headers: `loadingscreen.dc6` from d2data (both copies) extracted with
`tools/mpq-tool` into the scratchpad; d2exp has neither; `Patch_D2` not probed (no listfile). D2MOO not
used.
2026-10-08 (REC-251): 0x03 handler `0x0045C8E0` → `0x0044E100` (no act compare; `0x0061AFD0`, `0x006194A0`,
`0x004565E0`), post-video `0x0047F1D0` → `0x0044BA20`.
- 2026-10-09 (pc1-day3-c, REC-222): keys during loading measured on Windows with a scratch debugger probe (breakpoints `0x004565E0` loading draw, `0x0046A840` key-down, `0x004690B0` Esc command, server tick entry; key posted from the first loading-draw stop), L8 rule 3.

## Open questions

- **REC-220** Loading art source. Capture: hook the cel load `0x004788B0` at `0x004565A0` (log the path and
  the archive the file opens from) or extract `data\global\ui\Loading\loadingscreen.dc6` from
  `Patch_D2.mpq` by name. Settles L3.1: whether a Patch_D2 copy overrides d2data.
- **REC-221** Act-change end. Capture: single player, waypoint Rogue Encampment → Lut Gholein; record
  S→C packets with tick numbers and screenshots per client frame. Settles L9.3: the tick of 0x04 after
  the 0x49 drain, and the presented sequence (world, frame 0, black, Act II world).
- **REC-222** Keys during loading: *settled 2026-10-09* (L8 rule 3).
- **REC-223** After an act-start video. Capture: a first-time Warriv travel (Act I → II) with video
  enabled; screenshot the frame after the video ends and before the world appears. Settles L10.2
  (black vs the loading frame).
