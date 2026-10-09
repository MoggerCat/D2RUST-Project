// Spec: specs/render/unit-composite.md §8; specs/skills/bodies-2b.md §8.11
//! Leap's arc and Whirlwind's spin in the play preview, headless on the
//! user's install (`real_rig`): the barbarian's own `skills` rows, her
//! `charstats` speeds. Provisional parts: REC-275 in `docs/HANDOFF.md` §7.

mod real_rig;

mod app_support;

use d2_client::app::play::add_walk;
use d2_client::app::single_player;
use d2_client::bridge::predict::{Speeds, WalkTap};
use d2_client::bridge::BridgeResource;
use d2_client::rules::unit_composite::MotionRecord;
use d2_client::world_view::skill_motion::{leap_record, SkillMotion};
use real_rig::Rig;

/// The install's `skills.txt` `Id`s.
const LEAP: usize = 132;
const WHIRLWIND: usize = 151;

/// The barbarian's walk / run speeds (`charstats`).
fn speeds() -> Speeds {
    let c = single_player::new_character("barbarian", "Test").unwrap();
    single_player::walk_speeds(&app_support::game_data(), &c)
        .unwrap()
        .expect("charstats speeds")
}

fn rig_with_walk(skills: &[usize]) -> Rig {
    let mut r = Rig::new("barbarian", skills);
    add_walk(&mut r.app, WalkTap::default(), Some(speeds()));
    r.leave_town();
    r.strengthen();
    r
}

fn offsets(r: &Rig) -> Option<(i32, i32)> {
    let w = r.app.world();
    let key = w.resource::<BridgeResource>().0.world().local()?.key;
    w.resource::<SkillMotion>().offsets().get(&key).copied()
}

// Covers: specs/render/unit-composite.md §8 r1, §8 r2, §8 r6
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_leap_draw_height_follows_the_timed_arc_frame_by_frame() {
    let mut r = rig_with_walk(&[LEAP]);
    r.select_right(LEAP);
    r.right_click_point(5, 0);
    // The expected curve: the spec's record for the leap's distance,
    // stepped once per server tick.
    let mut want: Vec<(i32, i32)> = Vec::new();
    let mut rec: Option<MotionRecord> = None;
    let mut got: Vec<(i32, i32)> = Vec::new();
    let mut d = 0;
    for _ in 0..60 {
        r.step(1);
        if d == 0 {
            // The distance `0x006417F0` of the request's point.
            let w = r.app.world().resource::<BridgeResource>().0.world();
            let u = w.local().unwrap();
            if let (Some(q), Some(at)) = (u.last_mode_request, u.position) {
                let (dx, dy) = (
                    (q.record[2] - i32::from(at.0)).abs(),
                    (q.record[3] - i32::from(at.1)).abs(),
                );
                d = dx.max(dy) + dx.min(dy) / 2;
            }
        }
        if let Some(o) = offsets(&r) {
            got.push(o);
        }
    }
    assert!(!got.is_empty(), "the leap started an arc ({})", r.errors());
    let mut m = leap_record(d, i32::from(speeds().run)).unwrap();
    while m.flags & 1 == 0 {
        m.update(false, None).unwrap();
        want.push(m.draw_offset());
        rec = Some(m);
    }
    assert!(rec.is_some());
    want.retain(|o| *o != (0, 0));
    got.retain(|o| *o != (0, 0));
    assert_eq!(got, want, "the draw offsets are the spec's curve");
    assert!(got.iter().any(|o| o.1 < 0), "the unit rises");
    assert_eq!(offsets(&r), None, "the arc ends on landing");
}

// Covers: specs/skills/bodies-2b.md §8.11
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn whirlwind_shows_its_skill_mode_while_it_spins() {
    let mut r = rig_with_walk(&[WHIRLWIND]);
    r.select_right(WHIRLWIND);
    r.right_click_point(8, 0);
    // Whirlwind's mode: its client row's `anim` (the install's `skills`).
    let anim = single_player::client_skill_rows(app_support::live().archives.as_ref()).unwrap()
        [WHIRLWIND]
        .anim;
    let mut spun = false;
    for _ in 0..60 {
        r.step(1);
        let w = r.app.world();
        spun |= w
            .resource::<SkillMotion>()
            .spinning()
            .is_some_and(|(_, m)| m == u32::from(anim));
    }
    assert!(spun, "spin mode {anim} shown ({})", r.errors());
    assert_eq!(r.app.world().resource::<SkillMotion>().spinning(), None);
}
