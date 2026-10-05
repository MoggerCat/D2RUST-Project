// Spec: specs/sim/rng.md
//! The D2 seeded RNG: a 64-bit multiply-with-carry state split into two
//! u32 words, the draw helpers built on it, and the seed setters.
//!
//! Every game object that needs randomness owns a [`Seed`]. Seeds are value
//! types: copying one copies the generator (spec, edge case 4).

/// Step multiplier (spec §2).
pub const K: u32 = 0x6AC6_90C5;

/// High word of every freshly initialized seed (spec §4.1).
pub const INIT_HI: u32 = 666;

/// One seed: the generator state (spec §1). D2MOO: `D2SeedStrc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seed {
    /// Low word (offset 0); a draw's raw output.
    pub lo: u32,
    /// High word (offset 4); the multiply-with-carry carry.
    pub hi: u32,
}

impl Default for Seed {
    /// `{1, 666}`, the state `init()` writes.
    fn default() -> Self {
        Self::init()
    }
}

impl Seed {
    /// `{lo, hi}` as given (`set(lo, hi)`, spec §4).
    pub const fn new(lo: u32, hi: u32) -> Self {
        Self { lo, hi }
    }

    /// `init()`: `{1, 666}` (spec §4).
    pub const fn init() -> Self {
        Self::init_low(1)
    }

    /// `init_low(x)`: `{x, 666}` (spec §4).
    pub const fn init_low(x: u32) -> Self {
        Self { lo: x, hi: INIT_HI }
    }

    /// Overwrites the whole state (`set(lo, hi)`, spec §4).
    pub fn set(&mut self, lo: u32, hi: u32) {
        *self = Self::new(lo, hi);
    }

    /// One step (spec §2): `v = lo·K + hi`, state becomes
    /// `{v mod 2^32, v >> 32}`. Returns the new low word.
    pub fn step(&mut self) -> u32 {
        let v = u64::from(self.lo) * u64::from(K) + u64::from(self.hi);
        // Truncations are the rule: low and high halves of v.
        self.lo = v as u32;
        self.hi = (v >> 32) as u32;
        self.lo
    }

    /// A child seed derived from this one (spec §5): one step, then
    /// `init_low(lo')` on the child.
    pub fn derive(&mut self) -> Seed {
        Seed::init_low(self.step())
    }

    /// `roll(n)` (spec §3): a value in `[0, n)`. `n < 1` (signed) returns 0
    /// **without stepping**. Otherwise one step and `lo' mod n` (unsigned;
    /// equal to the original's power-of-two mask branch, spec §3.3).
    pub fn roll(&mut self, n: i32) -> u32 {
        if n < 1 {
            return 0;
        }
        self.step() % n.unsigned_abs()
    }

    /// `mask(n)` (spec §3): one step, `lo' & (n−1)`. No range check:
    /// `n = 0` gives `lo'`, a non-power-of-two gives the biased mask
    /// (edge case 2).
    pub fn mask(&mut self, n: u32) -> u32 {
        self.step() & n.wrapping_sub(1)
    }

    /// `mask_range(min, n)` (spec §3): one step, `(lo' & (n−1)) + min`,
    /// wrapping 32-bit arithmetic read as i32.
    pub fn mask_range(&mut self, min: i32, n: u32) -> i32 {
        (self.mask(n) as i32).wrapping_add(min)
    }

    /// `roll_range(min, n)` (spec §3): `n < 1` returns `min` without
    /// stepping, else `roll(n) + min` (wrapping, read as i32).
    pub fn roll_range(&mut self, min: i32, n: i32) -> i32 {
        if n < 1 {
            return min;
        }
        (self.roll(n) as i32).wrapping_add(min)
    }
}

/// The time-value mix (spec §5.1). `x` is the caller's sum
/// `time() + GetTickCount() + v` (u32 wrap), supplied as an input: the sim
/// never reads the clock (CLAUDE.md rule 6). Three LCG rounds, then
/// `& 0x7FFFFFFF`.
pub fn time_value(x: u32) -> u32 {
    let mut x = x;
    for _ in 0..3 {
        x = x.wrapping_mul(0x0019_660D).wrapping_add(0x3C6E_F35F);
    }
    x & 0x7FFF_FFFF
}

#[cfg(test)]
mod tests {
    // Test vectors: specs/sim/rng.md, "Test vectors".
    use super::*;

    const START: Seed = Seed::init();

    #[test]
    fn step_vectors() {
        let cases = [
            ((1, 666), (1_791_398_751, 0)),
            ((0, 666), (666, 0)),
            ((666, 0), (3_365_183_618, 277)),
            ((0xFFFF_FFFF, 0xFFFF_FFFF), (0x9539_6F3A, 0x6AC6_90C5)),
        ];
        for ((lo, hi), (lo2, hi2)) in cases {
            let mut s = Seed::new(lo, hi);
            assert_eq!(s.step(), lo2);
            assert_eq!(s, Seed::new(lo2, hi2), "step from {{{lo}, {hi}}}");
        }
    }

    #[test]
    fn zero_is_fixed_point() {
        let mut s = Seed::new(0, 0);
        for _ in 0..4 {
            assert_eq!(s.step(), 0);
        }
        assert_eq!(s, Seed::new(0, 0));
    }

    #[test]
    fn five_steps_from_default() {
        let lo = [
            1_791_398_751,
            791_599_131,
            671_516_612,
            3_064_641_593,
            3_217_527_747,
        ];
        let hi = [0, 747_178_749, 330_169_957, 280_084_454, 1_278_238_622];
        let mut s = Seed::default();
        for i in 0..5 {
            assert_eq!(s.step(), lo[i]);
            assert_eq!(s.hi, hi[i]);
        }
    }

    #[test]
    fn previous_hi_from_two_low_words() {
        // Spec §2: hi = (lo' − lo·K) mod 2^32.
        let mut s = Seed::new(971_488_495, 666);
        for _ in 0..16 {
            let before = s;
            let lo2 = s.step();
            assert_eq!(lo2.wrapping_sub(before.lo.wrapping_mul(K)), before.hi);
        }
    }

    #[test]
    fn roll_nonpositive_does_not_step() {
        for n in [0, -5, i32::MIN] {
            let mut s = START;
            assert_eq!(s.roll(n), 0);
            assert_eq!(s, START);
        }
    }

    #[test]
    fn roll_vectors() {
        for (n, want) in [
            (1, 0),
            (8, 7),
            (10, 1),
            (100, 51),
            (0x7FFF_FFFF, 1_791_398_751),
        ] {
            let mut s = START;
            assert_eq!(s.roll(n), want, "roll({n})");
            assert_eq!(s, Seed::new(1_791_398_751, 0));
        }
    }

    #[test]
    fn mask_vectors() {
        for (n, want) in [(0, 1_791_398_751), (16, 15), (10, 9)] {
            let mut s = START;
            assert_eq!(s.mask(n), want, "mask({n})");
            assert_eq!(s, Seed::new(1_791_398_751, 0));
        }
        let mut s = START;
        assert_eq!(s.mask_range(3, 8), 10);
    }

    #[test]
    fn roll_range_vectors() {
        let mut s = START;
        assert_eq!(s.roll_range(5, 0), 5);
        assert_eq!(s, START, "n < 1 must not step");
        for ((min, n), want) in [((5, 10), 6), ((-3, 8), 4)] {
            let mut s = START;
            assert_eq!(s.roll_range(min, n), want, "roll_range({min}, {n})");
        }
    }

    #[test]
    fn time_value_vectors() {
        assert_eq!(time_value(0), 1_372_387_049);
        assert_eq!(time_value(12345), 185_352_726);
    }

    #[test]
    fn setters_and_derive() {
        let mut s = Seed::new(5, 6);
        s.set(7, 8);
        assert_eq!(s, Seed::new(7, 8));
        assert_eq!(Seed::init_low(42), Seed::new(42, 666));
        // sim-0003 #1: the DRLG seed's first step is the level start seed.
        let mut drlg = Seed::init_low(644_409_375);
        assert_eq!(drlg.derive(), Seed::new(4_014_346_869, 666));
        assert_eq!(drlg, Seed::new(4_014_346_869, 268_778_232));
    }

    #[test]
    fn recorded_values() {
        // Spec "Test vectors", values from the recorded traces.
        let mut s = Seed::new(0, 666);
        assert_eq!(s.roll(3), 0);
        assert_eq!(s, Seed::new(666, 0));
        assert_eq!(s.roll(3), 2);
        assert_eq!(s, Seed::new(3_365_183_618, 277));

        let mut s = Seed::new(971_488_495, 666);
        assert_eq!(s.step(), 7_657_093);
        assert_eq!(s, Seed::new(7_657_093, 405_200_438));
        assert_eq!(s.step(), 664_322_703);
        assert_eq!(s, Seed::new(664_322_703, 3_193_715));

        let mut s = Seed::new(1_936_801_471, 624_310_379);
        assert_eq!(s.roll(3), 2);
        assert_eq!(s, Seed::new(1_281_421_670, 807_825_114));
    }
}
