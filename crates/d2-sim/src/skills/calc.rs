// Spec: specs/data/calc-expressions.md
//! The formula evaluator (§3) over a family code buffer from
//! `d2_data::bin::BinSet::code`. The compiler and the constant evaluator
//! live in `d2_data::calc`; this module adds the runtime half: the
//! parameter callback and the function table, both supplied by a
//! [`CalcContext`] (skills and missiles contexts: [`super::levels`]).
//!
//! Policy (§d2rs policy 5): −2³¹ / −1 gives −2³¹ instead of faulting.

/// The context behind one evaluation (§3.5): the parameter callback and
/// the family's function table.
pub trait CalcContext {
    /// Function-table size of the family (§3.4: skills 7, missiles and
    /// items 4).
    fn function_count(&self) -> u8;
    /// Arity of function `index` (< [`CalcContext::function_count`]).
    fn arity(&self, index: u8) -> u8;
    /// `param(c)` (§3.5). `c` is the operand as the opcode extends it.
    fn param(&mut self, c: i32) -> i32;
    /// Calls function `index` with `args` in source order.
    fn call(&mut self, index: u8, args: &[i32]) -> i32;
}

/// Stack depth (§3.2).
const STACK: usize = 64;

/// `eval(family, offset, context)` (§3.1): 0 when `offset` is at or past
/// the end of `buffer` (0xFFFFFFFF always is), else §3.3 from `offset`.
pub fn eval(buffer: &[u8], offset: u32, ctx: &mut dyn CalcContext) -> i32 {
    match usize::try_from(offset) {
        Ok(o) if o < buffer.len() => run(&buffer[o..], ctx),
        _ => 0,
    }
}

/// §3.3 over `code` (the bytes from the expression start to the buffer
/// end).
pub fn run(code: &[u8], ctx: &mut dyn CalcContext) -> i32 {
    let mut stack: Vec<i32> = Vec::with_capacity(STACK);
    fn push(s: &mut Vec<i32>, v: i32) {
        if s.len() < STACK {
            s.push(v);
        }
    }
    fn pop(s: &mut Vec<i32>) -> i32 {
        s.pop().unwrap_or(0)
    }
    let mut i = 0;
    while i < code.len() {
        let op = code[i];
        i += 1;
        let len = match op {
            0x01 | 0x04 | 0x07 => 1,
            0x05 | 0x08 => 2,
            0x06 | 0x09 => 4,
            _ => 0,
        };
        if i + len > code.len() {
            // A cut operand stops with result 0 (§3.3).
            return 0;
        }
        let o = &code[i..i + len];
        i += len;
        match op {
            0x01 => {
                let index = o[0];
                let count = ctx.function_count();
                let k = if index < count { ctx.arity(index) } else { 4 };
                if k > 3 {
                    push(&mut stack, 0);
                    continue;
                }
                let mut args = [0i32; 3];
                for a in args[..usize::from(k)].iter_mut().rev() {
                    *a = pop(&mut stack);
                }
                let v = ctx.call(index, &args[..usize::from(k)]);
                push(&mut stack, v);
            }
            0x04 => {
                let v = ctx.param(i32::from(o[0]));
                push(&mut stack, v);
            }
            0x05 => {
                let v = ctx.param(i32::from(i16::from_le_bytes([o[0], o[1]])));
                push(&mut stack, v);
            }
            0x06 => {
                let v = ctx.param(i32::from_le_bytes([o[0], o[1], o[2], o[3]]));
                push(&mut stack, v);
            }
            0x07 => push(&mut stack, i32::from(o[0] as i8)),
            0x08 => push(&mut stack, i32::from(i16::from_le_bytes([o[0], o[1]]))),
            0x09 => push(&mut stack, i32::from_le_bytes([o[0], o[1], o[2], o[3]])),
            0x0A..=0x14 => {
                let b = pop(&mut stack);
                let a = pop(&mut stack);
                let v = match op {
                    0x0A => i32::from(a < b),
                    0x0B => i32::from(a > b),
                    0x0C => i32::from(a <= b),
                    0x0D => i32::from(a >= b),
                    0x0E => i32::from(a == b),
                    0x0F => i32::from(a != b),
                    0x10 => a.wrapping_add(b),
                    0x11 => a.wrapping_sub(b),
                    0x12 => a.wrapping_mul(b),
                    0x13 => {
                        if b == 0 {
                            0
                        } else {
                            a.wrapping_div(b)
                        }
                    }
                    _ => {
                        if b <= 0 {
                            1
                        } else {
                            a.wrapping_pow(b as u32)
                        }
                    }
                };
                push(&mut stack, v);
            }
            0x15 => {
                let v = pop(&mut stack);
                push(&mut stack, v.wrapping_neg());
            }
            0x16 => {
                let f = pop(&mut stack);
                let t = pop(&mut stack);
                let c = pop(&mut stack);
                push(&mut stack, if c != 0 { t } else { f });
            }
            _ => return pop(&mut stack),
        }
    }
    0
}

/// `rand(a, b)` (§Randomness): `a ≥ b` → `a`, no draw; else `a +
/// R(b − a + 1)` with `R` = `roll` (an `n` that wrapped below 1 gives 0
/// with no step).
pub fn rand(seed: &mut crate::rng::Seed, a: i32, b: i32) -> i32 {
    if a >= b {
        return a;
    }
    let n = b.wrapping_sub(a).wrapping_add(1);
    a.wrapping_add(seed.roll(n) as i32)
}

#[cfg(test)]
mod tests {
    // Test vectors: specs/data/calc-expressions.md "Evaluator" and
    // "Randomness".
    use super::*;
    use crate::rng::Seed;

    /// The spec's stub context: `param(c)` = c; `min`, `max` real;
    /// `skill(s, c)` = 100·s + c; `sklvl(s, a, b)` = 10,000·s + 100·a + b.
    struct Stub;
    impl CalcContext for Stub {
        fn function_count(&self) -> u8 {
            7
        }
        fn arity(&self, index: u8) -> u8 {
            if index == 6 {
                3
            } else {
                2
            }
        }
        fn param(&mut self, c: i32) -> i32 {
            c
        }
        fn call(&mut self, index: u8, a: &[i32]) -> i32 {
            match index {
                0 => a[0].min(a[1]),
                1 => a[0].max(a[1]),
                3 => 100 * a[0] + a[1],
                6 => 10_000 * a[0] + 100 * a[1] + a[2],
                _ => 0,
            }
        }
    }

    fn ev(bytes: &[u8]) -> i32 {
        eval(bytes, 0, &mut Stub)
    }

    // Covers: specs/data/calc-expressions.md §3.1 r1, §3.2, §3.3
    #[test]
    fn evaluator_vectors() {
        assert_eq!(ev(&[0x04, 0x10, 0x07, 0x02, 0x12, 0x00]), 32);
        assert_eq!(
            ev(&[0x07, 0x66, 0x07, 0x04, 0x07, 0x11, 0x01, 0x06, 0x00]),
            1_020_417
        );
        assert_eq!(
            ev(&[0x04, 0x02, 0x08, 0x96, 0x00, 0x01, 0x00, 0x15, 0x00]),
            -2
        );
        assert_eq!(ev(&[0x07, 0x05, 0x07, 0x00, 0x13, 0x00]), 0);
        assert_eq!(ev(&[0x07, 0xF9, 0x07, 0x02, 0x13, 0x00]), -3);
        assert_eq!(ev(&[0x07, 0x07, 0x07, 0xFE, 0x13, 0x00]), -3);
        assert_eq!(ev(&[0x07, 0xFE, 0x07, 0x03, 0x14, 0x00]), -8);
        assert_eq!(ev(&[0x07, 0x02, 0x07, 0x00, 0x14, 0x00]), 1);
        assert_eq!(ev(&[0x07, 0x02, 0x07, 0xFF, 0x14, 0x00]), 1);
        assert_eq!(ev(&[0x09, 0x00, 0x00, 0x00, 0x80, 0x15, 0x00]), i32::MIN);
        assert_eq!(ev(&[0x07, 0x05, 0x07, 0x07, 0x11, 0x00]), -2);
        assert_eq!(ev(&[0x06, 0x00, 0x00, 0x01, 0x00, 0x00]), 65_536);
        assert_eq!(ev(&[0x07, 0x03, 0x07, 0x64, 0x14, 0x00]), -818_408_495);
        assert_eq!(ev(&[0x09, 0, 0, 1, 0, 0x09, 0, 0, 1, 0, 0x12, 0x00]), 0);
        assert_eq!(
            ev(&[0x09, 0x00, 0x00, 0x00, 0x80, 0x07, 0xFF, 0x13, 0x00]),
            i32::MIN
        );
        assert_eq!(ev(&[0x10, 0x00]), 0);
        assert_eq!(ev(&[0x07, 0x05, 0x07, 0x06, 0x16, 0x00]), 6);
        assert_eq!(ev(&[0x07, 0x00, 0x07, 0x01, 0x16, 0x07, 0x05, 0x00]), 5);
        assert_eq!(ev(&[0x07, 0x05, 0x07, 0x06, 0x01, 0x09, 0x00]), 0);
        assert_eq!(ev(&[0x04, 0xFF, 0x00]), 255);
        assert_eq!(ev(&[0x05, 0xFF, 0xFF, 0x00]), -1);
        assert_eq!(ev(&[0x07, 0xFF, 0x00]), -1);
        for stop in [0x03, 0x17, 0xFF] {
            assert_eq!(ev(&[0x07, 0x05, stop, 0x07, 0x06, 0x00]), 5);
        }
        assert_eq!(ev(&[0x07, 0x05, 0x02, 0x07, 0x06, 0x10, 0x00]), 5);
        for cut in [&[0x07][..], &[0x08, 0x05], &[0x01], &[0x07, 0x05]] {
            assert_eq!(ev(cut), 0, "{cut:02x?}");
        }
        let mut full = Vec::new();
        for _ in 0..64 {
            full.extend([0x07, 0x01]);
        }
        full.extend([0x07, 0x02, 0x00]);
        assert_eq!(ev(&full), 1);
        assert_eq!(eval(&[0x07, 0x05, 0x00], 0xFFFF_FFFF, &mut Stub), 0);
        assert_eq!(eval(&[0x07, 0x05, 0x00], 3, &mut Stub), 0);
    }

    // Covers: specs/data/calc-expressions.md §3.3
    #[test]
    fn comparisons() {
        // (a, b) = (−1, 1), (2, 2), (3, 2) per op 0x0A..0x0F.
        let want = [
            [1, 0, 0],
            [0, 0, 1],
            [1, 1, 0],
            [0, 1, 1],
            [0, 1, 0],
            [1, 0, 1],
        ];
        for (k, w) in want.iter().enumerate() {
            let op = 0x0A + k as u8;
            for (j, (a, b)) in [(0xFF, 1), (2, 2), (3, 2)].into_iter().enumerate() {
                assert_eq!(
                    ev(&[0x07, a, 0x07, b, op, 0x00]),
                    w[j],
                    "op {op:#x} case {j}"
                );
            }
        }
    }

    // Covers: specs/data/calc-expressions.md §3.5
    #[test]
    fn rand_vectors() {
        let mut s = Seed::new(1, 0);
        assert_eq!(rand(&mut s, 1, 6), 4);
        assert_eq!(s, Seed::new(0x6AC6_90C5, 0));
        let mut s = Seed::new(1, 0);
        assert_eq!(rand(&mut s, 0, 7), 5);
        let mut s = Seed::new(1, 0);
        assert_eq!(rand(&mut s, 5, 5), 5);
        assert_eq!(rand(&mut s, -2, i32::MAX), -2);
        assert_eq!(s, Seed::new(1, 0));
    }
}
