// Spec: specs/client/model.md (§6), specs/sim/unit-order.md (§5 rule 6)
//! The position check `0x004804E0`: compares a point the server states
//! with the unit's own position and, when they disagree beyond the
//! tolerance (or the unit would be drawn off-screen), corrects it: the
//! local player asks the server with C→S 0x5F, other units are moved.

use crate::rules::camera::{moving_to_client, static_to_client};

use super::dispatch::HandlerError;
use super::world::{
    ClientUnit, ClientWorld, ModelInputs, UnitKey, ITEM, MISSILE, MONSTER, OBJECT, PLAYER, TILE,
};

/// What the check did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checked {
    /// x or y is 0, or the unit is dead (rules 1–2), or the unit is not in
    /// the model.
    Skipped,
    /// Within tolerance (or accepted by rule 5) and visible: no change.
    Kept,
    /// The local player: C→S 0x5F appended to `outgoing` (rule 8).
    Asked,
    /// Another unit moved to (x, y) (rule 8).
    Moved,
    /// Rule 8 found no room for (x, y) in the client DRLG: nothing.
    NoRoom,
}

/// `L` of rule 4: `([0x007A04A4] + 0x32) >> 7`, read from the model's
/// ping round trip (`model.md` §7 r11; d2rs sends no ping, so 0).
fn latency(world: &ClientWorld) -> u32 {
    world.ping.rtt.wrapping_add(0x32) >> 7
}

/// Static-path kinds (rule 3): objects, items, tiles.
fn is_static(unit_type: u8) -> bool {
    matches!(unit_type, OBJECT | ITEM | TILE)
}

/// Tolerance T (rule 4).
fn tolerance(world: &ClientWorld, unit: &ClientUnit, kind: u8) -> u32 {
    match kind {
        1 => 10,
        2 => 0,
        _ if world.local_player == Some(unit.key) => {
            let l = latency(world);
            match unit.mode {
                1 => 3 + l,
                3 => 7 + l,
                _ => 5 + l,
            }
        }
        _ if unit.key.unit_type == MONSTER && (3..=5).contains(&unit.mode) => 5,
        _ if unit.key.unit_type == MONSTER && (6..=11).contains(&unit.mode) => 7,
        _ => 15,
    }
}

/// The unit's client pixel point (rule 6: `0x00620650` / `0x006206B0`):
/// the static path's point, or the dynamic path's precise position (the
/// cell centre, model §3 rule 3) projected by `render/camera.md` §2.
fn client_point(unit: &ClientUnit) -> (i32, i32) {
    let (cx, cy) = unit.cell();
    let p = if is_static(unit.key.unit_type) {
        static_to_client(i32::from(cx), i32::from(cy))
    } else {
        moving_to_client(
            (u32::from(cx) << 16) | 0x8000,
            (u32::from(cy) << 16) | 0x8000,
        )
    };
    (p.x, p.y)
}

fn sq(d: i32) -> i64 {
    i64::from(d) * i64::from(d)
}

/// `check(U, x, y, kind, tx, ty)` (§6). `tx`, `ty` are signed as in
/// 1.14d (rule 5 tests `tx > 0`).
#[allow(clippy::too_many_arguments)]
pub fn check(
    world: &mut ClientWorld,
    inputs: &ModelInputs,
    key: UnitKey,
    x: u16,
    y: u16,
    kind: u8,
    tx: i32,
    ty: i32,
) -> Result<Checked, HandlerError> {
    // Rule 1.
    if x == 0 || y == 0 {
        return Ok(Checked::Skipped);
    }
    let Some(unit) = world.units.get_mut(&key) else {
        return Ok(Checked::Skipped);
    };
    // Rule 2.
    if unit.is_dead() {
        return Ok(Checked::Skipped);
    }
    // Rule 3.
    unit.server_point = (x, y);
    let unit = &world.units[&key];
    let (cx, cy) = unit.cell();
    let (xi, yi, cxi, cyi) = (i32::from(x), i32::from(y), i32::from(cx), i32::from(cy));
    // Rule 4.
    let t = tolerance(world, unit, kind);
    // Rule 5.
    let mut far = xi.abs_diff(cxi) > t;
    if far || yi.abs_diff(cyi) > t {
        if kind != 0 || tx <= 0 {
            return Ok(correct(world, key, x, y));
        }
        let d1 = sq(cxi - xi) + sq(cyi - yi);
        if d1 >= 100 {
            return Ok(correct(world, key, x, y));
        }
        let d2 = sq(cxi - tx) + sq(cyi - ty);
        if d2 >= d1 {
            return Ok(correct(world, key, x, y));
        }
        far = false;
    }
    // Rule 6.
    let visible = if x == cx || y == cy {
        true
    } else {
        let pred = inputs.visible.as_ref().ok_or(HandlerError::Unspecified(
            "model.md open question 7: the visibility predicate 0x004DBF20",
        ))?;
        let (a, b) = client_point(unit);
        let p = static_to_client(xi, yi);
        pred.visible(unit, a, b) || pred.visible(unit, p.x, p.y)
    };
    // Rule 7.
    if visible && !far {
        Ok(Checked::Kept)
    } else {
        Ok(correct(world, key, x, y))
    }
}

/// Rule 8: room' := the cell lookup from U's room, else the act lookup
/// (`model.md` §12 rule 2); none → nothing. Without a client DRLG (no
/// DRLG source) the room is taken as found.
fn correct(world: &mut ClientWorld, key: UnitKey, x: u16, y: u16) -> Checked {
    let mut room = None;
    if world.active_rooms.is_some() {
        let start = world.unit_room(key).copied();
        let Some(found) = world.room_from(start.as_ref(), x, y) else {
            return Checked::NoRoom;
        };
        room = Some(found.room);
    }
    if world.local_player == Some(key) {
        // C→S 0x5F with the unit's own position.
        let (cx, cy) = world.units[&key].cell();
        let mut m = vec![0x5F];
        m.extend_from_slice(&cx.to_le_bytes());
        m.extend_from_slice(&cy.to_le_bytes());
        world.outgoing.push(m);
        return Checked::Asked;
    }
    // Another player (`0x00463180`: re-placed, walking modes restarted,
    // Phase 6) or a teleport (`sim/path-placement.md` §6 rule 4): the
    // model's position is (x, y) either way.
    if let Some(u) = world.units.get_mut(&key) {
        u.position = Some((x, y));
    }
    // A dynamic-path unit placed at (x, y) is recached into room'
    // (`sim/unit-order.md` §5 rule 6, `0x0064FAD0`).
    if room.is_some() && matches!(key.unit_type, PLAYER | MONSTER | MISSILE) {
        world.room_units.place(key, room);
    }
    Checked::Moved
}

#[cfg(test)]
pub(crate) fn latency_for_test(world: &ClientWorld) -> u32 {
    latency(world)
}
