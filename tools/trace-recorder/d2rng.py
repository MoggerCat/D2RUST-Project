"""The 1.14d seeded RNG rule, as written in specs/sim/rng.md.

Shared by check_rng.py and convert_rng.py. Our own code, written from the
spec; nothing here is derived from Blizzard code.
"""

M32 = 0xFFFFFFFF
MULTIPLIER = 0x6AC690C5  # spec §2


def s32(x):
    """Reinterpret a u32 as i32."""
    x &= M32
    return x - 0x1_0000_0000 if x & 0x8000_0000 else x


def step(lo, hi):
    """One step (spec §2): (lo, hi) -> (lo', hi')."""
    v = (lo & M32) * MULTIPLIER + (hi & M32)
    return v & M32, (v >> 32) & M32


def roll(lo, hi, n):
    """spec §3 roll(n). Returns (value, new_lo, new_hi, drew)."""
    if s32(n) < 1:
        return 0, lo, hi, False
    lo2, hi2 = step(lo, hi)
    n &= M32
    if n & (n - 1) == 0:
        return lo2 & (n - 1), lo2, hi2, True
    return lo2 % n, lo2, hi2, True


def mask(lo, hi, n):
    """spec §3 mask(n): no range check, always draws."""
    lo2, hi2 = step(lo, hi)
    return lo2 & ((n - 1) & M32), lo2, hi2, True


def mask_range(lo, hi, lo_bound, n):
    """spec §3 mask_range(min, n)."""
    v, lo2, hi2, _ = mask(lo, hi, n)
    return (v + lo_bound) & M32, lo2, hi2, True


def roll_range(lo, hi, lo_bound, n):
    """spec §3 roll_range(min, n): n < 1 returns min without a draw."""
    if s32(n) < 1:
        return lo_bound & M32, lo, hi, False
    v, lo2, hi2, _ = roll(lo, hi, n)
    return (v + lo_bound) & M32, lo2, hi2, True


def step_op(lo, hi):
    lo2, hi2 = step(lo, hi)
    return lo2, lo2, hi2, True


def apply(op, lo, hi, n=None, lo_bound=None):
    """Apply a named op (the names the recorder writes). Returns
    (value, new_lo, new_hi, drew)."""
    if op == "step":
        return step_op(lo, hi)
    if op == "roll":
        return roll(lo, hi, n)
    if op == "mask":
        return mask(lo, hi, n)
    if op == "mask_range":
        return mask_range(lo, hi, lo_bound, n)
    if op == "roll_range":
        return roll_range(lo, hi, lo_bound, n)
    raise ValueError(f"unknown op {op!r}")


def random_value(x):
    """spec §5.1 time-based seed value (input = time + ticks + caller value)."""
    for _ in range(3):
        x = (x * 0x19660D + 0x3C6EF35F) & M32
    return x & 0x7FFFFFFF
