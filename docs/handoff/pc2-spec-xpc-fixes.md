# PC 2 spec worker: xpc-fixes (2026-10-07)

Branch `claude/spec-xpc-fixes`, from `origin/claude/local-pc2-integration`
271ce75. Applies PC 1's requests from `docs/handoff/xpc-to-pc2.md`.

## Answered

- `world/hirelings-ai.md` OQ1 → Answered by `monsters/ai-bodies-6.md`
  §7 (Hireable `0x005E52D0`, "Hireling attack" `0x005E5050`).
- `world/quests.md` §6.7: the 0x91 byte @1 is the act argument of
  `0x00545100` (stored from BL at `0x00545176`; switch table
  `0x0054519C`, acts 0–4); D2MOO hedge dropped.
- `world/hirelings-ai.md` §1: the skill pick moved to
  `monsters/ai-bodies-6.md` §7 step 7 "Hireling skill" `0x005E4D30`; §1
  is now a pointer (number kept); edge case 1 points to ai-bodies-6
  edge case 4. `world/hirelings.md` §14 and edge case 12 pointers
  updated.
- `world/npc.md` §2 rule 2: links `monsters/ai.md` §9.9 ("Effect of
  param 0 := 40") for what the NPC AI does with param 0 = 40.
- `items/inventory.md`: no rule refers to the attack weapon /
  `0x00623990` (only the weapon-in-use field +0x1C and `0x006233A0`);
  nothing added.

## xpc-to-pc2 lines done

Commit: 4e6adc4 (all four lines).

- line 18 `world/quests.md` §6.7 (0x91 act byte).
- line 20 `world/hirelings-ai.md` §1 (moved to ai-bodies-6 §7; OQ1).
- line 21 `world/npc.md` §2 rule 2 (link ai.md §9.9).
- line 19 `items/inventory.md` (no rule refers to it; also covers the
  superseded line 14).

## Still open

None.

## CODE-TABLE CHANGE commits

None.

## Cross-file requests

None.

## Recording list

None.
