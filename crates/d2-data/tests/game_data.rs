// Spec: specs/data/txt-format.md, specs/data/field-types.md, specs/data/calc-expressions.md (1.14d data vectors)
//! Game-file tests: they need the 1.14d install in `D2_GAME_DIR`.

use std::sync::OnceLock;

use d2_data::bin::{self, excel_path, read_excel};
use d2_data::compile_set::{compile_all, CompiledSet};
use d2_data::schema::{schema, CalcBuffer};
use d2_data::strings::StringTables;
use d2_data::txt::{bind, ErrorCode, TxtTable};
use d2_formats::mpq::{archive_file_name, Archive, ArchiveSet};

fn set() -> &'static ArchiveSet {
    static SET: OnceLock<ArchiveSet> = OnceLock::new();
    SET.get_or_init(|| {
        let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
        ArchiveSet::open_dir(dir).expect("archives open")
    })
}

fn compiled() -> &'static CompiledSet {
    static C: OnceLock<CompiledSet> = OnceLock::new();
    C.get_or_init(|| {
        let strings = StringTables::load(set(), "eng", true).expect("string tables");
        let mut read = |f: &str| read_excel(set(), f).map_err(|e| e.to_string());
        compile_all(&mut read, &strings).expect("text set compiles")
    })
}

fn record(table: &str, i: usize) -> &'static [u8] {
    compiled().table(table).unwrap().compiled.record(i)
}

fn offset(table: &str, column: &str) -> usize {
    schema().table(table).unwrap().field(column).unwrap().offset as usize
}

fn u16_at(r: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([r[o], r[o + 1]])
}

fn u32_at(r: &[u8], o: usize) -> u32 {
    bin::u32_at(r, o)
}

fn archive(name: &str) -> &'static Archive {
    set()
        .archives()
        .iter()
        .find(|a| archive_file_name(a).eq_ignore_ascii_case(name))
        .unwrap()
}

fn txt(archive_name: &str, file: &str) -> TxtTable {
    let bytes = archive(archive_name).read(&excel_path(file)).unwrap();
    TxtTable::parse(file, &bytes).unwrap()
}

/// `txt-format.md` survey: every excel `.txt` parses except `Aiparms.txt`
/// (E8, line 13), and record counts equal the same-archive `.bin` count
/// (128 pairs).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn txt_survey() {
    let mut names_p: Vec<String> = schema()
        .tables
        .iter()
        .flat_map(|t| [t.txt_name.clone(), t.bin_name.replace(".bin", ".txt")])
        .chain(["soundenviron.txt".to_owned(), "inventory.txt".to_owned()])
        .filter(|n| !n.is_empty())
        .collect();
    names_p.sort();
    names_p.dedup();
    let (mut files, mut pairs) = (0, 0);
    for name in ["patch_d2.mpq", "d2exp.mpq", "d2data.mpq"] {
        let a = archive(name);
        let names: Vec<String> = match a.listfile().unwrap() {
            Some(list) => list
                .into_iter()
                .filter(|n| {
                    let l = n.to_ascii_lowercase();
                    l.starts_with("data\\global\\excel\\") && l.ends_with(".txt")
                })
                .collect(),
            None => names_p
                .iter()
                .map(|n| excel_path(n))
                .filter(|p| a.contains(p))
                .collect(),
        };
        for path in names {
            let bytes = a.read(&path).unwrap();
            files += 1;
            match TxtTable::parse(&path, &bytes) {
                Err(e) => {
                    assert!(path.to_ascii_lowercase().ends_with("aiparms.txt"), "{e}");
                    assert_eq!((e.code, e.line), (ErrorCode::E8, Some(13)));
                }
                Ok(t) => {
                    let bin_path = format!("{}.bin", &path[..path.len() - 4]);
                    if let Ok(b) = a.read(&bin_path) {
                        pairs += 1;
                        let count = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
                        assert_eq!(t.records.len(), count, "{name} {path}");
                    }
                }
            }
        }
    }
    assert_eq!(files, 193);
    assert_eq!(pairs, 128);
}

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn txt_binding_vectors() {
    let armor = txt("patch_d2.mpq", "armor.txt");
    assert_eq!((armor.records.len(), armor.removed_lines.len()), (202, 1));
    let b = bind(&armor.header, &["mindam", "maxdam"]);
    assert_eq!(b.field_column, [Some(63), Some(64)]);
    assert!(b.duplicate_columns.contains(&161) && b.duplicate_columns.contains(&162));
    assert_eq!((b.column_field[161], b.column_field[162]), (None, None));

    let weapons = txt("patch_d2.mpq", "weapons.txt");
    assert!(weapons.header[18].is_empty());

    let objgroup = txt("d2exp.mpq", "objgroup.txt");
    assert_eq!(objgroup.records.len(), 133);
    assert_eq!(objgroup.records[97].line, 99);
    assert_eq!(objgroup.records[97].cells[0], b"EXPANSION");

    let unique = txt("patch_d2.mpq", "uniqueitems.txt");
    assert_eq!(unique.records.len(), 402);
    assert!(unique.records[401].cells.iter().all(Vec::is_empty));
    assert_eq!(unique.records[401].cells.len(), 70);

    let itemtypes = txt("patch_d2.mpq", "itemtypes.txt");
    assert_eq!(
        (itemtypes.records.len(), itemtypes.removed_lines.len()),
        (103, 1)
    );
}

/// `field-types.md` / `txt-format.md` 1.14d data vectors, on the compiled
/// records.
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn field_vectors() {
    assert_eq!(record("misc", 45)[314], 0xF0);
    assert_eq!(record("misc", 37)[252], 0xE7);
    assert_eq!(record("skills", 159)[8] & 0x10, 0x10);
    assert_eq!(record("missiles", 82)[4] & 0x02, 0x02);
    assert_eq!(u16_at(record("missiles", 568), 14), 0xFDB4);
    assert_eq!(&record("objects", 0)[192..195], b"NU\0");
    assert_eq!(&record("weapons", 0)[128..136], b"hax hax ");
    assert_eq!(u16_at(record("magicprefix", 444), 106), 26);
    assert_eq!(u16_at(record("magicsuffix", 433), 108), 10);
    assert_eq!(u16_at(record("magicprefix", 0), 106), 0);
    assert_eq!(u16_at(record("armor", 0), 244), 1930);
    assert_eq!(u16_at(record("misc", 1), 244), 5382);
    for (r, v) in [(617, 617), (723, 617), (724, 723), (733, 732)] {
        assert_eq!(u16_at(record("monstats", r), 0), v, "monstats {r}");
    }
    assert_eq!(u16_at(record("monseq", 0), 0), 0);
    assert_eq!(u16_at(record("monseq", 1), 0), 1);
    assert_eq!(u16_at(record("monseq", 1009), 0), 59);
    assert_eq!(u32_at(record("runes", 0), 152), 617);
    assert_eq!(u16_at(record("monstats", 0), 134), 430);
    assert_eq!(
        u32_at(record("runes", 27), offset("runes", "t1param4")),
        155
    );
    assert_eq!(u32_at(record("runes", 6), offset("runes", "t1param4")), 41);
    assert_eq!(record("armor", 22)[254], 1);
    for i in 0..137 {
        assert_eq!(u16_at(record("levels", i), 74), 0xFFFF);
    }
    for i in 0..202 {
        assert_eq!(record("armor", i)[316], 0xFF);
    }
    for i in 0..306 {
        assert_eq!(u16_at(record("weapons", i), 182), 0);
        assert_eq!(u32_at(record("weapons", i), 188), 0);
        assert_eq!(u32_at(record("weapons", i), 164), u32::MAX);
    }
    for i in 0..151 {
        assert_eq!(record("misc", i)[96], 0);
    }
    assert!(record("objgroup", 97).iter().all(|&b| b == 0));
    let skills_pettype = (0..357)
        .filter(|&i| record("skills", i)[190] == 0xFF)
        .count();
    assert_eq!(skills_pettype, 326); // field-types.md vector, corrected by the bin cross-check
                                     // itemtypes empty codes keep "    " in the record.
    for i in [0, 1, 14, 17, 23] {
        assert_eq!(&record("itemtypes", i)[..4], b"    ");
    }
    // uniqueitems last record (all cells empty).
    let r = record("uniqueitems", 401);
    assert_eq!(&r[40..44], b"    ");
    assert_eq!(&r[56..58], [0xFF, 0xFF]);
    assert_eq!(&r[140..144], [0xFF; 4]);
}

/// `calc-expressions.md` real formulas: field value and bytes.
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn formula_vectors() {
    let buf = |b: CalcBuffer| &compiled().buffers[&b];
    let expr = |b: CalcBuffer, start: u32| {
        let code = buf(b);
        let s = start as usize;
        let mut i = s;
        loop {
            let op = code[i];
            i += 1 + match op {
                0x01 | 0x04 | 0x07 => 1,
                0x05 | 0x08 => 2,
                0x06 | 0x09 => 4,
                _ => 0,
            };
            if op == 0 {
                return code[s..i].to_vec();
            }
        }
    };
    let hex = |s: &str| -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    };
    let cases: [(&str, usize, &str, u32, &str); 18] = [
        ("missiles", 12, "DmgCalc1", 0, "04 29 00"),
        ("missiles", 38, "SrvCalc1", 6, "07 00 00"),
        (
            "missiles",
            230,
            "EDmgSymPerCalc",
            24,
            "07 07 07 29 01 03 07 05 12 00",
        ),
        (
            "missiles",
            455,
            "EDmgSymPerCalc",
            78,
            "08 E1 00 07 29 01 03 07 08 12 00",
        ),
        ("missiles", 648, "SrvCalc1", 193, "07 03 00"),
        ("misc", 0, "calc1", 0, "07 05 00"),
        ("misc", 5, "len", 3, "08 EE 02 00"),
        ("misc", 94, "calc1", 155, "07 19 00"),
        (
            "skills",
            7,
            "EDmgSymPerCalc",
            0,
            "07 10 07 29 01 03 04 0F 12 00",
        ),
        ("skills", 12, "calc1", 36, "07 18 04 00 01 00 00"),
        ("skills", 326, "calc2", 5793, "08 FA 00 04 00 01 01 00"),
        ("skills", 282, "calc1", 5512, "04 0A 04 0B 01 02 00"),
        (
            "skills",
            62,
            "passivecalc1",
            1212,
            "08 49 01 07 00 01 05 00",
        ),
        (
            "skills",
            106,
            "calc2",
            2508,
            "04 10 07 05 0A 07 00 04 10 07 04 11 04 0B 12 16 07 60 07 29 01 03 04 0F 12 10 00",
        ),
        (
            "skills",
            51,
            "EDmgSymPerCalc",
            950,
            "07 25 07 29 01 03 04 0F 12 07 29 07 29 01 03 04 0E 12 10 02 00",
        ),
        ("skills", 78, "calc2", 1701, "04 0A 00"),
        (
            "skilldesc",
            166,
            "dsc2calca1",
            2971,
            "08 ED 00 07 10 01 03 07 00 0B 08 ED 00 07 00 01 03 07 00 16 00",
        ),
        (
            "skilldesc",
            219,
            "desccalca5",
            4201,
            "08 37 02 07 16 01 04 07 4B 12 08 00 01 13 00",
        ),
    ];
    for (table, rec, column, value, bytes) in cases {
        let field = schema().table(table).unwrap().field(column).unwrap();
        let d2_data::schema::Link::Calc(b) = field.link else {
            panic!("{table} {column} is not a calc field")
        };
        let v = u32_at(record(table, rec), field.offset as usize);
        assert_eq!(v, value, "{table} {rec} {column}");
        assert_eq!(expr(b, v), hex(bytes), "{table} {rec} {column}");
    }
    let f = offset("skilldesc", "desccalcb2");
    assert_eq!(u32_at(record("skilldesc", 95), f), u32::MAX);
    let lens: Vec<usize> = CalcBuffer::ALL.iter().map(|&b| buf(b).len()).collect();
    assert_eq!(lens, [196, 5_891, 4_252, 158]);
}

/// The comparison is not vacuous: perturbed compiled bytes are reported
/// against the right field or unwritten byte, also in a table with table
/// callbacks (their bytes are compiled now, `callbacks.md`).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn crosscheck_catches_perturbations() {
    use d2_data::crosscheck::compare_sets;
    let data = bin::load(set(), bin::DEFAULT_LANGUAGE).unwrap();
    let mut c = compiled().clone();
    let mut poke = |table: &str, rec: usize, offset: usize| {
        let t = c.tables.iter_mut().find(|t| t.name == table).unwrap();
        let at = rec * t.compiled.record_size + offset;
        t.compiled.records[at] ^= 0x5A;
    };
    poke("armor", 22, 254); // mindam
    poke("pettype", 3, 5); // unwritten byte
    poke("monstats", 10, 10); // unwritten byte of a table with callbacks
    let report = compare_sets(set(), &data, &c).unwrap();
    let t = |n: &str| report.tables.iter().find(|t| t.name == n).unwrap();
    let armor = t("armor");
    assert!(!armor.matches());
    assert_eq!(armor.mismatches.keys().collect::<Vec<_>>(), ["mindam"]);
    assert_eq!(armor.mismatches["mindam"].examples[0].record, 22);
    assert_eq!(
        t("pettype").mismatches.keys().collect::<Vec<_>>(),
        ["unwritten byte +5"]
    );
    assert_eq!(
        t("monstats").mismatches.keys().collect::<Vec<_>>(),
        ["unwritten byte +10"]
    );
    assert_eq!(
        report.tables.iter().filter(|t| !t.matches()).count(),
        3,
        "only the three perturbed tables differ"
    );
}

/// `callbacks.md` test vectors taken from 1.14d records.
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn callback_vectors() {
    let cube = |rec: usize, at: usize| record("cubemain", rec)[at..at + 8].to_vec();
    assert_eq!(cube(15, 28), [0x02, 0, 0x5d, 0, 0, 0, 0, 3]); // "gem2,qty=3"
    assert_eq!(cube(11, 20), [0x02, 0, 0x1c, 0, 0, 0, 0, 0]); // axe
    assert_eq!(cube(62, 36), [0x41, 0, 0x0a, 0x02, 0x7b, 0, 7, 0]); // SoJ
    assert_eq!(record("cubemain", 2)[76 + 8], 1); // Cow Portal
    assert_eq!(record("monstats2", 0)[37], 49); // skeleton1 total
    assert_eq!(&record("monstats2", 0)[22..23], [3]); // TRv count
    assert_eq!(&record("monstats2", 0)[50..53], [1, 2, 4]); // lit,med,hvy
    assert_eq!(&record("monpreset", 0)[1..4], [1, 0x93, 0]); // gheed
    assert_eq!(&record("monpreset", 40)[1..4], [2, 6, 0]); // The Countess
    assert_eq!(&record("monpreset", 39)[1..4], [2, 5, 0]); // Griswold
    let m45 = record("monstats", 45);
    assert!((0..8).any(|i| m45[384 + i] == 4)); // A1 with skill 321
}

/// The applied `loading.md` §7.4 fix-ups on the live set.
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn fixups_on_live_set() {
    let data = bin::load(set(), bin::DEFAULT_LANGUAGE).unwrap();
    let anim = d2_data::fixup::read_animdata(set()).unwrap();
    let f = d2_data::fixup::apply(&data, &anim).unwrap();
    assert_eq!(f.uniques.len(), 402);
    assert_eq!(f.uniques.find(b"the stone of jordan"), Some(122));
    assert_eq!(f.sets.len(), 127);
    assert_eq!(f.item_codes.len(), 306 + 202 + 151);
    assert!(f.superunique_hc.iter().all(Option::is_some));
    let u = f.table("uniqueitems").unwrap();
    for (i, r) in u.iter().enumerate() {
        assert_eq!(u16_at(r, 0), i as u16);
        assert_ne!(u16_at(r, 0x22), 0);
    }
    for r in f.table("hireling").unwrap().iter() {
        assert!(u16_at(r, 0x116) > u16_at(r, 0x114) && u16_at(r, 0x114) != 0);
    }
    for r in f.table("missiles").unwrap().iter() {
        assert!(r[0x183] <= 8);
    }
    let pets: usize = f
        .table("pettype")
        .unwrap()
        .iter()
        .map(|r| usize::from(u16_at(r, 0xBC)))
        .sum();
    assert!(pets > 0);
    // The shipped bytes stay untouched in the loaded set.
    assert_eq!(u16_at(data.table("uniqueitems").unwrap().record(5), 0), 0);
    // fixups.md / runtime-maps.md real vectors.
    let ms = f.table("monstats").unwrap();
    let speeds = |r: usize| {
        let m = ms.record(r);
        (u16_at(m, 0x36), u16_at(m, 0x38), m[0x4A], m[0x4B])
    };
    assert_eq!(speeds(0), (128, 64, 7, 0));
    assert_eq!(speeds(443), (138, 329, 5, 2));
    assert_eq!(speeds(671), (208, 374, 5, 3));
    assert_eq!(speeds(684), (256, 256, 11, 10));
    let golem = f.table("pettype").unwrap().record(3);
    let list: Vec<u16> = (0..4).map(|k| u16_at(golem, 0xC0 + 2 * k)).collect();
    assert_eq!((u32_at(golem, 0xBC), list), (4, vec![75, 85, 90, 94]));
    let lvlsub = f.table("lvlsub").unwrap().record(0);
    assert!(lvlsub[4..].starts_with(b"DATA\\GLOBAL\\TILES\\Act1\\Outdoors\\BorderCliffs.ds1\0"));
    assert_eq!(u32_at(f.table("objects").unwrap().record(1), 0xD8), 256);
    assert_eq!((f.stat_stuff, f.stat_mask), (6, 0x3F));
    assert_eq!(f.stat_desc_list.len(), 207);
    assert_eq!(&f.stat_desc_list[..4], [91, 252, 204, 253]);
    assert_eq!(f.skill_lists.counts, [30; 7]);
    assert_eq!(
        f.portals,
        [1, 3, 5, 7, 27, 29, 33, 36, 40, 43, 45, 46, 53, 54, 74, 134]
    );
    assert_eq!(
        f.lvlsub_types,
        [0, 1, 2, 3, 4, 6, 10, 16, 17, 21, 28, 31, 33]
    );
    assert_eq!(f.monpreset.count, [47, 59, 39, 28, 56]);
    let g = f.gamble.index.as_ref().unwrap();
    assert_eq!((g.len(), &g[..3]), (125, &[520, 522, 25][..]));
    assert_eq!(&f.gamble.thresholds[..4], [2, 10, 11, 17]);
    assert_eq!(f.automap.records.len(), 3286);
    assert_eq!(f.automap.ranges[1], (0, 83));
}

/// Every live table decodes with its generated struct (`schema.md`).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn typed_tables_decode() {
    use d2_data::tables::{decode_all, decode_by_name, text, Uniqueitems, Weapons};
    let data = bin::load(set(), bin::DEFAULT_LANGUAGE).unwrap();
    for t in &data.tables {
        let n = decode_by_name(t)
            .expect("runtime table")
            .expect("right size");
        assert_eq!(n, t.count, "{}", t.name);
    }
    let w: Vec<Weapons> = decode_all(data.table("weapons").unwrap()).unwrap();
    assert_eq!(w.len(), 306);
    assert_eq!(&w[0].code, b"hax ");
    let u: Vec<Uniqueitems> = decode_all(data.table("uniqueitems").unwrap()).unwrap();
    assert_eq!(text(&u[122].index), b"The Stone of Jordan");
    assert_eq!((&u[122].code, u[122].lvl), (b"rin ", 39));
}
