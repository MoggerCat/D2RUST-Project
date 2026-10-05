// Imports a community address list (tab-separated: Library, Name, Locator
// Type, Locator Value, Comments) as labels in the 1.14d Game.exe program.
//
// In 1.14d the old DLLs are merged into Game.exe, so each "Offset" is
// relative to Game.exe's image base. Labels are named <Module>_<Name>, e.g.
// D2Client_DifficultyLevel. If a function starts at the address, the
// function is renamed too. Rows with other locator types are skipped.
//
// Usage (headless): -postScript ImportCommunityLabels.java <list.txt>
//@category D2RS

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

public class ImportCommunityLabels extends GhidraScript {

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            printerr("usage: ImportCommunityLabels.java <list.txt>");
            return;
        }
        List<String> lines = Files.readAllLines(Path.of(args[0]), StandardCharsets.UTF_8);
        long base = currentProgram.getImageBase().getOffset();
        int labels = 0, functions = 0, skipped = 0;

        for (String line : lines.subList(1, lines.size())) { // skip header row
            String[] col = line.split("\t", -1);
            if (col.length < 4 || !col[2].trim().equalsIgnoreCase("Offset")) {
                if (!line.isBlank()) {
                    skipped++;
                }
                continue;
            }
            String module = col[0].trim().replaceFirst("(?i)\\.(dll|exe)$", "");
            String name = module + "_" + col[1].trim();
            long offset = Long.decode(col[3].trim());
            Address addr = toAddr(base + offset);
            if (!currentProgram.getMemory().contains(addr)) {
                printerr("outside Game.exe: " + name + " @ " + addr);
                skipped++;
                continue;
            }

            Function f = getFunctionAt(addr);
            if (f != null) {
                f.setName(name, SourceType.IMPORTED);
                functions++;
            } else {
                createLabel(addr, name, true, SourceType.IMPORTED);
            }
            labels++;

            String comment = (col.length > 4 ? col[4].trim() : "")
                    + (col.length > 5 && !col[5].isBlank() ? " | " + col[5].trim() : "");
            if (!comment.isBlank()) {
                setEOLComment(addr, comment);
            }
        }
        println(String.format("ImportCommunityLabels: %d labels (%d functions renamed), %d skipped",
                labels, functions, skipped));
    }
}
