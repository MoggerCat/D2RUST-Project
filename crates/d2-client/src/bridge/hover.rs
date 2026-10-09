// Spec: specs/ui/controls.md (§6 r2: the hover target `0x00467A10`); preview fill: docs/PLAN.md decision D1
//! The `play` preview's hover target: which unit a world click is on.
//!
//! d2rs-own, unverified. No spec gives the hover model (`0x00467A10`,
//! `client/model.md` hover; the original hit-tests the drawn sprites),
//! so the preview picks the unit whose feet are nearest the mouse,
//! inside a box standing on the feet ([`HIT_HALF_WIDTH`],
//! [`HIT_ABOVE`], [`HIT_BELOW`]). Only monsters, objects and items are
//! picked (never a player: town players need the rest of the hover
//! model); a dead monster and an object whose mode is not `Selectable` (the
//! "Dummy" under a town NPC, REC-1040) are skipped. Ties go to the lower unit key.
//! The strict path never calls this ([`super::click::ClickView::pick`]
//! is `false`).

use crate::rules::camera::{moving_to_client, static_to_client, Camera};

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

/// The screen point of a unit's feet by its type: static units (objects
/// 2, items 4, tiles 5) use the static rule `(sx − sy) × 16, (sx + sy) × 8`
/// they are drawn with, moving units (players 0, monsters 1, missiles 3)
/// the cell centre of [`feet`] (`render/camera.md` §2, §4).
pub fn unit_feet(cam: &Camera, unit_type: u8, cell: (u16, u16)) -> (i32, i32) {
    if matches!(unit_type, 2 | 4 | 5) {
        cam.unit_draw(
            static_to_client(i32::from(cell.0), i32::from(cell.1)),
            (0, 0),
        )
    } else {
        feet(cam, cell)
    }
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
        if key.unit_type == 2
            && world
                .objclient
                .selectable
                .get(u.class as usize)
                .is_some_and(|s| s[(u.mode & 7) as usize] == 0)
        {
            continue;
        }
        let (fx, fy) = unit_feet(cam, key.unit_type, cell);
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

#[cfg(test)]
mod feet_tests {
    use super::*;

    // Covers: specs/render/camera.md §2
    #[test]
    fn static_units_stand_8_rows_above_a_moving_unit_on_the_same_sub_tile() {
        use crate::rules::camera::{FrameSize, OpenMode};
        let at = moving_to_client(100 << 16, 100 << 16);
        let cam = Camera::new(FrameSize::D2RS, OpenMode::NONE, at, (0, 0));
        let (mx, my) = unit_feet(&cam, 1, (103, 100));
        for t in [2, 4] {
            // static: (sx − sy)·16, (sx + sy)·8; moving: the centre
            // (c << 16) | 0x8000 gives (sx + sy)·8 + 8 and the same x.
            assert_eq!(unit_feet(&cam, t, (103, 100)), (mx, my - 8), "type {t}");
        }
        assert_eq!(unit_feet(&cam, 0, (103, 100)), (mx, my));
    }

    // Covers: specs/world/npc.md §2 text
    #[test]
    fn a_non_selectable_object_under_an_npc_is_not_picked() {
        use crate::bridge::world::ClientUnit;
        use crate::rules::camera::{FrameSize, OpenMode};
        let at = moving_to_client(100 << 16, 100 << 16);
        let cam = Camera::new(FrameSize::D2RS, OpenMode::NONE, at, (0, 0));
        let mut w = ClientWorld::default();
        let mut npc = ClientUnit::new(UnitKey::new(1, 7));
        npc.mode = 1;
        npc.position = Some((103, 100));
        w.units.insert(npc.key, npc);
        let mut dummy = ClientUnit::new(UnitKey::new(2, 8));
        dummy.class = 1;
        dummy.position = Some((103, 100));
        w.units.insert(dummy.key, dummy);
        let (x, y) = unit_feet(&cam, 1, (103, 100));
        // A static object stands 8 rows above the NPC's feet: nearer.
        let mouse = (x, y - 8);
        w.objclient.selectable = vec![[1; 8], [0; 8]];
        assert_eq!(pick(&w, &cam, mouse), Some(UnitKey::new(1, 7)));
        w.objclient.selectable = vec![[1; 8], [1; 8]];
        assert_eq!(pick(&w, &cam, mouse), Some(UnitKey::new(2, 8)));
    }
}
