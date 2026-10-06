// Spec: specs/tools/scenario.md §1–§3
//! Scenario scripts: the strict line parser, the canonical writer and the
//! typed-message encoder (references resolved against a [`World`]).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use d2_proto::schema::{ClientMessage, FieldType, SizeRule};
use d2_proto::CLIENT_MESSAGES;

/// Script format version (`scenario.md` §2 rule 2).
pub const SCENARIO_FORMAT_VERSION: u32 = 1;

/// Largest `end` tick (§2 header table).
pub const MAX_END: u32 = 1_000_000;

/// Largest raw message (`sim/intents-events.md` §2.1: 0x204 bytes).
pub const MAX_HEX: usize = 0x204;

/// A script line that does not parse (§2 rule 3).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {message}")]
pub struct ScriptError {
    /// 1-based line number (0: the script as a whole).
    pub line: usize,
    pub message: String,
}

/// Game difficulty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Normal,
    Nightmare,
    Hell,
}

impl Difficulty {
    pub fn index(self) -> u8 {
        self as u8
    }
    fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Nightmare => "nightmare",
            Self::Hell => "hell",
        }
    }
}

/// A stream of `record` (§2 recording table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stream {
    S2c,
    Rng,
    Units,
    Stats,
    Frames,
}

impl Stream {
    pub const ALL: [Stream; 5] = [Self::S2c, Self::Rng, Self::Units, Self::Stats, Self::Frames];
    pub fn name(self) -> &'static str {
        match self {
            Self::S2c => "s2c",
            Self::Rng => "rng",
            Self::Units => "units",
            Self::Stats => "stats",
            Self::Frames => "frames",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|x| x.name() == s)
    }
}

/// Where an inline item goes (§2 character table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Inv(u8, u8),
    Stash(u8, u8),
    Cube(u8, u8),
    Belt(u8),
    Body(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub code: String,
    pub place: Place,
}

/// `char at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Default,
    At(u32, u32),
}

/// An inline character (§2 character table).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inline {
    pub class: u8,
    pub level: u8,
    pub act: u8,
    pub area: u32,
    pub at: Start,
    /// (itemstatcost row, base value), script order, ids unique.
    pub stats: Vec<(u16, i32)>,
    /// (skill id, hard points), script order, ids unique.
    pub skills: Vec<(u16, u8)>,
    /// Level ids, script order, unique.
    pub waypoints: Vec<u32>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Character {
    /// `char save <path>`.
    Save(String),
    Inline(Inline),
}

/// A reference (§3 rule 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ref {
    Player,
    /// `@x±N`.
    X(i32),
    /// `@y±N`.
    Y(i32),
    /// `@<type>[:<class>][#<n>]`.
    Unit {
        ty: u8,
        class: Option<u32>,
        n: u32,
    },
    /// `@wp[#<n>]`.
    Waypoint(u32),
}

impl std::fmt::Display for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let nth = |f: &mut std::fmt::Formatter<'_>, n: u32| {
            if n == 0 {
                Ok(())
            } else {
                write!(f, "#{n}")
            }
        };
        let off = |f: &mut std::fmt::Formatter<'_>, axis: &str, d: i32| match d {
            0 => write!(f, "@{axis}"),
            d if d > 0 => write!(f, "@{axis}+{d}"),
            d => write!(f, "@{axis}-{}", d.unsigned_abs()),
        };
        match *self {
            Self::Player => write!(f, "@player"),
            Self::X(d) => off(f, "x", d),
            Self::Y(d) => off(f, "y", d),
            Self::Unit { ty, class, n } => {
                write!(f, "@{ty}")?;
                if let Some(c) = class {
                    write!(f, ":{c}")?;
                }
                nth(f, n)
            }
            Self::Waypoint(n) => {
                write!(f, "@wp")?;
                nth(f, n)
            }
        }
    }
}

/// A field value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Num(u32),
    Ref(Ref),
}

/// A step's message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepMsg {
    Hex(Vec<u8>),
    /// A typed message: the client-messages row and its fields in layout
    /// order.
    Typed {
        id: u8,
        fields: Vec<(String, Arg)>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub tick: u32,
    pub msg: StepMsg,
}

/// A parsed scenario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    pub name: String,
    /// The game seed's `init_low` value (`sim/rng.md` §5.2).
    pub seed: u32,
    /// The character's map ID: the DRLG seed (`sim/rng.md` §5.4).
    pub map: u32,
    pub difficulty: Difficulty,
    pub expansion: bool,
    pub end: u32,
    pub character: Character,
    /// Sorted, unique.
    pub record: Vec<Stream>,
    pub snapshot_every: Option<u32>,
    /// Sorted, unique.
    pub snapshot_at: Vec<u32>,
    pub steps: Vec<Step>,
}

impl Scenario {
    pub fn records(&self, s: Stream) -> bool {
        self.record.contains(&s)
    }

    /// The snapshot ticks (§2: `every` multiples, `at` ticks and `end`);
    /// empty when neither `units` nor `stats` is recorded.
    pub fn snapshot_ticks(&self) -> BTreeSet<u32> {
        let mut out = BTreeSet::new();
        if !(self.records(Stream::Units) || self.records(Stream::Stats)) {
            return out;
        }
        if let Some(n) = self.snapshot_every {
            out.extend((0..=self.end).step_by(n as usize));
        }
        out.extend(self.snapshot_at.iter().copied());
        out.insert(self.end);
        out
    }

    /// Canonical text (§2 rule 6).
    pub fn to_text(&self) -> String {
        let mut o = String::new();
        let mut line = |s: String| {
            o.push_str(&s);
            o.push('\n');
        };
        line(format!("scenario {SCENARIO_FORMAT_VERSION}"));
        line(format!("name {}", self.name));
        line("game 1.14d".into());
        line(format!("seed 0x{:08x}", self.seed));
        line(format!("map 0x{:08x}", self.map));
        line(format!("difficulty {}", self.difficulty.name()));
        line(format!(
            "expansion {}",
            if self.expansion { "yes" } else { "no" }
        ));
        line(format!("end {}", self.end));
        match &self.character {
            Character::Save(p) => line(format!("char save {p}")),
            Character::Inline(c) => {
                line(format!("char class {}", c.class));
                line(format!("char level {}", c.level));
                line(format!("char area {} {}", c.act, c.area));
                line(match c.at {
                    Start::Default => "char at default".into(),
                    Start::At(x, y) => format!("char at {x} {y}"),
                });
                for (s, v) in &c.stats {
                    line(format!("char stat {s} {v}"));
                }
                for (s, l) in &c.skills {
                    line(format!("char skill {s} {l}"));
                }
                for w in &c.waypoints {
                    line(format!("char waypoint {w}"));
                }
                for i in &c.items {
                    let place = match i.place {
                        Place::Inv(x, y) => format!("inv {x} {y}"),
                        Place::Stash(x, y) => format!("stash {x} {y}"),
                        Place::Cube(x, y) => format!("cube {x} {y}"),
                        Place::Belt(s) => format!("belt {s}"),
                        Place::Body(b) => format!("body {b}"),
                    };
                    line(format!("char item {} {place}", i.code));
                }
            }
        }
        if !self.record.is_empty() {
            let names: Vec<&str> = self.record.iter().map(|s| s.name()).collect();
            line(format!("record {}", names.join(" ")));
        }
        if let Some(n) = self.snapshot_every {
            line(format!("snapshot every {n}"));
        }
        if !self.snapshot_at.is_empty() {
            let t: Vec<String> = self.snapshot_at.iter().map(u32::to_string).collect();
            line(format!("snapshot at {}", t.join(" ")));
        }
        for s in &self.steps {
            let mut l = format!("at {} ", s.tick);
            match &s.msg {
                StepMsg::Hex(b) => {
                    l.push_str("hex");
                    for x in b {
                        let _ = write!(l, " {x:02x}");
                    }
                }
                StepMsg::Typed { id, fields } => {
                    let _ = write!(l, "msg {}", message(*id).name);
                    for (n, v) in fields {
                        match v {
                            Arg::Num(x) => {
                                let _ = write!(l, " {n}={x}");
                            }
                            Arg::Ref(r) => {
                                let _ = write!(l, " {n}={r}");
                            }
                        }
                    }
                }
            }
            line(l);
        }
        o
    }

    /// SHA-256 (hex) of the canonical text (FORMAT.md header
    /// `scenario_sha256`).
    pub fn sha256(&self) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(self.to_text().as_bytes())
            .iter()
            .fold(String::new(), |mut s, b| {
                let _ = write!(s, "{b:02x}");
                s
            })
    }

    /// Parses a script (§2, §3).
    pub fn parse(text: &str) -> Result<Self, ScriptError> {
        Parser::default().run(text)
    }
}

fn message(id: u8) -> &'static ClientMessage {
    &CLIENT_MESSAGES[usize::from(id)]
}

/// A typed message's size and fields, or why it cannot be typed (§3
/// rule 1).
fn typed_layout(m: &ClientMessage) -> Result<usize, String> {
    let SizeRule::Fixed(size) = m.transport_size else {
        return Err(format!("{} has no fixed size: write it as hex", m.name));
    };
    if size == 0 {
        return Err(format!("{} is never a valid message", m.name));
    }
    for f in m.layout {
        match f.ty {
            FieldType::U8
            | FieldType::U16
            | FieldType::U32
            | FieldType::Bits(_)
            | FieldType::Bit(_)
                if f.offset.is_some() => {}
            _ => {
                return Err(format!(
                    "{} field {} cannot be typed: write the message as hex",
                    m.name, f.name
                ))
            }
        }
    }
    Ok(usize::from(size))
}

/// Largest value a field holds.
fn field_max(ty: FieldType) -> u32 {
    match ty {
        FieldType::U8 => 0xFF,
        FieldType::U16 => 0xFFFF,
        FieldType::Bits(n) if n < 32 => (1u32 << n) - 1,
        FieldType::Bit(_) => 1,
        _ => u32::MAX,
    }
}

fn find_message(name: &str) -> Option<&'static ClientMessage> {
    CLIENT_MESSAGES
        .iter()
        .find(|m| m.name == name && m.name != "-")
}

/// The game state references resolve against (§3 rules 3–4).
pub trait World {
    /// The character's GUID and position (sub-tiles).
    fn player(&self) -> Option<(u32, i32, i32)>;
    /// Every unit of the unit lists.
    fn units(&self) -> Vec<UnitRef>;
}

/// A unit as references see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitRef {
    pub ty: u8,
    pub class: u32,
    pub guid: u32,
    /// An object whose `objects` row has operate function 23.
    pub waypoint: bool,
}

/// Resolves one reference; `Err` names why it did not resolve.
pub fn resolve(r: &Ref, w: &dyn World) -> Result<i64, String> {
    let nth = |mut guids: Vec<u32>, n: u32| {
        guids.sort_unstable();
        guids
            .get(n as usize)
            .map(|&g| i64::from(g))
            .ok_or_else(|| format!("{r}: no such unit"))
    };
    match *r {
        Ref::Player => w
            .player()
            .map(|p| i64::from(p.0))
            .ok_or_else(|| format!("{r}: no player")),
        Ref::X(d) | Ref::Y(d) => {
            let (_, x, y) = w.player().ok_or_else(|| format!("{r}: no player"))?;
            let base = if matches!(r, Ref::X(_)) { x } else { y };
            Ok(i64::from(base) + i64::from(d))
        }
        Ref::Unit { ty, class, n } => nth(
            w.units()
                .into_iter()
                .filter(|u| u.ty == ty && class.is_none_or(|c| u.class == c))
                .map(|u| u.guid)
                .collect(),
            n,
        ),
        Ref::Waypoint(n) => nth(
            w.units()
                .into_iter()
                .filter(|u| u.ty == 2 && u.waypoint)
                .map(|u| u.guid)
                .collect(),
            n,
        ),
    }
}

/// A step that did not resolve (§3 rule 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unresolved {
    /// The reference as written (the trace's `unresolved`).
    pub reference: String,
    /// Why, for diagnostics.
    pub why: String,
}

/// The bytes of a step (§3 rule 2), or the reference that did not
/// resolve (§3 rule 5).
pub fn encode(msg: &StepMsg, w: &dyn World) -> Result<Vec<u8>, Unresolved> {
    let (id, fields) = match msg {
        StepMsg::Hex(b) => return Ok(b.clone()),
        StepMsg::Typed { id, fields } => (*id, fields),
    };
    let m = message(id);
    // Checked at parse time.
    let size = typed_layout(m).expect("typed messages are checked at parse time");
    let mut out = vec![0u8; size];
    out[0] = id;
    for f in m.layout {
        let (_, arg) = fields
            .iter()
            .find(|(n, _)| n == f.name)
            .expect("every field is checked at parse time");
        let v = match arg {
            Arg::Num(v) => *v,
            Arg::Ref(r) => {
                let unresolved = |why: String| Unresolved {
                    reference: r.to_string(),
                    why,
                };
                let v = resolve(r, w).map_err(unresolved)?;
                u32::try_from(v)
                    .ok()
                    .filter(|&v| v <= field_max(f.ty))
                    .ok_or_else(|| unresolved(format!("value {v} does not fit field {}", f.name)))?
            }
        };
        let off = usize::from(f.offset.unwrap_or(0));
        match f.ty {
            FieldType::U8 => out[off] = v as u8,
            FieldType::U16 => out[off..off + 2].copy_from_slice(&(v as u16).to_le_bytes()),
            FieldType::U32 => out[off..off + 4].copy_from_slice(&v.to_le_bytes()),
            FieldType::Bits(_) | FieldType::Bit(_) => {
                let shift = match f.ty {
                    FieldType::Bit(n) => u32::from(n),
                    _ => 0,
                };
                let old = u32::from_le_bytes(out[off..off + 4].try_into().expect("4 bytes"));
                out[off..off + 4].copy_from_slice(&(old | (v << shift)).to_le_bytes());
            }
            _ => unreachable!("typed_layout admits integer fields only"),
        }
    }
    Ok(out)
}

#[derive(Default)]
struct Parser {
    name: Option<String>,
    game: bool,
    seed: Option<u32>,
    map: Option<u32>,
    difficulty: Option<Difficulty>,
    expansion: Option<bool>,
    end: Option<u32>,
    save: Option<String>,
    class: Option<u8>,
    level: Option<u8>,
    area: Option<(u8, u32)>,
    at: Option<Start>,
    stats: Vec<(u16, i32)>,
    skills: Vec<(u16, u8)>,
    waypoints: Vec<u32>,
    items: Vec<Item>,
    inline_lines: Option<usize>,
    record: Option<Vec<Stream>>,
    every: Option<u32>,
    at_ticks: Option<Vec<u32>>,
    steps: Vec<(usize, Step)>,
}

fn num(t: &str) -> Result<u32, String> {
    let r = match t.strip_prefix("0x") {
        Some(h) if !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit()) => {
            u32::from_str_radix(h, 16)
        }
        Some(_) => return Err(format!("bad number {t:?}")),
        None if !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) => t.parse(),
        None => return Err(format!("bad number {t:?}")),
    };
    r.map_err(|_| format!("number {t:?} does not fit 32 bits"))
}

fn ranged<T: TryFrom<u32>>(t: &str, lo: u32, hi: u32, what: &str) -> Result<T, String> {
    let v = num(t)?;
    if v < lo || v > hi {
        return Err(format!("{what} {v} outside {lo}..={hi}"));
    }
    T::try_from(v).map_err(|_| format!("{what} {v} out of range"))
}

fn signed(t: &str) -> Result<i32, String> {
    match t.strip_prefix('-') {
        Some(rest) => {
            let v = i64::from(num(rest)?);
            i32::try_from(-v).map_err(|_| format!("number {t:?} does not fit i32"))
        }
        None => i32::try_from(num(t)?).map_err(|_| format!("number {t:?} does not fit i32")),
    }
}

fn parse_ref(t: &str) -> Result<Ref, String> {
    let body = t.strip_prefix('@').ok_or("not a reference")?;
    if body == "player" {
        return Ok(Ref::Player);
    }
    for (axis, mk) in [("x", Ref::X as fn(i32) -> Ref), ("y", Ref::Y)] {
        if let Some(rest) = body.strip_prefix(axis) {
            let d = match rest.as_bytes().first() {
                None => 0,
                Some(b'+') if !rest[1..].starts_with('-') => signed(&rest[1..])?,
                Some(b'-') if rest.len() > 1 => signed(rest)?,
                _ => return Err(format!("bad reference {t:?}")),
            };
            // `@x+0` / `@x-0` are written `@x`.
            if d == 0 && !rest.is_empty() {
                return Err(format!("{t:?}: write a zero offset as @{axis}"));
            }
            return Ok(mk(d));
        }
    }
    let (head, n) = match body.split_once('#') {
        Some((h, n)) => {
            let n = num(n)?;
            if n == 0 {
                return Err(format!("{t:?}: write #0 as nothing"));
            }
            (h, n)
        }
        None => (body, 0),
    };
    if head == "wp" {
        return Ok(Ref::Waypoint(n));
    }
    let (ty, class) = match head.split_once(':') {
        Some((ty, c)) => (ty, Some(num(c)?)),
        None => (head, None),
    };
    let ty = ranged::<u8>(ty, 0, 5, "unit type")?;
    Ok(Ref::Unit { ty, class, n })
}

impl Parser {
    fn run(mut self, text: &str) -> Result<Scenario, ScriptError> {
        let mut version_seen = false;
        for (k, raw) in text.lines().enumerate() {
            let line = k + 1;
            // A comment starts at a token that begins with `#` (§2 rule 1).
            let toks: Vec<&str> = raw
                .split_ascii_whitespace()
                .take_while(|t| !t.starts_with('#'))
                .collect();
            if toks.is_empty() {
                continue;
            }
            let err = |message: String| ScriptError { line, message };
            if !version_seen {
                if toks != ["scenario", "1"] {
                    return Err(err(match toks.as_slice() {
                        ["scenario", v] => format!("unknown scenario format version {v}"),
                        _ => "the first line must be `scenario 1`".into(),
                    }));
                }
                version_seen = true;
                continue;
            }
            self.line(line, &toks).map_err(err)?;
        }
        if !version_seen {
            return Err(ScriptError {
                line: 0,
                message: "empty script: no `scenario 1` line".into(),
            });
        }
        self.finish()
    }

    fn line(&mut self, line: usize, toks: &[&str]) -> Result<(), String> {
        let rest = &toks[1..];
        let want = |n: usize| {
            if rest.len() == n {
                Ok(())
            } else {
                Err(format!(
                    "`{}` takes {n} value(s), got {}",
                    toks[0],
                    rest.len()
                ))
            }
        };
        if toks[0] != "at" && !self.steps.is_empty() {
            return Err(format!("`{}` after the first `at` line", toks[0]));
        }
        fn once<T>(slot: &mut Option<T>, v: T, what: &str) -> Result<(), String> {
            if slot.is_some() {
                return Err(format!("`{what}` given twice"));
            }
            *slot = Some(v);
            Ok(())
        }
        match toks[0] {
            "name" => {
                want(1)?;
                let n = rest[0];
                if !n
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                {
                    return Err(format!("name {n:?} is not [a-z0-9-]+"));
                }
                once(&mut self.name, n.to_owned(), "name")
            }
            "game" => {
                want(1)?;
                if rest[0] != "1.14d" {
                    return Err(format!("game {:?}: only 1.14d is accepted", rest[0]));
                }
                if self.game {
                    return Err("`game` given twice".into());
                }
                self.game = true;
                Ok(())
            }
            "seed" => {
                want(1)?;
                once(&mut self.seed, num(rest[0])?, "seed")
            }
            "map" => {
                want(1)?;
                once(&mut self.map, num(rest[0])?, "map")
            }
            "difficulty" => {
                want(1)?;
                let d = match rest[0] {
                    "normal" => Difficulty::Normal,
                    "nightmare" => Difficulty::Nightmare,
                    "hell" => Difficulty::Hell,
                    d => return Err(format!("unknown difficulty {d:?}")),
                };
                once(&mut self.difficulty, d, "difficulty")
            }
            "expansion" => {
                want(1)?;
                let e = match rest[0] {
                    "yes" => true,
                    "no" => false,
                    e => return Err(format!("expansion {e:?}: yes or no")),
                };
                once(&mut self.expansion, e, "expansion")
            }
            "end" => {
                want(1)?;
                once(&mut self.end, ranged(rest[0], 0, MAX_END, "end")?, "end")
            }
            "char" => self.char_line(line, rest),
            "record" => {
                if rest.is_empty() {
                    return Err("`record` needs at least one stream".into());
                }
                let mut v = Vec::new();
                for s in rest {
                    let s = Stream::parse(s).ok_or_else(|| {
                        if *s == "c2s" {
                            "c2s is always recorded: do not list it".to_owned()
                        } else {
                            format!("unknown stream {s:?}")
                        }
                    })?;
                    if v.contains(&s) {
                        return Err(format!("stream {} listed twice", s.name()));
                    }
                    v.push(s);
                }
                v.sort();
                once(&mut self.record, v, "record")
            }
            "snapshot" => match rest {
                ["every", n] => once(
                    &mut self.every,
                    ranged(n, 1, MAX_END, "snapshot interval")?,
                    "snapshot every",
                ),
                ["at", ticks @ ..] if !ticks.is_empty() => {
                    let mut v = ticks
                        .iter()
                        .map(|t| num(t))
                        .collect::<Result<Vec<u32>, _>>()?;
                    let n = v.len();
                    v.sort_unstable();
                    v.dedup();
                    if v.len() != n {
                        return Err("a snapshot tick is listed twice".into());
                    }
                    once(&mut self.at_ticks, v, "snapshot at")
                }
                _ => Err("`snapshot every <n>` or `snapshot at <tick>...`".into()),
            },
            "at" => {
                if rest.len() < 2 {
                    return Err("`at <tick> hex|msg ...`".into());
                }
                let tick = ranged(rest[0], 0, MAX_END, "tick")?;
                if let Some((_, prev)) = self.steps.last() {
                    if tick < prev.tick {
                        return Err(format!(
                            "step at tick {tick} after a step at tick {}",
                            prev.tick
                        ));
                    }
                }
                let msg = match rest[1] {
                    "hex" => {
                        let b = rest[2..]
                            .iter()
                            .map(|t| {
                                if t.len() == 2 && t.bytes().all(|b| b.is_ascii_hexdigit()) {
                                    u8::from_str_radix(t, 16).map_err(|e| e.to_string())
                                } else {
                                    Err(format!("hex byte {t:?}: two hex digits"))
                                }
                            })
                            .collect::<Result<Vec<u8>, _>>()?;
                        if b.is_empty() || b.len() > MAX_HEX {
                            return Err(format!("hex message of {} bytes: 1..={MAX_HEX}", b.len()));
                        }
                        StepMsg::Hex(b)
                    }
                    "msg" => typed(&rest[2..])?,
                    k => return Err(format!("unknown step kind {k:?}: hex or msg")),
                };
                self.steps.push((line, Step { tick, msg }));
                Ok(())
            }
            k => Err(format!("unknown keyword {k:?}")),
        }
    }

    fn char_line(&mut self, line: usize, rest: &[&str]) -> Result<(), String> {
        let Some((&kw, args)) = rest.split_first() else {
            return Err("`char` needs a field".into());
        };
        if kw == "save" {
            if args.len() != 1 {
                return Err("`char save <path>`".into());
            }
            if self.save.is_some() {
                return Err("`char save` given twice".into());
            }
            self.save = Some(args[0].to_owned());
            return Ok(());
        }
        self.inline_lines.get_or_insert(line);
        let want = |n: usize| {
            if args.len() == n {
                Ok(())
            } else {
                Err(format!(
                    "`char {kw}` takes {n} value(s), got {}",
                    args.len()
                ))
            }
        };
        fn once<T>(slot: &mut Option<T>, v: T, what: &str) -> Result<(), String> {
            if slot.is_some() {
                return Err(format!("`char {what}` given twice"));
            }
            *slot = Some(v);
            Ok(())
        }
        match kw {
            "class" => {
                want(1)?;
                once(&mut self.class, ranged(args[0], 0, 6, "class")?, "class")
            }
            "level" => {
                want(1)?;
                once(&mut self.level, ranged(args[0], 1, 99, "level")?, "level")
            }
            "area" => {
                want(2)?;
                let a = (ranged(args[0], 0, 4, "act")?, num(args[1])?);
                once(&mut self.area, a, "area")
            }
            "at" => {
                let s = match args {
                    ["default"] => Start::Default,
                    [x, y] => Start::At(num(x)?, num(y)?),
                    _ => return Err("`char at default` or `char at <x> <y>`".into()),
                };
                once(&mut self.at, s, "at")
            }
            "stat" => {
                want(2)?;
                let id = ranged::<u16>(args[0], 0, 0xFFFF, "stat id")?;
                if self.stats.iter().any(|s| s.0 == id) {
                    return Err(format!("stat {id} given twice"));
                }
                self.stats.push((id, signed(args[1])?));
                Ok(())
            }
            "skill" => {
                want(2)?;
                let id = ranged::<u16>(args[0], 0, 0xFFFF, "skill id")?;
                if self.skills.iter().any(|s| s.0 == id) {
                    return Err(format!("skill {id} given twice"));
                }
                self.skills
                    .push((id, ranged(args[1], 1, 255, "skill level")?));
                Ok(())
            }
            "waypoint" => {
                want(1)?;
                let l = num(args[0])?;
                if self.waypoints.contains(&l) {
                    return Err(format!("waypoint {l} given twice"));
                }
                self.waypoints.push(l);
                Ok(())
            }
            "item" => {
                let [code, place @ ..] = args else {
                    return Err("`char item <code> <place>`".into());
                };
                if !(3..=4).contains(&code.len())
                    || !code
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                {
                    return Err(format!("item code {code:?}: 3-4 of [a-z0-9]"));
                }
                let cell = |t: &str| ranged::<u8>(t, 0, 15, "grid cell");
                let place = match place {
                    ["inv", x, y] => Place::Inv(cell(x)?, cell(y)?),
                    ["stash", x, y] => Place::Stash(cell(x)?, cell(y)?),
                    ["cube", x, y] => Place::Cube(cell(x)?, cell(y)?),
                    ["belt", s] => Place::Belt(ranged(s, 0, 15, "belt slot")?),
                    ["body", b] => Place::Body(ranged(b, 1, 12, "body location")?),
                    _ => {
                        return Err(
                            "item place: inv|stash|cube <x> <y>, belt <slot> or body <loc>".into(),
                        )
                    }
                };
                self.items.push(Item {
                    code: (*code).to_owned(),
                    place,
                });
                Ok(())
            }
            k => Err(format!("unknown character field {k:?}")),
        }
    }

    fn finish(self) -> Result<Scenario, ScriptError> {
        let whole = |message: String| ScriptError { line: 0, message };
        let missing = |what: &str| whole(format!("missing `{what}` line"));
        let name = self.name.ok_or_else(|| missing("name"))?;
        if !self.game {
            return Err(missing("game"));
        }
        let seed = self.seed.ok_or_else(|| missing("seed"))?;
        let map = self.map.ok_or_else(|| missing("map"))?;
        let difficulty = self.difficulty.ok_or_else(|| missing("difficulty"))?;
        let expansion = self.expansion.ok_or_else(|| missing("expansion"))?;
        let end = self.end.ok_or_else(|| missing("end"))?;
        let character = match (self.save, self.inline_lines) {
            (Some(_), Some(line)) => {
                return Err(ScriptError {
                    line,
                    message: "`char save` excludes inline character lines".into(),
                })
            }
            (Some(p), None) => Character::Save(p),
            (None, _) => {
                let (act, area) = self.area.ok_or_else(|| missing("char area"))?;
                Character::Inline(Inline {
                    class: self.class.ok_or_else(|| missing("char class"))?,
                    level: self.level.unwrap_or(1),
                    act,
                    area,
                    at: self.at.unwrap_or(Start::Default),
                    stats: self.stats,
                    skills: self.skills,
                    waypoints: self.waypoints,
                    items: self.items,
                })
            }
        };
        let record = self.record.unwrap_or_default();
        let snapshots = record.contains(&Stream::Units) || record.contains(&Stream::Stats);
        if (self.every.is_some() || self.at_ticks.is_some()) && !snapshots {
            return Err(whole(
                "`snapshot` lines need `units` or `stats` recorded".into(),
            ));
        }
        let snapshot_at = self.at_ticks.unwrap_or_default();
        if let Some(&t) = snapshot_at.iter().find(|&&t| t > end) {
            return Err(whole(format!("snapshot tick {t} after end {end}")));
        }
        let mut steps = Vec::with_capacity(self.steps.len());
        for (line, s) in self.steps {
            if s.tick > end {
                return Err(ScriptError {
                    line,
                    message: format!("step at tick {} after end {end}", s.tick),
                });
            }
            steps.push(s);
        }
        Ok(Scenario {
            name,
            seed,
            map,
            difficulty,
            expansion,
            end,
            character,
            record,
            snapshot_every: self.every,
            snapshot_at,
            steps,
        })
    }
}

/// `msg <Name> <field>=<value>...` (§3 rules 1–2).
fn typed(toks: &[&str]) -> Result<StepMsg, String> {
    let Some((&name, assigns)) = toks.split_first() else {
        return Err("`msg` needs a message name".into());
    };
    let m = find_message(name).ok_or_else(|| format!("unknown message {name:?}"))?;
    typed_layout(m)?;
    let mut given: Vec<(&str, Arg)> = Vec::new();
    for a in assigns {
        let (n, v) = a
            .split_once('=')
            .ok_or_else(|| format!("{a:?}: <field>=<value>"))?;
        let f = m
            .layout
            .iter()
            .find(|f| f.name == n)
            .ok_or_else(|| format!("{name} has no field {n:?}"))?;
        if given.iter().any(|(g, _)| *g == n) {
            return Err(format!("field {n} given twice"));
        }
        let v = if v.starts_with('@') {
            Arg::Ref(parse_ref(v)?)
        } else {
            let x = num(v)?;
            if x > field_max(f.ty) {
                return Err(format!("{n}={x} does not fit the field"));
            }
            Arg::Num(x)
        };
        given.push((n, v));
    }
    let mut fields = Vec::with_capacity(m.layout.len());
    for f in m.layout {
        let Some(i) = given.iter().position(|(g, _)| *g == f.name) else {
            return Err(format!("{name} needs field {}", f.name));
        };
        fields.push((f.name.to_owned(), given.swap_remove(i).1));
    }
    Ok(StepMsg::Typed { id: m.id, fields })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct W;
    impl World for W {
        fn player(&self) -> Option<(u32, i32, i32)> {
            Some((7, 100, 200))
        }
        fn units(&self) -> Vec<UnitRef> {
            let u = |ty, class, guid, waypoint| UnitRef {
                ty,
                class,
                guid,
                waypoint,
            };
            vec![
                u(0, 1, 7, false),
                u(1, 148, 30, false),
                u(1, 148, 12, false),
                u(2, 0, 5, true),
                u(2, 9, 4, false),
            ]
        }
    }

    const BASE: &str = "scenario 1\nname t\ngame 1.14d\nseed 1\nmap 2\ndifficulty normal\nexpansion yes\nend 10\nchar class 1\nchar area 0 1\n";

    fn steps(extra: &str) -> Result<Scenario, ScriptError> {
        Scenario::parse(&format!("{BASE}{extra}"))
    }

    fn bytes(step: &str) -> Vec<u8> {
        let s = steps(&format!("{step}\n")).unwrap();
        encode(&s.steps[0].msg, &W).unwrap()
    }

    // Covers: specs/tools/scenario.md §3 r2
    #[test]
    fn typed_messages_encode_from_the_layout() {
        assert_eq!(bytes("at 0 msg Walk x=10 y=20"), [1, 10, 0, 20, 0]);
        assert_eq!(bytes("at 0 msg Walk y=20 x=10"), [1, 10, 0, 20, 0]);
        assert_eq!(
            bytes("at 0 msg SelectSkill skill=36 left=0 item=0xFFFFFFFF"),
            [0x3C, 36, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]
        );
        assert_eq!(
            bytes("at 0 msg SelectSkill skill=36 left=1 item=0"),
            [0x3C, 36, 0, 0, 0x80, 0, 0, 0, 0]
        );
        assert_eq!(bytes("at 0 hex 01 02"), [1, 2]);
    }

    // Covers: specs/tools/scenario.md §3 r3, §3 r4, §3 r5
    #[test]
    fn references_resolve_in_guid_order() {
        assert_eq!(bytes("at 0 msg Walk x=@x+5 y=@y-3"), [1, 105, 0, 197, 0]);
        assert_eq!(
            bytes("at 0 msg InteractWithEntity type=1 id=@1:148"),
            [0x13, 1, 0, 0, 0, 12, 0, 0, 0]
        );
        assert_eq!(
            bytes("at 0 msg InteractWithEntity type=1 id=@1:148#1"),
            [0x13, 1, 0, 0, 0, 30, 0, 0, 0]
        );
        assert_eq!(
            bytes("at 0 msg InteractWithEntity type=2 id=@wp"),
            [0x13, 2, 0, 0, 0, 5, 0, 0, 0]
        );
        assert_eq!(
            bytes("at 0 msg InteractWithEntity type=0 id=@player"),
            [0x13, 0, 0, 0, 0, 7, 0, 0, 0]
        );
        let s = steps("at 0 msg InteractWithEntity type=1 id=@1:148#2\n").unwrap();
        assert_eq!(
            encode(&s.steps[0].msg, &W),
            Err(Unresolved {
                reference: "@1:148#2".into(),
                why: "@1:148#2: no such unit".into()
            })
        );
        let s = steps("at 0 msg Walk x=@x-101 y=0\n").unwrap();
        let e = encode(&s.steps[0].msg, &W).unwrap_err();
        assert_eq!(e.reference, "@x-101");
        assert!(e.why.contains("does not fit"), "{e:?}");
    }

    // Covers: specs/tools/scenario.md §2 r2, §2 r3, §2 r5, §3 r1
    #[test]
    fn strict_errors_name_the_line() {
        let cases: &[(&str, usize, &str)] = &[
            ("bogus 1\n", 11, "unknown keyword"),
            ("seed 2\n", 11, "given twice"),
            ("char class 2\n", 11, "given twice"),
            ("at 5 hex 01\nat 4 hex 01\n", 12, "after a step at tick 5"),
            ("at 11 hex 01\n", 11, "after end"),
            ("at 1 msg Walk x=1\n", 11, "needs field y"),
            ("at 1 msg Walk x=1 y=70000\n", 11, "does not fit"),
            ("at 1 msg Walk x=1 y=2 z=3\n", 11, "no field"),
            ("at 1 msg Nope\n", 11, "unknown message"),
            ("at 1 hex 1\n", 11, "two hex digits"),
            ("at 1 hex 01\nseed 3\n", 12, "after the first"),
            ("record c2s\n", 11, "always recorded"),
            ("record s2c s2c\n", 11, "twice"),
            ("char item abcde inv 0 0\n", 11, "item code"),
            ("char level 100\n", 11, "level 100"),
            ("at 1 msg Walk x=@z y=1\n", 11, "bad number"),
            ("at 1 msg Walk x=@6 y=1\n", 11, "unit type"),
            (
                "at 1 msg Walk x=1 y=2 # trailing comment\nat 1 msg Walk x=1 y=2#3\n",
                12,
                "bad number",
            ),
            ("at 1 msg Walk x=@x+0 y=1\n", 11, "zero offset"),
        ];
        for (extra, line, needle) in cases {
            let e = steps(extra).unwrap_err();
            assert_eq!(e.line, *line, "{extra:?}: {e}");
            assert!(e.message.contains(needle), "{extra:?}: {e}");
        }
        assert_eq!(
            Scenario::parse("scenario 2\n").unwrap_err().message,
            "unknown scenario format version 2"
        );
        let e = Scenario::parse("scenario 1\nname t\n").unwrap_err();
        assert_eq!((e.line, e.message.as_str()), (0, "missing `game` line"));
        let e = steps("snapshot every 5\n").unwrap_err();
        assert!(e.message.contains("need `units` or `stats`"), "{e}");
        let e = steps("char save x.d2s\n").unwrap_err();
        assert!(e.message.contains("excludes"), "{e}");
    }

    // Covers: specs/tools/scenario.md §2 r6, §edge-cases-original-bugs r4
    #[test]
    fn canonical_text_round_trips() {
        let s = steps(
            "record units s2c\nsnapshot every 25\nsnapshot at 3 1\nchar stat 0 -5\nchar item hp1 belt 2\nat 1 msg Walk x=@x+1 y=@y\nat 1 hex 6d 00\n",
        )
        .unwrap();
        let text = s.to_text();
        let again = Scenario::parse(&text).unwrap();
        assert_eq!(again, s);
        assert_eq!(again.to_text(), text);
        assert_eq!(
            s.snapshot_ticks().into_iter().collect::<Vec<_>>(),
            [0, 1, 3, 10]
        );
        assert!(text.contains("seed 0x00000001\n"), "{text}");
        assert!(text.contains("at 1 msg Walk x=@x+1 y=@y\n"), "{text}");
    }
}
