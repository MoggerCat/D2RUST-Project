# PC 2 spec worker: `formats/d2s.md` (topic d2s), 2026-10-07

Branch `claude/spec-d2s` (from `claude/local-pc2-integration`, with
`claude/spec-d2s-buddy` f254d8f merged first: it was not in the
integration branch). Owned files: `specs/formats/d2s.md` and the new split
`specs/formats/d2s-load.md` (§9 rules 6–7 moved there; d2s.md was over 60 KB). Inputs:
`docs/handoff/impl-d2s.md` §5, HANDOFF §7 Ninth set DS-1..DS-5,
`wire-world-staging.md` §3 item 4 (save restore), the C66 local run
`local-buddy-q-saves.md` (branch `claude/local-buddy-q-saves-2026-10-07`).
Evidence: 1.14d exports and `tools/ghidra/disasm.py`; addresses in the
spec.

## Answered

- DS-1 / d2s OQ3 (classic part) -> OQ3 Answered: the 13 saves round-trip;
  classic `TestAma` / `TestStub` re-saved by 1.14d end after the corpse
  header with no `jf`/`kf`, status without 0x20.
- DS-2a (alt-code record) -> §8.1 rule 9, edge case 14: every
  save-format call of `0x006313E0` passes alt-code 0 (8 call sites
  listed), so a game save never has one; a hand-made one reads to the
  base code only (`0x0062CBE0`) with 0 children.
- DS-2b (children of compact / alt-code records) -> §8.2 rule 8: the
  peek `0x0062AE20` gives child count 0 when the flags have 0x200000 or
  0x2000000. Confirmed.
- DS-2c (children write-backs, filled == children) -> §8.1 rule 10, edge
  case 15: the writer writes a child per item in the item's inventory
  (`0x006312B0`, not gated by compact or `hasinv`), the reader reads the
  filled count (`0x0062A900`, `hasinv`-gated); the game does not check
  them against each other; d2rs refuses a mismatch. Write-backs: bitstream
  request below.
- DS-3 (`kf` marker without g byte) -> §8.5 rule 5, edge case 16: the
  reader takes g unchecked from the byte after the data (uninitialised
  stack in `0x005343A0`'s 8 KB buffer); undefined by the file; d2rs keeps
  23.
- DS-4 (completed-quest bits) -> §4 rule 4: defined in
  `specs/world/quests.md` (quests-core); d2s.md only links it.
- DS-5 (the 1-bit 0 trailer) -> exact for every item whose item data
  +0x20 = 0; the only setter (`0x00629EA0`) is called only by the two
  save readers; every measured record has bit 0 and the game accepted
  d2rs's. Layout request to bitstream below.
- C66 flag 0x2000 -> §8.2 rule 7, edge case 17, test vectors: the
  loader's item create `0x00558CB0` sets 0x80000 and clears 0x2000 on
  every save item (`0x00558D37`–`0x00558D4C`), so 0x00A02010 ->
  0x00A00010 and 0x00802010 -> 0x00800010 after a load and save.
- C66 +0x88 -> 0xFF -> §2.8, edge case 18: the writer pre-fills the 32
  bytes with 0xFF and only equipped (mode 1) items change them
  (`0x0063E510`); the loader never reads them.
- C66 stub 335 -> 953 -> §9 rule 6 -> `formats/d2s-load.md` §1 (new file, split for size): the new-character start
  `0x00569F80` (stats, start items, `StartSkill` as right skill, quest
  entry mode 1); the next save writes a full file.
- WW save restore (wire-world-staging §3 item 4) -> §9 rule 7 -> `formats/d2s-load.md` §2: the load
  effects in order with code and owner spec, for `d2-server` character
  storage.
- quests-act2 cross-file request (§6 rule 3, OQ10) -> §6 rule 3: field A
  = D2MOO `pQuestIntroFlags`; setter `0x00572360` has five direct calls
  (`0x0058E9D4`, `0x0058EA25`, `0x0058F8C2`, `0x00598464` chain 38
  event 11, `0x005B6CCF`), each re-read with `disasm.py at`; it sets only
  the first matching pair's bit (bit 0 only when no pair matches).
  `formats/d2s-load.md` had no "no caller" claim; unchanged.

- Third pass (2026-10-07, worktree `w-spec-d2s7`, exports + disasm.py
  only; saves read with `tools/d2s_check.py`):
  - OQ17 -> new `formats/d2s-appearance.md` (split, d2s.md at its size
    limit): token table `0x0063D710` (255 entries from the item tables,
    simulated over the 1.14d tables: reproduces all 8 measured component
    values), hand owners `0x0063C050`, body armour parts via `armtype`,
    colour byte `0x0062C100` = (Transform × 32 + colour) mod 256 + 1.
    Broken weapons draw in the left hand (bdMercTwo, bdGolem).
  - OQ5 -> d2s-load §4: reloaded list is file order; indices move once.
  - OQ7 -> +0xCF only from/to the file; realm-only readers.
  - OQ8 -> dead: `0x0052C0E0` (only writer of `[0x00883D50/54]`) is
    unreferenced.
  - OQ9 -> d2s-load §7: single player game type 3 (`0x00477CDF`), seed
    restored when loaded in the saved difficulty.
  - OQ10 -> §6 rule 6: the NPC classes and message ids that set field A
    in each act's callback.
  - OQ11 -> no code sets unit +0xC8 0x8000 on a server unit.
  - OQ12 -> cache flag = interacting with a player (`0x00532400`),
    blob = player data +0x5C (`pTrade`).
  - OQ13 (partial) -> d2s-load §6: stored / cursor non-matching runeword
    item deleted; equipped one unequipped (end place open).
  - OQ14 -> no reader in Game.exe.
  - OQ15 -> d2s-load §3: GUID in client +0x484; on join `0x005394A0`
    re-casts Iron Golem (`0x0056F7F0`) on the item.
  - OQ16 -> d2s-load §5: S→C 0xB4 -> client index -> string ids
    (`0x0070F384`).

## Still open

- OQ1 (legacy loader) and OQ2 (item records below version 0x60, owner
  `items/bitstream.md`): out of scope, unchanged.
- OQ4 (dead hireling, hireling items), OQ6 (corpse path null vs zero):
  partial as before; OQ4 needs local saves.
- OQ13: where the equipped non-matching runeword item ends up
  (d2s-load OQ2).
- d2s-appearance OQ1 (state-colour list builder), OQ2 (no measured
  colour / armour / helm byte yet); d2s-load OQ3 (index confirmation).
- `formats/d2s.md` is 65 KB (over ~60 KB): splitting §8 would move
  rules that code `Covers:` claims cite; left for the orchestrator to
  decide.

## CODE-TABLE CHANGE commits

None.

## Cross-file requests

- to PC 2 items (`items/bitstream.md` §5 rule 2, OQ3): trailer = 1 bit
  (item data +0x20 ≠ 0; getter `0x00629E40`); when 1: 32 bits +0x1C, 32
  bits +0x20, 32 bits 0 (compact `0x0062B341`–`0x0062B387`, full via
  `0x006309E4`). Reader (`0x0062AC05` compact, `0x0062D2AF` full) only for
  save format with save version > 0x56: 1 bit; when 1: 32 bits a, 32 bits
  b, then for version > 0x5D 32 more bits discarded; a, b stored by
  `0x00629EA0` (its only callers are those two readers). Change: name the
  two fields and the version conditions; close OQ3.
- to PC 2 items (`items/bitstream.md` §4.1 rule 4, OQ1): every save-format
  caller of `0x006313E0` passes alt-code 0 (`0x00531712`, `0x00531899`,
  `0x005318C3`, `0x005318F0`, `0x00531929`, `0x0053195A`, `0x00541B5C`,
  `0x0055A2E1`). Reader side: `0x0062E430` strips 0x2000000 (and
  0x80000) from the stored flags; `0x0062CBE0` with alt set ends after the
  32-bit code with item level 1 and quality 1 (no +0x28, no trailer).
  Change: add the reader rule and the save callers to OQ1.
- to PC 2 items (`items/bitstream.md` §2 rule 5): the child loop of
  `0x006312B0` runs when alt-code = 0, children = 1 and unit +0x60 ≠ 0,
  for compact and full records alike and regardless of `hasinv`; each
  child goes through `0x006312B0` -> `0x0062FFF0`, so the §4.1 rule 8 and
  §4.3 rule 7 write-backs apply to every child as well. Reader: the peek
  `0x0062AE20` gives 3 bits filled count only when flags lack 0x200000
  and 0x2000000, else 0. Change: state both.
- to PC 2 items (`items/generation.md` §1.4 / `items/inventory.md` §5.7):
  flag 0x2000 (instore) is also cleared, and 0x80000 set, on every item
  created from a save record (`0x00558CB0`, `0x00558D37`–`0x00558D4C`);
  callers `0x005335E0` (player, corpse, hireling lists), `0x0056ACE0`
  (golem), `0x00541990`. Change: add to the 0x2000 / 0x80000 rows.

- to PC 1 (`sim/rng.md` Open question 2): single-player game type is
  3 (`0x00477CDF` -> `0x0053F17A` -> `0x00530CFF`), +0x84 = 0 without
  `-seed`, so the .d2s map seed (+0xAB) is restored on every load whose
  town byte for the game's difficulty has 0x80 (`formats/d2s-load.md`
  §7). Change: answer the question from the binary.
- to PC 2 quests-core (`world/quests.md` §10.3): the field-A setter's
  callers with their NPC classes and message ids (`formats/d2s.md` §6
  rule 6); `0x0058E990` also sets quest record 31 byte +0xC := 1 (when 0
  and +9 ≠ 0) on class 513 messages 0x4E45–0x4E47. Change: add both.
- to PC 1 (`skills/bodies*.md`, Iron Golem): on join, `0x005394A0`
  re-casts skill 90 through `0x0056F7F0`(game, player, 90, L, 1, 0, 0)
  on the saved golem item, placed at the player and set as the
  player's target (`formats/d2s-load.md` §3). Change: mention that
  caller.
- to PC 2 items (`items/properties.md` §10.1): on load a top-level item
  with flag 0x4000000 that fails `0x0062BED0` is deleted when stored or
  on the cursor and unequipped when equipped (`0x00563470`,
  `formats/d2s-load.md` §6). Change: link it.

## Recording list

No recording needed. Local saves (not recordings) that would confirm:
- d2s-appearance OQ2: one character with a helm, a body armour and a
  coloured (magic / set / unique) item equipped: +0x88..+0xA7 against
  `formats/d2s-appearance.md` rules 4–6.
- d2s-appearance OQ1: a save while Enchant is active on the weapon
  (colour byte 5 = 0x00 expected for `Transform` 1, 2 or 5–8).
- OQ4: a dead hireling (flags 0x10000) and a hireling carrying items.
- OQ15: a Necromancer with an Iron Golem made from an item.
- d2s-load OQ3: a hotkeyed tome linked after the equipped weapon,
  saved, reloaded, saved.
