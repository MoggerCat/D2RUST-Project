# Ghidra tooling

Our own scripts for analyzing the 1.14d `Game.exe`. Everything they produce
goes under `re/` (gitignored); `re/exports/` is also pushed, read-only, to
the private repo's `re/exports/` so cloud sessions can read it (CLAUDE.md
rule 3). Never into the public repo.

- Ghidra 12.1.4 in `%LOCALAPPDATA%\Programs\ghidra_12.1.4_PUBLIC`
- JDK: Temurin 21, pinned through `JAVA_HOME_OVERRIDE` in Ghidra's
  `support\launch.properties`. Ghidra 12.1.x fails on JDK 25: its plugin
  framework errors with "The data file must be inside the data dir".
- Project: `re/ghidra/D2_114d.gpr`, holding `Game.exe`, auto-analyzed, with
  the community labels from `../refs/1.14d-notes` applied

Run from the repo root. Relative paths avoid problems with the spaces in
the full path.

```
$G = "$env:LOCALAPPDATA\Programs\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat"

# One-time: import + analyze + apply community labels
& $G re\ghidra D2_114d -import game\Game.exe -scriptPath tools\ghidra `
    -postScript ImportCommunityLabels.java "..\refs\1.14d-notes\LoD 1.14D.txt"

# Export decompiled C: all functions, or a filter (hex address or name part)
& $G re\ghidra D2_114d -process Game.exe -noanalysis -readOnly -scriptPath tools\ghidra `
    -postScript ExportDecompiled.java re\exports
& $G re\ghidra D2_114d -process Game.exe -noanalysis -readOnly -scriptPath tools\ghidra `
    -postScript ExportDecompiled.java re\exports\rng Rand
```

`re/exports/functions.tsv` lists every function (entry, name, size, caller
count) and is the starting point for finding things. Open the project in
the GUI with `ghidraRun.bat` to rename functions and define structs as you
learn them. Later exports pick up those names.

## Disassembly and cross-references

The decompile export drops register arguments, so read register use from
the disassembly: `py tools/ghidra/disasm.py fn|at|xref|dump|selftest`
(capstone + pefile; usage in the script's docstring). `dump
re/exports/all.asm` writes the whole binary as one greppable file
(~7 s, ~890k lines), the fastest way to find every reader and writer of
a global or a struct offset.

## Index export (calls, data refs, switch tables, labels, strings)

```
& $G re\ghidra D2_114d -process Game.exe -noanalysis -readOnly -scriptPath tools\ghidra `
    -postScript ExportIndex.java re\exports\index
```

Writes `re/exports/index/`: `calls.tsv` (every call site with caller and
callee), `datarefs.tsv` (every data read / write / pointer from code with
the target's label, type and small value), `switches.tsv` (computed-jump
case targets per site), `labels.tsv`, `strings.tsv` and `data.tsv`
(every defined data item). About 2 minutes; grep these instead of opening
Ghidra.

`switches.tsv` `case_index` is the order of the computed-jump references on the instruction, not the case value: read the jump table itself (or `disasm.py`) for the case number (a close-hook table was found one case off this way).

## Cloud sessions (Linux, headless)

`sh tools/ghidra/cloud_setup.sh install` puts Ghidra 12.1.4 in
`$HOME/ghidra` (the GitHub release is out of the session's GitHub scope;
it takes the official distribution from the Docker Hub image
`blacktop/ghidra:12.1.4` through the registry API). `project` imports the
private repo's `install/Game.exe`, auto-analyzes (~10 min) and runs
`types`: ImportCommunityLabels, ApplyNames (`re/exports/functions.tsv`,
then names harvested from specs/ by `spec_harvest.py`) and ApplyTypes
(`d2_114d_types.h`, our structs written from specs/, plus argument types
from the specs' call shapes). Then `decompile 0xADDR` prints one function
with current names and types; `export DIR` re-exports everything (~3 min;
the private repo's `re/exports-typed/` was made this way). Add struct
fields to `d2_114d_types.h` as specs measure them: every field carries
its `// +0xNN` offset and every struct its `// size`, and ApplyTypes
rejects a header whose layout disagrees.

## Finding a function fast (cloud sessions)

Read-only helpers over the private repo's `re/` (`$D2_RE`, else
`../d2rust-private-repo/re`; typed export preferred, `exports/names.tsv`
overrides names). They print addresses and names only; each has
`--selftest` (no private files needed). Nothing is written into the repo.

```
python3 tools/ghidra/site2fn.py 0x5a55ba      # rng draw site / first-diff address ->
                                              # function, callers (depth 2), citing specs
python3 tools/ghidra/lookup.py FireClaws      # name part -> functions
python3 tools/ghidra/lookup.py 0x5a55ba       # containing function + callers + callees
python3 tools/ghidra/lookup.py --callers <name|addr>   # also --callees
python3 tools/ghidra/lookup.py --str "text"   # strings and the functions using them
sh tools/ghidra/decomp1.sh 0x5a5560           # decompile one function to a temp
                                              # dir (needs cloud_setup.sh install+project)
```
