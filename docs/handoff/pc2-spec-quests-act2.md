# Handoff: PC 2 spec worker — Act II quest questions (`claude/spec-quests-act2`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).

Answers to the questions `docs/handoff/impl-quests-act2.md` raised
(HANDOFF §7 ninth set, QB-1–QB-20), read from the 1.14d `Game.exe`
(`tools/ghidra/disasm.py`, raw image bytes) on 2026-10-07. New file
`specs/world/quests-act2-2.md` (§1 answers by QB id, §2 Jerhyn's spawns,
§3 orifice insert / S→C 0x58); `specs/world/quests-act2.md` corrected in
place and its open questions 11–31 list each QB as Answered.

## Answered

| Id | Spec | Answer |
|---|---|---|
| QB-1 | act2-2 §1.1; act2 test vector 1 | typo: table state 2 = msgs 315–324 (`0x007398FC`) |
| QB-2 | act2-2 §1.2 | chain 8 `0x00598980` → `0x00545530` (remove GUID when s.0 and s.1 and list non-empty); chain 7 `0x005987B0` removes from the extra list unconditionally |
| QB-3 | act2-2 §1.3; act2 §9 | both test the NPC class (377 / 378); chain 26 msg 59/60 also installs an inert chat-end callback `0x0059E0B0`; any other 377 message → 30.0 |
| QB-4 | act2-2 §1.4; act2 §1.3 | altar never calls `0x00545850` |
| QB-5 | act2-2 §1.5; act2 §5.8 | status-0 test sits outside the state ≤ 1 block but never passes |
| QB-6 | act2-2 §1.6 | game-start status/state = byte stores only |
| QB-7 | act2-2 §1.7; act2 §6.7 | "if intro: game 11.13" inside the 11.1 block; tome grants only in not-intro games with state ≠ 5 |
| QB-8 | act2-2 §1.8 | +0x09 := 0 every not-intro kill; 13.2 iterate inside status < 2 |
| QB-9 | act2-2 §1.9 | Tyrael: table state 2 if door mode 2, else nothing; always returns |
| QB-10 | act2-2 §1.10 | 442: only state := 5 depends on 14.13; 430: refresh before state := 2; 444–452 table read raw (452 → 14.10) |
| QB-11 | act2-2 §1.11, §3 | generic insert path `0x005852E0`; orifice skips `0x0055EEA0`; 0x58 = 58, GUID, result, byte 6 |
| QB-12 | act2-2 §1.12; act2 §8.11 | members: own test `0x0059CF20` (lacks 14.0, 14.3–14.5, in Act II) → 14.5 |
| QB-13 | act2-2 §1.13; act2 §4.8 | `tr1 ` with 10.3 set: no status, no 0x5D |
| QB-14 | act2-2 §1.14 | one per code (yes/no seam exact) |
| QB-15 | act2-2 §1.15; act2 OQ5 | chain 38 uses intro field A (record +0), 0x91 uses field B (+4) |
| QB-16 | act2-2 §2; act2 §6.10 | base point = palace-Jerhyn object (init 19) or harem blocker (event 3); `r` = the start room of the free-spot search; +0x3C stored only by init 18 `0x0059F380`; init 19 also spawns Kaelan |
| QB-17 | act2-2 §1.17; act2 §8.11 | members lacking 14.0/14.3/14.4 in Act II get 14.13, 14.3, progression step 2 (`0x0059C810`) |
| QB-18 | act2-2 §1.18 | `0x005940A0` removes from chain 4's +0xB4 list; `0x005985C0` false; status fns false (38 writes 0); OQ7 → act1-rest §5 |
| QB-19 | act2-2 §1.19; act2 §3.6 | status ≥ 2, state < 3: state 3 + flag iterate, no 0x5D; busy orifice: return 1, nothing else |
| QB-20 | act2-2 §1.20; act2 §1.3, edge 8, OQ6 | drop code set once before the count; `&level` is an out parameter (area level for objects) |

Also answered: `quests-act2.md` open questions 5, 6, 7.

## Still open

| Id | Why |
|---|---|
| act2-2 OQ1 / act2 OQ31 | S→C 0x58 byte 6 not written for results 0, 1, 4: Needs recording |
| act2 OQ1–4, 8–10 | unchanged (other owners or recordings) |

## CODE-TABLE CHANGE commits

None.

## Cross-file requests

- to PC 2 quests-core: `world/quests.md` (free-spot `0x00545340` uses at §8.4 / §9.1) and `quests-act1*.md`, `quests-act3/4/5*.md` ("radius r" of every `0x00545340` call); 1.14d fact: the sixth argument (`[ebp+0x14]`) has no reader in `0x00545340`–`0x0054547F` (`ret 0x14`), the ring search runs to the seventh argument (limit); change: say the "radius" argument is unused.
- to PC 2 d2s owner: `formats/d2s.md` §6 item 3 (and its open question 10); 1.14d fact: `0x00572360` (field A set) has five direct calls, `0x0058E9D4`, `0x0058EA25`, `0x0058F8C2`, `0x00598464` (chain 38 event 11), `0x005B6CCF` (found by `disasm.py xref`; they lie outside Ghidra function bodies, so `all.asm` misses them); field A = D2MOO `pQuestIntroFlags`; the 1.14d set writes only the first matching pair's bit (no extra bit 0); change: replace "no direct caller" and name field A.
- to PC 1: `sim/server-messages.tsv` 0x58 row and `sim/intents-events.md` §6 (masking); 1.14d fact: 0x58 is u8 0x58, u32 object GUID @1, u8 result @5 (0 open, 1 cancelled, 4 refused, 5 accepted), u8 @6 (1 = accepted with effect) built by `0x0053D8D0`; byte 6 is never written for result 0 (`0x0059DD6A`), 1 (`0x00585479`) and 4 (`0x0058538C`); change: add the field layout and a mask on byte 6 for results 0, 1, 4.

## Recording list

- Orifice insert (Act II, Horadric Staff assembled): operate the orifice (S→C 0x58 result 0), cancel the dialog (C→S 0x44 action 2 → 0x58 result 1), insert a wrong cursor item (result 4), then the staff (result 5, byte 6 = 1). The recording must show the 7 bytes of each 0x58, especially byte 6 for results 0, 1, 4 (`quests-act2-2.md` open question 1).
