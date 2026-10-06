// Spec: specs/client/bridge.md
//! Dispatch table (§6): one row per S→C id in
//! `specs/client/bridge-dispatch.tsv`, naming the spec that owns what the
//! message means for the client model, and the handlers registered for
//! owned ids in [`HANDLERS`]. The two must agree ([`check`]).

use d2_proto::SERVER_MESSAGES;

use super::world::{ClientWorld, UnitKey};

/// The dispatch table (spec §6 rule 1).
pub const TSV: &str = include_str!("../../../../specs/client/bridge-dispatch.tsv");

/// S→C ids 0x00..=0xB4 (`intents-events.md` §3.1).
pub const IDS: usize = 0xB5;

/// `owner` of a row no spec owns yet.
pub const TBD: &str = "TBD";

/// One S→C message as a handler sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message<'a> {
    pub id: u8,
    /// The whole message, id included.
    pub bytes: &'a [u8],
    /// The unit it addresses, if any (spec §5 rule 4).
    pub unit: Option<UnitKey>,
}

/// Why a handler could not apply its message (spec §6 rule 4).
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum HandlerError {
    #[error(transparent)]
    Decode(#[from] d2_proto::DecodeError),
    #[error("{0}")]
    Invalid(&'static str),
}

pub type HandlerFn = fn(&mut ClientWorld, &Message<'_>) -> Result<(), HandlerError>;

/// A handler registered for an owned id.
#[derive(Clone, Copy, Debug)]
pub struct Handler {
    pub id: u8,
    /// Spec path, equal to the TSV `owner` of the id.
    pub owner: &'static str,
    pub handle: HandlerFn,
}

/// Handlers for owned ids. Empty until a client-model spec takes an id
/// (spec §6 rule 2).
pub const HANDLERS: &[Handler] = &[];

/// One TSV row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: u8,
    pub name: String,
    /// `None` for `TBD`.
    pub owner: Option<String>,
}

/// A malformed dispatch TSV.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TsvError {
    #[error("header must be `id<TAB>name<TAB>owner`")]
    Header,
    #[error("line {line}: expected 3 tab-separated fields")]
    Fields { line: usize },
    #[error("line {line}: id must be 0x{expected:02X}")]
    Id { line: usize, expected: u8 },
    #[error("line {line}: empty name or owner")]
    Empty { line: usize },
    #[error("{0} rows, expected 0xB5")]
    Rows(usize),
}

/// Strict parse of the dispatch TSV: the header, then ids 0x00..=0xB4 in
/// order, written `0xNN`.
pub fn parse(text: &str) -> Result<Vec<Row>, TsvError> {
    let mut lines = text.lines();
    if lines.next() != Some("id\tname\towner") {
        return Err(TsvError::Header);
    }
    let mut rows = Vec::new();
    for (i, l) in lines.enumerate() {
        let line = i + 2;
        let f: Vec<&str> = l.split('\t').collect();
        let [id, name, owner] = f[..] else {
            return Err(TsvError::Fields { line });
        };
        let expected = u8::try_from(rows.len()).map_err(|_| TsvError::Rows(rows.len() + 1))?;
        if id != format!("0x{expected:02X}") {
            return Err(TsvError::Id { line, expected });
        }
        if name.is_empty() || owner.is_empty() {
            return Err(TsvError::Empty { line });
        }
        rows.push(Row {
            id: expected,
            name: name.to_owned(),
            owner: (owner != TBD).then(|| owner.to_owned()),
        });
    }
    if rows.len() != IDS {
        return Err(TsvError::Rows(rows.len()));
    }
    Ok(rows)
}

/// A disagreement between the TSV, `d2-proto` and [`HANDLERS`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mismatch {
    /// The TSV name differs from `server-messages.tsv` (via `d2-proto`).
    Name { id: u8, tsv: String, proto: String },
    /// An owned row without a registered handler.
    NoHandler { id: u8 },
    /// A handler for a `TBD` row.
    Unowned { id: u8 },
    /// Handler and row name different owners.
    Owner { id: u8, tsv: String, code: String },
    /// Two handlers for one id, or a handler for an id past 0xB4.
    BadHandler { id: u8 },
}

/// The mechanical check of spec §6 rule 5.
pub fn check(rows: &[Row], handlers: &[Handler]) -> Vec<Mismatch> {
    let mut out = Vec::new();
    let mut seen = [false; IDS];
    for h in handlers {
        match seen.get_mut(h.id as usize) {
            Some(s) if !*s => *s = true,
            _ => out.push(Mismatch::BadHandler { id: h.id }),
        }
    }
    for row in rows {
        let proto = SERVER_MESSAGES[row.id as usize].name;
        if row.name != proto {
            out.push(Mismatch::Name {
                id: row.id,
                tsv: row.name.clone(),
                proto: proto.to_owned(),
            });
        }
        let handler = handlers.iter().find(|h| h.id == row.id);
        match (&row.owner, handler) {
            (None, None) => {}
            (Some(_), None) => out.push(Mismatch::NoHandler { id: row.id }),
            (None, Some(_)) => out.push(Mismatch::Unowned { id: row.id }),
            (Some(o), Some(h)) if o != h.owner => out.push(Mismatch::Owner {
                id: row.id,
                tsv: o.clone(),
                code: h.owner.to_owned(),
            }),
            (Some(_), Some(_)) => {}
        }
    }
    out
}

/// The dispatch table could not be built.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TableError {
    #[error(transparent)]
    Tsv(#[from] TsvError),
    #[error("dispatch TSV and handlers disagree: {0:?}")]
    Mismatch(Vec<Mismatch>),
}

/// A registered handler as the bridge calls it.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub owner: &'static str,
    pub handle: HandlerFn,
}

/// Handlers indexed by id.
#[derive(Clone, Debug)]
pub struct Dispatch {
    entries: [Option<Entry>; IDS],
}

impl Dispatch {
    /// No handler for any id: every message is unowned.
    pub fn empty() -> Self {
        Self {
            entries: [None; IDS],
        }
    }

    /// The table of the spec: [`TSV`] checked against [`HANDLERS`].
    pub fn from_spec() -> Result<Self, TableError> {
        let rows = parse(TSV)?;
        let mismatches = check(&rows, HANDLERS);
        if !mismatches.is_empty() {
            return Err(TableError::Mismatch(mismatches));
        }
        let mut d = Self::empty();
        for h in HANDLERS {
            d.set(h.id, h.owner, h.handle);
        }
        Ok(d)
    }

    /// Registers `handle` for `id` (tests and tools; production handlers
    /// go in [`HANDLERS`]). Panics for an id past 0xB4.
    pub fn set(&mut self, id: u8, owner: &'static str, handle: HandlerFn) {
        self.entries[id as usize] = Some(Entry { owner, handle });
    }

    /// The handler of `id`, if one is registered.
    pub fn get(&self, id: u8) -> Option<&Entry> {
        self.entries.get(id as usize)?.as_ref()
    }
}
