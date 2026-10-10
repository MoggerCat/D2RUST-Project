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

use std::collections::BTreeMap;

use super::world::{ClientWorld, UnitKey};

/// Half the box width, in screen pixels. d2rs-own, unverified.
pub const HIT_HALF_WIDTH: i32 = 24;
/// Half the box width of an object: its sprite (a waypoint stone, a chest)
/// is wider than a unit's. PROVISIONAL (REC-2116): 1.14d hit-tests the drawn
/// sprite pixels (`0x00467A10`); the Act I waypoint click path of the
/// scene scripts hovers the waypoint 28 px from its feet. d2rs-own, unverified.
pub const HIT_HALF_WIDTH_OBJECT: i32 = 48;
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
        let half = if key.unit_type == 2 {
            HIT_HALF_WIDTH_OBJECT
        } else {
            HIT_HALF_WIDTH
        };
        if dx.abs() > half || !(-HIT_ABOVE..=HIT_BELOW).contains(&dy) {
            continue;
        }
        let d = dx.abs() + dy.abs();
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, *key));
        }
    }
    best.map(|(_, k)| k)
}

/// How far 1.14d's hover test widens a unit's drawn frame rectangle on each
/// side (`0x00470860`: the mouse must lie inside `left − 16 .. right + 16`
/// and `top − 16 .. bottom + 16`).
pub const FRAME_RECT_PAD: i32 = 16;

/// The screen rectangle `(left, top, right, bottom)` of each unit's drawn
/// cels (shadows left out) of one built frame, by unit: an item belongs to
/// the unit whose draw-order slot (`slots`) it carries.
// Spec: specs/tools/scenario-diff.md §3 r8.2 (hover = drawn frame rectangle)
pub fn unit_rects(
    items: &[crate::scene::DrawItem],
    frames: &crate::frames::FrameStore,
    slots: &BTreeMap<UnitKey, crate::rules::draw_order::UnitSlot>,
) -> BTreeMap<UnitKey, (i32, i32, i32, i32)> {
    let mut by_slot: BTreeMap<u64, UnitKey> = BTreeMap::new();
    for (k, s) in slots {
        if let crate::rules::draw_order::UnitSlot::Drawn(o) = s {
            if let Ok(dk) = crate::scene::DrawKey::new(o.pass, o.major, o.minor, 0) {
                by_slot.insert(dk.slot(), *k);
            }
        }
    }
    let mut out: BTreeMap<UnitKey, (i32, i32, i32, i32)> = BTreeMap::new();
    for it in items {
        if !matches!(it.tag, crate::scene::ItemTag::Unit(_))
            || it.key.pass() == crate::scene::order::pass::SHADOWS
        {
            continue;
        }
        let (Some(key), Some(f)) = (by_slot.get(&it.key.slot()), frames.frame(it.frame)) else {
            continue;
        };
        let (l, t) = (it.x, it.y);
        let (r, b) = (l + f.width as i32, t + f.height as i32);
        out.entry(*key)
            .and_modify(|e| *e = (e.0.min(l), e.1.min(t), e.2.max(r), e.3.max(b)))
            .or_insert((l, t, r, b));
    }
    out
}

/// The hover target at `mouse` by the drawn frame rectangles of the last
/// pass ([`unit_rects`]) widened by [`FRAME_RECT_PAD`]; the same units as
/// [`pick`]. PROVISIONAL (REC-2801): the nearest rectangle centre wins; the
/// original's candidate loop `0x00467AC0` ranks by a unit-type priority
/// table `0x00711F98` first.
pub fn pick_rects(
    world: &ClientWorld,
    rects: &BTreeMap<UnitKey, (i32, i32, i32, i32)>,
    mouse: (i32, i32),
) -> Option<UnitKey> {
    let mut best: Option<(i32, UnitKey)> = None;
    for (key, u) in &world.units {
        if !matches!(key.unit_type, 1 | 2 | 4) || Some(*key) == world.local_player {
            continue;
        }
        if key.unit_type == 1 && u.is_dead() {
            continue;
        }
        if key.unit_type == 2
            && world
                .objclient
                .selectable
                .get(u.class as usize)
                .is_some_and(|s| s[(u.mode & 7) as usize] == 0)
        {
            continue;
        }
        let Some(&(l, t, r, b)) = rects.get(key) else {
            continue;
        };
        let (x, y) = mouse;
        if x < l - FRAME_RECT_PAD
            || x >= r + FRAME_RECT_PAD
            || y < t - FRAME_RECT_PAD
            || y >= b + FRAME_RECT_PAD
        {
            continue;
        }
        let d = (x - (l + r) / 2).abs() + (y - (t + b) / 2).abs();
        if best.is_none_or(|(bd, _)| d < bd) {
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

#[cfg(test)]
mod rect_tests {
    use super::*;
    use crate::bridge::world::ClientUnit;

    fn world_with_monster() -> (ClientWorld, UnitKey) {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(1, 7);
        let mut m = ClientUnit::new(key);
        m.mode = 1;
        m.position = Some((10, 10));
        w.units.insert(key, m);
        (w, key)
    }

    // Covers: specs/tools/scenario-diff.md §3 r8.2
    #[test]
    fn the_cursor_hits_a_drawn_frame_rectangle_widened_by_16() {
        let (w, key) = world_with_monster();
        let rects = BTreeMap::from([(key, (100, 100, 140, 180))]);
        assert_eq!(pick_rects(&w, &rects, (84, 100)), Some(key));
        assert_eq!(pick_rects(&w, &rects, (83, 100)), None);
        assert_eq!(pick_rects(&w, &rects, (155, 195)), Some(key));
        assert_eq!(pick_rects(&w, &rects, (156, 195)), None);
        assert_eq!(pick_rects(&w, &rects, (120, 196)), None);
    }

    // Covers: specs/tools/scenario-diff.md §3 r8.2
    #[test]
    fn a_unit_without_a_drawn_rectangle_is_not_hovered() {
        let (w, _) = world_with_monster();
        assert_eq!(pick_rects(&w, &BTreeMap::new(), (120, 120)), None);
    }
}
