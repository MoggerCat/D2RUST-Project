# Ghidra tooling (spec-writing sessions only)

Our own scripts for analyzing the 1.14d `Game.exe`. Everything they produce
goes under `re/` (gitignored). Implementation sessions don't use any of
this (see `docs/CLEAN_ROOM.md`).

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
