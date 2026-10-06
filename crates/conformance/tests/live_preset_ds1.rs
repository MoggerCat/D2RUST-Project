// Spec: specs/drlg/preset.md §5.2, §5.3, §6 step 5 and 7, Test vectors
//! DS1 survey through the preset parser rules (HANDOFF §5 C entry 13):
//! every DS1 named by lvlprest, converted to `Ds1Input` by the server's
//! provider and run through `Ds1File::from_input` with the live
//! `PresetData`. `#[ignore]`, reads `D2_GAME_DIR`.

use std::collections::BTreeMap;

use d2_formats::ds1::Ds1;
use d2_formats::mpq::ArchiveSet;
use d2_server::world_data::tables::LevelTables;
use d2_server::world_data::{archive, archive_name, ds1_input, names_file};
use d2_sim::drlg::preset::{Ds1File, Ds1Input};

/// Tile-info entries of `preset.md` §6 step 7 (style 30..33): DS1-relative
/// (x, y, value).
type Tiles = Vec<(i32, i32, u32)>;
/// Pops after the finish of step 8: (style, sub, group, x, y, w, h).
type Corner = (i32, i32);
type Pops = Vec<(u32, u32, u32, i32, i32, i32, i32)>;

/// The scan of §6 steps 7–8 on a file's wall layers (independent of the
/// sim's `build_area`, which needs a whole DRLG).
fn scan(f: &Ds1File, scan: bool, pops: bool) -> (Tiles, Pops) {
    let (w, h) = (f.width as usize, f.height as usize);
    let stride = f.stride();
    let mut tiles = Vec::new();
    // style, sub, corner 1, corner 2
    let mut p: Vec<(u32, u32, Corner, Corner)> = Vec::new();
    for (walls, orients) in f.walls.iter().zip(&f.orientations) {
        for y in 0..h {
            for x in 0..w {
                let (o, v) = (orients[y * stride + x], walls[y * stride + x]);
                if o != 10 && o != 11 {
                    continue;
                }
                let (st, sb) = ((v >> 20) & 0x3F, (v >> 8) & 0xFF);
                let (xi, yi) = (x as i32, y as i32);
                if pops && (8..=29).contains(&st) {
                    match p.iter_mut().find(|e| e.0 == st) {
                        Some(e) => e.3 = (xi, yi),
                        None => p.push((st, sb, (xi, yi), (0, 0))),
                    }
                }
                if scan && (30..=33).contains(&st) {
                    let value = match st {
                        30 => sb,
                        31 => sb + 5,
                        32 => 10,
                        _ => 11,
                    };
                    tiles.push((xi, yi, value));
                }
            }
        }
    }
    let pops = p
        .into_iter()
        .map(|(st, sb, c1, c2)| {
            let (x0, x1) = (c1.0.min(c2.0), c1.0.max(c2.0));
            let (y0, y1) = (c1.1.min(c2.1), c1.1.max(c2.1));
            (st, sb, st / 4 - 1, x0, y0, x1 - x0 + 1, y1 - y0 + 1)
        })
        .collect();
    (tiles, pops)
}

struct Survey {
    tables: LevelTables,
    /// Table string -> stored input.
    inputs: BTreeMap<Vec<u8>, Ds1Input>,
    files: BTreeMap<Vec<u8>, Ds1File>,
}

fn survey() -> Survey {
    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
    let set = ArchiveSet::open_dir(dir).expect("archives open");
    let fixed = archive::fixed_tables(&set).expect("fixed tables");
    let tables = LevelTables::from_fixed(&fixed).expect("level tables");
    let mut inputs = BTreeMap::new();
    let mut files = BTreeMap::new();
    for path in tables.preset.defs.iter().flat_map(|d| d.file.iter()) {
        if !names_file(path) || inputs.contains_key(path) {
            continue;
        }
        let name = String::from_utf8(archive_name(path)).expect("ascii");
        let bytes = set.read(&name).unwrap_or_else(|e| panic!("{name}: {e}"));
        let d = Ds1::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let input = ds1_input(path, &d).unwrap_or_else(|e| panic!("{name}: {e}"));
        let file = Ds1File::from_input(&input, &tables.preset)
            .unwrap_or_else(|e| panic!("{name}: parser rules: {e:?}"));
        files.insert(path.clone(), file);
        inputs.insert(path.clone(), input);
    }
    Survey {
        tables,
        inputs,
        files,
    }
}

fn find<'a>(s: &'a Survey, suffix: &str) -> (&'a Vec<u8>, &'a Ds1File) {
    let want = suffix.to_ascii_lowercase().replace('/', "\\");
    let hits: Vec<_> = s
        .files
        .iter()
        .filter(|(k, _)| {
            let k = String::from_utf8_lossy(k)
                .to_ascii_lowercase()
                .replace('/', "\\");
            k.ends_with(&want)
        })
        .collect();
    assert_eq!(hits.len(), 1, "{suffix}: {} hits", hits.len());
    hits[0]
}

/// Kept object-unit counts by class id for classes >= 573.
fn kept_high(files: &BTreeMap<Vec<u8>, Ds1File>) -> BTreeMap<i32, usize> {
    let mut kept = BTreeMap::new();
    for f in files.values() {
        for u in &f.units {
            if u.unit_type == 2 && u.class >= 573 {
                *kept.entry(u.class).or_default() += 1;
            }
        }
    }
    kept
}

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn preset_ds1_survey() {
    let s = survey();
    let pd = &s.tables.preset;
    assert_eq!(s.inputs.len(), 2043, "DS1 files named by lvlprest");
    assert!(s.inputs.values().all(|i| (12..=18).contains(&i.version)));

    // Records before conversion.
    let (mut monsters, mut objects, mut other) = (0, 0, 0);
    let mut flagged = Vec::new();
    for (path, i) in &s.inputs {
        for o in &i.objects {
            match o.kind {
                1 => monsters += 1,
                2 => objects += 1,
                _ => other += 1,
            }
            if o.flags != 0 {
                flagged.push((path.clone(), o.flags));
            }
        }
    }
    assert_eq!((monsters, objects, other), (2267, 14105, 0));
    assert_eq!(flagged.len(), 1, "{flagged:?}");
    assert_eq!(flagged[0].1, 1);

    // Kept class ids >= 573 after conversion.
    let kept = kept_high(&s.files);
    assert_eq!(
        (kept.get(&580), kept.get(&581), kept.get(&582)),
        (Some(&46), Some(&135), Some(&24)),
        "{kept:?}"
    );

    // Every DS1 size equals its row's SizeX / SizeY (1,054 rows set them).
    let mut sized_rows = 0;
    for (n, d) in pd.defs.iter().enumerate() {
        if d.size_x == 0 || d.size_y == 0 {
            continue;
        }
        sized_rows += 1;
        for path in d.file.iter().filter(|p| names_file(p)) {
            let f = &s.files[path];
            assert_eq!(
                (f.width, f.height),
                (d.size_x, d.size_y),
                "Def {n} {}",
                String::from_utf8_lossy(path)
            );
        }
    }
    assert_eq!(sized_rows, 1054);

    // Every DrlgType 2 level's size equals its DS1 size.
    let mut checked = 0;
    for (id, lv) in s.tables.drlg.levels.iter().enumerate() {
        if lv.drlg_type != 2 {
            continue;
        }
        let def = pd.def_for_level(id as u32).expect("preset level has a row");
        let d = pd.def(def).unwrap();
        for path in d.file.iter().filter(|p| names_file(p)) {
            let f = &s.files[path];
            for (diff, &(w, h)) in lv.size.iter().enumerate() {
                assert_eq!(
                    (f.width as i32, f.height as i32),
                    (w, h),
                    "level {id} diff {diff} {}",
                    String::from_utf8_lossy(path)
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn preset_ds1_scan_vectors() {
    let s = survey();
    let pd = &s.tables.preset;
    let row = |path: &[u8]| {
        pd.defs
            .iter()
            .enumerate()
            .find(|(_, d)| d.file.iter().any(|p| p == path))
            .expect("row names the file")
    };

    // TownN1.ds1 at origin (X, Y): tile info in order.
    let (path, f) = find(&s, "townn1.ds1");
    let (_, d) = row(path);
    let (tiles, _) = scan(f, d.scan != 0, d.pops != 0);
    assert_eq!(tiles, [(26, 7, 0), (28, 7, 10), (30, 14, 11)]);

    // Act2/Town/LutN.ds1 (Def 301, Pops 3).
    let (path, f) = find(&s, "act2/town/lutn.ds1");
    let (def, d) = row(path);
    assert_eq!((def, d.pops), (301, 3));
    let (tiles, pops) = scan(f, d.scan != 0, d.pops != 0);
    assert_eq!(tiles.len(), 5, "{tiles:?}");
    assert_eq!(
        pops,
        [
            (8, 8, 1, 24, 3, 7, 7),
            (13, 13, 2, 10, 27, 4, 5),
            (12, 13, 2, 15, 33, 3, 3)
        ]
    );

    // MetroTemple2.ds1 (Def 647): style 8 rect (2, 2, 5x10), style 9 seen
    // once -> rect (0, 0, 12x7); both group 1.
    let (path, f) = find(&s, "metrotemple2.ds1");
    let (def, d) = row(path);
    assert_eq!(def, 647);
    let (_, pops) = scan(f, d.scan != 0, d.pops != 0);
    let got: Vec<_> = pops
        .iter()
        .map(|p| (p.0, p.2, p.3, p.4, p.5, p.6))
        .collect();
    assert_eq!(got, [(8, 1, 2, 2, 5, 10), (9, 1, 0, 0, 12, 7)]);
}

/// M08: the scan reports exactly the one input that is changed.
#[test]
fn scan_reports_the_planted_cells() {
    let (w, h) = (8usize, 8usize);
    let stride = w + 1;
    let mut f = Ds1File {
        width: w as u32,
        height: h as u32,
        walls: vec![vec![0; stride * (h + 1)]],
        orientations: vec![vec![0; stride * (h + 1)]],
        ..Ds1File::default()
    };
    let cell = |style: u32, sub: u32| (style << 20) | (sub << 8);
    let put = |f: &mut Ds1File, x: usize, y: usize, style: u32, sub: u32, o: u32| {
        f.walls[0][y * stride + x] = cell(style, sub);
        f.orientations[0][y * stride + x] = o;
    };
    put(&mut f, 1, 2, 31, 3, 10); // tile info, value sub + 5
    put(&mut f, 2, 2, 8, 8, 11); // pop style 8, first corner
    put(&mut f, 5, 4, 8, 8, 10); // second corner
    put(&mut f, 7, 7, 12, 1, 3); // wrong orientation: ignored
    let (tiles, pops) = scan(&f, true, true);
    assert_eq!(tiles, [(1, 2, 8)]);
    assert_eq!(pops, [(8, 8, 1, 2, 2, 4, 3)]);
    // Perturbation: the second corner moves one column right.
    put(&mut f, 5, 4, 0, 0, 0);
    put(&mut f, 6, 4, 8, 8, 10);
    let (_, pops) = scan(&f, true, true);
    assert_eq!(pops, [(8, 8, 1, 2, 2, 5, 3)]);
}
