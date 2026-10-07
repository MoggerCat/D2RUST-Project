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

## Still open

| Id | Why |
|---|---|
| ST4 / sound-table OQ 12 | natural-end tick depends on real audio time; recording below |
| ST7 / sound-table OQ 13 | async read completion time; recording below |
| sound-table OQ 3 | seed interleave confirmed by entry 74, users / order not identified |
| sound-table OQ 14 | other writers of the device gain G |

## CODE-TABLE CHANGE commits

(none yet)

## Cross-file requests

(none yet)

## Recording list

- sound-table OQ 12 (ST-4): one-shot natural-end ticks (hook the state-2
  store at `0x004DF8D2` with T, and `0x004E01B0` starts) for a few hundred
  known one-shots; compare end − start with ceil(frames / 882).
- sound-table OQ 13 (ST-7): `Async Only` first start attempt tick and the
  collecting preload pass tick (`0x00482BF0`).
