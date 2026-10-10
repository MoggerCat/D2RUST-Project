// Applies function names from one or more TSV files (first two columns:
// entry address, name; a header row is skipped) to the 1.14d Game.exe
// program. Later files win over earlier ones, but a community label
// (SourceType.IMPORTED, from ImportCommunityLabels.java) is never replaced.
// A missing function is created at the entry first (also for rows with a
// default name, so the function set matches the list). Default names
// (FUN_, thunk_FUN_, LAB_, caseD_, Unwind@, switchD_) are not applied.
//
// Usage (headless): -postScript ApplyNames.java <names.tsv> [<names.tsv> ...]
// e.g. re/exports/functions.tsv (PC 1's renames), then the output of
// tools/ghidra/spec_harvest.py (spec-names.tsv).
//@category D2RS

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

public class ApplyNames extends GhidraScript {

    private static final String DEFAULT_NAME =
            "^(FUN_|thunk_FUN_|LAB_|caseD_|Unwind@|switchD_|entry$).*";

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            printerr("usage: ApplyNames.java <names.tsv> [<names.tsv> ...]");
            return;
        }
        for (String file : args) {
            List<String> lines = Files.readAllLines(Path.of(file), StandardCharsets.UTF_8);
            int renamed = 0, created = 0, kept = 0, failed = 0;
            for (String line : lines) {
                String[] col = line.split("\t", -1);
                if (col.length < 2 || !col[0].startsWith("0x")) {
                    continue; // header or blank
                }
                String name = col[1].trim().replace(' ', '_');
                Address addr = toAddr(Long.decode(col[0].trim()));
                Function f = getFunctionAt(addr);
                if (f == null) {
                    f = createFunction(addr, null);
                    if (f == null) {
                        failed++;
                        continue;
                    }
                    created++;
                }
                if (name.isEmpty() || name.matches(DEFAULT_NAME) || f.getName().equals(name)) {
                    continue;
                }
                if (f.getSymbol().getSource() == SourceType.IMPORTED) {
                    kept++;
                    continue;
                }
                try {
                    f.setName(name, SourceType.USER_DEFINED);
                    renamed++;
                } catch (Exception e) {
                    printerr("rename " + addr + " -> " + name + ": " + e.getMessage());
                    failed++;
                }
            }
            println(String.format("ApplyNames %s: %d renamed, %d functions created, "
                    + "%d community names kept, %d failed", file, renamed, created, kept, failed));
        }
    }
}
