//! Enforces the crate dependency rules from CLAUDE.md (rule 5) by inspecting
//! each crate's full normal+build dependency tree with `cargo tree`.
//!
//! Usage: `cargo run -p depcheck`. Exits non-zero on any violation.

mod determinism;

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// Crates that must never pull in Bevy, directly or transitively.
const NO_BEVY: &[&str] = &[
    "d2-formats",
    "d2-data",
    "d2-sim",
    "d2-proto",
    "d2-net",
    "d2-server",
    "d2-verify",
    "conformance",
    "d2-native",
];

/// (crate, forbidden dependency) pairs: layering rules beyond Bevy.
const FORBIDDEN: &[(&str, &str)] = &[
    // The sim does no I/O and knows nothing about transport or the client.
    ("d2-sim", "d2-net"),
    ("d2-sim", "d2-server"),
    ("d2-sim", "d2-client"),
    ("d2-sim", "d2-proto"),
    // No ambient randomness in the sim (rule 6): only the seeded RNG.
    ("d2-sim", "rand"),
    ("d2-sim", "getrandom"),
    // The server never depends on the client.
    ("d2-server", "d2-client"),
];

fn deps_of(krate: &str) -> Result<Vec<String>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args([
            "tree",
            "-p",
            krate,
            "-e",
            "normal,build",
            "--prefix",
            "none",
        ])
        .args(["--format", "{p}"])
        .output()
        .with_context(|| format!("running cargo tree for {krate}"))?;
    if !out.status.success() {
        bail!(
            "cargo tree -p {krate} failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    // Lines look like "bevy v0.19.1" or "d2-data v0.0.0 (path)".
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_owned)
        .collect())
}

fn main() -> Result<()> {
    let mut violations = Vec::new();

    for krate in NO_BEVY {
        let deps = deps_of(krate)?;
        for dep in deps.iter().filter(|d| d.starts_with("bevy")) {
            violations.push(format!("{krate} depends on {dep} (Bevy is client-only)"));
        }
        for (_, bad) in FORBIDDEN.iter().filter(|(k, _)| k == krate) {
            if deps.iter().any(|d| d == bad) {
                violations.push(format!("{krate} must not depend on {bad}"));
            }
        }
    }

    // Hard rule 6: determinism lint over d2-sim's non-test sources.
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = here.join("../..");
    violations.extend(determinism::check(
        &root,
        &here.join("determinism-allow.txt"),
    )?);

    if violations.is_empty() {
        println!(
            "depcheck: OK ({} crates checked, d2-sim determinism lint clean)",
            NO_BEVY.len()
        );
        Ok(())
    } else {
        for v in &violations {
            eprintln!("depcheck: {v}");
        }
        bail!("{} dependency rule violation(s)", violations.len());
    }
}
