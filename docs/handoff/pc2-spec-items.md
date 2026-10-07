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
- xpc-to-pc2 line: cube.md §1 0x77 — done in 2a72110.
- Orchestrator relay #2 (ui worker) -> `vendors.md` §7.1 and §8.1:
  confirmed; the client writes the item mode (u16 unit +0x10) << 16 in
  0x32's u32 @9 (`0x004B2713`, `0x004B2760`–`0x004B2763`), and for a
  one-item 0x35 the mode at @9 and total stat 72 at @13 (`0x004B27F1`).
- `impl-items` OQ-G1 -> `treasure.md` §8 step 1: the gold override is
  request +0x54 (`0x00557AF9`).
- `impl-items` OQ-G2 / `gaps-items-stats` Q1 -> `generation.md` §9 rule 2,
  OQ5: only the socket count is format-0 (now specified); flag copies for
  every forced request.
- `gaps-items-stats` Q2 -> `generation.md` OQ7: §5.3 only with quest and a
  request.
- `mutants-items-treasure` MT2 -> `generation.md` OQ6: §3 step 9 is dead
  inside the pipeline (only `0x00579D60` sets 0x1000000).
- `impl-items` OQ-A1 -> `affixes.md` OQ5: force = 1 in rare, crafted,
  automagic (pushes listed).
- `impl-items` OQ-P1, P2 (and MT1), P3, P4 -> `properties.md` OQ5–OQ8:
  functions 18 / 19 use the plain list set (no valshift); function 14
  with cap < 1 writes nothing; runeword rows match with exactly c runes;
  set bonus lists use flags 0.
- `impl-items` OQ-Q1 -> `quality.md` OQ3 and the §8.1 vector reworded:
  the accept test precedes `nolimit`.
- `impl-treasure` questions 2, 5, 6, 8 -> `treasure.md` §3.1 (exact gate
  order: bone wall fatal before the collision test), OQ11.
- `impl-vendors` V1 -> `vendors.md` §7.2 rule 7: mask `0x006CE270` = 4,
  the uniqueitems `carry1` bit. V11 -> §7.2 rule 8: stat 70 := maxstack
  + stat 254 (cap 511) on every restored copy.
- `server-items` SI1 -> `cube.md` §2 step 1: the range test returns 1.

### Second pass (worktree `w-spec-items2`, 2026-10-07)

- `impl-vendors` V2 → `vendors.md` §9.2 rule 4: an affix id 0 or above
  the count has no record (`0x00633EE0`), adds nothing, slot by slot.
- V3 → §9.2 (A), (B): (A) and (B) encode 1 skip a skill id ≥ the count;
  (B) encode 2/3 read through a null record (fault, unreachable).
- V4 → §9.2 (B) encode 4: min = ((v>>2)&0x3FF)−256, max =
  ((v>>12)&0x3FF)−256, u = (min+max)/2 toward zero (`0x0065CA30`).
- V5 → §9.2 rule 6: the filler's record `cost` / 2 (`0x006292F0`).
- V6 → §9.4: normal code = `normcode` else `code` (`0x006287D0`);
  missing → null read / fatal 0xB1A (all 508 live normcodes exist);
  literal tests confirmed.
- V7 → §3.1 step 2 **corrected**: a round-2 mismatch returns null; a
  missing upgrade code requests class 0 and ends null; a null creation is
  a fatal assert (0x61A), not a failed try. §3.3 updated.
- V8 → §3: list codes always found (unreachable).
- V9 → §5.1: gamble index never none; rin/amu missing → item 0.
- V10 → §7.1 rule 2: GUID and kind of every 0x2A (refusals GUID −1
  kind 0, except §7.1 rules 1–2: requested GUID).
- V12 → §8.1 rule 7: handler `0x0054BB60` returns 0 for every 17-byte
  message; the routine's own results listed.
- V13 → §3.1 step 5, §7.1 rule 12: item flags 0x10 / 0x1 / 0x2;
  "store item" = the named source item.
- V14 → §4 step 3: inventory link order.
- `impl-treasure` 1, 3, 4, 7, 9–12, 14 → `treasure.md` OQ12 (strtol
  saturation; SSE2 `cvttsd2si` → 0x80000000; null TC fault; idiv fault;
  act table 0–4; +0x30/+0x32 = 0 in all 853 bin rows; quote cut; empty
  search → none; fatal list). 13 → OQ7 answered (`0x005541B0` = dead).
- `treasure.md` OQ10 → new §9.1: the three sub-pickers (armor
  `0x00555E70`, weapons `0x00555FB0`, misc `0x005560F0`), filter
  `0x00555E00` (rarity roll against `0x006427F0(L)`), `p6` = type filter,
  `p7` = skip the rarity roll; n = 0 returns an uninitialised slot.
- `server-items` SI2 → `cube.md` §8 "Exact" 1: `0x00557FD0` = unlink
  from any player list / cursor, then `0x00555600`; nothing else.
- SI3 → `cube.md` §2 step 3.1: argument = the player; 0x3F bytes
  `3F FF <GUID> FF FF`; cursor item not reset.
- SI4 / `cube.md` OQ1, OQ2 → answered from the code (§8 "Exact"):
  0x9D action 5 at once; removal and free send nothing; 0x9C action 4
  per placed item in the player update; S→C 0x2C to the actor's client.
- `cube.md` OQ4: item init / request → `generation.md` Randomness;
  free-spot `0x00545340` draws nothing. OQ5 → link order.
- `impl-world` C1 → §6.4 (`eli` only when `exc` clear); C2, C3 → §7.3
  null cases are fatal asserts; C4 → §7.6 step 2 (no bound, 18 = max);
  C5 → §7.5 (**correction**: the scan ends at 256 candidates; N = 0
  loops).
- `impl-world` W1 → `waypoints.md` §6.2: no room = act 0.
- `bitstream.md` OQ4 partly (5 of 7 sites bounded).
- Relay: `properties.md` §10.1 load rule for broken runewords
  (`0x00563470`); `generation.md` §1.4 callers were already listed.

## Still open

- `bitstream.md` OQ2: Needs recording (see Recording list). OQ4: the
  two request-name sites (`0x0055903B`, `0x0055910E`): writer of
  request +0x58.
- `cube.md` OQ4: portal creation `0x0056D130` draws.
- `treasure.md` OQ5 (x87 precision control), OQ12 tail (d2rs value for
  the sub-picker n = 0 result: a Ruleset choice).
- Not items: `impl-items` "stat vs base" readings; `gaps-items-stats`
  3–6.

## CODE-TABLE CHANGE commits

None.

## Cross-file requests

None.

## Recording list

- `bitstream.md` OQ2: pick up and store a set, unique, rare, runeword,
  ear, gold pile, tome and a socket-filled item; record S→C 0x9C / 0x9D;
  each stream must decode to its end and re-encode byte for byte.
