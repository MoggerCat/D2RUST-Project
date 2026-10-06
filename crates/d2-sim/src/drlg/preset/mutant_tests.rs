// Spec: specs/drlg/preset.md (mutation-testing tests)
//! Tests written to kill mutants `cargo mutants` left alive in
//! `drlg::preset`. Each asserts what `preset.md` says; the survivors no
//! test can observe are listed in `docs/handoff/mutants-drlg.md`.

use d2_data::tables::{Lvlprest, Record};

use super::data::{MonPresetRow, PresetDef, PresetTables};
use super::*;

// ---- data.rs ------------------------------------------------------------------

/// `preset.md` §13: the lvlprest columns the DRLG reads, `File1..6`
/// NUL-trimmed, `PopPad` and `Files` signed.
#[test]
fn preset_def_from_record() {
    let mut b = vec![0u8; Lvlprest::SIZE];
    let put = |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    for (k, at) in [0, 4, 8, 12, 16, 20, 24, 28, 40, 44, 48, 52, 56]
        .into_iter()
        .enumerate()
    {
        put(&mut b, at, 10 + k as u32);
    }
    put(&mut b, 60, (-4i32) as u32);
    put(&mut b, 64, 3);
    for (k, at) in [68, 128, 188, 248, 308, 368].into_iter().enumerate() {
        b[at..at + 3].copy_from_slice(format!("f{k}x").as_bytes()[..3].as_ref());
    }
    put(&mut b, 428, 0x55);
    let d = PresetDef::from_record(&Lvlprest::decode(&b));
    let want = PresetDef {
        def: 10,
        level_id: 11,
        populate: 12,
        logicals: 13,
        outdoors: 14,
        animate: 15,
        kill_edge: 16,
        fill_blanks: 17,
        size_x: 18,
        size_y: 19,
        automap: 20,
        scan: 21,
        pops: 22,
        pop_pad: -4,
        files: 3,
        file: [0, 1, 2, 3, 4, 5].map(|k| format!("f{k}x").into_bytes()),
        dt1_mask: 0x55,
        anim_speed: 0,
    };
    assert_eq!(d, want);
}

/// `preset.md` §5.3 (with `data/callbacks.md` §6): a monpreset record
/// holds the kind at +1 and the place index (u16) at +2.
#[test]
fn monpreset_row_from_bytes() {
    assert_eq!(
        MonPresetRow::from_record_bytes(&[9, 2, 0x34, 0x12]),
        MonPresetRow {
            kind: 2,
            place: 0x1234
        }
    );
}

/// `preset-tables.tsv` strict parse: door values (level, main, sub ≥ 0,
/// right 0 or 1, unit type ≥ 0) are checked, 0 is allowed, and errors
/// name the file line.
#[test]
fn preset_tables_door_ranges() {
    let header = "table\ta\tb\tc\td\te\tf\tg\th";
    let parse = |rows: &[&str]| PresetTables::from_tsv(&format!("{header}\n{}\n", rows.join("\n")));
    let t = parse(&[
        "door\t0\t0\t0\t0\t0\t5\t1\t-1",
        "door\t0\t1\t2\t1\t2\t6\t0\t0",
    ])
    .unwrap();
    assert_eq!(t.doors.len(), 1);
    assert_eq!(t.doors[0].1.len(), 2);
    for (k, bad) in [
        "door\t-1\t0\t0\t0\t0\t5\t1\t-1",
        "door\t0\t-1\t0\t0\t0\t5\t1\t-1",
        "door\t0\t0\t-1\t0\t0\t5\t1\t-1",
        "door\t0\t0\t0\t-1\t0\t5\t1\t-1",
        "door\t0\t0\t0\t2\t0\t5\t1\t-1",
        "door\t0\t0\t0\t0\t-1\t5\t1\t-1",
    ]
    .into_iter()
    .enumerate()
    {
        let ok = "door\t0\t0\t0\t0\t0\t5\t1\t-1";
        assert_eq!(
            parse(&[ok, ok, bad]),
            Err(PresetError::Tsv("line 4: door value out of range".into())),
            "case {k}"
        );
    }
}

// ---- ds1.rs ---------------------------------------------------------------------

use super::ds1::{Ds1File, Ds1Input, Ds1ObjectInput, ORIENTATION_REMAP};
use super::tests::{ds1, preset_data};

/// `preset.md` §5.2 at the version boundaries: act from v 8 (else 0),
/// tag type from v 10, the orientation remap below v 7, floors from v 4.
#[test]
fn ds1_version_boundaries() {
    let pd = preset_data();
    let file = |v: u32| {
        let mut i: Ds1Input = ds1(1, 1);
        i.version = v;
        i.act = 3;
        i.tag_type = 2;
        i.orientations[0][0] = 26;
        i.floors[0][0] = 7;
        Ds1File::from_input(&i, &pd).unwrap()
    };
    let (f7, f8) = (file(7), file(8));
    assert_eq!((f7.act, f8.act), (0, 3));
    let (f9, f10) = (file(9), file(10));
    assert_eq!((f9.tag_type, f10.tag_type), (0, 2));
    let (f6, f7) = (file(6), file(7));
    assert_eq!(f6.orientations[0][0], ORIENTATION_REMAP[26]);
    assert_eq!(f7.orientations[0][0], 26);
    let (f3, f4) = (file(3), file(4));
    assert!(f3.floors.is_empty());
    assert_eq!(f4.floors[0][0], 7);
}

/// `preset.md` §5.3: an object id ≥ 150 (v ≥ 6) becomes id − 150; the
/// monster → object conversions apply to monster records only.
#[test]
fn ds1_object_ids_and_conversions() {
    let pd = preset_data();
    let mut i: Ds1Input = ds1(1, 1);
    i.version = 18;
    i.act = 2;
    let obj = |kind, id| Ds1ObjectInput {
        kind,
        id,
        x: 1,
        y: 2,
        flags: 0,
    };
    // Object 150 → class 0; object 447 → class 297 (no conversion).
    i.objects = vec![obj(2, 150), obj(2, 447)];
    let f = Ds1File::from_input(&i, &pd).unwrap();
    // Head insertion: reverse file order.
    let got: Vec<_> = f
        .units
        .iter()
        .map(|u| (u.unit_type, u.class, u.mode))
        .collect();
    assert_eq!(got, [(2, 297, 0), (2, 0, 0)]);
}

// ---- map.rs ---------------------------------------------------------------------

use super::tests::World;

/// `preset.md` §6 r2, r10: in multi-room mode each room's link is the
/// map's link-grid value of its cell, the grid being (w/8 + 1) wide.
#[test]
fn build_area_links_by_cell() {
    let mut w = World::new();
    let l = w.level(2, 5, 16, 16);
    w.file(5, b"a.ds1", ds1(16, 16));
    w.init(l);
    let rooms = w
        .run(|p, d, c| {
            let rect = d.level(l).rect;
            let m = p.alloc_map(d, c, l, 5, rect)?;
            p.map_mut(m)?.link_grid = Some((1..=9).collect());
            p.build_area(d, c, l, m, 0, false)?;
            Ok::<_, PresetError>(d.level_rooms(l))
        })
        .unwrap();
    let mut links: Vec<(i32, i32, u32)> = rooms
        .iter()
        .map(|&r| {
            let rect = w.drlg.room(r).rect;
            (rect.x, rect.y, w.p.room(r).unwrap().link)
        })
        .collect();
    links.sort();
    assert_eq!(links, [(0, 0, 1), (0, 8, 4), (8, 0, 2), (8, 8, 5)]);
}

/// `preset.md` §3.2 r2: with direction −1 (`Files` = 0 at allocation, no
/// draw), the direction takes the map's picked file (0 here).
#[test]
fn generate_direction_from_the_picked_file() {
    let mut w = World::new();
    let l = w.level(2, 5, 8, 8);
    w.file(5, b"a.ds1", ds1(8, 8));
    w.pd.defs[5].files = 0;
    w.init(l);
    assert_eq!(w.p.info(l).unwrap().direction, -1);
    w.generate(l).unwrap();
    let info = w.p.info(l).unwrap();
    assert_eq!(info.direction, 0);
    let m = info.map.unwrap();
    assert_eq!(w.p.map(m).unwrap().picked_file, 0);
}
