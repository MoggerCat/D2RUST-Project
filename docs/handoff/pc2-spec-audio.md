# Handoff: PC 2 spec worker — audio (`claude/spec-audio`)

Spec session 2026-10-07. Task: the open points of
`docs/handoff/impl-audio.md` §3 (ST1–ST11, TR1–TR7, EN-A–EN-E; HANDOFF §7
ninth set ST-*, TR-*, EN-*) and the open questions of `specs/audio/*` and
`specs/formats/wav.md` that the 1.14d binary settles. Evidence: Ghidra
exports + `tools/ghidra/disasm.py` on `game/Game.exe`; image constants
with `pefile`; local recordings of `docs/handoff/local-buddy-q-rec.md`
(branch `origin/claude/local-buddy-q-rec-2026-10-07`) entries 69 and 74,
read from the raw `snd74*-sound.jsonl` (not committed) where cited;
`docs/handoff/local-buddy-q-data.md` entries 62, 72, 73. Scratch scripts
in `C:\Users\zffit\Desktop\D2test\scratch-audio\` (outside the repo).

## Answered

| Id | Spec § | Answer |
|---|---|---|
| ST1 | `sound-table.md` §2 | 13 EAX columns (`EAX Environ` 0x24 … `EAX Air Absorb` 0x54), 9 + 13 = 22 read columns; field list `0x00481CA6`–`0x00481FDA`. The draft's "12 at 0x24–0x50" was wrong. |
| ST2 | `sound-table.md` §7 r7 | Mode 0: slots 0–3 stereo (kind 1), slots 4–15 mono (kind 0); kinds must match. 4 stereo + 12 mono voices (`0x004DFAA0`, `0x004E025D`). Not "any". |
| ST3 | `sound-table.md` §7 r6 | The pick overwrites the request id; history goes on the pre-pick id's record; everything after (Stream, load, Async, failed, Stereo, Loop, blocks, sample, Reverb, path, volume chain) reads the variant's record. As implemented. Plus: restarts pick from the variant. |
| ST4 | `sound-table.md` §6.6 | Stop/steal save the stream position (4-byte units; 0 for non-stream). Natural end: upkeep `0x004DF890` sees the voice's playing flag cleared by the 50 ms wall-clock service thread `0x00516250`: not tick-exact in 1.14d. Model kept; OQ 12 Needs recording. |
| ST5 | `sound-table.md` §5 r2 | Wrong in the impl: a merged compound call **does** attach its unit (request unit list + unit's list, head, no dedupe). |
| ST6 | `sound-table.md` §6.4 r1 | `0x007A061C` is the game-loaded flag (S→C 0x04 sets, 0x05 clears): always on in game. Tracking on. |
| ST7 | `sound-table.md` §10 r5–r6 | Preload at T = 0, 25, 50 …, ids 1–2,933 only; async reads are collected only there; non-Async-Only pending loads finish synchronously at start. Latency of the read itself: OQ 13 Needs recording. |
| ST8 | `sound-table.md` §4 r6 | No guard: a draw with no local player crashes in 1.14d (seed at address 0x20). d2rs: internal error. |
| ST9 | `sound-table.md` §6.3 r6, §5 r6 | Restart by r1 starts next update (exclusive branches); r5 also runs for a request started in r3 the same pass; new requests at the list **head** (then sorted). |
| ST10 | `sound-table.md` §8.2 r12 | Exact sequence: CRT `log` / f32 ln 2 (0.6931471824645996), CRT `pow(10, ·)`, f32 stores, `cvttsd2si`. Residual CRT ulp → voice log. |
| ST11 | `sound-table.md` §7 r8, §5 r8 | Play position in 4-byte units (frames for the stereo songs), not bytes; start offset ×4 mod data size; set position **does** write distance²; unit lists newest first. |
| OQ2, 4–8, 11 | `sound-table.md` OQ | Pixel points per type; paused path; occlusion × (1 − occ) at the device; duck = single player + ESC/options; songs ignore blocks in the stream; river projection; −2³¹ silent. |
| — | `sound-table.md` §5 r3, §8.1 r1 | Corrections: no-unit position (0, 0, 320.0), unit z = 640.0 (both were 0 in the draft); changes the mode-0 gain of every unit sound (e.g. (320, 0, 640) → 228, not 255). |
| — | `sound-table.md` §6.3 r7 | Original bug: a failed fade-in start leaves volume 0; a looping request then never starts. |
| TR1 | `triggers.md` §7 r7 | 120 null records behave as all-zero rows: no transition, loop 0 → detach looping requests (no force), U+0x70/0x74 still set. "No call" is observably the same except that detach. |
| TR2 | `triggers.md` §7 r8 | Cairn table `0x00728338`: modes 1–5 → 413–417 `cairn_stone_1..5`, else 0. |
| TR3 | `triggers.md` §10 r5, `npc-greetings.tsv` | 35 classes → 28 records, dumped (new TSV, own commit); records shared by classes share last/tick. |
| TR4 | `triggers.md` §4.3 r2.2 | As implemented: >1 unit → detach with force; else fade to 0 len 6 (raised to Fade Out), U stays listed; every request of U in Neutral's group, playing or not. |
| TR5 | `triggers.md` §5 r9 | Speed is signed i16; sums wrap i32; reduction by mask (power of two, incl. 0) or signed `idiv` remainder; distances/compares signed; elapsed tests unsigned. Not plain u32. |
| TR6 | `triggers.md` §1 r11 | Id-0 requests return 0 at the entry with no side effect or draw; entry 74 logs many (caller `0x004D9BC7`), seed unchanged. |
| TR7 | `triggers.md` Test vectors, §10 r6 | 115 record addresses, 106 distinct contents (wording fixed); key 506 twice (order 37 → 3,533 wins, 38 → 3,534 never); lookup scans to a zero sound, 16-bit key. |
| TR8 (OQ 7, 11) | `triggers.md` OQ | NPC Speech flag only set by the options menu (image value 1); Init voice at client monster creation (0xAC / `0x00466730`), not first sight. |
| EN-A | `environment.md` §5 r2 | As implemented: `0x004BA950(a, 64 if weather active else 0)`, `0x004BA9D0(ev, 0)`; "raining" = this tick's weather-active flag, not the intensity. |
| EN-B | `environment.md` §6 r4 | Wrong in the impl: a gone handle reads volume 0, then min(6, v) is written to nothing and the stale handle is kept; rain stays silent until v = 0 or weather off. Also previous := 64 on every active tick (even at intensity 0). |
| EN-C | `environment.md` §3 r1 | As implemented (resume −1 → offset 0xFFFFFFFF), and the stream then starts at 0xFFFFFFFC mod data size (deterministic). |
| EN-D | `environment.md` §2 r11 | Listed per test: unsigned differences (75, 62, gap, C − P+0x7C), unsigned absolute compares (T > Ts + 125, C ≥ tM / tS, C < tH), signed play-position compares. |
| EN-E | `environment.md` §4 r1 | last checked := L before the flag test (after the count and equality tests). |
| EN-F (OQ 3) | `environment.md` §1 r3 | Day phase = lighting period index (act env +0x00); day = 1–3; entries 69 / 74 confirm (bed 70 → 71 at the period-4 start, with the §7 draws). |
| env OQ 2, 6, 7 | `environment.md` | Positions in frames; level flags reset per game, last checked never (bug kept); +0x220 = `-ns`. EAX call order corrected (before the cues). |
| wav OQ 3 | `formats/wav.md` OQ 3 | Stereo voice: the load's format check overwrites the row's `Stereo` from the file (`0x004DF695`); all 30 mismatched rows are non-stream. |
| — | `formats/wav.md` status, Survey | Status verified for parsing (C72 / entry 72); entry 62 Huffman table 8 fact added. `sound-table.md` status: table layer verified (C73 / entry 73), code predates this pass's corrections. |
| triggers OQ 3 | `triggers.md` OQ 3 | Partly: the +0xB0 writers are the player / monster mode machines (sites listed); the message field is `client/msg-units.md`'s. |
| — | `triggers.md` §1 r6 | Correction: the idle gap starts at 90 (reset each game by `0x004CA280`), not 0. |
| — | `triggers.md` §12 | Thunder draws on the player client seed (500 + roll(1500) timer, 25 + roll(50) delay, y then x = −200 + roll(400)) and sets the position. |

## Still open

| Id | Why |
|---|---|
| ST4 / sound-table OQ 12 | natural-end tick depends on real audio time; recording below |
| ST7 / sound-table OQ 13 | async read completion time; recording below |
| sound-table OQ 3 | seed interleave confirmed by entry 74, users / order not identified |
| sound-table OQ 14 | other writers of the device gain G |
| sound-table OQ 1, 9, 10 | voice-log conformance; slider mapping (`client/ui.md`); async effect in practice (with OQ 13) |
| triggers OQ 1, 2, 4, 5, 6, 8, 10, 12, 13 | need the request log replay or the owning features' specs (not settled by this pass); OQ 3 partly |
| environment OQ 1, 4, 5 | recording replay (entry 74 is partial: no cave, no Blood Raven); weather intensity (weather spec); front end |
| wav OQ 1, 2 | DirectSound buffer dump (C75, player lane) |

## CODE-TABLE CHANGE commits

| Sha | File |
|---|---|
| 0d617d9 | `specs/audio/npc-greetings.tsv` (new; TR-3) |

## Cross-file requests

- to PC 1: `specs/formats/mpq.md` Observations; new observation; every
  ADPCM-masked `.wav` sector (mask 0x41 / 0x81, 350,543 sectors) uses
  Huffman weight table 8 and no other, 89.4 % with an escape
  (`docs/handoff/local-buddy-q-data.md` entry 62); add it (the q-data
  note says it was not yet recorded there). Mirrored in `formats/wav.md`
  Survey.
- to PC 1: `specs/client/audio.md` §A3; the original detects a one-shot's
  end with a 50 ms wall-clock service thread (`0x00516250`) seen by the
  next sound-tick upkeep (`0x004DF890`), so end ticks are not
  tick-exact in 1.14d (`audio/sound-table.md` §6.6); state the d2rs
  end-tick model there (elapsed ticks × 40 ms ≥ duration) and that its
  conformance waits for `sound-table.md` OQ 12.
- to PC 1: `specs/client/audio.md` §A4; device gain = trunc((1 − occ) ×
  trunc(v × G / 255)) / 255 with G = 255 in game (`0x005157B0`,
  `audio/sound-table.md` §8.3 r3); the `GainCurve` must take the
  occlusion (0 or 0.5 targets, 0.05 steps) as an input, not only v and
  pan.

## Recording list

- sound-table OQ 12 (ST-4): one-shot natural-end ticks (hook the state-2
  store at `0x004DF8D2` with T, and `0x004E01B0` starts) for a few hundred
  known one-shots; compare end − start with ceil(frames / 882).
- sound-table OQ 13 (ST-7): `Async Only` first start attempt tick and the
  collecting preload pass tick (`0x00482BF0`).
- triggers OQ 3: write watch on client unit +0xB0 during a fight (which
  S→C message field arrives there).
- Existing recordings used: entry 74 (`snd74*-sound.jsonl`) and entry 69
  (`env69-sound.jsonl`); still missing from entry 74: a cave walk and
  Blood Raven's death (stinger 34), needed for `environment.md` OQ 1.
