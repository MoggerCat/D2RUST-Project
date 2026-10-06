// Spec: specs/drlg/levels.md, specs/drlg/preset.md, specs/drlg/maze.md, specs/drlg/outdoor.md, specs/drlg/outdoor-tilesub.md, specs/drlg/outdoor-act3-act5.md (table-level game-file checks)
//! Game-file tests of the DRLG table views: they need the 1.14d install in
//! `D2_GAME_DIR` (the MPQ set, loaded and fixed up as the server loads it).
//! Every expected value is a fact or a recorded vector of the specs above;
//! nothing here reads a DS1 (the DS1 / DT1 providers belong to `d2-server`).

use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use d2_data::bin;
use d2_data::fixup::{self, FixedSet};
use d2_data::tables::{
    decode_all, Leveldefs, Lvlmaze, Lvlprest, Lvlsub, Lvltypes, Lvlwarp, Objects, Record,
};
use d2_formats::mpq::ArchiveSet;
use d2_sim::drlg::maze::{Maze, MazeData};
use d2_sim::drlg::outdoor::{OutdoorData, SubFileMap};
use d2_sim::drlg::preset::{
    Ds1Input, Ds1Source, MonPresetRow, PresetData, PresetDef, PresetTables,
};
use d2_sim::drlg::{Drlg, DrlgData, LevelIdx, TileRect};
use d2_sim::rng::Seed;
use d2_sim::wiring::worldgen::WorldTypes;

// ---- live tables ---------------------------------------------------------------------

fn fixed() -> &'static FixedSet {
    static F: OnceLock<FixedSet> = OnceLock::new();
    F.get_or_init(|| {
        let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
        let set = ArchiveSet::open_dir(dir).expect("archives open");
        let data = bin::load(&set, bin::DEFAULT_LANGUAGE).expect("live set loads");
        let anim = fixup::read_animdata(&set).expect("AnimData.d2");
        fixup::apply(&data, &anim).expect("fix-ups apply")
    })
}

fn rows<T: Record>() -> Vec<T> {
    let t = fixed().table(T::TABLE).expect("table loaded");
    decode_all(t).expect("table decodes")
}

fn count(table: &str) -> u32 {
    fixed().table(table).expect("table loaded").count as u32
}

fn drlg_data() -> DrlgData {
    DrlgData::from_tables(
        &rows::<Leveldefs>(),
        &rows::<Lvlwarp>(),
        &rows::<Lvltypes>(),
        &rows::<Objects>(),
    )
}

fn preset_data() -> PresetData {
    let f = fixed();
    PresetData {
        defs: rows::<Lvlprest>()
            .iter()
            .map(PresetDef::from_record)
            .collect(),
        monpreset_acts: f.monpreset,
        monpreset: f
            .table("monpreset")
            .expect("monpreset loaded")
            .iter()
            .map(|r| MonPresetRow::from_record_bytes(&[r[0], r[1], r[2], r[3]]))
            .collect(),
        monstats_count: count("monstats"),
        superuniques_count: count("superuniques"),
        hdm_item: f
            .item_codes
            .find(u32::from_le_bytes(*b"hdm "))
            .map_or(-1, |i| i as i32),
        tables: PresetTables::spec().expect("preset-tables.tsv parses"),
    }
}

/// No DS1 files: allocation and act placement read none (`preset.md`
/// §3.1, `outdoor.md` §2); a request would be a test failure, not a skip.
struct NoDs1;

impl Ds1Source for NoDs1 {
    fn ds1(&self, _: &[u8]) -> Option<&Ds1Input> {
        None
    }
}

fn world_types(data: &Arc<DrlgData>) -> WorldTypes {
    let lvlmaze = rows::<Lvlmaze>();
    let lvlprest = rows::<Lvlprest>();
    WorldTypes::new(
        data.clone(),
        Maze::new(MazeData::from_tables(&lvlmaze, &lvlprest)),
        preset_data(),
        OutdoorData::from_tables(&rows::<Leveldefs>(), &lvlprest, &rows::<Lvlsub>()),
        Box::new(NoDs1),
        Box::new(SubFileMap::default()),
    )
}

// ---- spec facts ----------------------------------------------------------------------

/// `preset.md` Constants: the 35 DrlgType 2 levels.
const PRESET_LEVELS: [u32; 35] = [
    1, 13, 14, 15, 16, 20, 25, 26, 27, 32, 33, 37, 38, 40, 50, 73, 75, 90, 91, 93, 94, 95, 96, 97,
    98, 99, 102, 103, 109, 120, 121, 124, 131, 132, 136,
];

/// `preset.md` Constants: `Files` of the DrlgType 2 levels (1 for the rest).
fn preset_files(level: u32) -> i32 {
    match level {
        1 | 40 => 0,
        25 | 95..=99 => 2,
        27 | 94 => 3,
        124 => 4,
        90 | 91 => 6,
        _ => 1,
    }
}

/// `drlg/levels.md` Test vectors: the recorded Act I DRLG.
const INIT: u32 = 644_409_375;
const START: u32 = 4_014_346_869;

// ---- leveldefs: every level's DrlgType dispatches ------------------------------------

/// `levels.md` §4.3–§4.4: types 1, 2, 3 dispatch (level 0 "Null" has 0);
/// a maze level without an lvlmaze row (`maze.md` §1.1) or a preset level
/// without its lvlprest row (`preset.md` §3.1 rule 1) is fatal, and so is
/// a warp slot without an lvlwarp row (`levels.md` §7.4).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn every_level_drlg_type_dispatches() {
    let data = drlg_data();
    let maze = MazeData::from_tables(&rows::<Lvlmaze>(), &rows::<Lvlprest>());
    let pd = preset_data();
    assert_eq!(data.levels[0].drlg_type, 0, "level 0 (Null)");
    let mut presets = Vec::new();
    for (id, l) in data.levels.iter().enumerate().skip(1) {
        let id = id as u32;
        match l.drlg_type {
            1 => {
                maze.row_index(id)
                    .unwrap_or_else(|e| panic!("maze level {id}: {e:?}"));
            }
            2 => {
                presets.push(id);
                let claims = pd.defs.iter().filter(|d| d.level_id == id).count();
                assert_eq!(claims, 1, "lvlprest rows claiming level {id}");
            }
            3 => {}
            t => panic!("level {id}: DrlgType {t} does not dispatch"),
        }
        for (slot, &w) in l.warp.iter().enumerate() {
            if w != -1 {
                data.lvlwarp_row(w, b'b')
                    .unwrap_or_else(|e| panic!("level {id} warp slot {slot} ({w}): {e:?}"));
            }
        }
    }
    assert_eq!(presets, PRESET_LEVELS);
}

/// `levels.md` §10 rule 2: the levels with leveldefs `Position` ≠ 0.
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn leveldefs_position_levels() {
    let data = drlg_data();
    let got: Vec<u32> = (0..data.levels.len() as u32)
        .filter(|&id| data.levels[id as usize].position != 0)
        .collect();
    assert_eq!(
        got,
        [1, 38, 40, 46, 54, 73, 74, 75, 102, 103, 109, 121, 125, 126, 127, 132, 134, 135]
    );
}

// ---- lvlprest ------------------------------------------------------------------------

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn lvlprest_def_is_the_row_number() {
    let pd = preset_data();
    assert_eq!(pd.defs.len(), 1_091);
    for i in 0..pd.defs.len() as u32 {
        assert_eq!(pd.def(i).expect("row in range").def, i, "row {i}");
    }
    assert!(pd.def(1_091).is_err());
}

/// `preset.md` Constants (lvlprest measurements, the DrlgType 2 levels)
/// and §3.2 rule 4 (`AutoMap`), §13 (files beyond `Files`).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn lvlprest_measurements() {
    let pd = preset_data();
    let defs = &pd.defs;
    let n = |f: &dyn Fn(&PresetDef) -> bool| defs.iter().filter(|d| f(d)).count();
    let files: Vec<usize> = (0..=6).map(|k| n(&|d| d.files == k)).collect();
    assert_eq!(files, [37, 627, 187, 127, 80, 21, 12]);
    assert_eq!(n(&|d| d.scan == 1), 292);
    assert_eq!(n(&|d| d.pops != 0), 35);
    assert!(defs.iter().all(|d| d.pops <= 4));
    assert_eq!(n(&|d| d.pop_pad == -4), 130);
    assert_eq!(n(&|d| d.pop_pad == -5), 1);
    let animate: Vec<u32> = defs
        .iter()
        .filter(|d| d.animate == 1)
        .map(|d| d.def)
        .collect();
    let expected: Vec<u32> = (836..=862).chain(1053..=1058).collect();
    assert_eq!(animate, expected);
    let automap: Vec<(u32, u32)> = defs
        .iter()
        .filter(|d| d.automap == 1)
        .map(|d| (d.def, d.level_id))
        .collect();
    assert_eq!(
        automap,
        [(1, 1), (301, 40), (529, 75), (797, 103), (863, 109)]
    );
    assert_eq!(n(&|d| d.populate == 0), 62);
    // A slot "names a file" when it has more than one character:
    // `data/fixups.md` §12, the 0/1-character strings are the 5,321 `0`
    // placeholders (the server's `world_data::names_file` rule). Counting
    // non-empty slots counted the placeholders (1,079 = every row with
    // `Files` < 6).
    let beyond = n(&|d| {
        let k = d.files.max(0) as usize;
        d.file.iter().skip(k).any(|f| f.len() > 1)
    });
    assert_eq!(beyond, 82);
    for &id in &PRESET_LEVELS {
        let d = pd.def(pd.def_for_level(id).unwrap()).unwrap();
        assert_eq!(d.files, preset_files(id), "Files of level {id}");
        assert_eq!((d.size_x, d.size_y), (0, 0), "size of level {id}");
        if id == 26 {
            assert_eq!((d.def, d.scan, d.pops), (165, 0, 0), "level 26");
        } else {
            assert_eq!(d.scan, 1, "Scan of level {id}");
        }
    }
}

// ---- lvlmaze -------------------------------------------------------------------------

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn lvlmaze_rows_as_stated() {
    let data = drlg_data();
    let maze = rows::<Lvlmaze>();
    assert_eq!(maze.len(), 81);
    let not_maze: BTreeSet<u32> = maze
        .iter()
        .map(|r| r.level)
        .filter(|&l| data.levels[l as usize].drlg_type != 1)
        .collect();
    let expected: BTreeSet<u32> = [0, 13, 14, 15, 16, 25, 37, 90, 91, 93, 132].into();
    assert_eq!(not_maze, expected);
    for &l in expected.iter().filter(|&&l| l != 0) {
        assert_eq!(data.levels[l as usize].drlg_type, 2, "level {l}");
    }
}

// ---- lvlsub --------------------------------------------------------------------------

/// `outdoor-tilesub.md` Constants and the recorded Blood Moor room.
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn lvlsub_rows_as_stated() {
    let od = OutdoorData::from_tables(&rows::<Leveldefs>(), &rows::<Lvlprest>(), &rows::<Lvlsub>());
    let subs = &od.subs;
    // Types 0..12 in file order, first rows as stated.
    assert!(subs.windows(2).all(|w| w[0].type_ <= w[1].type_));
    assert_eq!(subs.first().map(|r| r.type_), Some(0));
    assert_eq!(subs.last().map(|r| r.type_), Some(12));
    let first: Vec<u32> = (0..=12)
        .map(|t| {
            subs.iter()
                .position(|r| r.type_ == t)
                .expect("type present") as u32
        })
        .collect();
    let expected = [0, 1, 2, 3, 4, 6, 10, 16, 17, 21, 28, 31, 33];
    assert_eq!(first, expected);
    assert_eq!(fixed().lvlsub_types, expected);
    let of_type = |t: i32| subs.iter().filter(move |r| r.type_ == t);
    for (t, bord) in [(0, 1), (1, 0), (2, 1), (3, 2), (12, 2)] {
        assert!(
            of_type(t).all(|r| r.bord_type == bord),
            "BordType of type {t}"
        );
    }
    assert!(of_type(12).all(|r| r.grid_size == 2));
    // Blood Moor 6 / 0 / 4 / 5, Cold Plains 6 / 1 / 4 / 5.
    let sub = |id: u32| {
        let s = od.sub_defs(id);
        (s.sub_type, s.sub_theme, s.sub_waypoint, s.sub_shrine)
    };
    assert_eq!(sub(2), (6, 0, 4, 5));
    assert_eq!(sub(3), (6, 1, 4, 5));
    // Type 6 rows (10..16): Prob0 of the recorded sub-theme draws; the
    // scattered Puddles (row 12) and Swamp Small (row 14) passes.
    let prob0: Vec<i32> = of_type(6).map(|r| r.prob[0]).collect();
    assert_eq!(prob0, [30, 50, 90, 0, 50, 20]);
    assert_eq!((subs[12].max[0], subs[12].trials[0]), (8, 20));
    assert_eq!((subs[14].max[0], subs[14].trials[0]), (2, 5));
}

// ---- Act I placement on the live tables -----------------------------------------------

/// `levels.md` and `outdoor.md` Test vectors: the recorded Act I DRLG
/// (server copy, init seed 644409375) built by the real level types from
/// the live tables. The town is not generated (town id 0): step 8 comes
/// after the placement and needs the town DS1.
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn act1_placement_on_live_tables() {
    let data = Arc::new(drlg_data());
    let mut types = world_types(&data);
    let drlg = Drlg::create(0, INIT, 0, 0, false, &data, &mut types).expect("act 0 creates");
    assert!(types.errors.is_empty(), "{:?}", types.errors);
    // Seq 2418: dwStartSeed; seq 2439: the one real DRLG-seed step.
    assert_eq!(drlg.start_seed, START);
    assert_eq!(drlg.seed, Seed::new(1_406_222_081, 1_674_353_446));
    // The level list, head first.
    let ids: Vec<u32> = drlg
        .level_list()
        .iter()
        .map(|&l| drlg.level(l).id)
        .collect();
    assert_eq!(
        ids,
        [16, 15, 14, 13, 12, 11, 10, 9, 8, 5, 27, 6, 7, 26, 39, 17, 1, 2, 3, 4]
    );
    // Level seeds: `init_low(start + id)`, plus one preset-file roll on
    // 26, 27, 13, 14, 15, 16 (seq 2433–2453); level 1 (`Files` 0): none.
    let rolled = [13, 14, 15, 16, 26, 27];
    for &l in &drlg.level_list() {
        let id = drlg.level(l).id;
        let mut seed = Seed::init_low(START.wrapping_add(id));
        if rolled.contains(&id) {
            seed.step();
        }
        assert_eq!(drlg.level(l).seed, seed, "seed of level {id}");
    }
    // Seq 2437–2438: level 27's roll(3), lo' 2260552554 → 0.
    assert_eq!(Seed::init_low(START + 27).step(), 2_260_552_554);
    let presets = types.act_presets(0).expect("act 0 preset state");
    let dir = |id: u32| {
        let l = drlg.find_level(id).expect("level allocated");
        presets.info(l).expect("preset info").direction
    };
    for id in [13, 14, 15, 16, 26] {
        assert_eq!(dir(id), 0, "direction of level {id}");
    }
    // Seq 2439: the Outer Cloister direction 1 (odd lo'), over the roll.
    assert_eq!(dir(27), 1);
    // Derived rects (`outdoor.md` Test vectors, simulation of §2) and the
    // town direction 3.
    let rect = |id: u32| drlg.level(drlg.find_level(id).unwrap()).rect;
    assert_eq!(rect(4), TileRect::new(1000, 1000, 80, 80));
    assert_eq!(rect(3), TileRect::new(920, 984, 80, 80));
    assert_eq!(rect(2), TileRect::new(904, 1064, 56, 96));
    assert_eq!(rect(1), TileRect::new(960, 1112, 56, 40));
    assert_eq!(rect(17), TileRect::new(880, 968, 40, 48));
    assert_eq!((rect(39).x, rect(39).y), (5000, 1148));
    assert_eq!((rect(26).x, rect(26).y), (3000, 1000));
    assert_eq!((rect(7).x, rect(7).y), (3000, 1018));
    assert_eq!((rect(6).x, rect(6).y), (2920, 1002));
    assert_eq!((rect(5).x, rect(5).y), (2904, 1082));
    assert_eq!(dir(1), 3);
}

/// `preset.md` Test vectors (derived from `rng.md`): file choice at
/// allocation of levels 90, 124 and 94 with dwStartSeed 4014346869.
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn preset_file_choice_derived_vectors() {
    let data = Arc::new(drlg_data());
    let mut types = world_types(&data);
    let mut drlg = Drlg::create(0, INIT, 0, 0, false, &data, &mut types).expect("act 0 creates");
    let pd = preset_data();
    for (id, lo, dir, file) in [
        (90, 3_449_482_213, 1, "act3/jungle/dungrm2a.ds1"),
        (124, 4_227_474_959, 3, "/nihlw.ds1"),
        (94, 2_025_139_961, 2, "/temple2.ds1"),
    ] {
        assert_eq!(Seed::init_low(START + id).step(), lo, "lo' of level {id}");
        let l: LevelIdx = drlg
            .get_or_alloc_level(&data, &mut types, id)
            .unwrap_or_else(|e| panic!("level {id}: {e:?}"));
        let info = types.act_presets(0).unwrap().info(l).expect("preset info");
        assert_eq!(info.direction, dir, "direction of level {id}");
        let def = pd.def(pd.def_for_level(id).unwrap()).unwrap();
        let name = String::from_utf8_lossy(&def.file[dir as usize])
            .replace('\\', "/")
            .to_ascii_lowercase();
        assert!(name.ends_with(file), "level {id} file {name}");
    }
    assert!(types.errors.is_empty(), "{:?}", types.errors);
}

// ---- Act III and Act V (`outdoor-act3-act5.md`) --------------------------------------

/// `outdoor-act3-act5.md` Test vectors, "Real 1.14d values": the
/// leveldefs and lvlprest rows the jungle placer, the jungle stamping and
/// the Act V build read.
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn act3_act5_table_values() {
    let data = drlg_data();
    for id in 76..=78 {
        let d = &data.levels[id];
        assert_eq!(d.size, [(64, 192); 3], "size of level {id}");
        assert_eq!(d.offset, (-1, -1), "offset of level {id}");
    }
    for id in [111, 112] {
        assert_eq!(data.levels[id].size, [(-1, -1); 3], "size of level {id}");
    }
    assert_eq!(data.levels[110].size, [(240, 48); 3]);
    assert_eq!(data.levels[110].offset, (760, 1000));
    assert_eq!(data.levels[117].size, [(128, 80); 3]);
    let pd = preset_data();
    let def = |p: u32| pd.def(p).expect("lvlprest row");
    for p in [573, 574] {
        let d = def(p);
        assert_eq!((d.size_x, d.size_y, d.files), (64, 32, 0), "lvlprest {p}");
    }
    for p in (530..=572).chain(575..=604) {
        let d = def(p);
        let files = match p {
            541 => 5,
            530..=544 => 3,
            _ => 1,
        };
        assert_eq!(
            (d.size_x, d.size_y, d.files),
            (32, 32, files),
            "lvlprest {p}"
        );
    }
    assert_eq!(def(865).size_x, 16);
    assert_eq!((def(652).size_x, def(652).size_y), (48, 16));
    for p in 653..=658 {
        assert_eq!(def(p).files, 1, "lvlprest {p}");
    }
}

/// `outdoor-act3-act5.md` Test vectors (derived): Act III and Act V
/// placement from the recorded init seed on the live tables.
// Covers: specs/drlg/outdoor-act3-act5.md §2.8 r1, §2.8 r3; specs/drlg/outdoor.md §9.2
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn act3_act5_placement_on_live_tables() {
    let data = Arc::new(drlg_data());
    let mut types = world_types(&data);
    let drlg = Drlg::create(2, INIT, 0, 0, false, &data, &mut types).expect("act 2 creates");
    assert!(types.errors.is_empty(), "{:?}", types.errors);
    assert_eq!(drlg.seed, Seed::new(4_015_082_244, 577_631_236));
    let rect = |d: &Drlg, id: u32| d.level(d.find_level(id).unwrap()).rect;
    assert_eq!(rect(&drlg, 76), TileRect::new(1000, 808, 64, 192));
    assert_eq!(rect(&drlg, 77), TileRect::new(936, 744, 64, 192));
    assert_eq!(rect(&drlg, 78), TileRect::new(1000, 616, 64, 192));
    assert_eq!(rect(&drlg, 79), TileRect::new(992, 552, 80, 64));
    assert_eq!(rect(&drlg, 80), TileRect::new(992, 488, 80, 64));
    assert_eq!(rect(&drlg, 81), TileRect::new(992, 424, 80, 64));
    assert_eq!(rect(&drlg, 82), TileRect::new(1008, 408, 48, 16));
    assert_eq!(rect(&drlg, 83), TileRect::new(1000, 344, 64, 64));
    let od = types.act_outdoor(2).expect("act 2 outdoor state");
    let ids = |id: u32| {
        let info = od
            .level(drlg.find_level(id).unwrap())
            .expect("outdoor info");
        (
            info.jungle_ids.clone().expect("jungle ids"),
            info.jungle_clearings,
        )
    };
    assert_eq!(
        ids(76),
        (
            vec![541, 533, 543, 565, 570, 582, 571, 575, 570, 575, 537, 0],
            3
        )
    );
    assert_eq!(
        ids(77),
        (
            vec![554, 577, 541, 0, 539, 534, 0, 541, 576, 569, 576, 566],
            3
        )
    );
    assert_eq!(
        ids(78),
        (
            vec![0, 533, 535, 565, 570, 582, 539, 534, 558, 565, 541, 581],
            2
        )
    );

    let mut types = world_types(&data);
    let drlg = Drlg::create(4, INIT, 0, 0, false, &data, &mut types).expect("act 4 creates");
    assert!(types.errors.is_empty(), "{:?}", types.errors);
    assert_eq!(rect(&drlg, 110), TileRect::new(760, 1000, 240, 48));
    assert_eq!(rect(&drlg, 111), TileRect::new(600, 968, 160, 64));
    assert_eq!(rect(&drlg, 112), TileRect::new(440, 968, 160, 64));
    assert_eq!(rect(&drlg, 117), TileRect::new(2000, 1896, 160, 64));
}
