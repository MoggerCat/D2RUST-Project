//! File-format tests against the user's install. Ignored by default; run
//! with `cargo test -p d2-formats -- --ignored`. The full every-file check
//! is `cargo run --release -p mpq-tool -- formats`.

use std::path::PathBuf;

use d2_formats::animdata::{AnimData, AnimRecord};
use d2_formats::cof::Cof;
use d2_formats::dc6::Dc6;
use d2_formats::dcc::Dcc;
use d2_formats::ds1::Ds1;
use d2_formats::dt1::Dt1;
use d2_formats::font::FontTable;
use d2_formats::mpq::Archive;
use d2_formats::palette::{Palette, Pl2};
use d2_formats::tbl::StringTable;

fn archive(name: &str) -> Archive {
    let dir = PathBuf::from(std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set"));
    let path = std::fs::read_dir(&dir)
        .expect("D2_GAME_DIR readable")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
        .unwrap_or_else(|| panic!("{name} not found"));
    Archive::open(path).unwrap()
}

/// Parses every listed file in `archive` whose name ends with `ext`.
fn parse_all(archive_name: &str, ext: &str, mut parse: impl FnMut(&str, &[u8])) -> usize {
    let a = archive(archive_name);
    let names = a.listfile().unwrap().expect("archive has a (listfile)");
    let mut count = 0;
    for name in names
        .iter()
        .filter(|n| n.to_ascii_lowercase().ends_with(ext))
    {
        parse(name, &a.read(name).unwrap());
        count += 1;
    }
    count
}

// Covers: specs/formats/palette.md §dat-palette, §pl2-palette-transform
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn palettes() {
    let a = archive("d2data.mpq");
    for act in 1..=5 {
        let dir = format!(r"data\global\palette\act{act}");
        Palette::parse(&a.read(&format!(r"{dir}\pal.dat")).unwrap()).unwrap();
        Pl2::parse(&a.read(&format!(r"{dir}\pal.pl2")).unwrap()).unwrap();
    }
}

// Covers: specs/formats/tbl.md §header-21-bytes, §strings; specs/formats/font-tbl.md §header-12-bytes, §glyph-records-14-bytes-each
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn string_and_font_tables() {
    let a = archive("d2data.mpq");
    let t = StringTable::parse(&a.read(r"data\local\lng\eng\string.tbl").unwrap()).unwrap();
    assert!(t.indices.len() > 5000);
    let f = FontTable::parse(&a.read(r"data\local\font\latin\font16.tbl").unwrap()).unwrap();
    assert_eq!(f.glyphs.len(), 256);
}

// Covers: specs/formats/dc6.md §file-header-24-bytes, §frame, §pixel-decoding
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn all_dc6_in_d2data() {
    let n = parse_all("d2data.mpq", ".dc6", |name, b| {
        Dc6::parse(b).unwrap_or_else(|e| panic!("{name}: {e}"));
    });
    assert!(n > 1000);
}

// Covers: specs/formats/cof.md §header-28-bytes, §layer-records-l-9-bytes, §frame-events-and-draw-order
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn all_cof_in_d2exp() {
    let n = parse_all("d2exp.mpq", ".cof", |name, b| {
        Cof::parse(b).unwrap_or_else(|e| panic!("{name}: {e}"));
    });
    assert!(n > 100);
}

// Covers: specs/formats/ds1.md §rules; specs/formats/dt1.md §file-header-276-bytes, §tile-header-96-bytes-each-consecutive, §block-header-20-bytes-each-at-the-tile-s-block-headers-offset, §block-pixels
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn all_ds1_and_dt1_in_d2exp() {
    parse_all("d2exp.mpq", ".ds1", |name, b| {
        Ds1::parse(b).unwrap_or_else(|e| panic!("{name}: {e}"));
    });
    parse_all("d2exp.mpq", ".dt1", |name, b| {
        Dt1::parse(b).unwrap_or_else(|e| panic!("{name}: {e}"));
    });
}

// Covers: specs/formats/dcc.md §file-header-little-endian-bytes, §direction-header-bits, §boxes, §cells, §stage-1-cell-colors-all-frames-in-order, §stage-2-building-frames-all-frames-in-order-after-stage-1, §end-checks
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn sample_dcc() {
    let a = archive("d2data.mpq");
    let dcc = Dcc::parse(&a.read(r"data\global\missiles\poisonNova.dcc").unwrap()).unwrap();
    let d0 = &dcc.directions[0];
    assert_eq!(d0.frames.len(), dcc.frames_per_direction as usize);
    assert!(d0.frames.iter().any(|f| f.pixels.iter().any(|&p| p != 0)));
}

// Covers: specs/formats/dc6.md §edge-cases-original-bugs
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn dc6_zero_size_frames() {
    let mut zero = 0;
    // patch_d2.mpq has no (listfile); X and D list 367 + 1,284 DC6.
    for a in ["d2exp.mpq", "d2data.mpq"] {
        parse_all(a, ".dc6", |name, b| {
            for f in Dc6::parse(b)
                .unwrap_or_else(|e| panic!("{name}: {e}"))
                .frames
            {
                if f.width == 0 || f.height == 0 {
                    assert!(f.pixels.is_empty(), "{name}");
                    zero += 1;
                }
            }
        });
    }
    assert_eq!(zero, 0, "no live DC6 frame has width or height 0");
}

/// Reads `name` from the first of `archives` that has it.
fn read_first(archives: &[&Archive], name: &str) -> Option<Vec<u8>> {
    archives
        .iter()
        .find(|a| a.contains(name))
        .map(|a| a.read(name).unwrap())
}

// Covers: specs/formats/dt1.md §edge-cases-original-bugs
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn dt1_zero_block_tiles_and_unused_version_4_files() {
    // Tiles with 0 blocks occur.
    let mut empty_tiles = 0;
    parse_all("d2exp.mpq", ".dt1", |name, b| {
        let dt1 = Dt1::parse(b).unwrap_or_else(|e| panic!("{name}: {e}"));
        empty_tiles += dt1.tiles.iter().filter(|t| t.blocks.is_empty()).count();
    });
    assert!(empty_tiles > 0);

    // The six version-4 files in d2data, and only those.
    let (p, x, d) = (
        archive("patch_d2.mpq"),
        archive("d2exp.mpq"),
        archive("d2data.mpq"),
    );
    let v4 = [
        r"ACT1\BARRACKS\barracks.dt1",
        r"ACT1\BARRACKS\gargtrap.dt1",
        r"ACT1\CATACOMB\Catacombs.dt1",
        r"ACT1\CATHEDRL\Cathedrl.dt1",
        r"ACT1\COURT\Court.dt1",
        r"ACT1\OUTDOORS\Outdoor1.dt1",
    ];
    let mut found = Vec::new();
    for name in d.listfile().unwrap().expect("listfile") {
        if !name.to_ascii_lowercase().ends_with(".dt1") {
            continue;
        }
        let b = d.read(&name).unwrap();
        let major = i32::from_le_bytes(b[..4].try_into().unwrap());
        if major != 7 {
            assert_eq!(major, 4, "{name}");
            assert!(Dt1::parse(&b).is_err(), "{name}: not supported");
            found.push(name.to_ascii_lowercase());
        }
    }
    found.sort();
    let mut want: Vec<String> = v4
        .iter()
        .map(|f| format!(r"data\global\tiles\{f}").to_ascii_lowercase())
        .collect();
    want.sort();
    assert_eq!(found, want);

    // No level table, classic or expansion, references them.
    let mut tables = 0;
    for t in ["LvlTypes", "LvlPrest", "LvlSub", "LvlMaze", "LvlWarp"] {
        for ext in ["txt", "bin"] {
            let path = format!(r"data\global\excel\{t}.{ext}");
            for a in [&p, &x, &d] {
                let Some(bytes) = read_first(&[a], &path) else {
                    continue;
                };
                tables += 1;
                let text = String::from_utf8_lossy(&bytes)
                    .to_ascii_lowercase()
                    .replace('/', "\\");
                for f in v4 {
                    // "act1\barracks\barracks.dt1" → "barracks\barracks.dt1".
                    let tail = f.to_ascii_lowercase();
                    let tail = tail.split_once('\\').unwrap().1;
                    assert!(!text.contains(tail), "{path} references {f}");
                }
            }
        }
    }
    assert!(tables >= 5, "level tables found: {tables}");
}

// Covers: specs/formats/tbl.md §edge-cases-original-bugs
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn tbl_crc_and_classification() {
    let a = archive("d2data.mpq");
    // The crc is not verified: any value parses to the same table.
    let path = r"data\local\lng\eng\string.tbl";
    let data = a.read(path).unwrap();
    let t = StringTable::parse(&data).unwrap();
    let mut d = data.clone();
    d[0] ^= 0xFF;
    d[1] ^= 0x5A;
    assert_eq!(StringTable::parse(&d).unwrap().entries, t.entries);

    // Classification by content over every listed .tbl.
    let is_text = |b: &[u8]| {
        b.iter()
            .all(|&c| c == b'\t' || c == b'\r' || c == b'\n' || (0x20..0x7F).contains(&c))
    };
    let mut text_files = Vec::new();
    let (mut fonts, mut strings) = (0, 0);
    for name in a.listfile().unwrap().expect("listfile") {
        if !name.to_ascii_lowercase().ends_with(".tbl") {
            continue;
        }
        let b = a.read(&name).unwrap();
        if FontTable::is_font_table(&b) {
            FontTable::parse(&b).unwrap_or_else(|e| panic!("{name}: {e}"));
            fonts += 1;
        } else if is_text(&b) {
            text_files.push(name.to_ascii_lowercase());
        } else {
            StringTable::parse(&b).unwrap_or_else(|e| panic!("{name}: {e}"));
            strings += 1;
        }
    }
    text_files.sort();
    assert_eq!(
        text_files,
        [
            r"data\local\font\latin\default.tbl",
            r"data\local\font\latin\fonter.tbl"
        ]
    );
    assert!(fonts > 0 && strings > 0);
}

/// The AnimData.d2 copy the game loads (the expansion one, §1).
fn animdata() -> AnimData {
    let x = archive("d2exp.mpq");
    AnimData::parse(&x.read(d2_formats::animdata::PATH).unwrap()).unwrap()
}

/// The `.cof` of an AnimData name, from the archives in search order.
fn cof_for(name: &str) -> Option<Cof> {
    let archives = [
        archive("patch_d2.mpq"),
        archive("d2exp.mpq"),
        archive("d2char.mpq"),
        archive("d2data.mpq"),
    ];
    let refs: Vec<&Archive> = archives.iter().collect();
    ["monsters", "chars", "objects"].iter().find_map(|dir| {
        let path = format!(r"data\global\{dir}\{}\cof\{name}.cof", &name[..2]);
        read_first(&refs, &path).map(|b| Cof::parse(&b).unwrap())
    })
}

fn name_of(r: &AnimRecord) -> String {
    let n = r.name.iter().position(|&b| b == 0).unwrap_or(8);
    String::from_utf8_lossy(&r.name[..n]).into_owned()
}

// Covers: specs/formats/animdata.md §edge-cases-original-bugs
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn animdata_edge_cases() {
    // Parsing succeeds, so no count is negative (d2rs rejects those).
    let a = animdata();
    let all: Vec<&AnimRecord> = a.buckets.iter().flatten().collect();

    // Duplicates: 29 names twice, same bucket; 20 identical, 9 differ.
    let mut by_name: std::collections::BTreeMap<String, Vec<(usize, &AnimRecord)>> =
        Default::default();
    for (b, bucket) in a.buckets.iter().enumerate() {
        for r in bucket {
            by_name.entry(name_of(r)).or_default().push((b, r));
        }
    }
    let dups: Vec<_> = by_name.iter().filter(|(_, v)| v.len() > 1).collect();
    assert_eq!(dups.len(), 29);
    let mut differ = Vec::new();
    for (name, v) in &dups {
        assert_eq!(v.len(), 2, "{name}");
        assert_eq!(v[0].0, v[1].0, "{name}: same bucket");
        if v[0].1 != v[1].1 {
            differ.push(name.as_str());
        }
        // The game uses the first copy.
        assert!(std::ptr::eq(a.record(name.as_bytes()).unwrap(), v[0].1));
    }
    let mut want = [
        "VMS1HTH", "VMGHHTH", "MINUHTH", "VMWLHTH", "VMNUHTH", "64A1HTH", "64NUHTH", "VMA1HTH",
        "3DNUHTH",
    ];
    want.sort();
    assert_eq!(differ, want);
    // The .cof matches the second copy in 6 and the first copy in 3.
    for name in want {
        let copy = match name {
            "64A1HTH" | "64NUHTH" | "MINUHTH" => 0,
            _ => 1,
        };
        let matching = by_name[name][copy].1;
        let cof = cof_for(name).unwrap_or_else(|| panic!("{name}: no .cof"));
        let mut events = cof.events.clone();
        events.resize(144, 0);
        assert_eq!(matching.frames, u32::from(cof.frames), "{name}");
        assert_eq!(matching.speed, cof.animation_rate, "{name}");
        assert_eq!(matching.events[..], events[..], "{name}");
    }
    let vms1 = &by_name["VMS1HTH"];
    assert_eq!((vms1[0].1.speed, vms1[1].1.speed), (200, 160));

    // Frames above 144: only 42DTHTH (200); the event scan stops at 144.
    let over: Vec<String> = all
        .iter()
        .filter(|r| r.frames > 144)
        .map(|r| name_of(r))
        .collect();
    assert_eq!(over, ["42DTHTH"]);
    let i = a.info(b"42DTHTH").unwrap();
    assert_eq!((i.frames, i.first_event), (200, 144));

    // Speed 0: 4 records, kept as read.
    assert_eq!(all.iter().filter(|r| r.speed == 0).count(), 4);

    // No record name is longer than 7 (8 would need a 9-byte query).
    assert!(all.iter().all(|r| r.name[7] == 0));
}
