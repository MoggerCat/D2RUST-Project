// Spec: specs/formats/native-assets.md §4.5 r3
//! `report.txt`: human-readable, not parsed. The only place a date or a
//! run time may appear (§4.2 r1).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

use d2_native::manifest::Manifest;

use crate::fsutil::utc_string;

pub struct Report<'a> {
    pub started_at: u64,
    pub install: String,
    pub manifest: &'a Manifest,
    /// (path, check, first difference).
    pub failures: &'a [(String, String, String)],
    /// (path, line) from the kinds (DT1 fallback tiles, `tbl` rebuilds).
    pub notes: &'a [(String, String)],
    pub unconverted: &'a BTreeMap<String, u64>,
    pub rejected_names: usize,
    pub kind_time: &'a BTreeMap<String, Duration>,
    pub wall: Duration,
}

pub fn render(r: &Report) -> String {
    let m = r.manifest;
    let mut s = String::new();
    let _ = writeln!(s, "d2-convert report");
    let _ = writeln!(s, "run date:  {}", utc_string(r.started_at));
    let _ = writeln!(s, "install:   {}", r.install);
    let _ = writeln!(
        s,
        "converter: {} ({})",
        m.converter_version, m.converter_commit
    );
    let _ = writeln!(s, "complete:  {}", m.complete);
    let _ = writeln!(s, "\nper kind (converted / failed / skipped, work seconds)");
    for (k, c) in &m.counts {
        let t = r.kind_time.get(k).map_or(0.0, Duration::as_secs_f64);
        let _ = writeln!(
            s,
            "  {k:<10} {:>7} {:>6} {:>6}   {t:.1}",
            c.converted, c.failed, c.skipped
        );
    }
    let _ = writeln!(s, "total wall time: {:.1}s", r.wall.as_secs_f64());
    let _ = writeln!(s, "\nfailures: {}", r.failures.len());
    for (path, check, detail) in r.failures {
        let _ = writeln!(s, "  {path}: {check}: {detail}");
    }
    let _ = writeln!(s, "\nnotes: {}", r.notes.len());
    for (path, line) in r.notes {
        let _ = writeln!(s, "  {path}: {line}");
    }
    let _ = writeln!(s, "\nunnamed blocks per archive (not converted)");
    for (a, n) in &m.unnamed_blocks {
        let _ = writeln!(s, "  {a}: {n}");
    }
    let _ = writeln!(
        s,
        "\nnames that cannot be native paths: {}",
        r.rejected_names
    );
    let _ = writeln!(s, "\nunconverted extensions (files, no reader)");
    for (e, n) in r.unconverted {
        match crate::kinds::SKIPPED_EXTENSIONS
            .iter()
            .find(|(x, _)| x == e)
        {
            Some((_, why)) => {
                let _ = writeln!(s, "  .{e}: {n} (skipped: {why})");
            }
            None => {
                let _ = writeln!(s, "  .{e}: {n}");
            }
        }
    }
    s
}
