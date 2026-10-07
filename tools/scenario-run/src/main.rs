// Spec: specs/tools/scenario.md (CLI of the d2rs runner and the comparator)
//! `scenario-run`:
//!
//! ```text
//! scenario-run run <script> [--synthetic | --game-dir <dir>] [-o <trace>]
//! scenario-run compare <original.trace.jsonl> <d2rs.trace.jsonl>
//! scenario-run check <script>...
//! ```
//!
//! `run` writes the d2rs trace (default `traces/raw/<name>.d2rs.trace.jsonl`;
//! data: `--game-dir`, else `$D2_GAME_DIR`, else `--synthetic` must be
//! given). `compare` prints the report and exits 0 match, 1 diverged,
//! 2 partial, 3 error. `check` parses scripts and checks that their
//! canonical form round-trips (`scenario.md` §2 rule 6).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use conformance::scenario::{compare, Scenario, TraceFile};
use scenario_run::{run, Data};

fn usage() -> ! {
    eprintln!(
        "usage:\n  scenario-run run <script> [--synthetic | --game-dir <dir>] [-o <trace>]\n  scenario-run compare <original> <d2rs>\n  scenario-run check <script>..."
    );
    std::process::exit(3)
}

fn read_trace(p: &Path) -> Result<TraceFile> {
    let text = std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
    TraceFile::parse(&text).with_context(|| format!("{}", p.display()))
}

fn read_script(p: &Path) -> Result<(Scenario, String)> {
    let text = std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
    let s = Scenario::parse(&text).with_context(|| format!("{}", p.display()))?;
    Ok((s, text))
}

fn cmd_run(args: &[String]) -> Result<ExitCode> {
    let mut script = None;
    let mut synthetic = false;
    let mut game_dir = std::env::var_os("D2_GAME_DIR").map(PathBuf::from);
    let mut out = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--synthetic" => synthetic = true,
            "--game-dir" => game_dir = Some(it.next().unwrap_or_else(|| usage()).into()),
            "-o" => out = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            _ if script.is_none() && !a.starts_with('-') => script = Some(PathBuf::from(a)),
            _ => usage(),
        }
    }
    let script = script.unwrap_or_else(|| usage());
    let (s, _) = read_script(&script)?;
    let data = if synthetic {
        let dir =
            std::env::temp_dir().join(format!("scenario-run-synthetic-{}", std::process::id()));
        let d = Data::synthetic(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        d?
    } else if let Some(dir) = game_dir {
        Data::live(&dir)?
    } else {
        bail!("no game data: give --game-dir, set D2_GAME_DIR, or pass --synthetic");
    };
    let r = run(&s, &data)?;
    for n in &r.notes {
        eprintln!("note: {n}");
    }
    for g in &r.trace.header.gaps {
        eprintln!("gap: {g}");
    }
    let out = out.unwrap_or_else(|| default_trace_path(&s.name, "d2rs"));
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&out, r.trace.to_text())
        .with_context(|| format!("writing {}", out.display()))?;
    println!(
        "{}: {} records → {}",
        s.name,
        r.trace.records.len(),
        out.display()
    );
    Ok(ExitCode::SUCCESS)
}

/// `traces/raw/<name>.<side>.trace.jsonl` (spec §1 rule 2).
fn default_trace_path(name: &str, side: &str) -> PathBuf {
    PathBuf::from(format!("traces/raw/{name}.{side}.trace.jsonl"))
}

fn cmd_compare(args: &[String]) -> Result<ExitCode> {
    let [a, b] = args else { usage() };
    let report = compare(&read_trace(Path::new(a))?, &read_trace(Path::new(b))?)?;
    print!("{report}");
    Ok(ExitCode::from(report.verdict.exit_code() as u8))
}

fn cmd_check(args: &[String]) -> Result<ExitCode> {
    if args.is_empty() {
        usage();
    }
    for p in args {
        let (s, _) = read_script(Path::new(p))?;
        let canonical = s.to_text();
        let again = Scenario::parse(&canonical)?;
        if again != s || again.to_text() != canonical {
            bail!("{p}: the canonical form does not round-trip:\n{canonical}");
        }
        println!("{p}: ok ({} steps, end {})", s.steps.len(), s.end);
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((cmd, rest)) = args.split_first() else {
        usage()
    };
    let r = match cmd.as_str() {
        "run" => cmd_run(rest),
        "compare" => cmd_compare(rest),
        "check" => cmd_check(rest),
        _ => usage(),
    };
    r.unwrap_or_else(|e| {
        eprintln!("error: {e:#}");
        ExitCode::from(3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/tools/scenario.md §1 r2
    #[test]
    fn default_trace_path_follows_the_convention() {
        assert_eq!(
            default_trace_path("walk-town", "d2rs"),
            PathBuf::from("traces/raw/walk-town.d2rs.trace.jsonl")
        );
        assert_eq!(
            default_trace_path("a-1", "original"),
            PathBuf::from("traces/raw/a-1.original.trace.jsonl")
        );
    }
}
