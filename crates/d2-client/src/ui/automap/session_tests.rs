//! Tests of the automap session (`ui/automap.md` §5 r1, §7, §8 r2, §14).

use std::path::PathBuf;

use super::options::MemoryStore;
use super::picker::{CelPicker, PickRecord};
use super::session::{open_files_in, AutomapSession, AutomapSource, SaveFiles};
use super::{Automap, AutomapLevels, FrameFacts, UnitCels, SEED};
use crate::bridge::world::{ActLoad, ClientUnit, ClientWorld, UnitKey, PLAYER};
use crate::rules::camera::ClientPos;
use crate::rules::draw_order::{
    Dt1Facts, Fade, LevelFacts, NearRooms, Room, TileRecord, TileRect, REC_DRAWN,
};

struct Levels;

impl AutomapLevels for Levels {
    fn layer(&self, level: u32) -> Option<u32> {
        Some(level)
    }
    fn level_type(&self, _: u32) -> Option<u32> {
        Some(1)
    }
    fn act(&self, _: u32) -> Option<u8> {
        Some(0)
    }
}

struct Cels;

impl UnitCels for Cels {
    fn monster_cel(&self, _: u32) -> Option<u32> {
        None
    }
    fn object_cel(&self, _: u32) -> Option<u32> {
        None
    }
}

/// LevelType 1, orientation 1: one record that always gives cel 40.
fn source() -> AutomapSource {
    let mut ranges = vec![(-1, -1); 36];
    ranges[1] = (0, 1);
    AutomapSource {
        picker: CelPicker {
            records: vec![PickRecord {
                level: 1,
                tile: 1,
                style: 0xFF,
                start: 0xFF,
                end: 0,
                cels: [40, -1, -1, -1],
                count: 1,
            }],
            ranges,
        },
        levels: Box::new(Levels),
        cels: Box::new(Cels),
    }
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("d2rs-automap-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn files(dir: &std::path::Path) -> SaveFiles {
    SaveFiles {
        dir: dir.to_path_buf(),
        sub: Some("sub".into()),
        name: "Hero".into(),
    }
}

// Covers: specs/ui/automap.md §7 text
#[test]
fn map_files_use_the_sub_dir_when_it_opens_else_the_save_dir() {
    let d = temp("sub");
    let f = files(&d);
    let (_, path, slot) = open_files_in(&f, 7).unwrap();
    assert_eq!((path, slot), (d.join("Hero.ma0"), 0));
    assert!(d.join("Hero.map").is_file());
    std::fs::create_dir_all(d.join("sub")).unwrap();
    let (_, path, _) = open_files_in(&f, 7).unwrap();
    assert_eq!(path, d.join("sub").join("Hero.ma0"));
    assert!(d.join("sub").join("Hero.map").is_file());
    // No sub dir named: the save dir.
    let f = SaveFiles { sub: None, ..f };
    let (_, path, _) = open_files_in(&f, 7).unwrap();
    assert_eq!(path, d.join("Hero.ma0"));
    let _ = std::fs::remove_dir_all(&d);
}

fn world(cell: (u16, u16)) -> ClientWorld {
    let mut w = ClientWorld::default();
    let p = UnitKey::new(PLAYER, 1);
    let mut u = ClientUnit::new(p);
    u.position = Some(cell);
    w.units.insert(p, u);
    w.local_player = Some(p);
    w.act = Some(ActLoad {
        act: 0,
        init_seed: 0x1234,
        town_level: 1,
        f8: 9,
    });
    w
}

fn near() -> NearRooms {
    let rec = TileRecord {
        tile: (0, 0),
        flags: REC_DRAWN,
        ty: 0,
        dt1: Dt1Facts {
            orientation: 1,
            sub: 5,
            ..Dt1Facts::default()
        },
        fade: Fade::OPAQUE,
        logical: None,
    };
    NearRooms {
        rooms: vec![Room {
            level: 2,
            tiles: TileRect {
                x: 10,
                y: 0,
                w: 8,
                h: 8,
            },
            floors: vec![rec],
            ..Room::default()
        }],
        player_tile: (0, 0),
        player_logical: 0,
        level: LevelFacts {
            id: 2,
            ..LevelFacts::default()
        },
    }
}

// Covers: specs/ui/automap.md §5 r1, §14 r3
#[test]
fn a_frame_opens_the_act_files_reveals_and_the_teardown_writes() {
    let d = temp("frame");
    let map = Automap::new(&MemoryStore::default(), SEED);
    let mut s = AutomapSession::new(map, source(), Some(files(&d)));
    // No act: nothing.
    let mut w = world((10, 10));
    let act = w.act.take();
    s.frame(&w, None).unwrap();
    assert!(s.data_path().is_none());
    w.act = act;
    let mut n = near();
    s.frame(&w, Some(&mut n)).unwrap();
    assert_eq!(s.data_path(), Some(d.join("Hero.ma0").as_path()));
    assert_eq!(s.map.current(), Some(2));
    assert_eq!(s.map.cells.tree(super::TreeKind::Floor).in_order().len(), 1);
    // The same act record does not reopen; the player did not move: no
    // second reveal.
    s.frame(&w, Some(&mut near())).unwrap();
    assert_eq!(s.map.current(), Some(2));
    s.teardown().unwrap();
    assert_eq!(s.map.current(), None);
    let bytes = std::fs::read(d.join("Hero.ma0")).unwrap();
    assert!(bytes.len() > 400, "table plus one record");
    let _ = std::fs::remove_dir_all(&d);
}

// Covers: specs/ui/automap.md §8 r2
#[test]
fn toggle_flips_ui_state_0x0a() {
    let map = Automap::new(&MemoryStore::default(), SEED);
    let mut s = AutomapSession::new(map, source(), None);
    let f = FrameFacts {
        width: 800,
        height: 600,
        open_mode: 0,
        mini_down: false,
        unit_origin: ClientPos::default(),
    };
    s.toggle(&f);
    assert!(s.open);
    s.toggle(&f);
    assert!(!s.open);
}
