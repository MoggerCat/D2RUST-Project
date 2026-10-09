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
