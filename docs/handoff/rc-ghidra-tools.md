# rc-ghidra-tools hand-back

Tooling task, no checks affected (EQUAL counts unchanged).

Added (tools/ghidra/, branch claude/rc-ghidra-tools, REC-2500):
- lookup.py: name / address (containing function) / --callers / --callees / --str; one line per hit.
- site2fn.py: address -> function, callers to depth 2, specs citing the entry or address.
- decomp1.sh: one function via cloud_setup.sh decompile, output in a temp dir.
- README section; all `--selftest` pass without private files.

Spot check on 0x5a55ba: FUN_005a5560 +0x5a, callers resolved.
Not run: decomp1.sh against Ghidra (needs the ~15 min project build); it only wraps cloud_setup.sh decompile.
Open: most names are still FUN_*; names.tsv overrides apply automatically as it grows.
