// Defines the 1.14d structs from tools/ghidra/d2_114d_types.h in the
// program's data type manager (category /D2), then sets parameter types on
// the functions listed in a signatures TSV, so the decompiler propagates
// them.
//
// The header is read by a small parser of its own strict format (see the
// header's top comment), not by Ghidra's C parser: every field carries its
// offset (`// +0xNN`) and every struct its size (`// size 0xNN`), and the
// structs are laid out exactly at those offsets (no alignment padding). A
// field whose offset disagrees with the running sum of sizes is an error.
//
// Signatures TSV (tools/ghidra/spec_harvest.py spec-signatures.tsv):
// entry, nargs, votes, types (comma-separated, '-' = leave as is). When the
// function already has nargs parameters only the named positions are
// retyped. Otherwise the parameter list is rebuilt with nargs parameters
// when the function's stack purge matches nargs under its calling
// convention (__fastcall: ECX, EDX then stack; __thiscall: ECX then stack;
// __stdcall: all on the stack); anything else is skipped and counted.
//
// Usage (headless): -postScript ApplyTypes.java <types.h> [<signatures.tsv>]
//@category D2RS

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.pcode.HighFunctionDBUtil;
import ghidra.program.model.pcode.HighFunctionDBUtil.ReturnCommitOption;
import ghidra.program.model.data.*;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Function.FunctionUpdateType;
import ghidra.program.model.listing.Parameter;
import ghidra.program.model.listing.ParameterImpl;
import ghidra.program.model.symbol.SourceType;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class ApplyTypes extends GhidraScript {

    private static final CategoryPath CAT = new CategoryPath("/D2");
    private static final Pattern OPEN = Pattern.compile(
            "^\\s*(struct|union)\\s+(\\w+)\\s*\\{\\s*(?://.*?size\\s+(0x[0-9A-Fa-f]+))?.*$");
    private static final Pattern FIELD = Pattern.compile(
            "^\\s*(?:(?:struct|union)\\s+)?(\\w+)\\s*(\\**)\\s*(\\w+)\\s*(?:\\[\\s*(0x[0-9A-Fa-f]+|\\d+)\\s*\\])?\\s*;"
                    + "\\s*(?://\\s*(?:\\+(0x[0-9A-Fa-f]+))?\\s*(.*))?$");

    private DataTypeManager dtm;
    private final Map<String, Composite> composites = new LinkedHashMap<>();

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            printerr("usage: ApplyTypes.java <types.h> [<signatures.tsv>]");
            return;
        }
        dtm = currentProgram.getDataTypeManager();
        int tx = dtm.startTransaction("D2 types");
        try {
            defineTypes(Files.readAllLines(Path.of(args[0]), StandardCharsets.UTF_8));
        } finally {
            dtm.endTransaction(tx, true);
        }
        if (args.length > 1) {
            applySignatures(Files.readAllLines(Path.of(args[1]), StandardCharsets.UTF_8));
        }
    }

    private static long num(String s) {
        return s.startsWith("0x") ? Long.parseLong(s.substring(2), 16) : Long.parseLong(s);
    }

    private void defineTypes(List<String> lines) throws Exception {
        // Pass 1: create every struct / union with its declared size.
        for (String line : lines) {
            Matcher m = OPEN.matcher(line);
            if (!m.matches()) {
                continue;
            }
            String name = m.group(2);
            Composite c;
            if (m.group(1).equals("struct")) {
                int size = m.group(3) != null ? (int) num(m.group(3)) : 0;
                c = new StructureDataType(CAT, name, size, dtm);
            } else {
                c = new UnionDataType(CAT, name, dtm);
            }
            c = (Composite) dtm.addDataType(c, DataTypeConflictHandler.REPLACE_HANDLER);
            if (c instanceof Structure s) {
                s.deleteAll();
                if (m.group(3) != null) {
                    s.growStructure((int) num(m.group(3)));
                }
            }
            composites.put(name, c);
        }
        // Pass 2: fields.
        Composite cur = null;
        String curName = null;
        int pos = 0, errors = 0, fields = 0;
        for (String line : lines) {
            Matcher open = OPEN.matcher(line);
            if (open.matches()) {
                curName = open.group(2);
                cur = composites.get(curName);
                pos = 0;
                continue;
            }
            if (cur == null) {
                continue;
            }
            if (line.trim().startsWith("}")) {
                if (cur instanceof Structure s && s.getLength() != pos && pos != 0) {
                    if (s.getLength() < pos) {
                        printerr(curName + ": fields end at 0x" + Integer.toHexString(pos)
                                + " past declared size 0x" + Integer.toHexString(s.getLength()));
                        errors++;
                    }
                }
                cur = null;
                continue;
            }
            Matcher f = FIELD.matcher(line);
            if (!f.matches()) {
                continue;
            }
            DataType dt = resolve(f.group(1), f.group(2).length());
            if (dt == null) {
                printerr(curName + "." + f.group(3) + ": unknown type " + f.group(1));
                errors++;
                continue;
            }
            int count = f.group(4) != null ? (int) num(f.group(4)) : 1;
            if (count > 1 || f.group(4) != null) {
                dt = new ArrayDataType(dt, count, dt.getLength(), dtm);
            }
            String fieldName = f.group(3);
            String comment = f.group(6) == null || f.group(6).isBlank() ? null : f.group(6).trim();
            if (cur instanceof Union u) {
                u.add(dt, fieldName, comment);
                fields++;
                continue;
            }
            Structure s = (Structure) cur;
            if (f.group(5) != null && num(f.group(5)) != pos) {
                printerr(String.format("%s.%s: comment says +0x%X, layout gives +0x%X",
                        curName, fieldName, num(f.group(5)), pos));
                errors++;
                pos = (int) num(f.group(5));
            }
            if (pos + dt.getLength() > s.getLength()) {
                s.growStructure(pos + dt.getLength() - s.getLength());
            }
            if (!fieldName.startsWith("_pad")) {
                s.replaceAtOffset(pos, dt, dt.getLength(), fieldName, comment);
                fields++;
            }
            pos += dt.getLength();
        }
        println(String.format("ApplyTypes: %d composites, %d fields, %d layout errors",
                composites.size(), fields, errors));
        if (errors > 0) {
            throw new IllegalStateException("header layout errors: " + errors);
        }
    }

    private DataType resolve(String base, int ptr) {
        DataType dt = switch (base) {
            case "u8" -> ByteDataType.dataType;
            case "i8", "char" -> SignedByteDataType.dataType;
            case "u16" -> WordDataType.dataType;
            case "i16", "short" -> ShortDataType.dataType;
            case "u32" -> DWordDataType.dataType;
            case "i32", "int", "long" -> IntegerDataType.dataType;
            case "void" -> VoidDataType.dataType;
            default -> composites.get(base);
        };
        if (dt == null) {
            return null;
        }
        for (int i = 0; i < ptr; i++) {
            dt = new PointerDataType(dt, 4, dtm);
        }
        return dt;
    }

    private DataType resolveSig(String t) {
        t = t.trim();
        int ptr = 0;
        while (t.endsWith("*")) {
            ptr++;
            t = t.substring(0, t.length() - 1).trim();
        }
        return resolve(t, ptr);
    }

    /** pUnit, pGame, ... for a typed argument; pUnit2 for a second unit. */
    private static String argName(String type, int index, String[] types) {
        if (!type.startsWith("D2") || !type.endsWith("Strc*")) {
            return null;
        }
        String base = "p" + type.substring(2, type.length() - 5);
        int n = 1;
        for (int i = 0; i < index; i++) {
            if (types[i].equals(type)) {
                n++;
            }
        }
        return n == 1 ? base : base + n;
    }

    private void applySignatures(List<String> lines) throws Exception {
        int retyped = 0, rebuilt = 0, skipped = 0, missing = 0;
        DecompInterface decomp = new DecompInterface();
        decomp.openProgram(currentProgram);
        try {
            for (String line : lines) {
                try {
                    String[] col = line.split("\t", -1);
                    if (col.length < 4 || !col[0].startsWith("0x")) {
                        continue;
                    }
                    Function f = getFunctionAt(toAddr(Long.decode(col[0])));
                    if (f == null) {
                        missing++;
                        continue;
                    }
                    int nargs = Integer.parseInt(col[1]);
                    String[] types = col[3].split(",");
                    if (f.getSignatureSource() == SourceType.DEFAULT) {
                        // The database has no parameters yet: commit the decompiler's
                        // inferred ones (convention and storage) first.
                        DecompileResults r = decomp.decompileFunction(f, 60, monitor);
                        if (r != null && r.decompileCompleted()) {
                            HighFunctionDBUtil.commitParamsToDatabase(r.getHighFunction(), false,
                                    ReturnCommitOption.NO_COMMIT, SourceType.ANALYSIS);
                        }
                    }
                    Parameter[] params = f.getParameters();
                    if (params.length == nargs) {
                        for (int i = 0; i < nargs; i++) {
                            DataType dt = types[i].equals("-") ? null : resolveSig(types[i]);
                            if (dt != null && params[i].getLength() == dt.getLength()
                                    && !params[i].isAutoParameter()) {
                                params[i].setDataType(dt, SourceType.USER_DEFINED);
                                String name = argName(types[i], i, types);
                                if (params[i].getName().startsWith("param_") && name != null) {
                                    params[i].setName(name, SourceType.USER_DEFINED);
                                }
                            }
                        }
                        retyped++;
                        continue;
                    }
                    String cc = f.getCallingConventionName();
                    int purge = f.getStackPurgeSize();
                    // Register arguments: from the convention when it is known,
                    // else from the purge (a callee-cleaned function with nargs
                    // arguments and purge 4k passes nargs - k in registers).
                    int regs = switch (cc) {
                        case "__fastcall" -> 2;
                        case "__thiscall" -> 1;
                        case "__stdcall" -> 0;
                        default -> purge > 0 ? nargs - purge / 4 : -1;
                    };
                    if (regs < 0 || regs > 2 || nargs < regs || purge != 4 * (nargs - regs)) {
                        skipped++;
                        continue;
                    }
                    String newCc = regs == 2 ? "__fastcall" : regs == 1 ? "__thiscall" : "__stdcall";
                    List<Parameter> list = new ArrayList<>();
                    // __thiscall: the ECX argument is Ghidra's automatic 'this'.
                    for (int i = regs == 1 ? 1 : 0; i < nargs; i++) {
                        DataType dt = types[i].equals("-") ? null : resolveSig(types[i]);
                        if (dt == null) {
                            dt = i < params.length && params[i].getLength() == 4
                                    ? params[i].getDataType() : Undefined4DataType.dataType;
                        }
                        String name = i < params.length ? params[i].getName() : "param_" + (i + 1);
                        if (name.startsWith("param_") && argName(types[i], i, types) != null) {
                            name = argName(types[i], i, types);
                        }
                        list.add(new ParameterImpl(name, dt, currentProgram));
                    }
                    try {
                        f.updateFunction(newCc, f.getReturn(), list,
                                FunctionUpdateType.DYNAMIC_STORAGE_ALL_PARAMS, true, SourceType.USER_DEFINED);
                        rebuilt++;
                    } catch (Exception e) {
                        printerr("signature " + f.getEntryPoint() + ": " + e.getMessage());
                        skipped++;
                    }
                } catch (Exception e) {
                    printerr("signature " + line.split("\t")[0] + ": " + e.getMessage());
                    skipped++;
                }
            }
        } finally {
            decomp.dispose();
        }
        println(String.format("ApplyTypes signatures: %d retyped, %d rebuilt, %d skipped, "
                + "%d not a function", retyped, rebuilt, skipped, missing));
    }
}
