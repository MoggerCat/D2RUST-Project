//! Raw recorder files: the JSON-lines output of `tools/trace-recorder`
//! (`traces/raw/*.jsonl`, gitignored; formats in
//! `tools/trace-recorder/README.md`), read by the harnesses whose
//! recordings have no format-1 converter yet (units, stats, packets).
//!
//! Each format names itself in its first line (`header`, field
//! `format`); a file whose header names another format, or none, is
//! rejected, as `traces/FORMAT.md` rejects an unknown `format_version`
//! (METHODS M20). Record indexes count lines from 0 (the header), the
//! numbering the Python checkers report, so a mismatch found here and one
//! found by `check_*.py` name the same record.

use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::TraceError;

/// Tick recordings (`record_tick.py`, key `k`): units harness.
pub const TICK_RAW: &str = "tick-raw-1";
/// Stat-list recordings (`record_stats.py`, key `k`): stats harness.
pub const STATS_RAW: &str = "stats-raw-1";
/// Message recordings (`record_packets.py`, key `type`): packets harness.
pub const PACKETS_RAW: &str = "packets-raw-1";

/// One raw recording, header checked.
#[derive(Clone, Debug)]
pub struct RawRecording {
    /// Where it came from (file path, or a fixture name).
    pub name: String,
    /// The header's `format`.
    pub format: String,
    /// Every line, header (index 0) and footer included.
    pub records: Vec<Value>,
}

impl RawRecording {
    /// Reads a JSON-lines file and checks that its header names `format`.
    pub fn load(path: &Path, format: &str) -> Result<Self, TraceError> {
        let shown = path.display().to_string();
        let text = std::fs::read_to_string(path).map_err(|source| TraceError::Io {
            path: shown.clone(),
            source,
        })?;
        Self::parse(&shown, &text, format)
    }

    /// Parses JSON-lines text (blank lines skipped, as the checkers do).
    pub fn parse(name: &str, text: &str, format: &str) -> Result<Self, TraceError> {
        let mut records = Vec::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let v: Value = serde_json::from_str(line).map_err(|source| TraceError::Json {
                path: format!("{name}, record {}", records.len()),
                source,
            })?;
            records.push(v);
        }
        Self::from_records(name, records, format)
    }

    /// Checks the header of already parsed records.
    pub fn from_records(name: &str, records: Vec<Value>, format: &str) -> Result<Self, TraceError> {
        let header = records.first().ok_or_else(|| {
            TraceError::Format(format!("{name}: empty recording, expected {format}"))
        })?;
        let kind = header.get("k").or_else(|| header.get("type"));
        if kind.and_then(Value::as_str) != Some("header") {
            return Err(TraceError::Format(format!(
                "{name}: first record is not a header"
            )));
        }
        let got = header["format"].as_str().unwrap_or("(none)");
        if got != format {
            return Err(TraceError::Format(format!(
                "{name}: format {got:?}, this harness reads {format:?}"
            )));
        }
        Ok(Self {
            name: name.to_owned(),
            format: got.to_owned(),
            records,
        })
    }
}

/// `traces/raw/`, or `$D2_TRACES_RAW` when set (a local session can point
/// the file-driven tests at another directory).
pub fn raw_dir() -> PathBuf {
    match std::env::var_os("D2_TRACES_RAW") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../traces/raw"),
    }
}

/// The recordings in [`raw_dir`] whose name ends in `suffix` (e.g.
/// `-stats.jsonl`), sorted by name (= recording time). Empty when the
/// directory does not exist (cloud sessions, CI).
pub fn raw_files(suffix: &str) -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(raw_dir()) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = dir
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(suffix))
        })
        .collect();
    out.sort();
    out
}

/// The first record where a replay and the recording part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    /// The recording or trace.
    pub source: String,
    /// The record: line index for raw recordings (as `check_*.py`
    /// report it), `seq` for packets and format-1 traces.
    pub at: u64,
    /// The differing field or quantity (e.g. `expire`, `bytes[3]`,
    /// `full[7/0]`, `adj[2]`).
    pub field: String,
    pub detail: String,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: record {}: {}: {}",
            self.source, self.at, self.field, self.detail
        )
    }
}

/// Why a harness did not pass.
#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    /// The input could not be read or is not the expected format.
    #[error(transparent)]
    Trace(#[from] TraceError),
    /// The replay disagrees with the recording.
    #[error("{0}")]
    Mismatch(Mismatch),
}

impl HarnessError {
    /// The mismatch, if this is one.
    pub fn mismatch(&self) -> Option<&Mismatch> {
        match self {
            HarnessError::Mismatch(m) => Some(m),
            HarnessError::Trace(_) => None,
        }
    }
}

/// A format error at record `i` of `name`.
pub(crate) fn bad(name: &str, i: usize, message: impl fmt::Display) -> HarnessError {
    HarnessError::Trace(TraceError::Format(format!("{name}: record {i}: {message}")))
}

/// Field accessors that turn a missing or mistyped field into a format
/// error naming the record.
pub(crate) struct Fields<'a> {
    pub name: &'a str,
    pub i: usize,
    pub r: &'a Value,
}

impl Fields<'_> {
    pub fn i64(&self, key: &str) -> Result<i64, HarnessError> {
        self.r[key].as_i64().ok_or_else(|| {
            bad(
                self.name,
                self.i,
                format!("field {key:?} is not an integer"),
            )
        })
    }

    pub fn i32(&self, key: &str) -> Result<i32, HarnessError> {
        let v = self.i64(key)?;
        i32::try_from(v).map_err(|_| bad(self.name, self.i, format!("field {key:?} = {v}")))
    }

    pub fn u32(&self, key: &str) -> Result<u32, HarnessError> {
        let v = self.i64(key)?;
        u32::try_from(v).map_err(|_| bad(self.name, self.i, format!("field {key:?} = {v}")))
    }

    pub fn str(&self, key: &str) -> Result<&str, HarnessError> {
        self.r[key]
            .as_str()
            .ok_or_else(|| bad(self.name, self.i, format!("field {key:?} is not a string")))
    }

    pub fn bool(&self, key: &str) -> Result<bool, HarnessError> {
        match &self.r[key] {
            Value::Bool(b) => Ok(*b),
            // The recorders write some flags as 0 / 1.
            Value::Number(n) if n.as_i64() == Some(0) => Ok(false),
            Value::Number(n) if n.as_i64() == Some(1) => Ok(true),
            _ => Err(bad(
                self.name,
                self.i,
                format!("field {key:?} is not a flag"),
            )),
        }
    }

    /// Bytes from a hex string without prefix (`"2a0100"`).
    pub fn bytes(&self, key: &str) -> Result<Vec<u8>, HarnessError> {
        let s = self.r[key].as_str().unwrap_or("");
        decode_hex(s).ok_or_else(|| bad(self.name, self.i, format!("field {key:?} is not hex")))
    }
}

/// `"0x…"` (or bare hex digits) to u32.
pub(crate) fn parse_hex(s: &str) -> Option<u32> {
    let t = s.strip_prefix("0x").unwrap_or(s);
    u32::from_str_radix(t, 16).ok()
}

pub(crate) fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

pub(crate) fn encode_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_format_is_checked() {
        let ok = "{\"k\":\"header\",\"format\":\"tick-raw-1\"}\n\n{\"k\":\"tick\",\"f\":1}\n";
        let r = RawRecording::parse("t", ok, TICK_RAW).unwrap();
        assert_eq!(r.records.len(), 2);
        let other = RawRecording::parse("t", ok, STATS_RAW).unwrap_err();
        assert!(other.to_string().contains("this harness reads"), "{other}");
        let none = RawRecording::parse("t", "{\"k\":\"tick\"}\n", TICK_RAW).unwrap_err();
        assert!(none.to_string().contains("not a header"), "{none}");
        let typed = "{\"type\":\"header\",\"format\":\"packets-raw-1\"}\n";
        RawRecording::parse("p", typed, PACKETS_RAW).unwrap();
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(decode_hex("2a01ff"), Some(vec![0x2a, 1, 0xff]));
        assert_eq!(decode_hex("2a0"), None);
        assert_eq!(encode_hex(&[0x2a, 1, 0xff]), "2a01ff");
        assert_eq!(parse_hex("0x553b10"), Some(0x553b10));
    }
}
