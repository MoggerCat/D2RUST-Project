# Handoff: determinism lint — `claude/determinism-lint`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud tooling session, 2026-10-06, base `main` `edad871`. Repo only (M09).
Makes CLAUDE.md hard rule 6 machine-checked (M05, M07).

## What is checked

`cargo run -p depcheck` (already a CI step, so `.github/workflows/` is
unchanged) now also scans every non-test `.rs` file under
`crates/d2-sim/src` (`tools/depcheck/src/determinism.rs`) and reports
`file:line: determinism rule `<rule>``:

| Rule | Fires on |
|---|---|
| `float` | `f32`, `f64`, float literals (`1.5`, `1.`, `2e3`, `1E+5`, `7f32`; tuple fields `x.0.1` and `0usize` are not floats) |
| `hash-collection` | `HashMap`, `HashSet` (`BTreeMap` / ordered maps are fine) |
| `clock` | `std::time` / `core::time`, `Instant`, `SystemTime` |
| `ambient-rng` | `rand::`, `thread_rng` |
| `io` | `std::fs`, `std::io` (also inside `use std::{…}` groups) |
| `env` | `std::env` |
| `global-state` | `thread_local!`, `static mut` |
| `unsafe` | `unsafe` |

Not scanned: comments, string / raw-string / char literals, `#[cfg(test)]`
items (inline `mod tests { … }`, fns, uses) and the files of
`#[cfg(test)] mod x;` (`x.rs`, `x/mod.rs` and everything below). `cfg(not(test))`
and `cfg(any(…))` are scanned. Tests under `crates/d2-sim/tests/` are
outside `src` and not scanned. Also added to the dependency rules:
`d2-sim` must not depend on `rand` or `getrandom`.

Allowlist: `tools/depcheck/determinism-allow.txt`, lines of
`file<TAB>rule<TAB>reason`. A missing reason is a parse error; an entry that
matches no hit is a stale-entry error. The `IndexMap` condition of the task
needs no code: it is not banned, and without the dependency it would not
compile.

## Existing hits and dispositions

None. The first run reported 11 `float` hits; all were false positives of
the scanner (`0usize` read as an exponent literal: `place.rs:549`,
`vertex.rs:21,37`, `wild.rs:123,257`, `affixes.rs:286,350`, `walk.rs:193,289`).
The scanner was fixed (integer suffixes are not exponents; selftest cases
`0usize`, `4isize`), not the code. Result: **zero real hits, zero
allowlist entries, no d2-sim change.** `softfloat.rs` mentions `f64` only in
comments / test code.

## Selftests (M08)

`cargo test -p depcheck` (also in the CI `cargo test --workspace`), 5 tests:
each of 22 banned constructs is caught with its line; comments, nested
block comments, strings, raw strings, byte strings, chars and lifetimes are
not flagged; `#[cfg(test)]` items skipped while `cfg(not(test))` and code
after are scanned; external test-module files skipped in a fixture tree
(a sibling non-test file is still reported); allowlist reason mandatory and
stale entries reported. By hand: appending `f64`, `HashMap` and `unsafe` to
`d2-sim/src/rng.rs` made `depcheck` exit non-zero with all four hits at the
line; reverted.

## Determinism property test

Not added: two-run same-seed tests already exist
(`wiring::action::tests::e2e::tick_runs_the_combined_dispatcher_deterministically`,
`wiring::worldgen::tests::e2e::generated_level_is_populated_and_ticks_deterministically`,
`wiring::economy::tests::{items::creation_is_deterministic,treasure::walk_is_deterministic}`).

## Limits (say where the claim holds, M09)

Lexical, not semantic: it does not see `HashMap` reached through a type
alias in another crate, floats from a dependency's API, or iteration order
of a `BTreeMap` with a non-total key. `std::time` via `use std::time as t`
is caught at the `use`. Rule 6's "no `HashMap` iteration affecting
outcomes" is enforced as a stricter "no hash collections".

## Gates

All pass: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`;
`cargo test -p depcheck` (5 passed); `cargo run -p depcheck` (OK, lint clean);
`spec_index.py --check`; `methods.py check` (21 OK); `coverage.py --check`
(3195 claims, 0 errors) and `--selftest` (ok). d2-sim untouched, so its tests were not re-run.
