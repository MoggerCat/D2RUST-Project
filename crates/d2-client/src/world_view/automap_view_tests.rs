// Spec: specs/ui/automap.md (§8 r5, §10, §11)
//! The automap draw sink with synthetic fixtures: a revealed cell, the
//! cel file in a memory source, the player marker.

use super::*;
use crate::assets::path::MemorySource;
use crate::bridge::world::{ActLoad, ClientUnit, UnitKey, PLAYER};
use crate::rules::camera::ClientPos;
use crate::rules::draw_order::{
    Dt1Facts, Fade, LevelFacts, NearRooms, Room, TileRecord, TileRect, REC_DRAWN,
};
use crate::ui::automap::options::MemoryStore;
use crate::ui::automap::picker::{CelPicker, PickRecord};
use crate::ui::automap::session::AutomapSource;
use crate::ui::automap::{Automap, AutomapLevels, UnitCels, SEED};
use d2_formats::palette::{Palette, Rgb};

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
        player_subtile: (0, 0),
        edge: None,
        level: LevelFacts {
            id: 2,
            ..LevelFacts::default()
        },
    }
}

/// A DC6 of one direction, `frames` frames of `w × h` literal pixels,
/// offsets 0, bottom-up (`formats/dc6.md`).
fn dc6(frames: u32, w: u32, h: u32) -> Vec<u8> {
    let mut rows = Vec::new();
    for _ in 0..h {
        rows.push(w as u8);
        rows.extend((0..w).map(|i| 1 + i as u8));
        rows.push(0x80);
    }
    let mut d = Vec::new();
    for v in [6i32, 1, 0] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[0xEE; 4]);
    d.extend_from_slice(&1u32.to_le_bytes());
    d.extend_from_slice(&frames.to_le_bytes());
    let mut at = d.len() + 4 * frames as usize;
    let mut body = Vec::new();
    for _ in 0..frames {
        d.extend_from_slice(&(at as u32).to_le_bytes());
        for v in [0u32, w, h, 0, 0, 0, 0, rows.len() as u32] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        body.extend_from_slice(&rows);
        body.extend_from_slice(&[0xEE; 3]);
        at += 32 + rows.len() + 3;
    }
    d.extend(body);
    d
}

fn session() -> AutomapSession {
    AutomapSession::new(Automap::new(&MemoryStore::default(), SEED), source(), None)
}

fn assets() -> ViewAssets {
    let mut colors = [Rgb::default(); 256];
    for (i, c) in colors.iter_mut().enumerate() {
        *c = Rgb {
            r: i as u8,
            g: i as u8,
            b: i as u8,
        };
    }
    ViewAssets::new(Palette { colors })
}

fn view() -> AutomapView {
    let mut src = MemorySource::default();
    for p in cel_paths(false, true).into_iter().flatten() {
        src.insert(&p, dc6(64, 8, 4));
    }
    AutomapView::new(Arc::new(src), true)
}

// Covers: specs/ui/automap.md §10 r2, §11 r5
#[test]
fn an_open_automap_draws_its_cells_and_the_player_marker() {
    let w = world((200, 200));
    let mut s = session();
    s.frame(&w, Some(&mut near())).unwrap();
    let mut v = view();
    let mut a = assets();
    let mode = OpenMode::NONE;
    // Closed: nothing.
    let mut frame = WorldFrame::default();
    assert!(v
        .add_to_frame(&mut s, &w, mode, &mut a, &mut frame)
        .is_empty());
    assert!(frame.items.is_empty());
    // Open (Tab): the cell's cel and the marker's lines.
    let f = FrameFacts {
        width: 800,
        height: 600,
        open_mode: 0,
        mini_down: false,
        unit_origin: ClientPos::default(),
    };
    s.toggle(&f);
    let log = v.add_to_frame(&mut s, &w, mode, &mut a, &mut frame);
    assert!(log.is_empty(), "{log:?}");
    let cels = v
        .last
        .iter()
        .filter(|d| matches!(d, AutomapDraw::Cel { .. }))
        .count();
    let lines = v
        .last
        .iter()
        .filter(|d| matches!(d, AutomapDraw::Line { .. }))
        .count();
    assert!(cels >= 1, "{:?}", v.last);
    assert!(lines >= 1, "{:?}", v.last);
    assert!(frame.items.len() > cels);
    assert!(frame
        .items
        .iter()
        .all(|i| i.key.pass() == pass::UNIDENTIFIED_9));
    // Closed again: nothing.
    s.toggle(&f);
    let mut frame = WorldFrame::default();
    v.add_to_frame(&mut s, &w, mode, &mut a, &mut frame);
    assert!(frame.items.is_empty());
}

// Covers: specs/ui/automap.md §8 r5
#[test]
fn a_missing_cel_file_is_logged_once_and_not_drawn() {
    let w = world((200, 200));
    let mut s = session();
    s.frame(&w, Some(&mut near())).unwrap();
    s.toggle(&FrameFacts {
        width: 800,
        height: 600,
        open_mode: 0,
        mini_down: false,
        unit_origin: ClientPos::default(),
    });
    let mut v = AutomapView::new(Arc::new(MemorySource::default()), true);
    let mut a = assets();
    let mut frame = WorldFrame::default();
    let log = v.add_to_frame(&mut s, &w, OpenMode::NONE, &mut a, &mut frame);
    assert!(log.iter().any(|l| l.contains("in no archive")), "{log:?}");
    let log = v.add_to_frame(&mut s, &w, OpenMode::NONE, &mut a, &mut frame);
    assert!(log.is_empty());
}

#[test]
fn bresenham_includes_both_ends() {
    assert_eq!(line((0, 0), (3, 1)), vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
    assert_eq!(line((2, 2), (2, 2)), vec![(2, 2)]);
}
