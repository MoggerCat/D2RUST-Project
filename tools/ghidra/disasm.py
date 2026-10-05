"""Disassembly and cross-references for the 1.14d Game.exe (spec-writing
sessions only; output stays in the terminal or under re/).

The Ghidra decompile export drops register arguments (fastcall ECX/EDX,
custom conventions), so specs read register use from the disassembly.

  py tools/ghidra/disasm.py fn 0x52D870            disassemble one function
                                                    (size from re/exports/functions.tsv)
  py tools/ghidra/disasm.py at 0x6CAF10 0x20        disassemble N bytes at an address
  py tools/ghidra/disasm.py xref 0x52D870 ...       rel32 calls/jumps to, and 4-byte
                                                    pointers to, each address
  py tools/ghidra/disasm.py dump re/exports/all.asm whole binary, one line per
                                                    instruction, "@<function>" suffix
  py tools/ghidra/disasm.py selftest                checks itself (METHODS M10)

Needs Python 3.10 with capstone and pefile (`py -m pip install capstone
pefile`). Our own code.
"""

import os
import struct
import sys

import capstone
import pefile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
EXE = os.path.join(REPO, "game", "Game.exe")
TSV = os.path.join(REPO, "re", "exports", "functions.tsv")


class Image:
    def __init__(self, exe=EXE, tsv=TSV):
        self.pe = pefile.PE(exe, fast_load=True)
        self.base = self.pe.OPTIONAL_HEADER.ImageBase
        self.img = self.pe.get_memory_mapped_image()
        self.funcs = {}
        if os.path.exists(tsv):
            with open(tsv, encoding="utf-8") as f:
                for line in f.read().splitlines()[1:]:
                    a, n, s, c = line.split("\t")
                    self.funcs[int(a, 16)] = (n, int(s), int(c))
        self.md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        text = next(s for s in self.pe.sections if s.Name.startswith(b".text"))
        self.text = (text.VirtualAddress, text.VirtualAddress + text.Misc_VirtualSize)

    def lines(self, addr, size, tag=""):
        code = self.img[addr - self.base:addr - self.base + size]
        for i in self.md.disasm(code, addr):
            extra = ""
            if i.mnemonic in ("call", "jmp") and i.op_str.startswith("0x"):
                t = int(i.op_str, 16)
                if t in self.funcs:
                    extra = "  ; " + self.funcs[t][0]
            yield f"{i.address:08X}: {i.mnemonic} {i.op_str}{extra}{tag}"

    def function(self, addr):
        return self.lines(addr, self.funcs.get(addr, ("", 0x200, 0))[1])

    def xref(self, target):
        out = []
        lo, hi = self.text
        img = self.img
        for off in range(lo, hi - 5):
            if img[off] in (0xE8, 0xE9):
                rel = struct.unpack_from("<i", img, off + 1)[0]
                if off + 5 + rel + self.base == target:
                    out.append(f"{off + self.base:#x}" + ("c" if img[off] == 0xE8 else "j"))
        pat = struct.pack("<I", target)
        i = img.find(pat)
        while i != -1:
            out.append(f"ptr@{i + self.base:#x}")
            i = img.find(pat, i + 1)
        return out


def selftest():
    im = Image()
    tick = list(im.function(0x52D870))
    ok = tick[3].startswith("0052D873: mov edi, ecx") and \
        any("add dword ptr [edi + 0xa8], 1" in line for line in tick)
    refs = im.xref(0x52D870)
    ok = ok and "0x52fd19c" in refs  # the tick driver's call (specs/sim/tick.md §1)
    print("selftest:", "pass" if ok else "FAIL")
    return ok


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    cmd, args = sys.argv[1], sys.argv[2:]
    if cmd == "selftest":
        sys.exit(0 if selftest() else 1)
    im = Image()
    if cmd == "fn":
        for a in args:
            print("\n".join(im.function(int(a, 16))))
    elif cmd == "at":
        print("\n".join(im.lines(int(args[0], 16), int(args[1], 16) if len(args) > 1 else 0x40)))
    elif cmd == "xref":
        for a in args:
            print(a, " ".join(im.xref(int(a, 16))))
    elif cmd == "dump":
        with open(args[0], "w", encoding="utf-8", newline="\n") as f:
            for a in sorted(im.funcs):
                n, s, c = im.funcs[a]
                f.write(f"=== {a:08X} {n} size={s} callers={c}\n")
                for line in im.lines(a, s, f"\t@{a:08X}"):
                    f.write(line + "\n")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
