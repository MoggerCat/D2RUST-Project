# Handoff: PC 2 spec worker, quests core — `claude/spec-quests-core`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).

Spec session, 2026-10-07, from `claude/local-pc2-integration`. Files
owned: `specs/world/quests.md`, `specs/world/quests-act1*.md`,
`specs/world/quests.tsv`, `specs/world/quest-messages.tsv`. Every answer
is read from the 1.14d `Game.exe` (exports, `tools/ghidra/disasm.py`,
`Game.exe` bytes for jump/message tables, live `objects.txt`).

## Answered

| Id | Spec § | Answer |
|---|---|---|
| QA-1 | `quests-act1-rest.md` §9 item 1 | `0x005B2A00` returns null for a null room at `0x005B2B50` before any allocation or draw; `0x00463740(null)` = null. The trap retry with no room spawns and draws nothing: d2rs's skip is exact |
| QA-2 | §9 item 2 | a roomless object is possible (`drlg/rooms.md` §8.2 r4); 1.14d's effect per site is tabled: nothing spawns / no missile while the trap monster is none / missile still made once it exists; only the gibbet event's portal (`0x0056D130`, null room → internal error `0x0056D147`) is fatal. Replace d2rs's `QuestError::Fatal` at the other sites with these effects (also supersedes §8 item 3's policy) |
| QA-3 | §9 item 3 | the party walk is inside L4's test (`0x00593195`–`0x005931AF`); reading correct |
| QA-4 | §9 item 4 | no null test before `0x00538680` reads client +0x0A: 1.14d would fault; treat as fatal (invariant) |
| QA-5 | §9 item 5 | whole-binary scan of chain-4 lookups: no reader of extra +0x38 |
| QA-6 | §9 item 6 | `0x0058F870` (= `quests-act1.md` §10.3 event 11) sends and draws nothing; does nothing for message 92: order unobservable |
| wiring §3 item 1 (C79) | §9 item 7 | `InitFn` runs inside `0x00555230` after the unit seed step, before add-to-world and the `PreOperate` draw; the town Cain spawn's draws (population §9.3, init §4) happen there. Inits 6, 7, 9, 54, 61 all draw, schedule or set a mode the footprint stamp reads: draining them later is exact for none |
| WW-6 (Act I) | §9 items 8–11 | gibbet init 7 (`0x00544990` → `0x00594060`), tree init 9 (`0x00593FC0`), cain portal init 61 (`0x00594290`) and its event 7 `0x005942C0` (new extra +0x80, +0x92), Wirt's body operate 33 (`0x00583E70`, drop code `leg `). Wirt's body has `InitFn` 0; init 37 (`0x0059DA50`) is chain 13's (Act II), used by no 1.14d row |
| WW-10 | §9 item 12 | `0x0061AED0(room, clear)`: DRLG room flag 0x400000 set (clear = 0) or cleared; it blocks room removal |
| DS-4 (impl-d2s) | `quests.md` §1.8 | completion = bit 0 (or 15) of the slot (`0x005462FD` / `0x0054630F`); bits a played completion leaves vary per path (13/14 dropped at load, 12 only after 0x58); minimal accepted = bit 0; slot 41 / slot 4 bit 10 are not completions. Exact per-quest bits: recording below |
| `quests.md` OQ6 / QE-7 | `quests.md` §2.4, OQ6; `quests.tsv` row 40; `quest-messages.tsv` | Act V intro init `0x0058EA50` read; row 40 filled, table `0x00732FF8` (15 rows) added |
| QC-7 | `quests.tsv` `spec`, `quests.md` §2.4 | Act III rows 17–24, 39 → `specified`; owner file per act named |
| impl-quests-act1 note items 1–10 | `quests-act1-rest.md` §1–§9 | all settled (none left open) |

Also: `quests.md` §6.7 now names the two NPC bit arrays (record +0x00
first-talk, setters / testers listed; +0x04 introduced), consistent with
`formats/d2s.md` §6 rule 3.

## Still open

- `quests.md` OQ14 (new): per-quest completion bits — needs a recording.
- `quests-act1-rest.md` OQ2–OQ5 unchanged (objects-spec ownership,
  recordings).

## CODE-TABLE CHANGE commits

| SHA | File |
|---|---|
| cfd93db | `specs/world/quests.tsv` (QC-7, Act III `spec` column) |
| b242ee7 | `specs/world/quests.tsv` (row 40) |
| 39dabf1 | `specs/world/quest-messages.tsv` (+15 rows, table `0x00732FF8`) |

Test assertions that must change (cargo not run here), in
`crates/d2-sim/src/world/quests/tests.rs` `tables_parse_and_check`:
`assert_eq!(t.messages.len(), 779)` → `794`; `assert!(t.rows[40].unknown
&& t.rows[40].callbacks.is_empty())` → row 40 is no longer unknown and has
2 callbacks. `act5/intro.rs` keeps its own `TABLE` (equal to the new rows)
until an implementer switches it to the TSV.

## Cross-file requests

- to PC 1, `specs/drlg/rooms.md` §8 rule 1: the setter of DRLG room flag
  0x400000 is `0x0061AED0(room, clear)` → `0x0061BAC0` (clear = 0 sets,
  ≠ 0 clears; quest code passes 0, missile bodies 1); please own it there
  (currently stated in `quests-act1-rest.md` §9 item 12).
- to PC 2 objects (`specs/world/object-functions.tsv`): init 37
  `0x0059DA50` is chain 13 (A2Q6) code with no 1.14d row; owner should be
  `world/quests-act2.md`, not "todo" (WW-6 wrongly calls it Wirt's body's).
- to the wiring / implementation (HANDOFF §5 C79): quest inits must run
  inside the object allocation (`quests-act1-rest.md` §9 item 7).

## Recording list

- R-QC-1 (`quests.md` OQ14): 1.14d expansion character that completed
  every Normal quest; save right after the last one, then after one more
  game; dump both quest sections and list per slot the set bits.
- R-QC-2 (`quests-act1-rest.md` OQ12, = C79): creation of the town-Cain
  marker (class 385) after Cain left Tristram, packets + RNG: Cain's
  spawn draws must come between the marker's unit-seed step and the next
  preset unit's.
