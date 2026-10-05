// Exports decompiler output for spec-writing sessions. Output goes under re/
// (gitignored) and must never be copied into specs/ or crates/.
//
// Writes:
//   <outDir>/functions.tsv           entry, name, size, caller count, for every function
//   <outDir>/funcs/<entry>_<name>.c  decompiled C for each selected function
//
// Usage (headless): -postScript ExportDecompiled.java <outDir> [filter]
//   filter: omitted = all functions; otherwise a hex entry address
//   (e.g. 0x4A1B20) or a case-insensitive substring of the function name.
//@category D2RS

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.program.model.listing.Function;
import ghidra.app.script.GhidraScript;

import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

public class ExportDecompiled extends GhidraScript {

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            printerr("usage: ExportDecompiled.java <outDir> [filter]");
            return;
        }
        Path out = Path.of(args[0]);
        String filter = args.length > 1 ? args[1].toLowerCase() : null;
        Path funcsDir = out.resolve("funcs");
        Files.createDirectories(funcsDir);

        DecompInterface decomp = new DecompInterface();
        decomp.openProgram(currentProgram);
        int exported = 0, failed = 0;

        try (PrintWriter index = new PrintWriter(
                Files.newBufferedWriter(out.resolve("functions.tsv"), StandardCharsets.UTF_8))) {
            index.println("entry\tname\tsize\tcallers");
            for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
                if (monitor.isCancelled()) {
                    break;
                }
                String entry = "0x" + f.getEntryPoint().toString().toUpperCase();
                index.printf("%s\t%s\t%d\t%d%n", entry, f.getName(), f.getBody().getNumAddresses(),
                        f.getCallingFunctions(monitor).size());

                if (filter != null && !entry.toLowerCase().equals(filter)
                        && !f.getName().toLowerCase().contains(filter)) {
                    continue;
                }
                DecompileResults r = decomp.decompileFunction(f, 60, monitor);
                if (r == null || !r.decompileCompleted()) {
                    failed++;
                    continue;
                }
                String file = entry + "_" + f.getName().replaceAll("[^A-Za-z0-9_]", "_") + ".c";
                Files.writeString(funcsDir.resolve(file),
                        "// " + f.getName() + " @ " + entry + "\n" + r.getDecompiledFunction().getC(),
                        StandardCharsets.UTF_8);
                exported++;
            }
        } finally {
            decomp.dispose();
        }
        println(String.format("ExportDecompiled: %d exported, %d failed -> %s", exported, failed, out));
    }
}
