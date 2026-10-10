# re-ghidra-cloud hand-back (REC-1885..1889 unused)

Checks: none run (tooling task; no Rust touched). EQUAL 0 -> 0.

## What changed
- Ghidra 12.1.4 headless runs in the cloud. GitHub release download is
  refused (github.com/NationalSecurityAgency/ghidra outside the session's
  GitHub scope, HTTP 403); `cloud_setup.sh install` takes the official
  distribution from Docker Hub `blacktop/ghidra:12.1.4` (registry API, no
  daemon). Cloud image JDK 21 (OpenJDK, not Temurin) works.
- tools/ghidra/: `cloud_setup.sh` (install | project | types | decompile
  0xADDR | export DIR), `ApplyNames.java`, `ApplyTypes.java` (own parser of
  the strict header format, offset/size checked; retypes or rebuilds
  signatures by stack purge), `spec_harvest.py` (names + argument types
  from specs/), `d2_114d_types.h` (45 structs, 1,342 fields, from specs/
  incl. data/fields.tsv for MonStats/MonStats2/Skills/Missiles/Items).
- Private repo `re/exports-typed/` (commit 8fe7715f): funcs/ (13,085),
  functions.tsv, index/. `re/exports/` untouched.

## Results
- Named functions: 773 -> 1,243 (of 13,085; spec names 470, AI think
  table included; PC 1 renames 28 beyond a fresh analysis).
- Raw `+ 0x` accesses in decompiled C: 40,803 -> 33,185.
- Signatures: 973 retyped, 65 rebuilt, 169 skipped (purge/arg count
  disagree with the specs' call shape).
- FUN_005f1440 before: `__fastcall (int,int,int)`, 43 raw offsets, 14
  FUN_ calls. After: AITHINK_Fn013_FallenShaman(D2GameStrc*, D2UnitStrc*,
  D2AiTickParamStrc*), 0 raw offsets; reads the game's difficulty and
  seed fields, the tick record's target, and MonStats aip1..5 / skill1..2
  by name; 12 calls still FUN_.

## Open
- re-lookup's names.tsv / offsets.tsv not pushed yet: `types` picks up
  names.tsv automatically; offsets go into d2_114d_types.h (S each).
- Many interior gaps stay padding (unit, game, monster/player/object
  data); no struct yet for the data-tables global (sgptDataTables at
  0x00744304, +0xBCC monstats ...) — biggest remaining raw-offset source (M).
- 169 skipped signatures: review per function (M).
- Struct sizes not in specs: game (ends 0x1DF4), player data (0x16C).
