# PC 2 spec worker, session 4: items lane (2026-10-07)

Branch `claude/spec-items-s4` (from `origin/claude/pc2-spec-gaps`
dd480ad5). Owned: `specs/items/*`. Evidence: 1.14d exports and
`tools/ghidra/disasm.py`; addresses are in the specs.

## Written (spec § → behaviour, addresses)

- `inventory-moves.md` §12 (new): corpse take-back. §12.1 outer
  `0x0057FB70` (state 7, permission, experience → `combat/vitals.md`,
  result → corpse list removal `0x0063D4E0`, room leave, S→C 0x8E
  `CorpseAssign` [0][U][C] to all, free, sound 93; failure sound 23).
  §12.2 `0x00562F30`: phase 1 body locations 1–12 repeated (requirements,
  pair map `0x0055F240` 4↔5 6↔7 11↔12, slot fit, grid fallback
  `0x005600A0` leave-room 0, link kind 3/4, timers), phase 2 item list
  (can-pick, §4.9, belt `0x0063C790` + kind 2, grid only on the second
  sweep), result rule. §12.3 slot fit `0x0055F2D0` (differs from
  `inventory.md` §4.4). Corpse lines of §7.1 and `inventory.md` §4.9 now
  point here.
- `inventory-moves.md` §7.8 failure outcomes (`0x00560F00`: E's unlink
  fatal; N put / link failures out 1, unreachable); §7.12 `0x00557FD0`
  defined (unlink from any player list or cursor, then free); §7.1 rule
  2.1 distance formula pointer (`sim/pathing.md` §9.5).
- `treasure.md` §3.1 collision word `0x0064CB30`: room lookup among the
  monster's room and its neighbours; no room / no collision record / no
  grid → 0x27 (no drop); the "no room → fatal 0x1FF" test is unreachable.
  §7 step 4: exact request fields of `0x0055A550` (+0x00 = U).
- `properties.md` §4.2: a new list from `0x0065CBF0` carries owner type 4
  as a constant (also for §11's lists on the player).
- `affixes.md` §9: `0x005C1BC0`(ECX item, EDX prefix) = rare name pick by
  format (§5 / §12.2); the tempered dispatch and `world/cube.md` §7.3 call
  the same routine; clears and saved S listed.
- `bitstream.md` §3 rule 3: name length (16 characters then the zeroed
  +0x5A; reader unbounded until 0); Outputs: the ilvl < 1 → 1 and quality
  → 2 write-backs persist in the server item (> 99 only clamped).
- `generation.md` §12 (new): repair `0x0055F900`, recharge `0x0055FE80`
  with set-charges `0x0065C940`, runeword removal `0x00558C50`.
- `treasure.md` §9.1 / OQ12: d2rs value for the sub-picker n = 0 result
  (−1, no item). `properties.md` OQ4: d2rs "no match" recorded.
- Wording only: stale "not specified" lines (`generation.md` §9 rule 2,
  OQ1; `treasure.md` OQ10; `inventory.md` MV4 tail).

## Code TODOs already answered by the specs (implementer action)

| Code | Spec answer |
|---|---|
| `items/bitstream.rs:341`, `d2-proto/item_bits.rs:268` | `bitstream.md` §3 rule 3 "Length": current behaviour is right |
| `items/bitstream.rs:667` | §4.6 rule 4.3: the record is per list (as coded) |
| `wiring/inventory/bits.rs:91` | `properties.md` §10.1 is the runeword record lookup (`0x0062BED0`) |
| `wiring/inventory/bits.rs:121` | `bitstream.md` Outputs: write the two changes back |
| `items/affixes.rs:124`, `items/quality.rs:46,383` | `affixes.md` §12, `quality.md` §10, `generation.md` §11 (format 0) |
| `items/create.rs:138,554` | `treasure.md` §8 step 1 (+0x54); `generation.md` §9 rule 2 (flags for every forced request) |
| `items/props.rs:405,433,500,595,632,698` | `properties.md` OQ5–OQ8, §13 (set-item state update, not a no-op) |
| `moves/ground.rs:53,97,456` | `inventory-moves.md` §8.1 rules 5 / 7, §9.3 (fatal) |
| `moves/ground.rs:520` | §10.2: a failed pile creation **skips** that pile and goes on (code breaks) |
| `moves/handlers.rs:552` | §7.8 failure outcomes: E's unlink failure is fatal |
| `moves/handlers.rs:1132` | §7.17: no hireling → the potion is used on the **player** (code returns) |
| `moves/handlers.rs:1464` | §7.23: a failed copy is fatal |
| `wiring/inventory/pending.rs:61` | §7.12 / `world/cube.md` §8 "Exact" 1: unlink + `0x00555600`, nothing else |
| `wiring/economy/cube_items.rs:49` | `affixes.md` §9: the two readings agree; wire it as the rare name pick |
| `wiring/economy/cube_items.rs` repair / recharge / drop_runeword_stats | `generation.md` §12 |
| `wiring/economy/death.rs:177` | `treasure.md` §3.1: outside every grid the word is 0x27 → **no drop** (code reads 0) |
| `wiring/economy/death.rs:178` | `treasure.md` §7 step 2: the search gets U's room, never the start lookup's |
| `wiring/economy/item_stats.rs:69` | `properties.md` §4.2: owner type 4 constant, GUID of the unit, reset 1 |
| `wiring/economy/treasure_items.rs:67` | `treasure.md` §7 step 4 exact: +0x00 := U |
| `d2-client/tests/e2e_full_loop.rs:2116` | §7.1 rule 2.1: `sim/pathing.md` §9.5 unit distance (wiring) |

## Pending points

- `properties.md` OQ4 tail (stale runeword slot on load / save / writer
  paths): stack trace at `0x0062BFBA`; d2rs "no match".
- `treasure.md` OQ12 tail (sub-picker n = 0 value): stack capture; d2rs −1.
- Recording-only (unchanged): `affixes.md` OQ1, `bitstream.md` OQ2,
  `generation.md` OQ2, `properties.md` OQ1, `quality.md` OQ1,
  `treasure.md` OQ1–OQ5, `inventory.md` OQ2.

## CODE-TABLE CHANGE commits

None (no TSV changed).

## Cross-file requests

- `combat/vitals.md` §4.7 rule 2: "The item take-back follows
  (`0x00562F30`)" → add "(`items/inventory-moves.md` §12)".
- `world/cube.md` §7.3 table row `useitem` and the null-case paragraph:
  "tempered affix rolls, owner: items" → "rare name pick by format,
  `items/affixes.md` §9 (`0x005C1BC0` → `0x005C1AB0` / `0x005C19A0`)".
- `world/cube.md` §7 (remove[j] `0x00558C50`, repair `0x0055F900`,
  recharge `0x0055FE80`): owner pointers → `items/generation.md` §12.3,
  §12.1, §12.2.
- `world/vendors.md` §8.2: `0x0055F900` "(item spec)" →
  `items/generation.md` §12.1; recharge `0x0055FE80` → §12.2.
- `sim/units.md` §6.5 event 3: "`0x0055F900` (repair path, items spec)"
  → `items/generation.md` §12.1.
