// Spec: specs/tools/coverage-map.md
//! Behaviour coverage counters (`coverage-map` feature, off by default).
//!
//! [`cov!`](crate::cov) copies ids the game already computed into the
//! `coverage-map` crate's per-thread counters; without the feature it
//! expands to nothing (its arguments are not even evaluated). The game
//! never reads the counters back, so no draw, state or order depends on
//! them (CLAUDE.md rule 6).

#[cfg(feature = "coverage-map")]
pub use coverage_map::Cat;

/// `cov!(Cat, a, b)`: count one event (see `coverage_map::Cat` for what
/// `a` and `b` are per category). Nothing without the feature.
#[macro_export]
macro_rules! cov {
    ($cat:ident, $a:expr, $b:expr) => {{
        #[cfg(feature = "coverage-map")]
        {
            $crate::debug::coverage::hit(
                $crate::debug::coverage::Cat::$cat,
                ($a) as u32,
                ($b) as u32,
            );
        }
    }};
}

#[cfg(feature = "coverage-map")]
pub use coverage_map::hit;

/// Writes the calling thread's counts now (a binary's main thread, whose
/// thread-locals are not dropped at exit). Nothing without the feature.
pub fn flush() {
    #[cfg(feature = "coverage-map")]
    coverage_map::flush();
}
