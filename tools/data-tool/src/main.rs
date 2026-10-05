// Spec: specs/data/loading.md ("d2-data policy" 3), specs/data/field-types.md §6.7, §10
//! Data-table tool.
//!
//! Usage (run with --release):
//!   data-tool tables [game_dir]
//!   data-tool gen-tables
//!   data-tool links [game_dir]
//!   data-tool dump-compare <dump_dir> [game_dir]
//!   data-tool patch (check | render | diff) ...
//!
//! `tables` loads and validates every live `.bin` (73 record tables, 4 code
//! buffers, `hitclass`), compiles every table's highest-priority `.txt` in
//! load order and compares them byte for byte. Exit status 1 when a
//! runtime table or code buffer differs in a way no spec rule explains.
//!
//! `gen-tables` regenerates `crates/d2-data/src/tables/generated.rs` (the
//! typed record structs) from the embedded schema.
//!
//! `links` checks every lookup field of the live set against its linker's
//! key count (`field-types.md` §6.7) and prints each broken link with
//! table, row and column. Exit status 1 when a link is broken or a linker
//! size is unknown.
//!
//! `dump-compare` compares a 1.14d memory dump of the loaded tables
//! (`tools/trace-recorder/dump_tables.py`, `traces/raw/<time>-tables/`)
//! with the live `.bin` set after `d2_data::fixup` (`loading.md` §7.4,
//! open question 15). Exit status 1 when a byte differs outside a pointer
//! field and outside the rows `fixup::PENDING` lists.
//!
//! `patch` checks a mod stack, renders patched tables or diffs an edited
//! table into a layer (`specs/data/patch-layers.md` §10).

mod patch;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use d2_data::crosscheck::{self, CrossCheck, Role, TableReport};
use d2_data::fixup::{self, FixedSet};
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
        Some("gen-tables") if args.len() == 1 => gen_tables(),
        Some("links") if args.len() <= 2 => links(&game_dir(args.get(1))?),
        Some("dump-compare") if (2..=3).contains(&args.len()) => {
            dump_compare(Path::new(&args[1]), &game_dir(args.get(2))?)
        }
        Some("patch") => std::process::exit(patch::main(&args[1..])),
        _ => bail!(
            "usage: data-tool tables|links [game_dir] | data-tool gen-tables | \
             data-tool dump-compare <dump_dir> [game_dir] | data-tool patch ..."
        ),
    }
}

fn gen_tables() -> Result<()> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/d2-data")
        .join(d2_data::codegen::GENERATED_PATH);
    let code = d2_data::codegen::generate(d2_data::schema::schema());
    std::fs::write(&path, &code).with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {} ({} bytes)", path.display(), code.len());
    Ok(())
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

// --- dump-compare ------------------------------------------------------------

/// Record fields that hold in-memory pointers in 1.14d (set at load time;
/// they change from run to run), as (table, offset, width, what). They are
/// excluded from the comparison.
const POINTER_FIELDS: &[(&str, usize, usize, &str)] = &[(
    "sets",
    0x110,
    24,
    "6 pointers to the attached setitems records",
)];

/// Where the `fixup::PENDING` rows write, as `loading.md` §7.4 states
/// it: (pending row, table, record byte range). Bytes the game changes
/// outside these ranges are reported as differences.
type PendingTarget = (&'static str, &'static str, (usize, usize));
const PENDING_TARGETS: &[PendingTarget] = &[
    ("itemstatcost", "itemstatcost", (0x51, 0x54)),
    ("itemstatcost", "itemstatcost", (0x5E, 0x100)),
    ("charstats", "charstats", (0x00, 0x20)),
    ("setitems", "sets", (0x0C, 0x10)),
    ("setitems", "setitems", (0x2E, 0x30)),
    ("gems", "gems", (0x2C, 0x2E)),
    ("gems", "weapons", (0xF0, 0xF4)),
    ("gems", "armor", (0xF0, 0xF4)),
    ("gems", "misc", (0xF0, 0xF4)),
    ("monstats", "monstats", (0x36, 0x3A)),
    ("monstats", "monstats", (0x4A, 0x4C)),
    ("levels", "levels", (0x33, 0x36)),
    ("levels", "levels", (0x16E, 0x20E)),
];

struct DumpEntry {
    kind: String,
    address: String,
    count: usize,
    size: usize,
}

fn read_manifest(dir: &Path) -> Result<BTreeMap<String, DumpEntry>> {
    let path = dir.join("manifest.tsv");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut out = BTreeMap::new();
    for line in text.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() != 5 {
            bail!("{}: bad line `{line}`", path.display());
        }
        out.insert(
            c[1].to_owned(),
            DumpEntry {
                kind: c[0].to_owned(),
                address: c[2].to_owned(),
                count: c[3].parse()?,
                size: c[4].parse()?,
            },
        );
    }
    Ok(out)
}

/// The `fields.tsv` field(s) whose footprint holds byte `offset`.
fn field_label(table: &str, offset: usize) -> String {
    let names: Vec<String> = d2_data::schema::schema()
        .runtime()
        .find(|t| t.name == table)
        .map(|d| {
            d.fields
                .iter()
                .filter(|f| f.footprint().contains(&offset))
                .map(|f| f.name())
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() {
        "(no field)".into()
    } else {
        names.join("|")
    }
}

fn pointer_at(table: &str, offset: usize) -> bool {
    POINTER_FIELDS
        .iter()
        .any(|(t, o, w, _)| *t == table && (*o..*o + *w).contains(&offset))
}

fn pending_at(table: &str, offset: usize) -> bool {
    PENDING_TARGETS
        .iter()
        .any(|(_, t, (a, b))| *t == table && (*a..*b).contains(&offset))
}

#[derive(Default)]
struct Group {
    records: usize,
    bytes: usize,
    /// Record offsets of the differing bytes.
    span: Option<(usize, usize)>,
    last_record: Option<usize>,
    examples: Vec<(usize, usize, Vec<u8>, Vec<u8>)>,
}

/// Bytes where d2rs and the dump differ, by (pending, field label).
#[derive(Default)]
struct TableDiff {
    groups: BTreeMap<(bool, String), Group>,
    /// Bytes the game's fix-ups changed from the shipped `.bin`.
    game_changed: usize,
    /// Bytes `d2_data::fixup` changed.
    ours_changed: usize,
    /// Pointer-field bytes that differ from the shipped `.bin` (ignored).
    pointer_bytes: usize,
}

fn diff_table(name: &str, size: usize, ours: &[u8], game: &[u8], shipped: &[u8]) -> TableDiff {
    let mut d = TableDiff::default();
    let records = ours
        .chunks_exact(size)
        .zip(game.chunks_exact(size))
        .zip(shipped.chunks_exact(size));
    for (r, ((o, g), s)) in records.enumerate() {
        let mut k = 0;
        while k < size {
            if pointer_at(name, k) {
                d.pointer_bytes += usize::from(g[k] != s[k]);
                k += 1;
                continue;
            }
            d.game_changed += usize::from(g[k] != s[k]);
            d.ours_changed += usize::from(o[k] != s[k]);
            if o[k] == g[k] {
                k += 1;
                continue;
            }
            let label = field_label(name, k);
            let start = k;
            k += 1;
            while k < size && o[k] != g[k] && !pointer_at(name, k) && field_label(name, k) == label
            {
                d.game_changed += usize::from(g[k] != s[k]);
                d.ours_changed += usize::from(o[k] != s[k]);
                k += 1;
            }
            let grp = d
                .groups
                .entry((pending_at(name, start), label))
                .or_default();
            grp.bytes += k - start;
            let (lo, hi) = grp.span.unwrap_or((start, k));
            grp.span = Some((lo.min(start), hi.max(k)));
            if grp.last_record != Some(r) {
                grp.records += 1;
                grp.last_record = Some(r);
            }
            if grp.examples.len() < 3 {
                grp.examples
                    .push((r, start, o[start..k].to_vec(), g[start..k].to_vec()));
            }
        }
    }
    d
}

/// Maps d2rs builds, compared with the dumped ones; the rest are listed.
fn compare_maps(dir: &Path, fixed: &FixedSet, manifest: &BTreeMap<String, DumpEntry>) -> bool {
    let mut ok = true;
    println!("\nruntime maps (map-<name>.bin)");
    for (name, e) in manifest.iter().filter(|(_, e)| e.kind == "map") {
        let data = std::fs::read(dir.join(format!("map-{name}.bin"))).unwrap_or_default();
        let ours: Option<Vec<u8>> = match name.as_str() {
            "superunique_hc" => Some(
                fixed
                    .superunique_hc
                    .iter()
                    .flat_map(|v| v.unwrap_or(0xFFFF).to_le_bytes())
                    .collect(),
            ),
            "stat_stuff" => Some(fixed.stat_stuff.to_le_bytes().to_vec()),
            _ => None,
        };
        let result = match ours {
            None => "no d2rs counterpart".to_string(),
            Some(o) => {
                let g = &data[..o.len().min(data.len())];
                if g == o.as_slice() {
                    "identical".into()
                } else {
                    ok = false;
                    format!("DIFFERS: d2rs [{}] game [{}]", hex(&o), hex(g))
                }
            }
        };
        println!(
            "  {name:<22} {:>10} {:>6} x {:<2} {result}",
            e.address, e.count, e.size
        );
    }
    ok
}

fn dump_compare(dir: &Path, game: &Path) -> Result<()> {
    let manifest = read_manifest(dir)?;
    let set = ArchiveSet::open_dir(game).with_context(|| format!("opening {}", game.display()))?;
    let data = d2_data::bin::load(&set, d2_data::bin::DEFAULT_LANGUAGE).context("loading .bin")?;
    let fixed = fixup::apply(&data).context("fix-ups")?;
    println!(
        "dump {} vs live .bin + d2_data::fixup (loading.md §7.4, open question 15)",
        dir.display()
    );
    let ptrs: Vec<String> = POINTER_FIELDS
        .iter()
        .map(|(t, o, w, _)| format!("{t}+{o:#X}({w})"))
        .collect();
    println!("pointer fields excluded: {}", ptrs.join(", "));
    println!(
        "\n  {:<18} {:>6} {:>7} {:>7} {:>6}  result",
        "table", "count", "game d", "d2rs d", "ptr d"
    );
    let (mut identical, mut pending_only, mut bad) = (0, 0, 0);
    for (t, shipped) in fixed.tables.iter().zip(&data.tables) {
        let Some(e) = manifest.get(&t.name) else {
            println!("  {:<18} not in the dump", t.name);
            bad += 1;
            continue;
        };
        if e.kind == "freed" {
            println!(
                "  {:<18} {:>6}  records freed by 1.14d after conversion (maps only)",
                t.name, e.count
            );
            continue;
        }
        if e.count != t.count || e.size != t.record_size {
            println!(
                "  {:<18} MISMATCH: dump {} x {}, d2rs {} x {}",
                t.name, e.count, e.size, t.count, t.record_size
            );
            bad += 1;
            continue;
        }
        let game_bytes = std::fs::read(dir.join(format!("{}.bin", t.name)))?;
        if game_bytes.len() != t.records.len() {
            bail!(
                "{}.bin: {} bytes, expected {}",
                t.name,
                game_bytes.len(),
                t.records.len()
            );
        }
        let d = diff_table(
            &t.name,
            t.record_size,
            &t.records,
            &game_bytes,
            &shipped.records,
        );
        let sum = |pending: bool| -> usize {
            d.groups
                .iter()
                .filter(|(k, _)| k.0 == pending)
                .map(|(_, g)| g.bytes)
                .sum()
        };
        let (pend, unexp) = (sum(true), sum(false));
        let result = if d.groups.is_empty() {
            identical += 1;
            "identical".to_string()
        } else if unexp == 0 {
            pending_only += 1;
            format!("{pend} bytes differ, all in PENDING rows")
        } else {
            bad += 1;
            format!("DIFFERS: {unexp} bytes ({pend} more in PENDING rows)")
        };
        println!(
            "  {:<18} {:>6} {:>7} {:>7} {:>6}  {result}",
            t.name, t.count, d.game_changed, d.ours_changed, d.pointer_bytes
        );
        for ((pending, label), g) in &d.groups {
            let tag = if *pending { " [pending]" } else { "" };
            let (lo, hi) = g.span.unwrap_or_default();
            println!(
                "      {label}{tag}: {} bytes in {} records, offsets +{lo:#X}..+{hi:#X}",
                g.bytes, g.records
            );
            for (r, o, ours, game) in &g.examples {
                println!(
                    "        record {r} +{o:#X}: d2rs [{}] game [{}]",
                    hex(ours),
                    hex(game)
                );
            }
        }
    }
    let maps_ok = compare_maps(dir, &fixed, &manifest);
    println!(
        "\nsummary: {identical} identical, {pending_only} differ only in PENDING rows, \
         {bad} differ elsewhere; compared maps: {}",
        if maps_ok { "identical" } else { "DIFFER" }
    );
    println!(
        "(game d / d2rs d: bytes the game's / d2rs's fix-ups changed from the shipped .bin; \
         ptr d: pointer-field bytes, excluded)"
    );
    if bad > 0 || !maps_ok {
        std::process::exit(1);
    }
    Ok(())
}
