// Spec: specs/data/patch-layers.md
//! Mod patch layers (`d2patch 1`): row and cell statements applied in
//! stack order to the user's own excel `.txt` cells, then compiled by the
//! verified txt compiler. Layers never touch `.bin` files or records.
//!
//! - [`syntax`]: stack and layer files (§3; S and P findings).
//! - [`apply`]: selectors, columns and the stack (§4–§6; A and N).
//! - [`diff`]: an edited `.txt` → a canonical layer (§9; D).
//! - [`check`]: compile the patched cells and compare (§7; C).
//!
//! Everything here is pure: no files, clock, environment, hash-map order
//! or floats (§11). Tools do the I/O (`data-tool patch`).

mod apply;
mod check;
mod diff;
#[cfg(test)]
mod robust_tests;
mod syntax;
#[cfg(test)]
mod tests;

use std::fmt;

use sha2::{Digest, Sha256};

use crate::compile::code4;
use crate::schema::{schema, FieldType, Schema};
use crate::txt::{bind, TxtTable};

pub use apply::apply_stack;
pub use check::{compile_patched, PatchedCompile};
pub use diff::{diff_tables, DiffError};
pub use syntax::{
    load_stack, parse_layer, parse_stack, Layer, Sel, StackEntry, Stmt, StmtLine, Tok,
};

/// The layer and stack format version this engine reads (§11).
pub const VERSION: u32 = 1;

// ------------------------------------------------------------- findings

/// The finding codes (§3–§9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Code {
    S01,
    S02,
    S03,
    S04,
    S05,
    S06,
    S07,
    P01,
    P02,
    P03,
    P04,
    P05,
    P06,
    P07,
    P08,
    P09,
    P10,
    P11,
    P12,
    B01,
    B02,
    B03,
    A01,
    A02,
    A03,
    A04,
    A05,
    A06,
    A07,
    A08,
    A09,
    A10,
    A11,
    A12,
    A13,
    A14,
    A15,
    N01,
    N02,
    N03,
    N04,
    C01,
    C02,
    C03,
    D01,
    D02,
    D03,
    D04,
    D05,
    D06,
}

impl Code {
    /// Notes (N) are not errors; everything else is (§8).
    pub fn is_error(self) -> bool {
        !matches!(self, Code::N01 | Code::N02 | Code::N03 | Code::N04)
    }

    /// The condition, in words (§3–§9; not normative).
    pub fn describe(self) -> &'static str {
        match self {
            Code::S01 => "line 1 is not `d2stack 1`",
            Code::S02 => "unsupported stack version",
            Code::S03 => "not a `layer <path>` line",
            Code::S04 => "invalid layer path",
            Code::S05 => "layer listed twice",
            Code::S06 => "layer file unreadable",
            Code::S07 => "more than 1,024 layers",
            Code::P01 => "byte outside 0x20-0x7E, LF, CR LF",
            Code::P02 => "line 1 is not `d2patch 1`",
            Code::P03 => "unsupported layer version",
            Code::P04 => "unknown statement",
            Code::P05 => "missing, extra or unexpected token",
            Code::P06 => "unterminated bracket",
            Code::P07 => "invalid row index",
            Code::P08 => "invalid table name",
            Code::P09 => "statement before the first `table`",
            Code::P10 => "token not followed by a space or line end",
            Code::P11 => "invalid pin",
            Code::P12 => "file, line or token too long",
            Code::B01 => "base table absent",
            Code::B02 => "base table unreadable",
            Code::B03 => "key column not addressable",
            Code::A01 => "not a patchable table",
            Code::A02 => "column not found or ambiguous",
            Code::A03 => "no such row",
            Code::A04 => "several rows have this key",
            Code::A05 => "row label differs",
            Code::A06 => "cell differs from the stated value",
            Code::A07 => "old and new values are equal",
            Code::A08 => "cell already written by this layer",
            Code::A09 => "add index is not the row count",
            Code::A10 => "table has a fixed row count",
            Code::A11 => "`Expansion` in column 0",
            Code::A12 => "empty or duplicate key",
            Code::A13 => "template pin differs",
            Code::A14 => "key selector renamed in this layer",
            Code::A15 => "row is a template in this layer",
            Code::N01 => "replaces an earlier layer's value",
            Code::N02 => "no list binds this column",
            Code::N03 => "statements skipped",
            Code::N04 => "layers not applied",
            Code::C01 => "compile failed",
            Code::C02 => "new compiler diagnostic",
            Code::C03 => "live-set check failed",
            Code::D01 => "rows removed",
            Code::D02 => "header changed",
            Code::D03 => "value not expressible",
            Code::D04 => "edited file unreadable",
            Code::D05 => "rows inserted, deleted or sorted",
            Code::D06 => "row count or key rule broken",
        }
    }

    fn class(self) -> u8 {
        match format!("{self:?}").as_bytes()[0] {
            b'S' => 0,
            b'P' => 1,
            b'B' => 2,
            b'A' | b'N' => 3,
            b'C' => 4,
            _ => 5,
        }
    }
}

/// One report entry (§8). Lines and columns are 1-based, 0 when none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub code: Code,
    /// Layer stack path, stack file, or `base:<archive>:<path>`.
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub table: Option<String>,
    /// (index, key).
    pub row: Option<(usize, Vec<u8>)>,
    /// Canonical column reference.
    pub column: Option<Vec<u8>>,
    pub expected: Option<Vec<u8>>,
    pub found: Option<Vec<u8>>,
    /// `base line N` or `layer:line`.
    pub writer: Option<String>,
    pub related: Vec<String>,
    /// Free wording (not normative).
    pub detail: String,
    /// Stack position: 0 the stack file, `k + 1` layer `k` (ordering only).
    pub pos: usize,
}

impl Finding {
    pub fn new(code: Code, file: &str, line: usize, col: usize) -> Finding {
        Finding {
            code,
            file: file.to_owned(),
            line,
            col,
            table: None,
            row: None,
            column: None,
            expected: None,
            found: None,
            writer: None,
            related: Vec::new(),
            detail: String::new(),
            pos: 0,
        }
    }

    fn at_pos(mut self, pos: usize) -> Finding {
        self.pos = pos;
        self
    }

    fn detail(mut self, d: impl Into<String>) -> Finding {
        self.detail = d.into();
        self
    }

    fn sort_key(&self) -> (u8, usize, usize, usize, Code, String, usize, Vec<u8>) {
        let class = self.code.class();
        let row = self.row.as_ref().map_or(0, |r| r.0);
        let column = self.column.clone().unwrap_or_default();
        let table = self.table.clone().unwrap_or_default();
        match class {
            0 | 1 => (
                class, self.pos, self.line, self.col, self.code, table, 0, column,
            ),
            3 if self.code == Code::N04 => (class, usize::MAX, 0, 0, self.code, table, 0, column),
            3 => (
                class, self.pos, self.line, self.col, self.code, table, 0, column,
            ),
            // B (table) and C (table, row, column): never by code.
            _ => (class, 0, 0, 0, Code::C01, table, row, column),
        }
    }
}

/// `[bytes]`, lossy.
fn bracket(v: &[u8]) -> String {
    format!("[{}]", String::from_utf8_lossy(v))
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let severity = if self.code.is_error() {
            "error"
        } else {
            "note"
        };
        write!(
            f,
            "{severity}[{:?}] {}:{}:{}:",
            self.code, self.file, self.line, self.col
        )?;
        if let Some(t) = &self.table {
            write!(f, " {t}")?;
        }
        if let Some((i, key)) = &self.row {
            write!(
                f,
                " {} (#{i})",
                String::from_utf8_lossy(&canonical_token(key))
            )?;
        }
        if let Some(c) = &self.column {
            write!(f, " {}", String::from_utf8_lossy(c))?;
        }
        if self.table.is_some() || self.row.is_some() || self.column.is_some() {
            write!(f, ":")?;
        }
        let mut parts = Vec::new();
        parts.push(if self.detail.is_empty() {
            self.code.describe().to_owned()
        } else {
            self.detail.clone()
        });
        if let Some(e) = &self.expected {
            parts.push(format!("expected {}", bracket(e)));
        }
        if let Some(v) = &self.found {
            parts.push(format!("found {}", bracket(v)));
        }
        if let Some(w) = &self.writer {
            parts.push(format!("writer {w}"));
        }
        if !self.related.is_empty() {
            parts.push(format!("related: {}", self.related.join(", ")));
        }
        write!(f, " {}", parts.join("; "))
    }
}

/// Sorts findings in report order (§8).
pub fn sort_report(findings: &mut [Finding]) {
    findings.sort_by_cached_key(Finding::sort_key);
}

/// Whether any finding is an error.
pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.code.is_error())
}

// ----------------------------------------------------------- table rules

/// How keys compare (§2): as the code or name linker does, or exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    /// First 4 bytes, space-padded, case-sensitive.
    Code,
    /// First 31 bytes, ASCII-lowercased.
    Name,
    /// Tables without a linker key (`multi`).
    Exact,
}

impl KeyKind {
    /// The normalized key of a cell.
    pub fn normalize(self, cell: &[u8]) -> Vec<u8> {
        match self {
            KeyKind::Code => code4(cell).to_vec(),
            KeyKind::Name => cell[..cell.len().min(31)].to_ascii_lowercase(),
            KeyKind::Exact => cell.to_vec(),
        }
    }
}

/// The patch rules of one table (§2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRules {
    /// Lowercase stem of the txt name.
    pub name: String,
    pub txt_name: String,
    /// The key field's column name; `None` = column 0.
    pub key_field: Option<Vec<u8>>,
    pub kind: KeyKind,
    /// `unique` (else `multi`).
    pub unique: bool,
    /// `fixed` rows (else `append`).
    pub fixed: bool,
    /// Uniqueness scope: the linker the key fills, or the table name.
    pub scope: String,
    /// Field column names of each list compiled from this txt.
    pub lists: Vec<Vec<Vec<u8>>>,
}

/// Tables whose row count `loading.md` §8 / §10.8 pins (§2).
const FIXED: [&str; 7] = [
    "arena",
    "armtype",
    "belts",
    "composit",
    "difficultylevels",
    "experience",
    "inventory",
];

/// The patchable tables of a schema: one per distinct `txt_name` of a
/// called list, in first-call order (§2).
pub fn rules_from_schema(schema: &Schema) -> Vec<TableRules> {
    let mut out: Vec<TableRules> = Vec::new();
    for def in schema.called() {
        let lists: Vec<Vec<u8>> = def.fields.iter().map(|f| f.column.clone()).collect();
        if let Some(r) = out.iter_mut().find(|r| r.txt_name == def.txt_name) {
            r.lists.push(lists);
            continue;
        }
        let name = def
            .txt_name
            .strip_suffix(".txt")
            .unwrap_or(&def.txt_name)
            .to_ascii_lowercase();
        out.push(TableRules {
            name: name.clone(),
            txt_name: def.txt_name.clone(),
            key_field: None,
            kind: KeyKind::Exact,
            unique: false,
            fixed: FIXED.contains(&name.as_str()),
            scope: name,
            lists: vec![lists],
        });
    }
    // The key: the first list with a key(code4)/key(name16)/key(name32)
    // entry named by its `key_column`.
    for r in &mut out {
        let key = schema
            .called()
            .filter(|d| d.txt_name == r.txt_name && !d.key_column.is_empty())
            .find_map(|d| {
                let f = d
                    .fields
                    .iter()
                    .find(|f| f.column.eq_ignore_ascii_case(d.key_column.as_bytes()))?;
                let kind = match f.field_type {
                    FieldType::AsciiToCode => KeyKind::Code,
                    FieldType::NameToIndex | FieldType::NameToIndex2 => KeyKind::Name,
                    _ => return None,
                };
                Some((f.column.clone(), kind, f.link.linker()?.to_owned()))
            });
        if let Some((column, kind, scope)) = key {
            r.key_field = Some(column);
            r.kind = kind;
            r.scope = scope;
            r.unique = r.name != "monseq";
        } else if r.name == "treasureclassex" {
            r.kind = KeyKind::Name;
            r.unique = true;
        }
    }
    out
}

/// The 1.14d patchable tables (85).
pub fn rules() -> Vec<TableRules> {
    rules_from_schema(schema())
}

// --------------------------------------------------------------- cells

/// A layer location: stack position (0-based layer index) and line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Loc {
    pub layer: usize,
    pub line: usize,
}

/// Where a row came from (§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Base file line.
    Base(usize),
    Added(Loc),
}

/// Who last wrote a cell (§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writer {
    Base,
    Add(Loc),
    Copy(Loc),
    Set(Loc),
}

impl Writer {
    pub fn loc(self) -> Option<Loc> {
        match self {
            Writer::Base => None,
            Writer::Add(l) | Writer::Copy(l) | Writer::Set(l) => Some(l),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub origin: Origin,
    pub cells: Vec<Vec<u8>>,
    pub writers: Vec<Writer>,
}

/// One table's cells with provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchTable {
    pub rules: TableRules,
    /// Archive the base came from.
    pub source: String,
    pub header: Vec<Vec<u8>>,
    pub rows: Vec<Row>,
    pub key_col: usize,
    /// Per column: some list of this txt binds it.
    pub bound: Vec<bool>,
}

impl PatchTable {
    /// Builds a table from its base file (§2; B02, B03 on failure).
    pub fn from_base(
        rules: &TableRules,
        source: &str,
        bytes: &[u8],
    ) -> Result<PatchTable, Box<Finding>> {
        let file = base_file(source, &rules.txt_name);
        let txt = TxtTable::parse(&file, bytes).map_err(|e| {
            let mut f = Finding::new(Code::B02, &file, e.line.unwrap_or(0), 0).detail(format!(
                "{:?} {}",
                e.code,
                e.code.describe()
            ));
            f.table = Some(rules.name.clone());
            Box::new(f)
        })?;
        let rows = txt
            .records
            .iter()
            .map(|r| Row {
                origin: Origin::Base(r.line),
                writers: vec![Writer::Base; r.cells.len()],
                cells: r.cells.clone(),
            })
            .collect();
        PatchTable::new(rules, source, txt.header, rows)
    }

    /// A table from a header and rows (tests, fixtures).
    pub fn new(
        rules: &TableRules,
        source: &str,
        header: Vec<Vec<u8>>,
        rows: Vec<Row>,
    ) -> Result<PatchTable, Box<Finding>> {
        let b03 = |detail: &str| {
            let mut f = Finding::new(Code::B03, &base_file(source, &rules.txt_name), 0, 0)
                .detail(detail.to_owned());
            f.table = Some(rules.name.clone());
            Box::new(f)
        };
        let mut bound = vec![false; header.len()];
        let mut key_col = 0;
        for list in &rules.lists {
            let b = bind(&header, list);
            for (c, f) in b.column_field.iter().enumerate() {
                if f.is_some() {
                    bound[c] = true;
                }
            }
        }
        if let Some(key) = &rules.key_field {
            let list = rules
                .lists
                .iter()
                .find(|l| l.iter().any(|n| n == key))
                .ok_or_else(|| b03("key field in no list"))?;
            let b = bind(&header, list);
            let f = list.iter().position(|n| n == key).expect("found above");
            key_col = b.field_column[f].ok_or_else(|| b03("the key entry binds no column"))?;
            let name = &header[key_col];
            if header
                .iter()
                .enumerate()
                .any(|(c, h)| c != key_col && h.eq_ignore_ascii_case(name))
            {
                return Err(b03("another column's name equals the key column's"));
            }
        }
        Ok(PatchTable {
            rules: rules.clone(),
            source: source.to_owned(),
            header,
            rows,
            key_col,
            bound,
        })
    }

    pub fn name(&self) -> &str {
        &self.rules.name
    }

    pub fn key(&self, row: usize) -> &[u8] {
        &self.rows[row].cells[self.key_col]
    }

    /// The canonical reference of column `c` (§4): the name if unique, else
    /// `name@k`.
    pub fn canonical(&self, c: usize) -> Vec<u8> {
        canonical_column(&self.header, c)
    }

    /// `render(T)` (§9): header and rows, TAB-joined, CR LF lines.
    pub fn render(&self) -> Vec<u8> {
        render(&self.header, self.rows.iter().map(|r| &r.cells[..]))
    }

    /// Hex SHA-256 of `render(T)`.
    pub fn digest(&self) -> String {
        hex(&Sha256::digest(self.render()))
    }

    /// The pin of row `r` (§5).
    pub fn pin(&self, r: usize) -> String {
        pin(&self.header, &self.rows[r].cells)
    }

    /// Rows keyed exactly `key`.
    pub fn rows_keyed(&self, key: &[u8]) -> Vec<usize> {
        (0..self.rows.len())
            .filter(|&i| self.key(i) == key)
            .collect()
    }
}

/// `base:<archive>:<path>`.
pub fn base_file(source: &str, txt_name: &str) -> String {
    format!("base:{source}:data\\global\\excel\\{txt_name}")
}

pub(crate) fn canonical_column(header: &[Vec<u8>], c: usize) -> Vec<u8> {
    let name = &header[c];
    let same: Vec<usize> = (0..header.len()).filter(|&i| &header[i] == name).collect();
    if same.len() == 1 {
        return name.clone();
    }
    let k = same.iter().position(|&i| i == c).expect("c is in same") + 1;
    let mut out = name.clone();
    out.extend_from_slice(format!("@{k}").as_bytes());
    out
}

pub(crate) fn render<'a>(header: &[Vec<u8>], rows: impl Iterator<Item = &'a [Vec<u8>]>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut line = |cells: &[Vec<u8>]| {
        out.extend_from_slice(&cells.join(&b'\t'));
        out.extend_from_slice(b"\r\n");
    };
    line(header);
    for r in rows {
        line(r);
    }
    out
}

pub(crate) fn pin(header: &[Vec<u8>], cells: &[Vec<u8>]) -> String {
    let mut h = Sha256::new();
    for (c, cell) in cells.iter().enumerate() {
        if cell.is_empty() {
            continue;
        }
        h.update(canonical_column(header, c));
        h.update(b"\t");
        h.update(cell);
        h.update(b"\n");
    }
    format!("sha:{}", &hex(&h.finalize())[..16])
}

pub(crate) fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The canonical token for a value (§9): bare if non-empty, bytes
/// 0x21–0x7E without `[` `]`, no leading `#`, not `->` or `like`; else
/// bracketed.
pub fn canonical_token(v: &[u8]) -> Vec<u8> {
    let bare = !v.is_empty()
        && v.iter()
            .all(|&b| (0x21..=0x7E).contains(&b) && b != b'[' && b != b']')
        && v[0] != b'#'
        && v != b"->"
        && v != b"like";
    if bare {
        v.to_vec()
    } else {
        let mut out = vec![b'['];
        out.extend_from_slice(v);
        out.push(b']');
        out
    }
}

// ----------------------------------------------------------------- data

/// A change-log entry (§5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub loc: Loc,
    pub table: usize,
    pub row: usize,
    /// Row key when the statement ran.
    pub key: Vec<u8>,
    /// `None` for an `add`.
    pub set: Option<SetChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetChange {
    pub column: usize,
    pub old: Vec<u8>,
    pub new: Vec<u8>,
}

/// The patchable tables with provenance, the layer names and the change
/// log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchData {
    pub tables: Vec<PatchTable>,
    /// Stack paths of the applied layers, by stack position.
    pub layers: Vec<String>,
    pub changes: Vec<Change>,
}

/// A source of base `.txt` files: `(archive, bytes)` for an excel file
/// name, `None` when absent.
pub type BaseReader<'a> = dyn FnMut(&str) -> Result<Option<(String, Vec<u8>)>, String> + 'a;

impl PatchData {
    /// Reads every table's base (§2): B01 absent, B02 reader error, B03.
    pub fn from_base(
        rules: &[TableRules],
        read: &mut BaseReader<'_>,
    ) -> Result<Self, Vec<Finding>> {
        let mut tables = Vec::new();
        let mut errors = Vec::new();
        for r in rules {
            let file = format!("data\\global\\excel\\{}", r.txt_name);
            let mut fail = |code, detail: String| {
                let mut f = Finding::new(code, &file, 0, 0).detail(detail);
                f.table = Some(r.name.clone());
                errors.push(f);
            };
            match read(&r.txt_name) {
                Ok(None) => fail(Code::B01, "not found in any archive".into()),
                Err(e) => fail(Code::B02, e),
                Ok(Some((source, bytes))) => match PatchTable::from_base(r, &source, &bytes) {
                    Ok(t) => tables.push(t),
                    Err(f) => errors.push(*f),
                },
            }
        }
        if errors.is_empty() {
            Ok(PatchData::from_tables(tables))
        } else {
            sort_report(&mut errors);
            Err(errors)
        }
    }

    pub fn from_tables(tables: Vec<PatchTable>) -> PatchData {
        PatchData {
            tables,
            layers: Vec::new(),
            changes: Vec::new(),
        }
    }

    pub fn table(&self, name: &str) -> Option<&PatchTable> {
        self.tables.iter().find(|t| t.name() == name)
    }

    pub fn table_index(&self, name: &[u8]) -> Option<usize> {
        self.tables.iter().position(|t| t.name().as_bytes() == name)
    }

    /// The data digest (§9): SHA-256 over, per table by name: name, TAB,
    /// table digest, LF.
    pub fn digest(&self) -> String {
        let mut order: Vec<&PatchTable> = self.tables.iter().collect();
        order.sort_by(|a, b| a.name().cmp(b.name()));
        let mut h = Sha256::new();
        for t in order {
            h.update(t.name());
            h.update(b"\t");
            h.update(t.digest());
            h.update(b"\n");
        }
        hex(&h.finalize())
    }

    /// `layer:line` for a location.
    pub fn loc_name(&self, loc: Loc) -> String {
        let layer = self.layers.get(loc.layer).map_or("?", String::as_str);
        format!("{layer}:{}", loc.line)
    }

    /// The report form of a writer: `base line N` or `layer:line`.
    pub fn writer_name(&self, table: usize, row: usize, w: Writer) -> String {
        match w.loc() {
            Some(l) => self.loc_name(l),
            None => match self.tables[table].rows[row].origin {
                Origin::Base(line) => format!("base line {line}"),
                Origin::Added(l) => self.loc_name(l),
            },
        }
    }
}
