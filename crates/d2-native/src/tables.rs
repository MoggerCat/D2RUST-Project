// Spec: specs/formats/native-assets.md §2.8, §4.3 (C-TABLE); policy from specs/data/field-types.md §10
//! Excel tables: the native file is the source `.txt` byte for byte; the
//! only extra is `_bin-overrides.toml`, the explained cells where the live
//! `.bin` differs from the compile of the text; the live `.bin` is the
//! check, never a native file (§2.8 r2).

use d2_data::bin::{BinSet, BinTable};
use d2_data::compile::Compiled;
use d2_data::compile_set::CompiledSet;
use d2_data::schema::schema;

use crate::toml_kinds::{hex_bytes, unhex_bytes, Out, Tab, TextError};

/// Where the overrides file lives under `base/` (§2.8 r3).
pub const OVERRIDES_PATH: &str = "data/global/excel/_bin-overrides.toml";

/// The (table, record, column) cells the owning spec explains
/// (`field-types.md` §10): `monstats` record 707 `NameStr`.
pub const EXPLAINED: &[(&str, u32, &str)] = &[("monstats", 707, "NameStr")];

/// The native `.txt`: the source bytes, unchanged (§2.8 r1).
pub fn write_excel(source: &[u8]) -> Vec<u8> {
    source.to_vec()
}

/// C-TABLE step 1: native `.txt` bytes equal the source `.txt` bytes.
pub fn check_excel(file: &str, source: &[u8], native: &[u8]) -> Result<(), TextError> {
    if source == native {
        return Ok(());
    }
    let at = (0..source.len().min(native.len()))
        .find(|&i| source[i] != native[i])
        .unwrap_or(source.len().min(native.len()));
    let line = 1 + source[..at.min(source.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count();
    Err(TextError::new(
        file,
        format!(
            "native differs from source at byte {at} (line {line}): source {:?}, native {:?} ({} vs {} bytes)",
            source.get(at),
            native.get(at),
            source.len(),
            native.len()
        ),
    ))
}

/// One explained cell: the live `.bin` bytes that replace the compiled ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinOverride {
    pub table: String,
    pub record: u32,
    /// Column name, for the reader of the file; `offset` is what applies.
    pub field: String,
    /// Byte offset of the field inside the record.
    pub offset: u32,
    /// The live bytes.
    pub bytes: Vec<u8>,
}

/// The contents of `_bin-overrides.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BinOverrides {
    pub entries: Vec<BinOverride>,
}

impl BinOverrides {
    pub fn to_toml(&self) -> String {
        let mut o = Out::header("bin-overrides");
        for e in &self.entries {
            o.table("[[override]]");
            o.str("table", &e.table);
            o.int("record", e.record);
            o.str("field", &e.field);
            o.int("offset", e.offset);
            o.str("bytes", &format!("0x{}", hex_bytes(&e.bytes)));
        }
        o.0
    }

    pub fn from_toml(file: &str, text: &str) -> Result<BinOverrides, TextError> {
        let mut t = Tab::parse(file, text, "bin-overrides")?;
        let mut entries = Vec::new();
        for mut e in t.tables("override")? {
            let table = e.string("table")?;
            let record = e.u32("record")?;
            let field = e.string("field")?;
            let offset = e.u32("offset")?;
            let raw = e.string("bytes")?;
            let bytes = raw
                .strip_prefix("0x")
                .and_then(unhex_bytes)
                .filter(|b| !b.is_empty())
                .ok_or_else(|| {
                    e.err(format!(
                        "bytes {raw:?}: expected \"0x\" and lowercase hex bytes"
                    ))
                })?;
            e.finish()?;
            entries.push(BinOverride {
                table,
                record,
                field,
                offset,
                bytes,
            });
        }
        t.finish()?;
        Ok(BinOverrides { entries })
    }

    /// Applies this table's overrides to compiled `records`; returns how
    /// many were applied. An override outside the table is an error.
    pub fn apply(
        &self,
        table: &str,
        record_size: usize,
        records: &mut [u8],
    ) -> Result<usize, TextError> {
        let mut n = 0;
        for e in self.entries.iter().filter(|e| e.table == table) {
            let at = e.record as usize * record_size + e.offset as usize;
            let cell = (e.offset as usize + e.bytes.len() <= record_size)
                .then(|| records.get_mut(at..at + e.bytes.len()))
                .flatten()
                .ok_or_else(|| {
                    TextError::new(
                        OVERRIDES_PATH,
                        format!(
                            "{table} record {} offset {} is outside the table",
                            e.record, e.offset
                        ),
                    )
                })?;
            cell.copy_from_slice(&e.bytes);
            n += 1;
        }
        Ok(n)
    }
}

/// The override for one explained cell, if the live bytes differ from the
/// compiled ones.
fn derive_cell(
    table: &str,
    record: u32,
    column: &str,
    ours: &Compiled,
    live: &BinTable,
) -> Result<Option<BinOverride>, TextError> {
    let err = |d: String| TextError::new(table, d);
    let def = schema()
        .table(table)
        .ok_or_else(|| err("not in the schema".into()))?;
    let f = def
        .field(column)
        .ok_or_else(|| err(format!("no column {column}")))?;
    let r = f.footprint();
    let (r_us, size) = (record as usize, def.record_size);
    if r_us >= ours.count
        || r_us >= live.count
        || ours.record_size != size
        || live.record_size != size
    {
        return Err(err(format!(
            "record {record} is not in both the compile and the .bin"
        )));
    }
    let (a, b) = (&ours.record(r_us)[r.clone()], &live.record(r_us)[r.clone()]);
    Ok((a != b).then(|| BinOverride {
        table: table.to_owned(),
        record,
        field: f.name(),
        offset: r.start as u32,
        bytes: b.to_vec(),
    }))
}

/// Builds the overrides file contents from the compile of the native text
/// set and the live `.bin` set: one entry per [`EXPLAINED`] cell whose
/// live bytes differ. Differences elsewhere are not listed (they fail
/// [`check_tables`]).
pub fn derive_overrides(compiled: &CompiledSet, live: &BinSet) -> Result<BinOverrides, TextError> {
    let mut entries = Vec::new();
    for &(table, record, column) in EXPLAINED {
        let (Some(c), Some(l)) = (compiled.table(table), live.table(table)) else {
            return Err(TextError::new(
                table,
                "missing from the compile or the live set",
            ));
        };
        entries.extend(derive_cell(table, record, column, &c.compiled, l)?);
    }
    Ok(BinOverrides { entries })
}

/// Compares one compiled table with its live `.bin` after applying the
/// overrides: identical bytes or the first difference.
pub fn compare_table(
    table: &str,
    ours: &Compiled,
    live: &BinTable,
    overrides: &BinOverrides,
) -> Result<(), TextError> {
    let err = |d: String| TextError::new(table, d);
    if ours.count != live.count || ours.record_size != live.record_size {
        return Err(err(format!(
            "{} records of {} bytes compiled, .bin has {} of {}",
            ours.count, ours.record_size, live.count, live.record_size
        )));
    }
    let mut bytes = ours.records.clone();
    overrides.apply(table, ours.record_size, &mut bytes)?;
    match (0..bytes.len()).find(|&i| bytes[i] != live.records[i]) {
        None => Ok(()),
        Some(i) => {
            let (rec, off) = (i / ours.record_size, i % ours.record_size);
            let col = schema()
                .table(table)
                .and_then(|d| {
                    d.fields
                        .iter()
                        .find(|f| f.footprint().contains(&off))
                        .map(|f| f.name())
                })
                .unwrap_or_else(|| "?".into());
            Err(err(format!(
                "record {rec} offset {off} (column {col}): compiled {:#04x}, .bin {:#04x}",
                bytes[i], live.records[i]
            )))
        }
    }
}

/// The outcome of C-TABLE step 2 over the runtime tables.
#[derive(Debug, Default)]
pub struct TableCheck {
    /// Tables that matched byte for byte.
    pub identical: Vec<String>,
    /// One failure per table that did not.
    pub failures: Vec<TextError>,
}

/// C-TABLE step 2 (§4.3): every runtime table compiled from the native
/// text set, with the overrides applied, against the live `.bin`.
pub fn check_tables(compiled: &CompiledSet, live: &BinSet, overrides: &BinOverrides) -> TableCheck {
    let mut out = TableCheck::default();
    for def in schema().runtime() {
        let r = match (compiled.table(&def.name), live.table(&def.name)) {
            (Some(c), Some(l)) => compare_table(&def.name, &c.compiled, l, overrides),
            _ => Err(TextError::new(
                &def.name,
                "missing from the compile or the live set",
            )),
        };
        match r {
            Ok(()) => out.identical.push(def.name.clone()),
            Err(e) => out.failures.push(e),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monstats(name_str: u16, count: usize) -> (Compiled, BinTable) {
        let def = schema().table("monstats").unwrap();
        let off = def.field("NameStr").unwrap().footprint().start;
        let mut records = vec![0u8; count * def.record_size];
        let put = |recs: &mut Vec<u8>, v: u16| {
            recs[707 * def.record_size + off..][..2].copy_from_slice(&v.to_le_bytes());
        };
        put(&mut records, 11154);
        let live_records = {
            let mut r = records.clone();
            put(&mut r, name_str);
            r
        };
        (
            Compiled {
                count,
                record_size: def.record_size,
                records,
                diagnostics: vec![],
            },
            BinTable {
                name: "monstats".into(),
                source: "d2data.mpq".into(),
                count,
                record_size: def.record_size,
                records: live_records,
            },
        )
    }

    // Covers: specs/formats/native-assets.md §2.8 r1, §7.1 r3
    #[test]
    fn excel_copy_is_exact_and_perturbation_is_named() {
        let src = b"a\tb\r\n1\t2\xff\r\n".to_vec();
        assert_eq!(write_excel(&src), src);
        check_excel("weapons.txt", &src, &write_excel(&src)).unwrap();
        let mut bad = src.clone();
        bad[9] = b'3';
        let e = check_excel("weapons.txt", &src, &bad).unwrap_err();
        assert!(
            e.detail.contains("byte 9") && e.detail.contains("line 2"),
            "{e}"
        );
        assert!(check_excel("weapons.txt", &src, &src[..5]).is_err());
    }

    // Covers: specs/formats/native-assets.md §2.8 r3, §4.3
    #[test]
    fn monstats_707_override_derives_applies_and_round_trips() {
        let (ours, live) = monstats(5382, 710);
        let o = derive_cell("monstats", 707, "NameStr", &ours, &live)
            .unwrap()
            .unwrap();
        assert_eq!(
            (o.record, o.field.as_str(), o.bytes.clone()),
            (707, "NameStr", 5382u16.to_le_bytes().to_vec())
        );
        let ov = BinOverrides { entries: vec![o] };
        let text = ov.to_toml();
        assert!(text.contains("bytes = \"0x0615\""), "{text}");
        assert_eq!(BinOverrides::from_toml(OVERRIDES_PATH, &text).unwrap(), ov);
        assert_eq!(text, ov.to_toml());
        compare_table("monstats", &ours, &live, &ov).unwrap();
        // Without the override the same cell is the reported difference.
        let e = compare_table("monstats", &ours, &live, &BinOverrides::default()).unwrap_err();
        assert!(e.detail.contains("column NameStr"), "{e}");
        // Identical bytes need no override.
        let (ours, live) = monstats(11154, 710);
        assert!(derive_cell("monstats", 707, "NameStr", &ours, &live)
            .unwrap()
            .is_none());
    }

    // Covers: specs/formats/native-assets.md §2.8 r3, §7.1 r3
    #[test]
    fn any_other_difference_fails() {
        let (ours, mut live) = monstats(5382, 710);
        let ov = BinOverrides {
            entries: vec![derive_cell("monstats", 707, "NameStr", &ours, &live)
                .unwrap()
                .unwrap()],
        };
        let size = ours.record_size;
        live.records[3 * size + 9] ^= 1;
        let e = compare_table("monstats", &ours, &live, &ov).unwrap_err();
        assert!(e.detail.contains("record 3 offset 9"), "{e}");
        let (ours, mut live) = monstats(11154, 710);
        live.count -= 1;
        live.records.truncate(live.count * size);
        assert!(compare_table("monstats", &ours, &live, &ov).is_err());
    }

    // Covers: specs/formats/native-assets.md §7.1 r5
    #[test]
    fn strict_overrides_reader() {
        let ov = BinOverrides {
            entries: vec![BinOverride {
                table: "monstats".into(),
                record: 707,
                field: "NameStr".into(),
                offset: 6,
                bytes: vec![6, 21],
            }],
        };
        let t = ov.to_toml();
        for bad in [
            t.replace("0x0615", "0x0615 "),
            t.replace("0x0615", "0x0A15".to_lowercase().replace("a", "A").as_str()),
            t.replace("0x0615", "0615"),
            t.replace("0x0615", "0x061"),
            t.replace("native_version = 1", "native_version = 3"),
            format!("{t}extra = 1\n"),
        ] {
            assert!(
                BinOverrides::from_toml(OVERRIDES_PATH, &bad).is_err(),
                "{bad}"
            );
        }
        // An override outside the table is refused when applied.
        let mut recs = vec![0u8; 8];
        assert!(ov.apply("monstats", 4, &mut recs).is_err());
        assert_eq!(BinOverrides::default().apply("x", 4, &mut recs).unwrap(), 0);
    }
}
