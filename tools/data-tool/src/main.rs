// Spec: specs/data/loading.md ("d2-data policy" 3), specs/data/field-types.md §6.7, §10
//! Data-table tool.
//!
//! Usage (run with --release):
//!   data-tool tables [game_dir]
//!   data-tool links [game_dir]
//!
//! `tables` loads and validates every live `.bin` (73 record tables, 4 code
//! buffers, `hitclass`), compiles every table's highest-priority `.txt` in
//! load order and compares them byte for byte. Exit status 1 when a
//! runtime table or code buffer differs in a way no spec rule explains.
//!
//! `links` checks every lookup field of the live set against its linker's
//! key count (`field-types.md` §6.7) and prints each broken link with
//! table, row and column. Exit status 1 when a link is broken or a linker
//! size is unknown.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use d2_data::crosscheck::{self, CrossCheck, Role, TableReport};
use d2_data::{bin, links as xref};
use d2_formats::mpq::ArchiveSet;

fn game_dir(arg: Option<&String>) -> Result<PathBuf> {
    Ok(match arg {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(std::env::var("D2_GAME_DIR").context("set D2_GAME_DIR")?),
    })
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("tables") if args.len() <= 2 => tables(&game_dir(args.get(1))?),
        Some("links") if args.len() <= 2 => links(&game_dir(args.get(1))?),
        _ => bail!("usage: data-tool tables|links [game_dir]"),
    }
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn short(archive: &str) -> &str {
    archive.strip_suffix(".mpq").unwrap_or(archive)
}

fn status(t: &TableReport) -> String {
    if let Some(note) = &t.note {
        return format!("not compared: {note}");
    }
    if !t.counts_equal() {
        return format!(
            "MISMATCH: {} records from txt, {} in bin",
            t.records_txt,
            t.records_bin.unwrap_or(0)
        );
    }
    if t.identical() {
        return "identical".into();
    }
    let explained: usize = t.explained.values().map(|(bytes, _)| bytes).sum();
    if t.matches() {
        return format!("explained ({explained} bytes)");
    }
    let bytes: usize = t.mismatches.values().map(|d| d.bytes).sum();
    format!("MISMATCH: {} fields, {bytes} bytes", t.mismatches.len())
}

fn print_details(t: &TableReport) {
    for (reason, (bytes, records)) in &t.explained {
        println!("      explained: {bytes} bytes in {records} records: {reason}");
    }
    for (field, d) in &t.mismatches {
        println!(
            "      field {field}: {} records, {} bytes",
            d.records, d.bytes
        );
        for e in &d.examples {
            println!(
                "        record {} +{}: bin [{}] txt [{}]",
                e.record,
                e.offset,
                hex(&e.shipped),
                hex(&e.compiled)
            );
        }
    }
}

fn print_group(report: &CrossCheck, role: Role, title: &str) {
    let rows: Vec<&TableReport> = report.tables.iter().filter(|t| t.role == role).collect();
    if rows.is_empty() {
        return;
    }
    println!("\n{title}");
    println!(
        "  {:<18} {:<9} {:<9} {:>7}  result",
        "table", "txt", "bin", "records"
    );
    for t in rows {
        let bin = t.bin_source.as_deref().map(short).unwrap_or("-");
        let moved = match (&t.bin_source, &t.expected_source) {
            (Some(a), Some(b)) if a != b => format!(" (tables.tsv: {})", short(b)),
            _ => String::new(),
        };
        println!(
            "  {:<18} {:<9} {:<9} {:>7}  {}{moved}",
            t.name,
            short(&t.txt_source),
            bin,
            t.records_txt,
            status(t)
        );
        print_details(t);
    }
}

fn tables(dir: &std::path::Path) -> Result<()> {
    let set = ArchiveSet::open_dir(dir).with_context(|| format!("opening {}", dir.display()))?;
    let report = crosscheck::run(&set).context("cross-check")?;
    println!(
        "live .bin set: 73 record tables, 4 code buffers and hitclass loaded and validated \
         (loading.md §4.2, §8, §10.8)"
    );

    print_group(&report, Role::Runtime, "runtime tables (loading.md §6)");
    print_group(
        &report,
        Role::ClientOnly,
        "client-only table (loading.md §3.5)",
    );
    print_group(
        &report,
        Role::ByProduct,
        "compile-only by-products (loading.md §7.2)",
    );

    println!("\ncode buffers (loading.md §4.3)");
    for b in &report.buffers {
        let r = &report.code_reports[&b.buffer];
        let result = match b.first_difference {
            None => "identical".to_string(),
            Some(at) => format!("MISMATCH: first difference at byte {at}"),
        };
        println!(
            "  {:<14} {:<9} bin {:>5} bytes, txt {:>5} bytes  {result}; {} expressions, {} unreferenced, {} shared, {} PAREN, {} unknown CALL",
            b.buffer.name(),
            short(&b.source),
            b.shipped,
            b.compiled,
            r.starts.len(),
            r.unreferenced,
            r.shared,
            r.paren,
            r.unknown_call
        );
    }

    let runtime: Vec<&TableReport> = report
        .tables
        .iter()
        .filter(|t| t.role == Role::Runtime)
        .collect();
    let identical = runtime.iter().filter(|t| t.identical()).count();
    let explained = runtime
        .iter()
        .filter(|t| t.matches() && !t.identical())
        .count();
    let bad = runtime.len() - identical - explained;
    let buffers_ok = report.buffers.iter().filter(|b| b.identical()).count();
    println!(
        "\nsummary: {} runtime tables: {identical} identical, {explained} explained, {bad} mismatched; \
         code buffers: {buffers_ok}/{} identical",
        runtime.len(),
        report.buffers.len()
    );

    let diags: Vec<String> = report
        .diagnostics
        .iter()
        .map(|(k, n)| format!("{k:?} {n}"))
        .collect();
    println!(
        "compiler diagnostics (all compiled lists): {}",
        diags.join(", ")
    );
    let calc: Vec<String> = report
        .calc_diagnostics
        .iter()
        .map(|((b, k), n)| format!("{} {k:?} {n}", b.name()))
        .collect();
    println!("calc diagnostics: {}", calc.join(", "));
    let cbs: Vec<String> = report
        .unspecified_callbacks
        .iter()
        .map(|(c, n)| format!("cb({c}) {n}"))
        .collect();
    if !cbs.is_empty() {
        println!(
            "unknown table callbacks (wrote nothing): {}",
            cbs.join(", ")
        );
    }

    if bad > 0 || buffers_ok != report.buffers.len() {
        std::process::exit(1);
    }
    Ok(())
}

fn links(dir: &std::path::Path) -> Result<()> {
    let set = ArchiveSet::open_dir(dir).with_context(|| format!("opening {}", dir.display()))?;
    let data = bin::load(&set, bin::DEFAULT_LANGUAGE).context("loading the live .bin set")?;
    let lookups = xref::load_lookups(&set).context("loading lookup by-products")?;
    let (sizes, report) = xref::validate_set(&data, &lookups);
    println!("linker sizes (field-types.md §6.7)");
    for (linker, n) in sizes.iter() {
        println!("  {linker:<24} {n:>6}");
    }
    for f in &report.unchecked {
        println!(
            "UNCHECKED: {} column `{}`: size of {} unknown",
            f.table, f.column, f.linker
        );
    }
    for b in &report.broken {
        println!("BROKEN: {b}");
    }
    println!(
        "\nsummary: {} valid, {} misses (-1), {} broken, {} unchecked fields",
        report.valid,
        report.misses,
        report.broken.len(),
        report.unchecked.len()
    );
    if !report.is_clean() || !report.unchecked.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
