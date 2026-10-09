// Spec: specs/tools/scenario.md §5, §6
//! The comparator: two scenario traces (the original's and d2rs') →
//! the first divergence and a summary.

use std::collections::BTreeMap;
use std::fmt;

use super::trace::{Record, TraceFile, KINDS};
use crate::raw::encode_hex;

/// The mask table (`specs/tools/scenario-masks.tsv`).
pub const MASKS_TSV: &str = include_str!("../../../../specs/tools/scenario-masks.tsv");

/// The key a mask row is restricted to: bytes every form of the id
/// writes (`sim/intents-events.md` §6 rule 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub offset: usize,
    /// 1 (`u8@`) or 2 (`u16@`, little-endian).
    pub width: usize,
    pub value: u16,
}

impl Key {
    fn holds(&self, m: &[u8]) -> bool {
        let v = match self.width {
            1 => m.get(self.offset).map(|&b| b as u16),
            _ => m
                .get(self.offset..self.offset + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]])),
        };
        v == Some(self.value)
    }
}

/// Where a masked range starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    At(usize),
    /// `nul@n`: the byte after the first 0 byte at or after n; no 0 in
    /// n…last → nothing masked.
    AfterNul(usize),
}

/// How far a masked range reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Length {
    Bytes(usize),
    /// `*`: to the end of the message.
    ToEnd,
    /// `..n`: through byte n.
    Through(usize),
}

/// One masked byte range of an S→C id (§6 rule 1; keyed rows:
/// `sim/intents-events.md` §6 rule 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mask {
    pub id: u8,
    pub key: Option<Key>,
    pub start: Start,
    pub len: Length,
}

impl Mask {
    /// The masked offsets of message `m` (an empty range when the key
    /// does not hold).
    fn range(&self, m: &[u8]) -> std::ops::Range<usize> {
        if m.first() != Some(&self.id) || self.key.is_some_and(|k| !k.holds(m)) {
            return 0..0;
        }
        let s = match self.start {
            Start::At(o) => o,
            Start::AfterNul(n) => match m.iter().skip(n).position(|&b| b == 0) {
                Some(p) => n + p + 1,
                None => return 0..0,
            },
        };
        let e = match self.len {
            Length::Bytes(l) => s + l,
            Length::ToEnd => usize::MAX,
            Length::Through(n) => n + 1,
        };
        s..e.max(s)
    }
}

fn parse_hex(t: &str, max: u32) -> Option<u32> {
    t.strip_prefix("0x")
        .and_then(|h| u32::from_str_radix(h, 16).ok())
        .filter(|&v| v <= max)
}

fn parse_key(t: &str) -> Option<Option<Key>> {
    if t == "-" {
        return Some(None);
    }
    let (width, rest, max) = match t.strip_prefix("u8@") {
        Some(r) => (1, r, 0xFF),
        None => (2, t.strip_prefix("u16@")?, 0xFFFF),
    };
    let (off, v) = rest.split_once('=')?;
    Some(Some(Key {
        offset: off.parse().ok()?,
        width,
        value: parse_hex(v, max)? as u16,
    }))
}

/// Reads a mask table strictly (§6 rule 3; columns per
/// `sim/intents-events.md` §6 rule 6).
pub fn parse_masks(tsv: &str) -> Result<Vec<Mask>, String> {
    let mut lines = tsv.lines().enumerate();
    match lines.next() {
        Some((_, "id\tkey\toffset\tlength\tsource")) => {}
        _ => return Err("line 1: header id, key, offset, length, source".into()),
    }
    let mut out = Vec::new();
    for (k, l) in lines {
        let line = k + 1;
        let c: Vec<&str> = l.split('\t').collect();
        let [id, key, off, len, source] = c.as_slice() else {
            return Err(format!("line {line}: five columns"));
        };
        let id = parse_hex(id, 0xB4)
            .ok_or_else(|| format!("line {line}: id {id:?} is not an S→C id"))?
            as u8;
        let key = parse_key(key).ok_or_else(|| format!("line {line}: key {key:?}"))?;
        let start = match off.strip_prefix("nul@") {
            Some(n) => n.parse().map(Start::AfterNul),
            None => off.parse().map(Start::At),
        }
        .map_err(|_| format!("line {line}: offset {off:?}"))?;
        let bad_len = || format!("line {line}: length {len:?}");
        let len = match *len {
            "*" => Length::ToEnd,
            n => match n.strip_prefix("..") {
                Some(t) => Length::Through(t.parse().map_err(|_| bad_len())?),
                None => Length::Bytes(
                    n.parse::<usize>()
                        .ok()
                        .filter(|&n| n > 0)
                        .ok_or_else(bad_len)?,
                ),
            },
        };
        if source.trim().is_empty() {
            return Err(format!("line {line}: no source"));
        }
        out.push(Mask {
            id,
            key,
            start,
            len,
        });
    }
    Ok(out)
}

/// The project's masks.
pub fn masks() -> Vec<Mask> {
    parse_masks(MASKS_TSV).expect("specs/tools/scenario-masks.tsv parses (tested)")
}

/// The traces are not two runs of one scenario (§5 rule 1).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("not comparable: {0}")]
pub struct CompareError(pub String);

/// The first difference (§5 rule 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub tick: u32,
    /// Record kind (`c2s`, `s2c`, …).
    pub stream: String,
    /// Index of the record within the tick's stream.
    pub index: usize,
    /// `bytes[k]`, a field name, `record` (one side has none) or `size`.
    pub at: String,
    pub expected: String,
    pub got: String,
    pub context: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Match,
    Partial,
    Diverged,
}

impl Verdict {
    /// CLI exit code (`scenario.md` Outputs).
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Match => 0,
            Self::Diverged => 1,
            Self::Partial => 2,
        }
    }
}

/// The comparison's result (§5 rules 6–7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub verdict: Verdict,
    pub first: Option<Divergence>,
    /// Records compared per kind, up to the divergence.
    pub compared: BTreeMap<String, usize>,
    pub ticks: u32,
    pub masked_bytes: usize,
    /// Streams not compared and why.
    pub not_compared: Vec<String>,
    pub gaps_original: Vec<String>,
    pub gaps_ours: Vec<String>,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = match self.verdict {
            Verdict::Match => "match",
            Verdict::Partial => "partial",
            Verdict::Diverged => "diverged",
        };
        writeln!(f, "verdict: {v}")?;
        if let Some(d) = &self.first {
            writeln!(
                f,
                "first divergence: tick {}, {}[{}], {}",
                d.tick, d.stream, d.index, d.at
            )?;
            writeln!(f, "  expected (original): {}", d.expected)?;
            writeln!(f, "  got (d2rs):          {}", d.got)?;
            for c in &d.context {
                writeln!(f, "  {c}")?;
            }
        }
        writeln!(f, "ticks compared: {}", self.ticks)?;
        for (k, n) in &self.compared {
            writeln!(f, "  {k}: {n} records")?;
        }
        writeln!(f, "masked bytes skipped: {}", self.masked_bytes)?;
        for s in &self.not_compared {
            writeln!(f, "not compared: {s}")?;
        }
        for g in &self.gaps_original {
            writeln!(f, "gap (original): {g}")?;
        }
        for g in &self.gaps_ours {
            writeln!(f, "gap (d2rs): {g}")?;
        }
        Ok(())
    }
}

/// Header stream name of a record kind.
fn stream_of(kind: &str) -> &str {
    match kind {
        "draw" => "rng-draws",
        "spawn" | "poke" => "c2s",
        "unit" => "units",
        k => k,
    }
}

fn pair(p: [u32; 2]) -> String {
    format!("[{}, {}]", p[0], p[1])
}

/// Up to 8 bytes either side of `k`, with the offset marked.
fn window(b: &[u8], k: usize) -> String {
    let lo = k.saturating_sub(8);
    let hi = (k + 9).min(b.len());
    let mut s = String::new();
    if lo > 0 {
        s.push_str("… ");
    }
    for (i, x) in b.iter().enumerate().take(hi).skip(lo) {
        if i == k {
            s.push_str(&format!("[{x:02x}] "));
        } else {
            s.push_str(&format!("{x:02x} "));
        }
    }
    if hi < b.len() {
        s.push('…');
    }
    s.trim_end().to_owned()
}

fn describe(r: &Record) -> String {
    match r {
        Record::C2s { bytes: Ok(b), .. } | Record::S2c { bytes: b, .. } => format!(
            "{} id 0x{:02x}, {} bytes: {}",
            r.kind(),
            b.first().copied().unwrap_or(0),
            b.len(),
            encode_hex(b)
        ),
        Record::C2s { bytes: Err(u), .. } => format!("c2s unresolved {u}"),
        _ => r.to_json().to_string(),
    }
}

/// Compares two byte strings with masks; `Err((offset, masked))` at the
/// first difference, `Ok(masked)` otherwise.
fn compare_bytes(a: &[u8], b: &[u8], masks: &[Mask]) -> Result<usize, (usize, usize)> {
    // Keys and NUL positions are read from the original's bytes; a key
    // byte that differs is itself a difference (keys are never masked).
    let rs: Vec<_> = masks
        .iter()
        .map(|m| m.range(a))
        .filter(|r| !r.is_empty())
        .collect();
    let mut masked = 0;
    for k in 0..a.len().min(b.len()) {
        if rs.iter().any(|r| r.contains(&k)) {
            masked += 1;
            continue;
        }
        if a[k] != b[k] {
            return Err((k, masked));
        }
    }
    Ok(masked)
}

/// What differs between two records of one kind: (where, expected,
/// got, offset for a byte window).
fn diff(
    a: &Record,
    b: &Record,
    masks: &[Mask],
    masked: &mut usize,
) -> Option<(String, String, String, Option<usize>)> {
    let field = |name: &str, x: String, y: String| (x != y).then(|| (name.to_owned(), x, y, None));
    match (a, b) {
        (Record::C2s { bytes: x, .. }, Record::C2s { bytes: y, .. }) => match (x, y) {
            (Ok(x), Ok(y)) => {
                if x.len() != y.len() {
                    return Some((
                        "size".into(),
                        x.len().to_string(),
                        y.len().to_string(),
                        None,
                    ));
                }
                compare_bytes(x, y, &[]).err().map(|(k, _)| {
                    (
                        format!("bytes[{k}]"),
                        format!("{:02x}", x[k]),
                        format!("{:02x}", y[k]),
                        Some(k),
                    )
                })
            }
            (Err(x), Err(y)) => field("unresolved", x.clone(), y.clone()),
            (x, y) => Some((
                "resolved".into(),
                x.as_ref()
                    .map_or_else(|u| format!("unresolved {u}"), |b| encode_hex(b)),
                y.as_ref()
                    .map_or_else(|u| format!("unresolved {u}"), |b| encode_hex(b)),
                None,
            )),
        },
        (Record::Spawn { guid: x, .. }, Record::Spawn { guid: y, .. }) => {
            let show = |g: &Result<Option<u32>, String>| match g {
                Ok(Some(g)) => format!("guid {g}"),
                Ok(None) => "failed".to_owned(),
                Err(u) => format!("unresolved {u}"),
            };
            field("spawned", show(x), show(y))
        }
        (
            Record::Poke {
                d: da,
                r: ra,
                guid: ga,
                ..
            },
            Record::Poke {
                d: db,
                r: rb,
                guid: gb,
                ..
            },
        ) => {
            // Field by field in the FORMAT.md order: d, r, guid.
            let g = |g: &Option<u32>| g.map_or_else(|| "none".to_owned(), |g| g.to_string());
            field("d", da.clone(), db.clone())
                .or_else(|| field("r", ra.clone(), rb.clone()))
                .or_else(|| field("guid", g(ga), g(gb)))
        }
        (
            Record::S2c {
                client: ca,
                bytes: x,
                ..
            },
            Record::S2c {
                client: cb,
                bytes: y,
                ..
            },
        ) => {
            if ca != cb {
                return field("client", ca.to_string(), cb.to_string());
            }
            if x.len() != y.len() {
                return Some((
                    "size".into(),
                    x.len().to_string(),
                    y.len().to_string(),
                    None,
                ));
            }
            match compare_bytes(x, y, masks) {
                Ok(m) => {
                    *masked += m;
                    None
                }
                Err((k, _)) => Some((
                    format!("bytes[{k}]"),
                    format!("{:02x}", x[k]),
                    format!("{:02x}", y[k]),
                    Some(k),
                )),
            }
        }
        (
            Record::Rng {
                before: b1,
                after: a1,
                ..
            },
            Record::Rng {
                before: b2,
                after: a2,
                ..
            },
        )
        | (
            Record::Draw {
                before: b1,
                after: a1,
                ..
            },
            Record::Draw {
                before: b2,
                after: a2,
                ..
            },
        ) => field("before", pair(*b1), pair(*b2)).or_else(|| field("after", pair(*a1), pair(*a2))),
        (Record::Unit { .. }, Record::Unit { .. })
        | (Record::Stats { .. }, Record::Stats { .. }) => {
            // Field by field in the FORMAT.md order.
            const ORDER: [&str; 9] = [
                "type", "guid", "class", "mode", "x", "y", "life", "mana", "base",
            ];
            let (ja, jb) = (a.to_json(), b.to_json());
            ORDER.iter().find_map(|k| {
                let (x, y) = (&ja[*k], &jb[*k]);
                (x != y).then(|| ((*k).to_owned(), x.to_string(), y.to_string(), None))
            })
        }
        _ => Some(("kind".into(), a.kind().into(), b.kind().into(), None)),
    }
}

/// Records by (tick, kind order), `end` left out.
fn group(t: &TraceFile) -> BTreeMap<(u32, usize), Vec<&Record>> {
    let mut m: BTreeMap<(u32, usize), Vec<&Record>> = BTreeMap::new();
    for r in &t.records {
        if !matches!(r, Record::End { .. }) {
            m.entry((r.tick(), r.order())).or_default().push(r);
        }
    }
    m
}

/// Compares `original` with `ours` (§5).
pub fn compare(original: &TraceFile, ours: &TraceFile) -> Result<Report, CompareError> {
    compare_with(original, ours, &masks())
}

/// [`compare`] with a given mask table.
pub fn compare_with(
    original: &TraceFile,
    ours: &TraceFile,
    masks: &[Mask],
) -> Result<Report, CompareError> {
    let (ha, hb) = (&original.header, &ours.header);
    let same = [
        ("scenario", ha.scenario.clone(), hb.scenario.clone()),
        (
            "scenario_sha256",
            ha.scenario_sha256.clone(),
            hb.scenario_sha256.clone(),
        ),
        ("seed", ha.seed.to_string(), hb.seed.to_string()),
        ("init", ha.init.to_string(), hb.init.to_string()),
        ("end", ha.end.to_string(), hb.end.to_string()),
    ];
    for (k, x, y) in same {
        if x != y {
            return Err(CompareError(format!("header {k}: {x} vs {y}")));
        }
    }
    // Streams compared (§5 rule 2): the union of both sides' lists;
    // a stream one side lacks is not compared.
    let mut compared_streams = Vec::new();
    let mut not_compared = Vec::new();
    let mut all: Vec<&String> = ha.streams.iter().chain(&hb.streams).collect();
    all.sort();
    all.dedup();
    for s in all {
        match (ha.has(s), hb.has(s)) {
            (true, true) => compared_streams.push(s.as_str()),
            (true, false) => not_compared.push(format!("{s}: d2rs did not record it")),
            (false, true) => not_compared.push(format!("{s}: the original did not record it")),
            (false, false) => {}
        }
    }
    let (ga, gb) = (group(original), group(ours));
    let mut compared: BTreeMap<String, usize> = BTreeMap::new();
    let mut masked = 0;
    let mut first = None;
    let mut ticks = 0;
    'ticks: for t in 0..=ha.end {
        ticks = t + 1;
        for (order, kind) in KINDS.iter().enumerate() {
            if !compared_streams.contains(&stream_of(kind)) {
                continue;
            }
            let empty = Vec::new();
            let xs = ga.get(&(t, order)).unwrap_or(&empty);
            let ys = gb.get(&(t, order)).unwrap_or(&empty);
            for i in 0..xs.len().max(ys.len()) {
                let prev = |v: &Vec<&Record>| {
                    i.checked_sub(1)
                        .and_then(|p| v.get(p))
                        .map(|r| format!("previous {kind}[{}]: {}", i - 1, describe(r)))
                };
                let found = match (xs.get(i), ys.get(i)) {
                    (Some(x), None) => {
                        Some(("record".into(), describe(x), "(none: missing)".into(), None))
                    }
                    (None, Some(y)) => {
                        Some(("record".into(), "(none: extra)".into(), describe(y), None))
                    }
                    (Some(x), Some(y)) => diff(x, y, masks, &mut masked),
                    (None, None) => None,
                };
                if let Some((at, expected, got, offset)) = found {
                    let mut context = Vec::new();
                    if let (Some(x), Some(y)) = (xs.get(i), ys.get(i)) {
                        context.push(format!("expected: {}", describe(x)));
                        context.push(format!("got:      {}", describe(y)));
                        if let (
                            Some(k),
                            Record::S2c { bytes: bx, .. } | Record::C2s { bytes: Ok(bx), .. },
                            Record::S2c { bytes: by, .. } | Record::C2s { bytes: Ok(by), .. },
                        ) = (offset, x, y)
                        {
                            context.push(format!("expected bytes: {}", window(bx, k)));
                            context.push(format!("got bytes:      {}", window(by, k)));
                        }
                    }
                    context.extend(prev(xs));
                    first = Some(Divergence {
                        tick: t,
                        stream: (*kind).to_owned(),
                        index: i,
                        at,
                        expected,
                        got,
                        context,
                    });
                    break 'ticks;
                }
                *compared.entry((*kind).to_owned()).or_default() += 1;
            }
        }
    }
    let gaps_original = ha.gaps.clone();
    let gaps_ours = hb.gaps.clone();
    let verdict = if first.is_some() {
        Verdict::Diverged
    } else if not_compared.is_empty() && gaps_original.is_empty() && gaps_ours.is_empty() {
        Verdict::Match
    } else {
        Verdict::Partial
    };
    Ok(Report {
        verdict,
        first,
        compared,
        ticks,
        masked_bytes: masked,
        not_compared,
        gaps_original,
        gaps_ours,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::trace::{Header, TraceFile};

    fn trace(side: &str, records: Vec<Record>) -> TraceFile {
        TraceFile {
            header: Header {
                side: side.into(),
                tool: "test".into(),
                data: "x".into(),
                scenario: "t".into(),
                scenario_sha256: "ab".into(),
                seed: 9,
                init: 4,
                end: 3,
                streams: ["c2s", "rng", "s2c", "stats", "units"]
                    .map(String::from)
                    .to_vec(),
                gaps: vec![],
            },
            records,
        }
    }

    fn base() -> Vec<Record> {
        vec![
            Record::C2s {
                t: 0,
                i: 0,
                bytes: Ok(vec![1, 10, 0, 20, 0]),
            },
            Record::S2c {
                t: 0,
                client: 0,
                bytes: vec![0x15, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            },
            Record::S2c {
                t: 0,
                client: 0,
                bytes: vec![0x2A, 4, 0, 0x11, 0x22, 0x33, 0x44, 1, 2, 3, 4, 5, 6, 7, 8],
            },
            Record::Rng {
                t: 0,
                before: [1, 666],
                after: [5, 6],
            },
            Record::Rng {
                t: 1,
                before: [5, 6],
                after: [7, 8],
            },
            Record::Unit {
                t: 3,
                ty: 0,
                guid: 1,
                class: 1,
                mode: 2,
                x: 10,
                y: 20,
                life: 256,
                mana: 512,
            },
            Record::Stats {
                t: 3,
                ty: 0,
                guid: 1,
                base: vec![(0, 0, 10)],
            },
            Record::End { t: 3 },
        ]
    }

    // Covers: specs/tools/scenario.md §6 r3, §6 row1, §6 row2, §6 row3, §6 row4, §6 row5, §6 row6, §6 row7, §6 row8
    #[test]
    fn mask_table_parses_and_is_strict() {
        let m = |id, key, o, l| Mask {
            id,
            key,
            start: Start::At(o),
            len: Length::Bytes(l),
        };
        let k8 = |offset, value| {
            Some(Key {
                offset,
                width: 1,
                value,
            })
        };
        let k16 = |value| {
            Some(Key {
                offset: 1,
                width: 2,
                value,
            })
        };
        let ms = masks();
        assert_eq!(ms.len(), 23);
        for want in [
            m(0x21, None, 11, 1),
            m(0x22, None, 2, 1),
            m(0x22, None, 10, 1),
            m(0x2A, None, 3, 4),
            m(0x62, None, 6, 1),
            m(0x7E, None, 1, 4),
            m(0x26, k8(1, 5), 8, 2),
            m(0x26, k8(1, 6), 9, 1),
            m(0x27, k8(6, 1), 12, 28),
            m(0x50, k16(4), 13, 2),
            m(0x50, k16(0x17), 3, 12),
        ] {
            assert!(ms.contains(&want), "{want:?}");
        }
        // 0x58: byte 6 for every code a 1.14d caller sends but 5.
        let codes: Vec<_> = ms
            .iter()
            .filter(|x| x.id == 0x58)
            .map(|x| {
                assert_eq!((x.start, x.len), (Start::At(6), Length::Bytes(1)));
                x.key.unwrap().value
            })
            .collect();
        assert_eq!(codes, [0, 1, 4, 6, 7]);
        assert!(ms.contains(&Mask {
            id: 0x82,
            key: None,
            start: Start::AfterNul(5),
            len: Length::Through(20),
        }));
        assert!(ms.contains(&Mask {
            id: 0x8F,
            key: None,
            start: Start::At(1),
            len: Length::ToEnd,
        }));
        let h = "id\tkey\toffset\tlength\tsource\n";
        for bad in [
            "id\toffset\tlength\tsource\n0x2A\t1\t1\tx".to_owned(),
            format!("{h}0x2A\t-\t1\t1"),
            format!("{h}0xB5\t-\t1\t1\tx"),
            format!("{h}0x2A\t-\t1\t0\tx"),
            format!("{h}0x2A\t-\t1\t1\t "),
            format!("{h}2A\t-\t1\t1\tx"),
            format!("{h}0x2A\tu8@5=5\t1\t1\tx"),
            format!("{h}0x2A\tu8@5=0x100\t1\t1\tx"),
            format!("{h}0x2A\tu32@5=0x1\t1\t1\tx"),
            format!("{h}0x2A\t\t1\t1\tx"),
            format!("{h}0x2A\t-\tnul@\t1\tx"),
            format!("{h}0x2A\t-\t1\t..x\tx"),
        ] {
            assert!(parse_masks(&bad).is_err(), "{bad:?}");
        }
    }

    // Covers: specs/sim/intents-events.md §6 r3, §6 r6
    #[test]
    fn keyed_masks_follow_their_key() {
        let ms = masks();
        // 0x58: byte 6 (`effect`) is masked for codes 0, 1, 4, 6, 7 and
        // compared for code 5 (M08: the same flip is found there).
        for code in [0u8, 1, 4, 6, 7] {
            let a = [0x58, 1, 2, 3, 4, code, 0];
            let mut b = a;
            b[6] = 1;
            assert_eq!(compare_bytes(&a, &b, &ms), Ok(1), "code {code}");
        }
        let a = [0x58, 1, 2, 3, 4, 5, 1];
        let mut b = a;
        b[6] = 0;
        assert_eq!(compare_bytes(&a, &b, &ms), Err((6, 0)));
        // The key byte itself is compared.
        let mut b = a;
        b[5] = 6;
        assert_eq!(compare_bytes(&a, &b, &ms), Err((5, 0)));
        // 0x50: u16 1 is fully written; u16 4 masks 13–14.
        let mut a = [0u8; 15];
        a[0] = 0x50;
        a[1] = 1;
        let mut b = a;
        b[14] = 9;
        assert_eq!(compare_bytes(&a, &b, &ms), Err((14, 0)));
        a[1] = 4;
        b[1] = 4;
        assert_eq!(compare_bytes(&a, &b, &ms), Ok(2));
        // 0x82: the name's bytes after its NUL, through byte 20.
        let mut a = [0u8; 29];
        a[0] = 0x82;
        a[5..8].copy_from_slice(b"Bob");
        let mut b = a;
        b[9] = 0x41;
        b[20] = 0x41;
        assert_eq!(compare_bytes(&a, &b, &ms), Ok(12));
        b[8] = 0x41;
        assert_eq!(compare_bytes(&a, &b, &ms), Err((8, 0)));
        // A 15-character name leaves nothing masked.
        a[5..20].copy_from_slice(b"ABCDEFGHIJKLMNO");
        let mut b = a;
        b[21] = 1;
        assert_eq!(compare_bytes(&a, &b, &ms), Err((21, 0)));
    }

    // Covers: specs/tools/scenario.md §5 r6, §6 r1
    #[test]
    fn identical_and_masked_traces_match() {
        let a = trace("original", base());
        let r = compare(&a, &trace("d2rs", base())).unwrap();
        assert_eq!(r.verdict, Verdict::Match, "{r}");
        assert_eq!(r.masked_bytes, 4);
        // Masked bytes 3–6 of 0x2A differ: still a match.
        let mut b = base();
        if let Record::S2c { bytes, .. } = &mut b[2] {
            bytes[3..7].copy_from_slice(&[0, 0, 0, 0]);
        }
        assert_eq!(
            compare(&a, &trace("d2rs", b)).unwrap().verdict,
            Verdict::Match
        );
    }

    // Covers: specs/tools/scenario.md §6 r1, §6 row1, §6 row2, §6 row6, §6 row7, §6 row9, §6 row10
    #[test]
    fn unwritten_builder_bytes_are_masked_and_only_those() {
        // (id, size, masked offsets) per `tools/original-hooks.md` §6.2.
        let cases: &[(u8, usize, &[usize])] = &[
            (0x21, 12, &[11]),
            (0x22, 12, &[2, 10]),
            (0x62, 7, &[6]),
            (0x7E, 5, &[1, 2, 3, 4]),
        ];
        let m = masks();
        for &(id, size, masked) in cases {
            let a: Vec<u8> = std::iter::once(id).chain(1..size as u8).collect();
            for k in 0..size {
                let mut b = a.clone();
                b[k] ^= 0xFF;
                let got = compare_bytes(&a, &b, &m);
                if masked.contains(&k) {
                    assert_eq!(got, Ok(masked.len()), "0x{id:02X} byte {k}");
                } else {
                    let before = masked.iter().filter(|&&o| o < k).count();
                    assert_eq!(got, Err((k, before)), "0x{id:02X} byte {k}");
                }
            }
        }
    }

    // Covers: specs/tools/scenario.md §5 r3, §5 r4, §5 r5
    #[test]
    fn every_perturbation_is_found_where_it_was_made() {
        let a = trace("original", base());
        // (record index, mutation, expected stream, index, at)
        type Mutate = fn(&mut Record);
        let cases: &[(usize, Mutate, &str, usize, &str, u32)] = &[
            (
                0,
                |r| {
                    if let Record::C2s { bytes: Ok(b), .. } = r {
                        b[3] = 21
                    }
                },
                "c2s",
                0,
                "bytes[3]",
                0,
            ),
            (
                0,
                |r| {
                    if let Record::C2s { bytes, .. } = r {
                        *bytes = Err("@1".into())
                    }
                },
                "c2s",
                0,
                "resolved",
                0,
            ),
            (
                1,
                |r| {
                    if let Record::S2c { bytes, .. } = r {
                        bytes[10] ^= 1
                    }
                },
                "s2c",
                0,
                "bytes[10]",
                0,
            ),
            (
                1,
                |r| {
                    if let Record::S2c { client, .. } = r {
                        *client = 1
                    }
                },
                "s2c",
                0,
                "client",
                0,
            ),
            (
                2,
                |r| {
                    if let Record::S2c { bytes, .. } = r {
                        bytes[7] ^= 1
                    }
                },
                "s2c",
                1,
                "bytes[7]",
                0,
            ),
            (
                2,
                |r| {
                    if let Record::S2c { bytes, .. } = r {
                        bytes.push(0)
                    }
                },
                "s2c",
                1,
                "size",
                0,
            ),
            (
                4,
                |r| {
                    if let Record::Rng { after, .. } = r {
                        after[1] = 0
                    }
                },
                "rng",
                0,
                "after",
                1,
            ),
            (
                5,
                |r| {
                    if let Record::Unit { x, .. } = r {
                        *x += 1
                    }
                },
                "unit",
                0,
                "x",
                3,
            ),
            (
                5,
                |r| {
                    if let Record::Unit { life, .. } = r {
                        *life -= 1
                    }
                },
                "unit",
                0,
                "life",
                3,
            ),
            (
                6,
                |r| {
                    if let Record::Stats { base, .. } = r {
                        base[0].2 = 11
                    }
                },
                "stats",
                0,
                "base",
                3,
            ),
        ];
        for (k, (rec, mutate, stream, index, at, tick)) in cases.iter().enumerate() {
            let mut b = base();
            mutate(&mut b[*rec]);
            let r = compare(&a, &trace("d2rs", b)).unwrap();
            assert_eq!(r.verdict, Verdict::Diverged, "case {k}");
            let d = r.first.as_ref().unwrap();
            assert_eq!(
                (d.tick, d.stream.as_str(), d.index, d.at.as_str()),
                (*tick, *stream, *index, *at),
                "case {k}: {r}"
            );
        }
        // A record missing on our side, and one extra.
        let mut b = base();
        b.remove(2);
        let d = compare(&a, &trace("d2rs", b)).unwrap().first.unwrap();
        assert_eq!(
            (d.tick, d.stream.as_str(), d.index, d.at.as_str()),
            (0, "s2c", 1, "record")
        );
        assert!(d.got.contains("missing"));
        let mut b = base();
        b.insert(
            5,
            Record::Rng {
                t: 2,
                before: [7, 8],
                after: [7, 8],
            },
        );
        let d = compare(&a, &trace("d2rs", b)).unwrap().first.unwrap();
        assert_eq!(
            (d.tick, d.stream.as_str(), d.at.as_str()),
            (2, "rng", "record")
        );
        assert!(d.expected.contains("extra"));
        // The byte window marks the offset.
        let mut b = base();
        if let Record::S2c { bytes, .. } = &mut b[1] {
            bytes[10] = 0xEE;
        }
        let r = compare(&a, &trace("d2rs", b)).unwrap();
        let text = r.to_string();
        assert!(text.contains("[ee]") && text.contains("[0a]"), "{text}");
    }

    #[test]
    fn spawn_records_compare_by_outcome() {
        let with = |guid| {
            let mut v = base();
            v.insert(1, Record::Spawn { t: 0, i: 1, guid });
            v
        };
        let a = trace("original", with(Ok(Some(5))));
        assert!(compare(&a, &trace("d2rs", with(Ok(Some(5)))))
            .unwrap()
            .first
            .is_none());
        for other in [Ok(Some(6)), Ok(None), Err("@1".to_owned())] {
            let d = compare(&a, &trace("d2rs", with(other)))
                .unwrap()
                .first
                .unwrap();
            assert_eq!(
                (d.tick, d.stream.as_str(), d.index, d.at.as_str()),
                (0, "spawn", 0, "spawned")
            );
        }
    }

    // Covers: specs/tools/poke.md §3 r3
    #[test]
    fn poke_records_compare_field_by_field_after_spawns() {
        let with = |d: &str, r: &str, guid| {
            let mut v = base();
            v.insert(
                1,
                Record::Spawn {
                    t: 0,
                    i: 1,
                    guid: Ok(Some(5)),
                },
            );
            v.insert(
                2,
                Record::Poke {
                    t: 0,
                    i: 2,
                    d: d.into(),
                    r: r.into(),
                    guid,
                },
            );
            v
        };
        let a = trace("original", with("object", "ok", Some(7)));
        assert!(compare(&a, &trace("d2rs", with("object", "ok", Some(7))))
            .unwrap()
            .first
            .is_none());
        for (other, at) in [
            (with("superunique", "ok", Some(7)), "d"),
            (with("object", "failed", None), "r"),
            (with("object", "ok", Some(8)), "guid"),
            (with("object", "ok", None), "guid"),
        ] {
            let d = compare(&a, &trace("d2rs", other)).unwrap().first.unwrap();
            assert_eq!(
                (d.tick, d.stream.as_str(), d.index, d.at.as_str()),
                (0, "poke", 0, at)
            );
        }
        // A poke missing on our side.
        let mut b = with("object", "ok", Some(7));
        b.remove(2);
        let d = compare(&a, &trace("d2rs", b)).unwrap().first.unwrap();
        assert_eq!((d.stream.as_str(), d.at.as_str()), ("poke", "record"));
    }

    // Covers: specs/tools/scenario.md §5 r1, §5 r2
    #[test]
    fn headers_decide_what_is_compared() {
        let a = trace("original", base());
        let mut b = trace("d2rs", base());
        b.header.seed = 10;
        assert!(compare(&a, &b).is_err());
        // d2rs lacks units: not compared, partial.
        let mut b = trace(
            "d2rs",
            base()
                .into_iter()
                .filter(|r| !matches!(r, Record::Unit { .. }))
                .collect(),
        );
        b.header.streams.retain(|s| s != "units");
        let r = compare(&a, &b).unwrap();
        assert_eq!(r.verdict, Verdict::Partial);
        assert_eq!(r.not_compared, ["units: d2rs did not record it"]);
        // A gap alone makes it partial.
        let mut b = trace("d2rs", base());
        b.header.gaps.push("items not created".into());
        assert_eq!(compare(&a, &b).unwrap().verdict, Verdict::Partial);
    }

    // Covers: specs/tools/scenario.md §6 row11
    #[test]
    fn pong_is_masked_from_byte_one_but_its_length_is_compared() {
        let m = masks();
        let a = [0x8F, 1, 2, 3, 4, 5, 6, 7, 8];
        let b = [0x8F, 9, 9, 9, 9, 9, 9, 9, 9];
        assert_eq!(compare_bytes(&a, &b, &m), Ok(8));
        // The id byte is not masked.
        let c = [0x90, 1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(compare_bytes(&a, &c, &m), Err((0, 0)));
        // A shorter message is a size difference, found by `diff`.
        let mut ta = base();
        let mut tb = base();
        ta.insert(
            1,
            Record::S2c {
                t: 0,
                client: 0,
                bytes: a.to_vec(),
            },
        );
        tb.insert(
            1,
            Record::S2c {
                t: 0,
                client: 0,
                bytes: b[..5].to_vec(),
            },
        );
        let r = compare(&trace("original", ta), &trace("d2rs", tb)).unwrap();
        assert_eq!(r.verdict, Verdict::Diverged);
        assert_eq!(r.first.unwrap().at, "size");
    }

    // Covers: specs/tools/scenario.md §6 r2
    #[test]
    fn only_the_listed_ids_are_masked() {
        let listed = [
            0x21u8, 0x22, 0x26, 0x27, 0x2A, 0x50, 0x58, 0x62, 0x7E, 0x82, 0x8F,
        ];
        let m = masks();
        let mut ids: Vec<u8> = m.iter().map(|x| x.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids, listed);
        // Any other id: a difference at any byte is found at that byte.
        for id in (0u8..=0xB4).filter(|i| !listed.contains(i)) {
            let a: Vec<u8> = std::iter::once(id).chain(1..40).collect();
            for k in [1usize, 5, 20, 39] {
                let mut b = a.clone();
                b[k] ^= 1;
                assert_eq!(compare_bytes(&a, &b, &m), Err((k, 0)), "0x{id:02X}[{k}]");
            }
        }
    }

    // Covers: specs/tools/scenario.md §5 r7
    #[test]
    fn summary_lists_counts_masks_streams_and_gaps() {
        let a = trace("original", base());
        let mut b = trace("d2rs", base());
        b.header.gaps.push("g-ours".into());
        let mut a2 = a.clone();
        a2.header.gaps.push("g-orig".into());
        a2.header.streams.retain(|s| s != "units");
        let r = compare(&a2, &b).unwrap();
        let text = r.to_string();
        assert_eq!(r.ticks, 4);
        assert_eq!(r.compared["rng"], 2);
        assert_eq!(r.masked_bytes, 4);
        assert!(text.contains("ticks compared: 4\n"), "{text}");
        assert!(text.contains("  rng: 2 records\n"), "{text}");
        assert!(text.contains("masked bytes skipped: 4\n"), "{text}");
        assert!(
            text.contains("not compared: units: the original did not record it\n"),
            "{text}"
        );
        assert!(text.contains("gap (original): g-orig\n"), "{text}");
        assert!(text.contains("gap (d2rs): g-ours\n"), "{text}");
    }
}
