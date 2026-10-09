// Spec: specs/missiles/client.md (§C2–§C4)
//! The client create on synthetic rows.

use super::*;
use crate::bridge::drlg::DrlgRoomId;
use crate::bridge::world::{ActiveRoom, ClientUnit};
use crate::rules::lighting::records::LightKind;

fn rows() -> Vec<ClientMissileRow> {
    vec![
        ClientMissileRow::default(),
        ClientMissileRow {
            vel: 8,
            vel_lev: 4,
            range: 10,
            lev_range: 2,
            activate: 3,
            anim_len: 6,
            anim_speed: 16,
            light: 10,
            rgb: (255, 64, 48),
            ..ClientMissileRow::default()
        },
    ]
}

fn at(x: i32, y: i32) -> CreateRecord {
    CreateRecord {
        flags: flag::POSITION,
        class: 1,
        x,
        y,
        level: 3,
        ..CreateRecord::default()
    }
}

// Covers: specs/missiles/client.md §c2-create-0x004cd540-start-room-target
// Covers: specs/missiles/client.md §c3-create-allocation-and-frames
#[test]
fn a_created_missile_holds_its_frames_velocity_and_target() {
    let mut w = ClientWorld::default();
    // r1: no row → none.
    let mut none = at(100, 100);
    none.class = 9;
    assert_eq!(create(&mut w, &rows(), &none, true).unwrap(), None);
    // Flag 1: start (100, 100); target (x, y) without flag 2 / 0x20.
    let k = create(&mut w, &rows(), &at(100, 100), true)
        .unwrap()
        .unwrap();
    assert_eq!(k.unit_type, MISSILE);
    let u = &w.objclient.set_c[&k];
    assert_eq!((u.class, u.position), (1, Some((100, 100))));
    let m = w.objclient.missiles[&k];
    // r11, r13: F = Range + LevRange × level = 10 + 2 · 3; activate =
    // F − Activate.
    assert_eq!((m.total, m.current, m.activate), (16, 16, 13));
    // r5, r7: (Vel + VelLev · level / 8) << 8 = 9 << 8, then × 75 / 100.
    assert_eq!(m.velocity, (9 << 8) * 75 / 100);
    // r10: length AnimLen << 8, speed AnimSpeed << 4.
    assert_eq!((m.anim_len, m.anim_speed), (6 << 8, 16 << 4));
    assert_eq!(m.target_point, (100, 100));
    // Flag 2: relative target; flag 0x20: absolute.
    let mut rel = at(100, 100);
    rel.flags |= flag::TARGET_RELATIVE;
    (rel.tx, rel.ty) = (3, -2);
    let k = create(&mut w, &rows(), &rel, true).unwrap().unwrap();
    assert_eq!(w.objclient.missiles[&k].target_point, (103, 98));
    // Flag 0x8000 range, 0x800 activate, 0x200 start frame.
    let mut r = at(100, 100);
    r.flags |= flag::RANGE | flag::ACTIVATE | flag::START_FRAME;
    (r.range, r.activate, r.start_frame) = (40, 5, 4);
    let k = create(&mut w, &rows(), &r, true).unwrap().unwrap();
    let m = w.objclient.missiles[&k];
    assert_eq!((m.total, m.activate, m.frame), (36, 31, 4 << 8));
}

// Covers: specs/missiles/client.md §c3-create-allocation-and-frames
#[test]
fn frames_from_the_distance() {
    let mut w = ClientWorld::default();
    let mut r = at(100, 100);
    r.flags |= flag::TARGET_ABSOLUTE | flag::FRAMES_FROM_DISTANCE;
    (r.tx, r.ty) = (110, 104);
    let k = create(&mut w, &rows(), &r, true).unwrap().unwrap();
    let m = w.objclient.missiles[&k];
    // r20: d = max(10, 4) + 4 / 2 = 12; frames = (d << 16) / (v << 4).
    let v = ((9 << 8) * 75 / 100) as u32;
    let n = ((12u32 << 16) / (v << 4)) as i32;
    assert_eq!((m.total, m.current), (n, n));
}

// Covers: specs/missiles/client.md §c4-create-tail
#[test]
fn the_missile_light_follows_quality_flag_and_row() {
    let mut w = ClientWorld::default();
    let k = create(&mut w, &rows(), &at(100, 100), true)
        .unwrap()
        .unwrap();
    let (_, rec) = w.lights.iter().next().unwrap();
    // `lighting.md` §8 missile row: kind 1, radius `Light`, the row's
    // colour; owner the missile (set C).
    assert_eq!(
        (
            rec.owner_type,
            rec.owner_guid,
            rec.lookup_flag,
            rec.kind,
            rec.radius,
            (rec.r, rec.g, rec.b)
        ),
        (3, k.guid, true, LightKind::Plain, 80, (255, 64, 48))
    );
    // Low quality, or flag 0x4000: no light.
    let mut w = ClientWorld::default();
    create(&mut w, &rows(), &at(100, 100), false).unwrap();
    let mut r = at(100, 100);
    r.flags |= flag::NO_LIGHT;
    create(&mut w, &rows(), &r, true).unwrap();
    assert!(w.lights.is_empty());
}

// Covers: specs/missiles/client.md §c2-create-0x004cd540-start-room-target
#[test]
fn no_room_at_the_start_creates_nothing() {
    let mut w = ClientWorld::default();
    let p = UnitKey::new(PLAYER, 1);
    let mut u = ClientUnit::new(p);
    u.position = Some((100, 100));
    w.units.insert(p, u);
    w.local_player = Some(p);
    w.active_rooms = Some(vec![ActiveRoom {
        x0: 90,
        y0: 90,
        w: 20,
        h: 20,
        level: 8,
        room: DrlgRoomId(1),
    }]);
    w.room_units.place(p, Some(DrlgRoomId(1)));
    // r3 (no owner: from the local player's room): in the room → made.
    assert!(create(&mut w, &rows(), &at(95, 95), true)
        .unwrap()
        .is_some());
    // Outside every room → none.
    assert_eq!(create(&mut w, &rows(), &at(300, 300), true).unwrap(), None);
}

fn row(f: u16) -> ClientMissileRow {
    ClientMissileRow {
        range: 4,
        anim_len: 3,
        anim_speed: 16,
        light: 5,
        rgb: (1, 2, 3),
        clt_do_func: f,
        ..ClientMissileRow::default()
    }
}

/// A world with one missile of `rows[1]` at (100, 100).
fn made(rows: &[ClientMissileRow]) -> (ClientWorld, UnitKey) {
    let mut w = ClientWorld::default();
    let mut r = at(100, 100);
    r.level = 0;
    let k = create(&mut w, rows, &r, true).unwrap().unwrap();
    (w, k)
}

// Covers: specs/missiles/client.md §c7-default-step-0x004d30c0-function-1
// Covers: specs/missiles/client.md §c9-end-0x004d2d70-m-u-forced
#[test]
fn the_default_step_animates_counts_down_and_ends() {
    let rows = vec![ClientMissileRow::default(), row(FN_DEFAULT_STEP)];
    let (mut w, k) = made(&rows);
    // F = 4, activate = 4 (`Activate` 0): active from the start; speed
    // 16 << 4 = 256 per update, length 3 << 8, no loop: the frame holds
    // at the end.
    let frames = |w: &ClientWorld| w.objclient.missiles.get(&k).map(|m| (m.current, m.frame));
    update(&mut w, &rows, k, true).unwrap();
    assert_eq!(frames(&w), Some((3, 256)));
    update(&mut w, &rows, k, true).unwrap();
    update(&mut w, &rows, k, true).unwrap();
    assert_eq!(frames(&w), Some((1, 512)), "frame + speed ≥ length: held");
    // Frames left reach 0: end(none, 0): the missile removed, its light
    // dying (§C9 r7–r8).
    update(&mut w, &rows, k, true).unwrap();
    assert!(!w.objclient.set_c.contains_key(&k) && !w.objclient.missiles.contains_key(&k));
    let (_, light) = w.lights.iter().next().unwrap();
    assert!(light.dying && light.target == 0);
}

// Covers: specs/missiles/client.md §c13-function-bodies-specified-here
#[test]
fn the_den_light_never_expires_and_function_11_lies_flat() {
    let rows = vec![ClientMissileRow::default(), row(FN_DEN_LIGHT)];
    let (mut w, k) = made(&rows);
    for _ in 0..600 {
        update(&mut w, &rows, k, true).unwrap();
    }
    // 23: frames left < 100 → 500 before each step.
    let m = w.objclient.missiles[&k];
    assert!(m.current >= 99 && m.current <= 500, "{}", m.current);
    assert_eq!(w.lights.len(), 1);
    let rows = vec![ClientMissileRow::default(), row(FN_FLAT_AT_END)];
    let (mut w, k) = made(&rows);
    update(&mut w, &rows, k, true).unwrap();
    assert!(!w.objclient.missiles[&k].flat);
    update(&mut w, &rows, k, true).unwrap();
    // At the end (frame 512 + 256 ≥ 768): flag 0x10000, light removed,
    // no countdown: the missile stays.
    update(&mut w, &rows, k, true).unwrap();
    let m = w.objclient.missiles[&k];
    assert!(m.flat);
    assert!(w.lights.is_empty());
    let left = m.current;
    for _ in 0..10 {
        update(&mut w, &rows, k, true).unwrap();
    }
    assert_eq!(w.objclient.missiles[&k].current, left);
}

// Covers: specs/missiles/client.md §c6-per-update-dispatch-0x004d2c70
#[test]
fn no_row_removes_and_init_steps_hide_until_active() {
    let rows = vec![ClientMissileRow::default(), row(FN_DEFAULT_STEP)];
    let (mut w, k) = made(&rows);
    update(&mut w, &[], k, true).unwrap();
    assert!(!w.objclient.set_c.contains_key(&k) && w.lights.is_empty());
    // `InitSteps` 2: not drawn until elapsed > 2.
    let mut r = row(FN_DEFAULT_STEP);
    (r.init_steps, r.range) = (2, 10);
    let rows = vec![ClientMissileRow::default(), r];
    let (mut w, k) = made(&rows);
    let hidden = |w: &ClientWorld| w.objclient.set_c[&k].flag_ex & FLAG_EX_NOT_DRAWN != 0;
    assert!(hidden(&w));
    for _ in 0..3 {
        update(&mut w, &rows, k, true).unwrap();
        assert!(hidden(&w));
    }
    update(&mut w, &rows, k, true).unwrap();
    assert!(!hidden(&w), "elapsed 3 > InitSteps 2");
}

// Covers: specs/missiles/client.md §c9-end-0x004d2d70-m-u-forced
#[test]
fn always_explode_makes_the_explosion_missile_with_the_owners_aim() {
    let mut r = row(FN_DEFAULT_STEP);
    (r.always_explode, r.explosion_missile) = (true, 2);
    let mut boom = row(FN_DEFAULT_STEP);
    boom.explosion_missile = -1;
    let rows = vec![ClientMissileRow::default(), r, boom];
    let mut w = ClientWorld::default();
    let p = UnitKey::new(PLAYER, 1);
    let mut u = ClientUnit::new(p);
    u.position = Some((100, 100));
    w.units.insert(p, u);
    // An owner aiming at its own cell (no target) without velocity: no
    // nudge (r8 runs only with v ≠ 0).
    let mut rec = at(100, 100);
    rec.owner = Some(p);
    rec.flags |= flag::RANDOM_DIRECTION;
    let k = create(&mut w, &rows, &rec, true).unwrap().unwrap();
    let dir = w.objclient.missiles[&k].direction;
    let x = end(&mut w, &rows, k, false, true)
        .unwrap()
        .expect("exploded");
    let xm = w.objclient.missiles[&x];
    assert_eq!(
        (w.objclient.set_c[&x].class, xm.owner, xm.direction),
        (2, Some(p), dir)
    );
    assert!(!w.objclient.set_c.contains_key(&k));
}

// Covers: specs/missiles/client.md §c2-create-0x004cd540-start-room-target
#[test]
fn the_aim_nudge_steps_off_the_owners_cell_by_its_direction() {
    let rows = rows();
    let mut w = ClientWorld::default();
    let p = UnitKey::new(PLAYER, 1);
    let mut u = ClientUnit::new(p);
    u.position = Some((100, 100));
    w.units.insert(p, u);
    let mut rec = at(100, 100);
    rec.owner = Some(p);
    // Without the owner's direction the nudge cannot run.
    assert!(create(&mut w, &rows, &rec, true).is_err());
    // dir64 20 → d = 2: (DX, DY) = (−2, 0).
    rec.owner_dir64 = Some(20);
    let k = create(&mut w, &rows, &rec, true).unwrap().unwrap();
    assert_eq!(w.objclient.missiles[&k].target_point, (98, 100));
}
