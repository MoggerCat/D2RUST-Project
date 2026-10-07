// Spec: specs/render/draw-order-2.md §15.1, §16
//! The line test `0x0064E260` (§16) and the collision line between two
//! units `0x00622AA0` (§15.1) on the DRLG collision rooms
//! ([`CollisionRooms`]). The server's monster AI, skill bodies and
//! melee range read them; the client's sight test has its own copy of
//! §16 in `d2-client::rules::draw_order::sight`.

use super::collision::{find_room, point_value};
use super::CollisionRooms;
use crate::units::RoomId;

/// Sizes above this read as it (§15.1 rule 3).
pub const SIZE_CAP: i32 = 2;

/// Line test `0x0064E260` (§16): room `room`, `from` → `to` in
/// sub-tiles, cells tested against `mask`. `Err(stop)`: blocked, with
/// the stop cell written back to "to" (`None` when no cell was reached,
/// rule 1); `Ok(())`: clear.
pub fn line_test<R: CollisionRooms + ?Sized>(
    rooms: &R,
    room: Option<RoomId>,
    from: (i32, i32),
    to: (i32, i32),
    mask: u16,
) -> Result<(), Option<(i32, i32)>> {
    // Rule 1.
    let (x0, y0) = from;
    let (x1, y1) = to;
    let mut current = find_room(rooms, Some(room.ok_or(None)?), x0, y0).ok_or(None)?;
    // Rule 3.
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x1 < x0 { -1 } else { 1 };
    let sy = if y1 < y0 { -1 } else { 1 };
    let y_major = dx < dy;
    let (major, minor) = if y_major { (dy, dx) } else { (dx, dy) };
    let (mut x, mut y) = (x0, y0);
    let mut e = 0;
    let mut first = true;
    loop {
        // Rule 4: leaving the current room's rect switches rooms.
        if !first
            && !rooms
                .subtile_rect(current)
                .is_some_and(|r| r.contains(x, y))
        {
            current = find_room(rooms, Some(current), x, y).ok_or(Some((x, y)))?;
        }
        first = false;
        // Rules 2, 5.
        if point_value(rooms, Some(current), x, y, mask) != 0 {
            return Err(Some((x, y)));
        }
        if (y_major && y == y1) || (!y_major && x == x1) {
            return Ok(());
        }
        if y_major {
            y += sy;
        } else {
            x += sx;
        }
        e += minor;
        if e >= major && major > 0 {
            e -= major;
            if y_major {
                x += sx;
            } else {
                y += sy;
            }
        }
    }
}

/// One unit of the collision line (§15.1 rules 1–3): its room
/// (`0x00620BB0`), sub-tile position (`0x0045ADF0` / `0x0045AE20`; (0, 0)
/// without a path) and `sim/path-placement.md` §3 size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineUnit {
    pub room: Option<RoomId>,
    pub x: i32,
    pub y: i32,
    pub size: i32,
}

/// The end pull `0x00622920` (§15.1 rule 5).
fn pull(a: &mut i32, b: &mut i32, sa: i32, sb: i32) {
    if *a < *b {
        *a += sa;
        *b -= sb;
    } else {
        *a -= sa;
        *b += sb;
    }
}

/// `0x00622AA0(a, b, mask)` (§15.1): blocked (true) or clear. The
/// null-unit fatal of rule 1 is the caller's (it has no unit to pass).
pub fn units_line_blocked<R: CollisionRooms + ?Sized>(
    rooms: &R,
    a: &LineUnit,
    b: &LineUnit,
    mask: u16,
) -> bool {
    // Rule 1.
    if a.room.is_none() {
        return false;
    }
    // Rules 2–3.
    let (mut ax, mut ay, mut bx, mut by) = (a.x, a.y, b.x, b.y);
    let sa = a.size.min(SIZE_CAP);
    let sb = b.size.min(SIZE_CAP);
    // Rule 4.
    let dx = bx.wrapping_sub(ax).wrapping_abs();
    let dy = by.wrapping_sub(ay).wrapping_abs();
    if dx.wrapping_add(dy) < sa + sb {
        return false;
    }
    // Rule 5.
    if !(sa == 0 && sb == 0) {
        if dy <= dx {
            pull(&mut ax, &mut bx, sa, sb);
        }
        if dy >= dx {
            pull(&mut ay, &mut by, sa, sb);
        }
    }
    // Rule 6 (the stop cell is discarded).
    line_test(rooms, a.room, (ax, ay), (bx, by), mask).is_err()
}

#[cfg(test)]
mod tests;
