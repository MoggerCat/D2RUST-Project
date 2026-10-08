// Spec: specs/render/unit-composite.md (§9), specs/render/camera.md (§2, §4), specs/render/sprite-placement.md (§8), specs/items/inventory-moves.md (§7.1, §7.2), specs/ui/controls.md (§6 r4)
//! Ground items with synthetic fixtures: a model with items on the ground,
//! flippy DC6 files in a memory source, a recording link.

use std::sync::{Arc, Mutex};

use d2_formats::palette::{Palette, Rgb};
use d2_proto::PROTOCOL_VERSION;

use super::*;
use crate::assets::path::MemorySource;
use crate::bridge::dispatch::Dispatch;
use crate::bridge::items::{ItemArtRow, ITEM};
use crate::bridge::link::{LinkError, Pumped, SendQueue, Sent};
use crate::bridge::world::{ClientUnit, ItemData, ItemRecord, KindData, PlayerData, UnitKey};
use crate::ui::Point;
use crate::world_view::model_feed::ModelFeed;
use crate::world_view::NoFeed;

/// A stream head: flags, version, mode, location, code (`bitstream.md`
/// §2, §3, §4.1).
fn stream(m: u8, loc: (u16, u16), code: &[u8; 4]) -> Vec<u8> {
    let mut bits: Vec<(u32, u32)> = vec![(0x10, 32), (0x65, 10), (u32::from(m), 3)];
    if matches!(m, 3 | 5) {
        bits.extend([(u32::from(loc.0), 16), (u32::from(loc.1), 16)]);
    } else {
        bits.extend([(0, 4), (u32::from(loc.0), 4), (u32::from(loc.1), 4), (0, 3)]);
    }
    bits.push((u32::from_le_bytes(*code), 32));
    let (mut out, mut acc, mut n) = (Vec::new(), 0u64, 0);
    for (v, w) in bits {
        acc |= u64::from(v) << n;
        n += w;
        while n >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            n -= 8;
        }
    }
    if n > 0 {
        out.push(acc as u8);
    }
    out
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

const PLAYER: UnitKey = UnitKey {
    unit_type: 0,
    guid: 1,
};

/// An item fixture: GUID, mode, location, code.
type Fixture<'a> = (u32, u8, (u16, u16), &'a [u8; 4]);

/// The local player at cell (100, 100); items `(guid, mode, (x, y),
/// code)`.
fn populate(w: &mut ClientWorld, items: &[Fixture<'_>]) {
    let mut p = ClientUnit::new(PLAYER);
    p.position = Some((100, 100));
    p.kind = KindData::Player(PlayerData::default());
    w.units.insert(PLAYER, p);
    w.local_player = Some(PLAYER);
    for &(guid, m, loc, code) in items {
        let k = UnitKey::new(ITEM, guid);
        let mut u = ClientUnit::new(k);
        if matches!(m, 3 | 5) {
            u.position = Some(loc);
        }
        u.kind = KindData::Item(ItemData {
            last: Some(ItemRecord {
                id: 0x9C,
                action: 0,
                category: 0,
                owner: None,
                stream: stream(m, loc, code),
            }),
            ..ItemData::default()
        });
        w.units.insert(k, u);
    }
}

fn rows() -> ItemArtRows {
    let mut r = ItemArtRows::default();
    r.0.insert(
        *b"cap ",
        ItemArtRow {
            inv_w: 2,
            inv_h: 2,
            inv_file: "invcap".into(),
            flippy_file: "flpcap".into(),
        },
    );
    r
}

fn source() -> Arc<dyn FileSource> {
    let mut src = MemorySource::default();
    src.insert("data\\global\\items\\flpcap.dc6", dc6(3, 4, 3));
    Arc::new(src)
}

fn assets() -> ViewAssets {
    ViewAssets::new(Palette {
        colors: [Rgb::default(); 256],
    })
}

// Covers: specs/render/unit-composite.md §9; specs/render/camera.md §4
#[test]
fn a_ground_item_draws_its_flippy_at_its_subtile() {
    let mut w = ClientWorld::default();
    populate(&mut w, &[(5, 3, (102, 100), b"cap ")]);
    let mut g = GroundItems::new(source(), rows());
    let mut a = assets();
    let mut frame = WorldFrame::default();
    let feed = ModelFeed::<NoFeed>::default();
    let log = g.add_to_frame(&w, &feed, &mut a, &mut frame);
    assert!(log.is_empty(), "{log:?}");
    assert_eq!(a.frames.len(), 3, "the whole direction is resident");
    // Player: client (0, 1608); unit origin (−400, 1324). Item (102, 100):
    // client (32, 1616) → draw at (432, 300); the 4 × 3 cel is bottom
    // anchored: top-left (432, 298).
    assert_eq!(frame.items.len(), 1);
    let d = frame.items[0];
    assert_eq!((d.x, d.y), (432, 298));
    assert_eq!(d.tag, ItemTag::Unit(5));
    assert_eq!(d.key.0 >> 60, u64::from(pass::SHADOWS));
    // The resting cel: the flippy's last frame.
    let set = FrameSetKey::new("data/global/items/flpcap.dc6", FramePart::Dir(0)).unwrap();
    assert_eq!(d.frame, a.id(&set, 2).unwrap());
    assert_eq!(g.hit(433, 299), Some(5));
    assert_eq!(g.hit(436, 299), None);
    assert_eq!(g.hit(433, 301), None);
}

// Covers: specs/render/unit-composite.md §9
#[test]
fn no_rows_or_missing_art_draw_nothing_and_never_fail() {
    let mut w = ClientWorld::default();
    populate(
        &mut w,
        &[(5, 3, (102, 100), b"cap "), (6, 0, (1, 1), b"cap ")],
    );
    let feed = ModelFeed::<NoFeed>::default();
    let mut frame = WorldFrame::default();
    let mut a = assets();
    // The default: no rows, no source.
    let log = GroundItems::default().add_to_frame(&w, &feed, &mut a, &mut frame);
    assert!(log.is_empty() && frame.items.is_empty());
    // A row whose file no archive holds: logged once, not drawn.
    let mut g = GroundItems::new(Arc::new(MemorySource::default()), rows());
    let log = g.add_to_frame(&w, &feed, &mut a, &mut frame);
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(frame.items.is_empty());
    assert!(g.add_to_frame(&w, &feed, &mut a, &mut frame).is_empty());
    // No local player: no camera, nothing drawn.
    let mut g = GroundItems::new(source(), rows());
    w.local_player = None;
    assert!(g.add_to_frame(&w, &feed, &mut a, &mut frame).is_empty());
    assert!(frame.items.is_empty());
}

#[derive(Clone, Default)]
struct RecordingLink {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl ServerLink for RecordingLink {
    fn protocol_version(&self) -> u32 {
        PROTOCOL_VERSION
    }
    fn send(&mut self, _: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.sent.lock().unwrap().push(msg.to_vec());
        Ok(Sent::Queued)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        Ok(Pumped { ticked: false })
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        Vec::new()
    }
}

fn press(x: i32, y: i32) -> UiEvent {
    UiEvent::Press {
        button: PointerButton::Left,
        at: Point::new(x, y),
    }
}

/// A bridge whose model holds the player and the items, and the ground
/// items drawn for it.
fn scene(items: &[Fixture<'_>]) -> (Bridge<RecordingLink>, RecordingLink, GroundItems) {
    let link = RecordingLink::default();
    let mut b = Bridge::with_dispatch(link.clone(), Dispatch::empty()).unwrap();
    populate(b.world_mut(), items);
    let mut g = GroundItems::new(source(), rows());
    let mut frame = WorldFrame::default();
    let feed = ModelFeed::<NoFeed>::default();
    g.add_to_frame(b.world(), &feed, &mut assets(), &mut frame);
    (b, link, g)
}

// Covers: specs/items/inventory-moves.md §7.1
#[test]
fn a_press_on_a_ground_item_sends_pick_item() {
    let (mut b, link, g) = scene(&[(5, 3, (102, 100), b"cap ")]);
    let rest = g
        .take_clicks(&mut b, &[press(433, 299), press(10, 10)])
        .unwrap();
    assert_eq!(
        rest,
        [press(10, 10)],
        "the ground click goes on to the walk"
    );
    assert_eq!(
        *link.sent.lock().unwrap(),
        [vec![0x16, 4, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0]]
    );
}

// Covers: specs/ui/controls.md §6 r4; specs/items/inventory-moves.md §7.2
#[test]
fn a_world_press_with_a_cursor_item_sends_drop_item() {
    let (mut b, link, g) = scene(&[(5, 3, (102, 100), b"cap "), (9, 4, (0, 0), b"cap ")]);
    if let Some(KindData::Player(d)) = b.world_mut().units.get_mut(&PLAYER).map(|u| &mut u.kind) {
        d.cursor_item = Some(9);
    }
    // Even over a ground item: the held item is dropped.
    let rest = g.take_clicks(&mut b, &[press(433, 299)]).unwrap();
    assert!(rest.is_empty());
    assert_eq!(*link.sent.lock().unwrap(), [vec![0x17, 9, 0, 0, 0]]);
}
