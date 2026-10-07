# PC 2 spec worker: items and the world economy (topic items), 2026-10-07

Branch `claude/spec-items` (from `claude/local-pc2-integration` ebc0577,
which already holds `claude/spec-items-answers`). Owned files:
`specs/items/*` and `specs/world/{vendors,cube,waypoints}.md` with their
TSVs. Inputs: `pc2-spec-d2s.md` cross-file requests, `wire-world-staging.md`
§3 item 3, HANDOFF §7 ninth set (WW-9, DS-2a–DS-5), the local sweep
finding of `local-buddy-tri-sim.md` (branch
`claude/local-buddy-tri-sim-2026-10-07`), the orchestrator relay of
`xpc-to-pc2.md` (PC 1). Evidence: 1.14d exports and `tools/ghidra/disasm.py`;
addresses in the specs.

## Answered

- d2s request / `bitstream.md` OQ3 (DS-5) -> §5 rule 2, OQ3 Answered: the
  trailer values are item data +0x1C / +0x20 (D2MOO `dwRealmData`), bit =
  (+0x20 ≠ 0); read only when save version > 0x56, one discarded u32 when
  > 0x5D; setter `0x00629EA0` called only by `0x0062AC26` and `0x0062D2EA`.
- d2s request / `bitstream.md` OQ1 (DS-2a) -> §4.1 rule 4, OQ1 Answered,
  edge case 9: the 8 save callers pass alt 0; of the 25 network sites only
  `0x0053EFB1` (0x9C action 0x0B from the store check `0x0053EF30`) passes
  1, for an unidentified quality 4–9 item, i.e. a gamble-list item; the
  alt code is items `normcode` (+0x84). Reader rule: flags stored without
  0x2000000 / 0x80000; `0x0062CBE0` reads the code, ilvl 1, quality 1, ends.
- d2s request / `bitstream.md` §2 rule 5 (DS-2b, DS-2c) -> §2 rule 5: child
  loop condition (alt 0, children ≠ 0, unit +0x60 ≠ none; not gated by
  compact or `hasinv`), each child byte-aligned (`0x00411070`), children
  go through the same record writers so the §4.1 r8 / §4.3 r7 write-backs
  apply to them; peek `0x0062AE20` child count 0 for compact / alt-code.
  Also edge case 10 (peek maps `nec ` → `neg ` for version < 0x5D).
- d2s request / `generation.md` §1.4 -> rows 0x2000 and 0x80000: every item
  created from a save record (`0x00558CB0`, `0x00558D37`–`0x00558D4C`) and
  the item copy `0x0055A2A0` sets 0x80000 and clears 0x2000.
  `inventory.md` §5.7 (the inventory pass) has no flag row, nothing to
  change there.
- wire-world-staging §3 item 3 (0x61 item copy `0x0055A2A0`) -> already
  specified in `world/vendors.md` §7.3 (re-read against the disassembly:
  matches); added: the copy advances the game seed 2 · (1 + k) steps (unit
  and item seed in `0x00555230`, k fillers read); `cube.md` OQ4 partly
  answered, `inventory.md` Randomness item 2 updated.
- WW-9 (`0x00559A30` quest_drop in no items spec) -> new
  `items/treasure.md` §9: arguments, level, drop-code class or random class
  pick (`0x00556240` split), magic retry loop, floor drop, request fields,
  return value. The three sub-pickers are treasure OQ10.
- local sweep `sweep_create_every_item_every_quality` ("affix 1 does not
  fit", item 519 `ibk`) -> `affixes.md` §1 rule 4, OQ4, Edge case 3
  measured, OQ3 corrected: suffix slot 0 of `scro` / `book` items is the
  `books` row index (`0x00556E80` → `0x005C2540`, `0x00627FB0`: `isc` /
  `ibk` 1, `0sc` 3), correct 1.14d output; Edge case 3 is only the 10
  ilvl-1 crafted requests. Implementation note (not a spec change): the
  test's `check_item` (`crates/d2-sim/tests/game_items.rs`) must skip
  suffix slot 0 for types `scro` / `book`; then print every remaining
  failure grouped by (item, error).
- xpc-to-pc2 line: cube.md §1 0x77 — done in <sha1>.

## Still open

- `bitstream.md` OQ2 (no recording of set / unique / rare / runeword / ear
  / gold / book / filled socket streams), OQ4 (name setter callers).
- `treasure.md` OQ10 (new): sub-pickers of the quest drop with no drop
  code.
- `cube.md` OQ4: item init, item request, free-spot, portal draws.

## CODE-TABLE CHANGE commits

None.

## Cross-file requests

None.

## Recording list

None new.
