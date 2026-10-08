// Spec: specs/seams/world-screen.md
//! Contract checks of the world ↔ screen seam: the draw side
//! (`rules::camera`, the world view's feed) and the pick side
//! (`bridge::click::screen_to_world`, `bridge::hover`) convert the same
//! values with the same origin, signs and rounding. No game files.

use d2_client::bridge::click::screen_to_world;
use d2_client::bridge::world::{ClientUnit, ClientWorld, UnitKey, PLAYER};
use d2_client::rules::camera::{
    moving_to_client, static_to_client, Camera, ClientPos, FrameSize, OpenMode,
};
use d2_client::world_view::corpse_click::camera_for;
use d2_client::world_view::preview::Preview;
use d2_client::world_view::{frame_camera, ModelFeed, NoFeed, ViewFeed};

fn centre(c: i32) -> u32 {
    ((c as u32) << 16) | 0x8000
}

fn cameras() -> Vec<Camera> {
    let mut out = Vec::new();
    for mode in 0..=3 {
        let mode = OpenMode::new(mode).unwrap();
        for player in [(0, 0), (1000, 2000), (-37, 411), (16 * 5, 8 * 9 + 8)] {
            let at = ClientPos {
                x: player.0,
                y: player.1,
            };
            out.push(Camera::new(FrameSize::D2RS, mode, at, (0, 0)));
            out.push(Camera::new(FrameSize::D2RS, mode, at, (-3, 2)));
        }
    }
    out
}

// Covers: specs/seams/world-screen.md §2.1
// Covers: specs/seams/world-screen.md §2.3
#[test]
fn moving_unit_drawn_feet_pick_back_to_its_subtile() {
    for cam in cameras() {
        for sx in 90..110 {
            for sy in 90..110 {
                let at = moving_to_client(centre(sx), centre(sy));
                let (x, y) = cam.unit_draw(at, (0, 0));
                assert_eq!(screen_to_world(&cam, x, y), (sx, sy), "{cam:?}");
            }
        }
    }
}

// Covers: specs/seams/world-screen.md §2.1
// Covers: specs/seams/world-screen.md §2.3
#[test]
fn static_unit_draw_point_picks_back_to_its_subtile() {
    for cam in cameras() {
        for sx in 90..110 {
            for sy in 90..110 {
                let (x, y) = cam.unit_draw(static_to_client(sx, sy), (0, 0));
                assert_eq!(screen_to_world(&cam, x, y), (sx, sy), "{cam:?}");
            }
        }
    }
}

// Covers: specs/seams/world-screen.md §2.3
#[test]
fn every_pixel_of_a_subtile_diamond_picks_that_subtile() {
    // The subtile diamond with its top vertex at the static point is 32
    // wide and 16 high (`camera.md` §2: × 16, × 8): the pick floors, so
    // the pixels of one diamond map to one subtile and no pixel is lost.
    let cam = Camera::new(
        FrameSize::D2RS,
        OpenMode::NONE,
        ClientPos::default(),
        (0, 0),
    );
    let (sx, sy) = (7, 3);
    let (x0, y0) = cam.unit_draw(static_to_client(sx, sy), (0, 0));
    let mut hits = 0;
    for dy in 0i32..16 {
        for dx in -16i32..16 {
            if (0..32).contains(&(dx + 2 * dy)) && (0..32).contains(&(2 * dy - dx)) {
                assert_eq!(screen_to_world(&cam, x0 + dx, y0 + dy), (sx, sy));
                hits += 1;
            }
        }
    }
    assert_eq!(hits, 256);
}

// Covers: specs/seams/world-screen.md §2.2
#[test]
fn local_player_draws_mid_play_area_and_picks_its_own_subtile() {
    for mode in 0..=3 {
        let mode = OpenMode::new(mode).unwrap();
        let at = moving_to_client(centre(5000), centre(4000));
        let cam = Camera::new(FrameSize::D2RS, mode, at, (0, 0));
        let (x, y) = cam.unit_draw(at, (0, 0));
        assert_eq!((x, y), (400 + cam.view.shift_x, 292));
        assert_eq!(screen_to_world(&cam, x, y), (5000, 4000));
    }
}

const ME: UnitKey = UnitKey {
    unit_type: PLAYER,
    guid: 1,
};

fn world_at(cell: (u16, u16)) -> ClientWorld {
    let mut w = ClientWorld::default();
    let mut p = ClientUnit::new(ME);
    p.position = Some(cell);
    w.units.insert(ME, p);
    w.local_player = Some(ME);
    w
}

// Covers: specs/seams/world-screen.md §2.2
#[test]
fn without_prediction_the_pick_camera_is_the_draw_camera() {
    let w = world_at((100, 100));
    let mut feed = ModelFeed::<NoFeed>::default().with_preview(Preview::default());
    feed.set_local_prediction(None);
    let draw = frame_camera(&w, &mut feed).unwrap().unwrap();
    assert_eq!(camera_for(&w, 0), Some(draw));
}
