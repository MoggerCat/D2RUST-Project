# PC 2 session 4 — lane audio (audio/*, formats/wav, formats/d2s*)

Branch `claude/spec-audio-s4` (base `origin/claude/pc2-spec-gaps`
`dd480ad`). Spec-only; no code, no TSV change.

## Written (spec § → behaviour, 1.14d addresses)

| Spec § | Behaviour | Addresses |
|---|---|---|
| `audio/triggers-2.md` §18 | Sound identity of a unit: type / class / mode through the draw substitution for players only (Druid wolf / bear forms sound as monsters 430 / 431); which rules use identity vs raw values; `monsounds` record choice (superunique `MonSound` > 0, unique / minion `UMonSound` > 0, else `MonSound`; row 0 for out-of-range) | `0x004CA2C0`, `0x004CA320`, `0x004CA380`, `0x00645270`, `0x004CA410`, `0x004AC7E0`, `0x00656FC0`, `0x00656F90` |
| `audio/triggers-2.md` §19 | Unit request list +0x78: order (newest first), lifetime (ended-not-freed requests stay; free and detach remove one node), group walk `0x004CA900`, skill-voice detach `0x004CB190` (raw `Skill1..4` cells vs group base), unit free detaches all without force | `0x004B94E0`, `0x004BA790`, `0x004CA8A0`, `0x004CA8D0`, `0x004CA900`, `0x004CA9C0`, `0x004CAA10`, `0x004CB190`, `0x00465870` |
| `audio/triggers-2.md` §20 | When objects make their mode-sound call: per update after lights; `ClientFn` ≤ 3 direct, ≥ 4 through the client function (0 return skips); `ClientFn` 3–6 call it themselves; `ClientFn` 18 Keeper's random grunt 2,505 (own seed, wall clock); mode change S→C 0x0E code 3; C-set second pass | `0x004BDFF0`, `0x004BDEE0` (table `0x007277F0`), `0x004BD7C0`–`0x004BD9C0`, `0x004BDD50`, `0x004BCF60`, `0x00463CC0` |
| `audio/triggers-2.md` §21 | Driver input contract: every client field the trigger rules read, with its owner spec | — |
| `audio/triggers.md` §1 r13, §3 r2, §10 r7, §11 | Pointer rule; exact player-event guard (U = P and mode 17; null U fatal); greeting mode 2 writes no last / tick; 0x5D first-match only | `0x004CB9C0`, `0x004E0590`, `0x004A2CB0` |
| `audio/sound-table-2.md` §16 | Sample cache exact: LRU list, load arguments per caller, eviction walks (lock / `Cache` protection in walk 0 only, 750-tick protection), unload; correction: the T = 0 preload is synchronous; pending counter leak on evicted async reads | `0x00482970`, `0x004824A0`, `0x004823E0`, `0x00482860`, `0x00481720`, `0x00482B40` (tail `0x00482C0B`) |
| `audio/sound-table-2.md` §17 | Start failures after a slot is taken (attach, play, stream open): slot freed, no failed flag for streams, retries | `0x004E01B0` (`0x004E034D`), `0x004DF9D0` |
| `audio/environment.md` §1 r5, r6 | Inputs owned elsewhere (day phase = `render/lighting.md` §9 period index of `[0x007A0634]`, creation index 2; weather = `render/draw-order-2.md` §11); "request with id X" = first active request by current id | `0x0061C220`, `0x004B9610`, `0x004B9D50` |
| `formats/wav.md` §5 | Decoder hook: what the client's `WavDecoder` / `SoundBank` must return and how failures map to +0x81 | `0x004DF630` |
| `formats/d2s-load.md` §8 | Player-record values at the join: 0x5F = player data +0x2C = bit of level 1 in the portal list (live 1, recorded); hand 0x23s for loaded saves and for a new character (items 0, plus one extra 0x23 from the `StartSkill` select) | `0x00539760`, `0x00621F90`, `0x0061AE30`, `0x00569F80`, `0x005701B0`, `0x00643B00` |
| `formats/d2s-legacy.md` (new) | Legacy loader for versions 0x47–0x5B: dispatch, 130-byte header (fields, checks, result codes), quests / waypoints / NPC / 16-stat / skill sections, items by version, corpses, hireling (incl. pre-0x5A experience conversion table), trailing hotkey / swap block, post-load; errors in items / corpse / hireling ignored | `0x00534330`, `0x00534020`, `0x00532690`–`0x00533F70` |

## Code `TODO(spec: …)` → where it is answered

sound-table: `app/sound.rs:14,41`, `audio/mod.rs:217,240` → `sound-table.md`
§1–§7, §13, `sound-table-2.md` §16–§17; `mixer.rs:108` → §8.3 r1–r4;
`mod.rs:411` → §6.6 r1, §5 r5/r7, `triggers.md` §1 r3–r4;
`system.rs:52` → §8.1 r1 (pixel points); `:146` → §7 r7; `:298` → §7 r4
(loop start = `Block 1` × 2 bytes), §7 r8 (offset × 4 bytes); `:331` →
§8.1 r1 River; `:387` → §5 r2 (a merged call **does** attach its unit);
`:585` → §4 r6; `:601` → Pending (OQ 10, 13: recording); `:631` →
`sound-table-2.md` §16 r1, r3; `:634` → §10 r5 (T = 0, 25, …; T = 0 sync,
§16 r2); `:953` → §7 r6; `:995` → `sound-table-2.md` §17 r2; `:1100` →
§6.6 r3–r4, Pending OQ 12 (recording); `:1314` → §7 r8 and
`environment.md` §1 r6 (current id); `:1326` → §5 r8 (distance²
recomputed, z + 640); `volume.rs:54` → §6.4 r1 (the game-loaded flag:
on in game); `d2-data/src/sounds.rs:62` → §2 (13 EAX columns, 22 read).
environment: `environment/mod.rs:395` → §4 r4; `:486` → §2 r9; `:716`
→ §5 r2 (exception 0); `:754` → §6 r4; `audio/mod.rs:207` →
`triggers*.md`, `triggers-2.md` §21.
triggers: `system.rs:1337` → `triggers-2.md` §19 r1–r2, r6 (include
ended requests); `events.rs:33` → open question 4 (answered); `:151` →
§3 r2; `triggers/mod.rs:333` → `triggers-2.md` §19 r4 (identity type 1
with a record; raw cells); `movement.rs:87` → §5 r9; `:201` → open
question 11 (client creation); `npc.rs:14` → `npc-greetings.tsv`, §10 r5;
`npc.rs:46` → §10 r7 (mode 2 writes nothing); `objects.rs:23` → §7 r7
(all-zero record: fields still updated); `:26` → §7 r8 (413–417);
`skills.rs:78` → open question 5; `:120` → `triggers-2.md` §16;
`ui.rs:39` → §11 (bit 0 set: bit-1 rows not reached).
wav: `app/sound.rs:15,63`, `audio/mod.rs:218`,
`docs/handoff/client-own-gaps.md:81` → `wav.md` §5.
d2s: `d2-server/src/adapters/session.rs:141` → `d2s-load.md` §8;
`tools/scenario-run/src/lib.rs:19,270`, `docs/handoff/scenario-harness.md:233`
→ the loader is specified (`d2s.md` §1–§10, `d2s-load.md` §1–§2, §8,
`d2s-legacy.md`); what remains is code.
Driver `PENDING` (`audio/driver.rs`): events 12/16/17/18 → open question 4,
`triggers-2.md` §18 r3, §19, `npc-greetings.tsv`; stingers / re-arm →
`environment.md` §3; mode sounds / footsteps / idle voices →
`triggers.md` §4–§6 with `triggers-2.md` §18, §21; object sounds →
`triggers-2.md` §20, positions `sound-table.md` §8.1 r1; skills / missiles
/ states / items → `triggers.md` §8–§9, `triggers-2.md` §13–§16; NPC
speech → §10; ambience / rain / music → `environment.md` §1 r3, r5 (day
phase and weather are answered: lighting §9, draw-order-2 §11). Every
remaining item is client-model wiring (requests below).

## Pending

- `formats/d2s-legacy.md` OQ 1: the version-0x47 item record
  (`0x00532F30`, `0x00531040`, `0x00531390`; 1.00–1.06 saves only).
- `formats/d2s-legacy.md` OQ 2, `formats/d2s-load.md` OQ 4: recordings
  (a 1.07/1.08 save loaded in 1.14d; a new-character join).
- Recording-only, unchanged: `sound-table.md` OQ 1, 10, 12, 13;
  `triggers.md` OQ 1; `environment.md` OQ 1; `wav.md` OQ 1.

## CODE-TABLE CHANGE commits

None.

## Cross-file requests

1. `client/model.md` (PC 1): hold the per-unit inputs of
   `audio/triggers-2.md` §21 (identity inputs: sequence mode +0x30/+0x40,
   flag-ex +0xC8 bit 3, transform states; +0x44/+0x48/+0x4C/+0x4E; +0xB0;
   monster data +0x16/+0x26 from 0xAC; the unit sound fields
   +0x70–+0x88; floor material) and report object updates / mode changes
   to the audio driver in the order of §20; on unit free run §19 r5
   before the per-type frees (`0x00465870`).
2. Owner of the client object functions (`client/model.md` or
   `world/objects.md`): `ClientFn` 1–18 (`0x004BDEE0`, table
   `0x007277F0`) have no spec; their client-side mode changes
   (`GetTickCount` timers, own seed +0x20/+0x24, U+0xD4) decide when
   `ClientFn` 3–6 objects' sounds change; audio needs only §20.
3. `sim/intents-events.md` §8.2 rule 3.1: a new-character (stub) load
   sends one extra S→C 0x23 during the loader messages
   (`0x00569F80` → `0x005701B0(P, hand 0, StartSkill, −1)`, item = skill
   entry +0x34 or −1), and the join's two 0x23 carry item 0 (not −1)
   for that path (`formats/d2s-load.md` §8 r3).
4. `client/audio.md` §B ("Original behavior to reproduce (not specified
   here)"): replace with pointers — §B1 → `formats/wav.md` §5; §B3 /
   §B8 → `audio/sound-table.md` (+ part 2 §16–§17); §B2, §B6 →
   `audio/triggers.md`, `triggers-2.md`; §B4, §B5 →
   `audio/environment.md`; §B7 → the Checks tables of those specs.
5. `items/bitstream.md`: item records for save versions 0x48–0x5F
   (`formats/d2s.md` OQ 2; the legacy loader passes its version to
   `0x0062AE20` / `0x00558CB0`, `formats/d2s-legacy.md` §8 r1).
6. Note for the orchestrator: `formats/d2s.md` is 66 KB (65 KB before
   this session); every § is cited by code `Covers:` lines, so it was not
   split. Moving its Provenance or Open questions to a part file would
   bring it under 60 KB.

## Follow-up (legacy items)

Applied the items lane's requests (`docs/handoff/pc2-s4-items.md`
"Follow-up 2"), each checked with `tools/ghidra/disasm.py` on
`0x00533350`:

- `formats/d2s.md` Open question 2: Answered by
  `items/bitstream-legacy.md` §1–§5 (cross-file request 5 above is done).
- `formats/d2s-legacy.md` Open question 1: Answered by
  `items/bitstream-legacy.md` §6–§8; §8 rule 1 now points to §1–§5.
- `formats/d2s-legacy.md` §8 rule 2 corrected in place: the duplicate
  skip of `0x00533350` never fires (count starts at 0 `0x005333B2`,
  compare/store loop entered only for count > 0 `0x005333FE`), so every
  record is placed. Added the failures: no "JM" → 14 (`0x005335C2`);
  failed top-level player item → next entry (`0x0053352F` →
  `0x005334EB`), its children read as top-level entries; failed child
  or any corpse item → 0xC / 0xD at once, byte count not written
  (`0x005335A9`, `0x00533497`); no length check. New edge case 5.

Pending: none new. CODE-TABLE CHANGE commits: none. Cross-file requests:
none.
