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

/// The local player's client position (the frame's one position).
fn at(w: &ClientWorld) -> ClientPos {
    let (x, y) = w.local().unwrap().cell();
    crate::rules::camera::moving_to_client(
        (u32::from(x) << 16) | 0x8000,
        (u32::from(y) << 16) | 0x8000,
    )
}

/// A built frame whose camera stands on the local player.
fn framed(w: &ClientWorld) -> WorldFrame {
    WorldFrame {
        camera: Some(crate::rules::camera::Camera::new(
            FrameSize::D2RS,
            OpenMode::NONE,
            at(w),
            (0, 0),
        )),
        ..WorldFrame::default()
    }
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
    let mut frame = framed(&w);
    assert!(v
        .add_to_frame(&mut s, &w, mode, at(&w), &mut a, &mut frame)
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
    let log = v.add_to_frame(&mut s, &w, mode, at(&w), &mut a, &mut frame);
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
        .all(|i| i.key.pass() == pass::UI && i.key.major() == pass::UI_AUTOMAP_MAJOR));
    // Closed again: nothing.
    s.toggle(&f);
    let mut frame = framed(&w);
    v.add_to_frame(&mut s, &w, mode, at(&w), &mut a, &mut frame);
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
    let mut frame = framed(&w);
    let log = v.add_to_frame(&mut s, &w, OpenMode::NONE, at(&w), &mut a, &mut frame);
    assert!(log.iter().any(|l| l.contains("in no archive")), "{log:?}");
    let log = v.add_to_frame(&mut s, &w, OpenMode::NONE, at(&w), &mut a, &mut frame);
    assert!(log.is_empty());
}

#[test]
fn bresenham_includes_both_ends() {
    assert_eq!(line((0, 0), (3, 1)), vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
    assert_eq!(line((2, 2), (2, 2)), vec![(2, 2)]);
}

// Covers: specs/render/draw-order.md §1
// Covers: specs/ui/panels.md §5 r3
#[test]
fn the_automap_draws_after_every_world_pass_and_before_the_panels() {
    // The rain / snow / flash items of pass 9 and the screen fade (10)
    // are world passes; the panel draws are the UI root's list.
    let automap = DrawKey::new(pass::UI, pass::UI_AUTOMAP_MAJOR, 0, 0).unwrap();
    let last_automap =
        DrawKey::new(pass::UI, pass::UI_AUTOMAP_MAJOR, DrawKey::MINOR_MAX, 0xFF).unwrap();
    let panels = DrawKey::new(pass::UI, pass::UI_PANELS_MAJOR, 0, 0).unwrap();
    for world in [
        DrawKey::new(pass::UNIDENTIFIED_9, 0, DrawKey::MINOR_MAX, 0xFF).unwrap(),
        DrawKey::new(
            pass::UNIDENTIFIED_9,
            DrawKey::MAJOR_MAX,
            DrawKey::MINOR_MAX,
            0xFF,
        )
        .unwrap(),
        DrawKey::new(
            pass::SCREEN_FADE,
            DrawKey::MAJOR_MAX,
            DrawKey::MINOR_MAX,
            0xFF,
        )
        .unwrap(),
    ] {
        assert!(world < automap, "{world:?}");
    }
    assert!(last_automap < panels);
}

// Covers: specs/ui/automap.md §10 r4
// Covers: specs/render/blend-modes.md §1
#[test]
fn automap_cel_modes_map_to_the_alpha_tables() {
    use crate::scene::MapId;
    let mut a = assets();
    // No act tables: opaque whatever the mode.
    assert_eq!(cel_blend(&a, 0), (ShadeChain::EMPTY, BlendOp::Opaque));
    a.shades = Some(crate::rules::shading::ShadeTables {
        light0: MapId(0),
        highlight: MapId(32),
        red: MapId(33),
        zero: MapId(34),
        remap0: MapId(35),
        alpha: [MapId(200), MapId(456), MapId(712)],
        additive: MapId(968),
        multiplicative: MapId(1224),
        max_component: MapId(1480),
    });
    // m = 0 → A2 (25 %), 1 → A1 (50 %), 2 → A0 (75 %), 5 → opaque; unlit.
    for (m, table) in [(0, 712), (1, 456), (2, 200)] {
        let (shade, blend) = cel_blend(&a, m);
        assert_eq!(blend, BlendOp::IndexTable(MapId(table)), "mode {m}");
        assert_eq!(shade, ShadeChain::EMPTY, "mode {m}");
    }
    assert_eq!(cel_blend(&a, 5), (ShadeChain::EMPTY, BlendOp::Opaque));
}
