# Spec: Data — Bin field tables (`fields.tsv`, `tables.tsv`)

- **Status:** verified by `data-tool tables` (2026-10-05). Extracted from 1.14d `Game.exe` on 2026-10-05; checks below. Embedded and parsed by
  `d2-data::schema` (data-driven) and generated into typed structs, one per runtime table
  (`d2-data::codegen` → `d2-data::tables`; `data-tool gen-tables`; a unit test fails when the
  committed output is stale). A unit test recomputes the §5 unwritten ranges from
  the footprints (1,702 bytes, 55 tables, equal to the notes); compiling every list reproduces the live `.bin`
  files (`loading.md` §11, d2rs cross-check; confirmed by bin cross-check).
- **Target version:** 1.14d
- **Crate/module:** `d2-data::schema` (parsed lists), `d2-data::compile` (field lists), `d2-data::bin` (record sizes)
- **Related specs:** `field-types.md` (§1, §3 type IDs, §6 linkers, §8 callbacks), `loading.md` (§6–§10), `txt-format.md` §6

## Summary
`fields.tsv`: every field list 1.14d passes to its table loader `0x006122F0`, one row per entry.
`tables.tsv`: one row per loader call (92). They replace prose per-table layouts; code is generated
from them, and per-table specs only add behavior the lists cannot express.

## Rules
### 1. `fields.tsv` (tab-separated, LF, header row, ASCII)
| Column | Meaning |
|---|---|
| `table` | table name from `tables.tsv` |
| `seq` | 0-based entry index in the list. Rows are in list order; tables in `tables.tsv` order |
| `column` | the entry's name string, exact bytes (binding is case-insensitive, `txt-format.md` §6) |
| `type_id` | 1–26 (`field-types.md` §3). The terminator (ID 0) is not a row |
| `type` | §3 vocabulary word without its parameter; only these occur: `u8 u16 u32 bit str code4 key(code4) key(name16) key(name32) link8 link16 link32 strkey cb` |
| `length` | `str` N, `bit` n; else 0 |
| `offset` | record byte offset. For ID 23 callbacks it is only the callback's argument (`input 1`–`7` → 0–6) |
| `link` | `<table>.<column>`: the linker filled by that table's key entry. `items.code`: filled by `weapons`, `armor`, `misc`. `@range`, `@treasureclass`: hand-built (`field-types.md` §6.4). `calc(<buffer>)`, `param`, `cb(<table>.<name>)`: callbacks (§8). Empty: link dword 0, or ID 22 (always the string-key resolver `0x6117B0`) |

- Links on types that read none are kept but unused: `monequip` `item1`–`item3` (`code4` → `items.code`),
  `cubemain` `param` (`u32` → `cb(cubemain.param)`). `type` merges IDs 2/8, 4/6, code/name links: use `type_id`.

### 2. `tables.tsv` (same format)
| Column | Meaning |
|---|---|
| `table` | loader name argument, or a distinct name (below). Rows in execution order |
| `txt_name`, `bin_name` | files the loader builds: `<name>.txt`, `<name>.bin`; `leveldefs` reads `levels.txt` |
| `record_size` | size argument of the call |
| `load_step` | `loading.md` §6 step. `S.NN` = the NNth compile-only call after step S (lookup group 1.01–1.15, `sounds` 3.01, temporary monstats/skilldesc links 9.01–9.02). Empty: never called |
| `key_column` | column of the list's key entry (IDs 10–18), which fills the table's own linker |
| `live_source` | `archive/file` read in normal play, or `-` (only compiled with `-txt`) |
| `notes` | call-site VA; txt archive when not `patch_d2`; combined arrays (`loading.md` §9); self links; unwritten byte ranges |

Distinct names: when a table name is compiled with two lists, the compile-only one is
`<table>_lookup` (`monmode`, `plrmode`, `skills` in the lookup group; `monstats`, `skilldesc` inside
the skills routine). Each owns its own linker (e.g. `skills_lookup.skill` is read by `missiles` and
`states`; `skills.skill` by later tables). `unused_653db0` is the list of the uncalled generic loader
`0x653DB0` (2-byte `Name` strkey; its name and count are parameters).

### 3. Extraction
1. Map `Game.exe` sections from the PE headers (base 0x400000). Scan `.text` for `E8` calls to
   `0x6122F0`: 92 sites in 63 functions. Five stack arguments: pool, name, list, &count, size.
2. Simulate each containing function linearly from its entry with a scratch x86-32 decoder: register
   constants, `ebp`/`esp` frame slots, the stack probe `0x6831B0` (frames > 4 KB), and the constant
   pointer `0x744304` = `0x96BC30` (the linker block; no code writes it). At each call read the
   arguments from the stack and the list from the frame at the list argument.
3. Entry = 20 bytes, 5 dwords: name pointer (`.rdata` string), type ID, length, offset, link pointer
   (a linker slot in the `0x96BC30` block, a frame slot of `0x613F80`, or a function). Confirmed by
   the binder `0x6BCE20` (stride `+0x14`, stops at dword `+4` = 0) and the compiler `0x6BD780`
   (index × 5 dwords). Terminator: name `end` (`0x6D9060`), type 0, rest 0; all 92 lists end with it.
4. Execution order: walking the static call tree of the load-all routine `0x619300` reaches 91 sites
   in exactly the `loading.md` §6 order; `0x653DFF` is unreachable.

### 4. Provenance of names and links
- Table names: the name argument. Column names: the entry strings, read from `.rdata`.
- Linker names: a link value is a slot address. Its name is the `table.column` of the key entry whose
  link is the same slot. Hand-built slots: `0x96C5E8` (`@treasureclass`, stored at `0x65A3A2` in the
  TC routine `0x65A390`) and frame slot `ebp−4` of `0x613F80` (`@range`, stored at `0x617760`).
  The temporary monstats and skilldesc linkers are slots `ebp−8` and `ebp−0xC` of `0x613F80`.
- Callbacks by address: `0x611BD0` `calc(skillscode)`, `0x611C70` `calc(skilldesccode)`, `0x631530`
  `calc(itemscode)`, `0x6619A0` `calc(misscode)`, `0x6336A0` `param` (`field-types.md` §8);
  `0x651150` `cb(monstats.skillmode)`, `0x657010` `cb(monstats2.composit)`, `0x659990`
  `cb(monpreset.place)`, `0x669080` `cb(cubemain.input)`, `0x6690B0` `cb(cubemain.output)`,
  `0x6690E0` `cb(cubemain.param)` (suffixes follow D2MOO names).
- Cross-checks: an independent extraction from the Ghidra decompiler text (earlier session) agrees on
  all 3,591 entries. D2MOO 1.10f: same column order and offsets in all 92 lists; the type differs in 7:
  `monplace` `code` (17 vs 10), `gamble` `code` (9 vs 10), `Name` in the four `unique*` name tables and
  `unused_653db0` (22 vs `str(31)`).

### 5. Mechanical checks (scratch scripts; results 2026-10-05)
| Check | Result |
|---|---|
| Fields fit the record (`str` N+1 bytes, `key(str)` N, `bit` byte offset+(n>>3), `calc`/`param` 4, others by width) | 3,464 entries: 0 outside. 35 ID-23 entries not checkable (offset is an argument) |
| Alignment: 2-byte fields at even offsets, 4-byte at multiples of 4 | 0 violations |
| `record_size` = live `.bin` size (`4 + count × size`) | 86 of 92 exact: all 73 record tables (step, size, archive, count and bytes also equal `loading.md` §6) and 13 lookup by-products. 5 `_lookup` lists have no `.bin` of their own (the record table overwrites it); `unused_653db0` has none |
| Column names in the header of the highest-priority `.txt` (P → X → D, case-insensitive) | 173 of 3,499 absent (below). 357 more match only by case |
| Link targets (574 reads, IDs 11/13/15/19–21) | 0 failures: every target has an owner or is hand-built; code IDs read code keys and name IDs name keys; the owner compiles earlier, or is the same table (self links: `itemtypes`, `montype`, `itemstatcost`, `missiles`, `skills`, `monstats`; pass 1 fills them first). All 40 key entries name their own `table.column` |
| Fields at the same offset | `levels` `camt1`–`camt4` (u16 at 220; the rightmost bound column wins) |
| Partial overlaps | 18 pairs, each a `str(N)` whose NUL lands on the next field's first byte: `pettype` `baseicon`→`micon1`→…→`micon4`; `overlay` `Filename`→`version`; `missiles` `CelFile`→`AnimLen`; `treasureclassex` `treasure class`→`picks`, `item1`→…→`item10`→`prob1`; `hireling` `namefirst`→`namelast` |
| Bytes no field writes | 1,702 bytes in 55 tables (ranges in `tables.tsv` notes). In `monstats`, `monstats2`, `monpreset`, `cubemain` the table-specific callbacks write some of them |

Columns absent from the txt (each one gets its missing-column value, `field-types.md` §3):
- `weapons` (35): minac, maxac, block, absorbs, missile, BetterGem, rArm, lArm, torso, legs, rspad,
  lspad, subtype, autobelt, spellicon, pSpell, state, cstate1, cstate2, len, stat1–3, calc1–3,
  spelldesc, spelldescstr, spelldesccalc, HraltiMagicLvl, Transmogrify, TMogType, TMogMin, TMogMax, multibuy.
- `armor` (39): hit class, 2handmindam, 2handmaxdam, minmisdam, maxmisdam, rangeadder, reqdex,
  spawnstack, missile, wclass, 2handedwclass, 1or2handed, BetterGem, 2handed, subtype, questdiffcheck,
  autobelt, spellicon, pSpell, state, cstate1, cstate2, len, stat1–3, calc1–3, spelldesc, spelldescstr,
  spelldesccalc, HraltiMagicLvl, Transmogrify, TMogType, TMogMin, TMogMax, PermStoreItem, multibuy.
- `misc` (33): hit class, magic lvl, auto prefix, 2handmindam, 2handmaxdam, minmisdam, maxmisdam,
  minac, maxac, block, absorbs, rangeadder, strbonus, dexbonus, reqstr, reqdex, setinvfile, durability,
  missile, normcode, ubercode, ultracode, wclass, 2handedwclass, 1or2handed, rArm, lArm, torso, legs,
  rspad, lspad, 2handed, subtype.
- `magicsuffix` (2): etype4, etype5. `automagic` (X txt, 3): classspecific, etype4, etype5. `monmode`
  (16): DT_Dir … RN_Dir (all `*_Dir`). `levels` (45): mon11–25, nmon11–25, umon11–25.
- Lower-priority copies not checked; e.g. X `inventory.txt` has `gridRows`/`gridCols`, not `gridX`/`gridY`.

## Randomness
None.

## Edge cases & original bugs
- No duplicate column names in any list; longest list 253 fields (`monstats`). NUL spills and `camt`
  overlaps are original; write order decides (`field-types.md` §2).

## Test vectors
| `fields.tsv` row | Expected |
|---|---|
| `skills 60 pettype 21 link8 0 190 pettype.pet type` | one byte (`field-types.md` §3) |
| `runes 13 rune1 11 link32 0 152 items.code` | cell `r08` → 617 (`field-types.md` test vectors) |

## Open questions
1. Footprints of the table-specific callbacks (§4) are unknown; their tables' unwritten ranges overstate.
2. Not loader calls, so not here: sound tables (`0x481950`), client composite loader (`0x504430`).
