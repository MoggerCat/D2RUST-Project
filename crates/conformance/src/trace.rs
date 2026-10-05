//! Trace files: parsing and the top-level checks from `traces/FORMAT.md`.

use std::path::Path;

use serde_json::Value;

use crate::{GAME_VERSION, TRACE_FORMAT_VERSION};

/// Why a trace could not be read or did not replay.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("parsing {path}: {source}")]
    Json {
        path: String,
        source: serde_json::Error,
    },
    #[error("{0}")]
    Format(String),
    #[error("trace {id}, expected[{index}]: {message}")]
    Mismatch {
        id: String,
        index: usize,
        message: String,
    },
}

/// One parsed trace. Top-level fields are checked on load; behavior-specific
/// parts (`setup`, event `data`) stay as JSON for the replayer.
#[derive(Debug, Clone)]
pub struct Trace {
    pub id: String,
    pub area: String,
    pub behavior: String,
    pub spec: String,
    pub setup: Value,
    pub inputs: Vec<Value>,
    pub expected: Vec<Value>,
    /// `compare.mode`, default `"exact"`.
    pub compare_mode: String,
    /// `compare.ignore`: paths within event `data` to skip.
    pub ignore: Vec<String>,
}

impl Trace {
    /// Reads and checks a trace file.
    pub fn load(path: &Path) -> Result<Self, TraceError> {
        let shown = path.display().to_string();
        let text = std::fs::read_to_string(path).map_err(|source| TraceError::Io {
            path: shown.clone(),
            source,
        })?;
        let json: Value = serde_json::from_str(&text).map_err(|source| TraceError::Json {
            path: shown.clone(),
            source,
        })?;
        let trace =
            Self::from_json(&json).map_err(|m| TraceError::Format(format!("{shown}: {m}")))?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if stem != trace.id {
            return Err(TraceError::Format(format!(
                "{shown}: id {:?} does not match the file name",
                trace.id
            )));
        }
        Ok(trace)
    }

    /// Checks the top-level object (`traces/FORMAT.md`, "Top-level object").
    pub fn from_json(json: &Value) -> Result<Self, String> {
        let version = json["format_version"]
            .as_u64()
            .ok_or("missing format_version")?;
        if version != u64::from(TRACE_FORMAT_VERSION) {
            return Err(format!("unknown format_version {version}"));
        }
        let text = |key: &str| {
            json[key]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("missing or non-string {key}"))
        };
        let game = text("game_version")?;
        if game != GAME_VERSION {
            return Err(format!(
                "game_version {game:?}, only {GAME_VERSION:?} is accepted"
            ));
        }
        if !json["recorded"].is_object() || !json["setup"].is_object() {
            return Err("missing recorded or setup object".into());
        }
        let array = |key: &str| {
            json[key]
                .as_array()
                .cloned()
                .ok_or_else(|| format!("missing array {key}"))
        };
        let compare = &json["compare"];
        let compare_mode = compare["mode"].as_str().unwrap_or("exact").to_owned();
        let ignore = match &compare["ignore"] {
            Value::Null => Vec::new(),
            Value::Array(a) => a
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or("non-string compare.ignore entry")
                })
                .collect::<Result<_, _>>()?,
            _ => return Err("compare.ignore is not an array".into()),
        };
        Ok(Self {
            id: text("id")?,
            area: text("area")?,
            behavior: text("behavior")?,
            spec: text("spec")?,
            setup: json["setup"].clone(),
            inputs: array("inputs")?,
            expected: array("expected")?,
            compare_mode,
            ignore,
        })
    }
}
