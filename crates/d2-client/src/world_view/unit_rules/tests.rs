// Spec: specs/render/unit-composite.md (§2, §5.1, §6, §10)
//! Synthetic fixtures only: invented tokens, COFs and DC6 files.

use std::sync::Arc;

use d2_formats::palette::{Palette, Rgb};

use super::*;
use crate::assets::path::MemorySource;
use crate::bridge::world::{UnitKey, MONSTER, OBJECT, PLAYER};
use crate::rules::unit_composite::{code, Code};
use crate::world_view::unit_assets::{MonsterRow, UnitArtLoader, UnitLooks};
use crate::world_view::{build, Unspecified};

/// COF bytes (`formats/cof.md`): one direction, `frames` frames, the
/// given layers (component, weapon class), animation rate 256 (one frame
/// per tick), each frame drawing the layers in order.
fn cof_bytes(frames: u8, layers: &[u8]) -> Vec<u8> {
    cof_dirs(1, frames, layers)
}

/// [`cof_bytes`] with `dirs` directions.
fn cof_dirs(dirs: u8, frames: u8, layers: &[u8]) -> Vec<u8> {
    let l = layers.len() as u8;
    let mut v = vec![l, frames, dirs, 20, 0, 0, 0, 0];
    for x in [-10i32, 10, -20, 0] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(&256u32.to_le_bytes());
    for c in layers {
        v.extend_from_slice(&[*c, 0, 1, 0, 0]);
        v.extend_from_slice(b"hth\0");
    }
    v.extend(std::iter::repeat_n(0, usize::from(frames)));
    for _ in 0..usize::from(frames) * usize::from(dirs) {
        v.extend_from_slice(layers);
    }
    v
}

/// A DC6 of one direction with `frames` frames of `w` × `h` literal
/// pixels (`formats/dc6.md`).
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

fn codes(names: &[&[u8]]) -> Vec<Code> {
    names.iter().map(|n| code(n)).collect()
}

/// Invented tokens: player class 0 = `QA`, monster 7 = `QM` (base weapon
/// `qwc`), object 342 (a DC6 object row, §6 r2) = `QO`.
fn looks() -> UnitLooks {
    let mut l = UnitLooks {
        player_tokens: codes(&[b"QA"]),
        player_modes: codes(&[b"QD", b"QN"]),
        monster_modes: codes(&[b"QD", b"QN"]),
        object_modes: codes(&[b"QN"]),
        components: codes(&[b"QH", b"QT"]),
        ..Default::default()
    };
    l.monsters.insert(
        7,
        MonsterRow {
            token: code(b"QM"),
            base_w: Some(code(b"qwc")),
            composite_death: false,
        },
    );
    l.objects.insert(342, code(b"QO"));
    l
}

fn unit(unit_type: u8, guid: u32, class: u32, mode: u32) -> ClientUnit {
    let mut u = ClientUnit::new(UnitKey { unit_type, guid });
    u.class = class;
    u.mode = mode;
    u.position = Some((10, 10));
    u
}

fn assets() -> ViewAssets {
    ViewAssets::new(Palette {
        colors: [Rgb::default(); 256],
    })
}

/// [`Unspecified`] with a fixed placement, so a unit can be built
/// without a camera.
struct Placed;

impl ViewRules for Placed {
    fn tiles(&self, w: &ClientWorld, a: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        Unspecified.tiles(w, a)
    }
    fn unit_pose(&self, w: &ClientWorld, u: &ClientUnit) -> Result<Option<UnitPose>, ViewError> {
        Unspecified.unit_pose(w, u)
    }
    fn unit_params(
        &self,
        w: &ClientWorld,
        u: &ClientUnit,
        p: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        Unspecified.unit_params(w, u, p)
    }
    fn component_frame(
        &self,
        u: &ClientUnit,
        p: &UnitPose,
        r: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError> {
        Unspecified.component_frame(u, p, r)
    }
    fn place(
        &self,
        _: &ClientUnit,
        _: &UnitPose,
        _: &ComponentRequest<'_>,
        _: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError> {
        Ok((100, 200))
    }
    fn shade(
        &self,
        u: &ClientUnit,
        r: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError> {
        Unspecified.shade(u, r)
    }
    fn blend(&self, u: &ClientUnit, r: &ComponentRequest<'_>) -> Result<BlendOp, CompositeError> {
        Unspecified.blend(u, r)
    }
}

impl UiRules for Placed {
    fn ui_image(&self, r: &ImageRequest, a: &ViewAssets) -> Result<UiSprite, ViewError> {
        Unspecified.ui_image(r, a)
    }
    fn ui_text(&self, r: &TextRequest, a: &ViewAssets) -> Result<Vec<UiSprite>, ViewError> {
        Unspecified.ui_text(r, a)
    }
    fn ui_pass(&self) -> Result<u32, ViewError> {
        Ok(pass::UI)
    }
}

fn setup(src: MemorySource) -> (UnitArtLoader, UnitRules<Placed>) {
    let looks = Arc::new(looks());
    let art = SharedUnitArt::default();
    let loader = UnitArtLoader {
        source: Arc::new(src),
        looks: looks.clone(),
        art: art.clone(),
    };
    let rules = UnitRules {
        rules: Placed,
        looks,
        art,
    };
    (loader, rules)
}

// Covers: specs/render/unit-composite.md §2, §2.1, §5.1 r3, §6 r1
#[test]
fn cof_and_component_names_from_the_looks() {
    let l = looks();
    let p = unit(PLAYER, 1, 0, 1);
    let name = unit_cof(&l, &p).unwrap();
    assert_eq!(name.full(), "DATA\\GLOBAL\\CHARS\\QA\\COF\\QAQNhth.COF");
    let layer = d2_formats::cof::CofLayer {
        component: 1,
        shadow: 0,
        selectable: 1,
        override_translucency: 0,
        new_translucency: 0,
        weapon_class: *b"hth\0",
    };
    // D1 preview: no items, so the torso is `lit`.
    let c = component_codes(&l, &p, &name, &layer).unwrap();
    assert_eq!(c.name(), "QAQTlitQNhth");
    // The file names the layer's own weapon class (REC-441); an empty
    // layer class keeps the unit's.
    let one_hand = d2_formats::cof::CofLayer {
        weapon_class: *b"1ht\0",
        ..layer
    };
    let c = component_codes(&l, &p, &name, &one_hand).unwrap();
    assert_eq!(c.name(), "QAQTlitQN1ht");
    let empty = d2_formats::cof::CofLayer {
        weapon_class: [0; 4],
        ..layer
    };
    let c = component_codes(&l, &p, &name, &empty).unwrap();
    assert_eq!(c.name(), "QAQTlitQNhth");
    // A monster's weapon class is its BaseW (§2.1).
    let m = unit(MONSTER, 2, 7, 1);
    assert_eq!(
        unit_cof(&l, &m).unwrap().full(),
        "DATA\\GLOBAL\\MONSTERS\\QM\\COF\\QMQNqwc.COF"
    );
    // Unknown class, unknown mode, a unit type without a composite: none.
    assert!(unit_cof(&l, &unit(MONSTER, 3, 8, 1)).is_none());
    assert!(unit_cof(&l, &unit(OBJECT, 4, 342, 5)).is_none());
    assert!(unit_cof(&l, &unit(4, 5, 0, 0)).is_none());
}

// Covers: specs/render/unit-composite.md §6 r2, §3 r2
#[test]
fn an_object_loads_draws_and_animates() {
    let mut src = MemorySource::default();
    src.insert(
        "data\\global\\objects\\QO\\cof\\QOQNhth.cof",
        cof_bytes(3, &[1]),
    );
    src.insert(
        "data\\global\\objects\\QO\\QT\\QOQTlitQNhth.dc6",
        dc6(3, 4, 2),
    );
    let (loader, rules) = setup(src);
    let mut world = ClientWorld::default();
    let o = unit(OBJECT, 9, 342, 0);
    world.units.insert(o.key, o.clone());
    let mut a = assets();

    // Not loaded yet: not drawn, no error.
    assert_eq!(rules.unit_pose(&world, &o).unwrap(), None);
    assert!(loader.ensure(&world, &mut a).is_empty());
    assert_eq!(a.cofs.len(), 1);
    // One direction of three frames, and its derived shadow set
    // (`unit_shadow`, blend-modes.md §5): three more.
    assert_eq!(a.frames.len(), 6, "one direction of three frames + shadows");
    // Loaded once.
    assert!(loader.ensure(&world, &mut a).is_empty());
    assert_eq!(a.frames.len(), 6);

    // §3 r2: the frame is the model's +0x44 >> 8 (the client object
    // update animates it), whatever the tick; frame 0 is a frame.
    let mut o = o;
    for (tick, counter, frame) in [(1, 0, 0), (2, 0, 0), (3, 0x100, 1), (4, 0x2FF, 2)] {
        world.server_ticks = tick;
        o.frame = counter;
        let pose = rules.unit_pose(&world, &o).unwrap().unwrap();
        assert_eq!((pose.dir, pose.frame), (0, frame), "tick {tick}");
        let built = build(&world, &[], &rules, &a).unwrap();
        assert_eq!((built.units_drawn, built.items.len()), (1, 1));
        let item = &built.items[0];
        assert_eq!((item.x, item.y), (100, 200));
        assert_eq!(item.tag, ItemTag::Unit(9));
        assert_eq!(
            (item.shade, item.blend),
            (ShadeChain::EMPTY, BlendOp::Opaque)
        );
    }
    // No `% F`: a frame past the COF's 3 frames is used as is (§3 r6).
    o.frame = 0x300;
    assert_eq!(rules.unit_pose(&world, &o).unwrap().unwrap().frame, 3);
}

// Covers: specs/render/unit-composite.md §2 r4, §5 r2, §5.1 r3, §6 r4
#[test]
fn missing_files_are_skipped_with_one_log_line() {
    let mut src = MemorySource::default();
    // The player's COF is there, its torso file is not; the monster has
    // no COF at all.
    src.insert(
        "data\\global\\chars\\QA\\cof\\QAQNhth.cof",
        cof_bytes(1, &[0, 1]),
    );
    let (loader, rules) = setup(src);
    let mut world = ClientWorld::default();
    for u in [unit(PLAYER, 1, 0, 1), unit(MONSTER, 2, 7, 1)] {
        world.units.insert(u.key, u);
    }
    let mut a = assets();
    let log = loader.ensure(&world, &mut a);
    // The missing COF is logged; the component files in no archive are
    // the normal empty slots of §6 r4 (no line).
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(log[0].contains("QMQNqwc.COF"), "{log:?}");
    assert_eq!(rules.art.read().unwrap().files.len(), 2, "both remembered");
    assert!(loader.ensure(&world, &mut a).is_empty(), "never retried");
    // The player draws (no component has a file), the monster is hidden.
    let built = build(&world, &[], &rules, &a).unwrap();
    assert_eq!((built.units_drawn, built.units_hidden), (1, 1));
    assert!(built.items.is_empty());
    // Each slot whose file is in no archive is still a draw call
    // (tools/facts-render.md §5 r15), in slot order; no shadow call
    // without the shadow slot of a draw order.
    let calls: Vec<(u8, &str, bool)> = built
        .unit_calls
        .iter()
        .map(|c| {
            (
                c.key.sub(),
                c.path.as_ref().map_or("", |p| p.as_str()),
                c.shadow,
            )
        })
        .collect();
    assert_eq!(
        calls,
        [
            (0, "data/global/chars/qa/qh/qaqhlitqnhth.dcc", false),
            (1, "data/global/chars/qa/qt/qaqtlitqnhth.dcc", false),
        ]
    );
    assert!(built.unit_calls.iter().all(|c| c.tag == ItemTag::Unit(1)));
}

// Covers: specs/render/unit-composite.md §5.1 r3, §6 r4
#[test]
fn an_unreadable_component_file_is_logged() {
    let mut src = MemorySource::default();
    src.insert(
        "data\\global\\chars\\QA\\cof\\QAQNhth.cof",
        cof_bytes(1, &[1]),
    );
    src.insert(
        "data\\global\\chars\\QA\\QT\\QAQTlitQNhth.dcc",
        vec![1, 2, 3],
    );
    let (loader, _) = setup(src);
    let mut world = ClientWorld::default();
    let p = unit(PLAYER, 1, 0, 1);
    world.units.insert(p.key, p);
    let log = loader.ensure(&world, &mut assets());
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(log[0].contains("QAQTlitQNhth.dcc"), "{log:?}");
}

// Covers: specs/render/unit-composite.md §3 r1, §3 r3, §3 r4; specs/sim/pathing.md §8.3; specs/client/model.md §8 r4
#[test]
fn units_face_their_walk_target_and_keep_the_facing() {
    let mut src = MemorySource::default();
    src.insert(
        "data\\global\\chars\\QA\\cof\\QAQNhth.cof",
        cof_dirs(16, 1, &[1]),
    );
    let (loader, rules) = setup(src);
    let mut world = ClientWorld::default();
    let mut remote = unit(PLAYER, 1, 0, 1);
    // 0x0F walk to (20, 10): east, dir64 56 (pathing case D1).
    remote.last_mode_request = Some(crate::bridge::world::ModeRequest {
        code: 0x01,
        record: [20, 10, 0, 0, 0, 0, 0],
    });
    let local = unit(PLAYER, 2, 0, 1);
    world.units.insert(remote.key, remote.clone());
    world.units.insert(local.key, local.clone());
    world.local_player = Some(local.key);
    let mut a = assets();
    loader.ensure(&world, &mut a);
    let dir = |w: &ClientWorld, u: &ClientUnit| rules.unit_pose(w, u).unwrap().unwrap().dir;
    // Remote player: n = 8 on a 16-direction COF, 56 snaps to 56 (table
    // 0, 0, 2, 2, … on 56 >> 2 = 14 → 14 << 2), cof_dir 14.
    assert_eq!(dir(&world, &remote), 14);
    // No facing known: 0.
    assert_eq!(dir(&world, &local), 0);
    // The local player's predicted facing (n = 16): dir64 6 → cof_dir 2
    // (§3 test vector).
    rules.art.write().unwrap().pose_dir = Some((local.key, 6));
    assert_eq!(dir(&world, &local), 2);

    // The remote player arrives and its request is spent: the facing
    // stays (the position change (10, 10) → (20, 10) is east as well).
    let r = world.units.get_mut(&remote.key).unwrap();
    r.position = Some((20, 10));
    r.last_mode_request = None;
    loader.ensure(&world, &mut a);
    assert_eq!(dir(&world, &remote), 14);
    // Standing still: kept.
    loader.ensure(&world, &mut a);
    assert_eq!(dir(&world, &remote), 14);
    // A placement north (0x15) turns it: (20, 10) → (20, 0) is dir64 40,
    // cof_dir 10.
    world.units.get_mut(&remote.key).unwrap().position = Some((20, 0));
    loader.ensure(&world, &mut a);
    assert_eq!(dir(&world, &remote), 10);
}

// Covers: specs/skills/bodies-2b.md §8.11
#[test]
fn a_spinning_unit_loops_its_frames_from_frame_three() {
    let mut src = MemorySource::default();
    src.insert(
        "data\\global\\chars\\QA\\cof\\QAQNhth.cof",
        cof_dirs(16, 8, &[1]),
    );
    let (loader, rules) = setup(src);
    let mut world = ClientWorld::default();
    let local = unit(PLAYER, 2, 0, 1);
    world.units.insert(local.key, local.clone());
    world.local_player = Some(local.key);
    let mut a = assets();
    loader.ensure(&world, &mut a);
    let frame = |w: &ClientWorld| {
        rules
            .unit_pose(w, w.local().unwrap())
            .unwrap()
            .unwrap()
            .frame
    };
    rules.art.write().unwrap().spin = Some(local.key);
    for t in 0..12u64 {
        world.server_ticks = t;
        assert!(frame(&world) >= 3, "tick {t} frame {}", frame(&world));
    }
}

// Covers: specs/render/unit-composite.md §3 r2
/// The measured phase: the draw of tick T has had T − 1 advances
/// (`facts/render/scenes`: rate 80, 16 frames: tick 42 → 12, 52 → 15,
/// 128 → 7, 174 → 6, 224 → 5, 43 → 13).
#[test]
fn tick_frame_has_one_advance_less_than_the_tick() {
    for (tick, frame) in [
        (42, 12),
        (43, 13),
        (52, 15),
        (128, 7),
        (174, 6),
        (224, 5),
        (274, 5),
        (354, 14),
    ] {
        assert_eq!(super::tick_frame(tick, 16, 80), frame, "tick {tick}");
    }
    // M08: the old phase (T advances) gives 13 at tick 42.
    assert_eq!((42 * 80) >> 8, 13, "the old phase");
    assert_ne!(super::tick_frame(42, 16, 80), 13);
    assert_eq!(super::tick_frame(0, 16, 80), 0);
    assert_eq!(super::tick_frame(1, 16, 256), 0);
    assert_eq!(super::tick_frame(2, 16, 256), 1);
}

// Covers: specs/sim/units.md §4.7
/// The town walk's frame counts from the walk request at speed 213
/// (`a1-walk-*`: started at tick 22, 1.14d frames 0, 3, 7, 3, 6, 2 at
/// ticks 32, 46, 60, 74, 88, 102).
#[test]
fn the_walk_frame_counts_from_the_walk_start() {
    for (tick, frame) in [(32, 0), (46, 3), (60, 7), (74, 3), (88, 6), (102, 2)] {
        assert_eq!(super::walk_frame(tick, 22, 8), frame, "tick {tick}");
    }
    // M08: the server-tick clock (rate 256) gives other frames.
    assert_ne!(super::tick_frame(46, 8, 256), 3);
    assert_eq!(super::walk_frame(22, 22, 8), 0);
}
