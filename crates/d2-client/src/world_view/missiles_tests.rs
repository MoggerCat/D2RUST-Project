// Spec: specs/render/unit-composite.md (§9), specs/missiles/missiles.md (R4.1), specs/sim/intents-events.md (§7.6 r1)
//! Client missiles with synthetic fixtures: a Fire Bolt cast request in the
//! model, missile and overlay DC6 files in a memory source.

use d2_formats::palette::{Palette, Rgb};

use super::*;
use crate::assets::path::MemorySource;
use crate::bridge::world::{KindData, PlayerData};
use crate::world_view::model_feed::ModelFeed;
use crate::world_view::NoFeed;

const PLAYER: UnitKey = UnitKey {
    unit_type: 0,
    guid: 1,
};
const FIRE_BOLT: usize = 3;

/// A DC6 of one direction, `frames` frames of `w × h` literal pixels.
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

fn rows() -> EffectRows {
    let mut by_skill = vec![0; 8];
    by_skill[FIRE_BOLT] = 1;
    let mut r = EffectRows {
        skill_missile: by_skill.clone(),
        skill_overlay: by_skill,
        missiles: vec![
            MissileRow::default(),
            MissileRow {
                cel_file: "firebolt".into(),
                vel: 16,
                range: 20,
                anim_rate: 256,
                anim_len: 4,
                loop_anim: true,
                offset: (0, 0, 0),
                explosion: 2,
                trans: 1,
                clt_do_func: 0,
            },
            MissileRow {
                cel_file: "boom".into(),
                anim_rate: 256,
                anim_len: 3,
                ..MissileRow::default()
            },
        ],
        overlays: vec![
            OverlayRow::default(),
            OverlayRow {
                file: "cast".into(),
                frames: 2,
                anim_rate: 256,
                offset: (0, 0),
                trans: 3,
                pre_draw: false,
            },
        ],
        ..EffectRows::default()
    };
    r.state_overlay.insert(9, 1);
    r
}

fn source() -> Arc<dyn FileSource> {
    let mut src = MemorySource::default();
    src.insert("data\\global\\missiles\\firebolt.dc6", dc6(4, 4, 3));
    src.insert("data\\global\\missiles\\boom.dc6", dc6(3, 6, 5));
    src.insert("data\\global\\overlays\\cast.dc6", dc6(2, 8, 8));
    Arc::new(src)
}

/// View assets with the act's shade tables (`blend-modes.md` §4 draws
/// through them).
fn assets() -> ViewAssets {
    let mut a = ViewAssets::new(Palette {
        colors: [Rgb::default(); 256],
    });
    let pl2 = d2_formats::palette::Pl2::parse(&super::super::tile_assets::tests::pl2()).unwrap();
    a.shades = Some(crate::rules::shading::ShadeTables::push(&mut a.maps, &pl2));
    a
}

fn world() -> ClientWorld {
    let mut w = ClientWorld::default();
    let mut p = ClientUnit::new(PLAYER);
    p.position = Some((100, 100));
    p.kind = KindData::Player(PlayerData::default());
    w.units.insert(PLAYER, p);
    w.local_player = Some(PLAYER);
    w
}

/// The cast request of S→C 0x4D: skill, point.
fn cast(w: &mut ClientWorld, skill: i32, at: (i32, i32)) {
    w.units.get_mut(&PLAYER).unwrap().last_mode_request = Some(ModeRequest {
        code: CAST_POINT,
        record: [skill, -1, at.0, at.1, 2, 0, 0],
    });
}

/// A built frame of `w`: its one camera (`seams/world-screen.md` §2.2),
/// `None` without a local player.
fn framed(w: &ClientWorld) -> WorldFrame {
    let mut feed = ModelFeed::<NoFeed>::default();
    WorldFrame {
        camera: crate::world_view::frame_camera(w, &mut feed).unwrap(),
        ..WorldFrame::default()
    }
}

struct Run {
    m: Missiles,
    a: ViewAssets,
    feed: ModelFeed<NoFeed>,
}

impl Run {
    fn new() -> Self {
        Run {
            m: Missiles::new(source(), rows()),
            a: assets(),
            feed: ModelFeed::<NoFeed>::default(),
        }
    }

    /// One frame at server tick `t`: the draws' positions.
    fn frame(&mut self, w: &mut ClientWorld, t: u64) -> Vec<(i32, i32)> {
        w.server_ticks = t;
        let mut frame = framed(w);
        let log = self.m.add_to_frame(w, &self.feed, &mut self.a, &mut frame);
        assert!(log.is_empty(), "{log:?}");
        frame.items.iter().map(|d| (d.x, d.y)).collect()
    }
}

// Covers: specs/missiles/missiles.md §r4-1-movement-in-fixed-point
#[test]
fn a_fire_bolt_cast_shows_a_missile_that_moves_each_tick() {
    let mut w = world();
    let mut r = Run::new();
    assert!(r.frame(&mut w, 10).is_empty(), "nothing cast yet");
    cast(&mut w, FIRE_BOLT as i32, (130, 100));
    let t0 = r.frame(&mut w, 11);
    // The cast overlay on the caster and the missile at its origin.
    assert_eq!(t0.len(), 2, "{t0:?}");
    let missile = |v: &[(i32, i32)]| v.iter().copied().max_by_key(|p| p.0).unwrap();
    let (x0, y0) = missile(&t0);
    let t1 = r.frame(&mut w, 12);
    let t2 = r.frame(&mut w, 13);
    let (x1, y1) = missile(&t1);
    let (x2, y2) = missile(&t2);
    // Vel 16 = one subtile east per tick = (+16, +8) on screen.
    assert_eq!((x1 - x0, y1 - y0), (16, 8), "{t0:?} {t1:?}");
    assert_eq!((x2 - x1, y2 - y1), (16, 8), "{t1:?} {t2:?}");
}

// Covers: specs/missiles/missiles.md §r4-1-movement-in-fixed-point
#[test]
fn the_missile_explodes_at_the_end_of_its_range_and_leaves() {
    let mut w = world();
    let mut r = Run::new();
    r.frame(&mut w, 10);
    cast(&mut w, FIRE_BOLT as i32, (130, 100));
    r.frame(&mut w, 11);
    // Range 20 ticks: the flight is gone, its explosion plays 3 ticks.
    r.frame(&mut w, 31);
    assert_eq!(r.m.live(), 1, "the explosion");
    r.frame(&mut w, 35);
    assert_eq!(r.m.live(), 0, "all effects have ended");
}

// Covers: specs/missiles/missiles.md §r4-1-movement-in-fixed-point
#[test]
fn the_missile_stops_at_a_monster_on_its_way() {
    let mut w = world();
    let mut r = Run::new();
    r.frame(&mut w, 10);
    let k = UnitKey::new(MONSTER, 7);
    let mut m = ClientUnit::new(k);
    m.position = Some((105, 100));
    m.mode = 1;
    w.units.insert(k, m);
    cast(&mut w, FIRE_BOLT as i32, (130, 100));
    r.frame(&mut w, 11);
    for t in 12..=16 {
        r.frame(&mut w, t);
    }
    // Hit at about subtile 104: long before the 20-tick range; the
    // explosion (3 ticks) is what is left, then nothing.
    assert_eq!(r.m.live(), 1, "the explosion replaces the flight");
    r.frame(&mut w, 22);
    assert_eq!(r.m.live(), 0);
}

// Covers: specs/render/overlay.md §1
#[test]
fn a_state_overlay_plays_on_its_unit_and_no_rows_draw_nothing() {
    let mut w = world();
    let mut r = Run::new();
    assert!(r.frame(&mut w, 1).is_empty());
    w.units.get_mut(&PLAYER).unwrap().states.insert(9);
    assert_eq!(r.frame(&mut w, 2).len(), 1);
    assert_eq!(r.frame(&mut w, 3).len(), 1, "it loops");
    w.units.get_mut(&PLAYER).unwrap().states.clear();
    assert!(r.frame(&mut w, 4).is_empty());
    // The default: no rows, no source.
    let mut frame = framed(&w);
    let mut a = assets();
    let feed = ModelFeed::<NoFeed>::default();
    let log = Missiles::default().add_to_frame(&w, &feed, &mut a, &mut frame);
    assert!(log.is_empty() && frame.items.is_empty());
}

// Covers: specs/render/unit-composite.md §9
#[test]
fn a_missing_art_file_is_logged_once_and_not_drawn() {
    let mut w = world();
    let feed = ModelFeed::<NoFeed>::default();
    let mut m = Missiles::new(Arc::new(MemorySource::default()), rows());
    let mut a = assets();
    m.add_to_frame(&w, &feed, &mut a, &mut framed(&w));
    cast(&mut w, FIRE_BOLT as i32, (130, 100));
    w.server_ticks = 2;
    let mut frame = framed(&w);
    let log = m.add_to_frame(&w, &feed, &mut a, &mut frame);
    assert_eq!(log.len(), 2, "overlay and missile: {log:?}");
    assert!(frame.items.is_empty());
    w.server_ticks = 3;
    assert!(m
        .add_to_frame(&w, &feed, &mut a, &mut framed(&w))
        .is_empty());
}

// Covers: specs/render/camera.md §2
// Covers: specs/seams/world-screen.md §2.4
#[test]
fn the_local_players_overlays_follow_its_predicted_position() {
    use crate::world_view::preview::Preview;
    let mut w = world();
    w.units.get_mut(&PLAYER).unwrap().states.insert(9);
    let mut m = Missiles::new(source(), rows());
    let mut a = assets();
    let mut feed = ModelFeed::<NoFeed>::default().with_preview(Preview::default());
    let mut draw = |feed: &mut ModelFeed<NoFeed>, t: u64| {
        w.server_ticks = t;
        let mut frame = WorldFrame {
            camera: crate::world_view::frame_camera(&w, feed).unwrap(),
            ..WorldFrame::default()
        };
        let log = m.add_to_frame(&w, &*feed, &mut a, &mut frame);
        assert!(log.is_empty(), "{log:?}");
        frame.items.iter().map(|d| (d.x, d.y)).collect::<Vec<_>>()
    };
    let at_cell = draw(&mut feed, 1);
    assert_eq!(at_cell.len(), 1);
    // The walk prediction three sub-tiles east of the model cell: the
    // camera follows it, and so does the overlay (same screen point).
    let c = |s: u32| (s << 16) | 0x8000;
    feed.set_local_prediction(Some((PLAYER, (c(103), c(100)))));
    assert_eq!(draw(&mut feed, 1), at_cell);
}

// Covers: specs/render/blend-modes.md §4
#[test]
fn missiles_and_overlays_draw_in_their_trans_mode() {
    let mut w = world();
    let mut r = Run::new();
    let t = r.a.shades.unwrap();
    r.frame(&mut w, 10);
    cast(&mut w, FIRE_BOLT as i32, (130, 100));
    r.frame(&mut w, 11);
    let ops = |m: &Missiles| -> Vec<(ShadeChain, BlendOp)> {
        m.last()
            .iter()
            .map(|d| (d.item.shade, d.item.blend))
            .collect()
    };
    // Cast overlay `Trans` 3 (the overlay's mode as is) and missile
    // `Trans` 1 → mode 3: both additive, unlit, no remap.
    let add = cel_ops(&t, 3, None, 0xFF);
    assert_eq!(ops(&r.m), vec![add, add]);
    assert_ne!(add.1, BlendOp::Opaque);
    // Missile `Trans` 2 → mode 4, anything else → 5 (opaque).
    assert_eq!(missile_mode(2, false), 4);
    assert_eq!(missile_mode(0, false), MODE_OPAQUE);
    let mut rows = rows();
    rows.missiles[1].trans = 2;
    rows.overlays[1].trans = 5;
    let mut r = Run::new();
    r.m = Missiles::new(source(), rows);
    let mut w = world();
    r.frame(&mut w, 10);
    cast(&mut w, FIRE_BOLT as i32, (130, 100));
    r.frame(&mut w, 11);
    assert_eq!(
        ops(&r.m),
        vec![cel_ops(&t, 5, None, 0xFF), cel_ops(&t, 4, None, 0xFF)]
    );
    assert_eq!(ops(&r.m)[0], (ShadeChain::EMPTY, BlendOp::Opaque));
}

// Covers: specs/render/draw-order.md §3 r4; specs/render/unit-composite.md §5 r4
#[test]
fn missiles_join_their_cell_and_overlays_their_host() {
    use crate::rules::camera::{moving_to_client, FrameSize, OpenMode};
    use crate::rules::draw_order::{tile_of, DrawGrid, OrderKey, UnitSlot};
    let mut w = world();
    let mut r = Run::new();
    r.frame(&mut w, 10);
    cast(&mut w, FIRE_BOLT as i32, (130, 100));
    r.frame(&mut w, 11);
    // The player's camera; the missile starts at the caster's cell
    // centre, so it is in the caster's draw cell.
    let at = moving_to_client(cell_centre((100, 100)).0, cell_centre((100, 100)).1);
    let camera = Camera::new(FrameSize::D2RS, OpenMode::NONE, at, (0, 0));
    let grid = DrawGrid::of_camera(&camera);
    let ci = grid.cell(tile_of(at.x, at.y)).unwrap() as u32;
    let slot = |minor| {
        UnitSlot::Drawn(OrderKey {
            pass: pass::WALLS_UNITS,
            major: ci,
            minor,
        })
    };
    let other = UnitKey::new(MONSTER, 9);
    let slots = BTreeMap::from([(PLAYER, slot(3)), (other, slot(4))]);
    // The caster below the missile (client y not smaller): the missile
    // goes before it (sub 0, ahead of equal keys); the cast overlay
    // (`PreDraw` 0) after the caster's slots (sub 255).
    let below = |k: UnitKey| (k == PLAYER).then_some(i32::MAX);
    let keyed = Missiles::keyed(r.m.last(), &slots, &grid, below);
    let got: Vec<_> = keyed.iter().map(|(d, b)| (d.item.key, *b)).collect();
    assert_eq!(
        got,
        vec![
            (DrawKey::new(6, ci, 3, 255).unwrap(), false),
            (DrawKey::new(6, ci, 3, 0).unwrap(), true),
        ]
    );
    // Every unit of the cell above it: after the cell's units.
    let keyed = Missiles::keyed(r.m.last(), &slots, &grid, |_| Some(i32::MIN));
    assert_eq!(
        keyed[1].0.item.key,
        DrawKey::new(6, ci, DrawKey::MINOR_MAX, 255).unwrap()
    );
    assert!(!keyed[1].1);
    // A back overlay draws before its host's slot 0; a hidden host hides
    // its overlays.
    let mut rows = rows();
    rows.overlays[1].pre_draw = true;
    let mut r2 = Run::new();
    r2.m = Missiles::new(source(), rows);
    let mut w2 = world();
    r2.frame(&mut w2, 10);
    cast(&mut w2, FIRE_BOLT as i32, (130, 100));
    r2.frame(&mut w2, 11);
    let keyed = Missiles::keyed(r2.m.last(), &slots, &grid, below);
    assert_eq!(
        (keyed[0].0.item.key, keyed[0].1),
        (DrawKey::new(6, ci, 3, 0).unwrap(), true)
    );
    let hidden = BTreeMap::from([(PLAYER, UnitSlot::NotDrawn)]);
    let keyed = Missiles::keyed(r2.m.last(), &hidden, &grid, below);
    assert_eq!(keyed.len(), 1, "the missile alone: no units in its cell");
    assert_eq!(
        keyed[0].0.item.key,
        DrawKey::new(6, ci, DrawKey::MINOR_MAX, 255).unwrap()
    );

    // In the frame, with the feed's positions: a monster two sub-tiles
    // south of the caster is below the missile, which goes before its
    // slot-0 item; the overlay after the caster's.
    let mut w3 = world();
    let mut m = ClientUnit::new(other);
    m.position = Some((100, 102));
    w3.units.insert(other, m);
    let mut frame = WorldFrame {
        camera: Some(camera),
        slots: Some(BTreeMap::from([(PLAYER, slot(3)), (other, slot(5))])),
        ..WorldFrame::default()
    };
    for (minor, tag) in [(3, ItemTag::Unit(1)), (5, ItemTag::Unit(9))] {
        let mut item = DrawItem::new(crate::scene::FrameId(0), 0, 0);
        item.key = DrawKey::new(6, ci, minor, 0).unwrap();
        item.tag = tag;
        frame.items.push(item);
    }
    let mut r3 = Run::new();
    r3.frame(&mut w3, 10);
    cast(&mut w3, FIRE_BOLT as i32, (130, 100));
    w3.server_ticks = 11;
    let feed = ModelFeed::<NoFeed>::default();
    let log = r3.m.add_to_frame(&w3, &feed, &mut r3.a, &mut frame);
    assert!(log.is_empty(), "{log:?}");
    let got: Vec<_> = frame
        .items
        .iter()
        .map(|i| (i.key.minor(), i.key.sub(), i.tag))
        .collect();
    assert_eq!(got.len(), 4, "{got:?}");
    assert_eq!(got[0], (3, 0, ItemTag::Unit(1)));
    assert_eq!(
        (got[1].0, got[1].1),
        (3, 255),
        "the overlay after the caster"
    );
    assert_eq!(
        (got[2].0, got[2].1),
        (5, 0),
        "the missile before the monster"
    );
    assert_ne!(got[2].2, ItemTag::Unit(9));
    assert_eq!(got[3], (5, 0, ItemTag::Unit(9)));
}

// Covers: specs/missiles/client.md §c13-function-bodies-specified-here
// Covers: specs/render/draw-order.md §3 r4
#[test]
fn flat_missiles_file_in_their_cells_shadow_list() {
    use crate::rules::camera::{moving_to_client, FrameSize, OpenMode};
    use crate::rules::draw_order::{tile_of, DrawGrid};
    // Functions 2 and 11 set flag 0x10000 at the animation end: frame +
    // speed ≥ length (§C4 r4), frame = age · speed.
    assert!(!flat_at_end(11, 3, 256, 1));
    assert!(flat_at_end(11, 3, 256, 2));
    assert!(!flat_at_end(2, 4, 128, 6));
    assert!(flat_at_end(2, 4, 128, 7));
    assert!(!flat_at_end(1, 3, 256, 99), "other functions never");
    // A flat missile goes to its cell's shadow list (pass 5), after the
    // cell's entries; not into the unit list.
    let at = moving_to_client(cell_centre((100, 100)).0, cell_centre((100, 100)).1);
    let camera = Camera::new(FrameSize::D2RS, OpenMode::NONE, at, (0, 0));
    let grid = DrawGrid::of_camera(&camera);
    let ci = grid.cell(tile_of(at.x, at.y)).unwrap() as u32;
    let draw = MissileDraw {
        id: 1,
        item: DrawItem::new(crate::scene::FrameId(0), 0, 0),
        join: Join::Flat { at },
    };
    let keyed = Missiles::keyed(&[draw], &BTreeMap::new(), &grid, |_| None);
    assert_eq!(
        keyed
            .iter()
            .map(|(d, b)| (d.item.key, *b))
            .collect::<Vec<_>>(),
        vec![(
            DrawKey::new(pass::SHADOWS, ci, DrawKey::MINOR_MAX, 255).unwrap(),
            false
        )]
    );
}

// Covers: specs/render/overlay.md §2 r5, §2 r8, §3 r2, §3 r9
#[test]
fn a_ui_overlay_call_creates_advances_draws_on_its_host_and_removes() {
    let mut w = world();
    let mut r = Run::new();
    r.m.rows.overlay_rules = vec![
        OverlayRules::default(),
        OverlayRules {
            xoffset: -5,
            yoffset: -7,
            height: [-30, 0, 0, -50],
            anim_rate: 9,
            ..OverlayRules::default()
        },
    ];
    // Class 3's `monstats2` row: `overlayHeight` 4 → `Height4`.
    r.m.rows.monster_overlay = vec![(false, None); 3];
    r.m.rows.monster_overlay.push((false, Some(4)));
    let k = UnitKey::new(MONSTER, 7);
    let mut npc = ClientUnit::new(k);
    npc.position = Some((102, 100));
    npc.class = 3;
    w.units.insert(k, npc);
    r.frame(&mut w, 1);
    let on = OverlayCall {
        unit: k,
        id: 1,
        on: true,
    };
    r.m.overlay_calls(2, [on]);
    // Created before the update's walk: advanced once in the same update.
    assert_eq!(r.frame(&mut w, 2).len(), 1);
    let rec = r.m.unit_overlays(k)[0];
    assert_eq!(
        (rec.kind, rec.rate, rec.frame, rec.frames),
        (3, 144, 144, 512)
    );
    assert_eq!((rec.x, rec.y), (-5, -57));
    let d = r.m.last()[0];
    assert_eq!(d.item.tag, ItemTag::Unit(7), "drawn in its host's unit run");
    assert_eq!(
        d.join,
        Join::Host {
            host: k,
            back: false
        }
    );
    // Two more updates: +0x18 = 432, frame 1; kind 3 wraps at the end.
    r.frame(&mut w, 4);
    assert_eq!(r.m.unit_overlays(k)[0].frame_index(), 1);
    r.frame(&mut w, 6);
    assert_eq!(r.m.unit_overlays(k)[0].frame, 5 * 144 - 512);
    // A second create of the same id replaces the record (§2 r5).
    r.m.overlay_calls(7, [on]);
    r.frame(&mut w, 7);
    assert_eq!(r.m.unit_overlays(k).len(), 1);
    assert_eq!(r.m.unit_overlays(k)[0].frame, 144);
    // Remove by id; then nothing draws.
    r.m.overlay_calls(8, [OverlayCall { on: false, ..on }]);
    assert!(r.frame(&mut w, 8).is_empty());
    assert!(r.m.unit_overlays(k).is_empty());
}

// Covers: specs/render/overlay.md §3 r2
#[test]
fn a_call_delivered_on_a_skipped_tick_runs_just_before_that_ticks_step() {
    let mut w = world();
    let mut r = Run::new();
    r.m.rows.overlay_rules = vec![
        OverlayRules::default(),
        OverlayRules {
            anim_rate: 9,
            ..OverlayRules::default()
        },
    ];
    let k = UnitKey::new(MONSTER, 7);
    let mut npc = ClientUnit::new(k);
    npc.position = Some((102, 100));
    w.units.insert(k, npc);
    r.frame(&mut w, 1);
    // Tick 2 draws no frame; the call delivered at tick 3 waits for the
    // frame of tick 3, which catches up ticks 2 and 3: one advance.
    r.m.overlay_calls(
        3,
        [OverlayCall {
            unit: k,
            id: 1,
            on: true,
        }],
    );
    r.frame(&mut w, 3);
    assert_eq!(r.m.unit_overlays(k)[0].frame, 144);
}
