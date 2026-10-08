// Spec: specs/ui/controls.md (§6 r2: the hover target `0x00467A10`); preview fill: docs/PLAN.md decision D1
//! The `play` preview's hover target: which unit a world click is on.
//!
//! d2rs-own, unverified. No spec gives the hover model (`0x00467A10`,
//! `client/model.md` hover; the original hit-tests the drawn sprites),
//! so the preview picks the unit whose feet are nearest the mouse,
//! inside a box standing on the feet ([`HIT_HALF_WIDTH`],
//! [`HIT_ABOVE`], [`HIT_BELOW`]). Only monsters, objects and items are
//! picked (never a player: town players need the rest of the hover
//! model); a dead monster is skipped. Ties go to the lower unit key.
//! The strict path never calls this ([`super::click::ClickView::pick`]
//! is `false`).

use crate::rules::camera::{moving_to_client, Camera};

use super::world::{ClientWorld, UnitKey};

/// Half the box width, in screen pixels. d2rs-own, unverified.
pub const HIT_HALF_WIDTH: i32 = 24;
/// How far the box reaches above the feet. d2rs-own, unverified.
pub const HIT_ABOVE: i32 = 96;
/// How far the box reaches below the feet. d2rs-own, unverified.
pub const HIT_BELOW: i32 = 16;

/// The screen point of a unit's feet: its cell centre drawn by the unit
/// rule (`render/camera.md` §4) with no extra offset.
pub fn feet(cam: &Camera, cell: (u16, u16)) -> (i32, i32) {
    let (x, y) = (u32::from(cell.0), u32::from(cell.1));
    cam.unit_draw(
        moving_to_client((x << 16) | 0x8000, (y << 16) | 0x8000),
        (0, 0),
    )
}

/// The hover target at screen point `mouse` (module doc).
pub fn pick(world: &ClientWorld, cam: &Camera, mouse: (i32, i32)) -> Option<UnitKey> {
    let mut best: Option<(i32, UnitKey)> = None;
    for (key, u) in &world.units {
        if !matches!(key.unit_type, 1 | 2 | 4) || Some(*key) == world.local_player {
            continue;
        }
        if key.unit_type == 1 && u.is_dead() {
            continue;
        }
        let Some(cell) = u.position else { continue };
        let (fx, fy) = feet(cam, cell);
        let (dx, dy) = (mouse.0 - fx, mouse.1 - fy);
        if dx.abs() > HIT_HALF_WIDTH || !(-HIT_ABOVE..=HIT_BELOW).contains(&dy) {
            continue;
        }
        let d = dx.abs() + dy.abs();
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, *key));
        }
    }
    best.map(|(_, k)| k)
}
