// Spec: specs/data/schema.md (with the type vocabulary of specs/data/field-types.md §3)
//! The 1.14d field lists (`fields.tsv`) and loader calls (`tables.tsv`),
//! embedded from our own spec files and parsed on first use. Data-driven:
//! the compiler and the `.bin` loader read their layouts from here.

use std::sync::OnceLock;

/// `specs/data/fields.tsv` (our spec file; no Blizzard data).
pub const FIELDS_TSV: &str = include_str!("../../../specs/data/fields.tsv");
/// `specs/data/tables.tsv` (our spec file; no Blizzard data).
pub const TABLES_TSV: &str = include_str!("../../../specs/data/tables.tsv");

/// Field type IDs 1–26 (`field-types.md` §3, `txt-format.md` §7). Names
/// follow D2MOO's `TXTFIELD_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FieldType {
    Ascii = 1,
    Dword = 2,
    Word = 3,
    Byte = 4,
    Unknown1 = 5,
    Unknown2 = 6,
    Byte2 = 7,
    Dword2 = 8,
    Raw = 9,
    AsciiToCode = 10,
    Unknown3 = 11,
    Unknown4 = 12,
    CodeToByte = 13,
    Unknown5 = 14,
    CodeToWord = 15,
    Unknown6 = 16,
    NameToIndex = 17,
    NameToIndex2 = 18,
    NameToDword = 19,
    NameToWord = 20,
    NameToWord2 = 21,
    KeyToWord = 22,
    CustomLink = 23,
    Unknown7 = 24,
    CalcToDword = 25,
    Bit = 26,
}

/// The two linker kinds (`field-types.md` §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkerKind {
    Code,
    Name,
}

impl FieldType {
    const ALL: [FieldType; 26] = [
        FieldType::Ascii,
        FieldType::Dword,
        FieldType::Word,
        FieldType::Byte,
        FieldType::Unknown1,
        FieldType::Unknown2,
        FieldType::Byte2,
        FieldType::Dword2,
        FieldType::Raw,
        FieldType::AsciiToCode,
        FieldType::Unknown3,
        FieldType::Unknown4,
        FieldType::CodeToByte,
        FieldType::Unknown5,
        FieldType::CodeToWord,
        FieldType::Unknown6,
        FieldType::NameToIndex,
        FieldType::NameToIndex2,
        FieldType::NameToDword,
        FieldType::NameToWord,
        FieldType::NameToWord2,
        FieldType::KeyToWord,
        FieldType::CustomLink,
        FieldType::Unknown7,
        FieldType::CalcToDword,
        FieldType::Bit,
    ];

    /// The type for ID 1–26.
    pub fn from_id(id: u32) -> Option<FieldType> {
        let i = usize::try_from(id).ok()?.checked_sub(1)?;
        FieldType::ALL.get(i).copied()
    }

    pub fn id(self) -> u8 {
        self as u8
    }

    /// Own-key types, converted in pass 1 (`txt-format.md` §7).
    pub fn is_own_key(self) -> bool {
        matches!(self.id(), 10 | 12 | 14 | 16 | 17 | 18)
    }

    /// Types that use the integer rule (2–6, 8, 26).
    pub fn is_integer(self) -> bool {
        matches!(self.id(), 2..=6 | 8 | 26)
    }

    /// The linker kind a type 10–21 field needs.
    pub fn linker_kind(self) -> Option<LinkerKind> {
        match self.id() {
            10..=15 => Some(LinkerKind::Code),
            16..=21 => Some(LinkerKind::Name),
            _ => None,
        }
    }

    /// Lookup types (11, 13, 15, 19, 20, 21).
    pub fn is_lookup(self) -> bool {
        matches!(self.id(), 11 | 13 | 15 | 19 | 20 | 21)
    }

    /// Field-callback types (23, 24, 25).
    pub fn is_field_callback(self) -> bool {
        matches!(self.id(), 23..=25)
    }

    /// §3 vocabulary word without its parameter, as in `fields.tsv`.
    pub fn vocabulary(self) -> &'static str {
        match self.id() {
            4..=6 => "u8",
            3 => "u16",
            2 | 8 => "u32",
            26 => "bit",
            1 | 7 => "str",
            9 => "code4",
            10 => "key(code4)",
            12 => "key(code1)",
            14 => "key(code2)",
            16 => "key(str)",
            17 => "key(name16)",
            18 => "key(name32)",
            13 | 21 => "link8",
            15 | 20 => "link16",
            11 | 19 => "link32",
            22 => "strkey",
            _ => "cb",
        }
    }

    /// Footprint width at `offset` for the field-fit check of
    /// `txt-format.md` §6.1; `None` for bit and callback types.
    pub fn width(self, len: u32) -> Option<u32> {
        Some(match self.id() {
            1 | 7 => len + 1,
            16 => len.max(1),
            2 | 8 | 9 | 10 | 11 | 18 | 19 => 4,
            3 | 14 | 15 | 17 | 20 | 22 => 2,
            4 | 5 | 6 | 12 | 13 | 21 => 1,
            _ => return None,
        })
    }
}

/// The four code buffers (`calc-expressions.md` §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CalcBuffer {
    MissCode,
    SkillsCode,
    SkillDescCode,
    ItemsCode,
}

impl CalcBuffer {
    pub const ALL: [CalcBuffer; 4] = [
        CalcBuffer::MissCode,
        CalcBuffer::SkillsCode,
        CalcBuffer::SkillDescCode,
        CalcBuffer::ItemsCode,
    ];

    /// The buffer's file name without extension (`misscode`, …).
    pub fn name(self) -> &'static str {
        match self {
            CalcBuffer::MissCode => "misscode",
            CalcBuffer::SkillsCode => "skillscode",
            CalcBuffer::SkillDescCode => "skilldesccode",
            CalcBuffer::ItemsCode => "itemscode",
        }
    }

    pub fn from_name(name: &str) -> Option<CalcBuffer> {
        CalcBuffer::ALL.into_iter().find(|b| b.name() == name)
    }
}

/// The `link` column of `fields.tsv` (schema.md §1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Link {
    /// Empty: no link (or ID 22, always the string-key resolver).
    None,
    /// A linker: `<table>.<column>`, `items.code`, `@range`,
    /// `@treasureclass`.
    Linker(String),
    /// `calc(<buffer>)` (`field-types.md` §8.1).
    Calc(CalcBuffer),
    /// `param` (`field-types.md` §8.2).
    Param,
    /// `cb(<table>.<name>)`: a table-specific callback (§8.3).
    Table(String),
}

impl Link {
    fn parse(s: &str) -> Result<Link, String> {
        if s.is_empty() {
            return Ok(Link::None);
        }
        if s == "param" {
            return Ok(Link::Param);
        }
        if let Some(inner) = s.strip_prefix("calc(").and_then(|r| r.strip_suffix(')')) {
            return CalcBuffer::from_name(inner)
                .map(Link::Calc)
                .ok_or_else(|| format!("unknown calc buffer `{inner}`"));
        }
        if let Some(inner) = s.strip_prefix("cb(").and_then(|r| r.strip_suffix(')')) {
            return Ok(Link::Table(inner.to_owned()));
        }
        Ok(Link::Linker(s.to_owned()))
    }

    /// The linker name, for linker links.
    pub fn linker(&self) -> Option<&str> {
        match self {
            Link::Linker(name) => Some(name),
            _ => None,
        }
    }
}

/// One field-list entry (a `fields.tsv` row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDef {
    /// Column name, exact bytes.
    pub column: Vec<u8>,
    pub field_type: FieldType,
    /// `str` N, `bit` n, callback argument; else 0.
    pub length: u32,
    pub offset: u32,
    pub link: Link,
}

impl FieldDef {
    pub fn new(column: &str, field_type: FieldType, length: u32, offset: u32, link: Link) -> Self {
        FieldDef {
            column: column.as_bytes().to_vec(),
            field_type,
            length,
            offset,
            link,
        }
    }

    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.column).into_owned()
    }

    /// Record bytes this field can write (`field-types.md` §9; the bit
    /// byte for `bit(n)`; 4 bytes for `calc`/`param`). Empty for
    /// table-specific callbacks, whose footprint is unknown.
    pub fn footprint(&self) -> std::ops::Range<usize> {
        let o = self.offset as usize;
        match self.field_type {
            FieldType::Bit => {
                let b = o + (self.length as usize >> 3);
                b..b + 1
            }
            t if t.is_field_callback() => match self.link {
                Link::Calc(_) | Link::Param => o..o + 4,
                _ => o..o,
            },
            t => o..o + t.width(self.length).unwrap_or(0) as usize,
        }
    }
}

/// One loader call (a `tables.tsv` row) with its field list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDef {
    pub name: String,
    /// `<name>.txt` (`levels.txt` for `leveldefs`); empty for the uncalled
    /// list.
    pub txt_name: String,
    pub bin_name: String,
    pub record_size: usize,
    /// `loading.md` §6 step (`S.NN` for compile-only calls); `None` = never
    /// called.
    pub load_step: Option<String>,
    pub key_column: String,
    /// `(archive, file)` read in normal play; `None` = compiled only.
    pub live_source: Option<(String, String)>,
    pub notes: String,
    pub fields: Vec<FieldDef>,
}

impl TableDef {
    /// A runtime table of `loading.md` §6 (step without a `.NN` suffix,
    /// with a live `.bin`).
    pub fn is_runtime(&self) -> bool {
        self.live_source.is_some() && self.load_step.as_deref().is_some_and(|s| !s.contains('.'))
    }

    /// Whether the table is compiled in the 1.14d load sequence.
    pub fn is_called(&self) -> bool {
        self.load_step.is_some()
    }

    /// The field with this column name (exact).
    pub fn field(&self, column: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.column == column.as_bytes())
    }
}

/// All 92 loader calls in execution order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub tables: Vec<TableDef>,
}

impl Schema {
    /// Parses the two TSV files.
    pub fn parse(tables_tsv: &str, fields_tsv: &str) -> Result<Schema, String> {
        let mut tables = Vec::new();
        let mut lines = tables_tsv.lines();
        let head = lines.next().ok_or("tables.tsv is empty")?;
        if head
            != "table\ttxt_name\tbin_name\trecord_size\tload_step\tkey_column\tlive_source\tnotes"
        {
            return Err(format!("tables.tsv: unexpected header `{head}`"));
        }
        for (n, line) in lines.enumerate() {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() != 8 {
                return Err(format!("tables.tsv line {}: {} columns", n + 2, c.len()));
            }
            let live_source = match c[6] {
                "-" | "" => None,
                s => {
                    let (archive, file) = s
                        .split_once('/')
                        .ok_or_else(|| format!("tables.tsv line {}: bad live_source", n + 2))?;
                    Some((archive.to_owned(), file.to_owned()))
                }
            };
            tables.push(TableDef {
                name: c[0].to_owned(),
                txt_name: c[1].to_owned(),
                bin_name: c[2].to_owned(),
                record_size: c[3]
                    .parse()
                    .map_err(|e| format!("tables.tsv line {}: record_size: {e}", n + 2))?,
                load_step: (!c[4].is_empty()).then(|| c[4].to_owned()),
                key_column: c[5].to_owned(),
                live_source,
                notes: c[7].to_owned(),
                fields: Vec::new(),
            });
        }

        let mut lines = fields_tsv.lines();
        let head = lines.next().ok_or("fields.tsv is empty")?;
        if head != "table\tseq\tcolumn\ttype_id\ttype\tlength\toffset\tlink" {
            return Err(format!("fields.tsv: unexpected header `{head}`"));
        }
        for (n, line) in lines.enumerate() {
            let at = |m: String| format!("fields.tsv line {}: {m}", n + 2);
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() != 8 {
                return Err(at(format!("{} columns", c.len())));
            }
            let num = |s: &str| s.parse::<u32>().map_err(|e| at(format!("`{s}`: {e}")));
            let table = tables
                .iter_mut()
                .find(|t| t.name == c[0])
                .ok_or_else(|| at(format!("unknown table `{}`", c[0])))?;
            if num(c[1])? as usize != table.fields.len() {
                return Err(at("seq out of order".into()));
            }
            let id = num(c[3])?;
            let field_type = FieldType::from_id(id).ok_or_else(|| at(format!("type id {id}")))?;
            if field_type.vocabulary() != c[4] {
                return Err(at(format!("type `{}` does not match id {id}", c[4])));
            }
            table.fields.push(FieldDef {
                column: c[2].as_bytes().to_vec(),
                field_type,
                length: num(c[5])?,
                offset: num(c[6])?,
                link: Link::parse(c[7]).map_err(at)?,
            });
        }
        Ok(Schema { tables })
    }

    /// The table named `name` (a `tables.tsv` name).
    pub fn table(&self, name: &str) -> Option<&TableDef> {
        self.tables.iter().find(|t| t.name == name)
    }

    /// Tables called by the 1.14d load, in execution order.
    pub fn called(&self) -> impl Iterator<Item = &TableDef> {
        self.tables.iter().filter(|t| t.is_called())
    }

    /// The runtime tables of `loading.md` §6, in load order.
    pub fn runtime(&self) -> impl Iterator<Item = &TableDef> {
        self.tables.iter().filter(|t| t.is_runtime())
    }
}

/// The embedded 1.14d schema, parsed on first use.
pub fn schema() -> &'static Schema {
    static SCHEMA: OnceLock<Schema> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        Schema::parse(TABLES_TSV, FIELDS_TSV).expect("embedded specs/data/*.tsv must parse")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/data/schema.md §1, §2
    #[test]
    fn embedded_schema_parses() {
        let s = schema();
        assert_eq!(s.tables.len(), 92);
        assert_eq!(
            s.tables.iter().map(|t| t.fields.len()).sum::<usize>(),
            3_499
        );
        assert_eq!(s.called().count(), 91);
        assert_eq!(s.runtime().count(), 73);
        assert_eq!(
            s.tables.iter().map(|t| t.fields.len()).max(),
            Some(253),
            "longest list (monstats)"
        );
    }

    /// `loading.md` §6 record sizes and first/last steps.
    // Covers: specs/data/schema.md §2; specs/data/loading.md §6
    #[test]
    fn runtime_tables_follow_loading_md() {
        let s = schema();
        let names: Vec<&str> = s.runtime().map(|t| t.name.as_str()).collect();
        assert_eq!(names[0], "compcode");
        assert_eq!(names[72], "difficultylevels");
        assert_eq!(s.table("levels").unwrap().record_size, 544);
        assert_eq!(s.table("leveldefs").unwrap().record_size, 156);
        assert_eq!(s.table("leveldefs").unwrap().txt_name, "levels.txt");
        assert_eq!(s.table("lvltypes").unwrap().record_size, 1928);
        let live_in_x = s
            .runtime()
            .filter(|t| t.live_source.as_ref().unwrap().0 == "d2exp")
            .count();
        assert_eq!(live_in_x, 17);
    }

    /// schema.md test vectors.
    // Covers: specs/data/schema.md §1
    #[test]
    fn schema_vectors() {
        let s = schema();
        let f = &s.table("skills").unwrap().fields[60];
        assert_eq!(f.column, b"pettype");
        assert_eq!(f.field_type, FieldType::NameToWord2);
        assert_eq!((f.offset, f.footprint()), (190, 190..191));
        assert_eq!(f.link, Link::Linker("pettype.pet type".into()));
        let f = &s.table("runes").unwrap().fields[13];
        assert_eq!(f.column, b"rune1");
        assert_eq!(f.field_type, FieldType::Unknown3);
        assert_eq!(f.link, Link::Linker("items.code".into()));
    }

    /// schema.md §5: bytes no field writes, as listed in the `tables.tsv`
    /// notes (`unwritten: a-b,c`), equal the bytes outside every field
    /// footprint.
    // Covers: specs/data/schema.md §5
    #[test]
    fn footprints_match_unwritten_notes() {
        let mut total = 0;
        let mut tables = 0;
        for t in schema().tables.iter() {
            let mut covered = vec![false; t.record_size];
            for f in &t.fields {
                for o in f.footprint() {
                    covered[o] = true;
                }
            }
            let ours: Vec<usize> = (0..t.record_size).filter(|&o| !covered[o]).collect();
            let listed: Vec<usize> = t
                .notes
                .split("; ")
                .find_map(|n| n.strip_prefix("unwritten: "))
                .map(|ranges| {
                    ranges
                        .split(',')
                        .flat_map(|r| {
                            let (a, b) = r.split_once('-').unwrap_or((r, r));
                            a.parse::<usize>().unwrap()..=b.parse::<usize>().unwrap()
                        })
                        .collect()
                })
                .unwrap_or_default();
            assert_eq!(ours, listed, "{}", t.name);
            if !ours.is_empty() {
                tables += 1;
            }
            total += ours.len();
        }
        assert_eq!((total, tables), (1_702, 55));
    }

    // Covers: specs/data/field-types.md §3
    #[test]
    fn type_ids_round_trip() {
        for id in 1..=26 {
            assert_eq!(u32::from(FieldType::from_id(id).unwrap().id()), id);
        }
        assert!(FieldType::from_id(0).is_none());
        assert!(FieldType::from_id(27).is_none());
    }
}
