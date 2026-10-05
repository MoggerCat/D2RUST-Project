//! File-format tests against the user's install. Ignored by default; run
//! with `cargo test -p d2-formats -- --ignored`. The full every-file check
//! is `cargo run --release -p mpq-tool -- formats`.

use std::path::PathBuf;

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
fn parse_all(archive_name: &str, ext: &str, parse: impl Fn(&str, &[u8])) -> usize {
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

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn string_and_font_tables() {
    let a = archive("d2data.mpq");
    let t = StringTable::parse(&a.read(r"data\local\lng\eng\string.tbl").unwrap()).unwrap();
    assert!(t.indices.len() > 5000);
    let f = FontTable::parse(&a.read(r"data\local\font\latin\font16.tbl").unwrap()).unwrap();
    assert_eq!(f.glyphs.len(), 256);
}

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn all_dc6_in_d2data() {
    let n = parse_all("d2data.mpq", ".dc6", |name, b| {
        Dc6::parse(b).unwrap_or_else(|e| panic!("{name}: {e}"));
    });
    assert!(n > 1000);
}

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn all_cof_in_d2exp() {
    let n = parse_all("d2exp.mpq", ".cof", |name, b| {
        Cof::parse(b).unwrap_or_else(|e| panic!("{name}: {e}"));
    });
    assert!(n > 100);
}

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

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn sample_dcc() {
    let a = archive("d2data.mpq");
    let dcc = Dcc::parse(&a.read(r"data\global\missiles\poisonNova.dcc").unwrap()).unwrap();
    let d0 = &dcc.directions[0];
    assert_eq!(d0.frames.len(), dcc.frames_per_direction as usize);
    assert!(d0.frames.iter().any(|f| f.pixels.iter().any(|&p| p != 0)));
}
