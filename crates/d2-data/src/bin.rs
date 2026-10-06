// Spec: specs/data/loading.md
//! The live `.bin` set: path rule (§1), live-file resolution through the
//! archive set (§2), the `.bin` container and its size check (§4), the load
//! sequence (§6), server-file checks (§3.3), code buffers (§4.3), the
//! runtime `.txt` sound tables (§3.4), the post-load checks (§8) and the
//! d2rs count checks (§10.8). The records keep their shipped bytes;
//! fix-ups (§7.4) are applied to a copy by [`crate::fixup`].

use std::collections::BTreeMap;

use d2_formats::mpq::{ArchiveSet, MpqError};

use crate::calc::{self, BufferError, BufferReport, Family};
use crate::compile::CodeLinker;
use crate::schema::{schema, CalcBuffer, FieldType, Link, TableDef};
use crate::strings::{StringTables, StringsError};
use crate::txt::{TxtError, TxtTable};

/// `DATA\GLOBAL\EXCEL\` (§1).
pub const EXCEL_DIR: &str = "data\\global\\excel\\";
/// The language whose string tables are loaded.
pub const DEFAULT_LANGUAGE: &str = "eng";

/// The archive path of an excel file (`armor.bin` → `data\global\excel\armor.bin`).
pub fn excel_path(file: &str) -> String {
    format!("{EXCEL_DIR}{file}")
}

/// A load failure. Loading is all-or-nothing (§Outputs).
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("{file}: {source}")]
    Mpq { file: String, source: MpqError },
    #[error("{file}: not found in any archive")]
    Missing { file: String },
    #[error("{file}: {len} bytes, shorter than the 4-byte count")]
    TooShort { file: String, len: usize },
    #[error("{file}: {len} bytes, expected 4 + {count} x {record_size} = {expected}")]
    SizeMismatch {
        file: String,
        len: usize,
        count: u32,
        record_size: usize,
        expected: u64,
    },
    #[error("{file}: server-only file present (checked before `{table}`)")]
    ServerFile { file: String, table: String },
    #[error("{table}: {rule}")]
    Check { table: String, rule: String },
    #[error("{buffer}.bin: {detail}")]
    CodeBuffer {
        buffer: &'static str,
        detail: String,
    },
    #[error(transparent)]
    Strings(#[from] StringsError),
    #[error(transparent)]
    Txt(#[from] TxtError),
}

fn check(table: &str, rule: impl Into<String>) -> LoadError {
    LoadError::Check {
        table: table.to_owned(),
        rule: rule.into(),
    }
}

/// Reads an excel file through the archive set: `(archive, bytes)`, or
/// `None` when no archive has it.
pub fn read_excel(set: &ArchiveSet, file: &str) -> Result<Option<(String, Vec<u8>)>, LoadError> {
    let path = excel_path(file);
    set.read_with_source(&path)
        .map_err(|source| LoadError::Mpq { file: path, source })
}

/// A `.bin` record table (§4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinTable {
    pub name: String,
    /// Archive the file was read from (lowercase file name).
    pub source: String,
    pub count: usize,
    pub record_size: usize,
    /// `count × record_size` bytes.
    pub records: Vec<u8>,
}

impl BinTable {
    /// Splits a `.bin` and checks its size strictly (§4.2 rules 1–2).
    pub fn parse(
        name: &str,
        source: &str,
        file: &str,
        data: &[u8],
        record_size: usize,
    ) -> Result<BinTable, LoadError> {
        if data.len() < 4 {
            return Err(LoadError::TooShort {
                file: file.to_owned(),
                len: data.len(),
            });
        }
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        // Saturating: no .bin is near u64::MAX bytes, so a saturated size
        // never equals the length and stays a size mismatch.
        let expected = u64::from(count)
            .saturating_mul(record_size as u64)
            .saturating_add(4);
        if data.len() as u64 != expected {
            return Err(LoadError::SizeMismatch {
                file: file.to_owned(),
                len: data.len(),
                count,
                record_size,
                expected,
            });
        }
        Ok(BinTable {
            name: name.to_owned(),
            source: source.to_owned(),
            count: count as usize,
            record_size,
            records: data[4..].to_vec(),
        })
    }

    pub fn record(&self, i: usize) -> &[u8] {
        &self.records[i * self.record_size..(i + 1) * self.record_size]
    }

    pub fn iter(&self) -> impl Iterator<Item = &[u8]> {
        self.records.chunks_exact(self.record_size.max(1))
    }
}

/// Little-endian reads at record offsets.
pub fn u32_at(record: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        record[offset],
        record[offset + 1],
        record[offset + 2],
        record[offset + 3],
    ])
}

/// The NUL-terminated string at `range` of a record (without the NUL).
pub fn cstr(record: &[u8], range: std::ops::Range<usize>) -> &[u8] {
    let s = &record[range.start..range.end.min(record.len())];
    let n = s.iter().position(|&b| b == 0).unwrap_or(s.len());
    &s[..n]
}

/// A raw formula code buffer (§4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeFile {
    pub buffer: CalcBuffer,
    pub source: String,
    pub bytes: Vec<u8>,
}

/// The tables a code buffer's formulas come from (§4.3).
pub fn buffer_tables(buffer: CalcBuffer) -> &'static [&'static str] {
    match buffer {
        CalcBuffer::MissCode => &["missiles"],
        CalcBuffer::SkillsCode => &["skills"],
        CalcBuffer::SkillDescCode => &["skilldesc"],
        CalcBuffer::ItemsCode => &["weapons", "armor", "misc"],
    }
}

/// Code buffers loaded right after a table (§4.3).
fn buffers_after(table: &str) -> &'static [CalcBuffer] {
    match table {
        "missiles" => &[CalcBuffer::MissCode],
        "skilldesc" => &[CalcBuffer::SkillsCode, CalcBuffer::SkillDescCode],
        "misc" => &[CalcBuffer::ItemsCode],
        _ => &[],
    }
}

/// Server-only files that must be absent before a table loads (§3.3).
fn server_files(table: &str) -> &'static [&'static str] {
    match table {
        "runes" => &["runessrv.txt", "runessrv.bin", "runessrv.xls"],
        "cubemain" => &["cubeserver.bin", "cubeserver.txt"],
        _ => &[],
    }
}

/// Everything 1.14d reads at startup (d2-data policy 1), validated.
#[derive(Debug, Clone)]
pub struct BinSet {
    /// `d2exp.mpq` is present.
    pub lod: bool,
    pub strings: StringTables,
    /// The 73 record tables of §6, in load order.
    pub tables: Vec<BinTable>,
    pub code: BTreeMap<CalcBuffer, CodeFile>,
    /// Validation of each code buffer with its formula fields
    /// (`calc-expressions.md` §1.5).
    pub code_reports: BTreeMap<CalcBuffer, BufferReport>,
    /// `hitclass.bin`, read by the client composite loader (§3.5).
    pub hitclass: BinTable,
    /// The runtime `.txt` sound tables (§3.4).
    pub sounds: TxtTable,
    pub soundenviron: TxtTable,
}

impl BinSet {
    pub fn table(&self, name: &str) -> Option<&BinTable> {
        self.tables.iter().find(|t| t.name == name)
    }
}

fn load_bin(set: &ArchiveSet, def: &TableDef) -> Result<BinTable, LoadError> {
    let (source, bytes) = read_excel(set, &def.bin_name)?.ok_or_else(|| LoadError::Missing {
        file: excel_path(&def.bin_name),
    })?;
    BinTable::parse(
        &def.name,
        &source,
        &excel_path(&def.bin_name),
        &bytes,
        def.record_size,
    )
}

/// Loads and validates the live `.bin` set (§3.1, §4, §6, §8, §10.8).
pub fn load(set: &ArchiveSet, language: &str) -> Result<BinSet, LoadError> {
    let lod = set.has_archive("d2exp.mpq");
    let strings = StringTables::load(set, language, lod)?;
    let mut tables: Vec<BinTable> = Vec::new();
    let mut code = BTreeMap::new();
    for def in schema().runtime() {
        for file in server_files(&def.name) {
            if set.contains(&excel_path(file)) {
                return Err(LoadError::ServerFile {
                    file: excel_path(file),
                    table: def.name.clone(),
                });
            }
        }
        let table = load_bin(set, def)?;
        post_load_check(&table, &tables, &strings, lod)?;
        tables.push(table);
        for &buffer in buffers_after(&def.name) {
            let file = format!("{}.bin", buffer.name());
            let (source, bytes) = read_excel(set, &file)?.ok_or_else(|| LoadError::Missing {
                file: excel_path(&file),
            })?;
            code.insert(
                buffer,
                CodeFile {
                    buffer,
                    source,
                    bytes,
                },
            );
        }
    }

    let mut code_reports = BTreeMap::new();
    for (&buffer, file) in &code {
        let report = calc::validate_buffer(
            Family::of(buffer),
            &file.bytes,
            formula_fields(buffer, &tables),
        )
        .map_err(|e| LoadError::CodeBuffer {
            buffer: buffer.name(),
            detail: match e {
                BufferError::BadOpcode { offset, opcode } => {
                    format!("opcode {opcode:#04x} at {offset}")
                }
                BufferError::Truncated { offset } => format!("expression cut at {offset}"),
                BufferError::BadField { value } => {
                    format!("formula field {value} is not an expression start")
                }
                BufferError::MissileRand { offset } => format!("missile rand at {offset}"),
            },
        })?;
        code_reports.insert(buffer, report);
    }

    let hitclass_def = schema().table("hitclass").expect("hitclass in tables.tsv");
    let hitclass = load_bin(set, hitclass_def)?;

    let read_txt = |file: &str| -> Result<TxtTable, LoadError> {
        let (_, bytes) = read_excel(set, file)?.ok_or_else(|| LoadError::Missing {
            file: excel_path(file),
        })?;
        Ok(TxtTable::parse(&excel_path(file), &bytes)?)
    };
    Ok(BinSet {
        lod,
        sounds: read_txt("sounds.txt")?,
        soundenviron: read_txt("soundenviron.txt")?,
        strings,
        tables,
        code,
        code_reports,
        hitclass,
    })
}

/// The formula field values that point into `buffer`.
pub fn formula_fields(buffer: CalcBuffer, tables: &[BinTable]) -> Vec<u32> {
    let mut values = Vec::new();
    for name in buffer_tables(buffer) {
        let (Some(def), Some(table)) = (
            schema().table(name),
            tables.iter().find(|t| &t.name == name),
        ) else {
            continue;
        };
        let offsets: Vec<usize> = def
            .fields
            .iter()
            .filter(|f| f.field_type == FieldType::CalcToDword && f.link == Link::Calc(buffer))
            .map(|f| f.offset as usize)
            .collect();
        for rec in table.iter() {
            values.extend(offsets.iter().map(|&o| u32_at(rec, o)));
        }
    }
    values
}

/// The runtime item code map (§7.4): `code` (+0x80) of the combined
/// weapons, armor and misc records, added in order with the code-linker
/// add (`field-types.md` §6.6).
pub fn item_code_map(tables: &[BinTable]) -> CodeLinker {
    let mut map = CodeLinker::default();
    for name in ["weapons", "armor", "misc"] {
        if let Some(t) = tables.iter().find(|t| t.name == name) {
            for rec in t.iter() {
                map.add(u32_at(rec, 0x80));
            }
        }
    }
    map
}

/// Number of treasure classes the TC routine builds (§10.6): the empty
/// TC, 32 automatic TCs per itemtypes record with byte 0x1D ≠ 0, then the
/// `treasureclassex` rows up to the first empty name.
pub fn tc_count(itemtypes: &BinTable, treasureclassex: &BinTable) -> usize {
    let auto = itemtypes.iter().filter(|r| r[0x1D] != 0).count();
    let rows = treasureclassex.iter().take_while(|r| r[0] != 0).count();
    1 + 32 * auto + rows
}

/// automap LevelName list A (§8).
pub const AUTOMAP_LEVEL_NAMES: [&str; 36] = [
    "None",
    "1 Town",
    "1 Wilderness",
    "1 Cave",
    "1 Crypt",
    "1 Monestary",
    "1 Courtyard",
    "1 Barracks",
    "1 Jail",
    "1 Cathedral",
    "1 Catacombs",
    "1 Tristram",
    "2 Town",
    "2 Sewer",
    "2 Harem",
    "2 Basement",
    "2 Desert",
    "2 Tomb",
    "2 Lair",
    "2 Arcane",
    "3 Town",
    "3 Jungle",
    "3 Kurast",
    "3 Spider",
    "3 Dungeon",
    "3 Sewer",
    "4 Town",
    "4 Mesa",
    "4 Lava",
    "5 Town",
    "5 Siege",
    "5 Barricade",
    "5 Temple",
    "5 Ice",
    "5 Baal",
    "5 Lava",
];

/// automap TileName list B (§8).
pub const AUTOMAP_TILE_NAMES: [&str; 20] = [
    "fl", "wl", "wr", "wtlr", "wtll", "wtr", "wbl", "wbr", "wld", "wrd", "wle", "wre", "co", "sh",
    "tr", "rf", "ld", "rd", "fd", "fi",
];

/// The 1.14d post-load checks (§8) and the d2rs count checks (§10.8) for
/// `t`, given the tables loaded before it.
pub fn post_load_check(
    t: &BinTable,
    earlier: &[BinTable],
    strings: &StringTables,
    lod: bool,
) -> Result<(), LoadError> {
    let name = t.name.as_str();
    let get = |n: &str| {
        earlier
            .iter()
            .find(|e| e.name == n)
            .ok_or_else(|| check(name, format!("needs `{n}` loaded first")))
    };
    let max = |limit: usize| {
        if t.count > limit {
            Err(check(name, format!("count {} > {limit}", t.count)))
        } else {
            Ok(())
        }
    };
    let exactly = |n: usize| {
        if t.count != n {
            Err(check(name, format!("count {} != {n}", t.count)))
        } else {
            Ok(())
        }
    };
    match name {
        "pettype" | "states" => max(255),
        "itemstatcost" => max(511),
        "skills" | "skilldesc" | "uniqueitems" | "sets" | "setitems" | "monstats" | "monequip" => {
            max(32_766)
        }
        "levels" => max(1_023),
        "inventory" => exactly(32),
        "difficultylevels" => exactly(3),
        "arena" => exactly(1),
        "composit" => exactly(16),
        "armtype" => exactly(3),
        "experience" => exactly(101),
        "belts" => {
            if t.count / 2 != 7 {
                Err(check(name, format!("count {} / 2 != 7", t.count)))
            } else {
                Ok(())
            }
        }
        "leveldefs" => {
            let levels = get("levels")?.count;
            if t.count != levels {
                Err(check(
                    name,
                    format!("count {} != levels count {levels}", t.count),
                ))
            } else {
                Ok(())
            }
        }
        "chartemplate" => {
            let mut highest = 0u8;
            for (i, r) in t.iter().enumerate() {
                if r[0x1F] < highest {
                    return Err(check(
                        name,
                        format!("record {i}: Level {} below {highest}", r[0x1F]),
                    ));
                }
                highest = r[0x1F];
            }
            Ok(())
        }
        "gamble" => {
            let items = item_code_map(earlier);
            for (i, r) in t.iter().enumerate() {
                let code = u32_at(r, 0);
                if items.find(code).is_none() {
                    return Err(check(
                        name,
                        format!("record {i}: code {:?} is not an item", code.to_le_bytes()),
                    ));
                }
            }
            Ok(())
        }
        "treasureclassex" => {
            let n = tc_count(get("itemtypes")?, t);
            if n > 65_534 {
                Err(check(name, format!("{n} treasure classes > 65534")))
            } else {
                Ok(())
            }
        }
        "superuniques" => {
            let rows: Vec<u32> = t.iter().take(512).map(|r| u32_at(r, 0x08)).collect();
            match (0..66).find(|v| !rows.contains(v)) {
                Some(v) => Err(check(name, format!("hcIdx {v} occurs in no row"))),
                None => Ok(()),
            }
        }
        "hireling" if lod => {
            for (i, r) in t.iter().enumerate() {
                let id = u32_at(r, 0x04);
                let first = strings.id(cstr(r, 0xD3..t.record_size));
                let last = strings.id(cstr(r, 0xF3..t.record_size));
                if id > 255 || first == 0 || last <= first {
                    return Err(check(
                        name,
                        format!("record {i}: Id {id}, NameFirst id {first}, NameLast id {last}"),
                    ));
                }
            }
            Ok(())
        }
        "automap" => {
            for (i, r) in t.iter().enumerate() {
                let level = cstr(r, 0..16);
                let tile = cstr(r, 16..24);
                let known = |s: &[u8], list: &[&str]| {
                    s.is_empty() || list.iter().any(|n| n.as_bytes() == s)
                };
                if !known(level, &AUTOMAP_LEVEL_NAMES) || !known(tile, &AUTOMAP_TILE_NAMES) {
                    return Err(check(
                        name,
                        format!(
                            "record {i}: unknown LevelName `{}` or TileName `{}`",
                            String::from_utf8_lossy(level),
                            String::from_utf8_lossy(tile)
                        ),
                    ));
                }
                if u32_at(r, 0x1C) == u32::MAX {
                    return Err(check(name, format!("record {i}: Cel1 is -1")));
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bin(count: u32, size: usize, extra: usize) -> Vec<u8> {
        let mut d = count.to_le_bytes().to_vec();
        d.resize(4 + count as usize * size + extra, 0);
        d
    }

    fn table(name: &str, count: u32, size: usize) -> BinTable {
        BinTable::parse(name, "p", name, &bin(count, size, 0), size).unwrap()
    }

    // Covers: specs/data/loading.md §4.2 r1, §4.2 r2
    #[test]
    fn container_size_rules() {
        assert!(matches!(
            BinTable::parse("x", "p", "x", &[1, 0, 0], 4),
            Err(LoadError::TooShort { .. })
        ));
        let t = BinTable::parse("levels", "p", "levels.bin", &bin(137, 544, 0), 544).unwrap();
        assert_eq!((t.count, t.records.len()), (137, 137 * 544));
        assert_eq!(4 + 137 * 544, 74_532);
        // armor.bin from d2exp: 202 records of the 592-byte layout.
        let x = bin(202, 592, 0);
        assert_eq!(x.len(), 119_588);
        assert!(matches!(
            BinTable::parse("armor", "x", "armor.bin", &x, 424),
            Err(LoadError::SizeMismatch {
                expected: 85_652,
                ..
            })
        ));
        assert!(BinTable::parse("a", "p", "a", &bin(1, 4, 1), 4).is_err());
    }

    // Covers: specs/data/loading.md §8, §10 r8
    #[test]
    fn count_checks() {
        let s = StringTables::default();
        let ok = |t: &BinTable, e: &[BinTable]| post_load_check(t, e, &s, true).is_ok();
        assert!(ok(&table("inventory", 32, 240), &[]));
        assert!(!ok(&table("inventory", 31, 240), &[]));
        assert!(!ok(&table("inventory", 33, 240), &[]));
        assert!(ok(&table("belts", 14, 264), &[]));
        assert!(ok(&table("belts", 15, 264), &[]));
        assert!(!ok(&table("belts", 16, 264), &[]));
        assert!(!ok(&table("difficultylevels", 4, 88), &[]));
        assert!(!ok(&table("experience", 100, 32), &[]));
        assert!(ok(&table("pettype", 255, 224), &[]));
        assert!(!ok(&table("pettype", 256, 224), &[]));
        let levels = table("levels", 137, 544);
        assert!(ok(
            &table("leveldefs", 137, 156),
            std::slice::from_ref(&levels)
        ));
        assert!(!ok(&table("leveldefs", 136, 156), &[levels]));
    }

    // Covers: specs/data/loading.md §8
    #[test]
    fn chartemplate_levels() {
        let s = StringTables::default();
        let mut t = table("chartemplate", 2, 240);
        t.records[0x1F] = 9;
        t.records[240 + 0x1F] = 3;
        assert!(post_load_check(&t, &[], &s, true).is_err());
        t.records[240 + 0x1F] = 9;
        assert!(post_load_check(&t, &[], &s, true).is_ok());
    }

    // Covers: specs/data/loading.md §8
    #[test]
    fn automap_names() {
        let s = StringTables::default();
        let mut t = table("automap", 1, 44);
        t.records[..6].copy_from_slice(b"1 Town");
        t.records[16..18].copy_from_slice(b"fl");
        assert!(post_load_check(&t, &[], &s, true).is_ok());
        t.records[..6].copy_from_slice(b"6 Town");
        assert!(post_load_check(&t, &[], &s, true).is_err());
        t.records[..6].copy_from_slice(b"1 Town");
        t.records[0x1C..0x20].copy_from_slice(&[0xFF; 4]);
        assert!(post_load_check(&t, &[], &s, true).is_err());
    }

    // Covers: specs/data/loading.md §8
    #[test]
    fn superunique_coverage() {
        let s = StringTables::default();
        let mut t = table("superuniques", 66, 52);
        for i in 0..66 {
            t.records[i * 52 + 8] = i as u8;
        }
        assert!(post_load_check(&t, &[], &s, true).is_ok());
        t.records[5 * 52 + 8] = 70;
        assert!(post_load_check(&t, &[], &s, true).is_err());
    }

    #[test]
    fn check_offsets_match_schema() {
        let s = schema();
        let f = |t: &str, c: &str| s.table(t).unwrap().field(c).unwrap().offset;
        assert_eq!(f("chartemplate", "level"), 0x1F);
        assert_eq!(f("superuniques", "hcIdx"), 0x08);
        assert_eq!(f("hireling", "id"), 0x04);
        assert_eq!(f("hireling", "namefirst"), 0xD3);
        assert_eq!(f("hireling", "namelast"), 0xF3);
        assert_eq!(f("automap", "LevelName"), 0);
        assert_eq!(f("automap", "TileName"), 16);
        assert_eq!(f("automap", "Cel1"), 0x1C);
        assert_eq!(f("itemtypes", "treasureclass"), 0x1D);
        assert_eq!(f("weapons", "code"), 0x80);
        assert_eq!(f("gamble", "code"), 0);
    }

    fn game_set() -> ArchiveSet {
        let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
        ArchiveSet::open_dir(dir).expect("archives open")
    }

    /// `loading.md` test vectors on the 1.14d install.
    // Covers: specs/data/loading.md §4.3, §6, §11
    #[test]
    #[ignore = "needs original game files in D2_GAME_DIR"]
    fn live_bin_set_loads() {
        let set = game_set();
        let data = load(&set, DEFAULT_LANGUAGE).expect("live .bin set loads");
        assert_eq!(data.tables.len(), 73);
        let t = |n: &str| data.table(n).unwrap();
        assert_eq!((t("levels").count, t("leveldefs").count), (137, 137));
        assert_eq!(
            (t("inventory").source.as_str(), t("inventory").count),
            ("d2exp.mpq", 32)
        );
        assert_eq!(t("armor").source, "patch_d2.mpq");
        assert_eq!(t("compcode").record(0), b"nil ");
        assert_eq!(data.code[&CalcBuffer::MissCode].bytes.len(), 196);
        assert_eq!(data.code[&CalcBuffer::SkillsCode].bytes.len(), 5_891);
        assert_eq!(data.code[&CalcBuffer::SkillDescCode].bytes.len(), 4_252);
        assert_eq!(data.code[&CalcBuffer::ItemsCode].bytes.len(), 158);
        assert_eq!(data.code_reports[&CalcBuffer::SkillsCode].paren, 1);
        assert_eq!(data.hitclass.count, 14);
        assert_eq!(data.sounds.records.len(), 4_699);
        assert_eq!(data.soundenviron.records.len(), 50);
        // Every table lives where tables.tsv says.
        for def in schema().runtime() {
            let (archive, _) = def.live_source.as_ref().unwrap();
            assert_eq!(
                t(&def.name).source,
                format!("{archive}.mpq"),
                "{}",
                def.name
            );
        }
        // monstats record 0 TreasureClass1 = 161 + 269.
        let m = t("monstats").record(0);
        assert_eq!(u16::from_le_bytes([m[0x86], m[0x87]]), 430);
        assert_eq!(tc_count(t("itemtypes"), t("treasureclassex")), 1_013);
    }

    // Covers: specs/data/loading.md §2
    #[test]
    #[ignore = "needs original game files in D2_GAME_DIR"]
    fn excel_lookup_order() {
        let set = game_set();
        let src = |f: &str| read_excel(&set, f).unwrap().map(|(s, b)| (s, b.len()));
        assert_eq!(src("inventory.bin").unwrap().0, "d2exp.mpq");
        assert_eq!(src("inventory.txt"), Some(("patch_d2.mpq".into(), 8_843)));
        assert_eq!(src("leveldefs.txt"), None);
        assert_eq!(src("armor.bin"), Some(("patch_d2.mpq".into(), 85_652)));
        let sounds = read_excel(&set, "sounds.bin").unwrap().unwrap().1;
        let t = BinTable::parse("sounds", "p", "sounds.bin", &sounds, 2).unwrap();
        assert_eq!(t.count, 4_699);
        assert!(t
            .iter()
            .enumerate()
            .all(|(i, r)| r == (i as u16).to_le_bytes()));
    }
}
