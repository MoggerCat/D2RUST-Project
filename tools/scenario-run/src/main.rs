// Spec: specs/tools/scenario.md (CLI of the d2rs runner and the comparator)
//! `scenario-run`:
//!
//! ```text
//! scenario-run run <script> [--synthetic | --game-dir <dir>] [-o <trace>]
//! scenario-run compare <original.trace.jsonl> <d2rs.trace.jsonl>
//! scenario-run check <script>...
//! scenario-run export <script>      (JSON for the original-side runner)
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
    let out =
        out.unwrap_or_else(|| PathBuf::from(format!("traces/raw/{}.d2rs.trace.jsonl", s.name)));
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

/// `export`: the parsed script as JSON for the original-side runner
/// (`tools/trace-recorder/run_scenario.py`), which does not parse scripts
/// itself: header facts, the canonical-text SHA-256, snapshot ticks and
/// the steps with typed messages as layout fields (handoff §3 change 1).
fn cmd_export(args: &[String]) -> Result<ExitCode> {
    use conformance::scenario::script::{Arg, Ref, StepMsg};
    use d2_proto::schema::FieldType;
    use d2_proto::CLIENT_MESSAGES;
    use serde_json::{json, Value};
    let [p] = args else { usage() };
    let (s, _) = read_script(Path::new(p))?;
    let arg = |a: &Arg| -> Value {
        match a {
            Arg::Num(v) => json!({"num": v}),
            Arg::Ref(r) => {
                let (kind, d, ty, class, n) = match r {
                    Ref::Player => ("player", 0, 0, None, 0),
                    Ref::X(d) => ("x", *d, 0, None, 0),
                    Ref::Y(d) => ("y", *d, 0, None, 0),
                    Ref::Unit { ty, class, n } => ("unit", 0, *ty, *class, *n),
                    Ref::Waypoint(n) => ("wp", 0, 0, None, *n),
                };
                json!({"ref": r.to_string(), "kind": kind, "d": d, "ty": ty, "class": class, "n": n})
            }
        }
    };
    let steps: Vec<Value> = s
        .steps
        .iter()
        .map(|st| match &st.msg {
            StepMsg::Hex(b) => json!({"tick": st.tick, "hex": b.iter().map(|x| format!("{x:02x}")).collect::<String>()}),
            StepMsg::Typed { id, fields } => {
                let m = &CLIENT_MESSAGES[usize::from(*id)];
                let size = match m.transport_size {
                    d2_proto::schema::SizeRule::Fixed(n) => n,
                    _ => 0,
                };
                let fs: Vec<Value> = m
                    .layout
                    .iter()
                    .map(|f| {
                        let (ty, bits, shift) = match f.ty {
                            FieldType::U8 => ("u8", 8, 0),
                            FieldType::U16 => ("u16", 16, 0),
                            FieldType::U32 => ("u32", 32, 0),
                            FieldType::Bits(n) => ("bits", n, 0),
                            FieldType::Bit(n) => ("bits", 1, n),
                            _ => ("?", 0, 0),
                        };
                        let a = fields.iter().find(|(n, _)| n == f.name).map(|(_, a)| arg(a));
                        json!({"name": f.name, "ty": ty, "bits": bits, "shift": shift,
                               "offset": f.offset.unwrap_or(0), "arg": a})
                    })
                    .collect();
                json!({"tick": st.tick, "name": m.name, "id": id, "size": size, "fields": fs})
            }
            StepMsg::Spawn(_) => json!({"tick": st.tick, "spawn": true}),
        })
        .collect();
    let out = json!({
        "name": s.name, "sha256": s.sha256(), "seed": s.seed, "init": s.init,
        "difficulty": s.difficulty.index(), "end": s.end, "save": s.save,
        "class": s.character.as_ref().map(|c| c.class),
        "streams": s.record.iter().map(|x| x.name()).collect::<Vec<_>>(),
        "snapshot_ticks": s.snapshot_ticks().into_iter().collect::<Vec<_>>(),
        "inline_lines": s.character.is_some(),
        "steps": steps,
    });
    println!("{out}");
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
        "export" => cmd_export(rest),
        _ => usage(),
    };
    r.unwrap_or_else(|e| {
        eprintln!("error: {e:#}");
        ExitCode::from(3)
    })
}
