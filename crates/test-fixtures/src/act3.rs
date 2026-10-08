// Spec: specs/drlg/maze.md §2–§6, §9; specs/drlg/levels.md §3–§6; specs/drlg/preset.md §2–§3; specs/data/fixups.md §12
//! An Act III-shaped variant of the synthetic set (task `q-a3-dungeons`):
//! Kurast Docks as the preset town (level 75, `TownOnly` creation: the
//! jungle placer needs block data this set does not have) and the Act III
//! dungeons by their 1.14d ids and level types, so the real maze
//! generator (`maze.md`) builds Spider Cave / Cavern, Swampy Pit,
//! Flayer Dungeon, the sewers and the Durance of Hate levels on made-up
//! data. Same method as [`crate::act2`].
//!
//! Fixed by the spec: the level ids (`maze.md` §3.3 / §6: 84, 85 type 23
//! Spider; 86..=91 type 24 Dungeon; 92, 93 type 25 Act 3 Sewer; 100, 101
//! type 22 Kurast), the lvlprest `Def` = row convention up to 1100 (the
//! highest def the Act III builders pick or stamp), the 24-tile cell.
//! Made up: names, every preset's content (one 24 × 24 floor cell),
//! `Rooms` / `Merge` of the lvlmaze rows (`Merge` 500 so the Spider
//! Cavern vector's draw 367 links), the vis chains, the town, and the
//! temples (94..=99) and Durance of Hate 3 (102) as one-cell preset
//! levels (their layouts are fixed DS1s in the original).

use d2_formats::ds1::Ds1;

use crate::act1::{clear, floor_preset, n};
use crate::content::{FLOOR_DT1, SUB_DS1, WALL_DT1};
use crate::drlg::archive_name;
use crate::synth::{synthetic, Synthetic, TableSet};

/// Kurast Docks, the Act III town.
pub const TOWN: u32 = 75;
/// Levels of ids 0 ..= 102 (Act III is 75..=102, `levels.md` §6.3).
pub const LEVEL_COUNT: u32 = 103;
pub const SPIDER_CAVE: u32 = 84;
pub const SPIDER_CAVERN: u32 = 85;
pub const SEWERS_1: u32 = 92;
pub const SEWERS_2: u32 = 93;
pub const DURANCE_1: u32 = 100;
pub const DURANCE_2: u32 = 101;
pub const DURANCE_3: u32 = 102;
/// The maze levels with their lvltypes `Id` (`maze.md` §3.3): Spider 23,
/// Dungeon 24 (Swampy Pit 86 / 87 / 89, Flayer Dungeon 88 / 90 / 91),
/// Act 3 Sewer 25, Kurast 22.
pub const MAZE_LEVELS: [(u32, u32); 12] = [
    (84, 23),
    (85, 23),
    (86, 24),
    (87, 24),
    (88, 24),
    (89, 24),
    (90, 24),
    (91, 24),
    (92, 25),
    (93, 25),
    (100, 22),
    (101, 22),
];
/// The temples and Fanes (94..=99) and Durance of Hate 3, preset levels.
pub const PRESET_LEVELS: [u32; 7] = [94, 95, 96, 97, 98, 99, 102];
/// The vis chains of the dungeons (made up: the original's entrances are
/// DS1 warp units of the outdoor levels).
pub const CHAINS: [&[u32]; 5] = [
    &[86, 87, 89],
    &[88, 90, 91],
    &[92, 93],
    &[100, 101, 102],
    &[],
];

/// The levels-row `Waypoint` index of the town and of Durance of Hate 2.
pub const WAYPOINTS: [(u32, u32); 2] = [(TOWN, 18), (DURANCE_2, 26)];

pub const TOWN_DEF: u32 = 1;
pub const LAST_DEF: u32 = 1100;
pub const TOWN_DS1: &str = "Synth\\Act3\\Town.ds1";
/// The 24 × 24 cell every maze and preset level uses.
pub const MAZE_DS1: &str = "Synth\\Act3\\Maze.ds1";
pub const CELL: u32 = 24;
const TOWN_SIZE: u32 = 56;
/// The maze levels' rect side (made up; the shipped rows are about 200).
const MAZE_SIZE: u32 = 200;

/// The Act III-shaped synthetic set.
pub fn act3() -> Synthetic {
    let mut s = synthetic();
    for t in ["levels", "lvlprest", "lvltypes", "lvlsub", "lvlmaze"] {
        clear(&mut s.tables, t);
    }
    levels(&mut s);
    tables(&mut s.tables);
    s.files = files();
    s
}

/// The level name of id `id`.
pub fn level_name(id: u32) -> String {
    format!("Synth A3 L{id}")
}

/// The vis list of `id`: the next level of its chain.
fn vis(id: u32) -> Vec<u32> {
    for c in CHAINS {
        if let Some(i) = c.iter().position(|&l| l == id) {
            return c.get(i + 1).copied().into_iter().collect();
        }
    }
    Vec::new()
}

fn levels(s: &mut Synthetic) {
    for id in 0..LEVEL_COUNT {
        let name = level_name(id);
        s.strings.base.push((name.clone(), name.clone()));
        let maze = MAZE_LEVELS.iter().find(|m| m.0 == id);
        let (drlg, ltype, size) = if id == TOWN {
            (2, 1, TOWN_SIZE)
        } else if let Some(&(_, t)) = maze {
            (1, t, MAZE_SIZE)
        } else if PRESET_LEVELS.contains(&id) {
            (2, 1, CELL)
        } else {
            (0, 0, 0)
        };
        let wp = WAYPOINTS.iter().find(|w| w.0 == id).map_or(255, |w| w.1);
        let act = if id >= TOWN { "2" } else { "0" };
        let offset = if id == TOWN { 1000 } else { 0 };
        let mut cells: Vec<(String, String)> = vec![
            ("Id".into(), n(id)),
            ("Act".into(), act.into()),
            ("LevelName".into(), name.clone()),
            ("LevelWarp".into(), name.clone()),
            ("EntryFile".into(), name),
            ("Waypoint".into(), n(wp)),
            ("IsInside".into(), n(u32::from(maze.is_some()))),
            ("DrlgType".into(), n(drlg)),
            ("LevelType".into(), n(ltype)),
            ("OffsetX".into(), n(offset)),
            ("OffsetY".into(), n(offset)),
            ("Layer".into(), n(id)),
        ];
        for sfx in ["", "(N)", "(H)"] {
            cells.push((format!("SizeX{sfx}"), n(size)));
            cells.push((format!("SizeY{sfx}"), n(size)));
        }
        for c in ["SubType", "SubTheme", "SubWaypoint", "SubShrine"] {
            cells.push((c.into(), "-1".into()));
        }
        let vis = vis(id);
        for k in 0..8 {
            cells.push((format!("Vis{k}"), n(vis.get(k).copied().unwrap_or(0))));
            cells.push((format!("Warp{k}"), "-1".into()));
        }
        if maze.is_some() {
            for (c, v) in [
                ("MonLvl1", "2"),
                ("MonDen", "500"),
                ("NumMon", "2"),
                ("mon1", "beast1"),
                ("mon2", "ghoul1"),
                ("nmon1", "beast1"),
                ("umon1", "ghoul1"),
            ] {
                cells.push((c.into(), v.into()));
            }
        }
        let refs: Vec<(&str, &str)> = cells
            .iter()
            .map(|(c, v)| (c.as_str(), v.as_str()))
            .collect();
        s.tables.row("levels", &refs);
    }
}

fn tables(t: &mut TableSet) {
    // lvltypes: 0 none, 1 town and preset levels, 2..=25 the rest.
    t.row("lvltypes", &[("Act", "0")]);
    for _ in 1..=25 {
        t.row(
            "lvltypes",
            &[("File 1", FLOOR_DT1[0]), ("File 2", WALL_DT1), ("Act", "2")],
        );
    }
    // lvlmaze: one row per maze level, 24-tile cells.
    for &(id, _) in &MAZE_LEVELS {
        t.row(
            "lvlmaze",
            &[
                ("Level", &n(id)),
                ("Rooms", "6"),
                ("Rooms(N)", "8"),
                ("Rooms(H)", "10"),
                ("SizeX", &n(CELL)),
                ("SizeY", &n(CELL)),
                ("Merge", "500"),
            ],
        );
    }
    // lvlprest, Def = row.
    t.row("lvlprest", &[("Def", "0")]);
    let preset = |t: &mut TableSet, def: u32, level: u32, size: u32, files: u32, f: &str| {
        let (def, level, size, files) = (n(def), n(level), n(size), n(files));
        let mut cells = vec![
            ("Def", def.as_str()),
            ("LevelId", level.as_str()),
            ("Populate", "1"),
            ("SizeX", size.as_str()),
            ("SizeY", size.as_str()),
            ("Files", files.as_str()),
            ("Dt1Mask", "1"),
        ];
        let names = ["File1", "File2", "File3", "File4", "File5", "File6"];
        cells.extend(names.iter().map(|&c| (c, f)));
        t.row("lvlprest", &cells);
    };
    preset(t, TOWN_DEF, TOWN, TOWN_SIZE, 0, TOWN_DS1);
    // Defs 2.. are maze cells; the one-cell preset levels take the rows
    // at their own ids (`LevelId` set, found by `preset.md` §2.1).
    for def in TOWN_DEF + 1..=LAST_DEF {
        let level = if PRESET_LEVELS.contains(&def) { def } else { 0 };
        preset(t, def, level, CELL, 6, MAZE_DS1);
    }
    for ty in ["0", "1", "2", "3"] {
        t.row(
            "lvlsub",
            &[
                ("Type", ty),
                ("File", SUB_DS1),
                ("BordType", "1"),
                ("GridSize", "2"),
                ("Dt1Mask", "1"),
                ("Prob0", "50"),
                ("Trials0", "1"),
                ("Max0", "2"),
            ],
        );
    }
}

/// Kurast Docks: 56 × 56 floor tiles and the town waypoint.
pub fn town() -> Ds1 {
    let mut d = floor_preset(TOWN_SIZE, TOWN_SIZE);
    let (kind, id, x, y) = crate::act1::TOWN_WAYPOINT;
    d.objects.push(d2_formats::ds1::Ds1Object {
        kind,
        id,
        x,
        y,
        flags: 0,
    });
    d
}

/// Every DRLG file of the Act III set.
pub fn files() -> Vec<(String, Vec<u8>)> {
    let presets = [(TOWN_DS1, town()), (MAZE_DS1, floor_preset(CELL, CELL))];
    let mut out: Vec<(String, Vec<u8>)> = presets
        .iter()
        .map(|(p, d)| (archive_name(p), crate::ds1::write(d)))
        .collect();
    out.extend(
        crate::drlg::files()
            .into_iter()
            .filter(|(name, _)| !name.ends_with(".ds1") || *name == archive_name(SUB_DS1)),
    );
    out
}
