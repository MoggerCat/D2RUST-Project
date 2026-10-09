// Spec: traces/FORMAT.md §Scenario traces; specs/tools/scenario.md §4
//! Scenario traces (`scenario-trace` 1, JSON lines): the header, the
//! records, a strict reader and the canonical writer (sorted keys, no
//! spaces: `serde_json` maps are sorted without `preserve_order`).

use serde_json::{json, Map, Value};

use crate::raw::{decode_hex, encode_hex};
use crate::GAME_VERSION;

/// Header `format`.
pub const SCENARIO_TRACE_FORMAT: &str = "scenario-trace";
/// Header `version`.
pub const SCENARIO_TRACE_VERSION: u32 = 1;

/// Why a trace does not read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("trace line {line}: {message}")]
pub struct TraceReadError {
    /// 1-based line (0: the file as a whole).
    pub line: usize,
    pub message: String,
}

/// The header line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub side: String,
    pub tool: String,
    pub data: String,
    pub scenario: String,
    pub scenario_sha256: String,
    pub seed: u32,
    pub init: u32,
    pub end: u32,
    /// Sorted, unique.
    pub streams: Vec<String>,
    pub gaps: Vec<String>,
}

/// Stream names a header may list.
pub const STREAMS: [&str; 7] = ["c2s", "frames", "rng", "rng-draws", "s2c", "stats", "units"];

/// One record (FORMAT.md records table).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Record {
    C2s {
        t: u32,
        i: u32,
        /// The bytes injected, or the unresolved reference.
        bytes: Result<Vec<u8>, String>,
    },
    /// A `spawn` step (`scenario.md` §3.1): the spawned unit's GUID
    /// (the leader for packs), `Err` = the unresolved reference, or
    /// `Ok(None)` = the game placed nothing.
    Spawn {
        t: u32,
        i: u32,
        guid: Result<Option<u32>, String>,
    },
    /// A `poke` step (`tools/poke.md` §3 rule 3): the directive keyword,
    /// the result (`ok`, `failed`, `unresolved`, `gap`) and the created
    /// unit's GUID (only with `ok`).
    Poke {
        t: u32,
        i: u32,
        d: String,
        r: String,
        guid: Option<u32>,
    },
    S2c {
        t: u32,
        client: u32,
        bytes: Vec<u8>,
    },
    Rng {
        t: u32,
        before: [u32; 2],
        after: [u32; 2],
    },
    Draw {
        t: u32,
        n: u32,
        before: [u32; 2],
        after: [u32; 2],
        site: String,
    },
    Unit {
        t: u32,
        ty: u32,
        guid: u32,
        class: u32,
        mode: u32,
        x: i32,
        y: i32,
        life: i32,
        mana: i32,
    },
    Stats {
        t: u32,
        ty: u32,
        guid: u32,
        /// (stat, layer, value), ordered by (stat, layer).
        base: Vec<(u32, u32, i32)>,
    },
    End {
        t: u32,
    },
}

impl Record {
    pub fn tick(&self) -> u32 {
        match *self {
            Self::C2s { t, .. }
            | Self::Spawn { t, .. }
            | Self::Poke { t, .. }
            | Self::S2c { t, .. }
            | Self::Rng { t, .. }
            | Self::Draw { t, .. }
            | Self::Unit { t, .. }
            | Self::Stats { t, .. }
            | Self::End { t } => t,
        }
    }

    /// The `k` value.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::C2s { .. } => "c2s",
            Self::Spawn { .. } => "spawn",
            Self::Poke { .. } => "poke",
            Self::S2c { .. } => "s2c",
            Self::Rng { .. } => "rng",
            Self::Draw { .. } => "draw",
            Self::Unit { .. } => "unit",
            Self::Stats { .. } => "stats",
            Self::End { .. } => "end",
        }
    }

    /// Position of the kind within a tick (FORMAT.md: `c2s`, `spawn`,
    /// `poke`, `s2c`, `rng`, `draw`, `unit`, `stats`; `end` last).
    pub fn order(&self) -> usize {
        KINDS
            .iter()
            .position(|k| *k == self.kind())
            .unwrap_or(KINDS.len())
    }

    pub fn to_json(&self) -> Value {
        match self {
            Self::C2s { t, i, bytes } => match bytes {
                Ok(b) => json!({"k": "c2s", "t": t, "i": i, "b": encode_hex(b)}),
                Err(r) => json!({"k": "c2s", "t": t, "i": i, "unresolved": r}),
            },
            Self::Spawn { t, i, guid } => match guid {
                Ok(Some(g)) => json!({"k": "spawn", "t": t, "i": i, "guid": g}),
                Ok(None) => json!({"k": "spawn", "t": t, "i": i, "failed": true}),
                Err(r) => json!({"k": "spawn", "t": t, "i": i, "unresolved": r}),
            },
            Self::Poke { t, i, d, r, guid } => match guid {
                Some(g) => json!({"k": "poke", "t": t, "i": i, "d": d, "r": r, "guid": g}),
                None => json!({"k": "poke", "t": t, "i": i, "d": d, "r": r}),
            },
            Self::S2c { t, client, bytes } => {
                json!({"k": "s2c", "t": t, "c": client, "b": encode_hex(bytes)})
            }
            Self::Rng { t, before, after } => {
                json!({"k": "rng", "t": t, "before": before, "after": after})
            }
            Self::Draw {
                t,
                n,
                before,
                after,
                site,
            } => {
                json!({"k": "draw", "t": t, "n": n, "before": before, "after": after, "site": site})
            }
            Self::Unit {
                t,
                ty,
                guid,
                class,
                mode,
                x,
                y,
                life,
                mana,
            } => json!({"k": "unit", "t": t, "type": ty, "guid": guid, "class": class,
                "mode": mode, "x": x, "y": y, "life": life, "mana": mana}),
            Self::Stats { t, ty, guid, base } => {
                let base: Vec<Value> = base.iter().map(|&(s, l, v)| json!([s, l, v])).collect();
                json!({"k": "stats", "t": t, "type": ty, "guid": guid, "base": base})
            }
            Self::End { t } => json!({"k": "end", "t": t}),
        }
    }

    pub fn from_json(v: &Value) -> Result<Self, String> {
        let o = v.as_object().ok_or("not an object")?;
        let k = o.get("k").and_then(Value::as_str).ok_or("no k")?;
        let r = Fields(o);
        let (rec, keys): (Self, &[&str]) = match k {
            "c2s" => {
                let bytes = match (o.get("b"), o.get("unresolved")) {
                    (Some(_), None) => Ok(r.bytes("b")?),
                    (None, Some(u)) => {
                        Err(u.as_str().ok_or("unresolved: not a string")?.to_owned())
                    }
                    _ => return Err("c2s needs exactly one of b, unresolved".into()),
                };
                let keys: &[&str] = if bytes.is_ok() {
                    &["k", "t", "i", "b"]
                } else {
                    &["k", "t", "i", "unresolved"]
                };
                (
                    Self::C2s {
                        t: r.u32("t")?,
                        i: r.u32("i")?,
                        bytes,
                    },
                    keys,
                )
            }
            "spawn" => {
                let (guid, key) = match (o.get("guid"), o.get("failed"), o.get("unresolved")) {
                    (Some(_), None, None) => (Ok(Some(r.u32("guid")?)), "guid"),
                    (None, Some(Value::Bool(true)), None) => (Ok(None), "failed"),
                    (None, None, Some(u)) => (
                        Err(u.as_str().ok_or("unresolved: not a string")?.to_owned()),
                        "unresolved",
                    ),
                    _ => {
                        return Err(
                            "spawn needs exactly one of guid, failed: true, unresolved".into()
                        )
                    }
                };
                let keys: &[&str] = match key {
                    "guid" => &["k", "t", "i", "guid"],
                    "failed" => &["k", "t", "i", "failed"],
                    _ => &["k", "t", "i", "unresolved"],
                };
                (
                    Self::Spawn {
                        t: r.u32("t")?,
                        i: r.u32("i")?,
                        guid,
                    },
                    keys,
                )
            }
            "poke" => {
                let r_ = r.str("r")?;
                if !POKE_RESULTS.contains(&r_.as_str()) {
                    return Err(format!("poke r {r_:?}: one of {}", POKE_RESULTS.join(", ")));
                }
                let guid = match o.get("guid") {
                    None => None,
                    Some(_) if r_ == "ok" => Some(r.u32("guid")?),
                    Some(_) => return Err("poke guid only with r ok".into()),
                };
                let keys: &[&str] = if guid.is_some() {
                    &["k", "t", "i", "d", "r", "guid"]
                } else {
                    &["k", "t", "i", "d", "r"]
                };
                (
                    Self::Poke {
                        t: r.u32("t")?,
                        i: r.u32("i")?,
                        d: r.str("d")?,
                        r: r_,
                        guid,
                    },
                    keys,
                )
            }
            "s2c" => (
                Self::S2c {
                    t: r.u32("t")?,
                    client: r.u32("c")?,
                    bytes: r.bytes("b")?,
                },
                &["k", "t", "c", "b"],
            ),
            "rng" => (
                Self::Rng {
                    t: r.u32("t")?,
                    before: r.pair("before")?,
                    after: r.pair("after")?,
                },
                &["k", "t", "before", "after"],
            ),
            "draw" => (
                Self::Draw {
                    t: r.u32("t")?,
                    n: r.u32("n")?,
                    before: r.pair("before")?,
                    after: r.pair("after")?,
                    site: r.str("site")?,
                },
                &["k", "t", "n", "before", "after", "site"],
            ),
            "unit" => (
                Self::Unit {
                    t: r.u32("t")?,
                    ty: r.u32("type")?,
                    guid: r.u32("guid")?,
                    class: r.u32("class")?,
                    mode: r.u32("mode")?,
                    x: r.i32("x")?,
                    y: r.i32("y")?,
                    life: r.i32("life")?,
                    mana: r.i32("mana")?,
                },
                &[
                    "k", "t", "type", "guid", "class", "mode", "x", "y", "life", "mana",
                ],
            ),
            "stats" => {
                let base = o
                    .get("base")
                    .and_then(Value::as_array)
                    .ok_or("base: not an array")?
                    .iter()
                    .map(|e| {
                        let a = e
                            .as_array()
                            .filter(|a| a.len() == 3)
                            .ok_or("base entry: [stat, layer, value]")?;
                        let u = |v: &Value| v.as_u64().and_then(|x| u32::try_from(x).ok());
                        let s = u(&a[0]).ok_or("base stat")?;
                        let l = u(&a[1]).ok_or("base layer")?;
                        let v = a[2]
                            .as_i64()
                            .and_then(|x| i32::try_from(x).ok())
                            .ok_or("base value")?;
                        Ok::<_, String>((s, l, v))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                (
                    Self::Stats {
                        t: r.u32("t")?,
                        ty: r.u32("type")?,
                        guid: r.u32("guid")?,
                        base,
                    },
                    &["k", "t", "type", "guid", "base"],
                )
            }
            "end" => (Self::End { t: r.u32("t")? }, &["k", "t"]),
            k => return Err(format!("unknown record kind {k:?}")),
        };
        if let Some(extra) = o.keys().find(|key| !keys.contains(&key.as_str())) {
            return Err(format!("{k}: unknown field {extra:?}"));
        }
        Ok(rec)
    }
}

/// Record kinds in their within-tick order.
pub const KINDS: [&str; 8] = [
    "c2s", "spawn", "poke", "s2c", "rng", "draw", "unit", "stats",
];

/// A `poke` record's `r` values (`tools/poke.md` §3 rule 3).
pub const POKE_RESULTS: [&str; 4] = ["ok", "failed", "unresolved", "gap"];

struct Fields<'a>(&'a Map<String, Value>);

impl Fields<'_> {
    fn get(&self, k: &str) -> Result<&Value, String> {
        self.0.get(k).ok_or_else(|| format!("missing field {k}"))
    }
    fn u32(&self, k: &str) -> Result<u32, String> {
        self.get(k)?
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| format!("{k}: not a u32"))
    }
    fn i32(&self, k: &str) -> Result<i32, String> {
        self.get(k)?
            .as_i64()
            .and_then(|v| i32::try_from(v).ok())
            .ok_or_else(|| format!("{k}: not an i32"))
    }
    fn str(&self, k: &str) -> Result<String, String> {
        self.get(k)?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{k}: not a string"))
    }
    fn bytes(&self, k: &str) -> Result<Vec<u8>, String> {
        let s = self.str(k)?;
        if s.bytes().any(|b| b.is_ascii_uppercase()) {
            return Err(format!("{k}: hex must be lower case"));
        }
        if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("{k}: bad hex"));
        }
        decode_hex(&s).ok_or_else(|| format!("{k}: bad hex"))
    }
    fn pair(&self, k: &str) -> Result<[u32; 2], String> {
        let a = self
            .get(k)?
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or_else(|| format!("{k}: not [lo, hi]"))?;
        let u = |v: &Value| v.as_u64().and_then(|x| u32::try_from(x).ok());
        match (u(&a[0]), u(&a[1])) {
            (Some(lo), Some(hi)) => Ok([lo, hi]),
            _ => Err(format!("{k}: not [lo, hi]")),
        }
    }
}

impl Header {
    pub fn to_json(&self) -> Value {
        json!({
            "k": "header",
            "format": SCENARIO_TRACE_FORMAT,
            "version": SCENARIO_TRACE_VERSION,
            "game_version": GAME_VERSION,
            "side": self.side,
            "tool": self.tool,
            "data": self.data,
            "scenario": self.scenario,
            "scenario_sha256": self.scenario_sha256,
            "seed": self.seed,
            "init": self.init,
            "end": self.end,
            "streams": self.streams,
            "gaps": self.gaps,
        })
    }

    pub fn from_json(v: &Value) -> Result<Self, String> {
        let o = v.as_object().ok_or("header: not an object")?;
        let r = Fields(o);
        if r.str("k")? != "header" {
            return Err("the first line is not the header".into());
        }
        let format = r.str("format")?;
        if format != SCENARIO_TRACE_FORMAT {
            return Err(format!(
                "format {format:?}, expected {SCENARIO_TRACE_FORMAT:?}"
            ));
        }
        let version = r.u32("version")?;
        if version != SCENARIO_TRACE_VERSION {
            return Err(format!("unknown scenario-trace version {version}"));
        }
        let game = r.str("game_version")?;
        if game != GAME_VERSION {
            return Err(format!("game_version {game:?}, only {GAME_VERSION:?}"));
        }
        let side = r.str("side")?;
        if side != "original" && side != "d2rs" {
            return Err(format!("side {side:?}: original or d2rs"));
        }
        let strings = |k: &str| -> Result<Vec<String>, String> {
            r.get(k)?
                .as_array()
                .ok_or_else(|| format!("{k}: not an array"))?
                .iter()
                .map(|s| {
                    s.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("{k}: not strings"))
                })
                .collect()
        };
        let streams = strings("streams")?;
        if let Some(s) = streams.iter().find(|s| !STREAMS.contains(&s.as_str())) {
            return Err(format!("unknown stream {s:?}"));
        }
        if !streams.windows(2).all(|w| w[0] < w[1]) || !streams.iter().any(|s| s == "c2s") {
            return Err("streams: sorted, unique, c2s included".into());
        }
        const KEYS: [&str; 14] = [
            "k",
            "format",
            "version",
            "game_version",
            "side",
            "tool",
            "data",
            "scenario",
            "scenario_sha256",
            "seed",
            "init",
            "end",
            "streams",
            "gaps",
        ];
        if let Some(extra) = o.keys().find(|k| !KEYS.contains(&k.as_str())) {
            return Err(format!("header: unknown field {extra:?}"));
        }
        Ok(Self {
            side,
            tool: r.str("tool")?,
            data: r.str("data")?,
            scenario: r.str("scenario")?,
            scenario_sha256: r.str("scenario_sha256")?,
            seed: r.u32("seed")?,
            init: r.u32("init")?,
            end: r.u32("end")?,
            streams,
            gaps: strings("gaps")?,
        })
    }

    pub fn has(&self, stream: &str) -> bool {
        self.streams.iter().any(|s| s == stream)
    }
}

/// A whole trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceFile {
    pub header: Header,
    /// In file order; the last is [`Record::End`].
    pub records: Vec<Record>,
}

impl TraceFile {
    /// The canonical JSON-lines text.
    pub fn to_text(&self) -> String {
        let mut s = self.header.to_json().to_string();
        s.push('\n');
        for r in &self.records {
            s.push_str(&r.to_json().to_string());
            s.push('\n');
        }
        s
    }

    /// Reads a trace: the header, records in tick and kind order, the
    /// `end` record last with the header's `end`.
    pub fn parse(text: &str) -> Result<Self, TraceReadError> {
        let mut lines = text.lines().enumerate();
        let err = |line: usize, message: String| TraceReadError { line, message };
        let (_, first) = lines.next().ok_or_else(|| err(0, "empty trace".into()))?;
        let v: Value = serde_json::from_str(first).map_err(|e| err(1, e.to_string()))?;
        let header = Header::from_json(&v).map_err(|m| err(1, m))?;
        let mut records: Vec<Record> = Vec::new();
        for (k, l) in lines {
            let line = k + 1;
            if matches!(records.last(), Some(Record::End { .. })) {
                return Err(err(line, "a record after the end record".into()));
            }
            let v: Value = serde_json::from_str(l).map_err(|e| err(line, e.to_string()))?;
            let r = Record::from_json(&v).map_err(|m| err(line, m))?;
            if r.tick() > header.end {
                return Err(err(
                    line,
                    format!("tick {} after end {}", r.tick(), header.end),
                ));
            }
            if let Some(p) = records.last() {
                if (r.tick(), r.order()) < (p.tick(), p.order()) {
                    return Err(err(
                        line,
                        format!(
                            "{} at tick {} after {} at tick {}",
                            r.kind(),
                            r.tick(),
                            p.kind(),
                            p.tick()
                        ),
                    ));
                }
            }
            if let Record::End { t } = r {
                if t != header.end {
                    return Err(err(
                        line,
                        format!("end record at {t}, header end {}", header.end),
                    ));
                }
            }
            records.push(r);
        }
        if !matches!(records.last(), Some(Record::End { .. })) {
            return Err(err(0, "no end record: the run did not finish".into()));
        }
        Ok(Self { header, records })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> TraceFile {
        TraceFile {
            header: Header {
                side: "d2rs".into(),
                tool: "test".into(),
                data: "synthetic".into(),
                scenario: "t".into(),
                scenario_sha256: "00".into(),
                seed: 1,
                init: 7,
                end: 2,
                streams: vec!["c2s".into(), "rng".into(), "s2c".into(), "units".into()],
                gaps: vec![],
            },
            records: vec![
                Record::C2s {
                    t: 0,
                    i: 0,
                    bytes: Ok(vec![1, 2]),
                },
                Record::C2s {
                    t: 0,
                    i: 1,
                    bytes: Err("@1".into()),
                },
                Record::Spawn {
                    t: 0,
                    i: 2,
                    guid: Ok(Some(9)),
                },
                Record::Spawn {
                    t: 0,
                    i: 3,
                    guid: Ok(None),
                },
                Record::Spawn {
                    t: 0,
                    i: 4,
                    guid: Err("@x".into()),
                },
                Record::Poke {
                    t: 0,
                    i: 5,
                    d: "object".into(),
                    r: "ok".into(),
                    guid: Some(12),
                },
                Record::Poke {
                    t: 0,
                    i: 6,
                    d: "warp".into(),
                    r: "gap".into(),
                    guid: None,
                },
                Record::S2c {
                    t: 0,
                    client: 0,
                    bytes: vec![0x2A, 0xFF],
                },
                Record::Rng {
                    t: 0,
                    before: [1, 666],
                    after: [3, 4],
                },
                Record::Unit {
                    t: 2,
                    ty: 0,
                    guid: 1,
                    class: 1,
                    mode: 1,
                    x: -5,
                    y: 6,
                    life: 256,
                    mana: 0,
                },
                Record::Stats {
                    t: 2,
                    ty: 0,
                    guid: 1,
                    base: vec![(0, 0, 10), (6, 0, -1)],
                },
                Record::End { t: 2 },
            ],
        }
    }

    #[test]
    fn text_round_trips() {
        let t = sample();
        let text = t.to_text();
        assert_eq!(TraceFile::parse(&text).unwrap(), t);
        assert!(text
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("{\"b\":\"0102\",\"i\":0,\"k\":\"c2s\""));
    }

    #[test]
    fn strict_reader() {
        let text = sample().to_text();
        let lines: Vec<&str> = text.lines().collect();
        let without_end = lines[..lines.len() - 1].join("\n");
        assert!(TraceFile::parse(&without_end)
            .unwrap_err()
            .message
            .contains("no end record"));
        let swapped = [lines[0], lines[3], lines[1]].join("\n");
        assert_eq!(TraceFile::parse(&swapped).unwrap_err().line, 3);
        let extra = text.replacen("\"k\":\"rng\"", "\"k\":\"rng\",\"z\":1", 1);
        assert!(TraceFile::parse(&extra)
            .unwrap_err()
            .message
            .contains("unknown field"));
        let v2 = text.replacen("\"version\":1", "\"version\":2", 1);
        assert!(TraceFile::parse(&v2)
            .unwrap_err()
            .message
            .contains("version 2"));
        let bad_r = text.replacen("\"r\":\"gap\"", "\"r\":\"maybe\"", 1);
        assert!(TraceFile::parse(&bad_r)
            .unwrap_err()
            .message
            .contains("poke r"));
        let gap_guid = text.replacen("\"r\":\"gap\"", "\"guid\":3,\"r\":\"gap\"", 1);
        assert!(TraceFile::parse(&gap_guid)
            .unwrap_err()
            .message
            .contains("only with r ok"));
        let upper = text.replacen("\"b\":\"2aff\"", "\"b\":\"2AFF\"", 1);
        assert!(TraceFile::parse(&upper)
            .unwrap_err()
            .message
            .contains("lower case"));
    }
}
