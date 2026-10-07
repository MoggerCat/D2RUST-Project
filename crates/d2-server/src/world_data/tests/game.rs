// Spec: specs/drlg/outdoor.md §2, §12; specs/drlg/maze.md; specs/drlg/preset.md §3–§6; specs/drlg/levels.md §3–§5
//! The Act I levels from the live 1.14d tables and files, through
//! `wiring::worldgen::levels::WorldTypes` (`#[ignore]`, `D2_GAME_DIR`):
//! the recorded placement vector (`outdoor.md` Test vectors), the Den of
//! Evil maze vector (`maze.md` Test vectors) and outdoor levels through
//! the dispatcher (HANDOFF §7 WG9).

use std::sync::{Arc, OnceLock};

use d2_formats::mpq::ArchiveSet;
use d2_sim::drlg::maze::Maze;
use d2_sim::drlg::{Drlg, DrlgRoomId, LevelIdx, RoomKind, Services, TileRect};
use d2_sim::game::Game;
use d2_sim::rng::Seed;
use d2_sim::wiring::worldgen::levels::{SharedTypes, WorldTypes};

use super::super::tables::LevelTables;
use super::super::{archive, WorldFiles};

/// The DRLG init seed of the recorded Act I creation; `dwStartSeed`
/// 4014346869 (`outdoor.md` Test vectors).
const INIT: u32 = 644_409_375;
const START: u32 = 4_014_346_869;
/// Rogue Encampment: the server's town level (`levels.md` §3 step 8).
const TOWN: u32 = 1;

fn live() -> &'static (LevelTables, WorldFiles) {
    static L: OnceLock<(LevelTables, WorldFiles)> = OnceLock::new();
    L.get_or_init(|| {
        let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
        let set = ArchiveSet::open_dir(dir).expect("archives open");
        archive::load(&set).expect("level tables and DRLG files load")
    })
}

/// Act 0 created through the dispatcher on the live data.
struct Act {
    drlg: Drlg,
    types: SharedTypes,
    data: Arc<d2_sim::drlg::DrlgData>,
    game: Game,
}

impl Act {
    fn create() -> Self {
        let (t, f) = live();
        let data = Arc::new(t.drlg.clone());
        let types = SharedTypes::new(WorldTypes::new(
            data.clone(),
            Maze::new(t.maze.clone()),
            t.preset.clone(),
            t.outdoor.clone(),
            Box::new(f.ds1.clone()),
            Box::new(f.subs.clone()),
        ));
        let mut handle = types.clone();
        let drlg = Drlg::create(0, INIT, 0, TOWN, false, &data, &mut handle).expect("act 0");
        let mut game = Game::new();
        game.lists.ensure_act(0).unwrap();
        let act = Self {
            drlg,
            types,
            data,
            game,
        };
        act.assert_clean();
        act
    }

    fn generate(&mut self, id: u32) -> (LevelIdx, Vec<DrlgRoomId>) {
        let mut handle = self.types.clone();
        let l = self
            .drlg
            .get_or_alloc_level(&self.data, &mut handle, id)
            .expect("allocate");
        self.drlg
            .generate_level(&self.data, &mut handle, l)
            .expect("generate");
        self.assert_clean();
        (l, self.drlg.level_rooms(l))
    }

    fn stream(&mut self, room: DrlgRoomId) {
        let mut handle = self.types.clone();
        let mut svc = Services {
            data: &self.data,
            tiles: &live().1.dt1,
            types: &mut handle,
            rooms: &mut self.game.lists,
        };
        self.drlg.stream_room(&mut svc, room).expect("stream");
        self.assert_clean();
    }

    fn assert_clean(&self) {
        let errors: Vec<String> = self
            .types
            .borrow()
            .errors
            .iter()
            .map(|e| format!("{e:?}"))
            .collect();
        assert_eq!(errors, Vec::<String>::new());
    }

    fn rect(&self, id: u32) -> TileRect {
        let l = self.drlg.find_level(id).expect("allocated");
        self.drlg.level(l).rect
    }
}

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn act1_placement_matches_the_recorded_vector() {
    let act = Act::create();
    let d = &act.drlg;
    assert_eq!(d.start_seed, START);
    assert_eq!(d.seed, Seed::new(1_406_222_081, 1_674_353_446));
    let mut order: Vec<u32> = d.level_list().into_iter().map(|l| d.level(l).id).collect();
    order.reverse();
    // `levels.md` Test vectors (seq 2425–2452): the placer rows, then the
    // `outdoor.md` §2.7 neighbour-entry walk over 1..17 allocates 8..16.
    assert_eq!(
        order,
        [4, 3, 2, 1, 17, 39, 26, 7, 6, 27, 5, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    );
    // The final rects the spec derives from the rules (`outdoor.md` Test
    // vectors, "derived, not yet recorded").
    assert_eq!(act.rect(4), TileRect::new(1000, 1000, 80, 80));
    assert_eq!(act.rect(3), TileRect::new(920, 984, 80, 80));
    assert_eq!(act.rect(2), TileRect::new(904, 1064, 56, 96));
    assert_eq!(act.rect(1), TileRect::new(960, 1112, 56, 40));
    assert_eq!(act.rect(17), TileRect::new(880, 968, 40, 48));
    for (id, x, y) in [
        (39, 5000, 1148),
        (26, 3000, 1000),
        (7, 3000, 1018),
        (6, 2920, 1002),
        (5, 2904, 1082),
    ] {
        let r = act.rect(id);
        assert_eq!((r.x, r.y), (x, y), "level {id}");
    }
}

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn den_of_evil_matches_the_maze_vector() {
    let mut act = Act::create();
    let (l, rooms) = act.generate(8);
    let rect = act.drlg.level(l).rect;
    let types = act.types.borrow();
    let p = types.act_presets(0).expect("act 0 presets");
    let maps: Vec<(u32, i32, i32)> = p
        .level_maps(l)
        .iter()
        .map(|&m| {
            let m = p.map(m).unwrap();
            (m.def, m.rect.x - rect.x, m.rect.y - rect.y)
        })
        .collect();
    // Head first: F (57, Cave SW), P (86, Cave Prev N), D (96, Cave Den
    // Of Evil E); after normalization D at the level origin, F at
    // (+24, 0), P at (+24, +24).
    assert_eq!(maps, [(57, 24, 0), (86, 24, 24), (96, 0, 0)]);
    assert!(!rooms.is_empty());
    for &r in &rooms {
        assert_eq!(act.drlg.room(r).kind, RoomKind::Preset);
    }
}

/// HANDOFF §7 WG9: an outdoor level generated through the dispatcher,
/// with the live lvlsub rows and substitution DS1s.
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn outdoor_levels_generate_through_the_dispatcher() {
    let mut act = Act::create();
    // Room allocations per level build (`outdoor.md` Test vectors):
    // Blood Moor 48 preset cells + 33 outdoor rooms, Cold Plains 61 + 37.
    let (l2, blood) = act.generate(2);
    assert_eq!(act.drlg.level(l2).rect, TileRect::new(904, 1064, 56, 96));
    assert_eq!(blood.len(), 81);
    let (l3, cold) = act.generate(3);
    let outdoor = |rooms: &[DrlgRoomId]| {
        rooms
            .iter()
            .filter(|&&r| act.drlg.room(r).kind != RoomKind::Preset)
            .count()
    };
    // Diagnostic for the local run (1.14d recorded 61 preset + 37
    // outdoor = 98, `outdoor.md` Test vectors, per-site draw counts):
    // which side the live build is short on.
    eprintln!(
        "Cold Plains rooms: {} total, {} preset, {} outdoor (recorded 98 = 61 + 37)",
        cold.len(),
        cold.len() - outdoor(&cold),
        outdoor(&cold)
    );
    // The grid and the border substitutions against the spec's derived
    // build (`outdoor.md` Test vectors, "Cold Plains grid"): every
    // difference is printed before the asserts, the first one names the
    // step that differs.
    let (grid_diffs, hit_diffs) = cold_plains_diffs(&act, l3);
    eprintln!("Cold Plains grid differences: {grid_diffs:#?}");
    eprintln!("Cold Plains substitution differences: {hit_diffs:#?}");
    assert_eq!(grid_diffs, Vec::<String>::new());
    assert_eq!(hit_diffs, Vec::<String>::new());
    assert_eq!(cold.len(), 98);
    assert_eq!((outdoor(&blood), outdoor(&cold)), (33, 37));
    // Streaming a room loads its DT1 library from the live files
    // (no recorded expectation: the call must not fail).
    act.stream(blood[0]);
}

/// `outdoor.md` Test vectors, "Cold Plains grid": grid 0 per cell after
/// §7, rows y = 0..9; `o` outdoor room, `A` one of 48 / 29 / 30, `-`
/// blank (0x100, no room).
const COLD_PLAINS: [&str; 10] = [
    " 9  6  6  6  6  6  6  6  6 10",
    " 5  o  A  o  o  o  o  A 44  7",
    " 5  o  o  o 15  4  4 12  o  7",
    " 5  o  o  o 14  6 10  5  o  7",
    " 5  o 51  o  o  o  7  5  o  7",
    " 5  o  o  o  A  o 14 13  o  7",
    " 8  4 12  o  o  o  o  o  o  7",
    " 9  6 13  o 15 12  o  o  o  7",
    " 5  o  o  o  7  5  o 15  4 11",
    " 8  4  4  4 11  8  4 11  -  -",
];

/// The border substitution replacements of the recorded Cold Plains
/// build (`outdoor.md` Test vectors): (lvlsub type, group, snapped cell,
/// variant, level seed lo' after the variant roll). Type 1: seq 8447
/// `0x0066F8DB` lo' 1833932632 mod 10 = 2; type 2: seq 8644 `0x0066F905`
/// lo' 3559729267 & 1 = 1; type 3: seq 10204 and 10499 (N 1). Seq s is
/// level-seed draw s − 6897 from {4014346872, 666} (both given lo'
/// values sit at draws 1550 and 1747), so the type-3 lo' are draws 3307
/// and 3602 of that seed.
const COLD_PLAINS_HITS: [(i32, usize, i32, i32, i32, u32); 4] = [
    (1, 0, 3, 1, 2, 1_833_932_632),
    (2, 1, 6, 6, 1, 3_559_729_267),
    (3, 8, 3, 6, 0, 1_651_351_014),
    (3, 11, 0, 5, 0, 2_564_466_130),
];

/// Per-cell (grid 0, grid 2) and per-substitution differences between
/// the live Cold Plains build and the spec's; prints the grid.
fn cold_plains_diffs(act: &Act, l: LevelIdx) -> (Vec<String>, Vec<String>) {
    let types = act.types.borrow();
    let info = types
        .act_outdoor(0)
        .and_then(|o| o.level(l))
        .expect("Cold Plains outdoor info");
    let mut grid = Vec::new();
    for (y, row) in COLD_PLAINS.iter().enumerate() {
        let mut line = String::new();
        for (x, want) in row.split_whitespace().enumerate() {
            let (x, y) = (x as i32, y as i32);
            let (g0, g2) = (info.grids[0].get(x, y), info.grids[2].get(x, y));
            line.push_str(&format!(" {g0:3}/{g2:05x}"));
            let blank = g2 & 0x100 != 0;
            let preset = g2 & 0x200 != 0;
            let ok = match want {
                "o" => g0 == 0 && !blank && !preset,
                "-" => g0 == 0 && blank && !preset,
                "A" => matches!(g0, 48 | 29 | 30) && preset,
                n => g0 == n.parse::<u32>().unwrap() && preset,
            };
            if !ok {
                grid.push(format!(
                    "({x}, {y}): want {want}, grid 0 {g0}, grid 2 {g2:#x}"
                ));
            }
        }
        eprintln!("{line}");
    }
    let got: Vec<_> = info
        .sub_hits
        .iter()
        .map(|h| (h.t, h.group, h.x, h.y, h.variant, h.lo))
        .collect();
    eprintln!(
        "Cold Plains substitutions (type, row, group, x, y, v, lo'): {:?}",
        info.sub_hits
    );
    let hits = if got == COLD_PLAINS_HITS {
        Vec::new()
    } else {
        vec![format!("got {got:?}, want {COLD_PLAINS_HITS:?}")]
    };
    (grid, hits)
}
