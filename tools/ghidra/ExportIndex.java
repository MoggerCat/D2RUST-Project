// Exports a greppable index of the analyzed program for spec-writing
// sessions: calls, data references, switch (computed-jump) tables, labels,
// strings and defined data. Output goes under re/ (gitignored) and must never
// be copied into specs/ or crates/.
//
// Writes (TSV with a header row, addresses as 0x%08X):
//   <outDir>/calls.tsv     site, from_fn, from_name, to_addr, to_fn, to_name
//   <outDir>/datarefs.tsv  site, from_fn, ref_type, to_addr, to_label, to_type, to_value
//   <outDir>/switches.tsv  site, from_fn, case_index, target
//   <outDir>/labels.tsv    addr, name, source, primary
//   <outDir>/strings.tsv   addr, length, value (escaped)
//   <outDir>/data.tsv      addr, length, type, label
//
// Usage (headless, -readOnly): -postScript ExportIndex.java <outDir>
//@category D2RS

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.symbol.RefType;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

public class ExportIndex extends GhidraScript {

    private static String hex(Address a) {
        return a == null ? "" : String.format("0x%08X", a.getOffset());
    }

    private static String esc(String s) {
        if (s == null) {
            return "";
        }
        StringBuilder b = new StringBuilder();
        for (char c : s.toCharArray()) {
            if (c == '\t') {
                b.append("\\t");
            } else if (c == '\n') {
                b.append("\\n");
            } else if (c == '\r') {
                b.append("\\r");
            } else if (c == '\\') {
                b.append("\\\\");
            } else if (c < 0x20 || c > 0x7E) {
                b.append(String.format("\\x%02X", (int) c & 0xFF));
            } else {
                b.append(c);
            }
        }
        return b.toString();
    }

    private PrintWriter open(Path dir, String name, String header) throws Exception {
        PrintWriter w = new PrintWriter(Files.newBufferedWriter(dir.resolve(name), StandardCharsets.UTF_8));
        w.println(header);
        return w;
    }

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            printerr("usage: ExportIndex.java <outDir>");
            return;
        }
        Path out = Path.of(args[0]);
        Files.createDirectories(out);
        FunctionManager fm = currentProgram.getFunctionManager();
        Listing listing = currentProgram.getListing();
        long nCalls = 0, nData = 0, nSw = 0;

        try (PrintWriter calls = open(out, "calls.tsv", "site\tfrom_fn\tfrom_name\tto_addr\tto_fn\tto_name");
             PrintWriter drefs = open(out, "datarefs.tsv", "site\tfrom_fn\tref_type\tto_addr\tto_label\tto_type\tto_value");
             PrintWriter sw = open(out, "switches.tsv", "site\tfrom_fn\tcase_index\ttarget")) {
            for (Function f : fm.getFunctions(true)) {
                if (monitor.isCancelled()) {
                    break;
                }
                String fe = hex(f.getEntryPoint());
                for (Instruction ins : listing.getInstructions(f.getBody(), true)) {
                    int caseIdx = 0;
                    for (Reference r : ins.getReferencesFrom()) {
                        RefType t = r.getReferenceType();
                        Address to = r.getToAddress();
                        if (t.isCall()) {
                            Function tf = fm.getFunctionAt(to);
                            calls.printf("%s\t%s\t%s\t%s\t%s\t%s%n", hex(ins.getAddress()), fe, f.getName(),
                                    hex(to), tf == null ? "" : hex(tf.getEntryPoint()), tf == null ? "" : tf.getName());
                            nCalls++;
                        } else if (t.isComputed() && t.isJump()) {
                            sw.printf("%s\t%s\t%d\t%s%n", hex(ins.getAddress()), fe, caseIdx++, hex(to));
                            nSw++;
                        } else if (t.isData() || t.isRead() || t.isWrite()) {
                            if (!to.isMemoryAddress()) {
                                continue;
                            }
                            Symbol s = getSymbolAt(to);
                            Data d = listing.getDataContaining(to);
                            String type = d == null ? "" : d.getDataType().getName();
                            String val = "";
                            if (d != null && d.isDefined() && d.getLength() <= 8 && d.getValue() != null) {
                                val = esc(d.getValue().toString());
                            }
                            drefs.printf("%s\t%s\t%s\t%s\t%s\t%s\t%s%n", hex(ins.getAddress()), fe, t.getName(),
                                    hex(to), s == null ? "" : esc(s.getName()), esc(type), val);
                            nData++;
                        }
                    }
                }
            }
        }

        long nLab = 0;
        try (PrintWriter lab = open(out, "labels.tsv", "addr\tname\tsource\tprimary")) {
            SymbolIterator it = currentProgram.getSymbolTable().getAllSymbols(true);
            while (it.hasNext() && !monitor.isCancelled()) {
                Symbol s = it.next();
                if (!s.getAddress().isMemoryAddress()) {
                    continue;
                }
                lab.printf("%s\t%s\t%s\t%s%n", hex(s.getAddress()), esc(s.getName(true)), s.getSource(), s.isPrimary());
                nLab++;
            }
        }

        long nStr = 0, nDef = 0;
        try (PrintWriter str = open(out, "strings.tsv", "addr\tlength\tvalue");
             PrintWriter dat = open(out, "data.tsv", "addr\tlength\ttype\tlabel")) {
            for (Data d : listing.getDefinedData(true)) {
                if (monitor.isCancelled()) {
                    break;
                }
                Symbol s = getSymbolAt(d.getAddress());
                dat.printf("%s\t%d\t%s\t%s%n", hex(d.getAddress()), d.getLength(), esc(d.getDataType().getName()),
                        s == null ? "" : esc(s.getName()));
                nDef++;
                if (d.hasStringValue()) {
                    Object v = d.getValue();
                    str.printf("%s\t%d\t%s%n", hex(d.getAddress()), d.getLength(), esc(v == null ? "" : v.toString()));
                    nStr++;
                }
            }
        }
        println(String.format("ExportIndex: %d calls, %d data refs, %d switch targets, %d labels, %d data, %d strings -> %s",
                nCalls, nData, nSw, nLab, nDef, nStr, out));
    }
}
