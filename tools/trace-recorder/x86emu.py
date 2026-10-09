"""A tiny 32-bit x86 interpreter for the debugger recorders: runs a few
instructions of the stopped game thread in Python instead of
single-stepping them (each single step is two debug events under the
Win32 debug API, the recorders' cost).

    python3 x86emu.py --selftest

Scope: the integer instructions found around the 1.14d RNG steps and
at the recorders' breakpoints (specs/tools/rng-trace.md §4 r7): mov
(32-bit and byte), movzx, lea, push, pop, the eight ALU ops (reg, r/m,
imm), inc, dec, test, mul, jcc, jmp, nop. 32-bit operands and 32-bit
addressing only; any prefix, any other opcode, a call or a return
raises Unsupported, and the caller falls back to single-stepping. Status flags are exact: a flag an
instruction leaves undefined (mul: SF ZF AF PF; logic ops: AF) is
tracked, a jcc that reads one raises Unsupported, and a result whose
final flags still hold an undefined one raises Unsupported at commit.

Memory writes are buffered (`Cpu.writes`) and reads see them; nothing
reaches the process until the caller commits the whole run.

Standard library only; our own code.
"""

import struct
import sys

M32 = 0xFFFFFFFF
CF, PF, AF, ZF, SF, OF = 0x1, 0x4, 0x10, 0x40, 0x80, 0x800
STATUS = CF | PF | AF | ZF | SF | OF
REG_NAMES = ("Eax", "Ecx", "Edx", "Ebx", "Esp", "Ebp", "Esi", "Edi")
EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI = range(8)


class Unsupported(Exception):
    pass


def _parity(v):
    return PF if bin(v & 0xFF).count("1") % 2 == 0 else 0


def _szp(res):
    return (SF if res & 0x80000000 else 0) | (ZF if res == 0 else 0) | _parity(res)


class Insn:
    __slots__ = ("addr", "len", "op", "a", "b", "c")

    def __init__(self, addr, length, op, a=None, b=None, c=None):
        self.addr, self.len, self.op, self.a, self.b, self.c = addr, length, op, a, b, c


ALU = ("add", "or", "adc", "sbb", "and", "sub", "xor", "cmp")


def decode(addr, code):
    """Decode the instruction at `addr` from `code` (its bytes, at least 15
    or up to the end of readable code). Operands: ("r", i) register,
    ("m", base, index, scale, disp) memory, ("i", value) immediate."""
    if not code:
        raise Unsupported("no code")
    op = code[0]

    def need(n):
        if len(code) < n:
            raise Unsupported("truncated")

    def modrm(at):
        need(at + 1)
        m = code[at]
        mod, reg, rm = m >> 6, (m >> 3) & 7, m & 7
        i = at + 1
        if mod == 3:
            return reg, ("r", rm), i
        base, index, scale, disp = rm, None, 1, 0
        if rm == 4:
            need(i + 1)
            sib = code[i]
            i += 1
            scale, idx, base = 1 << (sib >> 6), (sib >> 3) & 7, sib & 7
            index = None if idx == 4 else idx
            if base == 5 and mod == 0:
                base = None
                need(i + 4)
                disp = struct.unpack_from("<i", code, i)[0]
                i += 4
        elif rm == 5 and mod == 0:
            base = None
            need(i + 4)
            disp = struct.unpack_from("<i", code, i)[0]
            i += 4
        if mod == 1:
            need(i + 1)
            disp = struct.unpack_from("<b", code, i)[0]
            i += 1
        elif mod == 2:
            need(i + 4)
            disp = struct.unpack_from("<i", code, i)[0]
            i += 4
        return reg, ("m", base, index, scale, disp), i

    def imm32(at):
        need(at + 4)
        return struct.unpack_from("<I", code, at)[0]

    def imm8s(at):
        need(at + 1)
        return struct.unpack_from("<b", code, at)[0] & M32

    if op < 0x40 and op & 7 in (1, 3, 5):
        kind = ALU[op >> 3]
        if op & 7 == 5:
            return Insn(addr, 5, kind, ("r", EAX), ("i", imm32(1)))
        reg, rm, n = modrm(1)
        if op & 7 == 1:
            return Insn(addr, n, kind, rm, ("r", reg))
        return Insn(addr, n, kind, ("r", reg), rm)
    if 0x40 <= op <= 0x4F:
        return Insn(addr, 1, "inc" if op < 0x48 else "dec", ("r", op & 7))
    if 0x50 <= op <= 0x57:
        return Insn(addr, 1, "push", ("r", op & 7))
    if 0x58 <= op <= 0x5F:
        if op & 7 == ESP:
            raise Unsupported("pop esp")
        return Insn(addr, 1, "pop", ("r", op & 7))
    if 0x70 <= op <= 0x7F:
        return Insn(addr, 2, "jcc", op & 0xF, imm8s(1))
    if op in (0x81, 0x83):
        reg, rm, n = modrm(1)
        if op == 0x81:
            return Insn(addr, n + 4, ALU[reg], rm, ("i", imm32(n)))
        return Insn(addr, n + 1, ALU[reg], rm, ("i", imm8s(n)))
    if op == 0x85:
        reg, rm, n = modrm(1)
        return Insn(addr, n, "test", rm, ("r", reg))
    if op in (0x88, 0x8A):
        reg, rm, n = modrm(1)
        if op == 0x88:
            return Insn(addr, n, "mov8", rm, ("r", reg))
        return Insn(addr, n, "mov8", ("r", reg), rm)
    if op == 0x89:
        reg, rm, n = modrm(1)
        return Insn(addr, n, "mov", rm, ("r", reg))
    if op == 0x8B:
        reg, rm, n = modrm(1)
        return Insn(addr, n, "mov", ("r", reg), rm)
    if op == 0x8D:
        reg, rm, n = modrm(1)
        if rm[0] != "m":
            raise Unsupported("lea reg")
        return Insn(addr, n, "lea", ("r", reg), rm)
    if op == 0x90:
        return Insn(addr, 1, "nop")
    if op == 0xA9:
        return Insn(addr, 5, "test", ("r", EAX), ("i", imm32(1)))
    if 0xB8 <= op <= 0xBF:
        return Insn(addr, 5, "mov", ("r", op & 7), ("i", imm32(1)))
    if op == 0xC7:
        reg, rm, n = modrm(1)
        if reg != 0:
            raise Unsupported("C7 /%d" % reg)
        return Insn(addr, n + 4, "mov", rm, ("i", imm32(n)))
    if op == 0xEB:
        return Insn(addr, 2, "jmp", imm8s(1))
    if op == 0xE9:
        return Insn(addr, 5, "jmp", imm32(1))
    if op == 0xF7:
        reg, rm, n = modrm(1)
        if reg == 0:
            return Insn(addr, n + 4, "test", rm, ("i", imm32(n)))
        if reg == 4:
            return Insn(addr, n, "mul", rm)
        raise Unsupported("F7 /%d" % reg)
    if op == 0x0F:
        need(2)
        op2 = code[1]
        if 0x80 <= op2 <= 0x8F:
            need(6)
            return Insn(addr, 6, "jcc", op2 & 0xF, struct.unpack_from("<I", code, 2)[0])
        if op2 in (0xB6, 0xB7):
            reg, rm, n = modrm(2)
            return Insn(addr, n, "movzx8" if op2 == 0xB6 else "movzx16", ("r", reg), rm)
    raise Unsupported("opcode %02X at %#x" % (op, addr))


class Cpu:
    """Registers (list of 8, x86 order), eip, eflags; `read(addr, n)` reads
    process memory. Writes go to `writes` (addr -> byte)."""

    def __init__(self, regs, eip, eflags, read):
        self.r = [v & M32 for v in regs]
        self.eip = eip & M32
        self.fl = eflags & M32
        self.undef = 0
        self.read = read
        self.writes = {}

    # memory
    def rd(self, addr, n):
        b = bytearray(self.read(addr & M32, n))
        for k in range(n):
            a = (addr + k) & M32
            if a in self.writes:
                b[k] = self.writes[a]
        return bytes(b)

    def wr(self, addr, data):
        for k, v in enumerate(data):
            self.writes[(addr + k) & M32] = v

    def ea(self, m):
        _, base, index, scale, disp = m
        a = disp
        if base is not None:
            a += self.r[base]
        if index is not None:
            a += self.r[index] * scale
        return a & M32

    def get(self, o):
        if o[0] == "r":
            return self.r[o[1]]
        if o[0] == "i":
            return o[1] & M32
        return struct.unpack("<I", self.rd(self.ea(o), 4))[0]

    def put(self, o, v):
        v &= M32
        if o[0] == "r":
            self.r[o[1]] = v
        elif o[0] == "m":
            self.wr(self.ea(o), struct.pack("<I", v))
        else:
            raise Unsupported("store to an immediate")

    def get8(self, o):
        """Byte operand: registers 0-3 AL CL DL BL, 4-7 AH CH DH BH."""
        if o[0] == "r":
            return self.r[o[1] & 3] >> (8 * (o[1] >> 2)) & 0xFF
        return self.rd(self.ea(o), 1)[0]

    def put8(self, o, v):
        if o[0] == "r":
            sh = 8 * (o[1] >> 2)
            self.r[o[1] & 3] = (self.r[o[1] & 3] & ~(0xFF << sh) & M32) | ((v & 0xFF) << sh)
        else:
            self.wr(self.ea(o), bytes([v & 0xFF]))

    # flags
    def setflags(self, bits, defined, undefined=0):
        self.fl = (self.fl & ~defined & M32) | (bits & defined)
        self.undef = (self.undef & ~defined) | undefined
        self.fl &= ~undefined & M32

    def flag(self, f):
        if self.undef & f:
            raise Unsupported("jcc reads an undefined flag")
        return 1 if self.fl & f else 0

    def cond(self, cc):
        c = cc >> 1
        if c == 0:
            v = self.flag(OF)
        elif c == 1:
            v = self.flag(CF)
        elif c == 2:
            v = self.flag(ZF)
        elif c == 3:
            v = self.flag(CF) | self.flag(ZF)
        elif c == 4:
            v = self.flag(SF)
        elif c == 5:
            v = self.flag(PF)
        elif c == 6:
            v = self.flag(SF) ^ self.flag(OF)
        else:
            v = (self.flag(SF) ^ self.flag(OF)) | self.flag(ZF)
        return bool(v) != bool(cc & 1)

    def alu(self, kind, a, b):
        """Result (None for cmp/test) and flags of a 32-bit ALU op."""
        if kind in ("add", "adc"):
            c = self.flag(CF) if kind == "adc" else 0
            s = a + b + c
            res = s & M32
            f = (CF if s > M32 else 0) | (OF if (a ^ res) & (b ^ res) & 0x80000000 else 0)
            f |= ((a ^ b ^ res) & AF) | _szp(res)
            self.setflags(f, STATUS)
            return res
        if kind in ("sub", "sbb", "cmp"):
            c = self.flag(CF) if kind == "sbb" else 0
            res = (a - b - c) & M32
            f = (CF if a < b + c else 0) | (OF if (a ^ b) & (a ^ res) & 0x80000000 else 0)
            f |= ((a ^ b ^ res) & AF) | _szp(res)
            self.setflags(f, STATUS)
            return None if kind == "cmp" else res
        if kind in ("and", "test"):
            res = a & b
        elif kind == "or":
            res = a | b
        elif kind == "xor":
            res = a ^ b
        else:
            raise Unsupported(kind)
        self.setflags(_szp(res), STATUS, undefined=AF)
        return None if kind == "test" else res

    def execute(self, i):
        """Run one decoded instruction (eip must be i.addr)."""
        nxt = (i.addr + i.len) & M32
        op = i.op
        if op in ALU or op == "test":
            res = self.alu(op, self.get(i.a), self.get(i.b))
            if res is not None:
                self.put(i.a, res)
        elif op == "mov":
            self.put(i.a, self.get(i.b))
        elif op == "mov8":
            self.put8(i.a, self.get8(i.b))
        elif op == "movzx8":
            self.put(i.a, self.get8(i.b))
        elif op == "movzx16":
            v = self.r[i.b[1]] & 0xFFFF if i.b[0] == "r" \
                else struct.unpack("<H", self.rd(self.ea(i.b), 2))[0]
            self.put(i.a, v)
        elif op == "lea":
            self.put(i.a, self.ea(i.b))
        elif op in ("inc", "dec"):
            a = self.get(i.a)
            cf = self.fl & CF
            undef_cf = self.undef & CF
            res = self.alu("add" if op == "inc" else "sub", a, 1)
            self.fl = (self.fl & ~CF) | cf
            self.undef = (self.undef & ~CF) | undef_cf
            self.put(i.a, res)
        elif op == "push":
            v = self.get(i.a)
            self.r[ESP] = (self.r[ESP] - 4) & M32
            self.wr(self.r[ESP], struct.pack("<I", v))
        elif op == "pop":
            v = struct.unpack("<I", self.rd(self.r[ESP], 4))[0]
            self.r[ESP] = (self.r[ESP] + 4) & M32
            self.put(i.a, v)
        elif op == "mul":
            p = self.r[EAX] * self.get(i.a)
            self.r[EAX], self.r[EDX] = p & M32, p >> 32
            f = (CF | OF) if p >> 32 else 0
            self.setflags(f, CF | OF)
            self.setflags(0, PF | AF | ZF | SF, undefined=PF | AF | ZF | SF)
        elif op == "jcc":
            if self.cond(i.a):
                nxt = (nxt + i.b) & M32
        elif op == "jmp":
            nxt = (nxt + i.a) & M32
        elif op == "nop":
            pass
        else:
            raise Unsupported(op)
        self.eip = nxt

    def exact(self):
        """True when every status flag is defined (the run can be committed)."""
        return self.undef == 0


# --- self-test ------------------------------------------------------------------

def _mem(blob):
    def read(addr, n):
        out = bytearray()
        for k in range(n):
            out.append(blob.get((addr + k) & M32, 0))
        return bytes(out)
    return read


def _run(code, regs, eflags=0x202, mem=None, steps=None):
    """Run `code` placed at 0x1000 to its end (or `steps` instructions)."""
    cpu = Cpu(regs, 0x1000, eflags, _mem(mem or {}))
    n = 0
    while cpu.eip < 0x1000 + len(code) and (steps is None or n < steps):
        off = cpu.eip - 0x1000
        cpu.execute(decode(cpu.eip, code[off:off + 15]))
        n += 1
    return cpu


# Covers: specs/tools/rng-trace.md §4 r7
def selftest():
    ok = 0
    K = 0x6AC690C5

    def regs(**kw):
        r = [0] * 8
        for k, v in kw.items():
            r[REG_NAMES.index(k.capitalize())] = v
        return r

    # mov edx, K ; mul edx ; add eax, esi ; adc edx, ebx  (one 64-bit step)
    code = bytes.fromhex("BA") + struct.pack("<I", K) + bytes.fromhex("F7E2 03C6 13D3")
    lo, hi = 0x12345678, 0x9ABCDEF0
    cpu = _run(code, regs(eax=lo, esi=hi, ebx=0))
    v = lo * K + hi
    assert (cpu.r[EAX], cpu.r[EDX]) == (v & M32, v >> 32), (hex(cpu.r[EAX]), hex(cpu.r[EDX]))
    assert cpu.exact() and cpu.eip == 0x1000 + len(code)
    ok += 1
    # a je that is taken skips; the flags it reads come from the caller
    code = bytes.fromhex("7402 9090 90")
    assert _run(code, regs(), eflags=0x202 | ZF, steps=1).eip == 0x1004
    assert _run(code, regs(), eflags=0x202, steps=1).eip == 0x1002
    ok += 1
    # flags: add / adc / sub / cmp / xor against their definitions
    cases = [(0xFFFFFFFF, 1), (0x7FFFFFFF, 1), (0x80000000, 0x80000000), (0, 0), (5, 3),
             (3, 5), (0x0F, 0x01), (0x80000000, 1)]
    for a, b in cases:
        cpu = Cpu(regs(eax=a, ecx=b), 0, 0x202, _mem({}))
        cpu.execute(decode(0, bytes.fromhex("01C8")))   # add eax, ecx
        s = a + b
        r = s & M32
        assert cpu.r[EAX] == r
        assert bool(cpu.fl & CF) == (s > M32)
        assert bool(cpu.fl & OF) == ((a >> 31) == (b >> 31) != (r >> 31))
        assert bool(cpu.fl & ZF) == (r == 0) and bool(cpu.fl & SF) == bool(r >> 31)
        assert bool(cpu.fl & AF) == (((a & 0xF) + (b & 0xF)) > 0xF)
        assert bool(cpu.fl & PF) == (bin(r & 0xFF).count("1") % 2 == 0)
        cpu = Cpu(regs(eax=a, ecx=b), 0, 0x202, _mem({}))
        cpu.execute(decode(0, bytes.fromhex("39C8")))   # cmp eax, ecx
        r = (a - b) & M32
        assert cpu.r[EAX] == a and bool(cpu.fl & CF) == (a < b)
        assert bool(cpu.fl & OF) == ((a >> 31) != (b >> 31) and (r >> 31) != (a >> 31))
        assert bool(cpu.fl & AF) == ((a & 0xF) < (b & 0xF))
        for cf in (0, 1):
            cpu = Cpu(regs(edx=a, esi=b), 0, 0x202 | cf, _mem({}))
            cpu.execute(decode(0, bytes.fromhex("13D6")))   # adc edx, esi
            s = a + b + cf
            assert cpu.r[EDX] == s & M32 and bool(cpu.fl & CF) == (s > M32)
            assert bool(cpu.fl & AF) == (((a & 0xF) + (b & 0xF) + cf) > 0xF)
    ok += 1
    # memory operands: mov ecx, [ebp-8] ; mov [ebp-4], edx ; push ebx ; pop eax
    mem = {0x2000 - 8 + k: v for k, v in enumerate(struct.pack("<I", 0xCAFEBABE))}
    code = bytes.fromhex("8B4DF8 8955FC 53 58")
    cpu = _run(code, regs(ebp=0x2000, edx=0x11223344, ebx=7, esp=0x3000), mem=mem)
    assert cpu.r[ECX] == 0xCAFEBABE and cpu.r[EAX] == 7 and cpu.r[ESP] == 0x3000
    assert cpu.rd(0x2000 - 4, 4) == struct.pack("<I", 0x11223344)
    assert cpu.rd(0x3000 - 4, 4) == struct.pack("<I", 7)
    ok += 1
    # SIB with disp32 and no base; movzx byte; lea
    mem = {0x10 * 4 + 0x500: 0xAB}
    code = bytes.fromhex("0FB6048D00050000 8D4C2410")
    cpu = _run(code, regs(ecx=0x10, esp=0x100), mem=mem)
    assert cpu.r[EAX] == 0xAB and cpu.r[ECX] == 0x110
    ok += 1
    # mul leaves SF ZF AF PF undefined: a jcc on ZF after it refuses, the
    # run is not exact until an op defines them
    cpu = _run(bytes.fromhex("F7E1"), regs(eax=2, ecx=3))
    assert not cpu.exact() and cpu.fl & (CF | OF) == 0
    try:
        cpu.execute(decode(cpu.eip, bytes.fromhex("7400")))
        raise AssertionError("jcc on an undefined flag ran")
    except Unsupported:
        pass
    cpu.execute(decode(cpu.eip, bytes.fromhex("03C1")))
    assert cpu.exact()
    cpu = _run(bytes.fromhex("33C0"), regs(eax=5))  # xor: AF undefined
    assert cpu.r[EAX] == 0 and cpu.fl & ZF and not cpu.exact()
    ok += 1
    # unsupported: prefixes, call, ret, div
    for hx in ("66 01C8", "E8 00000000", "C3", "F7F1", "64 8B00"):
        try:
            decode(0, bytes.fromhex(hx.replace(" ", "")))
            raise AssertionError(hx)
        except Unsupported:
            pass
    ok += 1
    # byte moves: mov ah, [esi] ; mov [edi], cl
    cpu = _run(bytes.fromhex("8A26 880F"), regs(eax=0x11223344, ecx=0x55, esi=0x40, edi=0x41),
               mem={0x40: 0x99})
    assert cpu.r[EAX] == 0x11229944 and cpu.rd(0x41, 1) == b"\x55"
    ok += 1
    # inc keeps CF; 83 /0 sign-extended imm8
    cpu = _run(bytes.fromhex("40 83C0FF"), regs(eax=0), eflags=0x202 | CF)
    assert cpu.r[EAX] == 0 and cpu.fl & CF and cpu.fl & ZF  # 1 + 0xFFFFFFFF carries
    cpu = _run(bytes.fromhex("40"), regs(eax=0xFFFFFFFF), eflags=0x202)
    assert cpu.r[EAX] == 0 and not cpu.fl & CF and cpu.fl & ZF
    ok += 1
    print(f"x86emu selftest: {ok} checks passed")
    return 0


if __name__ == "__main__":
    if "--selftest" in sys.argv[1:]:
        sys.exit(selftest())
    print(__doc__)
